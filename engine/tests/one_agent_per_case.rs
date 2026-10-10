//! One agent per case, and one editor per file (WP-156, ADR-0041): a
//! session is a Hyprland window of an Omarchy launcher class whose process
//! or a descendant carries the marker. `agent start` refuses while one is
//! open (or a launch of the case is less than 10 s old), `--again` starts
//! another, `agent sessions` lists them, `agent focus` brings the window to
//! the front; `open --editor` focuses the terminal window it opened on the
//! same path. Without `hyprctl` nothing is tracked.
//!
//! The "windows" here are real processes the test starts (each the leader
//! of its own process group, killed by [`Live`] when the test ends, also on
//! a failure) and a stub `hyprctl` that lists them from
//! `<tmp>/clients.json`.

mod common;

use std::os::unix::process::CommandExt as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use common::{Env, Snapper, json, read, stderr, stdout};

const T0: &str = "2026-10-01T15:30:00+02:00";
const T5: &str = "2026-10-01T15:30:05+02:00";
const T11: &str = "2026-10-01T15:30:11+02:00";
const T20: &str = "2026-10-01T15:30:20+02:00";
const AGENT: &str = "org.omarchy.agent";

/// A program on the host's PATH (the engine's PATH is the stub directory).
fn host(name: &str) -> PathBuf {
    std::env::var("PATH")
        .unwrap_or_default()
        .split(':')
        .map(|d| Path::new(d).join(name))
        .find(|p| p.is_file())
        .unwrap_or_else(|| panic!("{name} on the host"))
}

/// Processes this test started, by process group; killed on drop.
#[derive(Default)]
struct Live {
    groups: std::cell::RefCell<Vec<u32>>,
}

impl Live {
    /// Starts `sh -c script` with exactly `vars` in its environment, the
    /// leader of its own process group. Its pid.
    fn spawn(&self, script: &str, vars: &[(&str, &str)]) -> u32 {
        let mut cmd = Command::new(host("sh"));
        cmd.args(["-c", script])
            .env_clear()
            .envs(vars.iter().copied())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .process_group(0);
        let pid = cmd.spawn().expect("spawn").id();
        self.groups.borrow_mut().push(pid);
        pid
    }

    /// Kills every group that still runs and waits until it is gone; a
    /// pid is never signalled after its process is gone.
    fn kill(&self) {
        let groups: Vec<u32> = self
            .groups
            .borrow_mut()
            .drain(..)
            .filter(|p| alive(*p))
            .collect();
        for pid in &groups {
            let _ = Command::new(host("kill"))
                .args(["--", &format!("-{pid}")])
                .status();
        }
        let deadline = Instant::now() + Duration::from_secs(5);
        while groups.iter().any(|p| alive(*p)) && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}

impl Drop for Live {
    fn drop(&mut self) {
        self.kill();
    }
}

/// Alive and not a zombie (a spawned child is never waited for here).
fn alive(pid: u32) -> bool {
    std::fs::read_to_string(format!("/proc/{pid}/stat")).is_ok_and(|s| {
        s.rsplit_once(')')
            .and_then(|(_, rest)| rest.split_whitespace().next().map(String::from))
            .is_some_and(|state| state != "Z" && state != "X")
    })
}

/// The marker `agent start` gives an agent on `case` in `root`.
fn marker<'a>(case: &'a str, root: &'a Path) -> Vec<(&'a str, &'a str)> {
    vec![
        ("SELDON_CASE", case),
        ("SELDON_LOGBOOK", root.to_str().unwrap()),
        ("SELDON_ACTOR", "agent:default"),
    ]
}

/// A process that carries `vars` and stays alive (a window's own process).
fn marked(live: &Live, vars: &[(&str, &str)]) -> u32 {
    live.spawn(&format!("exec '{}' 30", host("sleep").display()), vars)
}

/// A process without a marker whose child carries `vars` (a terminal and
/// the agent it runs). The parent's pid (the window's).
fn marked_child(live: &Live, vars: &[(&str, &str)]) -> u32 {
    let assign: String = vars.iter().map(|(k, v)| format!("{k}='{v}' ")).collect();
    let pid = live.spawn(
        &format!("{assign}'{}' 30 & wait", host("sleep").display()),
        &[],
    );
    // the child exists once the shell has forked it
    let deadline = Instant::now() + Duration::from_secs(5);
    while std::fs::read_to_string(format!("/proc/{pid}/task/{pid}/children"))
        .unwrap_or_default()
        .trim()
        .is_empty()
        && Instant::now() < deadline
    {
        std::thread::sleep(Duration::from_millis(10));
    }
    pid
}

