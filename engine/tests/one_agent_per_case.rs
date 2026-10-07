//! One agent per case, and one editor per file (WP-156): `agent start`
//! refuses while the session it launched on the case lives (found by its
//! `SELDON_CASE`/`SELDON_LOGBOOK` marker), `--again` starts another, `agent
//! sessions` lists them, `agent focus` brings the window to the front
//! through `hyprctl`; `open --editor` focuses the editor it opened on the
//! same path instead of starting a second one.
//!
//! The launchers here are stubs that stay alive (`exec sleep`), so their
//! markers are real processes in `/proc`; [`Live`] kills them (each is the
//! leader of its own process group) when a test ends, also on a failure.

mod common;

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use common::{Env, Snapper, json, read, stderr};

const T0: &str = "2026-10-01T15:30:00+02:00";

/// A program on the host's PATH (the engine's PATH is the stub directory).
fn host(name: &str) -> PathBuf {
    std::env::var("PATH")
        .unwrap_or_default()
        .split(':')
        .map(|d| Path::new(d).join(name))
        .find(|p| p.is_file())
        .unwrap_or_else(|| panic!("{name} on the host"))
}

/// The stubs' pids, written one per line to `file`; killed on drop.
struct Live {
    file: PathBuf,
}

impl Live {
    fn pids(&self) -> Vec<String> {
        std::fs::read_to_string(&self.file)
            .unwrap_or_default()
            .lines()
            .map(String::from)
            .collect()
    }

    /// Kills every stub that still runs (by its process group), waits
    /// until `/proc` no longer lists them, and forgets them: a pid is never
    /// signalled after its process is gone.
    fn kill(&self) {
        let pids: Vec<String> = self.pids().into_iter().filter(|p| alive(p)).collect();
        for pid in &pids {
            let _ = std::process::Command::new(host("kill"))
                .args(["--", &format!("-{pid}")])
                .status();
        }
        let deadline = Instant::now() + Duration::from_secs(5);
        while pids.iter().any(|p| alive(p)) && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        let _ = std::fs::remove_file(&self.file);
    }
}

impl Drop for Live {
    fn drop(&mut self) {
        self.kill();
    }
}

/// Alive and not a zombie.
fn alive(pid: &str) -> bool {
    std::fs::read_to_string(format!("/proc/{pid}/stat")).is_ok_and(|s| {
        s.rsplit_once(')')
            .and_then(|(_, rest)| rest.split_whitespace().next().map(String::from))
            .is_some_and(|state| state != "Z" && state != "X")
    })
}

/// A stub `name` that counts its calls in `<tmp>/<name>.calls`, writes its
/// pid to `<tmp>/<name>.pids` and stays alive.
fn live_stub(env: &Env, name: &str) -> (Live, PathBuf) {
    let base = env.tmp.path().join(name);
    let (calls, pids) = (base.with_extension("calls"), base.with_extension("pids"));
    env.stub(
        name,
        &format!(
            "echo call >> '{}'; echo $$ >> '{}'; exec '{}' 30",
            calls.display(),
            pids.display(),
            host("sleep").display()
        ),
    );
    (Live { file: pids }, calls)
}

