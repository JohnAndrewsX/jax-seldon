//! The `omarchy-agent` kit's Obsidian vault → this logbook (WP-043).
//!
//! The vault is read, never written. [`plan`] maps it onto the logbook as
//! it is now and returns every change with its final bytes:
//!
//! - **Cases** `pipeline/cases/*.md`, `archive/cases/*.md` → `work/<folder>/`.
//!   The id is kept unless the logbook (or an earlier kit case) has it; then
//!   the case gets the next free id of its year, the original id in its tags
//!   (`omarchy-agent/<id>`) and in a line under the title. Status `new`,
//!   `planned`, `in-progress`, `verification` → `queued` (an imported case
//!   is never active; the last two say so in the Log), `done` → `completed`,
//!   `dropped` → `dropped` (a missing `closed` becomes `created`, listed as
//!   an assumption). `## Auftrag` → `## Intent`, `## Plan` → `## Plan`,
//!   `## Ergebnis` → `## Result`, every other section verbatim under
//!   `## History` (headings one level deeper). One Log line names the
//!   source; one ledger `note` per case at the case's `created` date (local
//!   midnight), `meta.import: omarchy-agent`.
//! - **Id rewrites:** in every imported text, a kit id the logbook already
//!   had (`[[C-OLD…`, bare `C-OLD`) becomes the case's new id ([`Rewriter`]).
//! - **Journal** `journal/YYYY-MM.md`: split at the session headings
//!   `## YYYY-MM-DD …`; each day's sessions go under one `## Imported from
//!   omarchy-agent` heading of `journal/YYYY/YYYY-MM-DD.md` (appended when
//!   the day exists).
//! - **Knowledge** `knowledge/<topic>/*.md` → a `## ` section each in
//!   `memory/<topic>.md` (`lessons` → `memory/lessons.md`), with a source
//!   line; `knowledge/<name>.md` → `memory/<name>.md`; an existing file's
//!   `updated` moves to the import day.
//! - **Deviations** `system/deviations.md`: each `### ` entry that is not
//!   resolved and names a path (`~/…` or `/…`, heading first) → a user row
//!   of `deviations.table` without a case, when the path is not listed.
//! - Everything else (inbox, Dashboard, templates, the rest of `system/`)
//!   is listed in the report and not imported.
//!
//! Every line read passes the [`Scrubber`]. A file that should be imported
//! but cannot be mapped is an error: the report lists it, `--apply` refuses.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use chrono::{DateTime, FixedOffset, Local, NaiveDate, TimeZone as _};
use regex::Regex;

use super::{
    Hit, Rewriter, Scrubber, demote, sections, split_frontmatter, strip_title, trim_blank_lines,
    yaml_list, yaml_map, yaml_str,
};
use crate::dossier::{self, FENCES};
use crate::error::{Error, Result};
use crate::frontmatter::Document;
use crate::index::load::{fence_table, has_table_separator};
use crate::logbook::Logbook;
use crate::logbook::cases::{self, LOG_COMMENT};
use crate::model::{
    self, Case, CaseStatus, Journal, Memory, Priority, Risk, Zone, is_case_id, is_slug,
};
use crate::redact::Redactor;
use crate::sys;

/// The importer's name: `seldon import omarchy-agent`, `meta.import`.
pub const SOURCE: &str = "omarchy-agent";

/// The heading imported journal sessions go under.
pub const JOURNAL_HEADING: &str = "## Imported from omarchy-agent";

/// Tag of every imported case.
pub const TAG: &str = "omarchy-agent";

/// A file of the vault that is not imported, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Skipped {
    /// Relative to the vault (or the logbook, for a day file that blocks).
    pub path: String,
    pub reason: String,
    /// It should have been imported but could not be mapped.
    pub error: bool,
}

/// A kit case as it will be written.
#[derive(Debug, Clone)]
pub struct PlannedCase {
    /// Vault-relative source path.
    pub source: String,
    /// The kit's id.
    pub from: String,
    /// The id in this logbook.
    pub to: String,
    pub kit_status: String,
    /// The case (`events` empty until the ledger assigns the note's id).
    pub case: Case,
    /// The body (everything after the frontmatter).
    pub body: String,
    /// Logbook-relative path of the new file.
    pub path: String,
    /// The ledger note's time: `created` at local midnight.
    pub ts: DateTime<FixedOffset>,
    /// Who had the id first, when the case was renumbered.
    pub collision: Option<String>,
    /// What the import assumed for this case (a missing `closed`).
    pub assumption: Option<String>,
}

/// A journal day that gets imported sessions.
#[derive(Debug, Clone)]
pub struct PlannedDay {
    pub date: NaiveDate,
    /// Logbook-relative `journal/YYYY/YYYY-MM-DD.md`.
    pub path: String,
    /// Whether the day file exists (the sessions are appended).
    pub exists: bool,
    pub sessions: usize,
    /// The whole file as it will be written.
    pub text: String,
}

/// A memory file that gets imported sections.
#[derive(Debug, Clone)]
pub struct PlannedMemory {
    /// Logbook-relative `memory/<topic>.md`.
    pub path: String,
    pub exists: bool,
    /// Vault-relative sources, one section each.
    pub sources: Vec<String>,
    pub text: String,
}

/// A user row for `deviations.table`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedRow {
    pub path: String,
    pub reason: String,
    pub date: String,
    /// The kit entry's label (`Q14`).
    pub entry: String,
}

