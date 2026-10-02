//! `seldon hook …` (WP-009): the Claude Code and generic hooks against the
//! fixtures in `fixtures/hooks/`, the exit-0 promise, session start and
//! stop, and `hook install claude-code`.
//!
//! Everything runs in a throw-away home (`common::Env`); the pacman log,
//! theme file and plugin list point at temp files, never at the host.

mod common;

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Output, Stdio};
use std::time::Instant;

use common::{Env, Snapper, copy_dir, fixture_logbook, json, read, stderr, stdout};
use serde_json::{Value, json};

/// The clock of the hook tests (the fixture's C-2026-004 `yay` command).
const NOW: &str = "2026-10-01T10:11:20+02:00";

struct Hooks {
    env: Env,
    logbook: PathBuf,
}

impl Hooks {
    fn new() -> Self {
        let env = Env::new(Snapper::NoPermissions);
        let logbook = env.init_logbook();
        std::fs::write(env.tmp.path().join("pacman.log"), "").unwrap();
        Hooks { env, logbook }
    }

    fn command(&self, args: &[&str], now: Option<&str>) -> std::process::Command {
        let tmp = self.env.tmp.path();
        let mut cmd = self.env.command(args);
        cmd.env("SELDON_PACMAN_LOG", tmp.join("pacman.log"))
            .env("SELDON_PACMAN_DB_LOCK", tmp.join("no-db.lck"))
            .env("SELDON_OMARCHY", tmp.join("no-omarchy"))
            .env("SELDON_OMARCHY_PLUGINS_DIR", tmp.join("plugins"))
            .env("SELDON_THEME_FILE", tmp.join("theme.name"))
            .env("TZ", "Europe/Berlin");
        if let Some(now) = now {
            cmd.env("SELDON_NOW", now);
        }
        cmd
    }

    fn run(&self, args: &[&str]) -> Output {
        self.command(args, Some(NOW)).output().unwrap()
    }

    /// `seldon <args>` with `input` on stdin.
    fn piped(&self, args: &[&str], input: &str, now: Option<&str>) -> Output {
        let mut child = self
            .command(args, now)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(input.as_bytes())
            .unwrap();
        child.wait_with_output().unwrap()
    }

    /// `seldon hook <name>` with `payload`: must be silent and exit 0.
    fn hook(&self, name: &str, payload: &str) -> Output {
        let out = self.piped(&["hook", name], payload, Some(NOW));
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        assert_eq!(stdout(&out), "", "hooks print nothing");
        out
    }

    fn ledger(&self) -> Vec<Value> {
        common::ledger(&self.logbook)
    }

    fn commands(&self) -> Vec<Value> {
        self.ledger()
            .into_iter()
            .filter(|e| e["kind"] == "command")
            .collect()
    }

    /// A started case (`.seldon/active-case` names it); returns its id.
    fn active_case(&self) -> String {
        let out = self.run(&["plan", "new", "Zed installieren", "--json"]);
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        let id = json(&out)["event"]["subject"].as_str().unwrap().to_string();
        let out = self.run(&["plan", "start", &id]);
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        id
    }

    /// The case file of `id`.
    fn case_file(&self, id: &str) -> String {
        let dir = self.logbook.join("work/active");
        read(&common::find_file(&dir, id))
    }

    fn home(&self) -> &Path {
        &self.env.home
    }
}

/// `fixtures/hooks/<name>` with `hook_event_name` set to `event`.
fn payload(name: &str, event: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../fixtures/hooks")
        .join(name);
    let mut v: Value = serde_json::from_str(&read(&path)).unwrap();
    v["hook_event_name"] = json!(event);
    v.to_string()
}

/// A PreToolUse payload for `tool` with `input`.
fn tool_call(tool: &str, input: Value, id: &str) -> String {
    json!({
        "session_id": "6f1c2b9e-3a47-4d0e-9b8a-2c5d7e1f0a34",
        "transcript_path": "/home/user/.claude/projects/x.jsonl",
        "cwd": "/home/user/Seldon",
        "permission_mode": "default",
        "hook_event_name": "PreToolUse",
        "tool_name": tool,
        "tool_use_id": id,
        "tool_input": input,
    })
    .to_string()
}

mod claude_code {
    use super::*;

    #[test]
    fn mutating_fixture_records_one_command_event() {
        let h = Hooks::new();
        let case = h.active_case();
        h.hook(
            "claude-code",
            &payload("claude-code-mutating.json", "PreToolUse"),
        );
        let events = h.commands();
        assert_eq!(events.len(), 1, "{events:?}");
        let e = &events[0];
        assert_eq!(e["ts"], NOW, "the command's start (ADR-0017 §1)");
        assert_eq!(e["source"], "agent");
        assert_eq!(e["subject"], "yay");
        assert_eq!(e["actor"], "agent:claude-code");
        assert_eq!(e["case"], case.as_str());
        assert_eq!(e["zone"], "red");
        assert_eq!(e["meta"]["command"], "yay -S --noconfirm zed");
        assert_eq!(e["meta"]["toolUseId"], "toolu_01Hk7mX2qZP9wA3bC4dE5fG6");
        assert_eq!(
            e["meta"]["sessionId"],
            "6f1c2b9e-3a47-4d0e-9b8a-2c5d7e1f0a34"
        );
        assert!(e.get("detail").is_none());
        // the case file lists the event and the agent
        let file = h.case_file(&case);
        assert!(file.contains(e["id"].as_str().unwrap()), "{file}");
        assert!(file.contains("agents: [agent:claude-code]"), "{file}");
        // the tool's output is never recorded
        let ledger = read(&h.logbook.join("ledger/2026-10.jsonl"));
        assert!(!ledger.contains("Synchronizing"));
    }

