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
//! --theme-hook`, `hook install`; SPEC-ENGINE §5 rule 7), and every
//! capture explains Seldon's own plugin and package changes (rule 8),
//! also those an earlier capture left open (WP-088).
//!
//! A collector that took a new baseline because its state was missing,
//! unreadable or another logbook's ([`collectors::Lost`]) although the
//! ledger holds events of its source lost the changes in between. The
//! capture records that as one `seldon` `note` with the subject
//! [`STATE_RESET`] and prints a warning (WP-081). A collector that never
//! ran successfully for this logbook loses nothing (`loss`), and the first
//! capture of a logbook holds no events of its sources: no note.
//! A collector that degrades (WP-088) or is not run (WP-091) in the
//! capture that loses its state takes no baseline then; it is marked
//! `pendingBaseline` in `cursors.json` with what it lost (`cursors` or
//! `logbook`; a collector not run gets an entry with only the mark), and
//! its first successful run records its gap the same way.
//! When the snapper collector changes between degraded and ok since its
//! last run for this logbook (read access granted or removed, ADR-0026),
//! the capture records a `seldon` `note` with the subject `snapper`
//! ([`access_change`], WP-091).
//! A corrupt `owned.json` counts for the config collector; a capture that
//! runs that collector moves it to `owned.json.bad` after the ledger write,
//! so the next capture does not report it again.
//! `doctor` predicts the reset before the capture with the same gate and
//! ledger rule ([`pending_reset`], WP-083), while a restore prevents it.

use std::fmt::Write as _;
use std::path::Path;

use chrono::{DateTime, FixedOffset, Timelike as _};
use serde_json::json;

use super::{Context, Output};
use crate::attribution::{self, Stamps};
use crate::collectors::config::OwnWrites;
use crate::collectors::{
    self, CollectorState, Ctx, Cursors, Lost, PendingBaseline, REGISTRY, STATE_RESET, Sources, Tz,
};
use crate::config::Config;
use crate::error::{Error, Result};
use crate::ledger::Ledger;
use crate::logbook::{Logbook, lock};
use crate::model::event::{Event, Kind, Meta, Source, format_ts};
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
    let binding = Binding::of(&cursors, &logbook.root);
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
    let owned_file = OwnWrites::file(&ctx.dirs);
    let mut owned_corrupt = is_corrupt(&owned_file);
    let Collected {
        mut events,
        reports,
        mut states,
        stamps,
        lost,
    } = collect_all(
        &selected, &config, ctx, &ledger, &cursors, &logbook, &sources, now, baseline,
    );
    let mut lost = losses(lost, binding, &cursors, &logbook.root);
    let not_run: Vec<&'static str> = selected
        .iter()
        .filter(|(_, run)| !run)
        .map(|(name, _)| *name)
        .collect();
    let waiting = waiting_baselines(&ledger, &states, &not_run, binding, &cursors, &logbook.root)?;
    let mark = |name: &str| {
        waiting
            .iter()
            .find(|(n, _)| *n == name)
            .and_then(|(_, l)| PendingBaseline::of(*l))
    };
    for (name, state) in &mut states {
        state.pending_baseline = mark(name);
    }
    // not run: an entry with only the mark, which the index reads as none
    let bare: Vec<(&'static str, CollectorState)> = not_run
        .iter()
        .filter_map(|&name| Some((name, CollectorState::waiting(mark(name)?))))
        .collect();
    owned_corrupt &= reports.iter().any(|r| r.name == "config" && r.ran && r.ok);
    if owned_corrupt {
        lost.push(("config", Lost::Owned));
    }
    attribution::attribute_capture(&ledger, &mut events, &ctx.dirs.home, &stamps)?;
    let reset = state_reset(&ledger, &lost, baseline, now)?;
    events.extend(reset.as_ref().map(|r| r.note.clone()));
    events.extend(access_change(&cursors, &states, now));

    let written = ledger.append(&lock, events)?;
    for (name, state) in states.into_iter().chain(bare) {
        cursors.collectors.insert(name.to_string(), state);
    }
    cursors.save(&cursors_path)?;
    let mut warnings = Vec::new();
    if let Some(reset) = &reset {
        warnings.push(reset_warning(ctx, reset));
    }
    // seen (and recorded): the next capture must not report it again
    if owned_corrupt
        && let Err(e) = std::fs::rename(&owned_file, owned_file.with_extension("json.bad"))
    {
        let shown = ctx.dirs.display(&owned_file);
        warnings.push(format!("cannot move {shown} aside: {e}"));
    }
    // WP-008: reconciliation, the attributed collector event ids into
    // their case files (ADR-0012 §10); warnings only, the append is done
    crate::reconcile::after_capture(&logbook, &ledger, &written);
    // rule 7: only a run of the config collector has seen the own writes
    let mut explained = 0;
    if reports.iter().any(|r| r.name == "config" && r.ran && r.ok) {
        let (n, warnings) =
            crate::reconcile::explain_own_writes(&lock, &ledger, &owned_file, &written, now);
        explained = n;
        for w in warnings {
            eprintln!("seldon: warning: {w}");
        }
    }
    // rule 8: Seldon updating itself is no drift, also what an earlier
    // capture left open
    let (explained_self, own_warnings) =
        crate::reconcile::explain_own_changes(&lock, &ledger, &written, now);
    for w in own_warnings {
        eprintln!("seldon: warning: {w}");
    }
    crate::index::rebuild_if_initialised(ctx);
    drop(lock);

    Ok(render(
        &logbook,
        &written,
        &reports,
        &since_ignored,
        (explained, explained_self),
        &warnings,
    ))
}

