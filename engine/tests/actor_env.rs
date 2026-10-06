//! `$SELDON_ACTOR` (WP-096, ADR-0027 §5): the actor of `plan`, `log`,
//! `drift` and `event` when `--actor` is not given. `--actor` wins; a value
//! the command refuses is exit 1 naming the variable and the allowed form,
//! before anything is written.

mod common;

use std::path::{Path, PathBuf};
use std::process::Output;

use serde_json::Value;

use common::{Env, Snapper, copy_dir, fixture_logbook, json, read, stderr};

const T0: &str = "2026-10-01T15:30:00+02:00";

/// The sample index's clock, for the fixture copy (`tests/drift.rs`).
const GENERATED_AT: &str = "2026-10-01T17:05:12+02:00";

/// Open drift items of the fixture logbook (`tests/drift.rs`), all
/// attention: an agent may not dismiss a crisis (ADR-0028 §3).
const THEME: &str = "01M3VTGNY0NZG4AY80814WSKGR";
const MONITORS: &str = "01M3KVWFR06078ZQTPRZCFYHK0";
const OLLAMA: &str = "01M3VNFTF8EVHWFFZ687N14Q0C";

/// `seldon --json args…` at `now`, with `SELDON_ACTOR=actor` when given.
fn run(env: &Env, now: &str, actor: Option<&str>, args: &[&str]) -> Output {
    let mut cmd = env.command(&[&["--json"], args].concat());
    cmd.env("SELDON_NOW", now);
    if let Some(actor) = actor {
        cmd.env("SELDON_ACTOR", actor);
    }
    cmd.output().expect("run seldon")
}

fn ok(out: &Output) -> Value {
    assert_eq!(out.status.code(), Some(0), "{}", stderr(out));
    json(out)
}

/// The `--json` error message.
fn error(out: &Output) -> String {
    json(out)["error"]["message"].as_str().unwrap().to_string()
}

/// Exit 1, naming the variable and the allowed form.
fn refused(out: &Output) {
    assert_eq!(out.status.code(), Some(1), "{}", stderr(out));
    let err = error(out);
    assert!(
        err.contains("SELDON_ACTOR") && err.contains("agent:<name>"),
        "{err}"
    );
}

fn ledger_len(root: &Path) -> usize {
    common::ledger(root).len()
}

fn last_event(root: &Path) -> Value {
    common::ledger(root).pop().unwrap()
}

/// The case frontmatter as `plan show --json` reports it.
fn case(env: &Env, id: &str) -> Value {
    ok(&run(env, T0, None, &["plan", "show", id]))["case"].clone()
}

#[test]
fn log_takes_the_variable_without_actor() {
    let env = Env::new(Snapper::Missing);
    let root = env.init_logbook();

    let v = ok(&run(
        &env,
        T0,
        Some("agent:codex"),
        &["log", "from the env"],
    ));
    assert_eq!(v["event"]["actor"], "agent:codex");
    let journal = read(&root.join("journal/2026/2026-10-01.md"));
    assert!(journal.contains("· agent:codex"), "{journal}");

    // the explicit flag wins
    let v = ok(&run(
        &env,
        T0,
        Some("agent:codex"),
        &["log", "--actor", "human", "from the flag"],
    ));
    assert_eq!(v["event"]["actor"], "human");

    // without either, human
    let v = ok(&run(&env, T0, None, &["log", "plain"]));
    assert_eq!(v["event"]["actor"], "human");
    // an empty variable counts as unset
    let v = ok(&run(&env, T0, Some(""), &["log", "empty"]));
    assert_eq!(v["event"]["actor"], "human");

    // an agent from the variable is held to the agent's one-line rule
    let before = ledger_len(&root);
    let out = run(&env, T0, Some("agent:codex"), &["log", "two\nlines"]);
    assert_eq!(out.status.code(), Some(1), "{}", stderr(&out));
    assert!(error(&out).contains("must be one line"), "{}", error(&out));
    assert_eq!(ledger_len(&root), before);
}

