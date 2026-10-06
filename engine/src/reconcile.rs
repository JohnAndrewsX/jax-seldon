//! Reconciliation (SPEC-ENGINE §5): the rules that write.
//!
//! Drift is *derived* by the index builder ([`crate::index::build`]): the
//! eligibility rule (ADR-0012 §6), resolution folding, grouping per `txId`,
//! the leader, the computed zone and crisis (ADR-0013), proposals
//! (ADR-0015) and the 200-item cap (ADR-0020). Nothing here derives it a
//! second time: every function reads a [`Built`] index (`folded`,
//! `open_drift`, `index.drift`) and only decides what to write.
//!
//! - **Resolving drift** (`seldon drift link|explain|dismiss`, ADR-0008,
//!   ADR-0013 §4, ADR-0028 §3): [`select`] picks the members a command
//!   resolves (`link` also takes routine events; `explain|dismiss` refuse
//!   them),
//!   [`resolutions`] builds one `resolution` line per member for one ledger
//!   write. The original lines are never touched.
//! - **Case files** (ADR-0012 §10): [`attach`] records event ids in a
//!   case's `events:`, oldest first.
//! - **After a capture** (rules 1 and 2): the attribution pass and the
//!   pacman collector's transaction inheritance give collector events the
//!   case of the agent command that caused them, dependencies included,
//!   before the append; [`after_capture`] then records those ids in the
//!   case files.
//! - **The engine's own writes** (rule 7): [`explain_own_writes`] explains
//!   each new config event that reports a file `init` or `hook install`
//!   wrote, with one `explained` resolution right after the append.
//! - **Seldon updating itself** (rule 8): [`explain_own_changes`] explains
//!   each event of its own plugin or package
//!   ([`attribution::own_change`]) the same way: the new ones, and those
//!   an earlier capture left without a resolution (WP-088).
//! - **The planned-and-active link** (rule 9, ADR-0029 §1):
//!   [`planned_links`] links an event without case and resolution, by any
//!   actor, to the one case whose ledger window held its time and whose
//!   Plan names its subject; [`link_planned`] writes it after rules 7 and
//!   8, under the capture's lock, and tells the case files.

use std::collections::{BTreeMap, HashMap, HashSet};

use chrono::{DateTime, Duration, FixedOffset};
use ulid::Ulid;

use crate::attribution;
use crate::collectors::config::{self, OwnWrites};
use crate::config::Dirs;
use crate::error::{Error, Result};
use crate::index::Built;
use crate::index::build::{ClassifiedItem, r3_advisory};
use crate::index::class::Class;
use crate::index::drift::{AlwaysRed, names_token};
use crate::index::model::IndexEvent;
use crate::ledger::Ledger;
use crate::logbook::Logbook;
use crate::logbook::cases::{self, CaseFile};
use crate::logbook::lock::Lock;
use crate::model::event::{ACTOR_SYSTEM, Event, Kind, Meta, Resolution, Source};
use crate::model::{CaseStatus, Risk, is_ulid};

/// What a `drift link|explain|dismiss` on one event resolves.
#[derive(Debug, Clone)]
pub struct Selection<'a> {
    /// The named event as the index sees it (resolution folded).
    pub event: &'a IndexEvent,
    /// The members to resolve, oldest first: the named event and, for a
    /// pacman event without `--only`, every other linkable member of its
    /// transaction. Empty when the named event can no longer be resolved.
    pub members: Vec<&'a Event>,
    /// The transaction's `txId` when the write fans out over two or more
    /// members (`meta.txId` of every line, ADR-0013 §4).
    pub group: Option<&'a str>,
    /// The item the named event belongs to (class, rule; ADR-0028 §2);
    /// `None` when it can no longer be resolved.
    pub item: Option<&'a ClassifiedItem>,
}

impl Selection<'_> {
    /// Whether the named event's item is a crisis (ADR-0028 §3).
    pub fn crisis(&self) -> bool {
        self.item.is_some_and(|i| i.class == Class::Crisis)
    }
}

/// What a resolving command does, as far as the selection goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Intent {
    /// `drift link`: any linkable event, routine ones too (ADR-0028 §8).
    Link,
    /// `drift explain|dismiss`: open drift only; a routine event exits 1.
    Resolve,
}

/// Checks the form of an event id given on the command line.
pub fn check_event_id(id: &str) -> Result<Ulid> {
    if !is_ulid(id) {
        return Err(Error::user(format!(
            "`{id}` is not an event id (a ULID, 26 characters)"
        )));
    }
    id.parse()
        .map_err(|e| Error::user(format!("`{id}` is not an event id: {e}")))
}

/// The event `id` names, as folded by the index. Exit 1 when the ledger
/// has no such event, or it is a resolution itself.
pub fn find<'a>(built: &'a Built, id: &str) -> Result<&'a IndexEvent> {
    let ulid = check_event_id(id)?;
    if let Some(e) = built.folded.iter().find(|f| f.event.id == ulid) {
        return Ok(e);
    }
    if built.ledger.iter().any(|e| e.id == ulid) {
        return Err(Error::user(format!(
            "{id} is a resolution, not a drift event"
        )));
    }
    Err(Error::user(format!("unknown event {id}")))
}

/// The item of linkable events `e` belongs to, with its class.
pub fn item_of<'a>(built: &'a Built, e: &Event) -> Option<&'a ClassifiedItem> {
    if !built.linkable.contains(&e.id) {
        return None;
    }
    built
        .items
        .iter()
        .chain(&built.reresolvable)
        .find(|i| i.members.contains(&e.id))
}

/// Whether `e`'s latest resolution is the engine's (ADR-0029 §3): a
/// command on it writes a later line that wins.
pub fn engine_resolved(built: &Built, e: &Event) -> bool {
    built.reresolvable.iter().any(|i| i.members.contains(&e.id))
}

/// The open drift members of `e`'s drift item, oldest first: for an open
/// pacman event with a `txId`, every open pacman event of that transaction
/// (ADR-0013 §1); for any other open event, the event itself; nothing when
/// `e` is not open drift (resolved, or routine, ADR-0028).
pub fn open_members<'a>(built: &'a Built, e: &Event) -> Vec<&'a Event> {
    if !built.open_drift.contains(&e.id) {
        return Vec::new();
    }
    linkable_members(built, e)
}

