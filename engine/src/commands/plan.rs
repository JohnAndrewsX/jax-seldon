//! `seldon plan new|start|verify|done|drop|list|show` (SPEC-ENGINE §3): the
//! case lifecycle (SPEC-LOGBOOK §3, ADR-0012 §9).
//!
//! Every step changes frontmatter keys losslessly, appends one line to the
//! case's `## Log`, moves the file to its status folder, and records a
//! `seldon/case-*` event. The case body is otherwise never touched.

use chrono::Datelike as _;
use clap::{Args, Subcommand};
use serde_json::{Value, json};

use super::event::{clip, emit_one, event_json, parse_case_id, parse_person};
use super::{Context, Output, autocommit, one_line, write_new};
use crate::error::{Error, Result};
use crate::logbook::cases::{self, CaseFile, Transition};
use crate::logbook::{Logbook, journal};
use crate::model::event::{Event, Kind, Source};
use crate::model::{Case, CaseStatus, Language, Priority, Risk, Zone};

#[derive(Debug, Clone, Args)]
pub struct PlanArgs {
    #[command(subcommand)]
    pub command: PlanCommand,
}

#[derive(Debug, Clone, Subcommand)]
pub enum PlanCommand {
    /// Create a case in work/queued/
    New(NewArgs),
    /// queued → active (writes .seldon/active-case)
    Start(StepArgs),
    /// active → verification
    Verify(StepArgs),
    /// verification → completed
    Done(StepArgs),
    /// queued, active or verification → dropped
    Drop(StepArgs),
    /// List cases
    List(ListArgs),
    /// Show one case
    Show {
        #[arg(value_name = "ID", value_parser = parse_case_id)]
        id: String,
    },
}

#[derive(Debug, Clone, Args)]
pub struct NewArgs {
    /// The case title, as one argument
    #[arg(value_name = "TITLE", allow_hyphen_values = true)]
    pub title: String,

    /// green, yellow or red
    #[arg(long, value_name = "Z", default_value = "yellow")]
    pub zone: Zone,

    /// R0 to R3
    #[arg(long, value_name = "R", default_value = "R1")]
    pub risk: Risk,

    /// Area slug; created under areas/ on first use
    #[arg(long, value_name = "A")]
    pub area: Option<String>,

    /// high, normal or low
    #[arg(long, value_name = "P", default_value = "normal")]
    pub priority: Priority,

    /// Who creates the case
    #[arg(long, value_name = "A", default_value = "human", value_parser = parse_person)]
    pub actor: String,
}

#[derive(Debug, Clone, Args)]
pub struct StepArgs {
    #[arg(value_name = "ID", value_parser = parse_case_id)]
    pub id: String,

    /// Snapper snapshot taken before the work (`plan start` only)
    #[arg(long, value_name = "N")]
    pub snapshot: Option<u64>,

    /// Why (one line; goes into the Log line and the event detail)
    #[arg(long, value_name = "TEXT", allow_hyphen_values = true)]
    pub reason: Option<String>,

    /// Who takes the step
    #[arg(long, value_name = "A", default_value = "human", value_parser = parse_person)]
    pub actor: String,
}

#[derive(Debug, Clone, Args)]
pub struct ListArgs {
    /// Only cases with this status
    #[arg(long, value_name = "S")]
    pub status: Option<CaseStatus>,

    /// Only cases in this area
    #[arg(long, value_name = "A")]
    pub area: Option<String>,
}

pub fn run(ctx: &Context, args: PlanArgs) -> Result<Output> {
    match args.command {
        PlanCommand::New(a) => new(ctx, a),
        PlanCommand::Start(a) => step(ctx, Transition::Start, a),
        PlanCommand::Verify(a) => step(ctx, Transition::Verify, a),
        PlanCommand::Done(a) => step(ctx, Transition::Done, a),
        PlanCommand::Drop(a) => step(ctx, Transition::Drop, a),
        PlanCommand::List(a) => list(ctx, a),
        PlanCommand::Show { id } => show(ctx, &id),
    }
}

