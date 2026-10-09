//! `seldon preview` (WP-138, ADR-0047): before the logbook exists, what
//! the machine already remembers on its own — the last days' pacman
//! transactions and the files recently edited under `~/.config`.
//!
//! Read-only: no logbook is needed (one that exists is not read), no lock
//! is taken and nothing is written, not even the state directory. Its only
//! input besides the two sources is `config.toml` when there is one: its
//! `[redaction] skipPaths` and `patterns`. A config that cannot be read, or
//! whose patterns do not compile, withholds what they would filter (the
//! file list and the pacman command lines).
//!
//! Bounded (WP-138): at most 7 days; at most [`MAX_ROWS`] rows, one per
//! transaction and one per file, the files at most [`MAX_FILES`] of them;
//! at most [`MAX_PACKAGES`] packages listed per transaction (`count` holds
//! all). Time (WP-138 round 2): the `~/.config` walk runs first and stops
//! [`SCAN_BUDGET`] after it started; then the pacman log is read from its
//! end, at most [`TAIL_MAX`] bytes, which the dev host's release build
//! parses in about 0.17 s when every line is inside the window. Together
//! under 0.5 s. Every name and version is one line ([`shown`]) and clipped
//! to the schema's bounds. The JSON is `schema/preview.schema.json`.

use std::fmt::Write as _;
use std::fs::File;
use std::io::{Read as _, Seek as _, SeekFrom};
use std::path::Path;
use std::time::{Duration, Instant, SystemTime};

use chrono::{DateTime, FixedOffset, Timelike as _, Utc};
use clap::Args;
use serde_json::{Value, json};

use super::{Context, Output};
use crate::CONTRACT_VERSION;
use crate::collectors::config::SkipPaths;
use crate::collectors::pacman::{self, Tx};
use crate::collectors::{Sources, Tz};
use crate::config::Config;
use crate::config_scan::{self, Limits};
use crate::error::Result;
use crate::redact::Redactor;

/// The window's longest length, and its default.
pub const MAX_DAYS: u32 = 7;
/// Rows at most: transactions plus files.
pub const MAX_ROWS: usize = 200;
/// Files at most (WP-139's bound, the same scan).
pub const MAX_FILES: usize = 80;
/// Packages listed per transaction at most.
pub const MAX_PACKAGES: usize = 10;
/// The `~/.config` walk stops this long after it started; with the pacman
/// read bounded by [`TAIL_MAX`] the whole preview stays under half a
/// second (WP-138).
pub const SCAN_BUDGET: Duration = Duration::from_millis(250);
/// Directory entries the walk reads at most.
pub const SCAN_ENTRIES: usize = 200_000;
/// The most of `pacman.log` parsed, from its end: about 0.17 s on the dev
/// host's release build when every line is inside the window (21 ms per
/// MiB measured, WP-138 round 2). Seven days of a real log take a small
/// part of it.
pub const TAIL_MAX: u64 = 8 << 20;
/// The first slice of `pacman.log` read from its end; each next one is
/// four times larger, up to [`TAIL_MAX`].
const TAIL_FIRST: u64 = 256 << 10;
/// Longest command line shown (characters).
const COMMAND_MAX: usize = 256;
/// Longest package name shown (characters; the schema's bound).
const NAME_MAX: usize = 512;
/// Longest version shown (characters; the schema's bound).
const VERSION_MAX: usize = 256;

#[derive(Debug, Clone, Args)]
#[command(after_help = "Examples:
  seldon preview
  seldon preview --days 2 --json")]
pub struct PreviewArgs {
    /// Days to look back, 1 to 7
    #[arg(long, value_name = "N", default_value_t = MAX_DAYS,
          value_parser = clap::value_parser!(u32).range(1..=MAX_DAYS as i64))]
    pub days: u32,
}

