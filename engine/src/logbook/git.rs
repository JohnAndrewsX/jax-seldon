//! The logbook's git repository (SPEC-LOGBOOK §1). Only `git` with fixed
//! argument lists; commit messages are `seldon: <summary>`.
//!
//! Every git command runs in the logbook with the variables that select
//! another repository removed ([`REPOSITORY_VARS`]): a `seldon` started
//! from a git hook, from `git rebase -x` or from a dotfiles helper inherits
//! `GIT_DIR` and friends, and git gives them priority over the working
//! directory, so the autocommit would land in that other repository.
//! Everything else (the user's git config, hooks, signing, the terminal
//! for a passphrase prompt) stays as it is.

use std::path::Path;
use std::process::Command;
use std::time::Duration;

use crate::sys::{self, Run};

const TIMEOUT: Duration = Duration::from_secs(30);

/// Identity used when the user has none configured, so the first commit
/// does not fail on a fresh machine.
const FALLBACK_IDENTITY: [&str; 4] = [
    "-c",
    "user.name=Seldon",
    "-c",
    "user.email=seldon@localhost",
];

/// Variables that point git at another repository, index or object store
/// (`git rev-parse --local-env-vars`, plus `GIT_NAMESPACE`,
/// `GIT_CEILING_DIRECTORIES` and `GIT_QUARANTINE_PATH`).
const REPOSITORY_VARS: [&str; 18] = [
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_IMPLICIT_WORK_TREE",
    "GIT_INDEX_FILE",
    "GIT_OBJECT_DIRECTORY",
    "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    "GIT_COMMON_DIR",
    "GIT_NAMESPACE",
    "GIT_CEILING_DIRECTORIES",
    "GIT_QUARANTINE_PATH",
    "GIT_PREFIX",
    "GIT_CONFIG",
    "GIT_CONFIG_PARAMETERS",
    "GIT_CONFIG_COUNT",
    "GIT_GRAFT_FILE",
    "GIT_NO_REPLACE_OBJECTS",
    "GIT_REPLACE_REF_BASE",
    "GIT_SHALLOW_FILE",
];

/// `git args…` in `root` (or anywhere when `None`), with
/// [`REPOSITORY_VARS`] removed from its environment.
fn command(root: Option<&Path>, args: &[&str]) -> Command {
    let mut cmd = Command::new("git");
    cmd.args(args);
    for var in REPOSITORY_VARS {
        cmd.env_remove(var);
    }
    if let Some(dir) = root {
        cmd.current_dir(dir);
    }
    cmd
}

fn run(root: Option<&Path>, args: &[&str]) -> Run {
    sys::run_command(command(root, args), TIMEOUT)
}

/// `git version`, or `None` if git is not installed.
pub fn version() -> Option<String> {
    match run(None, &["--version"]) {
        Run::Exited {
            code: Some(0),
            stdout,
            ..
        } => Some(stdout.trim().to_string()),
        _ => None,
    }
}

/// Whether `root` is the top of a git work tree (has its own `.git`).
pub fn is_repo(root: &Path) -> bool {
    root.join(".git").exists()
}

/// `git init` in `root`.
pub fn init(root: &Path) -> Result<(), String> {
    git(root, &["init", "-q"])
}

/// Why nothing is committed while HEAD is not a branch.
pub const DETACHED: &str = "HEAD is detached (no branch is checked out)";

/// `git add -A` and `git commit -m "seldon: <summary>"` in `root`. Nothing
/// staged (only ignored files changed, e.g. `.seldon/active-case`) is not
/// an error; nothing is committed then. A detached HEAD is an error
/// ([`DETACHED`]) and nothing is staged: a commit there belongs to no
/// branch and is lost from view at the next checkout.
///
/// The whole work tree is committed, not only the files the engine wrote:
/// the logbook's history is its backup, so edits made in an editor since
/// the last command are recorded with the next engine write.
pub fn commit_all(root: &Path, summary: &str) -> Result<(), String> {
    if is_detached(root)? {
        return Err(DETACHED.to_string());
    }
    git(root, &["add", "-A"])?;
    if matches!(
        run(Some(root), &["diff", "--cached", "--quiet"]),
        Run::Exited { code: Some(0), .. }
    ) && has_head(root)
    {
        return Ok(());
    }
    let message = format!("seldon: {summary}");
    let mut args: Vec<&str> = Vec::new();
    if !has_identity(root) {
        args.extend(FALLBACK_IDENTITY);
    }
    args.extend(["commit", "-q", "-m", &message]);
    git(root, &args)
}

