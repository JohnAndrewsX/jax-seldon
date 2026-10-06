//! The logbook's git repository (SPEC-LOGBOOK §1). Only `git` with fixed
//! argument lists; commit messages are `seldon: <summary>`.
//!
//! Every git command runs in the logbook with the variables that select
//! another repository removed ([`REPOSITORY_VARS`]): a `seldon` started
//! from a git hook, from `git rebase -x` or from a dotfiles helper inherits
//! `GIT_DIR` and friends, and git gives them priority over the working
//! directory, so the autocommit would land in that other repository.
//! `GIT_CEILING_DIRECTORIES` is the logbook's parent, so an empty or broken
//! `.git` never makes git walk up into a repository around the logbook,
//! and [`commit_all`] checks `git rev-parse --show-toplevel` against the
//! logbook before it writes. Everything else (the user's git config,
//! hooks, signing, the terminal for a passphrase prompt) stays as it is:
//! git runs in the engine's process group
//! ([`sys::run_command_in_engine_group`]).

use std::path::{Path, PathBuf};
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
/// `GIT_CEILING_DIRECTORIES` and `GIT_QUARANTINE_PATH`). The ceiling is set
/// again to the logbook's parent when the command runs in the logbook.
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
/// [`REPOSITORY_VARS`] removed from its environment and, in `root`,
/// `GIT_CEILING_DIRECTORIES` set to its parent.
fn command(root: Option<&Path>, args: &[&str]) -> Command {
    let mut cmd = Command::new("git");
    cmd.args(args);
    for var in REPOSITORY_VARS {
        cmd.env_remove(var);
    }
    if let Some(dir) = root {
        cmd.current_dir(dir);
        if let Some(parent) = absolute(dir).parent() {
            cmd.env("GIT_CEILING_DIRECTORIES", parent);
        }
    }
    cmd
}

/// `path` resolved (symbolic links, `..`), or made absolute when it does
/// not exist.
fn absolute(path: &Path) -> PathBuf {
    path.canonicalize()
        .or_else(|_| std::path::absolute(path))
        .unwrap_or_else(|_| path.to_path_buf())
}

fn run(root: Option<&Path>, args: &[&str]) -> Run {
    sys::run_command_in_engine_group(command(root, args), TIMEOUT)
}

/// A read-only query in the logbook's repository, with the same
/// environment as every other git call here (`logbook.git` in the index).
pub fn query(root: &Path, args: &[&str], timeout: Duration) -> Run {
    sys::run_command_in_engine_group(command(Some(root), args), timeout)
}

/// Whether `root` is the logbook's own repository: the top of the work
/// tree git finds there, with its git directory at `<root>/.git` or, for a
/// linked work tree, at `<repo>/worktrees/<name>` whose `gitdir` file
/// names `<root>/.git`. An empty or broken `.git` (git sees no repository
/// below the ceiling), a `.git` that resolves to another work tree, and a
/// `.git` file (`gitdir: …`) that points at another repository's git
/// directory are errors that say which.
pub fn check_toplevel(root: &Path) -> Result<(), String> {
    let out = match run(
        Some(root),
        &["rev-parse", "--show-toplevel", "--absolute-git-dir"],
    ) {
        Run::Exited {
            code: Some(0),
            stdout,
            ..
        } => stdout,
        Run::Exited { stderr, .. } => {
            return Err(format!(
                "the logbook's .git is not a usable repository: {}",
                one_line(&stderr)
            ));
        }
        other => return Err(failure("rev-parse", &other)),
    };
    let mut lines = out.split('\n');
    let (Some(top), Some(git_dir)) = (lines.next(), lines.next()) else {
        return Err(format!("git rev-parse printed no git directory: {out:?}"));
    };
    let top = absolute(Path::new(top));
    if top != absolute(root) {
        return Err(format!(
            "the logbook's .git belongs to another work tree ({})",
            top.display()
        ));
    }
    let git_dir = absolute(Path::new(git_dir));
    let own = absolute(&root.join(".git"));
    if git_dir == own || is_linked_work_tree_of(&git_dir, &own) {
        Ok(())
    } else {
        Err(format!(
            "the logbook's .git points at another repository's git directory ({})",
            git_dir.display()
        ))
    }
}