pub fn run(ctx: &Context, args: PreviewArgs) -> Result<Output> {
    let started = Instant::now();
    let since = ctx.now - chrono::Duration::days(i64::from(args.days));
    // `None`: config.toml is there but cannot be used; what it would
    // filter is withheld
    let config = match ctx.load_config() {
        Ok(c) => Some(c.unwrap_or_default()),
        Err(_) => None,
    };
    let redactor = config.as_ref().and_then(|c| Redactor::for_config(c).ok());

    // the walk first, with its own budget: a slow pacman read cannot use
    // up its time (WP-138 round 2)
    let files = match (&config, &redactor) {
        (Some(config), Some(redactor)) => scan_files(ctx, config, redactor, since),
        _ => Section::failed(WITHHELD),
    };
    let sources = Sources::from_env();
    let pacman = read_pacman(&sources, since, redactor.as_ref());

    // files first (at most MAX_FILES), transactions fill the rest
    let (mut txs, mut truncated) = (pacman.items, pacman.cut || files.cut);
    let room = MAX_ROWS - files.items.len();
    if txs.len() > room {
        txs.truncate(room);
        truncated = true;
    }
    let elapsed = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    let pacman = pacman_json(&pacman.state, &txs);
    let human = human(ctx, args.days, &pacman, &files);
    let out = json!({
        "contractVersion": CONTRACT_VERSION,
        "generatedAt": ctx.now.to_rfc3339(),
        "since": since.to_rfc3339(),
        "days": args.days,
        "pacman": pacman,
        "files": section_json(&files.state, json!(ctx.dirs.display(&ctx.dirs.xdg_config_home)), &files.items),
        "truncated": truncated,
        "elapsedMs": elapsed,
    });
    Ok(Output::ok(human, out))
}

const WITHHELD: &str = "withheld: config.toml or one of its [redaction] patterns cannot be used";

/// How a source went.
#[derive(Debug, Clone, Default)]
struct State {
    ok: bool,
    message: Option<String>,
    /// Not all of the window was read.
    partial: bool,
}

/// One source's rows.
#[derive(Debug, Clone, Default)]
struct Section {
    state: State,
    items: Vec<Value>,
    /// Rows were cut by a bound.
    cut: bool,
}

impl Section {
    fn failed(message: &str) -> Self {
        Section {
            state: State {
                ok: false,
                message: Some(message.to_string()),
                partial: false,
            },
            ..Section::default()
        }
    }
}

fn state_json(s: &State) -> serde_json::Map<String, Value> {
    let mut m = serde_json::Map::new();
    m.insert("ok".into(), json!(s.ok));
    if let Some(msg) = &s.message {
        m.insert("message".into(), json!(msg));
    }
    m.insert("partial".into(), json!(s.partial));
    m
}

fn pacman_json(s: &State, txs: &[Value]) -> Value {
    let mut m = state_json(s);
    m.insert("transactions".into(), json!(txs));
    Value::Object(m)
}

fn section_json(s: &State, root: Value, items: &[Value]) -> Value {
    let mut m = state_json(s);
    m.insert("root".into(), root);
    m.insert("items".into(), json!(items));
    Value::Object(m)
}

// ---------------------------------------------------------------------------
// pacman
// ---------------------------------------------------------------------------

fn read_pacman(
    sources: &Sources,
    since: DateTime<FixedOffset>,
    redactor: Option<&Redactor>,
) -> Section {
    let path = &sources.pacman_log;
    let (bytes, capped) = match tail(path, since, Tz::Local) {
        Ok(t) => t,
        Err(e) => return Section::failed(&format!("cannot read {}: {e}", path.display())),
    };
    // the collector's rule (WP-160): a db.lck older than the boot holds
    // nothing back
    let lock = pacman::lock_state(&sources.pacman_db_lock, &sources.proc_stat);
    let parsed = pacman::parse(&bytes, 0, lock, Tz::Local);
    let mut txs: Vec<Value> = parsed
        .txs
        .iter()
        .filter_map(|tx| transaction(tx, since, redactor))
        .collect();
    txs.reverse();
    let cut = txs
        .iter()
        .any(|t| t["count"].as_u64() > Some(MAX_PACKAGES as u64));
    Section {
        state: State {
            ok: true,
            message: None,
            partial: capped,
        },
        items: txs,
        cut,
    }
}

