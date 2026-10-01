//! Ledger events and `seldon event` (SPEC-ENGINE §3, `schema/event.schema.json`).
//!
//! Every event this WP's commands write goes through [`emit`]: one JSON
//! object per line in `ledger/YYYY-MM.jsonl`, the month taken from the
//! event's own timestamp, appended while the state lock is held.
//!
//! This is the minimal writer the orchestrator asked for until WP-004's
//! `model::Event` and `ledger::append` are on main; then `Event` and
//! [`emit`] become thin wrappers over them. Redaction (SPEC-ENGINE §7,
//! WP-004's `redact.rs`) is not applied here yet.

use std::io::{Read as _, Seek as _, SeekFrom, Write as _};
use std::sync::Mutex;
use std::time::SystemTime;

use anyhow::Context as _;
use chrono::{DateTime, FixedOffset};
use clap::Args;
use serde::Serialize;
use serde_json::{Map, Value, json};

use super::{Context, Output, autocommit};
use crate::error::{Error, Result};
use crate::logbook::Logbook;
use crate::logbook::cases::{self, CaseFile};
use crate::logbook::lock::Lock;
use crate::model::case::str_enum;
use crate::model::{Zone, is_agent, is_case_id};

/// `detail` and `resolutionDetail` limit (`event.schema.json`), in characters.
pub const DETAIL_MAX: usize = 4096;
/// `subject` limit, in characters.
pub const SUBJECT_MAX: usize = 512;

str_enum!(
    /// `event.schema.json#/properties/source`.
    Source {
        Pacman = "pacman",
        Snapper = "snapper",
        Omarchy = "omarchy",
        Plugins = "plugins",
        Theme = "theme",
        Config = "config",
        Agent = "agent",
        Manual = "manual",
        Seldon = "seldon",
    }
);

str_enum!(
    /// `event.schema.json#/properties/kind`.
    Kind {
        Install = "install",
        Remove = "remove",
        Upgrade = "upgrade",
        Downgrade = "downgrade",
        Reinstall = "reinstall",
        Snapshot = "snapshot",
        SnapshotDelete = "snapshot-delete",
        Update = "update",
        PluginAdd = "plugin-add",
        PluginRemove = "plugin-remove",
        PluginEnable = "plugin-enable",
        PluginDisable = "plugin-disable",
        PluginUpdate = "plugin-update",
        ThemeSet = "theme-set",
        ConfigChange = "config-change",
        ConfigAdd = "config-add",
        ConfigRemove = "config-remove",
        Command = "command",
        Note = "note",
        Resolution = "resolution",
        Correction = "correction",
        CaseCreated = "case-created",
        CaseStarted = "case-started",
        CaseVerified = "case-verified",
        CaseCompleted = "case-completed",
        CaseDropped = "case-dropped",
    }
);

impl Kind {
    /// Kinds only the engine's own commands write (`plan`, `drift`):
    /// they carry state the engine must keep consistent.
    pub fn is_engine_only(self) -> bool {
        matches!(
            self,
            Kind::Resolution
                | Kind::Correction
                | Kind::CaseCreated
                | Kind::CaseStarted
                | Kind::CaseVerified
                | Kind::CaseCompleted
                | Kind::CaseDropped
        )
    }
}

/// What a command wants recorded; [`Event::new`] adds id and timestamp.
#[derive(Debug, Clone)]
pub struct NewEvent {
    pub source: Source,
    pub kind: Kind,
    pub subject: String,
    pub detail: Option<String>,
    pub actor: String,
    pub case: Option<String>,
    pub zone: Option<Zone>,
    pub meta: Map<String, Value>,
}

impl NewEvent {
    pub fn new(source: Source, kind: Kind, subject: impl Into<String>, actor: &str) -> Self {
        NewEvent {
            source,
            kind,
            subject: subject.into(),
            detail: None,
            actor: actor.to_string(),
            case: None,
            zone: None,
            meta: Map::new(),
        }
    }
}

/// One ledger line, keys in the order of `fixtures/logbook/ledger/`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Event {
    pub id: String,
    pub ts: String,
    pub source: Source,
    pub kind: Kind,
    pub subject: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    pub actor: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub case: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub zone: Option<Zone>,
    #[serde(skip_serializing_if = "Map::is_empty")]
    pub meta: Map<String, Value>,
    /// `YYYY-MM` of `ts` in its own offset: the ledger file.
    #[serde(skip)]
    pub month: String,
}

