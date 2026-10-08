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
//!
//! Rules for every git call (WP-154):
//! - Each output pipe keeps at most [`sys::OUTPUT_MAX`] bytes; a stdout
//!   over it is no answer ([`Run::Cut`]). A broken or hostile repository
//!   can make git print without end (a grafts file: a line of stderr for
//!   each bad line).
//! - A read-only query ([`ask`]: everything but `init`, `add` and
//!   `commit`) never reaches the network: [`QUERY_ENV`] and
//!   [`NO_LAZY_FETCH`]. A partial clone's missing object stays missing
//!   instead of starting a fetch, and with it a transport the
//!   repository's config may name (`ext::` runs a program). `init`, `add`
//!   and `commit` keep the user's git whole: hooks, signing, filters and
//!   transport.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crate::sys::{self, Run};

const TIMEOUT: Duration = Duration::from_secs(30);

/// Environment of a read-only query, on top of [`command`]'s: no protocol
/// at all, overriding any configuration (`none` is no protocol's name; a
/// repository's own `protocol.<name>.allow=always` beats
/// `protocol.allow=never`, not this), and no lazy fetch of a partial
/// clone's missing object (git 2.44 or later; an older git ignores the
/// variable, and the protocol rule alone refuses the fetch).
pub const QUERY_ENV: [(&str, &str); 2] =
    [("GIT_ALLOW_PROTOCOL", "none"), ("GIT_NO_LAZY_FETCH", "1")];

/// The option for `GIT_NO_LAZY_FETCH` (git 2.44 or later), first in a
/// query's argv. An older git refuses it ([`refuses_no_lazy_fetch`]);
/// the engine then asks once more without it and leaves it out for the
/// rest of the process.
pub const NO_LAZY_FETCH: &str = "--no-lazy-fetch";