/// Everything an apply would change, and what the report says.
#[derive(Debug, Clone)]
pub struct Plan {
    /// The vault as displayed (`~/…`).
    pub vault: String,
    pub cases: Vec<PlannedCase>,
    pub days: Vec<PlannedDay>,
    /// Imported journal sessions (all days).
    pub sessions: usize,
    pub memory: Vec<PlannedMemory>,
    pub rows: Vec<PlannedRow>,
    /// The dossier with the new rows set (`None` when there are none).
    pub dossier: Option<dossier::Files>,
    pub skipped: Vec<Skipped>,
    pub hits: Vec<Hit>,
    pub by_rule: BTreeMap<&'static str, usize>,
    pub private_paths: usize,
    /// Renumbered ids rewritten in imported text, per source file:
    /// (wikilinks, bare ids).
    pub rewrites: BTreeMap<String, (usize, usize)>,
}

impl Plan {
    pub fn errors(&self) -> usize {
        self.skipped.iter().filter(|s| s.error).count()
    }

    pub fn memory_sections(&self) -> usize {
        self.memory.iter().map(|m| m.sources.len()).sum()
    }

    /// Kit id → logbook id.
    pub fn id_map(&self) -> BTreeMap<String, String> {
        self.cases
            .iter()
            .map(|c| (c.from.clone(), c.to.clone()))
            .collect()
    }

    pub fn collisions(&self) -> impl Iterator<Item = &PlannedCase> {
        self.cases.iter().filter(|c| c.collision.is_some())
    }

    /// (wikilinks, bare ids) rewritten over every file.
    pub fn rewrite_totals(&self) -> (usize, usize) {
        self.rewrites
            .values()
            .fold((0, 0), |(l, b), (x, y)| (l + x, b + y))
    }
}

