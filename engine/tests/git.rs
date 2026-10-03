//! The autocommit (WP-061): it commits only into the logbook's own
//! repository whatever git variables `seldon` inherits, skips a detached
//! HEAD, and a commit that fails is one warning line on stderr plus the
//! `error` of `--json` `git`, with exit 0 (the data is written).
//! `tests/doctor.rs` covers what `doctor` says about the same states.

mod common;

use std::path::Path;

use common::{Env, Snapper, json, stderr, stdout};

fn head(env: &Env, repo: &Path) -> String {
    stdout(&env.git(repo, &["rev-parse", "HEAD"]))
        .trim()
        .to_string()
}

fn last_subject(env: &Env, repo: &Path) -> String {
    stdout(&env.git(repo, &["log", "-1", "--format=%s"]))
        .trim()
        .to_string()
}

/// The lines of `stderr` that are git warnings.
fn git_warnings(out: &std::process::Output) -> Vec<String> {
    stderr(out)
        .lines()
        .filter(|l| l.contains("git"))
        .map(str::to_string)
        .collect()
}

#[test]
fn an_inherited_git_dir_does_not_redirect_the_autocommit() {
    let env = Env::new(Snapper::NoPermissions);
    if !env.has_git {
        return;
    }
    let root = env.init_logbook();
    // a second repository with one commit, named by every variable a git
    // hook or a dotfiles helper would export
    let other = env.tmp.path().join("other");
    std::fs::create_dir_all(&other).unwrap();
    env.git(&other, &["init", "-q"]);
    let out = env.git(
        &other,
        &[
            "-c",
            "user.name=Other",
            "-c",
            "user.email=other@example.invalid",
            "commit",
            "-q",
            "--allow-empty",
            "-m",
            "other: first",
        ],
    );
    assert!(out.status.success(), "{}", stderr(&out));
    let other_head = head(&env, &other);
    let logbook_head = head(&env, &root);

    let git_dir = other.join(".git");
    let out = env
        .command(&["--json", "log", "--", "note with an inherited GIT_DIR"])
        .env("GIT_DIR", &git_dir)
        .env("GIT_WORK_TREE", &other)
        .env("GIT_INDEX_FILE", git_dir.join("index"))
        .env("GIT_OBJECT_DIRECTORY", git_dir.join("objects"))
        .env("GIT_COMMON_DIR", &git_dir)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let j = json(&out);
    assert_eq!(j["git"]["committed"], true, "{j}");

    // the logbook got the commit, the other repository nothing
    assert_ne!(head(&env, &root), logbook_head, "the logbook got no commit");
    assert_eq!(last_subject(&env, &root), "seldon: note");
    let status = stdout(&env.git(&root, &["status", "--porcelain"]));
    assert_eq!(status, "", "everything committed in the logbook");
    assert_eq!(head(&env, &other), other_head, "the other repository moved");
    let staged = stdout(&env.git(&other, &["ls-files"]));
    assert_eq!(staged, "", "the other repository's index got logbook files");
}

#[test]
fn a_detached_head_is_not_committed_and_says_so() {
    let env = Env::new(Snapper::NoPermissions);
    if !env.has_git {
        return;
    }
    let root = env.init_logbook();
    let out = env.git(&root, &["checkout", "-q", "--detach"]);
    assert!(out.status.success(), "{}", stderr(&out));
    let before = head(&env, &root);

    let out = env.seldon(&["--json", "log", "--", "note on a detached HEAD"]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let warnings = git_warnings(&out);
    assert_eq!(warnings.len(), 1, "{}", stderr(&out));
    assert!(
        warnings[0].starts_with("seldon: warning: git: not committed: HEAD is detached"),
        "{}",
        warnings[0]
    );
    let j = json(&out);
    assert_eq!(j["git"]["committed"], false, "{j}");
    assert!(
        j["git"]["error"]
            .as_str()
            .unwrap()
            .starts_with("HEAD is detached"),
        "{j}"
    );
    assert_eq!(head(&env, &root), before, "a commit on the detached HEAD");
    // nothing staged either: the note waits in the work tree
    let staged = env.git(&root, &["diff", "--cached", "--quiet"]);
    assert!(staged.status.success(), "the autocommit staged files");
    let status = stdout(&env.git(&root, &["status", "--porcelain"]));
    assert!(status.contains("journal/"), "{status}");

    // back on the branch, the next write commits the waiting note too
    let out = env.git(&root, &["checkout", "-q", "-"]);
    assert!(out.status.success(), "{}", stderr(&out));
    let out = env.seldon(&["log", "--", "back on the branch"]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert!(git_warnings(&out).is_empty(), "{}", stderr(&out));
    let status = stdout(&env.git(&root, &["status", "--porcelain"]));
    assert_eq!(status, "");
}

#[test]
fn a_stale_index_lock_is_one_warning_and_exit_0() {
    let env = Env::new(Snapper::NoPermissions);
    if !env.has_git {
        return;
    }
    let root = env.init_logbook();
    let before = head(&env, &root);
    std::fs::write(root.join(".git/index.lock"), "").unwrap();

    let out = env.seldon(&["--json", "log", "--", "note behind a stale lock"]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let warnings = git_warnings(&out);
    assert_eq!(warnings.len(), 1, "{}", stderr(&out));
    assert!(
        warnings[0].starts_with("seldon: warning: git: not committed: git add failed: "),
        "{}",
        warnings[0]
    );
    assert!(warnings[0].contains("index.lock"), "{}", warnings[0]);
    let j = json(&out);
    assert_eq!(j["git"]["committed"], false, "{j}");
    assert!(
        j["git"]["error"].as_str().unwrap().contains("index.lock"),
        "{j}"
    );
    assert_eq!(head(&env, &root), before);

    // the human output: the same one line on stderr, nothing on stdout
    let out = env.seldon(&["log", "--", "second note behind the lock"]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert_eq!(git_warnings(&out).len(), 1, "{}", stderr(&out));
    assert!(!stdout(&out).contains("not committed"), "{}", stdout(&out));
    assert!(stdout(&out).starts_with("Noted in"), "{}", stdout(&out));

    // the lock gone, the next write commits everything that waited
    std::fs::remove_file(root.join(".git/index.lock")).unwrap();
    let out = env.seldon(&["log", "--", "the lock is gone"]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert!(git_warnings(&out).is_empty(), "{}", stderr(&out));
    assert_eq!(stdout(&env.git(&root, &["status", "--porcelain"])), "");
}
