//! What a capture tells the cases after its append (ADR-0027 §2c, §3,
//! WP-101). Three notes, each a line in the case's `## Log`, written once:
//!
//! - **The rollback the agent forgot.** A new `snapshot` (not a `post`)
//!   fills an open case's empty `snapshotBefore`: the case whose id is the
//!   snapshot's description (the rules have the agent write
//!   `-d "<ID>"`), else the case of the agent's recorded `snapper …
//!   create` / `omarchy-snapshot create` (hook `command` events, subject
//!   [`SNAPSHOT_SUBJECT`]) in its window, `[date − ATTRIBUTION_WINDOW,
//!   date + SKEW]`; one command owns one snapshot, and commands of two
//!   cases in one window fill nothing but tell each case (WP-101 round 2).
//!   `plan snapshot`'s checks follow as Log lines.
//! - **A pruned rollback.** A `snapshot-delete` of a number that is some
//!   case's `snapshotBefore` (not dropped): `rollback for <ID> pruned
//!   (snapshot N)`; `seldon doctor` shows it too.
//! - **R3 after the fact.** A red change whose subject is in `[drift]
//!   alwaysRed` inside an open case below R3 (the index warns too,
//!   [`r3_advisory`]).
//!
//! The ledger write already happened, so nothing here fails the capture:
//! a problem is a warning.

use std::collections::{BTreeMap, BTreeSet, HashSet};

use chrono::{DateTime, Duration, FixedOffset};

use crate::attribution::ATTRIBUTION_WINDOW;
use crate::collectors::Sources;
use crate::commands::hook::SNAPSHOT_SUBJECT;
use crate::commands::plan::snapshot::warnings as snapshot_checks;
use crate::index::build::r3_advisory;
use crate::index::drift::AlwaysRed;
use crate::ledger::Ledger;
use crate::logbook::Logbook;
use crate::logbook::cases::{self, CaseFile};
use crate::model::event::{ACTOR_SYSTEM, Event, Kind, Source};
use crate::model::{CaseStatus, Risk, Zone, is_case_id};

/// How far a snapshot's date may lie *before* the command that made it:
/// snapper cuts its date to the second, the hook stamps the command when
/// it is about to run (WP-101 round 2: 5 s, not minutes, so a later
/// agent's command never claims an earlier snapshot).
const SKEW: i64 = 5;

