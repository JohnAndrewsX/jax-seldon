//! The act-then-account commands of ADR-0027 (WP-101): `plan set`,
//! `plan snapshot`, the agent's close (`plan done` refused without
//! evidence, tag `closed-by-agent`), `plan reopen`.

mod common;

use std::path::{Path, PathBuf};
use std::process::Output;

use common::{Env, Snapper, find_file, json, ledger, read, stderr, stdout};
use serde_json::Value;

const T0: &str = "2026-10-01T10:00:00+02:00";
const T1: &str = "2026-10-01T11:00:00+02:00";
const T2: &str = "2026-10-01T12:00:00+02:00";
const T3: &str = "2026-10-02T09:00:00+02:00";

/// `seldon args… --json` at `now`, with `SELDON_ACTOR` when given.
fn run(env: &Env, now: &str, actor: Option<&str>, args: &[&str]) -> Output {
    let mut all: Vec<&str> = args.to_vec();
    // `--json` before a `--` separator, as the plugin sends it
    match all.iter().position(|a| *a == "--") {
        Some(at) => all.insert(at, "--json"),
        None => all.push("--json"),
    }
    let mut cmd = env.command(&all);
    cmd.env("SELDON_NOW", now);
    if let Some(a) = actor {
        cmd.env("SELDON_ACTOR", a);
    }
    cmd.output().expect("run seldon")
}

fn ok(out: &Output) -> Value {
    assert_eq!(out.status.code(), Some(0), "{}{}", stdout(out), stderr(out));
    json(out)
}

/// Exit 1 with the JSON error message.
fn refused(out: &Output) -> String {
    assert_eq!(out.status.code(), Some(1), "{}{}", stdout(out), stderr(out));
    json(out)["error"]["message"].as_str().unwrap().to_string()
}

fn case_path(root: &Path, id: &str) -> PathBuf {
    for folder in ["queued", "active", "completed", "dropped"] {
        let dir = root.join("work").join(folder);
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for e in entries {
                let p = e.unwrap().path();
                if p.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with(&format!("{id}-"))
                {
                    return p;
                }
            }
        }
    }
    panic!("{id} not found");
}

fn show(env: &Env, id: &str) -> Value {
    ok(&run(env, T0, None, &["plan", "show", id]))["case"].clone()
}

fn log_lines(root: &Path, id: &str) -> Vec<String> {
    let text = read(&case_path(root, id));
    let start = text.find("## Log").unwrap();
    let end = text.rfind("## Result").unwrap();
    text[start..end]
        .lines()
        .filter(|l| l.starts_with("- "))
        .map(String::from)
        .collect()
}

/// Replaces `from` by `to` in the case file (the user or the agent
/// writing Markdown).
fn edit(root: &Path, id: &str, from: &str, to: &str) {
    let path = case_path(root, id);
    let text = read(&path);
    assert!(text.contains(from), "{from:?} not in {text}");
    std::fs::write(&path, text.replacen(from, to, 1)).unwrap();
}

/// A logbook with C-2026-001 started at T0 (yellow, R1, area `editors`).
fn logbook(env: &Env) -> PathBuf {
    let root = env.init_logbook();
    ok(&run(
        env,
        T0,
        None,
        &["plan", "new", "--area", "editors", "--", "Install zed"],
    ));
    ok(&run(env, T0, None, &["plan", "start", "C-2026-001"]));
    root
}

mod set {
    use super::*;

