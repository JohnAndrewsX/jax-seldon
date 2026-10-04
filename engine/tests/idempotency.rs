//! Running any collector twice produces no new events (AGENTS.md §7,
//! SPEC-ENGINE §10), in-process and through `seldon capture`.

mod common;
mod support;

use std::path::Path;
use std::process::Output;

use seldon::collectors::config::ConfigFiles;
use seldon::collectors::omarchy::Omarchy;
use seldon::collectors::pacman::Pacman;
use seldon::collectors::plugins::Plugins;
use seldon::collectors::snapper::Snapper;
use seldon::collectors::{Collector, REGISTRY};
use seldon::model::event::{Event, Kind, Source};
use support::{FIXTURE_CREATED, assert_schema_valid, fixture, story};

mod idempotency {
    use super::*;

    #[test]
    fn every_collector_twice_writes_nothing() {
        let mut b = story();
        let before = b.ledger.read_all().unwrap().len();
        for c in REGISTRY {
            let out = b.run(c, "2026-10-01T17:10:00+02:00");
            assert!(out.events.is_empty(), "{}: {:?}", c.name(), out.events);
        }
        // and a third time, with nothing changed in between
        for c in [&Snapper as &dyn Collector, &Pacman, &Omarchy] {
            assert!(b.run(c, "2026-10-01T17:20:00+02:00").events.is_empty());
        }
        assert_eq!(b.ledger.read_all().unwrap().len(), before);
    }

    #[test]
    fn lost_cursors_do_not_duplicate() {
        // cursors.json deleted (or written before a crash): pacman re-reads
        // from the baseline and drops what the ledger has; snapper drops
        // snapshots it already recorded
        let mut b = story();
        b.cursors.clear();
        assert!(
            b.run(&Pacman, "2026-10-01T17:10:00+02:00")
                .events
                .is_empty()
        );
        assert!(
            b.run(&Snapper, "2026-10-01T17:10:00+02:00")
                .events
                .is_empty()
        );
        assert!(
            b.run(&Omarchy, "2026-10-01T17:10:00+02:00")
                .events
                .is_empty()
        );
    }

    #[test]
    fn a_crash_before_the_cursor_save_does_not_duplicate() {
        // the ledger has the capture-time events, cursors.json still has
        // the state from before (crash between append and save)
        let mut b = support::Bench::new("crash");
        b.sources.snapper = b
            .scratch
            .stub_cat("snapper", &fixture("logs/snapper-before.json"));
        b.sources.omarchy_version = b.scratch.stub("omarchy-version", "echo 4.0.6-1");
        b.run(&Snapper, "2026-09-30T18:00:00+02:00");
        b.run(&Omarchy, "2026-09-30T18:00:00+02:00");
        let before = b.cursors.clone();

        b.sources.snapper = b.scratch.stub_cat("snapper", &fixture("logs/snapper.json"));
        b.sources.omarchy_version = b.scratch.stub("omarchy-version", "echo 4.0.7-1");
        let deletes = b.run(&Snapper, "2026-10-01T17:05:00+02:00");
        assert_eq!(
            deletes
                .events
                .iter()
                .filter(|e| e.kind == Kind::SnapshotDelete)
                .count(),
            2
        );
        assert_eq!(b.run(&Omarchy, "2026-10-01T17:05:00+02:00").events.len(), 1);

        b.cursors = before;
        assert!(
            b.run(&Snapper, "2026-10-01T17:06:00+02:00")
                .events
                .is_empty()
        );
        assert!(
            b.run(&Omarchy, "2026-10-01T17:06:00+02:00")
                .events
                .is_empty()
        );

        // a real re-upgrade after a downgrade is still recorded
        b.sources.omarchy_version = b.scratch.stub("omarchy-version", "echo 4.0.6-1");
        assert_eq!(b.run(&Omarchy, "2026-10-02T10:00:00+02:00").events.len(), 1);
        b.sources.omarchy_version = b.scratch.stub("omarchy-version", "echo 4.0.7-1");
        assert_eq!(b.run(&Omarchy, "2026-10-03T10:00:00+02:00").events.len(), 1);
    }