/// One window for `hyprctl clients -j`; `pid` "PPID" is the caller's parent
/// (the engine that asks).
fn window(address: &str, pid: &str, class: &str) -> String {
    format!(
        r#"{{"address":"{address}","pid":{pid},"class":"{class}","title":"t","workspace":{{"id":3,"name":"3"}}}}"#
    )
}

/// A stub `hyprctl`: `clients -j` prints `<tmp>/clients.json` (the word
/// PPID replaced by its parent's pid); `dispatch` appends its arguments to
/// `<tmp>/hyprctl.dispatch` (one call per line, joined by `|`) and answers
/// `lua` for the Lua form, `ok` for the legacy one.
fn hyprctl(env: &Env, windows: &[String], lua: &str) -> PathBuf {
    set_windows(env, windows);
    let clients = env.tmp.path().join("clients.json");
    let log = env.tmp.path().join("hyprctl.dispatch");
    env.stub(
        "hyprctl",
        &format!(
            "if [ \"$1\" = clients ]; then '{sed}' \"s/PPID/$PPID/\" '{clients}'; exit 0; fi\n\
             (IFS='|'; echo \"$*\") >> '{log}'\n\
             case \"$2\" in hl.dsp.*) echo '{lua}';; *) echo ok;; esac",
            sed = host("sed").display(),
            clients = clients.display(),
            log = log.display()
        ),
    );
    log
}

fn set_windows(env: &Env, windows: &[String]) {
    std::fs::write(
        env.tmp.path().join("clients.json"),
        format!("[{}]", windows.join(",")),
    )
    .unwrap();
}

/// A stub launcher `omarchy` that counts its calls and returns at once.
fn launcher(env: &Env) -> PathBuf {
    let calls = env.tmp.path().join("omarchy.calls");
    env.stub("omarchy", &format!("echo call >> '{}'", calls.display()));
    calls
}

fn calls(path: &Path) -> usize {
    std::fs::read_to_string(path).map_or(0, |t| t.lines().count())
}

/// A logbook with C-2026-001 and C-2026-002, both active.
fn logbook(env: &Env) -> PathBuf {
    let root = env.init_logbook();
    for title in ["One", "Two"] {
        let out = env.at(T0, &["plan", "new", "--", title]);
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    }
    for id in ["C-2026-001", "C-2026-002"] {
        let out = env.at(T0, &["plan", "start", id]);
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    }
    root
}

fn active_case(root: &Path) -> String {
    read(&root.join(".seldon/active-case")).trim().to_string()
}

fn message(out: &std::process::Output) -> String {
    json(out)["error"]["message"]
        .as_str()
        .unwrap_or_default()
        .to_string()
}