/// The linkable members of `e`'s item, oldest first (open drift or
/// routine); nothing when `e` can no longer be resolved.
pub fn linkable_members<'a>(built: &'a Built, e: &Event) -> Vec<&'a Event> {
    let Some(item) = item_of(built, e) else {
        return Vec::new();
    };
    let mut members: Vec<&Event> = built
        .folded
        .iter()
        .map(|f| &f.event)
        .filter(|m| item.members.contains(&m.id))
        .collect();
    members.sort_by_key(|m| (m.ts, m.id));
    members
}

/// What a command on `id` resolves: the members of its item, or the event
/// alone with `only` (ADR-0013 §4). A named event that can no longer be
/// resolved selects nothing, so a re-run writes nothing, also after
/// `--only` (the remaining members are a new item with a leader of their
/// own). Exit 1 for an unknown id, for an event that can never be drift
/// (ADR-0012 §6), and for `explain|dismiss` of a routine event (ADR-0028
/// §3: history, not drift; `link` still takes it).
pub fn select<'a>(built: &'a Built, id: &str, only: bool, intent: Intent) -> Result<Selection<'a>> {
    let event = find(built, id)?;
    let e = &event.event;
    if !e.source.is_drift_eligible() {
        return Err(Error::user(format!(
            "{id} is a {}/{} event; only pacman, omarchy, plugins, theme and config events can be drift",
            e.source, e.kind
        )));
    }
    let item = item_of(built, e);
    if intent == Intent::Resolve
        && let Some(i) = item.filter(|i| i.class == Class::Routine)
    {
        return Err(Error::user(format!(
            "{id} is routine (rule `{}`, ADR-0028): history, not drift; nothing to explain or dismiss. \
             `seldon drift link` ties it to a case",
            i.rule
        )));
    }
    let mut members = linkable_members(built, e);
    if only {
        members.retain(|m| m.id == e.id);
    }
    let group = match members.as_slice() {
        [_, _, ..] => e.tx_id.as_deref(),
        _ => None,
    };
    Ok(Selection {
        event,
        members,
        group,
        item,
    })
}

/// How a selection is resolved: the same `ts`, `actor`, `detail` and
/// `case` on every line (ADR-0013 §4).
#[derive(Debug, Clone)]
pub struct Resolve {
    pub resolution: Resolution,
    pub ts: DateTime<FixedOffset>,
    pub actor: String,
    pub detail: Option<String>,
    pub case: Option<String>,
}

/// One `resolution` line per selected member, for one ledger write:
/// `refersTo` the member, its subject (as in `fixtures/logbook/`), and
/// `meta.txId` when the write fans out over a group.
pub fn resolutions(sel: &Selection, r: &Resolve) -> Vec<Event> {
    sel.members
        .iter()
        .map(|m| {
            let mut e = Event::new(r.ts, Source::Seldon, Kind::Resolution, m.subject.clone())
                .actor(&r.actor)
                .case(r.case.clone())
                .meta(Meta {
                    tx_id: sel.group.map(String::from),
                    ..Meta::default()
                });
            e.detail = r.detail.clone();
            e.refers_to = Some(m.id);
            e.resolution = Some(r.resolution);
            e
        })
        .collect()
}

/// Records `events` in the case's `events:` (ADR-0012 §10: non-`seldon`
/// events, oldest first) and their agent actors in `agents`. A new id goes
/// before the first listed id whose event is later; ids already listed
/// stay where they are. `ts_of` gives the time of a listed id (`None`:
/// unknown, counts as earlier). Returns how many ids were added.
pub fn attach(
    file: &mut CaseFile,
    events: &[&Event],
    ts_of: impl Fn(&str) -> Option<DateTime<FixedOffset>>,
) -> usize {
    let mut sorted: Vec<&Event> = events
        .iter()
        .copied()
        .filter(|e| e.source != Source::Seldon)
        .collect();
    sorted.sort_by_key(|e| (e.ts, e.id));
    let mut added = 0;
    for e in sorted {
        file.add_agent(&e.actor);
        let id = e.id.to_string();
        let listed = &mut file.case.events;
        if listed.contains(&id) {
            continue;
        }
        let at = listed
            .iter()
            .position(|l| ts_of(l).is_some_and(|t| t > e.ts))
            .unwrap_or(listed.len());
        listed.insert(at, id);
        added += 1;
    }
    added
}

/// `ts` of every ledger event by id, for [`attach`].
pub fn ts_index(events: &[Event]) -> HashMap<String, DateTime<FixedOffset>> {
    events.iter().map(|e| (e.id.to_string(), e.ts)).collect()
}

/// After a capture: records every written collector event that carries a
/// case (set by the attribution pass, or inherited inside a pacman
/// transaction, SPEC-ENGINE §5 rules 1 and 2) in that case's `events:`.
///
/// The ledger write already happened, so nothing here fails the capture:
/// an unknown case, an unreadable case file or a failed save is a warning
/// on stderr, like the index rebuild's (the ids stay in the ledger).
pub fn after_capture(logbook: &Logbook, ledger: &Ledger, written: &[Event]) {
    for w in record_cases(logbook, ledger, written) {
        eprintln!("seldon: warning: {w}");
    }
}

/// [`after_capture`] without printing: the warnings.
pub fn record_cases(logbook: &Logbook, ledger: &Ledger, written: &[Event]) -> Vec<String> {
    let mut by_case: BTreeMap<&str, Vec<&Event>> = BTreeMap::new();
    for e in written.iter().filter(|e| e.source != Source::Seldon) {
        if let Some(case) = e.case.as_deref() {
            by_case.entry(case).or_default().push(e);
        }
    }
    let Some(from) = by_case.values().flatten().map(|e| e.ts).min() else {
        return Vec::new();
    };
    let mut warnings = Vec::new();
    // a listed id can only be later than a new event if its ts is at or
    // after the earliest new one; older ids need no lookup (a century
    // ahead stands for "everything after", without overflowing chrono)
    let later = match ledger.read_range(from, from + Duration::days(36_525)) {
        Ok(events) => ts_index(&events),
        Err(e) => {
            warnings.push(format!("case files not updated: {e:#}"));
            return warnings;
        }
    };
    for (case, events) in by_case {
        let mut file = match cases::find(logbook, case) {
            Ok(f) => f,
            Err(e) => {
                warnings.push(format!(
                    "{} event(s) attributed to {case} not recorded in a case file: {e}",
                    events.len()
                ));
                continue;
            }
        };
        if attach(&mut file, &events, |id| later.get(id).copied()) == 0 {
            continue;
        }
        if let Err(e) = file.save(logbook) {
            warnings.push(format!("{case}: case file not updated: {e}"));
        }
    }
    warnings
}