    #[test]
    fn a_failed_cursor_save_does_not_repeat_config_events() {
        // the config collector diffs against the generation its cursor
        // names; after a crash between the ledger write and the cursor
        // save that is the generation before the events (WP-069)
        let mut b = support::Bench::new("crash-config");
        let home = b.dirs.home.clone();
        let file = |rel: &str, text: &str| {
            let path = home.join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        };
        file(".config/hypr/monitors.conf", "monitor=,preferred,auto,1\n");
        file(".config/hypr/old.conf", "x\n");
        assert!(
            b.run(&ConfigFiles, "2026-10-01T10:00:00+02:00")
                .events
                .is_empty()
        );
        let before = b.cursors.clone();

        file(
            ".config/hypr/monitors.conf",
            "monitor=,preferred,auto,1.25\n",
        );
        file(".config/hypr/new.conf", "y\n");
        std::fs::remove_file(home.join(".config/hypr/old.conf")).unwrap();
        let written = b.run(&ConfigFiles, "2026-10-01T10:10:00+02:00");
        assert_eq!(written.events.len(), 3, "{:?}", written.events);

        b.cursors = before;
        let again = b.run(&ConfigFiles, "2026-10-01T10:20:00+02:00");
        assert!(again.ok, "{:?}", again.message);
        assert!(again.events.is_empty(), "{:?}", again.events);
        assert_eq!(b.ledger_events(Source::Config).len(), 3);

        // the next real change is recorded, also one back to an old state
        file(".config/hypr/monitors.conf", "monitor=,preferred,auto,1\n");
        let back = b.run(&ConfigFiles, "2026-10-01T10:30:00+02:00");
        assert_eq!(back.events.len(), 1);
        assert_eq!(back.events[0].kind, Kind::ConfigChange);
        file(
            ".config/hypr/monitors.conf",
            "monitor=,preferred,auto,1.25\n",
        );
        assert_eq!(
            b.run(&ConfigFiles, "2026-10-01T10:40:00+02:00")
                .events
                .len(),
            1,
            "the same step as at 10:10, after the cursor moved on"
        );
    }

    /// A bench whose `omarchy plugin list --json` prints `<scratch>/plugins.json`
    /// and whose plugin manifests live in `<scratch>/plugins`.
    fn plugin_bench(tag: &str) -> support::Bench {
        let mut b = support::Bench::new(tag);
        let list = b.scratch.path("plugins.json");
        b.sources.omarchy = b.scratch.stub(
            "omarchy",
            &format!("[ \"$2\" = list ] && exec cat '{}'; exit 1", list.display()),
        );
        b.sources.plugins_dir = Some(b.scratch.path("plugins"));
        b
    }

    /// `plugins.json` and the manifests: `(id, enabled, version)`.
    fn plugins(b: &support::Bench, list: &[(&str, bool, &str)]) {
        let entries: Vec<serde_json::Value> = list
            .iter()
            .map(|(id, enabled, version)| {
                let dir = b.scratch.path("plugins").join(id);
                std::fs::create_dir_all(&dir).unwrap();
                std::fs::write(
                    dir.join("manifest.json"),
                    serde_json::json!({ "id": id, "version": version }).to_string(),
                )
                .unwrap();
                serde_json::json!({ "id": id, "enabled": enabled, "firstParty": false })
            })
            .collect();
        std::fs::write(
            b.scratch.path("plugins.json"),
            serde_json::Value::from(entries).to_string(),
        )
        .unwrap();
    }