/// The notes of one capture's `written` events; the warnings.
pub fn after_capture(
    logbook: &Logbook,
    ledger: &Ledger,
    written: &[Event],
    always_red: &AlwaysRed,
    now: &DateTime<FixedOffset>,
) -> Vec<String> {
    let mut notes: BTreeMap<String, Vec<Note>> = BTreeMap::new();
    let mut warnings = Vec::new();
    let (all, _) = match cases::all(logbook) {
        Ok(all) => all,
        Err(e) => return vec![format!("case notes not written: {e}")],
    };
    let by_id: BTreeMap<&str, &CaseFile> = all.iter().map(|f| (f.case.id.as_str(), f)).collect();

    // the rollback the agent forgot
    let mut created: Vec<&Event> = written
        .iter()
        .filter(|e| e.source == Source::Snapper && e.kind == Kind::Snapshot)
        .filter(|e| e.meta.snapshot_type.as_deref() != Some("post"))
        .collect();
    created.sort_by_key(|e| (e.ts, e.id));
    let commands: Vec<Event> = match (created.first(), created.last()) {
        (Some(first), Some(last)) => {
            match ledger.read_range(
                first.ts - ATTRIBUTION_WINDOW,
                last.ts + Duration::seconds(SKEW),
            ) {
                Ok(events) => events
                    .into_iter()
                    .filter(|c| c.source == Source::Agent && c.kind == Kind::Command)
                    .filter(|c| c.subject == SNAPSHOT_SUBJECT && c.case.is_some())
                    .collect(),
                Err(e) => {
                    warnings.push(format!("snapshots not matched to cases: {e:#}"));
                    Vec::new()
                }
            }
        }
        _ => Vec::new(),
    };
    // a recorded command makes one snapshot: once it owns one, it is used
    let mut used: HashSet<ulid::Ulid> = HashSet::new();
    let mut filled: Vec<String> = Vec::new();
    let mut ledger_events: Option<Vec<Event>> = None;
    let fillable = |id: &str| {
        by_id.get(id).is_some_and(|f| {
            matches!(
                f.case.status,
                CaseStatus::Queued | CaseStatus::Active | CaseStatus::Verification
            ) && f.case.snapshot_before.is_none()
        })
    };
    for snap in &created {
        let Ok(n) = snap.subject.parse::<u64>() else {
            continue;
        };
        let (id, actor, why) = match owner(snap, &commands, &mut used) {
            Owner::One { id, actor, why } => (id, actor, why),
            Owner::None => continue,
            Owner::Several(ids) => {
                // two agents' snapshot commands in one window: the engine
                // cannot tell whose this is, so it fills nothing and says so
                for id in ids.iter().filter(|id| fillable(id)) {
                    notes.entry(id.clone()).or_default().push(Note::Line(
                        format!(
                            "snapshot {n} was taken while the agents of {} ran a snapshot command; \
                             if it is this case's rollback, record it: `seldon plan snapshot {id} {n}`",
                            ids.join(" and ")
                        ),
                        ACTOR_SYSTEM.into(),
                    ));
                }
                continue;
            }
        };
        if !fillable(&id) || filled.contains(&id) {
            continue;
        }
        filled.push(id.clone());
        // `plan snapshot`'s checks; a warning is a Log line, never a refusal
        let events = match &ledger_events {
            Some(events) => events,
            None => match ledger.read_all() {
                Ok(events) => ledger_events.insert(events),
                Err(e) => {
                    warnings.push(format!("snapshot {n} not checked: {e:#}"));
                    ledger_events.insert(Vec::new())
                }
            },
        };
        let checks = snapshot_checks(events, &Sources::from_env().snapshots, &id, n, true);
        let entry = notes.entry(id).or_default();
        entry.push(Note::Snapshot {
            n,
            text: format!("snapshot {n} ({why})"),
            actor,
        });
        entry.extend(
            checks
                .into_iter()
                .map(|w| Note::Line(w, ACTOR_SYSTEM.into())),
        );
    }

    // a pruned rollback
    for del in written
        .iter()
        .filter(|e| e.source == Source::Snapper && e.kind == Kind::SnapshotDelete)
    {
        let Ok(n) = del.subject.parse::<u64>() else {
            continue;
        };
        for f in all.iter().filter(|f| {
            f.case.snapshot_before == Some(n)
                && f.case.status != CaseStatus::Dropped
                && del.ts.date_naive() >= f.case.created
        }) {
            notes
                .entry(f.case.id.clone())
                .or_default()
                .push(Note::Line(pruned_line(&f.case.id, n), ACTOR_SYSTEM.into()));
        }
    }

    // R3 after the fact
    for e in written {
        let Some(f) = e.case.as_deref().and_then(|id| by_id.get(id)) else {
            continue;
        };
        if e.source != Source::Seldon
            && e.zone == Some(Zone::Red)
            && matches!(f.case.status, CaseStatus::Active | CaseStatus::Verification)
            && f.case.risk != Risk::R3
            && always_red.matches(&e.subject)
        {
            notes.entry(f.case.id.clone()).or_default().push(Note::Line(
                format!(
                    "advisory: {}",
                    r3_advisory(&f.case.id, f.case.risk, &e.subject)
                ),
                ACTOR_SYSTEM.into(),
            ));
        }
    }

    for (id, notes) in notes {
        if let Err(e) = apply(logbook, &id, &notes, now) {
            warnings.push(format!("{id}: case notes not written: {e}"));
        }
    }
    warnings
}

