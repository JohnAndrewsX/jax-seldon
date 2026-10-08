//! Command implementations. Each returns an [`Output`] with a human and a
//! JSON rendering (SPEC-ENGINE §1.4); `main.rs` picks one.

pub mod agent;
pub mod capture;
pub mod decide;
pub mod doctor;
pub mod dossier;
pub mod drift;
pub mod event;
pub mod hook;
pub mod import;
pub mod index;
pub mod init;
pub mod log;
pub mod manual;
pub mod open;
pub mod plan;
pub mod preview;
pub mod rebuild;
pub mod rules;
pub mod setup;
pub mod skills;
pub mod status;
pub mod triage;
pub mod watch;

use std::io::Write as _;
use std::path::{Path, PathBuf};

use anyhow::Context as _;
use chrono::{DateTime, FixedOffset, Local, Timelike as _};
use serde_json::json;

use crate::config::{self, Config, Dirs, LogbookSource};
use crate::error::{Error, Exit, Result};
use crate::logbook::{Logbook, git, lock};

pub use event::{emit, emit_one, event_json};

/// Environment variable that overrides the config file (`--config` wins).
pub const CONFIG_ENV: &str = "SELDON_CONFIG";

/// Environment variable that fixes the clock (RFC 3339 with offset), for
/// tests and reproducible demos. Never set it in normal use.
pub const NOW_ENV: &str = "SELDON_NOW";

/// Global flags and the environment, read once per process.
#[derive(Debug, Clone)]
pub struct Context {
    pub dirs: Dirs,
    pub json: bool,
    pub quiet: bool,
    pub no_commit: bool,
    /// `--logbook DIR`.
    pub logbook_flag: Option<PathBuf>,
    /// `$SELDON_LOGBOOK`.
    pub logbook_env: Option<String>,
    /// `config.toml`: `--config` > `$SELDON_CONFIG` > XDG default.
    pub config_file: PathBuf,
    /// The time of this invocation: every event, journal heading and Log
    /// line of one command carries it (`$SELDON_NOW` overrides).
    pub now: DateTime<FixedOffset>,
}

impl Context {
    pub fn from_env(
        json: bool,
        quiet: bool,
        no_commit: bool,
        logbook: Option<PathBuf>,
        config: Option<PathBuf>,
    ) -> Result<Self> {
        let dirs = Dirs::from_env()?;
        let config_file = config
            .map(|p| dirs.expand(&p.to_string_lossy()))
            .or_else(|| {
                std::env::var(CONFIG_ENV)
                    .ok()
                    .filter(|p| !p.is_empty())
                    .map(|p| dirs.expand(&p))
            })
            .unwrap_or_else(|| dirs.config_file());
        let now = match std::env::var(NOW_ENV).ok().filter(|s| !s.is_empty()) {
            Some(s) => DateTime::parse_from_rfc3339(&s)
                .map_err(|e| Error::user(format!("{NOW_ENV}={s}: {e}")))?,
            None => Local::now().fixed_offset(),
        };
        // whole seconds, like every `ts` in fixtures/logbook/ledger/: the
        // ledger keeps fractions when a timestamp has them
        let now = now.with_nanosecond(0).unwrap_or(now);
        Ok(Context {
            dirs,
            json,
            quiet,
            no_commit,
            logbook_flag: logbook,
            logbook_env: std::env::var(config::LOGBOOK_ENV).ok(),
            config_file,
            now,
        })
    }

    pub fn load_config(&self) -> Result<Option<Config>> {
        Config::load(&self.config_file)
    }

    /// The logbook path; `explicit` (a command's own `--path`) wins over
    /// everything else.
    pub fn resolve_logbook(
        &self,
        explicit: Option<&Path>,
        config: Option<&Config>,
    ) -> (PathBuf, LogbookSource) {
        config::resolve_logbook(
            &self.dirs,
            explicit.or(self.logbook_flag.as_deref()),
            self.logbook_env.as_deref(),
            config,
        )
    }

