//! `plugins` collector (SPEC-ENGINE §4): `omarchy plugin list --json` diff →
//! `plugin-add|plugin-remove|plugin-enable|plugin-disable|plugin-update`.
//!
//! `omarchy plugin list --json` is the shell IPC call `listPlugins`
//! (`omarchy-shell shell listPlugins`): it fails when the Omarchy shell is
//! not running, and the collector then degrades (`ok: false`, the message of
//! `omarchy-shell`, cursor kept). Its entries carry `id` and `enabled` (for a
//! bar widget: placed in the bar) but no version, and neither does
//! `omarchy plugin catalog` (memory/host.md item 3). The version comes from
//! the plugin's manifest (ADR-0014 §3): the `manifestPath` that
//! `omarchy plugin catalog` reports for the id, else
//! `~/.config/omarchy/plugins/<id>/manifest.json`; when the manifest has no
//! `version`, the short git HEAD of the plugin directory (`plugin add` is a
//! `git clone`). A version that cannot be read this time keeps the last one
//! seen, so a failing catalog never looks like an update.
//!
//! `plugin-update` fires only for plugins with `firstParty: false`
//! (ADR-0018): first-party plugins ship inside the `omarchy` package, whose
//! `update` event covers them; their versions are tracked in the cursor
//! only. Enabling and disabling fire for every plugin.
//!
//! A third-party plugin whose directory is its own git clone keeps its
//! full HEAD in the cursor (WP-136). Its `plugin-add` says it came by
//! `git clone` (`meta.git: clone`); its `plugin-update` with a moved HEAD
//! names the commits: `pull` (the old HEAD is an ancestor of the new one)
//! and `reset` (another history) list the subjects that came in,
//! `rollback` (the new HEAD is an ancestor) the ones that left. At most
//! [`COMMITS_MAX`] subjects, newest first, one per line in `meta.commits`,
//! each cleaned of control and direction characters, redacted and clipped
//! ([`commit_subject`]); the detail has the count and the newest subject.
//! git runs read-only with a fixed argv ([`Git`]); when it fails or times
//! out the event is the same without the commits.
//!
//! `plugin-add` and `plugin-update` are timed by the later mtime of the
//! plugin directory and its manifest, clamped to `[last check, now]`, so
//! attribution (ADR-0017) can match the agent command that cloned or
//! pulled it. Removal, enabling and disabling get the capture time.
//!
//! The cursor is the snapshot `{id: {enabled, version}}`, its SHA-256 and the
//! time of the last check. Without a cursor the collector takes a baseline
//! (no events). Events the ledger already holds since the last check (same
//! kind, id, version, enabled state, update step) are dropped: a capture
//! whose cursor save failed after its ledger write left the old snapshot in
//! the cursor, and the next diff would repeat them.

use std::collections::BTreeMap;
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime};

use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::config::changed_at;
use super::{Collector, Ctx, Lost, Outcome, RUN_TIMEOUT, Sources, to_cursor, typed_cursor};
use crate::logbook::git::REPOSITORY_VARS;
use crate::model::event::{Event, Kind, Meta, SUBJECT_MAX, Source};
use crate::redact::Redactor;
use crate::sys::{self, Run};

/// Omarchy's user plugin directory relative to `$HOME` (the CLI hard-codes
/// `$HOME/.config`, not XDG).
pub const PLUGINS_DIR: &str = ".config/omarchy/plugins";

/// Longest version string recorded (a manifest is user content).
const VERSION_MAX: usize = 64;

/// Largest manifest read.
const MANIFEST_MAX: u64 = 1024 * 1024;

/// Most commit subjects one event names.
pub const COMMITS_MAX: usize = 20;

/// Longest commit subject recorded, in characters (the cut ends in `…`).
pub const COMMIT_SUBJECT_MAX: usize = 100;

/// Columns of a subject git prints at most (`%<(N,trunc)`): bounds the
/// output before [`COMMIT_SUBJECT_MAX`] clips it.
const COMMIT_SUBJECT_COLUMNS: usize = 200;

/// Time one git query of a plugin clone may take.
const GIT_TIMEOUT: Duration = Duration::from_secs(2);

