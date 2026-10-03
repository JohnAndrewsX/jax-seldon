//! `ledger/YYYY-MM.jsonl`: the append-only event log (SPEC-LOGBOOK §4,
//! ADR-0003).
//!
//! The ledger is the only way events reach the logbook. [`Ledger::append`]
//! assigns each event a fresh ULID (monotonic within one call, so ids sort
//! in write order), redacts `subject`, `detail` and every string value of
//! `meta` (SPEC-ENGINE §7), validates it against the schema rules, and
//! appends it to the month file of its `ts` (in the event's own offset).
//! Lines are never rewritten; corrections are new events.
//!
//! Appending needs the state lock (`logbook::lock`), which the caller takes
//! once per command; the `&Lock` parameter proves it is held.

use std::collections::BTreeMap;
use std::fs::OpenOptions;
use std::io::{Read as _, Seek as _, SeekFrom, Write as _};
use std::os::unix::fs::OpenOptionsExt as _;
use std::path::{Path, PathBuf};

use anyhow::Context as _;
use chrono::{DateTime, Duration, FixedOffset};
use ulid::{Generator, Ulid};

use crate::error::Result;
use crate::logbook::Logbook;
use crate::logbook::lock::Lock;
use crate::model::event::{DETAIL_MAX, Event, Meta, SUBJECT_MAX, is_actor};
use crate::model::is_case_id;
use crate::redact::Redactor;

/// `ledger/`, relative to the logbook root.
pub const LEDGER_DIR: &str = "ledger";

/// The ledger of one logbook.
#[derive(Debug, Clone)]
pub struct Ledger {
    dir: PathBuf,
    redactor: Redactor,
}

/// One month file as read: its events, and the 1-based numbers of lines that
/// are not valid events (a torn write, a hand edit). Readers skip those.
/// A line is bad when it is not UTF-8 (a write torn inside a multi-byte
/// character, F-132), not an event, or names an actor or a case that
/// `append` would refuse (`event.schema.json`).
#[derive(Debug, Clone, Default)]
pub struct MonthFile {
    pub events: Vec<Event>,
    pub bad_lines: Vec<usize>,
}

impl Ledger {
    pub fn new(logbook: &Logbook, redactor: Redactor) -> Self {
        Ledger::at(logbook.path(LEDGER_DIR), redactor)
    }