    #[test]
    fn post_tool_use_after_pre_tool_use_writes_nothing() {
        let h = Hooks::new();
        h.hook(
            "claude-code",
            &payload("claude-code-mutating.json", "PreToolUse"),
        );
        h.hook(
            "claude-code",
            &payload("claude-code-mutating.json", "PostToolUse"),
        );
        assert_eq!(h.commands().len(), 1);
    }

    #[test]
    fn post_tool_use_alone_is_recorded() {
        // the fixtures as they are (PostToolUse): a settings file with only
        // the PostToolUse hook still records the command
        let h = Hooks::new();
        let raw = read(
            &Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../fixtures/hooks/claude-code-mutating.json"),
        );
        h.hook("claude-code", &raw);
        let events = h.commands();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0]["meta"]["command"], "yay -S --noconfirm zed");
        assert!(events[0].get("case").is_none(), "no active case");
    }

    #[test]
    fn non_mutating_fixture_records_nothing() {
        let h = Hooks::new();
        for event in ["PreToolUse", "PostToolUse"] {
            h.hook(
                "claude-code",
                &payload("claude-code-non-mutating.json", event),
            );
        }
        assert!(h.ledger().is_empty(), "{:?}", h.ledger());
    }

    #[test]
    fn secret_fixture_is_redacted() {
        let h = Hooks::new();
        h.hook(
            "claude-code",
            &payload("claude-code-secret.json", "PreToolUse"),
        );
        let events = h.commands();
        assert_eq!(events.len(), 1);
        let e = &events[0];
        assert_eq!(e["subject"], "git", "git inside ~/.config");
        assert_eq!(e["zone"], "yellow");
        let command = e["meta"]["command"].as_str().unwrap();
        assert!(command.contains("‹redacted›"), "{command}");
        assert!(command.contains("git -C ~/.config/hypr push"), "{command}");
        let line = read(&h.logbook.join("ledger/2026-10.jsonl"));
        for secret in [
            "AKIAIOSFODNN7EXAMPLE",
            "ghp_EXAMPLE",
            "user:",
            "hunter2",
            "sk-EXAMPLE",
            "this must never be recorded",
        ] {
            assert!(!line.contains(secret), "{secret} in {line}");
        }
    }

    #[test]
    fn edit_and_write_on_watched_paths() {
        let h = Hooks::new();
        let home = h.home().to_str().unwrap().to_string();
        // skipPaths: recorded with the path redacted (ADR-0014 §4)
        let mut config = seldon::config::Config::load(&h.env.config_file())
            .unwrap()
            .unwrap();
        config.redaction.skip_paths = vec!["secrets.conf".into()];
        config.save(&h.env.config_file()).unwrap();

        h.hook(
            "claude-code",
            &tool_call(
                "Edit",
                json!({"file_path": format!("{home}/.config/hypr/bindings.conf"),
                       "old_string": "a", "new_string": "hunter2"}),
                "toolu_edit",
            ),
        );
        h.hook(
            "claude-code",
            &tool_call(
                "Write",
                json!({"file_path": format!("{home}/.config/hypr/secrets.conf"),
                       "content": "token=hunter2"}),
                "toolu_write",
            ),
        );
        h.hook(
            "claude-code",
            &tool_call(
                "MultiEdit",
                json!({"file_path": format!("{home}/.config/systemd/user/a.service"),
                       "edits": [{"old_string": "a", "new_string": "b"}]}),
                "toolu_multi",
            ),
        );
        // not watched: nothing
        h.hook(
            "claude-code",
            &tool_call(
                "Write",
                json!({"file_path": "/tmp/notes.md", "content": "x"}),
                "toolu_tmp",
            ),
        );
        h.hook(
            "claude-code",
            &tool_call("Read", json!({"file_path": format!("{home}/.bashrc")}), "r"),
        );
        let got: Vec<(String, String, String)> = h
            .commands()
            .iter()
            .map(|e| {
                (
                    e["subject"].as_str().unwrap().into(),
                    e["meta"]["command"].as_str().unwrap().into(),
                    e["zone"].as_str().unwrap().into(),
                )
            })
            .collect();
        let row = |s: &str, c: &str, z: &str| (s.to_string(), c.to_string(), z.to_string());
        assert_eq!(
            got,
            [
                row("edit", "Edit ~/.config/hypr/bindings.conf", "yellow"),
                row("write", "Write ‹redacted›", "yellow"),
                row(
                    "multiedit",
                    "MultiEdit ~/.config/systemd/user/a.service",
                    "red"
                ),
            ]
        );
        let ledger = read(&h.logbook.join("ledger/2026-10.jsonl"));
        assert!(!ledger.contains("hunter2"), "content is never recorded");
        assert!(!ledger.contains("secrets.conf"));
    }

    #[test]
    fn heredoc_bodies_are_not_recorded() {
        let h = Hooks::new();
        h.hook(
            "claude-code",
            &tool_call(
                "Bash",
                json!({"command": "cat > ~/.config/hypr/extra.conf <<'EOF'\nbind = SUPER, Z, exec, hunter2\nEOF"}),
                "toolu_heredoc",
            ),
        );
        let events = h.commands();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0]["subject"], "cat");
        assert_eq!(
            events[0]["meta"]["command"],
            "cat > ~/.config/hypr/extra.conf <<'EOF'"
        );
    }

    #[test]
    fn always_exits_zero() {
        let h = Hooks::new();
        for bad in [
            "",
            "not json",
            "[]",
            "{}",
            r#"{"hook_event_name":"PreToolUse","tool_name":"Bash"}"#,
            r#"{"hook_event_name":"PreToolUse","tool_name":"Bash","tool_input":{"command":42}}"#,
            r#"{"hook_event_name":"Notification","message":"hi"}"#,
        ] {
            let out = h.piped(&["hook", "claude-code"], bad, Some(NOW));
            assert_eq!(out.status.code(), Some(0), "{bad}: {}", stderr(&out));
            assert_eq!(stdout(&out), "", "{bad}");
        }
        assert!(h.ledger().is_empty());

        // a bad clock, no logbook, a broken config: still exit 0, said on stderr
        let p = payload("claude-code-mutating.json", "PreToolUse");
        let out = h.piped(&["hook", "claude-code"], &p, Some("yesterday"));
        assert_eq!(out.status.code(), Some(0));
        assert!(stderr(&out).contains("SELDON_NOW"), "{}", stderr(&out));

        let elsewhere = h.env.tmp.path().join("nowhere");
        let out = h.piped(
            &[
                "--logbook",
                elsewhere.to_str().unwrap(),
                "hook",
                "claude-code",
            ],
            &p,
            Some(NOW),
        );
        assert_eq!(out.status.code(), Some(0));
        assert_eq!(stdout(&out), "");
        assert!(stderr(&out).contains("not initialised"), "{}", stderr(&out));
        for name in ["generic", "session-start", "session-stop"] {
            let out = h.piped(
                &["--logbook", elsewhere.to_str().unwrap(), "hook", name],
                "{}",
                Some(NOW),
            );
            assert_eq!(out.status.code(), Some(0), "{name}");
            assert_eq!(stdout(&out), "", "{name}");
        }

        let config = h.env.config_file();
        let saved = read(&config);
        std::fs::write(&config, "logbook = [").unwrap();
        let out = h.piped(&["hook", "claude-code"], &p, Some(NOW));
        assert_eq!(out.status.code(), Some(0));
        std::fs::write(&config, saved).unwrap();
        assert!(h.ledger().is_empty());
    }

    #[test]
    fn a_held_lock_is_waited_for() {
        let h = Hooks::new();
        let lock = seldon::logbook::lock::acquire(&h.env.lock_file()).unwrap();
        let release = std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(300));
            drop(lock);
        });
        h.hook(
            "claude-code",
            &payload("claude-code-mutating.json", "PreToolUse"),
        );
        release.join().unwrap();
        assert_eq!(h.commands().len(), 1);
    }

    #[test]
    fn fast_enough() {
        // < 5 ms is the release budget (measured in the handover); a debug
        // binary in a parallel test run gets a generous bound
        let h = Hooks::new();
        let p = payload("claude-code-mutating.json", "PreToolUse");
        let q = payload("claude-code-non-mutating.json", "PreToolUse");
        h.hook("claude-code", &p); // warm the page cache
        let start = Instant::now();
        for _ in 0..5 {
            h.hook("claude-code", &p);
            h.hook("claude-code", &q);
        }
        let each = start.elapsed() / 10;
        assert!(each.as_millis() < 400, "{each:?} per hook call");
    }
}