/// Options before every git query: no pager, no hooks, no file system
/// monitor, no transport (so no fetch of a missing object), no signature
/// check (gpg), no colour, no replace refs.
const GIT_OPTIONS: [&str; 12] = [
    "--no-pager",
    "--no-replace-objects",
    "-c",
    "core.hooksPath=/dev/null",
    "-c",
    "core.fsmonitor=false",
    "-c",
    "protocol.allow=never",
    "-c",
    "log.showSignature=false",
    "-c",
    "color.ui=false",
];

/// Environment of every git query besides [`REPOSITORY_VARS`] removed and
/// `GIT_CEILING_DIRECTORIES`: no system and no global config (the answer
/// depends on the clone alone), no prompt, no lock taken for an index
/// refresh, no lazy fetch of a partial clone.
const GIT_ENV: [(&str, &str); 5] = [
    ("GIT_CONFIG_NOSYSTEM", "1"),
    ("GIT_CONFIG_GLOBAL", "/dev/null"),
    ("GIT_TERMINAL_PROMPT", "0"),
    ("GIT_OPTIONAL_LOCKS", "0"),
    ("GIT_NO_LAZY_FETCH", "1"),
];

pub struct Plugins;

/// One entry of `omarchy plugin list --json` (other fields ignored).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Listed {
    pub id: String,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default, rename = "firstParty")]
    pub first_party: bool,
    /// The source id of a clone (`omarchy plugin clone`), else empty.
    #[serde(default, rename = "clonedFrom")]
    pub cloned_from: String,
}

/// One entry of `omarchy plugin catalog` (other fields ignored).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Cataloged {
    id: String,
    #[serde(default)]
    manifest_path: Option<PathBuf>,
}

/// A plugin as the cursor remembers it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct PluginState {
    enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    version: Option<String>,
    /// The full HEAD of a third-party plugin's own git clone (WP-136).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    head: Option<String>,
}

/// The plugins collector's cursor: the last snapshot, its hash and the
/// time of the last check.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct PluginsCursor {
    hash: String,
    plugins: BTreeMap<String, PluginState>,
    /// Capture time of the last check (missing in older cursors: the
    /// capture time is used then).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    checked: Option<DateTime<FixedOffset>>,
}

impl PluginsCursor {
    fn new(plugins: BTreeMap<String, PluginState>, checked: DateTime<FixedOffset>) -> Self {
        let body = serde_json::to_vec(&plugins).expect("a snapshot always serialises");
        PluginsCursor {
            hash: sys::sha256_hex(&body),
            plugins,
            checked: Some(checked),
        }
    }
}

/// What this run saw of a plugin beyond its cursor state.
#[derive(Debug, Clone, Default)]
struct Seen {
    first_party: bool,
    /// Later mtime of the plugin directory and its manifest.
    touched: Option<SystemTime>,
    /// The plugin's directory when it is a third-party plugin's own git
    /// clone.
    repo: Option<PathBuf>,
}

impl Plugins {
    /// Omarchy's user plugin directory: [`Sources::plugins_dir`]
    /// (`SELDON_OMARCHY_PLUGINS_DIR`), else `~/.config/omarchy/plugins`.
    /// The config collector excludes it.
    pub fn dir(sources: &Sources, home: &Path) -> PathBuf {
        sources
            .plugins_dir
            .clone()
            .unwrap_or_else(|| home.join(PLUGINS_DIR))
    }

