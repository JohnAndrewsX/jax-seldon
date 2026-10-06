//! The checks of a case's rollback snapshot (ADR-0027 §3, WP-101): the
//! snapshot exists, was taken after the case started and before the
//! case's first red change. Warnings only; the engine never refuses a
//! snapshot number, and it never runs `snapper`.
//!
//! When the snapshot was taken comes from its info file
//! (`<snapshots>/<N>/info.xml`, the ADR-0026 read grant), else from the
//! ledger's `snapshot` event of that number (the collector's record).

use std::path::Path;

use chrono::{DateTime, FixedOffset, NaiveDateTime};

use crate::collectors::Sources;
use crate::collectors::snapper::parse_info;
use crate::commands::doctor::SNAPPER_FIX;
use crate::config::Config;
use crate::error::Result;
use crate::ledger::Ledger;
use crate::logbook::Logbook;
use crate::model::Zone;
use crate::model::event::{Event, Kind, Source};
use crate::redact::Redactor;

/// What is known about snapshot `N`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Taken {
    /// It exists, taken at this instant.
    At(DateTime<FixedOffset>),
    /// It does not exist (any more).
    Missing,
    /// Neither the snapshot directory nor the ledger tells.
    Unknown,
}

/// The warnings for snapshot `n` as case `id`'s rollback. `after_start`:
/// also check that it is not older than the case's last `case-started`
/// event (`plan snapshot`; at `plan start --snapshot` the case has not
/// started yet).
pub fn checks(
    config: &Config,
    logbook: &Logbook,
    id: &str,
    n: u64,
    after_start: bool,
) -> Result<Vec<String>> {
    let events = Ledger::new(logbook, Redactor::for_config(config)?).read_all()?;
    let dir = Sources::from_env().snapshots;
    Ok(warnings(&events, &dir, id, n, after_start))
}

/// [`checks`] on events already read.
pub fn warnings(events: &[Event], dir: &Path, id: &str, n: u64, after_start: bool) -> Vec<String> {
    let at = match taken(events, dir, n) {
        Taken::At(t) => t,
        Taken::Missing => {
            return vec![format!(
                "snapshot {n} does not exist: neither {} nor the ledger has it; check the \
                 number (`snapper list`)",
                dir.display()
            )];
        }
        Taken::Unknown => {
            return vec![format!(
                "snapshot {n}: cannot check that it exists and when it was taken: {} is not \
                 readable and the ledger has no snapshot {n} yet (`seldon capture` records it); \
                 the read grant: {SNAPPER_FIX}",
                dir.display()
            )];
        }
    };
    let mut out = Vec::new();
    let mine = |e: &&Event| e.case.as_deref() == Some(id);
    if after_start
        && let Some(started) = events
            .iter()
            .filter(|e| {
                e.source == Source::Seldon && e.kind == Kind::CaseStarted && e.subject == id
            })
            .map(|e| e.ts)
            .max()
        && at < started
    {
        out.push(format!(
            "snapshot {n} was taken at {}, before {id} started at {}: it may be an older \
             snapshot, not this case's",
            stamp(at, started.offset()),
            stamp(started, started.offset())
        ));
    }
    if let Some(red) = events
        .iter()
        .filter(mine)
        .filter(|e| e.source != Source::Seldon && e.zone == Some(Zone::Red))
        .min_by_key(|e| e.ts)
        && at > red.ts
    {
        out.push(format!(
            "snapshot {n} was taken at {}, after {id}'s first red change (`{}` at {}): it does \
             not hold the state before that change",
            stamp(at, red.ts.offset()),
            red.subject,
            stamp(red.ts, red.ts.offset())
        ));
    }
    out
}

