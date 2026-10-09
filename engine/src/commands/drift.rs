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

use super::event::{
    ACTOR_ENV, actor_or_env, clip, emit, env_actor, event_json, parse_case_id, parse_person,
};
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
    /// Store an agent's triage proposal (JSON on stdin) for the user to
    /// apply; every item needs evidence the engine can resolve
    Propose(ProposeArgs),
    /// Apply a stored triage proposal as the user: link and explain its
    /// items; a crisis only when named by --item
    Apply(ApplyArgs),
    /// Remove a stored triage proposal; the logbook is not touched
    Discard(DiscardArgs),
}

#[derive(Debug, Clone, Args)]
#[command(after_help = "Input (stdin):
  {\"items\": [{\"eventId\": \"<EVENT>\", \"action\": \"link\", \"caseId\": \"<CASE>\",
              \"evidence\": [{\"kind\": \"plan\", \"ref\": \"<CASE>\"}]}]}
  action explain takes \"title\" and \"intent\" instead of \"caseId\"; evidence kinds:
  journal (YYYY-MM-DD HH:MM), event (<EVENT>), snapshot (<N>), case (<CASE>),
  plan (<CASE>: a line of its Plan that names the change)")]
pub struct ProposeArgs {
    /// Read the proposal from FILE instead of stdin
    #[arg(long, value_name = "FILE")]
    pub file: Option<std::path::PathBuf>,

    /// Who proposes: agent:NAME (default: $SELDON_ACTOR)
    #[arg(long, value_name = "ACTOR", value_parser = parse_person)]
    pub actor: Option<String>,
}

#[derive(Debug, Clone, Args)]
pub struct ApplyArgs {
    /// The proposal id, as `seldon drift propose` and index.json's triage name it
    #[arg(value_name = "PROPOSAL", value_parser = super::triage::parse_proposal_id)]
    pub id: String,

    /// Apply only this item (repeatable); the only way to apply a crisis
    #[arg(long = "item", value_name = "EVENT", value_parser = parse_event_id)]
    pub items: Vec<String>,

    /// Who applies it: human (default: $SELDON_ACTOR, else human); an agent
    /// is refused
    #[arg(long, value_name = "ACTOR", value_parser = parse_person)]
    pub actor: Option<String>,
}

#[derive(Debug, Clone, Args)]
pub struct DiscardArgs {
    /// The proposal id
    #[arg(value_name = "PROPOSAL", value_parser = super::triage::parse_proposal_id)]
    pub id: String,

    /// Who discards it: human (default: $SELDON_ACTOR, else human); an
    /// agent is refused
    #[arg(long, value_name = "ACTOR", value_parser = parse_person)]
    pub actor: Option<String>,
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
                title: None,
                proposed_by: None,
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
        Some(DriftCommand::Propose(a)) => super::triage::propose(ctx, a.file.as_deref(), a.actor),
        Some(DriftCommand::Apply(mut a)) => {
            a.items.dedup();
            super::triage::apply(ctx, &a.id, &a.items, a.actor)
        }
        Some(DriftCommand::Discard(a)) => super::triage::discard(ctx, &a.id, a.actor),
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
        Some(_) if reconcile::engine_resolved(&built, e) => {
            let _ = writeln!(
                human,
                "{} by the engine (SPEC-ENGINE §5); `seldon drift link|explain|dismiss` replaces it:",
                not_open(event)
            );
            for m in &members {
                let _ = writeln!(human, "  {}  {}", m.id, m);
            }
        }
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
pub(super) struct Explain {
    /// The case's title; `None`: the intent (`drift explain`).
    pub title: Option<String>,
    pub intent: String,
    pub zone: Option<Zone>,
    pub risk: Risk,
    pub area: Option<String>,
    /// `drift apply`: the agent whose proposal this is; the case gets the
    /// tag `proposed-by:<agent>` (WP-124 round 3, B4).
    pub proposed_by: Option<String>,
}

/// What a resolving command does besides the resolution lines.
pub(super) enum Action {
    Link { case: String },
    Explain(Explain),
    Dismiss { reason: String },
}

impl Action {
    /// The action with its free text (intent, reason) through `redactor`.
    pub(super) fn redacted(self, redactor: &Redactor) -> Action {
        match self {
            Action::Link { case } => Action::Link { case },
            Action::Explain(explain) => Action::Explain(Explain {
                title: explain.title.as_deref().map(|t| redactor.redact(t)),
                intent: redactor.redact(&explain.intent),
                ..explain
            }),
            Action::Dismiss { reason } => Action::Dismiss {
                reason: redactor.redact(&reason),
            },
        }
    }

    /// What the selection may take: `link` routine events too.
    pub(super) fn intent(&self) -> reconcile::Intent {
        match self {
            Action::Link { .. } => reconcile::Intent::Link,
            _ => reconcile::Intent::Resolve,
        }
    }

    pub(super) fn resolution(&self) -> Resolution {
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
    // an agent's session cannot resolve as a person (ADR-0028 §3, as
    // WP-101 for `plan done`; WP-109 round 2): `--actor human` would go
    // around the refusal for crises
    if actor.as_deref() == Some(ACTOR_HUMAN)
        && let Ok(Some(session)) = env_actor(parse_person)
        && crate::model::is_agent(&session)
    {
        return Err(Error::user(format!(
            "{id} is not resolved: `--actor human` in a session of {session} ({ACTOR_ENV}); \
             an agent's resolution is never recorded as human (ADR-0028 §3). Resolve it as \
             {session}, or from a session of your own (the panel)"
        )));
    }
    let actor = &actor_or_env(actor, parse_person, ACTOR_HUMAN)?;
    let (config, logbook) = ctx.open_logbook()?;
    // the new case and the ledger get the redacted intent or reason
    let action = action.redacted(&Redactor::for_config(&config)?);
    let lock = ctx.lock()?;
    // the folders a resolution writes into, before the index reads them
    // (WP-168)
    logbook.checked_dir(crate::ledger::LEDGER_DIR)?;
    if !matches!(action, Action::Dismiss { .. }) {
        cases::checked_folders(&logbook)?;
    }
    if let Action::Explain(Explain {
        area: Some(area), ..
    }) = &action
    {
        logbook.checked_dir(format!("areas/{area}"))?;
    }
    let built = index::derive(ctx, &config, &logbook)?;
    warn(&built);
    // a link names an existing case, also when there is nothing to resolve
    let case_file = match &action {
        Action::Link { case } => Some(cases::find(&logbook, case)?),
        _ => None,
    };
    let sel = reconcile::select(&built, id, only, action.intent())?;
    refuse_agent(actor, &sel, &action, case_file.as_ref())?;
    let resolution = action.resolution();
    if sel.members.is_empty() {
        drop(lock);
        return Ok(nothing_to_do(&sel, resolution));
    }
    let done = write_resolution(
        ctx, &config, &logbook, &lock, &built, &sel, actor, &action, None, case_file,
    )?;
    // never the subject: free text must not reach a command line
    let summary = match &done.case_id {
        Some(c) => format!("drift {}: {} event(s), {c}", done.verb(), done.resolved),
        None => format!("drift {}: {} event(s)", done.verb(), done.resolved),
    };
    let commit = autocommit(ctx, &config, &logbook, &summary);
    crate::index::rebuild_if_initialised(ctx);
    drop(lock);
    if let Some(e) = &done.case_error {
        return Err(Error::user(format!(
            "{} {} event(s) in the ledger, but the case file did not follow: {e}",
            capitalise(done.verb()),
            done.resolved
        )));
    }

    let mut human = format!(
        "{} {} event(s){}",
        capitalise(done.verb()),
        done.resolved,
        match (&action, &done.case_id) {
            (Action::Link { .. }, Some(c)) => format!(" to {c}"),
            (Action::Explain(_), Some(c)) => format!(" with the new completed case {c}"),
            _ => String::new(),
        }
    );
    if let Some(tx) = &done.group {
        let _ = write!(human, " (transaction {tx})");
    }
    if let (Some(file), Action::Explain(_)) = (&done.case, &action) {
        let _ = write!(human, "\nCase: {}", file.relative(&logbook));
    }
    if let Some(area) = &done.area_created {
        let _ = write!(human, "\nNew area: {area}");
    }
    human.push_str(&commit.human());
    Ok(Output::ok(
        human,
        json!({
            "eventId": done.event_id,
            "resolution": resolution,
            "only": only,
            "txId": done.group,
            "resolved": done.resolved,
            "events": done.written.iter().map(event_json).collect::<Vec<_>>(),
            "case": done.case.as_ref().map(|f| case_json(&logbook, f)),
            "areaCreated": done.area_created,
            "git": commit.json(),
        }),
    ))
}

/// What one resolving write did ([`write_resolution`]).
pub(super) struct Resolved {
    /// The named event.
    pub event_id: String,
    pub resolution: Resolution,
    /// `meta.txId` of the lines when the write fanned out over a group.
    pub group: Option<String>,
    /// How many `resolution` lines were written.
    pub resolved: usize,
    /// Every ledger line written: (case-created,) the resolutions (,
    /// case-completed).
    pub written: Vec<Event>,
    /// The linked case, or the retroactive one `explain` made.
    pub case: Option<CaseFile>,
    pub case_id: Option<String>,
    pub area_created: Option<String>,
    /// The case file could not follow the ledger lines, which are written
    /// (the caller still commits and rebuilds, then reports it).
    pub case_error: Option<Error>,
}

impl Resolved {
    pub fn verb(&self) -> &'static str {
        match self.resolution {
            Resolution::Linked => "linked",
            Resolution::Explained => "explained",
            Resolution::Dismissed => "dismissed",
        }
    }
}

/// The write of `drift link|explain|dismiss` on a selection with members,
/// under `lock`, from `built` (derived under that lock): one ledger write
/// ((case-created,) one `resolution` line per member (, case-completed)),
/// then the case files. `detail` replaces the resolution lines' detail
/// (`drift apply`'s "proposed by …"); without it a link has none, an
/// explanation its intent, a dismissal its reason. No autocommit and no
/// index rebuild: the caller does both once.
#[allow(clippy::too_many_arguments)]
pub(super) fn write_resolution(
    ctx: &Context,
    config: &crate::config::Config,
    logbook: &crate::logbook::Logbook,
    lock: &crate::logbook::lock::Lock,
    built: &Built,
    sel: &Selection,
    actor: &str,
    action: &Action,
    detail: Option<String>,
    mut case_file: Option<CaseFile>,
) -> Result<Resolved> {
    let resolution = action.resolution();
    let mut area_created = None;
    let (case_id, detail, case_events) = match action {
        Action::Link { case } => (Some(case.clone()), detail, Vec::new()),
        Action::Dismiss { reason } => (None, detail.or_else(|| Some(reason.clone())), Vec::new()),
        Action::Explain(explain) => {
            let intent = &explain.intent;
            let file = retroactive_case(ctx, logbook, built, sel, explain, actor)?;
            let id = file.case.id.clone();
            let created = Event::new(ctx.now, Source::Seldon, Kind::CaseCreated, &id)
                .detail(intent.clone())
                .actor(actor)
                .case(Some(id.clone()))
                .risk(file.case.risk);
            let completed = Event::new(ctx.now, Source::Seldon, Kind::CaseCompleted, &id)
                .actor(actor)
                .case(Some(id.clone()));
            case_file = Some(file);
            (
                Some(id),
                detail.or_else(|| Some(intent.clone())),
                vec![created, completed],
            )
        }
    };
    let lines = reconcile::resolutions(
        sel,
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
    // ADR-0029 §3: an event the engine linked to another case leaves that
    // case's `events:` when someone resolves it otherwise
    let mut moved: std::collections::BTreeMap<String, Vec<String>> = Default::default();
    for m in sel
        .members
        .iter()
        .filter(|m| reconcile::engine_resolved(built, m))
    {
        if let Some(before) = m.case.as_deref().filter(|c| Some(*c) != case_id.as_deref()) {
            moved
                .entry(before.to_string())
                .or_default()
                .push(m.id.to_string());
        }
    }
    let ts = reconcile::ts_index(&built.ledger);
    let attach = |file: &mut CaseFile| {
        reconcile::attach(file, &sel.members, |id| ts.get(id).copied());
        file.add_agent(actor);
    };
    // a linked case its save would refuse fails before the ledger changes
    // (WP-077); `explain` writes a new file, whole
    if let (Action::Link { .. }, Some(file)) = (action, &case_file) {
        file.prepare(logbook, attach)?;
    }
    let written = emit(lock, config, logbook, events)?;

    // the ledger lines are written and win; a case file that cannot follow
    // is reported with them, never as "nothing written" (WP-124 round 2)
    let mut after = || -> Result<()> {
        fail_after_ledger()?;
        if let Some(file) = case_file.as_mut() {
            attach(file);
            match action {
                Action::Explain(explain) => {
                    area_created = explain
                        .area
                        .as_deref()
                        .map(|a| cases::ensure_area(logbook, a))
                        .transpose()?
                        .flatten();
                    let text = crate::model::render_new(&file.case, &file.doc.body);
                    write_new(logbook, &file.path, &text)?;
                }
                _ => {
                    file.save(logbook)?;
                }
            }
        }
        Ok(())
    };
    let case_error = after().err();

    let done = Resolved {
        event_id: sel.event.event.id.to_string(),
        resolution,
        group: sel.group.map(String::from),
        resolved,
        written,
        case: case_file,
        case_id,
        area_created,
        case_error,
    };
    let verb = done.verb();
    for (before, ids) in &moved {
        let instead = match &done.case_id {
            Some(c) => format!("{verb} to {c}"),
            None => verb.to_string(),
        };
        let unlinked = cases::find(logbook, before).and_then(|mut file| {
            file.case.events.retain(|id| !ids.contains(id));
            file.log(
                &ctx.now,
                &format!(
                    "no longer linked here: {} ({instead} by {actor}; the engine had linked it)",
                    ids.join(", ")
                ),
                actor,
            );
            file.save(logbook).map(drop)
        });
        // the ledger line is written and wins; the case file follows
        if let Err(e) = unlinked {
            eprintln!("seldon: warning: {before}: case file not updated: {e}");
        }
    }
    Ok(done)
}

/// Debug builds only: `SELDON_TEST_DRIFT_FAIL_AFTER_LEDGER=1` fails a
/// resolving write right after its ledger lines, as a case file that
/// cannot be written would (the tests of that path).
fn fail_after_ledger() -> Result<()> {
    #[cfg(debug_assertions)]
    if std::env::var("SELDON_TEST_DRIFT_FAIL_AFTER_LEDGER").is_ok_and(|v| v == "1") {
        return Err(anyhow::anyhow!("the case file was not written (test)").into());
    }
    Ok(())
}

/// ADR-0028 §3, enforced: an agent actor may not explain or dismiss a
/// crisis, and may link one only to an active case that lists it in
/// `agents`. A human is never refused. Exit 1 before any write.
pub(super) fn refuse_agent(
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
/// from `--zone` or the drift item; the tag `closed-by-agent` when an agent
/// explains.
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
    let title = explain.title.as_deref().unwrap_or(intent);
    let id = cases::next_id(logbook, ctx.now.year())?;
    let case = Case {
        id: id.clone(),
        title: title.to_string(),
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
        // an agent's explanation closes a case too (ADR-0027 §5, WP-101)
        tags: {
            let mut tags = Vec::new();
            if crate::model::is_agent(actor) {
                tags.push(super::plan::TAG_CLOSED_BY_AGENT.to_string());
            }
            if let Some(agent) = &explain.proposed_by {
                tags.push(format!("{}{agent}", super::triage::PROPOSED_BY));
            }
            tags
        },
        source: None,
    };
    let body = cases::new_body(logbook, &id, title)?;
    let body = reconcile::append_to_section(&body, "Intent", intent);
    let path = logbook
        .path("work")
        .join(CaseStatus::Completed.folder())
        .join(case.file_name(&cases::slug(title, "case")));
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
pub(super) fn nothing_to_do(sel: &Selection, resolution: Resolution) -> Output {
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
pub(super) fn not_open(event: &crate::index::model::IndexEvent) -> String {
    let e = &event.event;
    match (e.resolution, e.case.as_deref()) {
        (Some(r), Some(c)) => format!("{} is already {r} ({c})", e.id),
        (Some(r), None) => format!("{} is already {r}", e.id),
        (None, Some(c)) => format!("{} belongs to {c}", e.id),
        (None, None) => format!("{} is not drift", e.id),
    }
}

/// Load warnings of the index model, on stderr (like the rebuild's).
pub(super) fn warn(built: &Built) {
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
