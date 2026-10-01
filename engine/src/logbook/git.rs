//! The logbook's git repository (SPEC-LOGBOOK §1). Only `git` with fixed
//! argument lists; commit messages are `seldon: <summary>`.

use std::path::Path;
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

/// `git --version`, or `None` if git is not installed.
pub fn version() -> Option<String> {
    match sys::run("git", &["--version"], None, TIMEOUT) {
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

/// `git add -A` and `git commit -m "seldon: <summary>"` in `root`.
pub fn commit_all(root: &Path, summary: &str) -> Result<(), String> {
    git(root, &["add", "-A"])?;
    let message = format!("seldon: {summary}");
    let has_identity = matches!(
        sys::run("git", &["config", "--get", "user.email"], Some(root), TIMEOUT),
        Run::Exited { code: Some(0), ref stdout, .. } if !stdout.trim().is_empty()
    );
    let mut args: Vec<&str> = Vec::new();
    if !has_identity {
        args.extend(FALLBACK_IDENTITY);
    }
    args.extend(["commit", "-q", "-m", &message]);
    git(root, &args)
}

fn git(root: &Path, args: &[&str]) -> Result<(), String> {
    match sys::run("git", args, Some(root), TIMEOUT) {
        Run::Exited { code: Some(0), .. } => Ok(()),
        Run::Exited { stderr, .. } => Err(format!(
            "git {} failed: {}",
            args.iter()
                .find(|a| !a.starts_with('-') && !a.contains('='))
                .unwrap_or(&""),
            stderr.trim()
        )),
        Run::NotFound => Err("git is not installed".into()),
        Run::TimedOut => Err(format!("git {} timed out", args.join(" "))),
        Run::Failed(e) => Err(format!("cannot run git: {e}")),
    }
}
