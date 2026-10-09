//! The index: `${XDG_STATE_HOME:-~/.local/state}/seldon/index.json`, the
//! only input of the plugin (docs/CONTRACT.md, SPEC-ENGINE §6), and the
//! generated Markdown views built from the same data.
//!
//! - [`load`] reads the logbook, [`build`] derives the index (ADR-0012,
//!   ADR-0013, ADR-0015), [`write`] replaces the file atomically (temp
//!   file + rename, CONTRACT.md rule 2), [`views`] renders
//!   `ledger/YYYY-MM.md` and `STATUS.md`, [`check`] validates against the
//!   schema.
//! - Every command that changes the logbook calls
//!   [`rebuild_if_initialised`] once, after its own writes.

pub mod autocommit;
pub mod build;
pub mod check;
pub mod class;
pub mod drift;
pub mod load;
pub mod model;
pub mod triage;
pub mod views;

use std::path::Path;
use std::time::Duration;

use chrono::{DateTime, FixedOffset};

use crate::collectors::{self, Cursors, ShownMessages};
use crate::commands::Context;
use crate::config::{Config, Dirs};
use crate::error::Result;
use crate::logbook::{Logbook, git};
use crate::sys::{self, Run};

pub use build::{Built, Input};
pub use model::Index;

/// Reads the logbook and derives the index at `ctx.now` (git info is left
/// out; see [`git_info`]).
pub fn derive(ctx: &Context, config: &Config, logbook: &Logbook) -> anyhow::Result<Built> {
    derive_at(&ctx.dirs, config, logbook, ctx.now)
}

/// [`derive`] with explicit directories and clock (tests, benches).
pub fn derive_at(
    dirs: &Dirs,
    config: &Config,
    logbook: &Logbook,
    now: DateTime<FixedOffset>,
) -> anyhow::Result<Built> {
    let mut loaded = load::load(logbook, now.date_naive())?;
    let (state, cursors_error) = collector_state(dirs, config, &logbook.root);
    loaded.warnings.extend(cursors_error);
    let input = Input {
        now,
        logbook_path: logbook.root.display().to_string(),
        language: logbook.meta.language.as_str().to_string(),
        machine: logbook.meta.machine_id.clone(),
        git: None,
        state,
        drift: config.drift.clone(),
        redactor: crate::redact::Redactor::for_config(config).ok(),
    };
    let mut built = build::build(loaded, &input);
    built.index.triage = triage::read(dirs, &logbook.root, &mut built.warnings);
    built.index.system.recent_config = crate::collectors::recent::shown(
        dirs,
        config,
        input.redactor.as_ref(),
        now,
        &mut built.warnings,
    );
    Ok(built)
}

/// The index as written: compact JSON plus a newline.
pub fn to_text(index: &Index) -> String {
    let mut text = serde_json::to_string(index).expect("the index always serialises");
    text.push('\n');
    text
}

/// Replaces `path` with the index atomically: a reader sees the old file
/// or the new one, never a partial one (CONTRACT.md rule 2).
pub fn write(path: &Path, index: &Index) -> anyhow::Result<()> {
    sys::write_atomic(path, to_text(index).as_bytes())
}

