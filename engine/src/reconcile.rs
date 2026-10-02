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
//!   ADR-0013 §4): [`select`] picks the open members a command resolves,
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

use std::collections::{BTreeMap, HashMap};

use chrono::{DateTime, Duration, FixedOffset};
use ulid::Ulid;

use crate::collectors::config::OwnWrites;
use crate::error::{Error, Result};
use crate::index::Built;
use crate::index::model::IndexEvent;
use crate::ledger::Ledger;
use crate::logbook::Logbook;
use crate::logbook::cases::{self, CaseFile};
use crate::logbook::lock::Lock;
use crate::model::event::{ACTOR_SYSTEM, Event, Kind, Meta, Resolution, Source};
use crate::model::is_ulid;

/// What a `drift link|explain|dismiss` on one event resolves.
#[derive(Debug, Clone)]
pub struct Selection<'a> {
    /// The named event as the index sees it (resolution folded).
    pub event: &'a IndexEvent,
    /// The open members to resolve, oldest first: the named event and, for
    /// a pacman event without `--only`, every other open member of its
    /// transaction. Empty when the named event is not open drift.
    pub members: Vec<&'a Event>,
    /// The transaction's `txId` when the write fans out over two or more
    /// members (`meta.txId` of every line, ADR-0013 §4).
    pub group: Option<&'a str>,
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

/// The open drift members of `e`'s drift item, oldest first: for an open
/// pacman event with a `txId`, every open pacman event of that transaction
/// (ADR-0013 §1); for any other open event, the event itself; nothing when
/// `e` is not open drift.
pub fn open_members<'a>(built: &'a Built, e: &Event) -> Vec<&'a Event> {
    if !built.open_drift.contains(&e.id) {
        return Vec::new();
    }
    let tx = e.tx_id.as_deref().filter(|_| e.source == Source::Pacman);
    let mut members: Vec<&Event> = built
        .folded
        .iter()
        .map(|f| &f.event)
        .filter(|m| built.open_drift.contains(&m.id))
        .filter(|m| match tx {
            Some(tx) => m.source == Source::Pacman && m.tx_id.as_deref() == Some(tx),
            None => m.id == e.id,
        })
        .collect();
    members.sort_by_key(|m| (m.ts, m.id));
    members
}

/// What a command on `id` resolves: the open members of its item, or the
/// event alone with `only` (ADR-0013 §4). A named event that is no longer
/// open selects nothing, so a re-run writes nothing, also after `--only`
/// (the remaining members are a new item with a leader of their own).
/// Exit 1 for an unknown id and for an event that can never be drift
/// (ADR-0012 §6).
pub fn select<'a>(built: &'a Built, id: &str, only: bool) -> Result<Selection<'a>> {
    let event = find(built, id)?;
    let e = &event.event;
    if !e.source.is_drift_eligible() {
        return Err(Error::user(format!(
            "{id} is a {}/{} event; only pacman, omarchy, plugins, theme and config events can be drift",
            e.source, e.kind
        )));
    }
    let mut members = open_members(built, e);
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
/// removal of the content deleted); `source: seldon`, actor `system`, no
/// case, detail `installed by <command>` (`removed by <command>` for a
/// removal command), at `ts`.
pub fn own_write_resolutions(
    written: &[Event],
    own: &OwnWrites,
    ts: DateTime<FixedOffset>,
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
            let w = own.explaining(e)?;
            let mut r = Event::new(ts, Source::Seldon, Kind::Resolution, e.subject.clone())
                .actor(ACTOR_SYSTEM)
                .detail(format!("{} by {}", w.op.verb(), w.by));
            r.refers_to = Some(e.id);
            r.resolution = Some(Resolution::Explained);
            Some(r)
        })
        .collect()
}

/// After a capture that ran the config collector (SPEC-ENGINE §5 rule 7):
/// appends the [`own_write_resolutions`] of `written` and forgets every
/// recorded own write, explained or not (the collector has now seen each
/// file: as an event, in its baseline, or changed by someone else).
/// Returns how many events were explained, and warnings: the append
/// already happened, so nothing here fails the capture.
pub fn explain_own_writes(
    lock: &Lock,
    ledger: &Ledger,
    file: &std::path::Path,
    written: &[Event],
    ts: DateTime<FixedOffset>,
) -> (usize, Vec<String>) {
    let own = match OwnWrites::load(file) {
        Ok(own) if own.0.is_empty() => return (0, Vec::new()),
        Ok(own) => own,
        Err(e) => return (0, vec![format!("own writes not read: {e}")]),
    };
    let mut warnings = Vec::new();
    let lines = own_write_resolutions(written, &own, ts);
    let explained = match ledger.append(lock, lines) {
        Ok(lines) => lines.len(),
        Err(e) => {
            warnings.push(format!("own writes not explained: {e}"));
            0
        }
    };
    if let Err(e) = OwnWrites::default().save(file) {
        warnings.push(format!("{e:#}"));
    }
    (explained, warnings)
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
