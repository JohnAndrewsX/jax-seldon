//! Helpers for the collector tests (WP-004): a scratch ledger, stub
//! programs, collector runs in-process against `fixtures/logs/`, schema
//! validation of events, and golden files.
//!
//! Nothing here reads or writes the real `~/.config`, `~/.local/state` or a
//! real logbook: every path is under a temp dir (AGENTS.md §6).
#![allow(dead_code)] // each test binary uses a different subset

use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use std::sync::atomic::{AtomicUsize, Ordering};

use chrono::{DateTime, FixedOffset};
use serde_json::Value;

use seldon::collectors::omarchy::Omarchy;
use seldon::collectors::pacman::Pacman;
use seldon::collectors::snapper::Snapper;
use seldon::collectors::{Collector, Ctx, Outcome, Sources, Tz};
use seldon::config::{Config, Dirs};
use seldon::ledger::Ledger;
use seldon::logbook::lock;
use seldon::model::event::{Event, Source};
use seldon::redact::Redactor;

/// The repository root.
pub fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
}

pub fn fixture(rel: &str) -> PathBuf {
    repo().join("fixtures").join(rel)
}

/// `RFC 3339` → instant.
pub fn ts(s: &str) -> DateTime<FixedOffset> {
    DateTime::parse_from_rfc3339(s).unwrap()
}

/// `created` of `fixtures/logbook/.seldon/logbook.toml`: the baseline the
/// fixture story starts from (README: cursor 6245 in pacman.log).
pub const FIXTURE_CREATED: &str = "2026-09-01T19:00:42+02:00";

/// The local zone of the fixture story (all of it is summer time).
pub fn cest() -> Tz {
    Tz::Fixed(FixedOffset::east_opt(2 * 3600).unwrap())
}

/// A scratch directory, removed on drop.
pub struct Scratch {
    pub dir: PathBuf,
}

