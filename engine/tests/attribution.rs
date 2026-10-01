//! The shared attribution pass (WP-009, ADR-0014 §1, ADR-0017 §2): config,
//! theme and plugins events take the actor and case of the agent command
//! that names them. In-process against the fixture ledger, and end to end
//! through `seldon hook claude-code` and `seldon capture`.
//!
//! The pacman/omarchy attribution tests (ADR-0017 scenarios) stay in
//! `collectors.rs`, unchanged.

mod common;

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Output, Stdio};

use chrono::DateTime;
use common::{Env, Snapper, stderr};
use seldon::attribution::attribute;
use seldon::model::event::{ACTOR_SYSTEM, Event, Kind, Source};
use serde_json::json;

const HOME: &str = "/home/user";

fn fixture_events() -> Vec<Event> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../fixtures/logbook/ledger");
    let mut out = Vec::new();
    for month in ["2026-09", "2026-10"] {
        let text = std::fs::read_to_string(dir.join(format!("{month}.jsonl"))).unwrap();
        out.extend(
            text.lines()
                .map(|l| serde_json::from_str::<Event>(l).unwrap()),
        );
    }
    out
}

fn ts(s: &str) -> DateTime<chrono::FixedOffset> {
    DateTime::parse_from_rfc3339(s).unwrap()
}

/// A hook command event.
fn hook(at: &str, actor: &str, case: Option<&str>, command: &str) -> Event {
    let mut e = Event::new(ts(at), Source::Agent, Kind::Command, "x")
        .actor(actor)
        .case(case.map(String::from));
    e.meta.command = Some(command.into());
    e
}

fn config_change(at: &str, path: &str) -> Event {
    Event::new(ts(at), Source::Config, Kind::ConfigChange, path)
}

/// (subject, actor, case) after the pass.
fn run(mut events: Vec<Event>, known: &[Event]) -> Vec<(String, String, Option<String>)> {
    attribute(&mut events, known, Path::new(HOME));
    events
        .into_iter()
        .map(|e| (e.subject, e.actor, e.case))
        .collect()
}

fn system(subject: &str) -> (String, String, Option<String>) {
    (subject.into(), ACTOR_SYSTEM.into(), None)
}

fn agent(subject: &str, actor: &str, case: Option<&str>) -> (String, String, Option<String>) {
    (subject.into(), actor.into(), case.map(String::from))
}

mod fixture {
    use super::*;

    /// Every config, theme and plugins event of the fixture ledger that an
    /// agent caused is reproduced from `actor: system`: the 10-01
    /// `bindings.conf` change (`sed -i`, C-2026-004) and Codex's
    /// `ollama.service` (`tee`, no case). The others stay as they are.
    #[test]
    fn reproduces_the_fixture_ledger() {
        let all = fixture_events();
        let expected: Vec<Event> = all
            .iter()
            .filter(|e| matches!(e.source, Source::Config | Source::Theme | Source::Plugins))
            .cloned()
            .collect();
        let mut reset = expected.clone();
        let mut n = 0;
        for e in &mut reset {
            if e.actor.starts_with("agent:") {
                e.actor = ACTOR_SYSTEM.into();
                e.case = None;
                n += 1;
            }
        }
        assert_eq!(n, 2, "bindings.conf and ollama.service");
        attribute(&mut reset, &all, Path::new(HOME));
        assert_eq!(reset, expected);
        let bindings = reset
            .iter()
            .find(|e| e.subject == "~/.config/hypr/bindings.conf")
            .unwrap();
        assert_eq!(bindings.actor, "agent:claude-code");
        assert_eq!(bindings.case.as_deref(), Some("C-2026-004"));
    }
}

mod rules {
    use super::*;

    const BINDINGS: &str = "~/.config/hypr/bindings.conf";

    #[test]
    fn config_needs_the_path_in_the_command() {
        let known = [
            hook(
                "2026-10-01T10:30:00+02:00",
                "agent:codex",
                None,
                "sed -i s/a/b/ ~/.config/hypr/monitors.conf",
            ),
            hook(
                "2026-10-01T10:31:00+02:00",
                "agent:codex",
                None,
                "hyprctl reload",
            ),
        ];
        assert_eq!(
            run(
                vec![config_change("2026-10-01T10:32:00+02:00", BINDINGS)],
                &known
            ),
            [system(BINDINGS)],
            "proximity alone is no proof"
        );
    }

