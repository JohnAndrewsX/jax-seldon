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
        let app = desk(&env, "kill -9 $$");
        let out = open(&env, &app, &["open", "--json"]);
        assert_eq!(out.status.code(), Some(1));
        let message = json(&out)["error"]["message"].as_str().unwrap().to_string();
        assert!(message.contains("exited with a signal"), "{message}");
    }

    /// An exit 0 within the 200 ms is a start: a second `seldon open`
    /// hands over to the window that is open (ADR-0058 §1).
    #[test]
    fn an_app_that_exits_0_at_once_has_started() {
        let env = Env::new(Snapper::Missing);
        let app = desk(&env, "exit 0");
        let out = open(&env, &app, &["open", "--json"]);
        assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
        assert_eq!(json(&out), serde_json::json!({ "started": true }));
    }

    /// A descriptor the caller passes without close-on-exec stays with
    /// the engine: the app gets only its /dev/null stdio.
    #[test]
    fn an_inherited_descriptor_does_not_reach_the_app() {
        use std::os::fd::AsRawFd as _;
        use std::os::unix::process::CommandExt as _;
        unsafe extern "C" {
            fn dup2(old: i32, new: i32) -> i32;
        }
        let env = Env::new(Snapper::Missing);
        let record = env.tmp.path().join("desk-fds");
        let pid_file = env.tmp.path().join("desk-pid");
        let app = desk(
            &env,
            &format!(
                "echo $$ > '{}'\n\
                 if [ -e /proc/$$/fd/7 ]; then echo fd7; else echo none; fi > '{}'\n\
                 exec '{}' 10",
                pid_file.display(),
                record.display(),
                host_sleep().display()
            ),
        );
        let passed = std::fs::File::create(env.tmp.path().join("passed")).unwrap();
        let fd = passed.as_raw_fd();
        let mut cmd = env.command(&["open", "--json"]);
        cmd.env("SELDON_DESK", &app);
        // SAFETY: dup2(2) only, in the forked child; the copy at 7 has no
        // close-on-exec flag, as a shell's `7>file` leaves it
        unsafe {
            cmd.pre_exec(move || {
                if dup2(fd, 7) < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
        let out = cmd.output().unwrap();
        assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
        assert_eq!(line_of(&record), "none");
        let _ = std::process::Command::new("kill")
            .args(["--", &format!("-{}", line_of(&pid_file))])
            .status();
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
            assert!(
                text.contains("The Seldon window is not installed"),
                "{text}"
            );
            assert!(text.contains("Install the seldon package."), "{text}");
            assert!(text.contains(missing.to_str().unwrap()), "{text}");
        }
    }

    /// There, but no regular executable file: said so, not "missing".
    #[test]
    fn a_path_that_is_no_executable_is_said_so() {
        let env = Env::new(Snapper::Missing);
        let dir = env.tmp.path().join("desk-dir");
        std::fs::create_dir_all(&dir).unwrap();
        let plain = env.tmp.path().join("desk-plain");
        std::fs::write(&plain, "#!/bin/sh\nexit 0\n").unwrap();
        for path in [&dir, &plain] {
            for args in [&["open"][..], &["open", "--json"]] {
                let out = open(&env, path, args);
                assert_eq!(out.status.code(), Some(1), "{path:?} {args:?}");
                let text = if args.len() == 2 {
                    json(&out)["error"]["message"].as_str().unwrap().to_string()
                } else {
                    stderr(&out)
                };
                assert!(text.contains("is not a regular executable file"), "{text}");
                assert!(!text.contains("missing"), "{text}");
            }
        }
    }

    /// A script whose interpreter is gone: the app's sentence, not the
    /// editor's advice.
    #[test]
    fn a_missing_interpreter_is_the_apps_error() {
        let env = Env::new(Snapper::Missing);
        let app = env.tmp.path().join("desk/seldon-desk");
        write_executable(&app, "#!/nonexistent/interpreter\n");
        for args in [&["open"][..], &["open", "--json"]] {
            let out = open(&env, &app, args);
            assert_eq!(out.status.code(), Some(1), "{args:?}");
            let text = if args.len() == 2 {
                json(&out)["error"]["message"].as_str().unwrap().to_string()
            } else {
                stderr(&out)
            };
            assert!(text.contains("interpreter or loader"), "{text}");
            assert!(!text.contains("EDITOR"), "{text}");
        }
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

    /// `skill install --replace` is `hook install skills --replace`: the
    /// same report from the same start, and a logbook needed (exit 3).
    #[test]
    fn skill_install_replace_is_hook_install_skills_replace() {
        let run = |args: &[&str]| -> Value {
            let env = Env::new(Snapper::Missing);
            env.init_logbook();
            let claude = env.home.join(".claude/skills");
            std::fs::create_dir_all(&claude).unwrap();
            let out = env.seldon(&["skill", "install"]);
            assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
            std::fs::write(claude.join("seldon/SKILL.md"), "my notes\n").unwrap();
            let out = env
                .command(args)
                .env("SELDON_NOW", "2026-10-06T10:00:00+02:00")
                .output()
                .unwrap();
            assert_eq!(out.status.code(), Some(0), "{args:?}: {}", stderr(&out));
            assert_eq!(read(&claude.join("seldon/SKILL.md")), SKILL_MD, "{args:?}");
            let mut dirs = json(&out)["dirs"].clone();
            // the temp home differs between the two runs
            for d in dirs.as_array_mut().unwrap() {
                d.as_object_mut().unwrap().remove("path");
            }
            dirs
        };
        let front = run(&["--json", "skill", "install", "--replace"]);
        assert_eq!(front[0]["action"], "replaced", "{front}");
        assert_eq!(
            front,
            run(&["--json", "hook", "install", "skills", "--replace"])
        );
        let env = Env::new(Snapper::Missing);
        std::fs::create_dir_all(env.home.join(".claude/skills")).unwrap();
        let out = env.seldon(&["skill", "install", "--replace", "--json"]);
        assert_eq!(out.status.code(), Some(3), "{}", stderr(&out));
    }

    #[test]
    fn skill_takes_only_install() {
        let env = Env::new(Snapper::Missing);
        for args in [
            &["skill", "uninstall"][..],
            &["skill", "--replace"],
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
        let events = v["events"].as_array().unwrap();
        let classes: Vec<(&str, &str, &Value)> = events
            .iter()
            .map(|e| {
                (
                    e["source"].as_str().unwrap(),
                    e["subject"].as_str().unwrap(),
                    &e["class"],
                )
            })
            .collect();
        // the machine changes by ADR-0028's table; commands, the snapshot
        // and the note are none
        assert_eq!(
            classes,
            [
                ("agent", "pacman", &Value::Null),
                ("pacman", "ollama", &Value::from("attention")),
                ("agent", "tee", &Value::Null),
                (
                    "config",
                    "~/.config/systemd/user/ollama.service",
                    &Value::from("crisis")
                ),
                ("snapper", "113", &Value::Null),
                ("manual", "journal", &Value::Null),
            ]
        );
        // the ledger lines, each with its class
        let lines: Vec<Value> = events
            .iter()
            .map(|e| {
                let mut e = e.clone();
                e.as_object_mut().unwrap().remove("class");
                e
            })
            .collect();
        let mut expected: Vec<Value> = ledger(&lb)
            .into_iter()
            .filter(|e| {
                let ts = e["ts"].as_str().unwrap();
                ("2026-10-01T12:20:00+02:00"..="2026-10-01T14:50:00+02:00").contains(&ts)
            })
            .collect();
        expected.sort_by(|a, b| a["ts"].as_str().cmp(&b["ts"].as_str()));
        assert_eq!(lines, expected);
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
            text.contains("2026-10-01 14:02:09  attention  pacman/install  ollama"),
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
            // past what a time can hold or RFC 3339 can write: exit 1, no panic
            &["--around", "@8210266876799", "--after", "31d"],
            &["--around", "@8210266800000", "--after", "0s"],
            &["--around", "@253402300799", "--after", "1d"],
            &["--around", "0001-01-01T00:30:00+00:00", "--before", "1h"],
        ] {
            let out = changes(&env, &lb, &[args, &["--json"]].concat());
            assert_eq!(out.status.code(), Some(1), "{args:?}: {}", stdout(&out));
            assert!(json(&out)["error"]["message"].is_string(), "{args:?}");
        }
        // a date alone: the message asks for a time and names the forms
        let out = changes(&env, &lb, &["--around", "2026-10-01", "--json"]);
        let message = json(&out)["error"]["message"].as_str().unwrap().to_string();
        assert!(message.contains("give a time"), "{message}");
        for form in [
            "RFC 3339",
            "2026-10-01 14:20",
            "@<seconds since 1970>",
            "now",
        ] {
            assert!(message.contains(form), "{form}: {message}");
        }
        // the ends themselves still read
        let out = changes(
            &env,
            &lb,
            &[
                "--around",
                "9999-12-30T00:00:00+00:00",
                "--after",
                "1d",
                "--json",
            ],
        );
        assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    }

    /// Local times follow the engine's zone (`TZ`). In the autumn overlap
    /// the first pass (still summer time); in the spring gap, exit 1.
    #[test]
    fn local_times_across_the_clock_changes() {
        let env = Env::new(Snapper::Missing);
        let lb = fixture_copy(&env);
        let around = |tz: &str, local: &str| -> Option<String> {
            let out = env
                .command(&[
                    "--logbook",
                    lb.to_str().unwrap(),
                    "changes",
                    "--around",
                    local,
                    "--json",
                ])
                .env("SELDON_NOW", T0)
                .env("TZ", tz)
                .output()
                .unwrap();
            match out.status.code() {
                Some(0) => Some(json(&out)["around"].as_str().unwrap().to_string()),
                Some(1) => None,
                c => panic!("{tz} {local}: exit {c:?}: {}", stdout(&out)),
            }
        };
        // the zone file, and the same rules as a POSIX TZ string (no
        // zone data needed)
        for tz in ["Europe/Berlin", "CET-1CEST,M3.5.0,M10.5.0/3"] {
            let at = |local: &str| around(tz, local);
            assert_eq!(
                at("2026-10-25 01:59").as_deref(),
                Some("2026-10-25T01:59:00+02:00"),
                "{tz}"
            );
            assert_eq!(
                at("2026-10-25 02:30").as_deref(),
                Some("2026-10-25T02:30:00+02:00"),
                "{tz}: the first 02:30"
            );
            assert_eq!(
                at("2026-10-25 02:59:59").as_deref(),
                Some("2026-10-25T02:59:59+02:00"),
                "{tz}"
            );
            assert_eq!(
                at("2026-10-25 03:00").as_deref(),
                Some("2026-10-25T03:00:00+01:00"),
                "{tz}"
            );
            assert_eq!(at("2026-03-29 02:30"), None, "{tz}: skipped");
            assert_eq!(
                at("2026-03-29 01:59").as_deref(),
                Some("2026-03-29T01:59:00+01:00"),
                "{tz}"
            );
            assert_eq!(
                at("2026-03-29 03:00").as_deref(),
                Some("2026-03-29T03:00:00+02:00"),
                "{tz}"
            );
        }
        // the exit 1 says why
        let out = env
            .command(&[
                "--logbook",
                lb.to_str().unwrap(),
                "changes",
                "--around",
                "2026-03-29 02:30",
                "--json",
            ])
            .env("SELDON_NOW", T0)
            .env("TZ", "Europe/Berlin")
            .output()
            .unwrap();
        let message = json(&out)["error"]["message"].as_str().unwrap().to_string();
        assert!(message.contains("does not exist"), "{message}");
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
        // the moment at n419: the 20 farthest are n0…n19, not the last 20
        // (first-500 would keep n0…n499)
        let v = ok(
            &env,
            &lb,
            &[
                "--around",
                "2026-10-01T10:06:59+02:00",
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
        assert_eq!(events[0]["subject"], "n20");
        assert_eq!(events[499]["subject"], "n519");
        // a note is no machine change: no class
        assert!(events.iter().all(|e| e["class"].is_null()));
    }
}
