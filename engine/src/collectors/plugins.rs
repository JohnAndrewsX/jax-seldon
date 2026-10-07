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
//! `plugin-add` and `plugin-update` are timed by the later mtime of the
//! plugin directory and its manifest, clamped to `[last check, now]`, so
//! attribution (ADR-0017) can match the agent command that cloned or
//! pulled it. Removal, enabling and disabling get the capture time.
//!
//! Third-party plugin trees (WP-113, ADR-0028 §8 WP-E; operator decision
//! 2026-10-06: hashes only): the directory `<plugins dir>/<id>/` of every
//! listed plugin with `firstParty: false` is hashed as one tree
//! ([`tree`]): every regular file at any size and content, `.git` left
//! out (the HEAD is the version), `[redaction] skipPaths` honoured, links
//! to files followed; any other link counts by its target as written (not
//! walked: Omarchy refuses links inside a plugin folder). The plugin
//! directory itself may be a link (`omarchy plugin` links a checkout). A tree-hash change is one `plugin-update` (with
//! `meta.hashFrom`/`hashTo`; with the version change when both moved), so
//! an in-place edit of a plugin's QML is seen. A tree whose files' size,
//! modification and change times and inodes are as the cursor's
//! fingerprint has them is not read again; a tree that cannot be read
//! this time keeps the last hash, and a plugin seen without one (a cursor
//! from before WP-113, a new plugin) takes it without an event.
//!
//! The cursor is the snapshot `{id: {enabled, version, tree}}`, its SHA-256,
//! the trees' fingerprints and the time of the last check. Without a cursor the collector takes a baseline
//! (no events). Events the ledger already holds since the last check (same
//! kind, id, version, enabled state, update step) are dropped: a capture
//! whose cursor save failed after its ledger write left the old snapshot in
//! the cursor, and the next diff would repeat them.

use std::collections::BTreeMap;
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::config::{
    FileStat, HASH_BASIS_KEY, STAT_HASH_ABOVE, SkipPaths, changed_at, persistent_hash,
};
use super::{Collector, Ctx, Lost, Outcome, RUN_TIMEOUT, Sources, to_cursor, typed_cursor};
use crate::model::event::{Event, Kind, Meta, SUBJECT_MAX, Source};
use crate::sys::{self, Run};

/// Omarchy's user plugin directory relative to `$HOME` (the CLI hard-codes
/// `$HOME/.config`, not XDG).
pub const PLUGINS_DIR: &str = ".config/omarchy/plugins";

/// Longest version string recorded (a manifest is user content).
const VERSION_MAX: usize = 64;

/// Largest manifest read.
const MANIFEST_MAX: u64 = 1024 * 1024;

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
    /// SHA-256 of the plugin's tree (third-party plugins, WP-113).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    tree: Option<String>,
    /// The tree counts some entries by their metadata only ([`Tree`]:
    /// unreadable, or cut off).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    partial: bool,
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
    /// id → fingerprint of the tree's files ([`Tree::stat`]); not part of
    /// `hash`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    stats: BTreeMap<String, String>,
}

impl PluginsCursor {
    fn new(
        plugins: BTreeMap<String, PluginState>,
        stats: BTreeMap<String, String>,
        checked: DateTime<FixedOffset>,
    ) -> Self {
        let body = serde_json::to_vec(&plugins).expect("a snapshot always serialises");
        PluginsCursor {
            hash: sys::sha256_hex(&body),
            plugins,
            checked: Some(checked),
            stats,
        }
    }
}

/// A plugin's tree as one capture saw it.
#[derive(Debug)]
struct Tree {
    /// SHA-256 over the sorted lines `<relative path> NUL <entry> LF`: the
    /// file's SHA-256, `stat <hash>` for a file over
    /// [`STAT_HASH_ABOVE`], `link <sha256 of the target>` for a link that
    /// is not to a file, `unreadable <hash>` for an entry that cannot be
    /// read, and a last line `NUL cut` when the tree has more than
    /// [`TREE_ENTRIES`] entries.
    hash: String,
    /// SHA-256 over every entry's relative path and [`FileStat`]; `None`
    /// when a file changed too recently to trust it (the next capture
    /// reads the tree again).
    stat: Option<String>,
    /// The latest modification time of a file in it.
    newest: Option<SystemTime>,
    /// Entries that could not be read.
    unreadable: usize,
    /// Cut off at [`TREE_ENTRIES`].
    cut: bool,
    /// A file hashed by [`stat_hash`].
    stat_hashed: bool,
}

