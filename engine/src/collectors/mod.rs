//! Collectors: read the system, produce ledger events (SPEC-ENGINE §4).
//!
//! A collector is a [`Collector`]: given a [`Ctx`] and its saved cursor it
//! returns an [`Outcome`] — new events, the cursor to save, and whether it
//! ran fully (`ok`) or degraded (`ok: false` with a message and an optional
//! one-line fix, ADR-0011). Collectors never write; `seldon capture`
//! (`commands/capture.rs`) runs them in [`REGISTRY`] order, appends all
//! events through the ledger and then saves the cursors, so a failed write
//! never advances a cursor.
//!
//! Cursors and the last status of each collector live in
//! `~/.local/state/seldon/cursors.json` ([`Cursors`]). Each collector owns
//! the JSON value under its name and types it itself.
//!
//! A collector without a cursor (first capture, or a different logbook)
//! takes a *baseline*: it emits only what happened at or after
//! [`Ctx::baseline`] (default: the logbook's `created` time, or
//! `capture --since`), so history from before the logbook is not drift.
//! It says so in [`Outcome::baseline`], naming the state file that was
//! missing or unreadable ([`Lost`]); when the ledger already holds events of
//! that source, `capture` records the gap as a state reset
//! ([`STATE_RESET`]) instead of starting over silently.
//!
//! Read-only on the host: collectors read files and run fixed programs with
//! fixed argv through [`crate::sys::run`] (snapper through
//! [`snapper::run_list`], in the C locale); nothing here changes the system.

pub mod config;
pub mod omarchy;
pub mod pacman;
pub mod plugins;
pub mod snapper;
pub mod theme;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::Context as _;
use chrono::{DateTime, FixedOffset, Local, NaiveDateTime, TimeZone as _};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::config::{Config, Dirs};
use crate::ledger::Ledger;
use crate::model::event::Event;
use crate::sys;

/// Timeout for every program a collector runs.
pub const RUN_TIMEOUT: Duration = Duration::from_secs(10);

/// A source of events. Implementations are stateless; state is the cursor.
pub trait Collector {
    /// `pacman`, `snapper`, … (`config.toml [collectors]` key, index name).
    fn name(&self) -> &'static str;

    /// Collects new events since `cursor` (`None`: take a baseline).
    fn collect(&self, ctx: &Ctx, cursor: Option<&Value>) -> Outcome;
}

/// What one collector run produced.
#[derive(Debug, Clone, Default)]
pub struct Outcome {
    pub events: Vec<Event>,
    /// The cursor to save; `None` keeps the saved one.
    pub cursor: Option<Value>,
    pub ok: bool,
    /// English, shown in the index and by `doctor`; never executed.
    pub message: Option<String>,
    /// A command the user can run to fix a degraded collector; printed,
    /// never run by Seldon.
    pub fix: Option<String>,
    /// The last check the events are measured from (the cursor's
    /// `checked`): an event stamped with the capture time happened after
    /// it, which widens its attribution window
    /// ([`crate::attribution::Stamps`]).
    pub since: Option<DateTime<FixedOffset>>,
    /// The run took a new baseline because this state file was missing or
    /// unreadable (`None`: it continued from its saved state).
    pub baseline: Option<Lost>,
}

/// The state file whose loss made a collector take a new baseline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Lost {
    /// The collector's entry in `cursors.json`: missing (a new or lost
    /// state directory, another logbook) or not readable as its cursor.
    Cursor,
    /// `manifest.json` (config): missing, corrupt, or without the
    /// generation the cursor names.
    Manifest,
    /// `owned.json` (config): corrupt, so the engine's own writes are lost.
    Owned,
}

impl Lost {
    /// The file's kind as `meta.files` of a state reset names it.
    pub fn as_str(self) -> &'static str {
        match self {
            Lost::Cursor => "cursors",
            Lost::Manifest => "manifest",
            Lost::Owned => "owned",
        }
    }
}

/// Subject of the `seldon` `note` a capture writes when collectors lost
/// their state although the ledger holds their events (WP-081).
pub const STATE_RESET: &str = "state-reset";

impl Outcome {
    pub fn ok(events: Vec<Event>, cursor: Value) -> Self {
        Outcome {
            events,
            cursor: Some(cursor),
            ok: true,
            ..Outcome::default()
        }
    }

    /// Sets [`Outcome::baseline`]: `Some` when the run took a new baseline
    /// because that state file was missing or unreadable.
    pub fn baseline(mut self, lost: Option<Lost>) -> Self {
        self.baseline = lost;
        self
    }

