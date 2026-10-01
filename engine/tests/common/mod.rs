//! Shared test helpers: a throw-away home directory and a `seldon` runner
//! that never sees the real `~/.config`, `~/.local/state` or logbook
//! (AGENTS.md §6).
#![allow(dead_code)] // each test binary uses a different subset

use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

/// `fixtures/logbook/` — read-only reference logbook (WP-002).
pub fn fixture_logbook() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../fixtures/logbook")
}

/// A temporary directory, removed on drop.
pub struct TempDir(PathBuf);

impl TempDir {
    pub fn new(tag: &str) -> Self {
        static N: AtomicUsize = AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!(
            "seldon-test-{tag}-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        TempDir(dir)
    }

    pub fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// What the stubbed `snapper` does.
#[derive(Clone, Copy)]
pub enum Snapper {
    /// Exit 1, `No permissions.` on stderr (the Omarchy default, ADR-0011).
    NoPermissions,
    /// Prints a JSON list with two snapshots plus `current`.
    Allowed,
    /// Not on PATH.
    Missing,
}

/// A fake home with XDG dirs and stub binaries first on PATH.
pub struct Env {
    pub tmp: TempDir,
    pub home: PathBuf,
    bin: PathBuf,
    /// Whether the host has git (git tests are skipped without it).
    pub has_git: bool,
}

impl Env {
    pub fn new(snapper: Snapper) -> Self {
        let tmp = TempDir::new("env");
        let home = tmp.path().join("home");
        let bin = tmp.path().join("bin");
        std::fs::create_dir_all(&home).unwrap();
        std::fs::create_dir_all(&bin).unwrap();
        stub(&bin, "omarchy-version", "echo 4.0.4-1");
        match snapper {
            Snapper::NoPermissions => stub(&bin, "snapper", "echo 'No permissions.' >&2; exit 1"),
            Snapper::Allowed => stub(
                &bin,
                "snapper",
                r#"echo '{"root":[{"number":0,"description":"current"},{"number":1},{"number":2}]}'"#,
            ),
            Snapper::Missing => {}
        }
        // PATH is this directory only: the stubs decide what snapper and
        // omarchy-version do, whatever the host has installed.
        let git = host_git();
        if let Some(g) = &git {
            std::os::unix::fs::symlink(g, bin.join("git")).unwrap();
        }
        Env {
            tmp,
            home,
            bin,
            has_git: git.is_some(),
        }
    }

    /// `seldon args…` in this environment.
    pub fn seldon(&self, args: &[&str]) -> Output {
        self.command(args).output().expect("run seldon")
    }

    pub fn command(&self, args: &[&str]) -> Command {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_seldon"));
        cmd.args(args)
            .env_clear()
            .env("HOME", &self.home)
            .env("PATH", &self.bin)
            .env("LANG", "C")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .current_dir(self.tmp.path());
        cmd
    }

    /// `git args…` in `dir`, with this environment's HOME and PATH.
    pub fn git(&self, dir: &Path, args: &[&str]) -> Output {
        Command::new(self.bin.join("git"))
            .args(args)
            .env_clear()
            .env("HOME", &self.home)
            .env("PATH", &self.bin)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .current_dir(dir)
            .output()
            .expect("run git")
    }

    pub fn config_file(&self) -> PathBuf {
        self.home.join(".config/seldon/config.toml")
    }

    pub fn lock_file(&self) -> PathBuf {
        self.home.join(".local/state/seldon/lock")
    }
}

/// The host's `git`, if any.
fn host_git() -> Option<PathBuf> {
    let path = std::env::var("PATH").unwrap_or_default();
    path.split(':')
        .map(|dir| Path::new(dir).join("git"))
        .find(|p| p.is_file())
}

fn stub(bin: &Path, name: &str, body: &str) {
    let path = bin.join(name);
    std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

pub fn stdout(out: &Output) -> String {
    String::from_utf8(out.stdout.clone()).expect("utf-8 stdout")
}

pub fn stderr(out: &Output) -> String {
    String::from_utf8(out.stderr.clone()).expect("utf-8 stderr")
}

pub fn json(out: &Output) -> serde_json::Value {
    serde_json::from_str(&stdout(out)).unwrap_or_else(|e| {
        panic!(
            "stdout is not JSON ({e}): {}\nstderr: {}",
            stdout(out),
            stderr(out)
        )
    })
}