    #[test]
    fn a_failed_cursor_save_does_not_repeat_plugin_events() {
        // the plugins collector diffs against the snapshot in its cursor;
        // after a crash between the ledger write and the cursor save that
        // is the snapshot before the events (WP-073)
        let mut b = plugin_bench("crash-plugins");
        plugins(
            &b,
            &[
                ("io.example.a", true, "1.0.0"),
                ("io.example.b", false, "1.0.0"),
                ("io.example.d", true, "1.0.0"),
            ],
        );
        assert!(
            b.run(&Plugins, "2026-10-01T10:00:00+02:00")
                .events
                .is_empty()
        );
        let before = b.cursors.clone();

        std::fs::remove_dir_all(b.scratch.path("plugins/io.example.d")).unwrap();
        plugins(
            &b,
            &[
                ("io.example.a", true, "1.1.0"),
                ("io.example.b", true, "1.0.0"),
                ("io.example.c", false, "0.1.0"),
            ],
        );
        let written = b.run(&Plugins, "2026-10-01T10:10:00+02:00");
        let kinds = |out: &seldon::collectors::Outcome| -> Vec<(Kind, String)> {
            out.events
                .iter()
                .map(|e| (e.kind, e.subject.clone()))
                .collect()
        };
        assert_eq!(
            kinds(&written),
            [
                (Kind::PluginUpdate, "io.example.a".into()),
                (Kind::PluginEnable, "io.example.b".into()),
                (Kind::PluginAdd, "io.example.c".into()),
                (Kind::PluginRemove, "io.example.d".into()),
            ]
        );

        b.cursors = before;
        let again = b.run(&Plugins, "2026-10-01T10:20:00+02:00");
        assert!(again.ok, "{:?}", again.message);
        assert!(again.events.is_empty(), "{:?}", again.events);
        assert_eq!(b.ledger_events(Source::Plugins).len(), 4);
        assert!(
            b.run(&Plugins, "2026-10-01T10:30:00+02:00")
                .events
                .is_empty()
        );

        // the next real change is recorded, also one back to an old state
        plugins(
            &b,
            &[
                ("io.example.a", true, "1.1.0"),
                ("io.example.b", false, "1.0.0"),
                ("io.example.c", false, "0.1.0"),
            ],
        );
        let back = b.run(&Plugins, "2026-10-01T10:40:00+02:00");
        assert_eq!(kinds(&back), [(Kind::PluginDisable, "io.example.b".into())]);
        plugins(
            &b,
            &[
                ("io.example.a", true, "1.1.0"),
                ("io.example.b", true, "1.0.0"),
                ("io.example.c", false, "0.1.0"),
            ],
        );
        assert_eq!(
            kinds(&b.run(&Plugins, "2026-10-01T10:50:00+02:00")),
            [(Kind::PluginEnable, "io.example.b".into())],
            "the same step as at 10:10, after the cursor moved on"
        );
        assert_eq!(b.ledger_events(Source::Plugins).len(), 6);
    }

    #[test]
    fn a_file_back_to_its_content_after_a_failed_cursor_save_is_recorded() {
        // A→B written, the cursor save failed, the file went back to A
        // before the next capture: the ledger gets B→A (WP-073)
        let mut b = support::Bench::new("crash-config-back");
        let path = b.dirs.home.join(".config/hypr/monitors.conf");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "monitor=,preferred,auto,1\n").unwrap();
        assert!(
            b.run(&ConfigFiles, "2026-10-01T10:00:00+02:00")
                .events
                .is_empty()
        );
        let before = b.cursors.clone();

        std::fs::write(&path, "monitor=,preferred,auto,1.25\n").unwrap();
        let there = b.run(&ConfigFiles, "2026-10-01T10:10:00+02:00");
        assert_eq!(there.events.len(), 1, "{:?}", there.events);

