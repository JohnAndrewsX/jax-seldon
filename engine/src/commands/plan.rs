//! `seldon plan new|start|verify|done|drop|list|show` (SPEC-ENGINE §3): the
//! case lifecycle (SPEC-LOGBOOK §3, ADR-0012 §9).
//!
//! Every step changes frontmatter keys losslessly, appends one line to the
//! case's `## Log`, moves the file to its status folder, and records a
//! `seldon/case-*` event. The case body is otherwise never touched.

use chrono::Datelike as _;
use clap::{Args, Subcommand};
use serde_json::{Value, json};

use super::event::{
    ACTOR_ENV, actor_or_env, clip, emit, emit_one, env_actor, event_json, parse_case_id,
    parse_person,
};
use super::{Context, Output, autocommit, one_line, write_new};
use crate::error::{Error, Result};
use crate::logbook::cases::{self, CaseFile, Transition};
use crate::logbook::{Logbook, journal};
use crate::model::event::{ACTOR_HUMAN, Event, Kind, Source};
use crate::model::{Case, CaseStatus, Language, Priority, Risk, Zone, is_agent};
use crate::redact::Redactor;

pub(crate) mod snapshot;

/// The tag of a case an agent closed (ADR-0027 §5; CONTRACT.md lists the
/// reserved tags).
pub const TAG_CLOSED_BY_AGENT: &str = "closed-by-agent";

/// The tag prefix of a case `plan reopen` made: `reopens:<ID>`.
pub const TAG_REOPENS: &str = "reopens:";

/// The tag of a case `seldon import task` made (WP-102).
pub const TAG_IMPORTED: &str = "imported";

/// An imported case is started by the user (WP-102 round 3, orchestrator
/// decision; ADR-0027 §2(a)): its Intent is text from a file, which becomes
/// an agent's authorisation only by the user's start. Refused when `actor`
/// or the session (`$SELDON_ACTOR`) is an agent, whatever `--actor` says.
pub(crate) fn refuse_agent_start_of_imported(file: &CaseFile, actor: &str) -> Result<()> {
    if !file.case.tags.iter().any(|t| t == TAG_IMPORTED) {
        return Ok(());
    }
    let session = env_actor(parse_person).ok().flatten();
    let agent = Some(actor)
        .filter(|a| is_agent(a))
        .or(session.as_deref().filter(|s| is_agent(s)));
    match agent {
        Some(agent) => Err(Error::user(format!(
            "{} is not started: an imported case is started by the user (ADR-0027 §2a); ask them to start it ({agent}'s session)",
            file.case.id
        ))),
        None => Ok(()),
    }
}

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
    /// Hand an active case to verification: active → verification (runs a
    /// capture first)
    Verify(CloseArgs),
    /// Complete a verified case: verification → completed (runs a capture
    /// first)
    Done(CloseArgs),
    /// Drop a case that is queued, active or in verification
    Drop(StepArgs),
    /// Change an open case's zone, risk or area, e.g. raise it to R3 before
    /// a step that can break boot
    Set(SetArgs),
    /// Record the snapper snapshot taken before the case's first red
    /// change as its rollback (checked, never refused)
    Snapshot(SnapshotArgs),
    /// Reopen a completed case: a new active case "Reopen: <title>" with
    /// the same Intent
    Reopen(ReopenArgs),
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

    /// Who creates the case: human or agent:NAME (default: $SELDON_ACTOR,
    /// else human)
    #[arg(long, value_name = "ACTOR", value_parser = parse_person)]
    pub actor: Option<String>,
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

    /// Who takes the step: human or agent:NAME (default: $SELDON_ACTOR,
    /// else human)
    #[arg(long, value_name = "ACTOR", value_parser = parse_person)]
    pub actor: Option<String>,
}

/// `plan verify|done`: a step that captures first (ADR-0029 §2).
#[derive(Debug, Clone, Args)]
pub struct CloseArgs {
    #[command(flatten)]
    pub step: StepArgs,

