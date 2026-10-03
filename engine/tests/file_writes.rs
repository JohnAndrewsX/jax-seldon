//! How the engine writes files and runs programs (WP-064): `sys::write_atomic`
//! follows a symbolic link to its target and keeps the target's mode, new
//! files are 0600 and new directories 0700 whatever the umask, a failed
//! write leaves no temp file, and the timeout of `sys::run` covers the
//! output pipes and the child's whole process group.

mod common;

use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use common::{Env, Snapper, TempDir, mode, read, stderr, with_umask};
use seldon::sys::{self, Run};

/// Names in `dir` that look like a `write_atomic` temp file.
fn temp_files(dir: &Path) -> Vec<String> {
    std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n.contains(".tmp-"))
        .collect()
}

fn set_mode(path: &Path, mode: u32) {
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).unwrap();
}

mod atomic {
    use super::*;

    #[test]
    fn a_symlinked_file_is_written_through() {
        let tmp = TempDir::new("write-link");
        let dotfiles = tmp.path().join("dotfiles");
        std::fs::create_dir(&dotfiles).unwrap();
        let real = dotfiles.join("settings.json");
        std::fs::write(&real, "old\n").unwrap();
        set_mode(&real, 0o600);
        let link = tmp.path().join("settings.json");
        std::os::unix::fs::symlink("dotfiles/settings.json", &link).unwrap();

        sys::write_atomic(&link, b"new\n").unwrap();

        assert!(link.symlink_metadata().unwrap().file_type().is_symlink());
        assert_eq!(
            std::fs::read_link(&link).unwrap(),
            Path::new("dotfiles/settings.json")
        );
        assert_eq!(read(&real), "new\n");
        assert_eq!(mode(&real), 0o600);
        assert!(temp_files(tmp.path()).is_empty());
        assert!(temp_files(&dotfiles).is_empty());
    }

    #[test]
    fn a_link_to_a_missing_file_creates_the_target() {
        let tmp = TempDir::new("write-dangling");
        let link = tmp.path().join("config.toml");
        std::os::unix::fs::symlink("real/config.toml", &link).unwrap();

        sys::write_atomic(&link, b"x = 1\n").unwrap();

        assert!(link.symlink_metadata().unwrap().file_type().is_symlink());
        let real = tmp.path().join("real/config.toml");
        assert_eq!(read(&real), "x = 1\n");
        assert_eq!(mode(&real), 0o600);
        assert_eq!(mode(&tmp.path().join("real")), 0o700);
    }

    #[test]
    fn a_link_loop_is_an_error() {
        let tmp = TempDir::new("write-loop");
        let (a, b) = (tmp.path().join("a"), tmp.path().join("b"));
        std::os::unix::fs::symlink("b", &a).unwrap();
        std::os::unix::fs::symlink("a", &b).unwrap();

        assert!(sys::write_atomic(&a, b"x").is_err());
        assert!(a.symlink_metadata().unwrap().file_type().is_symlink());
        assert!(temp_files(tmp.path()).is_empty());
    }

    #[test]
    fn the_mode_of_an_existing_file_is_kept() {
        let tmp = TempDir::new("write-mode");
        // 0664 and 0640 differ from what a 022 or 077 umask gives a new file
        for m in [0o600, 0o640, 0o664, 0o755] {
            let path = tmp.path().join(format!("file-{m:o}"));
            std::fs::write(&path, "old").unwrap();
            set_mode(&path, m);
            sys::write_atomic(&path, b"new").unwrap();
            assert_eq!(read(&path), "new");
            assert_eq!(mode(&path), m, "{}", path.display());
        }
    }

    #[test]
    fn a_new_file_and_its_new_directories_are_private() {
        let tmp = TempDir::new("write-new");
        let path = tmp.path().join("state/seldon/cursors.json");

        sys::write_atomic(&path, b"{}").unwrap();

        assert_eq!(mode(&path), 0o600);
        assert_eq!(mode(&tmp.path().join("state")), 0o700);
        assert_eq!(mode(&tmp.path().join("state/seldon")), 0o700);
    }

