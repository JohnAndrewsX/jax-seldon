//! `seldon agent start` (WP-022): the launcher argv from config.toml, the
//! prompt as one argument, the detached launch, the refusals.

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
    assert!(
        prompt.starts_with(&format!(
            "Work case C-2026-001 in the Seldon logbook at {}; every mutating command is recorded.\n\n# Seldon logbook context\n",
            root.display()
        )),
        "{prompt}"
    );
    // the session-start block, with the case now active, title as written
    assert!(
        prompt.contains(
            "## Active case\nC-2026-001 — Install zed as \"second\" editor; $(touch pwned) `id` & more"
        ),
        "{prompt}"
    );
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
fn a_shell_launcher_is_refused_before_anything_changes() {
    let env = Env::new(Snapper::Missing);
    let root = logbook(&env);
    let (_, calls, _) = recording_stub(&env, "bash");
    add_config(&env, "[agent]\nlauncher = [\"bash\", \"-c\", \"{prompt}\"]");
    let out = env.at(T0, &["agent", "start", "C-2026-001", "--json"]);
    assert_eq!(out.status.code(), Some(1));
    let message = json(&out)["error"]["message"].as_str().unwrap().to_string();
    assert!(
        message.contains("runs its arguments as shell code"),
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