/// `state` from `cursors.json`: one row per collector in the schema's
/// order, enabled per `config.toml`, with the last run's `ok`, `message`
/// and `lastRun` when the cursors belong to this logbook. `lastCapture`
/// is the latest `lastRun`. (`fix` has no field in the index; `doctor`
/// and `capture --json` carry it.)
///
/// A `cursors.json` that cannot be read makes every capture fail (F-133):
/// every enabled collector is then `ok: false` with that message, and the
/// second value is the load warning.
///
/// A message goes through the logbook's redaction (SPEC-ENGINE §7) once
/// more ([`ShownMessages`]): a capture saves it redacted since WP-105, an
/// older engine did not. The index is rebuilt from `cursors.json` each
/// time, so the extra pass does not accumulate. An invalid `[redaction]
/// patterns` entry withholds every message ([`collectors::MESSAGE_WITHHELD`]).
pub fn collector_state(
    dirs: &Dirs,
    config: &Config,
    root: &Path,
) -> (model::State, Option<String>) {
    let file = collectors::cursors_file(dirs);
    let (cursors, cause) = match Cursors::load(&file) {
        Ok(c) => (c, None),
        Err(e) => (Cursors::default(), Some(e.root_cause().to_string())),
    };
    let broken = cause.as_ref().map(|c| {
        format!("cursors.json is corrupt or unreadable, so every capture fails ({c}); `seldon doctor` has the fix")
    });
    let canonical = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    let mine = cursors.logbook.as_deref() == Some(canonical.as_path());
    let mut last_capture: Option<DateTime<FixedOffset>> = None;
    let shown = ShownMessages::new(Some(config));
    let rows = ["pacman", "snapper", "omarchy", "plugins", "theme", "config"]
        .into_iter()
        .map(|name| {
            let state = mine.then(|| cursors.collectors.get(name)).flatten();
            let last_run = state.and_then(|s| s.last_run.clone());
            if let Some(t) = last_run
                .as_deref()
                .and_then(|t| DateTime::parse_from_rfc3339(t).ok())
                && last_capture.is_none_or(|l| t > l)
            {
                last_capture = Some(t);
            }
            let enabled = config.collectors.get(name).unwrap_or(true);
            if enabled && let Some(message) = &broken {
                return model::CollectorRow {
                    name,
                    enabled,
                    ok: false,
                    message: Some(message.clone()),
                    last_run: None,
                };
            }
            model::CollectorRow {
                name,
                enabled,
                ok: state.is_none_or(|s| s.ok),
                message: state
                    .and_then(|s| s.message.as_deref())
                    .map(|m| shown.show(m)),
                last_run,
            }
        })
        .collect();
    let state = model::State {
        status: model::Status::Ok,
        last_capture: last_capture.map(|t| crate::model::event::format_ts(&t)),
        collectors: rows,
    };
    let warning = cause.map(|c| {
        format!(
            "{}: corrupt or unreadable ({c}); every enabled collector is shown as failing",
            dirs.display(&file)
        )
    });
    (state, warning)
}

/// `logbook.git`: the short HEAD and whether the work tree has changes;
/// `None` when the logbook is not a repository or git is missing. git runs
/// with the logbook's own environment (`git::query`: no inherited
/// `GIT_DIR`, no walking up into a repository around the logbook, no
/// network).
pub fn git_info(root: &Path) -> Option<model::GitInfo> {
    if !git::is_repo(root) {
        return None;
    }
    let timeout = Duration::from_secs(10);
    let head = match git::query(root, &["rev-parse", "--short", "HEAD"], timeout) {
        Run::Exited {
            code: Some(0),
            stdout,
            ..
        } => Some(stdout.trim().to_string()).filter(|h| !h.is_empty()),
        Run::Exited { .. } => None,
        _ => return None,
    };
    let dirty =
        !git::status_is_empty(git::query(root, &["status", "--porcelain"], timeout)).ok()?;
    Some(model::GitInfo {
        head,
        dirty: Some(dirty),
        autocommit: None,
    })
}

/// `logbook.git` without running git: the 7-character HEAD from
/// `.git/HEAD`, a loose ref or `packed-refs` (`head` absent on an unborn
/// branch), `dirty` unknown and left out. `None` when the logbook is not a
/// repository. Follows a `.git` file and `commondir`, each read as git
/// reads it (WP-154): the file starts with exactly `gitdir: ` (no byte
/// order mark, one space, lower case) and only the CRs and LFs at the end
/// of a path are dropped (white space is part of it); a relative path is
/// relative to the file's directory. A `commondir` that is there but
/// cannot be read, and an empty path in either file, is no repository
/// either (git stops there).
pub fn git_head_fast(root: &Path) -> Option<model::GitInfo> {
    let read = |path: &Path| sys::read_regular_string(path, sys::LOGBOOK_FILE_MAX);
    let dot = root.join(".git");
    let gitdir = if dot.is_file() {
        let text = read(&dot).ok()?;
        root.join(git_path(text.strip_prefix("gitdir: ")?)?)
    } else if dot.is_dir() {
        dot
    } else {
        return None;
    };
    let common = match read(&gitdir.join("commondir")) {
        Ok(c) => gitdir.join(git_path(&c)?),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => gitdir.clone(),
        Err(_) => return None,
    };
    let head = read(&gitdir.join("HEAD")).ok()?;
    let head = head.trim();
    let sha = match head.strip_prefix("ref:").map(str::trim) {
        None => Some(head.to_string()),
        Some(name) => [&gitdir, &common]
            .iter()
            .find_map(|d| read(&d.join(name)).ok())
            .map(|s| s.trim().to_string())
            .or_else(|| {
                let packed = read(&common.join("packed-refs")).ok()?;
                packed.lines().find_map(|l| {
                    let (sha, r) = l.split_once(' ')?;
                    (r.trim() == name).then(|| sha.to_string())
                })
            }),
    };
    let head = sha
        .filter(|s| s.len() >= 7 && s.bytes().all(|b| b.is_ascii_hexdigit()))
        .map(|s| s[..7].to_string());
    Some(model::GitInfo {
        head,
        dirty: None,
        autocommit: None,
    })
}

