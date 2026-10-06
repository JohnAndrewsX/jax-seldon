//! Contract v2 (ADR-0035, WP-120): the case's risk in the ledger, the
//! autocommit result, a decision's cases and the triage proposal, end to
//! end through the commands. The cut marker is tested with the clip
//! (`index.rs`), the state-loss kind with the state reset
//! (`idempotency.rs`), the harm guard's record in `planned_link.rs`.

mod common;

use std::path::{Path, PathBuf};
use std::process::Output;

use common::{Env, Snapper, json, ledger, read, stderr, stdout};
use serde_json::{Value, json};

const T0: &str = "2026-10-06T10:00:00+02:00";
const T1: &str = "2026-10-06T11:00:00+02:00";
const T2: &str = "2026-10-06T12:00:00+02:00";

fn run(env: &Env, now: &str, args: &[&str]) -> Output {
    let mut all: Vec<&str> = args.to_vec();
    match all.iter().position(|a| *a == "--") {
        Some(at) => all.insert(at, "--json"),
        None => all.push("--json"),
    }
    env.at(now, &all)
}

fn ok(env: &Env, now: &str, args: &[&str]) -> Value {
    let out = run(env, now, args);
    assert_eq!(
        out.status.code(),
        Some(0),
        "{args:?}: {}{}",
        stdout(&out),
        stderr(&out)
    );
    json(&out)
}

fn refused(env: &Env, args: &[&str]) -> String {
    let out = run(env, T0, args);
    assert_eq!(out.status.code(), Some(1), "{args:?}: {}", stdout(&out));
    json(&out)["error"]["message"].as_str().unwrap().to_string()
}

fn index(env: &Env) -> Value {
    serde_json::from_str(&read(&env.home.join(".local/state/seldon/index.json"))).unwrap()
}

fn state(env: &Env) -> PathBuf {
    env.home.join(".local/state/seldon")
}

/// The case lines of `id` as `(kind, meta.risk)`, ledger order.
fn case_lines(root: &Path, id: &str) -> Vec<(String, Value)> {
    ledger(root)
        .into_iter()
        .filter(|e| e["source"] == "seldon" && e["subject"] == id)
        .filter(|e| e["kind"].as_str().unwrap().starts_with("case-"))
        .map(|e| {
            (
                e["kind"].as_str().unwrap().to_string(),
                e["meta"]["risk"].clone(),
            )
        })
        .collect()
}

