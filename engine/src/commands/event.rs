//! `seldon event` (SPEC-ENGINE §3) and [`emit`], the one path through
//! which this WP's commands (`log`, `event`, `plan`) write ledger events.
//!
//! [`emit`] hands the events to WP-004's [`Ledger::append`], which assigns
//! the ULIDs, redacts `subject`, `detail` and every string `meta` value
//! (SPEC-ENGINE §7), cuts `subject` and `detail` to the schema limits and
//! validates every event before anything is written. Nothing here redacts
//! or numbers events itself.

use chrono::DateTime;
use clap::Args;
use serde_json::{Value, json};

use super::{Context, Output, autocommit};
use crate::attribution;
use crate::collectors::{self, Cursors};
use crate::config::Config;
use crate::error::{Error, Result};
use crate::ledger::Ledger;
use crate::logbook::Logbook;
use crate::logbook::cases::{self, CaseFile};
use crate::logbook::lock::Lock;
use crate::model::event::{ACTOR_SYSTEM, DETAIL_MAX, Event, Kind, Meta, SUBJECT_MAX, Source};
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
    let ledger = Ledger::new(logbook, Redactor::for_config(config)?);
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
            | Kind::CaseUpdated
            | Kind::StateLoss
    )
}

/// The command that writes `kind` (the refusal names it; WP-120 N1);
/// any other kind with `source: seldon` is the engine's own record.
fn writer(kind: Kind) -> &'static str {
    match kind {
        Kind::CaseCreated
        | Kind::CaseStarted
        | Kind::CaseVerified
        | Kind::CaseCompleted
        | Kind::CaseDropped
        | Kind::CaseUpdated => "`seldon plan`",
        Kind::Resolution | Kind::Correction => "`seldon drift`",
        Kind::StateLoss => "`seldon capture`",
        _ => "the engine's own commands",
    }
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
            "`{}` is not an actor (human, system, agent:<name> with a lowercase name)",
            s.escape_debug()
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

/// The actor of a command run without `--actor` (WP-096): `seldon agent
/// start` sets it for the agent it launches, so a write by that agent is
/// recorded as the agent even when it forgets `--actor` (ADR-0027 §5).
pub const ACTOR_ENV: &str = "SELDON_ACTOR";

/// `--actor` when given (clap checked it; `$SELDON_ACTOR` is then not
/// read), else `$SELDON_ACTOR` checked with `parse`, else `default`.
pub fn actor_or_env(
    flag: Option<String>,
    parse: fn(&str) -> Result<String, String>,
    default: &str,
) -> Result<String> {
    match flag {
        Some(actor) => Ok(actor),
        None => Ok(env_actor(parse)?.unwrap_or_else(|| default.to_string())),
    }
}

/// `$SELDON_ACTOR` checked with `parse`; `None` when it is unset or empty.
/// A value `parse` refuses is a user error (exit 1) that names the
/// variable and the allowed form.
pub fn env_actor(parse: fn(&str) -> Result<String, String>) -> Result<Option<String>> {
    checked_env_actor(std::env::var_os(ACTOR_ENV), parse)
}

fn checked_env_actor(
    value: Option<std::ffi::OsString>,
    parse: fn(&str) -> Result<String, String>,
) -> Result<Option<String>> {
    let Some(value) = value.filter(|v| !v.is_empty()) else {
        return Ok(None);
    };
    let value = value.to_str().ok_or_else(|| {
        Error::user(format!(
            "{ACTOR_ENV} is not UTF-8; set it to human or agent:<name>, or pass --actor"
        ))
    })?;
    parse(value)
        .map(Some)
        .map_err(|e| Error::user(format!("{ACTOR_ENV} (the actor when none is named): {e}")))
}

