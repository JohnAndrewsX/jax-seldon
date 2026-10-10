//! `pacman` collector (SPEC-ENGINE §4, ADR-0013 §5, ADR-0014 §1).
//!
//! Reads `/var/log/pacman.log` from the saved byte offset and turns the
//! `[ALPM] installed|removed|upgraded|downgraded|reinstalled` lines into
//! events.
//!
//! - **Files pacman left** (WP-141). `[ALPM] warning: <file> installed as
//!   <file>.pacnew` (the new default was not applied) and `… saved as
//!   <file>.pacsave|.pacorig` (the user's file was moved aside) become a
//!   `note` whose subject is the file pacman left. Its transaction is in
//!   `meta.transaction`, not in `txId`: `txId` is the drift group of the
//!   transaction's packages (ADR-0013 §1), and a merge still to do is no
//!   member of "I wanted that package". `/etc` is never read: whether the
//!   file was merged later is not known.
//! - **Lines.** The line grammar is a table ([`LINE_RULES`]): a new log edge
//!   case is a new row or a fixture line, not a new code path. A line that
//!   matches no row is ignored (malformed, unknown tag, scriptlet output).
//!   Only complete lines count: the cursor never passes an unterminated last
//!   line (an interrupted write).
//! - **Transactions.** Lines between `transaction started` and
//!   `transaction completed` share a `txId` (`tx-<local time of transaction
//!   started>`). The latest `[PACMAN] Running '…'` line before the start is
//!   the transaction's `meta.command`. A transaction is emitted after
//!   `transaction completed`, after the next `transaction started`, or at
//!   the end of the log when pacman's `db.lck` is absent or stale. While
//!   pacman still runs, the cursor stays at the start of the transaction's
//!   block, so the next capture reads it whole (ADR-0013 §5).
//! - **Stale lock** (WP-160). A `db.lck` older than the current boot
//!   (`/proc/stat` `btime`) was left by a pacman that was killed or lost
//!   its power ([`lock_state`]): the open transaction is emitted
//!   `unfinished` when its last `[ALPM…]` line is older than the boot too,
//!   and held back otherwise (pacman wrote it since: the clock was set
//!   forward after pacman took the lock). Without a boot time, or a lock
//!   time, the lock counts as held. The lock is only looked at, never
//!   removed or touched.
//! - **Status** (ADR-0043). A transaction that ended with `transaction
//!   failed` or `transaction interrupted` writes that word as
//!   `meta.txStatus` on each of its events; one closed by the next
//!   `transaction started` or by the end of the log without a held
//!   `db.lck` writes `unfinished`. A completed one writes none.
//! - **Explicit or dependency.** `meta.command` is parsed as argv ([`parse_command`],
//!   pacman logs it unquoted, so it is split on whitespace). Packages the
//!   command names are `explicit: true`; the others in the transaction are
//!   dependencies (`explicit: false`). Without a command line, `explicit`
//!   is omitted (never routine, ADR-0015 §3).
//! - **Cursor.** Inode plus byte offset. If the inode changed, the rotated
//!   `<log>.1` is read from the old offset when it still has the old inode,
//!   then the new log from 0. If the file shrank, it is read from 0. In
//!   every case events already in the ledger are dropped by
//!   `(ts, kind, subject, version)`, so a repeated block is not emitted twice.
//! - **Reading** (WP-198). The log is read one complete line at a time
//!   through a fixed buffer ([`Lines`]), never whole; a line longer than
//!   [`MAX_LINE`] is passed like a line the table does not know. Without a
//!   cursor (a baseline, ADR-0033's look-back) the lines before the
//!   baseline are only skipped ([`look_back_start`]): of each, the time is
//!   read, and the full grammar runs only on the Running and transaction
//!   lines. The parse then begins at the first line whose time is at or
//!   after the baseline, or earlier, at the start of the transaction (or
//!   the Running line) still open there, so it emits exactly what a parse
//!   of the whole log would.
//! - **Attribution** (ADR-0014 §1, ADR-0017 §2 §3 §5). An event takes
//!   `actor` and `case` from a hook `command` event in the ledger whose
//!   `ts` (the command's start) lies at most 10 minutes before the
//!   transaction began (its Running line, else `transaction started`) and
//!   not after it. The command must name the package, and only reaches a
//!   package the transaction's own command names too (`explicit` is not
//!   `false`); a full upgrade
//!   (`-Syu`, `omarchy update`, bare `yay`) only counts for a transaction
//!   whose own command is a full upgrade, or that has none. `omarchy update`
//!   also names the keyrings it installs first. Query commands (`-Ss`, `-Q`,
//!   …) never count. Every other member of an attributed transaction
//!   inherits. Time alone is never proof.

use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{BufRead as _, BufReader, Seek as _, SeekFrom};
use std::os::unix::fs::MetadataExt as _;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use chrono::{DateTime, FixedOffset, NaiveDateTime};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{Collector, Ctx, Lost, Outcome, Tz, to_cursor, typed_cursor};
use crate::model::event::{Event, Kind, Meta, Source, TxStatus};

// The command parser and the hook causes live in neutral modules (WP-009);
// re-exported so the collector's callers and tests keep their paths.
pub use crate::attribution::{ATTRIBUTION_WINDOW, Cause, causes, find_cause};
pub use crate::pkgcmd::{
    Intent, OMARCHY_UPDATE_NAMES, Op, PacmanCommand, command_intent, parse_command, split_logged,
};

pub struct Pacman;

/// `cursors.json` → `pacman.cursor`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PacmanCursor {
    pub inode: u64,
    /// Byte offset of the first line not yet emitted.
    pub offset: u64,
}

impl Collector for Pacman {
    fn name(&self) -> &'static str {
        "pacman"
    }

    fn cursor_reads(&self, cursor: &Value) -> bool {
        typed_cursor::<PacmanCursor>(Some(cursor)).is_some()
    }

    fn collect(&self, ctx: &Ctx, cursor: Option<&Value>) -> Outcome {
        let cursor: Option<PacmanCursor> = typed_cursor(cursor);
        let lost = cursor.is_none().then_some(Lost::Cursor);
        match collect(ctx, cursor) {
            Ok((events, cursor)) => Outcome::ok(events, to_cursor(&cursor)).baseline(lost),
            Err(e) => Outcome::degraded(format!("{e:#}"), None),
        }
    }
}

fn collect(ctx: &Ctx, cursor: Option<PacmanCursor>) -> anyhow::Result<(Vec<Event>, PacmanCursor)> {
    let path = &ctx.sources.pacman_log;
    let mut lines = Lines::open(path)?;
    let meta = lines.metadata()?;
    let lock = lock_state(&ctx.sources.pacman_db_lock, &ctx.sources.proc_stat);
    let mut txs = Vec::new();

    let start = match cursor {
        Some(c) if c.inode == meta.ino() && c.offset <= meta.len() => c.offset,
        Some(c) if c.inode != meta.ino() => {
            // rotated: finish the old file if it is still there
            let old = rotated(path);
            if std::fs::metadata(&old).is_ok_and(|m| m.ino() == c.inode) {
                let mut old = Lines::open(&old)?;
                old.seek(c.offset)?;
                // the old file is closed for good: emit what it has. A
                // transaction still open there is `unfinished` (ADR-0043):
                // holding it back would lose it, since the cursor moves to
                // the new file; a pacman still running at the rotation
                // writes its end into the old file, which is not read again
                txs.extend(read(&mut old, LockState::Absent, ctx.tz)?.txs);
            }
            0
        }
        _ => 0, // no cursor (baseline), or the file shrank (truncated)
    };
    if cursor.is_none() {
        let from = look_back_start(&mut lines, ctx.baseline, ctx.tz)?;
        lines.seek(from)?;
    } else {
        lines.seek(start)?;
    }
    let parsed = read(&mut lines, lock, ctx.tz)?;
    txs.extend(parsed.txs);

    let began: HashMap<String, DateTime<FixedOffset>> = txs
        .iter()
        .filter_map(|t| Some((t.tx_id.clone()?, t.began)))
        .collect();
    let mut events: Vec<Event> = txs.into_iter().flat_map(|t| t.events()).collect();
    if cursor.is_none() {
        events.retain(|e| e.ts >= ctx.baseline);
    }
    let events = dedupe(ctx, events)?;
    let events = attribute(ctx, events, &began)?;
    Ok((
        events,
        PacmanCursor {
            inode: meta.ino(),
            offset: parsed.resume,
        },
    ))
}

/// What pacman's `db.lck` says about pacman (WP-160).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LockState {
    /// No lock: pacman is not running.
    Absent,
    /// A lock from this boot, or one whose age cannot be told: pacman
    /// runs, its open transaction is held back (ADR-0013 §5).
    Held {
        /// The boot time, when `/proc/stat` gave one.
        boot: Option<SystemTime>,
    },
    /// A lock older than the current boot: left by a pacman that was
    /// killed or lost its power. Counts as absent (ADR-0043 `unfinished`).
    Stale {
        modified: SystemTime,
        boot: SystemTime,
    },
}

impl LockState {
    /// Whether an open transaction at the end of the log is held back.
    pub fn is_held(self) -> bool {
        matches!(self, LockState::Held { .. })
    }
}

