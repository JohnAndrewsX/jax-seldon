//! `seldon import omarchy-agent <vault> [--dry-run|--apply]` (SPEC-ENGINE
//! §3, WP-043): the kit's vault into this logbook, dry run first.
//!
//! Both modes build the same plan ([`crate::import::omarchy_agent`]) under
//! the lock and write `outputs/IMPORT-omarchy-agent.md` (only on change).
//! The dry run writes nothing else and commits the report as `seldon:
//! import omarchy-agent (dry run)`. `--apply` refuses while the plan has
//! errors; otherwise it appends one `note` per case to the ledger, writes
//! the cases, journal days, memory files and deviation rows, the report and
//! the marker `.seldon/imports/omarchy-agent.json`, commits once as
//! `seldon: import omarchy-agent` and rebuilds the index. A write that
//! fails half way says how to undo it (nothing is committed yet). The
//! marker makes every later run a no-op ("nothing changed"); import notes
//! in the ledger without the marker are refused with the same undo, so the
//! import never runs twice and is never reported done when it is not.

use std::path::PathBuf;

use clap::{Args, Subcommand};
use serde_json::{Value, json};

use super::event::emit;
use super::{Commit, Context, Output, autocommit, write_new};
use crate::config::Config;
use crate::error::{Error, Result};
use crate::import::omarchy_agent::{self, Plan, SOURCE};
use crate::import::report::{self, Mode};
use crate::import::{marker_path, report_path};
use crate::ledger::Ledger;
use crate::logbook::lock::Lock;
use crate::logbook::{Logbook, git};
use crate::model::event::{Event, Kind, Meta, Source};
use crate::model::{self, event::format_ts};
use crate::redact::Redactor;
use crate::sys;

#[derive(Debug, Clone, Args)]
pub struct ImportArgs {
    #[command(subcommand)]
    pub source: ImportSource,
}

#[derive(Debug, Clone, Subcommand)]
pub enum ImportSource {
    /// The omarchy-agent kit's Obsidian vault (read only; dry run unless --apply)
    OmarchyAgent(OmarchyAgentArgs),
}

#[derive(Debug, Clone, Args)]
pub struct OmarchyAgentArgs {
    /// The vault directory (the one with pipeline/, journal/, knowledge/)
    #[arg(value_name = "VAULT")]
    pub vault: PathBuf,

    /// Only write the report outputs/IMPORT-omarchy-agent.md (the default)
    #[arg(long, conflicts_with = "apply")]
    pub dry_run: bool,

    /// Import: cases, journal, memory and deviation rows, in one commit
    #[arg(long)]
    pub apply: bool,
}

pub fn run(ctx: &Context, args: ImportArgs) -> Result<Output> {
    match args.source {
        ImportSource::OmarchyAgent(a) => omarchy_agent(ctx, a),
    }
}

fn omarchy_agent(ctx: &Context, args: OmarchyAgentArgs) -> Result<Output> {
    let (config, logbook) = ctx.open_logbook()?;
    let vault = ctx.dirs.expand(&args.vault.to_string_lossy());
    let shown = ctx.dirs.display(&vault);
    let lock = ctx.lock()?;
    let mode = if args.apply { "apply" } else { "dry-run" };

    if let Some(previous) = already_imported(&logbook)? {
        drop(lock);
        let human = format!(
            "Nothing changed: {SOURCE} was already imported into this logbook ({}).",
            previous["by"].as_str().unwrap_or_default()
        );
        return Ok(Output::ok(
            human,
            json!({
                "mode": mode,
                "changed": false,
                "alreadyImported": previous,
                "vault": shown,
                "files": [],
                "git": Commit::Skipped("nothing changed").json(),
            }),
        ));
    }

    let redactor = Redactor::with_patterns(&config.redaction.patterns)?;
    let plan = omarchy_agent::plan(&vault, shown, &logbook, redactor, &ctx.now)?;
    let report_rel = report_path(SOURCE);

    if !args.apply || plan.errors() > 0 {
        let changed = write_report(&logbook, &plan, &Mode::DryRun)?;
        let commit = if changed {
            autocommit(
                ctx,
                &config,
                &logbook,
                &format!("import {SOURCE} (dry run)"),
            )
        } else {
            Commit::Skipped("nothing changed")
        };
        drop(lock);
        if args.apply {
            return Err(Error::user(format!(
                "{} error(s) in the vault; nothing was imported. See {report_rel}.",
                plan.errors()
            )));
        }
        let files: Vec<&str> = if changed {
            vec![report_rel.as_str()]
        } else {
            Vec::new()
        };
        let mut human = format!(
            "Dry run of the {SOURCE} import from {}:\n{}\nReport: {report_rel}\nApply: seldon import {SOURCE} <vault> --apply",
            plan.vault,
            summary(&plan)
        );
        human.push_str(&commit.human());
        return Ok(Output::ok(
            human,
            plan_json(&plan, mode, changed, &files, &commit, None),
        ));
    }

    // the ledger first: if it cannot be written, nothing else is
    for c in &plan.cases {
        if logbook.path(&c.path).exists() {
            return Err(Error::user(format!("{} already exists", c.path)));
        }
    }
    let files =
        write_plan(ctx, &config, &logbook, &lock, &plan).map_err(|e| after_failure(&logbook, e))?;
    let marker_rel = marker_path(SOURCE);

    let commit = autocommit(ctx, &config, &logbook, &format!("import {SOURCE}"));
    crate::index::rebuild_if_initialised(ctx);
    drop(lock);

    let mut human = format!(
        "Imported from {}:\n{}\nReport: {report_rel}",
        plan.vault,
        summary(&plan)
    );
    human.push_str(&commit.human());
    let files: Vec<&str> = files.iter().map(String::as_str).collect();
    Ok(Output::ok(
        human,
        plan_json(&plan, mode, true, &files, &commit, Some(&marker_rel)),
    ))
}