/// A path in a git file (`.git`, `commondir`): the text without the CRs
/// and LFs at its end, as git reads it; `None` when nothing is left (git
/// 2.55: an empty `commondir` "failed to read", one of only a newline
/// "not a git repository").
fn git_path(text: &str) -> Option<&Path> {
    let path = text.trim_end_matches(['\n', '\r']);
    (!path.is_empty()).then(|| Path::new(path))
}

/// The index of a logbook that does not exist yet: `state.status
/// notInitialised`, every section empty (fixtures/index-variants/
/// not-initialised.json), so the plugin can show its banner.
pub fn not_initialised(path: &Path, now: DateTime<FixedOffset>) -> Index {
    Index {
        contract_version: crate::CONTRACT_VERSION,
        generated_at: crate::model::event::format_ts(&now),
        engine_version: crate::VERSION.to_string(),
        logbook: model::LogbookInfo {
            path: path.display().to_string(),
            language: crate::model::Language::default().as_str().to_string(),
            machine: String::new(),
            git: None,
        },
        state: model::State {
            status: model::Status::NotInitialised,
            last_capture: None,
            collectors: Vec::new(),
        },
        summary: model::Summary::default(),
        today: model::Today {
            date: now.date_naive().to_string(),
            path: None,
            entries: Vec::new(),
            yesterday: None,
        },
        events: Vec::new(),
        drift: Vec::new(),
        cases: model::Cases::default(),
        decisions: Vec::new(),
        system: model::System::default(),
        memory: model::MemoryInfo::default(),
        series: model::Series::default(),
        triage: None,
    }
}

/// Rebuilds `index.json` after a command changed the logbook (CONTRACT.md
/// rule 2). Does nothing when the logbook is not initialised. A failure
/// does not fail the command (its write already happened): it is reported
/// on stderr, like every load warning, and the plugin shows the index as
/// stale until the next `seldon status`.
pub fn rebuild_if_initialised(ctx: &Context) {
    rebuild_reporting(ctx, GitProbe::Full);
}

/// [`rebuild_if_initialised`] for latency-bound callers (the hook path,
/// WP-009): spawns no `git`. `logbook.git.head` is read from the `.git`
/// files ([`git_head_fast`]) and `dirty` is left out (the schema allows
/// it); the next full rebuild fills it in again.
///
/// The rebuild reads the whole logbook, so its cost grows with the ledger
/// (WP-057). Above [`FAST_REBUILD_MAX_LINES`] ledger lines it does nothing
/// and returns `false`: the next `capture` or `status` (the plugin runs one
/// at least every 15 minutes) brings the index up to date. Counting stops
/// at the threshold, so a large ledger costs one bounded read.
pub fn rebuild_if_initialised_fast(ctx: &Context) -> bool {
    let config = ctx.load_config().ok().flatten().unwrap_or_default();
    let (root, _) = ctx.resolve_logbook(None, Some(&config));
    if ledger_lines_exceed(&root.join("ledger"), FAST_REBUILD_MAX_LINES) {
        return false;
    }
    rebuild_reporting(ctx, GitProbe::HeadOnly);
    true
}

/// Ledger lines up to which [`rebuild_if_initialised_fast`] rebuilds
/// (SPEC-ENGINE §8; release, dev host: the rebuild adds about 1.6 ms to a
/// 1.2 ms hook call at 900 lines, WP-092).
pub const FAST_REBUILD_MAX_LINES: usize = 1000;