    /// [`Collector::collect`] with an explicit program and plugin directory.
    pub fn collect_from(
        &self,
        ctx: &Ctx,
        cursor: Option<&Value>,
        omarchy: &str,
        plugins_dir: &Path,
    ) -> Outcome {
        let prev = typed_cursor::<PluginsCursor>(cursor);
        let listed = match list(omarchy) {
            Ok(l) => l,
            Err(message) => return Outcome::degraded(message, None),
        };
        if listed.is_empty() && prev.as_ref().is_some_and(|p| !p.plugins.is_empty()) {
            // a shell that lists nothing is not a shell without plugins
            return Outcome::degraded("omarchy plugin list --json returned no plugins", None);
        }

        let manifests = catalog(omarchy);
        let mut snapshot = BTreeMap::new();
        let mut seen = BTreeMap::new();
        // one `rev-parse` per clone and capture
        let mut heads: BTreeMap<PathBuf, Option<Head>> = BTreeMap::new();
        let mut head_of = |dir: &Path| {
            heads
                .entry(dir.to_path_buf())
                .or_insert_with(|| head(GIT, dir))
                .clone()
        };
        for p in listed {
            if p.id.is_empty() || p.id.chars().count() > SUBJECT_MAX {
                continue;
            }
            let fallback = plugins_dir.join(&p.id).join("manifest.json");
            let candidates: Vec<&PathBuf> = manifests
                .get(&p.id)
                .into_iter()
                .chain([&fallback])
                .collect();
            // the manifest's `version`, else the short HEAD of its
            // directory's own clone
            let found = candidates.iter().find_map(|m| {
                let version = manifest_version(m).or_else(|| Some(head_of(m.parent()?)?.short))?;
                Some((version, *m))
            });
            // the manifest that answered, else the first one that exists
            let manifest = found
                .as_ref()
                .map(|(_, m)| *m)
                .or_else(|| candidates.iter().copied().find(|m| m.exists()));
            let touched = manifest.and_then(|m| touched(m));
            let last = prev.as_ref().and_then(|prev| prev.plugins.get(&p.id));
            let version = found.map(|(v, _)| v).or_else(|| {
                // unreadable this time: keep the last one seen
                last?.version.clone()
            });
            // a third-party plugin's own clone: the manifest's directory,
            // else the plugin directory
            let dir = manifest
                .and_then(|m| m.parent())
                .map_or_else(|| plugins_dir.join(&p.id), Path::to_path_buf);
            let repo = (!p.first_party && is_clone(&dir)).then_some(dir);
            let head = repo.as_deref().and_then(|dir| {
                // unreadable this time: keep the last one seen
                head_of(dir).map(|h| h.full).or_else(|| last?.head.clone())
            });
            seen.insert(
                p.id.clone(),
                Seen {
                    first_party: p.first_party,
                    touched,
                    repo,
                },
            );
            snapshot.insert(
                p.id,
                PluginState {
                    enabled: p.enabled,
                    version,
                    head,
                },
            );
        }
        let next = PluginsCursor::new(snapshot, ctx.now);

        let (events, since) = match &prev {
            Some(prev) if prev.hash != next.hash => {
                let since = prev.checked.unwrap_or(ctx.now);
                let events = diff(ctx, &prev.plugins, &next.plugins, &seen, since);
                match unrecorded(ctx, events, since) {
                    Ok(events) => (events, Some(since)),
                    Err(e) => {
                        return Outcome::degraded(format!("cannot read the ledger: {e:#}"), None);
                    }
                }
            }
            _ => (Vec::new(), None), // baseline, or nothing changed
        };
        Outcome {
            since,
            ..Outcome::ok(events, to_cursor(&next))
        }
        .baseline(prev.is_none().then_some(Lost::Cursor))
    }
}

/// `events` without those the ledger holds since `since`: same kind, id,
/// version, enabled state and update step. A capture whose cursor save
/// failed after its ledger write left the cursor on the snapshot before
/// them (the config collector does the same).
fn unrecorded(
    ctx: &Ctx,
    events: Vec<Event>,
    since: DateTime<FixedOffset>,
) -> anyhow::Result<Vec<Event>> {
    if events.is_empty() {
        return Ok(events);
    }
    let recorded: Vec<Event> = ctx
        .ledger
        .read_range(since.min(ctx.now), ctx.now)?
        .into_iter()
        .filter(|r| r.source == Source::Plugins)
        .collect();
    if recorded.is_empty() {
        return Ok(events);
    }
    // the ledger holds subject and meta values redacted
    let redactor = ctx.ledger.redactor();
    let same = |held: &Option<String>, new: &Option<String>| {
        held.as_deref() == new.as_deref().map(|v| redactor.redact(v)).as_deref()
    };
    Ok(events
        .into_iter()
        .filter(|e| {
            let subject = redactor.redact(&e.subject);
            !recorded.iter().any(|r| {
                r.kind == e.kind
                    && r.subject == subject
                    && r.meta.enabled == e.meta.enabled
                    && same(&r.meta.version, &e.meta.version)
                    && same(&r.meta.from, &e.meta.from)
                    && same(&r.meta.to, &e.meta.to)
            })
        })
        .collect())
}