/// What `cursors.json` was bound to before this capture bound it to the
/// logbook.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Binding {
    /// No file, or no logbook in it: a new or lost state directory.
    None,
    This,
    Other,
}

impl Binding {
    /// The binding of `cursors` as loaded, before [`Cursors::bind`].
    pub(crate) fn of(cursors: &Cursors, logbook: &Path) -> Binding {
        match &cursors.logbook {
            None => Binding::None,
            Some(bound) if bound == logbook => Binding::This,
            Some(_) => Binding::Other,
        }
    }
}

/// The baselines among `lost` that are a loss ([`loss`]), with the
/// [`pending_baseline`] of each.
fn losses(
    lost: Vec<(&'static str, Lost)>,
    binding: Binding,
    cursors: &Cursors,
    logbook: &Path,
) -> Vec<(&'static str, Lost)> {
    lost.into_iter()
        .filter_map(|(name, l)| {
            let had_cursor = cursors.cursor(logbook, name).is_some();
            let pending = pending_baseline(cursors, name);
            Some((name, loss(l, binding, had_cursor, pending)?))
        })
        .collect()
}

/// The baseline `name` waits for in `cursors` since it degraded in the
/// capture that lost its state ([`CollectorState::pending_baseline`],
/// WP-088). Only asked for cursors bound to this logbook: [`loss`]
/// decides the others by the binding.
fn pending_baseline(cursors: &Cursors, name: &str) -> Option<PendingBaseline> {
    cursors.collectors.get(name)?.pending_baseline
}

/// The baselines `names` take from `cursors`: each whose cursor for
/// `logbook` is missing or does not read
/// ([`collectors::Collector::cursor_reads`]), as [`Lost::Cursor`].
fn baselines(
    cursors: &Cursors,
    logbook: &Path,
    names: impl IntoIterator<Item = &'static str>,
) -> Vec<(&'static str, Lost)> {
    names
        .into_iter()
        .filter_map(|name| {
            let collector = collectors::find(name)?;
            let reads = cursors
                .cursor(logbook, name)
                .is_some_and(|c| collector.cursor_reads(c));
            (!reads).then_some((name, Lost::Cursor))
        })
        .collect()
}

/// The collectors that lost their state although they took no baseline in
/// this capture, and what they lost: among `states` those that degraded
/// (WP-088), and among `not_run` those without an entry in `cursors` (as
/// bound for this capture), whose state the capture drops (WP-091; one
/// with an entry keeps it as it is). The baseline each would have taken
/// waits, through the capture's gate ([`losses`]: [`Lost::Cursor`] or
/// [`Lost::Logbook`]) and ledger rule ([`held_losses`]); a collector
/// already waiting stays so, with its kind. Its first successful run
/// records the gap as a state reset.
fn waiting_baselines(
    ledger: &Ledger,
    states: &[(&'static str, CollectorState)],
    not_run: &[&'static str],
    binding: Binding,
    cursors: &Cursors,
    logbook: &Path,
) -> Result<Vec<(&'static str, Lost)>> {
    let degraded = states.iter().filter(|(_, s)| !s.ok).map(|(n, _)| *n);
    let dropped = not_run
        .iter()
        .copied()
        .filter(|n| !cursors.collectors.contains_key(*n));
    let lost = losses(
        baselines(cursors, logbook, degraded.chain(dropped)),
        binding,
        cursors,
        logbook,
    );
    held_losses(ledger, &lost)
}

/// The state reset the next `seldon capture` would record, predicted from
/// `cursors` before it runs (WP-083, `doctor`): each enabled collector
/// whose cursor for `logbook` is missing or does not read
/// ([`collectors::Collector::cursor_reads`]) takes a baseline, and that
/// baseline goes through the capture's own gate ([`loss`]) and ledger rule
/// ([`held_losses`]); a collector whose baseline waits since it degraded
/// in an earlier capture ([`pending_baseline`]) counts as the loss it
/// recorded (`cursors`, or `logbook`). Not predicted: whether a collector degrades in that
/// capture (it then takes no baseline, and its baseline waits), and the
/// config collector's `manifest.json` and `owned.json` losses (doctor's
/// `state` rows check those files). Also returns the binding, which names
/// the cause.
pub(crate) fn pending_reset(
    ledger: &Ledger,
    config: &Config,
    cursors: &Cursors,
    logbook: &Path,
) -> Result<(Binding, Vec<(&'static str, Lost)>)> {
    let binding = Binding::of(cursors, logbook);
    let enabled = select(config, &CaptureArgs::default())?
        .into_iter()
        .filter(|&(_, run)| run)
        .map(|(name, _)| name);
    let lost = losses(
        baselines(cursors, logbook, enabled),
        binding,
        cursors,
        logbook,
    );
    Ok((binding, held_losses(ledger, &lost)?))
}

/// Whether a collector's baseline for want of `lost` is a loss. Bound to
/// another logbook, a missing cursor is [`Lost::Logbook`]. Bound to this
/// logbook, a collector without a cursor never ran successfully here
/// (degraded so far, disabled, or new): its first baseline loses nothing,
/// even when the ledger holds events of its source that something else
/// wrote (the theme hook, an agent); only a cursor that is there and does
/// not read is lost, or a baseline that waits since the collector
/// degraded in the capture that lost its state (`pending`, WP-088): that
/// one is the loss it recorded then, also `logbook`.
fn loss(
    lost: Lost,
    binding: Binding,
    had_cursor: bool,
    pending: Option<PendingBaseline>,
) -> Option<Lost> {
    match (lost, binding) {
        (Lost::Cursor, Binding::Other) => Some(Lost::Logbook),
        (Lost::Cursor, Binding::This) => match pending {
            Some(p) => Some(p.lost()),
            None => had_cursor.then_some(Lost::Cursor),
        },
        (l, _) => Some(l),
    }
}

/// The `seldon` note for the snapper collector changing between degraded
/// and ok since its last run for this logbook (WP-091): the user granted
/// or removed its read access (ADR-0026), or snapper failed or recovered
/// otherwise; the detail carries the message. `cursors` as bound for this
/// capture, so another logbook's state, a lost state directory and an
/// entry that never ran (only the mark) give no note; nor does a capture
/// that does not run snapper.
fn access_change(
    cursors: &Cursors,
    states: &[(&'static str, CollectorState)],
    now: DateTime<FixedOffset>,
) -> Option<Event> {
    const NAME: &str = "snapper";
    let (_, new) = states.iter().find(|(n, _)| *n == NAME)?;
    let old = cursors.collectors.get(NAME)?;
    if old.last_run.is_none() || old.ok == new.ok {
        return None;
    }
    let message = |s: &CollectorState| s.message.clone().unwrap_or_else(|| "failed".into());
    let detail = match (new.ok, &new.message) {
        (true, None) => format!(
            "snapper collector ok again; at its last run it was degraded: {}",
            message(old)
        ),
        (true, Some(current)) => format!(
            "snapper collector ok again ({current}); at its last run it was degraded: {}",
            message(old)
        ),
        (false, _) => format!(
            "snapper collector degraded: {}; at its last run it was ok",
            message(new)
        ),
    };
    Some(Event::new(now, Source::Seldon, Kind::Note, NAME).detail(detail))
}

/// Whether `path` exists and is not a valid `owned.json` (the collector
/// reads such a file as empty).
fn is_corrupt(path: &std::path::Path) -> bool {
    std::fs::read(path).is_ok_and(|b| serde_json::from_slice::<OwnWrites>(&b).is_err())
}

/// A state reset to record: the note, and the losses it names.
#[derive(Debug, Clone)]
struct Reset {
    note: Event,
    lost: Vec<(&'static str, Lost)>,
}

/// The state reset of the collectors in `lost` whose source the ledger
/// already holds events of; `None` when there is none (no loss, or the
/// first capture of a logbook).
fn state_reset(
    ledger: &Ledger,
    lost: &[(&'static str, Lost)],
    baseline: DateTime<FixedOffset>,
    now: DateTime<FixedOffset>,
) -> Result<Option<Reset>> {
    let lost = held_losses(ledger, lost)?;
    if lost.is_empty() {
        return Ok(None);
    }
    let sources = reset_sources(&lost);
    let named: Vec<String> = sources
        .iter()
        .map(|s| {
            let kinds: Vec<&str> = lost
                .iter()
                .filter(|(n, _)| n == s)
                .map(|(_, l)| l.as_str())
                .collect();
            format!("{s} ({})", kinds.join(", "))
        })
        .collect();
    let mut files: Vec<Lost> = lost.iter().map(|(_, l)| *l).collect();
    files.sort();
    files.dedup();
    let files: Vec<&str> = files.iter().map(|l| l.as_str()).collect();
    let mut meta = Meta::default();
    meta.extra
        .insert("sources".into(), json!(sources.join(",")));
    meta.extra.insert("files".into(), json!(files.join(",")));
    let note = Event::new(now, Source::Seldon, Kind::Note, STATE_RESET)
        .detail(format!(
            "state directory missing, unreadable or bound to another logbook: new baseline for {} at {}, recorded {}; changes made in between may not be recorded",
            named.join(", "),
            format_ts(&baseline),
            format_ts(&now)
        ))
        .meta(meta);
    Ok(Some(Reset { note, lost }))
}

/// The losses among `lost` whose source the ledger holds at least one event
/// of (the WP-081 rule: the first capture of a logbook loses nothing).
fn held_losses(
    ledger: &Ledger,
    lost: &[(&'static str, Lost)],
) -> Result<Vec<(&'static str, Lost)>> {
    let mut wanted: Vec<Source> = lost.iter().filter_map(|(n, _)| n.parse().ok()).collect();
    wanted.sort_by_key(|s| s.as_str());
    wanted.dedup();
    if wanted.is_empty() {
        return Ok(Vec::new());
    }
    let held = recorded_sources(ledger, &wanted)?;
    Ok(lost
        .iter()
        .copied()
        .filter(|(n, _)| held.iter().any(|s| s.as_str() == *n))
        .collect())
}

/// The collector names of `lost`, each once, in run order.
fn reset_sources(lost: &[(&'static str, Lost)]) -> Vec<&'static str> {
    let mut sources: Vec<&'static str> = Vec::new();
    for (n, _) in lost {
        if !sources.contains(n) {
            sources.push(n);
        }
    }
    sources
}

/// The sources among `wanted` the ledger holds at least one event of,
/// reading months newest first until all are found.
fn recorded_sources(ledger: &Ledger, wanted: &[Source]) -> Result<Vec<Source>> {
    let mut found = Vec::new();
    for month in ledger.months()?.iter().rev() {
        for e in ledger.read_month(month)?.events {
            if wanted.contains(&e.source) && !found.contains(&e.source) {
                found.push(e.source);
            }
        }
        if found.len() == wanted.len() {
            break;
        }
    }
    Ok(found)
}

/// The capture's warning for a state reset: what was lost and where the
/// restore steps are.
fn reset_warning(ctx: &Context, reset: &Reset) -> String {
    let hint = if reset.lost.iter().any(|(_, l)| *l == Lost::Logbook) {
        "Nothing can be restored: the state belonged to another logbook path, and the new baseline is this logbook's (user guide: Moving or copying the logbook)"
    } else {
        "If you have a backup of it, restore it and run `seldon capture` again (user guide: Back up and restore the state directory)"
    };
    format!(
        "state reset recorded: {} took a new baseline because {} was missing, unreadable or bound to another logbook, so changes made in between may be missing. {hint}",
        reset_sources(&reset.lost).join(", "),
        ctx.dirs.display(&ctx.dirs.state_dir)
    )
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
    /// Collectors that took a new baseline, and the state file they missed.
    lost: Vec<(&'static str, Lost)>,
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
    let mut lost = Vec::new();
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
        if out.ok
            && let Some(l) = out.baseline
        {
            lost.push((name, l));
        }
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
                last_run: Some(format_ts(&now)),
                events: out.events.len(),
                // set by `run` for a degraded run
                pending_baseline: None,
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
        lost,
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
    (explained, explained_self): (usize, usize),
    warnings: &[String],
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
        "explainedSelf": explained_self,
        "warnings": warnings,
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
    if explained_self > 0 {
        let _ = write!(
            human,
            "\nnote: {explained_self} event(s) explained as seldon updating itself"
        );
    }
    if !since_ignored.is_empty() {
        let _ = write!(
            human,
            "\nnote: --since ignored for {} (they continue from their cursor)",
            since_ignored.join(", ")
        );
    }
    for w in warnings {
        let _ = write!(human, "\nwarning: {w}");
    }
    Output::ok(human, json)
}
