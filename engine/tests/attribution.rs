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
use seldon::attribution::{Stamps, attribute, attribute_stamped};
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

    /// WP-164: a boot file outside the home directory is proven by an
    /// agent's command that writes its absolute path (with `sudo`).
    #[test]
    fn a_boot_file_is_proven_by_its_absolute_path() {
        const HOOKS: &str = "/etc/mkinitcpio.conf.d/omarchy_hooks.conf";
        for (command, proves) in [
            ("sudo tee /etc/mkinitcpio.conf.d/omarchy_hooks.conf", true),
            (
                "sudo sed -i 's/ plymouth//' /etc/mkinitcpio.conf.d/omarchy_hooks.conf",
                true,
            ),
            ("sudo cp /tmp/hooks.conf /etc/mkinitcpio.conf.d/", true),
            ("sudo rm -rf /etc/mkinitcpio.conf.d", true),
            ("cat /etc/mkinitcpio.conf.d/omarchy_hooks.conf", false),
            ("sudo tee /etc/mkinitcpio.conf.d/other.conf", false),
            ("sudo mkinitcpio -P", false),
        ] {
            let known = [hook(
                "2026-10-01T10:39:15+02:00",
                "agent:claude-code",
                Some("C-2026-004"),
                command,
            )];
            let want = if proves {
                agent(HOOKS, "agent:claude-code", Some("C-2026-004"))
            } else {
                system(HOOKS)
            };
            assert_eq!(
                run(
                    vec![config_change("2026-10-01T10:40:02+02:00", HOOKS)],
                    &known
                ),
                [want],
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
            ("omarchy-plugin-add io.github.example.tyme", true),
            ("omarchy-plugin-enable io.github.example.tyme", false),
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

    /// `omarchy plugin <verb> <id>` proves only the event kind of its verb;
    /// `add` also covers `plugin-enable` (WP-073).
    #[test]
    fn a_plugin_verb_proves_only_its_kind() {
        let id = "io.github.example.tyme";
        let kinds = [
            Kind::PluginAdd,
            Kind::PluginRemove,
            Kind::PluginEnable,
            Kind::PluginDisable,
            Kind::PluginUpdate,
        ];
        for (verb, proves) in [
            ("add", &[Kind::PluginAdd, Kind::PluginEnable][..]),
            ("remove", &[Kind::PluginRemove]),
            ("enable", &[Kind::PluginEnable]),
            ("disable", &[Kind::PluginDisable]),
            ("update", &[Kind::PluginUpdate]),
        ] {
            for command in [
                format!("omarchy plugin {verb} {id}"),
                format!("omarchy-plugin-{verb} {id}"),
            ] {
                let known = [hook(
                    "2026-10-01T16:09:00+02:00",
                    "agent:codex",
                    None,
                    &command,
                )];
                for kind in kinds {
                    let event =
                        Event::new(ts("2026-10-01T16:10:00+02:00"), Source::Plugins, kind, id);
                    let expected = if proves.contains(&kind) {
                        agent(id, "agent:codex", None)
                    } else {
                        system(id)
                    };
                    assert_eq!(run(vec![event], &known), [expected], "{command} → {kind}");
                }
            }
        }
    }

    /// An event stamped with the capture time happened after the
    /// collector's last check: its cause may start from 10 minutes before
    /// that check (WP-073).
    #[test]
    fn a_capture_time_event_reaches_back_to_the_last_check() {
        let id = "io.github.example.tyme";
        let now = "2026-10-01T10:30:00+02:00";
        let stamps = Stamps {
            now: Some(ts(now)),
            since: vec![(Source::Plugins, ts("2026-10-01T10:15:00+02:00"))],
        };
        let removal = |at: &str| Event::new(ts(at), Source::Plugins, Kind::PluginRemove, id);
        let remove = |at: &str| {
            hook(
                at,
                "agent:claude-code",
                Some("C-2026-001"),
                &format!("omarchy plugin remove {id}"),
            )
        };
        let stamped = |events: Vec<Event>, known: &[Event]| {
            let mut events = events;
            attribute_stamped(&mut events, known, Path::new(HOME), &stamps);
            events
                .into_iter()
                .map(|e| (e.subject, e.actor, e.case))
                .collect::<Vec<_>>()
        };
        let by_agent = agent(id, "agent:claude-code", Some("C-2026-001"));
        // 22 minutes before the capture, 7 before the last check: in
        let early = [remove("2026-10-01T10:08:00+02:00")];
        assert_eq!(
            stamped(vec![removal(now)], &early),
            std::slice::from_ref(&by_agent)
        );
        assert_eq!(
            run(vec![removal(now)], &early),
            [system(id)],
            "without the last check: 10 minutes before the capture time"
        );
        // exactly 10 minutes before the last check: in; one second more: out
        assert_eq!(
            stamped(vec![removal(now)], &[remove("2026-10-01T10:05:00+02:00")]),
            std::slice::from_ref(&by_agent)
        );
        assert_eq!(
            stamped(vec![removal(now)], &[remove("2026-10-01T10:04:59+02:00")]),
            [system(id)]
        );
        // after the capture: out
        assert_eq!(
            stamped(vec![removal(now)], &[remove("2026-10-01T10:30:01+02:00")]),
            [system(id)]
        );
        // an event timed by its change keeps its own window
        assert_eq!(
            stamped(vec![removal("2026-10-01T10:29:00+02:00")], &early),
            [system(id)]
        );
        // a recorded change of the same plugin after the command: the
        // command came before it and cannot have caused this newer one
        let between = |subject: &str| {
            Event::new(
                ts("2026-10-01T10:20:00+02:00"),
                Source::Plugins,
                Kind::PluginAdd,
                subject,
            )
        };
        let mut known = early.to_vec();
        known.push(between(id));
        assert_eq!(stamped(vec![removal(now)], &known), [system(id)]);
        let mut known = early.to_vec();
        known.push(between("io.github.example.other"));
        assert_eq!(
            stamped(vec![removal(now)], &known),
            std::slice::from_ref(&by_agent),
            "another plugin's event does not block"
        );
        // only the source whose collector checked then
        let config = Event::new(ts(now), Source::Config, Kind::ConfigRemove, "~/.bashrc");
        let rm = [hook(
            "2026-10-01T10:08:00+02:00",
            "agent:claude-code",
            None,
            "rm ~/.bashrc",
        )];
        assert_eq!(stamped(vec![config], &rm), [system("~/.bashrc")]);
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

    /// The reviewer's scenario: an agent command that only *reads* a config
    /// file never claims a later edit of it.
    #[test]
    fn a_read_is_no_proof() {
        for command in [
            "cat ~/.config/hypr/bindings.conf && sudo pacman -S zed",
            "grep -n bindd ~/.config/hypr/bindings.conf",
            "cp ~/.config/hypr/bindings.conf /tmp/backup.conf",
            "sed s/a/b/ ~/.config/hypr/bindings.conf",
            "vim ~/.config/hypr/bindings.conf",
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
                [system(BINDINGS)],
                "{command}"
            );
        }
    }

    #[test]
    fn writers_prove_what_they_write() {
        let event = |kind, path: &str| {
            Event::new(ts("2026-10-01T10:40:00+02:00"), Source::Config, kind, path)
        };
        let agent_ = |path: &str| agent(path, "agent:claude-code", Some("C-2026-004"));
        for (command, kind, path, proves) in [
            // a directory destination proves the files directly in it
            (
                "cp new.conf ~/.config/hypr/",
                Kind::ConfigAdd,
                "~/.config/hypr/new.conf",
                true,
            ),
            (
                "cp -t ~/.config/hypr a.conf",
                Kind::ConfigAdd,
                "~/.config/hypr/a.conf",
                true,
            ),
            (
                "cp new.conf ~/.config/hypr/",
                Kind::ConfigAdd,
                "~/.config/hypr/sub/new.conf",
                false,
            ),
            // a move removes its source
            (
                "mv ~/.config/hypr/old.conf /tmp/",
                Kind::ConfigRemove,
                "~/.config/hypr/old.conf",
                true,
            ),
            // a removal covers everything below it
            (
                "rm -rf ~/.config/hypr/scripts",
                Kind::ConfigRemove,
                "~/.config/hypr/scripts/a.sh",
                true,
            ),
            (
                "rm ~/.config/hypr/a.conf",
                Kind::ConfigRemove,
                "~/.config/hypr/b.conf",
                false,
            ),
            (
                "truncate -s 0 ~/.config/hypr/a.conf",
                Kind::ConfigChange,
                "~/.config/hypr/a.conf",
                true,
            ),
            // shells are opened
            (
                "bash -c 'sed -i s/a/b/ ~/.config/hypr/a.conf'",
                Kind::ConfigChange,
                "~/.config/hypr/a.conf",
                true,
            ),
            (
                "eval \"echo x >> ~/.bashrc\"",
                Kind::ConfigChange,
                "~/.bashrc",
                true,
            ),
        ] {
            let known = [hook(
                "2026-10-01T10:39:00+02:00",
                "agent:claude-code",
                Some("C-2026-004"),
                command,
            )];
            let expected = if proves { agent_(path) } else { system(path) };
            assert_eq!(
                run(vec![event(kind, path)], &known),
                [expected],
                "{command}"
            );
        }
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

/// Capture-time plugin events through `seldon capture` (WP-073): a
/// plugin disabled by an agent command 15 minutes before the next capture
/// (the plugin's default interval) is the agent's; a command from before
/// the previous capture's window is not.
mod capture_time {
    use super::*;

    const ID: &str = "io.github.example.tyme";

    struct Machine {
        env: Env,
        logbook: PathBuf,
    }

    impl Machine {
        fn new() -> Self {
            let env = Env::new(Snapper::Missing);
            let logbook = env.init_logbook();
            let list = env.tmp.path().join("plugins.json");
            common::write_executable(
                &env.tmp.path().join("omarchy-stub"),
                &format!(
                    "#!/bin/sh\n[ \"$2\" = list ] && exec /bin/cat '{}'; exit 1\n",
                    list.display()
                ),
            );
            let m = Machine { env, logbook };
            m.enabled(true);
            m
        }

        fn enabled(&self, enabled: bool) {
            let list = json!([{ "id": ID, "enabled": enabled, "firstParty": false }]);
            std::fs::write(self.env.tmp.path().join("plugins.json"), list.to_string()).unwrap();
        }

        fn run(&self, now: &str, args: &[&str]) -> Output {
            let tmp = self.env.tmp.path();
            let out = self
                .env
                .command(args)
                .env("SELDON_NOW", now)
                .env("SELDON_OMARCHY", tmp.join("omarchy-stub"))
                .env("SELDON_OMARCHY_PLUGINS_DIR", tmp.join("plugins"))
                .output()
                .unwrap();
            assert_eq!(out.status.code(), Some(0), "{args:?}: {}", stderr(&out));
            out
        }

        /// What `seldon hook` records for an agent's command at `now`.
        fn command(&self, now: &str, command: &str) {
            let meta = format!("command={command}");
            self.run(
                now,
                &[
                    "event",
                    "agent",
                    "command",
                    "--subject",
                    "omarchy",
                    "--actor",
                    "agent:claude-code",
                    "--meta",
                    &meta,
                ],
            );
        }

        fn capture(&self, now: &str) {
            self.capture_source(now, "plugins");
        }

        fn capture_source(&self, now: &str, source: &str) {
            let out = self.run(now, &["capture", "--source", source, "--json"]);
            assert_eq!(common::json(&out)["ok"], true, "{}", common::stdout(&out));
        }

        /// The last `kind` event of `subject`.
        fn last(&self, kind: &str, subject: &str) -> serde_json::Value {
            let ledger = common::ledger(&self.logbook);
            ledger
                .iter()
                .rev()
                .find(|e| e["kind"] == kind && e["subject"] == subject)
                .unwrap_or_else(|| panic!("{kind} {subject} in {ledger:?}"))
                .clone()
        }

        /// The actor of the last `kind` event of the plugin.
        fn actor(&self, kind: &str) -> String {
            self.last(kind, ID)["actor"].as_str().unwrap().to_string()
        }

        /// Asserts that the last `kind` event of `subject` is `system`'s,
        /// without a case, and unclaimed: open drift, or routine (a plugin
        /// toggle, ADR-0028), which `drift --all` lists.
        fn assert_drift(&self, now: &str, kind: &str, subject: &str) {
            let e = self.last(kind, subject);
            assert_eq!(e["actor"], ACTOR_SYSTEM, "{e}");
            assert!(e.get("case").is_none(), "{e}");
            let drift = common::json(&self.run(now, &["drift", "--all", "--json"]));
            let open: Vec<&str> = drift["drift"]
                .as_array()
                .unwrap()
                .iter()
                .map(|d| d["eventId"].as_str().unwrap())
                .collect();
            assert!(open.contains(&e["id"].as_str().unwrap()), "{drift}");
        }

        fn bashrc(&self, text: Option<&str>) {
            let path = self.env.home.join(".bashrc");
            match text {
                Some(t) => std::fs::write(path, t).unwrap(),
                None => std::fs::remove_file(path).unwrap(),
            }
        }
    }

    const BASHRC: &str = "~/.bashrc";

    #[test]
    fn a_plugin_change_found_15_minutes_later_is_the_agents() {
        let m = Machine::new();
        m.capture("2026-10-01T10:00:00+02:00"); // baseline
        m.command(
            "2026-10-01T10:05:00+02:00",
            &format!("omarchy plugin disable {ID}"),
        );
        m.enabled(false);
        m.capture("2026-10-01T10:20:00+02:00");
        assert_eq!(m.actor("plugin-disable"), "agent:claude-code");

        // a command 11 minutes before the previous check: it cannot have
        // caused a change that check did not see yet
        m.command(
            "2026-10-01T10:09:00+02:00",
            &format!("omarchy plugin enable {ID}"),
        );
        m.enabled(true);
        m.capture("2026-10-01T10:30:00+02:00");
        assert_eq!(m.actor("plugin-enable"), ACTOR_SYSTEM);
    }

    /// An agent's command that proved one change claims no later change of
    /// the same plugin inside the wider window: the agent disables, a
    /// person enables and disables again (review round 1, B1).
    #[test]
    fn a_proving_command_claims_no_later_plugin_change() {
        let m = Machine::new();
        m.capture("2026-10-01T10:00:00+02:00"); // baseline
        m.command(
            "2026-10-01T10:05:00+02:00",
            &format!("omarchy plugin disable {ID}"),
        );
        m.enabled(false);
        m.capture("2026-10-01T10:06:00+02:00");
        assert_eq!(m.actor("plugin-disable"), "agent:claude-code");
        m.enabled(true);
        m.capture("2026-10-01T10:10:00+02:00");
        m.assert_drift("2026-10-01T10:10:00+02:00", "plugin-enable", ID);
        // 10:05 lies in [10:10 − 10 min, 10:20], but before the enabling
        m.enabled(false);
        m.capture("2026-10-01T10:20:00+02:00");
        m.assert_drift("2026-10-01T10:20:00+02:00", "plugin-disable", ID);
    }

    /// A config removal found 15 minutes after the agent's `rm` is the
    /// agent's (review round 1, N1).
    #[test]
    fn a_config_removal_found_15_minutes_later_is_the_agents() {
        let m = Machine::new();
        m.bashrc(Some("alias ll='ls -l'\n"));
        m.capture_source("2026-10-01T10:00:00+02:00", "config"); // baseline
        m.command("2026-10-01T10:05:00+02:00", "rm ~/.bashrc");
        m.bashrc(None);
        m.capture_source("2026-10-01T10:20:00+02:00", "config");
        let e = m.last("config-remove", BASHRC);
        assert_eq!(e["ts"], "2026-10-01T10:20:00+02:00");
        assert_eq!(e["actor"], "agent:claude-code", "{e}");
    }

    /// The agent removes the file, a person puts it back and removes it
    /// again: the second removal is the person's (review round 1, B1).
    #[test]
    fn a_proving_command_claims_no_later_config_removal() {
        let m = Machine::new();
        m.bashrc(Some("alias ll='ls -l'\n"));
        m.capture_source("2026-10-01T10:00:00+02:00", "config"); // baseline
        m.command("2026-10-01T10:05:00+02:00", "rm ~/.bashrc");
        m.bashrc(None);
        m.capture_source("2026-10-01T10:06:00+02:00", "config");
        assert_eq!(
            m.last("config-remove", BASHRC)["actor"],
            "agent:claude-code"
        );
        m.bashrc(Some("alias ll='ls -la'\n"));
        m.capture_source("2026-10-01T10:10:00+02:00", "config");
        m.last("config-add", BASHRC); // whose: a follow-up (an `rm` proves an addition)
        // 10:05 lies in [10:10 − 10 min, 10:20], but before the addition
        m.bashrc(None);
        m.capture_source("2026-10-01T10:20:00+02:00", "config");
        m.assert_drift("2026-10-01T10:20:00+02:00", "config-remove", BASHRC);
    }
}

/// Package attribution reads a hook's command line with the hook's own
/// parser (WP-071): what the hook records as an agent's package command is
/// what the pacman collector attributes to the agent, and a command the
/// hook does not take for one (`yay --version`) claims nothing.
mod packages {
    use super::*;
    use seldon::attribution::{causes, find_cause};

    const T0: &str = "2026-10-01T10:00:00+02:00";
    const SINCE: &str = "2026-10-01T09:00:00+02:00";

    struct Bench {
        env: Env,
        logbook: PathBuf,
    }

    impl Bench {
        /// A logbook with C-2026-001 active.
        fn new() -> Self {
            let env = Env::new(Snapper::Missing);
            let logbook = env.tmp.path().join("logbook");
            let b = Bench { env, logbook };
            b.run(
                &[
                    "init",
                    "--non-interactive",
                    "--no-capture",
                    "--no-git",
                    "--path",
                    b.logbook.to_str().unwrap(),
                ],
                None,
            );
            b.run(&["plan", "new", "--zone", "red", "--", "Editors"], None);
            b.run(&["plan", "start", "C-2026-001"], None);
            b
        }

        fn run(&self, args: &[&str], stdin: Option<&str>) -> Output {
            let tmp = self.env.tmp.path();
            let mut child = self
                .env
                .command(args)
                .env("SELDON_NOW", T0)
                .env("SELDON_PACMAN_LOG", tmp.join("pacman.log"))
                .env("SELDON_PACMAN_DB_LOCK", tmp.join("no-db.lck"))
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

        /// `seldon hook generic` for an agent's command started at `at`.
        fn hook(&self, command: &str, at: &str) {
            let payload = json!({
                "command": command,
                "actor": "agent:codex",
                "cwd": self.logbook,
                "startedAt": at,
            });
            self.run(&["hook", "generic"], Some(&payload.to_string()));
        }

        /// Writes the package log and captures it; (subject, actor, case)
        /// of each package event.
        fn capture(&self, log: &str) -> Vec<(String, String, String)> {
            std::fs::write(self.env.tmp.path().join("pacman.log"), log).unwrap();
            self.run(&["capture", "--source", "pacman", "--since", SINCE], None);
            common::ledger(&self.logbook)
                .iter()
                .filter(|e| e["source"] == "pacman")
                .map(|e| {
                    (
                        e["subject"].as_str().unwrap().to_string(),
                        e["actor"].as_str().unwrap().to_string(),
                        e["case"].as_str().unwrap_or("-").to_string(),
                    )
                })
                .collect()
        }
    }

    /// One install transaction of `package` at 10:`minute`:30.
    fn install(minute: u32, package: &str) -> String {
        let at = |s: u32| format!("[2026-10-01T10:{minute:02}:{s:02}+0200]");
        format!(
            "{} [PACMAN] Running 'pacman -S {package}'\n\
             {} [ALPM] transaction started\n\
             {} [ALPM] installed {package} (1.0-1)\n\
             {} [ALPM] transaction completed\n",
            at(30),
            at(31),
            at(32),
            at(32)
        )
    }

    /// F-530, F-531: wrappers and option clusters the old intent split did
    /// not read; each install is the agent's, in its case.
    #[test]
    fn a_wrapped_install_is_the_agents() {
        let b = Bench::new();
        let lines = [
            "sudo -u root pacman -S zed1",
            "sudo -Eu root pacman -S zed2",
            "pkexec pacman -S zed3",
            "run0 pacman -S zed4",
            "timeout 600 yay -S zed5",
            "bash -c 'yay -S zed6'",
            "nice -n 10 pacman -S zed7",
        ];
        let mut log = String::new();
        for (n, line) in lines.iter().enumerate() {
            let minute = n as u32 + 1;
            b.hook(line, &format!("2026-10-01T10:{minute:02}:00+02:00"));
            log.push_str(&install(minute, &format!("zed{minute}")));
        }
        let got = b.capture(&log);
        let want: Vec<(String, String, String)> = (1..=lines.len())
            .map(|n| {
                (
                    format!("zed{n}"),
                    "agent:codex".to_string(),
                    "C-2026-001".to_string(),
                )
            })
            .collect();
        assert_eq!(got, want);
    }

    /// F-532: `yay --version` is no full upgrade, so a person's `-Syu`
    /// two minutes later stays theirs (drift), not the agent's.
    #[test]
    fn a_version_probe_claims_no_upgrade() {
        let b = Bench::new();
        b.hook("yay --version", "2026-10-01T10:01:00+02:00");
        assert!(
            common::ledger(&b.logbook)
                .iter()
                .all(|e| e["kind"] != "command"),
            "the hook records no command"
        );
        let got = b.capture(
            "[2026-10-01T10:03:00+0200] [PACMAN] Running 'pacman -Syu'\n\
             [2026-10-01T10:03:01+0200] [ALPM] transaction started\n\
             [2026-10-01T10:03:02+0200] [ALPM] upgraded zed (1.0-1 -> 1.1-1)\n\
             [2026-10-01T10:03:02+0200] [ALPM] transaction completed\n",
        );
        assert_eq!(
            got,
            [("zed".to_string(), ACTOR_SYSTEM.to_string(), "-".to_string())]
        );
    }

    /// The same from a ledger an older engine wrote: a recorded `yay
    /// --version` or `yay -h` is no cause for a full upgrade.
    #[test]
    fn a_recorded_probe_is_no_cause() {
        let began = ts("2026-10-01T10:03:00+02:00");
        for command in ["yay --version", "yay -h", "paru -V", "yay -G zed"] {
            let known = causes(&[hook(
                "2026-10-01T10:01:00+02:00",
                "agent:codex",
                Some("C-2026-001"),
                command,
            )]);
            assert!(
                find_cause(&known, "zed", began, true, true).is_none(),
                "{command}"
            );
        }
        let known = causes(&[hook(
            "2026-10-01T10:01:00+02:00",
            "agent:codex",
            None,
            "sudo -u root pacman -S zed",
        )]);
        assert!(find_cause(&known, "zed", began, true, false).is_some());
    }
}
