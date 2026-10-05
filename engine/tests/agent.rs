//! `seldon agent start` (WP-022): the launcher argv from config.toml, the
//! prompt as one argument (the case id and the logbook path, no logbook
//! text), the detached launch, the refusals.

mod common;

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use common::{Env, Snapper, json, read, stderr};

const T0: &str = "2026-10-01T15:30:00+02:00";

/// A logbook with C-2026-001 (active), C-2026-002 (queued) and
/// C-2026-003 (active, started last: the active case).
fn logbook(env: &Env) -> PathBuf {
    let root = env.init_logbook();
    for title in [
        "Install zed as \"second\" editor; $(touch pwned) `id` & more",
        "Later",
        "Other",
    ] {
        let out = env.at(T0, &["plan", "new", "--", title]);
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    }
    for id in ["C-2026-001", "C-2026-003"] {
        let out = env.at(T0, &["plan", "start", id]);
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    }
    assert_eq!(active_case(&root).as_deref(), Some("C-2026-003"));
    root
}

fn active_case(root: &Path) -> Option<String> {
    std::fs::read_to_string(root.join(".seldon/active-case"))
        .ok()
        .map(|s| s.trim().to_string())
}

fn add_config(env: &Env, toml: &str) {
    let path = env.config_file();
    let text = read(&path);
    std::fs::write(&path, format!("{text}\n{toml}\n")).unwrap();
}

/// A stub `name` that appends each call's argv (NUL-separated) to
/// `<tmp>/<name>.argv`, one call per `<tmp>/<name>.calls` line, and its
/// working directory and `SELDON_LOGBOOK` to `<tmp>/<name>.env`.
fn recording_stub(env: &Env, name: &str) -> (PathBuf, PathBuf, PathBuf) {
    let base = env.tmp.path().join(name);
    let (argv, calls, vars) = (
        base.with_extension("argv"),
        base.with_extension("calls"),
        base.with_extension("env"),
    );
    env.stub(
        name,
        &format!(
            "printf '%s\\0' \"$@\" >> '{}'; echo call >> '{}'; printf '%s\\n%s\\n' \"$(pwd)\" \"$SELDON_LOGBOOK\" > '{}'",
            argv.display(),
            calls.display(),
            vars.display()
        ),
    );
    (argv, calls, vars)
}

fn recorded(argv: &Path) -> Vec<String> {
    let bytes = std::fs::read(argv).unwrap();
    let text = String::from_utf8(bytes).unwrap();
    let mut args: Vec<String> = text.split('\0').map(String::from).collect();
    assert_eq!(args.pop().as_deref(), Some(""), "NUL-terminated");
    args
}