/// Whether HEAD is detached: `git symbolic-ref -q HEAD` exits 1. An unborn
/// branch is not detached.
pub fn is_detached(root: &Path) -> Result<bool, String> {
    match run(Some(root), &["symbolic-ref", "-q", "HEAD"]) {
        Run::Exited { code: Some(0), .. } => Ok(false),
        Run::Exited { code: Some(1), .. } => Ok(true),
        other => Err(failure("symbolic-ref", &other)),
    }
}

/// The local branch names (`refs/heads/`), for a fix line.
pub fn branches(root: &Path) -> Vec<String> {
    match run(
        Some(root),
        &["for-each-ref", "--format=%(refname:short)", "refs/heads/"],
    ) {
        Run::Exited {
            code: Some(0),
            stdout,
            ..
        } => stdout.lines().map(str::to_string).collect(),
        _ => Vec::new(),
    }
}

/// `git commit --dry-run` as the autocommit would run it (no hooks, no
/// signing; it needs `.git/index.lock` like the real commit). "Nothing to
/// commit" (exit 1) is fine. `GIT_OPTIONAL_LOCKS=0` keeps it from
/// refreshing the index on the side.
pub fn commit_dry_run(root: &Path) -> Result<(), String> {
    let mut args: Vec<&str> = Vec::new();
    if !has_identity(root) {
        args.extend(FALLBACK_IDENTITY);
    }
    args.extend(["commit", "--dry-run", "-q"]);
    let mut cmd = command(Some(root), &args);
    cmd.env("GIT_OPTIONAL_LOCKS", "0");
    match sys::run_command(cmd, TIMEOUT) {
        Run::Exited {
            code: Some(0 | 1), ..
        } => Ok(()),
        other => Err(failure("commit --dry-run", &other)),
    }
}

/// Whether the work tree has changes that are not committed (ignored
/// files do not count).
pub fn is_dirty(root: &Path) -> Result<bool, String> {
    match run(Some(root), &["status", "--porcelain"]) {
        Run::Exited {
            code: Some(0),
            stdout,
            ..
        } => Ok(!stdout.trim().is_empty()),
        other => Err(failure("status", &other)),
    }
}

/// The full hash of HEAD, `None` before the first commit.
pub fn head(root: &Path) -> Option<String> {
    match run(Some(root), &["rev-parse", "--verify", "-q", "HEAD"]) {
        Run::Exited {
            code: Some(0),
            stdout,
            ..
        } => Some(stdout.trim().to_string()).filter(|h| !h.is_empty()),
        _ => None,
    }
}

/// Whether the repository has a first commit (`diff --cached` against an
/// unborn HEAD says nothing useful).
fn has_head(root: &Path) -> bool {
    head(root).is_some()
}

fn has_identity(root: &Path) -> bool {
    matches!(
        run(Some(root), &["config", "--get", "user.email"]),
        Run::Exited { code: Some(0), ref stdout, .. } if !stdout.trim().is_empty()
    )
}

fn git(root: &Path, args: &[&str]) -> Result<(), String> {
    let verb = args
        .iter()
        .find(|a| !a.starts_with('-') && !a.contains('='))
        .unwrap_or(&"");
    match run(Some(root), args) {
        Run::Exited { code: Some(0), .. } => Ok(()),
        Run::TimedOut => Err(format!("git {} timed out", args.join(" "))),
        other => Err(failure(verb, &other)),
    }
}

/// The message of a git call that did not succeed, on one line (git's
/// own message may have several).
fn failure(verb: &str, run: &Run) -> String {
    match run {
        Run::Exited { stderr, .. } => {
            let text: Vec<&str> = stderr
                .split(|c: char| c.is_control())
                .map(str::trim)
                .filter(|l| !l.is_empty())
                .collect();
            format!("git {verb} failed: {}", text.join(" "))
        }
        Run::NotFound => "git is not installed".into(),
        Run::TimedOut => format!("git {verb} timed out"),
        Run::Failed(e) => format!("cannot run git: {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_command_drops_every_repository_variable() {
        let cmd = command(Some(Path::new("/logbook")), &["status"]);
        let removed: Vec<String> = cmd
            .get_envs()
            .filter(|(_, value)| value.is_none())
            .map(|(key, _)| key.to_string_lossy().into_owned())
            .collect();
        for var in REPOSITORY_VARS {
            assert!(removed.iter().any(|r| r == var), "{var} is not removed");
        }
        assert_eq!(cmd.get_current_dir(), Some(Path::new("/logbook")));
    }
}