impl Tree {
    /// Some entries count by their metadata only: unreadable, or past
    /// the cut.
    fn partial(&self) -> bool {
        self.unreadable > 0 || self.cut
    }
}

/// Directory depth below a plugin's directory; deeper trees are not walked.
const TREE_DEPTH: usize = 32;

/// Entries one plugin tree walk reads; the rest is cut off (WP-113 round
/// 2): the tree is hashed from the first ones in walk order and marked
/// partial.
pub const TREE_ENTRIES: usize = 10_000;

/// One entry of a plugin's tree.
enum Entry {
    /// A regular file, or a link to one (counted with its target's content).
    File(PathBuf, std::fs::Metadata),
    /// Any other link: SHA-256 of its target as written. Omarchy refuses
    /// links inside a plugin folder (`omarchy-plugin-validate`), so a link
    /// to a directory is not walked; its arrival changes the tree.
    Link(String),
    /// A file that cannot be opened or a directory that cannot be read:
    /// counted by its size, modification time and mode, so it changes the
    /// tree once and visibly and the rest is still seen.
    Unreadable(std::fs::Metadata),
}

/// SHA-256 of an unreadable entry's size, modification time (ns) and mode.
fn unreadable_hash(meta: &std::fs::Metadata) -> String {
    use std::os::unix::fs::MetadataExt as _;
    let mtime = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_nanos());
    sys::sha256_hex(format!("{} {} {:o}\n", meta.len(), mtime, meta.mode()).as_bytes())
}

/// The tree of the plugin directory `dir` ([`Tree`]); `known` is the last
/// fingerprint and hash, reused while the fingerprint holds. `None` when
/// the plugin directory itself cannot be read (the caller keeps the last
/// hash).
fn tree(
    dir: &Path,
    skip: &SkipPaths,
    known: Option<(&str, &str)>,
    started: SystemTime,
) -> Option<Tree> {
    let mut entries = Vec::new();
    let mut left = TREE_ENTRIES;
    std::fs::read_dir(dir).ok()?;
    let cut = !walk_tree(dir, Path::new(""), skip, 0, &mut left, &mut entries);
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    let newest = entries
        .iter()
        .filter_map(|(_, e)| match e {
            Entry::File(_, meta) => meta.modified().ok(),
            _ => None,
        })
        .max();
    let unreadable = entries
        .iter()
        .filter(|(_, e)| matches!(e, Entry::Unreadable(_)))
        .count();
    let stat_hashed = entries
        .iter()
        .any(|(_, e)| matches!(e, Entry::File(_, m) if m.len() > STAT_HASH_ABOVE));
    let stat = entries
        .iter()
        .map(|(rel, e)| {
            Some(match e {
                Entry::File(_, meta) | Entry::Unreadable(meta) => {
                    (rel.as_str(), Some(FileStat::of(meta, started)?), None)
                }
                Entry::Link(hash) => (rel.as_str(), None, Some(hash.as_str())),
            })
        })
        .collect::<Option<Vec<_>>>()
        .map(|stats| sys::sha256_hex(&serde_json::to_vec(&(stats, cut)).expect("stats serialise")));
    let reuse = match (&stat, known) {
        (Some(stat), Some((known_stat, known_hash))) if stat == known_stat => {
            Some(known_hash.to_string())
        }
        _ => None,
    };
    let hash = reuse.unwrap_or_else(|| {
        let mut lines = sys::Sha256::new();
        for (rel, e) in &entries {
            let hash = match e {
                Entry::File(path, meta) => match persistent_hash(path, meta) {
                    Some(h) if meta.len() > STAT_HASH_ABOVE => format!("stat {h}"),
                    Some(h) => h,
                    // opened a moment ago, unreadable now
                    None => format!("unreadable {}", unreadable_hash(meta)),
                },
                Entry::Link(hash) => format!("link {hash}"),
                Entry::Unreadable(meta) => format!("unreadable {}", unreadable_hash(meta)),
            };
            lines.update(rel.as_bytes());
            lines.update(b"\0");
            lines.update(hash.as_bytes());
            lines.update(b"\n");
        }
        if cut {
            lines.update(b"\0cut\n");
        }
        lines.finish_hex()
    });
    Some(Tree {
        hash,
        stat,
        newest,
        unreadable,
        cut,
        stat_hashed,
    })
}