    /// A ledger in `dir` (tests).
    pub fn at(dir: impl Into<PathBuf>, redactor: Redactor) -> Self {
        Ledger {
            dir: dir.into(),
            redactor,
        }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn redactor(&self) -> &Redactor {
        &self.redactor
    }

    /// `ledger/<month>.jsonl`.
    pub fn month_file(&self, month: &str) -> PathBuf {
        self.dir.join(format!("{month}.jsonl"))
    }

    /// Months that have a `.jsonl` file, sorted (`YYYY-MM`).
    pub fn months(&self) -> anyhow::Result<Vec<String>> {
        let read = match std::fs::read_dir(&self.dir) {
            Ok(r) => r,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => {
                return Err(
                    anyhow::Error::new(e).context(format!("cannot list {}", self.dir.display()))
                );
            }
        };
        let mut months = Vec::new();
        for entry in read {
            let name = entry?.file_name().to_string_lossy().into_owned();
            if let Some(m) = name.strip_suffix(".jsonl")
                && is_month(m)
            {
                months.push(m.to_string());
            }
        }
        months.sort();
        Ok(months)
    }

    /// Reads one month file; a missing file is empty. The file is read as
    /// bytes and decoded line by line, so one bad line is skipped and
    /// counted in [`MonthFile::bad_lines`] instead of failing the month.
    pub fn read_month(&self, month: &str) -> anyhow::Result<MonthFile> {
        let path = self.month_file(month);
        let bytes = match std::fs::read(&path) {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(MonthFile::default()),
            Err(e) => {
                return Err(
                    anyhow::Error::new(e).context(format!("cannot read {}", path.display()))
                );
            }
        };
        let mut file = MonthFile::default();
        for (n, raw) in bytes.split(|&b| b == b'\n').enumerate() {
            let raw = raw.strip_suffix(b"\r").unwrap_or(raw);
            if raw.trim_ascii().is_empty() {
                continue;
            }
            match std::str::from_utf8(raw)
                .ok()
                .and_then(|line| serde_json::from_str::<Event>(line).ok())
                .filter(loadable)
            {
                Some(e) => file.events.push(e),
                None => file.bad_lines.push(n + 1),
            }
        }
        Ok(file)
    }

    /// The bad lines of every month that has one (`month`, line numbers),
    /// months ascending: what `doctor` reports (WP-070).
    pub fn bad_lines(&self) -> anyhow::Result<Vec<(String, Vec<usize>)>> {
        let mut out = Vec::new();
        for m in self.months()? {
            let bad = self.read_month(&m)?.bad_lines;
            if !bad.is_empty() {
                out.push((m, bad));
            }
        }
        Ok(out)
    }

    /// Every event of every month, in file order (months ascending).
    pub fn read_all(&self) -> anyhow::Result<Vec<Event>> {
        let mut out = Vec::new();
        for m in self.months()? {
            out.extend(self.read_month(&m)?.events);
        }
        Ok(out)
    }

    /// Events whose instant lies in `[from, to]`, in file order. Reads only
    /// the month files that can hold them (a day of slack on both sides,
    /// because a month file is chosen by the event's own offset).
    pub fn read_range(
        &self,
        from: DateTime<FixedOffset>,
        to: DateTime<FixedOffset>,
    ) -> anyhow::Result<Vec<Event>> {
        let first = (from - Duration::days(1)).format("%Y-%m").to_string();
        let last = (to + Duration::days(1)).format("%Y-%m").to_string();
        let mut out = Vec::new();
        for m in self.months()? {
            if m >= first && m <= last {
                out.extend(
                    self.read_month(&m)?
                        .events
                        .into_iter()
                        .filter(|e| e.ts >= from && e.ts <= to),
                );
            }
        }
        Ok(out)
    }

    /// Appends `events` and returns them as written (with ids, redacted).
    ///
    /// Every event gets a new ULID, whatever its `id` was. `subject`,
    /// `detail` and every string value of `meta` are redacted; `subject` and
    /// `detail` are cut to the schema's limits.
    /// If any event is invalid nothing is written (engine error). Lines go
    /// to the month file of each event's `ts`, one `write` per file.
    pub fn append(&self, _lock: &Lock, events: Vec<Event>) -> Result<Vec<Event>> {
        if events.is_empty() {
            return Ok(events);
        }
        let mut ids = Generator::new();
        let mut written = Vec::with_capacity(events.len());
        for mut e in events {
            e.id = next_id(&mut ids);
            e.subject = truncate(&self.redactor.redact(&e.subject), SUBJECT_MAX);
            if let Some(d) = &e.detail {
                e.detail = Some(truncate(&self.redactor.redact(d), DETAIL_MAX));
            }
            redact_meta(&self.redactor, &mut e.meta);
            e.validate().map_err(|msg| {
                anyhow::anyhow!("refusing to write an invalid event ({e}): {msg}")
            })?;
            written.push(e);
        }

        let mut by_month: BTreeMap<String, String> = BTreeMap::new();
        for e in &written {
            let text = by_month.entry(e.month()).or_default();
            text.push_str(&e.to_line());
            text.push('\n');
        }
        crate::sys::create_dir_private(&self.dir)
            .with_context(|| format!("cannot create {}", self.dir.display()))?;
        for (month, text) in by_month {
            append_file(&self.month_file(&month), text.as_bytes())?;
        }
        Ok(written)
    }
}

/// Whether a parsed line can be used: its actor and case are what `append`
/// writes (`Event::validate`), so a hand-edited `actor` or `case` cannot
/// reach the views, links and paths built from them (WP-059 review).
fn loadable(e: &Event) -> bool {
    is_actor(&e.actor) && e.case.as_deref().is_none_or(is_case_id)
}

/// The next id of a monotonic generator (on the practically impossible
/// overflow of 80 random bits, a fresh random id).
fn next_id(ids: &mut Generator) -> Ulid {
    ids.generate()
        .unwrap_or_else(|o| o.commit_overflow_random())
}

/// Appends `bytes` to `path`. If the file does not end in a newline (a torn
/// earlier write), a newline goes first so the new lines stay whole.
fn append_file(path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    let mut file = OpenOptions::new()
        .create(true)
        .read(true)
        .append(true)
        .mode(crate::sys::NEW_FILE_MODE)
        .open(path)
        .with_context(|| format!("cannot open {}", path.display()))?;
    let len = file.metadata()?.len();
    let mut out = Vec::with_capacity(bytes.len() + 1);
    if len > 0 {
        let mut last = [0u8; 1];
        file.seek(SeekFrom::Start(len - 1))?;
        file.read_exact(&mut last)?;
        if last[0] != b'\n' {
            out.push(b'\n');
        }
    }
    out.extend_from_slice(bytes);
    file.write_all(&out)
        .and_then(|()| file.sync_data())
        .with_context(|| format!("cannot append to {}", path.display()))
}

/// Every string value of `meta`, the conventional keys and the others,
/// through `redactor` (meta values are scalars, `event.schema.json`).
fn redact_meta(redactor: &Redactor, meta: &mut Meta) {
    for value in [
        &mut meta.command,
        &mut meta.version,
        &mut meta.from,
        &mut meta.to,
        &mut meta.hash_from,
        &mut meta.hash_to,
        &mut meta.snapshot_type,
        &mut meta.cleanup,
        &mut meta.tx_id,
    ]
    .into_iter()
    .flatten()
    {
        *value = redactor.redact(value);
    }
    for value in meta.extra.values_mut() {
        if let serde_json::Value::String(s) = value {
            *s = redactor.redact(s);
        }
    }
}

fn truncate(s: &str, max_chars: usize) -> String {
    match s.char_indices().nth(max_chars) {
        Some((i, _)) => {
            let mut t = s[..i].to_string();
            t.pop();
            t.push('…');
            t
        }
        None => s.to_string(),
    }
}

fn is_month(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 7
        && b[4] == b'-'
        && b[..4].iter().all(u8::is_ascii_digit)
        && b[5..].iter().all(u8::is_ascii_digit)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logbook::lock;
    use crate::model::event::{Kind, Meta, Source};

    fn tmp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("seldon-ledger-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn ts(s: &str) -> DateTime<FixedOffset> {
        DateTime::parse_from_rfc3339(s).unwrap()
    }

    #[test]
    fn append_rolls_over_by_ts_redacts_and_assigns_ids() {
        let dir = tmp("append");
        let lock = lock::acquire(&dir.join("lock")).unwrap();
        let ledger = Ledger::at(dir.join("ledger"), Redactor::builtin());
        let events = vec![
            Event::new(
                ts("2026-09-30T23:59:59+02:00"),
                Source::Pacman,
                Kind::Install,
                "a",
            )
            .detail("1-1"),
            Event::new(
                ts("2026-10-01T00:00:01+02:00"),
                Source::Pacman,
                Kind::Install,
                "b",
            )
            .meta(Meta {
                command: Some("pacman -S b --password hunter2".into()),
                ..Meta::default()
            }),
        ];
        let written = ledger.append(&lock, events).unwrap();
        assert!(written[0].id < written[1].id, "monotonic ids");
        assert_eq!(ledger.months().unwrap(), ["2026-09", "2026-10"]);
        let oct = ledger.read_month("2026-10").unwrap();
        assert_eq!(oct.events, written[1..]);
        assert_eq!(
            oct.events[0].meta.command.as_deref(),
            Some("pacman -S b --password ‹redacted›")
        );
        // a torn last line stays a bad line; the next append starts a new line
        let path = ledger.month_file("2026-10");
        let mut f = OpenOptions::new().append(true).open(&path).unwrap();
        f.write_all(b"{\"id\":\"tor").unwrap();
        let again = Event::new(
            ts("2026-10-02T10:00:00+02:00"),
            Source::Snapper,
            Kind::Snapshot,
            "9",
        );
        ledger.append(&lock, vec![again]).unwrap();
        let oct = ledger.read_month("2026-10").unwrap();
        assert_eq!(oct.events.len(), 2);
        assert_eq!(oct.bad_lines, [2]);
        let range = ledger
            .read_range(
                ts("2026-10-01T00:00:00+02:00"),
                ts("2026-10-01T23:00:00+02:00"),
            )
            .unwrap();
        assert_eq!(range.len(), 1);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// F-132: a line torn inside a multi-byte character is one bad line,
    /// not a failed month; WP-059 review: a line whose actor or case
    /// `append` would refuse is a bad line too.
    #[test]
    fn bad_lines_are_skipped_one_by_one() {
        let dir = tmp("bad");
        let lock = lock::acquire(&dir.join("lock")).unwrap();
        let ledger = Ledger::at(dir.join("ledger"), Redactor::builtin());
        let note = |s: &str| {
            Event::new(
                ts("2026-10-03T09:00:00+02:00"),
                Source::Manual,
                Kind::Note,
                s,
            )
        };
        let good = ledger
            .append(&lock, vec![note("Lüfter getauscht")])
            .unwrap()
            .remove(0);
        let line = good.to_line();
        let path = ledger.month_file("2026-10");
        let mut f = OpenOptions::new().append(true).open(&path).unwrap();
        // 2: torn after the first byte of `ü`
        f.write_all(b"{\"id\":\"01M4TORN\",\"subject\":\"L\xc3")
            .unwrap();
        ledger.append(&lock, vec![note("zwei")]).unwrap();
        let mut f = OpenOptions::new().append(true).open(&path).unwrap();
        // 4: a CRLF line is fine; 5, 6: actor and case `append` refuses
        let crlf = line.replace("Lüfter getauscht", "crlf");
        let actor = line.replace("\"actor\":\"system\"", "\"actor\":\"Robot ]] x\"");
        let case = line.replace(
            "\"actor\":\"system\"",
            "\"actor\":\"system\",\"case\":\"../x\"",
        );
        assert_ne!(actor, line);
        assert_ne!(case, line);
        f.write_all(format!("{crlf}\r\n{actor}\n{case}\n").as_bytes())
            .unwrap();

        let oct = ledger.read_month("2026-10").unwrap();
        let subjects: Vec<_> = oct.events.iter().map(|e| e.subject.as_str()).collect();
        assert_eq!(subjects, ["Lüfter getauscht", "zwei", "crlf"]);
        assert_eq!(oct.bad_lines, [2, 5, 6]);
        assert_eq!(ledger.read_all().unwrap().len(), 3);
        assert_eq!(
            ledger.bad_lines().unwrap(),
            [("2026-10".to_string(), vec![2, 5, 6])]
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn invalid_event_writes_nothing() {
        let dir = tmp("invalid");
        let lock = lock::acquire(&dir.join("lock")).unwrap();
        let ledger = Ledger::at(dir.join("ledger"), Redactor::builtin());
        let good = Event::new(
            ts("2026-10-01T10:00:00+02:00"),
            Source::Pacman,
            Kind::Install,
            "a",
        );
        let bad = Event::new(
            ts("2026-10-01T10:00:00+02:00"),
            Source::Pacman,
            Kind::Install,
            "b",
        )
        .actor("robot");
        assert!(ledger.append(&lock, vec![good, bad]).is_err());
        assert!(ledger.months().unwrap().is_empty());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn truncates_by_characters() {
        assert_eq!(truncate("äöü", 5), "äöü");
        assert_eq!(truncate("äöüß", 3), "äö…");
    }
}
