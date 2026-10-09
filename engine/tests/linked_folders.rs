//! Every folder of the logbook the engine writes a file in is a real
//! directory inside the logbook (WP-168): a symbolic link there (the write
//! would land wherever it points) or a file in its place is refused with
//! exit 1 and a reason naming the folder, and nothing is written, in the
//! logbook, through the link or in the ledger. One test per folder, each
//! with a link to a copy of the folder outside the logbook and with a file
//! in its place. `inbox/` (WP-166's writer) and the setup kit's `.claude/`
//! (only `init` copies it, into an empty logbook) are covered by the unit
//! tests of `logbook::checked_dir` and `setup::copy_tree`.

mod common;

use std::path::{Path, PathBuf};

use common::{Env, Snapper, copy_dir, fixture_logbook, hardware_root, ledger, stderr, tree};

const T0: &str = "2026-10-09T09:00:00+02:00";
const T1: &str = "2026-10-09T10:00:00+02:00";
/// `generatedAt` of the fixture logbook's sample index.
const FIXTURE_AT: &str = "2026-10-01T17:05:12+02:00";

/// What takes the folder's place.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Swap {
    /// A symbolic link to a copy of the folder outside the logbook.
    Link,
    /// A regular file.
    File,
}

impl Swap {
    fn what(self) -> &'static str {
        match self {
            Swap::Link => "a symbolic link",
            Swap::File => "no directory",
        }
    }
}

/// Puts `how` where the folder `rel` of `root` is (made first if it is
/// missing).
fn swap(env: &Env, root: &Path, rel: &str, how: Swap) {
    let dir = root.join(rel);
    std::fs::create_dir_all(&dir).unwrap();
    match how {
        Swap::Link => {
            let outside = env
                .tmp
                .path()
                .join(format!("outside-{}", rel.replace('/', "-")));
            copy_dir(&dir, &outside);
            std::fs::remove_dir_all(&dir).unwrap();
            std::os::unix::fs::symlink(&outside, &dir).unwrap();
        }
        Swap::File => {
            std::fs::remove_dir_all(&dir).unwrap();
            std::fs::write(&dir, "not a folder\n").unwrap();
        }
    }
}

/// Which logbook a test starts from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Start {
    /// A fresh logbook; the command runs at `T1`.
    Fresh,
    /// A copy of the fixture logbook (drift, cases); the command runs at
    /// its `generatedAt`.
    Fixture,
}

/// A fresh logbook `before` prepared at `T0`, the folder `rel` swapped
/// each way, then `args` at `T1`: refused (see [`refused_with`]).
fn refused(rel: &str, shown: &str, before: impl Fn(&Env, &Path), args: &[&str]) {
    refused_with(
        Start::Fresh,
        &[Swap::Link, Swap::File],
        rel,
        shown,
        before,
        args,
    );
}

/// The logbook `start`, `before` prepared at `T0`, the folder `rel`
/// swapped each way of `hows`, then `args` (with the fixture hardware):
/// exit 1, the reason names `shown`, and the logbook (through the link
/// included) and its ledger are as they were.
fn refused_with(
    start: Start,
    hows: &[Swap],
    rel: &str,
    shown: &str,
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
        swap(&env, &root, rel, how);
        let snapshot = tree(&root);
        let events = if rel == "ledger" {
            None
        } else {
            Some(ledger(&root).len())
        };

        let out = env
            .command(args)
            .env("SELDON_LOGBOOK", &root)
            .env("SELDON_NOW", now)
            .env("SELDON_HARDWARE_ROOT", hardware_root())
            .output()
            .expect("run seldon");

        let err = stderr(&out);
        assert_eq!(out.status.code(), Some(1), "{rel} {how:?} {args:?}: {err}");
        let reason = format!(
            "{shown} is {}, not a folder of the logbook; make it a folder and run the command again",
            how.what()
        );
        assert!(err.contains(&reason), "{rel} {how:?} {args:?}: {err}");
        assert!(
            tree(&root) == snapshot,
            "{rel} {how:?} {args:?}: a file changed"
        );
        if let Some(n) = events {
            assert_eq!(ledger(&root).len(), n, "{rel} {how:?} {args:?}");
        }
    }
}

