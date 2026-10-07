//! The user-space collectors (WP-005): plugins, theme, config.
//!
//! Each collector runs in-process against `fixtures/logs/` and temp
//! directories, like `seldon capture` runs it (collect, append, keep the
//! cursor); the last module drives the real binary. Nothing here reads or
//! writes the real `~/.config` or `~/.local/state` (AGENTS.md §6).

mod common;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::LazyLock;
use std::time::SystemTime;

use chrono::{DateTime, FixedOffset};
use serde_json::{Value, json};

use common::TempDir;
use seldon::collectors::config::{ConfigFiles, MAX_FILE_SIZE, Manifest};
use seldon::collectors::plugins::Plugins;
use seldon::collectors::theme::Theme;
use seldon::collectors::{Ctx, Outcome, Sources, Tz};
use seldon::config::{Config, Dirs};
use seldon::ledger::Ledger;
use seldon::logbook::lock;
use seldon::model::event::{Event, Kind, Meta, Source};
use seldon::redact::Redactor;

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn fixture(rel: &str) -> PathBuf {
    repo().join("fixtures").join(rel)
}

fn ts(s: &str) -> DateTime<FixedOffset> {
    DateTime::parse_from_rfc3339(s).unwrap()
}

/// Writes `text`, creating parent directories.
fn write(path: &Path, text: impl AsRef<[u8]>) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

/// An executable `sh` script at `path`, written without a descriptor the
/// test's other threads can inherit (`common::write_executable`).
fn script(path: &Path, body: &str) {
    common::write_executable(path, &format!("#!/bin/sh\n{body}\n"));
}

/// `sh` that prints `file` with builtins only (the CLI tests run with a
/// PATH that has no `cat`).
fn print_file(file: &Path) -> String {
    format!(
        "while IFS= read -r l || [ -n \"$l\" ]; do printf '%s\\n' \"$l\"; done < '{}'",
        file.display()
    )
}

static EVENT_SCHEMA: LazyLock<jsonschema::Validator> = LazyLock::new(|| {
    let text = std::fs::read_to_string(repo().join("schema/event.schema.json")).unwrap();
    let schema: Value = serde_json::from_str(&text).unwrap();
    jsonschema::options()
        .should_validate_formats(true)
        .build(&schema)
        .expect("event.schema.json compiles")
});

fn assert_schema_valid(event: &Event) {
    let instance = serde_json::to_value(event).unwrap();
    let errors: Vec<String> = EVENT_SCHEMA
        .iter_errors(&instance)
        .map(|e| format!("{e} at {}", e.instance_path()))
        .collect();
    assert!(
        errors.is_empty(),
        "{}: {}",
        event.to_line(),
        errors.join("; ")
    );
}

/// The first ledger line of the fixture logbook with this kind and subject.
fn fixture_event(kind: &str, subject: &str) -> Value {
    fixture_event_where(kind, subject, |_| true)
}

/// The first ledger line of the fixture logbook with this kind and subject
/// for which `pick` holds.
fn fixture_event_where(kind: &str, subject: &str, pick: impl Fn(&Value) -> bool) -> Value {
    for entry in std::fs::read_dir(fixture("logbook/ledger")).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|e| e != "jsonl") {
            continue;
        }
        for line in std::fs::read_to_string(&path).unwrap().lines() {
            let v: Value = serde_json::from_str(line).unwrap();
            if v["kind"] == kind && v["subject"] == subject && pick(&v) {
                return v;
            }
        }
    }
    panic!("no {kind} {subject} in fixtures/logbook/ledger");
}

/// `event` and the fixture line without what differs by design: `id`, `ts`
/// and, for hook-recorded lines, `actor`.
fn assert_matches_fixture(event: &Event, kind: &str, subject: &str, keys: &[&str]) {
    assert_matches_line(event, &fixture_event(kind, subject), keys);
}

/// `event` and the fixture line `theirs` in `keys`.
fn assert_matches_line(event: &Event, theirs: &Value, keys: &[&str]) {
    let ours = serde_json::to_value(event).unwrap();
    for key in keys {
        assert_eq!(ours[key], theirs[key], "{key} of {}", event.to_line());
    }
}

/// `git args…` in `dir` with `home` as HOME and no system or global
/// config; `None` when there is no git.
fn git_in(home: &Path, dir: &Path, args: &[&str]) -> Option<Output> {
    let out = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@example.invalid"])
        .args(args)
        .current_dir(dir)
        .env("HOME", home)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .output();
    match out {
        Ok(out) => Some(out),
        Err(_) => {
            eprintln!("skipped: no git on this host");
            None
        }
    }
}

/// A scratch home, ledger and state directory; runs collectors in-process
/// the way `seldon capture` does.
struct Bench {
    /// Taken once and held: re-acquiring per run races with stub processes
    /// that other test threads fork, because a child holds the inherited
    /// flock until it execs. Declared first, so it is released first.
    lock: lock::Lock,
    tmp: TempDir,
    dirs: Dirs,
    config: Config,
    ledger: Ledger,
    sources: Sources,
    cursors: BTreeMap<&'static str, Value>,
    /// Every event written, in order.
    written: Vec<Event>,
}

impl Bench {
    fn new(tag: &str) -> Self {
        let tmp = TempDir::new(tag);
        let home = tmp.path().join("home");
        std::fs::create_dir_all(&home).unwrap();
        let dirs = Dirs {
            xdg_config_home: home.join(".config"),
            state_dir: home.join(".local/state/seldon"),
            home,
        };
        // every field spelled out, no `Sources::default()` (the host's
        // paths and programs; WP-076): each test passes its own paths to
        // `collect_from`, so these are never read
        let missing = tmp.path().join("no-such-program").display().to_string();
        let sources = Sources {
            pacman_log: tmp.path().join("pacman.log"),
            pacman_db_lock: tmp.path().join("db.lck"),
            snapper: missing.clone(),
            snapshots: tmp.path().join("no-snapshots"),
            omarchy_version: missing.clone(),
            pacman: missing.clone(),
            omarchy: missing,
            plugins_dir: Some(tmp.path().join("plugins")),
            theme_file: Some(tmp.path().join("theme.name")),
            omarchy_path: tmp.path().join("omarchy"),
        };
        Bench {
            lock: lock::acquire(&tmp.path().join("lock")).unwrap(),
            ledger: Ledger::at(tmp.path().join("logbook/ledger"), Redactor::builtin()),
            tmp,
            dirs,
            config: Config::default(),
            sources,
            cursors: BTreeMap::new(),
            written: Vec::new(),
        }
    }

    fn home(&self, rel: &str) -> PathBuf {
        self.dirs.home.join(rel)
    }

    fn path(&self, rel: &str) -> PathBuf {
        self.tmp.path().join(rel)
    }

    /// Runs `collect` at `now` with the saved cursor of `name`; appends the
    /// events and keeps the new cursor, like `capture`. Every event written
    /// must validate against the schema.
    fn run(
        &mut self,
        name: &'static str,
        now: &str,
        collect: impl FnOnce(&Ctx, Option<&Value>) -> Outcome,
    ) -> Outcome {
        let out = self.collect(name, now, collect);
        let written = self.ledger.append(&self.lock, out.events.clone()).unwrap();
        for e in &written {
            assert_schema_valid(e);
        }
        self.written.extend(written);
        if let Some(c) = &out.cursor {
            self.cursors.insert(name, c.clone());
        }
        out
    }

    /// Runs `collect` without appending or keeping the cursor (a capture
    /// whose ledger write failed).
    fn collect(
        &self,
        name: &'static str,
        now: &str,
        collect: impl FnOnce(&Ctx, Option<&Value>) -> Outcome,
    ) -> Outcome {
        let now = ts(now);
        let ctx = Ctx {
            now,
            baseline: now,
            tz: Tz::Fixed(*now.offset()),
            sources: &self.sources,
            config: &self.config,
            dirs: &self.dirs,
            ledger: &self.ledger,
            earlier: &[],
        };
        collect(&ctx, self.cursors.get(name))
    }
}

mod plugins {
    use super::*;

    /// A bench with a stub `omarchy` that prints `list.json` for `plugin
    /// list --json` (or fails with `fail-list` present) and `catalog.json`
    /// for `plugin catalog` (or fails without it).
    pub(super) struct PluginBench {
        pub(super) b: Bench,
        omarchy: String,
        pub(super) plugins_dir: PathBuf,
    }

    impl PluginBench {
        pub(super) fn new(tag: &str) -> Self {
            let b = Bench::new(tag);
            let stub = b.path("bin/omarchy");
            let list = b.path("list.json");
            let catalog = b.path("catalog.json");
            let fail = b.path("fail-list");
            script(
                &stub,
                &format!(
                    "case \"$2\" in\n\
                     list) if [ -e '{fail}' ]; then echo 'omarchy-shell is not running' >&2; exit 1; fi; {list};;\n\
                     catalog) [ -e '{catalog_path}' ] || exit 1; {catalog};;\n\
                     esac",
                    fail = fail.display(),
                    list = print_file(&list),
                    catalog_path = catalog.display(),
                    catalog = print_file(&catalog),
                ),
            );
            let plugins_dir = b.home(".config/omarchy/plugins");
            PluginBench {
                omarchy: stub.to_string_lossy().into_owned(),
                plugins_dir,
                b,
            }
        }

        pub(super) fn list(&self, json: &str) {
            write(&self.b.path("list.json"), json);
        }

        pub(super) fn list_fixture(&self, name: &str) {
            self.list(&std::fs::read_to_string(fixture(name)).unwrap());
        }

        /// The fixture catalog, its user plugins moved into this home.
        fn catalog_fixture(&self) {
            let text = std::fs::read_to_string(fixture("logs/plugin-catalog.json")).unwrap();
            let text = text.replace(
                "/home/user/.config/omarchy/plugins",
                &self.plugins_dir.to_string_lossy(),
            );
            write(&self.b.path("catalog.json"), text);
        }

        /// Sets the mtime of a plugin's manifest and directory.
        fn touch(&self, id: &str, at: &str) {
            let t: SystemTime = ts(at).into();
            let dir = self.plugins_dir.join(id);
            for path in [dir.join("manifest.json"), dir] {
                std::fs::File::open(&path).unwrap().set_modified(t).unwrap();
            }
        }

        pub(super) fn manifest(&self, id: &str, version: Option<&str>) {
            let mut m = json!({"id": id, "name": id, "kinds": ["bar-widget"]});
            if let Some(v) = version {
                m["version"] = json!(v);
            }
            write(
                &self.plugins_dir.join(id).join("manifest.json"),
                m.to_string(),
            );
        }

        pub(super) fn run(&mut self, now: &str) -> Outcome {
            let (omarchy, dir) = (self.omarchy.clone(), self.plugins_dir.clone());
            self.b.run("plugins", now, |ctx, cursor| {
                Plugins.collect_from(ctx, cursor, &omarchy, &dir)
            })
        }
    }

    /// The fixture story's user plugins at their 10-01 versions.
    fn story() -> PluginBench {
        story_in("plugins")
    }

    /// [`story`] in a bench named `tag`.
    pub(super) fn story_in(tag: &str) -> PluginBench {
        let p = PluginBench::new(tag);
        p.catalog_fixture();
        p.manifest("io.github.example.weather-plus", Some("1.3.0"));
        p.manifest("user.clock", Some("1.0.0"));
        p.manifest("io.github.example.tyme", Some("1.1.0"));
        p
    }

    #[test]
    fn fixture_pair_adds_tyme_once() {
        let mut p = story();
        p.list_fixture("logs/plugin-list-before.json");
        let first = p.run("2026-10-01T16:00:00+02:00");
        assert!(first.ok, "{:?}", first.message);
        assert!(first.events.is_empty(), "the first run is a baseline");

        // `omarchy plugin add` (a clone) at 16:05
        p.list_fixture("logs/plugin-list-after.json");
        p.touch("io.github.example.tyme", "2026-10-01T16:05:00+02:00");
        let second = p.run("2026-10-01T16:10:00+02:00");
        assert!(second.ok);
        assert_eq!(second.events.len(), 1, "{:?}", second.events);
        let e = &p.b.written[0];
        assert_eq!(e.ts, ts("2026-10-01T16:05:00+02:00"), "the clone's time");
        assert_matches_fixture(
            e,
            "plugin-add",
            "io.github.example.tyme",
            &[
                "source", "kind", "subject", "detail", "actor", "zone", "meta",
            ],
        );

        let third = p.run("2026-10-01T16:20:00+02:00");
        assert!(third.ok);
        assert!(
            third.events.is_empty(),
            "second run on the same state: no events"
        );
        assert_eq!(p.b.written.len(), 1);
    }