/// clap value parser: `C-YYYY-NNN`.
pub fn parse_case_id(s: &str) -> Result<String, String> {
    if is_case_id(s) {
        Ok(s.to_string())
    } else {
        Err(format!(
            "`{}` is not a case id (C-YYYY-NNN)",
            s.escape_debug()
        ))
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
    #[arg(long, value_name = "SUBJECT", allow_hyphen_values = true)]
    pub subject: String,

    /// Human-readable detail
    #[arg(long, value_name = "TEXT", allow_hyphen_values = true)]
    pub detail: Option<String>,

    /// Attribute the event to this case
    #[arg(long = "case", value_name = "ID", value_parser = parse_case_id)]
    pub case_id: Option<String>,

    /// Who did it: system (like a collector), human or agent:NAME; hooks
    /// and scripts name the one they act for (default: the agent command
    /// that caused a config, theme or plugins change, else $SELDON_ACTOR,
    /// else system)
    #[arg(long, value_name = "ACTOR", value_parser = parse_actor)]
    pub actor: Option<String>,

    /// Extra key=value (repeatable), e.g. `--meta enabled=true`; `enabled` takes true or false
    #[arg(long = "meta", value_name = "KEY=VALUE")]
    pub meta: Vec<String>,
}

pub fn run(ctx: &Context, args: EventArgs) -> Result<Output> {
    if args.source == Source::Seldon || is_engine_only(args.kind) {
        return Err(Error::user(format!(
            "{}/{} events are written by {}, not by `seldon event`",
            args.source,
            args.kind,
            writer(args.kind)
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
    let mut meta = parse_meta(&args.meta)?;
    let mut detail = args.detail.filter(|d| !d.trim().is_empty());
    // read without `--actor` only, and checked before anything is opened
    let env_actor = match &args.actor {
        Some(_) => None,
        None => env_actor(parse_actor)?,
    };

    let (config, logbook) = ctx.open_logbook()?;
    let lock = ctx.lock()?;
    if args.kind == Kind::ThemeSet
        && meta.from.is_none()
        && let Some(from) = previous_theme(ctx, &logbook, subject)?
    {
        detail.get_or_insert_with(|| format!("{from} → {subject}"));
        meta.to.get_or_insert_with(|| subject.to_string());
        meta.from = Some(from);
    }
    let mut case_file: Option<CaseFile> = args
        .case_id
        .as_deref()
        .map(|id| cases::find(&logbook, id))
        .transpose()?;

    let mut event = Event::new(ctx.now, args.source, args.kind, subject)
        .actor(args.actor.as_deref().unwrap_or(ACTOR_SYSTEM))
        .case(args.case_id.clone())
        .meta(meta);
    if let Some(d) = detail {
        event = event.detail(d);
    }
    // a hook-recorded change (theme-set.sh) takes the agent command that
    // caused it, as the capture would: the collector skips it afterwards
    if event.actor == ACTOR_SYSTEM && event.case.is_none() {
        let ledger = Ledger::new(&logbook, Redactor::builtin());
        attribution::attribute_from_ledger(
            &ledger,
            std::slice::from_mut(&mut event),
            &ctx.dirs.home,
        )?;
    }
    // `$SELDON_ACTOR` after that: the command found there also names its case
    if let Some(actor) = env_actor
        && event.actor == ACTOR_SYSTEM
    {
        event.actor = actor;
    }
    // a save the case would refuse fails before the ledger changes (WP-077)
    if let Some(file) = &case_file {
        let id = cases::pending_ids(1).remove(0);
        file.prepare(&logbook, |f| f.attach(&id, &event.actor))?;
    }
    // the ledger first: it assigns the id the case file records
    let event = emit_one(&lock, &config, &logbook, event)?;
    if let Some(file) = case_file.as_mut() {
        file.attach(&event.id.to_string(), &event.actor);
        file.save(&logbook)?;
    }
    // never the subject: free text must not reach a command line
    let summary = format!("event {}/{}", event.source, event.kind);
    let commit = autocommit(ctx, &config, &logbook, &summary);
    crate::index::rebuild_if_initialised(ctx);
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

/// The theme a `theme-set` to `subject` replaces (the theme-set hook passes
/// only the new slug): the theme collector's cursor, unless the ledger
/// holds a `theme-set` newer than the cursor's last check, then that one's
/// theme; `None` when neither is known or it is `subject` itself.
fn previous_theme(ctx: &Context, logbook: &Logbook, subject: &str) -> Result<Option<String>> {
    let root = std::fs::canonicalize(&logbook.root).unwrap_or_else(|_| logbook.root.clone());
    let cursors = Cursors::load(&collectors::cursors_file(&ctx.dirs)).unwrap_or_default();
    let cursor = cursors.cursor(&root, "theme").and_then(|c| {
        let theme = c.get("theme")?.as_str()?.to_string();
        let checked = DateTime::parse_from_rfc3339(c.get("checked")?.as_str()?).ok()?;
        Some((theme, checked))
    });

    let ledger = Ledger::new(logbook, Redactor::builtin());
    let mut latest = None;
    for month in ledger.months()?.iter().rev() {
        latest = ledger
            .read_month(month)?
            .events
            .into_iter()
            .filter(|e| e.kind == Kind::ThemeSet)
            .max_by_key(|e| e.ts);
        if latest.is_some() {
            break;
        }
    }

    let from = match (cursor, latest) {
        (Some((theme, checked)), Some(e)) if e.ts <= checked => Some(theme),
        (_, Some(e)) => Some(e.subject),
        (Some((theme, _)), None) => Some(theme),
        (None, None) => None,
    };
    Ok(from.filter(|f| f != subject))
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
            "risk" => {
                return Err(Error::user(
                    "--meta risk is only written on case lines (`seldon plan`)",
                ));
            }
            "txStatus" => {
                return Err(Error::user(
                    "--meta txStatus is written by the pacman collector only",
                ));
            }
            crate::model::event::TRUNCATED => {
                return Err(Error::user(
                    "--meta truncated is index-only; the ledger keeps every text whole",
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
            "risk=R1",
            "truncated=true",
            "txStatus=interrupted",
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
    fn the_actor_variable() {
        let var = |s: &str| Some(std::ffi::OsString::from(s));
        let get = |v, p| checked_env_actor(v, p);
        assert_eq!(get(None, parse_person).unwrap(), None);
        assert_eq!(get(var(""), parse_person).unwrap(), None);
        assert_eq!(
            get(var("agent:codex"), parse_person).unwrap().as_deref(),
            Some("agent:codex")
        );
        assert_eq!(
            get(var("system"), parse_actor).unwrap().as_deref(),
            Some("system")
        );
        for (bad, parse) in [
            (
                "agent:Codex",
                parse_person as fn(&str) -> Result<String, String>,
            ),
            ("system", parse_person),
            ("root", parse_actor),
            ("agent:", parse_actor),
        ] {
            let e = get(var(bad), parse).unwrap_err();
            assert!(matches!(e.exit(), crate::error::Exit::UserError), "{bad}");
            let e = e.to_string();
            assert!(
                e.starts_with("SELDON_ACTOR (the actor when none is named): ")
                    && e.contains("agent:<name>"),
                "{bad}: {e}"
            );
        }
        use std::os::unix::ffi::OsStringExt as _;
        let e = get(Some(std::ffi::OsString::from_vec(vec![0xff])), parse_person)
            .unwrap_err()
            .to_string();
        assert!(e.contains("SELDON_ACTOR is not UTF-8"), "{e}");
    }

    #[test]
    fn clipping() {
        assert_eq!(clip("abc", 3), "abc");
        assert_eq!(clip("abcd", 3), "ab…");
    }
}
