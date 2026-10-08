//! Collectors: read the system, produce ledger events (SPEC-ENGINE §4).
//!
//! A collector is a [`Collector`]: given a [`Ctx`] and its saved cursor it
//! returns an [`Outcome`] — new events, the cursor to save, and whether it
//! ran fully (`ok`) or degraded (`ok: false` with a message and an optional
//! one-line fix, ADR-0026). Collectors never write; `seldon capture`
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
//! ([`STATE_RESET`]) instead of starting over silently. `doctor` predicts
//! that reset before the capture from the cursors alone
//! ([`Collector::cursor_reads`]), while a restore still prevents it.
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

use std::cell::OnceCell;
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
use crate::redact::Redactor;
use crate::sys;

/// Timeout for every program a collector runs.
pub const RUN_TIMEOUT: Duration = Duration::from_secs(10);

/// A source of events. Implementations are stateless; state is the cursor.
pub trait Collector {
    /// `pacman`, `snapper`, … (`config.toml [collectors]` key, index name).
    fn name(&self) -> &'static str;

    /// Collects new events since `cursor` (`None`: take a baseline).
    fn collect(&self, ctx: &Ctx, cursor: Option<&Value>) -> Outcome;

    /// Whether `cursor` reads as this collector's cursor ([`typed_cursor`]
    /// of its type). One that does not counts as none: [`Collector::collect`]
    /// takes a baseline and reports [`Lost::Cursor`]. `doctor` asks this
    /// before the capture (WP-083).
    fn cursor_reads(&self, cursor: &Value) -> bool;
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
    /// `cursors.json` belonged to another logbook. Set by `capture`, which
    /// knows the binding, for a collector that reported [`Lost::Cursor`].
    Logbook,
    /// The collector's cursor in `cursors.json`: none (a new or lost state
    /// directory, another logbook) or not readable as its cursor.
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
            Lost::Logbook => "logbook",
            Lost::Cursor => "cursors",
            Lost::Manifest => "manifest",
            Lost::Owned => "owned",
        }
    }
}

/// Subject of the `seldon` `state-loss` line a capture writes when
/// collectors lost their state although the ledger holds their events
/// (WP-081; a `note` before contract 2, ADR-0035 §4).
pub const STATE_RESET: &str = crate::model::event::STATE_LOSS_SUBJECT;

/// Whether `e` records a state loss: a `state-loss` line, or the `note`
/// with subject [`STATE_RESET`] an engine before contract 2 wrote. Old
/// lines are never rewritten (append-only), so every reader takes both.
pub fn is_state_loss(e: &Event) -> bool {
    use crate::model::event::{Kind, Source};
    e.source == Source::Seldon
        && (e.kind == Kind::StateLoss || (e.kind == Kind::Note && e.subject == STATE_RESET))
}

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
    /// `OMARCHY_PATH`, default `/usr/share/omarchy`: Omarchy's shipped
    /// files, read only to compare a config file with Omarchy's own copy
    /// (`meta.matches = "omarchy-default"`, ADR-0028 §2). Under
    /// `SELDON_TEST_GUARD` without the variable it is `<guard>/omarchy`.
    pub omarchy_path: PathBuf,
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
            omarchy_path: PathBuf::from("/usr/share/omarchy"),
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
            omarchy_path: var("OMARCHY_PATH")
                .map(PathBuf::from)
                .or_else(|| {
                    var(crate::config::TEST_GUARD_ENV).map(|g| Path::new(&g).join("omarchy"))
                })
                .unwrap_or(d.omarchy_path),
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
    /// `naive` in this zone; for an ambiguous time chrono's `earliest()`,
    /// the reading with the smaller offset, which in the repeated hour is
    /// the later instant (pacman keeps this; snapper resolves such times
    /// itself, WP-082); `None` for a time that does not exist (a DST gap).
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
    /// Runs `program args…` (fixed argv, no shell) with [`RUN_TIMEOUT`]
    /// and [`sys::OUTPUT_MAX`].
    pub fn run(&self, program: &str, args: &[&str]) -> sys::Run {
        sys::run(program, args, None, RUN_TIMEOUT, sys::OUTPUT_MAX)
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
    /// The times (RFC 3339) of the `seldon` notes a capture was about to
    /// append when it saved this file right before the append, the rest
    /// unchanged; its save after the append clears them (WP-099). A note
    /// the ledger holds at such a time was written by a capture that
    /// stopped before that save, and the next capture does not write it
    /// again. Not written while empty: older files read unchanged.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pending_notes: Vec<String>,
    /// Per logbook, the sources a capture baselined without a note (the
    /// ledger held no event of them) when it saved this file right before
    /// the append, with [`Cursors::pending_notes`]; cleared by the same
    /// save after the append (WP-104). The events that append wrote are no
    /// loss of the next capture's (`capture::held_sources`). Not written
    /// while empty: older files read unchanged.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub silent_baselines: BTreeMap<PathBuf, Vec<String>>,
}