/// `omarchy plugin list --json`, or the message to degrade with (also
/// read by `seldon dossier`). `OMARCHY_PATH` defaults as
/// [`sys::omarchy_command`] says: `omarchy-shell` refuses to run without it.
pub fn list(omarchy: &str) -> Result<Vec<Listed>, String> {
    const WHAT: &str = "omarchy plugin list --json";
    let cmd = sys::omarchy_command(omarchy, &["plugin", "list", "--json"]);
    match sys::run_command(cmd, RUN_TIMEOUT) {
        Run::Exited {
            code: Some(0),
            stdout,
            ..
        } => serde_json::from_str(&stdout).map_err(|e| format!("{WHAT}: unexpected output ({e})")),
        Run::Exited { code, stderr, .. } => {
            let reason = stderr
                .lines()
                .map(str::trim)
                .find(|l| !l.is_empty())
                .map_or_else(
                    || {
                        format!(
                            "exit code {}",
                            code.map_or("none".into(), |c| c.to_string())
                        )
                    },
                    str::to_string,
                );
            Err(format!(
                "{WHAT}: {reason} (it needs the running Omarchy shell)"
            ))
        }
        Run::NotFound => Err(format!("{WHAT}: `{omarchy}` not found")),
        Run::TimedOut => Err(format!("{WHAT}: timed out")),
        Run::Failed(e) => Err(format!("{WHAT}: {e}")),
    }
}

/// id → manifest path from `omarchy plugin catalog` (it walks
/// `$OMARCHY_PATH/shell/plugins`); empty if it fails (the manifest
/// fallback and the remembered versions cover that).
fn catalog(omarchy: &str) -> BTreeMap<String, PathBuf> {
    let Run::Exited {
        code: Some(0),
        stdout,
        ..
    } = sys::run_command(
        sys::omarchy_command(omarchy, &["plugin", "catalog"]),
        RUN_TIMEOUT,
    )
    else {
        return BTreeMap::new();
    };
    serde_json::from_str::<Vec<Cataloged>>(&stdout)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|c| Some((c.id, c.manifest_path?)))
        .collect()
}

/// `version` of the manifest at `manifest`, trimmed, at most
/// [`VERSION_MAX`] characters.
fn manifest_version(manifest: &Path) -> Option<String> {
    let f = std::fs::File::open(manifest).ok()?;
    let mut text = String::new();
    f.take(MANIFEST_MAX).read_to_string(&mut text).ok()?;
    let v: Value = serde_json::from_str(&text).ok()?;
    let version = v.get("version")?.as_str()?.trim();
    (!version.is_empty()).then(|| version.chars().take(VERSION_MAX).collect())
}

/// Whether `dir` is a git clone of its own: only the plugin's own clone,
/// never a repository further up (a dotfiles repo in ~/.config would
/// answer for every plugin).
fn is_clone(dir: &Path) -> bool {
    dir.join(".git").exists()
}

/// The HEAD of a plugin's clone.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Head {
    /// The full hash.
    full: String,
    /// git's short form (the version of a plugin whose manifest has none).
    short: String,
}

/// The HEAD of the clone at `dir` ([`is_clone`]), else `None`; also `None`
/// when git cannot say (no commit yet, no git, a timeout).
fn head(git: Git, dir: &Path) -> Option<Head> {
    if !is_clone(dir) {
        return None;
    }
    let out = git.stdout(dir, &["rev-parse", "HEAD", "--short", "HEAD"])?;
    let mut lines = out.lines().map(str::trim);
    let (full, short) = (lines.next()?, lines.next()?);
    (is_hash(full) && matches!(full.len(), 40 | 64) && is_hash(short)).then(|| Head {
        full: full.to_string(),
        short: short.to_string(),
    })
}

/// A hexadecimal object name of 4 to 64 digits.
fn is_hash(s: &str) -> bool {
    (4..=64).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_hexdigit())
}

