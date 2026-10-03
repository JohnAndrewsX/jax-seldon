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

pub mod build;
pub mod check;
pub mod drift;
pub mod load;
pub mod model;
pub mod views;

use std::path::Path;
use std::time::Duration;

use chrono::{DateTime, FixedOffset};

use crate::collectors::{self, Cursors};
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
    let loaded = load::load(logbook, now.date_naive())?;
    let input = Input {
        now,
        logbook_path: logbook.root.display().to_string(),
        language: logbook.meta.language.as_str().to_string(),
        machine: logbook.meta.machine_id.clone(),
        git: None,
        state: collector_state(dirs, config, &logbook.root),
        always_red: config.drift.always_red.clone(),
    };
    Ok(build::build(loaded, &input))
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
    sys::write_generated(path, to_text(index).as_bytes())
}

/// `state` from `cursors.json`: one row per collector in the schema's
/// order, enabled per `config.toml`, with the last run's `ok`, `message`
/// and `lastRun` when the cursors belong to this logbook. `lastCapture`
/// is the latest `lastRun`. (`fix` has no field in the index; `doctor`
/// and `capture --json` carry it.)
pub fn collector_state(dirs: &Dirs, config: &Config, root: &Path) -> model::State {
    let cursors = Cursors::load(&collectors::cursors_file(dirs)).unwrap_or_default();
    let canonical = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    let mine = cursors.logbook.as_deref() == Some(canonical.as_path());
    let mut last_capture: Option<DateTime<FixedOffset>> = None;
    let rows = ["pacman", "snapper", "omarchy", "plugins", "theme", "config"]
        .into_iter()
        .map(|name| {
            let state = mine.then(|| cursors.collectors.get(name)).flatten();
            let last_run = state.map(|s| s.last_run.clone());
            if let Some(t) = last_run
                .as_deref()
                .and_then(|t| DateTime::parse_from_rfc3339(t).ok())
                && last_capture.is_none_or(|l| t > l)
            {
                last_capture = Some(t);
            }
            model::CollectorRow {
                name,
                enabled: config.collectors.get(name).unwrap_or(true),
                ok: state.is_none_or(|s| s.ok),
                message: state.and_then(|s| s.message.clone()),
                last_run,
            }
        })
        .collect();
    model::State {
        status: model::Status::Ok,
        last_capture: last_capture.map(|t| crate::model::event::format_ts(&t)),
        collectors: rows,
    }
}

/// `logbook.git`: the short HEAD and whether the work tree has changes;
/// `None` when the logbook is not a repository or git is missing.
pub fn git_info(root: &Path) -> Option<model::GitInfo> {
    if !git::is_repo(root) {
        return None;
    }
    let timeout = Duration::from_secs(10);
    let head = match sys::run(
        "git",
        &["rev-parse", "--short", "HEAD"],
        Some(root),
        timeout,
    ) {
        Run::Exited {
            code: Some(0),
            stdout,
            ..
        } => Some(stdout.trim().to_string()).filter(|h| !h.is_empty()),
        Run::Exited { .. } => None,
        _ => return None,
    };
    let dirty = match sys::run("git", &["status", "--porcelain"], Some(root), timeout) {
        Run::Exited {
            code: Some(0),
            stdout,
            ..
        } => !stdout.trim().is_empty(),
        _ => return None,
    };
    Some(model::GitInfo {
        head,
        dirty: Some(dirty),
    })
}

/// `logbook.git` without running git: the 7-character HEAD from
/// `.git/HEAD`, a loose ref or `packed-refs` (`head` absent on an unborn
/// branch), `dirty` unknown and left out. `None` when the logbook is not a
/// repository. Follows a `.git` file (`gitdir:`) and `commondir`.
pub fn git_head_fast(root: &Path) -> Option<model::GitInfo> {
    let dot = root.join(".git");
    let gitdir = if dot.is_file() {
        let text = std::fs::read_to_string(&dot).ok()?;
        let dir = Path::new(text.trim().strip_prefix("gitdir:")?.trim());
        root.join(dir)
    } else if dot.is_dir() {
        dot
    } else {
        return None;
    };
    let common = std::fs::read_to_string(gitdir.join("commondir"))
        .map(|c| gitdir.join(c.trim()))
        .unwrap_or_else(|_| gitdir.clone());
    let head = std::fs::read_to_string(gitdir.join("HEAD")).ok()?;
    let head = head.trim();
    let sha = match head.strip_prefix("ref:").map(str::trim) {
        None => Some(head.to_string()),
        Some(name) => [&gitdir, &common]
            .iter()
            .find_map(|d| std::fs::read_to_string(d.join(name)).ok())
            .map(|s| s.trim().to_string())
            .or_else(|| {
                let packed = std::fs::read_to_string(common.join("packed-refs")).ok()?;
                packed.lines().find_map(|l| {
                    let (sha, r) = l.split_once(' ')?;
                    (r.trim() == name).then(|| sha.to_string())
                })
            }),
    };
    let head = sha
        .filter(|s| s.len() >= 7 && s.bytes().all(|b| b.is_ascii_hexdigit()))
        .map(|s| s[..7].to_string());
    Some(model::GitInfo { head, dirty: None })
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
/// (SPEC-ENGINE §8; release, dev host: the rebuild adds about 3 ms to a
/// 2 ms hook call at 1000 lines).
pub const FAST_REBUILD_MAX_LINES: usize = 1000;

/// Whether the `*.jsonl` files in `dir` hold more than `max` lines. Reads
/// at most until the count passes `max`; an unreadable directory or file
/// counts as nothing (the rebuild reports it).
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
        let Ok(mut file) = std::fs::File::open(&path) else {
            continue;
        };
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
    write(&ctx.dirs.index_file(), &built.index)?;
    Ok(built.warnings)
}
