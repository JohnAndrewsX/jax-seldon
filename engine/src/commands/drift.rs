//! `seldon drift [--crisis-only] [--all]`, `drift link|explain|dismiss|show`
//! (SPEC-ENGINE §3 §5, CONTRACT.md "Commands the plugin may run").
//!
//! The list is the index's drift model (WP-007), not a second derivation:
//! the newest 200 open items, crises first (ADR-0020), with the totals of
//! every open item; `--all` lists every item that can still be resolved,
//! routine ones too, uncapped (ADR-0028 §4c). Each item carries its class
//! and the rule that gave it (ADR-0028 §2). `link` also resolves a routine
//! event; `explain` and `dismiss` refuse one (exit 1). An agent actor may
//! not explain or dismiss a crisis, and may link one only to an active
//! case that lists it in `agents` (ADR-0028 §3); a human is never refused. A resolving command appends one `resolution` line per
//! open member of the named event's item in one ledger write (ADR-0008,
//! ADR-0013 §4); `--only` resolves the named event alone; a re-run finds
//! nothing open and writes nothing. `link` and `explain` record the
//! resolved ids in the case's `events:` (ADR-0012 §10); `explain` creates
//! a retroactive case through WP-006's case store (template, id, folder),
//! completed at once. Every write rebuilds the index (CONTRACT.md rule 2).

use std::fmt::Write as _;

use chrono::Datelike as _;
use clap::{Args, Subcommand};
use serde_json::{Value, json};

use super::event::{actor_or_env, clip, emit, event_json, parse_case_id, parse_person};
use super::plan::case_json;
use super::{Context, Output, autocommit, one_line, write_new};
use crate::error::{Error, Result};
use crate::index::build::ClassifiedItem;
use crate::index::class::Class;
use crate::index::{self, Built};
use crate::logbook::cases::{self, CaseFile};
use crate::model::event::{ACTOR_HUMAN, Event, Kind, Resolution, Source};
use crate::model::{Case, CaseStatus, Priority, Risk, Zone};
use crate::reconcile::{self, Resolve, Selection};
use crate::redact::Redactor;

#[derive(Debug, Clone, Args)]
#[command(args_conflicts_with_subcommands = true)]
pub struct DriftArgs {
    #[command(subcommand)]
    pub command: Option<DriftCommand>,

    /// Only crises (changes that can affect boot, login or the shell)
    #[arg(long)]
    pub crisis_only: bool,

    /// Also routine events (history, not drift), every item, uncapped
    #[arg(long)]
    pub all: bool,
}

#[derive(Debug, Clone, Subcommand)]
pub enum DriftCommand {
    /// Link a drift event, and the open members of its group, to a case
    Link(LinkArgs),
    /// Explain a drift event with a new retroactive, completed case
    Explain(ExplainArgs),
    /// Dismiss a drift event with a reason
    Dismiss(DismissArgs),
    /// Show a drift event and every open member of its group
    Show {
        /// The drift event id, as `seldon drift` prints it
        #[arg(value_name = "EVENT", value_parser = parse_event_id)]
        id: String,
    },
}

#[derive(Debug, Clone, Args)]
pub struct LinkArgs {
    /// The drift event id, as `seldon drift` prints it
    #[arg(value_name = "EVENT", value_parser = parse_event_id)]
    pub id: String,

    /// The case id, e.g. C-2026-004 (a completed or dropped case too)
    #[arg(value_name = "CASE", value_parser = parse_case_id)]
    pub case_id: String,

    /// Resolve the named event only, not the rest of its group
    #[arg(long)]
    pub only: bool,

    /// Who resolves it: human or agent:NAME (default: $SELDON_ACTOR, else
    /// human)
    #[arg(long, value_name = "ACTOR", value_parser = parse_person)]
    pub actor: Option<String>,
}

#[derive(Debug, Clone, Args)]
#[command(after_help = "Example:
  seldon drift explain <EVENT> --area hardware -- \"Driver for the new GPU\"")]
