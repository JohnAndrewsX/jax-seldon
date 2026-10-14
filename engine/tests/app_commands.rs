//! The commands the Seldon app brings (WP-229, ADR-0058): `seldon open`
//! starts the app, `seldon edit <target>` is the old `open <target>` (which
//! stays as a deprecated alias; `tests/commands.rs` keeps its tests),
//! `seldon skill [install]`, and `seldon changes --around`.
//!
//! Every run goes through `common::Env` (own HOME, no runtime dir, PATH the
//! stub directory only); the app is a fake `SELDON_DESK` in the temp dir.

mod common;

use std::path::{Path, PathBuf};
use std::process::Output;
use std::time::{Duration, Instant};

use common::{
    Env, Snapper, copy_dir, fixture_logbook, json, ledger, read, stderr, stdout, tree,
    write_executable,
};
use serde_json::Value;

const T0: &str = "2026-10-01T15:30:00+02:00";

/// `fixtures/commands/<name>`.
fn fixture_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../fixtures/commands")
        .join(name)
}

/// The fixture `name`, or with `SELDON_WRITE_COMMAND_FIXTURES=1` the
/// engine's output written there first.
fn held_to_fixture(name: &str, actual: &Value) {
    let path = fixture_path(name);
    if std::env::var_os("SELDON_WRITE_COMMAND_FIXTURES").is_some() {
        std::fs::write(&path, serde_json::to_string_pretty(actual).unwrap() + "\n").unwrap();
    }
    let expected: Value = serde_json::from_str(&read(&path)).unwrap();
    assert_eq!(
        actual, &expected,
        "{name}: rerun with SELDON_WRITE_COMMAND_FIXTURES=1"
    );
}

/// The host's `sleep`, by its path (PATH in `Env` is the stubs only).
fn host_sleep() -> PathBuf {
    std::env::var("PATH")
        .unwrap_or_default()
        .split(':')
        .map(|d| Path::new(d).join("sleep"))
        .find(|p| p.is_file())
        .expect("sleep on the host")
}