mod generic {
    use super::*;

    #[test]
    fn records_with_the_given_start_and_actor() {
        let h = Hooks::new();
        h.hook(
            "generic",
            &json!({
                "command": "sudo pacman -S --noconfirm ollama",
                "actor": "agent:codex",
                "cwd": "/tmp",
                "startedAt": "2026-10-01T14:01:48+02:00",
            })
            .to_string(),
        );
        h.hook(
            "generic",
            &json!({"command": "pacman -Qi ollama", "actor": "agent:codex", "cwd": "/tmp"})
                .to_string(),
        );
        let events = h.commands();
        assert_eq!(events.len(), 1);
        let e = &events[0];
        assert_eq!(e["ts"], "2026-10-01T14:01:48+02:00");
        assert_eq!(e["actor"], "agent:codex");
        assert_eq!(e["subject"], "pacman");
        assert_eq!(e["zone"], "red");
        assert_eq!(e["meta"]["command"], "sudo pacman -S --noconfirm ollama");
        assert!(e["meta"].get("toolUseId").is_none());
    }

    #[test]
    fn bad_input_is_reported_not_raised() {
        let h = Hooks::new();
        for bad in [
            json!({"command": "pacman -S x", "actor": "system"}),
            json!({"command": "pacman -S x", "actor": "Agent:X"}),
            json!({"command": "pacman -S x"}),
            json!({"command": "pacman -S x", "actor": "agent:x", "startedAt": "soon"}),
        ] {
            let out = h.piped(&["hook", "generic"], &bad.to_string(), Some(NOW));
            assert_eq!(out.status.code(), Some(0), "{bad}");
            assert!(!stderr(&out).is_empty(), "{bad}");
        }
        assert!(h.ledger().is_empty());
    }
}