    #[test]
    fn changes_zone_risk_and_area_with_one_log_line() {
        let env = Env::new(Snapper::Missing);
        let root = logbook(&env);
        let before = ledger(&root).len();
        let v = ok(&run(
            &env,
            T1,
            Some("agent:claude-code"),
            &[
                "plan",
                "set",
                "C-2026-001",
                "--risk",
                "R3",
                "--zone",
                "red",
                "--area",
                "boot",
            ],
        ));
        assert_eq!(v["case"]["risk"], "R3");
        assert_eq!(v["case"]["zone"], "red");
        assert_eq!(v["case"]["area"], "boot");
        assert_eq!(
            v["case"]["agents"],
            serde_json::json!(["agent:claude-code"])
        );
        assert_eq!(v["areaCreated"], "areas/boot/README.md");
        assert_eq!(
            v["changed"],
            serde_json::json!([
                {"key": "zone", "from": "yellow", "to": "red"},
                {"key": "risk", "from": "R1", "to": "R3"},
                {"key": "area", "from": "editors", "to": "boot"},
            ])
        );
        // R3 without a snapshot: the advice, never a refusal
        let w = v["warnings"].as_array().unwrap();
        assert_eq!(w.len(), 1, "{w:?}");
        assert!(
            w[0].as_str()
                .unwrap()
                .contains("plan snapshot C-2026-001 <N>")
        );
        assert_eq!(
            log_lines(&root, "C-2026-001").last().unwrap(),
            "- 2026-10-01 11:00 · set zone yellow → red, risk R1 → R3, area editors → boot · \
             agent:claude-code"
        );
        // no ledger event: no kind fits, the Log line and the commit are the record
        assert_eq!(ledger(&root).len(), before);
        assert!(root.join("areas/boot/README.md").is_file());

        // the same values again: nothing changed, nothing written
        let file = read(&case_path(&root, "C-2026-001"));
        let v = ok(&run(
            &env,
            T2,
            None,
            &["plan", "set", "C-2026-001", "--risk", "R3"],
        ));
        assert_eq!(v["changed"], serde_json::json!([]));
        assert_eq!(read(&case_path(&root, "C-2026-001")), file);

        // human output
        let mut cmd = env.command(&["plan", "set", "C-2026-001", "--risk", "R2"]);
        cmd.env("SELDON_NOW", T2);
        let out = cmd.output().unwrap();
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        assert!(
            stdout(&out).starts_with("C-2026-001: risk R3 → R2\nwarning: C-2026-001 is R2"),
            "{}",
            stdout(&out)
        );
    }

    #[test]
    fn refusals() {
        let env = Env::new(Snapper::Missing);
        let root = logbook(&env);
        // nothing to set: clap's usage error, exit 1
        let out = run(&env, T1, None, &["plan", "set", "C-2026-001"]);
        assert_eq!(out.status.code(), Some(1));
        let m = refused(&run(
            &env,
            T1,
            None,
            &["plan", "set", "C-2026-001", "--area", "Not A Slug"],
        ));
        assert!(m.contains("is not a lowercase slug"), "{m}");
        let m = refused(&run(
            &env,
            T1,
            None,
            &["plan", "set", "C-2026-009", "--risk", "R2"],
        ));
        assert!(m.contains("unknown case C-2026-009"), "{m}");
        // a closed case
        ok(&run(&env, T1, None, &["plan", "verify", "C-2026-001"]));
        ok(&run(&env, T1, None, &["plan", "done", "C-2026-001"]));
        let file = read(&case_path(&root, "C-2026-001"));
        let m = refused(&run(
            &env,
            T2,
            None,
            &["plan", "set", "C-2026-001", "--risk", "R2"],
        ));
        assert!(
            m.contains("C-2026-001 is completed; `seldon plan set` changes an open case only")
                && m.contains("plan reopen C-2026-001"),
            "{m}"
        );
        assert_eq!(read(&case_path(&root, "C-2026-001")), file);
    }
}

mod snapshot {
    use super::*;

