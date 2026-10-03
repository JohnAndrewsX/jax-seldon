//! `snapper` collector (SPEC-ENGINE §4, ADR-0011).
//!
//! Runs `snapper --jsonout list` as the user, never with sudo. New snapshot
//! numbers become `snapshot` events (`ts` = the snapshot's date in the local
//! zone, `detail` = description, `meta.type`/`cleanup`, and `meta.pairOf` on
//! a `post`). Numbers that disappear become `snapshot-delete` events at
//! capture time. Snapshot 0 (`current`) is not a snapshot.
//!
//! The cursor is the set of known snapshots (number, type, description,
//! date), so a deletion can name what was deleted. snapper gives a new
//! snapshot the number after the highest one, so a number comes back when
//! the newest snapshot is deleted before the next one is made: a known
//! number with another date becomes a `snapshot-delete` of the old snapshot
//! and a `snapshot` of the new one, both at the new snapshot's date. A
//! cursor entry without a date (written before WP-073) gets one without an
//! event. Without a cursor, snapshots older than the baseline are recorded
//! as known without an event.
//!
//! snapper translates its messages (`Keine Berechtigungen.` under
//! `LANG=de_DE.UTF-8`), so every invocation is built by [`list_command`]
//! with `LC_ALL=C` and without `LANGUAGE`, and its stderr is matched in
//! English (issue #1).
//!
//! Without `ALLOW_USERS`, snapper exits 1 with `No permissions.` on stderr.
//! The collector then reads the snapshots from the info files instead
//! (`<snapshots>/<number>/info.xml`, [`read_info_files`]), which needs only
//! read access to the snapshot directory. The events and the cursor are the
//! same as from the list, so switching between the two ways adds no events.
//! When the info files cannot be read either, or none is found, the
//! collector degrades:
//! `ok: false`, a message, and the one-line fix, which is printed and never
//! run. Nothing is invented and the cursor stays. The same goes for a
//! missing snapper or unreadable output.
//!
//! Only the `root` config is read (it is the only one on Omarchy; with one
//! config of another name, that one), because the event subject is the bare
//! snapshot number.

use std::collections::{BTreeMap, HashSet};
use std::path::Path;
use std::process::Command;
use std::time::Duration;

use chrono::{DateTime, FixedOffset, Local, NaiveDateTime};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{Collector, Ctx, Outcome, RUN_TIMEOUT, Tz, to_cursor, typed_cursor};
use crate::commands::doctor::SNAPPER_FIX;
use crate::model::event::{Event, Kind, Meta, Source};
use crate::sys::{self, Run};

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

/// Where a list of [`Snapshot`]s came from; it decides the zone of `date`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    /// `snapper --jsonout list`: local time.
    List,
    /// The info files: UTC, as snapper stores it.
    InfoFiles,
}

/// The snapshots read from the info files of a snapshot directory.
#[derive(Debug, Clone, Default)]
pub struct InfoFiles {
    pub snapshots: Vec<Snapshot>,
    /// Numbered directories whose `info.xml` could not be read or parsed:
    /// still present (never reported as deleted), and why.
    pub skipped: Vec<(u64, String)>,
}

/// What the cursor remembers of a snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Known {
    #[serde(rename = "type")]
    pub snapshot_type: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    /// The snapshot's date as an instant (missing in cursors before WP-073,
    /// and for a snapshot without a usable date): the same number with
    /// another date is another snapshot.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date: Option<DateTime<FixedOffset>>,
}

/// `cursors.json` → `snapper.cursor`: snapshot number → what it was.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapperCursor {
    pub known: BTreeMap<u64, Known>,
}

/// The degraded message for a permission error (index and doctor).
pub const NO_PERMISSIONS: &str = "snapper: No permissions. The snapper config does not list this user in ALLOW_USERS; see `seldon doctor`.";

