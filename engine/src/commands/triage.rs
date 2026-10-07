//! Bulk triage (ADR-0034 §6, ADR-0035 §6, ADR-0036): `seldon drift
//! propose`, `drift apply`, `drift discard`.
//!
//! - **Propose** (an agent): JSON on stdin ([`Input`]) — items `{eventId,
//!   action: link|explain, caseId | title + intent, evidence: [{kind,
//!   ref}]}`. Every item must name an open drift item, every evidence ref
//!   must resolve in the logbook ([`Evidencer`]); the engine writes each
//!   ref's `text` and each item's `crisis` itself, an agent cannot. All or
//!   nothing: the first bad item exits 1 and names it. The proposal is
//!   stored as `<state>/proposals/<id>.json` (`proposal.schema.json`,
//!   checked before it is written) and replaces this logbook's earlier
//!   one; the index points at it (`index.triage`).
//! - **Apply** (the user): reads the file as the index does, then decides
//!   every item again from the ledger: an item no longer open is skipped, a
//!   crisis (the engine's class now, or the file's flag) only when named
//!   by `--item`, every evidence ref is resolved again (the file's `text`
//!   is never read). Links and explanations go through `drift
//!   link|explain`'s write ([`super::drift::write_resolution`]) with the
//!   resolution detail `proposed by agent:<name> — <evidence>`. A run
//!   without `--item` marks the proposal applied.
//! - **Discard** (the user): removes the file.
//!
//! Nothing here ever reaches a prompt or a command line: the agent is
//! launched with ids only (`seldon agent ask`, ADR-0036 §1).

use std::collections::HashSet;
use std::fmt::Write as _;
use std::io::{IsTerminal as _, Read as _};
use std::path::{Path, PathBuf};

use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use ulid::Ulid;

use super::drift::{Action, Explain, not_open, warn, write_resolution};
use super::event::{ACTOR_ENV, actor_or_env, clip, env_actor, parse_person};
use super::{Context, Output, autocommit, one_line};
use crate::config::{Config, Dirs};
use crate::error::{Error, Result};
use crate::index::check::Validator;
use crate::index::class::Class;
use crate::index::drift::names_token;
use crate::index::{self, Built};
use crate::logbook::lock::Lock;
use crate::logbook::{Logbook, cases};
use crate::model::event::{ACTOR_HUMAN, ACTOR_SYSTEM, Event, Kind, Source, format_ts};
use crate::model::{Journal, Risk, is_agent, is_case_id, is_ulid};
use crate::redact::Redactor;
use crate::sys;

/// The proposals' directory in the state directory (as the index reads it).
pub use crate::index::triage::DIR;

/// Most items in one proposal (`proposal.schema.json`).
pub const ITEMS_MAX: usize = 200;
/// Most evidence refs of one item.
pub const REFS_MAX: usize = 10;
/// Longest ref, in characters.
pub const REF_MAX: usize = 64;
/// Longest resolved evidence text, in characters.
pub const TEXT_MAX: usize = 256;
/// Longest title of an `explain` item, in characters.
pub const TITLE_MAX: usize = 256;
/// Longest intent of an `explain` item, in characters.
pub const INTENT_MAX: usize = 4096;
/// Longest evidence text in a resolution's detail, in characters.
pub const DETAIL_TEXT_MAX: usize = 120;
/// Longest resolution detail `drift apply` writes, in characters.
pub const DETAIL_MAX: usize = 1024;

/// What an item proposes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Proposed {
    Link,
    Explain,
}

impl Proposed {
    pub fn as_str(self) -> &'static str {
        match self {
            Proposed::Link => "link",
            Proposed::Explain => "explain",
        }
    }
}

/// What an evidence ref names (ADR-0036 §2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RefKind {
    /// A journal entry: `YYYY-MM-DD HH:MM`.
    Journal,
    /// A ledger event: its id.
    Event,
    /// A snapper snapshot the ledger recorded: its number.
    Snapshot,
    /// A case: its id (its title).
    Case,
    /// A case's *Plan*: the case id (the line naming the change).
    Plan,
}

impl RefKind {
    pub fn as_str(self) -> &'static str {
        match self {
            RefKind::Journal => "journal",
            RefKind::Event => "event",
            RefKind::Snapshot => "snapshot",
            RefKind::Case => "case",
            RefKind::Plan => "plan",
        }
    }
}

