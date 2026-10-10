//! `seldon import omarchy-agent <vault> [--dry-run|--apply]` (SPEC-ENGINE
//! §3, WP-043): the kit's vault into this logbook, dry run first.
//! `seldon import task <FILE>…` (WP-102) lives in [`task`].
//!
//! Both modes build the same plan ([`crate::import::omarchy_agent`]) under
//! the lock and write `outputs/IMPORT-omarchy-agent.md` (only on change).
//! The dry run writes nothing else and commits the report as `seldon:
//! import omarchy-agent (dry run)`. `--apply` refuses while the plan has
//! errors; otherwise it appends one `note` for the apply and one per case
//! to the ledger, writes
//! the cases, journal days, memory files and deviation rows, the report and
//! the marker `.seldon/imports/omarchy-agent.json`, commits once as
//! `seldon: import omarchy-agent` and rebuilds the index. Before its first
//! write it commits the logbook's pending changes (`seldon: before import
//! omarchy-agent`) and refuses to start while any are left (WP-061). A
//! write that fails half way prints the undo, which restores only the
//! files the import wrote from that commit and removes the ones it created
//! (also kept in `.seldon/imports/omarchy-agent.undo.json`). The marker
//! makes every later run a no-op ("nothing changed"); import notes in the
//! ledger without the marker are refused with the same undo, so the import
//! never runs twice and is never reported done when it is not. The apply's
//! own note makes that hold for a vault without cases too (F-141).

use std::path::{Path, PathBuf};

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

pub(crate) mod task;

#[derive(Debug, Clone, Args)]
pub struct ImportArgs {
    #[command(subcommand)]
    pub source: ImportSource,
}

#[derive(Debug, Clone, Subcommand)]
pub enum ImportSource {
    /// Import the omarchy-agent kit's Obsidian vault, which is only read (dry run unless --apply)
    OmarchyAgent(OmarchyAgentArgs),
    /// Import your Markdown task files as cases: one queued case per open
    /// `- [ ]` item (applies unless --dry-run; the files are only read)
    Task(task::TaskArgs),
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
        ImportSource::Task(a) => task::run(ctx, a),
    }
}

