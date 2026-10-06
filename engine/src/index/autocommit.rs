//! `logbook.git.autocommit` (ADR-0035 §2): the last autocommit the engine
//! attempted in a logbook, kept in `<state>/autocommit.json` so the index
//! can show it after the command that ran it has ended.
//!
//! [`record`] is called by `commands::autocommit*` for every attempt (a
//! commit or a git failure; a skip is no attempt). [`attach`] puts the
//! record into an index's `logbook.git` when the logbook is a repository,
//! `git.autocommit` is on and the record is this logbook's (bound by its
//! canonical path, like `cursors.json`).

use std::path::{Path, PathBuf};

use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};

use super::model::{AutocommitInfo, GitInfo};
use crate::collectors::ShownMessages;
use crate::config::{Config, Dirs};
use crate::model::event::format_ts;

/// Longest `message` in the index (characters, `…` included).
pub const MESSAGE_MAX: usize = 256;

/// `<state>/autocommit.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Record {
    logbook: PathBuf,
    ok: bool,
    at: String,
    message: String,
}

pub fn file(dirs: &Dirs) -> PathBuf {
    dirs.state_dir.join("autocommit.json")
}

/// Records an attempt in `root`: `ok` with the commit subject, or not with
/// the git error. The message is kept as the index shows it (one line,
/// redacted, clipped). Best effort: a record that cannot be written leaves
/// the index with the previous one; the command's own output carries the
/// result either way.
pub fn record(
    dirs: &Dirs,
    config: &Config,
    root: &Path,
    at: DateTime<FixedOffset>,
    ok: bool,
    message: &str,
) {
    let record = Record {
        logbook: canonical(root),
        ok,
        at: format_ts(&at),
        message: shown(&ShownMessages::new(Some(config)), message),
    };
    if let Ok(mut text) = serde_json::to_string(&record) {
        text.push('\n');
        let _ = crate::sys::write_atomic(&file(dirs), text.as_bytes());
    }
}

/// `git.autocommit` from the record, when `git` is present, autocommit is
/// on and the record belongs to `root`. A missing or unreadable record is
/// no field.
pub fn attach(git: &mut Option<GitInfo>, dirs: &Dirs, config: &Config, root: &Path) {
    let Some(git) = git.as_mut() else {
        return;
    };
    git.autocommit = None;
    if !config.git.autocommit {
        return;
    }
    let Ok(text) = std::fs::read_to_string(file(dirs)) else {
        return;
    };
    let Ok(record) = serde_json::from_str::<Record>(&text) else {
        return;
    };
    if record.logbook != canonical(root) || DateTime::parse_from_rfc3339(&record.at).is_err() {
        return;
    }
    git.autocommit = Some(AutocommitInfo {
        ok: record.ok,
        at: record.at,
        // once more through today's redaction, as collector messages
        message: shown(&ShownMessages::new(Some(config)), &record.message),
    });
}

/// `message` as the index carries it: its first non-empty line, trimmed,
/// redacted, at most [`MESSAGE_MAX`] characters.
fn shown(shown: &ShownMessages, message: &str) -> String {
    let line = message
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or_default();
    let line = shown.show(line);
    match line.char_indices().nth(MESSAGE_MAX - 1) {
        Some((i, _)) if line.chars().count() > MESSAGE_MAX => format!("{}…", &line[..i]),
        _ => line,
    }
}

fn canonical(root: &Path) -> PathBuf {
    std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_line_clipped() {
        let config = Config::default();
        let s = ShownMessages::new(Some(&config));
        assert_eq!(shown(&s, "\n  first  \nsecond"), "first");
        let long = "x".repeat(300);
        let cut = shown(&s, &long);
        assert_eq!(cut.chars().count(), MESSAGE_MAX);
        assert!(cut.ends_with('…'));
        assert_eq!(shown(&s, &"y".repeat(MESSAGE_MAX)), "y".repeat(MESSAGE_MAX));
    }
}