#[test]
fn launches_the_default_launcher_with_the_prompt_as_one_argument() {
    let env = Env::new(Snapper::Missing);
    let root = logbook(&env);
    let (argv, calls, vars) = recording_stub(&env, "omarchy");

    let start = Instant::now();
    let out = env.at(T0, &["agent", "start", "C-2026-001", "--json"]);
    let took = start.elapsed();
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert!(took < Duration::from_secs(1), "took {took:?}");

    let v = json(&out);
    assert_eq!(v["launched"], true);
    assert_eq!(v["launcher"], "default");
    assert_eq!(v["program"], "omarchy");
    assert_eq!(
        v["argv"],
        serde_json::json!(["omarchy", "agent", "prompt", "{prompt}"])
    );
    assert_eq!(v["case"], "C-2026-001");
    assert_eq!(v["cwd"], root.to_str().unwrap());
    assert_eq!(v["previousActiveCase"], "C-2026-003");

    assert_eq!(read(&calls), "call\n", "exactly one launch");
    let args = recorded(&argv);
    assert_eq!(args.len(), 3, "{args:?}");
    assert_eq!(args[..2], ["agent", "prompt"]);
    let prompt = &args[2];
    assert_eq!(
        *prompt,
        format!(
            "Work case C-2026-001 in the Seldon logbook at {}. First run `seldon hook session-start` \
             (the logbook context) and `seldon plan show C-2026-001` (the case file). Every \
             mutating command is recorded.",
            root.display()
        )
    );
    // the agent reads the title itself; it is not in the arguments
    assert!(!prompt.contains("Install zed"), "{prompt}");
    assert!(!env.tmp.path().join("pwned").exists());
    assert!(!root.join("pwned").exists());

    // the case is the active case; the launcher runs in the logbook
    assert_eq!(active_case(&root).as_deref(), Some("C-2026-001"));
    let canonical = std::fs::canonicalize(&root).unwrap();
    assert_eq!(
        read(&vars),
        format!("{}\n{}\n", canonical.display(), root.display())
    );

    // human output names the launcher
    let out = env.at(T0, &["agent", "start", "C-2026-003"]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let human = String::from_utf8(out.stdout).unwrap();
    assert!(
        human.starts_with("Agent started on C-2026-003 with launcher `default` (omarchy)"),
        "{human}"
    );
    assert_eq!(read(&calls), "call\ncall\n");
}

#[test]
fn the_launcher_comes_from_config() {
    let env = Env::new(Snapper::Missing);
    let root = logbook(&env);
    let (argv, calls, _) = recording_stub(&env, "my-agent");
    let (claude, _, _) = recording_stub(&env, "claude-stub");
    add_config(
        &env,
        "[agent]\nlauncher = [\"my-agent\", \"--title\", \"two words\", \"{prompt}\", \"--after\"]\n\
         [agent.launchers]\nclaude = [\"claude-stub\", \"--\", \"{prompt}\"]",
    );
    let out = env.at(T0, &["agent", "start", "C-2026-001", "--json"]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert_eq!(json(&out)["program"], "my-agent");
    let args = recorded(&argv);
    assert_eq!(args.len(), 4, "{args:?}");
    assert_eq!(args[..2], ["--title", "two words"]);
    assert!(args[2].starts_with("Work case C-2026-001 "));
    assert_eq!(args[3], "--after");
    assert_eq!(read(&calls), "call\n");

    let out = env.at(
        T0,
        &[
            "agent",
            "start",
            "C-2026-003",
            "--launcher",
            "claude",
            "--json",
        ],
    );
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert_eq!(json(&out)["launcher"], "claude");
    let args = recorded(&claude);
    assert_eq!(args.len(), 2);
    assert_eq!(args[0], "--");
    assert!(args[1].starts_with("Work case C-2026-003 "));
    assert_eq!(active_case(&root).as_deref(), Some("C-2026-003"));

    // an unknown name: nothing runs, nothing changes
    let out = env.at(
        T0,
        &[
            "agent",
            "start",
            "C-2026-001",
            "--launcher",
            "codex",
            "--json",
        ],
    );
    assert_eq!(out.status.code(), Some(1));
    let message = json(&out)["error"]["message"].as_str().unwrap().to_string();
    assert!(
        message.contains("unknown launcher `codex`; known: default, omarchy, claude"),
        "{message}"
    );
    assert_eq!(active_case(&root).as_deref(), Some("C-2026-003"));
}

#[test]
fn the_launcher_arguments_hold_no_logbook_text() {
    let env = Env::new(Snapper::Missing);
    let root = logbook(&env);
    // records its argv and also writes it to stderr, the launch log
    let argv = env.tmp.path().join("omarchy.argv");
    env.stub(
        "omarchy",
        &format!(
            "printf '%s\\0' \"$@\" >> '{}'; printf 'args: %s\\n' \"$*\" >&2",
            argv.display()
        ),
    );
    // made-up text in each place the session-start block reads
    let out = env.at(
        T0,
        &[
            "log",
            "--case",
            "C-2026-001",
            "--",
            "JOURNAL-SENTINEL-7f3a\nsecond line",
        ],
    );
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    std::fs::write(
        root.join("memory/lessons.md"),
        "# Lessons\n\n## LESSON-SENTINEL-7f3a\n",
    )
    .unwrap();
    let out = env.at(T0, &["status"]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));

    let out = env.at(T0, &["agent", "start", "C-2026-001", "--json"]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let v = json(&out);
    assert_eq!(
        v["argv"],
        serde_json::json!(["omarchy", "agent", "prompt", "{prompt}"])
    );
    let args = recorded(&argv);
    let all = args.join("\n");
    assert!(all.contains("C-2026-001"), "{all}");
    assert!(all.contains(root.to_str().unwrap()), "{all}");
    assert!(all.contains("seldon hook session-start"), "{all}");
    for text in [
        "SENTINEL",
        "Install zed",
        "second line",
        "Seldon logbook context",
        "\n",
    ] {
        assert!(!args[2].contains(text), "{text:?} in {:?}", args[2]);
    }
    // nor in the launch log, which got the arguments
    let logged = read(&env.lock_file().with_file_name("agent-launch.log"));
    assert!(
        logged.contains("args: agent prompt Work case C-2026-001"),
        "{logged}"
    );
    assert!(!logged.contains("SENTINEL"), "{logged}");
}

#[test]
fn a_shell_launcher_is_refused_before_anything_changes() {
    let env = Env::new(Snapper::Missing);
    let root = logbook(&env);
    let (_, calls, _) = recording_stub(&env, "bash");
    add_config(&env, "[agent]\nlauncher = [\"bash\", \"-c\", \"{prompt}\"]");
    let out = env.at(T0, &["agent", "start", "C-2026-001", "--json"]);
    assert_eq!(out.status.code(), Some(1));
    let message = json(&out)["error"]["message"].as_str().unwrap().to_string();
    assert!(
        message.contains("`bash` can run its arguments as code")
            && message.contains("a heuristic check by program name, not a sandbox"),
        "{message}"
    );
    assert!(!calls.exists(), "never started");
    assert_eq!(active_case(&root).as_deref(), Some("C-2026-003"));
}

#[test]
fn a_queued_case_is_refused_with_the_hint() {
    let env = Env::new(Snapper::Missing);
    let root = logbook(&env);
    let (_, calls, _) = recording_stub(&env, "omarchy");
    let out = env.at(T0, &["agent", "start", "C-2026-002", "--json"]);
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(
        json(&out)["error"]["message"],
        "C-2026-002 is queued; start it first: `seldon plan start C-2026-002`"
    );
    // human form on stderr
    let out = env.at(T0, &["agent", "start", "C-2026-002"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(stderr(&out).contains("`seldon plan start C-2026-002`"));
    assert!(!calls.exists());
    assert_eq!(active_case(&root).as_deref(), Some("C-2026-003"));

    // not active either: verification, completed; an unknown or bad id
    env.at(T0, &["plan", "verify", "C-2026-001"]);
    let out = env.at(T0, &["agent", "start", "C-2026-001", "--json"]);
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(
        json(&out)["error"]["message"],
        "C-2026-001 has status verification; an agent starts on an active case only"
    );
    assert_eq!(
        env.at(T0, &["agent", "start", "C-2026-009"]).status.code(),
        Some(1)
    );
    assert_eq!(
        env.at(T0, &["agent", "start", "--json", "--", "-x; rm"])
            .status
            .code(),
        Some(1)
    );
    assert!(!calls.exists());
}

#[test]
fn a_missing_launcher_is_an_error_and_restores_the_active_case() {
    let env = Env::new(Snapper::Missing);
    let root = logbook(&env);
    // no `omarchy` on PATH
    let out = env.at(T0, &["agent", "start", "C-2026-001", "--json"]);
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(
        json(&out)["error"]["message"],
        "launcher `default`: `omarchy` not found; set `[agent] launcher` in config.toml"
    );
    assert_eq!(active_case(&root).as_deref(), Some("C-2026-003"));

    // without a previous active case, none is left behind
    std::fs::remove_file(root.join(".seldon/active-case")).unwrap();
    let out = env.at(T0, &["agent", "start", "C-2026-001"]);
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(active_case(&root), None);
}

#[test]
fn a_launcher_that_fails_at_once_reports_its_message() {
    let env = Env::new(Snapper::Missing);
    let root = logbook(&env);
    env.stub(
        "omarchy",
        "echo 'Choose default agent with: omarchy default agent <name>' >&2; exit 1",
    );
    let out = env.at(T0, &["agent", "start", "C-2026-001", "--json"]);
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(
        json(&out)["error"]["message"],
        "launcher `default` (omarchy) exited with 1: Choose default agent with: omarchy default agent <name>"
    );
    assert_eq!(active_case(&root).as_deref(), Some("C-2026-003"));

    // the log is appended, never truncated (a running launcher may hold
    // it), and a silent failure never reports an earlier launch's lines
    env.stub("omarchy", "exit 2");
    let out = env.at(T0, &["agent", "start", "C-2026-001", "--json"]);
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(
        json(&out)["error"]["message"],
        "launcher `default` (omarchy) exited with 2"
    );
    let log = read(&env.home.join(".local/state/seldon/agent-launch.log"));
    assert_eq!(
        log,
        "Choose default agent with: omarchy default agent <name>\n"
    );
}

/// The launcher may keep running (a terminal with the agent in it): it is
/// started detached, reported at once and left alone.
#[test]
fn a_launcher_that_keeps_running_is_detached() {
    let env = Env::new(Snapper::Missing);
    let root = logbook(&env);
    let pid_file = env.tmp.path().join("agent-pid");
    // PATH is the stub directory only: the host's sleep by its path
    let sleep = std::env::var("PATH")
        .unwrap_or_default()
        .split(':')
        .map(|d| Path::new(d).join("sleep"))
        .find(|p| p.is_file())
        .expect("sleep on the host");
    env.stub(
        "omarchy",
        &format!(
            "echo $$ > '{}'; exec '{}' 5",
            pid_file.display(),
            sleep.display()
        ),
    );
    let start = Instant::now();
    // output() waits for stdout/stderr to close: an inherited pipe would
    // hold it until the launcher exits
    let out = env.at(T0, &["agent", "start", "C-2026-001", "--json"]);
    let took = start.elapsed();
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert!(took < Duration::from_secs(1), "took {took:?}");
    assert_eq!(json(&out)["launched"], true);
    assert_eq!(active_case(&root).as_deref(), Some("C-2026-001"));

    let pid = read(&pid_file).trim().to_string();
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).expect("the launcher runs");
    // `pid (comm) state ppid pgrp …`
    let fields: Vec<&str> = stat
        .rsplit_once(')')
        .unwrap()
        .1
        .split_whitespace()
        .collect();
    assert!(fields[0] != "Z" && fields[0] != "X", "alive: {stat}");
    assert_eq!(fields[2], pid, "process group of its own");
    let _ = std::process::Command::new("kill")
        .args(["--", &format!("-{pid}")])
        .status();
}

#[test]
fn needs_a_logbook() {
    let env = Env::new(Snapper::Missing);
    let out = env.at(T0, &["agent", "start", "C-2026-001", "--json"]);
    assert_eq!(out.status.code(), Some(3));
}

/// A stub `name` that writes `SELDON_ACTOR|SELDON_ATTENDED` as it got them
/// to `<tmp>/<name>.vars`.
fn vars_stub(env: &Env, name: &str) -> PathBuf {
    let vars = env.tmp.path().join(name).with_extension("vars");
    env.stub(
        name,
        &format!(
            "printf '%s|%s\\n' \"${{SELDON_ACTOR-unset}}\" \"${{SELDON_ATTENDED-unset}}\" > '{}'",
            vars.display()
        ),
    );
    vars
}

/// WP-096: the launched agent gets `SELDON_ACTOR=agent:<launcher name>`
/// and `SELDON_ATTENDED=1`, whatever the caller had set.
#[test]
fn the_launched_agent_gets_its_actor_and_the_attended_marker() {
    let env = Env::new(Snapper::Missing);
    let root = logbook(&env);
    let default = vars_stub(&env, "omarchy");
    let named = vars_stub(&env, "claude-stub");
    add_config(
        &env,
        "[agent.launchers]\n\"Claude Code\" = [\"claude-stub\", \"{prompt}\"]\n\
         \"___\" = [\"claude-stub\", \"{prompt}\"]",
    );
    let start = |args: &[&str], inherited: bool| {
        let mut cmd = env.command(&[&["agent", "start"], args, &["--json"]].concat());
        cmd.env("SELDON_NOW", T0);
        if inherited {
            cmd.env("SELDON_ACTOR", "agent:caller")
                .env("SELDON_ATTENDED", "0");
        }
        cmd.output().unwrap()
    };

    let out = start(&["C-2026-001"], false);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert_eq!(read(&default), "agent:default|1\n");
    assert_eq!(json(&out)["actor"], "agent:default");

    let out = start(&["C-2026-003", "--launcher", "Claude Code"], true);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert_eq!(read(&named), "agent:claude-code|1\n", "not the caller's");
    assert_eq!(json(&out)["actor"], "agent:claude-code");
    let out = env.at(
        T0,
        &["agent", "start", "C-2026-001", "--launcher", "omarchy"],
    );
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert!(
        String::from_utf8(out.stdout)
            .unwrap()
            .contains(", as agent:omarchy"),
    );
    assert_eq!(read(&default), "agent:omarchy|1\n");

    // a name that gives no actor: refused before anything changes
    std::fs::remove_file(&named).unwrap();
    let out = start(&["C-2026-003", "--launcher", "___"], false);
    assert_eq!(out.status.code(), Some(1));
    let message = json(&out)["error"]["message"].as_str().unwrap().to_string();
    assert!(message.contains("its name gives no actor"), "{message}");
    assert!(!named.exists(), "nothing launched");
    assert_eq!(active_case(&root).as_deref(), Some("C-2026-001"));
}

/// WP-096 end to end: the launched agent's own `seldon` call without
/// `--actor` is recorded as the agent, not as human.
#[test]
fn the_launched_agents_writes_are_recorded_as_the_agent() {
    let env = Env::new(Snapper::Missing);
    let root = logbook(&env);
    let tmp = env.tmp.path();
    let (go, done) = (tmp.join("go"), tmp.join("done"));
    // waits (shell builtins only) until `agent start` returned and dropped
    // the lock, then logs a note and closes the case without --actor
    env.stub(
        "omarchy",
        &format!(
            "(i=0; while [ ! -e '{go}' ] && [ $i -lt 2000000 ]; do i=$((i+1)); done; \
             '{bin}' log --case C-2026-001 'from the agent' && \
             '{bin}' plan verify C-2026-001 && '{bin}' plan done C-2026-001; \
             echo $? > '{done}') >/dev/null 2>&1 &",
            go = go.display(),
            done = done.display(),
            bin = env!("CARGO_BIN_EXE_seldon"),
        ),
    );
    let out = env.at(T0, &["agent", "start", "C-2026-001"]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    std::fs::write(&go, "").unwrap();
    let deadline = Instant::now() + Duration::from_secs(20);
    while !done.exists() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(read(&done).trim(), "0");

    let ledger = common::ledger(&root);
    let mine: Vec<(String, String)> = ledger
        .iter()
        .filter(|e| e["case"] == "C-2026-001" || e["subject"] == "from the agent")
        .map(|e| {
            (
                e["kind"].as_str().unwrap().to_string(),
                e["actor"].as_str().unwrap().to_string(),
            )
        })
        .collect();
    let tail: Vec<(&str, &str)> = mine[mine.len() - 3..]
        .iter()
        .map(|(k, a)| (k.as_str(), a.as_str()))
        .collect();
    assert_eq!(
        tail,
        [
            ("note", "agent:default"),
            ("case-verified", "agent:default"),
            ("case-completed", "agent:default"),
        ],
        "{mine:?}"
    );
}