/// The state of the lock at `lock`, with the boot time from `proc_stat`.
/// Reads the lock's metadata and `proc_stat`, nothing else.
pub fn lock_state(lock: &Path, proc_stat: &Path) -> LockState {
    let Ok(meta) = std::fs::symlink_metadata(lock) else {
        return LockState::Absent;
    };
    let boot = std::fs::read_to_string(proc_stat)
        .ok()
        .and_then(|text| boot_time(&text));
    match (meta.modified(), boot) {
        (Ok(modified), Some(boot)) if modified < boot => LockState::Stale { modified, boot },
        _ => LockState::Held { boot },
    }
}

/// The `btime` line of `/proc/stat`: the boot time in whole seconds since
/// the epoch (rounded down, so a lock made after the boot is never older).
pub fn boot_time(proc_stat: &str) -> Option<SystemTime> {
    let secs = proc_stat
        .lines()
        .find_map(|l| l.strip_prefix("btime "))?
        .trim()
        .parse::<u64>()
        .ok()
        .filter(|&s| s > 0)?;
    UNIX_EPOCH.checked_add(Duration::from_secs(secs))
}

/// A Running line whose transaction has not started yet (pacman is still
/// downloading) is read again next time while any lock is there, held or
/// stale, or the transaction would lose its command line (WP-160 stage 2:
/// under a stale-looking lock this is the download phase after a forward
/// clock jump). Nothing is held back by it: it has no events. Once the
/// lock is gone it is passed, and a newer Running line replaces it.
fn hold_running(
    resume: &mut u64,
    command: Option<&(u64, DateTime<FixedOffset>, String)>,
    lock: LockState,
) {
    if let Some((off, ..)) = command
        && lock != LockState::Absent
    {
        *resume = *off;
    }
}

/// Whether `ts` (whole seconds) lies before `boot`.
fn before_boot(ts: DateTime<FixedOffset>, boot: SystemTime) -> bool {
    boot.duration_since(UNIX_EPOCH)
        .is_ok_and(|d| i64::try_from(d.as_secs()).is_ok_and(|b| ts.timestamp() < b))
}

/// `pacman.log` → `pacman.log.1`.
fn rotated(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".1");
    path.with_file_name(name)
}

/// The longest line the collector reads (WP-198): of a longer line no more
/// than this is held, and it is passed like a line the table does not
/// know. pacman's own lines are far shorter: a Running line naming 2000
/// packages is about 40 KiB.
pub const MAX_LINE: usize = 1024 * 1024;

/// The read buffer of [`Lines`].
const READ_BUFFER: usize = 64 * 1024;

/// The complete lines of a log, one at a time, through a fixed buffer
/// (WP-198): what is held is the buffer and one line, whatever the size of
/// the file.
pub struct Lines {
    reader: BufReader<File>,
    /// Offset of the next line.
    pos: u64,
    line: Vec<u8>,
}

impl Lines {
    /// Opens the log at `path` as a regular file ([`crate::sys::open_regular`]:
    /// a FIFO or a device is an error, never a read that blocks or never
    /// ends), at offset 0.
    pub fn open(path: &Path) -> anyhow::Result<Self> {
        let file = crate::sys::open_regular(path)
            .map_err(|e| anyhow::anyhow!("cannot read {}: {e}", path.display()))?;
        Ok(Lines {
            reader: BufReader::with_capacity(READ_BUFFER, file),
            pos: 0,
            line: Vec::new(),
        })
    }

    /// The metadata of the open file (its inode is the one read).
    pub fn metadata(&self) -> std::io::Result<std::fs::Metadata> {
        self.reader.get_ref().metadata()
    }

    /// Continues at absolute offset `offset`.
    pub fn seek(&mut self, offset: u64) -> std::io::Result<()> {
        self.reader.seek(SeekFrom::Start(offset))?;
        self.pos = offset;
        Ok(())
    }

    /// Offset of the next line: after the last complete line read.
    pub fn pos(&self) -> u64 {
        self.pos
    }

    /// The next complete line, without its newline, and its offset; empty
    /// for a line longer than [`MAX_LINE`]. `None` at the end of the file,
    /// also before an unterminated last line (an interrupted write), which
    /// [`Lines::pos`] then stays in front of.
    pub fn next_line(&mut self) -> std::io::Result<Option<(u64, &[u8])>> {
        let at = self.pos;
        let mut len = 0u64;
        let mut long = false;
        self.line.clear();
        loop {
            let buf = self.reader.fill_buf()?;
            if buf.is_empty() {
                return Ok(None);
            }
            let (part, done) = match buf.iter().position(|&b| b == b'\n') {
                Some(nl) => (&buf[..nl], true),
                None => (buf, false),
            };
            long = long || self.line.len() + part.len() > MAX_LINE;
            if long {
                self.line.clear();
            } else {
                self.line.extend_from_slice(part);
            }
            let used = part.len() + usize::from(done);
            self.reader.consume(used);
            len += used as u64;
            if done {
                self.pos = at + len;
                return Ok(Some((at, &self.line)));
            }
        }
    }
}

/// Parses the lines from where `lines` stands to the end ([`Parser`]).
fn read(lines: &mut Lines, lock: LockState, tz: Tz) -> std::io::Result<Parsed> {
    let mut parser = Parser::new(lock, tz);
    while let Some((at, line)) = lines.next_line()? {
        parser.line(at, line);
    }
    Ok(parser.finish(lines.pos()))
}

/// The time at the start of a line, as [`parse_line`] reads it: `None` for
/// a line that has none (it is no line of the table either).
fn line_ts(line: &[u8], tz: Tz) -> Option<DateTime<FixedOffset>> {
    let rest = line.strip_prefix(b"[")?;
    let close = rest.iter().position(|&b| b == b']')?;
    parse_ts(std::str::from_utf8(&rest[..close]).ok()?, tz)
}

/// Where a read that keeps only events at or after `baseline` begins
/// (WP-198, ADR-0033), reading from where `lines` stands. Lines are
/// skipped up to the first whose time is at or after `baseline`: of each,
/// only the time is read, and the line table only for a `[PACMAN] Running`
/// or `[ALPM] transaction` line. The start is that line, or, when a
/// transaction is open there, the start of its block (its Running line,
/// else `transaction started`, as [`parse`] rewinds), or else the latest
/// Running line no transaction has taken yet. A parse from there emits the
/// events at or after `baseline` that a parse of the whole log does: every
/// line before the start is older than `baseline`, and what a line before
/// it passes on to the lines after it (the open transaction, the Running
/// line) begins at or after the start. Without such a line, the end of the
/// complete lines, or the open block before it.
pub fn look_back_start(
    lines: &mut Lines,
    baseline: DateTime<FixedOffset>,
    tz: Tz,
) -> std::io::Result<u64> {
    // the parser's state as offsets: the open transaction's block, the
    // Running line not yet taken
    let mut open: Option<u64> = None;
    let mut running: Option<u64> = None;
    let stop = loop {
        let Some((at, line)) = lines.next_line()? else {
            break lines.pos();
        };
        let Some(ts) = line_ts(line, tz) else {
            continue;
        };
        if ts >= baseline {
            break at;
        }
        let rest = &line[line.iter().position(|&b| b == b']').unwrap_or(0)..];
        if !(rest.starts_with(b"] [PACMAN] Running '")
            || rest.starts_with(b"] [ALPM] transaction "))
        {
            continue;
        }
        match parse_line(&String::from_utf8_lossy(line), tz) {
            Some((_, Line::Command(_))) => running = Some(at),
            Some((_, Line::TxStart)) => open = Some(running.take().unwrap_or(at)),
            Some((_, Line::TxEnd(_))) => open = None,
            _ => {}
        }
    };
    Ok(open.or(running).unwrap_or(stop))
}

// ---------------------------------------------------------------------------
// Line grammar
// ---------------------------------------------------------------------------

/// What a log line means.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Line {
    /// `[PACMAN] Running '<argv joined by spaces>'`.
    Command(String),
    /// `[ALPM] transaction started`.
    TxStart,
    /// `[ALPM] transaction completed|failed|interrupted`: `None` when it
    /// completed (ADR-0043).
    TxEnd(Option<TxStatus>),
    /// `[ALPM] <verb> <name> (<version>)` or `(<from> -> <to>)`.
    Package {
        kind: Kind,
        name: String,
        from: Option<String>,
        to: String,
    },
    /// `[ALPM] warning: <file> installed as <file>.pacnew` or `saved as
    /// <file>.pacsave|.pacorig`: the file pacman left beside `file`.
    Left { file: String, left: String },
}

/// The suffixes of the files pacman leaves beside a configuration file,
/// with the verb its log line uses.
const LEFT: [(&str, &str); 3] = [
    ("installed", ".pacnew"),
    ("saved", ".pacsave"),
    ("saved", ".pacorig"),
];

/// A `warning: <file> <verb> as <left>` line whose `left` is `file` with
/// the suffix the verb leaves; anything else is no such line.
fn left_line(c: &regex::Captures) -> Option<Line> {
    let (file, verb, left) = (&c[1], &c[2], &c[3]);
    LEFT.iter()
        .any(|(v, suffix)| *v == verb && left.strip_suffix(suffix) == Some(file))
        .then(|| Line::Left {
            file: file.to_string(),
            left: left.to_string(),
        })
}

