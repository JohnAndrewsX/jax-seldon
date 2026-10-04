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
use crate::redact::Redactor;

#[derive(Debug, Clone, Args)]
pub struct PlanArgs {
    #[command(subcommand)]
    pub command: PlanCommand,
}

#[derive(Debug, Clone, Subcommand)]
pub enum PlanCommand {
    /// Create a case in work/queued/
    New(NewArgs),
    /// Start a case: queued → active; it becomes the active case
    Start(StartArgs),
    /// Hand an active case to verification: active → verification
    Verify(StepArgs),
    /// Complete a verified case: verification → completed
    Done(StepArgs),
    /// Drop a case that is queued, active or in verification
    Drop(StepArgs),
    /// List cases, optionally by status or area
    List(ListArgs),
    /// Print one case file with its path
    Show {
        /// The case id, e.g. C-2026-004
        #[arg(value_name = "ID", value_parser = parse_case_id)]
        id: String,
    },
}

#[derive(Debug, Clone, Args)]
pub struct NewArgs {
    /// The case title, as one argument (after `--` when it starts with `-`)
    #[arg(value_name = "TITLE", allow_hyphen_values = true)]
    pub title: String,

    /// green, yellow or red
    #[arg(long, value_name = "ZONE", default_value = "yellow")]
    pub zone: Zone,

    /// R0 to R3
    #[arg(long, value_name = "RISK", default_value = "R1")]
    pub risk: Risk,

    /// Area slug; created under areas/ on first use
    #[arg(long, value_name = "AREA")]
    pub area: Option<String>,

    /// high, normal or low
    #[arg(long, value_name = "PRIORITY", default_value = "normal")]
    pub priority: Priority,

    /// Who creates the case: human or agent:NAME
    #[arg(long, value_name = "ACTOR", default_value = "human", value_parser = parse_person)]
    pub actor: String,
}

/// `plan start`: a step that can name the snapshot taken before the work.
#[derive(Debug, Clone, Args)]
pub struct StartArgs {
    #[command(flatten)]
    pub step: StepArgs,

    /// Snapper snapshot number taken before the work, e.g. 42 (an R2 or R3
    /// case started without one gets a warning, ADR-0023)
    #[arg(long, value_name = "NUMBER")]
    pub snapshot: Option<u64>,
}

#[derive(Debug, Clone, Args)]
pub struct StepArgs {
    /// The case id, e.g. C-2026-004
    #[arg(value_name = "ID", value_parser = parse_case_id)]
    pub id: String,

    /// Why, in one line; goes into the Log line and the event detail
    #[arg(long, value_name = "TEXT", allow_hyphen_values = true)]
    pub reason: Option<String>,

    /// Who takes the step: human or agent:NAME
    #[arg(long, value_name = "ACTOR", default_value = "human", value_parser = parse_person)]
    pub actor: String,
}

#[derive(Debug, Clone, Args)]
pub struct ListArgs {
    /// Only cases with this status (queued, active, verification, completed, dropped)
    #[arg(long, value_name = "STATUS")]
    pub status: Option<CaseStatus>,

    /// Only cases in this area
    #[arg(long, value_name = "AREA")]
    pub area: Option<String>,
}

pub fn run(ctx: &Context, args: PlanArgs) -> Result<Output> {
    match args.command {
        PlanCommand::New(a) => new(ctx, a),
        PlanCommand::Start(a) => step(ctx, Transition::Start, a.step, a.snapshot),
        PlanCommand::Verify(a) => step(ctx, Transition::Verify, a, None),
        PlanCommand::Done(a) => step(ctx, Transition::Done, a, None),
        PlanCommand::Drop(a) => step(ctx, Transition::Drop, a, None),
        PlanCommand::List(a) => list(ctx, a),
        PlanCommand::Show { id } => show(ctx, &id),
    }
}