/// How the collector runs `git` on a plugin's clone: read-only queries
/// with a fixed argv and `-C <clone>`, [`GIT_OPTIONS`] and [`GIT_ENV`],
/// the variables that select another repository removed, the clone's
/// parent as ceiling (a broken `.git` never makes git walk up into a
/// repository around the plugins), in its own process group, bounded by
/// `timeout`. The program is a field so that tests can put a slow one in.
#[derive(Debug, Clone, Copy)]
struct Git<'a> {
    program: &'a str,
    timeout: Duration,
}

/// `git` from `PATH` with [`GIT_TIMEOUT`].
const GIT: Git<'static> = Git {
    program: "git",
    timeout: GIT_TIMEOUT,
};

impl Git<'_> {
    fn command(&self, dir: &Path, args: &[&str]) -> Command {
        let mut cmd = Command::new(self.program);
        cmd.args(GIT_OPTIONS).arg("-C").arg(dir).args(args);
        for var in REPOSITORY_VARS {
            cmd.env_remove(var);
        }
        cmd.envs(GIT_ENV);
        let dir = std::path::absolute(dir).unwrap_or_else(|_| dir.to_path_buf());
        if let Some(parent) = dir.parent() {
            cmd.env("GIT_CEILING_DIRECTORIES", parent);
        }
        cmd
    }

    /// The output of a query that exited 0.
    fn stdout(&self, dir: &Path, args: &[&str]) -> Option<String> {
        match sys::run_command(self.command(dir, args), self.timeout) {
            Run::Exited {
                code: Some(0),
                stdout,
                ..
            } => Some(stdout),
            _ => None,
        }
    }
}

/// How a clone's HEAD moved between two captures, and the commits.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Step {
    /// `pull`, `rollback` or `reset` (`meta.git`).
    how: &'static str,
    /// Commits that came in (`pull`, `reset`) or left (`rollback`).
    count: u64,
    /// `reset`: commits that left.
    left: u64,
    /// The subjects of the commits counted, newest first, at most
    /// [`COMMITS_MAX`], each by [`commit_subject`].
    subjects: Vec<String>,
}

impl Step {
    /// The detail's part: `pulled 3 commits: <newest> …`.
    fn summary(&self) -> String {
        let n = |k: u64| format!("{k} commit{}", if k == 1 { "" } else { "s" });
        let head = match self.how {
            "pull" => format!("pulled {}", n(self.count)),
            "rollback" => format!("rolled back {}", n(self.count)),
            _ => format!("reset: {} in, {} out", n(self.count), self.left),
        };
        match self.subjects.first() {
            Some(first) if self.count > 1 => format!("{head}: {first} …"),
            Some(first) => format!("{head}: {first}"),
            None => head,
        }
    }
}

/// The step of the clone at `dir` from HEAD `old` to `new`: two queries,
/// `rev-list --left-right --count` and `log` of the side that moved.
/// `None` when the heads are not object names, are equal, or git cannot
/// say (an object gone after a garbage collection, a shallow clone, a
/// timeout).
fn step(git: Git, redactor: &Redactor, dir: &Path, old: &str, new: &str) -> Option<Step> {
    // the cursor is a file: never let a value of it become an option
    if !is_hash(old) || !is_hash(new) || old == new {
        return None;
    }
    let both = format!("{old}...{new}");
    let counts = git.stdout(
        dir,
        &[
            "rev-list",
            "--left-right",
            "--count",
            "--end-of-options",
            &both,
            "--",
        ],
    )?;
    let (left, came) = counts.trim().split_once(char::is_whitespace)?;
    let (left, came): (u64, u64) = (left.trim().parse().ok()?, came.trim().parse().ok()?);
    let (how, range, count) = match (left, came) {
        (0, 0) => return None,
        (0, came) => ("pull", format!("{old}..{new}"), came),
        (left, 0) => ("rollback", format!("{new}..{old}"), left),
        (_, came) => ("reset", format!("{old}..{new}"), came),
    };
    let max = format!("--max-count={COMMITS_MAX}");
    let format = format!("--format=%<({COMMIT_SUBJECT_COLUMNS},trunc)%s");
    let log = git.stdout(
        dir,
        &[
            "log",
            "-z",
            "--no-show-signature",
            "--no-notes",
            "--no-mailmap",
            &max,
            &format,
            "--end-of-options",
            &range,
            "--",
        ],
    )?;
    let subjects = log
        .split_terminator('\0')
        .take(COMMITS_MAX)
        .map(|s| commit_subject(s, redactor))
        .collect();
    Some(Step {
        how,
        count,
        left: if how == "reset" { left } else { 0 },
        subjects,
    })
}

