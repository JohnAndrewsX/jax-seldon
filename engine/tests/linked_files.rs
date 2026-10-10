//! Every file of the logbook the engine writes (replaces or appends to)
//! is a regular file inside the logbook (WP-171, ADR-0049): a symbolic
//! link there (the write would land wherever it points; a dangling one
//! would create its target) or anything else in its place is refused with
//! exit 1 and a reason naming the file, and nothing is written: not in the
//! logbook, not through the link, not in the ledger. One test per writer.
//! The generated views (`STATUS.md`, `DECISIONS.md`, `ledger/*.md`) are
//! skipped with a warning instead, and `index.json` is still written.
//! The setup kit's copy is covered by the unit tests of
//! `setup::copy_tree`, the write primitives by those of `sys` and
//! `ledger`; the capture's rules upgrade by `tests/rules.rs`.

mod common;

use std::collections::BTreeMap;
use std::ffi::CString;
use std::os::unix::ffi::OsStrExt as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

use common::{Env, Snapper, copy_dir, find_file, fixture_logbook, hardware_root, stderr};
use seldon::logbook::layout::{self, Misplaced};

const T0: &str = "2026-10-09T09:00:00+02:00";
const T1: &str = "2026-10-09T10:00:00+02:00";
/// `generatedAt` of the fixture logbook's sample index.
const FIXTURE_AT: &str = "2026-10-01T17:05:12+02:00";

unsafe extern "C" {
    fn mkfifo(path: *const std::ffi::c_char, mode: u32) -> i32;
}

/// What takes the file's place.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Swap {
    /// A symbolic link to a copy of the file outside the logbook (a file
    /// of its own when there is none yet).
    Link,
    /// A symbolic link to a file outside that does not exist.
    Dangling,
    /// A directory.
    Dir,
    /// A FIFO: opened, it would block the command.
    Fifo,
}

const ALL: &[Swap] = &[Swap::Link, Swap::Dangling, Swap::Dir, Swap::Fifo];

impl Swap {
    fn what(self) -> &'static str {
        match self {
            Swap::Link | Swap::Dangling => "a symbolic link",
            Swap::Dir | Swap::Fifo => "no regular file",
        }
    }
}

/// Where a swap's link points: `<tmp>/outside/<rel with - for />`.
fn outside(env: &Env, rel: &str) -> PathBuf {
    env.tmp.path().join("outside").join(rel.replace('/', "-"))
}

/// Puts `how` where the file `rel` of `root` is (its folder made first).
fn swap(env: &Env, root: &Path, rel: &str, how: Swap) {
    let path = root.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let target = outside(env, rel);
    std::fs::create_dir_all(target.parent().unwrap()).unwrap();
    let old = std::fs::read(&path).ok();
    let _ = std::fs::remove_file(&path);
    match how {
        Swap::Link => {
            std::fs::write(&target, old.unwrap_or_else(|| b"outside\n".to_vec())).unwrap();
            std::os::unix::fs::symlink(&target, &path).unwrap();
        }
        Swap::Dangling => std::os::unix::fs::symlink(&target, &path).unwrap(),
        Swap::Dir => std::fs::create_dir(&path).unwrap(),
        Swap::Fifo => {
            let c = CString::new(path.as_os_str().as_bytes()).unwrap();
            assert_eq!(unsafe { mkfifo(c.as_ptr(), 0o600) }, 0, "mkfifo {rel}");
        }
    }
}