mod sessions {
    use super::*;

    #[test]
    fn session_start_prints_the_context_block() {
        let env = Env::new(Snapper::NoPermissions);
        let logbook = env.tmp.path().join("logbook");
        copy_dir(&fixture_logbook(), &logbook);
        std::fs::write(logbook.join(".seldon/active-case"), "C-2026-004\n").unwrap();
        let out = env
            .command(&[
                "--logbook",
                logbook.to_str().unwrap(),
                "hook",
                "session-start",
            ])
            .env("SELDON_NOW", "2026-10-01T18:00:00+02:00")
            .stdin(Stdio::null())
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        assert_eq!(stderr(&out), "");
        golden("session-start.txt", &stdout(&out));
    }

    #[test]
    fn session_start_without_status_case_or_journal() {
        let h = Hooks::new();
        let out = h.piped(&["hook", "session-start"], "{}", Some(NOW));
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        let text = stdout(&out);
        assert!(text.starts_with("# Seldon logbook context\n"), "{text}");
        assert!(
            text.contains("None (`seldon plan start <id>` sets one)."),
            "{text}"
        );
    }

    #[test]
    fn session_stop_writes_the_stub_captures_and_commits() {
        let h = Hooks::new();
        let case = h.active_case();
        h.hook(
            "claude-code",
            &payload("claude-code-mutating.json", "PreToolUse"),
        );
        h.hook(
            "claude-code",
            &payload("claude-code-secret.json", "PreToolUse"),
        );
        let stop = json!({
            "session_id": "6f1c2b9e-3a47-4d0e-9b8a-2c5d7e1f0a34",
            "hook_event_name": "SessionEnd",
            "cwd": "/home/user/Seldon",
            "reason": "prompt_input_exit",
        });
        let out = h.piped(
            &["hook", "session-stop"],
            &stop.to_string(),
            Some("2026-10-01T10:45:00+02:00"),
        );
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        assert_eq!(stdout(&out), "");
        let journal = read(&h.logbook.join("journal/2026/2026-10-01.md"));
        assert!(
            journal.contains(&format!(
                "## 10:45 · agent:claude-code · {case}\nsession ended; 2 events recorded\n"
            )),
            "{journal}"
        );
        // capture ran (its cursors exist) and everything is committed
        assert!(h.home().join(".local/state/seldon/cursors.json").is_file());
        if h.env.has_git {
            let log = h.env.git(&h.logbook, &["log", "-1", "--format=%s"]);
            assert_eq!(
                String::from_utf8_lossy(&log.stdout).trim(),
                "seldon: session ended (agent:claude-code)"
            );
            let status = h.env.git(&h.logbook, &["status", "--porcelain"]);
            assert_eq!(String::from_utf8_lossy(&status.stdout), "");
        }
    }

    #[test]
    fn session_stop_without_a_session_counts_today() {
        let h = Hooks::new();
        let out = h.piped(
            &["hook", "session-stop", "--actor", "agent:codex"],
            "",
            Some(NOW),
        );
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        let journal = read(&h.logbook.join("journal/2026/2026-10-01.md"));
        assert!(
            journal.contains("## 10:11 · agent:codex\nsession ended; 0 events recorded\n"),
            "{journal}"
        );
    }

    /// Compares with `tests/golden/<name>`; `SELDON_BLESS=1` rewrites it.
    fn golden(name: &str, actual: &str) {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/golden")
            .join(name);
        if std::env::var_os("SELDON_BLESS").is_some() {
            std::fs::write(&path, actual).unwrap();
            return;
        }
        let expected = std::fs::read_to_string(&path)
            .unwrap_or_else(|_| panic!("{} missing; run with SELDON_BLESS=1", path.display()));
        assert_eq!(actual, expected, "{} differs", path.display());
    }
}

mod install {
    use super::*;