/// `drift propose`'s input: what the agent may say, nothing more.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    items: Vec<InputItem>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct InputItem {
    event_id: String,
    action: Proposed,
    #[serde(default)]
    case_id: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    intent: Option<String>,
    evidence: Vec<InputRef>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct InputRef {
    kind: RefKind,
    #[serde(rename = "ref")]
    reference: String,
}

/// A stored proposal (`schema/proposal.schema.json`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Proposal {
    pub id: String,
    pub at: String,
    pub actor: String,
    pub logbook: String,
    pub applied: Option<String>,
    pub items: Vec<Item>,
}

/// One item of a stored proposal.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Item {
    pub event_id: String,
    pub action: Proposed,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub case_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intent: Option<String>,
    pub crisis: bool,
    pub evidence: Vec<Evidence>,
}

/// One evidence ref with the engine's text.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Evidence {
    pub kind: RefKind,
    #[serde(rename = "ref")]
    pub reference: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
}

// ---------------------------------------------------------------------------
// Evidence
// ---------------------------------------------------------------------------

/// Resolves evidence refs against the logbook as it is now (ADR-0036 §2).
/// The text is the logbook's own (user content), redacted, one line, at
/// most [`TEXT_MAX`] characters.
pub struct Evidencer<'a> {
    pub logbook: &'a Logbook,
    pub built: &'a Built,
    pub redactor: &'a Redactor,
}

impl Evidencer<'_> {
    /// The text `kind ref` resolves to for an item of `members`; `Err`:
    /// why it does not resolve.
    pub fn resolve(
        &self,
        kind: RefKind,
        r: &str,
        members: &[&Event],
    ) -> std::result::Result<String, String> {
        if r.chars().count() > REF_MAX {
            return Err(format!("longer than {REF_MAX} characters"));
        }
        let raw = match kind {
            RefKind::Journal => self.journal(r)?,
            RefKind::Event => self.event(r, members)?,
            RefKind::Snapshot => self.snapshot(r)?,
            RefKind::Case => self.case(r)?.case.title,
            RefKind::Plan => self.plan(r, members)?,
        };
        let text = clip(&one_line_text(&self.redactor.redact(&raw)), TEXT_MAX);
        if text.is_empty() {
            return Err("it holds no text".to_string());
        }
        Ok(text)
    }

    /// The text of the journal entry headed `HH:MM` on that day.
    fn journal(&self, r: &str) -> std::result::Result<String, String> {
        let at = NaiveDateTime::parse_from_str(r, "%Y-%m-%d %H:%M")
            .ok()
            .filter(|_| r.len() == 16)
            .ok_or("not a journal time (YYYY-MM-DD HH:MM)")?;
        let rel = Journal::relative_path(at.date());
        let text = sys::read_small_file(&self.logbook.path(&rel), sys::STATE_FILE_MAX)
            .map_err(|e| format!("{rel}: {e}"))?
            .ok_or_else(|| format!("no journal on {}", at.date()))?;
        let time = at.time().format("%H:%M").to_string();
        let entries = index::load::journal_entries(&text);
        let entry = entries
            .iter()
            .find(|e| e.time == time && !e.text.trim().is_empty())
            .ok_or_else(|| format!("no journal entry at {r}"))?;
        Ok(entry.text.clone())
    }

    /// A ledger event that is no resolution and not the item's own.
    fn event(&self, r: &str, members: &[&Event]) -> std::result::Result<String, String> {
        let id: Ulid = is_ulid(r)
            .then(|| r.parse().ok())
            .flatten()
            .ok_or("not an event id (a ULID)")?;
        if members.iter().any(|m| m.id == id) {
            return Err("the change itself is no evidence for it".to_string());
        }
        let Some(e) = self
            .built
            .folded
            .iter()
            .map(|f| &f.event)
            .find(|e| e.id == id)
        else {
            return Err(if self.built.ledger.iter().any(|e| e.id == id) {
                "a resolution is no evidence".to_string()
            } else {
                "no such event in the ledger".to_string()
            });
        };
        let mut text = format!("{} {}", e.kind, e.subject);
        if e.actor != ACTOR_SYSTEM {
            let _ = write!(text, " by {}", e.actor);
        }
        if let Some(d) = e.detail.as_deref().filter(|d| !d.trim().is_empty()) {
            let _ = write!(text, ": {d}");
        }
        Ok(text)
    }

    /// The newest snapper snapshot event with that number.
    fn snapshot(&self, r: &str) -> std::result::Result<String, String> {
        if r.is_empty() || r.len() > 10 || !r.bytes().all(|b| b.is_ascii_digit()) {
            return Err("not a snapshot number".to_string());
        }
        let e = self
            .built
            .ledger
            .iter()
            .rev()
            .find(|e| e.source == Source::Snapper && e.kind == Kind::Snapshot && e.subject == r)
            .ok_or_else(|| format!("the ledger records no snapshot {r}"))?;
        Ok(e.detail
            .clone()
            .filter(|d| !d.trim().is_empty())
            .unwrap_or_else(|| format!("snapshot {r}")))
    }

    fn case(&self, r: &str) -> std::result::Result<cases::CaseFile, String> {
        if !is_case_id(r) {
            return Err("not a case id".to_string());
        }
        cases::find(self.logbook, r).map_err(|e| e.to_string())
    }

    /// The first line of the case's *Plan* that names a member's subject
    /// as a whole word (ADR-0015 §4).
    fn plan(&self, r: &str, members: &[&Event]) -> std::result::Result<String, String> {
        let file = self.case(r)?;
        let body = &file.doc.body;
        let range = cases::section(body, "Plan").ok_or_else(|| format!("{r} has no Plan"))?;
        body[range]
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .find(|l| members.iter().any(|m| names_token(l, &m.subject)))
            .map(str::to_string)
            .ok_or_else(|| format!("the Plan of {r} names none of the change's subjects"))
    }
}