/// One collector's entry in `cursors.json`. `ok`, `message` and `lastRun`
/// feed `index.state.collectors` (WP-007). An entry without `lastRun` is a
/// collector that was not run in the capture that lost its state and only
/// carries the mark ([`CollectorState::waiting`], WP-091).
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
    /// RFC 3339; `None` for a [`CollectorState::waiting`] entry, which never
    /// ran.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_run: Option<String>,
    /// Events written by the last run.
    #[serde(default)]
    pub events: usize,
    /// The collector degraded (WP-088) or was not run (WP-091) in a capture
    /// in which its state was lost (missing, unreadable or another
    /// logbook's, while the ledger holds events of its source), so it took
    /// no baseline then; its first successful run records the gap as a
    /// state reset with this kind and clears it. Not written while `None`:
    /// older files read unchanged.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pending_baseline: Option<PendingBaseline>,
}

impl CollectorState {
    /// The entry of a collector that was not run (`--source`, or disabled)
    /// in a capture that dropped its state, which it would have lost
    /// (WP-091): only the mark, no cursor and no run. `index.state.collectors`
    /// reads it as it reads no entry: `ok`, no message, no `lastRun`.
    pub fn waiting(mark: PendingBaseline) -> Self {
        CollectorState {
            cursor: None,
            ok: true,
            message: None,
            fix: None,
            last_run: None,
            events: 0,
            pending_baseline: Some(mark),
        }
    }
}

/// What a collector's waiting baseline lost ([`CollectorState::pending_baseline`]),
/// written as the state reset's `files` word.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PendingBaseline {
    /// [`Lost::Cursor`].
    Cursors,
    /// [`Lost::Logbook`].
    Logbook,
}

impl PendingBaseline {
    /// The mark for a lost cursor; `None` for the config collector's files,
    /// which a degraded run never reports.
    pub fn of(lost: Lost) -> Option<Self> {
        match lost {
            Lost::Cursor => Some(PendingBaseline::Cursors),
            Lost::Logbook => Some(PendingBaseline::Logbook),
            Lost::Manifest | Lost::Owned => None,
        }
    }

    /// The loss the collector's first successful run records.
    pub fn lost(self) -> Lost {
        match self {
            PendingBaseline::Cursors => Lost::Cursor,
            PendingBaseline::Logbook => Lost::Logbook,
        }
    }
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

/// Shown in place of every collector message while `config.toml` or one
/// of its `[redaction] patterns` cannot be used: the redaction the user
/// asked for cannot run, and nothing is shown unredacted (SPEC-ENGINE §7).
pub const MESSAGE_WITHHELD: &str =
    "‹redacted› (withheld: config.toml or one of its [redaction] patterns cannot be used)";

/// Collector messages and probe output as the index and `doctor` show them
/// (WP-105): through the logbook's redaction, built for the first message
/// only (most states hold none), or [`MESSAGE_WITHHELD`] when it cannot be
/// built. Each call starts from the text it is given (`cursors.json`, a
/// probe's output), so a user pattern that matches across a marker does
/// not grow what is shown from one call to the next.
pub struct ShownMessages<'a> {
    /// `None`: config.toml could not be read, its patterns are unknown.
    config: Option<&'a Config>,
    redactor: OnceCell<Option<Redactor>>,
}

impl<'a> ShownMessages<'a> {
    /// With the redaction of `config`; `None` withholds every message.
    pub fn new(config: Option<&'a Config>) -> Self {
        ShownMessages {
            config,
            redactor: OnceCell::new(),
        }
    }

    /// `message` as shown.
    pub fn show(&self, message: &str) -> String {
        let redactor = self
            .redactor
            .get_or_init(|| Redactor::for_config(self.config?).ok());
        match redactor {
            Some(r) => r.redact(message),
            None => MESSAGE_WITHHELD.to_string(),
        }
    }
}