    /// A settings file a user already has: a guard hook, a Stop hook,
    /// permissions.
    pub(super) const EXISTING: &str = r#"{
  "permissions": { "allow": ["Bash(ls:*)"] },
  "hooks": {
    "PreToolUse": [
      { "matcher": "Bash", "hooks": [{ "type": "command", "command": "bash scripts/guard.sh" }] }
    ],
    "Stop": [
      { "hooks": [{ "type": "command", "command": "notify-send done" }] }
    ]
  },
  "model": "opus"
}
"#;

    fn ours(settings: &Value, event: &str) -> Vec<Value> {
        settings["hooks"][event]
            .as_array()
            .unwrap()
            .iter()
            .filter(|g| g.to_string().contains("seldon hook"))
            .cloned()
            .collect()
    }

    #[test]
    fn merges_into_existing_hooks_and_is_idempotent() {
        let h = Hooks::new();
        let path = h.env.tmp.path().join("project/.claude/settings.json");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, EXISTING).unwrap();

        let out = h.run(&[
            "hook",
            "install",
            "claude-code",
            "--settings",
            path.to_str().unwrap(),
        ]);
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        assert!(
            stdout(&out).contains("added    PreToolUse"),
            "{}",
            stdout(&out)
        );
        let after: Value = serde_json::from_str(&read(&path)).unwrap();
        let before: Value = serde_json::from_str(EXISTING).unwrap();
        // everything that was there is still there
        assert_eq!(after["permissions"], before["permissions"]);
        assert_eq!(after["model"], "opus");
        assert_eq!(after["hooks"]["Stop"], before["hooks"]["Stop"]);
        assert_eq!(
            after["hooks"]["PreToolUse"][0],
            before["hooks"]["PreToolUse"][0]
        );
        // ours, once each
        assert_eq!(
            ours(&after, "PreToolUse"),
            [json!({"matcher": "Bash|Edit|Write|MultiEdit",
                    "hooks": [{"type": "command", "command": "seldon hook claude-code", "timeout": 10}]})]
        );
        assert_eq!(
            ours(&after, "SessionStart"),
            [
                json!({"hooks": [{"type": "command", "command": "seldon hook session-start", "timeout": 10}]})
            ]
        );
        assert_eq!(
            ours(&after, "SessionEnd"),
            [
                json!({"hooks": [{"type": "command", "command": "seldon hook session-stop", "timeout": 60}]})
            ]
        );

        // again: nothing changes
        let text = read(&path);
        let out = h.run(&[
            "hook",
            "install",
            "claude-code",
            "--settings",
            path.to_str().unwrap(),
            "--json",
        ]);
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        let report = json(&out);
        assert_eq!(report["added"], json!([]));
        assert_eq!(report["present"].as_array().unwrap().len(), 3);
        assert_eq!(read(&path), text, "byte for byte");
    }

    #[test]
    fn default_path_is_the_logbook() {
        let h = Hooks::new();
        let out = h.run(&["hook", "install", "claude-code", "--json"]);
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        let path = h.logbook.join(".claude/settings.json");
        let settings: Value = serde_json::from_str(&read(&path)).unwrap();
        assert_eq!(json(&out)["added"].as_array().unwrap().len(), 3);
        assert_eq!(settings["hooks"].as_object().unwrap().len(), 3);
        if h.env.has_git {
            let log = h.env.git(&h.logbook, &["log", "-1", "--format=%s"]);
            assert_eq!(
                String::from_utf8_lossy(&log.stdout).trim(),
                "seldon: hook install claude-code"
            );
        }
    }

    #[test]
    fn refuses_a_broken_file() {
        let h = Hooks::new();
        let path = h.env.tmp.path().join("settings.json");
        for bad in [
            "{ not json",
            "[]",
            r#"{"hooks": []}"#,
            r#"{"hooks": {"PreToolUse": {}}}"#,
        ] {
            std::fs::write(&path, bad).unwrap();
            let out = h.run(&[
                "hook",
                "install",
                "claude-code",
                "--settings",
                path.to_str().unwrap(),
            ]);
            assert_eq!(out.status.code(), Some(1), "{bad}");
            assert!(
                stderr(&out).contains("nothing was changed"),
                "{}",
                stderr(&out)
            );
            assert_eq!(read(&path), bad);
        }
    }
}

/// `seldon hook uninstall claude-code` (WP-049): exactly what `install`
/// added comes out again.
mod uninstall {
    use super::*;

    /// `seldon hook install|uninstall claude-code --settings <path> --json`.
    fn hook(h: &Hooks, verb: &str, path: &std::path::Path) -> Value {
        let out = h.run(&[
            "hook",
            verb,
            "claude-code",
            "--settings",
            path.to_str().unwrap(),
            "--json",
        ]);
        assert_eq!(out.status.code(), Some(0), "{verb}: {}", stderr(&out));
        json(&out)
    }

    fn settings(path: &std::path::Path) -> Value {
        serde_json::from_str(&read(path)).unwrap()
    }

    #[test]
    fn takes_out_exactly_what_install_added() {
        let h = Hooks::new();
        let path = h.env.tmp.path().join("project/.claude/settings.json");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, super::install::EXISTING).unwrap();
        let before = settings(&path);
        hook(&h, "install", &path);

        let v = hook(&h, "uninstall", &path);
        assert_eq!(v["removed"].as_array().unwrap().len(), 3, "{v}");
        assert_eq!(v["absent"], json!([]));
        assert_eq!(v["deleted"], false);
        // the user's file as it was: their PreToolUse guard and Stop hook,
        // permissions, model; no empty SessionStart/SessionEnd lists left
        assert_eq!(settings(&path), before);