fn sessions(env: &Env, at: &str) -> serde_json::Value {
    let out = env.at(at, &["agent", "sessions", "--json"]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    json(&out)
}

#[test]
fn an_open_agent_window_refuses_a_second_start() {
    let env = Env::new(Snapper::Missing);
    let root = logbook(&env);
    let live = Live::default();
    let calls_file = launcher(&env);
    let pid = marked(&live, &marker("C-2026-001", &root));
    hyprctl(&env, &[window("0xa1", &pid.to_string(), AGENT)], "ok");

    assert_eq!(active_case(&root), "C-2026-002");
    let out = env.at(T0, &["agent", "start", "C-2026-001", "--json"]);
    assert_eq!(out.status.code(), Some(1), "{}", stderr(&out));
    assert_eq!(
        message(&out),
        "an agent is already working on C-2026-001 (window 0xa1 on workspace 3); focus it with \
         `seldon agent focus C-2026-001`, or start another with `seldon agent start \
         C-2026-001 --again`; nothing was launched"
    );
    assert_eq!(calls(&calls_file), 0, "nothing launched");
    assert_eq!(
        active_case(&root),
        "C-2026-002",
        "the active case untouched"
    );

    let v = sessions(&env, T0);
    assert_eq!(v["tracking"], true);
    assert_eq!(
        v["sessions"],
        serde_json::json!([{ "case": "C-2026-001", "starting": false,
            "window": { "address": "0xa1", "workspace": "3", "pid": pid },
            "pids": [pid], "actor": "agent:default" }])
    );

    // --again starts another anyway
    let out = env.at(T0, &["agent", "start", "C-2026-001", "--again", "--json"]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert_eq!(calls(&calls_file), 1);
    assert_eq!(active_case(&root), "C-2026-001");

    // the window closed (and the grace of that launch over): free again
    live.kill();
    set_windows(&env, &[]);
    let out = env.at(T11, &["agent", "sessions"]);
    assert_eq!(
        stdout(&out).trim(),
        "No window of an agent that `seldon agent start` launched is open."
    );
    let out = env.at(T11, &["agent", "start", "C-2026-001", "--json"]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert_eq!(calls(&calls_file), 2);
}

/// The terminal's own process has no marker; the agent it runs does.
#[test]
fn a_marked_descendant_makes_the_window_a_session() {
    let env = Env::new(Snapper::Missing);
    let root = logbook(&env);
    let live = Live::default();
    let calls_file = launcher(&env);
    let pid = marked_child(&live, &marker("C-2026-001", &root));
    hyprctl(&env, &[window("0xa2", &pid.to_string(), AGENT)], "ok");
    let out = env.at(T0, &["agent", "start", "C-2026-001", "--json"]);
    assert_eq!(out.status.code(), Some(1), "{}", stderr(&out));
    assert!(message(&out).contains("(window 0xa2 on workspace 3)"));
    assert_eq!(calls(&calls_file), 0);
    let v = sessions(&env, T0);
    assert_eq!(v["sessions"][0]["window"]["pid"], pid);
    assert_ne!(
        v["sessions"][0]["pids"][0], pid,
        "the marked one is the child"
    );
}

/// An orphan the agent left behind (a daemon) carries the marker but has no
/// window: no session; nor is a marked window of another class one.
#[test]
fn a_marked_process_without_an_agent_window_is_no_session() {
    let env = Env::new(Snapper::Missing);
    let root = logbook(&env);
    let live = Live::default();
    let calls_file = launcher(&env);
    let _orphan = marked(&live, &marker("C-2026-001", &root));
    let foot = marked(&live, &marker("C-2026-001", &root));
    hyprctl(&env, &[window("0xf0", &foot.to_string(), "foot")], "ok");
    assert_eq!(sessions(&env, T0)["sessions"], serde_json::json!([]));
    let out = env.at(T0, &["agent", "start", "C-2026-001", "--json"]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert_eq!(calls(&calls_file), 1);
    let out = env.at(T11, &["agent", "focus", "C-2026-001", "--json"]);
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(
        message(&out),
        "no agent is working on C-2026-001: no window of an agent `seldon agent start` \
         launched on it is open; start one with `seldon agent start C-2026-001`"
    );
}

/// A launch counts as a session for 10 s while its window is not mapped.
#[test]
fn a_launch_counts_for_ten_seconds_without_a_window() {
    let env = Env::new(Snapper::Missing);
    logbook(&env);
    let calls_file = launcher(&env);
    let log = hyprctl(&env, &[], "ok");
    let out = env.at(T0, &["agent", "start", "C-2026-001", "--json"]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));

    let out = env.at(T5, &["agent", "start", "C-2026-001", "--json"]);
    assert_eq!(out.status.code(), Some(1), "{}", stderr(&out));
    assert_eq!(
        message(&out),
        "an agent was started on C-2026-001 5 s ago and its window is not open yet; focus it \
         with `seldon agent focus C-2026-001`, or start another with `seldon agent start \
         C-2026-001 --again`; nothing was launched"
    );
    assert_eq!(
        sessions(&env, T5)["sessions"],
        serde_json::json!([{ "case": "C-2026-001", "starting": true, "window": null,
            "pids": [], "actor": null }])
    );
    let out = env.at(T5, &["agent", "focus", "C-2026-001", "--json"]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert_eq!(
        json(&out),
        serde_json::json!({ "focused": false, "starting": true, "case": "C-2026-001" })
    );
    assert!(!log.exists(), "nothing dispatched");
    // another case is not held up
    let out = env.at(T5, &["agent", "start", "C-2026-002", "--json"]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));

    let out = env.at(T11, &["agent", "start", "C-2026-001", "--json"]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert_eq!(calls(&calls_file), 3);
    let launches = env.home.join(".local/state/seldon/launches.json");
    let kept = read(&launches);
    assert!(
        !kept.contains("15:30:00"),
        "records past the grace go: {kept}"
    );
    assert!(
        kept.contains("C-2026-002") && kept.contains("15:30:11"),
        "{kept}"
    );
    // C-2026-002's record (T5) is past the grace at T20: the next write drops it
    let out = env.at(T20, &["agent", "start", "C-2026-001", "--again", "--json"]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let kept = read(&launches);
    assert!(!kept.contains("C-2026-002"), "{kept}");
    assert!(kept.contains("15:30:20"), "{kept}");
}

/// Without hyprctl nothing is tracked: nothing is refused.
#[test]
fn without_hyprctl_nothing_is_tracked() {
    let env = Env::new(Snapper::Missing);
    let root = logbook(&env);
    let live = Live::default();
    let calls_file = launcher(&env);
    marked(&live, &marker("C-2026-001", &root));
    for _ in 0..2 {
        let out = env.at(T0, &["agent", "start", "C-2026-001", "--json"]);
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    }
    assert_eq!(calls(&calls_file), 2);
    let v = sessions(&env, T0);
    assert_eq!(v["tracking"], false);
    assert_eq!(v["sessions"], serde_json::json!([]));
    let out = env.at(T0, &["agent", "sessions"]);
    assert!(
        stdout(&out).starts_with("Not tracked: hyprctl not found"),
        "{}",
        stdout(&out)
    );
    let out = env.at(T0, &["agent", "focus", "C-2026-001", "--json"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(message(&out).starts_with("cannot focus the agent on C-2026-001: hyprctl not found"));
}

#[test]
fn again_is_for_a_case_id_only() {
    let env = Env::new(Snapper::Missing);
    logbook(&env);
    let calls_file = launcher(&env);
    let out = env.at(
        T0,
        &[
            "agent", "start", "--new", "--again", "--json", "--", "Do it.",
        ],
    );
    assert_eq!(out.status.code(), Some(1), "{}", stderr(&out));
    assert_eq!(calls(&calls_file), 0);
}

/// `--new` makes a new case, which no agent works on yet: never refused.
#[test]
fn a_new_case_is_never_refused() {
    let env = Env::new(Snapper::Missing);
    logbook(&env);
    std::fs::create_dir_all(env.home.join(".config/omarchy/defaults")).unwrap();
    std::fs::write(env.home.join(".config/omarchy/defaults/agent"), "claude\n").unwrap();
    let calls_file = launcher(&env);
    hyprctl(&env, &[], "ok");
    for (i, case) in ["C-2026-003", "C-2026-004"].iter().enumerate() {
        let out = env.at(
            T0,
            &["agent", "start", "--new", "--json", "--", "Same words."],
        );
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        assert_eq!(json(&out)["case"], *case);
        assert_eq!(calls(&calls_file), i + 1);
    }
}

/// Only this logbook's marker with a case id counts; an actor that does
/// not read as one is not reported.
#[test]
fn only_this_logbooks_case_markers_count() {
    let env = Env::new(Snapper::Missing);
    let root = logbook(&env);
    let live = Live::default();
    let other = marked(&live, &marker("C-2026-001", Path::new("/elsewhere")));
    let bogus = marked(&live, &marker("not-a-case", &root));
    let evil = marked(
        &live,
        &[
            ("SELDON_CASE", "C-2026-002"),
            ("SELDON_LOGBOOK", root.to_str().unwrap()),
            ("SELDON_ACTOR", "agent:Evil Name"),
        ],
    );
    hyprctl(
        &env,
        &[
            window("0x1", &other.to_string(), AGENT),
            window("0x2", &bogus.to_string(), AGENT),
            window("0x3", &evil.to_string(), AGENT),
        ],
        "ok",
    );
    let v = sessions(&env, T0);
    let found = v["sessions"].as_array().unwrap();
    assert_eq!(found.len(), 1, "{v}");
    assert_eq!(found[0]["case"], "C-2026-002");
    assert_eq!(found[0]["actor"], serde_json::Value::Null);
}

/// The engine's own process is no session, even inside a marked window.
#[test]
fn the_engine_itself_is_no_session() {
    let env = Env::new(Snapper::Missing);
    let root = logbook(&env);
    hyprctl(&env, &[window("0x5e1f", "PPID", AGENT)], "ok");
    let out = env
        .command(&["agent", "sessions", "--json"])
        .env("SELDON_NOW", T0)
        .env("SELDON_CASE", "C-2026-001")
        .env("SELDON_LOGBOOK", &root)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert_eq!(json(&out)["sessions"], serde_json::json!([]));
}

/// `agent ask` hands the agent no case: no session.
#[test]
fn an_ask_is_no_session() {
    let env = Env::new(Snapper::Missing);
    logbook(&env);
    let calls_file = launcher(&env);
    hyprctl(&env, &[], "ok");
    std::fs::create_dir_all(env.home.join(".config/omarchy/defaults")).unwrap();
    std::fs::write(env.home.join(".config/omarchy/defaults/agent"), "claude\n").unwrap();
    std::fs::create_dir_all(env.home.join(".claude/skills")).unwrap();
    let out = env.at(T0, &["hook", "install", "skills", "--json"]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let out = env.at(T0, &["agent", "ask", "case", "C-2026-001", "--json"]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert_eq!(sessions(&env, T0)["sessions"], serde_json::json!([]));
    let out = env.at(T0, &["agent", "start", "C-2026-001", "--json"]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert_eq!(calls(&calls_file), 2);
}

#[test]
fn focus_brings_the_agents_window_to_the_front() {
    let env = Env::new(Snapper::Missing);
    let root = logbook(&env);
    let live = Live::default();
    let pid = marked(&live, &marker("C-2026-001", &root));

    // the Lua dispatcher, Omarchy's first choice
    let log = hyprctl(&env, &[window("0x5a1d", &pid.to_string(), AGENT)], "ok");
    let out = env.at(T0, &["agent", "focus", "C-2026-001", "--json"]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let v = json(&out);
    assert_eq!(v["focused"], true);
    assert_eq!(v["case"], "C-2026-001");
    assert_eq!(v["address"], "0x5a1d");
    assert_eq!(v["workspace"], "3");
    assert_eq!(v["pid"], pid);
    assert_eq!(
        read(&log),
        "dispatch|hl.dsp.focus({ window = \"address:0x5a1d\" })\n"
    );

    // a Hyprland without it: the legacy dispatcher
    std::fs::remove_file(&log).unwrap();
    hyprctl(
        &env,
        &[window("0x5a1d", &pid.to_string(), AGENT)],
        "error: unknown dispatcher",
    );
    let out = env.at(T0, &["agent", "focus", "C-2026-001", "--json"]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert_eq!(
        read(&log),
        "dispatch|hl.dsp.focus({ window = \"address:0x5a1d\" })\n\
         dispatch|focuswindow|address:0x5a1d\n"
    );

    // an address that is not one never reaches a dispatch
    std::fs::remove_file(&log).unwrap();
    hyprctl(&env, &[window("0x1g", &pid.to_string(), AGENT)], "ok");
    let out = env.at(T0, &["agent", "focus", "C-2026-001", "--json"]);
    assert_eq!(out.status.code(), Some(1), "{}", stderr(&out));
    assert!(
        message(&out).contains("is not a window address"),
        "{}",
        message(&out)
    );
    assert!(!log.exists(), "nothing dispatched");
}

#[test]
fn opening_the_same_file_twice_focuses_its_terminal_window() {
    let env = Env::new(Snapper::Missing);
    let root = env.init_logbook();
    let live = Live::default();
    let calls_file = env.tmp.path().join("editor.calls");
    env.stub(
        "omarchy-launch-editor",
        &format!("echo call >> '{}'", calls_file.display()),
    );
    let status = root.join("STATUS.md");
    let editor = marked(&live, &[("SELDON_OPEN", status.to_str().unwrap())]);

    // no hyprctl (not a Hyprland session): opened as before
    let out = env.at(T0, &["open", "status", "--editor", "--json"]);
    assert_eq!(json(&out)["editor"]["launched"], true, "{}", stderr(&out));

    // a GUI editor's window: opened as before
    let log = hyprctl(&env, &[window("0xc0de", &editor.to_string(), "code")], "ok");
    let out = env.at(T0, &["open", "status", "--editor", "--json"]);
    assert_eq!(json(&out)["editor"]["launched"], true, "{}", stderr(&out));
    assert_eq!(calls(&calls_file), 2);

    // Omarchy's terminal window of it: focused, nothing started
    set_windows(
        &env,
        &[window("0xed17", &editor.to_string(), "org.omarchy.nvim")],
    );
    let out = env.at(T0, &["open", "status", "--editor", "--json"]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert_eq!(
        json(&out)["editor"],
        serde_json::json!({ "launched": false, "focused": true, "address": "0xed17",
            "pid": editor, "program": "omarchy-launch-editor" })
    );
    assert_eq!(
        read(&log),
        "dispatch|hl.dsp.focus({ window = \"address:0xed17\" })\n"
    );
    let out = env.at(T0, &["open", "status", "--editor"]);
    assert_eq!(
        stdout(&out).trim(),
        format!(
            "{} (already open; focused its window 0xed17)",
            status.display()
        )
    );
    assert_eq!(calls(&calls_file), 2, "one editor for the file");

    // another file opens its own
    let out = env.at(T0, &["open", "logbook", "--editor", "--json"]);
    assert_eq!(json(&out)["editor"]["launched"], true, "{}", stderr(&out));
    assert_eq!(calls(&calls_file), 3);
}
