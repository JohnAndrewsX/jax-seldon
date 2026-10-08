//! `pacman` collector (SPEC-ENGINE §4, ADR-0013 §5, ADR-0014 §1).
//!
//! Reads `/var/log/pacman.log` from the saved byte offset and turns the
//! `[ALPM] installed|removed|upgraded|downgraded|reinstalled` lines into
//! events.
//!
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
//!   the end of the log when pacman's `db.lck` is absent. While pacman still
//!   runs, the cursor stays at the start of the transaction's block, so
//!   the next capture reads it whole (ADR-0013 §5).
//! - **Status** (ADR-0043). A transaction that ended with `transaction
//!   failed` or `transaction interrupted` writes that word as
//!   `meta.txStatus` on each of its events; one closed by the next
//!   `transaction started` or by the end of the log without `db.lck` writes
//!   `unfinished`. A completed one writes none.
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
use std::io::{Read as _, Seek as _, SeekFrom};
use std::os::unix::fs::MetadataExt as _;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

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
    let meta = std::fs::metadata(path)
        .map_err(|e| anyhow::anyhow!("cannot read {}: {e}", path.display()))?;
    let lock_present = ctx.sources.pacman_db_lock.exists();
    let mut txs = Vec::new();

    let start = match cursor {
        Some(c) if c.inode == meta.ino() && c.offset <= meta.len() => c.offset,
        Some(c) if c.inode != meta.ino() => {
            // rotated: finish the old file if it is still there
            let old = rotated(path);
            if std::fs::metadata(&old).is_ok_and(|m| m.ino() == c.inode) {
                let bytes = read_from(&old, c.offset)?;
                // the old file is closed for good: emit what it has. A
                // transaction still open there is `unfinished` (ADR-0043):
                // holding it back would lose it, since the cursor moves to
                // the new file; a pacman still running at the rotation
                // writes its end into the old file, which is not read again
                txs.extend(parse(&bytes, c.offset, false, ctx.tz).txs);
            }
            0
        }
        _ => 0, // no cursor (baseline), or the file shrank (truncated)
    };
    let bytes = read_from(path, start)?;
    let parsed = parse(&bytes, start, lock_present, ctx.tz);
    txs.extend(parsed.txs);

    let began: HashMap<String, DateTime<FixedOffset>> = txs
        .iter()
        .filter_map(|t| Some((t.tx_id.clone()?, t.began)))
        .collect();
    let mut events: Vec<Event> = txs.iter().flat_map(Tx::events).collect();
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

/// `pacman.log` → `pacman.log.1`.
fn rotated(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".1");
    path.with_file_name(name)
}

fn read_from(path: &Path, offset: u64) -> anyhow::Result<Vec<u8>> {
    let mut f =
        File::open(path).map_err(|e| anyhow::anyhow!("cannot read {}: {e}", path.display()))?;
    f.seek(SeekFrom::Start(offset))?;
    let mut buf = Vec::new();
    f.read_to_end(&mut buf)?;
    Ok(buf)
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
    /// How it ended when it did not complete (ADR-0043): pacman's `failed`
    /// or `interrupted`, or `unfinished` without an end line. `None` when
    /// it completed, and outside any transaction.
    pub status: Option<TxStatus>,
}

