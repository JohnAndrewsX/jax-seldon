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
//! `git clone` (`meta.git: clone`); its `plugin-update` (a version or a
//! tree change) with a moved HEAD names the commits: `pull` (the old HEAD is an ancestor of the new one)
//! and `reset` (another history) list the subjects that came in,
//! `rollback` (the new HEAD is an ancestor) the ones that left. At most
//! [`COMMITS_MAX`] subjects, newest first, one per line in `meta.commits`,
//! each cleaned of control and direction characters, redacted and clipped
//! ([`commit_subject`]); the detail has the count and the newest subject.
//! The HEAD is read from the clone's files ([`head_from_files`]), else
//! from git; git runs read-only with a fixed argv ([`Git`]); when it fails
//! or times out the event is the same without the commits. Only a `.git`
//! whose repository stays inside the plugin folder is read
//! ([`GitDir`]); for any other the update says so ([`OUTSIDE`]) (WP-136
//! round 2).
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
//! fingerprint has them is not read again; a plugin seen without a tree
//! hash (a cursor from before WP-113, a new plugin) takes it without an
//! event. WP-113 round 2: an entry that cannot be read counts by its
//! size, time and mode, a file over 64 MiB by its size, time and inode,
//! and a tree past [`TREE_ENTRIES`] is cut off; such a tree is `partial`
//! (cursor and event meta). Only an unreadable plugin directory keeps the
//! last hash.
//!
//! The cursor is the snapshot `{id: {enabled, version, tree, head}}`, its SHA-256,
//! the trees' fingerprints and the time of the last check. Without a cursor the collector takes a baseline
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

use super::config::{
    FileStat, HASH_BASIS_KEY, STAT_HASH_ABOVE, SkipPaths, changed_at, persistent_hash,
};
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

/// Most bytes kept of each output pipe of a git query; the rest is read
/// and dropped ([`sys::run_command_capped`]), and a cut stdout is no
/// answer. A query's real output is a few KiB.
const GIT_OUTPUT_MAX: usize = 64 * 1024;

/// Options before every git query: no pager; no lazy fetch of a missing
/// object in a partial clone (git 2.44 or later; an older git refuses the
/// option and the query names nothing); no replace refs; no hooks; no file
/// system monitor; `protocol.allow=never` as the default for protocols the
/// clone's config does not name (a repository-local `protocol.<name>.allow`
/// beats it: [`GIT_ENV`]'s `GIT_ALLOW_PROTOCOL` is what refuses them all);
/// no signature check (gpg); no colour; the log in UTF-8 whatever the
/// clone's `i18n.logOutputEncoding` says (no NUL of UTF-16 in `-z`
/// output).
const GIT_OPTIONS: [&str; 15] = [
    "--no-pager",
    "--no-lazy-fetch",
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
    "-c",
    "i18n.logOutputEncoding=UTF-8",
];

/// Environment of every git query, set after [`REPOSITORY_VARS`] are
/// removed, besides `GIT_CEILING_DIRECTORIES`: no system and no global
/// config (the answer depends on the clone alone); no grafts (the clone's
/// `info/grafts` would fake parents, and each bad line of it is a line of
/// stderr); no protocol at all, overriding any configuration (`none` is no
/// protocol's name); no prompt; no lock taken for an index refresh; no lazy
/// fetch (as `--no-lazy-fetch`).
const GIT_ENV: [(&str, &str); 7] = [
    ("GIT_CONFIG_NOSYSTEM", "1"),
    ("GIT_CONFIG_GLOBAL", "/dev/null"),
    ("GIT_GRAFT_FILE", "/dev/null"),
    ("GIT_ALLOW_PROTOCOL", "none"),
    ("GIT_TERMINAL_PROMPT", "0"),
    ("GIT_OPTIONAL_LOCKS", "0"),
    ("GIT_NO_LAZY_FETCH", "1"),
];

/// The detail's part for a plugin whose `.git` could lead git out of the
/// plugin's folder ([`GitDir::Outside`]).
pub const OUTSIDE: &str =
    "commit history not read (the repository points outside the plugin folder)";

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