/// A commit subject as an event holds it: control characters become
/// spaces, direction and invisible format characters are dropped (the
/// set of ADR-0038), white space at the ends trimmed, then redacted
/// (before the clip: a secret at the cut is masked whole) and clipped to
/// [`COMMIT_SUBJECT_MAX`] characters with `…`. An empty one reads
/// `(no subject)`.
fn commit_subject(raw: &str, redactor: &Redactor) -> String {
    let clean: String = raw
        .chars()
        .filter(|c| !invisible(*c))
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    let clean = clean.trim();
    if clean.is_empty() {
        return "(no subject)".to_string();
    }
    let redacted = redactor.redact(clean);
    if redacted.chars().count() <= COMMIT_SUBJECT_MAX {
        return redacted;
    }
    let mut cut: String = redacted.chars().take(COMMIT_SUBJECT_MAX - 1).collect();
    cut.truncate(cut.trim_end().len());
    cut.push('…');
    cut
}

/// A character that turns the direction of the text around it or is an
/// invisible format character (ADR-0038 §2): a subject is shown in the
/// shell process, and a zero-width space inside a token would hide it
/// from its redaction rule.
fn invisible(c: char) -> bool {
    matches!(
        c,
        '\u{200B}'..='\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2060}' | '\u{2066}'..='\u{2069}' | '\u{FEFF}'
    )
}

/// The later mtime of `manifest` and its directory (a clone or a pull
/// touches at least one of them).
fn touched(manifest: &Path) -> Option<SystemTime> {
    let mtime = |p: &Path| std::fs::metadata(p).and_then(|m| m.modified()).ok();
    let dir = manifest.parent().and_then(mtime);
    mtime(manifest).max(dir)
}

/// Events for the step from `old` to `new`, ordered by plugin id. `seen`
/// holds what this run saw of the plugins in `new`; `since` is the last
/// check.
fn diff(
    ctx: &Ctx,
    old: &BTreeMap<String, PluginState>,
    new: &BTreeMap<String, PluginState>,
    seen: &BTreeMap<String, Seen>,
    since: DateTime<FixedOffset>,
) -> Vec<Event> {
    let unseen = Seen::default();
    let seen_of = |id: &str| seen.get(id).unwrap_or(&unseen);
    // add and update: when the plugin directory changed (ADR-0017 window)
    let changed = |id: &str| changed_at(ctx, seen_of(id).touched, since);
    let event = |kind, id: &str, meta: Meta| {
        let ts = match kind {
            Kind::PluginAdd => changed(id),
            _ => ctx.now,
        };
        let mut e = Event::new(ts, Source::Plugins, kind, id);
        e.detail = meta.version.clone();
        e.meta(meta)
    };
    let redactor = ctx.ledger.redactor();
    let mut ids: Vec<&String> = old.keys().chain(new.keys()).collect();
    ids.sort();
    ids.dedup();
    let mut events = Vec::new();
    for id in ids {
        match (old.get(id), new.get(id)) {
            (None, Some(n)) => {
                let mut e = event(
                    Kind::PluginAdd,
                    id,
                    Meta {
                        version: n.version.clone(),
                        enabled: Some(n.enabled),
                        ..Meta::default()
                    },
                );
                if seen_of(id).repo.is_some() && n.head.is_some() {
                    // `omarchy plugin add` clones
                    e.detail = Some(match &n.version {
                        Some(v) => format!("{v}, installed by git clone"),
                        None => "installed by git clone".to_string(),
                    });
                    e.meta.extra.insert("git".into(), "clone".into());
                }
                events.push(e);
            }
            (Some(o), None) => events.push(event(
                Kind::PluginRemove,
                id,
                Meta {
                    version: o.version.clone(),
                    ..Meta::default()
                },
            )),
            (Some(o), Some(n)) => {
                // first-party versions belong to the omarchy package (ADR-0018)
                if let (Some(from), Some(to)) = (&o.version, &n.version)
                    && from != to
                    && !seen_of(id).first_party
                {
                    let mut detail = format!("{from} → {to}");
                    let mut meta = Meta {
                        from: Some(from.clone()),
                        to: Some(to.clone()),
                        ..Meta::default()
                    };
                    // the commits, when the plugin's clone moved (WP-136)
                    let moved = match (&seen_of(id).repo, &o.head, &n.head) {
                        (Some(dir), Some(old), Some(new)) => step(GIT, redactor, dir, old, new),
                        _ => None,
                    };
                    if let Some(step) = moved {
                        detail = format!("{detail}, {}", step.summary());
                        meta.extra.insert("git".into(), step.how.into());
                        meta.extra
                            .insert("commits".into(), step.subjects.join("\n").into());
                    }
                    events.push(
                        Event::new(
                            changed(id),
                            Source::Plugins,
                            Kind::PluginUpdate,
                            id.as_str(),
                        )
                        .detail(detail)
                        .meta(meta),
                    );
                }
                if o.enabled != n.enabled {
                    let kind = if n.enabled {
                        Kind::PluginEnable
                    } else {
                        Kind::PluginDisable
                    };
                    events.push(event(
                        kind,
                        id,
                        Meta {
                            version: n.version.clone(),
                            enabled: Some(n.enabled),
                            ..Meta::default()
                        },
                    ));
                }
            }
            (None, None) => {}
        }
    }
    events
}