/// `args` at `T0`, exit 0.
fn ok(env: &Env, args: &[&str]) {
    let out = env.at(T0, args);
    assert_eq!(out.status.code(), Some(0), "{args:?}: {}", stderr(&out));
}

fn nothing(_: &Env, _: &Path) {}

fn queued(env: &Env, _: &Path) {
    ok(env, &["plan", "new", "--", "Zed"]);
}

fn active(env: &Env, root: &Path) {
    queued(env, root);
    ok(env, &["plan", "start", "C-2026-001"]);
}

#[test]
fn decisions() {
    refused(
        "decisions",
        "decisions",
        nothing,
        &["decide", "--no-edit", "--", "Zed statt VS Code"],
    );
    refused(
        "decisions",
        "decisions",
        |env, _| ok(env, &["decide", "--no-edit", "--", "Zed statt VS Code"]),
        &["decide", "accept", "ADR-0001"],
    );
}

#[test]
fn work() {
    refused("work", "work", nothing, &["plan", "new", "--", "Zed"]);
    refused("work", "work", queued, &["plan", "start", "C-2026-001"]);
}

#[test]
fn work_queued() {
    refused(
        "work/queued",
        "work/queued",
        nothing,
        &["plan", "new", "--", "Zed"],
    );
}

#[test]
fn work_active() {
    refused(
        "work/active",
        "work/active",
        queued,
        &["plan", "start", "C-2026-001"],
    );
    refused(
        "work/active",
        "work/active",
        active,
        &["log", "--case", "C-2026-001", "--", "progress"],
    );
    refused(
        "work/active",
        "work/active",
        active,
        &["plan", "set", "C-2026-001", "--risk", "R2"],
    );
}

#[test]
fn work_completed() {
    refused(
        "work/completed",
        "work/completed",
        active,
        &["plan", "done", "C-2026-001"],
    );
}

#[test]
fn journal() {
    refused(
        "journal",
        "journal",
        nothing,
        &["log", "--", "Zed ausprobiert"],
    );
}

#[test]
fn journal_year() {
    refused(
        "journal/2026",
        "journal/2026",
        |env, _| ok(env, &["log", "--", "first"]),
        &["log", "--", "Zed ausprobiert"],
    );
}

#[test]
fn ledger_folder() {
    refused(
        "ledger",
        "ledger",
        nothing,
        &["log", "--", "Zed ausprobiert"],
    );
    refused("ledger", "ledger", nothing, &["plan", "new", "--", "Zed"]);
}

#[test]
fn areas() {
    refused(
        "areas",
        "areas",
        nothing,
        &["plan", "new", "--area", "editors", "--", "Zed"],
    );
}

#[test]
fn an_area() {
    refused(
        "areas/editors",
        "areas/editors",
        |env, _| ok(env, &["plan", "new", "--area", "editors", "--", "Zed"]),
        &["plan", "new", "--area", "editors", "--", "Helix"],
    );
    refused(
        "areas/editors",
        "areas/editors",
        queued,
        &["plan", "set", "C-2026-001", "--area", "editors"],
    );
}