/// The end of the log from a line that starts before `since` (or from
/// the file's start), and whether [`TAIL_MAX`] cut it short of that.
/// Reads slices from the end, each [`TAIL_FIRST`] times four larger,
/// until one starts before the window.
pub fn tail(path: &Path, since: DateTime<FixedOffset>, tz: Tz) -> std::io::Result<(Vec<u8>, bool)> {
    tail_within(path, since, tz, TAIL_FIRST, TAIL_MAX)
}

/// [`tail`] with its first slice and its cap.
fn tail_within(
    path: &Path,
    since: DateTime<FixedOffset>,
    tz: Tz,
    first: u64,
    max: u64,
) -> std::io::Result<(Vec<u8>, bool)> {
    let mut f = File::open(path)?;
    let len = f.metadata()?.len();
    let mut size = first;
    loop {
        let start = len.saturating_sub(size.min(max));
        f.seek(SeekFrom::Start(start))?;
        let mut bytes = Vec::with_capacity(usize::try_from(len - start).unwrap_or(0));
        f.by_ref().take(len - start).read_to_end(&mut bytes)?;
        if start == 0 {
            return Ok((bytes, false));
        }
        // the slice's first line is cut: it starts after the next newline
        let first = bytes
            .iter()
            .position(|&b| b == b'\n')
            .map_or(bytes.len(), |n| n + 1);
        bytes.drain(..first);
        if first_time(&bytes, tz).is_some_and(|t| t < since) {
            return Ok((bytes, false));
        }
        if size >= max {
            return Ok((bytes, true));
        }
        size *= 4;
    }
}

/// The time of the first line the pacman grammar reads.
fn first_time(bytes: &[u8], tz: Tz) -> Option<DateTime<FixedOffset>> {
    bytes
        .split(|&b| b == b'\n')
        .find_map(|l| pacman::parse_line(&String::from_utf8_lossy(l), tz))
        .map(|(ts, _)| ts)
}

/// A transaction's package lines in the window, as the preview lists
/// them; `None` when it has none there.
fn transaction(
    tx: &Tx,
    since: DateTime<FixedOffset>,
    redactor: Option<&Redactor>,
) -> Option<Value> {
    let lines: Vec<_> = tx.lines.iter().filter(|l| l.ts >= since).collect();
    let first = lines.first()?;
    let at = if tx.began >= since {
        tx.began
    } else {
        first.ts
    };
    let packages: Vec<Value> = lines
        .iter()
        .take(MAX_PACKAGES)
        .map(|l| {
            // pacman's grammar takes any `\S+`: one line, within the
            // schema's bounds (WP-138 round 2)
            let version = |v: &str| super::event::clip(&shown(v), VERSION_MAX);
            let mut p = json!({
                "kind": l.kind.as_str(),
                "name": super::event::clip(&shown(&l.name), NAME_MAX),
            });
            match &l.from {
                Some(from) => {
                    p["from"] = json!(version(from));
                    p["to"] = json!(version(&l.to));
                }
                None => p["version"] = json!(version(&l.to)),
            }
            p
        })
        .collect();
    let mut kinds: Vec<&str> = Vec::new();
    for l in &lines {
        if !kinds.contains(&l.kind.as_str()) {
            kinds.push(l.kind.as_str());
        }
    }
    let mut t = json!({
        "at": at.to_rfc3339(),
        "count": lines.len(),
        "kinds": kinds,
        "packages": packages,
    });
    // the command line only through the redaction
    if let (Some(command), Some(r)) = (&tx.command, redactor) {
        t["command"] = json!(super::event::clip(&shown(&r.redact(command)), COMMAND_MAX));
    }
    if let Some(status) = tx.status {
        t["status"] = json!(status.as_str());
    }
    Some(t)
}