/// `text` on one line: every control character a space, white space runs
/// one space, trimmed.
fn one_line_text(text: &str) -> String {
    text.chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// The resolution detail of an applied item: `proposed by <actor> — <kind>
/// <ref> "<text>"; …`, each text at most [`DETAIL_TEXT_MAX`] characters,
/// the whole at most [`DETAIL_MAX`].
pub fn detail_line(actor: &str, evidence: &[(RefKind, String, String)]) -> String {
    let refs: Vec<String> = evidence
        .iter()
        .map(|(kind, r, text)| format!("{} {r} \"{}\"", kind.as_str(), clip(text, DETAIL_TEXT_MAX)))
        .collect();
    clip(
        &format!("proposed by {actor} — {}", refs.join("; ")),
        DETAIL_MAX,
    )
}

// ---------------------------------------------------------------------------
// Store
// ---------------------------------------------------------------------------

/// `<state>/proposals/`.
pub fn dir(dirs: &Dirs) -> PathBuf {
    dirs.state_dir.join(DIR)
}

/// The logbook as a proposal names it: its canonical path.
fn logbook_key(logbook: &Logbook) -> String {
    std::fs::canonicalize(&logbook.root)
        .unwrap_or_else(|_| logbook.root.clone())
        .to_string_lossy()
        .into_owned()
}

/// Ids of the readable proposals of the logbook `key` in `dir`, with their
/// `applied`, other than `except`. A file that is no regular file or does
/// not read is not looked at.
fn proposals_of(dir: &Path, key: &str, except: &str) -> Vec<(String, Option<String>)> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in entries.filter_map(|e| e.ok()) {
        let name = entry.file_name().to_string_lossy().into_owned();
        let Some(id) = name.strip_suffix(".json").filter(|id| is_ulid(id)) else {
            continue;
        };
        if id == except {
            continue;
        }
        let Ok(Some(text)) = sys::read_small_file(&entry.path(), sys::STATE_FILE_MAX) else {
            continue;
        };
        let Ok(v) = serde_json::from_str::<Value>(&text) else {
            continue;
        };
        if v["logbook"].as_str() == Some(key) {
            out.push((id.to_string(), v["applied"].as_str().map(String::from)));
        }
    }
    out.sort();
    out
}

