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
/// on and the record belongs to `root`. A missing record, or one of
/// another logbook, is no field; a record that is not a regular file of at
/// most 4 MiB (a FIFO, a link, a device: never opened), cannot be read or
/// is not a record is no field and a build warning, returned for the
/// caller's warnings (WP-120 round 3).
#[must_use]
pub fn attach(
    git: &mut Option<GitInfo>,
    dirs: &Dirs,
    config: &Config,
    root: &Path,
) -> Option<String> {
    let git = git.as_mut()?;
    git.autocommit = None;
    if !config.git.autocommit {
        return None;
    }
    let path = file(dirs);
    let skipped = |why: &str| {
        Some(format!(
            "{}: {why}; logbook.git.autocommit left out",
            dirs.display(&path).escape_debug()
        ))
    };
    let text = match crate::sys::read_small_file(&path, crate::sys::STATE_FILE_MAX) {
        Ok(Some(text)) => text,
        Ok(None) => return None,
        Err(why) => return skipped(&format!("not read ({why})")),
    };
    let Ok(record) = serde_json::from_str::<Record>(&text) else {
        return skipped("not an autocommit record");
    };
    if record.logbook != canonical(root) {
        return None;
    }
    if DateTime::parse_from_rfc3339(&record.at).is_err() {
        return skipped("not an autocommit record");
    }
    git.autocommit = Some(AutocommitInfo {
        ok: record.ok,
        at: record.at,
        // once more through today's redaction, as collector messages
        message: shown(&ShownMessages::new(Some(config)), &record.message),
    });
    None
}

/// `message` as the index carries it: its first non-empty line, every
/// control character (ESC, BEL, TAB, …) a space (WP-120 round 3: git's
/// colour codes and a hook's bell are no text), trimmed, redacted, at most
/// [`MESSAGE_MAX`] characters.
fn shown(shown: &ShownMessages, message: &str) -> String {
    let line: String = message
        .lines()
        .map(|l| {
            l.chars()
                .map(|c| if c.is_control() { ' ' } else { c })
                .collect::<String>()
        })
        .map(|l| l.trim().to_string())
        .find(|l| !l.is_empty())
        .unwrap_or_default();
    let line = shown.show(&line);
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

    /// WP-120 round 2, B2: what [`record`] writes is redacted, whatever
    /// the caller passed (the commands redact a git error first; a commit
    /// subject is the engine's own text, but the record does not rely on
    /// either).
    #[test]
    fn the_record_is_redacted_when_written() {
        let tmp = std::env::temp_dir().join(format!("seldon-autocommit-{}", std::process::id()));
        let dirs = Dirs {
            home: tmp.clone(),
            xdg_config_home: tmp.join("config"),
            state_dir: tmp.join("state"),
        };
        let root = tmp.join("logbook");
        std::fs::create_dir_all(&root).unwrap();
        let at = DateTime::parse_from_rfc3339("2026-10-06T10:00:00+02:00").unwrap();
        record(
            &dirs,
            &Config::default(),
            &root,
            at,
            false,
            "refused: https://user:geheim@example.org token=abc123geheim\nsecond line",
        );
        let text = std::fs::read_to_string(file(&dirs)).unwrap();
        let _ = std::fs::remove_dir_all(&tmp);
        let r: Record = serde_json::from_str(&text).unwrap();
        assert!(!r.message.contains("geheim"), "{}", r.message);
        assert!(r.message.contains("‹redacted›"), "{}", r.message);
        assert!(!r.message.contains("second line"));
        assert_eq!(r.at, "2026-10-06T10:00:00+02:00");
    }

    /// WP-120 round 3: control characters are spaces before the clip.
    #[test]
    fn control_characters_are_spaces() {
        let config = Config::default();
        let s = ShownMessages::new(Some(&config));
        let got = shown(&s, "fatal: \x1b[31mred\x1b[0m\ttab \x07bell");
        assert_eq!(got, "fatal:  [31mred [0m tab  bell");
        assert!(!got.chars().any(char::is_control), "{got:?}");
        // a line of control characters alone is no line
        assert_eq!(shown(&s, "\x1b\x07\t\nsecond"), "second");
    }

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