    /// The config (defaults if there is none) and the opened logbook; exit 3
    /// when it is not initialised.
    pub fn open_logbook(&self) -> Result<(Config, Logbook)> {
        let config = self.load_config()?;
        let (root, _) = self.resolve_logbook(None, config.as_ref());
        let logbook = Logbook::open(&root)?;
        Ok((config.unwrap_or_default(), logbook))
    }

    /// Takes the state lock (exit 4 when another writer holds it).
    pub fn lock(&self) -> Result<lock::Lock> {
        lock::acquire(&self.dirs.lock_file())
    }
}

/// What a command prints, and how it exits.
#[derive(Debug, Clone)]
pub struct Output {
    pub human: String,
    pub json: serde_json::Value,
    pub exit: Exit,
}

impl Output {
    pub fn ok(human: impl Into<String>, json: serde_json::Value) -> Self {
        Output {
            human: human.into(),
            json,
            exit: Exit::Ok,
        }
    }
}

/// The outcome of the git autocommit after a logbook write.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Commit {
    Committed(String),
    /// `--no-commit`, `git.autocommit = false`, or no repository.
    Skipped(&'static str),
    /// Reported, never fatal: the logbook write already happened.
    Failed(String),
    /// A failed autocommit whose warning is already on stderr (F-503): the
    /// human output adds nothing, `--json` carries the error.
    Warned(String),
}

impl Commit {
    pub fn json(&self) -> serde_json::Value {
        match self {
            Commit::Committed(message) => json!({ "committed": true, "message": message }),
            Commit::Skipped(reason) => json!({ "committed": false, "reason": reason }),
            Commit::Failed(error) | Commit::Warned(error) => {
                json!({ "committed": false, "error": error })
            }
        }
    }

    /// Keeps an attempt for `logbook.git.autocommit` (ADR-0035 §2); a
    /// skip is no attempt.
    pub fn record(&self, ctx: &Context, config: &Config, logbook: &Logbook) {
        let (ok, message) = match self {
            Commit::Committed(m) => (true, m),
            Commit::Failed(e) | Commit::Warned(e) => (false, e),
            Commit::Skipped(_) => return,
        };
        crate::index::autocommit::record(&ctx.dirs, config, &logbook.root, ctx.now, ok, message);
    }

    /// A line for the human output, empty when there is nothing to say.
    pub fn human(&self) -> String {
        match self {
            Commit::Failed(e) => format!("\nGit: not committed: {e}"),
            _ => String::new(),
        }
    }
}

/// `git add -A` + `git commit -m "seldon: <summary>"` in the logbook when
/// `git.autocommit` is on, `--no-commit` is not given and the logbook is a
/// repository (SPEC-LOGBOOK §1). A failure (a stale `.git/index.lock`, a
/// refusing hook, a detached HEAD) is one warning line on stderr and the
/// `error` of `--json` `git`; the command still exits 0, because the data
/// is written (SPEC-ENGINE §3, WP-061).
pub fn autocommit(ctx: &Context, config: &Config, logbook: &Logbook, summary: &str) -> Commit {
    if ctx.no_commit {
        return Commit::Skipped("--no-commit");
    }
    if !config.git.autocommit {
        return Commit::Skipped("git.autocommit = false");
    }
    if !git::is_repo(&logbook.root) {
        return Commit::Skipped("the logbook is not a git repository");
    }
    let commit = match git::commit_all(&logbook.root, summary) {
        Ok(()) => Commit::Committed(format!("seldon: {summary}")),
        Err(e) => {
            let e = redacted_git_error(config, &e);
            // not eprintln!: a closed stderr must not abort the command
            let _ = writeln!(
                std::io::stderr(),
                "seldon: warning: git: not committed: {e}"
            );
            Commit::Warned(e)
        }
    };
    commit.record(ctx, config, logbook);
    commit
}