    /// Degraded: no events, cursor unchanged.
    pub fn degraded(message: impl Into<String>, fix: Option<String>) -> Self {
        Outcome {
            ok: false,
            message: Some(message.into()),
            fix,
            ..Outcome::default()
        }
    }
}

/// All collectors in run order (SPEC-ENGINE §4): snapper, pacman, omarchy,
/// plugins, theme, config. Later collectors see earlier ones' events in
/// [`Ctx::earlier`] (omarchy attributes its `update` from pacman's events).
pub static REGISTRY: [&(dyn Collector + Sync); 6] = [
    &snapper::Snapper,
    &pacman::Pacman,
    &omarchy::Omarchy,
    &plugins::Plugins,
    &theme::Theme,
    &config::ConfigFiles,
];

/// The collector called `name`, if any.
pub fn find(name: &str) -> Option<&'static (dyn Collector + Sync)> {
    REGISTRY.iter().copied().find(|c| c.name() == name)
}

/// Where collectors read from. Defaults are the real host paths and
/// programs; environment variables point them at fixtures (tests, and the
/// acceptance runs in docs/TESTING.md). Read once per process.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sources {
    /// `SELDON_PACMAN_LOG`, default `/var/log/pacman.log`.
    pub pacman_log: PathBuf,
    /// `SELDON_PACMAN_DB_LOCK`, default `/var/lib/pacman/db.lck`: present
    /// while pacman runs (ADR-0013 §5).
    pub pacman_db_lock: PathBuf,
    /// `SELDON_SNAPPER`, default `snapper`.
    pub snapper: String,
    /// `SELDON_SNAPSHOTS_DIR`, default `/.snapshots`: the `root` config's
    /// snapshot directory, read when `snapper list` is not permitted. Under
    /// `SELDON_TEST_GUARD` without the variable it is `<guard>/.snapshots`,
    /// so a guarded run never reads the host's snapshots.
    pub snapshots: PathBuf,
    /// `SELDON_OMARCHY_VERSION`, default `omarchy-version`.
    pub omarchy_version: String,
    /// `SELDON_PACMAN`, default `pacman`; only ever run as `-Q omarchy`
    /// (the fallback of the omarchy collector).
    pub pacman: String,
    /// `SELDON_OMARCHY`, default `omarchy`; run as `plugin list --json` and
    /// `plugin catalog` (plugins collector).
    pub omarchy: String,
    /// `SELDON_OMARCHY_PLUGINS_DIR`; `None` = Omarchy's
    /// `~/.config/omarchy/plugins` ([`plugins::Plugins::dir`]).
    pub plugins_dir: Option<PathBuf>,
    /// `SELDON_THEME_FILE`; `None` = Omarchy's
    /// `~/.local/state/omarchy/current/theme.name` ([`theme::Theme::file`]).
    pub theme_file: Option<PathBuf>,
}

impl Default for Sources {
    fn default() -> Self {
        Sources {
            pacman_log: PathBuf::from("/var/log/pacman.log"),
            pacman_db_lock: PathBuf::from("/var/lib/pacman/db.lck"),
            snapper: "snapper".into(),
            snapshots: PathBuf::from("/.snapshots"),
            omarchy_version: "omarchy-version".into(),
            pacman: "pacman".into(),
            omarchy: "omarchy".into(),
            plugins_dir: None,
            theme_file: None,
        }
    }
}

impl Sources {
    pub fn from_env() -> Self {
        let var = |name: &str| std::env::var(name).ok().filter(|v| !v.is_empty());
        let d = Sources::default();
        Sources {
            pacman_log: var("SELDON_PACMAN_LOG").map_or(d.pacman_log, PathBuf::from),
            pacman_db_lock: var("SELDON_PACMAN_DB_LOCK").map_or(d.pacman_db_lock, PathBuf::from),
            snapper: var("SELDON_SNAPPER").unwrap_or(d.snapper),
            snapshots: var("SELDON_SNAPSHOTS_DIR")
                .map(PathBuf::from)
                .or_else(|| {
                    var(crate::config::TEST_GUARD_ENV).map(|g| Path::new(&g).join(".snapshots"))
                })
                .unwrap_or(d.snapshots),
            omarchy_version: var("SELDON_OMARCHY_VERSION").unwrap_or(d.omarchy_version),
            pacman: var("SELDON_PACMAN").unwrap_or(d.pacman),
            omarchy: var("SELDON_OMARCHY").unwrap_or(d.omarchy),
            plugins_dir: var("SELDON_OMARCHY_PLUGINS_DIR").map(PathBuf::from),
            theme_file: var("SELDON_THEME_FILE").map(PathBuf::from),
        }
    }
}