#[test]
fn seldon_folder() {
    // a file in its place: the logbook is not initialised (exit 3)
    let env = Env::new(Snapper::Missing);
    let root = env.init_logbook();
    queued(&env, &root);
    let outside = env.tmp.path().join("outside-seldon");
    copy_dir(&root.join(".seldon"), &outside);
    std::fs::remove_dir_all(root.join(".seldon")).unwrap();
    std::os::unix::fs::symlink(&outside, root.join(".seldon")).unwrap();
    let snapshot = tree(&root);
    let events = ledger(&root).len();
    let out = env.at(T1, &["plan", "start", "C-2026-001"]);
    assert_eq!(out.status.code(), Some(1), "{}", stderr(&out));
    assert!(
        stderr(&out).contains(
            ".seldon is a symbolic link, not a folder of the logbook; make it a folder and run the command again"
        ),
        "{}",
        stderr(&out)
    );
    assert!(tree(&root) == snapshot);
    assert_eq!(ledger(&root).len(), events);
    assert!(!outside.join("active-case").exists());

    std::fs::remove_file(root.join(".seldon")).unwrap();
    std::fs::write(root.join(".seldon"), "not a folder\n").unwrap();
    let snapshot = tree(&root);
    let out = env.at(T1, &["plan", "start", "C-2026-001"]);
    assert_eq!(out.status.code(), Some(3), "{}", stderr(&out));
    assert!(tree(&root) == snapshot);
}

#[test]
fn seldon_imports() {
    let tasks = |env: &Env, _: &Path| {
        std::fs::write(env.home.join("TODO.md"), "- [ ] Zed ausprobieren\n").unwrap();
    };
    refused(
        ".seldon/imports",
        ".seldon/imports",
        tasks,
        &["import", "task", "~/TODO.md"],
    );
}

#[test]
fn system() {
    refused("system", "system", nothing, &["dossier"]);
}

#[test]
fn outputs() {
    refused("outputs", "outputs", nothing, &["--no-commit", "rebuild"]);
}

#[test]
fn archive() {
    // Seldon's v1 rules no release wrote: `rules update` archives the file
    let old_rules = |_: &Env, root: &Path| {
        std::fs::write(
            root.join("AGENTS.md"),
            "# Mine\n\n<!-- seldon:begin rules v1 -->\nPropose a case and wait.\n<!-- seldon:end -->\n",
        )
        .unwrap();
    };
    refused("archive", "archive", old_rules, &["rules", "update"]);
}

#[test]
fn memory_and_the_import_folders() {
    let vault = |env: &Env, _: &Path| {
        copy_dir(&omarchy_agent_vault(), &env.home.join("vault"));
    };
    for rel in ["memory", "work/completed", "system", "outputs"] {
        refused(
            rel,
            rel,
            vault,
            &["import", "omarchy-agent", "~/vault", "--apply"],
        );
    }
}

fn omarchy_agent_vault() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../fixtures/vaults/omarchy-agent")
}

/// The fixture's drift items `drift link` and `drift explain` resolve.
const THEME: &str = "01M3VTGNY0NZG4AY80814WSKGR"; // proposed for C-2026-005 (queued)
const OLLAMA: &str = "01M3VNFTF8EVHWFFZ687N14Q0C"; // pacman install

#[test]
fn drift_explain_and_link() {
    let explain = [
        "drift",
        "explain",
        OLLAMA,
        "--area",
        "dev-env",
        "--",
        "Codex set up ollama",
    ];
    let both = [Swap::Link, Swap::File];
    for rel in ["work", "work/completed", "areas", "areas/dev-env", "ledger"] {
        refused_with(Start::Fixture, &both, rel, rel, |_, _| {}, &explain);
    }
    let link = ["drift", "link", THEME, "C-2026-005"];
    for rel in ["work/queued", "work/active", "ledger"] {
        refused_with(Start::Fixture, &both, rel, rel, |_, _| {}, &link);
    }
}

#[test]
fn event_on_a_case() {
    for rel in ["work/queued", "ledger"] {
        refused(
            rel,
            rel,
            queued,
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
    }
}

/// `seldon index` writes the month views `ledger/*.md`; a ledger behind a
/// link is read (the index stays whole) but its views are not written.
#[test]
fn ledger_views() {
    let logged = |env: &Env, _: &Path| ok(env, &["log", "--", "first"]);
    refused_with(
        Start::Fresh,
        &[Swap::Link],
        "ledger",
        "ledger",
        logged,
        &["index"],
    );
}
