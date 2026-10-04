//! `seldon capture [--source a,b | --all] [--since TS]` (SPEC-ENGINE §3, §4).
//!
//! Runs the selected collectors in registry order under the state lock,
//! appends their events to the ledger in one pass (sorted by `ts`, stable),
//! then saves `cursors.json`. A collector that degrades (`ok: false`, e.g.
//! snapper without permissions, ADR-0026) does not fail the capture; it is
//! reported with its message and fix. Running it twice in a row writes
//! nothing the second time.
//!
//! Selection: no flag or `--all` runs every collector enabled in
//! `config.toml [collectors]`; `--source` runs exactly the named ones, also
//! when disabled. `--since TS` sets the baseline for collectors that have no
//! cursor yet (default: the logbook's `created` time); collectors with a
//! cursor ignore it, and the output says so (`sinceIgnored`).
//!
//! Before the append, the shared attribution pass gives config, theme and
//! plugins events the actor and case of the agent command that provably
//! caused them (`attribution.rs`). The pacman and omarchy collectors
//! attribute their own events. After the append, a run of the config
//! collector explains the files the engine wrote itself (`init
//! --theme-hook`, `hook install`; SPEC-ENGINE §5 rule 7).

use std::fmt::Write as _;

use chrono::{DateTime, FixedOffset, Timelike as _};
use serde_json::json;

use super::{Context, Output};
use crate::attribution::{self, Stamps};
use crate::collectors::config::OwnWrites;
use crate::collectors::{self, CollectorState, Ctx, Cursors, REGISTRY, Sources, Tz};
use crate::config::Config;
use crate::error::{Error, Result};
use crate::ledger::Ledger;
use crate::logbook::{Logbook, lock};
use crate::model::event::{Event, format_ts};
use crate::redact::Redactor;

#[derive(Debug, Clone, Default)]
pub struct CaptureArgs {
    /// `--source a,b`; empty with `all` or alone = every enabled collector.
    pub sources: Vec<String>,
    pub all: bool,
    /// `--since`, RFC 3339.
    pub since: Option<String>,
}

/// One collector's line in the report.
#[derive(Debug, Clone)]
pub struct CollectorReport {
    pub name: &'static str,
    pub enabled: bool,
    pub ran: bool,
    pub ok: bool,
    pub events: usize,
    pub message: Option<String>,
    pub fix: Option<String>,
}

/// `seldon capture`.
pub fn run(ctx: &Context, args: CaptureArgs) -> Result<Output> {
    let config = ctx.load_config()?.unwrap_or_default();
    let (root, _) = ctx.resolve_logbook(None, Some(&config));
    let mut logbook = Logbook::open(&root)?;
    // one logbook, one key in cursors.json, however its path was spelled
    if let Ok(canonical) = std::fs::canonicalize(&logbook.root) {
        logbook.root = canonical;
    }
    let since = args
        .since
        .as_deref()
        .map(|s| {
            DateTime::parse_from_rfc3339(s)
                .map_err(|e| Error::user(format!("--since `{s}` is not RFC 3339: {e}")))
        })
        .transpose()?;
    let selected = select(&config, &args)?;

    let lock = lock::acquire(&ctx.dirs.lock_file())?;
    let ledger = Ledger::new(
        &logbook,
        Redactor::with_patterns(&config.redaction.patterns)?,
    );
    let cursors_path = collectors::cursors_file(&ctx.dirs);
    let mut cursors = Cursors::load(&cursors_path)?;
    cursors.bind(&logbook.root);
    // --since only sets the baseline of collectors without a cursor
    let since_ignored: Vec<&str> = match since {
        Some(_) => selected
            .iter()
            .filter(|(name, run)| *run && cursors.cursor(&logbook.root, name).is_some())
            .map(|(name, _)| *name)
            .collect(),
        None => Vec::new(),
    };

    // the invocation's clock (`$SELDON_NOW` sets it), whole seconds
    let now = ctx.now.with_nanosecond(0).unwrap_or(ctx.now);
    let baseline = match since {
        Some(s) => s,
        None => created(&logbook)?,
    };
    let sources = Sources::from_env();
    let Collected {
        mut events,
        reports,
        states,
        stamps,
    } = collect_all(
        &selected, &config, ctx, &ledger, &cursors, &logbook, &sources, now, baseline,
    );
    attribution::attribute_capture(&ledger, &mut events, &ctx.dirs.home, &stamps)?;

    let written = ledger.append(&lock, events)?;
    for (name, state) in states {
        cursors.collectors.insert(name.to_string(), state);
    }
    cursors.save(&cursors_path)?;
    // WP-008: reconciliation, the attributed collector event ids into
    // their case files (ADR-0012 §10); warnings only, the append is done
    crate::reconcile::after_capture(&logbook, &ledger, &written);
    // rule 7: only a run of the config collector has seen the own writes
    let mut explained = 0;
    if reports.iter().any(|r| r.name == "config" && r.ran && r.ok) {
        let (n, warnings) = crate::reconcile::explain_own_writes(
            &lock,
            &ledger,
            &OwnWrites::file(&ctx.dirs),
            &written,
            now,
        );
        explained = n;
        for w in warnings {
            eprintln!("seldon: warning: {w}");
        }
    }
    crate::index::rebuild_if_initialised(ctx);
    drop(lock);

    Ok(render(
        &logbook,
        &written,
        &reports,
        &since_ignored,
        explained,
    ))
}