/// Writes the plan: ledger notes, cases, journal days, memory files,
/// deviation rows, the report and the marker. Returns the files written.
fn write_plan(
    ctx: &Context,
    config: &Config,
    logbook: &Logbook,
    lock: &Lock,
    plan: &Plan,
) -> Result<Vec<String>> {
    let events: Vec<Event> = plan.cases.iter().map(note).collect();
    let written = emit(lock, config, logbook, events)?;
    let mut files: Vec<String> = Vec::new();
    for (c, event) in plan.cases.iter().zip(&written) {
        let mut case = c.case.clone();
        case.events.push(event.id.to_string());
        write_new(&logbook.path(&c.path), &model::render_new(&case, &c.body))?;
        files.push(c.path.clone());
    }
    for d in &plan.days {
        sys::write_atomic(&logbook.path(&d.path), d.text.as_bytes())?;
        files.push(d.path.clone());
    }
    for m in &plan.memory {
        sys::write_atomic(&logbook.path(&m.path), m.text.as_bytes())?;
        files.push(m.path.clone());
    }
    if let Some(dossier) = &plan.dossier {
        files.extend(dossier.write()?);
    }
    let at = ctx.now.format("%Y-%m-%d %H:%M").to_string();
    if write_report(logbook, plan, &Mode::Applied(at))? {
        files.push(report_path(SOURCE));
    }
    let marker_rel = marker_path(SOURCE);
    let marker = json!({
        "source": SOURCE,
        "vault": plan.vault,
        "importedAt": format_ts(&ctx.now),
        "cases": plan.id_map(),
        "counts": report::counts(plan),
    });
    sys::write_atomic(
        &logbook.path(&marker_rel),
        format!("{}\n", serde_json::to_string_pretty(&marker).expect("json")).as_bytes(),
    )?;
    files.push(marker_rel);
    let mut months: Vec<String> = written
        .iter()
        .map(|e| format!("ledger/{}.jsonl", e.month()))
        .collect();
    months.sort();
    months.dedup();
    files.extend(months);

    Ok(files)
}

/// The undo for an apply that failed after it began to write: nothing is
/// committed yet, so the logbook's git state is the way back.
fn after_failure(logbook: &Logbook, e: Error) -> Error {
    let hint = if git::is_repo(&logbook.root) {
        format!(
            "the import stopped half way and nothing was committed; undo it with `git checkout -- . && git clean -fd` in {}, then run it again",
            logbook.root.display()
        )
    } else {
        "the import stopped half way; the logbook is not a git repository, so restore it from a backup before running it again".to_string()
    };
    match e {
        Error::User(m) => Error::User(format!("{m}; {hint}")),
        Error::Engine(a) => Error::Engine(anyhow::anyhow!("{a:#}; {hint}")),
        other => other,
    }
}

/// The ledger note of an imported case: at its `created` date, by the
/// human who runs the import.
fn note(c: &omarchy_agent::PlannedCase) -> Event {
    let mut meta = Meta::default();
    for (key, value) in [
        ("import", SOURCE),
        ("originalId", c.from.as_str()),
        ("originalStatus", c.kit_status.as_str()),
        ("source", c.source.as_str()),
    ] {
        meta.extra.insert(key.into(), Value::String(value.into()));
    }
    Event::new(c.ts, Source::Manual, Kind::Note, &c.to)
        .detail(format!("{} (imported from {SOURCE})", c.case.title))
        .actor("human")
        .case(Some(c.to.clone()))
        .meta(meta)
}