/// A git error as the engine shows it — on stderr, in `--json` `git.error`
/// and in `autocommit.json` — through the logbook's redaction (SPEC-ENGINE
/// §7; WP-120 round 2, N6): a hook's output or a remote URL may carry a
/// secret. An invalid `[redaction] patterns` entry withholds it.
fn redacted_git_error(config: &Config, error: &str) -> String {
    crate::collectors::ShownMessages::new(Some(config)).show(error)
}

/// [`autocommit`] of `paths` alone (relative to the logbook): a commit of
/// its own that leaves the user's other changes out ([`git::commit_paths`]).
pub fn autocommit_paths(
    ctx: &Context,
    config: &Config,
    logbook: &Logbook,
    paths: &[&str],
    summary: &str,
) -> Commit {
    if ctx.no_commit {
        return Commit::Skipped("--no-commit");
    }
    if !config.git.autocommit {
        return Commit::Skipped("git.autocommit = false");
    }
    if !git::is_repo(&logbook.root) {
        return Commit::Skipped("the logbook is not a git repository");
    }
    let commit = match git::commit_paths(&logbook.root, paths, summary) {
        Ok(()) => Commit::Committed(format!("seldon: {summary}")),
        Err(e) => {
            let e = redacted_git_error(config, &e);
            let _ = writeln!(
                std::io::stderr(),
                "seldon: warning: git: not committed: {e}"
            );
            Commit::Warned(e)
        }
    };
    commit.record(ctx, config, logbook);
    commit
}

/// The text of a free-text argument, or a user error when it is blank.
pub(crate) fn required_text(what: &str, text: &str) -> Result<String> {
    let text = text.trim();
    if text.is_empty() {
        return Err(Error::user(format!("{what} must not be empty")));
    }
    Ok(text.to_string())
}

/// A one-line free text (titles, reasons): blank or multi-line is an error,
/// and so are the Unicode line and paragraph separators and the bidi
/// controls, which break or reorder a line where it is shown (WP-124
/// round 2).
pub(crate) fn one_line(what: &str, text: &str) -> Result<String> {
    let text = required_text(what, text)?;
    if text.contains(['\n', '\r']) || text.chars().any(is_line_breaking) {
        return Err(Error::user(format!(
            "{what} must be one line (no line or paragraph separator, no bidi control)"
        )));
    }
    Ok(text)
}

/// U+2028, U+2029 and the bidi controls U+202A–U+202E, U+2066–U+2069.
pub(crate) fn is_line_breaking(c: char) -> bool {
    matches!(c, '\u{2028}' | '\u{2029}' | '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}')
}

/// Creates `path` with `text`; an existing file is a user error, never
/// overwritten.
pub(crate) fn write_new(path: &Path, text: &str) -> Result<()> {
    if let Some(dir) = path.parent() {
        crate::sys::create_dir_private(dir)
            .with_context(|| format!("cannot create {}", dir.display()))?;
    }
    let mut file = crate::sys::create_new_private(path).map_err(|e| match e.kind() {
        std::io::ErrorKind::AlreadyExists => {
            Error::user(format!("{} already exists", path.display()))
        }
        _ => anyhow::Error::new(e)
            .context(format!("cannot create {}", path.display()))
            .into(),
    })?;
    file.write_all(text.as_bytes())
        .with_context(|| format!("cannot write {}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_line_refuses_separators_and_bidi_controls() {
        assert_eq!(one_line("t", " a b ").unwrap(), "a b");
        for bad in [
            "a\nb",
            "a\rb",
            "a\u{2028}b",
            "a\u{2029}b",
            "a\u{202A}b",
            "a\u{202E}b",
            "a\u{2066}b",
            "a\u{2069}b",
        ] {
            let e = one_line("the title", bad).unwrap_err().to_string();
            assert!(e.contains("the title must be one line"), "{bad:?}: {e}");
        }
        assert!(one_line("t", "a\u{2027}b\u{206A}c").is_ok());
    }
}