    /// `<tmp>/.snapshots/<n>/info.xml` (the guarded snapshot directory)
    /// with `date` in UTC.
    fn info(env: &Env, n: u64, date: &str) {
        let dir = env.tmp.path().join(".snapshots").join(n.to_string());
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("info.xml"),
            format!(
                "<?xml version=\"1.0\"?>\n<snapshot>\n  <type>single</type>\n  <num>{n}</num>\n  \
                 <date>{date}</date>\n  <description>C-2026-001</description>\n</snapshot>\n"
            ),
        )
        .unwrap();
    }

    #[test]
    fn records_the_number_once_and_checks_it() {
        let env = Env::new(Snapper::Missing);
        let root = logbook(&env);
        // 08:30 UTC is 10:30 local: after the start (10:00)
        info(&env, 42, "2026-10-01 08:30:00");
        let before = ledger(&root).len();
        let v = ok(&run(
            &env,
            T1,
            Some("agent:claude-code"),
            &["plan", "snapshot", "C-2026-001", "42"],
        ));
        assert_eq!(v["recorded"], true);
        assert_eq!(v["snapshot"], 42);
        assert_eq!(v["case"]["snapshotBefore"], 42);
        assert_eq!(v["warnings"], serde_json::json!([]));
        assert_eq!(
            log_lines(&root, "C-2026-001").last().unwrap(),
            "- 2026-10-01 11:00 · snapshot 42 · agent:claude-code"
        );
        assert_eq!(ledger(&root).len(), before, "no ledger event");

        // the same number again: nothing written
        let file = read(&case_path(&root, "C-2026-001"));
        let v = ok(&run(
            &env,
            T2,
            None,
            &["plan", "snapshot", "C-2026-001", "42"],
        ));
        assert_eq!(v["recorded"], false);
        assert_eq!(read(&case_path(&root, "C-2026-001")), file);
        // another number: exit 1, naming the one it has
        let m = refused(&run(
            &env,
            T2,
            None,
            &["plan", "snapshot", "C-2026-001", "43"],
        ));
        assert!(m.contains("C-2026-001 has snapshot 42 recorded"), "{m}");
        assert_eq!(read(&case_path(&root, "C-2026-001")), file);
        // 0 is not a snapshot
        let out = run(&env, T2, None, &["plan", "snapshot", "C-2026-001", "0"]);
        assert_eq!(out.status.code(), Some(1));
    }

    #[test]
    fn warns_about_a_snapshot_before_the_start_or_after_the_first_red_change() {
        let env = Env::new(Snapper::Missing);
        let root = logbook(&env);
        ok(&run(&env, T0, None, &["plan", "new", "--", "Second"]));
        ok(&run(&env, T0, None, &["plan", "start", "C-2026-002"]));
        // a red change of C-2026-002 at 10:15 local
        ok(&run(
            &env,
            "2026-10-01T10:15:00+02:00",
            None,
            &[
                "event",
                "pacman",
                "install",
                "--subject",
                "zed",
                "--case",
                "C-2026-002",
                "--actor",
                "agent:x",
            ],
        ));
        // 07:00 UTC = 09:00 local, before the start; 08:30 UTC after the red change
        info(&env, 40, "2026-10-01 07:00:00");
        info(&env, 41, "2026-10-01 08:30:00");
        let v = ok(&run(
            &env,
            T1,
            None,
            &["plan", "snapshot", "C-2026-001", "40"],
        ));
        let w = v["warnings"].as_array().unwrap();
        assert_eq!(w.len(), 1, "{w:?}");
        assert!(
            w[0].as_str().unwrap().starts_with(
                "snapshot 40 was taken at 2026-10-01 09:00:00, before C-2026-001 started at \
                 2026-10-01 10:00:00"
            ),
            "{w:?}"
        );
        // recorded anyway
        assert_eq!(v["case"]["snapshotBefore"], 40);
        let v = ok(&run(
            &env,
            T1,
            None,
            &["plan", "snapshot", "C-2026-002", "41"],
        ));
        let w = v["warnings"].as_array().unwrap();
        assert_eq!(w.len(), 1, "{w:?}");
        assert!(
            w[0].as_str()
                .unwrap()
                .contains("after C-2026-002's first red change (`zed` at 2026-10-01 10:15:00)"),
            "{w:?}"
        );
        // human output carries the warning lines
        assert!(read(&case_path(&root, "C-2026-002")).contains("snapshotBefore: 41"));
    }

    #[test]
    fn a_missing_or_uncheckable_snapshot_is_a_warning() {
        let env = Env::new(Snapper::Missing);
        let root = logbook(&env);
        ok(&run(&env, T0, None, &["plan", "new", "--", "Second"]));
        // no snapshot directory, nothing in the ledger
        let v = ok(&run(
            &env,
            T1,
            None,
            &["plan", "snapshot", "C-2026-001", "7"],
        ));
        let w = v["warnings"][0].as_str().unwrap();
        assert!(w.contains("cannot check") && w.contains("setfacl"), "{w}");
        // the directory lists other snapshots
        info(&env, 5, "2026-10-01 08:30:00");
        let v = ok(&run(
            &env,
            T1,
            None,
            &["plan", "snapshot", "C-2026-002", "8"],
        ));
        let w = v["warnings"][0].as_str().unwrap();
        assert!(w.starts_with("snapshot 8 does not exist"), "{w}");
        // a queued case takes one too; its start is not checked yet
        assert_eq!(show(&env, "C-2026-002")["snapshotBefore"], 8);
        assert!(root.join(".snapshots").exists() || env.tmp.path().join(".snapshots").exists());
    }

    #[test]
    fn start_with_snapshot_checks_the_first_red_change_only() {
        let env = Env::new(Snapper::Missing);
        env.init_logbook();
        ok(&run(&env, T0, None, &["plan", "new", "--", "One"]));
        // a snapshot from long before: no "before the start" warning at start
        info(&env, 3, "2026-09-01 08:00:00");
        let v = ok(&run(
            &env,
            T1,
            None,
            &["plan", "start", "C-2026-001", "--snapshot", "3"],
        ));
        assert_eq!(v["warnings"], serde_json::json!([]));
        // a number snapper does not have is named
        ok(&run(&env, T0, None, &["plan", "new", "--", "Two"]));
        let v = ok(&run(
            &env,
            T1,
            None,
            &["plan", "start", "C-2026-002", "--snapshot", "4"],
        ));
        assert!(
            v["warnings"][0]
                .as_str()
                .unwrap()
                .starts_with("snapshot 4 does not exist"),
            "{v}"
        );
    }

    #[test]
    fn a_closed_case_is_refused() {
        let env = Env::new(Snapper::Missing);
        let root = logbook(&env);
        ok(&run(
            &env,
            T1,
            None,
            &["plan", "drop", "C-2026-001", "--reason", "x"],
        ));
        let file = read(&case_path(&root, "C-2026-001"));
        let m = refused(&run(
            &env,
            T2,
            None,
            &["plan", "snapshot", "C-2026-001", "4"],
        ));
        assert!(m.contains("C-2026-001 is dropped"), "{m}");
        assert_eq!(read(&case_path(&root, "C-2026-001")), file);
    }
}