fn new(ctx: &Context, args: NewArgs) -> Result<Output> {
    let title = one_line("the title", &args.title)?;
    let (config, logbook) = ctx.open_logbook()?;
    // the case file, its name, STATUS.md and the ledger get the redacted title
    let title = Redactor::for_config(&config)?.redact(&title);
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
    crate::index::rebuild_if_initialised(ctx);
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

/// One step; `snapshot` only comes with `plan start` (clap has no
/// `--snapshot` on the other steps).
fn step(
    ctx: &Context,
    transition: Transition,
    args: StepArgs,
    snapshot: Option<u64>,
) -> Result<Output> {
    let reason = args
        .reason
        .as_deref()
        .map(|r| one_line("--reason", r))
        .transpose()?;
    let (config, logbook) = ctx.open_logbook()?;
    let redactor = Redactor::for_config(&config)?;
    let reason = reason.map(|r| redactor.redact(&r));
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
            if snapshot.is_some() {
                file.case.snapshot_before = snapshot;
            }
        }
        Transition::Done | Transition::Drop => file.case.closed = Some(today),
        Transition::Verify => {}
    }
    file.add_agent(&args.actor);
    let mut line = transition.log_word().to_string();
    if let Some(n) = snapshot {
        line.push_str(&format!(" (snapshot {n})"));
    }
    if let Some(r) = &reason {
        line.push_str(": ");
        line.push_str(r);
    }
    file.log(&ctx.now, &line, &args.actor);
    let stub = (transition == Transition::Done).then(|| {
        redactor.redact(&match logbook.meta.language {
            Language::En => format!("Case completed: {}", file.case.title),
            Language::De => format!("Case abgeschlossen: {}", file.case.title),
        })
    });

    // the journal day is read before the ledger is written: a day file
    // the engine cannot read fails the step before anything changes
    // (WP-057)
    let journal_entry = stub
        .as_deref()
        .map(|text| journal::prepare(&logbook, &ctx.now, &args.actor, Some(&args.id), text))
        .transpose()?;

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
    let journal_entry = match journal_entry {
        Some(pending) => Some(pending.write()?.path),
        None => None,
    };
    let commit = autocommit(ctx, &config, &logbook, &format!("{} {}", args.id, to));
    crate::index::rebuild_if_initialised(ctx);
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
    let warnings: Vec<String> = (transition == Transition::Start)
        .then(|| snapshot_warning(&file.case))
        .flatten()
        .into_iter()
        .collect();
    human.push_str(&super::index::warnings_human(&warnings));
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
            "warnings": warnings,
        }),
    ))
}

/// The advice for a case started without a snapshot (ADR-0023): R2 and R3
/// want one, R3 also the human's explicit go per step. Advice only; the
/// step is never refused.
fn snapshot_warning(case: &Case) -> Option<String> {
    if case.snapshot_before.is_some() {
        return None;
    }
    let id = &case.id;
    match case.risk {
        Risk::R0 | Risk::R1 => None,
        Risk::R2 => Some(format!(
            "{id} is risk R2 and was started without --snapshot: its rollback needs a \
             snapshot or backup; take one before the first change and note it in the case \
             (ADR-0023)"
        )),
        Risk::R3 => Some(format!(
            "{id} is risk R3 and was started without --snapshot: a snapshot is mandatory \
             before the first change, and the work needs the human's explicit go per step, \
             never unattended (ADR-0023)"
        )),
    }
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
    // a case file that does not load is a warning: the others are listed
    let (files, warnings) = cases::all(&logbook)?;
    let files: Vec<CaseFile> = files
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
    let mut human = human.trim_end().to_string();
    human.push_str(&super::index::warnings_human(&warnings));
    Ok(Output::ok(
        human,
        json!({
            "cases": files.iter().map(|f| case_json(&logbook, f)).collect::<Vec<_>>(),
            "warnings": warnings,
        }),
    ))
}

/// `plan show`: the case file's path and text as quoted lines (`> `, as
/// `hook session-start` prints logbook text), under a line that says what
/// they are; `--json` gives them unquoted.
fn show(ctx: &Context, id: &str) -> Result<Output> {
    let (_, logbook) = ctx.open_logbook()?;
    let file = cases::find(&logbook, id)?;
    let mut human = format!(
        "Case {id}: its file's path and text.\n{}\n",
        super::hook::DATA_NOTE
    );
    super::hook::quote(&mut human, &file.relative(&logbook));
    super::hook::quote(&mut human, file.doc.render().trim_end());
    let human = human.trim_end().to_string();
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