    #[test]
    fn an_explicit_mode_is_set_exactly() {
        let tmp = TempDir::new("write-explicit");
        let path = tmp.path().join("hook");
        std::fs::write(&path, "old").unwrap();
        set_mode(&path, 0o600);

        sys::write_atomic_mode(&path, b"#!/bin/sh\n", 0o755).unwrap();

        assert_eq!(mode(&path), 0o755);
    }

    #[test]
    fn a_failed_replace_leaves_no_temp_file() {
        let tmp = TempDir::new("write-fail");
        // a non-empty directory at the path: the rename fails
        let path = tmp.path().join("STATUS.md");
        std::fs::create_dir(&path).unwrap();
        std::fs::write(path.join("keep"), "x").unwrap();

        assert!(sys::write_atomic(&path, b"text").is_err());
        assert!(path.is_dir());
        assert!(
            temp_files(tmp.path()).is_empty(),
            "{:?}",
            temp_files(tmp.path())
        );
    }
}

mod run {
    use super::*;

    /// Whether process `pid` has ended (gone, or a zombie nobody reaped
    /// yet), waiting up to a second.
    fn ended(pid: &str) -> bool {
        let stat = PathBuf::from(format!("/proc/{pid}/stat"));
        let deadline = Instant::now() + Duration::from_secs(1);
        loop {
            match std::fs::read_to_string(&stat) {
                Err(_) => return true,
                Ok(s) if s.rsplit_once(") ").is_some_and(|(_, r)| r.starts_with('Z')) => {
                    return true;
                }
                Ok(_) if Instant::now() >= deadline => return false,
                Ok(_) => std::thread::sleep(Duration::from_millis(20)),
            }
        }
    }

    #[test]
    fn a_helper_that_keeps_the_pipe_open_is_stopped_at_the_deadline() {
        let tmp = TempDir::new("run-pipe");
        let pid_file = tmp.path().join("pid");
        let script = format!("sleep 5 & echo $! > '{}'; echo done", pid_file.display());
        let started = Instant::now();

        let r = sys::run("sh", &["-c", &script], None, Duration::from_secs(1));

        let took = started.elapsed();
        assert!(took < Duration::from_millis(1800), "took {took:?}");
        assert_eq!(
            r,
            Run::Exited {
                code: Some(0),
                stdout: "done\n".into(),
                stderr: String::new()
            }
        );
        let pid = read(&pid_file);
        assert!(ended(pid.trim()), "the background helper still runs");
    }

    #[test]
    fn a_timeout_stops_the_whole_process_group() {
        let tmp = TempDir::new("run-group");
        let pid_file = tmp.path().join("pid");
        let script = format!("sleep 5 & echo $! > '{}'; sleep 5", pid_file.display());
        let started = Instant::now();

        let r = sys::run("sh", &["-c", &script], None, Duration::from_millis(500));

        let took = started.elapsed();
        assert_eq!(r, Run::TimedOut);
        assert!(took < Duration::from_millis(1500), "took {took:?}");
        let pid = read(&pid_file);
        assert!(ended(pid.trim()), "the background helper still runs");
    }

    #[test]
    fn a_quick_program_keeps_its_output() {
        let r = sys::run(
            "sh",
            &["-c", "printf 'a\\nb\\n'; echo e >&2"],
            None,
            Duration::from_secs(5),
        );
        assert_eq!(
            r,
            Run::Exited {
                code: Some(0),
                stdout: "a\nb\n".into(),
                stderr: "e\n".into()
            }
        );
    }
}

mod cli {
    use super::*;