        b.cursors = before;
        std::fs::write(&path, "monitor=,preferred,auto,1\n").unwrap();
        let back = b.run(&ConfigFiles, "2026-10-01T10:20:00+02:00");
        assert!(back.ok, "{:?}", back.message);
        assert_eq!(back.events.len(), 1, "{:?}", back.events);
        let (a, e) = (&there.events[0], &back.events[0]);
        assert_eq!(e.kind, Kind::ConfigChange);
        assert_eq!(e.subject, "~/.config/hypr/monitors.conf");
        assert_eq!(
            (&e.meta.hash_from, &e.meta.hash_to),
            (&a.meta.hash_to, &a.meta.hash_from),
            "B→A closes A→B"
        );
        assert!(
            b.run(&ConfigFiles, "2026-10-01T10:30:00+02:00")
                .events
                .is_empty()
        );
        assert_eq!(b.ledger_events(Source::Config).len(), 2);
    }

    #[test]
    fn capture_writes_the_ledger_once() {
        let cli = Cli::new();
        let first = cli.capture(&["--since", FIXTURE_CREATED]);
        assert_eq!(
            first["written"], 15,
            "3 snapshots + 12 package events: {first}"
        );
        assert_eq!(first["ok"], true);
        assert_eq!(cli.capture(&[])["written"], 0);
        // --since is ignored, and said to be, for collectors with a cursor
        let again = cli.capture(&["--since", FIXTURE_CREATED]);
        assert_eq!(again["written"], 0);
        assert_eq!(
            again["sinceIgnored"],
            serde_json::json!(["snapper", "pacman", "omarchy", "plugins", "theme", "config"])
        );
        // another spelling of the same logbook keeps its cursors
        let detour = cli.logbook.join("../logbook");
        let out = cli.run(&[
            "capture",
            "--json",
            "--since",
            FIXTURE_CREATED,
            "--logbook",
            detour.to_str().unwrap(),
        ]);
        let out = common::json(&out);
        assert_eq!(out["written"], 0);
        assert_eq!(
            out["sinceIgnored"],
            serde_json::json!(["snapper", "pacman", "omarchy", "plugins", "theme", "config"]),
            "the cursors were not dropped as another logbook's"
        );
        let human = cli.run(&["capture", "--since", FIXTURE_CREATED]);
        assert!(common::stdout(&human).contains(
            "note: --since ignored for snapper, pacman, omarchy, plugins, theme, config"
        ));

        // later: snapper sees the newer list, Omarchy was updated
        cli.stub_snapper(&fixture("logs/snapper.json"));
        cli.stub_version("4.0.7-1");
        let third = cli.capture(&[]);
        assert_eq!(
            third["written"], 8,
            "+111 +112 +113 +114 +115 −108 −109, one update: {third}"
        );
        assert_eq!(cli.capture(&[])["written"], 0);

        // cursors.json lost after the capture that wrote the update and the
        // deletions: nothing is written again, even from the old baseline
        let cursors = cli.env.home.join(".local/state/seldon/cursors.json");
        std::fs::remove_file(&cursors).unwrap();
        assert_eq!(cli.capture(&["--since", FIXTURE_CREATED])["written"], 0);

        let events = cli.ledger();
        events.iter().for_each(assert_schema_valid);
        let count = |s: Source| events.iter().filter(|e| e.source == s).count();
        assert_eq!(
            (
                count(Source::Pacman),
                count(Source::Snapper),
                count(Source::Omarchy)
            ),
            (12, 10, 1)
        );
        // state stays in the fake home
        assert!(
            cli.env
                .home
                .join(".local/state/seldon/cursors.json")
                .is_file()
        );
    }

    #[test]
    fn capture_reports_a_degraded_collector_and_exits_0() {
        let cli = Cli::new();
        cli.stub_snapper_no_permissions();
        let out = cli.capture(&["--since", FIXTURE_CREATED]);
        assert_eq!(out["ok"], false);
        let snapper = &out["collectors"][0];
        assert_eq!(snapper["name"], "snapper");
        assert_eq!(snapper["ok"], false);
        assert_eq!(snapper["fix"], seldon::commands::doctor::SNAPPER_FIX);
        assert_eq!(out["written"], 12, "the other collectors still run");
    }

    #[test]
    fn capture_source_selection_and_errors() {
        let cli = Cli::new();
        let out = cli.run(&["capture", "--source", "omarchy,snapper", "--json"]);
        assert_eq!(out.status.code(), Some(0), "{}", common::stderr(&out));
        let out = common::json(&out);
        let ran: Vec<_> = out["collectors"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|c| c["ran"] == true)
            .map(|c| c["name"].as_str().unwrap().to_string())
            .collect();
        assert_eq!(ran, ["snapper", "omarchy"], "registry order");
        let bad = cli.run(&["capture", "--source", "nope", "--json"]);
        assert_eq!(bad.status.code(), Some(1));
        let bad = cli.run(&["capture", "--source", "pacman", "--all", "--json"]);
        assert_eq!(bad.status.code(), Some(1));
        let bad = cli.run(&["capture", "--since", "yesterday", "--json"]);
        assert_eq!(bad.status.code(), Some(1));
        let other = common::Env::new(common::Snapper::Missing);
        let out = other.seldon(&["capture", "--json"]);
        assert_eq!(out.status.code(), Some(3), "logbook not initialised");
    }
}

