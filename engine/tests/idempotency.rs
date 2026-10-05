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
use seldon::redact::REDACTED;
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

    /// Desktop entries whose names differ only in the local part of an
    /// address (`<name>.webapp@example.com`): the ledger holds all of them
    /// under one redacted subject, [`twin`] (WP-093).
    fn twins<const N: usize>(b: &support::Bench, names: [&str; N]) -> [std::path::PathBuf; N] {
        let dir = b.dirs.home.join(".local/share/applications");
        std::fs::create_dir_all(&dir).unwrap();
        names.map(|name| dir.join(format!("Mail ({name}.webapp@example.com).desktop")))
    }

    fn twin() -> String {
        format!("~/.local/share/applications/Mail ({REDACTED}@example.com).desktop")
    }

    /// The desktop entry `Name=<name>`.
    fn entry_text(name: &str) -> String {
        format!("[Desktop Entry]\nName={name}\n")
    }

    /// Writes the entry `name` to `path`; with `mtime`, the file's
    /// modification time is set to it.
    fn entry(path: &Path, name: &str, mtime: Option<&str>) {
        std::fs::write(path, entry_text(name)).unwrap();
        if let Some(t) = mtime {
            let at = std::time::UNIX_EPOCH
                + std::time::Duration::from_secs(support::ts(t).timestamp() as u64);
            let file = std::fs::File::options().write(true).open(path).unwrap();
            file.set_modified(at).unwrap();
        }
    }

    /// The entry names the twin tests write.
    const NAMES: [&str; 6] = ["Alice 1", "Alice 2", "Alice 3", "Bob 1", "Bob 2", "Same"];

    type Step = (Kind, Option<&'static str>, Option<&'static str>);

    /// Each event as (kind, from, to), its hashes read back as entry names;
    /// every event names [`twin`].
    fn steps(events: &[Event]) -> Vec<Step> {
        let name = |h: &Option<String>| {
            h.as_ref().map(|h| {
                *NAMES
                    .iter()
                    .find(|n| seldon::sys::sha256_hex(entry_text(n).as_bytes()) == *h)
                    .unwrap_or_else(|| panic!("no entry has the hash {h}"))
            })
        };
        events
            .iter()
            .map(|e| {
                assert_eq!(e.subject, twin());
                (e.kind, name(&e.meta.hash_from), name(&e.meta.hash_to))
            })
            .collect()
    }

    /// Runs the config collector at `10:00`-style `at` on 2026-10-01.
    fn twin_run(b: &mut support::Bench, at: &str) -> Vec<Step> {
        let out = b.run(&ConfigFiles, &format!("2026-10-01T{at}:00+02:00"));
        assert!(out.ok, "{:?}", out.message);
        steps(&out.events)
    }

    const ADD: Kind = Kind::ConfigAdd;
    const CHANGE: Kind = Kind::ConfigChange;
    const REMOVE: Kind = Kind::ConfigRemove;

    #[test]
    fn twins_after_a_failed_cursor_save_are_told_apart_by_their_hashes() {
        // two files share a subject in the ledger; after a failed cursor
        // save each replayed event must go to its own file, or the file
        // that moved on before the next capture gets a wrong event (WP-103)
        let mut b = support::Bench::new("crash-config-twins");
        let [alice, bob] = twins(&b, ["alice", "bob"]);
        assert_eq!(twin_run(&mut b, "10:00"), []);

        // added; Bob's entry is the older one, so its event comes first
        let before = b.cursors.clone();
        entry(&bob, "Bob 1", Some("2026-10-01T10:05:00+02:00"));
        entry(&alice, "Alice 1", Some("2026-10-01T10:07:00+02:00"));
        assert_eq!(
            twin_run(&mut b, "10:10"),
            [(ADD, None, Some("Alice 1")), (ADD, None, Some("Bob 1"))]
        );
        b.cursors = before; // the cursor save failed
        entry(&alice, "Alice 2", None);
        assert_eq!(
            twin_run(&mut b, "10:20"),
            [(CHANGE, Some("Alice 1"), Some("Alice 2"))]
        );
        assert_eq!(twin_run(&mut b, "10:30"), []);

        // changed; Alice's entry goes back before the next capture
        let before = b.cursors.clone();
        entry(&alice, "Alice 3", None);
        entry(&bob, "Bob 2", None);
        assert_eq!(
            twin_run(&mut b, "10:40"),
            [
                (CHANGE, Some("Alice 2"), Some("Alice 3")),
                (CHANGE, Some("Bob 1"), Some("Bob 2"))
            ]
        );
        b.cursors = before;
        entry(&alice, "Alice 2", None);
        assert_eq!(
            twin_run(&mut b, "10:50"),
            [(CHANGE, Some("Alice 3"), Some("Alice 2"))]
        );
        assert_eq!(twin_run(&mut b, "11:00"), []);

        // removed; Alice's entry comes back before the next capture
        let before = b.cursors.clone();
        std::fs::remove_file(&alice).unwrap();
        std::fs::remove_file(&bob).unwrap();
        assert_eq!(
            twin_run(&mut b, "11:10"),
            [
                (REMOVE, Some("Alice 2"), None),
                (REMOVE, Some("Bob 2"), None)
            ]
        );
        b.cursors = before;
        entry(&alice, "Alice 2", None);
        assert_eq!(twin_run(&mut b, "11:20"), [(ADD, None, Some("Alice 2"))]);
        assert_eq!(twin_run(&mut b, "11:30"), []);
        assert_eq!(b.ledger_events(Source::Config).len(), 9);
    }

    #[test]
    fn twins_with_the_same_content_after_a_failed_cursor_save() {
        // which of several files with the same content an event belongs
        // to cannot be read from the ledger; the events must still cover
        // every file once (WP-103)
        let mut b = support::Bench::new("crash-config-same");
        let [alice, bob, carol] = twins(&b, ["alice", "bob", "carol"]);
        for path in [&alice, &bob, &carol] {
            entry(path, "Same", None);
        }
        assert_eq!(twin_run(&mut b, "10:00"), []);

        // two of three change alike: two events alike, and either fits
        // all three files
        let before = b.cursors.clone();
        entry(&bob, "Bob 1", None);
        entry(&carol, "Bob 1", None);
        assert_eq!(
            twin_run(&mut b, "10:10"),
            [
                (CHANGE, Some("Same"), Some("Bob 1")),
                (CHANGE, Some("Same"), Some("Bob 1"))
            ]
        );
        b.cursors = before;
        assert_eq!(twin_run(&mut b, "10:20"), []);

        // one changes, the save fails, then another changes alike
        let before = b.cursors.clone();
        entry(&bob, "Same", None);
        assert_eq!(
            twin_run(&mut b, "10:30"),
            [(CHANGE, Some("Bob 1"), Some("Same"))]
        );
        b.cursors = before;
        entry(&carol, "Same", None);
        assert_eq!(
            twin_run(&mut b, "10:40"),
            [(CHANGE, Some("Bob 1"), Some("Same"))]
        );
        assert_eq!(twin_run(&mut b, "10:50"), []);
        assert_eq!(b.ledger_events(Source::Config).len(), 4);
    }

    #[test]
    fn twins_with_the_same_content_removed_one_after_the_other() {
        // a removal carries the capture time, which is the next capture's
        // `since`; a ledger dedupe on every capture took the second removal
        // for the first (WP-103)
        let mut b = support::Bench::new("config-same-removed");
        let [alice, bob] = twins(&b, ["alice", "bob"]);
        entry(&alice, "Same", None);
        entry(&bob, "Same", None);
        assert_eq!(twin_run(&mut b, "10:00"), []);
        std::fs::remove_file(&alice).unwrap();
        assert_eq!(twin_run(&mut b, "10:10"), [(REMOVE, Some("Same"), None)]);
        std::fs::remove_file(&bob).unwrap();
        assert_eq!(twin_run(&mut b, "10:20"), [(REMOVE, Some("Same"), None)]);
        assert_eq!(twin_run(&mut b, "10:30"), []);
    }

    #[test]
    fn a_twin_added_and_removed_around_a_failed_cursor_save_is_recorded() {
        // Bob's entry is in neither the cursor's generation nor the scan;
        // the generation the failed capture stored names it (WP-103)
        let mut b = support::Bench::new("crash-config-twin-gone");
        let [alice, bob] = twins(&b, ["alice", "bob"]);
        entry(&alice, "Alice 1", None);
        assert_eq!(twin_run(&mut b, "10:00"), []);
        let before = b.cursors.clone();
        entry(&bob, "Bob 1", None);
        assert_eq!(twin_run(&mut b, "10:10"), [(ADD, None, Some("Bob 1"))]);
        b.cursors = before;
        std::fs::remove_file(&bob).unwrap();
        assert_eq!(twin_run(&mut b, "10:20"), [(REMOVE, Some("Bob 1"), None)]);
        assert_eq!(twin_run(&mut b, "10:30"), []);
    }

    #[test]
    fn twins_after_two_failed_cursor_saves() {
        // the first event fits both files and neither is in the state it
        // leaves; it waits until the other events have their files (WP-103)
        let mut b = support::Bench::new("crash-config-twins-twice");
        let [alice, bob] = twins(&b, ["alice", "bob"]);
        assert_eq!(twin_run(&mut b, "10:00"), []);
        let before = b.cursors.clone();
        entry(&bob, "Bob 1", None);
        assert_eq!(twin_run(&mut b, "10:10"), [(ADD, None, Some("Bob 1"))]);
        b.cursors = before.clone();
        entry(&alice, "Alice 1", None);
        entry(&bob, "Bob 2", None);
        assert_eq!(
            twin_run(&mut b, "10:20"),
            [
                (ADD, None, Some("Alice 1")),
                (CHANGE, Some("Bob 1"), Some("Bob 2"))
            ]
        );
        b.cursors = before;
        assert_eq!(twin_run(&mut b, "10:30"), []);
        assert_eq!(twin_run(&mut b, "10:40"), []);
        assert_eq!(b.ledger_events(Source::Config).len(), 3);
    }

    #[test]
    fn a_step_taken_again_after_two_failed_cursor_saves_is_recorded() {
        // A→B and B→A recorded while the cursor stayed on A: the manifest
        // is back on the cursor's generation, and the next A→B is a new
        // change, not one the ledger holds (WP-103)
        let mut b = support::Bench::new("crash-config-again");
        let path = b.dirs.home.join(".config/hypr/monitors.conf");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let (a, bb) = (
            "monitor=,preferred,auto,1\n",
            "monitor=,preferred,auto,1.25\n",
        );
        std::fs::write(&path, a).unwrap();
        let run = |b: &mut support::Bench, at: &str| {
            b.run(&ConfigFiles, &format!("2026-10-01T{at}:00+02:00"))
                .events
                .len()
        };
        assert_eq!(run(&mut b, "10:00"), 0);
        let before = b.cursors.clone();
        std::fs::write(&path, bb).unwrap();
        assert_eq!(run(&mut b, "10:10"), 1);
        b.cursors = before.clone();
        std::fs::write(&path, a).unwrap();
        assert_eq!(run(&mut b, "10:20"), 1);
        b.cursors = before;
        std::fs::write(&path, bb).unwrap();
        assert_eq!(run(&mut b, "10:30"), 1);
        assert_eq!(run(&mut b, "10:40"), 0);
        assert_eq!(b.ledger_events(Source::Config).len(), 3);
    }

    #[test]
    fn a_file_whose_removal_the_ledger_lost_after_a_failed_cursor_save_is_recorded() {
        // added (cursor save failed), then removed in a capture whose
        // ledger write failed too: the file is in no generation the next
        // capture has, and its unredacted subject names it (WP-073, WP-103)
        let mut b = support::Bench::new("crash-config-lost");
        let path = b.dirs.home.join(".config/hypr/extra.conf");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let run = |b: &mut support::Bench, at: &str| {
            b.run(&ConfigFiles, &format!("2026-10-01T{at}:00+02:00"))
                .events
        };
        assert!(run(&mut b, "10:00").is_empty());
        let before = b.cursors.clone();
        std::fs::write(&path, "x\n").unwrap();
        assert_eq!(run(&mut b, "10:10").len(), 1);
        b.cursors = before.clone();
        let month = b.ledger.month_file("2026-10");
        let ledger = std::fs::read(&month).unwrap();
        std::fs::remove_file(&path).unwrap();
        // another file, so this generation is not the cursor's again
        std::fs::write(path.with_file_name("other.conf"), "y\n").unwrap();
        assert_eq!(run(&mut b, "10:20").len(), 2);
        std::fs::write(&month, ledger).unwrap(); // the ledger write failed
        b.cursors = before;
        let lost: Vec<(Kind, String)> = run(&mut b, "10:30")
            .into_iter()
            .map(|e| (e.kind, e.subject))
            .collect();
        assert_eq!(
            lost,
            [
                (Kind::ConfigRemove, "~/.config/hypr/extra.conf".into()),
                (Kind::ConfigAdd, "~/.config/hypr/other.conf".into())
            ]
        );
        assert!(run(&mut b, "10:40").is_empty());
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
        // deletions: nothing is written again, even from the old baseline;
        // only the state reset is recorded, once (WP-081)
        let cursors = cli.env.home.join(".local/state/seldon/cursors.json");
        std::fs::remove_file(&cursors).unwrap();
        let lost = cli.capture(&["--since", FIXTURE_CREATED]);
        assert_eq!(lost["written"], 1, "{lost}");
        let reset = cli.ledger().pop().unwrap();
        assert_eq!(
            (reset.source, reset.kind, reset.subject.as_str()),
            (Source::Seldon, Kind::Note, "state-reset")
        );
        assert_eq!(reset.meta.extra["sources"], "snapper,pacman,omarchy");
        assert_eq!(reset.meta.extra["files"], "cursors");
        assert_eq!(cli.capture(&[])["written"], 0, "the reset is recorded once");

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

/// WP-081: a collector that lost its state although the ledger holds its
/// events leaves one `seldon` note `state-reset`, a warning, and nothing
/// the next time.
mod state_reset {
    use super::*;

    fn resets(cli: &Cli) -> Vec<Event> {
        cli.ledger()
            .into_iter()
            .filter(|e| e.source == Source::Seldon && e.subject == "state-reset")
            .collect()
    }

    fn state(cli: &Cli) -> std::path::PathBuf {
        cli.env.home.join(".local/state/seldon")
    }

    /// WP-083: the sources (comma list) of doctor's "the next capture will
    /// record a state reset" row, `None` without one. Asked before a
    /// capture, it must name what that capture's note names.
    fn predicted(cli: &Cli) -> Option<String> {
        const ROW: &str = "the next capture will record a state reset for ";
        let doctor = common::json(&cli.run(&["doctor", "--json"]));
        let rows: Vec<&str> = doctor["checks"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|c| c["name"] == "state")
            .filter_map(|c| c["message"].as_str()?.strip_prefix(ROW))
            .collect();
        assert!(rows.len() <= 1, "{doctor}");
        let sources = rows.first()?.split(':').next().unwrap();
        Some(sources.replace(", ", ","))
    }

    /// WP-091: doctor's row for collectors whose baseline waits
    /// (`pendingBaseline`): their names (comma list) and the row, `None`
    /// without one. Its names are what their next successful run records.
    fn waiting(cli: &Cli) -> Option<(String, serde_json::Value)> {
        const ROW: &str = " degraded or not run since a state reset (";
        let doctor = common::json(&cli.run(&["doctor", "--json"]));
        let rows: Vec<&serde_json::Value> = doctor["checks"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|c| c["name"] == "state" && c["message"].as_str().unwrap().contains(ROW))
            .collect();
        assert!(rows.len() <= 1, "{doctor}");
        let row = rows.first()?;
        assert_eq!(row["status"], "degraded", "{row}");
        let names = row["message"].as_str().unwrap().split(ROW).next().unwrap();
        Some((names.replace(", ", ","), (*row).clone()))
    }

    /// `capture --source config --json`.
    fn capture_config(cli: &Cli) -> serde_json::Value {
        let out = cli.run(&["capture", "--source", "config", "--json"]);
        assert_eq!(out.status.code(), Some(0), "{}", common::stderr(&out));
        common::json(&out)
    }

    /// A config file under a watch path, recorded once as a change.
    fn config_event(cli: &Cli) {
        let conf = cli.env.home.join(".config/hypr/hyprland.conf");
        std::fs::create_dir_all(conf.parent().unwrap()).unwrap();
        std::fs::write(&conf, "a = 1\n").unwrap();
        assert_eq!(capture_config(cli)["written"], 0);
        std::fs::write(&conf, "a = 2\n").unwrap();
        assert_eq!(capture_config(cli)["written"], 1);
    }

    #[test]
    fn a_lost_state_directory_is_recorded_once() {
        let cli = Cli::new();
        assert_eq!(predicted(&cli), None, "a fresh logbook");
        let first = cli.capture(&["--since", FIXTURE_CREATED]);
        assert!(first["written"].as_u64().unwrap() > 0, "{first}");
        assert!(resets(&cli).is_empty(), "the first capture is no reset");
        assert_eq!(first["warnings"], serde_json::json!([]));

        assert_eq!(predicted(&cli), None, "cursors of every collector here");
        std::fs::remove_dir_all(state(&cli)).unwrap();
        assert_eq!(predicted(&cli).as_deref(), Some("snapper,pacman"));
        let out = cli.run(&["capture", "--all", "--since", "2026-09-15T00:00:00+02:00"]);
        assert_eq!(out.status.code(), Some(0), "{}", common::stderr(&out));
        let human = common::stdout(&out);
        assert!(
            human.contains(
                "\nwarning: state reset recorded: snapper, pacman took a new baseline because ~/.local/state/seldon was missing, unreadable or bound to another logbook"
            ),
            "{human}"
        );
        assert!(
            human.contains("(user guide: Back up and restore the state directory)"),
            "{human}"
        );
        let reset = resets(&cli);
        assert_eq!(reset.len(), 1, "{reset:?}");
        let r = &reset[0];
        assert_eq!((r.kind, r.actor.as_str()), (Kind::Note, "system"));
        assert_eq!(r.meta.extra["sources"], "snapper,pacman");
        assert_eq!(r.meta.extra["files"], "cursors");
        let detail = r.detail.as_deref().unwrap();
        assert!(
            detail.starts_with(&format!(
                "state directory missing, unreadable or bound to another logbook: new baseline for snapper (cursors), pacman (cursors) at 2026-09-15T00:00:00+02:00, recorded {}; ",
                r.ts.to_rfc3339()
            )),
            "{detail}"
        );
        cli.ledger().iter().for_each(assert_schema_valid);
        assert_eq!(predicted(&cli), None, "the WP-081 row took over");

        let again = cli.capture(&[]);
        assert_eq!(again["written"], 0, "{again}");
        assert_eq!(again["warnings"], serde_json::json!([]));
        assert_eq!(resets(&cli).len(), 1);
    }

    #[test]
    fn a_collector_without_events_in_the_ledger_has_no_reset() {
        let cli = Cli::new();
        cli.capture(&["--since", FIXTURE_CREATED]);
        // theme and plugins took their baseline silently: no events of theirs
        let file = state(&cli).join("cursors.json");
        let mut cursors: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
        // cursors that do not read (an entry without one is a collector
        // that never ran successfully here: no loss)
        for name in ["theme", "plugins"] {
            cursors["collectors"][name]["cursor"] = serde_json::json!(7);
        }
        std::fs::write(&file, cursors.to_string()).unwrap();
        assert_eq!(predicted(&cli), None);
        let out = cli.capture(&[]);
        assert_eq!(out["written"], 0, "{out}");
        assert!(resets(&cli).is_empty());
    }

    #[test]
    fn plugins_and_theme_with_events_have_a_reset() {
        let cli = Cli::new();
        cli.capture(&[]);
        std::fs::write(cli.env.tmp.path().join("theme.name"), "tokyo-night\n").unwrap();
        cli.stub(
            "omarchy",
            &format!(
                "[ \"$2\" = list ] && exec /bin/cat '{}'; exit 1",
                fixture("logs/plugin-list-after.json").display()
            ),
        );
        let changed = cli.capture(&[]);
        let ran = |name: &str| {
            let c = changed["collectors"].as_array().unwrap();
            c.iter().find(|c| c["name"] == name).unwrap()["events"].clone()
        };
        assert!(ran("theme").as_u64().unwrap() > 0, "{changed}");
        assert!(ran("plugins").as_u64().unwrap() > 0, "{changed}");
        assert_eq!(predicted(&cli), None, "their cursors read");
        let file = state(&cli).join("cursors.json");
        let mut cursors: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
        // cursors that do not read (an entry without one is a collector
        // that never ran successfully here: no loss)
        for name in ["theme", "plugins"] {
            cursors["collectors"][name]["cursor"] = serde_json::json!(7);
        }
        std::fs::write(&file, cursors.to_string()).unwrap();
        assert_eq!(predicted(&cli).as_deref(), Some("plugins,theme"));
        assert_eq!(cli.capture(&[])["written"], 1);
        let reset = resets(&cli);
        assert_eq!(reset.len(), 1);
        assert_eq!(reset[0].meta.extra["sources"], "plugins,theme");
        assert_eq!(predicted(&cli), None);
        assert_eq!(cli.capture(&[])["written"], 0);
    }

    /// Review F1: the theme hook wrote a `theme-set` while the theme
    /// collector was degraded (no theme file yet): its first successful run
    /// is a baseline, not a loss.
    #[test]
    fn a_first_successful_run_after_a_degraded_init_is_no_reset() {
        let env = common::Env::new(common::Snapper::Missing);
        let log = env.tmp.path().join("pkg.log");
        std::fs::write(&log, "").unwrap();
        let run = |args: &[&str]| {
            let out = env
                .command(args)
                .env("SELDON_PACMAN_LOG", &log)
                .env("SELDON_PACMAN_DB_LOCK", env.tmp.path().join("no-db.lck"))
                .env("SELDON_OMARCHY", env.tmp.path().join("no-omarchy"))
                .output()
                .unwrap();
            assert_eq!(
                out.status.code(),
                Some(0),
                "{args:?}: {}",
                common::stderr(&out)
            );
            out
        };
        let root = env.tmp.path().join("logbook");
        let init = run(&[
            "init",
            "--non-interactive",
            "--no-git",
            "--path",
            root.to_str().unwrap(),
            "--json",
        ]);
        let init = common::json(&init);
        let theme = init["capture"]["collectors"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["name"] == "theme")
            .unwrap();
        assert_eq!(theme["ok"], false, "no theme file yet: {theme}");
        run(&["event", "theme", "theme-set", "--subject", "tokyo-night"]);
        let file = env.home.join(".local/state/omarchy/current/theme.name");
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(&file, "tokyo-night\n").unwrap();
        let state_ok = || {
            let doctor = common::json(&run(&["doctor", "--json"]));
            let rows = doctor["checks"].as_array().unwrap();
            assert!(
                rows.iter()
                    .all(|c| c["name"] != "state" || c["status"] == "ok"),
                "{doctor}"
            );
        };
        // WP-083: no prediction either
        state_ok();
        let out = common::json(&run(&["capture", "--json"]));
        assert_eq!(out["warnings"], serde_json::json!([]), "{out}");
        let ledger = seldon::ledger::Ledger::at(root.join("ledger"), Default::default());
        let notes: Vec<Event> = ledger
            .read_all()
            .unwrap()
            .into_iter()
            // WP-091: and no note on the theme collector's change
            .filter(|e| e.source == Source::Seldon && e.kind == Kind::Note)
            .collect();
        assert!(notes.is_empty(), "{notes:?}");
        state_ok();
    }

    /// Review F1: a collector disabled since the first capture and enabled
    /// later takes its first baseline without a note.
    #[test]
    fn a_collector_enabled_later_is_no_reset() {
        let cli = Cli::new();
        let config = cli.env.config_file();
        let mut toml: toml::Table = std::fs::read_to_string(&config).unwrap().parse().unwrap();
        let mut off = toml::Table::new();
        off.insert("theme".into(), toml::Value::Boolean(false));
        toml.insert("collectors".into(), toml::Value::Table(off));
        std::fs::write(&config, toml.to_string()).unwrap();
        cli.capture(&["--since", FIXTURE_CREATED]);
        let out = cli.run(&["event", "theme", "theme-set", "--subject", "tokyo-night"]);
        assert_eq!(out.status.code(), Some(0), "{}", common::stderr(&out));
        toml.remove("collectors");
        std::fs::write(&config, toml.to_string()).unwrap();
        assert_eq!(predicted(&cli), None);
        let out = cli.capture(&[]);
        assert_eq!(out["written"], 0, "{out}");
        assert!(resets(&cli).is_empty());
    }

    /// Review F2: cursors bound to another logbook are no backup to restore;
    /// the note says `logbook`, and the warning and doctor say so.
    #[test]
    fn state_of_another_logbook_is_a_reset_without_a_restore() {
        let cli = Cli::new();
        cli.capture(&["--since", FIXTURE_CREATED]);
        let other = cli.env.tmp.path().join("other");
        let out = cli.env.seldon(&[
            "init",
            "--non-interactive",
            "--no-git",
            "--no-capture",
            "--path",
            other.to_str().unwrap(),
        ]);
        assert_eq!(out.status.code(), Some(0), "{}", common::stderr(&out));
        let out = cli.run(&[
            "capture",
            "--all",
            "--json",
            "--logbook",
            other.to_str().unwrap(),
        ]);
        assert_eq!(out.status.code(), Some(0), "{}", common::stderr(&out));
        assert_eq!(
            common::json(&out)["warnings"],
            serde_json::json!([]),
            "no events there"
        );

        assert_eq!(predicted(&cli).as_deref(), Some("snapper,pacman"));
        let doctor = common::json(&cli.run(&["doctor", "--json"]));
        let row = doctor["checks"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["name"] == "state" && c["status"] == "degraded")
            .unwrap_or_else(|| panic!("{doctor}"))
            .clone();
        assert!(
            row["message"].as_str().unwrap().ends_with(
                ": cursors in ~/.local/state/seldon bound to another logbook, so changes made since the last capture may not be recorded"
            ),
            "{row}"
        );
        assert!(
            row["fix"]
                .as_str()
                .unwrap()
                .starts_with("nothing to restore: "),
            "{row}"
        );

        let back = cli.capture(&[]);
        assert_eq!(back["written"], 1, "{back}");
        assert_eq!(predicted(&cli), None);
        let reset = resets(&cli);
        assert_eq!(reset.len(), 1);
        assert_eq!(reset[0].meta.extra["files"], "logbook");
        assert_eq!(reset[0].meta.extra["sources"], "snapper,pacman");
        let detail = reset[0].detail.as_deref().unwrap();
        assert!(
            detail.contains("snapper (logbook), pacman (logbook)"),
            "{detail}"
        );
        let warning = back["warnings"][0].as_str().unwrap();
        assert!(
            warning.ends_with(
                "Nothing can be restored: the state belonged to another logbook path, and the new baseline is this logbook's (user guide: Moving or copying the logbook)"
            ),
            "{warning}"
        );
        let doctor = common::json(&cli.run(&["doctor", "--json"]));
        let row = doctor["checks"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["name"] == "state" && c["status"] == "degraded")
            .unwrap_or_else(|| panic!("{doctor}"))
            .clone();
        assert!(
            row["message"]
                .as_str()
                .unwrap()
                .contains("(the state in ~/.local/state/seldon belonged to another logbook)"),
            "{row}"
        );
        assert!(
            row["fix"]
                .as_str()
                .unwrap()
                .starts_with("nothing to restore: "),
            "{row}"
        );
    }

    /// WP-083: doctor's prediction asks each collector whether its saved
    /// cursor reads: every collector's own cursor does, a stray value not.
    #[test]
    fn every_collector_reads_its_own_cursor() {
        let cli = Cli::new();
        let out = cli.capture(&["--since", FIXTURE_CREATED]);
        let file = state(&cli).join("cursors.json");
        let cursors: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
        for c in REGISTRY {
            let saved = &cursors["collectors"][c.name()]["cursor"];
            assert!(!saved.is_null(), "{}: {out}", c.name());
            assert!(c.cursor_reads(saved), "{}: {saved}", c.name());
            for stray in [serde_json::json!("not a cursor"), serde_json::json!(7)] {
                assert!(!c.cursor_reads(&stray), "{}: {stray}", c.name());
            }
        }
    }

    /// WP-083: doctor warns while a restore still prevents the gap; after
    /// the restore there is nothing to warn about and nothing to record.
    #[test]
    fn a_restore_before_the_capture_prevents_the_reset() {
        let cli = Cli::new();
        cli.capture(&["--since", FIXTURE_CREATED]);
        let backup = cli.env.tmp.path().join("state-backup");
        std::fs::rename(state(&cli), &backup).unwrap();
        assert_eq!(predicted(&cli).as_deref(), Some("snapper,pacman"));
        std::fs::rename(&backup, state(&cli)).unwrap();
        assert_eq!(predicted(&cli), None);
        let out = cli.capture(&[]);
        assert_eq!(out["written"], 0, "{out}");
        assert!(resets(&cli).is_empty());
    }

    /// Review F3 (R1): only a run of the config collector reads and moves
    /// a corrupt `owned.json`.
    #[test]
    fn a_corrupt_owned_file_waits_for_the_config_collector() {
        let cli = Cli::new();
        config_event(&cli);
        let owned = state(&cli).join("owned.json");
        std::fs::write(&owned, "[").unwrap();
        let out = cli.run(&["capture", "--source", "pacman", "--json"]);
        assert_eq!(out.status.code(), Some(0), "{}", common::stderr(&out));
        assert_eq!(common::json(&out)["warnings"], serde_json::json!([]));
        assert!(owned.is_file());
        assert!(resets(&cli).is_empty());
    }

    /// WP-088: what `name`'s entry in `cursors.json` waits for
    /// (`pendingBaseline`: `cursors` or `logbook`, written only while set).
    fn pending(cli: &Cli, name: &str) -> Option<String> {
        let text = std::fs::read_to_string(state(cli).join("cursors.json")).unwrap();
        let cursors: serde_json::Value = serde_json::from_str(&text).unwrap();
        let entry = &cursors["collectors"][name];
        assert!(entry.is_object(), "{name}: {cursors}");
        let mark = entry.get("pendingBaseline")?;
        Some(
            mark.as_str()
                .unwrap_or_else(|| panic!("{cursors}"))
                .to_string(),
        )
    }

    /// WP-088: a collector that degrades in the capture that records a
    /// state reset takes no baseline then; its first successful run
    /// records its gap, once.
    #[test]
    fn a_collector_degraded_in_the_reset_records_its_gap_when_it_runs() {
        let cli = Cli::new();
        cli.capture(&["--since", FIXTURE_CREATED]);
        std::fs::remove_dir_all(state(&cli)).unwrap();
        cli.stub_snapper_no_permissions();
        // doctor cannot know that snapper will degrade
        assert_eq!(predicted(&cli).as_deref(), Some("snapper,pacman"));
        let out = cli.capture(&[]);
        assert_eq!(out["ok"], false, "{out}");
        let reset = resets(&cli);
        assert_eq!(reset.len(), 1, "{reset:?}");
        assert_eq!(reset[0].meta.extra["sources"], "pacman");
        assert_eq!(pending(&cli, "snapper").as_deref(), Some("cursors"));
        assert_eq!(pending(&cli, "pacman"), None, "it took its baseline");

        // degraded again, or not run: it keeps waiting, nothing recorded
        assert_eq!(predicted(&cli), None);
        let (names, row) = waiting(&cli).unwrap();
        assert_eq!(names, "snapper");
        assert_eq!(
            row["message"],
            "snapper degraded or not run since a state reset (cursors missing or unreadable in ~/.local/state/seldon); its next successful capture records the gap, so changes made in between may not be recorded"
        );
        assert_eq!(
            row["fix"],
            "run seldon capture --source snapper once it can run (a degraded collector: the collectors row's fix first)"
        );
        assert_eq!(cli.capture(&[])["written"], 0);
        assert_eq!(pending(&cli, "snapper").as_deref(), Some("cursors"));
        cli.stub_snapper(&fixture("logs/snapper-before.json"));
        let out = cli.run(&["capture", "--source", "pacman", "--json"]);
        assert_eq!(common::json(&out)["written"], 0);
        assert_eq!(pending(&cli, "snapper").as_deref(), Some("cursors"));

        // the first successful run records the gap and clears the mark
        let out = cli.capture(&[]);
        assert_eq!(out["written"], 2, "the note and, WP-091, access: {out}");
        assert_eq!(access(&cli).len(), 1);
        let warnings = out["warnings"].as_array().unwrap();
        assert_eq!(warnings.len(), 1, "{out}");
        assert!(
            warnings[0]
                .as_str()
                .unwrap()
                .starts_with("state reset recorded: snapper took a new baseline"),
            "{out}"
        );
        let reset = resets(&cli);
        assert_eq!(reset.len(), 2, "{reset:?}");
        assert_eq!(reset[1].meta.extra["sources"], "snapper");
        assert_eq!(reset[1].meta.extra["files"], "cursors");
        assert_eq!(pending(&cli, "snapper"), None);
        assert_eq!(predicted(&cli), None);
        assert_eq!(waiting(&cli), None);

        let again = cli.capture(&[]);
        assert_eq!(again["written"], 0, "{again}");
        assert_eq!(resets(&cli).len(), 2);
    }

    /// WP-088: also when the degraded collector is the only one that lost
    /// its state, so that the capture records no note at all.
    #[test]
    fn a_collector_that_alone_lost_its_state_while_degraded_records_it_later() {
        let cli = Cli::new();
        cli.capture(&["--since", FIXTURE_CREATED]);
        std::fs::remove_dir_all(state(&cli)).unwrap();
        cli.stub_snapper_no_permissions();
        let snapper =
            |cli: &Cli| common::json(&cli.run(&["capture", "--source", "snapper", "--json"]));
        let out = snapper(&cli);
        assert_eq!(
            (out["ok"].clone(), out["written"].clone()),
            (false.into(), 0.into()),
            "{out}"
        );
        assert!(resets(&cli).is_empty());
        assert_eq!(pending(&cli, "snapper").as_deref(), Some("cursors"));
        cli.stub_snapper(&fixture("logs/snapper-before.json"));
        let out = snapper(&cli);
        assert_eq!(out["written"], 2, "the note and, WP-091, access: {out}");
        assert_eq!(access(&cli).len(), 1);
        let reset = resets(&cli);
        assert_eq!(reset.len(), 1, "{reset:?}");
        assert_eq!(reset[0].meta.extra["sources"], "snapper");
        assert_eq!(pending(&cli, "snapper"), None);
        assert_eq!(snapper(&cli)["written"], 0);
    }

    /// WP-088 review B1: the mark keeps what was lost. State bound to
    /// another logbook while snapper degraded: its later note says
    /// `logbook` too, and its warning that nothing can be restored.
    #[test]
    fn a_collector_degraded_when_the_state_was_another_logbooks_says_so_later() {
        let cli = Cli::new();
        cli.capture(&["--since", FIXTURE_CREATED]);
        let other = cli.env.tmp.path().join("other");
        let out = cli.env.seldon(&[
            "init",
            "--non-interactive",
            "--no-git",
            "--no-capture",
            "--path",
            other.to_str().unwrap(),
        ]);
        assert_eq!(out.status.code(), Some(0), "{}", common::stderr(&out));
        let out = cli.run(&[
            "capture",
            "--all",
            "--json",
            "--logbook",
            other.to_str().unwrap(),
        ]);
        assert_eq!(out.status.code(), Some(0), "{}", common::stderr(&out));

        cli.stub_snapper_no_permissions();
        cli.capture(&[]);
        assert_eq!(pending(&cli, "snapper").as_deref(), Some("logbook"));
        cli.capture(&[]);
        assert_eq!(
            pending(&cli, "snapper").as_deref(),
            Some("logbook"),
            "degraded again"
        );
        // WP-091: doctor says why, not "cursors unreadable"
        assert_eq!(predicted(&cli), None);
        let (names, row) = waiting(&cli).unwrap();
        assert_eq!(names, "snapper");
        assert!(
            row["message"].as_str().unwrap().starts_with(
                "snapper degraded or not run since a state reset (the state in ~/.local/state/seldon belonged to another logbook); its next successful capture records the gap"
            ),
            "{row}"
        );

        cli.stub_snapper(&fixture("logs/snapper-before.json"));
        let out = cli.capture(&[]);
        assert_eq!(out["written"], 2, "the note and, WP-091, access: {out}");
        assert_eq!(access(&cli).len(), 1);
        let notes: Vec<(String, String)> = resets(&cli)
            .iter()
            .map(|r| {
                let m = &r.meta.extra;
                (
                    m["sources"].as_str().unwrap().into(),
                    m["files"].as_str().unwrap().into(),
                )
            })
            .collect();
        let pair = |a: &str, b: &str| (a.to_string(), b.to_string());
        assert_eq!(
            notes,
            [pair("pacman", "logbook"), pair("snapper", "logbook")]
        );
        let warning = out["warnings"][0].as_str().unwrap();
        assert!(
            warning.starts_with("state reset recorded: snapper took a new baseline")
                && warning.contains("Nothing can be restored"),
            "{warning}"
        );
        assert_eq!(pending(&cli, "snapper"), None);
        assert_eq!(cli.capture(&[])["written"], 0);
    }

    /// WP-091: a mark in cursors bound to another logbook is that
    /// logbook's; doctor on this one names the binding, no waiting row.
    #[test]
    fn a_mark_of_another_logbook_is_no_waiting_row_here() {
        let cli = Cli::new();
        cli.capture(&["--since", FIXTURE_CREATED]);
        let other = cli.env.tmp.path().join("other");
        let out = cli.env.seldon(&[
            "init",
            "--non-interactive",
            "--no-git",
            "--no-capture",
            "--path",
            other.to_str().unwrap(),
        ]);
        assert_eq!(out.status.code(), Some(0), "{}", common::stderr(&out));
        let other = other.to_str().unwrap();
        let out = cli.run(&[
            "capture",
            "--all",
            "--json",
            "--since",
            FIXTURE_CREATED,
            "--logbook",
            other,
        ]);
        assert_eq!(out.status.code(), Some(0), "{}", common::stderr(&out));
        cli.stub_snapper_no_permissions();
        cli.capture(&[]);
        assert_eq!(pending(&cli, "snapper").as_deref(), Some("logbook"));
        assert_eq!(waiting(&cli).unwrap().0, "snapper");

        let doctor = common::json(&cli.run(&["doctor", "--json", "--logbook", other]));
        let rows: Vec<&str> = doctor["checks"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|c| c["name"] == "state" && c["status"] == "degraded")
            .map(|c| c["message"].as_str().unwrap())
            .collect();
        assert_eq!(
            rows,
            [
                "the next capture will record a state reset for snapper, pacman: cursors in ~/.local/state/seldon bound to another logbook, so changes made since the last capture may not be recorded"
            ],
            "{doctor}"
        );
    }

    /// WP-088: a degraded collector that lost nothing waits for nothing:
    /// the capture's gate (no cursor here: it never ran successfully here)
    /// and ledger rule (no events of its source) hold for the mark too.
    #[test]
    fn a_degraded_collector_that_lost_nothing_waits_for_nothing() {
        // degraded from the first capture on: no events of its source
        let cli = Cli::new();
        cli.stub_snapper_no_permissions();
        cli.capture(&["--since", FIXTURE_CREATED]);
        assert_eq!(pending(&cli, "snapper"), None);
        cli.capture(&[]);
        assert_eq!(pending(&cli, "snapper"), None);
        cli.stub_snapper(&fixture("logs/snapper-before.json"));
        let out = cli.capture(&["--since", FIXTURE_CREATED]);
        assert!(
            out["written"].as_u64().unwrap() > 0,
            "its first events: {out}"
        );
        assert!(resets(&cli).is_empty());

        // bound here, events of its source, but no cursor entry (WP-081 F1)
        let cli = Cli::new();
        cli.capture(&["--since", FIXTURE_CREATED]);
        let file = state(&cli).join("cursors.json");
        let mut cursors: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
        cursors["collectors"]
            .as_object_mut()
            .unwrap()
            .remove("snapper");
        std::fs::write(&file, cursors.to_string()).unwrap();
        cli.stub_snapper_no_permissions();
        cli.capture(&[]);
        assert_eq!(pending(&cli, "snapper"), None);
        cli.stub_snapper(&fixture("logs/snapper-before.json"));
        assert_eq!(cli.capture(&[])["written"], 1, "WP-091: access");
        assert_eq!(access(&cli).len(), 1);
        assert!(resets(&cli).is_empty());
    }

    /// WP-088: `pendingBaseline` is new; a `cursors.json` without it reads
    /// as not waiting, and it is written only while set, as the word the
    /// state reset's `files` uses.
    #[test]
    fn a_cursors_file_without_the_mark_reads_unchanged() {
        use seldon::collectors::{CollectorState, PendingBaseline};
        let old =
            r#"{"cursor":{"offset":7},"ok":true,"lastRun":"2026-10-01T10:00:00+02:00","events":2}"#;
        let state: CollectorState = serde_json::from_str(old).unwrap();
        assert_eq!(state.pending_baseline, None);
        assert_eq!(serde_json::to_string(&state).unwrap(), old);
        for (mark, word) in [
            (PendingBaseline::Cursors, "cursors"),
            (PendingBaseline::Logbook, "logbook"),
        ] {
            let waiting = CollectorState {
                pending_baseline: Some(mark),
                ..state.clone()
            };
            let text = serde_json::to_string(&waiting).unwrap();
            let expected = format!(r#"{},"pendingBaseline":"{word}"}}"#, &old[..old.len() - 1]);
            assert_eq!(text, expected);
            let back: CollectorState = serde_json::from_str(&text).unwrap();
            assert_eq!(back, waiting);
        }
    }

    /// `name`'s row in `index.state.collectors`.
    fn index_row(cli: &Cli, name: &str) -> serde_json::Value {
        let text = std::fs::read_to_string(state(cli).join("index.json")).unwrap();
        let index: serde_json::Value = serde_json::from_str(&text).unwrap();
        index["state"]["collectors"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["name"] == name)
            .unwrap_or_else(|| panic!("{name}: {index}"))
            .clone()
    }

    /// `name`'s entry in `cursors.json`, `Null` without one.
    fn entry(cli: &Cli, name: &str) -> serde_json::Value {
        let text = std::fs::read_to_string(state(cli).join("cursors.json")).unwrap();
        let cursors: serde_json::Value = serde_json::from_str(&text).unwrap();
        cursors["collectors"][name].clone()
    }

    /// WP-091 (WP-088 review N2): a collector not run (`--source`) in the
    /// capture that drops its state gets an entry with only the mark; the
    /// index reads it as no entry, and its first successful run records
    /// its gap, once.
    #[test]
    fn a_collector_not_run_in_the_reset_records_its_gap_when_it_runs() {
        let cli = Cli::new();
        let pacman = |cli: &Cli| {
            let out = cli.run(&["capture", "--source", "pacman", "--json"]);
            assert_eq!(out.status.code(), Some(0), "{}", common::stderr(&out));
            common::json(&out)
        };
        // no snapper events yet: not run, it loses nothing (ledger rule)
        let out = cli.run(&["capture", "--source", "pacman", "--since", FIXTURE_CREATED]);
        assert_eq!(out.status.code(), Some(0), "{}", common::stderr(&out));
        assert_eq!(entry(&cli, "snapper"), serde_json::Value::Null);
        cli.capture(&["--since", FIXTURE_CREATED]);
        assert!(resets(&cli).is_empty());

        std::fs::remove_dir_all(state(&cli)).unwrap();
        let out = pacman(&cli);
        assert_eq!(out["written"], 1, "only the note: {out}");
        let reset = resets(&cli);
        assert_eq!(reset.len(), 1, "{reset:?}");
        assert_eq!(reset[0].meta.extra["sources"], "pacman");
        assert_eq!(
            entry(&cli, "snapper"),
            serde_json::json!({"ok": true, "events": 0, "pendingBaseline": "cursors"}),
            "a bare entry"
        );
        for name in ["omarchy", "plugins", "theme", "config"] {
            assert_eq!(entry(&cli, name), serde_json::Value::Null, "{name}");
        }
        // review F2: doctor's "last capture" skips the bare entry's
        // missing `lastRun`, so the reset row still shows
        let doctor = common::json(&cli.run(&["doctor", "--json"]));
        let recorded: Vec<&str> = doctor["checks"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|c| c["name"] == "state")
            .filter_map(|c| {
                c["message"]
                    .as_str()?
                    .strip_prefix("the last capture recorded a state reset: ")
            })
            .collect();
        assert_eq!(recorded.len(), 1, "{doctor}");
        assert!(
            recorded[0].starts_with("pacman took a new baseline ("),
            "{doctor}"
        );
        // the index row of a bare entry is the row of no entry
        let none = index_row(&cli, "theme");
        assert_eq!(none["lastRun"], serde_json::Value::Null, "{none}");
        let mut bare = index_row(&cli, "snapper");
        bare["name"] = none["name"].clone();
        assert_eq!(bare, none);
        assert_eq!(predicted(&cli), None);
        assert_eq!(waiting(&cli).unwrap().0, "snapper");

        // not run again: the entry stays as it is
        assert_eq!(pacman(&cli)["written"], 0);
        assert_eq!(pending(&cli, "snapper").as_deref(), Some("cursors"));

        // an unreadable cursor besides: each in its own row, one note
        let file = state(&cli).join("cursors.json");
        let mut cursors: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
        cursors["collectors"]["pacman"]["cursor"] = serde_json::json!("not a cursor");
        std::fs::write(&file, cursors.to_string()).unwrap();
        assert_eq!(predicted(&cli).as_deref(), Some("pacman"));
        assert_eq!(waiting(&cli).unwrap().0, "snapper");

        let out = cli.capture(&[]);
        assert_eq!(out["written"], 1, "only the note: {out}");
        let reset = resets(&cli);
        assert_eq!(reset.len(), 2, "{reset:?}");
        assert_eq!(reset[1].meta.extra["sources"], "snapper,pacman");
        assert_eq!(reset[1].meta.extra["files"], "cursors");
        assert!(
            out["warnings"][0]
                .as_str()
                .unwrap()
                .starts_with("state reset recorded: snapper, pacman took a new baseline"),
            "{out}"
        );
        assert_eq!(pending(&cli, "snapper"), None);
        assert!(entry(&cli, "snapper")["lastRun"].is_string());
        assert_eq!(predicted(&cli), None);
        assert_eq!(waiting(&cli), None);
        assert_eq!(cli.capture(&[])["written"], 0);
        assert_eq!(resets(&cli).len(), 2);
    }

    /// WP-091: a disabled collector, while the state was another
    /// logbook's: the mark says `logbook`, and so does its note once it
    /// is enabled and runs.
    #[test]
    fn a_disabled_collector_when_the_state_was_another_logbooks_says_so_later() {
        let cli = Cli::new();
        cli.capture(&["--since", FIXTURE_CREATED]);
        let other = cli.env.tmp.path().join("other");
        let out = cli.env.seldon(&[
            "init",
            "--non-interactive",
            "--no-git",
            "--no-capture",
            "--path",
            other.to_str().unwrap(),
        ]);
        assert_eq!(out.status.code(), Some(0), "{}", common::stderr(&out));
        let out = cli.run(&[
            "capture",
            "--all",
            "--json",
            "--logbook",
            other.to_str().unwrap(),
        ]);
        assert_eq!(out.status.code(), Some(0), "{}", common::stderr(&out));

        let config = cli.env.config_file();
        let mut toml: toml::Table = std::fs::read_to_string(&config).unwrap().parse().unwrap();
        let mut off = toml::Table::new();
        off.insert("snapper".into(), toml::Value::Boolean(false));
        toml.insert("collectors".into(), toml::Value::Table(off));
        std::fs::write(&config, toml.to_string()).unwrap();
        let out = cli.capture(&[]);
        assert_eq!(out["written"], 1, "{out}");
        assert_eq!(resets(&cli)[0].meta.extra["sources"], "pacman");
        assert_eq!(pending(&cli, "snapper").as_deref(), Some("logbook"));
        assert_eq!(index_row(&cli, "snapper")["enabled"], false);

        toml.remove("collectors");
        std::fs::write(&config, toml.to_string()).unwrap();
        let out = cli.capture(&[]);
        assert_eq!(out["written"], 1, "only the note: {out}");
        let reset = resets(&cli);
        assert_eq!(reset.len(), 2, "{reset:?}");
        assert_eq!(reset[1].meta.extra["sources"], "snapper");
        assert_eq!(reset[1].meta.extra["files"], "logbook");
        assert!(
            out["warnings"][0]
                .as_str()
                .unwrap()
                .contains("Nothing can be restored"),
            "{out}"
        );
        assert_eq!(pending(&cli, "snapper"), None);
        assert_eq!(cli.capture(&[])["written"], 0);
    }

    /// WP-091: a collector not run keeps an entry it has (bound here) as it
    /// is; an unreadable cursor is recorded by its own next run, as before.
    #[test]
    fn a_collector_not_run_keeps_its_entry() {
        let cli = Cli::new();
        cli.capture(&["--since", FIXTURE_CREATED]);
        let file = state(&cli).join("cursors.json");
        let mut cursors: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
        cursors["collectors"]["snapper"]["cursor"] = serde_json::json!("not a cursor");
        std::fs::write(&file, cursors.to_string()).unwrap();
        let before = entry(&cli, "snapper");
        let out = cli.run(&["capture", "--source", "pacman", "--json"]);
        assert_eq!(common::json(&out)["written"], 0);
        assert_eq!(entry(&cli, "snapper"), before);
        assert_eq!(predicted(&cli).as_deref(), Some("snapper"));
        assert_eq!(waiting(&cli), None, "unreadable, not waiting");
        let out = cli.capture(&[]);
        assert_eq!(out["written"], 1, "only the note: {out}");
        assert_eq!(resets(&cli)[0].meta.extra["files"], "cursors");
    }

    #[test]
    fn an_unreadable_cursor_is_a_reset() {
        let cli = Cli::new();
        cli.capture(&["--since", FIXTURE_CREATED]);
        let file = state(&cli).join("cursors.json");
        let mut cursors: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
        cursors["collectors"]["pacman"]["cursor"] = serde_json::json!("not a cursor");
        std::fs::write(&file, cursors.to_string()).unwrap();
        assert_eq!(predicted(&cli).as_deref(), Some("pacman"));
        let out = cli.capture(&[]);
        assert_eq!(out["written"], 1, "only the note: {out}");
        let reset = resets(&cli);
        assert_eq!(reset.len(), 1);
        assert_eq!(reset[0].meta.extra["sources"], "pacman");
        assert_eq!(cli.capture(&[])["written"], 0);
    }

    #[test]
    fn a_corrupt_manifest_is_a_reset() {
        let cli = Cli::new();
        config_event(&cli);
        std::fs::write(state(&cli).join("manifest.json"), "{").unwrap();
        let out = capture_config(&cli);
        assert_eq!(out["written"], 1, "{out}");
        let reset = resets(&cli);
        assert_eq!(reset.len(), 1);
        assert_eq!(reset[0].meta.extra["sources"], "config");
        assert_eq!(reset[0].meta.extra["files"], "manifest");
        let warning = out["warnings"][0].as_str().unwrap();
        assert!(
            warning.starts_with("state reset recorded: config took a new baseline"),
            "{warning}"
        );
        assert_eq!(capture_config(&cli)["written"], 0);
    }

    #[test]
    fn a_corrupt_owned_file_is_a_reset_and_moved_aside() {
        let cli = Cli::new();
        config_event(&cli);
        let owned = state(&cli).join("owned.json");
        std::fs::write(&owned, "[").unwrap();
        let out = capture_config(&cli);
        assert_eq!(out["written"], 1, "{out}");
        let reset = resets(&cli);
        assert_eq!(reset.len(), 1);
        assert_eq!(reset[0].meta.extra["files"], "owned");
        assert!(!owned.exists());
        assert_eq!(
            std::fs::read_to_string(state(&cli).join("owned.json.bad")).unwrap(),
            "["
        );
        assert_eq!(capture_config(&cli)["written"], 0);
        assert_eq!(resets(&cli).len(), 1);
    }
}

/// WP-091: the details of the `seldon` notes on the snapper collector
/// changing between degraded and ok.
fn access(cli: &Cli) -> Vec<String> {
    cli.ledger()
        .into_iter()
        .filter(|e| e.source == Source::Seldon && e.kind == Kind::Note && e.subject == "snapper")
        .map(|e| e.detail.unwrap_or_default())
        .collect()
}

/// WP-091: the snapper collector changing between degraded and ok since
/// its last run for this logbook (ADR-0026: the read grant given or
/// taken away) leaves one `seldon` note, and nothing the next time.
mod snapper_access {
    use super::*;
    use seldon::collectors::snapper::NO_PERMISSIONS;

    fn snapshots(cli: &Cli) -> std::path::PathBuf {
        // `SELDON_TEST_GUARD` points the info files here
        cli.env.tmp.path().join(".snapshots")
    }

    fn copy_dir(from: &Path, to: &Path) {
        std::fs::create_dir_all(to).unwrap();
        for e in std::fs::read_dir(from).unwrap() {
            let e = e.unwrap();
            if e.file_type().unwrap().is_dir() {
                copy_dir(&e.path(), &to.join(e.file_name()));
            } else {
                std::fs::copy(e.path(), to.join(e.file_name())).unwrap();
            }
        }
    }

    #[test]
    fn a_grant_and_its_removal_are_recorded_once_each() {
        let cli = Cli::new();
        cli.stub_snapper_no_permissions();
        let first = cli.capture(&["--since", FIXTURE_CREATED]);
        assert_eq!(first["ok"], false, "{first}");
        assert!(access(&cli).is_empty(), "the first run is no change");
        assert_eq!(cli.capture(&[])["written"], 0, "degraded again");

        // the user runs the read grant: the info files can be read
        copy_dir(&fixture("logs/snapshots"), &snapshots(&cli));
        let out = cli.capture(&[]);
        assert_eq!(out["ok"], true, "{out}");
        assert_eq!(out["explainedSelf"], 0, "not an own change");
        let notes = access(&cli);
        assert_eq!(notes.len(), 1, "{notes:?}");
        assert!(
            notes[0].starts_with("snapper collector ok again (snapper list is not permitted; ")
                && notes[0].ends_with(&format!(
                    "; at its last run it was degraded: {NO_PERMISSIONS}"
                )),
            "{}",
            notes[0]
        );
        let note = cli
            .ledger()
            .into_iter()
            .find(|e| e.subject == "snapper" && e.source == Source::Seldon)
            .unwrap();
        assert_eq!((note.kind, note.actor.as_str()), (Kind::Note, "system"));
        assert_eq!(note.case, None);
        assert_eq!(cli.capture(&[])["written"], 0, "ok again: nothing");

        // a later `set-config` with SYNC_ACL=yes took the grant away
        std::fs::remove_dir_all(snapshots(&cli)).unwrap();
        let out = cli.capture(&[]);
        assert_eq!(out["ok"], false, "{out}");
        assert_eq!(out["written"], 1, "{out}");
        let notes = access(&cli);
        assert_eq!(
            notes[1],
            format!("snapper collector degraded: {NO_PERMISSIONS}; at its last run it was ok")
        );
        assert_eq!(cli.capture(&[])["written"], 0, "degraded again: nothing");
        assert_eq!(access(&cli).len(), 2);
        assert!(
            !cli.ledger().iter().any(|e| e.subject == "state-reset"),
            "no reset"
        );
        cli.ledger().iter().for_each(assert_schema_valid);
        // the notes are no drift
        let drift = common::json(&cli.run(&["drift", "--json"]));
        let items = drift["drift"].as_array().unwrap();
        assert!(items.iter().all(|i| i["eventId"].is_string()), "{drift}");
        assert!(!items.is_empty(), "drift is listed: {drift}");
        let ids: Vec<String> = cli
            .ledger()
            .into_iter()
            .filter(|e| e.subject == "snapper" && e.source == Source::Seldon)
            .map(|e| e.id.to_string())
            .collect();
        assert_eq!(ids.len(), 2);
        assert!(
            items
                .iter()
                .all(|i| !ids.iter().any(|id| i["eventId"] == **id)),
            "{drift}"
        );
    }

    /// Listing permitted, the other direction: ok by `snapper list`, then
    /// any failure; a capture that does not run snapper compares nothing.
    #[test]
    fn a_failure_and_recovery_of_the_list_are_recorded() {
        let cli = Cli::new();
        cli.capture(&["--since", FIXTURE_CREATED]);
        assert!(access(&cli).is_empty(), "the first run is no change");
        cli.stub("snapper", "exit 3");
        let out = cli.run(&["capture", "--source", "pacman", "--json"]);
        assert_eq!(common::json(&out)["written"], 0, "snapper not run");
        cli.capture(&[]);
        assert_eq!(
            access(&cli),
            ["snapper collector degraded: snapper failed (exit 3): ; at its last run it was ok"]
        );
        cli.stub_snapper(&fixture("logs/snapper-before.json"));
        cli.capture(&[]);
        assert_eq!(
            access(&cli)[1],
            "snapper collector ok again; at its last run it was degraded: snapper failed (exit 3): "
        );
        assert_eq!(cli.capture(&[])["written"], 0);
    }

    /// No earlier run for this logbook to compare with: a lost state
    /// directory, and an entry that only waits (WP-091 N2), never ran.
    #[test]
    fn no_change_without_an_earlier_run_here() {
        let cli = Cli::new();
        cli.capture(&["--since", FIXTURE_CREATED]);
        std::fs::remove_dir_all(cli.env.home.join(".local/state/seldon")).unwrap();
        cli.stub_snapper_no_permissions();
        cli.capture(&[]);
        assert!(access(&cli).is_empty(), "state lost: no earlier run");

        std::fs::remove_dir_all(cli.env.home.join(".local/state/seldon")).unwrap();
        let out = cli.run(&["capture", "--source", "pacman", "--json"]);
        assert_eq!(out.status.code(), Some(0), "{}", common::stderr(&out));
        cli.capture(&[]);
        assert!(access(&cli).is_empty(), "only the mark: it never ran");
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