/// The `explained` resolutions rule 7 writes: one per written config
/// event without a case that an own write in `own` explains
/// ([`OwnWrites::explaining`]: an add or change with the hash written, a
/// removal of the content deleted), detail `installed by <command>`
/// (`removed by <command>` for a removal command), or whose new content is
/// a built-in template (`template`, ADR-0028 §2: the detail it returns);
/// `source: seldon`, actor `system`, no case, at `ts`.
pub fn own_write_resolutions(
    written: &[Event],
    own: &OwnWrites,
    ts: DateTime<FixedOffset>,
    template: impl Fn(&Event) -> Option<&'static str>,
) -> Vec<Event> {
    written
        .iter()
        .filter(|e| e.source == Source::Config && e.case.is_none())
        .filter(|e| {
            matches!(
                e.kind,
                Kind::ConfigAdd | Kind::ConfigChange | Kind::ConfigRemove
            )
        })
        .filter_map(|e| {
            let detail = match own.explaining(e) {
                Some(w) => format!("{} by {}", w.op.verb(), w.by),
                None => template(e)?.to_string(),
            };
            let mut r = Event::new(ts, Source::Seldon, Kind::Resolution, e.subject.clone())
                .actor(ACTOR_SYSTEM)
                .detail(detail);
            r.refers_to = Some(e.id);
            r.resolution = Some(Resolution::Explained);
            Some(r)
        })
        .collect()
}

/// After a capture that ran the config collector (SPEC-ENGINE §5 rule 7):
/// appends the [`own_write_resolutions`] of `written` (own writes and the
/// built-in templates, [`config::builtin_template`]) and forgets every
/// recorded own write, explained or not (the collector has now seen each
/// file: as an event, in its baseline, or changed by someone else). An
/// unreadable `owned.json` is kept and only the templates explain.
/// Returns how many events were explained, and warnings: the append
/// already happened, so nothing here fails the capture.
pub fn explain_own_writes(
    lock: &Lock,
    ledger: &Ledger,
    dirs: &Dirs,
    file: &std::path::Path,
    written: &[Event],
    ts: DateTime<FixedOffset>,
) -> (usize, Vec<String>) {
    let mut warnings = Vec::new();
    let (own, loaded) = match OwnWrites::load(file) {
        Ok(own) => (own, true),
        Err(e) => {
            warnings.push(format!("own writes not read: {e}"));
            (OwnWrites::default(), false)
        }
    };
    let lines = own_write_resolutions(written, &own, ts, |e| config::builtin_template(dirs, e));
    let explained = if lines.is_empty() {
        0
    } else {
        match ledger.append(lock, lines) {
            Ok(lines) => lines.len(),
            Err(e) => {
                warnings.push(format!("own writes not explained: {e}"));
                0
            }
        }
    };
    if loaded
        && !own.0.is_empty()
        && let Err(e) = OwnWrites::default().save(file)
    {
        warnings.push(format!("{e:#}"));
    }
    (explained, warnings)
}

/// The `explained` resolutions rule 8 writes: one per event of `events`
/// without a case that is Seldon changing itself
/// ([`attribution::own_change`]) and that no resolution line in `events`
/// refers to (explained, dismissed, linked: it keeps its resolution);
/// `source: seldon`, actor `system`, no case, the reason as detail, at
/// `ts` or the event's time, whichever is later: the index folds a
/// resolution only onto an earlier line, and a month file is chosen by
/// the line's time, so an event dated after the capture clock (the clock
/// moved back) still gets a line after it (WP-088 review).
pub fn own_change_resolutions(events: &[Event], ts: DateTime<FixedOffset>) -> Vec<Event> {
    let resolved: HashSet<Ulid> = events
        .iter()
        .filter(|e| e.kind == Kind::Resolution)
        .filter_map(|e| e.refers_to)
        .collect();
    events
        .iter()
        .filter(|e| e.case.is_none() && !resolved.contains(&e.id))
        .filter_map(|e| {
            let why = attribution::own_change(e)?;
            let at = ts.max(e.ts);
            let mut r = Event::new(at, Source::Seldon, Kind::Resolution, e.subject.clone())
                .actor(ACTOR_SYSTEM)
                .detail(why);
            r.refers_to = Some(e.id);
            r.resolution = Some(Resolution::Explained);
            Some(r)
        })
        .collect()
}

/// After every capture (SPEC-ENGINE §5 rule 8): appends the
/// [`own_change_resolutions`] of the whole ledger `all`, `written`
/// included. That also catches up on own changes an earlier capture left
/// open (the engine stopped or the append failed between the two appends,
/// or a version before rule 8 wrote them, WP-088). When the ledger could
/// not be read (`all` is the error), only `written` is explained. Returns the
/// lines appended, and warnings: the append already happened, so nothing
/// here fails the capture.
pub fn explain_own_changes(
    lock: &Lock,
    ledger: &Ledger,
    written: &[Event],
    all: std::result::Result<&[Event], &str>,
    ts: DateTime<FixedOffset>,
) -> (Vec<Event>, Vec<String>) {
    let mut warnings = Vec::new();
    let lines = match all {
        Ok(events) => own_change_resolutions(events, ts),
        Err(e) => {
            warnings.push(format!("seldon's earlier own changes not checked: {e}"));
            own_change_resolutions(written, ts)
        }
    };
    if lines.is_empty() {
        return (Vec::new(), warnings);
    }
    match ledger.append(lock, lines) {
        Ok(lines) => (lines, warnings),
        Err(e) => {
            warnings.push(format!("seldon's own changes not explained: {e}"));
            (Vec::new(), warnings)
        }
    }
}

