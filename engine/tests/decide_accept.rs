//! `seldon decide accept <ADR-NNNN>` (WP-135, ADR-0040; SPEC-ENGINE §3):
//! the user accepts a proposed decision — status and date in its
//! frontmatter, one `seldon` note in the ledger, the commit and the index.

mod common;

use std::path::{Path, PathBuf};

use common::{Env, Snapper, find_file, index_errors, json, ledger, read, stderr, stdout, tree};
use seldon::model::{self, Decision, DecisionStatus};

const T0: &str = "2026-10-01T15:30:00+02:00";
const T1: &str = "2026-10-07T09:12:00+02:00";
const AGENT: &str = "agent:claude-code";

/// A logbook with case C-2026-001 and decision ADR-0001 (proposed, naming
/// the case) made at `T0`; returns the logbook and the decision's path.
fn proposed(env: &Env) -> (PathBuf, PathBuf) {
    let root = env.init_logbook();
    env.at(T0, &["plan", "new", "--", "Zed"]);
    let out = env.at(
        T0,
        &[
            "decide",
            "--no-edit",
            "--case",
            "C-2026-001",
            "--",
            "Zed statt VS Code",
        ],
    );
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let path = find_file(&root.join("decisions"), "ADR-0001-");
    (root, path)
}

fn index(env: &Env) -> serde_json::Value {
    let text = read(&env.home.join(".local/state/seldon/index.json"));
    serde_json::from_str(&text).unwrap()
}

fn head(env: &Env, root: &Path) -> String {
    stdout(&env.git(root, &["log", "-1", "--format=%H %s"]))
}