fn new(ctx: &Context, args: NewArgs) -> Result<Output> {
    let title = one_line("the title", &args.title)?;
    let (config, logbook) = ctx.open_logbook()?;
    let lock = ctx.lock()?;

    if let Some(area) = args.area.as_deref()
        && !crate::model::is_slug(area)
    {
        return Err(Error::user(format!(
            "area `{area}` is not a lowercase slug ([a-z0-9][a-z0-9-]*)"
        )));
    }
    let today = ctx.now.date_naive();
    let id = cases::next_id(&logbook, ctx.now.year())?;
    let case = Case {
        id: id.clone(),
        title: title.clone(),
        status: CaseStatus::Queued,
        zone: args.zone,
        risk: args.risk,
        priority: Some(args.priority),
        area: args.area.clone(),
        created: today,
        started: None,
        closed: None,
        snapshot_before: None,
        agents: Vec::new(),
        events: Vec::new(),
        tags: Vec::new(),
    };
    let body = cases::new_body(&logbook, &id, &title)?;
    let path = logbook
        .path("work")
        .join(CaseStatus::Queued.folder())
        .join(case.file_name(&cases::slug(&title, "case")));
    let mut file = CaseFile {
        path,
        case,
        doc: crate::frontmatter::Document {
            frontmatter: None,
            body,
        },
    };
    file.add_agent(&args.actor);
    file.log(
        &ctx.now,
        &format!("created (zone {}, risk {})", args.zone, args.risk),
        &args.actor,
    );
    let text = crate::model::render_new(&file.case, &file.doc.body);
    if file.path.exists() {
        return Err(Error::user(format!(
            "{} already exists",
            file.path.display()
        )));
    }

    // the ledger first: if it cannot be written, nothing else is
    let event = Event::new(ctx.now, Source::Seldon, Kind::CaseCreated, &id)
        .detail(title.clone())
        .actor(&args.actor)
        .case(Some(id.clone()));
    let event = emit_one(&lock, &config, &logbook, event)?;
    let area_created = args
        .area
        .as_deref()
        .map(|a| cases::ensure_area(&logbook, a))
        .transpose()?
        .flatten();
    write_new(&file.path, &text)?;
    let commit = autocommit(ctx, &config, &logbook, &format!("{id} created"));
    drop(lock);

    let mut human = format!("Created {id} \"{title}\" in {}", file.relative(&logbook));
    if let Some(area) = &area_created {
        human.push_str(&format!("\nNew area: {area}"));
    }
    human.push_str(&commit.human());
    Ok(Output::ok(
        human,
        json!({
            "case": case_json(&logbook, &file),
            "event": event_json(&event),
            "areaCreated": area_created,
            "git": commit.json(),
        }),
    ))
}

fn step(ctx: &Context, transition: Transition, args: StepArgs) -> Result<Output> {
    if args.snapshot.is_some() && transition != Transition::Start {
        return Err(Error::user("--snapshot only goes with `seldon plan start`"));
    }
    let reason = args
        .reason
        .as_deref()
        .map(|r| one_line("--reason", r))
        .transpose()?;
    let (config, logbook) = ctx.open_logbook()?;
    let lock = ctx.lock()?;
    let mut file = cases::find(&logbook, &args.id)?;
    let from = file.case.status;
    let to = transition
        .target(from)
        .map_err(|e| Error::user(format!("{} {e}", args.id)))?;

    let today = ctx.now.date_naive();
    file.case.status = to;
    match transition {
        Transition::Start => {
            file.case.started = Some(today);
            if args.snapshot.is_some() {
                file.case.snapshot_before = args.snapshot;
            }
        }
        Transition::Done | Transition::Drop => file.case.closed = Some(today),
        Transition::Verify => {}
    }
    file.add_agent(&args.actor);
    let mut line = transition.log_word().to_string();
    if let Some(n) = args.snapshot {
        line.push_str(&format!(" (snapshot {n})"));
    }
    if let Some(r) = &reason {
        line.push_str(": ");
        line.push_str(r);
    }
    file.log(&ctx.now, &line, &args.actor);
    let stub = (transition == Transition::Done).then(|| match logbook.meta.language {
        Language::En => format!("Case completed: {}", file.case.title),
        Language::De => format!("Case abgeschlossen: {}", file.case.title),
    });

    // the ledger first: if it cannot be written, the case is not moved,
    // the marker and the journal stay as they are
    let mut event = Event::new(ctx.now, Source::Seldon, kind(transition), &args.id)
        .actor(&args.actor)
        .case(Some(args.id.clone()));
    event.detail = reason.clone();
    let event = emit_one(&lock, &config, &logbook, event)?;

    let moved_from = file.save(&logbook)?.map(|p| cases::relative(&logbook, &p));
    let active_case = match transition {
        Transition::Start => {
            cases::set_active_case(&logbook, &args.id)?;
            json!({ "set": args.id })
        }
        Transition::Done | Transition::Drop => {
            json!({ "cleared": cases::clear_active_case(&logbook, &args.id)? })
        }
        Transition::Verify => Value::Null,
    };
    let journal_entry = match &stub {
        Some(text) => {
            Some(journal::append(&logbook, &ctx.now, &args.actor, Some(&args.id), text)?.path)
        }
        None => None,
    };
    let commit = autocommit(ctx, &config, &logbook, &format!("{} {}", args.id, to));
    drop(lock);

    let mut human = format!(
        "{} {from} → {to}{}",
        args.id,
        moved_from
            .as_ref()
            .map(|_| format!(" (now {})", file.relative(&logbook)))
            .unwrap_or_default()
    );
    if let Some(path) = &journal_entry {
        human.push_str(&format!("\nJournal: {path}"));
    }
    human.push_str(&commit.human());
    Ok(Output::ok(
        human,
        json!({
            "case": case_json(&logbook, &file),
            "from": from,
            "to": to,
            "movedFrom": moved_from,
            "activeCase": active_case,
            "journal": journal_entry,
            "event": event_json(&event),
            "git": commit.json(),
        }),
    ))
}