        // again: nothing to remove, the file is not touched
        let text = read(&path);
        let v = hook(&h, "uninstall", &path);
        assert_eq!(v["removed"], json!([]));
        assert_eq!(v["absent"].as_array().unwrap().len(), 3);
        assert_eq!(v["ownWrites"], Value::Null);
        assert_eq!(read(&path), text, "byte for byte");
    }

    #[test]
    fn keeps_hooks_the_user_added_to_seldons_groups() {
        let h = Hooks::new();
        let path = h.env.tmp.path().join("settings.json");
        hook(&h, "install", &path);
        // the user appends a hook of their own to Seldon's SessionStart group
        let mut s = settings(&path);
        s["hooks"]["SessionStart"][0]["hooks"]
            .as_array_mut()
            .unwrap()
            .push(json!({"type": "command", "command": "notify-send hi"}));
        std::fs::write(&path, serde_json::to_string_pretty(&s).unwrap()).unwrap();

        let v = hook(&h, "uninstall", &path);
        assert_eq!(v["deleted"], false, "{v}");
        assert_eq!(
            settings(&path),
            json!({"hooks": {"SessionStart": [
                {"hooks": [{"type": "command", "command": "notify-send hi"}]}
            ]}})
        );
    }

    #[test]
    fn a_file_with_only_seldons_hooks_is_deleted() {
        let h = Hooks::new();
        let path = h.env.tmp.path().join("new/.claude/settings.json");
        hook(&h, "install", &path);
        assert!(path.is_file());
        let out = h.run(&[
            "hook",
            "uninstall",
            "claude-code",
            "--settings",
            path.to_str().unwrap(),
        ]);
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        assert!(
            stdout(&out).contains("removed the Seldon hooks and the file"),
            "{}",
            stdout(&out)
        );
        assert!(!path.exists());
        assert!(path.parent().unwrap().is_dir(), "directories stay");
        // nothing there: nothing to do
        let v = hook(&h, "uninstall", &path);
        assert_eq!(v["removed"], json!([]));
        assert_eq!(v["deleted"], false);
        assert!(!path.exists());
    }

    #[test]
    fn default_path_is_the_logbook_and_commits() {
        let h = Hooks::new();
        let out = h.run(&["hook", "install", "claude-code"]);
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        let out = h.run(&["hook", "uninstall", "claude-code", "--json"]);
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        let v = json(&out);
        assert_eq!(v["deleted"], true, "{v}");
        assert_eq!(v["ownWrites"], json!([]), "the logbook is not watched");
        assert!(!h.logbook.join(".claude/settings.json").exists());
        if h.env.has_git {
            let log = h.env.git(&h.logbook, &["log", "-1", "--format=%s"]);
            assert_eq!(
                String::from_utf8_lossy(&log.stdout).trim(),
                "seldon: hook uninstall claude-code"
            );
            let status = h.env.git(&h.logbook, &["status", "--porcelain"]);
            assert_eq!(String::from_utf8_lossy(&status.stdout), "", "committed");
        }
    }

    #[test]
    fn refuses_a_broken_file() {
        let h = Hooks::new();
        let path = h.env.tmp.path().join("settings.json");
        for bad in [
            "{ not json",
            "[]",
            r#"{"hooks": []}"#,
            r#"{"hooks": {"SessionStart": {}}}"#,
        ] {
            std::fs::write(&path, bad).unwrap();
            let out = h.run(&[
                "hook",
                "uninstall",
                "claude-code",
                "--settings",
                path.to_str().unwrap(),
            ]);
            assert_eq!(out.status.code(), Some(1), "{bad}");
            assert!(
                stderr(&out).contains("nothing was changed"),
                "{}",
                stderr(&out)
            );
            assert_eq!(read(&path), bad);
        }
    }

    #[test]
    fn the_lock_covers_the_write() {
        // the lock is taken before the settings file is read or written:
        // while another seldon holds it, nothing changes (exit 4), so no
        // capture can see the file before its own-write record
        let h = Hooks::new();
        let path = h.env.tmp.path().join("settings.json");
        hook(&h, "install", &path);
        let text = read(&path);
        let lock = seldon::logbook::lock::acquire(&h.env.lock_file()).unwrap();
        for verb in ["uninstall", "install"] {
            let out = h.run(&[
                "hook",
                verb,
                "claude-code",
                "--settings",
                path.to_str().unwrap(),
            ]);
            assert_eq!(out.status.code(), Some(4), "{verb}: {}", stderr(&out));
            assert_eq!(read(&path), text, "{verb}: untouched");
        }
        // an install that would write: a fresh `{}` stays `{}`
        let fresh = h.env.tmp.path().join("fresh.json");
        std::fs::write(&fresh, "{}").unwrap();
        let out = h.run(&[
            "hook",
            "install",
            "claude-code",
            "--settings",
            fresh.to_str().unwrap(),
        ]);
        assert_eq!(out.status.code(), Some(4), "fresh: {}", stderr(&out));
        assert_eq!(read(&fresh), "{}", "fresh: untouched");
        drop(lock);
        assert_eq!(hook(&h, "uninstall", &path)["deleted"], true);
    }

    #[test]
    fn is_not_an_agent_hook() {
        // a harness never calls it: errors keep their exit code instead of
        // the agent hooks' silent 0
        let env = Env::new(Snapper::Missing);
        let out = env.seldon(&["hook", "uninstall", "claude-code"]);
        assert_eq!(out.status.code(), Some(3), "{}", stderr(&out));
        let out = env.seldon(&["hook", "uninstall", "generic"]);
        assert_eq!(out.status.code(), Some(1), "{}", stderr(&out));
    }
}

/// `seldon event` as the theme-set hook calls it (WP-005 decision 7).
mod event {
    use super::*;

