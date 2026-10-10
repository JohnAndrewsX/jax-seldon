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

#[test]
fn an_empty_dot_git_inside_another_repository_commits_nowhere() {
    let env = Env::new(Snapper::NoPermissions);
    if !env.has_git {
        return;
    }
    // the logbook sits in a work tree of its own; its .git is emptied
    let root = env.init_logbook_at("parent/logbook", "en");
    let parent = root.parent().unwrap().to_path_buf();
    env.git(&parent, &["init", "-q"]);
    let out = env.git(
        &parent,
        &[
            "-c",
            "user.name=Other",
            "-c",
            "user.email=other@example.invalid",
            "commit",
            "-q",
            "--allow-empty",
            "-m",
            "parent: first",
        ],
    );
    assert!(out.status.success(), "{}", stderr(&out));
    let parent_head = head(&env, &parent);
    std::fs::remove_dir_all(root.join(".git")).unwrap();
    std::fs::create_dir(root.join(".git")).unwrap();

    let out = env.seldon(&["--json", "log", "--", "note with an empty .git"]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let warnings = git_warnings(&out);
    assert_eq!(warnings.len(), 1, "{}", stderr(&out));
    assert!(
        warnings[0].starts_with(
            "seldon: warning: git: not committed: the logbook's .git is not a usable repository"
        ),
        "{}",
        warnings[0]
    );
    let j = json(&out);
    assert_eq!(j["git"]["committed"], false, "{j}");
    assert!(
        j["git"]["error"]
            .as_str()
            .unwrap()
            .starts_with("the logbook's .git is not a usable repository"),
        "{j}"
    );
    // nothing committed or staged in the parent, the empty .git stays empty
    assert_eq!(
        head(&env, &parent),
        parent_head,
        "the parent repository moved"
    );
    assert_eq!(stdout(&env.git(&parent, &["ls-files"])), "");
    assert_eq!(std::fs::read_dir(root.join(".git")).unwrap().count(), 0);

    // the index does not report the parent's HEAD as the logbook's
    let out = env.seldon(&["--json", "index"]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let index: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(json(&out)["index"].as_str().unwrap()).unwrap(),
    )
    .unwrap();
    assert!(
        index["logbook"]["git"]["head"].is_null(),
        "{}",
        index["logbook"]
    );
}

#[test]
fn the_index_reads_the_logbooks_head_under_an_inherited_git_dir() {
    let env = Env::new(Snapper::NoPermissions);
    if !env.has_git {
        return;
    }
    let root = env.init_logbook();
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
    let out = env
        .command(&["--json", "index"])
        .env("GIT_DIR", other.join(".git"))
        .env("GIT_WORK_TREE", &other)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let index: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(json(&out)["index"].as_str().unwrap()).unwrap(),
    )
    .unwrap();
    let logbook_head = head(&env, &root);
    let git = &index["logbook"]["git"];
    assert_eq!(git["head"].as_str(), Some(&logbook_head[..7]), "{git}");
    assert_eq!(git["dirty"], false, "{git}");
}