    /// Do not run `seldon capture` before the step
    #[arg(long)]
    pub no_capture: bool,
}

/// `plan set`: at least one of zone, risk and area.
#[derive(Debug, Clone, Args)]
#[command(group(clap::ArgGroup::new("change").required(true).multiple(true).args(["zone", "risk", "area"])))]
pub struct SetArgs {
    /// The case id, e.g. C-2026-004
    #[arg(value_name = "ID", value_parser = parse_case_id)]
    pub id: String,

    /// green, yellow or red
    #[arg(long, value_name = "ZONE")]
    pub zone: Option<Zone>,

    /// R0 to R3
    #[arg(long, value_name = "RISK")]
    pub risk: Option<Risk>,

    /// Area slug; created under areas/ on first use
    #[arg(long, value_name = "AREA")]
    pub area: Option<String>,

    /// Who changes it: human or agent:NAME (default: $SELDON_ACTOR, else
    /// human)
    #[arg(long, value_name = "ACTOR", value_parser = parse_person)]
    pub actor: Option<String>,
}

#[derive(Debug, Clone, Args)]
pub struct SnapshotArgs {
    /// The case id, e.g. C-2026-004
    #[arg(value_name = "ID", value_parser = parse_case_id)]
    pub id: String,

    /// The snapper snapshot number, e.g. 42 (`snapper create -p` prints it)
    #[arg(value_name = "NUMBER", value_parser = clap::value_parser!(u64).range(1..))]
    pub number: u64,

    /// Who records it: human or agent:NAME (default: $SELDON_ACTOR, else
    /// human)
    #[arg(long, value_name = "ACTOR", value_parser = parse_person)]
    pub actor: Option<String>,
}

#[derive(Debug, Clone, Args)]
pub struct ReopenArgs {
    /// The completed case, e.g. C-2026-004
    #[arg(value_name = "ID", value_parser = parse_case_id)]
    pub id: String,

    /// Who reopens it: human or agent:NAME (default: $SELDON_ACTOR, else
    /// human)
    #[arg(long, value_name = "ACTOR", value_parser = parse_person)]
    pub actor: Option<String>,
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
        PlanCommand::Start(a) => step(ctx, Transition::Start, a.step, a.snapshot, false),
        PlanCommand::Verify(a) => step(ctx, Transition::Verify, a.step, None, !a.no_capture),
        PlanCommand::Done(a) => step(ctx, Transition::Done, a.step, None, !a.no_capture),
        PlanCommand::Drop(a) => step(ctx, Transition::Drop, a, None, false),
        PlanCommand::Set(a) => set(ctx, a),
        PlanCommand::Snapshot(a) => record_snapshot(ctx, a),
        PlanCommand::Reopen(a) => reopen(ctx, a),
        PlanCommand::List(a) => list(ctx, a),
        PlanCommand::Show { id } => show(ctx, &id),
    }
}

fn new(ctx: &Context, args: NewArgs) -> Result<Output> {
    let title = one_line("the title", &args.title)?;
    let actor = actor_or_env(args.actor, parse_person, ACTOR_HUMAN)?;
    let (config, logbook) = ctx.open_logbook()?;
    // the case file, its name, STATUS.md and the ledger get the redacted title
    let title = Redactor::for_config(&config)?.redact(&title);
    let lock = ctx.lock()?;
    let created = create(
        ctx,
        &config,
        &logbook,
        &lock,
        Spec {
            title,
            zone: args.zone,
            risk: args.risk,
            area: args.area,
            priority: args.priority,
            actor,
            intent: None,
            tags: Vec::new(),
            note: None,
            start: false,
            point: false,
            done: None,
            source: None,
        },
    )?;
    let id = created.file.case.id.clone();
    let commit = autocommit(ctx, &config, &logbook, &format!("{id} created"));
    crate::index::rebuild_if_initialised(ctx);
    drop(lock);

    let mut human = format!(
        "Created {id} \"{}\" in {}",
        created.file.case.title,
        created.file.relative(&logbook)
    );
    if let Some(area) = &created.area_created {
        human.push_str(&format!("\nNew area: {area}"));
    }
    human.push_str(&commit.human());
    Ok(Output::ok(
        human,
        json!({
            "case": case_json(&logbook, &created.file),
            "event": event_json(&created.events[0]),
            "areaCreated": created.area_created,
            "git": commit.json(),
        }),
    ))
}