    fn theme_set(h: &Hooks, now: &str, subject: &str, extra: &[&str]) -> Value {
        let mut args = vec![
            "event",
            "theme",
            "theme-set",
            "--subject",
            subject,
            "--json",
        ];
        args.extend(extra);
        let out = h.command(&args, Some(now)).output().unwrap();
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        json(&out)["event"].clone()
    }

    #[test]
    fn the_default_actor_is_system() {
        let h = Hooks::new();
        let out = h.run(&["event", "manual", "note", "--subject", "x", "--json"]);
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        assert_eq!(json(&out)["event"]["actor"], "system");
    }

    #[test]
    fn theme_set_fills_from_with_the_theme_it_replaces() {
        let h = Hooks::new();
        // nothing known yet: no from
        let e = theme_set(&h, "2026-10-01T08:00:00+02:00", "catppuccin", &[]);
        assert!(e.get("meta").is_none(), "{e}");
        assert!(e.get("detail").is_none(), "{e}");

        // the theme collector last saw kanagawa at 09:00 (after 08:00)
        let root = std::fs::canonicalize(&h.logbook).unwrap();
        let cursors = json!({
            "logbook": root,
            "collectors": {"theme": {
                "cursor": {"theme": "kanagawa", "checked": "2026-10-01T09:00:00+02:00"},
                "ok": true, "lastRun": "2026-10-01T09:00:00+02:00", "events": 0,
            }},
        });
        let state = h.home().join(".local/state/seldon");
        std::fs::create_dir_all(&state).unwrap();
        std::fs::write(state.join("cursors.json"), cursors.to_string()).unwrap();
        let e = theme_set(&h, "2026-10-01T15:30:00+02:00", "tokyo-night", &[]);
        assert_eq!(e["actor"], "system");
        assert_eq!(e["detail"], "kanagawa → tokyo-night");
        assert_eq!(e["meta"], json!({"from": "kanagawa", "to": "tokyo-night"}));

        // a theme-set newer than the cursor wins
        let e = theme_set(&h, "2026-10-01T16:00:00+02:00", "rose-pine", &[]);
        assert_eq!(e["meta"]["from"], "tokyo-night");

        // given values are kept
        let e = theme_set(
            &h,
            "2026-10-01T16:30:00+02:00",
            "nord",
            &["--meta", "from=matte-black", "--detail", "by hand"],
        );
        assert_eq!(e["meta"], json!({"from": "matte-black"}));
        assert_eq!(e["detail"], "by hand");

        // never the theme itself
        let e = theme_set(&h, "2026-10-01T17:00:00+02:00", "matte-black", &[]);
        assert_eq!(e["meta"]["from"], "nord");
        let e = theme_set(&h, "2026-10-01T17:01:00+02:00", "matte-black", &[]);
        assert!(e.get("meta").is_none(), "{e}");
    }

    #[test]
    fn an_agents_theme_switch_is_attributed() {
        // the theme-set hook records the switch before any capture sees it
        let h = Hooks::new();
        let case = h.active_case();
        let out = h.piped(
            &["hook", "claude-code"],
            &tool_call(
                "Bash",
                json!({"command": "omarchy theme set 'Tokyo Night'"}),
                "toolu_theme",
            ),
            Some("2026-10-01T15:29:40+02:00"),
        );
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        let e = theme_set(&h, "2026-10-01T15:30:00+02:00", "tokyo-night", &[]);
        assert_eq!(e["actor"], "agent:claude-code");
        assert_eq!(e["case"], case.as_str());
        // a switch nobody's command named stays system
        let e = theme_set(&h, "2026-10-01T15:31:00+02:00", "nord", &[]);
        assert_eq!(e["actor"], "system");
    }
}

/// ADR-0019: agent commands no collector tracks are green, and recorded
/// only while a case is set.
mod green {
    use super::*;

    /// (subject, zone, meta.command) of the recorded command events.
    fn recorded(h: &Hooks) -> Vec<(String, String, String)> {
        h.commands()
            .iter()
            .map(|e| {
                (
                    e["subject"].as_str().unwrap().into(),
                    e["zone"].as_str().unwrap().into(),
                    e["meta"]["command"].as_str().unwrap().into(),
                )
            })
            .collect()
    }

    fn row(s: &str, z: &str, c: &str) -> (String, String, String) {
        (s.into(), z.into(), c.into())
    }

    /// The calls of the test: a tee into an unwatched config dir (the
    /// WP-015 fixture line), npm, git push in a project, Edit on an
    /// unwatched path, and a tee into a watched path (yellow, always).
    fn calls(h: &Hooks) {
        let home = h.home().to_str().unwrap().to_string();
        let bash = |command: &str, cwd: &str, id: &str| {
            let mut p: Value =
                serde_json::from_str(&tool_call("Bash", json!({"command": command}), id)).unwrap();
            p["cwd"] = json!(cwd);
            h.hook("claude-code", &p.to_string());
        };
        bash("tee ~/.config/zed/settings.json", "/tmp", "t1");
        bash("npm install", "/tmp/project", "t2");
        bash("git push origin main", "/tmp/project", "t3");
        h.hook(
            "claude-code",
            &tool_call(
                "Edit",
                json!({"file_path": format!("{home}/Work/notes.md"), "old_string": "a", "new_string": "b"}),
                "t4",
            ),
        );
        bash("tee -a ~/.bashrc", "/tmp", "t5");
        bash("npm run build 2>/dev/null", "/tmp/project", "t6");
    }