    /// Every directory under `dir` is 0700 and every file 0600, except
    /// `.git` (git writes it) and the paths `other` names: files and
    /// directories the engine creates without `sys::write_atomic` or the
    /// logbook layout (the lock file and its directories, the ledger's
    /// month files), whose modes are not part of WP-064.
    fn assert_private_tree(dir: &Path, other: &dyn Fn(&Path) -> bool) {
        if !other(dir) {
            assert_eq!(mode(dir), 0o700, "{}", dir.display());
        }
        for entry in std::fs::read_dir(dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if entry.file_name() == ".git" {
                continue;
            }
            if entry.file_type().unwrap().is_dir() {
                assert_private_tree(&path, other);
            } else if !other(&path) {
                assert_eq!(mode(&path), 0o600, "{}", path.display());
            }
        }
    }

    #[test]
    fn init_creates_private_directories_and_files_under_any_umask() {
        for umask in ["022", "000"] {
            let env = Env::new(Snapper::NoPermissions);
            let root = env.tmp.path().join("logbook");
            let out = with_umask(
                &env.command(&[
                    "init",
                    "--non-interactive",
                    "--path",
                    root.to_str().unwrap(),
                ]),
                umask,
            )
            .output()
            .unwrap();
            assert_eq!(
                out.status.code(),
                Some(0),
                "umask {umask}: {}",
                stderr(&out)
            );

            let ledger = root.join("ledger");
            assert_private_tree(&root, &|p| {
                p.parent() == Some(ledger.as_path()) && p.extension().is_some_and(|e| e == "jsonl")
            });
            // what the engine created under the home: config and state
            assert_private_tree(&env.home.join(".config"), &|_| false);
            let state = env.home.join(".local/state/seldon");
            assert_private_tree(&env.home.join(".local"), &|p| {
                state.starts_with(p) || p == state.join("lock")
            });
            for file in ["index.json", "cursors.json"] {
                assert_eq!(mode(&state.join(file)), 0o600, "{file}");
            }
        }
    }

    #[test]
    fn a_new_logbook_ignores_temp_files() {
        let env = Env::new(Snapper::NoPermissions);
        let root = env.init_logbook();
        let ignore = read(&root.join(".gitignore"));
        assert!(ignore.lines().any(|l| l == ".*.tmp-*"), "{ignore}");
        if env.has_git {
            std::fs::write(root.join(".STATUS.md.tmp-4242"), "x").unwrap();
            std::fs::write(root.join("ledger/.2026-10.jsonl.tmp-7"), "x").unwrap();
            let out = env.git(&root, &["status", "--porcelain", "--ignored=no"]);
            assert_eq!(common::stdout(&out), "");
        }
    }

    #[test]
    fn hook_install_writes_through_a_symlinked_settings_file() {
        let env = Env::new(Snapper::NoPermissions);
        env.init_logbook();
        let dotfiles = env.tmp.path().join("dotfiles");
        std::fs::create_dir(&dotfiles).unwrap();
        let real = dotfiles.join("settings.json");
        std::fs::write(
            &real,
            "{\n  \"env\": { \"TOKEN\": \"made-up\" },\n  \"model\": \"opus\"\n}\n",
        )
        .unwrap();
        set_mode(&real, 0o600);
        let link = env.tmp.path().join("project/.claude/settings.json");
        std::fs::create_dir_all(link.parent().unwrap()).unwrap();
        std::os::unix::fs::symlink(&real, &link).unwrap();

        let out = with_umask(
            &env.command(&[
                "hook",
                "install",
                "claude-code",
                "--settings",
                link.to_str().unwrap(),
            ]),
            "022",
        )
        .output()
        .unwrap();
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));

        assert!(link.symlink_metadata().unwrap().file_type().is_symlink());
        assert_eq!(std::fs::read_link(&link).unwrap(), real);
        let text = read(&real);
        assert!(text.contains("seldon hook claude-code"), "{text}");
        assert!(text.contains("made-up"), "{text}");
        assert_eq!(mode(&real), 0o600);
        assert!(temp_files(&dotfiles).is_empty());
        assert!(temp_files(link.parent().unwrap()).is_empty());
    }
}