/// Monotonic within the process: two events of one command never share
/// an id and sort in the order they were made.
static ULIDS: Mutex<ulid::Generator> = Mutex::new(ulid::Generator::new());

impl Event {
    /// Stamps `new` with a ULID for `at` and the RFC 3339 time (seconds,
    /// with offset). `detail` is cut to the schema limit.
    pub fn new(at: &DateTime<FixedOffset>, new: NewEvent) -> Event {
        let mut ulids = ULIDS.lock().unwrap_or_else(|e| e.into_inner());
        let id = match ulids.generate_from_datetime(SystemTime::from(*at)) {
            Ok(id) => id,
            Err(overflow) => overflow.commit_overflow_increment(),
        };
        Event {
            id: id.to_string(),
            ts: at.format("%Y-%m-%dT%H:%M:%S%:z").to_string(),
            source: new.source,
            kind: new.kind,
            subject: new.subject,
            detail: new.detail.map(|d| clip(&d, DETAIL_MAX)),
            actor: new.actor,
            case: new.case,
            zone: new.zone,
            meta: new.meta,
            month: at.format("%Y-%m").to_string(),
        }
    }

    pub fn json(&self) -> Value {
        serde_json::to_value(self).expect("an event always serialises")
    }

    /// `ledger/YYYY-MM.jsonl`, relative to the logbook root.
    pub fn ledger_file(&self) -> String {
        format!("ledger/{}.jsonl", self.month)
    }
}

/// Appends `event` to its month's ledger file. The caller holds the state
/// lock (the `&Lock` proves it); the file is opened for append, and a last
/// line without its newline (a crash mid-write) is closed first so it can
/// never swallow this one.
pub fn emit(_lock: &Lock, logbook: &Logbook, event: &Event) -> Result<()> {
    let path = logbook.path(event.ledger_file());
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).with_context(|| format!("cannot create {}", dir.display()))?;
    }
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .read(true)
        .open(&path)
        .with_context(|| format!("cannot open {}", path.display()))?;
    let mut line = String::new();
    if file.metadata()?.len() > 0 {
        let mut last = [0u8; 1];
        file.seek(SeekFrom::End(-1))?;
        file.read_exact(&mut last)?;
        if last[0] != b'\n' {
            line.push('\n');
        }
    }
    line.push_str(&serde_json::to_string(event).context("cannot serialise event")?);
    line.push('\n');
    file.write_all(line.as_bytes())
        .with_context(|| format!("cannot append to {}", path.display()))?;
    Ok(())
}

/// The first `max` characters of `s`, with `…` when it was longer.
pub fn clip(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max - 1).collect();
    out.push('…');
    out
}

/// Zone by source (ADR-0014 §2): red for pacman, omarchy and systemd
/// config; yellow for other config, theme, plugins; none otherwise.
pub fn zone_for(source: Source, subject: &str) -> Option<Zone> {
    match source {
        Source::Pacman | Source::Omarchy => Some(Zone::Red),
        Source::Config if subject.starts_with("~/.config/systemd/") => Some(Zone::Red),
        Source::Config | Source::Theme | Source::Plugins => Some(Zone::Yellow),
        Source::Snapper | Source::Agent | Source::Manual | Source::Seldon => None,
    }
}

/// clap value parser: `human`, `system` or `agent:<name>`.
pub fn parse_actor(s: &str) -> Result<String, String> {
    if s == "human" || s == "system" || is_agent(s) {
        Ok(s.to_string())
    } else {
        Err(format!(
            "`{s}` is not an actor (human, system, agent:<name> with a lowercase name)"
        ))
    }
}

/// clap value parser: `human` or `agent:<name>` (who writes notes and cases).
pub fn parse_person(s: &str) -> Result<String, String> {
    match parse_actor(s)? {
        a if a == "system" => Err("`system` cannot write this; use human or agent:<name>".into()),
        a => Ok(a),
    }
}