/// The detail of a rule-9 `linked` line (ADR-0029 §1).
pub fn planned_detail(case: &str) -> String {
    format!("planned by {case}; active at the time")
}

/// What rule 9 needs of a case: its id and risk, the text of its `## Plan`
/// as it is now, and whether it is closed now (completed or dropped).
#[derive(Debug, Clone)]
pub struct PlanningCase {
    pub id: String,
    pub risk: Risk,
    pub plan: String,
    pub closed: bool,
}

impl PlanningCase {
    pub fn of(file: &CaseFile) -> Self {
        PlanningCase {
            id: file.case.id.clone(),
            risk: file.case.risk,
            plan: cases::section(&file.doc.body, "Plan")
                .map(|r| file.doc.body[r].to_string())
                .unwrap_or_default(),
            closed: matches!(
                file.case.status,
                CaseStatus::Completed | CaseStatus::Dropped
            ),
        }
    }
}

/// One stretch of time a case was open (active or in verification): from
/// a `case-started` to the next `case-completed`/`case-dropped`, both ends
/// included; `end` is `None` while it is still open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Window {
    pub start: DateTime<FixedOffset>,
    pub end: Option<DateTime<FixedOffset>>,
}

impl Window {
    pub fn holds(&self, ts: DateTime<FixedOffset>) -> bool {
        self.start <= ts && self.end.is_none_or(|end| ts <= end)
    }
}

/// Every case's windows, read from the ledger's case events alone (ADR-0029
/// §1a): never from the case file's status or dates, never from
/// `.seldon/active-case`, which keeps no history. A queued case has none.
pub fn case_windows(events: &[Event]) -> BTreeMap<&str, Vec<Window>> {
    let mut steps: Vec<&Event> = events
        .iter()
        .filter(|e| e.source == Source::Seldon)
        .filter(|e| {
            matches!(
                e.kind,
                Kind::CaseStarted | Kind::CaseCompleted | Kind::CaseDropped
            )
        })
        .collect();
    steps.sort_by_key(|e| e.ts);
    let mut out: BTreeMap<&str, Vec<Window>> = BTreeMap::new();
    for e in steps {
        let windows = out.entry(e.subject.as_str()).or_default();
        let open = windows.last_mut().filter(|w| w.end.is_none());
        match (e.kind, open) {
            (Kind::CaseStarted, None) => windows.push(Window {
                start: e.ts,
                end: None,
            }),
            (Kind::CaseCompleted | Kind::CaseDropped, Some(w)) => w.end = Some(e.ts),
            _ => {}
        }
    }
    out
}

/// What rule 9 writes into one case file.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CaseNotes {
    /// The events linked to it, for its `events:`.
    pub events: Vec<Event>,
    /// Its new Log lines, by `system`, each written once.
    pub log: Vec<String>,
}

/// What rule 9 writes (ADR-0029 §1).
#[derive(Debug, Clone, Default)]
pub struct PlannedLinks {
    /// One `linked` resolution per linked event, oldest event first.
    pub lines: Vec<Event>,
    /// Per case id.
    pub cases: BTreeMap<String, CaseNotes>,
}

/// `pacman install glow at 13:40:57`: an event as a Log line names it,
/// its time in its own offset.
fn named(e: &Event) -> String {
    format!(
        "{} {} {} at {}",
        e.source,
        e.kind,
        e.subject,
        e.ts.format("%H:%M:%S")
    )
}

/// The events rule 9 still takes (ADR-0029 §1): drift eligible, no case,
/// no resolution line at all (the engine never writes over anyone's).
fn unresolved(events: &[Event]) -> impl Fn(&Event) -> bool + '_ {
    let resolved: HashSet<Ulid> = events
        .iter()
        .filter(|e| e.kind == Kind::Resolution)
        .filter_map(|e| e.refers_to)
        .collect();
    move |e: &Event| {
        e.source.is_drift_eligible()
            && e.kind != Kind::Resolution
            && e.case.is_none()
            && !resolved.contains(&e.id)
    }
}

/// Whether rule 9 has anything to test: an event it still takes inside
/// some case's window. Cheap: no case file is read.
pub fn has_planned_candidates(events: &[Event]) -> bool {
    let windows = case_windows(events);
    if windows.is_empty() {
        return false;
    }
    let open = unresolved(events);
    events
        .iter()
        .filter(|e| open(e))
        .any(|e| windows.values().flatten().any(|w| w.holds(e.ts)))
}