    #[test]
    fn config_path_spellings() {
        for command in [
            "sed -i s/a/b/ ~/.config/hypr/bindings.conf",
            "sed -i s/a/b/ \"$HOME/.config/hypr/bindings.conf\"",
            "cp /tmp/b ${HOME}/.config/hypr/bindings.conf",
            "sudo tee /home/user/.config/hypr/bindings.conf",
            "sed -i 's/x/y/' .config/hypr/bindings.conf",
            "echo bind >> ~/.config/hypr/bindings.conf",
            "Edit ~/.config/hypr/bindings.conf",
            "cd /tmp && cp x ~/.config/hypr/./bindings.conf",
        ] {
            let known = [hook(
                "2026-10-01T10:39:15+02:00",
                "agent:claude-code",
                Some("C-2026-004"),
                command,
            )];
            assert_eq!(
                run(
                    vec![config_change("2026-10-01T10:40:02+02:00", BINDINGS)],
                    &known
                ),
                [agent(BINDINGS, "agent:claude-code", Some("C-2026-004"))],
                "{command}"
            );
        }
    }

    #[test]
    fn the_window_is_ten_minutes_after_the_start() {
        let sed = |at: &str| {
            hook(
                at,
                "agent:claude-code",
                Some("C-2026-004"),
                "sed -i s/a/b/ ~/.config/hypr/bindings.conf",
            )
        };
        let change = || vec![config_change("2026-10-01T10:40:00+02:00", BINDINGS)];
        // exactly 10 minutes: in
        assert_eq!(
            run(change(), &[sed("2026-10-01T10:30:00+02:00")]),
            [agent(BINDINGS, "agent:claude-code", Some("C-2026-004"))]
        );
        // 10 minutes and 1 second: out
        assert_eq!(
            run(change(), &[sed("2026-10-01T10:29:59+02:00")]),
            [system(BINDINGS)]
        );
        // after the change: out
        assert_eq!(
            run(change(), &[sed("2026-10-01T10:40:01+02:00")]),
            [system(BINDINGS)]
        );
        // the latest proving command wins
        let known = [
            sed("2026-10-01T10:31:00+02:00"),
            hook(
                "2026-10-01T10:35:00+02:00",
                "agent:codex",
                None,
                "tee ~/.config/hypr/bindings.conf",
            ),
        ];
        assert_eq!(
            run(change(), &known),
            [agent(BINDINGS, "agent:codex", None)]
        );
    }

    #[test]
    fn theme_needs_the_slug() {
        let theme = |slug: &str| {
            Event::new(
                ts("2026-10-01T15:30:00+02:00"),
                Source::Theme,
                Kind::ThemeSet,
                slug,
            )
        };
        for (command, proves) in [
            ("omarchy theme set tokyo-night", true),
            ("omarchy theme set 'Tokyo Night'", true),
            ("omarchy-theme-set \"Tokyo Night\"", true),
            ("omarchy theme set kanagawa", false),
            ("omarchy theme list", false),
            ("echo tokyo-night", false),
        ] {
            let known = [hook(
                "2026-10-01T15:29:40+02:00",
                "agent:claude-code",
                None,
                command,
            )];
            let expected = if proves {
                agent("tokyo-night", "agent:claude-code", None)
            } else {
                system("tokyo-night")
            };
            assert_eq!(
                run(vec![theme("tokyo-night")], &known),
                [expected],
                "{command}"
            );
        }
    }

    #[test]
    fn plugins_need_the_id() {
        let id = "io.github.example.tyme";
        let add = || {
            vec![Event::new(
                ts("2026-10-01T16:10:00+02:00"),
                Source::Plugins,
                Kind::PluginAdd,
                id,
            )]
        };
        for (command, proves) in [
            ("omarchy plugin add io.github.example.tyme", true),
            (
                "omarchy plugin add https://github.com/example/io.github.example.tyme.git",
                true,
            ),
            ("omarchy-plugin-enable io.github.example.tyme", true),
            ("omarchy plugin list --json", false),
            ("omarchy plugin add https://github.com/example/tyme", false),
            (
                "git clone https://github.com/example/io.github.example.tyme",
                false,
            ),
        ] {
            let known = [hook(
                "2026-10-01T16:09:00+02:00",
                "agent:codex",
                None,
                command,
            )];
            let expected = if proves {
                agent(id, "agent:codex", None)
            } else {
                system(id)
            };
            assert_eq!(run(add(), &known), [expected], "{command}");
        }
    }

    #[test]
    fn other_events_are_left_alone() {
        let known = [hook(
            "2026-10-01T10:00:00+02:00",
            "agent:claude-code",
            Some("C-2026-004"),
            "yay -S zed && sed -i s/a/b/ ~/.config/hypr/bindings.conf",
        )];
        // pacman attributes in its collector; the pass does not touch it
        let pkg = Event::new(
            ts("2026-10-01T10:01:00+02:00"),
            Source::Pacman,
            Kind::Install,
            "zed",
        );
        // already attributed (another case): kept
        let mut linked = config_change("2026-10-01T10:02:00+02:00", BINDINGS);
        linked.case = Some("C-2026-003".into());
        // a `system` command never attributes
        let system_cmd = [hook(
            "2026-10-01T10:00:00+02:00",
            ACTOR_SYSTEM,
            None,
            "sed -i s/a/b/ ~/.config/hypr/bindings.conf",
        )];
        assert_eq!(
            run(vec![pkg, linked], &known),
            [
                system("zed"),
                (
                    BINDINGS.into(),
                    ACTOR_SYSTEM.into(),
                    Some("C-2026-003".into())
                )
            ]
        );
        assert_eq!(
            run(
                vec![config_change("2026-10-01T10:02:00+02:00", BINDINGS)],
                &system_cmd
            ),
            [system(BINDINGS)]
        );
    }
}