pub struct ExplainArgs {
    /// The drift event id, as `seldon drift` prints it
    #[arg(value_name = "EVENT", value_parser = parse_event_id)]
    pub id: String,

    /// Why it happened, as one argument after `--`; the new case's title
    #[arg(value_name = "INTENT", allow_hyphen_values = true)]
    pub intent: String,

    /// Resolve the named event only, not the rest of its group
    #[arg(long)]
    pub only: bool,

    /// Zone of the new case (default: the drift item's zone)
    #[arg(long, value_name = "ZONE")]
    pub zone: Option<Zone>,

    /// Risk of the new case
    #[arg(long, value_name = "RISK", default_value = "R1")]
    pub risk: Risk,

    /// Area slug of the new case; created under areas/ on first use
    #[arg(long, value_name = "AREA")]
    pub area: Option<String>,

    /// Who resolves it: human or agent:NAME (default: $SELDON_ACTOR, else
    /// human)
    #[arg(long, value_name = "ACTOR", value_parser = parse_person)]
    pub actor: Option<String>,
}

#[derive(Debug, Clone, Args)]
#[command(after_help = "Example:
  seldon drift dismiss <EVENT> -- \"Tried a theme, reverted it\"")]
pub struct DismissArgs {
    /// The drift event id, as `seldon drift` prints it
    #[arg(value_name = "EVENT", value_parser = parse_event_id)]
    pub id: String,

    /// Why it can be ignored, as one argument after `--`
    #[arg(value_name = "REASON", allow_hyphen_values = true)]
    pub reason: String,

    /// Resolve the named event only, not the rest of its group
    #[arg(long)]
    pub only: bool,

    /// Who resolves it: human or agent:NAME (default: $SELDON_ACTOR, else
    /// human)
    #[arg(long, value_name = "ACTOR", value_parser = parse_person)]
    pub actor: Option<String>,
}

/// clap value parser: an event id (ULID, `event.schema.json#/$defs/ulid`).
pub fn parse_event_id(s: &str) -> Result<String, String> {
    reconcile::check_event_id(s)
        .map(|_| s.to_string())
        .map_err(|e| e.to_string())
}

pub fn run(ctx: &Context, args: DriftArgs) -> Result<Output> {
    match args.command {
        None => list(ctx, args.crisis_only, args.all),
        Some(DriftCommand::Show { id }) => show(ctx, &id),
        Some(DriftCommand::Link(a)) => resolve(
            ctx,
            &a.id,
            a.only,
            a.actor,
            Action::Link { case: a.case_id },
        ),
        Some(DriftCommand::Explain(a)) => {
            let intent = one_line("the intent", &a.intent)?;
            if let Some(area) = a.area.as_deref()
                && !crate::model::is_slug(area)
            {
                return Err(Error::user(format!(
                    "area `{area}` is not a lowercase slug ([a-z0-9][a-z0-9-]*)"
                )));
            }
            let action = Action::Explain(Explain {
                intent,
                zone: a.zone,
                risk: a.risk,
                area: a.area,
            });
            resolve(ctx, &a.id, a.only, a.actor, action)
        }
        Some(DriftCommand::Dismiss(a)) => {
            let reason = one_line("the reason", &a.reason)?;
            resolve(ctx, &a.id, a.only, a.actor, Action::Dismiss { reason })
        }
    }
}

/// An item as the list and `show` print it: the index's fields plus its
/// class and rule (ADR-0028 §2).
fn item_json(i: &ClassifiedItem) -> Value {
    let mut v = serde_json::to_value(&i.item).expect("an item always serialises");
    v["class"] = json!(i.class.as_str());
    v["rule"] = json!(i.rule);
    v
}