/// Writes `p` (checked against its schema), then removes this logbook's
/// other proposals: what it replaced.
fn store(dirs: &Dirs, p: &Proposal) -> Result<Vec<(String, Option<String>)>> {
    let value = serde_json::to_value(p).map_err(anyhow::Error::from)?;
    if let Some(e) = Validator::new()
        .validate(&value, "proposal.schema.json")
        .first()
    {
        return Err(anyhow::anyhow!("refusing to store an invalid proposal: {e}").into());
    }
    let dir = dir(dirs);
    sys::create_dir_private(&dir)
        .map_err(|e| anyhow::anyhow!("cannot create {}: {e}", dir.display()))?;
    let mut text = serde_json::to_string_pretty(&value).map_err(anyhow::Error::from)?;
    text.push('\n');
    sys::write_atomic_replace(
        &dir.join(format!("{}.json", p.id)),
        text.as_bytes(),
        sys::NEW_FILE_MODE,
    )?;
    let replaced = proposals_of(&dir, &p.logbook, &p.id);
    for (id, _) in &replaced {
        if let Err(e) = std::fs::remove_file(dir.join(format!("{id}.json"))) {
            eprintln!("seldon: warning: proposal {id} not removed: {e}");
        }
    }
    Ok(replaced)
}

/// The proposal `id` of this logbook, read as the index reads it (a
/// regular file of at most 4 MiB, its schema, its name its id); exit 1
/// otherwise.
fn load(dirs: &Dirs, logbook: &Logbook, id: &str) -> Result<(PathBuf, Proposal)> {
    let path = dir(dirs).join(format!("{id}.json"));
    let text = sys::read_small_file(&path, sys::STATE_FILE_MAX)
        .map_err(|e| Error::user(format!("proposal {id} is not read: {e}")))?
        .ok_or_else(|| Error::user(format!("no proposal {id}")))?;
    let value: Value = serde_json::from_str(&text)
        .map_err(|e| Error::user(format!("proposal {id} is not JSON: {e}")))?;
    if let Some(e) = Validator::new()
        .validate(&value, "proposal.schema.json")
        .first()
    {
        return Err(Error::user(format!(
            "proposal {id} is not a valid proposal: {e}"
        )));
    }
    let p: Proposal = serde_json::from_value(value)
        .map_err(|e| Error::user(format!("proposal {id} is not a valid proposal: {e}")))?;
    if p.id != id {
        return Err(Error::user(format!("proposal {id} names another id")));
    }
    if p.logbook != logbook_key(logbook) {
        return Err(Error::user(format!(
            "proposal {id} was made for another logbook"
        )));
    }
    Ok((path, p))
}

/// clap value parser: a proposal id (a ULID).
pub fn parse_proposal_id(s: &str) -> Result<String, String> {
    if is_ulid(s) {
        Ok(s.to_string())
    } else {
        Err(format!(
            "`{s}` is not a proposal id (a ULID, 26 characters)"
        ))
    }
}

/// The actor of the user's act on a proposal (apply, discard): human only.
/// An agent proposes; applying is the user's (ADR-0036 §4), and an agent's
/// act is never recorded as human (ADR-0028 §3).
fn user_actor(flag: Option<String>, what: &str) -> Result<String> {
    if flag.as_deref() == Some(ACTOR_HUMAN)
        && let Ok(Some(session)) = env_actor(parse_person)
        && is_agent(&session)
    {
        return Err(Error::user(format!(
            "`--actor human` in a session of {session} ({ACTOR_ENV}): an agent's act is \
             never recorded as human (ADR-0028 §3); {what} a proposal from your own session \
             (the desk)"
        )));
    }
    let actor = actor_or_env(flag, parse_person, ACTOR_HUMAN)?;
    if is_agent(&actor) {
        return Err(Error::user(format!(
            "{actor} may propose (`seldon drift propose`); only the user may {what} a \
             proposal (ADR-0036)"
        )));
    }
    Ok(actor)
}

// ---------------------------------------------------------------------------
// drift propose
// ---------------------------------------------------------------------------