/// Whether `git_dir` is `<repo>/worktrees/<name>` registered for the
/// `.git` file `dot_git` (its `gitdir` file names that file).
fn is_linked_work_tree_of(git_dir: &Path, dot_git: &Path) -> bool {
    let registered = git_dir
        .parent()
        .and_then(Path::file_name)
        .is_some_and(|n| n == "worktrees");
    registered
        && std::fs::read_to_string(git_dir.join("gitdir"))
            .is_ok_and(|back| absolute(Path::new(back.trim_end_matches(['\n', '\r']))) == dot_git)
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
    check_toplevel(root)?;
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

/// Commits `paths` (relative to `root`) alone, `seldon: <summary>`:
/// whatever else is changed or staged stays as it is (`git commit --
/// <paths>`). Nothing to commit in them: `Ok`, no commit.
pub fn commit_paths(root: &Path, paths: &[&str], summary: &str) -> Result<(), String> {
    check_toplevel(root)?;
    if is_detached(root)? {
        return Err(DETACHED.to_string());
    }
    let mut add = vec!["add", "--"];
    add.extend(paths);
    git(root, &add)?;
    let mut diff = vec!["diff", "--cached", "--quiet", "--"];
    diff.extend(paths);
    if matches!(run(Some(root), &diff), Run::Exited { code: Some(0), .. }) && has_head(root) {
        return Ok(());
    }
    let message = format!("seldon: {summary}");
    let mut args: Vec<&str> = Vec::new();
    if !has_identity(root) {
        args.extend(FALLBACK_IDENTITY);
    }
    args.extend(["commit", "-q", "-m", &message, "--"]);
    args.extend(paths);
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

/// Whether the autocommit could resolve its committer and author, with
/// the same fallback identity it uses (`git var`, which only reads).
pub fn check_identity(root: &Path) -> Result<(), String> {
    for ident in ["GIT_COMMITTER_IDENT", "GIT_AUTHOR_IDENT"] {
        let mut args: Vec<&str> = Vec::new();
        if !has_identity(root) {
            args.extend(FALLBACK_IDENTITY);
        }
        args.extend(["var", ident]);
        match run(Some(root), &args) {
            Run::Exited { code: Some(0), .. } => {}
            other => return Err(failure("var", &other)),
        }
    }
    Ok(())
}

/// Whether HEAD is a commit, or an unborn branch; anything else (a ref
/// that points nowhere, a broken object) is an error.
pub fn check_head(root: &Path) -> Result<(), String> {
    match run(Some(root), &["rev-parse", "--verify", "-q", "HEAD"]) {
        Run::Exited { code: Some(0), .. } => Ok(()),
        Run::Exited { .. } => {
            // unborn: HEAD names a branch that has no ref yet
            let unborn = match run(Some(root), &["symbolic-ref", "-q", "HEAD"]) {
                Run::Exited {
                    code: Some(0),
                    stdout,
                    ..
                } => !run(Some(root), &["show-ref", "--verify", "-q", stdout.trim()]).success(),
                _ => false,
            };
            if unborn {
                Ok(())
            } else {
                Err("HEAD does not name a commit".to_string())
            }
        }
        other => Err(failure("rev-parse", &other)),
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

/// git's message on one line: its `fatal:`/`error:` lines when it has
/// any (the advice around them is left out), else every line.
fn one_line(stderr: &str) -> String {
    let lines: Vec<&str> = stderr
        .split(|c: char| c.is_control())
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();
    let errors: Vec<&str> = lines
        .iter()
        .copied()
        .filter(|l| l.starts_with("fatal:") || l.starts_with("error:"))
        .collect();
    if errors.is_empty() {
        lines.join(" ")
    } else {
        errors.join(" ")
    }
}

/// The message of a git call that did not succeed, on one line.
fn failure(verb: &str, run: &Run) -> String {
    match run {
        Run::Exited { stderr, .. } => format!("git {verb} failed: {}", one_line(stderr)),
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
            if var != "GIT_CEILING_DIRECTORIES" {
                assert!(removed.iter().any(|r| r == var), "{var} is not removed");
            }
        }
        assert_eq!(cmd.get_current_dir(), Some(Path::new("/logbook")));
        let ceiling = cmd
            .get_envs()
            .find(|(key, _)| *key == "GIT_CEILING_DIRECTORIES")
            .and_then(|(_, value)| value);
        assert_eq!(ceiling, Some(std::ffi::OsStr::new("/")));
        // anywhere (git --version): no ceiling, the inherited one removed
        let cmd = command(None, &["--version"]);
        assert!(
            cmd.get_envs()
                .any(|(key, value)| key == "GIT_CEILING_DIRECTORIES" && value.is_none())
        );
    }
}