    #[test]
    fn enable_disable_remove_and_update() {
        let mut p = story();
        p.manifest("io.github.example.weather-plus", Some("1.2.0"));
        // weather-plus is a clone (WP-136): the update names its commits
        // as the fixture line does; without git the line has none
        let home = p.b.dirs.home.clone();
        let clone = p.plugins_dir.join("io.github.example.weather-plus");
        let git = |args: &[&str]| {
            let out = git_in(&home, &clone, args)?;
            assert!(
                out.status.success(),
                "{}",
                String::from_utf8_lossy(&out.stderr)
            );
            Some(())
        };
        let commit = |message: &str| {
            std::fs::write(clone.join(message.replace(' ', "-")), message).unwrap();
            git(&["add", "."])?;
            git(&["commit", "-q", "-m", message])
        };
        let cloned = git(&["init", "-q"]).and_then(|()| commit("Release 1.2.0"));
        p.list_fixture("logs/plugin-list-after.json");
        p.run("2026-09-24T18:00:00+02:00");

        // weather-plus 1.2.0 → 1.3.0, omarchy.clock enabled, omarchy.agents
        // disabled, user.clock removed
        p.manifest("io.github.example.weather-plus", Some("1.3.0"));
        if cloned.is_some() {
            commit("Fix the unit toggle in the panel").unwrap();
            commit("Add a wind gust row").unwrap();
            commit("Release 1.3.0").unwrap();
        }
        p.touch(
            "io.github.example.weather-plus",
            "2026-09-24T19:00:14+02:00",
        );
        let mut list: Vec<Value> = serde_json::from_str(
            &std::fs::read_to_string(fixture("logs/plugin-list-after.json")).unwrap(),
        )
        .unwrap();
        list.retain(|p| p["id"] != "user.clock");
        for entry in &mut list {
            match entry["id"].as_str().unwrap() {
                "omarchy.clock" => entry["enabled"] = json!(true),
                "omarchy.agents" => entry["enabled"] = json!(false),
                _ => {}
            }
        }
        p.list(&serde_json::to_string(&list).unwrap());
        let out = p.run("2026-09-24T19:00:14+02:00");
        assert!(out.ok);
        let got: Vec<(Kind, &str)> = out
            .events
            .iter()
            .map(|e| (e.kind, e.subject.as_str()))
            .collect();
        assert_eq!(
            got,
            [
                (Kind::PluginUpdate, "io.github.example.weather-plus"),
                (Kind::PluginDisable, "omarchy.agents"),
                (Kind::PluginEnable, "omarchy.clock"),
                (Kind::PluginRemove, "user.clock"),
            ]
        );
        let update = &p.b.written[0];
        // the version step of 09-24 (09-22 is an in-place edit, WP-113);
        // the clone's commits as there (WP-136), the detail only with git
        let theirs = fixture_event_where("plugin-update", "io.github.example.weather-plus", |v| {
            v["meta"]["from"].is_string()
        });
        let keys: &[&str] = if cloned.is_some() {
            &["ts", "source", "kind", "subject", "detail", "actor", "zone"]
        } else {
            eprintln!("no git on this host: the update's detail and commits not compared");
            &["ts", "source", "kind", "subject", "actor", "zone"]
        };
        assert_matches_line(update, &theirs, keys);
        // the fixture line is from before WP-113: the version step as
        // there, plus the tree's hashes (the manifest is part of the tree)
        assert_eq!(update.meta.from.as_deref(), theirs["meta"]["from"].as_str());
        assert_eq!(update.meta.to.as_deref(), theirs["meta"]["to"].as_str());
        assert!(update.meta.hash_from.is_some() && update.meta.hash_to.is_some());
        if cloned.is_some() {
            for key in ["git", "commits"] {
                assert_eq!(update.meta.extra.get(key), theirs["meta"].get(key), "{key}");
            }
        }
        // first-party versions come from the catalog's manifestPath, which
        // does not exist here: no version, but still the enabled state
        let disable = &p.b.written[1];
        assert_eq!(disable.meta.enabled, Some(false));
        let enable = &p.b.written[2];
        assert_eq!(enable.meta.enabled, Some(true));
        let remove = &p.b.written[3];
        assert_eq!(remove.meta.version.as_deref(), Some("1.0.0"));
        assert_eq!(remove.meta.enabled, None);
        assert!(p.b.written.iter().all(|e| e.zone.is_some()));

        assert!(p.run("2026-09-24T19:10:00+02:00").events.is_empty());
    }

