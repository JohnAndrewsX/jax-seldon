//! `seldon event`, `seldon decide`, `seldon open` (SPEC-ENGINE §3).

mod common;

use common::{Env, Snapper, find_file, json, ledger, read, stderr, stdout};
use seldon::model::{self, Case, Decision, DecisionStatus};

const T0: &str = "2026-10-01T15:30:00+02:00";

mod event {
    use super::*;

    #[test]
    fn records_a_manual_event() {
        let env = Env::new(Snapper::Missing);
        let root = env.init_logbook();
        let out = env.at(
            T0,
            &[
                "event",
                "theme",
                "theme-set",
                "--subject",
                "tokyo-night",
                "--detail",
                "kanagawa → tokyo-night",
                "--meta",
                "from=kanagawa",
                "--meta",
                "to=tokyo-night",
                "--actor",
                "human",
                "--json",
            ],
        );
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        let events = ledger(&root);
        assert_eq!(events.len(), 1);
        let e = &events[0];
        assert_eq!(json(&out)["event"], *e);
        // the fixture line's shape (ledger/2026-10.jsonl, 15:30)
        let line = read(&root.join("ledger/2026-10.jsonl"));
        let expected = format!(
            "{{\"id\":\"{}\",\"ts\":\"2026-10-01T15:30:00+02:00\",\"source\":\"theme\",\"kind\":\"theme-set\",\"subject\":\"tokyo-night\",\"detail\":\"kanagawa → tokyo-night\",\"actor\":\"human\",\"zone\":\"yellow\",\"meta\":{{\"from\":\"kanagawa\",\"to\":\"tokyo-night\"}}}}\n",
            e["id"].as_str().unwrap()
        );
        assert_eq!(line, expected);
        if env.has_git {
            // the subject is not redacted, so it stays out of git's argv
            let head = stdout(&env.git(&root, &["log", "-1", "--format=%s"]));
            assert_eq!(head.trim(), "seldon: event theme/theme-set");
        }
    }

    #[test]
    fn attributes_to_a_case_and_types_meta() {
        let env = Env::new(Snapper::Missing);
        let root = env.init_logbook();
        env.at(T0, &["plan", "new", "--", "Tyme"]);
        let out = env.at(
            T0,
            &[
                "event",
                "plugins",
                "plugin-add",
                "--subject",
                "io.github.example.tyme",
                "--case",
                "C-2026-001",
                "--actor",
                "agent:codex",
                "--meta",
                "version=1.1.0",
                "--meta",
                "enabled=false",
                "--meta",
                "note=x y",
                "--json",
            ],
        );
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        let e = ledger(&root).pop().unwrap();
        assert_eq!(
            e["meta"],
            serde_json::json!({"version": "1.1.0", "enabled": false, "note": "x y"})
        );
        assert_eq!(e["case"], "C-2026-001");
        let path = find_file(&root.join("work/queued"), "C-2026-001-");
        let (case, _) = model::load::<Case>(&path).unwrap();
        assert_eq!(case.events, [e["id"].as_str().unwrap()]);
        assert_eq!(case.agents, ["agent:codex"]);
    }

    #[test]
    fn engine_kinds_and_bad_values_are_refused() {
        let env = Env::new(Snapper::Missing);
        let root = env.init_logbook();
        for args in [
            &["event", "seldon", "note", "--subject", "x"][..],
            &["event", "manual", "case-started", "--subject", "C-2026-001"][..],
            &["event", "manual", "resolution", "--subject", "x"][..],
            &["event", "manual", "correction", "--subject", "x"][..],
            &["event", "bogus", "note", "--subject", "x"][..],
            &["event", "manual", "bogus", "--subject", "x"][..],
            &["event", "manual", "note"][..],
            &["event", "manual", "note", "--subject", " "][..],
            &[
                "event",
                "manual",
                "note",
                "--subject",
                "x",
                "--meta",
                "txId=1",
            ][..],
            &[
                "event",
                "manual",
                "note",
                "--subject",
                "x",
                "--meta",
                "nokey",
            ][..],
            &[
                "event",
                "manual",
                "note",
                "--subject",
                "x",
                "--actor",
                "root",
            ][..],
            &[
                "event",
                "manual",
                "note",
                "--subject",
                "x",
                "--case",
                "C-2026-001",
            ][..],
        ] {
            let out = env.at(T0, args);
            assert_eq!(out.status.code(), Some(1), "{args:?}: {}", stderr(&out));
        }
        let long = "x".repeat(513);
        let out = env.at(T0, &["event", "manual", "note", "--subject", &long]);
        assert_eq!(out.status.code(), Some(1));
        assert!(ledger(&root).is_empty());
    }

    #[test]
    fn hyphen_values_are_text() {
        let env = Env::new(Snapper::Missing);
        let root = env.init_logbook();
        let out = env.at(
            T0,
            &[
                "event",
                "config",
                "config-change",
                "--subject",
                "~/.config/systemd/user/x.service",
                "--detail",
                "--json",
            ],
        );
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        assert!(!stdout(&out).starts_with('{'));
        let e = ledger(&root).pop().unwrap();
        assert_eq!(e["detail"], "--json");
        assert_eq!(e["zone"], "red", "systemd units are red (ADR-0014 §2)");
    }
}