mod close {
    use super::*;

    const RESULT: &str = "## Result\n";
    const VERIFICATION: &str = "- Verification:\n";

    fn verified(env: &Env, root: &Path) {
        ok(&run(env, T1, None, &["plan", "verify", "C-2026-001"]));
        assert!(read(&case_path(root, "C-2026-001")).contains(VERIFICATION));
    }

    #[test]
    fn an_agent_close_without_evidence_is_refused_with_the_reason() {
        let env = Env::new(Snapper::Missing);
        let root = logbook(&env);
        verified(&env, &root);
        let before = ledger(&root).len();
        let file = read(&case_path(&root, "C-2026-001"));
        // by --actor, and by SELDON_ACTOR alone: the resolved actor counts
        for (actor, args) in [
            (
                None,
                &["plan", "done", "C-2026-001", "--actor", "agent:x"][..],
            ),
            (Some("agent:x"), &["plan", "done", "C-2026-001"]),
        ] {
            let m = refused(&run(&env, T2, actor, args));
            assert_eq!(
                m,
                "C-2026-001 is not closed: agent:x closes a case only with its evidence, and its \
                 Result is empty and its Plan › Verification is not filled in; fill it in, then \
                 run `seldon plan done C-2026-001` again (ADR-0027 §5)"
            );
        }
        assert_eq!(ledger(&root).len(), before);
        assert_eq!(read(&case_path(&root, "C-2026-001")), file);

        // a comment is not a Result; a filled Result alone is not enough
        edit(&root, "C-2026-001", RESULT, "## Result\n<!-- todo -->\n");
        let m = refused(&run(
            &env,
            T2,
            Some("agent:x"),
            &["plan", "done", "C-2026-001"],
        ));
        assert!(m.contains("its Result is empty and"), "{m}");
        edit(
            &root,
            "C-2026-001",
            "<!-- todo -->\n",
            "`zed --version` printed 0.150.\n",
        );
        let m = refused(&run(
            &env,
            T2,
            Some("agent:x"),
            &["plan", "done", "C-2026-001"],
        ));
        assert!(
            m.contains("and its Plan › Verification is not filled in") && !m.contains("Result"),
            "{m}"
        );
        // a verification on the lines below the item counts
        edit(
            &root,
            "C-2026-001",
            VERIFICATION,
            "- Verification:\n  - `pacman -Q zed` shows the package\n",
        );
        let v = ok(&run(
            &env,
            T2,
            Some("agent:x"),
            &["plan", "done", "C-2026-001"],
        ));
        assert_eq!(v["to"], "completed");
        assert_eq!(v["case"]["tags"], serde_json::json!(["closed-by-agent"]));
        assert_eq!(v["event"]["actor"], "agent:x");
        assert_eq!(
            log_lines(&root, "C-2026-001").last().unwrap(),
            "- 2026-10-01 12:00 · completed · agent:x"
        );
    }