/// The collectors to run, in registry order.
fn select(config: &Config, args: &CaptureArgs) -> Result<Vec<(&'static str, bool)>> {
    for name in &args.sources {
        if collectors::find(name).is_none() {
            return Err(Error::user(format!(
                "unknown source `{name}` (one of {})",
                REGISTRY.map(|c| c.name()).join(", ")
            )));
        }
    }
    Ok(REGISTRY
        .iter()
        .map(|c| {
            let enabled = config.collectors.get(c.name()).unwrap_or(true);
            let run = if args.sources.is_empty() {
                enabled
            } else {
                args.sources.iter().any(|s| s == c.name())
            };
            (c.name(), run)
        })
        .collect())
}

/// What the collectors of one capture produced.
struct Collected {
    /// Sorted by `ts`.
    events: Vec<Event>,
    reports: Vec<CollectorReport>,
    states: Vec<(&'static str, CollectorState)>,
    /// When the events happened, for attribution.
    stamps: Stamps,
}

#[allow(clippy::too_many_arguments)]
fn collect_all(
    selected: &[(&'static str, bool)],
    config: &Config,
    ctx: &Context,
    ledger: &Ledger,
    cursors: &Cursors,
    logbook: &Logbook,
    sources: &Sources,
    now: DateTime<FixedOffset>,
    baseline: DateTime<FixedOffset>,
) -> Collected {
    let mut events: Vec<Event> = Vec::new();
    let mut reports = Vec::new();
    let mut states = Vec::new();
    let mut stamps = Stamps {
        now: Some(now),
        since: Vec::new(),
    };
    for &(name, run) in selected {
        let enabled = config.collectors.get(name).unwrap_or(true);
        if !run {
            reports.push(CollectorReport {
                name,
                enabled,
                ran: false,
                ok: true,
                events: 0,
                message: None,
                fix: None,
            });
            continue;
        }
        let collector = collectors::find(name).expect("selected from the registry");
        let cctx = Ctx {
            now,
            baseline,
            tz: Tz::Local,
            sources,
            config,
            dirs: &ctx.dirs,
            ledger,
            earlier: &events,
        };
        let out = collector.collect(&cctx, cursors.cursor(&logbook.root, name));
        states.push((
            name,
            CollectorState {
                cursor: out
                    .cursor
                    .clone()
                    .or_else(|| cursors.cursor(&logbook.root, name).cloned()),
                ok: out.ok,
                message: out.message.clone(),
                fix: out.fix.clone(),
                last_run: format_ts(&now),
                events: out.events.len(),
            },
        ));
        reports.push(CollectorReport {
            name,
            enabled,
            ran: true,
            ok: out.ok,
            events: out.events.len(),
            message: out.message,
            fix: out.fix,
        });
        if let Some(since) = out.since {
            for e in &out.events {
                if !stamps.since.iter().any(|(s, _)| *s == e.source) {
                    stamps.since.push((e.source, since));
                }
            }
        }
        events.extend(out.events);
    }
    // one capture reads chronologically in the ledger; ties keep collector
    // and log order, so a transaction's lines keep theirs
    events.sort_by_key(|e| e.ts);
    Collected {
        events,
        reports,
        states,
        stamps,
    }
}

/// The logbook's `created` time from `.seldon/logbook.toml`.
fn created(logbook: &Logbook) -> Result<DateTime<FixedOffset>> {
    let text = logbook.meta.created.to_string();
    DateTime::parse_from_rfc3339(&text).map_err(|e| {
        anyhow::anyhow!("logbook.toml: created `{text}` is not an RFC 3339 date-time: {e}").into()
    })
}

fn render(
    logbook: &Logbook,
    written: &[Event],
    reports: &[CollectorReport],
    since_ignored: &[&str],
    explained: usize,
) -> Output {
    let ok = reports.iter().all(|r| r.ok);
    let mut files: Vec<String> = written
        .iter()
        .map(|e| format!("ledger/{}.jsonl", e.month()))
        .collect();
    files.dedup();
    let collectors: Vec<_> = reports
        .iter()
        .map(|r| {
            let mut c = json!({
                "name": r.name, "enabled": r.enabled, "ran": r.ran, "ok": r.ok, "events": r.events,
            });
            if let Some(m) = &r.message {
                c["message"] = json!(m);
            }
            if let Some(f) = &r.fix {
                c["fix"] = json!(f);
            }
            c
        })
        .collect();
    let json = json!({
        "ok": ok,
        "logbook": logbook.root,
        "written": written.len(),
        "files": files,
        "collectors": collectors,
        "sinceIgnored": since_ignored,
        "explainedOwn": explained,
    });

    let mut human = format!("Captured {} new event(s).", written.len());
    for r in reports.iter().filter(|r| r.ran) {
        let _ = write!(human, "\n  {:<8} {:>4}", r.name, r.events);
        if !r.ok {
            human.push_str("  degraded");
        }
        if let Some(m) = &r.message {
            let _ = write!(human, "  {m}");
        }
        if let Some(f) = &r.fix {
            let _ = write!(human, "\n           fix: {f}");
        }
    }
    if explained > 0 {
        let _ = write!(
            human,
            "\nnote: {explained} config event(s) explained as written by seldon itself"
        );
    }
    if !since_ignored.is_empty() {
        let _ = write!(
            human,
            "\nnote: --since ignored for {} (they continue from their cursor)",
            since_ignored.join(", ")
        );
    }
    Output::ok(human, json)
}
