//! `snapper` collector (SPEC-ENGINE §4, ADR-0011).
//!
//! Runs `snapper --jsonout list` as the user, never with sudo. New snapshot
//! numbers become `snapshot` events (`ts` = the snapshot's date in the local
//! zone, `detail` = description, `meta.type`/`cleanup`, and `meta.pairOf` on
//! a `post`). Numbers that disappear become `snapshot-delete` events at
//! capture time. Snapshot 0 (`current`) is not a snapshot.
//!
//! The cursor is the set of known snapshots (number, type, description), so
//! a deletion can name what was deleted. Without a cursor, snapshots older
//! than the baseline are recorded as known without an event.
//!
//! Without `ALLOW_USERS`, snapper exits 1 with `No permissions.` on stderr.
//! The collector then degrades: `ok: false`, a message, and the one-line fix,
//! which is printed and never run. Nothing is invented and the cursor stays.
//! The same goes for a missing snapper or unreadable output.
//!
//! Only the `root` config is read (it is the only one on Omarchy; with one
//! config of another name, that one), because the event subject is the bare
//! snapshot number.

use std::collections::{BTreeMap, HashSet};

use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{Collector, Ctx, Outcome, to_cursor, typed_cursor};
use crate::commands::doctor::SNAPPER_FIX;
use crate::model::event::{Event, Kind, Meta, Source};
use crate::sys::Run;

pub struct Snapper;

/// One entry of `snapper --jsonout list` (`schema/external/snapper-list.schema.json`).
#[derive(Debug, Clone, Deserialize)]
pub struct Snapshot {
    pub number: u64,
    #[serde(rename = "type")]
    pub snapshot_type: String,
    #[serde(rename = "pre-number", default)]
    pub pre_number: Option<u64>,
    /// Local time without offset, empty for `current`.
    pub date: String,
    #[serde(default)]
    pub cleanup: String,
    #[serde(default)]
    pub description: String,
}

/// What the cursor remembers of a snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Known {
    #[serde(rename = "type")]
    pub snapshot_type: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
}

/// `cursors.json` → `snapper.cursor`: snapshot number → what it was.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapperCursor {
    pub known: BTreeMap<u64, Known>,
}

/// The degraded message for a permission error (index and doctor).
pub const NO_PERMISSIONS: &str = "snapper: No permissions. The snapper config does not list this user in ALLOW_USERS; see `seldon doctor`.";

impl Collector for Snapper {
    fn name(&self) -> &'static str {
        "snapper"
    }

    fn collect(&self, ctx: &Ctx, cursor: Option<&Value>) -> Outcome {
        let stdout = match ctx.run(&ctx.sources.snapper, &["--jsonout", "list"]) {
            Run::Exited {
                code: Some(0),
                stdout,
                ..
            } => stdout,
            Run::Exited { stderr, .. } if stderr.contains("No permissions") => {
                return Outcome::degraded(NO_PERMISSIONS, Some(SNAPPER_FIX.to_string()));
            }
            Run::Exited { code, stderr, .. } => {
                let code = code.map_or("a signal".to_string(), |c| format!("exit {c}"));
                return Outcome::degraded(
                    format!("snapper failed ({code}): {}", first_line(&stderr)),
                    None,
                );
            }
            Run::NotFound => return Outcome::degraded("snapper is not installed", None),
            Run::TimedOut => return Outcome::degraded("snapper did not answer in time", None),
            Run::Failed(e) => return Outcome::degraded(format!("cannot run snapper: {e}"), None),
        };
        let list = match parse_list(&stdout) {
            Ok(l) => l,
            Err(e) => return Outcome::degraded(format!("unexpected snapper output: {e}"), None),
        };
        match diff(ctx, typed_cursor(cursor), &list) {
            Ok((events, next)) => Outcome::ok(events, to_cursor(&next)),
            Err(e) => Outcome::degraded(format!("{e:#}"), None),
        }
    }
}

fn first_line(s: &str) -> &str {
    s.lines()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("")
        .trim()
}

/// The snapshots of the `root` config (or of the only config), without
/// snapshot 0.
pub fn parse_list(stdout: &str) -> anyhow::Result<Vec<Snapshot>> {
    let mut configs: BTreeMap<String, Vec<Snapshot>> = serde_json::from_str(stdout)?;
    let list = match configs.remove("root") {
        Some(l) => l,
        None if configs.len() == 1 => configs.into_values().next().unwrap_or_default(),
        None => anyhow::bail!("no `root` config in the list"),
    };
    Ok(list.into_iter().filter(|s| s.number != 0).collect())
}