/// clap value parser: `C-YYYY-NNN`.
pub fn parse_case_id(s: &str) -> Result<String, String> {
    if is_case_id(s) {
        Ok(s.to_string())
    } else {
        Err(format!("`{s}` is not a case id (C-YYYY-NNN)"))
    }
}

/// `seldon event <source> <kind> --subject S [--detail D] [--case ID]
/// [--actor A] [--meta k=v]…`
#[derive(Debug, Clone, Args)]
pub struct EventArgs {
    /// Event source (pacman, snapper, omarchy, plugins, theme, config, agent, manual)
    #[arg(value_name = "SOURCE")]
    pub source: Source,

    /// Event kind (install, theme-set, config-change, command, note, …)
    #[arg(value_name = "KIND")]
    pub kind: Kind,

    /// What it is about: package, ~-relative path, theme, plugin id, …
    #[arg(long, value_name = "S", allow_hyphen_values = true)]
    pub subject: String,

    /// Human-readable detail
    #[arg(long, value_name = "D", allow_hyphen_values = true)]
    pub detail: Option<String>,

    /// Attribute the event to this case
    #[arg(long = "case", value_name = "ID", value_parser = parse_case_id)]
    pub case_id: Option<String>,

    /// Who did it
    #[arg(long, value_name = "A", default_value = "human", value_parser = parse_actor)]
    pub actor: String,

    /// Extra key=value (repeatable); `enabled` takes true or false
    #[arg(long = "meta", value_name = "KEY=VALUE")]
    pub meta: Vec<String>,
}

pub fn run(ctx: &Context, args: EventArgs) -> Result<Output> {
    if args.source == Source::Seldon || args.kind.is_engine_only() {
        return Err(Error::user(format!(
            "{}/{} events are written by `seldon plan` and `seldon drift`, not by `seldon event`",
            args.source, args.kind
        )));
    }
    let subject = args.subject.trim();
    let chars = subject.chars().count();
    if chars == 0 || chars > SUBJECT_MAX || subject.contains(['\n', '\r']) {
        return Err(Error::user(format!(
            "--subject must be one line of 1 to {SUBJECT_MAX} characters"
        )));
    }
    if let Some(detail) = &args.detail
        && detail.chars().count() > DETAIL_MAX
    {
        return Err(Error::user(format!(
            "--detail is longer than {DETAIL_MAX} characters"
        )));
    }
    let meta = parse_meta(&args.meta)?;

    let (config, logbook) = ctx.open_logbook()?;
    let lock = ctx.lock()?;
    let mut case_file: Option<CaseFile> = args
        .case_id
        .as_deref()
        .map(|id| cases::find(&logbook, id))
        .transpose()?;

    let event = Event::new(
        &ctx.now,
        NewEvent {
            zone: zone_for(args.source, subject),
            detail: args.detail.filter(|d| !d.trim().is_empty()),
            case: args.case_id.clone(),
            meta,
            ..NewEvent::new(args.source, args.kind, subject, &args.actor)
        },
    );
    if let Some(file) = case_file.as_mut() {
        file.attach(&event.id, &event.actor);
        file.save(&logbook)?;
    }
    emit(&lock, &logbook, &event)?;
    let summary = format!(
        "event {}/{} {}",
        event.source,
        event.kind,
        clip(subject, 60)
    );
    let commit = autocommit(ctx, &config, &logbook, &summary);
    drop(lock);

    let mut human = format!(
        "Recorded {}/{} `{}` ({}){}",
        event.source,
        event.kind,
        event.subject,
        event.id,
        event
            .case
            .as_deref()
            .map(|c| format!(" for {c}"))
            .unwrap_or_default()
    );
    human.push_str(&commit.human());
    Ok(Output::ok(
        human,
        json!({ "event": event.json(), "ledger": event.ledger_file(), "git": commit.json() }),
    ))
}