/// A stub `hyprctl`: `clients -j` lists one window `address` for the first
/// pid in `pids` (none when `address` is empty); `dispatch` appends its
/// arguments to `<tmp>/hyprctl.dispatch` (one call per line, the
/// arguments joined by `|`) and answers `lua` for the Lua form, `ok` for
/// the legacy one.
fn hyprctl(env: &Env, pids: &Path, address: &str, lua: &str) -> PathBuf {
    let log = env.tmp.path().join("hyprctl.dispatch");
    let clients = if address.is_empty() {
        "echo '[]'".to_string()
    } else {
        format!(
            "read pid < '{}'; printf '[{{\"address\":\"%s\",\"pid\":%s,\"class\":\"org.omarchy.agent\",\
             \"title\":\"t\",\"workspace\":{{\"id\":3,\"name\":\"3\"}}}}]\\n' '{address}' \"$pid\"",
            pids.display()
        )
    };
    env.stub(
        "hyprctl",
        &format!(
            "if [ \"$1\" = clients ]; then {clients}; exit 0; fi\n\
             (IFS='|'; echo \"$*\") >> '{log}'\n\
             case \"$2\" in hl.dsp.*) echo '{lua}';; *) echo ok;; esac",
            log = log.display()
        ),
    );
    log
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

#[test]
fn a_second_start_is_refused_while_the_first_agent_works() {
    let env = Env::new(Snapper::Missing);
    let root = logbook(&env);
    let (live, calls) = live_stub(&env, "omarchy");

    let out = env.at(T0, &["agent", "start", "C-2026-001", "--json"]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let first = live.pids()[0].clone();
    assert!(alive(&first));

    // another case becomes the active one; the refusal must not touch it
    env.at(T0, &["agent", "start", "C-2026-002", "--json"]);
    assert_eq!(active_case(&root), "C-2026-002");
    let out = env.at(T0, &["agent", "start", "C-2026-001", "--json"]);
    assert_eq!(out.status.code(), Some(1), "{}", stderr(&out));
    assert_eq!(
        message(&out),
        format!(
            "an agent is already working on C-2026-001 (pid {first}); focus it with `seldon \
             agent focus C-2026-001`, or start another with `seldon agent start C-2026-001 \
             --again`; nothing was launched"
        )
    );
    assert_eq!(read(&calls), "call\ncall\n", "no third launch");
    assert_eq!(
        active_case(&root),
        "C-2026-002",
        "the active case untouched"
    );

    // the list: one entry per case, oldest pid first
    let v = json(&env.at(T0, &["agent", "sessions", "--json"]));
    let sessions = v["sessions"].as_array().unwrap();
    assert_eq!(sessions.len(), 2, "{v}");
    assert_eq!(sessions[0]["case"], "C-2026-001");
    assert_eq!(
        sessions[0]["pids"],
        serde_json::json!([first.parse::<u32>().unwrap()])
    );
    assert_eq!(sessions[0]["actor"], "agent:default");
    assert_eq!(sessions[1]["case"], "C-2026-002");

    // --again starts another anyway
    let out = env.at(T0, &["agent", "start", "C-2026-001", "--again", "--json"]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert_eq!(read(&calls).lines().count(), 3);
    assert_eq!(active_case(&root), "C-2026-001");
    let v = json(&env.at(T0, &["agent", "sessions", "--json"]));
    assert_eq!(v["sessions"][0]["pids"].as_array().unwrap().len(), 2, "{v}");

    // the sessions end: nothing listed, and a start launches again
    live.kill();
    let out = env.at(T0, &["agent", "sessions"]);
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(
        common::stdout(&out).trim(),
        "No agent that `seldon agent start` launched is running."
    );
    let out = env.at(T0, &["agent", "start", "C-2026-001", "--json"]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert_eq!(read(&calls).lines().count(), 4);
}

#[test]
fn again_is_for_a_case_id_only() {
    let env = Env::new(Snapper::Missing);
    logbook(&env);
    let (_live, calls) = live_stub(&env, "omarchy");
    let out = env.at(
        T0,
        &[
            "agent", "start", "--new", "--again", "--json", "--", "Do it.",
        ],
    );
    assert_eq!(out.status.code(), Some(1), "{}", stderr(&out));
    assert!(!calls.exists(), "nothing launched");
}

/// `--new` makes a new case, which no agent works on yet: never refused.
#[test]
fn a_new_case_is_never_refused() {
    let env = Env::new(Snapper::Missing);
    logbook(&env);
    std::fs::create_dir_all(env.home.join(".config/omarchy/defaults")).unwrap();
    std::fs::write(env.home.join(".config/omarchy/defaults/agent"), "claude\n").unwrap();
    let (_live, calls) = live_stub(&env, "omarchy");
    for (i, case) in ["C-2026-003", "C-2026-004"].iter().enumerate() {
        let out = env.at(
            T0,
            &["agent", "start", "--new", "--json", "--", "Same words."],
        );
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        assert_eq!(json(&out)["case"], *case);
        assert_eq!(read(&calls).lines().count(), i + 1);
    }
}

/// A case id is unique per logbook: another logbook's session on the same
/// id is not this one's.
#[test]
fn another_logbooks_session_does_not_count() {
    let a = Env::new(Snapper::Missing);
    let b = Env::new(Snapper::Missing);
    logbook(&a);
    logbook(&b);
    let (_live_a, _) = live_stub(&a, "omarchy");
    let (_live_b, calls_b) = live_stub(&b, "omarchy");
    let out = a.at(T0, &["agent", "start", "C-2026-001", "--json"]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let out = b.at(T0, &["agent", "start", "C-2026-001", "--json"]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert_eq!(read(&calls_b), "call\n");
    let v = json(&b.at(T0, &["agent", "sessions", "--json"]));
    assert_eq!(v["sessions"].as_array().unwrap().len(), 1, "{v}");
}

/// `agent ask` hands the agent no case: it is no session of one.
#[test]
fn an_ask_is_no_session() {
    let env = Env::new(Snapper::Missing);
    logbook(&env);
    let (_live, calls) = live_stub(&env, "omarchy");
    std::fs::create_dir_all(env.home.join(".config/omarchy/defaults")).unwrap();
    std::fs::write(env.home.join(".config/omarchy/defaults/agent"), "claude\n").unwrap();
    std::fs::create_dir_all(env.home.join(".claude/skills")).unwrap();
    let out = env.at(T0, &["hook", "install", "skills", "--json"]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let out = env.at(T0, &["agent", "ask", "case", "C-2026-001", "--json"]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let v = json(&env.at(T0, &["agent", "sessions", "--json"]));
    assert_eq!(v["sessions"], serde_json::json!([]));
    let out = env.at(T0, &["agent", "start", "C-2026-001", "--json"]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert_eq!(read(&calls).lines().count(), 2);
}

#[test]
fn focus_brings_the_agents_window_to_the_front() {
    let env = Env::new(Snapper::Missing);
    logbook(&env);

    // no session yet
    let out = env.at(T0, &["agent", "focus", "C-2026-001", "--json"]);
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(
        message(&out),
        "no agent is working on C-2026-001: none that `seldon agent start` launched still \
         runs; start one with `seldon agent start C-2026-001`"
    );

    let (live, _) = live_stub(&env, "omarchy");
    let out = env.at(T0, &["agent", "start", "C-2026-001", "--json"]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let pid = live.pids()[0].clone();

    // no hyprctl: said, not guessed
    let out = env.at(T0, &["agent", "focus", "C-2026-001", "--json"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(
        message(&out).contains("hyprctl not found"),
        "{}",
        message(&out)
    );

    // the Lua dispatcher, Omarchy's first choice
    let log = hyprctl(&env, &live.file, "0x5a1d", "ok");
    let out = env.at(T0, &["agent", "focus", "C-2026-001", "--json"]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let v = json(&out);
    assert_eq!(v["focused"], true);
    assert_eq!(v["case"], "C-2026-001");
    assert_eq!(v["address"], "0x5a1d");
    assert_eq!(v["workspace"], "3");
    assert_eq!(v["pid"].to_string(), pid);
    assert_eq!(
        read(&log),
        "dispatch|hl.dsp.focus({ window = \"address:0x5a1d\" })\n"
    );

    // a Hyprland without it: the legacy dispatcher
    std::fs::remove_file(&log).unwrap();
    hyprctl(&env, &live.file, "0x5a1d", "error: unknown dispatcher");
    let out = env.at(T0, &["agent", "focus", "C-2026-001", "--json"]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert_eq!(
        read(&log),
        "dispatch|hl.dsp.focus({ window = \"address:0x5a1d\" })\n\
         dispatch|focuswindow|address:0x5a1d\n"
    );

    // no window of it
    std::fs::remove_file(&log).unwrap();
    hyprctl(&env, &live.file, "", "ok");
    let out = env.at(T0, &["agent", "focus", "C-2026-001", "--json"]);
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(
        message(&out),
        format!("an agent is working on C-2026-001 (pid {pid}), but Hyprland has no window of it")
    );
    assert!(!log.exists(), "nothing dispatched");

    // an address that is not one never reaches a dispatch
    hyprctl(&env, &live.file, "0x1g", "ok");
    let out = env.at(T0, &["agent", "focus", "C-2026-001", "--json"]);
    assert_eq!(out.status.code(), Some(1), "{}", stderr(&out));
    assert!(
        message(&out).contains("is not a window address"),
        "{}",
        message(&out)
    );
    assert!(!log.exists(), "nothing dispatched");

    // the session ends
    live.kill();
    let out = env.at(T0, &["agent", "focus", "C-2026-001", "--json"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(message(&out).starts_with("no agent is working on C-2026-001"));
}

#[test]
fn opening_the_same_file_twice_focuses_the_first_editor() {
    let env = Env::new(Snapper::Missing);
    let root = env.init_logbook();
    let (live, calls) = live_stub(&env, "omarchy-launch-editor");

    let out = env.at(T0, &["open", "status", "--editor", "--json"]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert_eq!(json(&out)["editor"]["launched"], true);

    // no hyprctl (not a Hyprland session): opened again, as before
    let out = env.at(T0, &["open", "status", "--editor", "--json"]);
    assert_eq!(json(&out)["editor"]["launched"], true, "{}", stderr(&out));
    assert_eq!(read(&calls).lines().count(), 2);
    live.kill();
    std::fs::remove_file(&calls).unwrap();

    let out = env.at(T0, &["open", "status", "--editor", "--json"]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let pid: u32 = live.pids()[0].parse().unwrap();

    // its window is not there yet: nothing started
    let log = hyprctl(&env, &live.file, "", "ok");
    let out = env.at(T0, &["open", "status", "--editor", "--json"]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert_eq!(
        json(&out)["editor"],
        serde_json::json!({ "launched": false, "running": true, "pid": pid,
            "program": "omarchy-launch-editor" })
    );

    // its window: focused
    hyprctl(&env, &live.file, "0xed17", "ok");
    let out = env.at(T0, &["open", "status", "--editor", "--json"]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert_eq!(
        json(&out)["editor"],
        serde_json::json!({ "launched": false, "focused": true, "address": "0xed17", "pid": pid,
            "program": "omarchy-launch-editor" })
    );
    assert_eq!(
        read(&log),
        "dispatch|hl.dsp.focus({ window = \"address:0xed17\" })\n"
    );
    let out = env.at(T0, &["open", "status", "--editor"]);
    assert_eq!(
        common::stdout(&out).trim(),
        format!(
            "{} (already open; focused its window 0xed17)",
            root.join("STATUS.md").display()
        )
    );
    assert_eq!(read(&calls), "call\n", "one editor for the file");

    // another file opens its own
    let out = env.at(T0, &["open", "logbook", "--editor", "--json"]);
    assert_eq!(json(&out)["editor"]["launched"], true, "{}", stderr(&out));
    assert_eq!(read(&calls).lines().count(), 2);
}