/// Every entry below `dir` (but `.git`), never followed or opened: a
/// link as its target, a FIFO as such, a file by its bytes.
fn snapshot(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        let Ok(read) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in read {
            let path = entry.unwrap().path();
            if path.file_name().is_some_and(|n| n == ".git") {
                continue;
            }
            let rel = path
                .strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .into_owned();
            let kind = path.symlink_metadata().unwrap().file_type();
            if kind.is_symlink() {
                let to = std::fs::read_link(&path).unwrap();
                out.insert(rel, format!("-> {}", to.display()).into_bytes());
            } else if kind.is_dir() {
                out.insert(rel.clone(), b"dir".to_vec());
                walk(root, &path, out);
            } else if kind.is_file() {
                out.insert(rel, std::fs::read(&path).unwrap());
            } else {
                out.insert(rel, b"special".to_vec());
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(dir, dir, &mut out);
    out
}

/// The output of `cmd`; a command that has not finished after a minute
/// (a FIFO opened) is killed, by its own handle, and the test fails.
fn within(mut cmd: Command, what: &str) -> Output {
    let mut child = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("run seldon");
    let deadline = Instant::now() + Duration::from_secs(60);
    while child.try_wait().unwrap().is_none() {
        if Instant::now() > deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("{what}: still running after a minute (a FIFO opened?)");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    child.wait_with_output().unwrap()
}

/// Which logbook a test starts from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Start {
    /// A fresh logbook; the command runs at `T1`.
    Fresh,
    /// A copy of the fixture logbook; the command runs at its
    /// `generatedAt`.
    Fixture,
}

/// A fresh logbook `before` prepared at `T0`, the file `rel` (a function
/// of the root) swapped each way of `hows`, then `args` at `T1`: refused
/// (see [`refused_with`]).
fn refused(
    hows: &[Swap],
    rel: impl Fn(&Path) -> String,
    before: impl Fn(&Env, &Path),
    args: &[&str],
) {
    refused_with(Start::Fresh, hows, rel, before, args);
}

/// The logbook `start`, `before` prepared at `T0`, the file `rel` swapped
/// each way of `hows`, then `args` (with the fixture hardware): exit 1,
/// the reason names the file, and the logbook (its ledger included) and
/// what is outside it are as they were; a dangling link's target is not
/// made.
fn refused_with(
    start: Start,
    hows: &[Swap],
    rel: impl Fn(&Path) -> String,
    before: impl Fn(&Env, &Path),
    args: &[&str],
) {
    for &how in hows {
        let env = Env::new(Snapper::Missing);
        let (root, now) = match start {
            Start::Fresh => (env.init_logbook(), T1),
            Start::Fixture => {
                let root = env.tmp.path().join("logbook");
                copy_dir(&fixture_logbook(), &root);
                (root, FIXTURE_AT)
            }
        };
        before(&env, &root);
        let rel = rel(&root);
        swap(&env, &root, &rel, how);
        named_by_doctor(&root, &rel, how, true);
        let inside = snapshot(&root);
        let beside = snapshot(&env.tmp.path().join("outside"));

        let mut cmd = env.command(args);
        cmd.env("SELDON_LOGBOOK", &root)
            .env("SELDON_NOW", now)
            .env("SELDON_HARDWARE_ROOT", hardware_root());
        let out = within(cmd, &format!("{rel} {how:?} {args:?}"));

        let err = stderr(&out);
        assert_eq!(out.status.code(), Some(1), "{rel} {how:?} {args:?}: {err}");
        let reason = format!(
            "{rel} is {}, not a file of the logbook; make it a file and run the command again",
            how.what()
        );
        assert!(err.contains(&reason), "{rel} {how:?} {args:?}: {err}");
        let after = snapshot(&root);
        let changed: Vec<&String> = after
            .keys()
            .chain(inside.keys())
            .filter(|k| after.get(*k) != inside.get(*k))
            .collect();
        assert!(
            changed.is_empty(),
            "{rel} {how:?} {args:?}: the logbook changed: {changed:?}"
        );
        assert!(
            snapshot(&env.tmp.path().join("outside")) == beside,
            "{rel} {how:?} {args:?}: a file outside changed"
        );
    }
}

/// What ties the writers to doctor's `layout` row (ADR-0049 §3): the file
/// a writer refuses (`refused`) or skips is named, as a refusal or not.
fn named_by_doctor(root: &Path, rel: &str, how: Swap, refused: bool) {
    let what = match how {
        Swap::Link | Swap::Dangling => Misplaced::Link,
        Swap::Dir | Swap::Fifo => Misplaced::NoRegularFile,
    };
    let found = layout::misplaced(root);
    assert!(
        found
            .iter()
            .any(|f| f.rel == rel && f.what == what && f.refused == refused),
        "{rel} {how:?}: doctor's layout row does not name it (refused: {refused}): {found:?}"
    );
}

/// `args` at `T0`, exit 0.
fn ok(env: &Env, args: &[&str]) {
    let out = env.at(T0, args);
    assert_eq!(out.status.code(), Some(0), "{args:?}: {}", stderr(&out));
}

fn nothing(_: &Env, _: &Path) {}

fn logged(env: &Env, _: &Path) {
    ok(env, &["log", "--", "first"]);
}

fn queued(env: &Env, _: &Path) {
    ok(env, &["plan", "new", "--", "Zed"]);
}

fn active(env: &Env, root: &Path) {
    queued(env, root);
    ok(env, &["plan", "start", "C-2026-001"]);
}

/// The one case file in `work/<folder>/`, relative to the root.
fn case_in(root: &Path, folder: &str) -> String {
    let file = find_file(&root.join("work").join(folder), "C-2026-001");
    file.strip_prefix(root)
        .unwrap()
        .to_string_lossy()
        .into_owned()
}

fn fixed(rel: &'static str) -> impl Fn(&Path) -> String {
    move |_| rel.to_string()
}

const DAY: &str = "journal/2026/2026-10-09.md";

#[test]
fn a_journal_day() {
    // the day exists, and is written for the first time
    refused(ALL, fixed(DAY), logged, &["log", "--", "Zed ausprobiert"]);
    refused(ALL, fixed(DAY), nothing, &["log", "--", "Zed ausprobiert"]);
}

#[test]
fn a_ledger_month() {
    let month = fixed("ledger/2026-10.jsonl");
    refused(ALL, &month, logged, &["log", "--", "Zed ausprobiert"]);
    refused(ALL, &month, nothing, &["plan", "new", "--", "Zed"]);
}

#[test]
fn a_case_file() {
    // a case is found by a file there (a dangling link is no case); where
    // it moves to, a link to a file would be read as a case of its own
    let hows = &[Swap::Link];
    let target = &[Swap::Dangling];
    // moved out of a linked file, a link where it goes, saved in place
    refused(
        hows,
        |root| case_in(root, "queued"),
        queued,
        &["plan", "start", "C-2026-001"],
    );
    refused(
        target,
        |root| case_in(root, "queued").replace("queued", "active"),
        queued,
        &["plan", "start", "C-2026-001"],
    );
    refused(
        hows,
        |root| case_in(root, "queued"),
        queued,
        &["plan", "set", "C-2026-001", "--risk", "R2"],
    );
    refused(
        hows,
        |root| case_in(root, "active"),
        active,
        &["log", "--case", "C-2026-001", "--", "progress"],
    );
    refused(
        hows,
        |root| case_in(root, "active"),
        active,
        &[
            "event",
            "manual",
            "note",
            "--subject",
            "x",
            "--case",
            "C-2026-001",
        ],
    );
    refused(
        target,
        |root| case_in(root, "active").replace("active", "completed"),
        |env, root| {
            active(env, root);
            ok(env, &["plan", "verify", "C-2026-001"]);
        },
        &["plan", "done", "C-2026-001"],
    );
}

#[test]
fn the_active_case() {
    let file = fixed(".seldon/active-case");
    refused(ALL, &file, queued, &["plan", "start", "C-2026-001"]);
    let verifying = |env: &Env, root: &Path| {
        active(env, root);
        ok(env, &["plan", "verify", "C-2026-001"]);
    };
    refused(
        &[Swap::Link],
        &file,
        verifying,
        &["plan", "done", "C-2026-001"],
    );
}

#[test]
fn an_area_readme() {
    let readme = fixed("areas/editors/README.md");
    refused(
        ALL,
        &readme,
        nothing,
        &["plan", "new", "--area", "editors", "--", "Zed"],
    );
    refused(
        ALL,
        &readme,
        queued,
        &["plan", "set", "C-2026-001", "--area", "editors"],
    );
}

#[test]
fn a_decision() {
    // a dangling link or a folder is no decision file: "unknown decision"
    refused(
        &[Swap::Link],
        |root| {
            let file = find_file(&root.join("decisions"), "ADR-0001");
            file.strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .into_owned()
        },
        |env, _| ok(env, &["decide", "--no-edit", "--", "Zed statt VS Code"]),
        &["decide", "accept", "ADR-0001"],
    );
}

/// A generated view is skipped, not refused (WP-171 round 2): `status`
/// and `index` exit 0 with the reason as a warning (human and JSON), the
/// view and what it points to stay as they were, and `index.json` is
/// written all the same. A linked `ledger/` folder is still refused
/// (`tests/linked_folders.rs`).
#[test]
fn the_views_are_skipped() {
    let status = |env: &Env, root: &Path| {
        logged(env, root);
        ok(env, &["status"]);
    };
    for (file, command) in [
        ("STATUS.md", "status"),
        ("DECISIONS.md", "status"),
        ("ledger/2026-10.md", "status"),
        ("ledger/2026-10.md", "index"),
    ] {
        for &how in ALL {
            let what = format!("{file} {how:?} {command}");
            let env = Env::new(Snapper::Missing);
            let root = env.init_logbook();
            status(&env, &root);
            swap(&env, &root, file, how);
            named_by_doctor(&root, file, how, false);
            let view = snapshot(&root).get(file).cloned();
            let beside = snapshot(&env.tmp.path().join("outside"));
            let warning = format!(
                "{file} not updated: {file} is {}, not a file of the logbook; make it a file and run the command again",
                how.what()
            );

            let mut cmd = env.command(&["--json", command]);
            cmd.env("SELDON_LOGBOOK", &root).env("SELDON_NOW", T1);
            let out = within(cmd, &what);
            assert_eq!(out.status.code(), Some(0), "{what}: {}", stderr(&out));
            let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
            assert!(
                v["warnings"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|w| *w == warning.as_str()),
                "{what}: {v}"
            );
            let index: serde_json::Value = serde_json::from_str(
                &std::fs::read_to_string(env.home.join(".local/state/seldon/index.json")).unwrap(),
            )
            .unwrap();
            assert_eq!(index["generatedAt"], T1, "{what}");
            assert_eq!(snapshot(&root).get(file).cloned(), view, "{what}");
            assert!(
                snapshot(&env.tmp.path().join("outside")) == beside,
                "{what}: a file outside changed"
            );

            let mut cmd = env.command(&[command]);
            cmd.env("SELDON_LOGBOOK", &root).env("SELDON_NOW", T1);
            let out = within(cmd, &what);
            assert_eq!(out.status.code(), Some(0), "{what}");
            let human = format!("{}{}", String::from_utf8_lossy(&out.stdout), stderr(&out));
            assert!(
                human.contains(&format!("warning: {warning}")),
                "{what}: {human}"
            );
        }
    }
}

#[test]
fn the_rebuild() {
    refused(
        ALL,
        fixed("outputs/REBUILD.md"),
        nothing,
        &["--no-commit", "rebuild"],
    );
}

#[test]
fn a_dossier_file() {
    refused(ALL, fixed("system/hardware.md"), nothing, &["dossier"]);
}

#[test]
fn the_rules() {
    // Seldon's v1 rules no release wrote: `rules update` archives the file
    // first, so the check comes before the archive copy
    let old_rules = |_: &Env, root: &Path| {
        std::fs::write(
            root.join("AGENTS.md"),
            "# Mine\n\n<!-- seldon:begin rules v1 -->\nPropose a case and wait.\n<!-- seldon:end -->\n",
        )
        .unwrap();
    };
    refused(ALL, fixed("AGENTS.md"), old_rules, &["rules", "update"]);
}

fn omarchy_agent_vault() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../fixtures/vaults/omarchy-agent")
}

#[test]
fn the_import() {
    let vault = |env: &Env, _: &Path| {
        copy_dir(&omarchy_agent_vault(), &env.home.join("vault"));
    };
    let apply = ["import", "omarchy-agent", "~/vault", "--apply"];
    for file in [
        "outputs/IMPORT-omarchy-agent.md",
        ".seldon/imports/omarchy-agent.json",
        ".seldon/imports/omarchy-agent.undo.json",
        "system/deviations.md",
    ] {
        refused(&[Swap::Link, Swap::Dangling], fixed(file), vault, &apply);
    }
    // a day of the vault the logbook does not have yet (a link to a file
    // is read as the day, and its text is no journal day)
    refused(
        &[Swap::Dangling],
        fixed("journal/2026/2026-09-03.md"),
        vault,
        &apply,
    );
    // a dry run writes its report only
    refused(
        ALL,
        fixed("outputs/IMPORT-omarchy-agent.md"),
        vault,
        &["import", "omarchy-agent", "~/vault"],
    );
}

#[test]
fn the_task_import_marker() {
    let tasks = |env: &Env, _: &Path| {
        std::fs::write(env.home.join("TODO.md"), "- [ ] Zed ausprobieren\n").unwrap();
    };
    refused(
        ALL,
        fixed(".seldon/imports/tasks.json"),
        tasks,
        &["import", "task", "~/TODO.md"],
    );
}

/// The fixture's drift item `drift explain` resolves.
const OLLAMA: &str = "01M3VNFTF8EVHWFFZ687N14Q0C"; // pacman install

#[test]
fn a_drift_explanation_in_an_area() {
    refused_with(
        Start::Fixture,
        &[Swap::Link, Swap::Dangling],
        fixed("areas/dev-env/README.md"),
        |_, root| {
            let _ = std::fs::remove_dir_all(root.join("areas/dev-env"));
        },
        &[
            "drift",
            "explain",
            OLLAMA,
            "--area",
            "dev-env",
            "--",
            "Codex set up ollama",
        ],
    );
}

/// The write primitives on their own, without the checks a command makes
/// first: the hooks write the journal and the ledger with nothing in front
/// of them.
mod primitives {
    use super::*;
    use seldon::error::Error;
    use seldon::logbook::{Logbook, cases, journal};

    /// `f`'s result; one that has not come after a minute (a FIFO
    /// opened in this process) fails the test, its thread left blocked
    /// until the test binary exits.
    fn in_time<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(f());
        });
        rx.recv_timeout(Duration::from_secs(60))
            .expect("still running after a minute (a FIFO opened?)")
    }

    fn refused<T: std::fmt::Debug>(r: Result<T, Error>, rel: &str, what: &str) {
        match r {
            Err(Error::User(m)) => assert_eq!(
                m,
                format!(
                    "{rel} is {what}, not a file of the logbook; make it a file and run the command again"
                )
            ),
            other => panic!("{rel}: expected a user error, got {other:?}"),
        }
    }

    #[test]
    fn the_journal_is_not_written_through_a_link() {
        for how in [Swap::Link, Swap::Dangling, Swap::Fifo] {
            let env = Env::new(Snapper::Missing);
            let root = env.init_logbook();
            swap(&env, &root, DAY, how);
            let logbook = Logbook::open(&root).unwrap();
            let inside = snapshot(&root);
            let beside = snapshot(&env.tmp.path().join("outside"));
            let now = chrono::DateTime::parse_from_rfc3339(T1).unwrap();
            let book = logbook.clone();
            refused(
                in_time(move || journal::append(&book, &now, "human", None, "x")),
                DAY,
                how.what(),
            );
            if how != Swap::Link {
                // a link to a file: the day counts as there, nothing made
                let book = logbook.clone();
                refused(
                    in_time(move || journal::ensure_day(&book, &now)),
                    DAY,
                    how.what(),
                );
            }
            assert!(snapshot(&root) == inside, "{how:?}");
            assert!(
                snapshot(&env.tmp.path().join("outside")) == beside,
                "{how:?}"
            );
        }
    }

    #[test]
    fn the_active_case_and_an_area_are_not_written_through_a_link() {
        let env = Env::new(Snapper::Missing);
        let root = env.init_logbook();
        swap(&env, &root, ".seldon/active-case", Swap::Dangling);
        swap(&env, &root, "areas/editors/README.md", Swap::Dangling);
        let logbook = Logbook::open(&root).unwrap();
        let inside = snapshot(&root);
        refused(
            cases::set_active_case(&logbook, "C-2026-001"),
            ".seldon/active-case",
            "a symbolic link",
        );
        refused(
            cases::ensure_area(&logbook, "editors"),
            "areas/editors/README.md",
            "a symbolic link",
        );
        assert!(snapshot(&root) == inside);
        assert!(!outside(&env, ".seldon/active-case").exists());
        assert!(!outside(&env, "areas/editors/README.md").exists());
    }
}