/// The open drift items of the index model (crises first past the cap,
/// ADR-0020), or with `--all` every item of linkable events, routine ones
/// included, uncapped; `--crisis-only` keeps the crises. `openDrift` and
/// `crisis` count every open item, also past the cap.
fn list(ctx: &Context, crisis_only: bool, all: bool) -> Result<Output> {
    let (config, logbook) = ctx.open_logbook()?;
    let built = index::derive(ctx, &config, &logbook)?;
    warn(&built);
    let summary = &built.index.summary;
    let listed: Vec<&ClassifiedItem> = if all {
        built.items.iter().collect()
    } else {
        // the capped index list, in its order
        built
            .index
            .drift
            .iter()
            .filter_map(|d| built.items.iter().find(|i| i.item.event_id == d.event_id))
            .collect()
    };
    let items: Vec<&ClassifiedItem> = listed
        .into_iter()
        .filter(|i| !crisis_only || i.class == Class::Crisis)
        .collect();
    let routine = built
        .items
        .iter()
        .filter(|i| i.class == Class::Routine)
        .count();
    let shown_total = match (crisis_only, all) {
        (true, _) => summary.crisis,
        (false, true) => summary.open_drift + routine,
        (false, false) => summary.open_drift,
    };

    let mut human = String::new();
    for i in &items {
        let d = &i.item;
        let _ = writeln!(
            human,
            "{:<9}  {}  {}/{}  {}{}{}  {}",
            match i.class {
                Class::Crisis => "CRISIS",
                Class::Routine => "routine",
                Class::Attention => d.zone.as_deref().unwrap_or("-"),
            },
            short_ts(&d.ts),
            d.source,
            d.kind,
            clip(&d.subject, 60),
            d.members
                .map(|n| format!(" (+{} more)", n - 1))
                .unwrap_or_default(),
            d.proposed_case
                .as_deref()
                .map(|c| format!(" → {c}?"))
                .unwrap_or_default(),
            d.event_id
        );
    }
    if items.is_empty() {
        human.push_str(match (crisis_only, all) {
            (true, _) => "No crises.",
            (false, true) => "Nothing to resolve.",
            (false, false) => "No open drift.",
        });
    } else {
        let _ = write!(
            human,
            "{} open drift item(s), {} crisis",
            summary.open_drift, summary.crisis
        );
        if all {
            let _ = write!(human, "; {routine} routine (history, not drift)");
        }
        if items.len() < shown_total {
            let _ = write!(human, "; showing the newest {}", items.len());
        }
    }
    Ok(Output::ok(
        human.trim_end(),
        json!({
            "drift": items.iter().map(|i| item_json(i)).collect::<Vec<_>>(),
            "openDrift": summary.open_drift,
            "crisis": summary.crisis,
            "routine": routine,
        }),
    ))
}

/// `drift show <id>`: the event as the index folds it, its item with class
/// and rule, and every member of its item that can still be resolved,
/// oldest first (open drift, or routine: ADR-0028).
fn show(ctx: &Context, id: &str) -> Result<Output> {
    let (config, logbook) = ctx.open_logbook()?;
    let built = index::derive(ctx, &config, &logbook)?;
    warn(&built);
    let event = reconcile::find(&built, id)?;
    let members = reconcile::linkable_members(&built, &event.event);
    let item = reconcile::item_of(&built, &event.event);
    let open = item.is_some_and(|i| i.listed);
    let folded = |m: &Event| -> Value {
        built
            .folded
            .iter()
            .find(|f| f.event.id == m.id)
            .map(|f| serde_json::to_value(f).expect("an event always serialises"))
            .unwrap_or_else(|| event_json(m))
    };

    let e = &event.event;
    let mut human = format!("{}  {}\n", e.id, e);
    match item {
        None => human.push_str(&not_open(event)),
        Some(i) => {
            let d = &i.item;
            let _ = writeln!(
                human,
                "{} (rule {}):",
                match i.class {
                    Class::Routine =>
                        "Routine, history, not drift; `drift link` still takes it".to_string(),
                    Class::Crisis =>
                        format!("Open drift ({}, crisis)", d.zone.as_deref().unwrap_or("-")),
                    Class::Attention =>
                        format!("Open drift ({})", d.zone.as_deref().unwrap_or("-")),
                },
                i.rule
            );
            for m in &members {
                let _ = writeln!(
                    human,
                    "  {}  {}{}",
                    m.id,
                    m,
                    if m.explicit == Some(true) {
                        "  (explicit)"
                    } else {
                        ""
                    }
                );
            }
        }
    }
    Ok(Output::ok(
        human.trim_end(),
        json!({
            "event": folded(e),
            "open": open,
            "class": item.map(|i| i.class.as_str()),
            "rule": item.map(|i| i.rule),
            "item": item.map(item_json),
            "txId": e.tx_id.as_deref().filter(|_| members.len() > 1),
            "members": members.iter().map(|m| folded(m)).collect::<Vec<_>>(),
        }),
    ))
}