/// `program --jsonout list` (`program` is `snapper`, or `SELDON_SNAPPER`),
/// with `LC_ALL=C` and `LANGUAGE` removed so snapper's messages stay
/// English whatever the user's locale (issue #1). The one place a snapper
/// command is built: the collector and `doctor` (and through it `init`)
/// run it with [`run_list`].
pub fn list_command(program: &str) -> Command {
    let mut cmd = Command::new(program);
    cmd.args(["--jsonout", "list"])
        .env("LC_ALL", "C")
        .env_remove("LANGUAGE");
    cmd
}

/// Runs [`list_command`] with `timeout`.
pub fn run_list(program: &str, timeout: Duration) -> Run {
    sys::run_command(list_command(program), timeout)
}

/// Whether snapper's stderr is its permission error (English, see
/// [`list_command`]).
pub fn is_no_permissions(stderr: &str) -> bool {
    stderr.contains("No permissions")
}

impl Collector for Snapper {
    fn name(&self) -> &'static str {
        "snapper"
    }

    fn collect(&self, ctx: &Ctx, cursor: Option<&Value>) -> Outcome {
        let stdout = match run_list(&ctx.sources.snapper, RUN_TIMEOUT) {
            Run::Exited {
                code: Some(0),
                stdout,
                ..
            } => stdout,
            Run::Exited { stderr, .. } if is_no_permissions(&stderr) => {
                return match readable_info_files(&ctx.sources.snapshots) {
                    Some(info) => from_info_files(ctx, cursor, &ctx.sources.snapshots, &info),
                    None => Outcome::degraded(NO_PERMISSIONS, Some(SNAPPER_FIX.to_string())),
                };
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
        match diff(ctx, typed_cursor(cursor), &list, Origin::List, &[]) {
            Ok((events, next)) => Outcome::ok(events, to_cursor(&next)),
            Err(e) => Outcome::degraded(format!("{e:#}"), None),
        }
    }
}

/// The collector's result from the info files: `ok`, with a message that
/// says so and names every skipped info file.
fn from_info_files(ctx: &Ctx, cursor: Option<&Value>, dir: &Path, info: &InfoFiles) -> Outcome {
    let kept: Vec<u64> = info.skipped.iter().map(|(n, _)| *n).collect();
    match diff(
        ctx,
        typed_cursor(cursor),
        &info.snapshots,
        Origin::InfoFiles,
        &kept,
    ) {
        Ok((events, next)) => {
            let mut out = Outcome::ok(events, to_cursor(&next));
            out.message = Some(info_files_message(dir, info));
            out
        }
        Err(e) => Outcome::degraded(format!("{e:#}"), None),
    }
}

/// "snapper list is not permitted; N snapshots read from the info files in
/// DIR", plus one clause per skipped info file.
pub fn info_files_message(dir: &Path, info: &InfoFiles) -> String {
    let mut m = format!(
        "snapper list is not permitted; {} snapshot{} read from the info files in {}",
        info.snapshots.len(),
        if info.snapshots.len() == 1 { "" } else { "s" },
        dir.display()
    );
    for (n, why) in &info.skipped {
        m.push_str(&format!("; skipped {n}/info.xml: {why}"));
    }
    m
}

/// [`read_info_files`] when it gives a usable answer: the directory is
/// readable and at least one snapshot was read from it. `None` keeps the
/// collector degraded with the cursor unchanged. An empty directory is not
/// an answer: right after booting into a snapshot `/.snapshots` is an empty
/// nested subvolume, and reading it as "no snapshots" would record every
/// known snapshot as deleted.
pub fn readable_info_files(dir: &Path) -> Option<InfoFiles> {
    let info = read_info_files(dir).ok()?;
    if info.snapshots.is_empty() {
        None
    } else {
        Some(info)
    }
}

/// Reads `dir/<number>/info.xml` for every numbered directory in `dir` (the
/// snapshot directory of the `root` config, `/.snapshots`). Other entries
/// are ignored. An info file that cannot be read or parsed is skipped and
/// named in [`InfoFiles::skipped`]. Fails only when `dir` itself cannot be
/// read.
pub fn read_info_files(dir: &Path) -> std::io::Result<InfoFiles> {
    let mut numbers: Vec<u64> = std::fs::read_dir(dir)?
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            let name = entry.file_name();
            let name = name.to_str()?;
            // only plain decimal numbers, as snapper names them
            if name.is_empty() || !name.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            name.parse::<u64>().ok()
        })
        .filter(|n| *n != 0)
        .collect();
    numbers.sort_unstable();
    let mut info = InfoFiles::default();
    for number in numbers {
        let file = dir.join(number.to_string()).join("info.xml");
        let parsed = std::fs::read_to_string(&file)
            .map_err(|e| match e.kind() {
                std::io::ErrorKind::NotFound => "missing".to_string(),
                kind => kind.to_string(),
            })
            .and_then(|text| parse_info(&text, number));
        match parsed {
            Ok(s) => info.snapshots.push(s),
            Err(why) => info.skipped.push((number, why)),
        }
    }
    Ok(info)
}