mod decide {
    use super::*;

    #[test]
    fn creates_a_proposed_decision() {
        let env = Env::new(Snapper::Missing);
        let root = env.init_logbook();
        env.at(T0, &["plan", "new", "--", "Zed"]);
        let out = env.at(
            T0,
            &[
                "decide",
                "--no-edit",
                "--case",
                "C-2026-001",
                "--json",
                "--",
                "Zed statt VS Code",
            ],
        );
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        let v = json(&out);
        assert_eq!(v["decision"]["id"], "ADR-0001");
        assert_eq!(
            v["decision"]["path"],
            "decisions/ADR-0001-zed-statt-vs-code.md"
        );
        assert!(v["editor"].is_null());
        let (d, doc) =
            model::load::<Decision>(&root.join("decisions/ADR-0001-zed-statt-vs-code.md")).unwrap();
        assert_eq!(d.title, "Zed statt VS Code");
        assert_eq!(d.status, DecisionStatus::Proposed);
        assert_eq!(d.date.to_string(), "2026-10-01");
        assert_eq!(d.cases, ["C-2026-001"]);
        assert!(
            doc.body
                .starts_with("# ADR-0001 — Zed statt VS Code\n\n## Context\n")
        );
        assert!(doc.body.contains("\n## Decision\n") && doc.body.contains("\n## Consequences\n"));

        let out = env.at(T0, &["decide", "--no-edit", "--json", "--", "Second"]);
        assert_eq!(json(&out)["decision"]["id"], "ADR-0002");
        // no ledger kind fits a decision: only the case-created line
        assert_eq!(ledger(&root).len(), 1);
        if env.has_git {
            let head = stdout(&env.git(&root, &["log", "-1", "--format=%s"]));
            assert_eq!(head.trim(), "seldon: ADR-0002 proposed");
        }
        assert_eq!(
            env.at(
                T0,
                &["decide", "--no-edit", "--case", "C-2026-009", "--", "x"]
            )
            .status
            .code(),
            Some(1)
        );
    }

    #[test]
    fn the_logbook_template_wins() {
        let env = Env::new(Snapper::Missing);
        let root = env.init_logbook();
        std::fs::write(
            root.join(".seldon/templates/decision.md"),
            "# {{id}}: {{title}}\n\nMine.\n",
        )
        .unwrap();
        env.at(T0, &["decide", "--no-edit", "--", "{{id}} in a title"]);
        let path = find_file(&root.join("decisions"), "ADR-0001-");
        assert!(read(&path).ends_with("---\n# ADR-0001: {{id}} in a title\n\nMine.\n"));
    }

    #[test]
    fn opens_the_editor_with_the_path_as_one_argument() {
        let env = Env::new(Snapper::Missing);
        let root = env.init_logbook_at("my logbook", "en");
        let record = env.tmp.path().join("editor-args");
        env.stub(
            "omarchy-launch-editor",
            &format!(
                "for a in \"$@\"; do echo \"[$a]\"; done > '{}'",
                record.display()
            ),
        );
        let out = env.at(
            T0,
            &[
                "--logbook",
                root.to_str().unwrap(),
                "decide",
                "--json",
                "--",
                "Edit me",
            ],
        );
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        assert_eq!(json(&out)["editor"]["launched"], true);
        let path = root.join("decisions/ADR-0001-edit-me.md");
        assert_eq!(read(&record), format!("[{}]\n", path.display()));
    }
}

mod open {
    use super::*;

    #[test]
    fn prints_paths() {
        let env = Env::new(Snapper::Missing);
        let root = env.init_logbook();
        let path_of = |args: &[&str]| -> String {
            let out = env.at(T0, args);
            assert_eq!(out.status.code(), Some(0), "{args:?}: {}", stderr(&out));
            stdout(&out).trim().to_string()
        };
        assert_eq!(
            path_of(&["open", "status"]),
            root.join("STATUS.md").display().to_string()
        );
        assert_eq!(
            path_of(&["open", "journal"]),
            root.join("journal/2026/2026-10-01.md")
                .display()
                .to_string()
        );
        assert!(
            !root.join("journal/2026").exists(),
            "printing creates nothing"
        );
        assert_eq!(
            path_of(&["open", "ledger"]),
            root.join("ledger/2026-10.jsonl").display().to_string()
        );
        std::fs::write(root.join("ledger/2026-10.md"), "view\n").unwrap();
        assert_eq!(
            path_of(&["open", "ledger"]),
            root.join("ledger/2026-10.md").display().to_string(),
            "the generated view first"
        );
        assert_eq!(path_of(&["open", "logbook"]), root.display().to_string());

        assert_eq!(
            env.at(T0, &["open", "case"]).status.code(),
            Some(1),
            "no active case"
        );
        env.at(T0, &["plan", "new", "--", "x"]);
        env.at(T0, &["plan", "start", "C-2026-001"]);
        let active = path_of(&["open", "case"]);
        assert!(active.ends_with("work/active/C-2026-001-x.md"), "{active}");
        assert_eq!(path_of(&["open", "C-2026-001"]), active);
        env.at(T0, &["decide", "--no-edit", "--", "d"]);
        assert!(path_of(&["open", "ADR-0001"]).ends_with("decisions/ADR-0001-d.md"));
        let v = json(&env.at(T0, &["open", "status", "--json"]));
        assert_eq!(v["what"], "status");
        assert!(v["editor"].is_null());
        for bad in ["bogus", "C-2026-009", "ADR-0009"] {
            assert_eq!(env.at(T0, &["open", bad]).status.code(), Some(1), "{bad}");
        }
    }