/// SHA-256 of an unreadable entry's size, modification time (ns), change
/// time and mode: an in-place write with the modification time put back
/// changes it (WP-113 round 3), and so does a `chmod` round trip.
fn unreadable_hash(meta: &std::fs::Metadata) -> String {
    use std::os::unix::fs::MetadataExt as _;
    let mtime = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_nanos());
    sys::sha256_hex(
        format!(
            "{} {} {} {} {:o}\n",
            meta.len(),
            mtime,
            meta.ctime(),
            meta.ctime_nsec(),
            meta.mode()
        )
        .as_bytes(),
    )
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
#[derive(Debug, Clone, Default)]
struct Seen {
    first_party: bool,
    /// The tree has a file hashed by its metadata ([`Tree::stat_hashed`]).
    stat_hashed: bool,
    /// Later mtime of the plugin directory and its manifest, and of the
    /// files of its tree.
    touched: Option<SystemTime>,
    /// The plugin's directory when it is a third-party plugin's own git
    /// clone ([`GitDir::Contained`]).
    repo: Option<PathBuf>,
    /// A third-party plugin's `.git` that git is not asked about
    /// ([`GitDir::Outside`]).
    outside: bool,
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
        // at most one `rev-parse` per clone and capture
        let mut heads: BTreeMap<PathBuf, Option<Head>> = BTreeMap::new();
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
                let version =
                    manifest_version(m).or_else(|| Some(asked(&mut heads, m.parent()?)?.short))?;
                Some((version, *m))
            });
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
            // a third-party plugin's own clone (WP-136): the manifest's
            // directory, else the plugin directory
            let clone_dir = manifest
                .and_then(|m| m.parent())
                .map_or_else(|| plugins_dir.join(&p.id), Path::to_path_buf);
            let git_dir = git_dir(&clone_dir);
            let outside = !p.first_party && git_dir == GitDir::Outside;
            let repo = (!p.first_party && git_dir == GitDir::Contained).then_some(clone_dir);
            let head = repo.as_deref().and_then(|dir| {
                // from the clone's files (no process), else from git
                // (asked already when the version came from it); unreadable
                // this time: keep the last one seen
                let known = heads.get(dir).cloned().flatten().map(|h| h.full);
                known
                    .or_else(|| head_from_files(dir))
                    .or_else(|| asked(&mut heads, dir).map(|h| h.full))
                    .or_else(|| last?.head.clone())
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
                    repo,
                    outside,
                },
            );
            snapshot.insert(
                p.id,
                PluginState {
                    enabled: p.enabled,
                    version,
                    tree,
                    partial,
                    head,
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

/// [`head`] of `dir`, asked once per capture.
fn asked(heads: &mut BTreeMap<PathBuf, Option<Head>>, dir: &Path) -> Option<Head> {
    heads
        .entry(dir.to_path_buf())
        .or_insert_with(|| head(GIT, dir))
        .clone()
}

/// Whether `dir` is a git clone of its own: only the plugin's own clone,
/// never a repository further up (a dotfiles repo in ~/.config would
/// answer for every plugin), and only one git reads inside the folder.
fn is_clone(dir: &Path) -> bool {
    git_dir(dir) == GitDir::Contained
}

/// What the `.git` of a plugin directory is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GitDir {
    /// None: not a clone.
    None,
    /// A real directory whose repository stays inside it: read.
    Contained,
    /// One that can make git read another repository: a link, a
    /// `gitdir:` file (a linked work tree), `objects/info/alternates`,
    /// `commondir`, or an `include`/`includeIf` section in its config
    /// (`config`, `config.worktree`) or a config that cannot be read.
    /// Its HEAD and commits would be another repository's; not read.
    Outside,
}

/// Largest config of a clone scanned for `include` sections; a larger one
/// counts as [`GitDir::Outside`].
const GIT_CONFIG_MAX: u64 = 1024 * 1024;

/// [`GitDir`] of `dir`.
fn git_dir(dir: &Path) -> GitDir {
    let git = dir.join(".git");
    let meta = match std::fs::symlink_metadata(&git) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return GitDir::None,
        Err(_) => return GitDir::Outside,
    };
    if !meta.is_dir() {
        return GitDir::Outside;
    }
    // a link where git keeps objects, refs or the HEAD leads elsewhere
    let linked = |rel: &str| {
        std::fs::symlink_metadata(git.join(rel)).is_ok_and(|m| m.file_type().is_symlink())
    };
    if ["objects", "refs", "packed-refs", "HEAD"]
        .into_iter()
        .any(linked)
    {
        return GitDir::Outside;
    }
    let exists = |rel: &str| std::fs::symlink_metadata(git.join(rel)).is_ok();
    if exists("objects/info/alternates") || exists("commondir") {
        return GitDir::Outside;
    }
    for name in ["config", "config.worktree"] {
        match sys::read_small_file(&git.join(name), GIT_CONFIG_MAX) {
            Ok(None) => {}
            Ok(Some(text)) if !includes(&text) => {}
            _ => return GitDir::Outside,
        }
    }
    GitDir::Contained
}