impl Scratch {
    pub fn new(tag: &str) -> Self {
        static N: AtomicUsize = AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!(
            "seldon-wp004-{tag}-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("bin")).unwrap();
        Scratch { dir }
    }

    pub fn path(&self, rel: &str) -> PathBuf {
        self.dir.join(rel)
    }

    /// An executable `bin/<name>` running `body` with `sh`.
    pub fn stub(&self, name: &str, body: &str) -> String {
        let path = self.dir.join("bin").join(name);
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path.to_string_lossy().into_owned()
    }

    /// A stub that prints the file `file`.
    pub fn stub_cat(&self, name: &str, file: &Path) -> String {
        self.stub(name, &format!("cat '{}'", file.display()))
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// A collector bench: a scratch ledger, sources, config and the current
/// cursor of each collector, run in-process like `seldon capture` does
/// (collect, append, keep the cursor).
pub struct Bench {
    pub scratch: Scratch,
    pub ledger: Ledger,
    pub sources: Sources,
    pub config: Config,
    pub dirs: Dirs,
    pub baseline: DateTime<FixedOffset>,
    pub cursors: std::collections::BTreeMap<&'static str, Value>,
    /// The state lock, taken once for the bench's lifetime. Re-taking it
    /// per run races with stub programs other test threads fork: a child
    /// holds the inherited flock until it execs.
    pub lock: lock::Lock,
}

impl Bench {
    pub fn new(tag: &str) -> Self {
        let scratch = Scratch::new(tag);
        let home = scratch.path("home");
        let lock = lock::acquire(&scratch.path("lock")).unwrap();
        let missing = scratch
            .path("bin/no-such-program")
            .to_string_lossy()
            .into_owned();
        // every field spelled out, no `..Sources::default()` (the host's
        // paths and programs): a new source does not compile until it has a
        // scratch value here (WP-060 `snapshots`, WP-076 `omarchy`)
        let sources = Sources {
            pacman_log: fixture("logs/pacman.log"),
            pacman_db_lock: scratch.path("db.lck"),
            snapper: scratch.stub_cat("snapper", &fixture("logs/snapper.json")),
            snapshots: scratch.path("no-snapshots"),
            omarchy_version: scratch.stub("omarchy-version", "echo 4.0.4-1"),
            pacman: missing.clone(),
            omarchy: missing,
            plugins_dir: Some(scratch.path("plugins")),
            theme_file: Some(scratch.path("theme.name")),
            omarchy_path: scratch.path("omarchy"),
            etc_dir: scratch.path("etc"),
        };
        Bench {
            ledger: Ledger::at(scratch.path("logbook/ledger"), Redactor::builtin()),
            dirs: Dirs {
                xdg_config_home: home.join(".config"),
                state_dir: home.join(".local/state/seldon"),
                home,
            },
            scratch,
            sources,
            config: Config::default(),
            baseline: ts(FIXTURE_CREATED),
            cursors: Default::default(),
            lock,
        }
    }

    /// Copies the fixture ledger's lines of `sources` into the scratch
    /// ledger, unchanged (the hook `command` events attribution needs).
    pub fn seed(&self, sources: &[Source]) {
        std::fs::create_dir_all(self.ledger.dir()).unwrap();
        for month in ["2026-09", "2026-10"] {
            let text =
                std::fs::read_to_string(fixture(&format!("logbook/ledger/{month}.jsonl"))).unwrap();
            let lines: String = text
                .lines()
                .filter(|l| {
                    let e: Event = serde_json::from_str(l).unwrap();
                    sources.contains(&e.source)
                })
                .map(|l| format!("{l}\n"))
                .collect();
            std::fs::write(self.ledger.month_file(month), lines).unwrap();
        }
    }

    /// Runs `collector` once at `now`, appends its events, keeps its cursor.
    /// Returns the outcome with the events as written (ids assigned).
    pub fn run(&mut self, collector: &dyn Collector, now: &str) -> Outcome {
        self.run_with(collector, now, &[])
    }

    pub fn run_with(&mut self, collector: &dyn Collector, now: &str, earlier: &[Event]) -> Outcome {
        let ctx = Ctx {
            now: ts(now),
            baseline: self.baseline,
            tz: cest(),
            sources: &self.sources,
            config: &self.config,
            dirs: &self.dirs,
            ledger: &self.ledger,
            earlier,
        };
        let mut out = collector.collect(&ctx, self.cursors.get(collector.name()));
        out.events = self
            .ledger
            .append(&self.lock, std::mem::take(&mut out.events))
            .unwrap();
        if let Some(c) = &out.cursor {
            self.cursors.insert(collector.name(), c.clone());
        }
        out
    }

    /// Ledger events of `source`, in file order.
    pub fn ledger_events(&self, source: Source) -> Vec<Event> {
        self.ledger
            .read_all()
            .unwrap()
            .into_iter()
            .filter(|e| e.source == source)
            .collect()
    }
}

/// The fixture ledger's events of `source`, in file order.
pub fn fixture_events(source: Source) -> Vec<Event> {
    let mut out = Vec::new();
    for month in ["2026-09", "2026-10"] {
        let text =
            std::fs::read_to_string(fixture(&format!("logbook/ledger/{month}.jsonl"))).unwrap();
        out.extend(
            text.lines()
                .map(|l| serde_json::from_str::<Event>(l).unwrap())
                .filter(|e| e.source == source),
        );
    }
    out
}

/// An event as a JSON line with its id replaced by `<id>` (ids carry
/// capture time and randomness; fixtures/README.md "Event ids").
pub fn normalised(e: &Event) -> String {
    e.to_line()
        .replacen(&format!("\"id\":\"{}\"", e.id), "\"id\":\"<id>\"", 1)
}

/// Normalised lines, sorted by instant then subject (append order is not
/// part of the contract).
pub fn normalised_sorted(events: &[Event]) -> Vec<String> {
    let mut sorted: Vec<&Event> = events.iter().collect();
    sorted.sort_by(|a, b| {
        a.ts.cmp(&b.ts)
            .then(a.subject.cmp(&b.subject))
            .then(a.kind.as_str().cmp(b.kind.as_str()))
    });
    sorted.into_iter().map(normalised).collect()
}

static EVENT_SCHEMA: LazyLock<jsonschema::Validator> = LazyLock::new(|| {
    let text = std::fs::read_to_string(repo().join("schema/event.schema.json")).unwrap();
    let schema: Value = serde_json::from_str(&text).unwrap();
    jsonschema::options()
        .should_validate_formats(true)
        .build(&schema)
        .expect("event.schema.json compiles")
});

/// Panics unless `event` validates against `schema/event.schema.json`.
pub fn assert_schema_valid(event: &Event) {
    let instance = serde_json::to_value(event).unwrap();
    let errors: Vec<String> = EVENT_SCHEMA
        .iter_errors(&instance)
        .map(|e| format!("{} at {}", e, e.instance_path()))
        .collect();
    assert!(
        errors.is_empty(),
        "{}: {}",
        event.to_line(),
        errors.join("; ")
    );
}

/// Compares `lines` with `tests/golden/<name>`; `SELDON_BLESS=1` rewrites it.
pub fn assert_golden(name: &str, lines: &[String]) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden")
        .join(name);
    let actual: String = lines.iter().map(|l| format!("{l}\n")).collect();
    if std::env::var_os("SELDON_BLESS").is_some() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, &actual).unwrap();
        return;
    }
    let expected = std::fs::read_to_string(&path)
        .unwrap_or_else(|_| panic!("{} missing; run with SELDON_BLESS=1", path.display()));
    if expected != actual {
        let diff: Vec<String> = expected
            .lines()
            .zip(actual.lines())
            .filter(|(e, a)| e != a)
            .take(3)
            .map(|(e, a)| format!("- {e}\n+ {a}"))
            .collect();
        panic!(
            "{} differs ({} expected lines, {} actual):\n{}",
            path.display(),
            expected.lines().count(),
            actual.lines().count(),
            diff.join("\n")
        );
    }
}

