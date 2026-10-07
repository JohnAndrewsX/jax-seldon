//! Shared test helpers: a throw-away home directory and a `seldon` runner
//! that never sees the real `~/.config`, `~/.local/state` or logbook
//! (AGENTS.md §6).
#![allow(dead_code)] // each test binary uses a different subset

pub mod scale;

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
    /// Exit 1, `No permissions.` on stderr (the Omarchy default, ADR-0026).
    NoPermissions,
    /// [`Snapper::NoPermissions`] as snapper translates it: German
    /// `Keine Berechtigungen.` unless `LC_ALL` is `C` (issue #1).
    NoPermissionsLocalized,
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
        // made here, not on first use: tests compare the temp tree before
        // and after a command
        let probe = tmp.path().join("user-probe");
        std::fs::write(&probe, "").unwrap();
        user_owned(&probe);
        stub(&bin, "omarchy-version", "echo 4.0.4-1");
        match snapper {
            Snapper::NoPermissions => stub(&bin, "snapper", "echo 'No permissions.' >&2; exit 1"),
            Snapper::NoPermissionsLocalized => stub(
                &bin,
                "snapper",
                "if [ \"$LC_ALL\" = C ]; then echo 'No permissions.' >&2; \
                 else echo 'Keine Berechtigungen.' >&2; fi; exit 1",
            ),
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
            // the engine refuses to run when its dirs leave the temp dir
            .env("SELDON_TEST_GUARD", self.tmp.path())
            // Omarchy's package lists (dossier, WP-036): the fixture copies,
            // never the host's `/usr/share/omarchy`
            .env("SELDON_OMARCHY_PACKAGES", omarchy_packages())
            // the silent upgrades run only for a user (sys::runner); CI runs
            // the tests as root, so the probe is a file a user owns
            .env("SELDON_TEST_ROOT_PROBE", self.user_probe())
            .current_dir(self.tmp.path());
        cmd
    }

    /// A file in the temp dir that a user (not root) owns: the default
    /// `SELDON_TEST_ROOT_PROBE` of [`Env::command`].
    pub fn user_probe(&self) -> PathBuf {
        self.tmp.path().join("user-probe")
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

    /// Adds a stub program to this environment's PATH.
    pub fn stub(&self, name: &str, body: &str) {
        stub(&self.bin, name, body);
    }

    /// Shims for the dossier's read-only host queries (WP-035): the package
    /// manager, `systemctl` and `omarchy` print `fixtures/logs/` files for
    /// exactly the query argument lists and fail (exit 64) for anything
    /// else; `omarchy-version` prints the fixture's 4.0.7-1. Each call's
    /// arguments are appended to the returned file, one line per call, so
    /// a test can prove that only queries ran. No real program runs: PATH
    /// is the stub directory only.
    pub fn query_shims(&self) -> PathBuf {
        let calls = self.tmp.path().join("query-calls.log");
        let logs = Path::new(env!("CARGO_MANIFEST_DIR")).join("../fixtures/logs");
        let shim = |name: &str, cases: &[(&str, &str)]| {
            let mut body = format!(
                "printf '%s %s\\n' {name} \"$*\" >> '{}'\ncase \"$*\" in\n",
                calls.display()
            );
            for (args, file) in cases {
                body.push_str(&format!("  \"{args}\") f='{}/{file}' ;;\n", logs.display()));
            }
            body.push_str(
                "  *) echo \"shim: unexpected arguments: $*\" >&2; exit 64 ;;\nesac\n\
                 while IFS= read -r l || [ -n \"$l\" ]; do printf '%s\\n' \"$l\"; done < \"$f\"",
            );
            self.stub(name, &body);
        };
        shim(
            "pacman",
            &[
                ("-Qqe", "pacman-Qqe.txt"),
                ("-Qqm", "pacman-Qqm.txt"),
                ("-Q", "pacman-Q.txt"),
            ],
        );
        let units = "list-unit-files --state=enabled --no-legend --no-pager";
        shim(
            "systemctl",
            &[
                (&format!("--system {units}"), "systemctl-system.txt"),
                (&format!("--user {units}"), "systemctl-user.txt"),
            ],
        );
        shim(
            "omarchy",
            &[
                ("plugin list --json", "plugin-list-after.json"),
                ("plugin catalog", "plugin-catalog.json"),
            ],
        );
        self.stub("omarchy-version", "echo 4.0.7-1");
        calls
    }

    /// `seldon init --non-interactive --no-capture` of a fresh logbook at
    /// `<tmp>/<dir>`: no collector has a cursor yet, so a test's first
    /// `capture` records the state it set up as the baseline (WP-024 made
    /// the first capture part of `init`; `tests/init.rs` covers it).
    pub fn init_logbook_at(&self, dir: &str, language: &str) -> PathBuf {
        let root = self.tmp.path().join(dir);
        let out = self.seldon(&[
            "init",
            "--non-interactive",
            "--no-capture",
            "--path",
            root.to_str().unwrap(),
            "--language",
            language,
        ]);
        assert_eq!(out.status.code(), Some(0), "init: {}", stderr(&out));
        root
    }

    /// A fresh English logbook at `<tmp>/logbook` (the config points at it).
    pub fn init_logbook(&self) -> PathBuf {
        self.init_logbook_at("logbook", "en")
    }

    /// `seldon args…` with the clock fixed at `now` (`SELDON_NOW`).
    pub fn at(&self, now: &str, args: &[&str]) -> Output {
        self.command(args)
            .env("SELDON_NOW", now)
            .output()
            .expect("run seldon")
    }
}