/// The writers that follow a link (`write_atomic`, `write_generated` and
/// their `_mode`/`_replace` forms) are called only for files outside the
/// logbook (ADR-0049 §2): every call site in `engine/src`, per file, is in
/// this list. A new one fails here: a file of the logbook goes through
/// `write_atomic_nofollow` or `write_generated_nofollow`, is checked with
/// `logbook::checked_file` first and is listed in `layout::written`; a file
/// outside the logbook is added below with what it writes.
#[test]
fn only_files_outside_the_logbook_are_written_through_a_link() {
    const OUTSIDE: &[(&str, usize, &str)] = &[
        (
            "collectors/config.rs",
            2,
            "the config collector's manifest and owned files (state)",
        ),
        ("collectors/mod.rs", 1, "cursors.json (state)"),
        ("collectors/recent.rs", 1, "the recent-config list (state)"),
        ("commands/agent.rs", 1, "the launches file (state)"),
        ("commands/capture.rs", 1, "config.toml"),
        ("commands/config_cmd.rs", 1, "config.toml (config watch)"),
        (
            "commands/hook.rs",
            2,
            "Claude Code's settings.json, named by the user",
        ),
        ("commands/init.rs", 1, "config.toml"),
        ("commands/setup.rs", 1, "the theme hook script (~/.config)"),
        ("commands/skills.rs", 1, "the agent skill folders"),
        (
            "commands/triage.rs",
            1,
            "proposals/<id>.json (state; replaces a link)",
        ),
        ("config.rs", 1, "config.toml"),
        ("index/autocommit.rs", 1, "autocommit.json (state)"),
        ("index/mod.rs", 1, "index.json (state)"),
        ("sys.rs", 5, "the definitions and their tests"),
    ];
    let pattern =
        regex::Regex::new(r"\b(write_atomic(_mode|_replace)?|write_generated)\(").unwrap();
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut found: BTreeMap<String, usize> = BTreeMap::new();
    let mut stack = vec![src.clone()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "rs") {
                let calls = std::fs::read_to_string(&path)
                    .unwrap()
                    .lines()
                    .filter(|l| !l.trim_start().starts_with("//"))
                    .map(|l| pattern.find_iter(l).count())
                    .sum::<usize>();
                if calls > 0 {
                    let rel = path
                        .strip_prefix(&src)
                        .unwrap()
                        .to_string_lossy()
                        .into_owned();
                    found.insert(rel, calls);
                }
            }
        }
    }
    let listed: BTreeMap<String, usize> = OUTSIDE
        .iter()
        .map(|(file, n, _)| (file.to_string(), *n))
        .collect();
    assert_eq!(
        found, listed,
        "a call of write_atomic/write_generated was added or removed: a logbook file goes through the nofollow writers"
    );
}
