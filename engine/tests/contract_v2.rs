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
    // N1: the refusal names the command that writes the kind
    for (kind, writer) in [
        ("case-updated", "`seldon plan`"),
        ("state-loss", "`seldon capture`"),
        ("case-created", "`seldon plan`"),
        ("resolution", "`seldon drift`"),
    ] {
        let m = refused(&env, &["event", "manual", kind, "--subject", "x"]);
        assert!(
            m.contains(&format!("written by {writer}, not by `seldon event`")),
            "{m}"
        );
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
    // ADR-0043: the pacman collector's key
    let m = refused(
        &env,
        &[
            "event",
            "manual",
            "note",
            "--subject",
            "x",
            "--meta",
            "txStatus=interrupted",
        ],
    );
    assert!(m.contains("--meta txStatus"), "{m}");
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
    std::fs::write(dir.join(format!("{}.json", MINE.to_lowercase())), "{}").unwrap();
    std::fs::write(dir.join("README"), "not json").unwrap();

    let ix = ok(&env, T1, &["index", "--check"]);
    assert_eq!(ix["valid"], json!(true), "{ix}");
    let warnings = ix["warnings"].to_string();
    assert!(
        warnings.contains(&format!("{BROKEN}.json: not a valid proposal")),
        "{warnings}"
    );
    assert!(!warnings.contains(OTHER), "{warnings}");
    // N2: a `.json` not named `<ULID>.json` is named, other files are not
    assert!(
        warnings.contains("notes.json: not named <ULID>.json"),
        "{warnings}"
    );
    assert!(
        warnings.contains(&format!("{}.json: not named", MINE.to_lowercase())),
        "{warnings}"
    );
    assert!(!warnings.contains("README"), "{warnings}");
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
    assert_eq!(v["contractReadableFrom"], json!(2), "ADR-0051");
    assert!(v["logbook"]["git"]["autocommit"].is_object(), "{v}");
    assert!(v["triage"].is_object());
}