/// The Log line of a pruned rollback.
pub fn pruned_line(id: &str, n: u64) -> String {
    format!("rollback for {id} pruned (snapshot {n})")
}

enum Note {
    /// Fill `snapshotBefore` (if still empty) with a Log line.
    Snapshot { n: u64, text: String, actor: String },
    /// A Log line, unless the case's Log has it already.
    Line(String, String),
}

/// Whose a new snapshot is.
enum Owner {
    One {
        id: String,
        actor: String,
        why: String,
    },
    /// Recorded commands of these cases (sorted) are in its window.
    Several(Vec<String>),
    None,
}

/// The case a new snapshot belongs to: its description, when that is a
/// case id; else the case of the recorded snapshot commands not yet used
/// in its window, `[date − ATTRIBUTION_WINDOW, date + SKEW]`, the latest
/// one owning it. Commands of more than one case there: [`Owner::Several`].
/// The command that owns a snapshot (also the latest of the named case
/// when the description decides) is marked used.
fn owner(snap: &Event, commands: &[Event], used: &mut HashSet<ulid::Ulid>) -> Owner {
    let window: Vec<&Event> = commands
        .iter()
        .filter(|c| !used.contains(&c.id))
        .filter(|c| {
            c.ts >= snap.ts - ATTRIBUTION_WINDOW && c.ts <= snap.ts + Duration::seconds(SKEW)
        })
        .collect();
    let latest_of = |id: &str| {
        window
            .iter()
            .filter(|c| c.case.as_deref() == Some(id))
            .max_by_key(|c| (c.ts, c.id))
            .map(|c| c.id)
    };
    if let Some(d) = snap.detail.as_deref().and_then(described_case) {
        if let Some(c) = latest_of(d) {
            used.insert(c);
        }
        return Owner::One {
            id: d.to_string(),
            actor: ACTOR_SYSTEM.to_string(),
            why: "its description names the case".to_string(),
        };
    }
    let cases: BTreeSet<&str> = window.iter().filter_map(|c| c.case.as_deref()).collect();
    match cases.len() {
        0 => Owner::None,
        1 => {
            let c = window
                .iter()
                .max_by_key(|c| (c.ts, c.id))
                .expect("one case has a command");
            used.insert(c.id);
            Owner::One {
                id: c.case.clone().unwrap_or_default(),
                actor: c.actor.clone(),
                why: "from the recorded snapshot command".to_string(),
            }
        }
        _ => Owner::Several(cases.into_iter().map(String::from).collect()),
    }
}

/// The case a snapshot description names: the whole description, or its
/// start followed by `:` or white space (`C-2026-001: Install zed`, the
/// ADR-0027 §3 wording; WP-101 round 3).
fn described_case(description: &str) -> Option<&str> {
    let d = description.trim();
    let end = d
        .find(|c: char| c == ':' || c.is_whitespace())
        .unwrap_or(d.len());
    let id = &d[..end];
    is_case_id(id).then_some(id)
}

/// Writes `notes` into case `id` under the capture's lock: once each.
fn apply(
    logbook: &Logbook,
    id: &str,
    notes: &[Note],
    now: &DateTime<FixedOffset>,
) -> crate::error::Result<()> {
    let mut file = cases::find(logbook, id)?;
    let log = cases::section(&file.doc.body, "Log")
        .map_or(String::new(), |r| file.doc.body[r].to_string());
    let mut added: Vec<&str> = Vec::new();
    for note in notes {
        match note {
            Note::Snapshot { n, text, actor } => {
                if file.case.snapshot_before.is_none() {
                    file.case.snapshot_before = Some(*n);
                    file.log(now, text, actor);
                    added.push(text);
                }
            }
            Note::Line(text, actor) => {
                if !log.contains(text.as_str()) && !added.contains(&text.as_str()) {
                    file.log(now, text, actor);
                    added.push(text);
                }
            }
        }
    }
    if !added.is_empty() {
        file.save(logbook)?;
    }
    Ok(())
}