/// `fixtures/logs/hardware/`: the `proc/` and `sys/` files the dossier's
/// `hardware.summary` reads (`SELDON_HARDWARE_ROOT`).
pub fn hardware_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../fixtures/logs/hardware")
}

/// `fixtures/logs/omarchy-packages/`: copies of Omarchy's
/// `omarchy-base.packages` and `omarchy-other.packages`
/// (`SELDON_OMARCHY_PACKAGES`).
pub fn omarchy_packages() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../fixtures/logs/omarchy-packages")
}

/// Copies a directory tree (files and folders only).
pub fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), &target).unwrap();
        }
    }
}

pub fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn schema(name: &str) -> serde_json::Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../schema")
        .join(name);
    serde_json::from_str(&read(&path)).unwrap()
}

/// Panics unless `instance` validates against `schema` (formats checked).
fn assert_valid(schema: &serde_json::Value, instance: &serde_json::Value, what: &str) {
    let validator = jsonschema::options()
        .should_validate_formats(true)
        .build(schema)
        .expect("schema compiles");
    let errors: Vec<String> = validator
        .iter_errors(instance)
        .map(|e| format!("{e} at {}", e.instance_path()))
        .collect();
    assert!(
        errors.is_empty(),
        "{what} is invalid: {errors:?}\n{instance}"
    );
}

/// Every line of every `ledger/*.jsonl`, in file order, each validated
/// against `schema/event.schema.json`.
pub fn ledger(root: &Path) -> Vec<serde_json::Value> {
    let event_schema = schema("event.schema.json");
    let mut files: Vec<PathBuf> = std::fs::read_dir(root.join("ledger"))
        .map(|r| r.map(|e| e.unwrap().path()).collect())
        .unwrap_or_default();
    files.retain(|p| p.extension().is_some_and(|e| e == "jsonl"));
    files.sort();
    let mut out = Vec::new();
    for file in files {
        for (n, line) in read(&file).lines().enumerate() {
            let v: serde_json::Value = serde_json::from_str(line)
                .unwrap_or_else(|e| panic!("{}:{}: {e}", file.display(), n + 1));
            assert_valid(&event_schema, &v, &format!("{}:{}", file.display(), n + 1));
            out.push(v);
        }
    }
    out
}

/// Panics unless `case` validates against `schema/case.schema.json` (its
/// `$ref`s into the event schema are inlined).
pub fn assert_valid_case(case: &serde_json::Value) {
    let text = read(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../schema/case.schema.json"))
        .replace("event.schema.json#/$defs/", "#/$defs/");
    let mut case_schema: serde_json::Value = serde_json::from_str(&text).unwrap();
    case_schema["$defs"] = schema("event.schema.json")["$defs"].clone();
    assert_valid(&case_schema, case, "case");
}

/// Every file under `dir` (except `.git/`) with its bytes, by relative path.
pub fn tree(dir: &Path) -> std::collections::BTreeMap<String, Vec<u8>> {
    fn walk(root: &Path, dir: &Path, out: &mut std::collections::BTreeMap<String, Vec<u8>>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.file_name().is_some_and(|n| n == ".git") {
                continue;
            }
            if path.is_dir() {
                walk(root, &path, out);
            } else {
                let rel = path.strip_prefix(root).unwrap().to_string_lossy();
                out.insert(rel.into_owned(), std::fs::read(&path).unwrap());
            }
        }
    }
    let mut out = std::collections::BTreeMap::new();
    walk(dir, dir, &mut out);
    out
}

/// The one file in `dir` whose name starts with `prefix`.
pub fn find_file(dir: &Path, prefix: &str) -> PathBuf {
    let found: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.file_name().unwrap().to_string_lossy().starts_with(prefix))
        .collect();
    assert_eq!(found.len(), 1, "{prefix}* in {}: {found:?}", dir.display());
    found[0].clone()
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

/// The contract schemas by `$id`, for `$ref`s across files.
struct SchemaFiles(std::collections::HashMap<String, serde_json::Value>);

impl jsonschema::Retrieve for SchemaFiles {
    fn retrieve(
        &self,
        uri: &jsonschema::Uri<String>,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
        self.0
            .get(uri.as_str())
            .cloned()
            .ok_or_else(|| format!("no schema {uri}").into())
    }
}

/// Errors of `instance` against `schema/index.schema.json` (with the
/// event and case schemas it references), formats checked.
pub fn index_errors(instance: &serde_json::Value) -> Vec<String> {
    let files = ["index.schema.json", "event.schema.json", "case.schema.json"]
        .map(|name| {
            let s = schema(name);
            (s["$id"].as_str().unwrap().to_string(), s)
        })
        .into_iter()
        .collect();
    let validator = jsonschema::options()
        .should_validate_formats(true)
        .with_retriever(SchemaFiles(files))
        .build(&schema("index.schema.json"))
        .expect("index schema compiles");
    validator
        .iter_errors(instance)
        .map(|e| format!("{e} at {}", e.instance_path()))
        .collect()
}