/// `seldon drift propose [--file F] [--actor A]`.
pub fn propose(ctx: &Context, file: Option<&Path>, actor: Option<String>) -> Result<Output> {
    let actor = actor_or_env(actor, parse_person, ACTOR_HUMAN)?;
    if !is_agent(&actor) {
        return Err(Error::user(format!(
            "a proposal is an agent's (--actor agent:<name>, or {ACTOR_ENV}); as {actor}, \
             resolve directly: `seldon drift link|explain`"
        )));
    }
    let text = read_input(file)?;
    let input: Input = serde_json::from_str(&text).map_err(|e| {
        Error::user(format!(
            "the proposal is not of the expected shape ({e}): {{\"items\": [{{\"eventId\", \
             \"action\": \"link\"|\"explain\", \"caseId\" | \"title\" + \"intent\", \
             \"evidence\": [{{\"kind\", \"ref\"}}]}}]}}; nothing was stored"
        ))
    })?;
    if input.items.is_empty() {
        return Err(Error::user(
            "the proposal has no items; when nothing has evidence, store none and tell the user"
                .to_string(),
        ));
    }
    if input.items.len() > ITEMS_MAX {
        return Err(Error::user(format!(
            "the proposal has {} items, at most {ITEMS_MAX}; nothing was stored",
            input.items.len()
        )));
    }

    let (config, logbook) = ctx.open_logbook()?;
    let redactor = Redactor::for_config(&config)?;
    let lock = ctx.lock()?;
    let built = index::derive(ctx, &config, &logbook)?;
    warn(&built);
    let evidencer = Evidencer {
        logbook: &logbook,
        built: &built,
        redactor: &redactor,
    };
    let mut seen = HashSet::new();
    let mut items = Vec::with_capacity(input.items.len());
    for (n, item) in input.items.into_iter().enumerate() {
        let shown = item.event_id.chars().take(40).collect::<String>();
        let checked = check_item(&evidencer, &logbook, item, &mut seen).map_err(|why| {
            Error::user(format!(
                "item {} ({}): {why}; nothing was stored",
                n + 1,
                shown.escape_debug()
            ))
        })?;
        items.push(checked);
    }

    let proposal = Proposal {
        id: Ulid::generate().to_string(),
        at: format_ts(&ctx.now),
        actor,
        logbook: logbook_key(&logbook),
        applied: None,
        items,
    };
    let replaced = store(&ctx.dirs, &proposal)?;
    index::rebuild_if_initialised(ctx);
    drop(lock);

    let crises = proposal.items.iter().filter(|i| i.crisis).count();
    let path = dir(&ctx.dirs).join(format!("{}.json", proposal.id));
    let mut human = format!(
        "Proposal {}: {} item(s), {crises} crisis; nothing is written to the logbook until the \
         user applies it (`seldon drift apply {}`).",
        proposal.id,
        proposal.items.len(),
        proposal.id
    );
    for (id, applied) in &replaced {
        if applied.is_none() {
            let _ = write!(human, "\nReplaced the unapplied proposal {id}.");
        }
    }
    Ok(Output::ok(
        human,
        json!({
            "proposal": {
                "id": proposal.id,
                "at": proposal.at,
                "actor": proposal.actor,
                "path": path,
                "counts": { "items": proposal.items.len(), "crises": crises },
            },
            "items": proposal.items,
            "replaced": replaced
                .iter()
                .map(|(id, applied)| json!({ "id": id, "applied": applied }))
                .collect::<Vec<_>>(),
        }),
    ))
}

/// The proposal's text: `--file` (tests), else stdin, at most 4 MiB.
fn read_input(file: Option<&Path>) -> Result<String> {
    let max = sys::STATE_FILE_MAX;
    if let Some(path) = file {
        return sys::read_small_file(path, max)
            .map_err(|e| Error::user(format!("{}: {e}", path.display())))?
            .ok_or_else(|| Error::user(format!("{}: no such file", path.display())));
    }
    let stdin = std::io::stdin();
    if stdin.is_terminal() {
        return Err(Error::user(
            "pipe the proposal as JSON on stdin (`seldon drift propose --json < proposal.json`)"
                .to_string(),
        ));
    }
    let mut bytes = Vec::new();
    stdin
        .lock()
        .take(max + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| Error::user(format!("cannot read stdin: {e}")))?;
    if bytes.len() as u64 > max {
        return Err(Error::user(format!(
            "the proposal is larger than {max} bytes"
        )));
    }
    String::from_utf8(bytes).map_err(|_| Error::user("the proposal is not UTF-8".to_string()))
}