type Build = fn(&regex::Captures) -> Option<Line>;

/// The line table: tag, message pattern, meaning. First match wins.
pub static LINE_RULES: LazyLock<Vec<(&'static str, Regex, Build)>> = LazyLock::new(|| {
    let re = |p: &str| Regex::new(p).expect("pacman line pattern compiles");
    vec![
        ("PACMAN", re(r"^Running '(.*)'$"), |c| {
            Some(Line::Command(c[1].to_string()))
        }),
        ("ALPM", re(r"^transaction started$"), |_| {
            Some(Line::TxStart)
        }),
        (
            "ALPM",
            re(r"^transaction (completed|failed|interrupted)$"),
            |c| {
                Some(Line::TxEnd(match &c[1] {
                    "failed" => Some(TxStatus::Failed),
                    "interrupted" => Some(TxStatus::Interrupted),
                    _ => None,
                }))
            },
        ),
        (
            "ALPM",
            re(r"^(installed|removed|reinstalled) (\S+) \(([^()\s]+)\)$"),
            |c| {
                Some(Line::Package {
                    kind: match &c[1] {
                        "installed" => Kind::Install,
                        "removed" => Kind::Remove,
                        _ => Kind::Reinstall,
                    },
                    name: c[2].to_string(),
                    from: None,
                    to: c[3].to_string(),
                })
            },
        ),
        (
            "ALPM",
            re(r"^(upgraded|downgraded) (\S+) \(([^()\s]+) -> ([^()\s]+)\)$"),
            |c| {
                Some(Line::Package {
                    kind: if &c[1] == "upgraded" {
                        Kind::Upgrade
                    } else {
                        Kind::Downgrade
                    },
                    name: c[2].to_string(),
                    from: Some(c[3].to_string()),
                    to: c[4].to_string(),
                })
            },
        ),
        (
            "ALPM",
            re(r"^warning: (/.+?) (installed|saved) as (/.+)$"),
            left_line,
        ),
    ]
});

static LINE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\[([^\]]+)\] \[([A-Z][A-Z-]*)\] (.*)$").expect("line pattern compiles")
});

/// Parses one line (without its newline): its time and meaning, or `None`
/// for anything the table does not know.
pub fn parse_line(line: &str, tz: Tz) -> Option<(DateTime<FixedOffset>, Line)> {
    let caps = LINE.captures(line.trim_end_matches('\r'))?;
    let ts = parse_ts(&caps[1], tz)?;
    let (tag, msg) = (&caps[2], &caps[3]);
    LINE_RULES
        .iter()
        .filter(|(t, _, _)| *t == tag)
        .find_map(|(_, re, build)| re.captures(msg).and_then(|c| build(&c)))
        .map(|l| (ts, l))
}

/// The time of a line libalpm wrote (`[ALPM]`, `[ALPM-SCRIPTLET]`), whatever
/// it says; `None` for pacman's own lines (`[PACMAN] Running` comes before
/// pacman takes the lock, WP-160) and malformed ones.
fn alpm_ts(line: &str, tz: Tz) -> Option<DateTime<FixedOffset>> {
    let caps = LINE.captures(line.trim_end_matches('\r'))?;
    caps[2]
        .starts_with("ALPM")
        .then(|| parse_ts(&caps[1], tz))
        .flatten()
}

/// `2026-09-03T21:14:06+0200` (pacman ≥ 5.2), or the old offset-less
/// `2019-01-01 12:00` in the local zone.
fn parse_ts(s: &str, tz: Tz) -> Option<DateTime<FixedOffset>> {
    DateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S%z")
        .ok()
        .or_else(|| {
            NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M")
                .ok()
                .and_then(|n| tz.localize(n))
        })
}

// ---------------------------------------------------------------------------
// Transactions
// ---------------------------------------------------------------------------

/// One package line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PkgLine {
    pub ts: DateTime<FixedOffset>,
    pub kind: Kind,
    pub name: String,
    pub from: Option<String>,
    pub to: String,
}

/// One file pacman left beside a configuration file (`.pacnew`,
/// `.pacsave`, `.pacorig`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeftLine {
    pub ts: DateTime<FixedOffset>,
    /// The configuration file.
    pub file: String,
    /// The file pacman left: `file` and the suffix.
    pub left: String,
}

/// The `meta` key of a left file's transaction id (WP-141): its `txId`
/// would make it a member of the transaction's drift group.
pub const TRANSACTION_KEY: &str = "transaction";

/// A transaction (or a package line outside any, from old logs).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tx {
    /// `None` for package lines outside a transaction.
    pub tx_id: Option<String>,
    /// The `[PACMAN] Running` line that started it.
    pub command: Option<String>,
    /// Time of the Running line, else of `transaction started`.
    pub began: DateTime<FixedOffset>,
    pub lines: Vec<PkgLine>,
    /// The files it left, in log order.
    pub left: Vec<LeftLine>,
    /// How it ended when it did not complete (ADR-0043): pacman's `failed`
    /// or `interrupted`, or `unfinished` without an end line. `None` when
    /// it completed, and outside any transaction.
    pub status: Option<TxStatus>,
}

impl Tx {
    /// The events of this transaction, unattributed (`actor: system`): its
    /// packages, then the files it left.
    pub fn events(&self) -> Vec<Event> {
        let mut events = self.package_events();
        events.extend(self.left.iter().map(|l| {
            let verb = if l.left.ends_with(".pacnew") {
                "installed"
            } else {
                "saved"
            };
            let mut meta = Meta {
                command: self.command.clone(),
                ..Meta::default()
            };
            if let Some(tx) = &self.tx_id {
                meta.extra
                    .insert(TRANSACTION_KEY.into(), Value::String(tx.clone()));
            }
            Event::new(l.ts, Source::Pacman, Kind::Note, &l.left)
                .detail(format!("{} {verb} as {}", l.file, l.left))
                .meta(meta)
        }));
        events
    }

    fn package_events(&self) -> Vec<Event> {
        let cmd = self
            .command
            .as_deref()
            .map(|c| parse_command(&split_logged(c)));
        self.lines
            .iter()
            .map(|l| {
                let mut meta = Meta {
                    command: self.command.clone(),
                    tx_status: self.status,
                    ..Meta::default()
                };
                let detail = match &l.from {
                    Some(from) => {
                        meta.from = Some(from.clone());
                        meta.to = Some(l.to.clone());
                        format!("{from} → {}", l.to)
                    }
                    None => {
                        meta.version = Some(l.to.clone());
                        l.to.clone()
                    }
                };
                let mut e = Event::new(l.ts, Source::Pacman, l.kind, &l.name)
                    .detail(detail)
                    .meta(meta);
                e.tx_id = self.tx_id.clone();
                e.explicit = cmd
                    .as_ref()
                    .map(|c| c.as_ref().is_some_and(|c| c.names(&l.name)));
                e
            })
            .collect()
    }
}

/// The result of reading a stretch of the log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parsed {
    /// Transactions ready to emit, in log order.
    pub txs: Vec<Tx>,
    /// Absolute offset where the next read starts: after the last complete
    /// line, or at the start of a transaction that is still running.
    pub resume: u64,
}

/// Parses `bytes`, which start at absolute offset `base` of the file.
/// `lock`: pacman's `db.lck` ([`lock_state`]). Held: an open transaction at
/// the end is still running and is held back. Stale (WP-160): it is
/// emitted `unfinished` only when its last `[ALPM…]` line is older than the
/// boot too; one that pacman wrote since (the clock was set forward after
/// pacman took the lock) holds it back as if held. Absent: emitted.
pub fn parse(bytes: &[u8], base: u64, lock: LockState, tz: Tz) -> Parsed {
    let mut parser = Parser::new(lock, tz);
    let mut pos = 0usize;
    while let Some(nl) = bytes[pos..].iter().position(|&b| b == b'\n') {
        let line = &bytes[pos..pos + nl];
        parser.line(
            base + pos as u64,
            if line.len() > MAX_LINE { &[] } else { line },
        );
        pos += nl + 1;
    }
    parser.finish(base + pos as u64)
}

/// A transaction the parser has seen start and not end.
struct Open {
    tx: Tx,
    rewind: u64,
    /// The newest `[ALPM…]` line of the transaction (WP-160).
    last: DateTime<FixedOffset>,
}

/// [`parse`] one complete line at a time (WP-198): [`Parser::line`] for
/// each, in log order, then [`Parser::finish`].
pub struct Parser {
    lock: LockState,
    tz: Tz,
    txs: Vec<Tx>,
    open: Option<Open>,
    /// Latest Running line: (offset, ts, command).
    command: Option<(u64, DateTime<FixedOffset>, String)>,
}

impl Parser {
    pub fn new(lock: LockState, tz: Tz) -> Self {
        Parser {
            lock,
            tz,
            txs: Vec::new(),
            open: None,
            command: None,
        }
    }