    #[test]
    fn recorded_with_a_case() {
        let h = Hooks::new();
        let case = h.active_case();
        calls(&h);
        assert_eq!(
            recorded(&h),
            [
                row("tee", "green", "tee ~/.config/zed/settings.json"),
                row("npm", "green", "npm install"),
                row("git", "green", "git push origin main"),
                row("edit", "green", "Edit ~/Work/notes.md"),
                row("tee", "yellow", "tee -a ~/.bashrc"),
            ]
        );
        assert!(h.commands().iter().all(|e| e["case"] == case.as_str()));
    }

    #[test]
    fn nothing_green_without_a_case() {
        let h = Hooks::new();
        calls(&h);
        assert_eq!(recorded(&h), [row("tee", "yellow", "tee -a ~/.bashrc")]);
    }

    #[test]
    fn the_generic_hook_takes_a_case() {
        let h = Hooks::new();
        let case = h.active_case();
        // clear the active case: only the explicit one counts
        std::fs::remove_file(h.logbook.join(".seldon/active-case")).unwrap();
        let npm = json!({"command": "npm install", "actor": "agent:codex", "cwd": "/tmp/p"});
        h.hook("generic", &npm.to_string());
        assert!(h.commands().is_empty(), "no case, nothing green");
        let out = h.piped(
            &["hook", "generic", "--case", &case],
            &npm.to_string(),
            Some(NOW),
        );
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        let mut with_field = npm.clone();
        with_field["case"] = json!(case);
        h.hook("generic", &with_field.to_string());
        let events = h.commands();
        assert_eq!(events.len(), 2);
        assert!(
            events
                .iter()
                .all(|e| e["case"] == case.as_str() && e["zone"] == "green")
        );
        // an unknown case is reported; a tracked command is still recorded
        let out = h.piped(
            &["hook", "generic", "--case", "C-2026-999"],
            &json!({"command": "yay -S zed", "actor": "agent:codex"}).to_string(),
            Some(NOW),
        );
        assert_eq!(out.status.code(), Some(0));
        assert!(stderr(&out).contains("C-2026-999"), "{}", stderr(&out));
        let last = h.commands().pop().unwrap();
        assert_eq!(
            (last["subject"].as_str(), last.get("case")),
            (Some("yay"), None)
        );
    }
}

/// Review round 1: a panic and a closed stdout never fail the agent.
mod robustness {
    use super::*;

    #[test]
    fn a_panic_exits_zero() {
        let h = Hooks::new();
        let mut cmd = h.command(&["hook", "claude-code"], Some(NOW));
        let out = cmd
            .env("SELDON_TEST_HOOK_PANIC", "1")
            .stdin(Stdio::null())
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(0));
        assert_eq!(stdout(&out), "");
        assert!(stderr(&out).contains("internal error"), "{}", stderr(&out));
    }

    #[test]
    fn session_start_survives_a_closed_pipe() {
        let env = Env::new(Snapper::NoPermissions);
        let logbook = env.tmp.path().join("logbook");
        copy_dir(&fixture_logbook(), &logbook);
        let mut child = env
            .command(&[
                "--logbook",
                logbook.to_str().unwrap(),
                "hook",
                "session-start",
            ])
            .env("SELDON_NOW", "2026-10-01T18:00:00+02:00")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        // the reader goes away before the hook writes
        drop(child.stdout.take());
        child.stdin.take().unwrap().write_all(b"{}").unwrap();
        let out = child.wait_with_output().unwrap();
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        assert!(!stderr(&out).contains("internal error"), "{}", stderr(&out));
    }
}

/// The index follows the hook's writes (CONTRACT rule 2, WP-007's fast
/// rebuild), and only them.
mod index {
    use super::*;

    fn index(h: &Hooks) -> Option<String> {
        std::fs::read_to_string(h.home().join(".local/state/seldon/index.json")).ok()
    }

    #[test]
    fn a_hook_write_rebuilds_the_index() {
        let h = Hooks::new();
        let before = index(&h);
        h.hook(
            "claude-code",
            &payload("claude-code-non-mutating.json", "PreToolUse"),
        );
        assert_eq!(index(&h), before, "nothing written, no rebuild");

        h.hook(
            "claude-code",
            &payload("claude-code-mutating.json", "PreToolUse"),
        );
        let id = h.commands()[0]["id"].as_str().unwrap().to_string();
        let after = index(&h).expect("index.json written");
        let v: Value = serde_json::from_str(&after).unwrap();
        assert!(
            v["events"]
                .as_array()
                .unwrap()
                .iter()
                .any(|e| e["id"] == id.as_str()),
            "the new event is in the index"
        );
        assert!(
            v["logbook"]["git"].get("dirty").is_none(),
            "the fast rebuild runs no git"
        );

        // session-stop's full rebuild runs after its commit
        let out = h.piped(&["hook", "session-stop"], "", Some(NOW));
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        let v: Value = serde_json::from_str(&index(&h).unwrap()).unwrap();
        if h.env.has_git {
            assert_eq!(v["logbook"]["git"]["dirty"], false, "{}", v["logbook"]);
        }
    }
}