/// How naive local times (snapper's `date`) get their offset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tz {
    /// The process's local zone (`TZ`, `/etc/localtime`).
    Local,
    /// A fixed offset (tests).
    Fixed(FixedOffset),
}

impl Tz {
    /// `naive` in this zone; the earlier instant of an ambiguous time, `None`
    /// for a time that does not exist (a DST gap).
    pub fn localize(self, naive: NaiveDateTime) -> Option<DateTime<FixedOffset>> {
        match self {
            Tz::Local => Local
                .from_local_datetime(&naive)
                .earliest()
                .map(|t| t.fixed_offset()),
            Tz::Fixed(off) => off.from_local_datetime(&naive).single(),
        }
    }
}

/// Everything a collector may read. Built once per capture.
pub struct Ctx<'a> {
    /// Capture time, local offset.
    pub now: DateTime<FixedOffset>,
    /// Without a cursor, events before this instant are history, not news.
    pub baseline: DateTime<FixedOffset>,
    pub tz: Tz,
    pub sources: &'a Sources,
    pub config: &'a Config,
    pub dirs: &'a Dirs,
    /// For lookups: rotation dedupe, attribution (ADR-0014 §1).
    pub ledger: &'a Ledger,
    /// Events of the collectors that ran before this one in this capture.
    pub earlier: &'a [Event],
}

impl Ctx<'_> {
    /// Runs `program args…` (fixed argv, no shell) with [`RUN_TIMEOUT`].
    pub fn run(&self, program: &str, args: &[&str]) -> sys::Run {
        sys::run(program, args, None, RUN_TIMEOUT)
    }
}

/// `cursors.json`: per collector its cursor and last status.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Cursors {
    /// The logbook these cursors belong to. Cursors of another logbook are
    /// ignored (each logbook takes its own baseline).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logbook: Option<PathBuf>,
    #[serde(default)]
    pub collectors: BTreeMap<String, CollectorState>,
}

/// One collector's entry in `cursors.json`. `ok`, `message` and `lastRun`
/// feed `index.state.collectors` (WP-007).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectorState {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<Value>,
    pub ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fix: Option<String>,
    /// RFC 3339.
    pub last_run: String,
    /// Events written by the last run.
    #[serde(default)]
    pub events: usize,
}

/// `cursors.json` in the state directory.
pub fn cursors_file(dirs: &Dirs) -> PathBuf {
    dirs.state_dir.join("cursors.json")
}

impl Cursors {
    /// Reads `path`; missing → empty. A corrupt file is an engine error:
    /// silently starting over would re-baseline every collector.
    pub fn load(path: &Path) -> anyhow::Result<Cursors> {
        match std::fs::read_to_string(path) {
            Ok(text) => serde_json::from_str(&text)
                .with_context(|| format!("{}: invalid cursors file", path.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Cursors::default()),
            Err(e) => Err(anyhow::Error::new(e).context(format!("cannot read {}", path.display()))),
        }
    }

    /// Writes atomically (temp + rename).
    pub fn save(&self, path: &Path) -> anyhow::Result<()> {
        let mut text = serde_json::to_string_pretty(self)?;
        text.push('\n');
        sys::write_atomic(path, text.as_bytes())
    }

    /// The saved cursor of `name`, if it belongs to `logbook`.
    pub fn cursor(&self, logbook: &Path, name: &str) -> Option<&Value> {
        if self.logbook.as_deref() != Some(logbook) {
            return None;
        }
        self.collectors.get(name)?.cursor.as_ref()
    }

    /// Switches to `logbook`, dropping cursors of any other logbook.
    pub fn bind(&mut self, logbook: &Path) {
        if self.logbook.as_deref() != Some(logbook) {
            self.collectors.clear();
            self.logbook = Some(logbook.to_path_buf());
        }
    }
}

/// `cursor` deserialised as `T`; an unreadable cursor counts as none.
pub fn typed_cursor<T: serde::de::DeserializeOwned>(cursor: Option<&Value>) -> Option<T> {
    cursor.and_then(|c| serde_json::from_value(c.clone()).ok())
}

/// `value` as a JSON value (cursors are small plain structs).
pub fn to_cursor<T: Serialize>(value: &T) -> Value {
    serde_json::to_value(value).expect("a cursor always serialises")
}