    /// One complete line, without its newline, at absolute offset `at`.
    pub fn line(&mut self, at: u64, bytes: &[u8]) {
        let (tz, txs, open) = (self.tz, &mut self.txs, &mut self.open);
        let text = String::from_utf8_lossy(bytes);
        let Some((ts, line)) = parse_line(&text, tz) else {
            // scriptlet output and hook lines are the transaction's too
            if let Some(o) = open
                && let Some(t) = alpm_ts(&text, tz)
            {
                o.last = o.last.max(t);
            }
            return;
        };
        match line {
            Line::Command(c) => self.command = Some((at, ts, c)),
            Line::TxStart => {
                // a transaction without an end line did not complete
                if let Some(mut o) = open.take() {
                    o.tx.status = Some(TxStatus::Unfinished);
                    txs.push(o.tx);
                }
                let (rewind, began, cmd) = match self.command.take() {
                    Some((off, t, c)) => (off, t, Some(c)),
                    None => (at, ts, None),
                };
                *open = Some(Open {
                    tx: Tx {
                        tx_id: Some(format!("tx-{}", ts.format("%Y%m%dT%H%M%S"))),
                        command: cmd,
                        began,
                        lines: Vec::new(),
                        left: Vec::new(),
                        status: None,
                    },
                    rewind,
                    last: ts,
                });
            }
            Line::TxEnd(status) => {
                if let Some(mut o) = open.take() {
                    o.tx.status = status;
                    txs.push(o.tx);
                }
            }
            Line::Package {
                kind,
                name,
                from,
                to,
            } => {
                let line = PkgLine {
                    ts,
                    kind,
                    name,
                    from,
                    to,
                };
                match open {
                    Some(o) => {
                        o.last = o.last.max(ts);
                        o.tx.lines.push(line);
                    }
                    None => txs.push(Tx {
                        tx_id: None,
                        command: None,
                        began: ts,
                        lines: vec![line],
                        left: Vec::new(),
                        status: None,
                    }),
                }
            }
            Line::Left { file, left } => {
                let line = LeftLine { ts, file, left };
                match open {
                    Some(o) => {
                        o.last = o.last.max(ts);
                        o.tx.left.push(line);
                    }
                    None => txs.push(Tx {
                        tx_id: None,
                        command: None,
                        began: ts,
                        lines: Vec::new(),
                        left: vec![line],
                        status: None,
                    }),
                }
            }
        }
    }

    /// The transactions read, and where the next read starts; `end` is the
    /// offset after the last complete line.
    pub fn finish(self, end: u64) -> Parsed {
        let Parser {
            lock,
            mut txs,
            open,
            command,
            ..
        } = self;
        // whether a transaction (or a Running line) whose last line has
        // time `last` is still pacman's at the end of the log
        let held = |last: DateTime<FixedOffset>| match lock {
            LockState::Absent => false,
            LockState::Held { .. } => true,
            LockState::Stale { boot, .. } => !before_boot(last, boot),
        };
        let mut resume = end;
        match open {
            Some(o) if held(o.last) => resume = o.rewind,
            // pacman is gone and never ended it (killed, a crash, power loss)
            Some(mut o) => {
                o.tx.status = Some(TxStatus::Unfinished);
                txs.push(o.tx);
                hold_running(&mut resume, command.as_ref(), lock);
            }
            None => hold_running(&mut resume, command.as_ref(), lock),
        }
        txs.retain(|t| !t.lines.is_empty() || !t.left.is_empty());
        Parsed { txs, resume }
    }
}

// ---------------------------------------------------------------------------
// Dedupe and attribution
// ---------------------------------------------------------------------------

type Key = (i64, Kind, String, Option<String>);

fn key(e: &Event) -> Key {
    (
        e.ts.timestamp(),
        e.kind,
        e.subject.clone(),
        e.version_key().map(str::to_string),
    )
}

/// Drops events whose `(ts, kind, subject, version)` is already in the
/// ledger or earlier in `events` (SPEC-ENGINE §4 rotation dedupe).
fn dedupe(ctx: &Ctx, events: Vec<Event>) -> anyhow::Result<Vec<Event>> {
    let (Some(first), Some(last)) = (
        events.iter().map(|e| e.ts).min(),
        events.iter().map(|e| e.ts).max(),
    ) else {
        return Ok(events);
    };
    let mut seen: HashSet<Key> = ctx
        .ledger
        .read_range(first, last)?
        .iter()
        .filter(|e| e.source == Source::Pacman)
        .map(key)
        .collect();
    Ok(events.into_iter().filter(|e| seen.insert(key(e))).collect())
}

/// Whether a transaction with this logged command may be caused by a
/// full-upgrade command: its own command is a full upgrade, or it has none.
fn tx_is_full_upgrade(command: Option<&str>) -> bool {
    command.is_none_or(|c| parse_command(&split_logged(c)).is_some_and(|p| p.is_full_upgrade()))
}

/// The transaction of a pacman event: its `txId`, or for a file it left
/// `meta.transaction` ([`TRANSACTION_KEY`]).
pub fn transaction(e: &Event) -> Option<&str> {
    e.tx_id.as_deref().or_else(|| {
        (e.kind == Kind::Note)
            .then(|| e.meta.extra.get(TRANSACTION_KEY)?.as_str())
            .flatten()
    })
}