/// Waits up to 5 s for `path` to hold a line, and returns it.
fn line_of(path: &Path) -> String {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Ok(text) = std::fs::read_to_string(path)
            && text.ends_with('\n')
        {
            return text.trim().to_string();
        }
        assert!(
            Instant::now() < deadline,
            "{} never written",
            path.display()
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// `/proc/<pid>/stat` after the command name: state, ppid, pgrp, session,
/// tty_nr, …
fn stat(pid: &str) -> Vec<String> {
    std::fs::read_to_string(format!("/proc/{pid}/stat"))
        .expect("the app runs")
        .rsplit_once(')')
        .unwrap()
        .1
        .split_whitespace()
        .map(String::from)
        .collect()
}

mod open_app {
    use super::*;

    fn desk(env: &Env, body: &str) -> PathBuf {
        let path = env.tmp.path().join("desk/seldon-desk");
        write_executable(&path, &format!("#!/bin/sh\n{body}\n"));
        path
    }

    fn open(env: &Env, desk: &Path, args: &[&str]) -> Output {
        env.command(args).env("SELDON_DESK", desk).output().unwrap()
    }

    /// Without `--json` the engine becomes the app: same process, its
    /// output and its exit code. No logbook needed.
    #[test]
    fn bare_open_runs_the_app_in_the_foreground() {
        let env = Env::new(Snapper::Missing);
        let record = env.tmp.path().join("desk-run");
        let app = desk(
            &env,
            &format!(
                "echo \"$$ $#\" > '{}'\necho app says hello\necho app warns >&2\nexit 42",
                record.display()
            ),
        );
        let child = env
            .command(&["open"])
            .env("SELDON_DESK", &app)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        let pid = child.id().to_string();
        let out = child.wait_with_output().unwrap();
        assert_eq!(out.status.code(), Some(42), "the app's own exit code");
        assert_eq!(stdout(&out), "app says hello\n");
        assert_eq!(stderr(&out), "app warns\n");
        assert_eq!(
            read(&record),
            format!("{pid} 0\n"),
            "exec: the same process, no arguments"
        );
        assert!(!env.home.join("Seldon").exists(), "no logbook made");
    }

    /// With `--json` (the pill): started detached in a session of its own,
    /// stdio on /dev/null, `{"started": true}` at once; the app runs on.
    #[test]
    fn open_json_starts_the_app_detached_in_its_own_session() {
        let env = Env::new(Snapper::Missing);
        let record = env.tmp.path().join("desk-pid");
        let app = desk(
            &env,
            &format!(
                "echo $$ > '{}'\nexec '{}' 10",
                record.display(),
                host_sleep().display()
            ),
        );
        let start = Instant::now();
        let out = open(&env, &app, &["open", "--json"]);
        let took = start.elapsed();
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        assert!(took < Duration::from_secs(2), "took {took:?}");
        assert_eq!(json(&out), serde_json::json!({ "started": true }));
        held_to_fixture("open-app.json", &json(&out));
        assert_eq!(stderr(&out), "");

        let pid = line_of(&record);
        let fields = stat(&pid);
        assert_eq!(fields[3], pid, "a session of its own: {fields:?}");
        assert_eq!(fields[2], pid, "and its own process group");
        assert_eq!(fields[4], "0", "no controlling terminal");
        for fd in 0..3 {
            let target = std::fs::read_link(format!("/proc/{pid}/fd/{fd}")).unwrap();
            assert_eq!(target, Path::new("/dev/null"), "fd {fd}");
        }
        // it outlives the engine, which has exited
        std::thread::sleep(Duration::from_millis(300));
        let state = &stat(&pid)[0];
        assert!(state != "Z" && state != "X", "the app runs on: {state}");
        let _ = std::process::Command::new("kill")
            .args(["--", &format!("-{pid}")])
            .status();
    }

    #[test]
    fn an_app_that_fails_at_once_is_an_error_object() {
        let env = Env::new(Snapper::Missing);
        let app = desk(&env, "exit 3");
        let out = open(&env, &app, &["open", "--json"]);
        assert_eq!(out.status.code(), Some(1));
        let v = json(&out);
        assert_eq!(v["error"]["code"], 1);
        let message = v["error"]["message"].as_str().unwrap();
        assert!(message.contains("exited with 3"), "{message}");
    }

    #[test]
    fn a_missing_app_names_the_package() {
        let env = Env::new(Snapper::Missing);
        let missing = env.tmp.path().join("nowhere/seldon-desk");
        for args in [&["open"][..], &["open", "--json"]] {
            let out = open(&env, &missing, args);
            assert_eq!(out.status.code(), Some(1), "{args:?}");
            let text = if args.len() == 2 {
                json(&out)["error"]["message"].as_str().unwrap().to_string()
            } else {
                stderr(&out)
            };
            assert!(text.contains("install the seldon package"), "{text}");
            assert!(text.contains(missing.to_str().unwrap()), "{text}");
        }
        // a directory is no app either
        let dir = env.tmp.path().join("nowhere");
        std::fs::create_dir_all(&dir).unwrap();
        assert_eq!(open(&env, &dir, &["open", "--json"]).status.code(), Some(1));
    }

    /// `SELDON_DESK` names a file, never a program to look up on PATH.
    #[test]
    fn a_relative_desk_is_refused() {
        let env = Env::new(Snapper::Missing);
        env.stub("seldon-desk", "exit 0");
        let out = open(&env, Path::new("seldon-desk"), &["open", "--json"]);
        assert_eq!(out.status.code(), Some(1));
        let message = json(&out)["error"]["message"].as_str().unwrap().to_string();
        assert!(message.contains("not an absolute path"), "{message}");
    }

    #[test]
    fn editor_needs_a_target() {
        let env = Env::new(Snapper::Missing);
        let out = env.seldon(&["open", "--editor", "--json"]);
        assert_eq!(out.status.code(), Some(1));
    }
}

mod edit {
    use super::*;

    #[test]
    fn edit_prints_the_paths_open_printed() {
        let env = Env::new(Snapper::Missing);
        let root = env.init_logbook();
        let path_of = |args: &[&str]| -> String {
            let out = env.at(T0, args);
            assert_eq!(out.status.code(), Some(0), "{args:?}: {}", stderr(&out));
            assert_eq!(stderr(&out), "", "{args:?}");
            stdout(&out).trim().to_string()
        };
        assert_eq!(
            path_of(&["edit", "status"]),
            root.join("STATUS.md").display().to_string()
        );
        assert_eq!(
            path_of(&["edit", "journal"]),
            root.join("journal/2026/2026-10-01.md")
                .display()
                .to_string()
        );
        assert_eq!(path_of(&["edit", "logbook"]), root.display().to_string());
        env.at(T0, &["plan", "new", "--", "x"]);
        env.at(T0, &["plan", "start", "C-2026-001"]);
        assert!(path_of(&["edit", "case"]).ends_with("work/active/C-2026-001-x.md"));
        assert_eq!(path_of(&["edit", "C-2026-001"]), path_of(&["edit", "case"]));
        for bad in ["bogus", "C-2026-009", "ADR-0009"] {
            assert_eq!(env.at(T0, &["edit", bad]).status.code(), Some(1), "{bad}");
        }
        assert_eq!(env.at(T0, &["edit"]).status.code(), Some(1), "a target");
    }

    /// The alias: the same JSON on stdout, a note on stderr (none with
    /// `--quiet`).
    #[test]
    fn open_with_a_target_is_edit_with_a_note() {
        let env = Env::new(Snapper::Missing);
        env.init_logbook();
        let record = env.tmp.path().join("editor-args");
        env.stub(
            "omarchy-launch-editor",
            &format!(
                "for a in \"$@\"; do echo \"[$a]\"; done >> '{}'",
                record.display()
            ),
        );
        for args in [
            &["status", "--json"][..],
            &["ledger", "--json"],
            &["journal", "--editor", "--json"],
            &["status", "--editor", "--json"],
        ] {
            let edit = env.at(T0, &[&["edit"][..], args].concat());
            let open = env.at(T0, &[&["open"][..], args].concat());
            assert_eq!(edit.status.code(), Some(0), "{args:?}: {}", stderr(&edit));
            assert_eq!(open.status.code(), Some(0), "{args:?}: {}", stderr(&open));
            assert_eq!(json(&edit), json(&open), "{args:?}");
            assert_eq!(stderr(&edit), "", "{args:?}");
            let note = stderr(&open);
            assert!(
                note.contains("`seldon open <target>` is now `seldon edit <target>`"),
                "{note}"
            );
            assert_eq!(note.lines().count(), 1, "{note}");
        }
        let quiet = env.at(T0, &["--quiet", "open", "status"]);
        assert_eq!(quiet.status.code(), Some(0));
        assert_eq!(stderr(&quiet), "");
        let v = json(&env.at(T0, &["edit", "status", "--json"]));
        assert_eq!(v["what"], "status");
        assert!(v["editor"].is_null());
    }

    /// `seldon edit status --json` of the sample logbook at
    /// `/home/user/Seldon`, as the app and the 0.5 plugin read it.
    #[test]
    fn edit_json_is_the_fixture() {
        let env = Env::new(Snapper::Missing);
        let lb = env.tmp.path().join("logbook");
        copy_dir(&fixture_logbook(), &lb);
        let out = env.at(
            T0,
            &[
                "--logbook",
                lb.to_str().unwrap(),
                "edit",
                "status",
                "--json",
            ],
        );
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        let text = stdout(&out).replace(lb.to_str().unwrap(), "/home/user/Seldon");
        held_to_fixture("edit.json", &serde_json::from_str(&text).unwrap());
    }
}

mod skill {
    use super::*;

    const SKILL_MD: &str = include_str!("../assets/skills/seldon/SKILL.md");

    #[test]
    fn skill_prints_the_skill() {
        let env = Env::new(Snapper::Missing);
        let out = env.seldon(&["skill"]);
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        assert_eq!(stdout(&out), SKILL_MD);
        let v = json(&env.seldon(&["skill", "--json"]));
        assert_eq!(v["skill"], "seldon");
        assert_eq!(v["file"], "SKILL.md");
        assert_eq!(v["text"], SKILL_MD);
        assert_eq!(v["files"][0], "SKILL.md");
        assert_eq!(v["files"].as_array().unwrap().len(), 8);
        assert!(!env.home.join(".local").exists(), "nothing written");
    }

    /// `skill install` is `hook install skills`; twice changes nothing.
    #[test]
    fn skill_install_is_hook_install_skills_and_idempotent() {
        let env = Env::new(Snapper::Missing);
        for rel in [".agents/skills", ".claude/skills"] {
            std::fs::create_dir_all(env.home.join(rel)).unwrap();
        }
        let out = env.seldon(&["skill", "install", "--json"]);
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        let first = json(&out);
        let actions: Vec<&str> = first["dirs"]
            .as_array()
            .unwrap()
            .iter()
            .map(|d| d["action"].as_str().unwrap())
            .collect();
        assert_eq!(actions, ["installed", "installed"], "{first}");
        assert_eq!(
            read(&env.home.join(".agents/skills/seldon/SKILL.md")),
            SKILL_MD
        );
        let after_first = tree(&env.home);

        let out = env.seldon(&["skill", "install", "--json"]);
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        let second = json(&out);
        for d in second["dirs"].as_array().unwrap() {
            assert_eq!(d["action"], "unchanged", "{second}");
            assert_eq!(d["written"], serde_json::json!([]), "{second}");
        }
        let mut after_second = tree(&env.home);
        // the state lock file is touched by every run; the skill is not
        after_second.retain(|k, _| !k.ends_with("/lock"));
        let mut expected = after_first;
        expected.retain(|k, _| !k.ends_with("/lock"));
        assert_eq!(after_second, expected, "the second install wrote nothing");

        // and the same answer as the long form
        let long = env.seldon(&["hook", "install", "skills", "--json"]);
        assert_eq!(json(&long), second);
    }

    #[test]
    fn skill_takes_only_install() {
        let env = Env::new(Snapper::Missing);
        for args in [
            &["skill", "uninstall"][..],
            &["skill", "install", "--replace"],
            &["skill", "x"],
        ] {
            assert_eq!(env.seldon(args).status.code(), Some(1), "{args:?}");
        }
    }
}

mod changes {
    use super::*;

    fn fixture_copy(env: &Env) -> PathBuf {
        let lb = env.tmp.path().join("logbook");
        copy_dir(&fixture_logbook(), &lb);
        lb
    }

    fn changes(env: &Env, lb: &Path, args: &[&str]) -> Output {
        let mut all = vec!["--logbook", lb.to_str().unwrap(), "changes"];
        all.extend_from_slice(args);
        env.at(T0, &all)
    }

    fn ok(env: &Env, lb: &Path, args: &[&str]) -> Value {
        let out = changes(env, lb, args);
        assert_eq!(out.status.code(), Some(0), "{args:?}: {}", stderr(&out));
        json(&out)
    }

    /// The default window, 2 h before and 30 min after: the ledger's lines
    /// of that span, as written, in time order; nothing written anywhere.
    #[test]
    fn around_a_moment_lists_the_ledger_lines_of_the_window() {
        let env = Env::new(Snapper::Missing);
        let lb = fixture_copy(&env);
        let before = tree(&lb);
        let v = ok(
            &env,
            &lb,
            &["--around", "2026-10-01T14:20:00+02:00", "--json"],
        );
        assert_eq!(tree(&lb), before, "the logbook is untouched");
        assert!(
            !env.home.join(".local/state/seldon").exists(),
            "no state, no index"
        );
        assert_eq!(v["from"], "2026-10-01T12:20:00+02:00");
        assert_eq!(v["to"], "2026-10-01T14:50:00+02:00");
        assert_eq!(v["omitted"], 0);
        let mut expected: Vec<Value> = ledger(&lb)
            .into_iter()
            .filter(|e| {
                let ts = e["ts"].as_str().unwrap();
                ("2026-10-01T12:20:00+02:00"..="2026-10-01T14:50:00+02:00").contains(&ts)
            })
            .collect();
        expected.sort_by(|a, b| a["ts"].as_str().cmp(&b["ts"].as_str()));
        assert_eq!(v["events"], Value::Array(expected));
        let subjects: Vec<&str> = v["events"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e["subject"].as_str().unwrap())
            .collect();
        assert!(subjects.contains(&"ollama"), "{subjects:?}");
        assert!(
            subjects.contains(&"~/.config/systemd/user/ollama.service"),
            "{subjects:?}"
        );
        held_to_fixture("changes-around.json", &v);

        // the human form: one line per event
        let out = changes(&env, &lb, &["--around", "2026-10-01T14:20:00+02:00"]);
        let text = stdout(&out);
        assert!(
            text.starts_with("6 event(s) from 2026-10-01 12:20:00"),
            "{text}"
        );
        assert!(
            text.contains("2026-10-01 14:02:09  pacman/install  ollama"),
            "{text}"
        );
    }

    #[test]
    fn the_window_is_what_the_flags_say() {
        let env = Env::new(Snapper::Missing);
        let lb = fixture_copy(&env);
        let at = |args: &[&str]| -> Vec<String> {
            ok(&env, &lb, &[args, &["--json"]].concat())["events"]
                .as_array()
                .unwrap()
                .iter()
                .map(|e| e["subject"].as_str().unwrap().to_string())
                .collect()
        };
        assert_eq!(
            at(&[
                "--around",
                "2026-10-01T14:02:09+02:00",
                "--before",
                "0m",
                "--after",
                "0s"
            ]),
            ["ollama"]
        );
        // a window over the month's edge reads both files
        let both = at(&[
            "--around",
            "2026-10-01T00:00:00+02:00",
            "--before",
            "3d",
            "--after",
            "10h",
        ]);
        assert!(both.len() > 10, "{both:?}");
        assert!(both.contains(&"state-reset".to_string()), "{both:?}");
        // other spellings of the same moment, in another offset
        let rfc = at(&[
            "--around",
            "2026-10-01T12:20:00Z",
            "--before",
            "1h",
            "--after",
            "1h",
        ]);
        let epoch = at(&["--around", "@1790857200", "--before", "1h", "--after", "1h"]);
        assert_eq!(rfc, epoch);
        assert!(rfc.contains(&"ollama".to_string()), "{rfc:?}");
        // nothing in the window
        assert!(at(&["--around", "2026-12-24T18:00:00+01:00"]).is_empty());
    }

    #[test]
    fn bad_values_are_user_errors() {
        let env = Env::new(Snapper::Missing);
        let lb = fixture_copy(&env);
        for args in [
            &["--around", "yesterday"][..],
            &["--around", "2026-10-01"],
            &["--around", "now", "--before", "2"],
            &["--around", "now", "--after", "32d"],
            &["--around", "now", "--before", "-1h"],
            &[],
        ] {
            let out = changes(&env, &lb, &[args, &["--json"]].concat());
            assert_eq!(out.status.code(), Some(1), "{args:?}: {}", stdout(&out));
            assert!(json(&out)["error"]["message"].is_string(), "{args:?}");
        }
    }

    #[test]
    fn no_logbook_is_exit_3() {
        let env = Env::new(Snapper::Missing);
        let out = env.seldon(&["changes", "--around", "now", "--json"]);
        assert_eq!(out.status.code(), Some(3), "{}", stderr(&out));
        assert_eq!(json(&out)["error"]["code"], 3);
    }

    /// Over 500 events the ones nearest the moment stay.
    #[test]
    fn a_crowded_window_keeps_the_nearest_500() {
        let env = Env::new(Snapper::Missing);
        let lb = env.init_logbook();
        let base = chrono::DateTime::parse_from_rfc3339("2026-10-01T10:00:00+02:00").unwrap();
        let mut lines = String::new();
        for i in 0..520 {
            let ts = base + chrono::Duration::seconds(i);
            let event = serde_json::json!({
                "id": ulid::Ulid::generate().to_string(),
                "ts": ts.to_rfc3339(),
                "source": "manual",
                "kind": "note",
                "subject": format!("n{i}"),
                "actor": "human",
            });
            lines.push_str(&event.to_string());
            lines.push('\n');
        }
        std::fs::write(lb.join("ledger/2026-10.jsonl"), lines).unwrap();
        // the moment at n100: the 20 farthest are n500…n519
        let v = ok(
            &env,
            &lb,
            &[
                "--around",
                "2026-10-01T10:01:40+02:00",
                "--before",
                "1h",
                "--after",
                "1h",
                "--json",
            ],
        );
        let events = v["events"].as_array().unwrap();
        assert_eq!(events.len(), 500);
        assert_eq!(v["omitted"], 20);
        assert_eq!(events[0]["subject"], "n0");
        assert_eq!(events[499]["subject"], "n499");
    }
}