/// One input item, checked and resolved; `Err`: why it is refused.
fn check_item(
    ev: &Evidencer,
    logbook: &Logbook,
    item: InputItem,
    seen: &mut HashSet<String>,
) -> std::result::Result<Item, String> {
    let built = ev.built;
    let event = crate::reconcile::find(built, &item.event_id).map_err(|e| e.to_string())?;
    let e = &event.event;
    let open = built.open_drift.contains(&e.id);
    let classified = crate::reconcile::item_of(built, e).filter(|_| open);
    let Some(classified) = classified else {
        return Err(format!(
            "{} is not open drift ({}); only an open change can be proposed",
            e.id,
            not_open(event)
        ));
    };
    let leader = classified.item.event_id.clone();
    if !seen.insert(leader.clone()) {
        return Err(format!("its change ({leader}) is already an item"));
    }
    let members = crate::reconcile::linkable_members(built, e);
    let (case_id, title, intent) = match item.action {
        Proposed::Link => {
            if item.title.is_some() || item.intent.is_some() {
                return Err("a link takes a caseId, no title or intent".to_string());
            }
            let case = item.case_id.ok_or("a link needs a caseId")?;
            if !is_case_id(&case) {
                return Err(format!("`{}` is not a case id", case.escape_debug()));
            }
            cases::find(logbook, &case).map_err(|e| e.to_string())?;
            (Some(case), None, None)
        }
        Proposed::Explain => {
            if item.case_id.is_some() {
                return Err("an explanation takes a title and an intent, no caseId".to_string());
            }
            let title = explain_text("title", item.title.as_deref(), TITLE_MAX)?;
            let intent = explain_text("intent", item.intent.as_deref(), INTENT_MAX)?;
            (
                None,
                Some(ev.redactor.redact(&title)),
                Some(ev.redactor.redact(&intent)),
            )
        }
    };
    if item.evidence.is_empty() {
        return Err("no evidence; an item without evidence stays open".to_string());
    }
    if item.evidence.len() > REFS_MAX {
        return Err(format!("more than {REFS_MAX} evidence refs"));
    }
    let mut evidence = Vec::with_capacity(item.evidence.len());
    for r in item.evidence {
        let text = ev.resolve(r.kind, &r.reference, &members).map_err(|why| {
            format!(
                "evidence {} `{}` does not resolve ({why}); an item without evidence stays open",
                r.kind.as_str(),
                r.reference
                    .chars()
                    .take(REF_MAX)
                    .collect::<String>()
                    .escape_debug()
            )
        })?;
        evidence.push(Evidence {
            kind: r.kind,
            reference: r.reference,
            text: Some(text),
        });
    }
    Ok(Item {
        event_id: leader,
        action: item.action,
        case_id,
        title,
        intent,
        crisis: classified.class == Class::Crisis,
        evidence,
    })
}

/// An explain item's title or intent: one line, not blank, at most `max`
/// characters.
fn explain_text(what: &str, text: Option<&str>, max: usize) -> std::result::Result<String, String> {
    let text = text.ok_or_else(|| format!("an explanation needs a {what}"))?;
    let text = one_line(&format!("the {what}"), text).map_err(|e| e.to_string())?;
    if text.chars().count() > max {
        return Err(format!("the {what} is longer than {max} characters"));
    }
    Ok(text)
}

// ---------------------------------------------------------------------------
// drift apply
// ---------------------------------------------------------------------------

/// What `drift apply` did with one item.
enum Outcome {
    Done(Box<super::drift::Resolved>),
    Skipped(String),
    Refused(String),
}