/// The ledger kind of a step (`event.schema.json`).
fn kind(transition: Transition) -> Kind {
    match transition {
        Transition::Start => Kind::CaseStarted,
        Transition::Verify => Kind::CaseVerified,
        Transition::Done => Kind::CaseCompleted,
        Transition::Drop => Kind::CaseDropped,
    }
}

fn list(ctx: &Context, args: ListArgs) -> Result<Output> {
    let (_, logbook) = ctx.open_logbook()?;
    let files: Vec<CaseFile> = cases::all(&logbook)?
        .into_iter()
        .filter(|f| args.status.is_none_or(|s| f.case.status == s))
        .filter(|f| {
            args.area
                .as_deref()
                .is_none_or(|a| f.case.area.as_deref() == Some(a))
        })
        .collect();
    let mut human = String::new();
    for f in &files {
        let c = &f.case;
        human.push_str(&format!(
            "{}  {:<12}  {:<6} {}  {:<10}  {}\n",
            c.id,
            c.status.as_str(),
            c.zone.as_str(),
            c.risk,
            c.area.as_deref().unwrap_or("-"),
            clip(&c.title, 60)
        ));
    }
    if files.is_empty() {
        human.push_str("No cases.");
    }
    Ok(Output::ok(
        human.trim_end(),
        json!({ "cases": files.iter().map(|f| case_json(&logbook, f)).collect::<Vec<_>>() }),
    ))
}

fn show(ctx: &Context, id: &str) -> Result<Output> {
    let (_, logbook) = ctx.open_logbook()?;
    let file = cases::find(&logbook, id)?;
    let human = format!(
        "{}\n{}",
        file.relative(&logbook),
        file.doc.render().trim_end()
    );
    let mut json = json!({ "case": case_json(&logbook, &file) });
    json["body"] = Value::String(file.doc.body.clone());
    json["activeCase"] = Value::Bool(cases::active_case(&logbook).as_deref() == Some(id));
    Ok(Output::ok(human, json))
}

/// A case in the shape of `case.schema.json` as the index has it: the
/// frontmatter (without `type`), plus `path` and `steps`.
pub fn case_json(logbook: &Logbook, file: &CaseFile) -> Value {
    let c = &file.case;
    let (total, done) = file.steps();
    let mut v = json!({
        "id": c.id,
        "title": c.title,
        "status": c.status,
        "zone": c.zone,
        "risk": c.risk,
        "created": c.created.to_string(),
        "started": c.started.map(|d| d.to_string()),
        "closed": c.closed.map(|d| d.to_string()),
        "snapshotBefore": c.snapshot_before,
        "agents": c.agents,
        "events": c.events,
        "tags": c.tags,
        "path": file.relative(logbook),
        "steps": { "total": total, "done": done },
    });
    if let Some(p) = c.priority {
        v["priority"] = json!(p);
    }
    if let Some(a) = &c.area {
        v["area"] = json!(a);
    }
    v
}