/// Rule 9, the planned-and-active link (SPEC-ENGINE §5, ADR-0029 §1), over
/// the whole ledger `events`: every event without case and resolution, of
/// a drift-eligible source and by any actor, whose time lies in exactly one
/// case's window ([`case_windows`]) among the `cases` whose Plan names its
/// subject as a whole-word token (rule 3's test), is linked to that case.
///
/// A pacman event of a transaction with an explicit member is tested only
/// when it is explicit; the transaction's non-explicit members follow
/// their explicit members' case (when those go to one case only), one
/// line each, `meta.txId` on every line of a transaction with two or more.
/// Two or more cases: no link, and each gets a Log line naming the others
/// (rule 3 still proposes while one is open). Harm guard: an `alwaysRed`
/// subject links only to an R3 case; below R3 the case gets the R3
/// advisory instead. Lines: `source: seldon`, actor `system`, `case`,
/// detail [`planned_detail`], at `now` or the event's time, whichever is
/// later (WP-088).
pub fn planned_links(
    events: &[Event],
    cases: &[PlanningCase],
    always_red: &AlwaysRed,
    now: DateTime<FixedOffset>,
) -> PlannedLinks {
    let mut out = PlannedLinks::default();
    let windows = case_windows(events);
    if windows.is_empty() {
        return out;
    }
    let open = unresolved(events);
    let pacman_tx = |e: &Event| {
        e.tx_id
            .as_deref()
            .filter(|_| e.source == Source::Pacman)
            .map(str::to_string)
    };
    let explicit_tx: HashSet<String> = events
        .iter()
        .filter(|e| e.explicit == Some(true))
        .filter_map(pacman_tx)
        .collect();
    let follows = |e: &Event| {
        e.explicit != Some(true) && pacman_tx(e).is_some_and(|tx| explicit_tx.contains(&tx))
    };
    let mut cases: Vec<&PlanningCase> = cases.iter().collect();
    cases.sort_by(|a, b| a.id.cmp(&b.id));

    let mut links: Vec<(&Event, &PlanningCase)> = Vec::new();
    for e in events.iter().filter(|e| open(e) && !follows(e)) {
        let planned: Vec<&PlanningCase> = cases
            .iter()
            .copied()
            .filter(|c| {
                windows
                    .get(c.id.as_str())
                    .is_some_and(|ws| ws.iter().any(|w| w.holds(e.ts)))
            })
            .filter(|c| names_token(&c.plan, &e.subject))
            .collect();
        match planned.as_slice() {
            [] => {}
            [c] if always_red.matches(&e.subject) && c.risk != Risk::R3 => {
                let advisory = format!("advisory: {}", r3_advisory(&c.id, c.risk, &e.subject));
                out.cases
                    .entry(c.id.clone())
                    .or_default()
                    .log
                    .push(advisory);
            }
            [c] => {
                links.push((e, c));
                let line = format!(
                    "linked after the fact: {} ({})",
                    named(e),
                    if c.closed {
                        "planned here, no capture ran before the close"
                    } else {
                        "planned here"
                    }
                );
                out.cases.entry(c.id.clone()).or_default().log.push(line);
            }
            several => {
                for c in several {
                    let others: Vec<&str> = several
                        .iter()
                        .filter(|o| o.id != c.id)
                        .map(|o| o.id.as_str())
                        .collect();
                    let line = format!(
                        "not linked: {} is planned here and in {}, {} active at the time; \
                         `seldon drift link {} <CASE>` links it",
                        named(e),
                        others.join(", "),
                        if others.len() == 1 { "both" } else { "all" },
                        e.id
                    );
                    out.cases.entry(c.id.clone()).or_default().log.push(line);
                }
            }
        }
    }

    // the dependencies follow when their explicit members go to one case
    let mut tx_cases: HashMap<String, BTreeMap<&str, &PlanningCase>> = HashMap::new();
    for (e, c) in &links {
        if let Some(tx) = pacman_tx(e) {
            tx_cases.entry(tx).or_default().insert(&c.id, c);
        }
    }
    let mut members: Vec<(&Event, &PlanningCase)> = links.clone();
    for e in events.iter().filter(|e| open(e) && follows(e)) {
        let tx = pacman_tx(e).expect("a follower has a transaction");
        if let Some(only) = tx_cases.get(&tx).filter(|cs| cs.len() == 1) {
            members.push((e, *only.values().next().expect("one case")));
        }
    }
    members.sort_by_key(|(e, _)| (e.ts, e.id));
    let mut per_tx: HashMap<String, usize> = HashMap::new();
    for (e, _) in &members {
        if let Some(tx) = pacman_tx(e) {
            *per_tx.entry(tx).or_default() += 1;
        }
    }

    for (e, c) in &members {
        let tx_id = pacman_tx(e).filter(|tx| per_tx.get(tx).is_some_and(|n| *n > 1));
        let mut r = Event::new(
            now.max(e.ts),
            Source::Seldon,
            Kind::Resolution,
            e.subject.clone(),
        )
        .actor(ACTOR_SYSTEM)
        .case(Some(c.id.clone()))
        .detail(planned_detail(&c.id))
        .meta(Meta {
            tx_id,
            ..Meta::default()
        });
        r.refers_to = Some(e.id);
        r.resolution = Some(Resolution::Linked);
        out.lines.push(r);
        out.cases
            .entry(c.id.clone())
            .or_default()
            .events
            .push((*e).clone());
    }
    out
}

/// After every capture, after rules 7 and 8 (SPEC-ENGINE §5 rule 9):
/// appends the [`planned_links`] of the whole ledger `all` under the
/// capture's lock, then records the linked ids in each case's `events:`
/// and writes its Log lines (each once: a Log that has the line already
/// gets none). Case files are read only when some event without a case
/// lies in a case's window. Returns how many events were linked, and
/// warnings: the capture's append already happened, so nothing here fails
/// it.
pub fn link_planned(
    lock: &Lock,
    ledger: &Ledger,
    logbook: &Logbook,
    all: &[Event],
    always_red: &AlwaysRed,
    now: DateTime<FixedOffset>,
) -> (usize, Vec<String>) {
    let mut warnings = Vec::new();
    if !has_planned_candidates(all) {
        return (0, warnings);
    }
    let files = match cases::all(logbook) {
        Ok((files, _)) => files,
        Err(e) => {
            warnings.push(format!("planned changes not linked: {e}"));
            return (0, warnings);
        }
    };
    let planning: Vec<PlanningCase> = files.iter().map(PlanningCase::of).collect();
    let links = planned_links(all, &planning, always_red, now);
    let linked = if links.lines.is_empty() {
        0
    } else {
        match ledger.append(lock, links.lines) {
            Ok(lines) => lines.len(),
            Err(e) => {
                warnings.push(format!("planned changes not linked: {e}"));
                return (0, warnings);
            }
        }
    };
    let ts = ts_index(all);
    for (id, notes) in links.cases {
        let Some(mut file) = files.iter().find(|f| f.case.id == id).cloned() else {
            continue;
        };
        let log = cases::section(&file.doc.body, "Log")
            .map_or(String::new(), |r| file.doc.body[r].to_string());
        let mut changed = attach(&mut file, &notes.events.iter().collect::<Vec<_>>(), |id| {
            ts.get(id).copied()
        }) > 0;
        let mut added: Vec<String> = Vec::new();
        for line in &notes.log {
            // as `log_line` writes it: one line, single spaces
            let line = line.split_whitespace().collect::<Vec<_>>().join(" ");
            if !log.contains(&line) && !added.contains(&line) {
                file.log(&now, &line, ACTOR_SYSTEM);
                added.push(line);
                changed = true;
            }
        }
        if changed && let Err(e) = file.save(logbook) {
            warnings.push(format!("{id}: case file not updated: {e}"));
        }
    }
    (linked, warnings)
}

