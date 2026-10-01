//! `seldon event` (SPEC-ENGINE §3) and [`emit`], the one path through
//! which this WP's commands (`log`, `event`, `plan`) write ledger events.
//!
//! [`emit`] hands the events to WP-004's [`Ledger::append`], which assigns
//! the ULIDs, redacts `detail` and `meta.command` (SPEC-ENGINE §7), cuts
//! `detail` to the schema limit and validates every event before anything
//! is written. Nothing here redacts or numbers events itself.

use clap::Args;
use serde_json::{Value, json};

use super::{Context, Output, autocommit};
use crate::config::Config;
use crate::error::{Error, Result};
use crate::ledger::Ledger;
use crate::logbook::Logbook;
use crate::logbook::cases::{self, CaseFile};
use crate::logbook::lock::Lock;
use crate::model::event::{DETAIL_MAX, Event, Kind, Meta, SUBJECT_MAX, Source};
use crate::model::{is_agent, is_case_id};
use crate::redact::Redactor;

/// Appends `events` to the ledger (in one write per month file) and returns
/// them as written: with ids, redacted. The caller holds the state lock.
pub fn emit(
    lock: &Lock,
    config: &Config,
    logbook: &Logbook,
    events: Vec<Event>,
) -> Result<Vec<Event>> {
    let ledger = Ledger::new(
        logbook,
        Redactor::with_patterns(&config.redaction.patterns)?,
    );
    ledger.append(lock, events)
}

/// The one event of a command, as written.
pub fn emit_one(lock: &Lock, config: &Config, logbook: &Logbook, event: Event) -> Result<Event> {
    emit(lock, config, logbook, vec![event])?
        .pop()
        .ok_or_else(|| anyhow::anyhow!("the ledger wrote no event").into())
}

/// An event as JSON (the ledger line's shape).
pub fn event_json(event: &Event) -> Value {
    serde_json::to_value(event).expect("an event always serialises")
}

/// Kinds only the engine's own commands write (`plan`, `drift`): they
/// carry state the engine must keep consistent.
pub fn is_engine_only(kind: Kind) -> bool {
    matches!(
        kind,
        Kind::Resolution
            | Kind::Correction
            | Kind::CaseCreated
            | Kind::CaseStarted
            | Kind::CaseVerified
            | Kind::CaseCompleted
            | Kind::CaseDropped
    )
}

/// The first `max` characters of `s`, with `…` when it was longer (for
/// human output; the ledger cuts `detail` itself).
pub fn clip(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max - 1).collect();
    out.push('…');
    out
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
    if args.source == Source::Seldon || is_engine_only(args.kind) {
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

    let mut event = Event::new(ctx.now, args.source, args.kind, subject)
        .actor(&args.actor)
        .case(args.case_id.clone())
        .meta(meta);
    if let Some(d) = args.detail.filter(|d| !d.trim().is_empty()) {
        event = event.detail(d);
    }
    // the ledger first: it assigns the id the case file records
    let event = emit_one(&lock, &config, &logbook, event)?;
    if let Some(file) = case_file.as_mut() {
        file.attach(&event.id.to_string(), &event.actor);
        file.save(&logbook)?;
    }
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
        json!({
            "event": event_json(&event),
            "ledger": format!("ledger/{}.jsonl", event.month()),
            "git": commit.json(),
        }),
    ))
}

/// `key=value` pairs into the typed meta keys (`event.schema.json`):
/// `pairOf` is a number, `enabled` a boolean, every other value a string;
/// unknown keys go to `extra`. `txId` belongs to resolutions only.
fn parse_meta(pairs: &[String]) -> Result<Meta> {
    let mut meta = Meta::default();
    let mut seen = std::collections::BTreeSet::new();
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
        if !seen.insert(key.to_string()) {
            return Err(Error::user(format!("--meta {key} is given twice")));
        }
        let text = Some(value.to_string());
        match key {
            "txId" => {
                return Err(Error::user(
                    "--meta txId is only written on drift resolutions",
                ));
            }
            "enabled" => {
                meta.enabled = Some(match value {
                    "true" => true,
                    "false" => false,
                    _ => return Err(Error::user("--meta enabled takes true or false")),
                })
            }
            "pairOf" => {
                meta.pair_of = Some(
                    value
                        .parse()
                        .map_err(|_| Error::user("--meta pairOf takes a snapshot number"))?,
                )
            }
            "command" => meta.command = text,
            "version" => meta.version = text,
            "from" => meta.from = text,
            "to" => meta.to = text,
            "hashFrom" => meta.hash_from = text,
            "hashTo" => meta.hash_to = text,
            "type" => meta.snapshot_type = text,
            "cleanup" => meta.cleanup = text,
            _ => {
                meta.extra
                    .insert(key.to_string(), Value::String(value.to_string()));
            }
        }
    }
    Ok(meta)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn meta_pairs() {
        let m = parse_meta(&[
            "version=1.0".into(),
            "enabled=false".into(),
            "pairOf=112".into(),
            "x=a=b".into(),
        ])
        .unwrap();
        assert_eq!(m.version.as_deref(), Some("1.0"));
        assert_eq!(m.enabled, Some(false));
        assert_eq!(m.pair_of, Some(112));
        assert_eq!(m.extra["x"], "a=b");
        for bad in [
            "novalue",
            "txId=1",
            "enabled=yes",
            "pairOf=x",
            "=1",
            "a b=1",
        ] {
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

    #[test]
    fn clipping() {
        assert_eq!(clip("abc", 3), "abc");
        assert_eq!(clip("abcd", 3), "ab…");
    }
}