/// `seldon` in a fake home, a fresh logbook, the collectors pointed at
/// fixture copies through the `SELDON_*` variables, TZ of the fixtures.
struct Cli {
    env: common::Env,
    logbook: std::path::PathBuf,
    log: std::path::PathBuf,
}

impl Cli {
    fn new() -> Self {
        let env = common::Env::new(common::Snapper::Missing);
        let logbook = env.tmp.path().join("logbook");
        // no first capture: the test's own first capture is the baseline
        let out = env.seldon(&[
            "init",
            "--non-interactive",
            "--no-git",
            "--no-capture",
            "--path",
            logbook.to_str().unwrap(),
        ]);
        assert_eq!(out.status.code(), Some(0), "{}", common::stderr(&out));
        // what the agent hook (WP-009) would have written: the fixture's
        // agent command lines
        let mut lines = String::new();
        for month in ["2026-09", "2026-10"] {
            let text =
                std::fs::read_to_string(fixture(&format!("logbook/ledger/{month}.jsonl"))).unwrap();
            for l in text.lines().filter(|l| l.contains("\"source\":\"agent\"")) {
                lines.push_str(l);
                lines.push('\n');
            }
        }
        std::fs::write(logbook.join("ledger/2026-10.jsonl"), lines).unwrap();
        let log = env.tmp.path().join("pkg.log");
        std::fs::copy(fixture("logs/pacman.log"), &log).unwrap();
        let cli = Cli { env, logbook, log };
        cli.stub_snapper(&fixture("logs/snapper-before.json"));
        cli.stub_version("4.0.5-1");
        // the user-space collectors (WP-005) on fixed fixture inputs: they
        // record their first state silently and then see no change
        cli.stub(
            "omarchy",
            &format!(
                "[ \"$2\" = list ] && exec /bin/cat '{}'; exit 1",
                fixture("logs/plugin-list-before.json").display()
            ),
        );
        std::fs::write(cli.env.tmp.path().join("theme.name"), "osaka-jade\n").unwrap();
        cli
    }

    fn stub(&self, name: &str, body: &str) -> String {
        use std::os::unix::fs::PermissionsExt as _;
        let path = self.env.tmp.path().join(format!("stub-{name}"));
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path.to_string_lossy().into_owned()
    }

    fn stub_snapper(&self, list: &Path) {
        self.stub("snapper", &format!("exec /bin/cat '{}'", list.display()));
    }

    fn stub_snapper_no_permissions(&self) {
        self.stub("snapper", "echo 'No permissions.' >&2; exit 1");
    }

    fn stub_version(&self, version: &str) {
        self.stub("omarchy-version", &format!("echo {version}"));
    }

    fn run(&self, args: &[&str]) -> Output {
        let stub = |n: &str| self.env.tmp.path().join(format!("stub-{n}"));
        self.env
            .command(args)
            .env("SELDON_LOGBOOK", &self.logbook)
            .env("SELDON_PACMAN_LOG", &self.log)
            .env(
                "SELDON_PACMAN_DB_LOCK",
                self.env.tmp.path().join("no-db.lck"),
            )
            .env("SELDON_SNAPPER", stub("snapper"))
            .env("SELDON_OMARCHY_VERSION", stub("omarchy-version"))
            .env("SELDON_PACMAN", stub("missing"))
            .env("SELDON_OMARCHY", stub("omarchy"))
            .env(
                "SELDON_OMARCHY_PLUGINS_DIR",
                self.env.tmp.path().join("plugins"),
            )
            .env("SELDON_THEME_FILE", self.env.tmp.path().join("theme.name"))
            .env("TZ", "Europe/Berlin")
            .output()
            .expect("run seldon")
    }

    fn capture(&self, extra: &[&str]) -> serde_json::Value {
        let mut args = vec!["capture", "--all", "--json"];
        args.extend(extra);
        let out = self.run(&args);
        assert_eq!(out.status.code(), Some(0), "{}", common::stderr(&out));
        common::json(&out)
    }

    fn ledger(&self) -> Vec<Event> {
        seldon::ledger::Ledger::at(self.logbook.join("ledger"), Default::default())
            .read_all()
            .unwrap()
    }
}