/// `drift explain`'s options for the new case.
struct Explain {
    intent: String,
    zone: Option<Zone>,
    risk: Risk,
    area: Option<String>,
}

/// What a resolving command does besides the resolution lines.
enum Action {
    Link { case: String },
    Explain(Explain),
    Dismiss { reason: String },
}

impl Action {
    /// The action with its free text (intent, reason) through `redactor`.
    fn redacted(self, redactor: &Redactor) -> Action {
        match self {
            Action::Link { case } => Action::Link { case },
            Action::Explain(explain) => Action::Explain(Explain {
                intent: redactor.redact(&explain.intent),
                ..explain
            }),
            Action::Dismiss { reason } => Action::Dismiss {
                reason: redactor.redact(&reason),
            },
        }
    }

    fn resolution(&self) -> Resolution {
        match self {
            Action::Link { .. } => Resolution::Linked,
            Action::Explain(_) => Resolution::Explained,
            Action::Dismiss { .. } => Resolution::Dismissed,
        }
    }
}

fn resolve(
    ctx: &Context,
    id: &str,
    only: bool,
    actor: Option<String>,
    action: Action,
) -> Result<Output> {
    let actor = &actor_or_env(actor, parse_person, ACTOR_HUMAN)?;
    let (config, logbook) = ctx.open_logbook()?;
    // the new case and the ledger get the redacted intent or reason
    let action = action.redacted(&Redactor::for_config(&config)?);
    let lock = ctx.lock()?;
    let built = index::derive(ctx, &config, &logbook)?;
    warn(&built);
    // a link names an existing case, also when there is nothing to resolve
    let mut case_file = match &action {
        Action::Link { case } => Some(cases::find(&logbook, case)?),
        _ => None,
    };
    let intent = match &action {
        Action::Link { .. } => reconcile::Intent::Link,
        _ => reconcile::Intent::Resolve,
    };
    let sel = reconcile::select(&built, id, only, intent)?;
    refuse_agent(actor, &sel, &action, case_file.as_ref())?;
    let resolution = action.resolution();
    if sel.members.is_empty() {
        drop(lock);
        return Ok(nothing_to_do(&sel, resolution));
    }

    let mut area_created = None;
    let (case_id, detail, case_events) = match &action {
        Action::Link { case } => (Some(case.clone()), None, Vec::new()),
        Action::Dismiss { reason } => (None, Some(reason.clone()), Vec::new()),
        Action::Explain(explain) => {
            let intent = &explain.intent;
            let file = retroactive_case(ctx, &logbook, &built, &sel, explain, actor)?;
            let id = file.case.id.clone();
            let created = Event::new(ctx.now, Source::Seldon, Kind::CaseCreated, &id)
                .detail(intent.clone())
                .actor(actor)
                .case(Some(id.clone()));
            let completed = Event::new(ctx.now, Source::Seldon, Kind::CaseCompleted, &id)
                .actor(actor)
                .case(Some(id.clone()));
            case_file = Some(file);
            (Some(id), Some(intent.clone()), vec![created, completed])
        }
    };
    let lines = reconcile::resolutions(
        &sel,
        &Resolve {
            resolution,
            ts: ctx.now,
            actor: actor.to_string(),
            detail,
            case: case_id.clone(),
        },
    );
    let resolved = lines.len();

    // one ledger write: (case-created,) one line per member (, case-completed)
    let mut events = Vec::with_capacity(resolved + 2);
    let mut case_events = case_events.into_iter();
    events.extend(case_events.next());
    events.extend(lines);
    events.extend(case_events);
    let ts = reconcile::ts_index(&built.ledger);
    let attach = |file: &mut CaseFile| {
        reconcile::attach(file, &sel.members, |id| ts.get(id).copied());
        file.add_agent(actor);
    };
    // a linked case its save would refuse fails before the ledger changes
    // (WP-077); `explain` writes a new file, whole
    if let (Action::Link { .. }, Some(file)) = (&action, &case_file) {
        file.prepare(&logbook, attach)?;
    }
    let written = emit(&lock, &config, &logbook, events)?;

    if let Some(file) = case_file.as_mut() {
        attach(file);
        match &action {
            Action::Explain(explain) => {
                area_created = explain
                    .area
                    .as_deref()
                    .map(|a| cases::ensure_area(&logbook, a))
                    .transpose()?
                    .flatten();
                let text = crate::model::render_new(&file.case, &file.doc.body);
                write_new(&file.path, &text)?;
            }
            _ => {
                file.save(&logbook)?;
            }
        }
    }

    let verb = match resolution {
        Resolution::Linked => "linked",
        Resolution::Explained => "explained",
        Resolution::Dismissed => "dismissed",
    };
    // never the subject: free text must not reach a command line
    let summary = match &case_id {
        Some(c) => format!("drift {verb}: {resolved} event(s), {c}"),
        None => format!("drift {verb}: {resolved} event(s)"),
    };
    let commit = autocommit(ctx, &config, &logbook, &summary);
    crate::index::rebuild_if_initialised(ctx);
    drop(lock);

    let mut human = format!(
        "{} {resolved} event(s){}",
        capitalise(verb),
        match (&action, &case_id) {
            (Action::Link { .. }, Some(c)) => format!(" to {c}"),
            (Action::Explain(_), Some(c)) => format!(" with the new completed case {c}"),
            _ => String::new(),
        }
    );
    if let Some(tx) = sel.group {
        let _ = write!(human, " (transaction {tx})");
    }
    if let (Some(file), Action::Explain(_)) = (&case_file, &action) {
        let _ = write!(human, "\nCase: {}", file.relative(&logbook));
    }
    if let Some(area) = &area_created {
        let _ = write!(human, "\nNew area: {area}");
    }
    human.push_str(&commit.human());
    Ok(Output::ok(
        human,
        json!({
            "eventId": sel.event.event.id.to_string(),
            "resolution": resolution,
            "only": only,
            "txId": sel.group,
            "resolved": resolved,
            "events": written.iter().map(event_json).collect::<Vec<_>>(),
            "case": case_file.as_ref().map(|f| case_json(&logbook, f)),
            "areaCreated": area_created,
            "git": commit.json(),
        }),
    ))
}