/// Whether a git config text may have an `include` or `includeIf`
/// section: `[include` anywhere in it, case-insensitive (section names
/// are). A scan by line start would miss what git's parser reads as a
/// section header (WP-136 round 3): after a byte order mark (git skips
/// EF BB BF; `trim_start` does not), after a lone CR (git takes it for
/// white space; `lines` does not end a line there), after another header
/// on the same line (`[core] [include]`), or on the line after a value
/// continued with a backslash. The whole-text scan needs no case of its
/// own for any of them; `[include` in a comment or a value refuses a
/// clone it need not (no commit list), never the other way round.
fn includes(config: &str) -> bool {
    config.to_ascii_lowercase().contains("[include")
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

/// Largest `packed-refs` read by [`head_from_files`]; a larger one is
/// left to git.
const PACKED_REFS_MAX: u64 = 1024 * 1024;

/// The full HEAD of the clone at `dir` read from its files, without a
/// process (a capture runs on every agent command; one git process costs
/// about 10 ms there): `.git/HEAD` holds the object name, or `ref:
/// refs/…` whose object name is in `.git/<ref>` or `.git/packed-refs`.
/// `None` for anything else — a `.git` file (a linked work tree), a
/// symbolic link (`sys::read_small_file` refuses them), the reftable
/// format, an unborn branch, a ref name that is not plain — and git
/// answers instead.
fn head_from_files(dir: &Path) -> Option<String> {
    let git_dir = dir.join(".git");
    let read = |rel: &str, max: u64| sys::read_small_file(&git_dir.join(rel), max).ok()?;
    let head = read("HEAD", 4096)?;
    let head = head.trim_end_matches('\n');
    let full = |s: &str| (is_hash(s) && matches!(s.len(), 40 | 64)).then(|| s.to_string());
    let Some(name) = head.strip_prefix("ref: ") else {
        return full(head); // a detached HEAD
    };
    let plain = name.starts_with("refs/heads/")
        && name.split('/').all(|part| {
            !part.is_empty()
                && !part.starts_with('.')
                && part
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || "-_.+@".contains(c))
        });
    if !plain || name.contains("..") || name.ends_with(".lock") {
        return None;
    }
    if let Some(loose) = read(name, 4096) {
        return full(loose.trim_end_matches('\n'));
    }
    read("packed-refs", PACKED_REFS_MAX)?
        .lines()
        .filter(|l| !l.starts_with('#') && !l.starts_with('^'))
        .find_map(|l| match l.split_once(' ') {
            Some((hash, n)) if n == name => full(hash),
            _ => None,
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
/// repository around the plugins), in its own process group (killed
/// whole at the deadline), bounded by `timeout` and in memory by
/// [`GIT_OUTPUT_MAX`]. The program is a field so that tests can put a
/// slow one in.
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

    /// The output of a query that exited 0 with stdout under
    /// [`GIT_OUTPUT_MAX`] (invalid UTF-8 read lossily).
    fn stdout(&self, dir: &Path, args: &[&str]) -> Option<String> {
        let (run, cut) =
            sys::run_command_capped(self.command(dir, args), self.timeout, GIT_OUTPUT_MAX);
        match run {
            Run::Exited {
                code: Some(0),
                stdout,
                ..
            } if !cut => Some(stdout),
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

/// A commit subject as an event holds it: control characters and the line and paragraph separators become
/// spaces, direction and invisible format characters are dropped (the
/// set of ADR-0038), white space at the ends trimmed, then redacted
/// (before the clip: a secret at the cut is masked whole) and clipped to
/// [`COMMIT_SUBJECT_MAX`] characters with `…`. An empty one reads
/// `(no subject)`.
fn commit_subject(raw: &str, redactor: &Redactor) -> String {
    let clean: String = raw
        .chars()
        .filter(|c| !invisible(*c))
        .map(|c| if breaks(c) { ' ' } else { c })
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

/// A character that breaks a line or controls a terminal: a control
/// character, or the line and paragraph separators U+2028 and U+2029,
/// which the desk's plain text breaks on (one subject would look like
/// two).
fn breaks(c: char) -> bool {
    c.is_control() || matches!(c, '\u{2028}' | '\u{2029}')
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
                let third_party = !seen_of(id).first_party;
                let moved = |a: &Option<String>, b: &Option<String>| match (a, b) {
                    (Some(a), Some(b)) if a != b => Some((a.clone(), b.clone())),
                    _ => None,
                };
                let version = moved(&o.version, &n.version);
                // WP-113: the tree, seen in both snapshots
                let tree = moved(&o.tree, &n.tree);
                if third_party && (version.is_some() || tree.is_some()) {
                    let mut detail = match (&version, &tree) {
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
                    // WP-136: why the commits are missing, or the commits,
                    // when the plugin's clone moved (a pull without a
                    // version bump fires through the tree)
                    if seen_of(id).outside {
                        detail = format!("{detail}, {OUTSIDE}");
                    }
                    let step = match (&seen_of(id).repo, &o.head, &n.head) {
                        (Some(dir), Some(old), Some(new)) => step(GIT, redactor, dir, old, new),
                        _ => None,
                    };
                    if let Some(step) = step {
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
        let want = [
            "--no-pager",
            "--no-lazy-fetch",
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
            "-c",
            "i18n.logOutputEncoding=UTF-8",
            "-C",
            "/plugins/p",
            "rev-parse",
            "HEAD",
        ]
        .map(OsStr::new);
        assert_eq!(args, want);
        let envs: BTreeMap<&OsStr, Option<&OsStr>> = cmd.get_envs().collect();
        for var in REPOSITORY_VARS {
            if !["GIT_CEILING_DIRECTORIES", "GIT_GRAFT_FILE"].contains(&var) {
                assert_eq!(envs.get(OsStr::new(var)), Some(&None), "{var} removed");
            }
        }
        for (k, v) in [
            ("GIT_CONFIG_NOSYSTEM", "1"),
            ("GIT_CONFIG_GLOBAL", "/dev/null"),
            ("GIT_GRAFT_FILE", "/dev/null"),
            ("GIT_ALLOW_PROTOCOL", "none"),
            ("GIT_TERMINAL_PROMPT", "0"),
            ("GIT_OPTIONAL_LOCKS", "0"),
            ("GIT_NO_LAZY_FETCH", "1"),
        ] {
            assert_eq!(envs[OsStr::new(k)], Some(OsStr::new(v)), "{k}");
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
    fn the_timeout_is_two_seconds_and_the_program_git() {
        assert_eq!(GIT.timeout, Duration::from_secs(2));
        assert_eq!(GIT.program, "git");
    }

    #[test]
    fn a_slow_git_is_killed_with_what_it_started() {
        // a child of the fake git holds the output pipes; the whole
        // process group goes at the deadline
        let s = Scratch::new("group");
        let pid_file = s.0.join("pid");
        let program = s.program(
            "git",
            &format!("sleep 3 & echo $! > '{}'; wait", pid_file.display()),
        );
        let git = Git {
            program: &program,
            timeout: Duration::from_millis(300),
        };
        let (old, new) = ("a".repeat(40), "b".repeat(40));
        assert_eq!(
            step(git, &Redactor::builtin(), &s.clone_dir(), &old, &new),
            None
        );
        let pid = std::fs::read_to_string(&pid_file).unwrap();
        let alive = || {
            std::fs::read_to_string(format!("/proc/{}/stat", pid.trim()))
                .is_ok_and(|stat| !stat.contains(") Z "))
        };
        let until = Instant::now() + Duration::from_secs(1);
        while alive() && Instant::now() < until {
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(!alive(), "the fake git's child outlived the deadline");
    }

    #[test]
    fn a_flooding_git_is_no_answer_and_costs_no_memory() {
        let s = Scratch::new("flood");
        let program = s.program(
            "git",
            "head -c 33554432 /dev/zero >&2; head -c 1048576 /dev/zero | tr '\\0' 'x'",
        );
        let git = Git {
            program: &program,
            timeout: Duration::from_secs(10),
        };
        let started = Instant::now();
        assert_eq!(git.stdout(&s.clone_dir(), &["log"]), None, "a cut stdout");
        assert!(started.elapsed() < Duration::from_secs(10));
        // the same bytes under the cap are an answer
        let program = s.program("git", "head -c 1000 /dev/zero | tr '\\0' 'x'");
        let git = Git {
            program: &program,
            timeout: GIT_TIMEOUT,
        };
        assert_eq!(git.stdout(&s.clone_dir(), &["log"]), Some("x".repeat(1000)));
    }

    #[test]
    fn bytes_that_are_not_utf8_are_read_lossily() {
        let s = Scratch::new("lossy");
        let program = s.program(
            "git",
            "case \"$*\" in *rev-list*) printf '0\\t1\\n';; *) printf 'Caf\\351 au lait\\0';; esac",
        );
        let git = Git {
            program: &program,
            timeout: GIT_TIMEOUT,
        };
        let (old, new) = ("a".repeat(40), "b".repeat(40));
        let step = step(git, &Redactor::builtin(), &s.clone_dir(), &old, &new).unwrap();
        assert_eq!(step.subjects, ["Caf\u{FFFD} au lait"]);
        assert_eq!(step.how, "pull");
    }

    #[test]
    fn a_git_dir_that_points_outside_is_not_read() {
        let s = Scratch::new("outside");
        let dir = s.clone_dir();
        let git = dir.join(".git");
        assert_eq!(git_dir(&dir), GitDir::Contained);
        assert_eq!(git_dir(&s.0), GitDir::None);
        let write = |rel: &str, text: &str| {
            let path = git.join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, text).unwrap();
            path
        };
        let config = "[core]\n\tbare = false\n[remote \"origin\"]\n\turl = x\n";
        write("config", config);
        assert_eq!(git_dir(&dir), GitDir::Contained);
        for (rel, text) in [
            ("objects/info/alternates", "/elsewhere/objects\n"),
            ("commondir", "../elsewhere\n"),
            ("config", "[include]\n\tpath = /elsewhere/config\n"),
            (
                "config",
                "[core]\n  [includeIf \"gitdir:/x/\"]\n\tpath = y\n",
            ),
            ("config", "[Include]\n\tpath = y\n"),
            // round 3: what a scan by line would miss
            ("config", "\u{FEFF}[include]\n\tpath = y\n"),
            ("config", "[core]\r[include]\n\tpath = y\n"),
            ("config", "[core]\r\n[include]\r\n\tpath = y\r\n"),
            ("config", "[core] [include]\n\tpath = y\n"),
            (
                "config",
                "[core]\n\tbare = false \\\n[include]\n\tpath = y\n",
            ),
            ("config.worktree", "[include]\n\tpath = y\n"),
        ] {
            let path = write(rel, text);
            assert_eq!(git_dir(&dir), GitDir::Outside, "{rel}: {text:?}");
            if rel == "config" {
                write("config", config);
            } else {
                std::fs::remove_file(path).unwrap();
            }
            assert_eq!(git_dir(&dir), GitDir::Contained, "{rel} undone");
        }
        // a link where git keeps objects, refs or the HEAD (round 3)
        for rel in ["objects", "refs", "packed-refs", "HEAD"] {
            let path = git.join(rel);
            let aside = s.0.join(format!("aside-{rel}"));
            let moved = path.exists() && std::fs::rename(&path, &aside).is_ok();
            if !moved {
                std::fs::write(&aside, "").unwrap();
            }
            std::os::unix::fs::symlink(&aside, &path).unwrap();
            assert_eq!(git_dir(&dir), GitDir::Outside, "a linked {rel}");
            std::fs::remove_file(&path).unwrap();
            if moved {
                std::fs::rename(&aside, &path).unwrap();
            } else {
                std::fs::remove_file(&aside).unwrap();
            }
            assert_eq!(git_dir(&dir), GitDir::Contained, "{rel} undone");
        }
        // a config that is not UTF-8, or a link; a `.git` that is a link or
        // a file
        std::fs::write(git.join("config"), [0xff, 0xfe, b'\n']).unwrap();
        assert_eq!(git_dir(&dir), GitDir::Outside);
        std::fs::remove_file(git.join("config")).unwrap();
        std::os::unix::fs::symlink("/dev/null", git.join("config")).unwrap();
        assert_eq!(git_dir(&dir), GitDir::Outside);
        std::fs::remove_file(git.join("config")).unwrap();
        let elsewhere = s.0.join("elsewhere.git");
        std::fs::rename(&git, &elsewhere).unwrap();
        std::os::unix::fs::symlink(&elsewhere, &git).unwrap();
        assert_eq!(git_dir(&dir), GitDir::Outside, "a linked .git");
        std::fs::remove_file(&git).unwrap();
        std::fs::write(&git, format!("gitdir: {}\n", elsewhere.display())).unwrap();
        assert_eq!(git_dir(&dir), GitDir::Outside, "a gitdir: file");
        // and git is never asked about it
        let marker = s.0.join("ran");
        let program = s.program("git", &format!("touch '{}'", marker.display()));
        let fake = Git {
            program: &program,
            timeout: GIT_TIMEOUT,
        };
        assert_eq!(head(fake, &dir), None);
        assert!(!marker.exists());
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
    fn a_head_is_read_from_the_clones_files() {
        let s = Scratch::new("files");
        let dir = s.clone_dir();
        let git = |rel: &str, text: &str| {
            let path = dir.join(".git").join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        };
        let (a, b) = ("a".repeat(40), "b".repeat(64));
        // detached
        git("HEAD", &format!("{a}\n"));
        assert_eq!(head_from_files(&dir), Some(a.clone()));
        // a branch: loose, then packed (a sha256 clone)
        git("HEAD", "ref: refs/heads/main\n");
        assert_eq!(head_from_files(&dir), None, "unborn");
        git(
            "packed-refs",
            &format!(
                "# pack-refs with: peeled fully-peeled sorted\n{a} refs/heads/mainline\n{b} refs/heads/main\n^{a}\n"
            ),
        );
        assert_eq!(head_from_files(&dir), Some(b.clone()));
        git("refs/heads/main", &format!("{a}\n"));
        assert_eq!(head_from_files(&dir), Some(a.clone()), "a loose ref wins");
        // anything else is git's to answer, even where a file answers
        for rel in [
            "refs/heads/.invalid",
            "outside",
            "refs/heads/main.lock",
            "refs/tags/main",
        ] {
            git(rel, &format!("{a}\n"));
        }
        for head in [
            "ref: refs/heads/../../outside\n",
            "ref: refs/heads/.invalid\n", // reftable
            "ref: refs/heads/../../../etc\n",
            "ref: refs/tags/main\n",
            "ref: refs/heads/a b\n",
            "ref: refs/heads/main.lock\n",
            "ref: refs/heads/a..b\n",
            "0123\n",
            "",
        ] {
            git("HEAD", head);
            assert_eq!(head_from_files(&dir), None, "{head:?}");
        }
        git("HEAD", "ref: refs/heads/main\n");
        assert_eq!(head_from_files(&dir), Some(a.clone()));
        git("refs/heads/main", "not a hash\n");
        assert_eq!(head_from_files(&dir), None);
        // a `.git` file (a linked work tree) or a linked HEAD
        std::fs::remove_dir_all(dir.join(".git")).unwrap();
        std::fs::write(dir.join(".git"), "gitdir: /elsewhere\n").unwrap();
        assert_eq!(head_from_files(&dir), None);
        std::fs::remove_file(dir.join(".git")).unwrap();
        std::fs::create_dir_all(dir.join(".git")).unwrap();
        std::fs::write(s.0.join("head"), format!("{a}\n")).unwrap();
        std::os::unix::fs::symlink(s.0.join("head"), dir.join(".git/HEAD")).unwrap();
        assert_eq!(head_from_files(&dir), None);
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
        // isolates dropped; line and paragraph separators are spaces
        assert_eq!(
            commit_subject("a\u{2066}b\u{2067}c\u{2068}d\u{2069}e", &r),
            "abcde"
        );
        assert_eq!(
            commit_subject("one\u{2028}two\u{2029}three", &r),
            "one two three"
        );
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