/// WP-120 round 2, B1 (ADR-0035 §1): 0.1.x let `seldon event --meta
/// risk=…` write any value on any kind. Such a ledger still loads whole
/// (no line skipped), indexes valid (`--check` exit 0) without the user's
/// `risk` and `truncated` in the index, and doctor's ledger row stays ok.
#[test]
fn a_contract_1_ledger_with_a_user_risk_still_indexes() {
    let env = Env::new(Snapper::Missing);
    let root = env.init_logbook();
    let mut lines = String::new();
    for (n, meta) in [
        (1, json!({ "risk": "R1" })),
        (2, json!({ "risk": "banana" })),
        (
            3,
            json!({ "risk": "high", "truncated": "yes", "mine": "kept" }),
        ),
    ] {
        let note = json!({
            "id": format!("01K6Y00000000000000000000{n}"),
            "ts": format!("2026-10-06T09:0{n}:00+02:00"),
            "source": "manual", "kind": "note", "subject": "journal",
            "detail": format!("v1 note {n}"), "actor": "human", "meta": meta,
        });
        lines.push_str(&format!("{note}\n"));
    }
    let month = root.join("ledger/2026-10.jsonl");
    let mut text = std::fs::read_to_string(&month).unwrap_or_default();
    text.push_str(&lines);
    std::fs::write(&month, text).unwrap();

    let ix = ok(&env, T0, &["index", "--check"]);
    assert_eq!(ix["valid"], json!(true), "{ix}");
    assert_eq!(ix["warnings"], json!([]), "no line skipped: {ix}");
    let v = index(&env);
    common::assert_valid_index(&v);
    let notes: Vec<&Value> = v["events"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| {
            e["detail"]
                .as_str()
                .is_some_and(|d| d.starts_with("v1 note"))
        })
        .collect();
    assert_eq!(notes.len(), 3, "{v}");
    for n in &notes {
        assert!(n["meta"].get("risk").is_none(), "{n}");
        assert!(n["meta"].get("truncated").is_none(), "{n}");
    }
    assert!(notes.iter().any(|n| n["meta"] == json!({ "mine": "kept" })));

    let doctor = json(&run(&env, T0, &["doctor"]));
    let row = doctor["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["name"] == "ledger")
        .cloned()
        .unwrap_or_else(|| panic!("no ledger row: {doctor}"));
    assert_eq!(row["status"], json!("ok"), "{row}");
}

/// ADR-0043: `meta.txStatus` reaches the index on a pacman event with a
/// `txId`; a hand-edited one on a note, on a pacman line outside a
/// transaction, or with another word is dropped, and the index stays
/// valid.
#[test]
fn a_tx_status_is_kept_only_on_a_transaction_line() {
    let env = Env::new(Snapper::Missing);
    let root = env.init_logbook();
    let mut lines = String::new();
    for (n, source, kind, subject, tx, status) in [
        (1, "pacman", "upgrade", "gtk4", true, "interrupted"),
        (2, "pacman", "upgrade", "libadwaita", true, "banana"),
        (3, "pacman", "install", "btop", false, "failed"),
        (4, "manual", "note", "journal", false, "unfinished"),
    ] {
        let mut e = json!({
            "id": format!("01K6Y00000000000000000000{n}"),
            "ts": format!("2026-10-06T09:0{n}:00+02:00"),
            "source": source, "kind": kind, "subject": subject,
            "detail": format!("tx line {n}"), "actor": "human",
            "meta": { "txStatus": status },
        });
        if tx {
            e["txId"] = json!("tx-20261006T090100");
        }
        lines.push_str(&format!("{e}\n"));
    }
    let month = root.join("ledger/2026-10.jsonl");
    let mut text = std::fs::read_to_string(&month).unwrap_or_default();
    text.push_str(&lines);
    std::fs::write(&month, text).unwrap();

    let ix = ok(&env, T0, &["index", "--check"]);
    assert_eq!(ix["valid"], json!(true), "{ix}");
    let v = index(&env);
    common::assert_valid_index(&v);
    let status = |n: usize| {
        let e = v["events"]
            .as_array()
            .unwrap()
            .iter()
            .find(|e| e["detail"] == format!("tx line {n}"))
            .unwrap_or_else(|| panic!("tx line {n}: {v}"));
        e["meta"]["txStatus"].clone()
    };
    assert_eq!(status(1), json!("interrupted"));
    for n in 2..=4 {
        assert_eq!(status(n), Value::Null, "tx line {n}");
    }
}

/// WP-120 round 2, B2 and N6: a failed autocommit whose git error carries
/// secrets (a refusing pre-commit hook prints them) shows them nowhere:
/// not on stderr, not in `--json` `git.error`, not in `autocommit.json`,
/// not in `logbook.git.autocommit`; the index's message is one line of at
/// most 256 characters. A record planted unredacted (an older engine, a
/// hand edit) is redacted again when the index is built.
#[test]
fn a_failed_autocommit_is_redacted_everywhere() {
    const SECRETS: [&str; 3] = ["geheim", "abc123geheim", "hunter2"];
    let env = Env::new(Snapper::Missing);
    if !env.has_git {
        return;
    }
    let root = env.init_logbook();
    let padding = "x".repeat(300);
    common::write_executable(
        &root.join(".git/hooks/pre-commit"),
        &format!(
            "#!/bin/sh\necho 'refused: https://user:geheim@example.org token=abc123geheim \
             --password hunter2 {padding}' >&2\nexit 1\n"
        ),
    );
    let out = run(&env, T0, &["log", "--", "behind the hook"]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let j = json(&out);
    let error = j["git"]["error"].as_str().unwrap().to_string();
    let record = read(&state(&env).join("autocommit.json"));
    let a = index(&env)["logbook"]["git"]["autocommit"].clone();
    let message = a["message"].as_str().unwrap().to_string();
    for (what, text) in [
        ("stderr", stderr(&out)),
        ("--json git.error", error.clone()),
        ("autocommit.json", record.clone()),
        ("index", message.clone()),
    ] {
        for secret in SECRETS {
            assert!(!text.contains(secret), "{what} shows {secret}: {text}");
        }
        assert!(text.contains("‹redacted›"), "{what}: {text}");
    }
    assert_eq!(a["ok"], json!(false));
    assert!(message.chars().count() <= 256, "{message}");
    assert!(message.ends_with('…'), "{message}");
    assert!(!message.contains('\n'));

    // a record written unredacted is redacted at build time
    let mut planted: Value = serde_json::from_str(&record).unwrap();
    planted["message"] = json!("not committed: token=abc123geheim");
    std::fs::write(state(&env).join("autocommit.json"), planted.to_string()).unwrap();
    ok(&env, T1, &["index"]);
    let m = index(&env)["logbook"]["git"]["autocommit"]["message"].clone();
    assert_eq!(m, json!("not committed: token=‹redacted›"), "{m}");
}

/// `seldon args… --json` at `now`, killed after `limit`: `None` when it
/// did not finish (a reader blocked on a FIFO would hang the build).
fn run_within(env: &Env, now: &str, args: &[&str], limit: std::time::Duration) -> Option<Output> {
    let mut all = args.to_vec();
    all.push("--json");
    let mut child = env
        .command(&all)
        .env("SELDON_NOW", now)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let start = std::time::Instant::now();
    while start.elapsed() < limit {
        if child.try_wait().unwrap().is_some() {
            return Some(child.wait_with_output().unwrap());
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let _ = child.kill();
    let _ = child.wait();
    None
}

/// A FIFO at `path`, made by `mkfifo` (no shell string; the test is
/// skipped where there is none).
fn mkfifo(path: &Path) -> bool {
    ["/usr/bin/mkfifo", "/bin/mkfifo"]
        .iter()
        .find(|p| Path::new(p).exists())
        .is_some_and(|p| {
            std::process::Command::new(p)
                .arg(path)
                .status()
                .is_ok_and(|s| s.success())
        })
}

/// WP-120 round 3 (ADR-0035 §6, SPEC-ENGINE §2): the index reads a
/// proposal or `autocommit.json` only when it is a regular file of at most
/// 4 MiB. A FIFO (never opened: it would block), a symbolic link (here to
/// `/dev/zero`) and a 5 MiB file are each skipped with a warning, and the
/// build completes promptly with a valid index.
#[test]
fn state_files_that_are_no_regular_small_files_are_skipped() {
    let env = Env::new(Snapper::Missing);
    if !env.has_git {
        return;
    }
    let root = env.init_logbook();
    ok(&env, T0, &["log", "--", "a commit for the record"]);
    let dir = state(&env).join("proposals");
    std::fs::create_dir_all(&dir).unwrap();
    const FIFO: &str = "01K6Y0000000000000000000F1";
    const LINK: &str = "01K6Y0000000000000000000F2";
    const BIG: &str = "01K6Y0000000000000000000F3";
    const GOOD: &str = "01K6Y0000000000000000000A1";
    std::fs::write(
        dir.join(format!("{GOOD}.json")),
        proposal(GOOD, &root, None, false).to_string(),
    )
    .unwrap();
    if !mkfifo(&dir.join(format!("{FIFO}.json"))) {
        eprintln!("skipped: no mkfifo");
        return;
    }
    std::os::unix::fs::symlink("/dev/zero", dir.join(format!("{LINK}.json"))).unwrap();
    let mut big = proposal(BIG, &root, None, false);
    big["items"][0]["evidence"][0]["text"] = json!("x".repeat(200));
    let mut text = big.to_string();
    text.push_str(&" ".repeat(5 * 1024 * 1024));
    std::fs::write(dir.join(format!("{BIG}.json")), text).unwrap();

    let limit = std::time::Duration::from_secs(20);
    let out = run_within(&env, T1, &["index", "--check"], limit).expect("index finished");
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let ix = json(&out);
    assert_eq!(ix["valid"], json!(true), "{ix}");
    let warnings = ix["warnings"].to_string();
    for (id, why) in [
        (FIFO, "not a regular file"),
        (LINK, "a symbolic link"),
        (BIG, "more than 4194304"),
    ] {
        let w: Vec<String> = ix["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|w| w.as_str())
            .filter(|w| w.contains(&format!("{id}.json: not read (")))
            .map(String::from)
            .collect();
        assert!(w.len() == 1 && w[0].contains(why), "{id}: {warnings}");
    }
    // the valid one older than all three is still found
    assert_eq!(index(&env)["triage"]["id"], json!(GOOD));

    // the same guard for autocommit.json
    let record = state(&env).join("autocommit.json");
    for (case, why) in [
        ("fifo", "not a regular file"),
        ("link", "a symbolic link"),
        ("big", "more than 4194304"),
    ] {
        std::fs::remove_file(&record).unwrap();
        match case {
            "fifo" => assert!(mkfifo(&record)),
            "link" => std::os::unix::fs::symlink("/dev/zero", &record).unwrap(),
            _ => std::fs::write(&record, " ".repeat(5 * 1024 * 1024)).unwrap(),
        }
        let out = run_within(&env, T2, &["index"], limit).expect("index finished");
        assert_eq!(out.status.code(), Some(0), "{case}: {}", stderr(&out));
        let w = json(&out)["warnings"].to_string();
        assert!(w.contains("autocommit.json: "), "{case}: {w}");
        assert!(w.contains(why), "{case}: {w}");
        assert!(
            index(&env)["logbook"]["git"].get("autocommit").is_none(),
            "{case}"
        );
    }
}