/// `seldon drift apply <PROPOSAL> [--item <EVENT>]… [--actor A]`.
pub fn apply(ctx: &Context, id: &str, named: &[String], actor: Option<String>) -> Result<Output> {
    let actor = user_actor(actor, "apply")?;
    let (config, logbook) = ctx.open_logbook()?;
    let redactor = Redactor::for_config(&config)?;
    let lock = ctx.lock()?;
    let (path, mut proposal) = load(&ctx.dirs, &logbook, id)?;
    for n in named {
        if !proposal.items.iter().any(|i| &i.event_id == n) {
            return Err(Error::user(format!(
                "{n} is not an item of proposal {id}; nothing was applied"
            )));
        }
    }
    let selected: Vec<Item> = proposal
        .items
        .iter()
        .filter(|i| named.is_empty() || named.contains(&i.event_id))
        .cloned()
        .collect();

    let mut done = Vec::new();
    let mut skipped = Vec::new();
    let mut refused = Vec::new();
    let mut built: Option<Built> = None;
    let mut warned = false;
    for item in &selected {
        // a fresh derive after every write: groups and classes move
        if built.is_none() {
            let b = index::derive(ctx, &config, &logbook)?;
            if !warned {
                warn(&b);
                warned = true;
            }
            built = Some(b);
        }
        let b = built.as_ref().expect("derived above");
        let by_name = named.contains(&item.event_id);
        let outcome = apply_item(
            ctx,
            &config,
            &logbook,
            &lock,
            b,
            &redactor,
            &proposal.actor,
            &actor,
            item,
            by_name,
        );
        let id = item.event_id.clone();
        match outcome {
            Outcome::Done(r) => {
                built = None;
                done.push((item, r));
            }
            Outcome::Skipped(why) => skipped.push((id, why)),
            Outcome::Refused(why) => {
                // a failed write may have changed the logbook
                built = None;
                refused.push((id, why));
            }
        }
    }

    let first = named.is_empty() && proposal.applied.is_none();
    if first {
        proposal.applied = Some(format_ts(&ctx.now));
        let value = serde_json::to_value(&proposal).map_err(anyhow::Error::from)?;
        let mut text = serde_json::to_string_pretty(&value).map_err(anyhow::Error::from)?;
        text.push('\n');
        sys::write_atomic_replace(&path, text.as_bytes(), sys::NEW_FILE_MODE)?;
    }
    let events: usize = done.iter().map(|(_, r)| r.resolved).sum();
    let commit = if done.is_empty() {
        super::Commit::Skipped("nothing written")
    } else {
        autocommit(
            ctx,
            &config,
            &logbook,
            &format!(
                "drift apply: {} item(s), {events} event(s), proposal {id}",
                done.len()
            ),
        )
    };
    if first || !done.is_empty() {
        index::rebuild_if_initialised(ctx);
    }
    drop(lock);

    let mut human = format!(
        "Proposal {id}: {} applied, {} skipped, {} refused",
        done.len(),
        skipped.len(),
        refused.len()
    );
    for (item, r) in &done {
        let _ = write!(
            human,
            "\n  {} {} {} event(s){}",
            item.event_id,
            r.verb(),
            r.resolved,
            r.case_id
                .as_deref()
                .map(|c| format!(", {c}"))
                .unwrap_or_default()
        );
    }
    for (eid, why) in &skipped {
        let _ = write!(human, "\n  {eid} skipped: {why}");
    }
    for (eid, why) in &refused {
        let _ = write!(human, "\n  {eid} refused: {why}");
    }
    human.push_str(&commit.human());
    Ok(Output::ok(
        human,
        json!({
            "proposal": id,
            "applied": proposal.applied,
            "done": done.iter().map(|(item, r)| json!({
                "eventId": item.event_id,
                "action": item.action,
                "resolved": r.resolved,
                "case": r.case_id,
                "events": r.written.iter().map(super::event::event_json).collect::<Vec<_>>(),
            })).collect::<Vec<_>>(),
            "skipped": skipped.iter().map(|(eid, why)| json!({ "eventId": eid, "reason": why })).collect::<Vec<_>>(),
            "refused": refused.iter().map(|(eid, why)| json!({ "eventId": eid, "reason": why })).collect::<Vec<_>>(),
            "git": commit.json(),
        }),
    ))
}