#[test]
fn a_refused_variable_is_exit_1_and_writes_nothing() {
    let env = Env::new(Snapper::Missing);
    let root = env.init_logbook();
    ok(&run(&env, T0, None, &["plan", "new", "--", "Case"]));
    let before = ledger_len(&root);

    for bad in ["Agent:X", "agent:", "root", "system"] {
        for args in [
            &["log", "note"][..],
            &["plan", "new", "--", "Other"],
            &["plan", "start", "C-2026-001"],
            // WP-101: the new plan commands, and the creator of `agent start --new`
            &["plan", "set", "C-2026-001", "--risk", "R3"],
            &["plan", "snapshot", "C-2026-001", "42"],
            &["plan", "reopen", "C-2026-001"],
            &["agent", "start", "--new", "--", "Other"],
            // refused before the event is looked up
            &["drift", "dismiss", THEME, "--only", "--", "x"],
        ] {
            refused(&run(&env, T0, Some(bad), args));
        }
    }
    // `event` accepts system, not the rest
    for bad in ["Agent:X", "root"] {
        refused(&run(
            &env,
            T0,
            Some(bad),
            &["event", "manual", "note", "--subject", "x"],
        ));
    }
    assert_eq!(ledger_len(&root), before);
    assert_eq!(case(&env, "C-2026-001")["status"], "queued");
    assert_eq!(case(&env, "C-2026-001")["risk"], "R1");
    assert_eq!(case(&env, "C-2026-001")["snapshotBefore"], Value::Null);

    // with --actor the variable is not read, so a bad one does not matter
    let v = ok(&run(
        &env,
        T0,
        Some("Agent:X"),
        &["log", "--actor", "agent:codex", "flag"],
    ));
    assert_eq!(v["event"]["actor"], "agent:codex");
    let v = ok(&run(
        &env,
        T0,
        Some("root"),
        &[
            "event",
            "manual",
            "note",
            "--subject",
            "x",
            "--actor",
            "human",
        ],
    ));
    assert_eq!(v["event"]["actor"], "human");
}

#[test]
fn plan_steps_take_the_variable_without_actor() {
    let env = Env::new(Snapper::Missing);
    let root = env.init_logbook();
    let agent = Some("agent:codex");

    let v = ok(&run(
        &env,
        T0,
        agent,
        &["plan", "new", "--", "By the agent"],
    ));
    assert_eq!(v["event"]["actor"], "agent:codex");
    assert_eq!(
        v["case"]["agents"],
        serde_json::json!(["agent:codex"]),
        "{v}"
    );
    for step in ["start", "verify", "done"] {
        if step == "done" {
            // the evidence an agent's close needs (ADR-0027 §5, WP-101)
            let path = common::find_file(&root.join("work/active"), "C-2026-001");
            let text = read(&path)
                .replacen("- Verification:\n", "- Verification: it runs\n", 1)
                .replacen("## Result\n", "## Result\nIt runs.\n", 1);
            std::fs::write(&path, text).unwrap();
        }
        let v = ok(&run(&env, T0, agent, &["plan", step, "C-2026-001"]));
        assert_eq!(v["event"]["actor"], "agent:codex", "{step}");
    }
    let text = read(&common::find_file(
        &root.join("work/completed"),
        "C-2026-001",
    ));
    for step in ["started", "verification", "completed"] {
        assert!(
            text.lines()
                .any(|l| l.contains("agent:codex") && l.contains(step)),
            "{step}: {text}"
        );
    }

    // the explicit flag wins over the variable
    ok(&run(&env, T0, None, &["plan", "new", "--", "By the human"]));
    let v = ok(&run(
        &env,
        T0,
        agent,
        &["plan", "drop", "C-2026-002", "--actor", "human"],
    ));
    assert_eq!(v["event"]["actor"], "human");
    assert_eq!(case(&env, "C-2026-002")["agents"], serde_json::json!([]));
    let v = ok(&run(
        &env,
        T0,
        agent,
        &["plan", "new", "--actor", "human", "--", "Flag"],
    ));
    assert_eq!(v["event"]["actor"], "human");
}

/// A copy of the fixture logbook in `env`.
fn fixture_copy(env: &Env) -> PathBuf {
    let lb = env.tmp.path().join("logbook");
    copy_dir(&fixture_logbook(), &lb);
    lb
}