/// Errors of `instance` against `schema/proposal.schema.json` (WP-124),
/// formats checked.
pub fn proposal_errors(instance: &serde_json::Value) -> Vec<String> {
    let files = ["proposal.schema.json", "event.schema.json"]
        .map(|name| {
            let s = schema(name);
            (s["$id"].as_str().unwrap().to_string(), s)
        })
        .into_iter()
        .collect();
    let validator = jsonschema::options()
        .should_validate_formats(true)
        .with_retriever(SchemaFiles(files))
        .build(&schema("proposal.schema.json"))
        .expect("proposal schema compiles");
    validator
        .iter_errors(instance)
        .map(|e| format!("{e} at {}", e.instance_path()))
        .collect()
}

/// Panics unless `instance` validates against `schema/index.schema.json`.
pub fn assert_valid_index(instance: &serde_json::Value) {
    let errors = index_errors(instance);
    assert!(errors.is_empty(), "index is invalid: {errors:#?}");
}

/// `cmd` run through `/bin/sh` with the file mode creation mask `umask`
/// (octal, e.g. `"022"`): same program, arguments, environment and
/// working directory.
pub fn with_umask(cmd: &Command, umask: &str) -> Command {
    let mut sh = Command::new("/bin/sh");
    sh.arg("-c")
        .arg(format!("umask {umask}; exec \"$0\" \"$@\""))
        .arg(cmd.get_program())
        .args(cmd.get_args())
        .env_clear();
    for (key, value) in cmd.get_envs() {
        if let Some(value) = value {
            sh.env(key, value);
        }
    }
    if let Some(dir) = cmd.get_current_dir() {
        sh.current_dir(dir);
    }
    sh
}

/// The permission bits of `path` (a symbolic link is followed).
pub fn mode(path: &Path) -> u32 {
    std::fs::metadata(path)
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
        .permissions()
        .mode()
        & 0o777
}

/// Writes the executable `path` (mode 0755) with `text` from a child
/// process. Written by the test process, the file's write descriptor can
/// be inherited by a child another test thread forks at that moment, and
/// a program that executes the file before that child execs fails with
/// `ETXTBSY` ("text file busy"). `sys::run` retries that; bash and git do
/// not. The child's descriptors are never inherited by the test process's
/// forks. No PATH lookup: `printf` and the redirection are `sh` builtins.
pub fn write_executable(path: &Path, text: &str) {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).unwrap();
    }
    let status = Command::new("/bin/sh")
        .args(["-c", "printf '%s' \"$2\" > \"$1\"", "sh"])
        .arg(path)
        .arg(text)
        .status()
        .unwrap();
    assert!(status.success(), "cannot write {}", path.display());
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

/// Fails a timing budget test in a debug build: SPEC-ENGINE §1's budgets
/// hold for release code (`just check-perf` runs `--profile bench`, WP-076).
pub fn assert_optimised() {
    if cfg!(debug_assertions) {
        panic!("a timing budget needs an optimised build: run `just check-perf`");
    }
}

/// The median wall time of `runs` calls of `f` after one warm-up call, and
/// every time, sorted (WP-076).
pub fn median_time(
    runs: usize,
    mut f: impl FnMut(),
) -> (std::time::Duration, Vec<std::time::Duration>) {
    f();
    let mut times: Vec<std::time::Duration> = (0..runs)
        .map(|_| {
            let start = std::time::Instant::now();
            f();
            start.elapsed()
        })
        .collect();
    times.sort();
    (times[runs / 2], times)
}

/// Asserts that the [`median_time`] of `f` is under `budget`. A median at
/// or over it is measured once more before the test fails, so a load spike
/// of a shared host does not fail it; a slow build fails both times.
/// Prints every measurement. Returns the median that counted.
pub fn assert_within_budget(
    what: &str,
    budget: std::time::Duration,
    runs: usize,
    mut f: impl FnMut(),
) -> std::time::Duration {
    let mut medians = Vec::new();
    for attempt in 1..=2 {
        let (median, times) = median_time(runs, &mut f);
        eprintln!(
            "{what}: median {median:?} (budget {budget:?}, attempt {attempt}), all {times:?}"
        );
        medians.push(median);
        if median < budget {
            return median;
        }
    }
    panic!("{what}: medians {medians:?}, budget {budget:?}");
}

/// Gives `path` to an unprivileged owner when the tests run as root (CI's
/// container), so `sys::runner` reads it as a user's; a no-op otherwise.
pub fn user_owned(path: &Path) {
    use std::os::unix::fs::MetadataExt as _;
    if std::fs::metadata(path).is_ok_and(|m| m.uid() == 0) {
        std::os::unix::fs::chown(path, Some(65534), Some(65534)).expect("chown the probe");
    }
}