/// One `info.xml` (snapper's flat format: `<snapshot>` with `<type>`,
/// `<num>`, `<date>` in UTC, and optionally `<pre_num>`, `<description>`,
/// `<cleanup>`, `<uid>`, `<userdata>`). `<num>` must equal the directory's
/// `number`, as snapper requires. `<userdata>` is not part of the events and
/// is not read.
pub fn parse_info(text: &str, number: u64) -> Result<Snapshot, String> {
    if !text.contains("<snapshot>") {
        return Err("not a snapshot info file".into());
    }
    let num = element(text, "num").ok_or("no <num>")?;
    if num.trim().parse::<u64>().ok() != Some(number) {
        return Err(format!("<num> {} does not match the directory", num.trim()));
    }
    let snapshot_type = element(text, "type").ok_or("no <type>")?.trim().to_string();
    if !matches!(snapshot_type.as_str(), "single" | "pre" | "post") {
        return Err(format!("unknown <type> {snapshot_type}"));
    }
    let pre_number = match element(text, "pre_num") {
        Some(p) => Some(
            p.trim()
                .parse::<u64>()
                .map_err(|_| format!("<pre_num> {} is not a number", p.trim()))?,
        ),
        None => None,
    };
    Ok(Snapshot {
        number,
        snapshot_type,
        pre_number,
        date: element(text, "date").unwrap_or_default().trim().to_string(),
        cleanup: element(text, "cleanup").unwrap_or_default(),
        description: element(text, "description").unwrap_or_default(),
    })
}

/// The unescaped text of the first `<name>…</name>` (or `<name/>`, empty)
/// in `text`. The info format has no attributes and no nesting under these
/// names.
fn element(text: &str, name: &str) -> Option<String> {
    if text.contains(&format!("<{name}/>")) {
        return Some(String::new());
    }
    let open = format!("<{name}>");
    let start = text.find(&open)? + open.len();
    let end = start + text[start..].find(&format!("</{name}>"))?;
    Some(unescape(&text[start..end]))
}