    #[test]
    fn first_party_version_changes_are_not_events() {
        // ADR-0018: first-party plugins ship with Omarchy; only third-party
        // plugins get plugin-update
        let mut p = story();
        p.list_fixture("logs/plugin-list-after.json");
        let clock = p.b.path("omarchy/shell/plugins/clock/manifest.json");
        write(&clock, r#"{"id":"omarchy.clock","version":"1.0.0"}"#);
        let mut catalog: Vec<Value> =
            serde_json::from_str(&std::fs::read_to_string(p.b.path("catalog.json")).unwrap())
                .unwrap();
        for c in &mut catalog {
            if c["id"] == "omarchy.clock" {
                c["manifestPath"] = json!(clock);
            }
        }
        write(
            &p.b.path("catalog.json"),
            serde_json::to_string(&catalog).unwrap(),
        );
        p.run("2026-10-01T16:00:00+02:00");

        // a first-party bump alone: tracked, not reported
        write(&clock, r#"{"id":"omarchy.clock","version":"1.1.0"}"#);
        let out = p.run("2026-10-01T16:10:00+02:00");
        assert!(out.ok);
        assert!(out.events.is_empty(), "{:?}", out.events);
        assert_eq!(
            p.b.cursors["plugins"]["plugins"]["omarchy.clock"]["version"],
            "1.1.0"
        );

        // a third-party bump next to a first-party one: one plugin-update
        write(&clock, r#"{"id":"omarchy.clock","version":"1.2.0"}"#);
        p.manifest("io.github.example.weather-plus", Some("1.4.0"));
        let out = p.run("2026-10-01T16:20:00+02:00");
        let got: Vec<(Kind, &str)> = out
            .events
            .iter()
            .map(|e| (e.kind, e.subject.as_str()))
            .collect();
        assert_eq!(
            got,
            [(Kind::PluginUpdate, "io.github.example.weather-plus")]
        );
        assert_eq!(out.events[0].detail.as_deref(), Some("1.3.0 → 1.4.0"));

        // enabling and disabling still fire for first-party plugins
        let mut list: Vec<Value> =
            serde_json::from_str(&std::fs::read_to_string(p.b.path("list.json")).unwrap()).unwrap();
        for entry in &mut list {
            if entry["id"] == "omarchy.clock" {
                entry["enabled"] = json!(true);
            }
        }
        p.list(&serde_json::to_string(&list).unwrap());
        let out = p.run("2026-10-01T16:30:00+02:00");
        assert_eq!(out.events.len(), 1);
        assert_eq!(out.events[0].kind, Kind::PluginEnable);
        assert_eq!(out.events[0].subject, "omarchy.clock");
    }

    #[test]
    fn add_and_update_are_timed_by_the_plugin_directory() {
        let mut p = story();
        p.list_fixture("logs/plugin-list-before.json");
        p.run("2026-10-01T16:00:00+02:00");

        // an mtime from before the last check (a copied directory) is the
        // last check; one from the future is now
        p.list_fixture("logs/plugin-list-after.json");
        p.touch("io.github.example.tyme", "2026-09-01T10:00:00+02:00");
        let out = p.run("2026-10-01T16:10:00+02:00");
        assert_eq!(out.events[0].ts, ts("2026-10-01T16:00:00+02:00"));

        p.manifest("io.github.example.weather-plus", Some("1.4.0"));
        p.touch(
            "io.github.example.weather-plus",
            "2030-01-01T00:00:00+01:00",
        );
        let out = p.run("2026-10-01T16:20:00+02:00");
        assert_eq!(out.events[0].kind, Kind::PluginUpdate);
        assert_eq!(out.events[0].ts, ts("2026-10-01T16:20:00+02:00"));

        // a pull at 16:25 that only rewrote the manifest
        p.manifest("io.github.example.weather-plus", Some("1.5.0"));
        let manifest = p
            .plugins_dir
            .join("io.github.example.weather-plus/manifest.json");
        std::fs::File::open(&manifest)
            .unwrap()
            .set_modified(ts("2026-10-01T16:25:00+02:00").into())
            .unwrap();
        std::fs::File::open(manifest.parent().unwrap())
            .unwrap()
            .set_modified(ts("2026-09-01T10:00:00+02:00").into())
            .unwrap();
        let out = p.run("2026-10-01T16:30:00+02:00");
        assert_eq!(out.events[0].ts, ts("2026-10-01T16:25:00+02:00"));

        // removal, enabling and disabling: the capture time
        p.list_fixture("logs/plugin-list-before.json");
        let out = p.run("2026-10-01T16:40:00+02:00");
        assert_eq!(out.events[0].kind, Kind::PluginRemove);
        assert_eq!(out.events[0].ts, ts("2026-10-01T16:40:00+02:00"));
    }

    #[test]
    fn shell_not_running_degrades_and_keeps_the_cursor() {
        let mut p = story();
        p.list_fixture("logs/plugin-list-before.json");
        p.run("2026-10-01T16:00:00+02:00");
        let before = p.b.cursors["plugins"].clone();

        write(&p.b.path("fail-list"), "");
        p.list_fixture("logs/plugin-list-after.json");
        let out = p.run("2026-10-01T16:10:00+02:00");
        assert!(!out.ok);
        assert!(out.events.is_empty());
        assert_eq!(out.cursor, None);
        let message = out.message.unwrap();
        assert!(
            message.contains("omarchy-shell is not running"),
            "{message}"
        );
        assert_eq!(p.b.cursors["plugins"], before);

        // the shell is back: the add is found then
        std::fs::remove_file(p.b.path("fail-list")).unwrap();
        let out = p.run("2026-10-01T16:20:00+02:00");
        assert!(out.ok);
        assert_eq!(out.events.len(), 1);
        assert_eq!(out.events[0].subject, "io.github.example.tyme");
    }

    #[test]
    fn missing_omarchy_and_bad_output_degrade() {
        let mut p = story();
        let dir = p.plugins_dir.clone();
        let missing = p.b.path("bin/no-such-omarchy");
        let out = p.b.run("plugins", "2026-10-01T16:00:00+02:00", |ctx, c| {
            Plugins.collect_from(ctx, c, missing.to_str().unwrap(), &dir)
        });
        assert!(!out.ok);
        assert!(out.message.unwrap().contains("not found"));

        p.list("Not JSON");
        let out = p.run("2026-10-01T16:00:00+02:00");
        assert!(!out.ok);
        assert!(out.message.unwrap().contains("unexpected output"));
        assert!(p.b.cursors.is_empty());
    }

    #[test]
    fn an_empty_list_after_plugins_is_not_believed() {
        let mut p = story();
        p.list_fixture("logs/plugin-list-before.json");
        p.run("2026-10-01T16:00:00+02:00");
        p.list("[]");
        let out = p.run("2026-10-01T16:10:00+02:00");
        assert!(!out.ok);
        assert!(out.events.is_empty());
    }

    #[test]
    fn a_failing_catalog_never_looks_like_an_update() {
        let mut p = story();
        p.list_fixture("logs/plugin-list-after.json");
        // a first-party manifest the catalog points to
        let first_party = p.b.path("omarchy/shell/plugins/clock/manifest.json");
        write(&first_party, r#"{"id":"omarchy.clock","version":"1.0.0"}"#);
        let mut catalog: Vec<Value> =
            serde_json::from_str(&std::fs::read_to_string(p.b.path("catalog.json")).unwrap())
                .unwrap();
        for c in &mut catalog {
            if c["id"] == "omarchy.clock" {
                c["manifestPath"] = json!(first_party);
            }
        }
        write(
            &p.b.path("catalog.json"),
            serde_json::to_string(&catalog).unwrap(),
        );
        p.run("2026-10-01T16:00:00+02:00");
        let cursor = &p.b.cursors["plugins"];
        assert_eq!(cursor["plugins"]["omarchy.clock"]["version"], "1.0.0");

        // the catalog fails: the version is kept, nothing is reported
        std::fs::remove_file(p.b.path("catalog.json")).unwrap();
        let out = p.run("2026-10-01T16:10:00+02:00");
        assert!(out.ok);
        assert!(out.events.is_empty(), "{:?}", out.events);
        assert_eq!(
            p.b.cursors["plugins"]["plugins"]["omarchy.clock"]["version"],
            "1.0.0"
        );
        // user plugins still have the directory fallback
        assert_eq!(
            p.b.cursors["plugins"]["plugins"]["io.github.example.tyme"]["version"],
            "1.1.0"
        );
    }

    #[test]
    fn version_falls_back_to_the_git_head_of_the_clone() {
        let mut p = story();
        let home = p.b.dirs.home.clone();
        let git = |dir: &Path, args: &[&str]| {
            std::process::Command::new("git")
                .args(args)
                .current_dir(dir)
                .env("HOME", &home)
                .env("GIT_CONFIG_NOSYSTEM", "1")
                .output()
        };
        p.manifest("io.github.example.tyme", None);
        let clone = p.plugins_dir.join("io.github.example.tyme");
        let Ok(out) = git(&clone, &["init", "-q"]) else {
            eprintln!("skipped: no git on this host");
            return;
        };
        assert!(out.status.success());
        for args in [
            &["add", "manifest.json"][..],
            &[
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@example.invalid",
                "commit",
                "-q",
                "-m",
                "one",
            ],
        ] {
            assert!(git(&clone, args).unwrap().status.success());
        }
        let head = git(&clone, &["rev-parse", "--short", "HEAD"]).unwrap();
        let head = String::from_utf8(head.stdout).unwrap().trim().to_string();

        p.list_fixture("logs/plugin-list-before.json");
        p.run("2026-10-01T16:00:00+02:00");
        p.list_fixture("logs/plugin-list-after.json");
        let out = p.run("2026-10-01T16:10:00+02:00");
        assert_eq!(out.events.len(), 1);
        assert_eq!(out.events[0].meta.version.as_deref(), Some(head.as_str()));
        // WP-136: the add of a clone says so
        assert_eq!(
            out.events[0].detail.as_deref(),
            Some(format!("{head}, installed by git clone").as_str())
        );
        assert_eq!(out.events[0].meta.extra["git"], "clone");

        // a plugin directory without its own .git has no version, even
        // inside another repository
        assert!(
            git(&p.plugins_dir, &["init", "-q"])
                .unwrap()
                .status
                .success()
        );
        p.manifest("user.clock", None);
        p.run("2026-10-01T16:20:00+02:00");
        assert_eq!(
            p.b.cursors["plugins"]["plugins"]["user.clock"]["version"], "1.0.0",
            "kept from the last readable manifest"
        );
    }

    /// The fixture story's plugins with a few more files each.
    fn trees() -> PluginBench {
        let p = story();
        for id in ["io.github.example.weather-plus", "io.github.example.tyme"] {
            let dir = p.plugins_dir.join(id);
            write(&dir.join("Widget.qml"), "import QtQuick\nItem {}\n");
            write(&dir.join("assets/icon.png"), b"\x89PNG\0\0\x01");
        }
        p.list_fixture("logs/plugin-list-after.json");
        p
    }

    fn short(h: &Option<String>) -> String {
        h.as_deref().unwrap().chars().take(8).collect()
    }

    /// WP-113: an in-place edit of a third-party plugin (no version
    /// change, no git) is one `plugin-update` carrying the tree's hashes;
    /// a second capture writes nothing.
    #[test]
    fn an_in_place_edit_of_a_plugin_is_one_update() {
        let mut p = trees();
        assert!(p.run("2026-10-07T10:00:00+02:00").events.is_empty());
        let cursor = &p.b.cursors["plugins"]["plugins"];
        assert!(
            cursor["io.github.example.tyme"]["tree"].is_string(),
            "{cursor}"
        );
        assert!(
            cursor["omarchy.clock"].get("tree").is_none(),
            "first-party plugins have no tree: {cursor}"
        );

        let dir = p.plugins_dir.join("io.github.example.tyme");
        write(
            &dir.join("Widget.qml"),
            "import QtQuick\nItem { Component.onCompleted: run() }\n",
        );
        write(&dir.join("lib/helper.so"), b"\x7fELF\0\0");
        let out = p.run("2026-10-07T10:10:00+02:00");
        assert_eq!(
            out.events.len(),
            1,
            "one event per tree, not per file: {:?}",
            out.events
        );
        let e = &out.events[0];
        assert_eq!(
            (e.kind, e.subject.as_str()),
            (Kind::PluginUpdate, "io.github.example.tyme")
        );
        assert_eq!((e.meta.from.as_ref(), e.meta.to.as_ref()), (None, None));
        assert_ne!(e.meta.hash_from, e.meta.hash_to);
        assert_eq!(
            e.detail.as_deref().unwrap(),
            format!(
                "files changed (sha256 {} → {})",
                short(&e.meta.hash_from),
                short(&e.meta.hash_to)
            )
        );
        assert!(
            p.run("2026-10-07T10:20:00+02:00").events.is_empty(),
            "idempotent"
        );

        // a version bump with its files: still one event, with both steps
        p.manifest("io.github.example.tyme", Some("1.2.0"));
        write(&dir.join("Widget.qml"), "import QtQuick\nItem {}\n");
        let out = p.run("2026-10-07T10:30:00+02:00");
        assert_eq!(out.events.len(), 1, "{:?}", out.events);
        let e = &out.events[0];
        assert_eq!(e.detail.as_deref(), Some("1.1.0 → 1.2.0"));
        assert!(e.meta.hash_from.is_some() && e.meta.hash_to.is_some());
        assert!(p.run("2026-10-07T10:40:00+02:00").events.is_empty());
    }

    /// `.git` and `[redaction] skipPaths` are not part of a tree; a link
    /// that is not to a file counts by its target, not walked.
    #[test]
    fn what_a_tree_holds() {
        let mut p = trees();
        let dir = p.plugins_dir.join("io.github.example.weather-plus");
        write(&dir.join(".git/HEAD"), "ref: refs/heads/main\n");
        p.run("2026-10-07T10:00:00+02:00");
        write(&dir.join(".git/FETCH_HEAD"), "x\n");
        // a default skipPaths pattern (`~/.config/omarchy/**/state.json`)
        write(&dir.join("state.json"), "{}\n");
        assert!(p.run("2026-10-07T10:10:00+02:00").events.is_empty());

        let elsewhere = p.b.path("elsewhere");
        write(&elsewhere.join("Evil.qml"), "Item {}\n");
        std::os::unix::fs::symlink(&elsewhere, dir.join("extra")).unwrap();
        let out = p.run("2026-10-07T10:20:00+02:00");
        assert_eq!(out.events.len(), 1, "a link arrived: {:?}", out.events);
        write(&elsewhere.join("Evil.qml"), "Item { x: 1 }\n");
        assert!(
            p.run("2026-10-07T10:30:00+02:00").events.is_empty(),
            "a linked directory is not walked"
        );
        std::fs::remove_file(dir.join("extra")).unwrap();
        std::os::unix::fs::symlink(p.b.path("other"), dir.join("extra")).unwrap();
        assert_eq!(
            p.run("2026-10-07T10:40:00+02:00").events.len(),
            1,
            "a new target"
        );
    }

    /// A cursor from before WP-113 has no tree: the first capture takes it
    /// without an event.
    #[test]
    fn a_tree_is_taken_without_an_event() {
        let mut p = trees();
        p.run("2026-10-07T10:00:00+02:00");
        let mut old = p.b.cursors["plugins"].clone();
        old.as_object_mut().unwrap().remove("stats");
        for (_, plugin) in old["plugins"].as_object_mut().unwrap() {
            plugin.as_object_mut().unwrap().remove("tree");
        }
        p.b.cursors.insert("plugins", old);
        let dir = p.plugins_dir.join("io.github.example.tyme");
        write(&dir.join("Widget.qml"), "Item { y: 2 }\n");
        assert!(p.run("2026-10-07T10:10:00+02:00").events.is_empty());
        let tree = &p.b.cursors["plugins"]["plugins"]["io.github.example.tyme"]["tree"];
        assert!(tree.is_string());
    }

    /// Whether the tests run as root (CI): root reads a file whatever its
    /// mode, so the unreadable halves are skipped there.
    fn root() -> bool {
        use std::os::unix::fs::MetadataExt as _;
        std::fs::metadata("/proc/self").is_ok_and(|m| m.uid() == 0)
    }

    /// WP-113 round 2 (B3), the reviewer's probe: one unreadable file must
    /// not freeze the tree. It counts by its size, time and mode (the tree
    /// changes once, `partial`), and an edit elsewhere is still one
    /// `plugin-update`. An unreadable plugin directory keeps the last hash.
    #[test]
    fn an_unreadable_entry_does_not_freeze_the_tree() {
        if root() {
            eprintln!("skipped: root reads every file");
            return;
        }
        use std::os::unix::fs::PermissionsExt as _;
        let mode = |p: &Path, m: u32| {
            std::fs::set_permissions(p, std::fs::Permissions::from_mode(m)).unwrap()
        };
        let mut p = trees();
        p.run("2026-10-07T10:00:00+02:00");
        let dir = p.plugins_dir.join("io.github.example.tyme");
        write(&dir.join("junk"), "x\n");
        p.run("2026-10-07T10:05:00+02:00");
        mode(&dir.join("junk"), 0o000);
        write(&dir.join("Widget.qml"), "Item { visible: false }\n");
        let out = p.run("2026-10-07T10:10:00+02:00");
        assert_eq!(out.events.len(), 1, "{:?}", out.events);
        assert_eq!(out.events[0].meta.extra["partial"], true);
        assert!(
            out.message
                .as_deref()
                .unwrap()
                .contains("1 entr(ies) of plugin trees could not be read")
        );
        assert_eq!(
            p.b.cursors["plugins"]["plugins"]["io.github.example.tyme"]["partial"],
            true
        );
        // still unreadable: an edit elsewhere is one more event
        write(&dir.join("Widget.qml"), "Item { visible: true }\n");
        assert_eq!(p.run("2026-10-07T10:20:00+02:00").events.len(), 1);
        assert!(
            p.run("2026-10-07T10:25:00+02:00").events.is_empty(),
            "idempotent"
        );
        // an unreadable directory counts the same way
        mode(&dir.join("assets"), 0o000);
        assert_eq!(p.run("2026-10-07T10:30:00+02:00").events.len(), 1);
        mode(&dir.join("assets"), 0o755);
        mode(&dir.join("junk"), 0o644);
        let out = p.run("2026-10-07T10:40:00+02:00");
        assert_eq!(out.events.len(), 1, "readable again: one step back");
        assert!(!out.events[0].meta.extra.contains_key("partial"));

        // round 3: an unreadable file written in place with its
        // modification time put back (`touch -d`) — the change time shows it
        let junk = dir.join("junk");
        mode(&junk, 0o200);
        assert_eq!(p.run("2026-10-07T10:42:00+02:00").events.len(), 1);
        let before = std::fs::metadata(&junk).unwrap();
        {
            use std::io::Write as _;
            let mut f = std::fs::File::options().write(true).open(&junk).unwrap();
            f.write_all(b"y\n").unwrap();
            f.set_modified(before.modified().unwrap()).unwrap();
        }
        let after = std::fs::metadata(&junk).unwrap();
        assert_eq!(
            (after.len(), after.modified().unwrap()),
            (before.len(), before.modified().unwrap())
        );
        let out = p.run("2026-10-07T10:44:00+02:00");
        assert_eq!(out.events.len(), 1, "{:?}", out.events);
        assert_eq!(out.events[0].meta.extra["partial"], true);
        mode(&junk, 0o644);
        p.run("2026-10-07T10:46:00+02:00");

        // the plugin directory itself unreadable: the last hash is kept
        let tree = p.b.cursors["plugins"]["plugins"]["io.github.example.tyme"]["tree"].clone();
        mode(&dir, 0o000);
        let out = p.run("2026-10-07T10:50:00+02:00");
        mode(&dir, 0o755);
        assert!(out.events.is_empty(), "{:?}", out.events);
        assert_eq!(
            p.b.cursors["plugins"]["plugins"]["io.github.example.tyme"]["tree"],
            tree
        );
    }

    /// N2: a tree holds at most `TREE_ENTRIES` entries; past them it is cut
    /// off (hashed from the first ones in walk order, `partial`, counted).
    /// A file over 64 MiB counts by its metadata — the reviewer's 2 GiB
    /// sparse file returns at once.
    #[test]
    fn a_tree_is_capped_and_huge_files_count_by_metadata() {
        let mut p = trees();
        let dir = p.plugins_dir.join("io.github.example.tyme");
        let huge = dir.join("assets/huge.bin");
        std::fs::File::create(&huge)
            .unwrap()
            .set_len(2 << 30)
            .unwrap();
        let start = std::time::Instant::now();
        p.run("2026-10-07T10:00:00+02:00");
        assert!(
            start.elapsed() < std::time::Duration::from_secs(2),
            "{:?}",
            start.elapsed()
        );
        let t: SystemTime = ts("2026-10-01T10:00:00+02:00").into();
        std::fs::File::options()
            .write(true)
            .open(&huge)
            .unwrap()
            .set_modified(t)
            .unwrap();
        let out = p.run("2026-10-07T10:10:00+02:00");
        assert_eq!(out.events.len(), 1, "a touch changes the fingerprint");
        assert_eq!(out.events[0].meta.extra["hashBasis"], "stat");
        std::fs::remove_file(&huge).unwrap();
        p.run("2026-10-07T10:15:00+02:00");

        let cap = seldon::collectors::plugins::TREE_ENTRIES;
        for i in 0..cap {
            write(&dir.join(format!("many/{i:05}")), "");
        }
        let out = p.run("2026-10-07T10:20:00+02:00");
        assert_eq!(out.events.len(), 1, "{:?}", out.events);
        assert_eq!(out.events[0].meta.extra["partial"], true);
        assert!(
            out.message
                .as_deref()
                .unwrap()
                .contains("1 plugin tree(s) cut off at 10000 entries")
        );
        // within the cut an edit shows; past it (sorted walk order) not
        write(&dir.join("Widget.qml"), "Item { opacity: 0 }\n");
        assert_eq!(p.run("2026-10-07T10:30:00+02:00").events.len(), 1);
        write(&dir.join("many/09999"), "late\n");
        assert!(p.run("2026-10-07T10:40:00+02:00").events.is_empty());
    }

    /// N4: the stat fingerprint is used — and it does not hide an edit
    /// that keeps the size and puts the modification time back (the
    /// change time and inode catch it; the review's mutant "always reuse"
    /// fails here).
    #[test]
    fn the_tree_fingerprint_is_reused_but_not_fooled() {
        let mut p = trees();
        let dir = p.plugins_dir.join("io.github.example.tyme");
        let past: SystemTime = ts("2026-10-01T10:00:00+02:00").into();
        let backdate = |path: &Path| {
            std::fs::File::options()
                .write(true)
                .open(path)
                .unwrap()
                .set_modified(past)
                .unwrap()
        };
        for f in ["manifest.json", "Widget.qml", "assets/icon.png"] {
            backdate(&dir.join(f));
        }
        p.run("2026-10-07T10:00:00+02:00");
        let stats = &p.b.cursors["plugins"]["stats"];
        assert!(
            stats["io.github.example.tyme"].is_string(),
            "fingerprint kept: {stats}"
        );
        assert!(p.run("2026-10-07T10:05:00+02:00").events.is_empty());

        let widget = dir.join("Widget.qml");
        let before = std::fs::read(&widget).unwrap();
        let mut after = before.clone();
        after[0] = after[0].to_ascii_uppercase();
        assert_ne!(before, after);
        std::fs::write(&widget, &after).unwrap();
        backdate(&widget);
        let out = p.run("2026-10-07T10:10:00+02:00");
        assert_eq!(
            out.events.len(),
            1,
            "same size, same mtime: {:?}",
            out.events
        );
    }

    /// A capture whose cursor save failed after its ledger write repeats
    /// no tree update (the ledger holds it).
    #[test]
    fn a_lost_cursor_save_repeats_no_tree_update() {
        let mut p = trees();
        p.run("2026-10-07T10:00:00+02:00");
        let before = p.b.cursors["plugins"].clone();
        write(
            &p.plugins_dir.join("io.github.example.tyme/Widget.qml"),
            "Item { z: 3 }\n",
        );
        assert_eq!(p.run("2026-10-07T10:10:00+02:00").events.len(), 1);
        p.b.cursors.insert("plugins", before);
        assert!(p.run("2026-10-07T10:20:00+02:00").events.is_empty());
    }
}

/// Plugin updates name their incoming commits (WP-136): a third-party
/// plugin's own clone, built with the host's `git` in the bench's home
/// (no system or global config). Skipped without `git`.
mod plugin_commits {
    use super::*;

    const WEATHER: &str = "io.github.example.weather-plus";

    struct Clone {
        p: plugins::PluginBench,
        dir: PathBuf,
    }

    impl Clone {
        /// The story with weather-plus as a clone whose manifest has no
        /// version (the short HEAD is the version), one commit, and a
        /// baseline taken. `None` without git on this host.
        fn new(tag: &str) -> Option<Self> {
            let p = plugins::story_in(tag);
            let dir = p.plugins_dir.join(WEATHER);
            p.manifest(WEATHER, None);
            let c = Clone { p, dir };
            c.git(&["init", "-q", "-b", "main"])?;
            c.commit("Initial import");
            c.p.list_fixture("logs/plugin-list-after.json");
            let mut c = c;
            let first = c.p.run("2026-10-02T09:00:00+02:00");
            assert!(first.ok && first.events.is_empty(), "{:?}", first.events);
            Some(c)
        }

        fn git(&self, args: &[&str]) -> Option<Output> {
            let out = git_in(&self.p.b.dirs.home, &self.dir, args)?;
            assert!(
                out.status.success(),
                "git {args:?}: {}",
                String::from_utf8_lossy(&out.stderr)
            );
            Some(out)
        }

        /// A commit that touches the manifest, with `message` verbatim.
        fn commit(&self, message: &str) {
            let manifest = self.dir.join("manifest.json");
            let mut text = std::fs::read_to_string(&manifest).unwrap();
            text.push(' ');
            write(&manifest, text);
            self.git(&["add", "manifest.json"]).unwrap();
            self.git(&["commit", "-q", "--cleanup=verbatim", "-m", message])
                .unwrap();
        }

        fn head(&self) -> String {
            let out = self.git(&["rev-parse", "HEAD"]).unwrap();
            String::from_utf8(out.stdout).unwrap().trim().to_string()
        }

        fn short(&self) -> String {
            let out = self.git(&["rev-parse", "--short", "HEAD"]).unwrap();
            String::from_utf8(out.stdout).unwrap().trim().to_string()
        }

        /// One capture; its only event, a plugin-update of weather-plus.
        fn update(&mut self, now: &str) -> Event {
            let out = self.p.run(now);
            assert!(out.ok, "{:?}", out.message);
            assert_eq!(out.events.len(), 1, "{:?}", out.events);
            let e = self.p.b.written.last().unwrap().clone();
            assert_eq!((e.kind, e.subject.as_str()), (Kind::PluginUpdate, WEATHER));
            e
        }
    }

    fn extra<'a>(e: &'a Event, key: &str) -> Option<&'a str> {
        e.meta.extra.get(key).and_then(Value::as_str)
    }

    #[test]
    fn pull_rollback_and_reset_are_named_with_their_commits() {
        let Some(mut c) = Clone::new("commits-steps") else {
            return;
        };
        let base = c.short();

        // update by pull: three commits came in
        for s in [
            "Add wind gusts",
            "Fix the unit toggle",
            "Update translations",
        ] {
            c.commit(s);
        }
        let pulled = c.short();
        let e = c.update("2026-10-02T10:00:00+02:00");
        assert_eq!(extra(&e, "git"), Some("pull"));
        assert_eq!(
            extra(&e, "commits"),
            Some("Update translations\nFix the unit toggle\nAdd wind gusts")
        );
        assert_eq!(
            e.detail.as_deref(),
            Some(format!("{base} → {pulled}, pulled 3 commits: Update translations …").as_str())
        );
        assert_eq!(
            (e.meta.from.as_deref(), e.meta.to.as_deref()),
            (Some(base.as_str()), Some(pulled.as_str()))
        );
        assert_eq!(
            c.p.b.cursors["plugins"]["plugins"][WEATHER]["head"],
            c.head()
        );

        // idempotent: nothing moved, nothing new
        let again = c.p.run("2026-10-02T10:05:00+02:00");
        assert!(again.events.is_empty(), "{:?}", again.events);

        // rollback by reset: two commits left
        c.git(&["reset", "-q", "--hard", "HEAD~2"]).unwrap();
        let back = c.short();
        let e = c.update("2026-10-02T11:00:00+02:00");
        assert_eq!(extra(&e, "git"), Some("rollback"));
        assert_eq!(
            extra(&e, "commits"),
            Some("Update translations\nFix the unit toggle")
        );
        assert_eq!(
            e.detail.as_deref(),
            Some(
                format!("{pulled} → {back}, rolled back 2 commits: Update translations …").as_str()
            )
        );

        // reset to another history: one commit in, one out
        c.git(&["reset", "-q", "--hard", "HEAD~1"]).unwrap();
        c.commit("Rewrite the forecast panel");
        let other = c.short();
        let e = c.update("2026-10-02T12:00:00+02:00");
        assert_eq!(extra(&e, "git"), Some("reset"));
        assert_eq!(extra(&e, "commits"), Some("Rewrite the forecast panel"));
        assert_eq!(
            e.detail.as_deref(),
            Some(
                format!("{back} → {other}, reset: 1 commit in, 1 out: Rewrite the forecast panel")
                    .as_str()
            )
        );

        // one commit: no ellipsis (the refs packed, the HEAD detached:
        // read from the files all the same); more than 20: the count, 20
        // subjects
        c.commit("Only one");
        c.git(&["pack-refs", "--all"]).unwrap();
        assert!(!c.dir.join(".git/refs/heads/main").exists());
        let e = c.update("2026-10-02T13:00:00+02:00");
        assert_eq!(
            c.p.b.cursors["plugins"]["plugins"][WEATHER]["head"],
            c.head()
        );
        assert!(
            e.detail
                .as_deref()
                .unwrap()
                .ends_with(", pulled 1 commit: Only one"),
            "{:?}",
            e.detail
        );
        c.git(&["checkout", "-q", "--detach"]).unwrap();
        for k in 1..=25 {
            c.commit(&format!("Step {k}"));
        }
        let e = c.update("2026-10-02T14:00:00+02:00");
        assert_eq!(
            c.p.b.cursors["plugins"]["plugins"][WEATHER]["head"],
            c.head()
        );
        let commits: Vec<&str> = extra(&e, "commits").unwrap().split('\n').collect();
        assert_eq!(commits.len(), seldon::collectors::plugins::COMMITS_MAX);
        assert_eq!((commits[0], commits[19]), ("Step 25", "Step 6"));
        assert!(
            e.detail
                .as_deref()
                .unwrap()
                .ends_with(", pulled 25 commits: Step 25 …")
        );
    }

    #[test]
    fn a_hostile_subject_is_cleaned_redacted_and_clipped() {
        let Some(mut c) = Clone::new("commits-hostile") else {
            return;
        };
        let long = "x".repeat(300);
        c.commit("\u{1b}[31mred\u{1b}[0m\rover\u{7}written\ttab");
        c.commit("evil \u{202E}txt.exe\u{202C} and to\u{200B}ken=ghp_EXAMPLEsecret123 here");
        c.commit(&format!("token=ghp_EXAMPLEsecret456 {long}"));
        c.commit(&format!("{} token=ghp_EXAMPLEsecret789", "y".repeat(95)));
        // a bare GitHub token across the cut: clipped first, its prefix
        // would match no rule
        let token = format!("ghp_EXAMPLE{}", "0".repeat(29));
        c.commit(&format!("{} {token} tail", "z".repeat(80)));
        let e = c.update("2026-10-02T10:00:00+02:00");
        let commits = extra(&e, "commits").unwrap();
        let lines: Vec<&str> = commits.split('\n').collect();
        assert_eq!(lines.len(), 5, "{commits:?}");
        for line in &lines {
            assert!(!line.chars().any(char::is_control), "{line:?}");
            assert!(
                line.chars().count() <= seldon::collectors::plugins::COMMIT_SUBJECT_MAX,
                "{line:?}"
            );
            assert!(!line.contains("ghp_EXAMPLE"), "{line:?}");
        }
        // redacted before the clip: a secret at the cut is masked whole
        assert_eq!(lines[0], format!("{} ‹redacted› tail", "z".repeat(80)));
        let lines = &lines[1..];
        assert_eq!(
            lines[0],
            format!("{} token=‹redacted›", "y".repeat(95))
                .chars()
                .take(99)
                .collect::<String>()
                + "…"
        );
        assert!(
            lines[1].starts_with("token=‹redacted› xxx") && lines[1].ends_with('…'),
            "{:?}",
            lines[1]
        );
        assert_eq!(lines[2], "evil txt.exe and token=‹redacted› here");
        assert_eq!(lines[3], "[31mred [0m over written tab");
        assert!(!e.detail.as_deref().unwrap().contains("ghp_EXAMPLE"));
        assert!(!e.detail.as_deref().unwrap().chars().any(char::is_control));
    }

    #[test]
    fn without_a_clone_or_a_known_head_there_is_no_list() {
        // a plugin without git: the version step only
        let mut p = plugins::story_in("commits-none");
        p.list_fixture("logs/plugin-list-after.json");
        p.run("2026-10-02T09:00:00+02:00");
        p.manifest(WEATHER, Some("1.4.0"));
        let out = p.run("2026-10-02T10:00:00+02:00");
        assert_eq!(out.events.len(), 1);
        let e = &out.events[0];
        assert_eq!(e.detail.as_deref(), Some("1.3.0 → 1.4.0"));
        assert!(e.meta.extra.is_empty(), "{:?}", e.meta.extra);
        assert!(
            p.b.cursors["plugins"]["plugins"][WEATHER]
                .get("head")
                .is_none()
        );

        // a cursor from before WP-136 (no head): no list for this step,
        // then the next one has it
        let Some(mut c) = Clone::new("commits-legacy") else {
            return;
        };
        let cursor = c.p.b.cursors.get_mut("plugins").unwrap();
        cursor["plugins"][WEATHER]
            .as_object_mut()
            .unwrap()
            .remove("head");
        c.commit("Before the head was known");
        let e = c.update("2026-10-02T10:00:00+02:00");
        assert!(e.meta.extra.is_empty(), "{:?}", e.meta.extra);
        c.commit("After");
        let e = c.update("2026-10-02T11:00:00+02:00");
        assert_eq!(extra(&e, "commits"), Some("After"));

        // a HEAD unreadable for one capture keeps the last one
        let (head, aside) = (c.dir.join(".git/HEAD"), c.dir.join(".git/HEAD.aside"));
        std::fs::rename(&head, &aside).unwrap();
        let out = c.p.run("2026-10-02T11:10:00+02:00");
        assert!(out.ok && out.events.is_empty(), "{:?}", out.events);
        std::fs::rename(&aside, &head).unwrap();
        c.commit("After the gap");
        let e = c.update("2026-10-02T11:20:00+02:00");
        assert_eq!(extra(&e, "commits"), Some("After the gap"));

        // a head git no longer has, or one shaped like an option: the
        // event without the list, and git never sees it as an option
        for bad in ["0123456789abcdef0123456789abcdef01234567", "--output=pwned"] {
            let cursor = c.p.b.cursors.get_mut("plugins").unwrap();
            cursor["plugins"][WEATHER]["head"] = json!(bad);
            c.commit(&format!("After {bad}"));
            let e = c.update("2026-10-02T12:00:00+02:00");
            assert!(e.meta.extra.is_empty(), "{bad}: {:?}", e.meta.extra);
            assert!(!c.dir.join("pwned").exists());
        }
    }

    /// The capture cost of WP-136 (not part of `check`; run with
    /// `--ignored --nocapture`, best in the bench profile): 8 third-party
    /// plugins with a manifest version, without and with their own clone
    /// (one `rev-parse` each per capture), and an update whose HEAD moved
    /// (two more queries). Prints medians; asserts only generous ceilings.
    #[test]
    #[ignore]
    fn capture_cost_of_clone_heads() {
        const RUNS: usize = 31;
        let mut p = plugins::story_in("commits-cost");
        let home = p.b.dirs.home.clone();
        let ids: Vec<String> = (0..8).map(|k| format!("io.github.example.p{k}")).collect();
        let list: Vec<Value> = ids
            .iter()
            .map(|id| json!({"id": id, "enabled": true, "firstParty": false}))
            .collect();
        p.list(&serde_json::to_string(&list).unwrap());
        for id in &ids {
            p.manifest(id, Some("1.0.0"));
        }
        let median = |p: &mut plugins::PluginBench, what: &str| {
            let mut times = Vec::new();
            for _ in 0..RUNS {
                let start = std::time::Instant::now();
                assert!(p.run("2026-10-02T09:00:00+02:00").ok);
                times.push(start.elapsed());
            }
            times.sort();
            eprintln!("plugins capture, {what}: median {:?}", times[RUNS / 2]);
            times[RUNS / 2]
        };
        p.run("2026-10-02T08:00:00+02:00");
        let plain = median(&mut p, "8 plugins, no clone");
        for id in &ids {
            let dir = p.plugins_dir.join(id);
            for args in [
                &["init", "-q"][..],
                &["add", "."],
                &["commit", "-q", "-m", "one"],
            ] {
                let Some(out) = git_in(&home, &dir, args) else {
                    return;
                };
                assert!(out.status.success());
            }
        }
        p.run("2026-10-02T08:30:00+02:00");
        let clones = median(&mut p, "8 plugins, 8 clones");
        eprintln!(
            "delta: {:?} per capture, {:?} per clone",
            clones.saturating_sub(plain),
            clones.saturating_sub(plain) / 8
        );
        // an update with a moved HEAD: the version bump and a commit
        // between captures (not timed)
        let dir = p.plugins_dir.join(&ids[0]);
        let mut times = Vec::new();
        for k in 0..11 {
            p.manifest(&ids[0], Some(&format!("1.0.{}", k + 1)));
            let out = git_in(&home, &dir, &["commit", "-q", "-am", &format!("Step {k}")]).unwrap();
            assert!(out.status.success());
            let start = std::time::Instant::now();
            let out = p.run(&format!("2026-10-02T10:{k:02}:00+02:00"));
            times.push(start.elapsed());
            assert_eq!(out.events.len(), 1);
            assert!(out.events[0].meta.extra.contains_key("commits"));
        }
        times.sort();
        eprintln!(
            "plugins capture, 8 clones, one update with commits: median {:?}",
            times[5]
        );
        assert!(clones < std::time::Duration::from_millis(500), "{clones:?}");
        assert!(
            times[5] < std::time::Duration::from_millis(500),
            "{:?}",
            times[5]
        );
    }

    /// The merge with WP-113: a pull that leaves the manifest's version as
    /// it is fires through the tree hash, and that update names its
    /// commits too.
    #[test]
    fn a_pull_without_a_version_bump_names_its_commits() {
        let Some(mut c) = Clone::new("commits-tree") else {
            return;
        };
        c.p.manifest(WEATHER, Some("1.0.0"));
        c.commit("Pin the version");
        c.update("2026-10-02T09:30:00+02:00");
        c.commit("Fix the radar colours");
        c.commit("Tidy the panel");
        let e = c.update("2026-10-02T10:00:00+02:00");
        assert_eq!((e.meta.from.as_deref(), e.meta.to.as_deref()), (None, None));
        assert!(e.meta.hash_from.is_some() && e.meta.hash_to.is_some());
        assert_eq!(extra(&e, "git"), Some("pull"));
        assert_eq!(
            extra(&e, "commits"),
            Some("Tidy the panel\nFix the radar colours")
        );
        let detail = e.detail.as_deref().unwrap();
        assert!(
            detail.starts_with("files changed (sha256 ")
                && detail.ends_with("), pulled 2 commits: Tidy the panel …"),
            "{detail}"
        );
    }

    /// WP-136 round 2 (B1): the clone's `info/grafts` is never read: it
    /// would fake parents (a pull would read as a reset), and each bad
    /// line of it is a line of stderr (a 63 MB file made 132 MB).
    #[test]
    fn a_grafts_file_is_not_read() {
        let Some(mut c) = Clone::new("commits-grafts") else {
            return;
        };
        for s in ["A", "B", "C"] {
            c.commit(s);
        }
        let head = c.head();
        // the new HEAD made a root commit, then a flood of bad lines
        let mut grafts = format!("{head}\n");
        grafts.push_str(&"not a graft line\n".repeat(200_000));
        write(&c.dir.join(".git/info/grafts"), grafts);
        let started = std::time::Instant::now();
        let e = c.update("2026-10-02T10:00:00+02:00");
        assert!(started.elapsed() < std::time::Duration::from_secs(8));
        assert_eq!(extra(&e, "git"), Some("pull"));
        assert_eq!(extra(&e, "commits"), Some("C\nB\nA"));
    }

    /// WP-136 round 2 (N1): a partial clone whose promisor remote is an
    /// `ext::` command, allowed by the clone's own
    /// `protocol.ext.allow=always` (which beats `-c protocol.allow=never`),
    /// with the old HEAD's object missing: git would run the command to
    /// fetch it. It never runs; the update has no commits.
    #[test]
    fn a_partial_clone_never_fetches() {
        let Some(mut c) = Clone::new("commits-lazy") else {
            return;
        };
        let old = c.head();
        c.commit("Second");
        let marker = c.p.b.path("fetched");
        let fetch = c.p.b.path("fetch.sh");
        script(&fetch, &format!("touch '{}'\nexit 1", marker.display()));
        for (key, value) in [
            ("core.repositoryformatversion", "1".to_string()),
            ("extensions.partialClone", "origin".to_string()),
            ("remote.origin.url", format!("ext::{}", fetch.display())),
            ("remote.origin.promisor", "true".to_string()),
            ("protocol.ext.allow", "always".to_string()),
        ] {
            c.git(&["config", key, &value]).unwrap();
        }
        // from here on the test's own git is never run (it would fetch)
        let object = c.dir.join(".git/objects").join(&old[..2]).join(&old[2..]);
        std::fs::remove_file(&object).unwrap();
        let e = c.update("2026-10-02T10:00:00+02:00");
        assert!(!marker.exists(), "git ran the promisor's ext:: command");
        assert!(e.meta.extra.is_empty(), "{:?}", e.meta.extra);
    }

    /// WP-136 round 2 (N2): a `.git` that can make git read another
    /// repository is not read; the update keeps its version step and says
    /// why the commits are missing.
    #[test]
    fn a_repository_pointing_outside_names_no_commits() {
        /// Makes the clone's `.git` point outside the plugin folder.
        type PointOutside = fn(&Clone);
        let cases: [(&str, PointOutside); 8] = [
            ("link", |c| {
                let elsewhere = c.p.b.path("elsewhere.git");
                std::fs::rename(c.dir.join(".git"), &elsewhere).unwrap();
                std::os::unix::fs::symlink(&elsewhere, c.dir.join(".git")).unwrap();
            }),
            ("gitdir", |c| {
                let elsewhere = c.p.b.path("elsewhere.git");
                std::fs::rename(c.dir.join(".git"), &elsewhere).unwrap();
                write(
                    &c.dir.join(".git"),
                    format!("gitdir: {}\n", elsewhere.display()),
                );
            }),
            ("alternates", |c| {
                let other = c.p.b.path("other.git/objects");
                write(
                    &c.dir.join(".git/objects/info/alternates"),
                    format!("{}\n", other.display()),
                );
            }),
            ("commondir", |c| {
                write(&c.dir.join(".git/commondir"), "../../other.git\n");
            }),
            ("include", |c| {
                c.git(&["config", "include.path", "/dev/null"]).unwrap();
            }),
            ("bom", |c| {
                // git skips a byte order mark and reads the section after it
                let config = c.dir.join(".git/config");
                let text = std::fs::read_to_string(&config).unwrap();
                write(
                    &config,
                    format!("\u{FEFF}[include]\n\tpath = /dev/null\n{text}"),
                );
            }),
            ("linked-objects", |c| {
                let objects = c.dir.join(".git/objects");
                let elsewhere = c.p.b.path("elsewhere-objects");
                std::fs::rename(&objects, &elsewhere).unwrap();
                std::os::unix::fs::symlink(&elsewhere, &objects).unwrap();
            }),
            ("includeIf", |c| {
                c.git(&["config", "includeIf.gitdir:/nowhere/.path", "/dev/null"])
                    .unwrap();
            }),
        ];
        for (name, point_outside) in cases {
            let Some(mut c) = Clone::new(&format!("commits-outside-{name}")) else {
                return;
            };
            let from = c.short();
            c.p.manifest(WEATHER, Some("2.0.0"));
            c.commit("Release 2.0.0");
            point_outside(&c);
            let e = c.update("2026-10-02T10:00:00+02:00");
            assert_eq!(
                e.detail.as_deref(),
                Some(format!("{from} → 2.0.0, {}", seldon::collectors::plugins::OUTSIDE).as_str()),
                "{name}"
            );
            assert!(e.meta.extra.is_empty(), "{name}: {:?}", e.meta.extra);
            assert!(
                c.p.b.cursors["plugins"]["plugins"][WEATHER]
                    .get("head")
                    .is_none(),
                "{name}"
            );
        }
    }

    /// WP-136 round 2 (N4): the clone's `i18n.logOutputEncoding` does not
    /// change the output (UTF-16 would put NULs into the `-z` list); a
    /// subject that is not UTF-8 comes out as git converts it (here Latin-1
    /// to UTF-8; the lossy read of bytes git passes through is a unit test
    /// in `collectors/plugins.rs`).
    #[test]
    fn the_log_is_utf8_whatever_the_clone_says() {
        let Some(mut c) = Clone::new("commits-encoding") else {
            return;
        };
        c.git(&["config", "i18n.logOutputEncoding", "UTF-16"])
            .unwrap();
        c.commit("Plain subject");
        let message = c.p.b.path("message");
        write(&message, b"Caf\xe9 au lait\n");
        let manifest = c.dir.join("manifest.json");
        let mut text = std::fs::read_to_string(&manifest).unwrap();
        text.push(' ');
        write(&manifest, text);
        c.git(&["add", "manifest.json"]).unwrap();
        c.git(&[
            "commit",
            "-q",
            "--cleanup=verbatim",
            "-F",
            &message.to_string_lossy(),
        ])
        .unwrap();
        let e = c.update("2026-10-02T10:00:00+02:00");
        assert_eq!(extra(&e, "commits"), Some("Café au lait\nPlain subject"));
    }

    #[test]
    fn a_broken_git_dir_inside_another_repository_is_no_clone() {
        let mut p = plugins::story_in("commits-ceiling");
        let home = p.b.dirs.home.clone();
        if git_in(&home, &p.plugins_dir, &["init", "-q"]).is_none() {
            return;
        }
        // the outer repository has commits; the plugin's `.git` is empty
        let outer = |args: &[&str]| git_in(&home, &p.plugins_dir, args).unwrap();
        assert!(outer(&["add", "."]).status.success());
        assert!(outer(&["commit", "-q", "-m", "dotfiles"]).status.success());
        std::fs::create_dir_all(p.plugins_dir.join(WEATHER).join(".git")).unwrap();
        p.manifest(WEATHER, None);
        p.list_fixture("logs/plugin-list-after.json");
        p.run("2026-10-02T09:00:00+02:00");
        let state = &p.b.cursors["plugins"]["plugins"][WEATHER];
        assert!(
            state.get("head").is_none() && state.get("version").is_none(),
            "{state}"
        );
    }
}

mod theme {
    use super::*;

    fn run(b: &mut Bench, now: &str) -> Outcome {
        let file = b.home(".local/state/omarchy/current/theme.name");
        b.run("theme", now, |ctx, cursor| {
            Theme.collect_from(ctx, cursor, &file)
        })
    }

    fn set(b: &Bench, slug: &str, mtime: Option<&str>) {
        let file = b.home(".local/state/omarchy/current/theme.name");
        write(&file, format!("{slug}\n")); // omarchy-theme-set: echo "$THEME_NAME"
        if let Some(t) = mtime {
            let t: SystemTime = ts(t).into();
            std::fs::File::options()
                .write(true)
                .open(&file)
                .unwrap()
                .set_modified(t)
                .unwrap();
        }
    }

    #[test]
    fn fixture_pair_sets_kanagawa_once() {
        let mut b = Bench::new("theme");
        set(&b, "osaka-jade", None);
        let first = run(&mut b, "2026-09-20T20:00:00+02:00");
        assert!(first.ok && first.events.is_empty());

        set(&b, "kanagawa", Some("2026-09-20T21:02:47+02:00"));
        let second = run(&mut b, "2026-09-20T21:30:00+02:00");
        assert_eq!(second.events.len(), 1);
        // ts is the file's mtime; actor differs: the fixture line came from
        // the hook (human), the collector cannot know who switched
        assert_matches_fixture(
            &b.written[0],
            "theme-set",
            "kanagawa",
            &["ts", "source", "kind", "subject", "detail", "zone", "meta"],
        );
        assert_eq!(b.written[0].actor, "system");

        let third = run(&mut b, "2026-09-20T21:40:00+02:00");
        assert!(third.ok && third.events.is_empty());
        assert_eq!(b.written.len(), 1);
    }

    #[test]
    fn the_time_is_clamped_to_the_window_since_the_last_check() {
        let mut b = Bench::new("theme-clamp");
        set(&b, "osaka-jade", None);
        run(&mut b, "2026-09-20T20:00:00+02:00");
        // an mtime from before the last check (a copied file)
        set(&b, "kanagawa", Some("2026-08-01T10:00:00+02:00"));
        run(&mut b, "2026-09-20T21:00:00+02:00");
        assert_eq!(b.written[0].ts, ts("2026-09-20T20:00:00+02:00"));
        // an mtime from the future
        set(&b, "tokyo-night", Some("2030-01-01T00:00:00+01:00"));
        run(&mut b, "2026-09-20T22:00:00+02:00");
        assert_eq!(b.written[1].ts, ts("2026-09-20T22:00:00+02:00"));
        assert_eq!(b.written[1].meta.from.as_deref(), Some("kanagawa"));
    }

    #[test]
    fn a_change_the_hook_recorded_is_not_written_twice() {
        let mut b = Bench::new("theme-hook");
        set(&b, "kanagawa", None);
        run(&mut b, "2026-10-01T15:00:00+02:00");

        // the theme-set hook ran `seldon event theme theme-set --subject tokyo-night`
        set(&b, "tokyo-night", Some("2026-10-01T15:30:00+02:00"));
        let hook = Event::new(
            ts("2026-10-01T15:30:00+02:00"),
            Source::Theme,
            Kind::ThemeSet,
            "tokyo-night",
        )
        .actor("human")
        .meta(Meta {
            from: Some("kanagawa".into()),
            to: Some("tokyo-night".into()),
            ..Meta::default()
        });
        b.ledger.append(&b.lock, vec![hook]).unwrap();

        let out = run(&mut b, "2026-10-01T16:00:00+02:00");
        assert!(out.ok);
        assert!(out.events.is_empty(), "{:?}", out.events);

        // a later change without the hook is the collector's again
        set(&b, "osaka-jade", Some("2026-10-01T16:30:00+02:00"));
        let out = run(&mut b, "2026-10-01T17:00:00+02:00");
        assert_eq!(out.events.len(), 1);
        assert_eq!(out.events[0].meta.from.as_deref(), Some("tokyo-night"));
    }

    #[test]
    fn a_missing_or_empty_file_degrades() {
        let mut b = Bench::new("theme-missing");
        let out = run(&mut b, "2026-10-01T15:00:00+02:00");
        assert!(!out.ok);
        assert_eq!(out.cursor, None);
        let message = out.message.unwrap();
        assert!(
            message.contains("~/.local/state/omarchy/current/theme.name"),
            "{message}"
        );

        set(&b, "osaka-jade", None);
        run(&mut b, "2026-10-01T15:10:00+02:00");
        set(&b, "  ", None);
        let out = run(&mut b, "2026-10-01T15:20:00+02:00");
        assert!(!out.ok);
        assert_eq!(b.cursors["theme"]["theme"], "osaka-jade");
    }
}

mod config {
    use super::*;

    /// A bench watching `~/.config/hypr`, `~/.config/omarchy` and
    /// `~/.config/systemd`, with Omarchy's plugin directory excluded.
    struct ConfigBench {
        b: Bench,
    }

    impl ConfigBench {
        fn new(tag: &str) -> Self {
            let mut b = Bench::new(tag);
            b.config.watch_paths = vec![
                "~/.config/hypr".into(),
                "~/.config/omarchy".into(),
                "~/.config/systemd".into(),
                "~/.bashrc".into(),
            ];
            ConfigBench { b }
        }

        fn file(&self, rel: &str, text: impl AsRef<[u8]>) {
            write(&self.b.home(rel), text);
        }

        fn touch(&self, rel: &str, at: &str) {
            let t: SystemTime = ts(at).into();
            std::fs::File::options()
                .write(true)
                .open(self.b.home(rel))
                .unwrap()
                .set_modified(t)
                .unwrap();
        }

        fn collect(&self, now: &str) -> Outcome {
            self.b.collect("config", now, Self::collector(&self.b))
        }

        fn run(&mut self, now: &str) -> Outcome {
            let collect = Self::collector(&self.b);
            self.b.run("config", now, collect)
        }

        fn collector(b: &Bench) -> impl FnOnce(&Ctx, Option<&Value>) -> Outcome + use<> {
            let roots: Vec<PathBuf> = b
                .config
                .watch_paths
                .iter()
                .filter_map(|p| b.dirs.expand_config(p))
                .collect();
            let excluded = vec![b.home(".config/omarchy/plugins")];
            let manifest = Manifest::file(&b.dirs);
            move |ctx, cursor| ConfigFiles.collect_from(ctx, cursor, &roots, &excluded, &manifest)
        }

        fn manifest_text(&self) -> String {
            std::fs::read_to_string(Manifest::file(&self.b.dirs)).unwrap()
        }

        fn manifest(&self) -> Manifest {
            serde_json::from_str(&self.manifest_text()).unwrap()
        }
    }

    const ABC: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

    /// The fixture story's config events of 09-12 and 10-01: a changed
    /// monitors.conf, a changed bindings.conf, a new user unit.
    #[test]
    fn before_after_pair() {
        let mut c = ConfigBench::new("config");
        c.file(".config/hypr/monitors.conf", "monitor=,preferred,auto,1\n");
        c.file(
            ".config/hypr/bindings.conf",
            "bind = SUPER, Return, exec, kitty\n",
        );
        c.file(".config/hypr/old.conf", "abc");
        c.file(".config/omarchy/shell.json", "{}\n");
        c.file(".bashrc", "export EDITOR=nvim\n");
        let first = c.run("2026-10-01T10:00:00+02:00");
        assert!(first.ok, "{:?}", first.message);
        assert!(first.events.is_empty(), "the first run is a baseline");
        assert_eq!(c.manifest().current.files.len(), 5);
        assert_eq!(
            c.manifest().current.files["~/.config/hypr/old.conf"],
            ABC,
            "keys are ~-paths, values sha256"
        );

        c.file(
            ".config/hypr/monitors.conf",
            "monitor=,preferred,auto,1.25\n",
        );
        c.touch(".config/hypr/monitors.conf", "2026-10-01T10:40:02+02:00");
        c.file(
            ".config/systemd/user/ollama.service",
            "[Service]\nExecStart=ollama serve\n",
        );
        c.touch(
            ".config/systemd/user/ollama.service",
            "2026-10-01T14:03:30+02:00",
        );
        std::fs::remove_file(c.b.home(".config/hypr/old.conf")).unwrap();
        let second = c.run("2026-10-01T15:00:00+02:00");
        assert!(second.ok);
        let got: Vec<(Kind, &str)> =
            c.b.written
                .iter()
                .map(|e| (e.kind, e.subject.as_str()))
                .collect();
        assert_eq!(
            got,
            [
                (Kind::ConfigChange, "~/.config/hypr/monitors.conf"),
                (Kind::ConfigRemove, "~/.config/hypr/old.conf"),
                (Kind::ConfigAdd, "~/.config/systemd/user/ollama.service"),
            ]
        );
        let [change, remove, add] = &c.b.written[..] else {
            unreachable!()
        };
        // ts: the file's mtime; a removal has none, so the capture time
        assert_eq!(change.ts, ts("2026-10-01T10:40:02+02:00"));
        assert_eq!(remove.ts, ts("2026-10-01T15:00:00+02:00"));
        assert_eq!(add.ts, ts("2026-10-01T14:03:30+02:00"));
        // shapes of the fixture lines (hashes differ: other content)
        assert_matches_fixture(
            change,
            "config-change",
            "~/.config/hypr/monitors.conf",
            &["source", "kind", "subject", "zone"],
        );
        assert_matches_fixture(
            add,
            "config-add",
            "~/.config/systemd/user/ollama.service",
            &["source", "kind", "subject", "zone"],
        );
        let h = |e: &Event| (e.meta.hash_from.clone(), e.meta.hash_to.clone());
        let monitors = c.manifest().current.files["~/.config/hypr/monitors.conf"].clone();
        let (from, to) = h(change);
        assert_eq!(to.as_deref(), Some(monitors.as_str()));
        assert_eq!(
            change.detail.as_deref().unwrap(),
            format!("sha256 {} → {}", &from.unwrap()[..8], &monitors[..8])
        );
        assert_eq!(h(remove), (Some(ABC.into()), None));
        assert_eq!(remove.detail.as_deref(), Some("sha256 ba7816bf → —"));
        assert_eq!(h(add).0, None);
        assert!(add.detail.as_deref().unwrap().starts_with("sha256 — → "));
        assert_eq!(add.zone, Some(seldon::model::case::Zone::Red));
        assert_eq!(change.zone, Some(seldon::model::case::Zone::Yellow));

        let third = c.run("2026-10-01T15:10:00+02:00");
        assert!(third.ok && third.events.is_empty(), "{:?}", third.events);
        assert_eq!(c.b.written.len(), 3);
    }

    #[test]
    fn skip_paths_are_never_hashed() {
        let mut c = ConfigBench::new("config-skip");
        c.b.config.redaction.skip_paths = vec![
            "~/.config/hypr/secrets.conf".into(),
            "*.key".into(),
            "~/.config/omarchy/private/".into(),
        ];
        c.file(".config/hypr/hyprland.conf", "source = secrets.conf\n");
        c.file(".config/hypr/secrets.conf", "token=ghp_EXAMPLE\n");
        c.file(".config/omarchy/api.key", "sk-EXAMPLE\n");
        c.file(".config/omarchy/private/notes.txt", "x\n");
        c.run("2026-10-01T10:00:00+02:00");
        c.file(".config/hypr/secrets.conf", "token=ghp_EXAMPLE2\n");
        c.file(".config/omarchy/api.key", "sk-EXAMPLE2\n");
        c.file(".config/omarchy/private/more.txt", "y\n");
        let out = c.run("2026-10-01T10:10:00+02:00");
        assert!(out.ok);
        assert!(out.events.is_empty(), "{:?}", out.events);
        // the files are never listed; the scope keeps the patterns as
        // config.toml has them (WP-069)
        let m = c.manifest();
        let listed =
            serde_json::to_string(&(&m.current.files, &m.current.skipped, &m.stats)).unwrap();
        for needle in [
            "secrets.conf",
            "api.key",
            "private",
            "notes.txt",
            "more.txt",
        ] {
            assert!(
                !listed.contains(needle),
                "{needle} in the manifest:\n{listed}"
            );
        }
        assert!(listed.contains("hyprland.conf"));
        let text = c.manifest_text();
        assert!(
            !text.contains("notes.txt") && !text.contains("more.txt"),
            "{text}"
        );
        assert_eq!(
            m.current.scope.unwrap().skip,
            [
                "*.key",
                "~/.config/hypr/secrets.conf",
                "~/.config/omarchy/private/"
            ]
        );
    }

    #[test]
    fn large_and_binary_files_are_skipped() {
        let mut c = ConfigBench::new("config-large");
        let limit = MAX_FILE_SIZE as usize;
        c.file(".config/hypr/exactly-1mb.conf", vec![b'a'; limit]);
        c.file(".config/hypr/too-large.conf", vec![b'a'; limit + 1]);
        c.file(
            ".config/omarchy/backgrounds/x.png",
            b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR",
        );
        c.file(".config/hypr/grows.conf", "small\n");
        c.run("2026-10-01T10:00:00+02:00");
        let m = c.manifest().current;
        assert!(m.files.contains_key("~/.config/hypr/exactly-1mb.conf"));
        assert!(!m.files.contains_key("~/.config/hypr/too-large.conf"));
        assert!(!m.files.contains_key("~/.config/omarchy/backgrounds/x.png"));
        assert!(m.skipped.contains("~/.config/hypr/too-large.conf"));
        assert!(m.skipped.contains("~/.config/omarchy/backgrounds/x.png"));

        // a file that grows past the limit is not "removed", and a change
        // to a skipped file is not reported
        c.file(".config/hypr/grows.conf", vec![b'b'; limit + 1]);
        c.file(".config/hypr/too-large.conf", vec![b'c'; limit + 2]);
        let out = c.run("2026-10-01T10:10:00+02:00");
        assert!(out.ok);
        assert!(out.events.is_empty(), "{:?}", out.events);
        assert!(
            c.manifest()
                .current
                .skipped
                .contains("~/.config/hypr/grows.conf")
        );
    }

    #[test]
    fn plugins_git_and_special_files_are_excluded() {
        let mut c = ConfigBench::new("config-exclude");
        c.file(".config/omarchy/plugins/jax.seldon/manifest.json", "{}");
        c.file(".config/hypr/.git/config", "[core]\n");
        c.file(".config/hypr/hyprland.conf", "x\n");
        // a symlinked file is followed, a symlinked directory is not
        c.file("dotfiles/bashrc", "abc");
        std::os::unix::fs::symlink(c.b.home("dotfiles/bashrc"), c.b.home(".bashrc")).unwrap();
        c.file("elsewhere/deep.conf", "y\n");
        std::os::unix::fs::symlink(c.b.home("elsewhere"), c.b.home(".config/hypr/linked")).unwrap();
        c.run("2026-10-01T10:00:00+02:00");
        let files: Vec<String> = c.manifest().current.files.into_keys().collect();
        assert_eq!(files, ["~/.bashrc", "~/.config/hypr/hyprland.conf"]);
        assert_eq!(c.manifest().current.files["~/.bashrc"], ABC);

        // a change through the link is a change of ~/.bashrc
        c.file("dotfiles/bashrc", "abcd");
        let out = c.run("2026-10-01T10:10:00+02:00");
        assert_eq!(out.events.len(), 1);
        assert_eq!(out.events[0].subject, "~/.bashrc");
    }

    #[test]
    fn a_failed_write_loses_nothing() {
        let mut c = ConfigBench::new("config-retry");
        c.file(".config/hypr/a.conf", "1\n");
        c.run("2026-10-01T10:00:00+02:00");
        c.file(".config/hypr/a.conf", "2\n");
        // collect, but the ledger write "fails": events dropped, cursor not saved
        let lost = c.collect("2026-10-01T10:10:00+02:00");
        assert_eq!(lost.events.len(), 1);
        let m = c.manifest();
        assert!(m.previous.is_some(), "the caught-up generation is kept");

        let retry = c.run("2026-10-01T10:20:00+02:00");
        assert_eq!(retry.events.len(), 1, "the change is reported again");
        assert_eq!(retry.events[0].meta.hash_to, lost.events[0].meta.hash_to);
        // once caught up, the old generation is dropped
        c.run("2026-10-01T10:30:00+02:00");
        assert_eq!(c.manifest().previous, None);
    }

    #[test]
    fn a_lost_manifest_takes_a_new_baseline() {
        let mut c = ConfigBench::new("config-lost");
        c.file(".config/hypr/a.conf", "1\n");
        c.run("2026-10-01T10:00:00+02:00");
        std::fs::remove_file(Manifest::file(&c.b.dirs)).unwrap();
        c.file(".config/hypr/a.conf", "2\n");
        let out = c.run("2026-10-01T10:10:00+02:00");
        assert!(out.ok);
        assert!(out.events.is_empty());
        assert!(out.message.unwrap().contains("new baseline"));
        c.file(".config/hypr/a.conf", "3\n");
        assert_eq!(c.run("2026-10-01T10:20:00+02:00").events.len(), 1);
    }

    /// Kinds and subjects of `out`'s events.
    fn kinds(out: &Outcome) -> Vec<(Kind, &str)> {
        out.events
            .iter()
            .map(|e| (e.kind, e.subject.as_str()))
            .collect()
    }

    #[test]
    fn a_narrowed_scope_is_no_removal() {
        let mut c = ConfigBench::new("config-narrow");
        c.file(".config/hypr/hyprland.conf", "a\n");
        c.file(".config/omarchy/shell.json", "{}\n");
        c.file(".config/omarchy/extensions/menu.jsonc", "{}\n");
        c.file(".config/omarchy/backgrounds/x.png", b"\x89PNG\0");
        c.file(".config/systemd/user/a.service", "[Unit]\n");
        c.file(".bashrc", "export A=1\n");
        c.run("2026-10-01T10:00:00+02:00");

        // a watch path removed: its files left the scope, nothing was deleted
        c.b.config.watch_paths = vec![
            "~/.config/hypr".into(),
            "~/.config/systemd".into(),
            "~/.bashrc".into(),
        ];
        std::fs::remove_file(c.b.home(".config/hypr/hyprland.conf")).unwrap();
        let out = c.run("2026-10-01T10:10:00+02:00");
        assert!(out.ok);
        assert_eq!(
            kinds(&out),
            [(Kind::ConfigRemove, "~/.config/hypr/hyprland.conf")],
            "only the real deletion"
        );
        assert_eq!(
            out.message.as_deref(),
            Some("watch scope changed: 3 file(s) left it, 0 entered it; no events for them")
        );
        let scope = c.manifest().current.scope.unwrap();
        assert_eq!(
            scope.watch,
            ["~/.bashrc", "~/.config/hypr", "~/.config/systemd"]
        );
        assert_eq!(scope.exclude, ["~/.config/omarchy/plugins"]);

        // a skipPaths pattern added: no removal, the file is not named
        c.b.config.redaction.skip_paths = vec!["*.service".into()];
        let out = c.run("2026-10-01T10:20:00+02:00");
        assert!(out.events.is_empty(), "{:?}", out.events);
        assert_eq!(
            out.message.as_deref(),
            Some("watch scope changed: 1 file(s) left it, 0 entered it; no events for them")
        );

        // the same scope again: no notice
        let out = c.run("2026-10-01T10:30:00+02:00");
        assert!(out.events.is_empty() && out.message.is_none(), "{out:?}");
        assert_eq!(c.b.written.len(), 1);
    }

    #[test]
    fn a_widened_scope_is_no_flood() {
        let mut c = ConfigBench::new("config-widen");
        c.b.config.watch_paths = vec!["~/.config/hypr".into()];
        c.b.config.redaction.skip_paths = vec!["secret.conf".into()];
        c.file(".config/hypr/hyprland.conf", "a\n");
        c.file(".config/hypr/secret.conf", "token=1\n");
        for i in 0..20 {
            c.file(&format!(".config/nvim/lua/plugin{i}.lua"), "return {}\n");
        }
        c.file(".config/nvim/big.bin", b"\0\0");
        c.run("2026-10-01T10:00:00+02:00");

        // a watch path added, a pattern taken out: what enters is the
        // new baseline; a file created under a path watched before is new
        c.b.config.watch_paths.push("~/.config/nvim".into());
        c.b.config.redaction.skip_paths.clear();
        c.file(".config/hypr/new.conf", "b\n");
        let out = c.run("2026-10-01T10:10:00+02:00");
        assert_eq!(kinds(&out), [(Kind::ConfigAdd, "~/.config/hypr/new.conf")]);
        assert_eq!(
            out.message.as_deref(),
            Some("watch scope changed: 0 file(s) left it, 22 entered it; no events for them")
        );
        // from now on they are watched like the others
        c.file(".config/nvim/lua/plugin3.lua", "return { x = 1 }\n");
        c.file(".config/hypr/secret.conf", "token=2\n");
        let out = c.run("2026-10-01T10:20:00+02:00");
        assert_eq!(
            kinds(&out),
            [
                (Kind::ConfigChange, "~/.config/hypr/secret.conf"),
                (Kind::ConfigChange, "~/.config/nvim/lua/plugin3.lua"),
            ]
        );
        assert_eq!(out.message, None);
    }

    #[test]
    fn a_manifest_without_a_scope_still_drops_what_left_it() {
        // a manifest written before WP-069: the old scope is unknown, so
        // what left the scope is dropped, and nothing counts as entered
        let mut c = ConfigBench::new("config-old-manifest");
        c.file(".config/hypr/hyprland.conf", "a\n");
        c.file(".config/omarchy/shell.json", "{}\n");
        c.run("2026-10-01T10:00:00+02:00");
        let mut m: Value = serde_json::from_str(&c.manifest_text()).unwrap();
        m.as_object_mut().unwrap().remove("scope");
        m.as_object_mut().unwrap().remove("stats");
        write(&Manifest::file(&c.b.dirs), m.to_string());

        c.b.config.watch_paths = vec!["~/.config/hypr".into(), "~/.config/waybar".into()];
        c.file(".config/waybar/config.jsonc", "{}\n");
        let out = c.run("2026-10-01T10:10:00+02:00");
        assert_eq!(
            kinds(&out),
            [(Kind::ConfigAdd, "~/.config/waybar/config.jsonc")]
        );
        assert_eq!(
            out.message.as_deref(),
            Some("watch scope changed: 1 file(s) left it, 0 entered it; no events for them")
        );
    }

    #[test]
    fn default_skip_paths_skip_plugin_state() {
        // ConfigBench keeps Config::default()'s [redaction] skipPaths
        let mut c = ConfigBench::new("config-default-skip");
        c.file(".config/omarchy/example-timer/history.json", "[1]\n");
        c.file(".config/omarchy/example-timer/settings.json", "{}\n");
        c.run("2026-10-01T10:00:00+02:00");
        c.file(".config/omarchy/example-timer/history.json", "[1,2]\n");
        c.file(".config/omarchy/example-timer/settings.json", "{\"a\":1}\n");
        let out = c.run("2026-10-01T10:10:00+02:00");
        assert_eq!(
            kinds(&out),
            [(
                Kind::ConfigChange,
                "~/.config/omarchy/example-timer/settings.json"
            )]
        );
        let m = c.manifest();
        assert!(
            !m.current
                .files
                .contains_key("~/.config/omarchy/example-timer/history.json")
        );
    }

    #[test]
    fn an_unchanged_file_is_not_read_again() {
        let mut c = ConfigBench::new("config-stat");
        let old = "2026-09-01T08:00:00+02:00";
        for f in ["a", "b", "d"] {
            c.file(&format!(".config/hypr/{f}.conf"), format!("{f}{f}{f}{f}\n"));
            c.touch(&format!(".config/hypr/{f}.conf"), old);
        }
        c.run("2026-10-01T10:00:00+02:00");
        assert_eq!(c.manifest().stats.len(), 3);

        // a stored hash no file has: while size, times and inode match, the
        // walk takes it as it is, so the file was not read
        let fake = "f".repeat(64);
        let mut m: Value = serde_json::from_str(&c.manifest_text()).unwrap();
        m["files"]["~/.config/hypr/a.conf"] = json!(fake);
        write(&Manifest::file(&c.b.dirs), m.to_string());
        let out = c.run("2026-10-01T10:10:00+02:00");
        assert!(out.events.is_empty(), "{:?}", out.events);
        assert_eq!(c.manifest().current.files["~/.config/hypr/a.conf"], fake);

        // the change time moves on (coarse clocks: let a tick pass)
        std::thread::sleep(std::time::Duration::from_millis(20));
        // an edit of the same size in place, the mtime put back (`touch
        // -r`): the change time differs, so it is read again
        c.file(".config/hypr/a.conf", "AAAA\n");
        c.touch(".config/hypr/a.conf", old);
        // the same size and a new mtime: read again
        c.file(".config/hypr/b.conf", "BBBB\n");
        c.touch(".config/hypr/b.conf", "2026-09-01T08:00:01+02:00");
        let out = c.run("2026-10-01T10:20:00+02:00");
        assert_eq!(
            kinds(&out),
            [
                (Kind::ConfigChange, "~/.config/hypr/a.conf"),
                (Kind::ConfigChange, "~/.config/hypr/b.conf"),
            ]
        );
        assert_eq!(out.events[0].meta.hash_from.as_deref(), Some(fake.as_str()));
        assert_ne!(c.manifest().current.files["~/.config/hypr/a.conf"], fake);

        // a file written just now is read at every walk until it is older
        // than the walk by two seconds (a write in the same tick)
        c.file(".config/hypr/c.conf", "cccc\n");
        let out = c.run("2026-10-01T10:30:00+02:00");
        assert_eq!(kinds(&out), [(Kind::ConfigAdd, "~/.config/hypr/c.conf")]);
        assert!(!c.manifest().stats.contains_key("~/.config/hypr/c.conf"));
        c.file(".config/hypr/c.conf", "CCCC\n");
        let out = c.run("2026-10-01T10:40:00+02:00");
        assert_eq!(kinds(&out), [(Kind::ConfigChange, "~/.config/hypr/c.conf")]);
    }

    #[test]
    fn a_name_with_control_characters_is_skipped_with_a_warning() {
        let mut c = ConfigBench::new("config-control");
        c.file(".config/hypr/ok.conf", "a\n");
        c.file(".config/hypr/evil\x1b[2Jname.conf", "b\n");
        c.file(".config/hypr/tab\there/x.conf", "c\n");
        let out = c.run("2026-10-01T10:00:00+02:00");
        assert!(out.ok);
        assert_eq!(
            out.message.as_deref(),
            Some(
                "2 file(s) not watched: the name holds a control character or is longer than 512 characters"
            )
        );
        let text = c.manifest_text();
        assert!(!text.contains("evil") && !text.contains("tab"), "{text}");
        c.file(".config/hypr/evil\x1b[2Jname.conf", "changed\n");
        let out = c.run("2026-10-01T10:10:00+02:00");
        assert!(out.events.is_empty(), "{:?}", out.events);
    }

    #[test]
    fn missing_watch_paths_are_fine() {
        let mut c = ConfigBench::new("config-empty");
        let out = c.run("2026-10-01T10:00:00+02:00");
        assert!(out.ok);
        assert!(c.manifest().current.files.is_empty());
    }
}

/// `seldon capture --source plugins,theme,config` end to end, with the env
/// overrides pointing at fixtures and a temp home.
mod capture {
    use super::*;
    use common::{Env, Snapper, json, stderr};

    struct Capture {
        env: Env,
        omarchy: PathBuf,
        list: PathBuf,
        theme: PathBuf,
        plugins_dir: PathBuf,
    }

    impl Capture {
        fn new() -> Self {
            let env = Env::new(Snapper::NoPermissions);
            let root = env.tmp.path().to_path_buf();
            // no first capture: the test's own first capture is the baseline
            let out = env.seldon(&[
                "init",
                "--non-interactive",
                "--no-git",
                "--no-capture",
                "--path",
                root.join("logbook").to_str().unwrap(),
            ]);
            assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
            let list = root.join("fake/list.json");
            let omarchy = root.join("fake/omarchy");
            script(
                &omarchy,
                &format!(
                    "case \"$2\" in list) {};; *) exit 1;; esac",
                    print_file(&list)
                ),
            );
            let plugins_dir = root.join("fake/plugins");
            write(
                &plugins_dir.join("io.github.example.tyme/manifest.json"),
                r#"{"id":"io.github.example.tyme","version":"1.1.0"}"#,
            );
            Capture {
                theme: root.join("fake/theme.name"),
                env,
                omarchy,
                list,
                plugins_dir,
            }
        }

        fn capture(&self) -> Value {
            let out = self
                .env
                .command(&["capture", "--source", "plugins,theme,config", "--json"])
                .env("SELDON_OMARCHY", &self.omarchy)
                .env("SELDON_OMARCHY_PLUGINS_DIR", &self.plugins_dir)
                .env("SELDON_THEME_FILE", &self.theme)
                .output()
                .unwrap();
            assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
            json(&out)
        }

        fn ledger(&self) -> Vec<Event> {
            Ledger::at(
                self.env.tmp.path().join("logbook/ledger"),
                Redactor::builtin(),
            )
            .read_all()
            .unwrap()
        }
    }

    #[test]
    fn emits_the_expected_events_once() {
        let c = Capture::new();
        let home = &c.env.home;
        write(
            &c.list,
            std::fs::read(fixture("logs/plugin-list-before.json")).unwrap(),
        );
        write(&c.theme, "kanagawa\n");
        write(
            &home.join(".config/hypr/bindings.conf"),
            "bind = SUPER, Q, killactive\n",
        );

        let first = c.capture();
        assert_eq!(first["written"], 0, "{first}");
        for collector in first["collectors"].as_array().unwrap() {
            let ran = ["plugins", "theme", "config"].contains(&collector["name"].as_str().unwrap());
            assert_eq!(collector["ran"], ran, "{collector}");
            assert_eq!(collector["ok"], true, "{collector}");
        }
        assert!(home.join(".local/state/seldon/manifest.json").is_file());

        write(
            &c.list,
            std::fs::read(fixture("logs/plugin-list-after.json")).unwrap(),
        );
        write(&c.theme, "tokyo-night\n");
        write(
            &home.join(".config/hypr/bindings.conf"),
            "bind = SUPER, W, killactive\n",
        );
        let second = c.capture();
        assert_eq!(second["written"], 3, "{second}");
        let mut got: Vec<(String, String, String)> = c
            .ledger()
            .iter()
            .map(|e| {
                assert_schema_valid(e);
                (e.source.to_string(), e.kind.to_string(), e.subject.clone())
            })
            .collect();
        got.sort();
        assert_eq!(
            got,
            [
                (
                    "config".into(),
                    "config-change".into(),
                    "~/.config/hypr/bindings.conf".into()
                ),
                (
                    "plugins".into(),
                    "plugin-add".into(),
                    "io.github.example.tyme".into()
                ),
                ("theme".into(), "theme-set".into(), "tokyo-night".into()),
            ]
        );
        let lines: Vec<String> = c.ledger().iter().map(Event::to_line).collect();
        let text = lines.join("\n");
        assert!(
            !text.contains(&*home.to_string_lossy()),
            "no absolute home paths:\n{text}"
        );

        let third = c.capture();
        assert_eq!(third["written"], 0, "{third}");
        assert_eq!(c.ledger().len(), 3);
    }

    #[test]
    fn relative_config_paths_do_not_follow_the_working_directory() {
        // the plugin captures from the shell's directory, a hook from the
        // agent's: `logbook = "Logbook"` and `watchPaths = ["dotfiles"]`
        // name the same folders from both (WP-069)
        let env = Env::new(Snapper::NoPermissions);
        let out = env.seldon(&[
            "init",
            "--non-interactive",
            "--no-git",
            "--no-capture",
            "--path",
            env.home.join("Logbook").to_str().unwrap(),
        ]);
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        // the first run says where noisy files go (F-250)
        assert!(
            common::stdout(&out)
                .contains("list noisy or secret files in its [redaction] skipPaths"),
            "{}",
            common::stdout(&out)
        );
        let mut config = Config::load(&env.config_file()).unwrap().unwrap();
        assert_eq!(
            config.redaction.skip_paths,
            seldon::config::DEFAULT_SKIP_PATHS,
            "init writes the default list into the file"
        );
        config.logbook = Some(PathBuf::from("Logbook"));
        config.watch_paths = vec!["dotfiles".into(), "~/.config/omarchy".into()];
        // what `init` wrote before WP-069: an empty list is the defaults
        config.redaction.skip_paths = Vec::new();
        config.save(&env.config_file()).unwrap();
        assert!(common::read(&env.config_file()).contains("skipPaths = []"));
        write(&env.home.join("dotfiles/app.conf"), "a = 1\n");
        write(
            &env.home.join(".config/omarchy/example-timer/history.json"),
            "[1]\n",
        );
        let (a, b) = (env.tmp.path().join("a"), env.home.join(".config"));
        std::fs::create_dir_all(&a).unwrap();
        let capture = |cwd: &Path| {
            let out = env
                .command(&["capture", "--source", "config", "--json"])
                .current_dir(cwd)
                .output()
                .unwrap();
            assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
            json(&out)
        };
        let first = capture(&a);
        assert_eq!(
            first["logbook"].as_str(),
            Some(env.home.join("Logbook").to_str().unwrap())
        );
        assert_eq!(first["written"], 0);
        let other = capture(&b);
        assert_eq!(other["written"], 0, "{other}");
        assert_eq!(other["logbook"], first["logbook"]);
        write(&env.home.join("dotfiles/app.conf"), "a = 2\n");
        assert_eq!(capture(&b)["written"], 1);
        assert_eq!(capture(&a)["written"], 0);
        let manifest: Manifest = serde_json::from_slice(
            &std::fs::read(env.home.join(".local/state/seldon/manifest.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(
            manifest.current.files.keys().collect::<Vec<_>>(),
            ["~/dotfiles/app.conf"]
        );
    }

    #[test]
    fn plugins_degrade_without_the_shell() {
        let c = Capture::new();
        script(
            &c.omarchy,
            "echo 'omarchy-shell is not running' >&2; exit 1",
        );
        write(&c.theme, "kanagawa\n");
        let out = c.capture();
        assert_eq!(out["ok"], false);
        let plugins = out["collectors"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["name"] == "plugins")
            .unwrap();
        assert_eq!(plugins["ok"], false);
        assert!(
            plugins["message"]
                .as_str()
                .unwrap()
                .contains("omarchy-shell is not running")
        );
        let theme = out["collectors"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["name"] == "theme")
            .unwrap();
        assert_eq!(theme["ok"], true);
    }
}

/// `engine/hooks/theme-set.sh`, as `omarchy-hook theme-set <slug>` runs it.
mod hook {
    use super::*;

    fn bash() -> Option<PathBuf> {
        std::env::var("PATH")
            .unwrap_or_default()
            .split(':')
            .map(|dir| Path::new(dir).join("bash"))
            .find(|p| p.is_file())
    }

    /// Runs the hook with `args` and PATH = `bin` only.
    fn run_hook(bin: &Path, args: &[&str]) -> std::process::Output {
        let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("hooks/theme-set.sh");
        std::process::Command::new(bash().expect("bash"))
            .arg(script)
            .args(args)
            .env_clear()
            .env("PATH", bin)
            .output()
            .unwrap()
    }

    #[test]
    fn records_the_slug_silently() {
        if bash().is_none() {
            eprintln!("skipped: no bash on this host");
            return;
        }
        let tmp = TempDir::new("hook");
        let bin = tmp.path().join("bin");
        let argv = tmp.path().join("argv");
        script(
            &bin.join("seldon"),
            &format!(
                "printf '%s\\n' \"$@\" > '{}'; echo noise; echo noise >&2; exit 2",
                argv.display()
            ),
        );
        let out = run_hook(&bin, &["tokyo-night"]);
        assert_eq!(out.status.code(), Some(0));
        assert!(out.stdout.is_empty() && out.stderr.is_empty());
        assert_eq!(
            std::fs::read_to_string(&argv).unwrap(),
            "event\ntheme\ntheme-set\n--subject\ntokyo-night\n"
        );

        // a slug is one argument, never evaluated
        let out = run_hook(&bin, &["$(touch pwned) x"]);
        assert_eq!(out.status.code(), Some(0));
        assert!(
            std::fs::read_to_string(&argv)
                .unwrap()
                .ends_with("--subject\n$(touch pwned) x\n")
        );

        // no slug: nothing runs
        std::fs::remove_file(&argv).unwrap();
        assert_eq!(run_hook(&bin, &[]).status.code(), Some(0));
        assert!(!argv.exists());

        // no seldon on PATH: still exit 0, silent
        std::fs::remove_file(bin.join("seldon")).unwrap();
        let out = run_hook(&bin, &["kanagawa"]);
        assert_eq!(out.status.code(), Some(0));
        assert!(out.stdout.is_empty() && out.stderr.is_empty());
    }
}

/// WP-076: every source of the [`Bench`] lies under its temp dir, never a
/// host path or a program found on the host's PATH.
#[test]
fn the_bench_reads_nothing_of_the_host() {
    let b = Bench::new("hostless");
    let s = &b.sources;
    let paths = [
        s.pacman_log.as_path(),
        &s.pacman_db_lock,
        &s.snapshots,
        s.plugins_dir.as_deref().expect("plugins_dir"),
        s.theme_file.as_deref().expect("theme_file"),
    ];
    let programs = [&s.snapper, &s.omarchy_version, &s.pacman, &s.omarchy].map(Path::new);
    for p in paths.into_iter().chain(programs) {
        assert!(
            p.starts_with(b.tmp.path()),
            "{} is not a scratch path",
            p.display()
        );
    }
}