/// What kind of vault file this is.
enum Class {
    Case,
    Journal,
    /// The memory topic it belongs to.
    Knowledge(String),
    Deviations,
    Listed(&'static str),
    Ignored,
}

fn classify(rel: &str) -> Class {
    let parts: Vec<&str> = rel.split('/').collect();
    let name = parts.last().copied().unwrap_or_default();
    let md = name.ends_with(".md");
    if name == ".gitkeep" {
        return Class::Ignored;
    }
    match parts.as_slice() {
        ["pipeline" | "archive", "cases", _] if md => Class::Case,
        ["journal", n] if md && is_month_file(n) => Class::Journal,
        ["knowledge", topic, _] if md => Class::Knowledge(topic.to_string()),
        ["knowledge", n] if md => Class::Knowledge(n.trim_end_matches(".md").to_string()),
        ["system", "deviations.md"] => Class::Deviations,
        ["system", _] => {
            Class::Listed("the kit's dossier; Seldon keeps its own (`seldon dossier`)")
        }
        ["pipeline", "inbox", _] => Class::Listed("inbox item; triage it into a case or `inbox/`"),
        ["pipeline", "templates", _] => Class::Listed("kit template"),
        ["Dashboard.md"] => Class::Listed("the kit's dashboard; Seldon renders STATUS.md"),
        ["STRUCTURE.md"] => Class::Listed("the kit's map of its own layout"),
        _ => Class::Listed("not part of the kit's layout"),
    }
}

fn is_month_file(name: &str) -> bool {
    name.strip_suffix(".md")
        .is_some_and(|m| NaiveDate::parse_from_str(&format!("{m}-01"), "%Y-%m-%d").is_ok())
}

/// Builds the plan. `vault` must be a directory that looks like the kit's
/// vault (a user error otherwise); the logbook is read, not written.
pub fn plan(
    vault: &Path,
    vault_display: String,
    logbook: &Logbook,
    redactor: Redactor,
    now: &DateTime<FixedOffset>,
) -> Result<Plan> {
    if !vault.is_dir() {
        return Err(Error::user(format!("{vault_display} is not a directory")));
    }
    if !["pipeline", "archive", "journal", "knowledge"]
        .iter()
        .any(|d| vault.join(d).is_dir())
    {
        return Err(Error::user(format!(
            "{vault_display} does not look like an omarchy-agent vault (no pipeline/, archive/, journal/ or knowledge/)"
        )));
    }
    let mut files = Vec::new();
    let mut skipped = Vec::new();
    walk(vault, "", &mut files, &mut skipped)?;

    let mut scrub = Scrubber::new(redactor);
    let mut kit_cases = Vec::new();
    let mut journals = Vec::new();
    let mut knowledge: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new();
    let mut deviations = None;
    for rel in &files {
        let class = classify(rel);
        match &class {
            Class::Listed(reason) => {
                skipped.push(Skipped {
                    path: rel.clone(),
                    reason: reason.to_string(),
                    error: false,
                });
                continue;
            }
            Class::Ignored => continue,
            _ => {}
        }
        let text = match std::fs::read(vault.join(rel)) {
            Ok(bytes) => match String::from_utf8(bytes) {
                Ok(t) => scrub.text(rel, &t),
                Err(_) => {
                    skipped.push(error(rel, "not UTF-8 text"));
                    continue;
                }
            },
            Err(e) => {
                return Err(anyhow::Error::new(e)
                    .context(format!("cannot read {}", vault.join(rel).display()))
                    .into());
            }
        };
        match class {
            Class::Case => match parse_case(rel, &text) {
                Ok(c) => kit_cases.push(c),
                Err(reason) => skipped.push(error(rel, &reason)),
            },
            Class::Journal => journals.push((rel.clone(), text)),
            Class::Knowledge(topic) => knowledge
                .entry(memory_file(&topic))
                .or_default()
                .push((rel.clone(), text)),
            Class::Deviations => deviations = Some(text),
            Class::Listed(_) | Class::Ignored => unreachable!("handled above"),
        }
    }

    let ids = assign_ids(logbook, &mut kit_cases)?;
    // references to an id the logbook already had are rewritten to the new
    // id; a kit id that two kit files share stays (ambiguous)
    let renumbered: BTreeMap<String, String> = kit_cases
        .iter()
        .zip(&ids)
        .filter(|(_, a)| a.by_logbook)
        .map(|(k, a)| (k.id.clone(), a.to.clone()))
        .collect();
    let mut rw = Rewriter::new(renumbered);
    let cases: Vec<PlannedCase> = kit_cases
        .into_iter()
        .zip(ids)
        .map(|(k, a)| build_case(k, a.to, a.collision, now, &mut rw))
        .collect();
    let id_map: BTreeMap<String, String> = cases
        .iter()
        .map(|c| (c.from.clone(), c.to.clone()))
        .collect();
    let (days, sessions) = plan_journal(logbook, &journals, &id_map, &mut rw, &mut skipped)?;
    let memory = plan_memory(logbook, &knowledge, now, &mut rw)?;
    let (rows, dossier) = match deviations {
        Some(text) => plan_deviations(logbook, &text, &mut rw, &mut skipped)?,
        None => (Vec::new(), None),
    };
    skipped.sort_by(|a, b| (!a.error, &a.path).cmp(&(!b.error, &b.path)));
    Ok(Plan {
        vault: vault_display,
        cases,
        days,
        sessions,
        memory,
        rows,
        dossier,
        skipped,
        by_rule: scrub.by_rule(),
        hits: scrub.hits,
        private_paths: scrub.private_paths,
        rewrites: rw.by_file,
    })
}

fn error(path: &str, reason: &str) -> Skipped {
    Skipped {
        path: path.to_string(),
        reason: reason.to_string(),
        error: true,
    }
}

/// Every file under `dir`, vault-relative, sorted; `.git/` is left out,
/// `.obsidian/` and symbolic links are listed as skipped.
fn walk(root: &Path, rel: &str, files: &mut Vec<String>, skipped: &mut Vec<Skipped>) -> Result<()> {
    let dir = root.join(rel);
    let mut entries: Vec<PathBuf> = std::fs::read_dir(&dir)
        .and_then(|r| r.map(|e| e.map(|e| e.path())).collect())
        .map_err(|e| anyhow::Error::new(e).context(format!("cannot list {}", dir.display())))?;
    entries.sort();
    for path in entries {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let child = if rel.is_empty() {
            name.clone()
        } else {
            format!("{rel}/{name}")
        };
        let meta = std::fs::symlink_metadata(&path).map_err(|e| {
            anyhow::Error::new(e).context(format!("cannot stat {}", path.display()))
        })?;
        if meta.file_type().is_symlink() {
            skipped.push(Skipped {
                path: child,
                reason: "symbolic link, not followed".into(),
                error: false,
            });
        } else if meta.is_dir() {
            match name.as_str() {
                ".git" => {}
                ".obsidian" => skipped.push(Skipped {
                    path: format!("{child}/"),
                    reason: "Obsidian settings of the vault".into(),
                    error: false,
                }),
                _ => walk(root, &child, files, skipped)?,
            }
        } else {
            files.push(child);
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Cases
// ---------------------------------------------------------------------------

/// A kit case, read.
#[derive(Debug, Clone)]
struct KitCase {
    source: String,
    id: String,
    title: String,
    status: String,
    zone: Zone,
    risk: Risk,
    priority: Option<Priority>,
    created: NaiveDate,
    closed: Option<NaiveDate>,
    tags: Vec<String>,
    slug: String,
    body: String,
}

fn parse_case(source: &str, text: &str) -> std::result::Result<KitCase, String> {
    let (fm, body) = split_frontmatter(text);
    let fm = fm.ok_or("no frontmatter")?;
    serde_yaml::from_str::<serde_yaml::Value>(fm)
        .map_err(|e| format!("frontmatter is not valid YAML: {e}"))?;
    let map = yaml_map(Some(fm));
    if let Some(t) = yaml_str(&map, "type").filter(|t| t != "case") {
        return Err(format!("type `{t}` is not `case`"));
    }
    let required = |key: &str| yaml_str(&map, key).ok_or(format!("`{key}` is missing"));
    let id = required("id")?;
    if !is_case_id(&id) {
        return Err(format!("id `{id}` is not C-YYYY-NNN"));
    }
    let title = required("title")?
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let status = required("status")?;
    map_status(&status)?;
    let date = |key: &str, value: String| {
        NaiveDate::parse_from_str(&value, "%Y-%m-%d")
            .map_err(|_| format!("`{key}: {value}` is not a date (YYYY-MM-DD)"))
    };
    let created = date("created", required("created")?)?;
    let closed = yaml_str(&map, "closed")
        .map(|c| date("closed", c))
        .transpose()?;
    let zone: Zone = required("zone")?.parse()?;
    let risk: Risk = required("risk")?.parse()?;
    let priority = yaml_str(&map, "priority")
        .map(|p| p.parse::<Priority>())
        .transpose()?;
    let stem = Path::new(source)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let slug = stem
        .strip_prefix(&format!("{id}-"))
        .filter(|s| is_slug(s))
        .map_or_else(|| cases::slug(&title, "case"), str::to_string);
    Ok(KitCase {
        source: source.to_string(),
        id,
        title,
        status,
        zone,
        risk,
        priority,
        created,
        closed,
        tags: yaml_list(&map, "tags"),
        slug,
        body: body.to_string(),
    })
}

/// The kit status → the Seldon status, and a note for the Log.
fn map_status(status: &str) -> std::result::Result<(CaseStatus, Option<&'static str>), String> {
    Ok(match status {
        "new" | "planned" => (CaseStatus::Queued, None),
        "in-progress" | "verification" => (
            CaseStatus::Queued,
            Some("imported cases are never active; check the state before `seldon plan start`"),
        ),
        "done" => (CaseStatus::Completed, None),
        "dropped" => (CaseStatus::Dropped, None),
        other => {
            return Err(format!(
                "status `{other}` is not new|planned|in-progress|verification|done|dropped"
            ));
        }
    })
}

/// Case ids the logbook has: case files and workpiece folders (the ids
/// `cases::next_id` never reuses), with the file that holds each.
fn logbook_ids(logbook: &Logbook) -> Result<BTreeMap<String, String>> {
    let mut out = BTreeMap::new();
    let mut paths = logbook.case_files()?;
    if let Ok(entries) = std::fs::read_dir(logbook.path("work")) {
        let mut dirs: Vec<PathBuf> = entries
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.is_dir())
            .collect();
        dirs.sort();
        paths.extend(dirs);
    }
    for path in paths {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        if let Some(id) = id_prefix(&name) {
            out.entry(id)
                .or_insert_with(|| cases::relative(logbook, &path));
        }
    }
    Ok(out)
}

/// `C-YYYY-NNN` at the start of a file or folder name.
fn id_prefix(name: &str) -> Option<String> {
    let rest = name.strip_prefix("C-")?;
    let year = rest.get(..4)?;
    let digits: String = rest
        .get(5..)?
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    let id = format!("C-{year}-{digits}");
    (rest.as_bytes().get(4) == Some(&b'-') && is_case_id(&id)).then_some(id)
}

fn split_id(id: &str) -> (String, u32) {
    let rest = &id[2..];
    let (year, num) = rest.split_once('-').unwrap_or((rest, "0"));
    (year.to_string(), num.parse().unwrap_or(0))
}

/// The id a kit case gets here.
#[derive(Debug, Clone)]
struct Assigned {
    to: String,
    /// Who had the id first, when the case was renumbered.
    collision: Option<String>,
    /// The logbook had the id (not an earlier kit case).
    by_logbook: bool,
}

/// Sorts the kit cases by id and gives each its id here, in that order.
fn assign_ids(logbook: &Logbook, kit: &mut [KitCase]) -> Result<Vec<Assigned>> {
    kit.sort_by(|a, b| (&a.id, &a.source).cmp(&(&b.id, &b.source)));
    let existing = logbook_ids(logbook)?;
    // the highest number per year over the logbook and the whole kit, so a
    // renumbered case never takes an id a later kit case keeps
    let mut max: BTreeMap<String, u32> = BTreeMap::new();
    for id in existing.keys().chain(kit.iter().map(|k| &k.id)) {
        let (year, n) = split_id(id);
        let m = max.entry(year).or_insert(0);
        *m = (*m).max(n);
    }
    let mut taken: BTreeMap<String, String> = existing.clone();
    let mut out = Vec::new();
    for k in kit.iter() {
        let assigned = match taken.get(&k.id) {
            Some(holder) => {
                let (year, _) = split_id(&k.id);
                let n = max.entry(year.clone()).or_insert(0);
                *n += 1;
                Assigned {
                    to: format!("C-{year}-{:03}", *n),
                    collision: Some(holder.clone()),
                    by_logbook: existing.contains_key(&k.id),
                }
            }
            None => Assigned {
                to: k.id.clone(),
                collision: None,
                by_logbook: false,
            },
        };
        taken.insert(k.id.clone(), k.source.clone());
        taken.insert(assigned.to.clone(), k.source.clone());
        out.push(assigned);
    }
    Ok(out)
}

fn build_case(
    k: KitCase,
    to: String,
    collision: Option<String>,
    now: &DateTime<FixedOffset>,
    rw: &mut Rewriter,
) -> PlannedCase {
    let (status, note) = map_status(&k.status).expect("checked by parse_case");
    let mut tags = k.tags.clone();
    for tag in [
        Some(TAG.to_string()),
        collision.as_ref().map(|_| format!("{TAG}/{}", k.id)),
    ]
    .into_iter()
    .flatten()
    {
        if !tags.contains(&tag) {
            tags.push(tag);
        }
    }
    // a closed case without `closed`: assume it closed the day it was made
    let (closed, assumption) = match (status.is_open(), k.closed) {
        (true, _) => (None, None),
        (false, Some(c)) => (Some(c), None),
        (false, None) => (
            Some(k.created),
            Some(format!(
                "status {} without `closed`: closed set to `created` ({})",
                k.status, k.created
            )),
        ),
    };
    let case = Case {
        id: to.clone(),
        title: k.title.clone(),
        status,
        zone: k.zone,
        risk: k.risk,
        priority: k.priority,
        area: None,
        created: k.created,
        started: None,
        closed,
        snapshot_before: None,
        agents: Vec::new(),
        events: Vec::new(),
        tags,
    };
    let mut log = format!(
        "imported from {SOURCE} {} (status {} → {})",
        k.source, k.status, status
    );
    if collision.is_some() {
        log.push_str(&format!(", renumbered from {}", k.id));
    }
    if let Some(note) = note {
        log.push_str(&format!("; {note}"));
    }
    if assumption.is_some() {
        log.push_str(&format!("; closed date assumed: {}", k.created));
    }
    let body = case_body(
        &k.body,
        &to,
        &k.title,
        collision.is_some().then_some(k.id.as_str()),
        &cases::log_line(now, &log, "human"),
        &mut |text: &str| rw.text(&k.source, text),
    );
    PlannedCase {
        path: format!("work/{}/{}", status.folder(), case.file_name(&k.slug)),
        ts: local_midnight(k.created, now),
        source: k.source,
        from: k.id,
        to,
        kit_status: k.status,
        case,
        body,
        collision,
        assumption,
    }
}

/// The Seldon body of a kit case: the title line, Intent from `Auftrag`,
/// Plan from `Plan`, every other section under History, the Log with the
/// import line, Result from `Ergebnis`. `rewrite` runs over the kit's text
/// (not over the lines the import adds).
fn case_body(
    kit: &str,
    id: &str,
    title: &str,
    renumbered: Option<&str>,
    log: &str,
    rewrite: &mut dyn FnMut(&str) -> String,
) -> String {
    let parts = sections(kit);
    let (_, preamble) = strip_title(&parts.preamble);
    let mut intent = None;
    let mut plan = None;
    let mut result = None;
    // History: the preamble's text, then every other section, one blank
    // line after each block
    let mut history: Vec<String> = Vec::new();
    let rest = trim_blank_lines(&preamble);
    if !rest.is_empty() {
        history.push(demote(&rest));
    }
    for (name, content) in &parts.sections {
        match name.as_str() {
            "Auftrag" | "Intent" if intent.is_none() => intent = Some(content.as_str()),
            "Plan" if plan.is_none() => plan = Some(content.as_str()),
            "Ergebnis" | "Result" if result.is_none() => result = Some(content.as_str()),
            _ => history.push(format!(
                "### {name}\n{}",
                trim_blank_lines(&demote(content))
            )),
        }
    }
    let mut block = |text: Option<&str>| rewrite(&trim_blank_lines(text.unwrap_or("")));
    let mut b = format!("# {id} — {title}\n\n");
    if let Some(original) = renumbered {
        b.push_str(&format!(
            "*Imported from {SOURCE} as {original}; renumbered because this logbook already had that id.*\n\n"
        ));
    }
    b.push_str(&format!("## Intent\n{}\n", block(intent)));
    b.push_str(&format!("## Plan\n{}\n", block(plan)));
    b.push_str(&format!(
        "## History\n<!-- imported from {SOURCE}: the kit's other sections, verbatim -->\n"
    ));
    for h in &history {
        b.push_str(&block(Some(h)));
        b.push('\n');
    }
    if history.is_empty() {
        b.push('\n');
    }
    b.push_str(&format!(
        "## Log\n{LOG_COMMENT}\n{log}\n\n## Result\n{}",
        block(result)
    ));
    b
}

/// `date` at local midnight (the ledger note of a case: `ts` = created);
/// `now`'s offset when local midnight does not exist.
fn local_midnight(date: NaiveDate, now: &DateTime<FixedOffset>) -> DateTime<FixedOffset> {
    let midnight = date.and_hms_opt(0, 0, 0).expect("midnight exists");
    Local
        .from_local_datetime(&midnight)
        .earliest()
        .map(|t| t.fixed_offset())
        .unwrap_or_else(|| {
            now.offset()
                .from_local_datetime(&midnight)
                .earliest()
                .unwrap_or(*now)
        })
}

// ---------------------------------------------------------------------------
// Journal
// ---------------------------------------------------------------------------

/// A session of a month file: its date, heading line and text.
#[derive(Debug, Clone)]
struct Session {
    date: NaiveDate,
    source: String,
    /// The `## …` heading and everything up to the next session heading.
    text: String,
}

/// The sessions of a month file. Text before the first session heading
/// (the month title) is not a session; any other `## ` heading belongs to
/// the session before it.
fn split_sessions(source: &str, text: &str) -> Vec<Session> {
    let (_, body) = split_frontmatter(text);
    let mut out: Vec<Session> = Vec::new();
    let mut in_fence = false;
    for line in body.split_inclusive('\n') {
        let t = line.trim_start();
        if t.starts_with("```") || t.starts_with("~~~") {
            in_fence = !in_fence;
        }
        let date = (!in_fence)
            .then(|| line.strip_prefix("## "))
            .flatten()
            .and_then(|h| h.get(..10))
            .and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok());
        match (date, out.last_mut()) {
            (Some(date), _) => out.push(Session {
                date,
                source: source.to_string(),
                text: line.to_string(),
            }),
            (None, Some(s)) => s.text.push_str(line),
            (None, None) => {}
        }
    }
    out
}

fn plan_journal(
    logbook: &Logbook,
    months: &[(String, String)],
    id_map: &BTreeMap<String, String>,
    rw: &mut Rewriter,
    skipped: &mut Vec<Skipped>,
) -> Result<(Vec<PlannedDay>, usize)> {
    let mut by_day: BTreeMap<NaiveDate, Vec<Session>> = BTreeMap::new();
    let mut count = 0;
    for (source, text) in months {
        let sessions = split_sessions(source, text);
        if sessions.is_empty() {
            skipped.push(Skipped {
                path: source.clone(),
                reason: "no session headings (`## YYYY-MM-DD …`)".into(),
                error: false,
            });
        }
        for s in sessions {
            count += 1;
            by_day.entry(s.date).or_default().push(s);
        }
    }
    let link = Regex::new(r"\[\[(C-\d{4}-\d{3,})").expect("link pattern compiles");
    let mut out = Vec::new();
    for (date, sessions) in by_day {
        let rel = Journal::relative_path(date);
        let path = logbook.path(&rel);
        let mut sources: Vec<&str> = sessions.iter().map(|s| s.source.as_str()).collect();
        sources.dedup();
        let mut block = format!(
            "{JOURNAL_HEADING}\n*Source: {}.*\n\n",
            sources
                .iter()
                .map(|s| format!("`{s}`"))
                .collect::<Vec<_>>()
                .join(", ")
        );
        let mut ids: Vec<String> = Vec::new();
        for s in &sessions {
            block.push_str(&trim_blank_lines(&demote(&rw.text(&s.source, &s.text))));
            block.push('\n');
            for cap in link.captures_iter(&s.text) {
                if let Some(to) = id_map.get(&cap[1])
                    && !ids.contains(to)
                {
                    ids.push(to.clone());
                }
            }
        }
        let block = block.trim_end().to_string() + "\n";
        let existing = match std::fs::read_to_string(&path) {
            Ok(t) => Some(t),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => {
                return Err(anyhow::Error::new(e)
                    .context(format!("cannot read {}", path.display()))
                    .into());
            }
        };
        let text = match &existing {
            None => model::render_new(&Journal { date, cases: ids }, &block),
            Some(old) => match append_day(old, &block, &ids) {
                Ok(t) => t,
                Err(e) => {
                    skipped.push(error(&rel, &format!("invalid journal frontmatter: {e}")));
                    continue;
                }
            },
        };
        out.push(PlannedDay {
            date,
            path: rel,
            exists: existing.is_some(),
            sessions: sessions.len(),
            text,
        });
    }
    Ok((out, count))
}

/// An existing day with `block` appended and `ids` added to `cases:`.
fn append_day(
    old: &str,
    block: &str,
    ids: &[String],
) -> std::result::Result<String, crate::frontmatter::FrontmatterError> {
    let (mut journal, mut doc): (Journal, Document) = model::parse(old)?;
    let before = journal.cases.len();
    for id in ids {
        if !journal.cases.contains(id) {
            journal.cases.push(id.clone());
        }
    }
    if journal.cases.len() != before {
        model::update(&mut doc, &journal);
    }
    doc.body = join_blocks(&doc.body, block);
    Ok(doc.render())
}

/// `text` and `block` separated by one blank line.
fn join_blocks(text: &str, block: &str) -> String {
    let mut out = text.to_string();
    if !out.trim().is_empty() {
        if !out.ends_with('\n') {
            out.push('\n');
        }
        if !out.ends_with("\n\n") {
            out.push('\n');
        }
    }
    out.push_str(block);
    out
}

// ---------------------------------------------------------------------------
// Knowledge → memory
// ---------------------------------------------------------------------------

/// `memory/<topic>.md` for a knowledge topic.
fn memory_file(topic: &str) -> String {
    let slug = sys::slugify(topic);
    format!(
        "memory/{}.md",
        if slug.is_empty() { "knowledge" } else { &slug }
    )
}

fn plan_memory(
    logbook: &Logbook,
    knowledge: &BTreeMap<String, Vec<(String, String)>>,
    now: &DateTime<FixedOffset>,
    rw: &mut Rewriter,
) -> Result<Vec<PlannedMemory>> {
    let mut out = Vec::new();
    for (rel, files) in knowledge {
        let sections: Vec<String> = files
            .iter()
            .map(|(source, text)| rw.text(source, &memory_section(source, text)))
            .collect();
        let path = logbook.path(rel);
        let existing = match std::fs::read_to_string(&path) {
            Ok(t) => Some(t),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => {
                return Err(anyhow::Error::new(e)
                    .context(format!("cannot read {}", path.display()))
                    .into());
            }
        };
        let joined = sections.join("\n");
        let text = match &existing {
            // the file's `updated` moves to the import day
            Some(old) => match model::parse::<Memory>(old) {
                Ok((mut memory, mut doc)) => {
                    memory.updated = Some(now.date_naive());
                    model::update(&mut doc, &memory);
                    join_blocks(&doc.render(), &joined)
                }
                Err(_) => join_blocks(old, &joined),
            },
            None => {
                let topic = rel
                    .trim_start_matches("memory/")
                    .trim_end_matches(".md")
                    .to_string();
                let mut title = topic.clone();
                if let Some(first) = title.get_mut(..1) {
                    first.make_ascii_uppercase();
                }
                model::render_new(
                    &Memory {
                        topic,
                        updated: Some(now.date_naive()),
                    },
                    &format!("# {title}\n\n{joined}"),
                )
            }
        };
        out.push(PlannedMemory {
            path: rel.clone(),
            exists: existing.is_some(),
            sources: files.iter().map(|(s, _)| s.clone()).collect(),
            text,
        });
    }
    Ok(out)
}

/// One knowledge file as a `## ` section: its title (frontmatter, else the
/// first heading, else the file name), a source line, the body one heading
/// level deeper.
fn memory_section(source: &str, text: &str) -> String {
    let (fm, body) = split_frontmatter(text);
    let map = yaml_map(fm);
    let (heading, body) = strip_title(body);
    let title = yaml_str(&map, "title")
        .or(heading)
        .unwrap_or_else(|| {
            Path::new(source)
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default()
        })
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let updated = yaml_str(&map, "updated")
        .map(|u| format!(", updated {u}"))
        .unwrap_or_default();
    let body = trim_blank_lines(&demote(&body));
    let mut s = format!("## {title}\n*Imported from {SOURCE}: `{source}`{updated}.*\n");
    if !body.is_empty() {
        s.push('\n');
        s.push_str(&body);
    }
    s
}

// ---------------------------------------------------------------------------
// Deviations
// ---------------------------------------------------------------------------

/// A `### ` entry of the kit's `system/deviations.md`.
#[derive(Debug, Clone)]
struct KitDeviation {
    label: String,
    heading: String,
    body: String,
    resolved: bool,
}

fn deviation_entries(text: &str) -> Vec<KitDeviation> {
    let (_, body) = split_frontmatter(text);
    let mut out: Vec<KitDeviation> = Vec::new();
    let mut section = String::new();
    let mut in_fence = false;
    let mut in_entry = false;
    for line in body.split_inclusive('\n') {
        let t = line.trim_start();
        if t.starts_with("```") || t.starts_with("~~~") {
            in_fence = !in_fence;
        }
        let bare = line.trim_end();
        if !in_fence && let Some(h) = bare.strip_prefix("## ") {
            section = h.to_lowercase();
            in_entry = false;
            continue;
        }
        if !in_fence && let Some(h) = bare.strip_prefix("### ") {
            let heading = h.trim().to_string();
            let lower = heading.to_lowercase();
            out.push(KitDeviation {
                label: heading
                    .split([' ', '—'])
                    .next()
                    .unwrap_or_default()
                    .to_string(),
                resolved: section.starts_with("aufgelöst")
                    || section.starts_with("resolved")
                    || lower.contains("aufgelöst")
                    || lower.contains("resolved")
                    || heading.contains('✅'),
                heading,
                body: String::new(),
            });
            in_entry = true;
            continue;
        }
        if in_entry && let Some(e) = out.last_mut() {
            e.body.push_str(line);
        }
    }
    out
}

/// The first `` `~/…` `` or `` `/…` `` span of `text` without white space
/// or `|`.
fn first_path(text: &str) -> Option<String> {
    text.split('`')
        .skip(1)
        .step_by(2)
        .map(str::trim)
        .find(|s| {
            (s.starts_with("~/") || (s.starts_with('/') && s.len() > 1))
                && !s.contains(char::is_whitespace)
                && !s.contains('|')
        })
        .map(str::to_string)
}

fn plan_deviations(
    logbook: &Logbook,
    text: &str,
    rw: &mut Rewriter,
    skipped: &mut Vec<Skipped>,
) -> Result<(Vec<PlannedRow>, Option<dossier::Files>)> {
    let fence = FENCES
        .iter()
        .find(|f| f.name == "deviations.table")
        .expect("deviations.table is a fence");
    let mut files = dossier::Files::read(&logbook.path("system"), logbook.meta.language)?;
    let mut body = files
        .body(fence.name)
        .filter(|b| has_table_separator(b))
        .unwrap_or_else(|| "| path | reason | date | case |\n|---|---|---|---|\n".to_string());
    let mut listed: BTreeSet<String> = fence_table(&body)
        .into_iter()
        .filter_map(|r| r.get("path").cloned())
        .collect();
    let dated = Regex::new(r"Datum:?\s*(\d{4}-\d{2}-\d{2})").expect("date pattern compiles");
    let any_date = Regex::new(r"\d{4}-\d{2}-\d{2}").expect("date pattern compiles");
    let mut rows = Vec::new();
    for e in deviation_entries(text) {
        let at = format!("system/deviations.md {}", e.label);
        let skip = |reason: &str| Skipped {
            path: at.clone(),
            reason: reason.to_string(),
            error: false,
        };
        if e.resolved {
            skipped.push(skip("resolved in the kit"));
            continue;
        }
        let Some(path) = first_path(&e.heading).or_else(|| first_path(&e.body)) else {
            skipped.push(skip("names no path (`~/…` or `/…`)"));
            continue;
        };
        if listed.contains(&path) {
            skipped.push(skip(&format!("`{path}` is already listed")));
            continue;
        }
        let date = dated
            .captures(&e.body)
            .map(|c| c[1].to_string())
            .or_else(|| any_date.find(&e.body).map(|m| m.as_str().to_string()))
            .or_else(|| any_date.find(&e.heading).map(|m| m.as_str().to_string()))
            .unwrap_or_default();
        let reason = format!("{} ({SOURCE})", rw.text("system/deviations.md", &e.heading))
            .replace('|', "/")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        listed.insert(path.clone());
        rows.push(PlannedRow {
            path,
            reason,
            date,
            entry: e.label,
        });
    }
    if rows.is_empty() {
        return Ok((rows, None));
    }
    if !body.ends_with('\n') {
        body.push('\n');
    }
    for r in &rows {
        body.push_str(&format!("| {} | {} | {} | — |\n", r.path, r.reason, r.date));
    }
    if let Err(w) = files.set(fence, &body) {
        // a damaged fence is the user's to repair: no rows, the reason reported
        skipped.push(Skipped {
            path: "system/deviations.md".to_string(),
            reason: w,
            error: true,
        });
        return Ok((Vec::new(), None));
    }
    Ok((rows, Some(files)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn now() -> DateTime<FixedOffset> {
        DateTime::parse_from_rfc3339("2026-10-02T10:00:00+02:00").unwrap()
    }

    const CASE: &str = "---\nid: C-2026-011\ntype: case\ntitle: \"Einfügen   reparieren\"   # Freitext\nstatus: verification\nzone: yellow\nrisk: R1\npriority: normal\ncreated: 2026-08-27\nclosed:\ntags: [foot, terminal]\n---\n\n# C-2026-011 — Einfügen reparieren\n\nVorspann.\n\n## Auftrag\nWarum.\n\n## Plan\n- **Ziel:** x\n\n## Protokoll\n- 2026-08-27 · angelegt\n\n```\n## kein Abschnitt\n```\n### Unterpunkt\n\n## Ergebnis\nGut.\n";

    #[test]
    fn a_kit_case_maps_onto_a_seldon_case() {
        let k = parse_case("pipeline/cases/C-2026-011-foot-paste.md", CASE).unwrap();
        assert_eq!(k.title, "Einfügen reparieren");
        assert_eq!(k.slug, "foot-paste");
        let mut rw = Rewriter::new(BTreeMap::new());
        let p = build_case(k, "C-2026-011".into(), None, &now(), &mut rw);
        assert_eq!(p.case.status, CaseStatus::Queued);
        assert_eq!(p.case.tags, ["foot", "terminal", "omarchy-agent"]);
        assert_eq!(p.path, "work/queued/C-2026-011-foot-paste.md");
        assert_eq!(
            p.body,
            "# C-2026-011 — Einfügen reparieren\n\n## Intent\nWarum.\n\n## Plan\n- **Ziel:** x\n\n## History\n<!-- imported from omarchy-agent: the kit's other sections, verbatim -->\nVorspann.\n\n### Protokoll\n- 2026-08-27 · angelegt\n\n```\n## kein Abschnitt\n```\n#### Unterpunkt\n\n## Log\n<!-- append-only; engine and agents add dated lines -->\n- 2026-10-02 10:00 · imported from omarchy-agent pipeline/cases/C-2026-011-foot-paste.md (status verification → queued); imported cases are never active; check the state before `seldon plan start` · human\n\n## Result\nGut.\n"
        );
        // the body is a valid case document with the canonical frontmatter
        let text = model::render_new(&p.case, &p.body);
        let (case, _) = model::parse::<Case>(&text).unwrap();
        assert_eq!(case, p.case);
        assert_eq!(
            cases::section(&p.body, "Plan").map(|r| &p.body[r]),
            Some("- **Ziel:** x\n\n")
        );
    }

    #[test]
    fn a_renumbered_done_case_without_closed() {
        let text = CASE
            .replace("status: verification", "status: done")
            .replace("Warum.", "Folgt auf [[C-2026-010-alt]] und C-2026-010.");
        let k = parse_case("archive/cases/C-2026-011-foot-paste.md", &text).unwrap();
        let mut rw = Rewriter::new(BTreeMap::from([
            ("C-2026-011".to_string(), "C-2026-040".to_string()),
            ("C-2026-010".to_string(), "C-2026-039".to_string()),
        ]));
        let p = build_case(
            k,
            "C-2026-040".into(),
            Some("work/queued/C-2026-011-x.md".into()),
            &now(),
            &mut rw,
        );
        assert_eq!(p.case.closed, NaiveDate::from_ymd_opt(2026, 8, 27));
        assert_eq!(
            p.assumption.as_deref(),
            Some("status done without `closed`: closed set to `created` (2026-08-27)")
        );
        assert!(
            p.body
                .contains("## Intent\nFolgt auf [[C-2026-039-alt]] und C-2026-039.\n")
        );
        // the import's own lines keep the old id
        assert!(
            p.body
                .contains("*Imported from omarchy-agent as C-2026-011;")
        );
        assert!(
            p.body
                .contains("renumbered from C-2026-011; closed date assumed: 2026-08-27 · human")
        );
        assert_eq!(p.case.tags.last().unwrap(), "omarchy-agent/C-2026-011");
        assert_eq!(rw.by_file["archive/cases/C-2026-011-foot-paste.md"], (1, 1));
    }

    #[test]
    fn bad_cases_are_errors_with_a_reason() {
        for (from, to, reason) in [
            (
                "status: verification",
                "status: waiting",
                "status `waiting`",
            ),
            ("zone: yellow", "zone: blue", "not a valid Zone"),
            ("created: 2026-08-27", "created: gestern", "not a date"),
            ("id: C-2026-011", "id: 11", "not C-YYYY-NNN"),
            ("type: case", "type: finding", "not `case`"),
        ] {
            let err = parse_case("x.md", &CASE.replace(from, to)).unwrap_err();
            assert!(err.contains(reason), "{to}: {err}");
        }
        assert_eq!(parse_case("x.md", "# no\n").unwrap_err(), "no frontmatter");
    }

    #[test]
    fn sessions_split_at_dated_headings() {
        let text = "# Journal 2026-09\n\n*Append-only.*\n\n## 2026-09-02 — A\neins\n### Nachtrag\nzwei\n## Notizen\ndrei\n## 2026-09-03 (Fortsetzung) — B\n```\n## 2026-09-04 im Code\n```\n";
        let s = split_sessions("journal/2026-09.md", text);
        assert_eq!(s.len(), 2);
        assert_eq!(
            s[0].text,
            "## 2026-09-02 — A\neins\n### Nachtrag\nzwei\n## Notizen\ndrei\n"
        );
        assert_eq!(s[1].date, NaiveDate::from_ymd_opt(2026, 9, 3).unwrap());
    }

    #[test]
    fn deviation_entries_and_paths() {
        let text = "# Abweichungen\n\n## Aktuelle Abweichungen\n\n### Q1 — Reste in `~/.config/hypr/`  *(alt)*\nText. Datum: 2026-08-17.\n\n### Q2 — Software\n`install/x.packages`, `zed`. Datum 2026-08-18\n\n### Q3 — Schlüssel\nIn `/etc/vconsole.conf` und `~/.bashrc`.\n\n### Q7 — Reste ✅ aufgelöst 2026-08-26\n`~/.x`\n\n## Aufgelöst (alt)\n\n### A1 — `~/.y`\n";
        let e = deviation_entries(text);
        assert_eq!(
            e.iter()
                .map(|e| (e.label.as_str(), e.resolved))
                .collect::<Vec<_>>(),
            [
                ("Q1", false),
                ("Q2", false),
                ("Q3", false),
                ("Q7", true),
                ("A1", true)
            ]
        );
        assert_eq!(
            first_path(&e[0].heading).as_deref(),
            Some("~/.config/hypr/")
        );
        assert_eq!(first_path(&e[1].body), None);
        assert_eq!(
            first_path(&e[2].body).as_deref(),
            Some("/etc/vconsole.conf")
        );
    }

    #[test]
    fn ids_from_names() {
        assert_eq!(
            id_prefix("C-2026-004-zed.md").as_deref(),
            Some("C-2026-004")
        );
        assert_eq!(id_prefix("C-2026-1004").as_deref(), Some("C-2026-1004"));
        assert_eq!(id_prefix("C-26-004-x.md"), None);
        assert_eq!(id_prefix("notes.md"), None);
        assert_eq!(split_id("C-2026-012"), ("2026".to_string(), 12));
    }

    #[test]
    fn knowledge_sections() {
        let s = memory_section(
            "knowledge/omarchy/themes.md",
            "---\ntype: knowledge\ntitle: \"Themes  im Detail\"\nupdated: 2026-08-17\n---\n\n# Themes\n\nText.\n\n## Teil\nmehr\n",
        );
        assert_eq!(
            s,
            "## Themes im Detail\n*Imported from omarchy-agent: `knowledge/omarchy/themes.md`, updated 2026-08-17.*\n\nText.\n\n### Teil\nmehr\n"
        );
        let s = memory_section("knowledge/lessons/pfad.md", "# Den Pfad prüfen\n\nKern.\n");
        assert!(s.starts_with("## Den Pfad prüfen\n*Imported from omarchy-agent: `knowledge/lessons/pfad.md`.*\n\nKern.\n"), "{s}");
        assert_eq!(memory_file("lessons"), "memory/lessons.md");
        assert_eq!(memory_file("Claude Code"), "memory/claude-code.md");
    }
}