/// A case to create ([`create`]). `title` and `intent` are redacted
/// already; `note` goes into the `created` Log line.
pub(crate) struct Spec {
    pub title: String,
    pub zone: Zone,
    pub risk: Risk,
    pub area: Option<String>,
    pub priority: Priority,
    pub actor: String,
    pub intent: Option<String>,
    pub tags: Vec<String>,
    pub note: Option<String>,
    /// Created and started in one go: status active, `case-created` and
    /// `case-started` in one ledger write.
    pub start: bool,
    /// With `start`: the new case becomes `.seldon/active-case`, which the
    /// hooks attribute an agent's commands by.
    pub point: bool,
    /// Created completed in one go (`import task --include-done`, WP-102):
    /// `case-created` and `case-completed` in one ledger write, `done` the
    /// completed Log line's text. Never with `start`.
    pub done: Option<String>,
    /// The frontmatter's `source` (`import task` only, ADR-0038 §3).
    pub source: Option<String>,
}

/// What [`create`] wrote: the case file, its ledger events (created, then
/// started), the area README it created.
pub(crate) struct Created {
    pub file: CaseFile,
    pub events: Vec<Event>,
    pub area_created: Option<String>,
}

/// Creates a case under the caller's lock: the next id, the logbook's case
/// template (with `intent` in *Intent*), the Log line(s), the ledger
/// event(s) first, then the file, the area and, when started, the active
/// case. No commit and no index rebuild: the caller does both.
pub(crate) fn create(
    ctx: &Context,
    config: &crate::config::Config,
    logbook: &Logbook,
    lock: &crate::logbook::lock::Lock,
    spec: Spec,
) -> Result<Created> {
    if let Some(area) = spec.area.as_deref()
        && !crate::model::is_slug(area)
    {
        return Err(Error::user(format!(
            "area `{area}` is not a lowercase slug ([a-z0-9][a-z0-9-]*)"
        )));
    }
    let today = ctx.now.date_naive();
    let id = cases::next_id(logbook, ctx.now.year())?;
    let status = if spec.done.is_some() {
        CaseStatus::Completed
    } else if spec.start {
        CaseStatus::Active
    } else {
        CaseStatus::Queued
    };
    let case = Case {
        id: id.clone(),
        title: spec.title.clone(),
        status,
        zone: spec.zone,
        risk: spec.risk,
        priority: Some(spec.priority),
        area: spec.area.clone(),
        created: today,
        started: (spec.start || spec.done.is_some()).then_some(today),
        closed: spec.done.is_some().then_some(today),
        snapshot_before: None,
        agents: Vec::new(),
        events: Vec::new(),
        tags: spec.tags.clone(),
        source: spec.source.clone(),
    };
    let mut body = cases::new_body(logbook, &id, &spec.title)?;
    if let Some(intent) = &spec.intent {
        body = cases::put_intent(&body, intent);
    }
    let path = logbook
        .path("work")
        .join(status.folder())
        .join(case.file_name(&cases::slug(&spec.title, "case")));
    let mut file = CaseFile {
        path,
        case,
        doc: crate::frontmatter::Document {
            frontmatter: None,
            body,
        },
    };
    file.add_agent(&spec.actor);
    let mut line = format!("created (zone {}, risk {})", spec.zone, spec.risk);
    if let Some(note) = &spec.note {
        line.push_str(&format!(": {note}"));
    }
    file.log(&ctx.now, &line, &spec.actor);
    if spec.start {
        file.log(&ctx.now, Transition::Start.log_word(), &spec.actor);
    }
    if let Some(done) = &spec.done {
        file.log(&ctx.now, done, &spec.actor);
    }
    let text = crate::model::render_new(&file.case, &file.doc.body);
    if file.path.exists() {
        return Err(Error::user(format!(
            "{} already exists",
            file.path.display()
        )));
    }

    // the ledger first: if it cannot be written, nothing else is
    let mut events = vec![
        Event::new(ctx.now, Source::Seldon, Kind::CaseCreated, &id)
            .detail(spec.title.clone())
            .actor(&spec.actor)
            .case(Some(id.clone()))
            .risk(spec.risk),
    ];
    if spec.start {
        events.push(
            Event::new(ctx.now, Source::Seldon, Kind::CaseStarted, &id)
                .actor(&spec.actor)
                .case(Some(id.clone()))
                .risk(spec.risk),
        );
    }
    if spec.done.is_some() {
        events.push(
            Event::new(ctx.now, Source::Seldon, Kind::CaseCompleted, &id)
                .actor(&spec.actor)
                .case(Some(id.clone())),
        );
    }
    let events = emit(lock, config, logbook, events)?;
    let area_created = spec
        .area
        .as_deref()
        .map(|a| cases::ensure_area(logbook, a))
        .transpose()?
        .flatten();
    write_new(&file.path, &text)?;
    if spec.start && spec.point {
        cases::set_active_case(logbook, &id)?;
    }
    Ok(Created {
        file,
        events,
        area_created,
    })
}