/// Replays the fixture story; returns the bench with the ledger written.
pub fn story() -> Bench {
    let mut b = Bench::new("story");
    // the hook command events (WP-009 writes them; here: from the fixture)
    b.seed(&[Source::Agent]);

    b.sources.snapper = b
        .scratch
        .stub_cat("snapper", &fixture("logs/snapper-before.json"));
    let out = b.run(&Snapper, "2026-09-30T18:00:00+02:00");
    assert!(out.ok, "{:?}", out.message);
    assert_eq!(subjects(&out.events), ["108", "109", "110"]);

    let out = b.run(&Pacman, "2026-10-01T17:05:00+02:00");
    assert!(out.ok, "{:?}", out.message);
    assert_eq!(
        out.events.len(),
        22,
        "fixtures/README.md: the ledger's pacman events, got {:?}",
        subjects(&out.events)
    );

    for (version, now, n) in [
        ("4.0.5-1", "2026-09-15T20:00:00+02:00", 0), // baseline
        ("4.0.6-1", "2026-09-15T20:13:05+02:00", 1),
        ("4.0.6-1", "2026-09-20T10:00:00+02:00", 0),
        ("4.0.7-1", "2026-10-01T09:21:00+02:00", 1),
    ] {
        b.sources.omarchy_version = b
            .scratch
            .stub("omarchy-version", &format!("echo {version}"));
        let out = b.run(&Omarchy, now);
        assert!(out.ok, "{:?}", out.message);
        assert_eq!(out.events.len(), n, "omarchy {version}");
    }

    b.sources.snapper = b.scratch.stub_cat("snapper", &fixture("logs/snapper.json"));
    let out = b.run(&Snapper, "2026-09-30T19:05:00+02:00");
    assert_eq!(
        out.events
            .iter()
            .map(|e| format!("{} {}", e.kind, e.subject))
            .collect::<Vec<_>>(),
        [
            "snapshot 111",
            "snapshot 112",
            "snapshot 113",
            "snapshot 114",
            "snapshot 115",
            "snapshot-delete 108",
            "snapshot-delete 109"
        ]
    );
    b
}

pub fn subjects(events: &[Event]) -> Vec<&str> {
    events.iter().map(|e| e.subject.as_str()).collect()
}

/// WP-076: every source of a [`Bench`] lies under its scratch dir or the
/// fixtures, never a host path or a program found on the host's PATH.
/// (Runs once in each test binary that includes this module.)
#[test]
fn the_bench_reads_nothing_of_the_host() {
    let b = Bench::new("hostless");
    let s = &b.sources;
    let inside = |p: &Path| p.starts_with(&b.scratch.dir) || p.starts_with(fixture(""));
    let paths = [
        s.pacman_log.as_path(),
        &s.pacman_db_lock,
        &s.snapshots,
        s.plugins_dir.as_deref().expect("plugins_dir"),
        s.theme_file.as_deref().expect("theme_file"),
        &s.omarchy_path,
    ];
    let programs = [&s.snapper, &s.omarchy_version, &s.pacman, &s.omarchy].map(Path::new);
    for p in paths.into_iter().chain(programs) {
        assert!(
            inside(p),
            "{} is not a scratch or fixture path",
            p.display()
        );
    }
    assert!(b.dirs.home.starts_with(&b.scratch.dir));
    assert!(b.dirs.state_dir.starts_with(&b.scratch.dir));
    assert!(b.dirs.xdg_config_home.starts_with(&b.scratch.dir));
}