/// `key=value` pairs. Values are strings, except `enabled` (a boolean,
/// `event.schema.json`); `txId` belongs to resolutions only.
fn parse_meta(pairs: &[String]) -> Result<Map<String, Value>> {
    let mut meta = Map::new();
    for pair in pairs {
        let Some((key, value)) = pair.split_once('=') else {
            return Err(Error::user(format!("--meta `{pair}` is not KEY=VALUE")));
        };
        let valid_key = key.starts_with(|c: char| c.is_ascii_alphabetic())
            && key
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || "_.-".contains(c));
        if !valid_key {
            return Err(Error::user(format!("--meta key `{key}` is not a name")));
        }
        let value = match key {
            "txId" => {
                return Err(Error::user(
                    "--meta txId is only written on drift resolutions",
                ));
            }
            "enabled" => match value {
                "true" => Value::Bool(true),
                "false" => Value::Bool(false),
                _ => return Err(Error::user("--meta enabled takes true or false")),
            },
            _ => Value::String(value.to_string()),
        };
        if meta.insert(key.to_string(), value).is_some() {
            return Err(Error::user(format!("--meta {key} is given twice")));
        }
    }
    Ok(meta)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone as _;

    fn at(d: u32, h: u32, m: u32) -> DateTime<FixedOffset> {
        let month = if d > 1 { 10 } else { 11 };
        FixedOffset::east_opt(3600)
            .unwrap()
            .with_ymd_and_hms(2026, month, d, h, m, 59)
            .unwrap()
    }

    #[test]
    fn stamps_ids_and_months_by_timestamp() {
        let note = || NewEvent::new(Source::Manual, Kind::Note, "journal", "human");
        let a = Event::new(&at(31, 23, 59), note());
        let b = Event::new(&at(31, 23, 59), note());
        assert!(crate::model::is_ulid(&a.id), "{}", a.id);
        assert!(a.id < b.id, "monotonic within the same millisecond");
        assert_eq!(a.ts, "2026-10-31T23:59:59+01:00");
        assert_eq!(a.ledger_file(), "ledger/2026-10.jsonl");
        assert_eq!(
            serde_json::to_string(&a).unwrap(),
            format!(
                r#"{{"id":"{}","ts":"2026-10-31T23:59:59+01:00","source":"manual","kind":"note","subject":"journal","actor":"human"}}"#,
                a.id
            )
        );
        // the month is the timestamp's own: 00:30+01:00 on 1 November is
        // still October in UTC, and goes to the November file
        let c = Event::new(&at(1, 0, 30), note());
        assert_eq!(c.ts, "2026-11-01T00:30:59+01:00");
        assert_eq!(c.ledger_file(), "ledger/2026-11.jsonl");
    }

    #[test]
    fn detail_is_clipped_to_the_schema_limit() {
        let mut new = NewEvent::new(Source::Manual, Kind::Note, "journal", "human");
        new.detail = Some("ä".repeat(DETAIL_MAX + 10));
        let e = Event::new(&at(31, 1, 0), new);
        let detail = e.detail.unwrap();
        assert_eq!(detail.chars().count(), DETAIL_MAX);
        assert!(detail.ends_with('…'));
    }

    #[test]
    fn zones_follow_adr_0014() {
        assert_eq!(zone_for(Source::Pacman, "zed"), Some(Zone::Red));
        assert_eq!(
            zone_for(Source::Config, "~/.config/systemd/user/x.service"),
            Some(Zone::Red)
        );
        assert_eq!(
            zone_for(Source::Config, "~/.config/hypr/a.conf"),
            Some(Zone::Yellow)
        );
        assert_eq!(zone_for(Source::Theme, "x"), Some(Zone::Yellow));
        assert_eq!(zone_for(Source::Manual, "journal"), None);
    }

    #[test]
    fn meta_pairs() {
        let m =
            parse_meta(&["version=1.0".into(), "enabled=false".into(), "x=a=b".into()]).unwrap();
        assert_eq!(m["version"], "1.0");
        assert_eq!(m["enabled"], false);
        assert_eq!(m["x"], "a=b");
        for bad in ["novalue", "txId=1", "enabled=yes", "=1", "a b=1"] {
            assert!(parse_meta(&[bad.into()]).is_err(), "{bad}");
        }
        assert!(parse_meta(&["a=1".into(), "a=2".into()]).is_err());
    }

    #[test]
    fn actors() {
        assert!(parse_actor("agent:claude-code").is_ok());
        assert!(parse_actor("system").is_ok());
        assert!(parse_person("system").is_err());
        assert!(parse_actor("agent:Claude").is_err());
        assert!(parse_actor("root").is_err());
    }
}