/// One step; `snapshot` only comes with `plan start` (clap has no
/// `--snapshot` on the other steps). With `capture` (`plan verify|done`
/// without `--no-capture`, ADR-0029 §2), a step the case allows runs a
/// default capture first, under the capture's own lock, released before
/// the step takes its own: what the user did by hand inside the case is
/// recorded, and linked by rule 9, while the case is still open. A failed
/// or degraded capture is a warning; the step goes on.
fn step(
    ctx: &Context,
    transition: Transition,
    args: StepArgs,
    snapshot: Option<u64>,
    capture: bool,
) -> Result<Output> {
    let reason = args
        .reason
        .as_deref()
        .map(|r| one_line("--reason", r))
        .transpose()?;
    // an agent's session cannot close as a person (ADR-0027 §5: an agent
    // close is never recorded as human; WP-101 round 2)
    if transition == Transition::Done
        && args.actor.as_deref() == Some(ACTOR_HUMAN)
        && let Ok(Some(session)) = env_actor(parse_person)
        && is_agent(&session)
    {
        return Err(Error::user(format!(
            "{} is not closed: `--actor human` in a session of {session} ({ACTOR_ENV}); an \
             agent's close is never recorded as human (ADR-0027 §5). Close it as {session}, or \
             from a session of your own (the panel's Done)",
            args.id
        )));
    }
    let actor = actor_or_env(args.actor, parse_person, ACTOR_HUMAN)?;
    let (config, logbook) = ctx.open_logbook()?;
    let redactor = Redactor::for_config(&config)?;
    let reason = reason.map(|r| redactor.redact(&r));
    // only a step the case allows captures; the checks run again below,
    // under the step's lock
    let captured = (capture
        && cases::find(&logbook, &args.id).is_ok_and(|f| transition.target(f.case.status).is_ok()))
    .then(|| capture_first(ctx));
    let lock = ctx.lock()?;
    let mut file = cases::find(&logbook, &args.id)?;
    let from = file.case.status;
    let to = transition
        .target(from)
        .map_err(|e| Error::user(format!("{} {e}", args.id)))?;
    if transition == Transition::Start {
        refuse_agent_start_of_imported(&file, &actor)?;
    }

    // an agent closes only with evidence (ADR-0027 §5); the resolved
    // actor counts, so `SELDON_ACTOR` cannot go around it
    if transition == Transition::Done && is_agent(&actor) {
        let gaps = cases::close_gaps(&file.doc.body);
        if !gaps.is_empty() {
            return Err(Error::user(format!(
                "{} is not closed: {actor} closes a case only with its evidence, and {}; \
                 fill it in, then run `seldon plan done {}` again (ADR-0027 §5)",
                args.id,
                gaps.join(" and "),
                args.id
            )));
        }
        if !file.case.tags.iter().any(|t| t == TAG_CLOSED_BY_AGENT) {
            file.case.tags.push(TAG_CLOSED_BY_AGENT.to_string());
        }
    }
    // `--snapshot` at the start: only "before the first red event" can be
    // checked (the case was not started before); warnings, never refusal
    let snapshot_checks = match snapshot {
        Some(n) => snapshot::checks(&config, &logbook, &args.id, n, false)?,
        None => Vec::new(),
    };

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
    file.add_agent(&actor);
    let mut line = transition.log_word().to_string();
    if let Some(n) = snapshot {
        line.push_str(&format!(" (snapshot {n})"));
    }
    if let Some(r) = &reason {
        line.push_str(": ");
        line.push_str(r);
    }
    file.log(&ctx.now, &line, &actor);
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
        .map(|text| journal::prepare(&logbook, &ctx.now, &actor, Some(&args.id), text))
        .transpose()?;

    // the ledger first: if it cannot be written, the case is not moved,
    // the marker and the journal stay as they are
    let mut event = Event::new(ctx.now, Source::Seldon, kind(transition), &args.id)
        .actor(&actor)
        .case(Some(args.id.clone()));
    if transition == Transition::Start {
        // ADR-0035 §1: the risk the case starts with
        event = event.risk(file.case.risk);
    }
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
    let mut warnings: Vec<String> = (transition == Transition::Start)
        .then(|| snapshot_warning(&file.case))
        .flatten()
        .into_iter()
        .collect();
    warnings.extend(snapshot_checks);
    let capture = captured.map(|(summary, problems)| {
        warnings.extend(problems);
        summary
    });
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
            "capture": capture,
            "git": commit.json(),
            "warnings": warnings,
        }),
    ))
}