/// The git on `PATH` refused [`NO_LAZY_FETCH`] once in this process.
static NO_LAZY_FETCH_REFUSED: AtomicBool = AtomicBool::new(false);

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
/// again to the logbook's parent when the command runs in the logbook. The
/// plugins collector's queries of a plugin's clone remove them too.
pub const REPOSITORY_VARS: [&str; 18] = [
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

/// `init`, `add` or `commit`: the user's git as it is, transport
/// included.
fn run(root: Option<&Path>, args: &[&str]) -> Run {
    sys::run_command_in_engine_group(command(root, args), TIMEOUT, sys::OUTPUT_MAX)
}

/// [`command`] for a read-only query: [`NO_LAZY_FETCH`] first when
/// `option`, and [`QUERY_ENV`].
fn query_command(root: Option<&Path>, args: &[&str], option: bool) -> Command {
    let mut argv = Vec::with_capacity(args.len() + 1);
    if option {
        argv.push(NO_LAZY_FETCH);
    }
    argv.extend_from_slice(args);
    let mut cmd = command(root, &argv);
    cmd.envs(QUERY_ENV);
    cmd
}

/// A read-only query that never reaches the network (module doc), in
/// `root` or anywhere.
fn ask(root: Option<&Path>, args: &[&str], timeout: Duration) -> Run {
    let option = !NO_LAZY_FETCH_REFUSED.load(Ordering::Relaxed);
    let run = |option| {
        sys::run_command_in_engine_group(
            query_command(root, args, option),
            timeout,
            sys::OUTPUT_MAX,
        )
    };
    let answer = run(option);
    if option && refuses_no_lazy_fetch(&answer) {
        NO_LAZY_FETCH_REFUSED.store(true, Ordering::Relaxed);
        return run(false);
    }
    answer
}

/// Whether git refused [`NO_LAZY_FETCH`] as an unknown option: git before
/// 2.44 prints `unknown option: --no-lazy-fetch` (translated in other
/// languages, the option itself is not) and its usage, and exits 129.
/// Only a line that ends in the option counts: a current git's usage text
/// (any usage error at git's level) lists `[--no-lazy-fetch]`.
fn refuses_no_lazy_fetch(run: &Run) -> bool {
    matches!(run, Run::Exited { code: Some(129), stderr, .. }
        if stderr.lines().any(|l| l.trim_end().ends_with(NO_LAZY_FETCH)))
}

/// [`ask`] in the logbook with [`TIMEOUT`].
fn query_here(root: &Path, args: &[&str]) -> Run {
    ask(Some(root), args, TIMEOUT)
}

/// A read-only query in the logbook's repository, with the same
/// environment as every other query here (`logbook.git` in the index).
pub fn query(root: &Path, args: &[&str], timeout: Duration) -> Run {
    ask(Some(root), args, timeout)
}

/// Whether `root` is the logbook's own repository: the top of the work
/// tree git finds there, with its git directory at `<root>/.git` or, for a
/// linked work tree, at `<repo>/worktrees/<name>` whose `gitdir` file
/// names `<root>/.git`. An empty or broken `.git` (git sees no repository
/// below the ceiling), a `.git` that resolves to another work tree, and a
/// `.git` file (`gitdir: …`) that points at another repository's git
/// directory are errors that say which.
pub fn check_toplevel(root: &Path) -> Result<(), String> {
    let out = match query_here(
        root,
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
    match ask(None, &["--version"], TIMEOUT) {
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
    // a query: when it cannot answer (exit 128), the commit runs and says
    if matches!(
        query_here(root, &["diff", "--cached", "--quiet"]),
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
    if matches!(query_here(root, &diff), Run::Exited { code: Some(0), .. }) && has_head(root) {
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
    match query_here(root, &["symbolic-ref", "-q", "HEAD"]) {
        Run::Exited { code: Some(0), .. } => Ok(false),
        Run::Exited { code: Some(1), .. } => Ok(true),
        other => Err(failure("symbolic-ref", &other)),
    }
}

/// The local branch names (`refs/heads/`), for a fix line.
pub fn branches(root: &Path) -> Vec<String> {
    match query_here(
        root,
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
        match query_here(root, &args) {
            Run::Exited { code: Some(0), .. } => {}
            other => return Err(failure("var", &other)),
        }
    }
    Ok(())
}

/// Whether HEAD is a commit, or an unborn branch; anything else (a ref
/// that points nowhere, a broken object) is an error.
pub fn check_head(root: &Path) -> Result<(), String> {
    match query_here(root, &["rev-parse", "--verify", "-q", "HEAD"]) {
        Run::Exited { code: Some(0), .. } => Ok(()),
        Run::Exited { .. } => {
            // unborn: HEAD names a branch that has no ref yet
            let unborn = match query_here(root, &["symbolic-ref", "-q", "HEAD"]) {
                Run::Exited {
                    code: Some(0),
                    stdout,
                    ..
                } => !query_here(root, &["show-ref", "--verify", "-q", stdout.trim()]).success(),
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
    status_is_empty(query_here(root, &["status", "--porcelain"])).map(|empty| !empty)
}

/// Whether `path` (relative to `root`) has no change against HEAD, staged
/// or not (`git status --porcelain -- <path>` prints nothing).
pub fn is_clean_path(root: &Path, path: &str) -> Result<bool, String> {
    status_is_empty(query_here(root, &["status", "--porcelain", "--", path]))
}

/// Whether a `git status --porcelain` printed nothing. A status over
/// [`sys::OUTPUT_MAX`] ([`Run::Cut`]) printed something: changes.
pub fn status_is_empty(run: Run) -> Result<bool, String> {
    match run {
        Run::Exited {
            code: Some(0),
            stdout,
            ..
        } => Ok(stdout.trim().is_empty()),
        Run::Cut => Ok(false),
        other => Err(failure("status", &other)),
    }
}

/// The full hash of HEAD, `None` before the first commit.
pub fn head(root: &Path) -> Option<String> {
    match query_here(root, &["rev-parse", "--verify", "-q", "HEAD"]) {
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
        query_here(root, &["config", "--get", "user.email"]),
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
        Run::Cut => format!(
            "git {verb} printed more than {} MiB",
            sys::OUTPUT_MAX / (1024 * 1024)
        ),
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

    fn env_of(cmd: &Command, key: &str) -> Option<String> {
        cmd.get_envs()
            .find(|(k, _)| *k == key)
            .and_then(|(_, v)| v)
            .map(|v| v.to_string_lossy().into_owned())
    }

    fn argv(cmd: &Command) -> Vec<String> {
        cmd.get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect()
    }

    /// WP-154: a query carries the no-network rules as literals; a call
    /// that writes (`init`, `add`, `commit`) carries none of them, so the
    /// user's hooks, signing and transport stay in effect.
    #[test]
    fn a_query_never_reaches_the_network_and_a_commit_is_the_users() {
        let root = Path::new("/logbook");
        let q = query_command(Some(root), &["status", "--porcelain"], true);
        assert_eq!(argv(&q), ["--no-lazy-fetch", "status", "--porcelain"]);
        assert_eq!(env_of(&q, "GIT_ALLOW_PROTOCOL").as_deref(), Some("none"));
        assert_eq!(env_of(&q, "GIT_NO_LAZY_FETCH").as_deref(), Some("1"));
        assert_eq!(env_of(&q, "GIT_CEILING_DIRECTORIES").as_deref(), Some("/"));
        assert!(env_of(&q, "GIT_DIR").is_none());
        // a git that refused the option: the same query without it
        let q = query_command(Some(root), &["status"], false);
        assert_eq!(argv(&q), ["status"]);
        assert_eq!(env_of(&q, "GIT_ALLOW_PROTOCOL").as_deref(), Some("none"));
        // anywhere (git --version) too
        let q = query_command(None, &["--version"], true);
        assert_eq!(argv(&q), ["--no-lazy-fetch", "--version"]);
        assert_eq!(env_of(&q, "GIT_NO_LAZY_FETCH").as_deref(), Some("1"));
        // a commit: no protocol rule, no lazy-fetch rule, no option
        let c = command(Some(root), &["commit", "-q", "-m", "seldon: x"]);
        assert_eq!(argv(&c), ["commit", "-q", "-m", "seldon: x"]);
        for key in ["GIT_ALLOW_PROTOCOL", "GIT_NO_LAZY_FETCH"] {
            assert!(c.get_envs().all(|(k, _)| k != key), "{key} on a commit");
        }
    }

    /// WP-154: git before 2.44 exits 129 with `unknown option:
    /// --no-lazy-fetch` (or its translation) and the usage; nothing else is
    /// read as that refusal.
    #[test]
    fn an_old_git_refusing_the_option_is_recognised() {
        let exited = |code: i32, stderr: &str| Run::Exited {
            code: Some(code),
            stdout: String::new(),
            stderr: stderr.to_string(),
        };
        // git 2.43's usage names no --no-lazy-fetch; 2.55's does
        let usage = "usage: git [-v | --version] [-h | --help] [-C <path>]\n";
        let usage_2_55 = "usage: git [-v | --version] [-h | --help] [-C <path>] [-c <name>=<value>]
           [--exec-path[=<path>]] [--html-path] [--man-path] [--info-path]
           [-p | --paginate | -P | --no-pager] [--no-replace-objects] [--no-lazy-fetch]
           [--no-optional-locks] [--no-advice] [--bare] [--git-dir=<path>]
           [--work-tree=<path>] [--namespace=<name>] [--config-env=<name>=<envvar>]
           <command> [<args>]
";
        assert!(refuses_no_lazy_fetch(&exited(
            129,
            &format!("option inconnue\u{a0}: --no-lazy-fetch\r\n{usage}")
        )));
        assert!(refuses_no_lazy_fetch(&exited(
            129,
            &format!("unknown option: --no-lazy-fetch\n{usage}")
        )));
        assert!(refuses_no_lazy_fetch(&exited(
            129,
            &format!("Unbekannte Option: --no-lazy-fetch\n{usage}")
        )));
        // another usage error, another exit code, another outcome; git
        // 2.55's real `git --bogus` (exit 129) names the option in its usage
        assert!(!refuses_no_lazy_fetch(&exited(
            129,
            &format!("unknown option: --bogus\n{usage}")
        )));
        assert!(!refuses_no_lazy_fetch(&exited(
            129,
            &format!("unknown option: --bogus\n{usage_2_55}")
        )));
        // the refusal's line with another exit code
        assert!(!refuses_no_lazy_fetch(&exited(
            128,
            "unknown option: --no-lazy-fetch\n"
        )));
        assert!(!refuses_no_lazy_fetch(&exited(0, "")));
        for other in [Run::Cut, Run::NotFound, Run::TimedOut] {
            assert!(!refuses_no_lazy_fetch(&other));
        }
    }

    /// WP-154: a status that says nothing is clean, one over the cap is
    /// not (it printed more than [`sys::OUTPUT_MAX`] of changes), a
    /// failure is an error.
    #[test]
    fn a_status_over_the_cap_has_changes() {
        let status = |stdout: &str| Run::Exited {
            code: Some(0),
            stdout: stdout.to_string(),
            stderr: String::new(),
        };
        assert_eq!(status_is_empty(status("")), Ok(true));
        assert_eq!(status_is_empty(status("\n")), Ok(true));
        assert_eq!(status_is_empty(status(" M STATUS.md\n")), Ok(false));
        assert_eq!(status_is_empty(Run::Cut), Ok(false));
        assert_eq!(
            status_is_empty(Run::Exited {
                code: Some(128),
                stdout: String::new(),
                stderr: "fatal: unable to read tree\n".into(),
            }),
            Err("git status failed: fatal: unable to read tree".to_string())
        );
    }

    /// WP-154 rule 3: the back link of a linked work tree
    /// (`<repo>/worktrees/<name>/gitdir`) byte for byte. git drops only
    /// the CRs and LFs at its end (2.55: a byte order mark or a space
    /// makes the work tree "prunable"); so does the check.
    #[test]
    fn a_linked_work_tree_is_recognised_byte_for_byte() {
        let tmp = std::env::temp_dir().join(format!("seldon-linked-{}", std::process::id()));
        let registered = tmp.join("repo/.git/worktrees/wt");
        let work = tmp.join("wt");
        std::fs::create_dir_all(&registered).unwrap();
        std::fs::create_dir_all(&work).unwrap();
        let dot_git = work.join(".git");
        std::fs::write(&dot_git, format!("gitdir: {}\n", registered.display())).unwrap();
        let own = absolute(&dot_git);
        let path = own.display().to_string();
        for (text, linked) in [
            (format!("{path}\n"), true),
            (format!("{path}\r\n"), true),
            (path.clone(), true),
            (format!("{path}\n\n"), true),
            (format!("\u{FEFF}{path}\n"), false),
            (format!("{path} \n"), false),
            (format!(" {path}\n"), false),
            (format!("{path}x\n"), false),
        ] {
            std::fs::write(registered.join("gitdir"), &text).unwrap();
            assert_eq!(
                is_linked_work_tree_of(&absolute(&registered), &own),
                linked,
                "{text:?}"
            );
        }
        // only a git directory registered under `worktrees/`
        let other = tmp.join("repo/.git/elsewhere/wt");
        std::fs::create_dir_all(&other).unwrap();
        std::fs::write(other.join("gitdir"), format!("{path}\n")).unwrap();
        assert!(!is_linked_work_tree_of(&absolute(&other), &own));
        std::fs::remove_dir_all(&tmp).unwrap();
    }
}