/// One item, decided again from `built` (derived under `lock`).
#[allow(clippy::too_many_arguments)]
fn apply_item(
    ctx: &Context,
    config: &Config,
    logbook: &Logbook,
    lock: &Lock,
    built: &Built,
    redactor: &Redactor,
    proposer: &str,
    actor: &str,
    item: &Item,
    by_name: bool,
) -> Outcome {
    let action = match item.action {
        Proposed::Link => match item.case_id.as_deref() {
            Some(case) => Action::Link {
                case: case.to_string(),
            },
            None => return Outcome::Refused("a link without a case".to_string()),
        },
        Proposed::Explain => {
            let title = explain_text("title", item.title.as_deref(), TITLE_MAX);
            let intent = explain_text("intent", item.intent.as_deref(), INTENT_MAX);
            match (title, intent) {
                (Ok(title), Ok(intent)) => Action::Explain(Explain {
                    title: Some(title),
                    intent,
                    zone: None,
                    risk: Risk::R1,
                    area: None,
                }),
                (Err(why), _) | (_, Err(why)) => return Outcome::Refused(why),
            }
        }
    }
    .redacted(redactor);
    let sel = match crate::reconcile::select(built, &item.event_id, false, action.intent()) {
        Ok(sel) => sel,
        Err(e) => return Outcome::Refused(e.to_string()),
    };
    if sel.members.is_empty() {
        return Outcome::Skipped(not_open(sel.event));
    }
    // the engine's class now, or the file's flag: the flag can hold an item
    // back, never let one through (ADR-0035 §6, ADR-0028 §3)
    if (sel.crisis() || item.crisis) && !by_name {
        return Outcome::Skipped(
            "crisis: applied only one by one (`--item`), never with the rest".to_string(),
        );
    }
    // the evidence again, from the logbook as it is now; the file's text is
    // never read
    let evidencer = Evidencer {
        logbook,
        built,
        redactor,
    };
    let mut resolved = Vec::with_capacity(item.evidence.len());
    for r in &item.evidence {
        match evidencer.resolve(r.kind, &r.reference, &sel.members) {
            Ok(text) => resolved.push((r.kind, r.reference.clone(), text)),
            Err(why) => {
                return Outcome::Refused(format!(
                    "evidence {} `{}` no longer resolves ({why})",
                    r.kind.as_str(),
                    r.reference.escape_debug()
                ));
            }
        }
    }
    let case_file = match &action {
        Action::Link { case } => match cases::find(logbook, case) {
            Ok(file) => Some(file),
            Err(e) => return Outcome::Refused(e.to_string()),
        },
        _ => None,
    };
    let detail = detail_line(proposer, &resolved);
    match write_resolution(
        ctx,
        config,
        logbook,
        lock,
        built,
        &sel,
        actor,
        &action,
        Some(detail),
        case_file,
    ) {
        Ok(r) => Outcome::Done(Box::new(r)),
        Err(e) => Outcome::Refused(e.to_string()),
    }
}

// ---------------------------------------------------------------------------
// drift discard
// ---------------------------------------------------------------------------

/// `seldon drift discard <PROPOSAL> [--actor A]`: removes the file; the
/// ledger is not touched.
pub fn discard(ctx: &Context, id: &str, actor: Option<String>) -> Result<Output> {
    user_actor(actor, "discard")?;
    let (_, logbook) = ctx.open_logbook()?;
    let lock = ctx.lock()?;
    let (path, proposal) = load(&ctx.dirs, &logbook, id)?;
    std::fs::remove_file(&path)
        .map_err(|e| anyhow::anyhow!("cannot remove {}: {e}", path.display()))?;
    index::rebuild_if_initialised(ctx);
    drop(lock);
    Ok(Output::ok(
        format!(
            "Discarded proposal {id}{}; the logbook is unchanged",
            if proposal.applied.is_some() {
                " (applied before)"
            } else {
                ""
            }
        ),
        json!({ "discarded": id, "applied": proposal.applied }),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_detail_names_the_proposer_and_each_ref() {
        let d = detail_line(
            "agent:claude-code",
            &[
                (
                    RefKind::Journal,
                    "2026-10-01 14:40".into(),
                    "Snapshot 113".into(),
                ),
                (RefKind::Plan, "C-2026-005".into(), "x".repeat(200)),
            ],
        );
        assert!(d.starts_with(
            "proposed by agent:claude-code — journal 2026-10-01 14:40 \"Snapshot 113\"; plan C-2026-005 \""
        ));
        assert!(
            d.contains(&format!("{}…\"", "x".repeat(DETAIL_TEXT_MAX - 1))),
            "{d}"
        );
        let long: Vec<_> = (0..10)
            .map(|i| (RefKind::Case, format!("C-2026-{i:03}"), "y".repeat(200)))
            .collect();
        assert_eq!(detail_line("agent:x", &long).chars().count(), DETAIL_MAX);
    }

    #[test]
    fn texts_are_one_line() {
        assert_eq!(one_line_text(" a\n\tb\r\n  c\u{7}d "), "a b c d");
    }
}