/// Collects the entries below `dir` (relative path `rel`), in sorted
/// order; `false` when the budget `left` ran out (the tree is cut off).
fn walk_tree(
    dir: &Path,
    rel: &Path,
    skip: &SkipPaths,
    depth: usize,
    left: &mut usize,
    entries: &mut Vec<(String, Entry)>,
) -> bool {
    let Ok(read) = std::fs::read_dir(dir) else {
        return true;
    };
    let mut paths: Vec<PathBuf> = read.filter_map(|e| e.ok().map(|e| e.path())).collect();
    paths.sort();
    for path in paths {
        let Some(name) = path.file_name().map(|n| n.to_os_string()) else {
            continue;
        };
        if name == ".git" || skip.matches(&path) {
            continue;
        }
        if *left == 0 {
            return false;
        }
        *left -= 1;
        let rel = rel.join(&name);
        let Ok(meta) = std::fs::symlink_metadata(&path) else {
            continue;
        };
        let key = || rel.to_string_lossy().into_owned();
        if meta.file_type().is_symlink() {
            match std::fs::metadata(&path) {
                Ok(target) if target.is_file() => entries.push(file_entry(key(), path, target)),
                _ => {
                    let target = std::fs::read_link(&path).unwrap_or_default();
                    let hash = sys::sha256_hex(target.as_os_str().as_encoded_bytes());
                    entries.push((key(), Entry::Link(hash)));
                }
            }
        } else if meta.is_file() {
            entries.push(file_entry(key(), path, meta));
        } else if meta.is_dir() && depth < TREE_DEPTH {
            if std::fs::read_dir(&path).is_err() {
                entries.push((format!("{}/", key()), Entry::Unreadable(meta)));
            } else if !walk_tree(&path, &rel, skip, depth + 1, left, entries) {
                return false;
            }
        }
    }
    true
}

/// A file of a tree: [`Entry::File`], or [`Entry::Unreadable`] when it
/// cannot be opened.
fn file_entry(key: String, path: PathBuf, meta: std::fs::Metadata) -> (String, Entry) {
    if std::fs::File::open(&path).is_ok() {
        (key, Entry::File(path, meta))
    } else {
        (key, Entry::Unreadable(meta))
    }
}