/// `text` with control and line-breaking characters and the direction and
/// format set the index drops ([`crate::redact::is_invisible`])
/// as U+FFFD: a name shown on one line that cannot reorder, break or hide
/// part of it (WP-138 stage 2, F2).
fn shown(text: &str) -> String {
    text.chars()
        .map(|c| {
            if c.is_control()
                || super::is_line_breaking(c)
                || crate::redact::is_invisible(c)
            {
                '\u{FFFD}'
            } else {
                c
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// ~/.config
// ---------------------------------------------------------------------------

fn scan_files(
    ctx: &Context,
    config: &Config,
    redactor: &Redactor,
    since: DateTime<FixedOffset>,
) -> Section {
    let skip = SkipPaths::new(&ctx.dirs.home, &config.redaction.skip_paths);
    let limits = Limits {
        since: SystemTime::from(since),
        max_files: MAX_FILES,
        deadline: Some(Instant::now() + SCAN_BUDGET),
        max_entries: SCAN_ENTRIES,
        // the shell's own: plugins and its settings file
        exclude: vec![
            ctx.dirs.xdg_config_home.join("omarchy").join("plugins"),
            ctx.dirs.xdg_config_home.join("omarchy").join("shell.json"),
        ],
    };
    let scan = config_scan::scan(&ctx.dirs.xdg_config_home, &skip, &limits);
    let offset = *ctx.now.offset();
    let items = scan
        .files
        .iter()
        .map(|f| {
            // whole seconds, like every time the engine writes
            let modified = DateTime::<Utc>::from(f.modified).with_timezone(&offset);
            let modified = modified.with_nanosecond(0).unwrap_or(modified);
            json!({
                "path": shown(&redactor.redact(&ctx.dirs.display(&f.path))),
                "modified": modified.to_rfc3339(),
            })
        })
        .collect();
    Section {
        state: State {
            ok: true,
            message: None,
            partial: scan.partial,
        },
        items,
        cut: scan.matched > scan.files.len(),
    }
}

// ---------------------------------------------------------------------------
// Human output
// ---------------------------------------------------------------------------

fn human(ctx: &Context, days: u32, pacman: &Value, files: &Section) -> String {
    let mut h = format!(
        "What this machine remembers of the last {days} days on its own: no who, no why,\n\
         gone when the logs rotate. Set up Seldon to keep it: seldon init\n"
    );
    let time = |v: &Value| {
        v.as_str()
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|t| t.format("%Y-%m-%d %H:%M").to_string())
            .unwrap_or_default()
    };
    h.push_str("\nPackages (pacman)\n");
    match pacman["transactions"].as_array() {
        _ if pacman["ok"] == json!(false) => {
            let _ = writeln!(h, "  {}", pacman["message"].as_str().unwrap_or_default());
        }
        Some(txs) if !txs.is_empty() => {
            for t in txs {
                let _ = write!(h, "  {}  {}", time(&t["at"]), tx_summary(t));
                if let Some(c) = t["command"].as_str() {
                    let _ = write!(h, "  ({c})");
                }
                h.push('\n');
            }
        }
        _ => h.push_str("  no transaction\n"),
    }
    let _ = writeln!(
        h,
        "\nEdited under {}",
        ctx.dirs.display(&ctx.dirs.xdg_config_home)
    );
    if let Some(m) = files.state.message.as_deref() {
        let _ = writeln!(h, "  {m}");
    } else if files.items.is_empty() {
        h.push_str("  no file\n");
    }
    for f in &files.items {
        let _ = writeln!(
            h,
            "  {}  {}",
            time(&f["modified"]),
            f["path"].as_str().unwrap_or_default()
        );
    }
    h.truncate(h.trim_end().len());
    h
}

/// `upgraded 143 packages`, `installed btop`; `changed 3 packages` for
/// a transaction of several kinds.
fn tx_summary(t: &Value) -> String {
    let count = t["count"].as_u64().unwrap_or(0);
    let verb = match t["kinds"].as_array().map(Vec::as_slice) {
        Some([kind]) => match kind.as_str() {
            Some("install") => "installed",
            Some("remove") => "removed",
            Some("upgrade") => "upgraded",
            Some("downgrade") => "downgraded",
            _ => "reinstalled",
        },
        _ => "changed",
    };
    match (count, t["packages"].get(0)) {
        (1, Some(p)) => format!("{verb} {}", p["name"].as_str().unwrap_or_default()),
        _ => format!("{verb} {count} packages"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ts(s: &str) -> DateTime<FixedOffset> {
        DateTime::parse_from_rfc3339(s).unwrap()
    }

    fn tz() -> Tz {
        Tz::Fixed(FixedOffset::east_opt(7200).unwrap())
    }

    struct Tmp(std::path::PathBuf);
    impl Drop for Tmp {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }

    fn log(tag: &str, text: &str) -> Tmp {
        let p =
            std::env::temp_dir().join(format!("seldon-preview-{tag}-{}.log", std::process::id()));
        std::fs::write(&p, text).unwrap();
        Tmp(p)
    }

    /// A transaction of `n` upgrades at `day` (2026-10-DD) 12:00.
    fn tx(day: u32, n: usize, command: &str) -> String {
        let at = format!("2026-10-{day:02}T12:00:00+0200");
        let mut s =
            format!("[{at}] [PACMAN] Running '{command}'\n[{at}] [ALPM] transaction started\n");
        for i in 0..n {
            let _ = writeln!(s, "[{at}] [ALPM] upgraded pkg{i} (1.0-1 -> 1.1-1)");
        }
        let _ = writeln!(s, "[{at}] [ALPM] transaction completed");
        s
    }

    #[test]
    fn the_tail_starts_before_the_window() {
        // 4000 old transactions of 10 lines push the window past the first slice
        let mut text = String::new();
        for _ in 0..4000 {
            text.push_str(&tx(1, 10, "pacman -S old"));
        }
        let old = text.len();
        text.push_str(&tx(7, 2, "pacman -S new"));
        let t = log("tail", &text);
        assert!(old as u64 > TAIL_FIRST);
        let (bytes, capped) = tail(&t.0, ts("2026-10-05T00:00:00+02:00"), tz()).unwrap();
        assert!(!capped);
        assert!(bytes.len() < text.len(), "read the whole log");
        assert!(bytes.len() as u64 <= TAIL_FIRST);
        let first = first_time(&bytes, tz()).unwrap();
        assert!(first < ts("2026-10-05T00:00:00+02:00"));
        // a window that starts before the log: the whole file
        let (bytes, capped) = tail(&t.0, ts("2026-09-01T00:00:00+02:00"), tz()).unwrap();
        assert_eq!((bytes.len(), capped), (text.len(), false));
    }

    #[test]
    fn the_cap_says_the_window_was_not_reached() {
        let mut text = String::new();
        for _ in 0..200 {
            text.push_str(&tx(6, 10, "pacman -Syu"));
        }
        let t = log("cap", &text);
        let since = ts("2026-10-05T00:00:00+02:00");
        let (bytes, capped) = tail_within(&t.0, since, tz(), 1024, 16 << 10).unwrap();
        assert!(capped);
        assert!(
            bytes.len() <= 16 << 10 && bytes.len() > 15 << 10,
            "{}",
            bytes.len()
        );
        // a whole line first
        assert!(bytes.starts_with(b"[2026-10-06T12:00:00+0200]"));
        // the cap reaching the file's start is no cut
        let (bytes, capped) = tail_within(&t.0, since, tz(), 1024, text.len() as u64).unwrap();
        assert_eq!((bytes.len(), capped), (text.len(), false));
    }

    #[test]
    fn transactions_in_the_window_newest_first_bounded() {
        let text = [
            tx(1, 3, "pacman -S before"),
            tx(5, 1, "pacman -S btop"),
            tx(6, 25, "pacman -Syu"),
        ]
        .concat();
        let since = ts("2026-10-02T00:00:00+02:00");
        let parsed = pacman::parse(text.as_bytes(), 0, pacman::LockState::Absent, tz());
        let redactor = Redactor::builtin();
        let mut txs: Vec<Value> = parsed
            .txs
            .iter()
            .filter_map(|t| transaction(t, since, Some(&redactor)))
            .collect();
        txs.reverse();
        assert_eq!(txs.len(), 2);
        assert_eq!(txs[0]["count"], 25);
        assert_eq!(txs[0]["packages"].as_array().unwrap().len(), MAX_PACKAGES);
        assert_eq!(txs[0]["command"], "pacman -Syu");
        assert_eq!(
            txs[0]["packages"][0],
            json!({"kind": "upgrade", "name": "pkg0", "from": "1.0-1", "to": "1.1-1"})
        );
        assert_eq!(txs[1]["at"], "2026-10-05T12:00:00+02:00");
        assert_eq!(tx_summary(&txs[1]), "upgraded pkg0");
        assert_eq!(tx_summary(&txs[0]), "upgraded 25 packages");
        assert_eq!(txs[0]["kinds"], json!(["upgrade"]));
        // without a redactor (config withheld) no command line
        let t = transaction(&parsed.txs[1], since, None).unwrap();
        assert!(t.get("command").is_none());
    }

    #[test]
    fn a_transaction_across_the_window_start_keeps_its_lines_inside() {
        let text = "[2026-10-01T23:59:59+0200] [ALPM] transaction started\n\
                    [2026-10-01T23:59:59+0200] [ALPM] installed a (1-1)\n\
                    [2026-10-02T00:00:00+0200] [ALPM] installed b (1-1)\n\
                    [2026-10-02T00:00:01+0200] [ALPM] transaction failed\n";
        let parsed = pacman::parse(text.as_bytes(), 0, pacman::LockState::Absent, tz());
        let t = transaction(&parsed.txs[0], ts("2026-10-02T00:00:00+02:00"), None).unwrap();
        assert_eq!(t["count"], 1);
        assert_eq!(t["packages"][0]["name"], "b");
        assert_eq!(t["at"], "2026-10-02T00:00:00+02:00");
        assert_eq!(t["status"], "failed");
    }

    #[test]
    fn a_transaction_begins_at_its_running_line() {
        let text = "[2026-10-05T11:59:50+0200] [PACMAN] Running 'pacman -S x'\n\
                    [2026-10-05T11:59:55+0200] [ALPM] transaction started\n\
                    [2026-10-05T12:00:03+0200] [ALPM] installed x (1-1)\n\
                    [2026-10-05T12:00:04+0200] [ALPM] transaction completed\n";
        let parsed = pacman::parse(text.as_bytes(), 0, pacman::LockState::Absent, tz());
        let t = transaction(&parsed.txs[0], ts("2026-10-01T00:00:00+02:00"), None).unwrap();
        assert_eq!(t["at"], "2026-10-05T11:59:50+02:00");
    }

    #[test]
    fn a_command_line_is_redacted_and_one_line() {
        let text = "[2026-10-05T12:00:00+0200] [PACMAN] Running 'pacman -S x --config=/a\u{202E}b password=hunter2secret'\n\
                    [2026-10-05T12:00:00+0200] [ALPM] transaction started\n\
                    [2026-10-05T12:00:00+0200] [ALPM] installed x (1-1)\n\
                    [2026-10-05T12:00:00+0200] [ALPM] transaction completed\n";
        let parsed = pacman::parse(text.as_bytes(), 0, pacman::LockState::Absent, tz());
        let t = transaction(
            &parsed.txs[0],
            ts("2026-10-01T00:00:00+02:00"),
            Some(&Redactor::builtin()),
        )
        .unwrap();
        let c = t["command"].as_str().unwrap();
        assert!(!c.contains("hunter2secret"), "{c}");
        assert!(!c.contains('\u{202E}'), "{c}");
    }

    #[test]
    fn shown_replaces_what_breaks_a_line() {
        assert_eq!(
            shown("a\nb\u{2028}c\u{202E}d\te"),
            "a\u{FFFD}b\u{FFFD}c\u{FFFD}d\u{FFFD}e"
        );
        // the direction and format set the index drops (WP-138 stage 2, F2)
        assert_eq!(
            shown("a\u{200F}b\u{200E}c\u{061C}d\u{E0041}e"),
            "a\u{FFFD}b\u{FFFD}c\u{FFFD}d\u{FFFD}e"
        );
        assert_eq!(shown("~/.config/größe.conf"), "~/.config/größe.conf");
    }
}