/// The marker, when this logbook has the kit imported (`None` when not).
/// Import notes in the ledger without the marker mean an apply that did
/// not finish, or a deleted `.seldon/`: a user error that says which, so
/// the import is never run twice and never reported done when it is not.
fn already_imported(logbook: &Logbook) -> Result<Option<Value>> {
    let rel = marker_path(SOURCE);
    match std::fs::read_to_string(logbook.path(&rel)) {
        Ok(text) => {
            let marker: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
            return Ok(Some(json!({
                "by": rel,
                "importedAt": marker.get("importedAt").cloned().unwrap_or(Value::Null),
            })));
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => {
            return Err(anyhow::Error::new(e)
                .context(format!("cannot read {rel}"))
                .into());
        }
    }
    let ledger = Ledger::new(logbook, Redactor::builtin());
    let found = ledger
        .read_all()?
        .into_iter()
        .find(|e| e.meta.extra.get("import").and_then(Value::as_str) == Some(SOURCE));
    match found {
        None => Ok(None),
        Some(e) => Err(Error::user(format!(
            "ledger/{}.jsonl has {SOURCE} import notes ({}) but {rel} is missing: an earlier --apply did not finish, or .seldon/ was deleted. After a failed apply, undo it with `git checkout -- . && git clean -fd` in {} and run it again; after a deleted .seldon/ the import is complete",
            e.month(),
            e.id,
            logbook.root.display()
        ))),
    }
}

/// Writes the report (text outside its fence kept); `true` when it changed.
fn write_report(logbook: &Logbook, plan: &Plan, mode: &Mode) -> Result<bool> {
    let path = logbook.path(report_path(SOURCE));
    let existing = match std::fs::read_to_string(&path) {
        Ok(t) => Some(t),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => {
            return Err(anyhow::Error::new(e)
                .context(format!("cannot read {}", path.display()))
                .into());
        }
    };
    let text = report::merge(existing.as_deref(), &report::render(plan, mode));
    if existing.as_deref() == Some(text.as_str()) {
        return Ok(false);
    }
    sys::write_atomic(&path, text.as_bytes())?;
    Ok(true)
}

/// Two lines for the human output.
fn summary(plan: &Plan) -> String {
    format!(
        "  {} case(s) ({} renumbered), {} journal session(s) on {} day(s), {} memory section(s), {} deviation row(s)\n  {} file(s) not imported, {} error(s), {} redacted line(s), {} private path(s) rewritten, {} id rewrite(s), {} assumption(s)",
        plan.cases.len(),
        plan.collisions().count(),
        plan.sessions,
        plan.days.len(),
        plan.memory_sections(),
        plan.rows.len(),
        plan.skipped.iter().filter(|s| !s.error).count(),
        plan.errors(),
        plan.hits.len(),
        plan.private_paths,
        plan.rewrite_totals().0 + plan.rewrite_totals().1,
        plan.cases.iter().filter(|c| c.assumption.is_some()).count()
    )
}

fn plan_json(
    plan: &Plan,
    mode: &str,
    changed: bool,
    files: &[&str],
    commit: &Commit,
    marker: Option<&str>,
) -> Value {
    let cases: Vec<Value> = plan
        .cases
        .iter()
        .map(|c| {
            json!({
                "from": c.from,
                "to": c.to,
                "kitStatus": c.kit_status,
                "status": c.case.status.as_str(),
                "path": c.path,
                "source": c.source,
                "renumbered": c.collision.is_some(),
            })
        })
        .collect();
    let collisions: Vec<Value> = plan
        .collisions()
        .map(|c| json!({ "from": c.from, "to": c.to, "takenBy": c.collision }))
        .collect();
    let rewrites: Vec<Value> = plan
        .rewrites
        .iter()
        .map(|(file, (links, ids))| json!({ "file": file, "wikilinks": links, "ids": ids }))
        .collect();
    let assumptions: Vec<Value> = plan
        .cases
        .iter()
        .filter_map(|c| {
            let a = c.assumption.as_ref()?;
            Some(json!({ "case": c.to, "source": c.source, "assumption": a }))
        })
        .collect();
    let skipped: Vec<Value> = plan
        .skipped
        .iter()
        .map(|s| json!({ "path": s.path, "reason": s.reason, "error": s.error }))
        .collect();
    json!({
        "mode": mode,
        "changed": changed,
        "alreadyImported": Value::Null,
        "vault": plan.vault,
        "report": report_path(SOURCE),
        "errors": plan.errors(),
        "counts": report::counts(plan),
        "cases": cases,
        "collisions": collisions,
        "rewrites": rewrites,
        "assumptions": assumptions,
        "skipped": skipped,
        "files": files,
        "marker": marker,
        "git": commit.json(),
    })
}