/// When snapshot `n` was taken: its info file decides when the directory
/// can be read (a readable directory without it: missing); else the last
/// `snapshot` event of the number in the ledger, unless a `snapshot-delete`
/// of it follows.
pub fn taken(events: &[Event], dir: &Path, n: u64) -> Taken {
    match info_date(dir, n) {
        Some(Some(t)) => return Taken::At(t),
        Some(None) => return Taken::Missing,
        None => {}
    }
    let subject = n.to_string();
    let last = events
        .iter()
        .filter(|e| e.source == Source::Snapper && e.subject == subject)
        .filter(|e| matches!(e.kind, Kind::Snapshot | Kind::SnapshotDelete))
        .max_by_key(|e| e.ts);
    match last {
        Some(e) if e.kind == Kind::Snapshot => Taken::At(e.ts),
        Some(_) => Taken::Missing,
        None => Taken::Unknown,
    }
}

/// The date of snapshot `n` from `dir/<n>/info.xml`: `Some(Some(t))`;
/// `Some(None)` when `dir` lists snapshots but not this one; `None` when
/// `dir` cannot be read, lists no snapshot at all (the empty subvolume
/// after booting into a snapshot) or the file cannot be read.
fn info_date(dir: &Path, n: u64) -> Option<Option<DateTime<FixedOffset>>> {
    let file = dir.join(n.to_string()).join("info.xml");
    match std::fs::read_to_string(&file) {
        Ok(text) => {
            let info = parse_info(&text, n).ok()?;
            let utc = NaiveDateTime::parse_from_str(&info.date, "%Y-%m-%d %H:%M:%S").ok()?;
            Some(Some(utc.and_utc().fixed_offset()))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            let numbered = std::fs::read_dir(dir)
                .ok()?
                .filter_map(|e| e.ok())
                .any(|e| {
                    e.file_name()
                        .to_str()
                        .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
                });
            numbered.then_some(None)
        }
        Err(_) => None,
    }
}