/// ADR-0028 §3, enforced: an agent actor may not explain or dismiss a
/// crisis, and may link one only to an active case that lists it in
/// `agents`. A human is never refused. Exit 1 before any write.
fn refuse_agent(
    actor: &str,
    sel: &Selection,
    action: &Action,
    case: Option<&CaseFile>,
) -> Result<()> {
    if !crate::model::is_agent(actor) || !sel.crisis() {
        return Ok(());
    }
    let id = &sel.event.event.id;
    match (action, case) {
        (Action::Link { .. }, Some(file))
            if file.case.status == CaseStatus::Active
                && file.case.agents.iter().any(|a| a == actor) =>
        {
            Ok(())
        }
        (Action::Link { case: c }, _) => Err(Error::user(format!(
            "{id} is a crisis (ADR-0028 §3): {actor} may link it only to an active case that lists {actor} in `agents`, which {c} does not; tell the user"
        ))),
        _ => Err(Error::user(format!(
            "{id} is a crisis (ADR-0028 §3): an agent may not explain or dismiss it; tell the user in one line, or link it to your active case if your own work caused it"
        ))),
    }
}

/// The case `drift explain` creates: WP-006's template and id sequence,
/// title and `## Intent` from the intent, status completed (created and
/// started on the day of the earliest resolved event, closed today), zone
/// from `--zone` or the drift item.
fn retroactive_case(
    ctx: &Context,
    logbook: &crate::logbook::Logbook,
    built: &Built,
    sel: &Selection,
    explain: &Explain,
    actor: &str,
) -> Result<CaseFile> {
    let first = sel
        .members
        .first()
        .map_or(ctx.now.date_naive(), |m| m.ts.date_naive());
    let zone = explain.zone.unwrap_or_else(|| item_zone(built, sel));
    let risk = explain.risk;
    let intent = explain.intent.as_str();
    let id = cases::next_id(logbook, ctx.now.year())?;
    let case = Case {
        id: id.clone(),
        title: intent.to_string(),
        status: CaseStatus::Completed,
        zone,
        risk,
        priority: Some(Priority::Normal),
        area: explain.area.clone(),
        created: first,
        started: Some(first),
        closed: Some(ctx.now.date_naive()),
        snapshot_before: None,
        agents: Vec::new(),
        events: Vec::new(),
        tags: Vec::new(),
    };
    let body = cases::new_body(logbook, &id, intent)?;
    let body = reconcile::append_to_section(&body, "Intent", intent);
    let path = logbook
        .path("work")
        .join(CaseStatus::Completed.folder())
        .join(case.file_name(&cases::slug(intent, "case")));
    let mut file = CaseFile {
        path,
        case,
        doc: crate::frontmatter::Document {
            frontmatter: None,
            body,
        },
    };
    file.log(
        &ctx.now,
        &format!(
            "created retroactively for drift {} (zone {zone}, risk {risk})",
            sel.event.event.id
        ),
        actor,
    );
    file.log(
        &ctx.now,
        &format!("completed: explained {} drift event(s)", sel.members.len()),
        actor,
    );
    if file.path.exists() {
        return Err(Error::user(format!(
            "{} already exists",
            file.path.display()
        )));
    }
    Ok(file)
}