#[test]
fn accepts_a_proposed_decision() {
    let env = Env::new(Snapper::Missing);
    let (root, path) = proposed(&env);
    let before = read(&path);
    let events = ledger(&root).len();

    let out = env.at(T1, &["decide", "accept", "ADR-0001", "--json"]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let v = json(&out);
    assert_eq!(v["already"], false);
    assert_eq!(v["decision"]["id"], "ADR-0001");
    assert_eq!(v["decision"]["status"], "accepted");
    assert_eq!(v["decision"]["date"], "2026-10-07");
    assert_eq!(v["decision"]["cases"], serde_json::json!(["C-2026-001"]));
    assert_eq!(
        v["decision"]["path"],
        "decisions/ADR-0001-zed-statt-vs-code.md"
    );

    // two keys change, every other byte stays
    let after = read(&path);
    assert_eq!(
        after,
        before
            .replace("status: proposed\n", "status: accepted\n")
            .replace("date: 2026-10-01\n", "date: 2026-10-07\n")
    );
    let (d, _) = model::load::<Decision>(&path).unwrap();
    assert_eq!(d.status, DecisionStatus::Accepted);
    assert_eq!(d.date.to_string(), "2026-10-07");

    // one `seldon` note, the user's
    let lines = ledger(&root);
    assert_eq!(lines.len(), events + 1);
    let e = lines.last().unwrap();
    assert_eq!(v["event"], *e);
    assert_eq!(e["source"], "seldon");
    assert_eq!(e["kind"], "note");
    assert_eq!(e["subject"], "ADR-0001");
    assert_eq!(e["actor"], "human");
    assert_eq!(e["detail"], "accepted: Zed statt VS Code");
    assert_eq!(e["ts"], T1);
    assert!(e.get("case").is_none(), "{e}");

    // DECISIONS.md, the index, the commit
    let decisions = read(&root.join("DECISIONS.md"));
    assert!(
        decisions.contains("| [[ADR-0001]] | Zed statt VS Code | accepted | 2026-10-07 |"),
        "{decisions}"
    );
    let ix = index(&env);
    assert_eq!(index_errors(&ix), Vec::<String>::new());
    let d = &ix["decisions"][0];
    assert_eq!(
        (&d["status"], &d["date"]),
        (&"accepted".into(), &"2026-10-07".into())
    );
    assert!(
        ix["events"]
            .as_array()
            .unwrap()
            .iter()
            .any(|x| x["id"] == e["id"])
    );
    if env.has_git {
        assert!(head(&env, &root).ends_with(" seldon: ADR-0001 accepted\n"));
        assert_eq!(v["git"]["committed"], true);
        let status = stdout(&env.git(&root, &["status", "--porcelain"]));
        assert_eq!(status, "", "everything committed");
    }

    // the human line
    let out = env.at(T1, &["plan", "new", "--", "other"]);
    assert_eq!(out.status.code(), Some(0));
    env.at(T1, &["decide", "--no-edit", "--", "Second"]);
    let out = env.at(T1, &["decide", "accept", "ADR-0002"]);
    assert_eq!(
        stdout(&out),
        "Accepted ADR-0002 \"Second\" in decisions/ADR-0002-second.md\n"
    );
}

#[test]
fn a_second_accept_changes_nothing() {
    let env = Env::new(Snapper::Missing);
    let (root, _) = proposed(&env);
    let out = env.at(T1, &["decide", "accept", "ADR-0001"]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let files = tree(&root);
    let commit = env.has_git.then(|| head(&env, &root));

    for now in [T1, "2026-10-09T08:00:00+02:00"] {
        let out = env.at(now, &["decide", "accept", "ADR-0001", "--json"]);
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        let v = json(&out);
        assert_eq!(v["already"], true);
        assert!(v["event"].is_null());
        assert_eq!(v["git"]["committed"], false);
        assert_eq!(v["decision"]["status"], "accepted");
        assert_eq!(
            v["decision"]["date"], "2026-10-07",
            "the day it was accepted"
        );
        assert!(tree(&root) == files, "nothing written");
        assert_eq!(env.has_git.then(|| head(&env, &root)), commit);
    }
    let out = env.at(T1, &["decide", "accept", "ADR-0001"]);
    assert_eq!(
        stdout(&out),
        "ADR-0001 is accepted already; nothing changed\n"
    );
}

#[test]
fn refuses_a_decision_that_is_not_proposed() {
    let env = Env::new(Snapper::Missing);
    let (root, path) = proposed(&env);
    std::fs::write(
        &path,
        read(&path).replace("status: proposed\n", "status: superseded\n"),
    )
    .unwrap();
    let files = tree(&root);
    for (args, want) in [
        (
            vec!["decide", "accept", "ADR-0001"],
            "ADR-0001 is superseded; only a proposed decision is accepted",
        ),
        (
            vec!["decide", "accept", "ADR-0009"],
            "unknown decision ADR-0009",
        ),
    ] {
        let out = env.at(T1, &args);
        assert_eq!(out.status.code(), Some(1), "{args:?}");
        assert!(stderr(&out).contains(want), "{args:?}: {}", stderr(&out));
        assert!(tree(&root) == files, "{args:?}: nothing written");
    }

    // a frontmatter that names another decision: refused, not rewritten
    std::fs::write(
        &path,
        read(&path)
            .replace("status: superseded\n", "status: proposed\n")
            .replace("id: ADR-0001\n", "id: ADR-0007\n"),
    )
    .unwrap();
    let files = tree(&root);
    let out = env.at(T1, &["decide", "accept", "ADR-0001"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(
        stderr(&out).contains("names ADR-0007 in its frontmatter, not ADR-0001"),
        "{}",
        stderr(&out)
    );
    assert!(tree(&root) == files);

    // a frontmatter that does not read (round 2, N1): refused, not rewritten
    std::fs::write(
        &path,
        read(&path).replace("id: ADR-0007\n", "id: [ADR-0001\n"),
    )
    .unwrap();
    let files = tree(&root);
    let out = env.at(T1, &["decide", "accept", "ADR-0001"]);
    assert_eq!(out.status.code(), Some(1), "{}", stderr(&out));
    assert!(
        stderr(&out).contains("invalid frontmatter"),
        "{}",
        stderr(&out)
    );
    assert!(tree(&root) == files);

    // ids are checked by the parser; --json reports it as JSON
    let out = env.at(T1, &["decide", "accept", "ADR-1", "--json"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(
        json(&out)["error"]["message"]
            .as_str()
            .unwrap()
            .contains("`ADR-1` is not a decision id (ADR-NNNN)")
    );
}

#[test]
fn an_agent_never_accepts() {
    let env = Env::new(Snapper::Missing);
    let (root, _) = proposed(&env);
    let files = tree(&root);
    let commit = env.has_git.then(|| head(&env, &root));
    let agent = "agent:claude-code may propose a decision (`seldon decide`), only the user \
                 accepts one (ADR-0040)";
    for (args, vars, want) in [
        (
            vec!["decide", "accept", "ADR-0001", "--actor", AGENT],
            vec![],
            agent,
        ),
        (
            vec!["decide", "accept", "ADR-0001"],
            vec![("SELDON_ACTOR", AGENT)],
            agent,
        ),
        (
            vec!["decide", "accept", "ADR-0001", "--actor", "human"],
            vec![("SELDON_ACTOR", AGENT)],
            "`--actor human` in a session of agent:claude-code (SELDON_ACTOR)",
        ),
        (
            vec!["decide", "accept", "ADR-0001", "--actor", "system"],
            vec![],
            "`system` cannot write this",
        ),
        // a session whose actor does not read may be an agent's (round 2, N3)
        (
            vec!["decide", "accept", "ADR-0001", "--actor", "human"],
            vec![("SELDON_ACTOR", "agent:Not Valid")],
            "ADR-0001 is not accepted: SELDON_ACTOR",
        ),
        (
            vec!["decide", "accept", "ADR-0001", "--actor", "human"],
            vec![("SELDON_ACTOR", "system")],
            "fix or unset SELDON_ACTOR",
        ),
        (
            vec!["decide", "accept", "ADR-0001"],
            vec![("SELDON_ACTOR", "nobody")],
            "ADR-0001 is not accepted: SELDON_ACTOR",
        ),
    ] {
        let out = env
            .command(&args)
            .env("SELDON_NOW", T1)
            .envs(vars.iter().copied())
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(1), "{args:?} {vars:?}");
        assert!(
            stderr(&out).contains(want),
            "{args:?} {vars:?}: {}",
            stderr(&out)
        );
        assert!(tree(&root) == files, "{args:?} {vars:?}: nothing written");
        assert_eq!(env.has_git.then(|| head(&env, &root)), commit);
    }

    // refused before anything is read: no logbook needed for the refusal
    let bare = Env::new(Snapper::Missing);
    let out = bare
        .command(&["decide", "accept", "ADR-0001", "--actor", AGENT])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1), "{}", stderr(&out));

    // a person's own session accepts
    let out = env
        .command(&["decide", "accept", "ADR-0001", "--json"])
        .env("SELDON_NOW", T1)
        .env("SELDON_ACTOR", "human")
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert_eq!(json(&out)["event"]["actor"], "human");
}

#[test]
fn accept_is_a_subcommand_only_before_the_separator() {
    let env = Env::new(Snapper::Missing);
    let root = env.init_logbook();
    // a decision titled "accept" goes after `--`, as the plugin passes titles
    let out = env.at(T0, &["decide", "--no-edit", "--json", "--", "accept"]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert_eq!(json(&out)["decision"]["title"], "accept");
    assert!(root.join("decisions/ADR-0001-accept.md").is_file());
    // the title form's options do not mix with the subcommand
    for args in [
        vec!["decide", "accept"],
        vec!["decide", "--no-edit", "accept", "ADR-0001"],
        vec!["decide"],
    ] {
        let out = env.at(T0, &args);
        assert_eq!(out.status.code(), Some(1), "{args:?}: {}", stderr(&out));
    }
    let (d, _) = model::load::<Decision>(&root.join("decisions/ADR-0001-accept.md")).unwrap();
    assert_eq!(d.status, DecisionStatus::Proposed);
}

/// The ledger first, as a plan step (round 2, B1): when the ledger cannot
/// be written, the decision stays proposed and nothing else changes.
#[test]
fn a_ledger_failure_accepts_nothing() {
    let env = Env::new(Snapper::Missing);
    let (root, path) = proposed(&env);

    // 1. the ledger refuses to start: an invalid redaction pattern (exit 1)
    let bad = env.tmp.path().join("bad-redaction.toml");
    std::fs::write(
        &bad,
        format!(
            "logbook = \"{}\"\n[redaction]\npatterns = [\"(\"]\n",
            root.display()
        ),
    )
    .unwrap();
    let files = tree(&root);
    let commit = env.has_git.then(|| head(&env, &root));
    let config = bad.to_str().unwrap();
    let out = env.at(T1, &["--config", config, "decide", "accept", "ADR-0001"]);
    assert_eq!(out.status.code(), Some(1), "{}", stderr(&out));
    assert!(
        stderr(&out).contains("[redaction] patterns"),
        "{}",
        stderr(&out)
    );
    assert!(tree(&root) == files, "nothing written, the decision too");
    assert_eq!(env.has_git.then(|| head(&env, &root)), commit);

    // 2. the ledger cannot be written: a new month in a read-only ledger/
    //    (exit 2); skipped where permissions do not bind (root)
    let ledger_dir = root.join("ledger");
    let mut perms = std::fs::metadata(&ledger_dir).unwrap().permissions();
    std::os::unix::fs::PermissionsExt::set_mode(&mut perms, 0o555);
    std::fs::set_permissions(&ledger_dir, perms.clone()).unwrap();
    let probe = ledger_dir.join(".probe");
    if std::fs::write(&probe, "").is_ok() {
        let _ = std::fs::remove_file(&probe);
    } else {
        let out = env.at(
            "2026-11-02T08:00:00+01:00",
            &["decide", "accept", "ADR-0001"],
        );
        assert_eq!(out.status.code(), Some(2), "{}", stderr(&out));
        assert!(tree(&root) == files, "nothing written, the decision too");
    }
    std::os::unix::fs::PermissionsExt::set_mode(&mut perms, 0o755);
    std::fs::set_permissions(&ledger_dir, perms).unwrap();
    let (d, _) = model::load::<Decision>(&path).unwrap();
    assert_eq!(d.status, DecisionStatus::Proposed);
}