impl Collector for Plugins {
    fn name(&self) -> &'static str {
        "plugins"
    }

    fn cursor_reads(&self, cursor: &Value) -> bool {
        typed_cursor::<PluginsCursor>(Some(cursor)).is_some()
    }

    fn collect(&self, ctx: &Ctx, cursor: Option<&Value>) -> Outcome {
        self.collect_from(
            ctx,
            cursor,
            &ctx.sources.omarchy,
            &Plugins::dir(ctx.sources, &ctx.dirs.home),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsStr;
    use std::os::unix::fs::PermissionsExt as _;
    use std::time::Instant;

    /// A scratch directory, removed on drop.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(tag: &str) -> Self {
            let dir =
                std::env::temp_dir().join(format!("seldon-wp136-{tag}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(dir.join("plugins/p/.git")).unwrap();
            Scratch(dir)
        }

        fn clone_dir(&self) -> PathBuf {
            self.0.join("plugins/p")
        }

        /// An executable `name` running `body` with `sh`.
        fn program(&self, name: &str, body: &str) -> String {
            let path = self.0.join(name);
            std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
            path.to_string_lossy().into_owned()
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn the_git_command_is_fixed_and_read_only() {
        let dir = Path::new("/plugins/p");
        let cmd = GIT.command(dir, &["rev-parse", "HEAD"]);
        assert_eq!(cmd.get_program(), "git");
        let args: Vec<&OsStr> = cmd.get_args().collect();
        let mut want: Vec<&OsStr> = GIT_OPTIONS.iter().map(OsStr::new).collect();
        want.extend(["-C", "/plugins/p", "rev-parse", "HEAD"].map(OsStr::new));
        assert_eq!(args, want);
        let envs: BTreeMap<&OsStr, Option<&OsStr>> = cmd.get_envs().collect();
        for var in REPOSITORY_VARS {
            if var != "GIT_CEILING_DIRECTORIES" {
                assert_eq!(envs.get(OsStr::new(var)), Some(&None), "{var} removed");
            }
        }
        for (k, v) in GIT_ENV {
            assert_eq!(envs[OsStr::new(k)], Some(OsStr::new(v)));
        }
        assert_eq!(
            envs[OsStr::new("GIT_CEILING_DIRECTORIES")],
            Some(OsStr::new("/plugins"))
        );
    }

    #[test]
    fn a_slow_git_is_cut_off_and_names_nothing() {
        let s = Scratch::new("slow");
        let program = s.program("git", "exec sleep 5");
        let git = Git {
            program: &program,
            timeout: Duration::from_millis(200),
        };
        let started = Instant::now();
        assert_eq!(head(git, &s.clone_dir()), None);
        let (old, new) = ("a".repeat(40), "b".repeat(40));
        let redactor = Redactor::builtin();
        assert_eq!(step(git, &redactor, &s.clone_dir(), &old, &new), None);
        assert!(
            started.elapsed() < Duration::from_secs(3),
            "{:?}",
            started.elapsed()
        );
    }

    #[test]
    fn heads_that_are_not_object_names_never_reach_git() {
        let s = Scratch::new("option");
        let marker = s.0.join("ran");
        let program = s.program("git", &format!("touch '{}'", marker.display()));
        let git = Git {
            program: &program,
            timeout: GIT_TIMEOUT,
        };
        let redactor = Redactor::builtin();
        let ok = "a".repeat(40);
        for bad in ["--output=x", "HEAD", "", "abc", "a b", &"a".repeat(65)] {
            assert_eq!(
                step(git, &redactor, &s.clone_dir(), bad, &ok),
                None,
                "{bad:?}"
            );
            assert_eq!(
                step(git, &redactor, &s.clone_dir(), &ok, bad),
                None,
                "{bad:?}"
            );
        }
        assert_eq!(step(git, &redactor, &s.clone_dir(), &ok, &ok), None);
        assert!(!marker.exists(), "git ran");
        // without `<dir>/.git` there is no clone and no git call
        assert_eq!(head(git, &s.0), None);
        assert!(!marker.exists(), "git ran");
    }

    #[test]
    fn a_head_is_two_object_names() {
        let s = Scratch::new("head");
        let full = "0123456789abcdef0123456789abcdef01234567";
        for (out, want) in [
            (format!("{full}\n0123456\n"), true),
            ("HEAD\nHEAD\n".to_string(), false),
            (format!("{full}\n"), false),
            ("0123456\n0123456\n".to_string(), false),
        ] {
            let program = s.program("git", &format!("printf '{}'", out.replace('\n', "\\n")));
            let git = Git {
                program: &program,
                timeout: GIT_TIMEOUT,
            };
            let got = head(git, &s.clone_dir());
            assert_eq!(got.is_some(), want, "{out:?}");
            if want {
                let h = got.unwrap();
                assert_eq!((h.full.as_str(), h.short.as_str()), (full, "0123456"));
            }
        }
    }

    #[test]
    fn summaries_name_the_step() {
        let step = |how, count, left, subjects: &[&str]| Step {
            how,
            count,
            left,
            subjects: subjects.iter().map(|s| s.to_string()).collect(),
        };
        assert_eq!(
            step("pull", 3, 0, &["c", "b", "a"]).summary(),
            "pulled 3 commits: c …"
        );
        assert_eq!(step("pull", 1, 0, &["a"]).summary(), "pulled 1 commit: a");
        assert_eq!(
            step("rollback", 2, 0, &["b", "a"]).summary(),
            "rolled back 2 commits: b …"
        );
        assert_eq!(
            step("reset", 1, 4, &["x"]).summary(),
            "reset: 1 commit in, 4 out: x"
        );
        assert_eq!(step("pull", 2, 0, &[]).summary(), "pulled 2 commits");
    }

    #[test]
    fn a_subject_is_clean_and_short() {
        let r = Redactor::builtin();
        assert_eq!(commit_subject("   ", &r), "(no subject)");
        assert_eq!(commit_subject("\u{1b}\u{200B}\u{202E}", &r), "(no subject)");
        assert_eq!(commit_subject("  Fix it  ", &r), "Fix it");
        let long = "ä".repeat(150);
        let cut = commit_subject(&long, &r);
        assert_eq!(cut.chars().count(), COMMIT_SUBJECT_MAX);
        assert!(cut.ends_with("ä…"));
        // a cut after a space keeps no trailing space
        let spaced = format!("{} {}", "a".repeat(98), "b".repeat(10));
        assert_eq!(commit_subject(&spaced, &r), format!("{}…", "a".repeat(98)));
    }
}