/// End to end: the hook writes the command event, the change happens, the
/// next `capture` attributes it. Real clock (capture has no SELDON_NOW).
mod end_to_end {
    use super::*;

    struct Machine {
        env: Env,
        logbook: PathBuf,
    }

    impl Machine {
        fn new() -> Self {
            let env = Env::new(Snapper::NoPermissions);
            let logbook = env.init_logbook();
            let tmp = env.tmp.path();
            std::fs::write(tmp.join("pacman.log"), "").unwrap();
            std::fs::write(tmp.join("theme.name"), "kanagawa\n").unwrap();
            let hypr = env.home.join(".config/hypr");
            std::fs::create_dir_all(&hypr).unwrap();
            std::fs::write(
                hypr.join("bindings.conf"),
                "bindd = SUPER, E, Editor, exec, nvim\n",
            )
            .unwrap();
            std::fs::write(
                hypr.join("monitors.conf"),
                "monitor = , preferred, auto, 1\n",
            )
            .unwrap();
            Machine { env, logbook }
        }

        fn run(&self, args: &[&str], stdin: Option<&str>) -> Output {
            let tmp = self.env.tmp.path();
            let mut child = self
                .env
                .command(args)
                .env("SELDON_PACMAN_LOG", tmp.join("pacman.log"))
                .env("SELDON_PACMAN_DB_LOCK", tmp.join("no-db.lck"))
                .env("SELDON_OMARCHY", tmp.join("no-omarchy"))
                .env("SELDON_OMARCHY_PLUGINS_DIR", tmp.join("plugins"))
                .env("SELDON_THEME_FILE", tmp.join("theme.name"))
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap();
            child
                .stdin
                .take()
                .unwrap()
                .write_all(stdin.unwrap_or("").as_bytes())
                .unwrap();
            let out = child.wait_with_output().unwrap();
            assert_eq!(out.status.code(), Some(0), "{args:?}: {}", stderr(&out));
            out
        }

        fn bash(&self, command: &str) {
            let payload = json!({
                "session_id": "s-1",
                "cwd": self.logbook,
                "hook_event_name": "PreToolUse",
                "tool_name": "Bash",
                "tool_use_id": format!("toolu_{}", command.len()),
                "tool_input": {"command": command},
            });
            self.run(&["hook", "claude-code"], Some(&payload.to_string()));
        }

        fn ledger(&self) -> Vec<serde_json::Value> {
            common::ledger(&self.logbook)
        }
    }

    #[test]
    fn a_hooked_sed_attributes_the_config_change() {
        let b = Machine::new();
        let case = {
            let out = b.run(&["plan", "new", "Zed als Editor", "--json"], None);
            let id = common::json(&out)["event"]["subject"]
                .as_str()
                .unwrap()
                .to_string();
            b.run(&["plan", "start", &id], None);
            id
        };
        b.run(&["capture", "--all"], None); // baselines
        b.bash("sed -i 's/nvim/zeditor/' ~/.config/hypr/bindings.conf");
        std::fs::write(
            b.env.home.join(".config/hypr/bindings.conf"),
            "bindd = SUPER, E, Editor, exec, zeditor\n",
        )
        .unwrap();
        // a change no command named: system
        std::fs::write(
            b.env.home.join(".config/hypr/monitors.conf"),
            "monitor = , preferred, auto, 1.25\n",
        )
        .unwrap();
        b.bash("omarchy theme set 'Tokyo Night'");
        std::fs::write(b.env.tmp.path().join("theme.name"), "tokyo-night\n").unwrap();
        b.run(&["capture", "--all"], None);

        let ledger = b.ledger();
        let find = |subject: &str| {
            ledger
                .iter()
                .find(|e| e["subject"] == subject && e["source"] != "agent")
                .unwrap_or_else(|| panic!("{subject} in {ledger:?}"))
                .clone()
        };
        let bindings = find("~/.config/hypr/bindings.conf");
        assert_eq!(bindings["kind"], "config-change");
        assert_eq!(bindings["actor"], "agent:claude-code");
        assert_eq!(bindings["case"], case.as_str());
        let monitors = find("~/.config/hypr/monitors.conf");
        assert_eq!(monitors["actor"], "system");
        assert!(monitors.get("case").is_none());
        let theme = find("tokyo-night");
        assert_eq!(theme["kind"], "theme-set");
        assert_eq!(theme["actor"], "agent:claude-code");
        assert_eq!(theme["case"], case.as_str());
    }
}
