//! What a capture tells the cases after its append (ADR-0027 §2c, §3,
//! WP-101). Three notes, each a line in the case's `## Log`, written once:
//!
//! - **The rollback the agent forgot.** A new `snapshot` (not a `post`)
//!   fills an open case's empty `snapshotBefore`: the case whose id is the
//!   snapshot's description (the rules have the agent write
//!   `-d "<ID>"`), else the case of the agent's recorded `snapper …
//!   create` / `omarchy-snapshot create` (hook `command` events, subject
//!   [`SNAPSHOT_SUBJECT`]) in the [`ATTRIBUTION_WINDOW`] before it.
//! - **A pruned rollback.** A `snapshot-delete` of a number that is some
//!   case's `snapshotBefore` (not dropped): `rollback for <ID> pruned
//!   (snapshot N)`; `seldon doctor` shows it too.
//! - **R3 after the fact.** A red change whose subject is in `[drift]
//!   alwaysRed` inside an open case below R3 (the index warns too,
//!   [`r3_advisory`]).
//!
//! The ledger write already happened, so nothing here fails the capture:
//! a problem is a warning.

use std::collections::BTreeMap;

use chrono::{DateTime, Duration, FixedOffset};

use crate::attribution::ATTRIBUTION_WINDOW;
use crate::commands::hook::SNAPSHOT_SUBJECT;
use crate::index::build::r3_advisory;
use crate::index::drift::AlwaysRed;
use crate::ledger::Ledger;
use crate::logbook::Logbook;
use crate::logbook::cases::{self, CaseFile};
use crate::model::event::{ACTOR_SYSTEM, Event, Kind, Source};
use crate::model::{CaseStatus, Risk, Zone, is_case_id};

/// How long after the recorded command the snapshot may carry its date:
/// the hook records the command before it runs, snapper dates the
/// snapshot when it is made.
const AFTER: i64 = 120;

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
    let created: Vec<&Event> = written
        .iter()
        .filter(|e| e.source == Source::Snapper && e.kind == Kind::Snapshot)
        .filter(|e| e.meta.snapshot_type.as_deref() != Some("post"))
        .collect();
    let commands = match created.iter().map(|e| e.ts).min() {
        Some(from) => {
            let to = created.iter().map(|e| e.ts).max().unwrap_or(from);
            match ledger.read_range(from - ATTRIBUTION_WINDOW, to + Duration::seconds(AFTER)) {
                Ok(events) => events,
                Err(e) => {
                    warnings.push(format!("snapshots not matched to cases: {e:#}"));
                    Vec::new()
                }
            }
        }
        None => Vec::new(),
    };
    let mut filled: Vec<&str> = Vec::new();
    for snap in &created {
        let Ok(n) = snap.subject.parse::<u64>() else {
            continue;
        };
        let Some((id, actor, why)) = owner(snap, &commands) else {
            continue;
        };
        let open = by_id.get(id.as_str()).is_some_and(|f| {
            matches!(
                f.case.status,
                CaseStatus::Queued | CaseStatus::Active | CaseStatus::Verification
            ) && f.case.snapshot_before.is_none()
        });
        if open && !filled.contains(&by_id[id.as_str()].case.id.as_str()) {
            filled.push(by_id[id.as_str()].case.id.as_str());
            notes.entry(id).or_default().push(Note::Snapshot {
                n,
                text: format!("snapshot {n} ({why})"),
                actor,
            });
        }
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

/// The case a new snapshot belongs to: its description, when that is a
/// case id; else the case of the last recorded snapshot command in the
/// window before it. (id, actor of the Log line, why).
fn owner(snap: &Event, commands: &[Event]) -> Option<(String, String, String)> {
    if let Some(d) = snap.detail.as_deref().map(str::trim)
        && is_case_id(d)
    {
        return Some((
            d.to_string(),
            ACTOR_SYSTEM.to_string(),
            "its description names the case".to_string(),
        ));
    }
    commands
        .iter()
        .filter(|c| c.source == Source::Agent && c.kind == Kind::Command)
        .filter(|c| c.subject == SNAPSHOT_SUBJECT && c.case.is_some())
        .filter(|c| {
            c.ts >= snap.ts - ATTRIBUTION_WINDOW && c.ts <= snap.ts + Duration::seconds(AFTER)
        })
        .max_by_key(|c| c.ts)
        .map(|c| {
            (
                c.case.clone().unwrap_or_default(),
                c.actor.clone(),
                "from the recorded snapshot command".to_string(),
            )
        })
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