/// `t` as `YYYY-MM-DD HH:MM:SS` in `offset`.
fn stamp(t: DateTime<FixedOffset>, offset: &FixedOffset) -> String {
    t.with_timezone(offset)
        .format("%Y-%m-%d %H:%M:%S")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::event::Event;

    fn ts(s: &str) -> DateTime<FixedOffset> {
        DateTime::parse_from_rfc3339(s).unwrap()
    }

    fn snap(kind: Kind, n: &str, at: &str) -> Event {
        Event::new(ts(at), Source::Snapper, kind, n)
    }

    fn started(id: &str, at: &str) -> Event {
        Event::new(ts(at), Source::Seldon, Kind::CaseStarted, id).case(Some(id.into()))
    }

    fn red(id: &str, subject: &str, at: &str) -> Event {
        let mut e =
            Event::new(ts(at), Source::Pacman, Kind::Install, subject).case(Some(id.into()));
        e.zone = Some(Zone::Red);
        e
    }

    const NOWHERE: &str = "/nonexistent/seldon-test/.snapshots";

    #[test]
    fn the_ledger_tells_when_the_directory_cannot() {
        let dir = Path::new(NOWHERE);
        let events = [
            snap(Kind::Snapshot, "5", "2026-10-01T10:00:00+02:00"),
            snap(Kind::Snapshot, "6", "2026-10-01T10:00:00+02:00"),
            snap(Kind::SnapshotDelete, "6", "2026-10-02T10:00:00+02:00"),
        ];
        assert_eq!(
            taken(&events, dir, 5),
            Taken::At(ts("2026-10-01T10:00:00+02:00"))
        );
        assert_eq!(taken(&events, dir, 6), Taken::Missing);
        assert_eq!(taken(&events, dir, 7), Taken::Unknown);
        // a number used again after its delete is the new snapshot
        let reused = [
            events[1].clone(),
            events[2].clone(),
            snap(Kind::Snapshot, "6", "2026-10-03T10:00:00+02:00"),
        ];
        assert_eq!(
            taken(&reused, dir, 6),
            Taken::At(ts("2026-10-03T10:00:00+02:00"))
        );
    }

    #[test]
    fn the_info_file_decides_when_it_can_be_read() {
        let dir = std::env::temp_dir().join(format!("seldon-snapcheck-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("9")).unwrap();
        std::fs::write(
            dir.join("9/info.xml"),
            "<snapshot><type>single</type><num>9</num><date>2026-10-01 08:00:00</date></snapshot>",
        )
        .unwrap();
        // the ledger says otherwise: the file wins
        let events = [snap(Kind::SnapshotDelete, "9", "2026-10-01T12:00:00+02:00")];
        assert_eq!(
            taken(&events, &dir, 9),
            Taken::At(ts("2026-10-01T10:00:00+02:00"))
        );
        // the directory lists snapshots, not this one: missing, whatever
        // the ledger says
        let events = [snap(Kind::Snapshot, "10", "2026-10-01T12:00:00+02:00")];
        assert_eq!(taken(&events, &dir, 10), Taken::Missing);
        // an empty directory tells nothing: the ledger decides
        std::fs::remove_dir_all(dir.join("9")).unwrap();
        assert_eq!(
            taken(&events, &dir, 10),
            Taken::At(ts("2026-10-01T12:00:00+02:00"))
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_snapshot_must_lie_between_the_start_and_the_first_red_change() {
        let dir = Path::new(NOWHERE);
        let id = "C-2026-004";
        let base = vec![
            started(id, "2026-10-01T10:00:00+02:00"),
            red("C-2026-009", "zed", "2026-10-01T10:05:00+02:00"),
            red(id, "zed", "2026-10-01T11:00:00+02:00"),
            red(id, "zed-docs", "2026-10-01T12:00:00+02:00"),
        ];
        let with = |at: &str| {
            let mut v = base.clone();
            v.push(snap(Kind::Snapshot, "42", at));
            v
        };
        // in between: nothing to say
        assert!(warnings(&with("2026-10-01T10:30:00+02:00"), dir, id, 42, true).is_empty());
        // older than the start
        let w = warnings(&with("2026-10-01T09:59:59+02:00"), dir, id, 42, true);
        assert_eq!(
            w,
            [
                "snapshot 42 was taken at 2026-10-01 09:59:59, before C-2026-004 started at \
              2026-10-01 10:00:00: it may be an older snapshot, not this case's"
            ]
        );
        // `plan start --snapshot`: the start is not checked
        assert!(warnings(&with("2026-10-01T09:59:59+02:00"), dir, id, 42, false).is_empty());
        // after the first red change of this case (another case's does not count)
        let w = warnings(&with("2026-10-01T11:00:01+02:00"), dir, id, 42, false);
        assert_eq!(
            w,
            [
                "snapshot 42 was taken at 2026-10-01 11:00:01, after C-2026-004's first red \
              change (`zed` at 2026-10-01 11:00:00): it does not hold the state before that \
              change"
            ]
        );
        // exactly at the start or at the red change is fine
        assert!(warnings(&with("2026-10-01T10:00:00+02:00"), dir, id, 42, true).is_empty());
        assert!(warnings(&with("2026-10-01T11:00:00+02:00"), dir, id, 42, true).is_empty());
        // the last start counts
        let mut again = with("2026-10-01T10:30:00+02:00");
        again.push(started(id, "2026-10-01T10:45:00+02:00"));
        assert_eq!(warnings(&again, dir, id, 42, true).len(), 1);
        // missing and unknown say so, and nothing else
        let w = warnings(&base, dir, id, 42, true);
        assert!(
            w[0].contains("cannot check") && w[0].contains(SNAPPER_FIX),
            "{w:?}"
        );
        let mut gone = with("2026-10-01T10:30:00+02:00");
        gone.push(snap(
            Kind::SnapshotDelete,
            "42",
            "2026-10-01T10:40:00+02:00",
        ));
        let w = warnings(&gone, dir, id, 42, true);
        assert!(w.len() == 1 && w[0].contains("does not exist"), "{w:?}");
    }
}