/// What this run saw of a plugin beyond its cursor state.
#[derive(Debug, Clone, Copy, Default)]
struct Seen {
    first_party: bool,
    /// The tree has a file hashed by its metadata ([`Tree::stat_hashed`]).
    stat_hashed: bool,
    /// Later mtime of the plugin directory and its manifest, and of the
    /// files of its tree.
    touched: Option<SystemTime>,
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
        let skip = SkipPaths::new(&ctx.dirs.home, &ctx.config.redaction.skip_paths);
        let started = SystemTime::now();
        let mut snapshot = BTreeMap::new();
        let mut stats = BTreeMap::new();
        let mut seen = BTreeMap::new();
        let (mut unreadable, mut cut) = (0, 0);
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
            let found = candidates
                .iter()
                .find_map(|m| version(ctx, m).map(|v| (v, *m)));
            // the manifest that answered, else the first one that exists
            let manifest = found
                .as_ref()
                .map(|(_, m)| *m)
                .or_else(|| candidates.iter().copied().find(|m| m.exists()));
            let last = prev.as_ref().and_then(|c| c.plugins.get(&p.id));
            let version = found.map(|(v, _)| v).or_else(|| {
                // unreadable this time: keep the last one seen
                last?.version.clone()
            });
            // third-party trees only: first-party plugins ship with Omarchy
            let dir = plugins_dir.join(&p.id);
            let tree = (!p.first_party && dir.is_dir())
                .then(|| {
                    let known = prev
                        .as_ref()
                        .and_then(|c| Some((c.stats.get(&p.id)?.as_str(), last?.tree.as_deref()?)));
                    tree(&dir, &skip, known, started)
                })
                .flatten();
            let touched = manifest
                .and_then(|m| touched(m))
                .max(tree.as_ref().and_then(|t| t.newest));
            if let Some(stat) = tree.as_ref().and_then(|t| t.stat.clone()) {
                stats.insert(p.id.clone(), stat);
            }
            if let Some(t) = &tree {
                unreadable += t.unreadable;
                cut += usize::from(t.cut);
            }
            let stat_hashed = tree.as_ref().is_some_and(|t| t.stat_hashed);
            let (tree, partial) = match tree {
                Some(t) => (Some(t.hash.clone()), t.partial()),
                // the plugin directory cannot be read: keep the last one
                // (a tree that is gone is a removed plugin, which the list
                // tells)
                None if !p.first_party => (
                    last.and_then(|l| l.tree.clone()),
                    last.is_some_and(|l| l.partial),
                ),
                None => (None, false),
            };
            seen.insert(
                p.id.clone(),
                Seen {
                    first_party: p.first_party,
                    stat_hashed,
                    touched,
                },
            );
            snapshot.insert(
                p.id,
                PluginState {
                    enabled: p.enabled,
                    version,
                    tree,
                    partial,
                },
            );
        }
        let next = PluginsCursor::new(snapshot, stats, ctx.now);

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
        let mut notes = Vec::new();
        if unreadable > 0 {
            notes.push(format!(
                "{unreadable} entr(ies) of plugin trees could not be read; counted by size, time and mode"
            ));
        }
        if cut > 0 {
            notes.push(format!(
                "{cut} plugin tree(s) cut off at {TREE_ENTRIES} entries"
            ));
        }
        Outcome {
            since,
            message: (!notes.is_empty()).then(|| notes.join("; ")),
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
                    && r.meta.hash_from == e.meta.hash_from
                    && r.meta.hash_to == e.meta.hash_to
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

/// The plugin's version: `version` of the manifest at `manifest`, else the
/// short git HEAD of the manifest's directory when that is a git clone.
fn version(ctx: &Ctx, manifest: &Path) -> Option<String> {
    let from_manifest = std::fs::File::open(manifest).ok().and_then(|f| {
        let mut text = String::new();
        f.take(MANIFEST_MAX).read_to_string(&mut text).ok()?;
        let v: Value = serde_json::from_str(&text).ok()?;
        let version = v.get("version")?.as_str()?.trim();
        (!version.is_empty()).then(|| version.chars().take(VERSION_MAX).collect())
    });
    from_manifest.or_else(|| {
        // only the plugin's own clone; never a repository further up (a
        // dotfiles repo in ~/.config would answer for every plugin)
        let dir = manifest.parent()?;
        if !dir.join(".git").exists() {
            return None;
        }
        let dir = dir.to_str()?;
        match ctx.run("git", &["-C", dir, "rev-parse", "--short", "HEAD"]) {
            Run::Exited {
                code: Some(0),
                stdout,
                ..
            } => {
                let head = stdout.trim();
                (!head.is_empty() && head.chars().all(|c| c.is_ascii_hexdigit()))
                    .then(|| head.to_string())
            }
            _ => None,
        }
    })
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
    let seen_of = |id: &str| seen.get(id).copied().unwrap_or_default();
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
    let mut ids: Vec<&String> = old.keys().chain(new.keys()).collect();
    ids.sort();
    ids.dedup();
    let mut events = Vec::new();
    for id in ids {
        match (old.get(id), new.get(id)) {
            (None, Some(n)) => events.push(event(
                Kind::PluginAdd,
                id,
                Meta {
                    version: n.version.clone(),
                    enabled: Some(n.enabled),
                    ..Meta::default()
                },
            )),
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
                let third_party = !seen_of(id).first_party;
                let moved = |a: &Option<String>, b: &Option<String>| match (a, b) {
                    (Some(a), Some(b)) if a != b => Some((a.clone(), b.clone())),
                    _ => None,
                };
                let version = moved(&o.version, &n.version);
                // WP-113: the tree, seen in both snapshots
                let tree = moved(&o.tree, &n.tree);
                if third_party && (version.is_some() || tree.is_some()) {
                    let detail = match (&version, &tree) {
                        (Some((from, to)), _) => format!("{from} → {to}"),
                        (None, Some((from, to))) => {
                            let short = |h: &str| h.chars().take(8).collect::<String>();
                            format!("files changed (sha256 {} → {})", short(from), short(to))
                        }
                        (None, None) => unreachable!("one of them moved"),
                    };
                    let (from, to) = version.unzip();
                    let (hash_from, hash_to) = tree.unzip();
                    let mut meta = Meta {
                        from,
                        to,
                        hash_from,
                        hash_to,
                        ..Meta::default()
                    };
                    // WP-113 round 2: how far the tree hash rests on
                    // metadata
                    if n.partial {
                        meta.extra.insert("partial".into(), Value::Bool(true));
                    }
                    if seen_of(id).stat_hashed {
                        meta.extra
                            .insert(HASH_BASIS_KEY.into(), Value::String("stat".into()));
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