/// The capture before `plan verify|done` (ADR-0029 §2): a default `seldon
/// capture`, waiting for a held lock as a hook does. Returns its summary
/// for `--json` (`null` when it failed) and the warning when it failed (an
/// error, or the lock still held after the wait). A degraded collector is
/// no warning here: `seldon doctor` reports it (orchestrator ruling, WP-115
/// round 2).
fn capture_first(ctx: &Context) -> (Value, Vec<String>) {
    let start = std::time::Instant::now();
    let out = loop {
        match super::capture::run(ctx, super::capture::CaptureArgs::default()) {
            Err(Error::LockHeld(_)) if start.elapsed() < super::hook::LOCK_PATIENCE => {
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            other => break other,
        }
    };
    match out {
        Ok(out) => (
            json!({
                "ok": out.json["ok"],
                "written": out.json["written"],
                "linkedPlanned": out.json["linkedPlanned"],
            }),
            Vec::new(),
        ),
        Err(e) => (
            Value::Null,
            vec![format!(
                "the capture before the step did not run: {e}; the step went on \
                 (`seldon capture` records what it missed)"
            )],
        ),
    }
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

/// A case that is still open (queued, active, verification), or why a
/// command `what` cannot change it.
fn open_only(file: &CaseFile, what: &str) -> Result<()> {
    match file.case.status {
        CaseStatus::Queued | CaseStatus::Active | CaseStatus::Verification => Ok(()),
        closed => Err(Error::user(format!(
            "{} is {closed}; `seldon plan {what}` changes an open case only{}",
            file.case.id,
            if closed == CaseStatus::Completed {
                format!(" (`seldon plan reopen {}` starts a new one)", file.case.id)
            } else {
                String::new()
            }
        ))),
    }
}

/// `plan set`: zone, risk and area of an open case, in its frontmatter,
/// with one Log line and one `case-updated` ledger line that carries the
/// risk after the change (ADR-0035 §1; the harm guard reads it). A value
/// equal to the current one is no change; with nothing changed nothing is
/// written (exit 0).
fn set(ctx: &Context, args: SetArgs) -> Result<Output> {
    let actor = actor_or_env(args.actor, parse_person, ACTOR_HUMAN)?;
    if let Some(area) = args.area.as_deref()
        && !crate::model::is_slug(area)
    {
        return Err(Error::user(format!(
            "area `{area}` is not a lowercase slug ([a-z0-9][a-z0-9-]*)"
        )));
    }
    let (config, logbook) = ctx.open_logbook()?;
    let lock = ctx.lock()?;
    let mut file = cases::find(&logbook, &args.id)?;
    open_only(&file, "set")?;

    let c = &mut file.case;
    let mut changed: Vec<Value> = Vec::new();
    let mut words: Vec<String> = Vec::new();
    let mut note = |key: &str, from: String, to: String| {
        words.push(format!("{key} {from} → {to}"));
        changed.push(json!({ "key": key, "from": from, "to": to }));
    };
    if let Some(z) = args.zone.filter(|z| *z != c.zone) {
        note("zone", c.zone.to_string(), z.to_string());
        c.zone = z;
    }
    if let Some(r) = args.risk.filter(|r| *r != c.risk) {
        note("risk", c.risk.to_string(), r.to_string());
        c.risk = r;
    }
    if let Some(a) = args.area.filter(|a| c.area.as_deref() != Some(a.as_str())) {
        note(
            "area",
            c.area.clone().unwrap_or_else(|| "-".into()),
            a.clone(),
        );
        c.area = Some(a);
    }
    if changed.is_empty() {
        drop(lock);
        return Ok(Output::ok(
            format!("{}: nothing changed", args.id),
            json!({
                "case": case_json(&logbook, &file),
                "changed": changed,
                "event": Value::Null,
                "areaCreated": Value::Null,
                "git": Value::Null,
            }),
        ));
    }
    file.add_agent(&actor);
    file.log(&ctx.now, &format!("set {}", words.join(", ")), &actor);
    file.prepare(&logbook, |_| {})?;
    // a new area's README first (WP-120 round 2, N5): a failure there
    // leaves the ledger and the case file as they were; an area left
    // behind by a later failure is harmless (the next set finds it)
    let area_created = match &file.case.area {
        Some(a) if changed.iter().any(|c| c["key"] == "area") => cases::ensure_area(&logbook, a)?,
        _ => None,
    };
    // then the ledger: if it cannot be written, the case file stays
    let event = Event::new(ctx.now, Source::Seldon, Kind::CaseUpdated, &args.id)
        .detail(words.join(", "))
        .actor(&actor)
        .case(Some(args.id.clone()))
        .risk(file.case.risk);
    let event = emit_one(&lock, &config, &logbook, event)?;
    file.save(&logbook)?;
    let commit = autocommit(
        ctx,
        &config,
        &logbook,
        &format!("{} set {}", args.id, words.join(", ")),
    );
    crate::index::rebuild_if_initialised(ctx);
    drop(lock);

    let mut human = format!("{}: {}", args.id, words.join(", "));
    if let Some(area) = &area_created {
        human.push_str(&format!("\nNew area: {area}"));
    }
    let warnings: Vec<String> = raised_warning(&file.case).into_iter().collect();
    human.push_str(&super::index::warnings_human(&warnings));
    human.push_str(&commit.human());
    Ok(Output::ok(
        human,
        json!({
            "case": case_json(&logbook, &file),
            "changed": changed,
            "event": event_json(&event),
            "areaCreated": area_created,
            "git": commit.json(),
            "warnings": warnings,
        }),
    ))
}

/// The advice for an R2 or R3 case without a recorded snapshot after
/// `plan set` (ADR-0027 §3, §2c).
fn raised_warning(case: &Case) -> Option<String> {
    if case.snapshot_before.is_some() {
        return None;
    }
    let id = &case.id;
    match case.risk {
        Risk::R0 | Risk::R1 => None,
        Risk::R2 => Some(format!(
            "{id} is R2: take a snapshot before its first red change and record it with \
             `seldon plan snapshot {id} <N>` (ADR-0027 §3)"
        )),
        Risk::R3 => Some(format!(
            "{id} is R3: take a snapshot before its first red change, record it with \
             `seldon plan snapshot {id} <N>`, and ask for the user's explicit go per R3 step \
             (ADR-0027 §2c)"
        )),
    }
}

/// `plan snapshot`: records the rollback snapshot in `snapshotBefore` when
/// it is empty (ADR-0027 §3). Another number there is exit 1; the same one
/// again changes nothing. The checks are warnings, never a refusal.
fn record_snapshot(ctx: &Context, args: SnapshotArgs) -> Result<Output> {
    let actor = actor_or_env(args.actor, parse_person, ACTOR_HUMAN)?;
    let (config, logbook) = ctx.open_logbook()?;
    let lock = ctx.lock()?;
    let mut file = cases::find(&logbook, &args.id)?;
    open_only(&file, "snapshot")?;
    let n = args.number;
    match file.case.snapshot_before {
        Some(m) if m == n => {
            drop(lock);
            return Ok(Output::ok(
                format!("{}: snapshot {n} is already recorded", args.id),
                json!({
                    "case": case_json(&logbook, &file),
                    "recorded": false,
                    "snapshot": n,
                    "git": Value::Null,
                    "warnings": [],
                }),
            ));
        }
        Some(m) => {
            return Err(Error::user(format!(
                "{} has snapshot {m} recorded as its rollback; a case keeps the snapshot from \
                 before its first change (write a later one into its Log)",
                args.id
            )));
        }
        None => {}
    }
    let warnings = snapshot::checks(&config, &logbook, &args.id, n, true)?;
    file.case.snapshot_before = Some(n);
    file.add_agent(&actor);
    file.log(&ctx.now, &format!("snapshot {n}"), &actor);
    file.save(&logbook)?;
    let commit = autocommit(ctx, &config, &logbook, &format!("{} snapshot {n}", args.id));
    crate::index::rebuild_if_initialised(ctx);
    drop(lock);

    let mut human = format!("{}: snapshot {n} recorded as its rollback", args.id);
    human.push_str(&super::index::warnings_human(&warnings));
    human.push_str(&commit.human());
    Ok(Output::ok(
        human,
        json!({
            "case": case_json(&logbook, &file),
            "recorded": true,
            "snapshot": n,
            "git": commit.json(),
            "warnings": warnings,
        }),
    ))
}

/// `plan reopen`: a completed case stays completed (ADR-0003); a new
/// active case "Reopen: <title>" takes its zone, risk, area, priority and
/// Intent, the tag `reopens:<ID>`, and becomes the active case. Both get a
/// Log line. Every reopen makes a new case; the output names the earlier
/// ones.
fn reopen(ctx: &Context, args: ReopenArgs) -> Result<Output> {
    let actor = actor_or_env(args.actor, parse_person, ACTOR_HUMAN)?;
    let (config, logbook) = ctx.open_logbook()?;
    let redactor = Redactor::for_config(&config)?;
    let lock = ctx.lock()?;
    let mut old = cases::find(&logbook, &args.id)?;
    if old.case.status != CaseStatus::Completed {
        return Err(Error::user(format!(
            "{} is {}; `seldon plan reopen` reopens a completed case only",
            args.id, old.case.status
        )));
    }
    let tag = format!("{TAG_REOPENS}{}", args.id);
    let (all, _) = cases::all(&logbook)?;
    let earlier: Vec<String> = all
        .iter()
        .filter(|f| f.case.tags.contains(&tag))
        .map(|f| f.case.id.clone())
        .collect();
    let intent = cases::intent(&old.doc.body).to_string();
    let title = redactor.redact(&format!("Reopen: {}", old.case.title));
    // the active-case marker routes a running agent's recorded commands:
    // a reopen takes it only when no open case holds it (WP-101 round 2)
    let holder = cases::active_case(&logbook).filter(|held| {
        all.iter().any(|f| {
            f.case.id == *held
                && matches!(
                    f.case.status,
                    CaseStatus::Queued | CaseStatus::Active | CaseStatus::Verification
                )
        })
    });
    // the old case's Log line is written after the new case exists, so
    // its save is checked first (WP-077)
    old.prepare(&logbook, |f| {
        f.log(&ctx.now, "reopened as C-0000-000", &actor)
    })?;
    let created = create(
        ctx,
        &config,
        &logbook,
        &lock,
        Spec {
            title,
            zone: old.case.zone,
            risk: old.case.risk,
            area: old.case.area.clone(),
            priority: old.case.priority.unwrap_or(Priority::Normal),
            actor: actor.clone(),
            intent: (!intent.is_empty()).then(|| redactor.redact(&intent)),
            tags: vec![tag],
            note: Some(format!("reopens {}", args.id)),
            start: true,
            point: holder.is_none(),
            done: None,
            source: None,
        },
    )?;
    let id = created.file.case.id.clone();
    old.log(&ctx.now, &format!("reopened as {id}"), &actor);
    old.save(&logbook)?;
    let commit = autocommit(
        ctx,
        &config,
        &logbook,
        &format!("{} reopened as {id}", args.id),
    );
    crate::index::rebuild_if_initialised(ctx);
    drop(lock);

    let mut human = format!(
        "{} reopened as {id} \"{}\" (active) in {}",
        args.id,
        created.file.case.title,
        created.file.relative(&logbook)
    );
    if !earlier.is_empty() {
        human.push_str(&format!(
            "\nReopened before as {}; this is a new case",
            earlier.join(", ")
        ));
    }
    if let Some(held) = &holder {
        human.push_str(&format!(
            "\nThe active case stays {held}: commands an agent runs are still recorded on it. \
             `seldon agent start {id}` hands the new case to an agent"
        ));
    }
    human.push_str(&commit.human());
    Ok(Output::ok(
        human,
        json!({
            "case": case_json(&logbook, &created.file),
            "reopens": args.id,
            "earlier": earlier,
            "events": created.events.iter().map(event_json).collect::<Vec<_>>(),
            "activeCase": match &holder {
                Some(held) => json!({ "kept": held }),
                None => json!({ "set": id }),
            },
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
/// The most of a case's *Intent* `plan show --json` gives in `intent.text`,
/// in bytes (WP-102b: the desk shows it whole before an imported case's
/// Start).
pub const SHOW_INTENT_MAX: usize = 64 * 1024;

fn show(ctx: &Context, id: &str) -> Result<Output> {
    let (config, logbook) = ctx.open_logbook()?;
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
    json["intent"] = intent_json(&config, &file.doc.body);
    Ok(Output::ok(human, json))
}

/// `plan show --json` `intent` (WP-102b): the whole *Intent* section as
/// display text (`index::build::marked_text`: control characters as
/// spaces, every direction or format character marked `‹U+XXXX›` and
/// counted in `hidden`, redacted), at most [`SHOW_INTENT_MAX`] bytes cut at
/// a character, with its line count before the cut. `null` while the
/// config's redaction patterns do not compile (withheld, as the index
/// withholds its texts).
fn intent_json(config: &crate::config::Config, body: &str) -> Value {
    let Ok(redactor) = Redactor::for_config(config) else {
        return Value::Null;
    };
    let (text, hidden) = crate::index::build::marked_text(&redactor, cases::intent(body));
    let lines = if text.is_empty() {
        0
    } else {
        text.lines().count()
    };
    let truncated = text.len() > SHOW_INTENT_MAX;
    let mut end = text.len().min(SHOW_INTENT_MAX);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    json!({ "text": &text[..end], "lines": lines, "truncated": truncated, "hidden": hidden })
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
    // the frontmatter's `source` while it has the schema's shape (ADR-0038 §3)
    if let Some(s) = c
        .source
        .as_deref()
        .filter(|s| crate::import::is_case_source(s))
    {
        v["source"] = json!(s);
    }
    v
}