#[test]
fn drift_resolutions_take_the_variable_without_actor() {
    let env = Env::new(Snapper::Missing);
    let lb = fixture_copy(&env);
    let at = |actor, args: &[&str]| {
        let mut all = vec!["--logbook", lb.to_str().unwrap()];
        all.extend_from_slice(args);
        run(&env, GENERATED_AT, actor, &all)
    };
    let agent = Some("agent:codex");

    let before = ledger_len(&lb);
    refused(&at(
        Some("human:x"),
        &["drift", "link", THEME, "C-2026-005"],
    ));
    assert_eq!(ledger_len(&lb), before);

    let v = ok(&at(agent, &["drift", "link", THEME, "C-2026-005"]));
    assert_eq!(v["events"][0]["actor"], "agent:codex");
    let v = ok(&at(
        agent,
        &["drift", "dismiss", MONITORS, "--only", "--", "Known"],
    ));
    assert_eq!(v["events"][0]["actor"], "agent:codex");
    // an agent's session cannot resolve as a person (ADR-0028 §3, WP-109
    // round 2, as WP-101 for `plan done`); another agent's name is a flag
    // like any other
    for verb in [
        &[
            "drift", "explain", OLLAMA, "--only", "--actor", "human", "--", "Why",
        ][..],
        &["drift", "dismiss", OLLAMA, "--actor", "human", "--", "Why"],
        &["drift", "link", OLLAMA, "C-2026-004", "--actor", "human"],
    ] {
        let out = at(agent, verb);
        assert_eq!(out.status.code(), Some(1), "{verb:?}");
        assert_eq!(
            error(&out),
            format!(
                "{OLLAMA} is not resolved: `--actor human` in a session of agent:codex \
                 (SELDON_ACTOR); an agent's resolution is never recorded as human (ADR-0028 \
                 §3). Resolve it as agent:codex, or from a session of your own (the panel)"
            )
        );
    }
    let v = ok(&at(
        agent,
        &[
            "drift",
            "explain",
            OLLAMA,
            "--only",
            "--actor",
            "agent:claude-code",
            "--",
            "Why",
        ],
    ));
    assert_eq!(
        v["events"][0]["actor"], "agent:claude-code",
        "the flag wins"
    );
    // a person's own session: the flag is the person
    let v = ok(&at(
        None,
        &[
            "drift", "dismiss", MONITORS, "--actor", "human", "--", "again",
        ],
    ));
    assert_eq!(v["resolved"], 0, "already resolved above, nothing written");
    assert!(
        common::ledger(&lb)[before..]
            .iter()
            .all(|e| e["kind"] != "resolution" || e["actor"] != "system")
    );
}

#[test]
fn drift_explain_takes_the_variable_too() {
    let env = Env::new(Snapper::Missing);
    let lb = fixture_copy(&env);
    let mut all = vec!["--logbook", lb.to_str().unwrap()];
    all.extend_from_slice(&["drift", "explain", OLLAMA, "--only", "--", "Why"]);
    let v = ok(&run(&env, GENERATED_AT, Some("agent:codex"), &all));
    assert_eq!(v["events"][0]["actor"], "agent:codex");
}

#[test]
fn event_takes_the_variable_after_the_ledger_attribution() {
    let env = Env::new(Snapper::Missing);
    let root = env.init_logbook();
    ok(&run(&env, T0, None, &["plan", "new", "--", "Theme"]));
    ok(&run(&env, T0, None, &["plan", "start", "C-2026-001"]));
    let agent = Some("agent:codex");

    // a manual event, a case named: the variable's actor
    let v = ok(&run(
        &env,
        T0,
        agent,
        &[
            "event",
            "manual",
            "note",
            "--subject",
            "x",
            "--case",
            "C-2026-001",
        ],
    ));
    assert_eq!(v["event"]["actor"], "agent:codex");
    assert_eq!(v["event"]["case"], "C-2026-001");
    // --actor system is explicit: the variable does not replace it
    let v = ok(&run(
        &env,
        T0,
        agent,
        &[
            "event",
            "manual",
            "note",
            "--subject",
            "y",
            "--actor",
            "system",
        ],
    ));
    assert_eq!(v["event"]["actor"], "system");
    // without either, system
    let v = ok(&run(
        &env,
        T0,
        None,
        &["event", "manual", "note", "--subject", "z"],
    ));
    assert_eq!(v["event"]["actor"], "system");

    // the theme hook's event (no --actor, no --case) inside an agent session:
    // the agent command the ledger holds names actor and case, as without
    // the variable; the variable is only the fallback
    let mut hook = env.command(&["hook", "generic"]);
    hook.env("SELDON_NOW", T0)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    let mut child = hook.spawn().unwrap();
    use std::io::Write as _;
    child
        .stdin
        .take()
        .unwrap()
        .write_all(
            serde_json::json!({
                "command": "omarchy theme set kanagawa",
                "actor": "agent:claude-code",
                "cwd": root.to_str().unwrap(),
            })
            .to_string()
            .as_bytes(),
        )
        .unwrap();
    let out = child.wait_with_output().unwrap();
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let cmd = last_event(&root);
    assert_eq!(
        (cmd["kind"].as_str(), cmd["case"].as_str()),
        (Some("command"), Some("C-2026-001")),
        "{cmd}"
    );

    let v = ok(&run(
        &env,
        T0,
        agent,
        &["event", "theme", "theme-set", "--subject", "kanagawa"],
    ));
    assert_eq!(
        (v["event"]["actor"].as_str(), v["event"]["case"].as_str()),
        (Some("agent:claude-code"), Some("C-2026-001")),
        "{v}"
    );
    // no agent command for this one: the variable
    let v = ok(&run(
        &env,
        T0,
        agent,
        &["event", "theme", "theme-set", "--subject", "nord"],
    ));
    assert_eq!(
        (v["event"]["actor"].as_str(), v["event"].get("case")),
        (Some("agent:codex"), None),
        "{v}"
    );
}