fn omarchy_agent(ctx: &Context, args: OmarchyAgentArgs) -> Result<Output> {
    let (config, logbook) = ctx.open_logbook()?;
    let vault = vault_path(ctx, &args.vault);
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
    if args.apply {
        // the import's folders, before the plan reads them (WP-168)
        for rel in IMPORT_FOLDERS {
            logbook.checked_dir(rel)?;
        }
    }
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
    check_folders(&logbook, &plan)?;
    commit_pending(ctx, &config, &logbook)?;
    let mut undo = Undo::new(&logbook);
    let files = write_plan(ctx, &config, &logbook, &lock, &plan, &mut undo)
        .map_err(|e| after_failure(&logbook, &undo, e))?;
    // the undo of an earlier failure in the ledger write (no note, so no
    // refusal reported it) is moot now
    let _ = std::fs::remove_file(logbook.path(undo_path()));
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

/// The folders an apply writes into (SPEC-ENGINE §3 `import`).
const IMPORT_FOLDERS: [&str; 9] = [
    "ledger",
    "work/queued",
    "work/active",
    "work/completed",
    "journal",
    "memory",
    "system",
    "outputs",
    ".seldon/imports",
];

/// Every folder the apply writes a file in is a real folder of the
/// logbook ([`Logbook::checked_file`], WP-168), checked before the ledger.
fn check_folders(logbook: &Logbook, plan: &Plan) -> Result<()> {
    let files = plan.cases.iter().map(|c| c.path.clone());
    let files = files.chain(plan.days.iter().map(|d| d.path.clone()));
    let files = files.chain(plan.memory.iter().map(|m| m.path.clone()));
    let files = files.chain(plan.dossier.iter().flat_map(|d| d.changed()));
    for rel in files.chain([report_path(SOURCE), marker_path(SOURCE), undo_path()]) {
        logbook.checked_file(&rel)?;
    }
    Ok(())
}

/// Writes the plan: ledger notes, cases, journal days, memory files,
/// deviation rows, the report and the marker. Returns the files written.
/// Every file is noted in `undo` before it is written.
fn write_plan(
    ctx: &Context,
    config: &Config,
    logbook: &Logbook,
    lock: &Lock,
    plan: &Plan,
    undo: &mut Undo,
) -> Result<Vec<String>> {
    // the apply's note first, then one per case (in the same order back)
    let events: Vec<Event> = std::iter::once(apply_note(ctx, plan))
        .chain(plan.cases.iter().map(note))
        .collect();
    for e in &events {
        undo.note(&format!("ledger/{}.jsonl", e.month()));
    }
    let written = emit(lock, config, logbook, events)?;
    let mut files: Vec<String> = Vec::new();
    for (c, event) in plan.cases.iter().zip(&written[1..]) {
        let mut case = c.case.clone();
        case.events.push(event.id.to_string());
        undo.note(&c.path);
        write_new(
            logbook,
            &logbook.path(&c.path),
            &model::render_new(&case, &c.body),
        )?;
        files.push(c.path.clone());
    }
    for d in &plan.days {
        undo.note(&d.path);
        sys::write_atomic_nofollow(&logbook.path(&d.path), d.text.as_bytes())?;
        files.push(d.path.clone());
    }
    for m in &plan.memory {
        undo.note(&m.path);
        sys::write_atomic_nofollow(&logbook.path(&m.path), m.text.as_bytes())?;
        files.push(m.path.clone());
    }
    if let Some(dossier) = &plan.dossier {
        for path in dossier.changed() {
            undo.note(&path);
        }
        files.extend(dossier.write()?);
    }
    let at = ctx.now.format("%Y-%m-%d %H:%M").to_string();
    undo.note(&report_path(SOURCE));
    if write_report(logbook, plan, &Mode::Applied(at))? {
        files.push(report_path(SOURCE));
    }
    let marker_rel = marker_path(SOURCE);
    undo.note(&marker_rel);
    let marker = json!({
        "source": SOURCE,
        "vault": plan.vault,
        "importedAt": format_ts(&ctx.now),
        "cases": plan.id_map(),
        "counts": report::counts(plan),
    });
    sys::write_atomic_nofollow(
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

/// Before the first write of `--apply`: the logbook's pending changes are
/// committed as `seldon: before import omarchy-agent` (the autocommit
/// rule), and the apply is refused while any are left (`--no-commit`,
/// `git.autocommit = false`, a failed commit). Then the undo of a failed
/// apply, which takes the import's files back to this commit, cannot lose
/// a change that is not the import's (F-142).
fn commit_pending(ctx: &Context, config: &Config, logbook: &Logbook) -> Result<()> {
    if !git::is_repo(&logbook.root) {
        return Ok(());
    }
    // a `.git` or `HEAD` git would wait on: refused like any file of the
    // logbook that is no regular file, exit 1 (WP-175)
    git::check_files(&logbook.root).map_err(Error::user)?;
    let dirty = || {
        git::is_dirty(&logbook.root)
            .map_err(|e| Error::from(anyhow::anyhow!("cannot read the logbook's git status: {e}")))
    };
    if !dirty()? {
        return Ok(());
    }
    let commit = autocommit(ctx, config, logbook, &format!("before import {SOURCE}"));
    if !dirty()? {
        return Ok(());
    }
    let why = match &commit {
        Commit::Skipped(reason) => format!(" (not committed: {reason})"),
        Commit::Failed(e) | Commit::Warned(e) => format!(" (not committed: {e})"),
        Commit::Committed(_) => String::new(),
    };
    Err(Error::user(format!(
        "the logbook has uncommitted changes{why}; commit them first (git -C {root} add -A && git -C {root} commit), then run the import again: a failed import can only be undone safely from a clean logbook",
        root = logbook.root.display()
    )))
}

/// The files an apply wrote, noted before each write: the ones that
/// existed are taken back to `base` (the commit before the import), the
/// new ones are removed. Nothing else in the logbook is touched.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
struct Undo {
    /// HEAD before the first write; `None` without a repository or commit.
    base: Option<String>,
    restore: Vec<String>,
    remove: Vec<String>,
    #[serde(skip)]
    root: PathBuf,
}

impl Undo {
    fn new(logbook: &Logbook) -> Self {
        Undo {
            base: git::head(&logbook.root),
            root: logbook.root.clone(),
            ..Undo::default()
        }
    }

    /// Notes `rel` before the import writes it (once).
    fn note(&mut self, rel: &str) {
        if self.restore.iter().chain(&self.remove).any(|r| r == rel) {
            return;
        }
        if self.root.join(rel).exists() {
            self.restore.push(rel.to_string());
        } else {
            self.remove.push(rel.to_string());
        }
    }

    /// Whether a kept undo can be printed as it is: a commit hash as
    /// `base` whenever files are restored, and every path relative, in one
    /// of the folders the import writes ([`UNDO_DIRS`], `.seldon/imports/`),
    /// never through a `.git`, without glob or pathspec-magic characters
    /// (the command also says `--literal-pathspecs`) and without control
    /// characters, and with no part of a path under `root` that is a
    /// symbolic link (a folder replaced by a link would take the printed
    /// `rm` out of the logbook). The file lives in the logbook, which
    /// agents and editors write too; a command built from anything else is
    /// not offered.
    fn is_sane(&self, root: &Path) -> bool {
        let hash = |b: &str| matches!(b.len(), 40 | 64) && b.chars().all(|c| c.is_ascii_hexdigit());
        let base_ok = match self.base.as_deref() {
            Some(b) => hash(b),
            None => self.restore.is_empty(),
        };
        let path_ok = |p: &String| {
            if p.chars()
                .any(|c| c.is_control() || matches!(c, '*' | '?' | '[' | '\\'))
                || p.starts_with(':')
            {
                return false;
            }
            let mut parts = Vec::new();
            for c in std::path::Path::new(p).components() {
                match c {
                    std::path::Component::Normal(part) if part != ".git" => parts.push(part),
                    _ => return false,
                }
            }
            match parts.as_slice() {
                [first, _, ..] if UNDO_DIRS.iter().any(|d| first == d) => true,
                [first, second, _, ..] => *first == ".seldon" && *second == "imports",
                _ => false,
            }
        };
        let no_link = |p: &String| {
            let mut at = root.to_path_buf();
            Path::new(p).components().all(|c| {
                at.push(c);
                !std::fs::symlink_metadata(&at).is_ok_and(|m| m.file_type().is_symlink())
            })
        };
        base_ok
            && self
                .restore
                .iter()
                .chain(&self.remove)
                .all(|p| path_ok(p) && no_link(p))
    }

    /// The shell line, run in the logbook.
    fn command(&self) -> String {
        let quoted = |paths: &[String]| {
            paths
                .iter()
                .map(|p| shell_quote(p))
                .collect::<Vec<_>>()
                .join(" ")
        };
        let mut steps = Vec::new();
        if !self.restore.is_empty() {
            let base = self
                .base
                .as_deref()
                .map_or(String::new(), |b| format!("{b} "));
            steps.push(format!(
                "git --literal-pathspecs checkout {base}-- {}",
                quoted(&self.restore)
            ));
        }
        if !self.remove.is_empty() {
            steps.push(format!("rm -f -- {}", quoted(&self.remove)));
        }
        steps.join(" && ")
    }
}

/// The logbook folders an apply writes into (besides `.seldon/imports/`).
const UNDO_DIRS: [&str; 6] = ["ledger", "work", "journal", "memory", "system", "outputs"];

/// Where a failed apply keeps its undo, for the next run's refusal.
fn undo_path() -> String {
    format!(".seldon/imports/{SOURCE}.undo.json")
}

/// `path` as one shell word: as is when it is plain, else single-quoted.
fn shell_quote(path: &str) -> String {
    if !path.is_empty()
        && path
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "._-/+@%,:".contains(c))
    {
        path.to_string()
    } else {
        format!("'{}'", path.replace('\'', "'\\''"))
    }
}

/// The undo for an apply that failed after it began to write: nothing of
/// the import is committed, and its files go back to the commit before it.
fn after_failure(logbook: &Logbook, undo: &Undo, e: Error) -> Error {
    let hint = if git::is_repo(&logbook.root) {
        let mut undo_file = undo.clone();
        undo_file.note(&undo_path());
        // best effort: the hint below is the undo either way
        let _ = serde_json::to_string_pretty(&undo_file).map(|text| {
            sys::write_atomic_nofollow(&logbook.path(undo_path()), format!("{text}\n").as_bytes())
        });
        format!(
            "the import stopped half way and nothing was committed; undo it with `{}` in {} (it touches only the files the import wrote), then run it again",
            undo_file.command(),
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

/// The vault argument as a path: `~` is the home directory, the rest is
/// taken as given, bytes and all (a name that is not UTF-8 stays as it is,
/// F-143).
fn vault_path(ctx: &Context, vault: &Path) -> PathBuf {
    match vault.strip_prefix("~") {
        Ok(rest) => ctx.dirs.expand("~").join(rest),
        Err(_) => std::path::absolute(vault).unwrap_or_else(|_| vault.to_path_buf()),
    }
}

/// The apply's own ledger note, the first line it writes, with or without
/// cases: a later run without the marker finds it and is refused, so a
/// vault of only journal and knowledge files is never imported twice
/// (F-141). At the time of the apply, by the human who runs it, no case.
fn apply_note(ctx: &Context, plan: &Plan) -> Event {
    let mut meta = Meta::default();
    meta.extra
        .insert("import".into(), Value::String(SOURCE.into()));
    Event::new(ctx.now, Source::Manual, Kind::Note, SOURCE)
        .detail(format!(
            "import from {SOURCE}: {} case(s), {} journal day(s), {} memory file(s), {} deviation row(s)",
            plan.cases.len(),
            plan.days.len(),
            plan.memory.len(),
            plan.rows.len()
        ))
        .actor("human")
        .meta(meta)
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
    // a link there is not read as "imported", a FIFO not opened (WP-171)
    match sys::read_regular_string(&logbook.checked_file(&rel)?, sys::LOGBOOK_FILE_MAX) {
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
    let Some(e) = found else {
        return Ok(None);
    };
    let root = logbook.root.display();
    let undo = sys::read_regular_string(&logbook.path(undo_path()), sys::LOGBOOK_FILE_MAX)
        .ok()
        .and_then(|text| serde_json::from_str::<Undo>(&text).ok())
        .filter(|undo| undo.is_sane(&logbook.root));
    let way_back = match undo {
        Some(undo) => format!(
            "After a failed apply, undo it with `{}` in {root} (it touches only the files that apply wrote) and run it again",
            undo.command()
        ),
        None => format!(
            "After a failed apply, take back the files it wrote (`git -C {root} status` lists the uncommitted changes) and run it again"
        ),
    };
    Err(Error::user(format!(
        "ledger/{}.jsonl has {SOURCE} import notes ({}) but {rel} is missing: an earlier --apply did not finish, or .seldon/ was deleted. {way_back}; after a deleted .seldon/ the import is complete",
        e.month(),
        e.id,
    )))
}

/// Writes the report (text outside its fence kept); `true` when it changed.
fn write_report(logbook: &Logbook, plan: &Plan, mode: &Mode) -> Result<bool> {
    let path = logbook.checked_file(report_path(SOURCE))?;
    let existing = match sys::read_regular_string(&path, sys::LOGBOOK_FILE_MAX) {
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
    sys::write_atomic_nofollow(&path, text.as_bytes())?;
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

#[cfg(test)]
mod tests {
    use super::*;

    fn undo(base: Option<&str>, restore: &[&str], remove: &[&str]) -> Undo {
        Undo {
            base: base.map(str::to_string),
            restore: restore.iter().map(|p| p.to_string()).collect(),
            remove: remove.iter().map(|p| p.to_string()).collect(),
            root: PathBuf::new(),
        }
    }

    #[test]
    fn the_undo_command_quotes_and_names_only_its_files() {
        let sha = "0123456789abcdef0123456789abcdef01234567";
        let u = undo(
            Some(sha),
            &["memory/lessons.md", "ledger/2026-10.jsonl"],
            &["work/queued/C-2026-002-a b.md", "memory/it's.md"],
        );
        assert!(u.is_sane(Path::new("/nonexistent")));
        assert_eq!(
            u.command(),
            format!(
                "git --literal-pathspecs checkout {sha} -- memory/lessons.md ledger/2026-10.jsonl && rm -f -- 'work/queued/C-2026-002-a b.md' 'memory/it'\\''s.md'"
            )
        );
        assert_eq!(undo(None, &[], &["a.md"]).command(), "rm -f -- a.md");
        assert_eq!(
            undo(None, &["a.md"], &[]).command(),
            "git --literal-pathspecs checkout -- a.md"
        );
    }

    #[test]
    fn a_kept_undo_outside_the_imports_files_is_not_offered() {
        let sha = "0123456789abcdef0123456789abcdef01234567";
        let ok = |u: Undo| u.is_sane(Path::new("/nonexistent"));
        // a hash base whenever files are restored
        assert!(!ok(undo(Some("HEAD; rm -rf ~"), &["memory/a.md"], &[])));
        assert!(!ok(undo(Some("HEAD"), &["memory/a.md"], &[])));
        assert!(!ok(undo(None, &["memory/a.md"], &[])));
        assert!(ok(undo(None, &[], &["memory/a.md"])));
        // only the import's own folders, relative, never through .git
        for bad in [
            "../outside.md",
            "/etc/passwd",
            "memory/../../x",
            "",
            "memory",
            "PROJECT.md",
            "AGENTS.md",
            ".git/config",
            "memory/.git/config",
            ".seldon/logbook.toml",
            ".seldon/imports",
            "inbox/idee.md",
            // glob and pathspec magic, control characters
            "memory/*",
            ":/memory/a.md",
            "memory/[ab].md",
            "memory/a?.md",
            "memory/a\nb.md",
        ] {
            assert!(!ok(undo(Some(sha), &[bad], &[])), "{bad:?} offered");
            assert!(!ok(undo(Some(sha), &[], &[bad])), "{bad:?} offered");
        }
        assert!(ok(undo(
            Some(sha),
            &[
                "memory/lessons.md",
                "system/deviations.md",
                "ledger/2026-10.jsonl"
            ],
            &[
                "work/queued/C-2026-002-x.md",
                "journal/2026/2026-08-20.md",
                "outputs/IMPORT-omarchy-agent.md",
                ".seldon/imports/omarchy-agent.undo.json",
            ],
        )));
    }
}