/// XML's five named entities and numeric character references; anything
/// else is kept as written.
fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(amp) = rest.find('&') {
        out.push_str(&rest[..amp]);
        rest = &rest[amp..];
        let decoded = rest.find(';').and_then(|semi| {
            let entity = &rest[1..semi];
            let c = match entity {
                "amp" => Some('&'),
                "lt" => Some('<'),
                "gt" => Some('>'),
                "quot" => Some('"'),
                "apos" => Some('\''),
                _ => entity
                    .strip_prefix("#x")
                    .and_then(|h| u32::from_str_radix(h, 16).ok())
                    .or_else(|| entity.strip_prefix('#').and_then(|d| d.parse().ok()))
                    .and_then(char::from_u32),
            }?;
            Some((c, semi + 1))
        });
        match decoded {
            Some((c, len)) => {
                out.push(c);
                rest = &rest[len..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// A snapshot's `date` as an instant: local time from the list, UTC from
/// the info files. `None` for an empty or unparsable date, or a local time
/// in a DST gap.
fn snapshot_ts(tz: Tz, date: &str, origin: Origin) -> Option<DateTime<FixedOffset>> {
    let naive = NaiveDateTime::parse_from_str(date, "%Y-%m-%d %H:%M:%S").ok()?;
    match origin {
        Origin::List => tz.localize(naive),
        Origin::InfoFiles => {
            let utc = naive.and_utc();
            Some(match tz {
                Tz::Local => utc.with_timezone(&Local).fixed_offset(),
                Tz::Fixed(off) => utc.with_timezone(&off),
            })
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
/// cursor. `kept` are numbers that exist but could not be read: they are
/// neither new nor deleted, and a known one stays in the cursor.
fn diff(
    ctx: &Ctx,
    cursor: Option<SnapperCursor>,
    list: &[Snapshot],
    origin: Origin,
    kept: &[u64],
) -> anyhow::Result<(Vec<Event>, SnapperCursor)> {
    let first_run = cursor.is_none();
    let known = cursor.unwrap_or_default().known;
    let mut events = Vec::new();
    for s in list {
        let Some(ts) = snapshot_ts(ctx.tz, &s.date, origin) else {
            continue; // no usable date: remembered, not reported
        };
        if let Some(k) = known.get(&s.number) {
            // a known number with another date: deleted and created again
            // (snapper reuses the number after the highest one). An entry
            // without a date (an older cursor) only gets one.
            match k.date {
                Some(date) if date != ts => {
                    // the deletion goes before the creation it made room for
                    events.push(deleted(ts, s.number, k));
                }
                _ => continue,
            }
        } else if first_run && ts < ctx.baseline {
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
    let present: HashSet<u64> = list
        .iter()
        .map(|s| s.number)
        .chain(kept.iter().copied())
        .collect();
    for (number, k) in known.iter().filter(|(n, _)| !present.contains(n)) {
        events.push(deleted(ctx.now, *number, k));
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
                        date: snapshot_ts(ctx.tz, &s.date, origin),
                    },
                )
            })
            .chain(
                kept.iter()
                    .filter_map(|n| known.get(n).map(|k| (*n, k.clone()))),
            )
            .collect(),
    };
    Ok((events, next))
}

/// A `snapshot-delete` of the snapshot `k` the cursor knew as `number`, at
/// `ts`.
fn deleted(ts: DateTime<FixedOffset>, number: u64, k: &Known) -> Event {
    let mut e = Event::new(
        ts,
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
    e
}

/// Drops `snapshot` events already in the ledger (same number and time),
/// e.g. after a crash between the ledger write and the cursor save.
///
/// Likewise a `snapshot-delete` whose number the ledger already records as
/// deleted after its last creation: deletions carry capture time, so only
/// the subject can tell them apart. The deletion of a reused number
/// carries the new snapshot's time instead and goes when that snapshot is
/// in the ledger already.
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
    // deletions of a reused number: at the time of the snapshot replacing it
    let replaced: HashSet<(String, i64)> = events
        .iter()
        .filter(|e| e.kind == Kind::Snapshot)
        .map(|e| (e.subject.clone(), e.ts.timestamp()))
        .collect();
    Ok(events
        .into_iter()
        .filter(|e| {
            let key = (e.subject.clone(), e.ts.timestamp());
            match e.kind {
                Kind::Snapshot => !seen.contains(&key),
                Kind::SnapshotDelete if replaced.contains(&key) => {
                    !seen.contains(&key) && !deleted.contains(&e.subject)
                }
                Kind::SnapshotDelete => !deleted.contains(&e.subject),
                _ => true,
            }
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

    fn info(body: &str) -> String {
        format!("<?xml version=\"1.0\"?>\n<snapshot>\n{body}</snapshot>\n")
    }

    #[test]
    fn parses_info_files() {
        // the fixture tree mirrors snapper.json; dates are UTC there
        let text = include_str!("../../../fixtures/logs/snapshots/115/info.xml");
        let post = parse_info(text, 115).unwrap();
        assert_eq!(
            (
                post.snapshot_type.as_str(),
                post.pre_number,
                post.date.as_str()
            ),
            ("post", Some(114), "2026-10-01 14:30:04")
        );
        assert_eq!(post.description, "tailscale: MagicDNS");
        assert_eq!(post.cleanup, "");
        let pre = parse_info(
            include_str!("../../../fixtures/logs/snapshots/114/info.xml"),
            114,
        )
        .unwrap();
        assert_eq!((pre.snapshot_type.as_str(), pre.pre_number), ("pre", None));

        // optional fields missing: empty; entities decoded
        let s = parse_info(
            &info("  <type>single</type>\n  <num>7</num>\n  <uid>1000</uid>\n"),
            7,
        )
        .unwrap();
        assert_eq!(
            (s.date.as_str(), s.description.as_str(), s.cleanup.as_str()),
            ("", "", "")
        );
        let s = parse_info(
            &info("<type>single</type><num>8</num><date>2026-10-01 07:00:00</date><description>a &lt;b&gt; &amp; &quot;c&quot; &apos;d&apos; &#228;&#xFC; &bogus; &amp</description><cleanup>number</cleanup><userdata><key>description</key><value>x</value></userdata>"),
            8,
        )
        .unwrap();
        assert_eq!(s.description, "a <b> & \"c\" 'd' äü &bogus; &amp");
        assert_eq!(s.cleanup, "number");
        let s = parse_info(&info("<type>single</type><num>9</num><description/>"), 9).unwrap();
        assert_eq!(s.description, "");

        // required fields
        let err = |body: &str, n: u64| parse_info(&info(body), n).unwrap_err();
        assert_eq!(err("<type>single</type>", 1), "no <num>");
        assert_eq!(err("<num>1</num>", 1), "no <type>");
        assert_eq!(
            err("<type>single</type><num>2</num>", 1),
            "<num> 2 does not match the directory"
        );
        assert_eq!(
            err("<type>other</type><num>1</num>", 1),
            "unknown <type> other"
        );
        assert_eq!(
            err("<type>post</type><num>1</num><pre_num>x</pre_num>", 1),
            "<pre_num> x is not a number"
        );
        assert_eq!(parse_info("", 1).unwrap_err(), "not a snapshot info file");
    }

    #[test]
    fn reads_only_numbered_directories() {
        let dir = std::env::temp_dir().join(format!("seldon-snapshots-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        for (name, body) in [
            ("3", info("<type>single</type><num>3</num>")),
            ("12", info("<type>pre</type><num>12</num>")),
            ("0", info("<type>single</type><num>0</num>")),
            ("+4", info("<type>single</type><num>4</num>")),
            ("5a", info("<type>single</type><num>5</num>")),
            ("lost+found", String::new()),
        ] {
            std::fs::create_dir_all(dir.join(name)).unwrap();
            std::fs::write(dir.join(name).join("info.xml"), body).unwrap();
        }
        std::fs::create_dir_all(dir.join("13")).unwrap(); // no info.xml
        let got = read_info_files(&dir).unwrap();
        assert_eq!(
            got.snapshots.iter().map(|s| s.number).collect::<Vec<_>>(),
            [3, 12]
        );
        assert_eq!(got.skipped, [(13, "missing".to_string())]);
        assert!(readable_info_files(&dir).is_some());
        assert!(read_info_files(&dir.join("missing")).is_err());
        assert!(readable_info_files(&dir.join("missing")).is_none());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// Issue #1: the one snapper command is `--jsonout list` in the C
    /// locale, with `LANGUAGE` removed.
    #[test]
    fn list_command_runs_in_the_c_locale() {
        use std::ffi::OsStr;
        let cmd = list_command("snapper");
        assert_eq!(cmd.get_program(), "snapper");
        let args: Vec<&OsStr> = cmd.get_args().collect();
        assert_eq!(args, ["--jsonout", "list"]);
        let envs: Vec<(&OsStr, Option<&OsStr>)> = cmd.get_envs().collect();
        assert!(envs.contains(&(OsStr::new("LC_ALL"), Some(OsStr::new("C")))));
        assert!(envs.contains(&(OsStr::new("LANGUAGE"), None)));
        assert!(is_no_permissions("No permissions.\n"));
        assert!(!is_no_permissions("Keine Berechtigungen.\n"));
    }
}
