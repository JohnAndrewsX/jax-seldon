//! `seldon hook session-start` context block.

mod common;

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Output, Stdio};

use common::{Env, Snapper, copy_dir, fixture_logbook, stderr, stdout};

/// The clock of the hook tests (the fixture's C-2026-004 `yay` command).
const NOW: &str = "2026-10-01T10:11:20+02:00";

struct Hooks {
    env: Env,
    #[allow(dead_code)]
    logbook: PathBuf,
}

impl Hooks {
    fn new() -> Self {
        let env = Env::new(Snapper::NoPermissions);
        let logbook = env.init_logbook();
        std::fs::write(env.tmp.path().join("pacman.log"), "").unwrap();
        Hooks { env, logbook }
    }

    fn command(&self, args: &[&str], now: Option<&str>) -> std::process::Command {
        let tmp = self.env.tmp.path();
        let mut cmd = self.env.command(args);
        cmd.env("SELDON_PACMAN_LOG", tmp.join("pacman.log"))
            .env("SELDON_PACMAN_DB_LOCK", tmp.join("no-db.lck"))
            .env("SELDON_OMARCHY", tmp.join("no-omarchy"))
            .env("SELDON_OMARCHY_PLUGINS_DIR", tmp.join("plugins"))
            .env("SELDON_THEME_FILE", tmp.join("theme.name"))
            .env("TZ", "Europe/Berlin");
        if let Some(now) = now {
            cmd.env("SELDON_NOW", now);
        }
        cmd
    }

    /// `seldon <args>` with `input` on stdin.
    fn piped(&self, args: &[&str], input: &str, now: Option<&str>) -> Output {
        let mut child = self
            .command(args, now)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(input.as_bytes())
            .unwrap();
        child.wait_with_output().unwrap()
    }
}

/// Compares with `tests/golden/<name>`; `SELDON_BLESS=1` rewrites it.
fn golden(name: &str, actual: &str) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden")
        .join(name);
    if std::env::var_os("SELDON_BLESS").is_some() {
        std::fs::write(&path, actual).unwrap();
        return;
    }
    let expected = std::fs::read_to_string(&path)
        .unwrap_or_else(|_| panic!("{} missing; run with SELDON_BLESS=1", path.display()));
    assert_eq!(actual, expected, "{} differs", path.display());
}

#[test]
fn session_start_prints_the_context_block() {
    let env = Env::new(Snapper::NoPermissions);
    let logbook = env.tmp.path().join("logbook");
    copy_dir(&fixture_logbook(), &logbook);
    std::fs::write(logbook.join(".seldon/active-case"), "C-2026-004\n").unwrap();
    let out = env
        .command(&[
            "--logbook",
            logbook.to_str().unwrap(),
            "hook",
            "session-start",
        ])
        .env("SELDON_NOW", "2026-10-01T18:00:00+02:00")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert_eq!(stderr(&out), "");
    golden("session-start.txt", &stdout(&out));
}

#[test]
fn session_start_without_status_case_or_journal() {
    let h = Hooks::new();
    let out = h.piped(&["hook", "session-start"], "{}", Some(NOW));
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let text = stdout(&out);
    assert!(text.starts_with("# Seldon logbook context\n"), "{text}");
    assert!(
        text.contains("None (`seldon plan start <id>` sets one)."),
        "{text}"
    );
}

#[test]
fn session_start_survives_a_closed_pipe() {
    let env = Env::new(Snapper::NoPermissions);
    let logbook = env.tmp.path().join("logbook");
    copy_dir(&fixture_logbook(), &logbook);
    let mut child = env
        .command(&[
            "--logbook",
            logbook.to_str().unwrap(),
            "hook",
            "session-start",
        ])
        .env("SELDON_NOW", "2026-10-01T18:00:00+02:00")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    // the reader goes away before the hook writes
    drop(child.stdout.take());
    child.stdin.take().unwrap().write_all(b"{}").unwrap();
    let out = child.wait_with_output().unwrap();
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert!(!stderr(&out).contains("internal error"), "{}", stderr(&out));
}
