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
    sys::write_atomic(path, to_text(index).as_bytes())
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
    Some(model::GitInfo { head, dirty })
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
/// on stderr, and the plugin shows the index as stale until the next
/// `seldon status`.
pub fn rebuild_if_initialised(ctx: &Context) {
    if let Err(e) = try_rebuild(ctx) {
        eprintln!("seldon: warning: index.json not rebuilt: {e}");
    }
}

fn try_rebuild(ctx: &Context) -> Result<()> {
    let config = ctx.load_config()?.unwrap_or_default();
    let (root, _) = ctx.resolve_logbook(None, Some(&config));
    if !Logbook::is_initialised(&root) {
        return Ok(());
    }
    let logbook = Logbook::open(&root)?;
    let mut built = derive(ctx, &config, &logbook)?;
    built.index.logbook.git = git_info(&logbook.root);
    write(&ctx.dirs.index_file(), &built.index)?;
    Ok(())
}