/// Events for the difference between the cursor and `list`, and the next
/// cursor.
fn diff(
    ctx: &Ctx,
    cursor: Option<SnapperCursor>,
    list: &[Snapshot],
) -> anyhow::Result<(Vec<Event>, SnapperCursor)> {
    let first_run = cursor.is_none();
    let known = cursor.unwrap_or_default().known;
    let mut events = Vec::new();
    for s in list.iter().filter(|s| !known.contains_key(&s.number)) {
        let Some(ts) = NaiveDateTime::parse_from_str(&s.date, "%Y-%m-%d %H:%M:%S")
            .ok()
            .and_then(|n| ctx.tz.localize(n))
        else {
            continue; // no usable date: remembered, not reported
        };
        if first_run && ts < ctx.baseline {
            continue;
        }
        let mut e =
            Event::new(ts, Source::Snapper, Kind::Snapshot, s.number.to_string()).meta(Meta {
                snapshot_type: Some(s.snapshot_type.clone()),
                cleanup: Some(s.cleanup.clone()),
                pair_of: s.pre_number.filter(|_| s.snapshot_type == "post"),
                ..Meta::default()
            });
        if !s.description.is_empty() {
            e = e.detail(&s.description);
        }
        events.push(e);
    }
    let present: HashSet<u64> = list.iter().map(|s| s.number).collect();
    for (number, k) in known.iter().filter(|(n, _)| !present.contains(n)) {
        let mut e = Event::new(
            ctx.now,
            Source::Snapper,
            Kind::SnapshotDelete,
            number.to_string(),
        )
        .meta(Meta {
            snapshot_type: Some(k.snapshot_type.clone()),
            ..Meta::default()
        });
        if !k.description.is_empty() {
            e = e.detail(&k.description);
        }
        events.push(e);
    }
    let events = dedupe(ctx, events)?;
    let next = SnapperCursor {
        known: list
            .iter()
            .map(|s| {
                (
                    s.number,
                    Known {
                        snapshot_type: s.snapshot_type.clone(),
                        description: s.description.clone(),
                    },
                )
            })
            .collect(),
    };
    Ok((events, next))
}

/// Drops `snapshot` events already in the ledger (same number and time),
/// e.g. after a crash between the ledger write and the cursor save.
///
/// Likewise a `snapshot-delete` whose number the ledger already records as
/// deleted after its last creation: deletions carry capture time, so only
/// the subject can tell them apart.
fn dedupe(ctx: &Ctx, events: Vec<Event>) -> anyhow::Result<Vec<Event>> {
    let created = || {
        events
            .iter()
            .filter(|e| e.kind == Kind::Snapshot)
            .map(|e| e.ts)
    };
    let seen: HashSet<(String, i64)> = match (created().min(), created().max()) {
        (Some(first), Some(last)) => ctx
            .ledger
            .read_range(first, last)?
            .into_iter()
            .filter(|e| e.source == Source::Snapper && e.kind == Kind::Snapshot)
            .map(|e| (e.subject, e.ts.timestamp()))
            .collect(),
        _ => HashSet::new(),
    };
    let deleted: HashSet<String> = if events.iter().any(|e| e.kind == Kind::SnapshotDelete) {
        // the latest snapper event per number, over the whole ledger
        let mut last: BTreeMap<String, Event> = BTreeMap::new();
        for e in ctx.ledger.read_all()? {
            if e.source == Source::Snapper && last.get(&e.subject).is_none_or(|l| l.ts <= e.ts) {
                last.insert(e.subject.clone(), e);
            }
        }
        last.into_values()
            .filter(|e| e.kind == Kind::SnapshotDelete)
            .map(|e| e.subject)
            .collect()
    } else {
        HashSet::new()
    };
    Ok(events
        .into_iter()
        .filter(|e| match e.kind {
            Kind::Snapshot => !seen.contains(&(e.subject.clone(), e.ts.timestamp())),
            Kind::SnapshotDelete => !deleted.contains(&e.subject),
            _ => true,
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_fixture_list() {
        let text = include_str!("../../../fixtures/logs/snapper.json");
        let list = parse_list(text).unwrap();
        assert_eq!(
            list.iter().map(|s| s.number).collect::<Vec<_>>(),
            [1, 105, 106, 107, 110, 111, 112, 113, 114, 115]
        );
        let before =
            parse_list(include_str!("../../../fixtures/logs/snapper-before.json")).unwrap();
        let post = before.iter().find(|s| s.number == 109).unwrap();
        assert_eq!(
            (post.snapshot_type.as_str(), post.pre_number),
            ("post", Some(108))
        );
        assert!(parse_list("{}").is_err());
        assert!(parse_list("No permissions.").is_err());
    }
}