    #[test]
    fn editor_without_a_terminal() {
        let env = Env::new(Snapper::Missing);
        let root = env.init_logbook();
        // no omarchy-launch-editor on PATH: a user error that names the fix
        let out = env.at(T0, &["open", "status", "--editor", "--json"]);
        assert_eq!(out.status.code(), Some(1));
        assert!(
            json(&out)["error"]["message"]
                .as_str()
                .unwrap()
                .contains("$EDITOR")
        );

        let record = env.tmp.path().join("editor-args");
        env.stub(
            "omarchy-launch-editor",
            &format!(
                "for a in \"$@\"; do echo \"[$a]\"; done > '{}'",
                record.display()
            ),
        );
        let out = env.at(T0, &["open", "journal", "--editor", "--json"]);
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        let day = root.join("journal/2026/2026-10-01.md");
        assert_eq!(read(&record), format!("[{}]\n", day.display()));
        // --editor creates the day so the editor opens a valid file
        assert_eq!(
            read(&day),
            "---\ntype: journal\ndate: 2026-10-01\ncases: []\n---\n"
        );
        assert_eq!(json(&out)["editor"]["program"], "omarchy-launch-editor");
    }

    /// `omarchy-launch-editor` stays in the foreground while a terminal
    /// editor runs (WP-012, decision 1): the engine starts it detached,
    /// reports it launched at once and never kills it (WP-008).
    #[test]
    fn a_launcher_that_keeps_running_is_launched_and_left_alone() {
        let env = Env::new(Snapper::Missing);
        env.init_logbook();
        let pid_file = env.tmp.path().join("editor-pid");
        // PATH is the stub directory only: the host's sleep by its path
        let sleep = std::env::var("PATH")
            .unwrap_or_default()
            .split(':')
            .map(|d| std::path::Path::new(d).join("sleep"))
            .find(|p| p.is_file())
            .expect("sleep on the host");
        env.stub(
            "omarchy-launch-editor",
            &format!(
                "echo $$ > '{}'; '{}' 12",
                pid_file.display(),
                sleep.display()
            ),
        );
        let start = std::time::Instant::now();
        // output() waits for stdout/stderr to close: an inherited pipe
        // would hold it until the editor exits
        let out = env.at(T0, &["open", "status", "--editor", "--json"]);
        let took = start.elapsed();
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        assert!(took < std::time::Duration::from_secs(1), "took {took:?}");
        let v = json(&out);
        assert_eq!(v["editor"]["launched"], true);
        assert_eq!(v["editor"]["program"], "omarchy-launch-editor");

        let pid = read(&pid_file).trim().to_string();
        let stat = |pid: &str| std::fs::read_to_string(format!("/proc/{pid}/stat")).ok();
        // `pid (comm) state ppid pgrp …`: its own process group
        let fields: Vec<String> = stat(&pid)
            .expect("the launcher runs")
            .rsplit_once(')')
            .unwrap()
            .1
            .split_whitespace()
            .map(String::from)
            .collect();
        assert_eq!(fields[2], pid, "process group of its own");
        std::thread::sleep(std::time::Duration::from_millis(10_500));
        let state = stat(&pid).and_then(|s| {
            s.rsplit_once(')')
                .and_then(|(_, rest)| rest.split_whitespace().next().map(String::from))
        });
        assert!(
            state.as_deref().is_some_and(|s| s != "Z" && s != "X"),
            "the launcher survives 10 s: {state:?}"
        );
        let _ = std::process::Command::new("kill")
            .args(["--", &format!("-{pid}")])
            .status();
    }

    #[test]
    fn a_launcher_that_fails_at_once_is_an_error() {
        let env = Env::new(Snapper::Missing);
        env.init_logbook();
        env.stub("omarchy-launch-editor", "echo 'no display' >&2; exit 3");
        let out = env.at(T0, &["open", "status", "--editor", "--json"]);
        assert_eq!(out.status.code(), Some(1));
        let message = json(&out)["error"]["message"].as_str().unwrap().to_string();
        assert!(
            message.contains("omarchy-launch-editor exited with 3"),
            "{message}"
        );
    }
}