#[test]
fn a_dot_git_file_pointing_at_another_repository_commits_nowhere() {
    let env = Env::new(Snapper::NoPermissions);
    if !env.has_git {
        return;
    }
    let root = env.init_logbook_at("parent/logbook", "en");
    let parent = root.parent().unwrap().to_path_buf();
    env.git(&parent, &["init", "-q"]);
    let out = env.git(
        &parent,
        &[
            "-c",
            "user.name=Other",
            "-c",
            "user.email=other@example.invalid",
            "commit",
            "-q",
            "--allow-empty",
            "-m",
            "parent: first",
        ],
    );
    assert!(out.status.success(), "{}", stderr(&out));
    let parent_head = head(&env, &parent);
    // the logbook's .git becomes a file that names the parent's git dir:
    // git takes the logbook as the work tree of the parent's repository
    std::fs::remove_dir_all(root.join(".git")).unwrap();
    std::fs::write(
        root.join(".git"),
        format!("gitdir: {}\n", parent.join(".git").display()),
    )
    .unwrap();

    let out = env.seldon(&["--json", "log", "--", "note with a foreign gitdir"]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let warnings = git_warnings(&out);
    assert_eq!(warnings.len(), 1, "{}", stderr(&out));
    assert!(
        warnings[0].starts_with(
            "seldon: warning: git: not committed: the logbook's .git points at another repository's git directory"
        ),
        "{}",
        warnings[0]
    );
    assert_eq!(json(&out)["git"]["committed"], false);
    assert_eq!(
        head(&env, &parent),
        parent_head,
        "the parent repository moved"
    );
    assert_eq!(stdout(&env.git(&parent, &["ls-files"])), "");

    let out = env.seldon(&["--json", "doctor", "--path", root.to_str().unwrap()]);
    let v = json(&out);
    let git = v["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["name"] == "git")
        .unwrap()
        .clone();
    assert_eq!(git["status"], "degraded", "{git}");
    assert!(
        git["message"]
            .as_str()
            .unwrap()
            .contains("points at another repository's git directory"),
        "{git}"
    );
}

#[test]
fn a_linked_work_tree_of_the_logbooks_repository_is_committed() {
    let env = Env::new(Snapper::NoPermissions);
    if !env.has_git {
        return;
    }
    let root = env.init_logbook();
    let linked = env.tmp.path().join("linked");
    let out = env.git(
        &root,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "side",
            linked.to_str().unwrap(),
        ],
    );
    assert!(out.status.success(), "{}", stderr(&out));
    let before = stdout(&env.git(&root, &["rev-parse", "side"]));

    let out = env.seldon(&[
        "--json",
        "--logbook",
        linked.to_str().unwrap(),
        "log",
        "--",
        "note in a linked work tree",
    ]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert!(git_warnings(&out).is_empty(), "{}", stderr(&out));
    assert_eq!(json(&out)["git"]["committed"], true);
    let after = stdout(&env.git(&root, &["rev-parse", "side"]));
    assert_ne!(before, after, "the linked work tree's branch got no commit");
    assert_eq!(stdout(&env.git(&linked, &["status", "--porcelain"])), "");
}

/// `(pid, process group)` from a `/proc/<pid>/stat` line.
fn pid_and_group(stat: &str) -> (i64, i64) {
    let pid = stat.split_whitespace().next().unwrap().parse().unwrap();
    let after = &stat[stat.rfind(')').unwrap() + 1..];
    let group = after.split_whitespace().nth(2).unwrap().parse().unwrap();
    (pid, group)
}

/// WP-061 on top of WP-064: the git commands `logbook::git` builds run in
/// the engine's process group (hooks and a signing prompt may use the
/// terminal), while other programs (here snapper) get their own.
#[test]
fn git_runs_in_the_engines_process_group() {
    use std::os::unix::fs::PermissionsExt as _;
    let env = Env::new(Snapper::NoPermissions);
    if !env.has_git {
        return;
    }
    let root = env.init_logbook();
    let bin = env.tmp.path().join("bin");
    let host_git = std::fs::read_link(bin.join("git")).unwrap();
    let log = env.tmp.path().join("groups.log");
    // remove the link first: writing to it would write the host's git
    std::fs::remove_file(bin.join("git")).unwrap();
    let wrapper = format!(
        "#!/bin/sh\nread -r stat < /proc/$$/stat\nverb=-\nfor a in \"$@\"; do case $a in -*|*=*) ;; *) verb=$a; break ;; esac; done\necho \"git $verb $stat\" >> '{log}'\nexec '{git}' \"$@\"\n",
        log = log.display(),
        git = host_git.display()
    );
    std::fs::write(bin.join("git"), wrapper).unwrap();
    std::fs::set_permissions(bin.join("git"), std::fs::Permissions::from_mode(0o755)).unwrap();
    env.stub(
        "snapper",
        &format!(
            "read -r stat < /proc/$$/stat\necho \"snapper - $stat\" >> '{}'\necho 'No permissions.' >&2; exit 1",
            log.display()
        ),
    );

    let out = env.seldon(&["--json", "log", "--", "note in the engine's group"]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert_eq!(json(&out)["git"]["committed"], true, "{}", stderr(&out));
    let out = env.seldon(&["--json", "doctor", "--path", root.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));

    // seldon inherits this process's group; git must share it
    let (_, engine_group) = pid_and_group(&std::fs::read_to_string("/proc/self/stat").unwrap());
    let calls = std::fs::read_to_string(&log).unwrap();
    let mut git_calls = 0;
    let mut snapper_calls = 0;
    for line in calls.lines() {
        let mut parts = line.splitn(3, ' ');
        let (program, verb, stat) = (
            parts.next().unwrap(),
            parts.next().unwrap(),
            parts.next().unwrap(),
        );
        let (pid, group) = pid_and_group(stat);
        if program == "git" {
            git_calls += 1;
            assert_eq!(group, engine_group, "git {verb} ran in its own group");
        } else {
            snapper_calls += 1;
            assert_ne!(group, engine_group, "snapper ran in the engine's group");
            assert_eq!(group, pid, "snapper leads its own group");
        }
    }
    assert!(calls.contains("git rev-parse "), "{calls}");
    assert!(calls.contains("git add "), "{calls}");
    assert!(git_calls > 3 && snapper_calls > 0, "{calls}");
}

// WP-154: the rules for every git call. A stdout or stderr flood is
// capped, a read-only query never reaches the network, and a commit keeps
// the user's hooks and signing.

/// `program` on this test process's PATH (the environment's PATH is its
/// stub directory alone).
fn host_program(program: &str) -> Option<std::path::PathBuf> {
    std::env::var("PATH")
        .unwrap_or_default()
        .split(':')
        .map(|dir| Path::new(dir).join(program))
        .find(|p| p.is_file())
}

/// The doctor's `git` check.
fn git_check(env: &Env, root: &Path) -> serde_json::Value {
    let out = env.seldon(&["--json", "doctor", "--path", root.to_str().unwrap()]);
    json(&out)["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["name"] == "git")
        .cloned()
        .unwrap_or_else(|| panic!("no git check: {}", stdout(&out)))
}

/// A git that writes 100 MB to stderr costs the engine 1 MiB of it: the
/// doctor's message, git's stderr on one line, stops there.
#[test]
fn a_hundred_megabytes_from_git_are_capped() {
    let env = Env::new(Snapper::NoPermissions);
    let (Some(yes), Some(head)) = (host_program("yes"), host_program("head")) else {
        return;
    };
    if !env.has_git {
        return;
    }
    let root = env.init_logbook();
    env.wrap_git(&format!(
        "case \"$*\" in\n*--show-toplevel*) '{}' x | '{}' -c 100000000 >&2; exit 128 ;;\nesac\nexec \"$REAL_GIT\" \"$@\"",
        yes.display(),
        head.display()
    ));
    let check = git_check(&env, &root);
    let message = check["message"].as_str().unwrap();
    assert!(
        message.contains("not a usable repository: x x x"),
        "{}",
        &message[..message.len().min(200)]
    );
    // one line of "x x x …" for git's 1 MiB of "x\n"
    let mib = 1024 * 1024;
    assert!(
        (mib - 64..mib + 1024).contains(&message.len()),
        "{} bytes",
        message.len()
    );
}

/// A git before 2.44 refuses `--no-lazy-fetch`: the engine asks once more
/// without it, once per process, and every answer is the same.
#[test]
fn a_git_without_no_lazy_fetch_still_answers() {
    let env = Env::new(Snapper::NoPermissions);
    if !env.has_git {
        return;
    }
    let root = env.init_logbook();
    let calls = env.tmp.path().join("git-calls.log");
    env.wrap_git(&format!(
        "printf '%s\\n' \"$*\" >> '{}'\nif [ \"$1\" = --no-lazy-fetch ]; then\n  echo 'unknown option: --no-lazy-fetch' >&2\n  echo 'usage: git [-v | --version] [-h | --help] [-C <path>]' >&2\n  exit 129\nfi\nexec \"$REAL_GIT\" \"$@\"",
        calls.display()
    ));
    let out = env.at("2026-10-08T09:00:00+02:00", &["--json", "status"]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let index: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(env.home.join(".local/state/seldon/index.json")).unwrap(),
    )
    .unwrap();
    let git = &index["logbook"]["git"];
    assert_eq!(git["head"].as_str(), Some(&head(&env, &root)[..7]), "{git}");
    assert!(git["dirty"].is_boolean(), "{git}");
    let log = std::fs::read_to_string(&calls).unwrap();
    let tried = log
        .lines()
        .filter(|l| l.starts_with("--no-lazy-fetch"))
        .count();
    assert_eq!(tried, 1, "{log}");
    assert!(log.contains("\nstatus --porcelain\n"), "{log}");
    // doctor: one more process, the same rule
    std::fs::remove_file(&calls).unwrap();
    let check = git_check(&env, &root);
    assert_eq!(check["status"], "ok", "{check}");
    let log = std::fs::read_to_string(&calls).unwrap();
    let tried = log
        .lines()
        .filter(|l| l.starts_with("--no-lazy-fetch"))
        .count();
    assert_eq!(tried, 1, "{log}");
}

/// A logbook that is a partial clone, its promisor remote an `ext::`
/// command that would leave a marker, the clone's own config allowing that
/// transport, and HEAD's tree missing: `seldon status` never starts the
/// fetch. With the query rules off, `git status` would.
#[test]
fn a_partial_clone_logbook_never_fetches_during_status() {
    let env = Env::new(Snapper::NoPermissions);
    if !env.has_git {
        return;
    }
    let root = env.init_logbook();
    let now = "2026-10-08T09:00:00+02:00";
    // the first status writes its files and commits them; the second has
    // nothing to commit, so only read-only queries run after the break
    let out = env.at(now, &["--json", "status"]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let out = env.at(now, &["--json", "status"]);
    assert_eq!(json(&out)["git"]["committed"], false, "{}", stdout(&out));
    let tree = stdout(&env.git(&root, &["rev-parse", "HEAD^{tree}"]))
        .trim()
        .to_string();
    let marker = env.tmp.path().join("fetched");
    let fetch = env.tmp.path().join("fetch.sh");
    common::write_executable(
        &fetch,
        &format!("#!/bin/sh\n: > '{}'\nexit 1\n", marker.display()),
    );
    for (key, value) in [
        ("core.repositoryformatversion", "1".to_string()),
        ("extensions.partialClone", "origin".to_string()),
        ("remote.origin.url", format!("ext::{}", fetch.display())),
        ("remote.origin.promisor", "true".to_string()),
        ("protocol.ext.allow", "always".to_string()),
    ] {
        let out = env.git(&root, &["config", key, &value]);
        assert!(out.status.success(), "{}", stderr(&out));
    }
    // from here on the test's own git is never run (it would fetch)
    let object = root.join(".git/objects").join(&tree[..2]).join(&tree[2..]);
    std::fs::remove_file(&object).unwrap();

    let out = env.at(now, &["--json", "status"]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert!(!marker.exists(), "git ran the promisor's ext:: command");
    assert_eq!(json(&out)["git"]["committed"], false, "{}", stdout(&out));
    // the doctor's read-only queries do not fetch either
    let _ = git_check(&env, &root);
    assert!(
        !marker.exists(),
        "doctor: git ran the promisor's ext:: command"
    );
    // nor the import's check for pending changes (`git::is_dirty`): git
    // cannot read HEAD's tree, so the import refuses before any write
    let vault = env.home.join("vault");
    common::copy_dir(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("../fixtures/vaults/omarchy-agent"),
        &vault,
    );
    let out = env.at(
        now,
        &[
            "--json",
            "import",
            "omarchy-agent",
            vault.to_str().unwrap(),
            "--apply",
        ],
    );
    assert!(
        stdout(&out).contains("cannot read the logbook's git status"),
        "{}{}",
        stdout(&out),
        stderr(&out)
    );
    assert!(
        !marker.exists(),
        "import: git ran the promisor's ext:: command"
    );
}

/// The autocommit is the user's commit: their global config signs it with
/// their signing program, their hooks run, and neither sees the query
/// rules (a hook may fetch).
#[test]
fn commits_still_sign_and_run_hooks_as_configured() {
    let env = Env::new(Snapper::NoPermissions);
    if !env.has_git {
        return;
    }
    let root = env.init_logbook();
    let signer = env.tmp.path().join("signer");
    let signed = env.tmp.path().join("signed.log");
    // git's contract with gpg.program: the payload on stdin, the detached
    // signature on stdout, SIG_CREATED on the status fd (2); no external
    // program is needed (PATH is the stub directory)
    common::write_executable(
        &signer,
        &format!(
            "#!/bin/sh\nprintf '%s\\n' \"$*\" >> '{}'\nwhile IFS= read -r l || [ -n \"$l\" ]; do :; done\nprintf '\\n[GNUPG:] SIG_CREATED D 1 8 00 1759900000 0000\\n' >&2\nprintf '%s\\n' '-----BEGIN PGP SIGNATURE-----' '' 'c2VsZG9uLXRlc3Q=' '-----END PGP SIGNATURE-----'\n",
            signed.display()
        ),
    );
    let hooks = env.tmp.path().join("hooks");
    let ran = env.tmp.path().join("hooks.log");
    for hook in ["pre-commit", "post-commit"] {
        common::write_executable(
            &hooks.join(hook),
            &format!(
                "#!/bin/sh\necho \"{hook} ${{GIT_ALLOW_PROTOCOL-unset}} ${{GIT_NO_LAZY_FETCH-unset}}\" >> '{}'\n",
                ran.display()
            ),
        );
    }
    std::fs::write(
        env.home.join(".gitconfig"),
        format!(
            "[user]\n\tname = Logbook Owner\n\temail = owner@example.invalid\n[commit]\n\tgpgSign = true\n[gpg]\n\tprogram = {}\n[core]\n\thooksPath = {}\n",
            signer.display(),
            hooks.display()
        ),
    )
    .unwrap();

    let out = env.seldon(&["--json", "log", "--", "a signed note"]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert_eq!(json(&out)["git"]["committed"], true, "{}", stdout(&out));
    let commit = stdout(&env.git(&root, &["cat-file", "commit", "HEAD"]));
    assert!(
        commit.contains("\ngpgsig -----BEGIN PGP SIGNATURE-----\n"),
        "{commit}"
    );
    assert!(commit.contains("\n\nseldon: note"), "{commit}");
    let signs = std::fs::read_to_string(&signed).unwrap();
    assert!(signs.contains("-bsau"), "{signs}");
    assert_eq!(
        std::fs::read_to_string(&ran).unwrap(),
        "pre-commit unset unset\npost-commit unset unset\n"
    );
}

/// WP-154 round 2 (B1): rule 2 for each caller. A wrapper `git` logs
/// every call the engine makes with both query variables: through `log`,
/// `status`, a `capture` that upgrades the agent rules (`is_clean_path`,
/// then a commit of `AGENTS.md` alone), `doctor`, and `doctor` on a
/// detached HEAD (`branches`). Every call but `add` and `commit` is a
/// query: `--no-lazy-fetch` first, `GIT_ALLOW_PROTOCOL=none`,
/// `GIT_NO_LAZY_FETCH=1`; `add` and `commit` see neither.
#[test]
fn every_git_call_but_add_and_commit_is_a_query_without_network() {
    let env = Env::new(Snapper::Allowed);
    if !env.has_git {
        return;
    }
    let root = env.init_logbook();
    // no maintenance child of a commit (it would be git's call, not ours)
    std::fs::write(
        env.home.join(".gitconfig"),
        "[user]\n\tname = Logbook Owner\n\temail = owner@example.invalid\n[maintenance]\n\tauto = false\n[gc]\n\tauto = 0\n",
    )
    .unwrap();
    // the agent rules as 0.1.3 shipped them: the capture upgrades them
    let rules = root.join("AGENTS.md");
    std::fs::write(
        &rules,
        common::read(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("templates/rules-v3/AGENTS-wp111-en.md"),
        ),
    )
    .unwrap();
    let out = env.git(&root, &["commit", "-qam", "rules v3"]);
    assert!(out.status.success(), "{}", stderr(&out));
    let calls = env.tmp.path().join("git-calls.log");
    env.wrap_git(&format!(
        "printf '%s %s | %s\\n' \"${{GIT_ALLOW_PROTOCOL-unset}}\" \"${{GIT_NO_LAZY_FETCH-unset}}\" \"$*\" >> '{}'\nexec \"$REAL_GIT\" \"$@\"",
        calls.display()
    ));

    let now = "2026-10-08T09:00:00+02:00";
    let out = env.at(now, &["--json", "log", "--", "a note"]);
    assert_eq!(json(&out)["git"]["committed"], true, "{}", stdout(&out));
    let out = env.at(now, &["--json", "status"]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let out = env
        .command(&["--json", "capture", "--all"])
        .env("SELDON_NOW", now)
        .env("SELDON_PACMAN_LOG", env.tmp.path().join("pacman.log"))
        .env("SELDON_PACMAN_DB_LOCK", env.tmp.path().join("no-db.lck"))
        .env("SELDON_OMARCHY_PLUGINS_DIR", env.tmp.path().join("plugins"))
        .env("SELDON_THEME_FILE", env.tmp.path().join("theme.name"))
        .env("SELDON_HARDWARE_ROOT", common::hardware_root())
        .output()
        .unwrap();
    let v = json(&out);
    assert_eq!(v["rulesUpdated"]["git"]["committed"], true, "{v}");
    assert_eq!(git_check(&env, &root)["status"], "ok");
    let out = env.git(&root, &["checkout", "-q", "--detach"]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(git_check(&env, &root)["status"], "degraded");

    let log = std::fs::read_to_string(&calls).unwrap();
    let mut verbs = std::collections::BTreeSet::new();
    for line in log.lines() {
        let (vars, argv) = line.split_once(" | ").unwrap();
        let args: Vec<&str> = argv.split(' ').collect();
        // the test's own `checkout --detach` went through the wrapper too
        if args[0] == "checkout" {
            continue;
        }
        let verb = args
            .iter()
            .copied()
            .find(|a| !a.starts_with('-') && !a.contains('='));
        if verb.is_some_and(|v| ["add", "commit"].contains(&v)) {
            assert_eq!(vars, "unset unset", "{line}");
            assert_ne!(args[0], "--no-lazy-fetch", "{line}");
            verbs.insert(verb.unwrap().to_string());
        } else {
            assert_eq!(vars, "none 1", "{line}");
            assert_eq!(args[0], "--no-lazy-fetch", "{line}");
            verbs.insert(args[1].to_string());
        }
    }
    // the queries the WP names, the five more the handover names (but
    // `show-ref`, asked on an unborn branch only), and the two writes
    for verb in [
        "--version",
        "rev-parse",
        "status",
        "symbolic-ref",
        "for-each-ref",
        "var",
        "config",
        "diff",
        "add",
        "commit",
    ] {
        assert!(verbs.contains(verb), "no {verb} in\n{log}");
    }
    for query in [
        "rev-parse --show-toplevel --absolute-git-dir",
        "rev-parse --verify -q HEAD",
        "rev-parse --short HEAD",
        "status --porcelain -- AGENTS.md",
        "status --porcelain\n",
        "for-each-ref --format=%(refname:short) refs/heads/",
    ] {
        assert!(
            log.contains(&format!("--no-lazy-fetch {query}")),
            "no {query:?} in\n{log}"
        );
    }
}

/// The processes whose working directory is `dir` or below it, as
/// `<pid> <comm>`; processes of other users (no `cwd` to read) and those
/// that end while they are read are left out.
fn processes_in(dir: &Path) -> Vec<String> {
    let mut found = Vec::new();
    for entry in std::fs::read_dir("/proc").unwrap().flatten() {
        let pid = entry.file_name();
        if !pid.to_string_lossy().bytes().all(|b| b.is_ascii_digit()) {
            continue;
        }
        let Ok(cwd) = std::fs::read_link(entry.path().join("cwd")) else {
            continue;
        };
        if cwd.starts_with(dir) {
            let comm = std::fs::read_to_string(entry.path().join("comm")).unwrap_or_default();
            found.push(format!("{} {}", pid.to_string_lossy(), comm.trim()));
        }
    }
    found
}

/// WP-199: `init` (its first commit) starts no git that outlives it. git
/// 2.55's `commit` runs `git maintenance run --auto --detach`, which
/// leaves the commit at once and creates and removes
/// `.git/objects/maintenance.lock` after `init` has returned (a test that
/// removed `.git` right after `init` failed on it under load). git's own
/// trace (`GIT_TRACE2`, every git process `seldon` starts and their
/// children) shows no `maintenance` and no `gc` child, and no process has
/// its working directory in the logbook when `init` returns. Three
/// logbooks, three chances to see a straggler.
#[test]
fn init_leaves_no_git_running_in_the_logbook() {
    let env = Env::new(Snapper::NoPermissions);
    if !env.has_git {
        return;
    }
    let trace = env.tmp.path().join("git-trace2.log");
    for n in 0..3 {
        let root = env.tmp.path().join(format!("logbook-{n}"));
        let out = env
            .command(&[
                "init",
                "--non-interactive",
                "--no-capture",
                "--path",
                root.to_str().unwrap(),
            ])
            .env("GIT_TRACE2", &trace)
            .env("GIT_TRACE2_BRIEF", "1")
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        let left = processes_in(&root.canonicalize().unwrap());
        assert!(left.is_empty(), "still running in the logbook: {left:?}");
    }
    let log = std::fs::read_to_string(&trace).unwrap();
    // the trace saw the commits: the check below is not empty
    assert!(log.contains("cmd_name commit "), "{log}");
    let background: Vec<&str> = log
        .lines()
        .filter(|l| l.starts_with("child_start"))
        .filter(|l| l.contains(" maintenance ") || l.contains(" gc "))
        .collect();
    assert!(background.is_empty(), "{background:?}\n{log}");
}