    #[test]
    fn a_human_close_is_never_refused_and_gets_no_tag() {
        let env = Env::new(Snapper::Missing);
        let root = logbook(&env);
        verified(&env, &root);
        let v = ok(&run(&env, T2, None, &["plan", "done", "C-2026-001"]));
        assert_eq!(v["to"], "completed");
        assert_eq!(v["case"]["tags"], serde_json::json!([]));
        assert_eq!(v["event"]["actor"], "human");
    }

    #[test]
    fn the_verification_line_may_hold_the_text() {
        let env = Env::new(Snapper::Missing);
        let root = logbook(&env);
        verified(&env, &root);
        edit(&root, "C-2026-001", RESULT, "## Result\nWorks.\n");
        edit(
            &root,
            "C-2026-001",
            VERIFICATION,
            "- verification: `zed --version` exits 0\n",
        );
        let v = ok(&run(
            &env,
            T2,
            None,
            &["plan", "done", "C-2026-001", "--actor", "agent:x"],
        ));
        assert_eq!(v["case"]["tags"], serde_json::json!(["closed-by-agent"]));
    }
}

mod reopen {
    use super::*;

    fn completed(env: &Env) -> PathBuf {
        let root = logbook(env);
        edit(
            &root,
            "C-2026-001",
            "## Intent\n<!-- Why this case? What should be different afterwards? -->\n",
            "## Intent\nA second editor, `zed`.\n\nWith its docs.\n",
        );
        ok(&run(
            env,
            T0,
            None,
            &["plan", "set", "C-2026-001", "--zone", "red", "--risk", "R2"],
        ));
        ok(&run(env, T1, None, &["plan", "verify", "C-2026-001"]));
        ok(&run(env, T1, None, &["plan", "done", "C-2026-001"]));
        root
    }