/// Whether the `*.jsonl` files in `dir` hold more than `max` lines. Reads
/// at most until the count passes `max`; an unreadable directory or file
/// counts as nothing (the rebuild reports it). A month over
/// [`sys::LEDGER_MONTH_MAX`] (or of a size that cannot be read) counts as
/// too many without a read (WP-175): one without a newline (a sparse
/// file) would be read to its end, and the full rebuild refuses it
/// anyway, saying why.
fn ledger_lines_exceed(dir: &Path, max: usize) -> bool {
    use std::io::Read as _;
    let Ok(entries) = std::fs::read_dir(dir) else {
        return false;
    };
    let mut lines = 0;
    let mut buf = vec![0u8; 64 * 1024];
    for path in entries.filter_map(|e| e.ok()).map(|e| e.path()) {
        if path.extension().is_none_or(|e| e != "jsonl") {
            continue;
        }
        // a FIFO is not waited on, a device not read (WP-174)
        let Ok(mut file) = sys::open_regular(&path) else {
            continue;
        };
        if !file
            .metadata()
            .is_ok_and(|m| m.len() <= sys::LEDGER_MONTH_MAX)
        {
            return true;
        }
        while let Ok(n) = file.read(&mut buf) {
            if n == 0 {
                break;
            }
            lines += buf[..n].iter().filter(|&&b| b == b'\n').count();
            if lines > max {
                return true;
            }
        }
    }
    false
}

/// How `logbook.git` is filled in by a rebuild.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GitProbe {
    /// `git rev-parse` + `git status` ([`git_info`]).
    Full,
    /// `.git/HEAD` only ([`git_head_fast`]).
    HeadOnly,
}

fn rebuild_reporting(ctx: &Context, probe: GitProbe) {
    match try_rebuild(ctx, probe) {
        Ok(warnings) => {
            for w in warnings {
                eprintln!("seldon: warning: {w}");
            }
        }
        Err(e) => eprintln!("seldon: warning: index.json not rebuilt: {e}"),
    }
}

/// Rebuilds and writes the index; returns the load warnings.
fn try_rebuild(ctx: &Context, probe: GitProbe) -> Result<Vec<String>> {
    let config = ctx.load_config()?.unwrap_or_default();
    let (root, _) = ctx.resolve_logbook(None, Some(&config));
    if !Logbook::is_initialised(&root) {
        return Ok(Vec::new());
    }
    let logbook = Logbook::open(&root)?;
    let mut built = derive(ctx, &config, &logbook)?;
    built.index.logbook.git = match probe {
        GitProbe::Full => git_info(&logbook.root),
        GitProbe::HeadOnly => git_head_fast(&logbook.root),
    };
    built.warnings.extend(autocommit::attach(
        &mut built.index.logbook.git,
        &ctx.dirs,
        &config,
        &logbook.root,
    ));
    write(&ctx.dirs.index_file(), &built.index)?;
    Ok(built.warnings)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A sparse month far over the read cap and without a newline is not
    /// read by the fast rebuild's count (WP-175): it counts as too many
    /// at once. Without the bound the count reads 1 TiB of zeros; in a
    /// thread with a time limit, so that fails instead of hanging. Months
    /// under the cap are counted as before.
    #[test]
    fn a_month_over_the_cap_is_not_counted() {
        let tmp = crate::logbook::scratch::scratch("seldon-ledger-count");
        let dir = tmp.join("ledger");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("2026-09.jsonl"), "{}\n{}\n").unwrap();
        assert!(!ledger_lines_exceed(&dir, 2));
        assert!(ledger_lines_exceed(&dir, 1));
        let month = std::fs::File::create(dir.join("2026-10.jsonl")).unwrap();
        month.set_len(sys::LEDGER_MONTH_MAX).unwrap();
        // at the cap: read and counted (no newline in it)
        assert!(!ledger_lines_exceed(&dir, 2));
        month.set_len(1 << 40).unwrap();
        let (done, finished) = std::sync::mpsc::channel();
        let count = std::thread::spawn(move || done.send(ledger_lines_exceed(&dir, 2)).unwrap());
        match finished.recv_timeout(std::time::Duration::from_secs(10)) {
            Ok(exceeds) => assert!(exceeds),
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => count.join().unwrap(),
            Err(e) => panic!("the count read the sparse month: {e}"),
        }
    }
}