impl Tx {
    /// The events of this transaction, unattributed (`actor: system`).
    pub fn events(&self) -> Vec<Event> {
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
/// `lock_present`: pacman's `db.lck` exists, so an open transaction at the
/// end is still running and is held back.
pub fn parse(bytes: &[u8], base: u64, lock_present: bool, tz: Tz) -> Parsed {
    struct Open {
        tx: Tx,
        rewind: u64,
    }
    let mut txs = Vec::new();
    let mut open: Option<Open> = None;
    // latest Running line: (offset, ts, command)
    let mut command: Option<(u64, DateTime<FixedOffset>, String)> = None;
    let mut pos = 0usize;
    while let Some(nl) = bytes[pos..].iter().position(|&b| b == b'\n') {
        let at = base + pos as u64;
        let text = String::from_utf8_lossy(&bytes[pos..pos + nl]);
        pos += nl + 1;
        let Some((ts, line)) = parse_line(&text, tz) else {
            continue;
        };
        match line {
            Line::Command(c) => command = Some((at, ts, c)),
            Line::TxStart => {
                // a transaction without an end line did not complete
                if let Some(mut o) = open.take() {
                    o.tx.status = Some(TxStatus::Unfinished);
                    txs.push(o.tx);
                }
                let (rewind, began, cmd) = match command.take() {
                    Some((off, t, c)) => (off, t, Some(c)),
                    None => (at, ts, None),
                };
                open = Some(Open {
                    tx: Tx {
                        tx_id: Some(format!("tx-{}", ts.format("%Y%m%dT%H%M%S"))),
                        command: cmd,
                        began,
                        lines: Vec::new(),
                        status: None,
                    },
                    rewind,
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
                match &mut open {
                    Some(o) => o.tx.lines.push(line),
                    None => txs.push(Tx {
                        tx_id: None,
                        command: None,
                        began: ts,
                        lines: vec![line],
                        status: None,
                    }),
                }
            }
        }
    }
    let mut resume = base + pos as u64;
    match open {
        Some(o) if lock_present => resume = o.rewind,
        // pacman is gone and never ended it (killed, a crash, power loss)
        Some(mut o) => {
            o.tx.status = Some(TxStatus::Unfinished);
            txs.push(o.tx);
        }
        // a Running line whose transaction has not started yet (pacman is
        // still downloading): read it again next time, or the transaction
        // would lose its command line
        None if lock_present => {
            if let Some((off, ..)) = command {
                resume = off;
            }
        }
        None => {}
    }
    txs.retain(|t| !t.lines.is_empty());
    Parsed { txs, resume }
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

/// Sets `actor`/`case` from the hook command that caused each event
/// (ADR-0014 §1, ADR-0017 §2 §3). `began` maps a txId to the time its
/// pacman invocation began (the Running line, else `transaction started`);
/// an event outside a transaction begins at its own `ts`. A member without
/// a cause of its own inherits from an attributed member of its
/// transaction, explicit ones first.
pub fn attribute(
    ctx: &Ctx,
    mut events: Vec<Event>,
    began: &HashMap<String, DateTime<FixedOffset>>,
) -> anyhow::Result<Vec<Event>> {
    let start = |e: &Event| {
        e.tx_id
            .as_ref()
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
        if found[i].is_none() && events[i].tx_id.is_some() {
            let same_tx = |j: &usize| events[*j].tx_id == events[i].tx_id && found[*j].is_some();
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

    fn tz() -> Tz {
        Tz::Fixed(FixedOffset::east_opt(2 * 3600).unwrap())
    }

    fn argv(s: &str) -> PacmanCommand {
        parse_command(&split_logged(s)).unwrap()
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
        let p = parse(&log, 100, true, tz());
        assert!(p.txs.is_empty());
        assert_eq!(p.resume, 100);
        // only the Running line so far (downloading): cursor stays before it
        let running = lines("[2026-10-01T10:00:00+0200] [PACMAN] Running 'pacman -Syu'\n");
        let p = parse(&running, 100, true, tz());
        assert!(p.txs.is_empty());
        assert_eq!(p.resume, 100, "the Running line is read again");
        // ... unless pacman is not running (a no-op or failed invocation)
        assert_eq!(
            parse(&running, 100, false, tz()).resume,
            100 + running.len() as u64
        );
        // lock gone: emitted as is
        let p = parse(&log, 100, false, tz());
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
        let p = parse(&more, 0, true, tz());
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
            let p = parse(&lines(&block(end)), 0, lock, tz());
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
        let p = parse(&lines(&block("")), 0, true, tz());
        assert!(p.txs.is_empty());
        // a package line outside any transaction (old logs) has no status
        let p = parse(
            &lines("[2019-01-01 12:00] [ALPM] installed btop (1.0-1)\n"),
            0,
            false,
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
            false,
            tz(),
        );
        assert_eq!(p.txs.len(), 1);
        assert_eq!(p.txs[0].status, None);
    }
}