/// ADR-0035 §1: `case-created` and `case-started` carry the risk, `plan
/// set` writes `case-updated` with the new one, the other steps none; a
/// retroactive case (`drift explain`) carries its risk too.
#[test]
fn every_new_case_line_carries_its_risk() {
    let env = Env::new(Snapper::Missing);
    let root = env.init_logbook();
    let id = ok(
        &env,
        T0,
        &[
            "plan", "new", "--zone", "red", "--risk", "R2", "--", "Kernel",
        ],
    )["case"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    ok(&env, T0, &["plan", "start", &id]);
    ok(&env, T1, &["plan", "set", &id, "--risk", "R3"]);
    ok(&env, T1, &["plan", "set", &id, "--zone", "yellow"]);
    ok(&env, T2, &["plan", "drop", &id, "--reason", "not needed"]);
    assert_eq!(
        case_lines(&root, &id),
        [
            ("case-created".into(), json!("R2")),
            ("case-started".into(), json!("R2")),
            ("case-updated".into(), json!("R3")),
            ("case-updated".into(), json!("R3")),
            ("case-dropped".into(), Value::Null),
        ]
    );

    // a change without a case, explained into a retroactive case
    let e = ok(
        &env,
        T1,
        &[
            "event",
            "config",
            "config-change",
            "--subject",
            "~/.config/hypr/hyprland.conf",
        ],
    );
    let event = e["event"]["id"].as_str().unwrap().to_string();
    let x = ok(
        &env,
        T2,
        &["drift", "explain", &event, "--risk", "R1", "--", "tuning"],
    );
    let made = x["case"]["id"].as_str().unwrap().to_string();
    assert_eq!(
        case_lines(&root, &made),
        [
            ("case-created".into(), json!("R1")),
            ("case-completed".into(), Value::Null),
        ]
    );
    // every line above validated against event.schema.json (common::ledger)
    ok(&env, T2, &["index", "--check"]);
}

/// The engine-only kinds and the index-only and engine-only meta keys are
/// not `seldon event`'s.
#[test]
fn event_refuses_the_v2_kinds_and_keys() {
    let env = Env::new(Snapper::Missing);
    env.init_logbook();
    for kind in ["case-updated", "state-loss"] {
        let m = refused(&env, &["event", "manual", kind, "--subject", "x"]);
        assert!(m.contains("not by `seldon event`"), "{m}");
    }
    let m = refused(
        &env,
        &[
            "event",
            "manual",
            "note",
            "--subject",
            "x",
            "--meta",
            "risk=R1",
        ],
    );
    assert!(m.contains("--meta risk"), "{m}");
    let m = refused(
        &env,
        &[
            "event",
            "manual",
            "note",
            "--subject",
            "x",
            "--meta",
            "truncated=true",
        ],
    );
    assert!(m.contains("index-only"), "{m}");
}

/// ADR-0035 §2: the last autocommit attempt, ok or failed, is in
/// `logbook.git.autocommit`; a skip changes nothing; with autocommit off
/// or a record of another logbook there is no field.
#[test]
fn the_autocommit_result_is_in_the_index() {
    let env = Env::new(Snapper::Missing);
    if !env.has_git {
        return;
    }
    let root = env.init_logbook();
    ok(&env, T0, &["log", "--", "first"]);
    let git = &index(&env)["logbook"]["git"];
    assert_eq!(
        git["autocommit"],
        json!({ "ok": true, "at": T0, "message": "seldon: note" }),
        "{git}"
    );

    // a stale lock: not committed, the error
    std::fs::write(root.join(".git/index.lock"), "").unwrap();
    ok(&env, T1, &["log", "--", "behind the lock"]);
    let a = index(&env)["logbook"]["git"]["autocommit"].clone();
    assert_eq!(a["ok"], json!(false), "{a}");
    assert_eq!(a["at"], json!(T1));
    assert!(a["message"].as_str().unwrap().contains("index.lock"), "{a}");
    assert!(!a["message"].as_str().unwrap().contains('\n'));
    std::fs::remove_file(root.join(".git/index.lock")).unwrap();

    // --no-commit is no attempt: the record stays
    ok(&env, T2, &["--no-commit", "log", "--", "uncommitted"]);
    assert_eq!(index(&env)["logbook"]["git"]["autocommit"], a);
    common::assert_valid_index(&index(&env));

    // another logbook's record is not this one's
    let file = state(&env).join("autocommit.json");
    let mut record: Value = serde_json::from_str(&read(&file)).unwrap();
    record["logbook"] = json!("/elsewhere");
    std::fs::write(&file, record.to_string()).unwrap();
    ok(&env, T2, &["index"]);
    assert!(index(&env)["logbook"]["git"].get("autocommit").is_none());

    // autocommit off: no field, whatever the record says
    ok(&env, T2, &["log", "--", "committed again"]);
    assert!(index(&env)["logbook"]["git"]["autocommit"]["ok"] == json!(true));
    let config = env.config_file();
    let text = read(&config).replace("autocommit = true", "autocommit = false");
    assert!(text.contains("autocommit = false"), "{text}");
    std::fs::write(&config, text).unwrap();
    ok(&env, T2, &["index"]);
    assert!(index(&env)["logbook"]["git"].get("autocommit").is_none());
}

/// A proposal file as `proposal.schema.json` has it.
fn proposal(id: &str, logbook: &Path, applied: Option<&str>, crisis: bool) -> Value {
    json!({
        "id": id,
        "at": "2026-10-06T10:30:00+02:00",
        "actor": "agent:claude-code",
        "logbook": std::fs::canonicalize(logbook).unwrap(),
        "applied": applied,
        "items": [
            { "eventId": "01M3VTGNY0NZG4AY80814WSKGR", "action": "link", "caseId": "C-2026-001",
              "crisis": false, "evidence": [ { "kind": "plan", "ref": "C-2026-001" } ] },
            { "eventId": "01M3VNJ9JGZ9169T01XCW16FT0", "action": "explain", "title": "t",
              "intent": "i", "crisis": crisis,
              "evidence": [ { "kind": "journal", "ref": "2026-10-06 10:20", "text": "x" } ] },
        ],
    })
}

/// ADR-0035 §6: `triage` points at the newest valid proposal of this
/// logbook; an invalid one is skipped with a warning, another logbook's
/// silently; absent without one.
#[test]
fn triage_points_at_the_newest_proposal_of_this_logbook() {
    let env = Env::new(Snapper::Missing);
    let root = env.init_logbook();
    let ix = ok(&env, T0, &["index", "--check"]);
    assert_eq!(ix["valid"], json!(true));
    assert!(index(&env).get("triage").is_none());

    let dir = state(&env).join("proposals");
    std::fs::create_dir_all(&dir).unwrap();
    let write = |id: &str, v: &Value| std::fs::write(dir.join(format!("{id}.json")), v.to_string());
    const OLD: &str = "01K6Y0000000000000000000A1";
    const MINE: &str = "01K6Y0000000000000000000B2";
    const OTHER: &str = "01K6Y0000000000000000000C3";
    const BROKEN: &str = "01K6Y0000000000000000000D4";
    write(OLD, &proposal(OLD, &root, None, false)).unwrap();
    write(MINE, &proposal(MINE, &root, None, true)).unwrap();
    let elsewhere = env.tmp.path();
    write(OTHER, &proposal(OTHER, elsewhere, None, true)).unwrap();
    let mut broken = proposal(BROKEN, &root, None, false);
    broken["items"][0]["evidence"] = json!([]);
    write(BROKEN, &broken).unwrap();
    std::fs::write(dir.join("notes.json"), "{}").unwrap();

    let ix = ok(&env, T1, &["index", "--check"]);
    assert_eq!(ix["valid"], json!(true), "{ix}");
    let warnings = ix["warnings"].to_string();
    assert!(
        warnings.contains(&format!("{BROKEN}.json: not a valid proposal")),
        "{warnings}"
    );
    assert!(!warnings.contains(OTHER), "{warnings}");
    assert_eq!(
        index(&env)["triage"],
        json!({
            "id": MINE,
            "at": "2026-10-06T10:30:00+02:00",
            "actor": "agent:claude-code",
            "counts": { "items": 2, "crises": 1 },
            "path": format!("proposals/{MINE}.json"),
            "applied": null,
        })
    );

    // applied: the time; a name that is not the id: skipped
    write(MINE, &proposal(MINE, &root, Some(T1), true)).unwrap();
    write(BROKEN, &proposal(OLD, &root, None, false)).unwrap();
    let ix = ok(&env, T2, &["index"]);
    assert!(ix["warnings"].to_string().contains("its id is"), "{ix}");
    assert_eq!(index(&env)["triage"]["applied"], json!(T1));
    common::assert_valid_index(&index(&env));
}

/// ADR-0035 §5: `decisions[].cases` as the frontmatter writes them,
/// without repeats; `[]` when it names none.
#[test]
fn decision_cases_are_listed_without_repeats() {
    let env = Env::new(Snapper::Missing);
    let root = env.init_logbook();
    let d = ok(&env, T0, &["decide", "--no-edit", "--", "Kernel policy"]);
    let path = root.join(d["decision"]["path"].as_str().unwrap());
    ok(&env, T0, &["index"]);
    assert_eq!(index(&env)["decisions"][0]["cases"], json!([]));
    let text = read(&path).replace("cases: []", "cases: [C-2026-002, C-2026-001, C-2026-002]");
    assert!(text.contains("C-2026-001"), "{text}");
    std::fs::write(&path, text).unwrap();
    ok(&env, T1, &["index", "--check"]);
    assert_eq!(
        index(&env)["decisions"][0]["cases"],
        json!(["C-2026-002", "C-2026-001"])
    );
}

/// `seldon index` twice at the same clock writes the same bytes, with
/// every v2 field present, and appends nothing to the ledger.
#[test]
fn index_is_idempotent_with_every_v2_field() {
    let env = Env::new(Snapper::Missing);
    if !env.has_git {
        return;
    }
    let root = env.init_logbook();
    ok(&env, T0, &["log", "--", "note"]);
    let dir = state(&env).join("proposals");
    std::fs::create_dir_all(&dir).unwrap();
    const ID: &str = "01K6Y0000000000000000000B2";
    std::fs::write(
        dir.join(format!("{ID}.json")),
        proposal(ID, &root, None, true).to_string(),
    )
    .unwrap();
    let lines = ledger(&root).len();
    ok(&env, T1, &["index"]);
    let first = read(&state(&env).join("index.json"));
    ok(&env, T1, &["index"]);
    let second = read(&state(&env).join("index.json"));
    assert_eq!(first, second);
    assert_eq!(ledger(&root).len(), lines);
    let v: Value = serde_json::from_str(&first).unwrap();
    assert_eq!(v["contractVersion"], json!(2));
    assert!(v["logbook"]["git"]["autocommit"].is_object(), "{v}");
    assert!(v["triage"].is_object());
}