/// The zone of the drift item the selection belongs to (computed by the
/// index, ADR-0013 §3), else the named event's own zone, else yellow.
fn item_zone(built: &Built, sel: &Selection) -> Zone {
    let ids: Vec<String> = sel.members.iter().map(|m| m.id.to_string()).collect();
    built
        .index
        .drift
        .iter()
        .find(|d| ids.contains(&d.event_id))
        .and_then(|d| d.zone.as_deref())
        .and_then(|z| z.parse().ok())
        .or(sel.event.event.zone)
        .unwrap_or(Zone::Yellow)
}

/// Exit 0, nothing written: the named event is not open drift.
fn nothing_to_do(sel: &Selection, resolution: Resolution) -> Output {
    let e = &sel.event.event;
    Output::ok(
        format!("Nothing to resolve: {}", not_open(sel.event)),
        json!({
            "eventId": e.id.to_string(),
            "resolution": resolution,
            "resolved": 0,
            "events": [],
            "already": {
                "resolution": e.resolution,
                "case": e.case,
            },
        }),
    )
}

/// Why an event is not open drift.
fn not_open(event: &crate::index::model::IndexEvent) -> String {
    let e = &event.event;
    match (e.resolution, e.case.as_deref()) {
        (Some(r), Some(c)) => format!("{} is already {r} ({c})", e.id),
        (Some(r), None) => format!("{} is already {r}", e.id),
        (None, Some(c)) => format!("{} belongs to {c}", e.id),
        (None, None) => format!("{} is not drift", e.id),
    }
}

/// Load warnings of the index model, on stderr (like the rebuild's).
fn warn(built: &Built) {
    for w in &built.warnings {
        eprintln!("seldon: warning: {w}");
    }
}

/// `2026-10-01 14:03` of an RFC 3339 timestamp.
fn short_ts(ts: &str) -> String {
    ts.get(..16).unwrap_or(ts).replace('T', " ")
}

fn capitalise(s: &str) -> String {
    let mut c = s.chars();
    c.next()
        .map(|f| f.to_uppercase().chain(c).collect())
        .unwrap_or_default()
}