    #[test]
    fn makes_a_new_active_case_with_the_intent_and_the_tag() {
        let env = Env::new(Snapper::Missing);
        let root = completed(&env);
        let before = ledger(&root).len();
        let v = ok(&run(&env, T3, None, &["plan", "reopen", "C-2026-001"]));
        let c = &v["case"];
        assert_eq!(c["id"], "C-2026-002");
        assert_eq!(c["title"], "Reopen: Install zed");
        assert_eq!(c["status"], "active");
        assert_eq!(
            (&c["zone"], &c["risk"], &c["area"], &c["priority"]),
            (
                &Value::from("red"),
                &Value::from("R2"),
                &Value::from("editors"),
                &Value::from("normal")
            )
        );
        assert_eq!(c["tags"], serde_json::json!(["reopens:C-2026-001"]));
        assert_eq!(c["started"], "2026-10-02");
        assert_eq!(v["reopens"], "C-2026-001");
        assert_eq!(v["earlier"], serde_json::json!([]));
        assert_eq!(v["activeCase"]["set"], "C-2026-002");
        common::assert_valid_case(c);
        let text = read(&case_path(&root, "C-2026-002"));
        assert!(
            text.contains("## Intent\nA second editor, `zed`.\n\nWith its docs.\n\n## Plan\n"),
            "{text}"
        );
        assert_eq!(
            log_lines(&root, "C-2026-002"),
            [
                "- 2026-10-02 09:00 · created (zone red, risk R2): reopens C-2026-001 · human",
                "- 2026-10-02 09:00 · started · human",
            ]
        );
        assert_eq!(
            log_lines(&root, "C-2026-001").last().unwrap(),
            "- 2026-10-02 09:00 · reopened as C-2026-002 · human"
        );
        // the old case stays completed, in its folder
        assert!(
            case_path(&root, "C-2026-001")
                .to_string_lossy()
                .contains("/work/completed/")
        );
        assert_eq!(show(&env, "C-2026-001")["status"], "completed");
        // ledger: case-created and case-started of the new case
        let events = ledger(&root);
        let new: Vec<(&str, &str)> = events[before..]
            .iter()
            .map(|e| (e["kind"].as_str().unwrap(), e["subject"].as_str().unwrap()))
            .collect();
        assert_eq!(
            new,
            [
                ("case-created", "C-2026-002"),
                ("case-started", "C-2026-002")
            ]
        );
        assert_eq!(read(&root.join(".seldon/active-case")).trim(), "C-2026-002");
    }

    #[test]
    fn a_second_reopen_is_a_second_case_and_says_so() {
        let env = Env::new(Snapper::Missing);
        let root = completed(&env);
        ok(&run(&env, T3, None, &["plan", "reopen", "C-2026-001"]));
        let v = ok(&run(
            &env,
            T3,
            Some("agent:claude-code"),
            &["plan", "reopen", "C-2026-001"],
        ));
        assert_eq!(v["case"]["id"], "C-2026-003");
        assert_eq!(v["earlier"], serde_json::json!(["C-2026-002"]));
        assert_eq!(
            v["case"]["agents"],
            serde_json::json!(["agent:claude-code"])
        );
        let mut cmd = env.command(&["plan", "reopen", "C-2026-001"]);
        cmd.env("SELDON_NOW", T3);
        let out = cmd.output().unwrap();
        assert!(
            stdout(&out).contains("Reopened before as C-2026-002, C-2026-003; this is a new case"),
            "{}",
            stdout(&out)
        );
        find_file(&root.join("work/active"), "C-2026-004-");
    }

    #[test]
    fn only_a_completed_case_reopens() {
        let env = Env::new(Snapper::Missing);
        let root = logbook(&env);
        let before = ledger(&root).len();
        let m = refused(&run(&env, T1, None, &["plan", "reopen", "C-2026-001"]));
        assert_eq!(
            m,
            "C-2026-001 is active; `seldon plan reopen` reopens a completed case only"
        );
        ok(&run(
            &env,
            T1,
            None,
            &["plan", "drop", "C-2026-001", "--reason", "x"],
        ));
        let m = refused(&run(&env, T1, None, &["plan", "reopen", "C-2026-001"]));
        assert!(m.contains("is dropped"), "{m}");
        assert_eq!(ledger(&root).len(), before + 1);
    }
}