/// `body` with `text` as the last line of its `## <name>` section (after
/// the template's comment); unchanged when there is no such section.
pub fn append_to_section(body: &str, name: &str, text: &str) -> String {
    let Some(range) = cases::section(body, name) else {
        return body.to_string();
    };
    let content = &body[range.clone()];
    let at = match content.trim_end_matches([' ', '\t', '\r', '\n']).len() {
        0 => range.start,
        n => {
            let after = range.start + n;
            body[after..range.end]
                .find('\n')
                .map_or(range.end, |i| after + i + 1)
        }
    };
    let mut out = String::with_capacity(body.len() + text.len() + 1);
    out.push_str(&body[..at]);
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out.push_str(text);
    out.push('\n');
    out.push_str(&body[at..]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frontmatter::Document;
    use crate::model::{Case, CaseStatus, Risk, Zone};

    fn ts(s: &str) -> DateTime<FixedOffset> {
        DateTime::parse_from_rfc3339(s).unwrap()
    }

    fn event(n: u64, at: &str, source: Source) -> Event {
        let mut e = Event::new(ts(at), source, Kind::Install, format!("p{n}"));
        e.id = Ulid::from_parts(n, 1);
        e
    }

    fn case_file(events: &[&Event]) -> CaseFile {
        CaseFile {
            path: "work/active/C-2026-001-x.md".into(),
            case: Case {
                id: "C-2026-001".into(),
                title: "x".into(),
                status: CaseStatus::Active,
                zone: Zone::Yellow,
                risk: Risk::R1,
                priority: None,
                area: None,
                created: ts("2026-10-01T00:00:00+00:00").date_naive(),
                started: None,
                closed: None,
                snapshot_before: None,
                agents: Vec::new(),
                events: events.iter().map(|e| e.id.to_string()).collect(),
                tags: Vec::new(),
            },
            doc: Document {
                frontmatter: None,
                body: String::new(),
            },
        }
    }

    #[test]
    fn attach_keeps_the_list_oldest_first() {
        let a = event(1, "2026-10-01T10:00:00+02:00", Source::Pacman);
        let c = event(3, "2026-10-01T12:00:00+02:00", Source::Config);
        let mut file = case_file(&[&a, &c]);
        let known = ts_index(&[a.clone(), c.clone()]);
        let b = event(2, "2026-10-01T09:30:00+00:00", Source::Theme).actor("agent:codex");
        let d = event(4, "2026-10-01T13:00:00+02:00", Source::Plugins);
        let note = event(5, "2026-10-01T09:00:00+02:00", Source::Seldon);
        let added = attach(&mut file, &[&d, &b, &note, &a], |id| known.get(id).copied());
        assert_eq!(added, 2, "a is listed, the seldon event never is");
        let ids: Vec<String> = [&a, &b, &c, &d].iter().map(|e| e.id.to_string()).collect();
        assert_eq!(
            file.case.events, ids,
            "11:30+02:00 sorts between 10:00 and 12:00"
        );
        assert_eq!(file.case.agents, ["agent:codex"]);
        // unknown listed ids count as earlier: new ids go after them
        let mut file = case_file(&[&c]);
        attach(&mut file, &[&a], |_| None);
        assert_eq!(file.case.events, [c.id.to_string(), a.id.to_string()]);
    }

    #[test]
    fn seldons_own_changes_without_a_case_are_explained() {
        let at = "2026-10-01T10:00:00+02:00";
        let own = |n, source, kind, subject: &str| {
            let mut e = event(n, at, source);
            e.kind = kind;
            e.subject = subject.into();
            e
        };
        let plugin =
            own(1, Source::Plugins, Kind::PluginUpdate, "jax.seldon").actor("agent:claude-code");
        let package = own(2, Source::Pacman, Kind::Upgrade, "jax-seldon");
        let cased = own(3, Source::Plugins, Kind::PluginEnable, "jax.seldon")
            .case(Some("C-2026-001".into()));
        let other = own(
            4,
            Source::Plugins,
            Kind::PluginUpdate,
            "io.github.example.tyme",
        );
        let now = ts("2026-10-01T10:20:00+02:00");
        let lines = own_change_resolutions(&[plugin.clone(), package.clone(), cased, other], now);
        let got: Vec<_> = lines
            .iter()
            .map(|r| {
                (
                    r.refers_to,
                    r.subject.clone(),
                    r.detail.clone(),
                    r.source,
                    r.actor.clone(),
                    r.resolution,
                    r.case.is_none(),
                    r.ts,
                )
            })
            .collect();
        let line = |e: &Event, detail: &str| {
            (
                Some(e.id),
                e.subject.clone(),
                Some(detail.to_string()),
                Source::Seldon,
                ACTOR_SYSTEM.to_string(),
                Some(Resolution::Explained),
                true,
                now,
            )
        };
        assert_eq!(
            got,
            [
                line(&plugin, "seldon's own plugin"),
                line(&package, "seldon's own package")
            ]
        );
    }

    /// WP-088: the catch-up reads the whole ledger; an own change that
    /// has any resolution keeps it, one without is explained once.
    #[test]
    fn own_changes_with_a_resolution_keep_it() {
        let own = |n, kind| {
            let mut e = event(n, "2026-10-01T10:00:00+02:00", Source::Plugins);
            e.kind = kind;
            e.subject = attribution::OWN_PLUGIN.into();
            e
        };
        let resolution = |n, of: &Event, r| {
            let mut e = event(n, "2026-10-01T10:10:00+02:00", Source::Seldon);
            e.kind = Kind::Resolution;
            e.refers_to = Some(of.id);
            e.resolution = Some(r);
            e
        };
        let open = own(1, Kind::PluginUpdate);
        let dismissed = own(2, Kind::PluginUpdate);
        let explained = own(3, Kind::PluginEnable);
        let linked = own(4, Kind::PluginDisable);
        let ledger = [
            open.clone(),
            dismissed.clone(),
            explained.clone(),
            linked.clone(),
            resolution(5, &dismissed, Resolution::Dismissed),
            resolution(6, &explained, Resolution::Explained),
            resolution(7, &linked, Resolution::Linked),
        ];
        let now = ts("2026-10-01T10:20:00+02:00");
        let lines = own_change_resolutions(&ledger, now);
        let refers: Vec<_> = lines.iter().map(|r| r.refers_to).collect();
        assert_eq!(refers, [Some(open.id)]);
        let mut caught_up = ledger.to_vec();
        caught_up.extend(lines);
        assert!(own_change_resolutions(&caught_up, now).is_empty());
    }

    /// Rule 9's fixtures: case events and changes on 2026-10-06, CEST.
    mod planned {
        use super::*;

        pub const NOW: &str = "2026-10-06T14:00:00+02:00";

        pub fn at(hm: &str) -> DateTime<FixedOffset> {
            ts(&format!("2026-10-06T{hm}:00+02:00"))
        }

        pub fn step(n: u64, hm: &str, kind: Kind, case: &str) -> Event {
            let mut e = Event::new(at(hm), Source::Seldon, kind, case).case(Some(case.into()));
            e.id = Ulid::from_parts(n, 0);
            e
        }

        pub fn change(n: u64, hm: &str, subject: &str) -> Event {
            let mut e = Event::new(at(hm), Source::Pacman, Kind::Install, subject).actor("human");
            e.id = Ulid::from_parts(n, 0);
            e.explicit = Some(true);
            e
        }

        pub fn case(id: &str, risk: Risk, plan: &str) -> PlanningCase {
            PlanningCase {
                id: id.into(),
                risk,
                plan: plan.into(),
                closed: true,
            }
        }

        pub fn links(events: &[Event], cases: &[PlanningCase]) -> PlannedLinks {
            let red = AlwaysRed::new(&["linux*".to_string()]);
            planned_links(events, cases, &red, ts(NOW))
        }

        /// (event id, case) of every line.
        pub fn linked(l: &PlannedLinks) -> Vec<(u64, String)> {
            l.lines
                .iter()
                .map(|r| {
                    let n = (r.refers_to.unwrap().0 >> 80) as u64;
                    (n, r.case.clone().unwrap())
                })
                .collect()
        }
    }

    /// ADR-0029 §5 acceptance 3: the window comes from the ledger's case
    /// events, both ends included; verification stays inside it, a queued
    /// case has none, a reopened case's predecessor ends at its close, and
    /// a dropped case's window counts.
    #[test]
    fn the_window_is_read_from_the_case_events() {
        use planned::*;
        const A: &str = "C-2026-001";
        const B: &str = "C-2026-002";
        const Q: &str = "C-2026-003";
        const D: &str = "C-2026-004";
        let mut ledger = vec![
            step(1, "10:00", Kind::CaseCreated, A),
            step(2, "10:00", Kind::CaseStarted, A),
            step(3, "10:30", Kind::CaseVerified, A),
            step(4, "11:00", Kind::CaseCompleted, A),
            // A reopened as B at 11:30
            step(5, "11:30", Kind::CaseStarted, B),
            step(6, "10:00", Kind::CaseCreated, Q),
            step(7, "12:00", Kind::CaseStarted, D),
            step(8, "12:30", Kind::CaseDropped, D),
        ];
        let windows = case_windows(&ledger);
        assert_eq!(
            windows[A],
            [Window {
                start: at("10:00"),
                end: Some(at("11:00"))
            }]
        );
        assert_eq!(
            windows[B],
            [Window {
                start: at("11:30"),
                end: None
            }]
        );
        assert!(!windows.contains_key(Q), "queued: no window");
        assert!(windows[A][0].holds(at("10:00")) && windows[A][0].holds(at("11:00")));
        assert!(!windows[A][0].holds(at("09:59")) && !windows[A][0].holds(at("11:01")));

        ledger.extend([
            change(10, "09:59", "glow"),  // before A started
            change(11, "10:45", "glow"),  // A in verification
            change(12, "11:01", "glow"),  // after A's close, before B started
            change(13, "11:45", "mdcat"), // B open, but A (closed) names it only
            change(14, "10:15", "fzf"),   // a queued case names it
            change(15, "12:10", "bat"),   // D's window, D dropped since
        ]);
        let cases = [
            case(A, Risk::R1, "- Steps: install glow and mdcat"),
            case(B, Risk::R1, "- Steps: install glow"),
            case(Q, Risk::R1, "- Steps: install fzf"),
            case(D, Risk::R1, "- Steps: install bat"),
        ];
        let l = links(&ledger, &cases);
        assert_eq!(linked(&l), [(11, A.into()), (15, D.into())]);
        assert!(has_planned_candidates(&ledger));
    }

    /// Acceptance 4 and 5: exactly one case; any actor; an event with a
    /// case or any resolution is left alone; non-drift sources never link.
    #[test]
    fn one_case_any_actor_and_never_over_a_resolution() {
        use planned::*;
        const A: &str = "C-2026-001";
        const B: &str = "C-2026-002";
        let mut ledger = vec![
            step(1, "10:00", Kind::CaseStarted, A),
            step(2, "10:00", Kind::CaseStarted, B),
            change(10, "10:10", "glow"),
            change(11, "10:11", "zed").actor("system"),
            change(12, "10:12", "fzf").actor("agent:codex"),
            change(13, "10:13", "bat").case(Some(B.into())),
            change(14, "10:14", "mdcat"),
            change(15, "10:15", "htop"),
        ];
        let mut note = Event::new(at("10:16"), Source::Seldon, Kind::Note, "glow");
        note.id = Ulid::from_parts(16, 0);
        ledger.push(note);
        let mut dismissed =
            Event::new(at("10:20"), Source::Seldon, Kind::Resolution, "mdcat").actor("human");
        dismissed.id = Ulid::from_parts(17, 0);
        dismissed.refers_to = Some(Ulid::from_parts(14, 0));
        dismissed.resolution = Some(Resolution::Dismissed);
        ledger.push(dismissed);
        let cases = [
            case(A, Risk::R1, "glow zed fzf bat mdcat htop"),
            case(B, Risk::R1, "htop"),
        ];
        let l = links(&ledger, &cases);
        assert_eq!(
            linked(&l),
            [(10, A.into()), (11, A.into()), (12, A.into())],
            "human, system, agent; bat has a case, mdcat a resolution, htop two cases"
        );
        let r = &l.lines[0];
        assert_eq!(
            (r.source, r.kind, r.actor.as_str(), r.resolution, r.ts),
            (
                Source::Seldon,
                Kind::Resolution,
                ACTOR_SYSTEM,
                Some(Resolution::Linked),
                ts(NOW)
            )
        );
        assert_eq!(
            r.detail.as_deref(),
            Some("planned by C-2026-001; active at the time")
        );
        assert_eq!(l.cases[A].events.len(), 3);
        assert_eq!(
            l.cases[A].log[0],
            "linked after the fact: pacman install glow at 10:10:00 \
             (planned here, no capture ran before the close)"
        );
        let htop = Ulid::from_parts(15, 0);
        assert_eq!(
            l.cases[B].log,
            [format!(
                "not linked: pacman install htop at 10:15:00 is planned here and in {A}, both \
                 active at the time; `seldon drift link {htop} <CASE>` links it"
            )]
        );
        assert!(
            l.cases[A]
                .log
                .iter()
                .any(|x| x.contains(&format!("here and in {B}, both")))
        );

        // linked: the next pass has nothing left
        let mut after = ledger.clone();
        for (i, mut line) in l.lines.into_iter().enumerate() {
            line.id = Ulid::from_parts(100 + i as u64, 0);
            after.push(line);
        }
        let again = links(&after, &cases);
        assert!(again.lines.is_empty());
        // an open case says so in the Log line; a later event time wins
        let mut late = change(20, "15:00", "glow");
        late.id = Ulid::from_parts(20, 0);
        let open = [PlanningCase {
            closed: false,
            ..case(A, Risk::R1, "glow")
        }];
        let l = links(&[step(1, "10:00", Kind::CaseStarted, A), late], &open);
        assert_eq!(l.lines[0].ts, at("15:00"));
        assert_eq!(
            l.cases[A].log,
            ["linked after the fact: pacman install glow at 15:00:00 (planned here)"]
        );
    }

    /// Acceptance 6: an `alwaysRed` subject links to an R3 case only;
    /// below R3 the case gets the R3 advisory.
    #[test]
    fn the_harm_guard_wants_r3() {
        use planned::*;
        const A: &str = "C-2026-001";
        let ledger = [
            step(1, "10:00", Kind::CaseStarted, A),
            change(10, "10:10", "linux-zen"),
        ];
        let l = links(&ledger, &[case(A, Risk::R3, "linux-zen")]);
        assert_eq!(linked(&l), [(10, A.into())]);
        let l = links(&ledger, &[case(A, Risk::R2, "linux-zen")]);
        assert!(l.lines.is_empty());
        assert_eq!(
            l.cases[A].log,
            [format!(
                "advisory: {}",
                r3_advisory(A, Risk::R2, "linux-zen")
            )]
        );
    }

    /// Acceptance 7: the explicit member decides, the dependencies follow
    /// (one line each, `meta.txId`); a dependency the Plan names is not
    /// tested on its own; dependencies of explicit members that go to two
    /// cases stay; a transaction without explicit members tests each.
    #[test]
    fn dependencies_follow_their_explicit_member() {
        use planned::*;
        const A: &str = "C-2026-001";
        const B: &str = "C-2026-002";
        let tx = |mut e: Event, tx: &str, explicit: bool| {
            e.tx_id = Some(tx.into());
            e.explicit = Some(explicit);
            e
        };
        let ledger = [
            step(1, "10:00", Kind::CaseStarted, A),
            step(2, "10:00", Kind::CaseStarted, B),
            tx(change(10, "10:10", "glow"), "t1", true),
            tx(change(11, "10:10", "libyaml"), "t1", false),
            tx(change(12, "10:10", "oniguruma"), "t1", false),
            tx(change(20, "10:20", "zed"), "t2", true),
            tx(change(21, "10:20", "bat"), "t2", true),
            tx(change(22, "10:20", "alsa-lib"), "t2", false),
            tx(change(30, "10:30", "lua"), "t3", false),
            tx(change(31, "10:30", "mdcat"), "t3", true),
            tx(change(40, "10:40", "fzf"), "t4", false),
        ];
        let cases = [
            case(A, Risk::R1, "glow, zed, lua, fzf"),
            case(B, Risk::R1, "bat"),
        ];
        let l = links(&ledger, &cases);
        assert_eq!(
            linked(&l),
            [
                (10, A.into()),
                (11, A.into()),
                (12, A.into()),
                (20, A.into()),
                (21, B.into()),
                (40, A.into()),
            ],
            "alsa-lib: zed and bat went to two cases; lua follows mdcat, which no Plan names"
        );
        let txs: Vec<Option<&str>> = l.lines.iter().map(|r| r.meta.tx_id.as_deref()).collect();
        assert_eq!(
            txs,
            [
                Some("t1"),
                Some("t1"),
                Some("t1"),
                Some("t2"),
                Some("t2"),
                None
            ]
        );
        assert_eq!(
            l.cases[A].log.len(),
            3,
            "glow, zed, fzf: {:?}",
            l.cases[A].log
        );
    }

    #[test]
    fn intent_goes_to_the_end_of_its_section() {
        let body = "# C — t\n\n## Intent\n<!-- Why? -->\n\n## Plan\n- Goal:\n";
        assert_eq!(
            append_to_section(body, "Intent", "Because."),
            "# C — t\n\n## Intent\n<!-- Why? -->\nBecause.\n\n## Plan\n- Goal:\n"
        );
        assert_eq!(
            append_to_section("## Intent", "Intent", "x"),
            "## Intent\nx\n"
        );
        assert_eq!(append_to_section("# t\n", "Intent", "x"), "# t\n");
    }

    #[test]
    fn event_ids_are_checked_by_form() {
        assert!(check_event_id("01M3VNFTF8EVHWFFZ687N14Q0C").is_ok());
        for bad in [
            "",
            "01m3vnftf8evhwffz687n14q0c",
            "81M3VNFTF8EVHWFFZ687N14Q0C",
            "01M3VNFTF8EVHWFFZ687N14Q0",
            "01M3VNFTF8EVHWFFZ687N14Q0I",
            "--json",
        ] {
            assert!(check_event_id(bad).is_err(), "{bad:?}");
        }
    }
}