/// Sets `actor`/`case` from the hook command that caused each event
/// (ADR-0014 §1, ADR-0017 §2 §3). `began` maps a txId to the time its
/// pacman invocation began (the Running line, else `transaction started`);
/// an event outside a transaction begins at its own `ts`. A member without
/// a cause of its own inherits from an attributed member of its
/// transaction, explicit ones first; so does a file it left
/// ([`transaction`]).
pub fn attribute(
    ctx: &Ctx,
    mut events: Vec<Event>,
    began: &HashMap<String, DateTime<FixedOffset>>,
) -> anyhow::Result<Vec<Event>> {
    let start = |e: &Event| {
        transaction(e)
            .and_then(|t| began.get(t))
            .map_or(e.ts, |b| (*b).min(e.ts))
    };
    let (Some(first), Some(last)) = (
        events.iter().map(start).min(),
        events.iter().map(|e| e.ts).max(),
    ) else {
        return Ok(events);
    };
    let mut known = ctx.ledger.read_range(first - ATTRIBUTION_WINDOW, last)?;
    known.extend(ctx.earlier.iter().cloned());
    let causes = causes(&known);
    if causes.is_empty() {
        return Ok(events);
    }
    let mut found: Vec<Option<(String, Option<String>)>> = events
        .iter()
        .map(|e| {
            let full = tx_is_full_upgrade(e.meta.command.as_deref());
            let named = e.explicit != Some(false);
            find_cause(&causes, &e.subject, start(e), named, full)
                .map(|c| (c.actor.clone(), c.case.clone()))
        })
        .collect();
    // every member of an attributed transaction inherits (ADR-0017 §2)
    for i in 0..events.len() {
        if found[i].is_none() && transaction(&events[i]).is_some() {
            let same_tx = |j: &usize| {
                transaction(&events[*j]) == transaction(&events[i]) && found[*j].is_some()
            };
            found[i] = (0..events.len())
                .filter(same_tx)
                .find(|&j| events[j].explicit == Some(true))
                .or_else(|| (0..events.len()).find(same_tx))
                .and_then(|j| found[j].clone());
        }
    }
    for (e, f) in events.iter_mut().zip(found) {
        if let Some((actor, case)) = f {
            e.actor = actor;
            e.case = case;
        }
    }
    Ok(events)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// pacman runs (the tests before WP-160 passed `true`).
    const HELD: LockState = LockState::Held { boot: None };

    fn tz() -> Tz {
        Tz::Fixed(FixedOffset::east_opt(2 * 3600).unwrap())
    }

    fn argv(s: &str) -> PacmanCommand {
        parse_command(&split_logged(s)).unwrap()
    }

    /// WP-160: the `btime` line, and nothing that only looks like one.
    #[test]
    fn boot_time_from_proc_stat() {
        let at = |s: u64| Some(UNIX_EPOCH + Duration::from_secs(s));
        assert_eq!(
            boot_time("cpu  1 2 3\nintr 5\nctxt 9\nbtime 1759900000\nprocesses 4\n"),
            at(1_759_900_000)
        );
        assert_eq!(boot_time("btime 42"), at(42), "no newline at the end");
        for bad in [
            "",
            "cpu 1 2 3\n",
            "btime\n",
            "btime \n",
            "btime -5\n",
            "btime 0\n",
            "btime 12x\n",
            "xbtime 12\n",
            "btimes 12\n",
        ] {
            assert_eq!(boot_time(bad), None, "{bad:?}");
        }
    }

    /// WP-160: absent, held (this boot, or no boot time) and stale (older
    /// than the boot). The lock is never changed.
    #[test]
    fn lock_states() {
        /// Removes the folder also when an assertion fails.
        struct Cleanup(PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let dir = std::env::temp_dir().join(format!("seldon-dblck-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let _cleanup = Cleanup(dir.clone());
        let lock = dir.join("db.lck");
        let stat = dir.join("stat");
        let boot = UNIX_EPOCH + Duration::from_secs(1_759_900_000);
        std::fs::write(&stat, "cpu 1\nbtime 1759900000\n").unwrap();
        assert_eq!(lock_state(&lock, &stat), LockState::Absent);
        assert!(!LockState::Absent.is_held());

        std::fs::write(&lock, "").unwrap();
        let set = |t: SystemTime| {
            File::options()
                .write(true)
                .open(&lock)
                .unwrap()
                .set_modified(t)
                .unwrap();
        };
        // made after the boot, in the boot second, and a second before it
        set(boot + Duration::from_secs(60));
        assert_eq!(
            lock_state(&lock, &stat),
            LockState::Held { boot: Some(boot) }
        );
        set(boot);
        assert!(
            lock_state(&lock, &stat).is_held(),
            "the boot second is this boot"
        );
        let before = boot - Duration::from_secs(1);
        set(before);
        let state = lock_state(&lock, &stat);
        assert_eq!(
            state,
            LockState::Stale {
                modified: before,
                boot
            }
        );
        assert!(!state.is_held());

        // no boot time: today's rule, held
        assert_eq!(
            lock_state(&lock, &dir.join("no-stat")),
            LockState::Held { boot: None }
        );
        std::fs::write(&stat, "cpu 1\n").unwrap();
        assert_eq!(lock_state(&lock, &stat), LockState::Held { boot: None });

        // looked at, never touched
        assert_eq!(
            std::fs::metadata(&lock).unwrap().modified().unwrap(),
            before
        );
        assert_eq!(std::fs::read(&lock).unwrap(), b"");

        // a dangling symbolic link is a lock too: pacman's O_EXCL create
        // fails on it (its own mtime is now, after the boot)
        std::fs::write(&stat, "btime 1759900000\n").unwrap();
        let link = dir.join("link.lck");
        std::os::unix::fs::symlink(dir.join("nowhere"), &link).unwrap();
        assert_eq!(
            lock_state(&link, &stat),
            LockState::Held { boot: Some(boot) }
        );
        assert!(!dir.join("nowhere").exists(), "the link is not followed");
    }

    /// WP-160 round 2: a stale lock lets the open transaction go only when
    /// its last `[ALPM…]` line is older than the boot too. A line pacman
    /// wrote since (the clock was set forward after it took the lock)
    /// keeps it back; pacman's own `[PACMAN] Running` line does not count
    /// (pacman logs it before it takes the lock: a retry after the boot
    /// that failed on the lock).
    #[test]
    fn a_stale_lock_and_the_last_line() {
        // boot at 2026-10-01T10:30:00+02:00
        let boot = UNIX_EPOCH + Duration::from_secs(1_790_843_400);
        let stale = LockState::Stale {
            modified: boot - Duration::from_secs(1800),
            boot,
        };
        let open = "[2026-10-01T10:00:00+0200] [PACMAN] Running 'pacman -Syu'\n\
                    [2026-10-01T10:00:01+0200] [ALPM] transaction started\n\
                    [2026-10-01T10:00:02+0200] [ALPM] upgraded gtk4 (1:4.18.6-1 -> 1:4.18.7-1)\n";
        let emitted = |log: &str| {
            let p = parse(&lines(log), 0, stale, tz());
            assert_eq!(
                p.resume as usize,
                if p.txs.is_empty() { 0 } else { log.len() },
                "{log}"
            );
            p.txs.iter().map(|t| t.status).collect::<Vec<_>>()
        };
        assert_eq!(emitted(open), [Some(TxStatus::Unfinished)]);
        // a retry after the boot that failed on the lock: the dead
        // transaction is emitted, its Running line is read again while the
        // lock is there (stage 2 N1)
        let retry = format!("{open}[2026-10-01T10:31:00+0200] [PACMAN] Running 'pacman -S zed'\n");
        let p = parse(&lines(&retry), 0, stale, tz());
        assert_eq!(
            p.txs.iter().map(|t| t.status).collect::<Vec<_>>(),
            [Some(TxStatus::Unfinished)]
        );
        assert_eq!(
            p.resume as usize,
            open.len(),
            "the Running line is read again"
        );
        // the lock gone: passed
        let p = parse(&lines(&retry), 0, LockState::Absent, tz());
        assert_eq!(p.resume as usize, retry.len());
        // pacman wrote since the boot: a package line, a hook, a scriptlet
        for later in [
            "[2026-10-01T10:30:00+0200] [ALPM] upgraded linux (6.16.9-1 -> 6.16.10-1)\n",
            "[2026-10-01T10:31:00+0200] [ALPM] running '60-mkinitcpio-remove.hook'...\n",
            "[2026-10-01T10:31:00+0200] [ALPM-SCRIPTLET] ==> Building image\n",
            "[2026-10-01T10:31:00+0200] [ALPM] warning: /etc/pacman.conf installed as /etc/pacman.conf.pacnew\n",
        ] {
            assert!(emitted(&format!("{open}{later}")).is_empty(), "{later}");
        }
        // the last line one second before the boot
        assert_eq!(
            emitted(&format!(
                "{open}[2026-10-01T10:29:59+0200] [ALPM-SCRIPTLET] ==> Building image\n"
            )),
            [Some(TxStatus::Unfinished)]
        );
        // only a Running line (no transaction yet): read again while any
        // lock is there, from this boot or older (the download phase after
        // a forward clock jump, stage 2 N1); passed once the lock is gone
        let running = "[2026-10-01T10:31:00+0200] [PACMAN] Running 'pacman -Syu'\n";
        assert_eq!(parse(&lines(running), 7, stale, tz()).resume, 7);
        let old = "[2026-10-01T10:00:00+0200] [PACMAN] Running 'pacman -Syu'\n";
        assert_eq!(parse(&lines(old), 7, stale, tz()).resume, 7);
        assert_eq!(
            parse(&lines(old), 7, LockState::Absent, tz()).resume,
            7 + old.len() as u64
        );
    }

    #[test]
    fn line_table() {
        let l = |s: &str| parse_line(s, tz()).map(|(_, l)| l);
        assert_eq!(
            l("[2026-09-03T21:14:06+0200] [ALPM] installed btop (1.4.5-1)"),
            Some(Line::Package {
                kind: Kind::Install,
                name: "btop".into(),
                from: None,
                to: "1.4.5-1".into()
            })
        );
        assert!(matches!(
            l("[2026-08-30T21:20:13+0200] [ALPM] downgraded mesa (1:26.2.1-1 -> 1:26.2.0-2)"),
            Some(Line::Package {
                kind: Kind::Downgrade,
                ..
            })
        ));
        assert_eq!(
            l("[2026-09-03T21:14:06+0200] [PACMAN] Running 'pacman -S btop'"),
            Some(Line::Command("pacman -S btop".into()))
        );
        // ADR-0043: the end line says how it ended
        for (msg, want) in [
            ("completed", None),
            ("failed", Some(TxStatus::Failed)),
            ("interrupted", Some(TxStatus::Interrupted)),
        ] {
            assert_eq!(
                l(&format!(
                    "[2026-09-19T08:31:12+0200] [ALPM] transaction {msg}"
                )),
                Some(Line::TxEnd(want)),
                "{msg}"
            );
        }
        for bad in [
            "[2026-09-19T08:31:12+0200] [ALPM] transaction aborted",
            "[2026-09-19T08:31:12+0200] [ALPM] transaction interrupted by user",
            "[2026-09-19T08:31:12+0200] [PACMAN] transaction interrupted",
        ] {
            assert_eq!(l(bad), None, "{bad}");
        }
        for bad in [
            "",
            "garbage line without a timestamp",
            "[2026-13-45T99:99:99+0200] [ALPM] installed impossible-date (1.0-1)",
            "[2026-09-03T21:30:00+0200] [ALPM] inst",
            "[2026-09-03T21:30:01+0200] [ALPM] installed missing-version-parens 1.0-1",
            "[2026-09-03T21:30:02+0200] [UNKNOWN] something new",
            "[2026-10-01T09:13:58+0200] [ALPM] upgraded broken-line-without-version",
            "[2026-10-01T17:04:59+0200] [PACMAN] Running 'pacman -S --noconfirm nv",
            "[2026-08-28T12:15:10+0000] [ALPM-SCRIPTLET] installed fake (1-1)",
        ] {
            assert_eq!(l(bad), None, "{bad}");
        }
        // pre-5.2 format, local zone
        let (ts, _) = parse_line("[2019-01-01 12:00] [ALPM] installed a (1-1)", tz()).unwrap();
        assert_eq!(ts.to_rfc3339(), "2019-01-01T12:00:00+02:00");
    }

    /// WP-141: the three forms of a file pacman leaves; a line whose left
    /// file is not the configuration file and the verb's suffix is none.
    #[test]
    fn left_file_lines() {
        let l = |s: &str| parse_line(s, tz()).map(|(_, l)| l);
        let left = |file: &str, left: &str| {
            Some(Line::Left {
                file: file.into(),
                left: left.into(),
            })
        };
        assert_eq!(
            l(
                "[2026-10-01T09:13:58+0200] [ALPM] warning: /etc/pacman.conf installed as /etc/pacman.conf.pacnew"
            ),
            left("/etc/pacman.conf", "/etc/pacman.conf.pacnew")
        );
        assert_eq!(
            l(
                "[2026-10-01T09:13:58+0200] [ALPM] warning: /etc/ssh/sshd_config saved as /etc/ssh/sshd_config.pacsave"
            ),
            left("/etc/ssh/sshd_config", "/etc/ssh/sshd_config.pacsave")
        );
        assert_eq!(
            l(
                "[2026-10-01T09:13:58+0200] [ALPM] warning: /etc/my file.conf saved as /etc/my file.conf.pacorig\r"
            ),
            left("/etc/my file.conf", "/etc/my file.conf.pacorig"),
            "a space in the path, a CRLF ending"
        );
        for bad in [
            "[2026-10-01T09:13:58+0200] [ALPM] warning: /etc/a installed as /etc/b.pacnew",
            "[2026-10-01T09:13:58+0200] [ALPM] warning: /etc/a saved as /etc/a.pacnew",
            "[2026-10-01T09:13:58+0200] [ALPM] warning: /etc/a installed as /etc/a.pacsave",
            "[2026-10-01T09:13:58+0200] [ALPM] warning: /etc/a installed as /etc/a.pacnew.1",
            "[2026-10-01T09:13:58+0200] [ALPM] warning: etc/a installed as etc/a.pacnew",
            "[2026-10-01T09:13:58+0200] [ALPM] warning: directory permissions differ on /etc/x/",
            "[2026-10-01T09:13:58+0200] [ALPM-SCRIPTLET] warning: /etc/a installed as /etc/a.pacnew",
            "[2026-10-01T09:13:58+0200] [PACMAN] warning: /etc/a installed as /etc/a.pacnew",
            "[2026-10-01T09:13:58+0200] [ALPM] /etc/a installed as /etc/a.pacnew",
        ] {
            assert_eq!(l(bad), None, "{bad}");
        }
    }

    /// WP-141: a left file is a `note` of its transaction, its id in
    /// `meta.transaction` (never `txId`), after the packages; outside a
    /// transaction it has neither.
    #[test]
    fn left_files_are_notes_of_their_transaction() {
        let log = lines(
            "[2026-10-01T10:00:00+0200] [PACMAN] Running 'pacman -Syu'\n\
             [2026-10-01T10:00:01+0200] [ALPM] transaction started\n\
             [2026-10-01T10:00:02+0200] [ALPM] warning: /etc/mkinitcpio.conf installed as /etc/mkinitcpio.conf.pacnew\n\
             [2026-10-01T10:00:02+0200] [ALPM] upgraded mkinitcpio (40-1 -> 41-1)\n\
             [2026-10-01T10:00:03+0200] [ALPM] warning: /etc/foo.conf saved as /etc/foo.conf.pacsave\n\
             [2026-10-01T10:00:03+0200] [ALPM] removed foo (1-1)\n\
             [2026-10-01T10:00:03+0200] [ALPM] transaction completed\n\
             [2019-01-01 12:00] [ALPM] warning: /etc/old saved as /etc/old.pacorig\n",
        );
        let p = parse(&log, 0, LockState::Absent, tz());
        assert_eq!(p.txs.len(), 2);
        let events = p.txs[0].events();
        let kinds: Vec<(Kind, &str)> = events
            .iter()
            .map(|e| (e.kind, e.subject.as_str()))
            .collect();
        assert_eq!(
            kinds,
            [
                (Kind::Upgrade, "mkinitcpio"),
                (Kind::Remove, "foo"),
                (Kind::Note, "/etc/mkinitcpio.conf.pacnew"),
                (Kind::Note, "/etc/foo.conf.pacsave"),
            ]
        );
        let note = &events[2];
        assert_eq!(note.source, Source::Pacman);
        assert_eq!(note.tx_id, None, "no member of the package group");
        assert_eq!(note.explicit, None);
        assert_eq!(
            note.meta.extra.get(TRANSACTION_KEY),
            Some(&Value::String("tx-20261001T100001".into()))
        );
        assert_eq!(transaction(note), Some("tx-20261001T100001"));
        assert_eq!(note.meta.command.as_deref(), Some("pacman -Syu"));
        assert_eq!(
            note.detail.as_deref(),
            Some("/etc/mkinitcpio.conf installed as /etc/mkinitcpio.conf.pacnew")
        );
        assert_eq!(
            events[3].detail.as_deref(),
            Some("/etc/foo.conf saved as /etc/foo.conf.pacsave")
        );
        assert_eq!(note.zone, Some(crate::model::Zone::Red), "ADR-0014");
        assert_eq!(note.ts.to_rfc3339(), "2026-10-01T10:00:02+02:00");
        let lone = p.txs[1].events();
        assert_eq!(lone.len(), 1);
        assert_eq!(lone[0].subject, "/etc/old.pacorig");
        assert_eq!(transaction(&lone[0]), None);
        assert!(lone[0].meta.extra.is_empty());
    }

    #[test]
    fn argv_parsing() {
        let c = argv("pacman -Syu --noconfirm --overwrite /usr/share/omarchy/*");
        assert!(c.is_plain_full_upgrade(), "{c:?}");
        let c = argv("pacman -S --noconfirm --ask 4 quickshell");
        assert_eq!(c.targets, ["quickshell"]);
        let c = argv("pacman -S --needed --noconfirm --config /etc/pacman.conf -- extra/zed");
        assert_eq!(c.targets, ["zed"]);
        let c = argv("pacman -Rns --noconfirm libayatana-indicator vulkan-headers");
        assert_eq!(
            (c.op, c.sysupgrade, c.targets.as_slice()),
            (
                Some(Op::Remove),
                false,
                &["libayatana-indicator".to_string(), "vulkan-headers".into()][..]
            )
        );
        let c = argv("pacman -U /var/cache/pacman/pkg/mesa-1:26.2.0-2-x86_64.pkg.tar.zst");
        assert_eq!(c.targets, ["mesa"]);
        let c = argv(
            "pacman -b /mnt//var/lib/pacman/ -r /mnt -Sy --noconfirm --needed --config=/tmp/pacman.conf.Q3vx --disable-sandbox --cachedir=/mnt//var/cache/pacman/pkg/ base sudo",
        );
        assert_eq!(c.targets, ["base", "sudo"]);
        assert!(!c.is_full_upgrade());
        let c = argv("pacman -Syu --frobnicate value");
        assert_eq!(c.targets, ["value"], "unknown options take no argument");
        assert!(!c.is_plain_full_upgrade());
        let c = argv("pacman -Sy --noconfirm archlinux-keyring");
        assert!(!c.is_full_upgrade());
        assert!(argv("yay").is_plain_full_upgrade());
        assert_eq!(argv("yay -S --noconfirm zed").targets, ["zed"]);
        assert!(parse_command(&["sed", "-i"]).is_none());
        assert_eq!(
            argv("pacman -D -q --asexplicit -- zed").op,
            Some(Op::Database)
        );
    }

    #[test]
    fn query_forms_are_not_mutating() {
        for q in [
            "pacman -Ss zed",
            "pacman -Si zed",
            "pacman -Sl extra",
            "pacman -Sg base-devel",
            "pacman -Sp zed",
            "pacman -Sw zed",
            "pacman -Scc",
            "pacman -S --search zed",
            "pacman --sync --info zed",
            "pacman -S --downloadonly zed",
            "pacman -S --clean",
            "pacman -Qi zed",
            "pacman -Qdtq",
            "pacman --query zed",
            "pacman -T zed",
            "pacman -Rp zed",
            "pacman -U --print foo-1-1-x86_64.pkg.tar.zst",
            "yay -Ss zed",
            "yay -Qi zed",
        ] {
            assert!(!argv(q).is_mutating(), "{q}");
            assert_eq!(command_intent(q), Intent::default(), "{q}");
        }
        for m in [
            "pacman -S zed",
            "pacman -Syu",
            "pacman -Rns zed",
            "pacman -Rc zed",
            "pacman -U foo-1-1-x86_64.pkg.tar.zst",
            "yay",
            "yay zed",
        ] {
            assert!(argv(m).is_mutating(), "{m}");
        }
    }

    #[test]
    fn hook_intents() {
        let i = command_intent("omarchy update");
        assert!(i.full_upgrade);
        assert_eq!(i.packages, ["archlinux-keyring", "omarchy-keyring"]);
        let i = command_intent("sudo pacman -S --noconfirm ollama");
        assert_eq!(i.packages, ["ollama"]);
        assert!(!i.full_upgrade);
        let i = command_intent("OMARCHY_ALLOW_DIRECT_PACMAN=1 sudo -E pacman -Syu && yay -S zed");
        assert!(i.full_upgrade);
        assert_eq!(i.packages, ["zed"]);
        assert_eq!(
            command_intent("omarchy pkg add zed btop").packages,
            ["zed", "btop"]
        );
        assert_eq!(
            command_intent("omarchy pkg aur add foo-bin").packages,
            ["foo-bin"]
        );
        assert_eq!(command_intent("pacman -Qi zed"), Intent::default());
        assert_eq!(
            command_intent("sed -i 's/a/b/' ~/.config/hypr/bindings.conf"),
            Intent::default()
        );
    }

    fn lines(text: &str) -> Vec<u8> {
        text.as_bytes().to_vec()
    }

    #[test]
    fn transaction_buffering() {
        let log = lines(
            "[2026-10-01T10:00:00+0200] [PACMAN] Running 'pacman -S zed'\n\
             [2026-10-01T10:00:01+0200] [ALPM] transaction started\n\
             [2026-10-01T10:00:02+0200] [ALPM] installed alsa-lib (1-1)\n\
             [2026-10-01T10:00:03+0200] [ALPM] installed zed (2-1)\n",
        );
        // pacman still running: nothing emitted, cursor at the Running line
        let p = parse(&log, 100, HELD, tz());
        assert!(p.txs.is_empty());
        assert_eq!(p.resume, 100);
        // only the Running line so far (downloading): cursor stays before it
        let running = lines("[2026-10-01T10:00:00+0200] [PACMAN] Running 'pacman -Syu'\n");
        let p = parse(&running, 100, HELD, tz());
        assert!(p.txs.is_empty());
        assert_eq!(p.resume, 100, "the Running line is read again");
        // ... unless pacman is not running (a no-op or failed invocation)
        assert_eq!(
            parse(&running, 100, LockState::Absent, tz()).resume,
            100 + running.len() as u64
        );
        // lock gone: emitted as is
        let p = parse(&log, 100, LockState::Absent, tz());
        assert_eq!(p.txs.len(), 1);
        assert_eq!(p.resume, 100 + log.len() as u64);
        let events = p.txs[0].events();
        assert_eq!(events[0].explicit, Some(false));
        assert_eq!(events[1].explicit, Some(true));
        assert_eq!(events[1].tx_id.as_deref(), Some("tx-20261001T100001"));
        // the next transaction start closes the open one
        let mut more = log.clone();
        more.extend(lines(
            "[2026-10-01T10:05:00+0200] [ALPM] transaction started\n\
             [2026-10-01T10:05:01+0200] [ALPM] removed x (1-1)\n\
             [2026-10-01T10:05:01+0200] [ALPM] transaction completed\n\
             [2026-10-01T10:06:00+0200] [ALPM] installed half (1-",
        ));
        let p = parse(&more, 0, HELD, tz());
        assert_eq!(p.txs.len(), 2);
        assert_eq!(
            p.txs[1].command, None,
            "the Running line belongs to the first"
        );
        assert_eq!(p.txs[1].events()[0].explicit, None);
        assert_eq!(
            p.resume as usize,
            more.len() - "[2026-10-01T10:06:00+0200] [ALPM] installed half (1-".len()
        );
        // ADR-0043: the first never logged its end, the second completed
        assert_eq!(p.txs[0].status, Some(TxStatus::Unfinished));
        assert_eq!(p.txs[1].status, None);
    }

    /// ADR-0043: how a transaction ended reaches each of its events as
    /// `meta.txStatus`; a completed one, a package line outside any
    /// transaction and a transaction pacman still runs carry none.
    #[test]
    fn transaction_status() {
        let block = |end: &str| {
            format!(
                "[2026-10-01T10:00:00+0200] [PACMAN] Running 'pacman -Syu'\n\
                 [2026-10-01T10:00:01+0200] [ALPM] transaction started\n\
                 [2026-10-01T10:00:02+0200] [ALPM] upgraded gtk4 (1:4.18.6-1 -> 1:4.18.7-1)\n\
                 [2026-10-01T10:00:02+0200] [ALPM] removed pulseaudio (17.0-3)\n\
                 {end}"
            )
        };
        for (end, lock, want) in [
            (
                "[2026-10-01T10:00:03+0200] [ALPM] transaction completed\n",
                false,
                None,
            ),
            (
                "[2026-10-01T10:00:03+0200] [ALPM] transaction completed\n",
                true,
                None,
            ),
            (
                "[2026-10-01T10:00:03+0200] [ALPM] transaction failed\n",
                false,
                Some(TxStatus::Failed),
            ),
            (
                "[2026-10-01T10:00:03+0200] [ALPM] transaction interrupted\n",
                true,
                Some(TxStatus::Interrupted),
            ),
            // the log ends and pacman is gone: killed, a crash, power loss
            ("", false, Some(TxStatus::Unfinished)),
            // a later transaction began without this one's end line
            (
                "[2026-10-01T10:05:00+0200] [ALPM] transaction started\n\
                 [2026-10-01T10:05:01+0200] [ALPM] installed zed (2-1)\n\
                 [2026-10-01T10:05:01+0200] [ALPM] transaction completed\n",
                false,
                Some(TxStatus::Unfinished),
            ),
        ] {
            let p = parse(
                &lines(&block(end)),
                0,
                if lock { HELD } else { LockState::Absent },
                tz(),
            );
            assert_eq!(p.txs[0].status, want, "{end:?} lock {lock}");
            let events = p.txs[0].events();
            assert_eq!(events.len(), 2);
            assert!(events.iter().all(|e| e.meta.tx_status == want), "{end:?}");
            if let Some(next) = p.txs.get(1) {
                assert_eq!(next.status, None, "the later one completed");
                assert_eq!(next.events()[0].meta.tx_status, None);
            }
        }
        // still running: held back, so no status is guessed
        let p = parse(&lines(&block("")), 0, HELD, tz());
        assert!(p.txs.is_empty());
        // a package line outside any transaction (old logs) has no status
        let p = parse(
            &lines("[2019-01-01 12:00] [ALPM] installed btop (1.0-1)\n"),
            0,
            LockState::Absent,
            tz(),
        );
        assert_eq!(p.txs[0].tx_id, None);
        assert_eq!(p.txs[0].status, None);
        assert_eq!(p.txs[0].events()[0].meta.tx_status, None);
        // an end line without a start closes nothing
        let p = parse(
            &lines(
                "[2026-10-01T10:00:03+0200] [ALPM] transaction interrupted\n\
                 [2026-10-01T10:00:04+0200] [ALPM] transaction started\n\
                 [2026-10-01T10:00:05+0200] [ALPM] installed zed (2-1)\n\
                 [2026-10-01T10:00:06+0200] [ALPM] transaction completed\n",
            ),
            0,
            LockState::Absent,
            tz(),
        );
        assert_eq!(p.txs.len(), 1);
        assert_eq!(p.txs[0].status, None);
    }

    /// A file in a fresh folder of the temp dir, removed on drop.
    struct TempLog(PathBuf);
    impl TempLog {
        fn new(tag: &str, bytes: &[u8]) -> Self {
            static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
            let dir = std::env::temp_dir().join(format!(
                "seldon-pacman-{tag}-{}-{}",
                std::process::id(),
                N.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
            ));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join("pacman.log"), bytes).unwrap();
            TempLog(dir)
        }
        fn open(&self) -> Lines {
            Lines::open(&self.0.join("pacman.log")).unwrap()
        }
    }
    impl Drop for TempLog {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// Every line `Lines` gives, with its offset, and where it stops.
    fn read_all(lines: &mut Lines) -> (Vec<(u64, Vec<u8>)>, u64) {
        let mut got = Vec::new();
        while let Some((at, line)) = lines.next_line().unwrap() {
            got.push((at, line.to_vec()));
        }
        (got, lines.pos())
    }

    /// WP-198: the lines and offsets `split` gives, across the read
    /// buffer's edges; an over-long line is empty, but passed; the
    /// unterminated last line is not passed.
    #[test]
    fn lines_through_the_buffer() {
        let mut log = Vec::new();
        let mut want = Vec::new();
        let mut push = |log: &mut Vec<u8>, line: Vec<u8>, seen: Vec<u8>| {
            want.push((log.len() as u64, seen));
            log.extend(&line);
            log.push(b'\n');
        };
        // lines that end just before, on and after the buffer's edges
        for n in [
            0,
            1,
            READ_BUFFER - 2,
            READ_BUFFER - 1,
            READ_BUFFER,
            3 * READ_BUFFER + 7,
        ] {
            let line = vec![b'a' + (n % 26) as u8; n];
            push(&mut log, line.clone(), line);
        }
        push(&mut log, b"crlf\r".to_vec(), b"crlf\r".to_vec());
        let max = vec![b'm'; MAX_LINE];
        push(&mut log, max.clone(), max);
        push(&mut log, vec![b'x'; MAX_LINE + 1], Vec::new());
        push(&mut log, vec![b'y'; 3 * MAX_LINE], Vec::new());
        push(&mut log, b"after".to_vec(), b"after".to_vec());
        let complete = log.len() as u64;
        log.extend(b"[2026-10-01T10:06:00+0200] [ALPM] installed half (1-");

        let t = TempLog::new("lines", &log);
        let mut lines = t.open();
        let (got, end) = read_all(&mut lines);
        assert_eq!(got.len(), want.len());
        for (g, w) in got.iter().zip(&want) {
            assert_eq!((g.0, g.1.len()), (w.0, w.1.len()));
            assert!(g.1 == w.1, "line at {}", w.0);
        }
        assert_eq!(end, complete, "the unterminated line is not passed");
        assert_eq!(lines.next_line().unwrap(), None);

        // from an offset, as a cursor reads
        let at = want[7].0;
        lines.seek(at).unwrap();
        let (got, end) = read_all(&mut lines);
        assert_eq!(got.first().map(|g| g.0), Some(at));
        assert_eq!((got.len(), end), (want.len() - 7, complete));

        // `parse` passes an over-long line the same way
        let long = format!(
            "[2026-10-01T10:00:00+0200] [PACMAN] Running 'pacman -S {}'\n",
            "a".repeat(MAX_LINE)
        );
        assert!(
            parse(long.as_bytes(), 0, LockState::Absent, tz())
                .txs
                .is_empty()
        );
        let p = parse(long.as_bytes(), 0, HELD, tz());
        assert_eq!(p.resume, long.len() as u64, "no Running line to hold");
    }

    /// WP-198: `Lines` opens a regular file only; a missing one names the
    /// path, as before.
    #[test]
    fn lines_open_regular_files_only() {
        let t = TempLog::new("regular", b"");
        let err = Lines::open(&t.0).err().unwrap().to_string();
        assert!(err.contains("a directory, not a regular file"), "{err}");
        let missing = t.0.join("nope.log");
        let err = Lines::open(&missing).err().unwrap().to_string();
        assert!(
            err.starts_with(&format!("cannot read {}: ", missing.display())),
            "{err}"
        );
    }

    /// The events at or after `baseline`, and `resume`, of `parsed`.
    fn kept(parsed: Parsed, baseline: DateTime<FixedOffset>) -> (Vec<Event>, u64) {
        let mut events: Vec<Event> = parsed.txs.iter().flat_map(Tx::events).collect();
        events.retain(|e| e.ts >= baseline);
        for e in &mut events {
            e.id = ulid::Ulid::nil();
        }
        (events, parsed.resume)
    }

    /// WP-198: for every baseline around every line of logs of every shape
    /// (a transaction open across the baseline, a Running line without a
    /// transaction, one without an end line, a newer start without an end,
    /// scriptlet lines, package lines outside any transaction, the old
    /// time format, a clock set back, a line without a time, CRLF), a parse
    /// from [`look_back_start`] keeps exactly the events and the resume
    /// offset of a parse of the whole log, under every lock state.
    #[test]
    fn the_look_back_start_keeps_what_the_whole_log_keeps() {
        let log = "\
garbage before anything
[2026-01-01T10:00:00+0200] [ALPM] installed lone (1-1)
[2026-01-01T10:00:00+0200] [PACMAN] Running 'pacman -S nothing'
[2026-01-02T10:00:00+0200] [PACMAN] Running 'pacman -S zed'
[2026-01-02T10:00:01+0200] [ALPM] transaction started
[2026-01-02T10:00:02+0200] [ALPM] installed alsa-lib (1-1)
[2026-01-02T10:00:03+0200] [ALPM-SCRIPTLET] ==> hello
[2026-01-03T10:00:03+0200] [ALPM] installed zed (2-1)
[2026-01-03T10:00:04+0200] [ALPM] transaction completed
[2026-01-04T10:00:00+0200] [PACMAN] Running 'pacman -Syu'
[2026-01-04T10:00:01+0200] [ALPM] transaction started
[2026-01-04T10:00:02+0200] [ALPM] upgraded gtk4 (1-1 -> 2-1)
[2026-01-05T10:00:00+0200] [ALPM] transaction started
[2026-01-05T10:00:01+0200] [ALPM] removed x (1-1)\r
[2026-01-05T10:00:02+0200] [ALPM] warning: /etc/a.conf installed as /etc/a.conf.pacnew
[2026-01-05T10:00:03+0200] [ALPM] transaction failed
[2026-01-05T10:00:04+0200] [PACMAN] Running 'pacman -S btop'
[2026-01-06T10:00:00+0200] [ALPM] running '60-mkinitcpio-remove.hook'...
no time here
[2019-01-01 12:00] [ALPM] installed old (1-1)
[2026-01-07T10:00:00+0200] [ALPM] transaction started
[2026-01-07T10:00:01+0200] [ALPM] installed btop (1-1)
[2026-01-06T09:00:00+0200] [ALPM] installed clockback (1-1)
[2026-01-08T10:00:00+0200] [ALPM] transaction interrupted
[2026-01-08T10:00:01+0200] [ALPM] transaction completed
[2026-01-09T10:00:00+0200] [ALPM] reinstalled lone (1-1)
[2026-01-10T10:00:00+0200] [PACMAN] Running 'pacman -S tail'
[2026-01-10T10:00:01+0200] [ALPM] transaction started
[2026-01-10T10:00:02+0200] [ALPM] installed tail (1-1)
[2026-01-10T10:00:03+0200] [ALPM-SCRIPTLET] ==> still writing
";
        // the log, the log without its open end, and with a Running line at
        // the end (pacman downloading)
        let open_end = log.find("[2026-01-10T10:00:00").unwrap();
        let logs = [
            log.to_string(),
            log[..open_end].to_string(),
            format!(
                "{}[2026-01-11T10:00:00+0200] [PACMAN] Running 'pacman -S dl'\n",
                &log[..open_end]
            ),
        ];
        // a boot between the two last transactions, and one before the log
        let boot = |s: &str| SystemTime::from(DateTime::parse_from_rfc3339(s).unwrap());
        let locks = [
            LockState::Absent,
            HELD,
            LockState::Stale {
                modified: boot("2025-12-01T00:00:00+02:00"),
                boot: boot("2026-01-09T12:00:00+02:00"),
            },
            LockState::Stale {
                modified: boot("2025-12-01T00:00:00+02:00"),
                boot: boot("2026-01-30T00:00:00+02:00"),
            },
        ];
        for text in &logs {
            let t = TempLog::new("look-back", text.as_bytes());
            // a baseline before, at and after the time of every line
            let mut baselines: Vec<DateTime<FixedOffset>> = text
                .lines()
                .filter_map(|l| line_ts(l.as_bytes(), tz()))
                .flat_map(|ts| {
                    [
                        ts - chrono::Duration::seconds(1),
                        ts,
                        ts + chrono::Duration::seconds(1),
                    ]
                })
                .collect();
            baselines.push(DateTime::parse_from_rfc3339("2030-01-01T00:00:00+00:00").unwrap());
            let mut starts = HashSet::new();
            for baseline in baselines {
                let mut lines = t.open();
                let from = look_back_start(&mut lines, baseline, tz()).unwrap();
                assert!(from as usize <= text.len());
                assert!(from == 0 || text.as_bytes()[from as usize - 1] == b'\n');
                starts.insert(from);
                for lock in locks {
                    let whole = kept(parse(text.as_bytes(), 0, lock, tz()), baseline);
                    lines.seek(from).unwrap();
                    let part = kept(read(&mut lines, lock, tz()).unwrap(), baseline);
                    assert_eq!(part, whole, "baseline {baseline}, start {from}, {lock:?}");
                }
            }
            assert!(
                starts.len() > 5,
                "the start moves with the baseline: {starts:?}"
            );
        }
    }

    /// WP-198: the start rewinds to the block of the transaction open at
    /// the baseline, to a Running line no transaction took, and skips a
    /// whole log older than the baseline, except a block still open.
    #[test]
    fn the_look_back_start_rewinds_to_the_open_block() {
        let log = "\
[2026-01-01T10:00:00+0200] [PACMAN] Running 'pacman -S a'
[2026-01-01T10:00:01+0200] [ALPM] transaction started
[2026-01-01T10:00:02+0200] [ALPM] installed a (1-1)
[2026-01-01T10:00:03+0200] [ALPM] transaction completed
[2026-02-01T10:00:00+0200] [PACMAN] Running 'pacman -S b'
[2026-02-01T10:00:01+0200] [ALPM] transaction started
[2026-02-01T10:00:02+0200] [ALPM] installed b (1-1)
[2026-03-01T10:00:00+0200] [ALPM] installed c (1-1)
[2026-03-01T10:00:01+0200] [ALPM] transaction completed
[2026-04-01T10:00:00+0200] [PACMAN] Running 'pacman -S d'
[2026-05-01T10:00:00+0200] [ALPM] transaction started
[2026-05-01T10:00:01+0200] [ALPM] installed d (1-1)
[2026-05-01T10:00:02+0200] [ALPM] transaction completed
[2026-06-01T10:00:00+0200] [ALPM] transaction started
[2026-06-01T10:00:01+0200] [ALPM] installed e (1-1)
";
        let t = TempLog::new("rewind", log.as_bytes());
        let at = |needle: &str| log.find(needle).unwrap() as u64;
        let start = |baseline: &str| {
            let b = DateTime::parse_from_rfc3339(baseline).unwrap();
            look_back_start(&mut t.open(), b, tz()).unwrap()
        };
        assert_eq!(start("2025-01-01T00:00:00+00:00"), 0);
        assert_eq!(
            start("2026-01-15T00:00:00+02:00"),
            at("[2026-02-01T10:00:00"),
            "the first line at or after the baseline"
        );
        assert_eq!(
            start("2026-02-15T00:00:00+02:00"),
            at("[2026-02-01T10:00:00"),
            "b's transaction is open at c: its Running line"
        );
        assert_eq!(
            start("2026-04-15T00:00:00+02:00"),
            at("[2026-04-01T10:00:00"),
            "d's Running line, taken by the start after the baseline"
        );
        assert_eq!(
            start("2026-06-01T10:00:01+02:00"),
            at("[2026-06-01T10:00:00"),
            "a transaction without a Running line: its start"
        );
        assert_eq!(
            start("2030-01-01T00:00:00+00:00"),
            at("[2026-06-01T10:00:00"),
            "all older: the open block at the end is read"
        );
        let closed = &log[..at("[2026-06-01T10:00:00") as usize];
        let t = TempLog::new("rewind-closed", closed.as_bytes());
        let b = DateTime::parse_from_rfc3339("2030-01-01T00:00:00+00:00").unwrap();
        assert_eq!(
            look_back_start(&mut t.open(), b, tz()).unwrap(),
            closed.len() as u64,
            "all older and closed: the end"
        );
    }
}
