//! `config` collector (SPEC-ENGINE §4): sha256 manifest of `watchPaths` →
//! `config-add|config-change|config-remove`.
//!
//! Every regular file under the watch paths (`config.toml watchPaths`, `~`
//! expanded; missing paths are skipped) is hashed with SHA-256 and recorded
//! under its `~`-path (SPEC-LOGBOOK §7: no absolute private paths). Not
//! hashed, never read beyond the first bytes:
//!
//! - Omarchy's plugin directory `~/.config/omarchy/plugins/` (the `plugins`
//!   collector covers it), the desktop entries' MIME cache
//!   [`MIME_CACHE`] (`update-desktop-database` rewrites it on many package
//!   transactions; the `.desktop` files it is built from are watched) and
//!   every `.git` directory;
//! - files and directories matching `config.toml [redaction] skipPaths`
//!   ([`SkipPaths`]); they do not appear in the manifest at all;
//! - binary files (a NUL byte in the first 8000 bytes, as git decides) and
//!   files larger than 1 MB: listed as `skipped` without a hash, so a file
//!   that grows past the limit is not reported as removed;
//! - sockets, FIFOs and devices; symlinks to directories (no loops).
//!   Symlinks to files are followed (stow-style dotfiles);
//! - files whose `~`-path cannot be an event subject (control characters,
//!   longer than [`SUBJECT_MAX`]): counted in the collector's message.
//!
//! The manifest lives in `$XDG_STATE_HOME/seldon/manifest.json`
//! ([`Manifest`]); the cursor holds the hash of the generation the ledger has
//! caught up with. `capture` saves cursors only after the ledger write, so
//! the file keeps the generation before the newest one too: when a write
//! fails, the next run diffs against the generation its cursor names and
//! nothing is lost. When the cursor save fails after the ledger write, the
//! events the ledger holds since the cursor's check are applied to that
//! generation first ([`replay`]), so the diff neither repeats them nor
//! misses a file that went back to its old content. Files whose names
//! differ only in a part the redaction masks share a subject in the
//! ledger; the replay tells them apart by their hashes. The first run (no
//! cursor) is a baseline: no events.
//!
//! Each generation keeps the scope it was taken with ([`WatchScope`]). A file
//! that left the scope (a watch path removed, a `skipPaths` pattern added) is
//! no removal, and a file that entered it is no addition: one notice line
//! counts them. A file whose size, modification time, change time and inode
//! are as the manifest has them ([`FileStat`]) keeps its stored hash and is
//! not read.
//!
//! The engine's own writes (SPEC-ENGINE §5 rule 7): when `init`, `hook
//! install` or one of the removal commands (`hook uninstall`, `init
//! --remove-theme-hook`) writes or deletes a file this collector would
//! hash, it records the file's hash, the command and what it did
//! ([`OwnOp`]) in `$XDG_STATE_HOME/seldon/owned.json` ([`OwnWrites`],
//! [`record_own_writes`], [`delete_own_file`]). The next capture that runs
//! this collector explains the matching event
//! (`crate::reconcile::explain_own_writes`).

use std::collections::{BTreeMap, BTreeSet};
use std::io::Read as _;
use std::os::unix::fs::MetadataExt as _;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use chrono::{DateTime, FixedOffset, Local, Timelike as _, Utc};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::plugins::Plugins;
use super::{Collector, Ctx, Lost, Outcome, Tz, to_cursor, typed_cursor};
use crate::config::{Config, Dirs};
use crate::logbook::lock::Lock;
use crate::model::event::{Event, Kind, Meta, SUBJECT_MAX, Source};
use crate::sys;

/// File name of the manifest in the state directory (SPEC-ENGINE §2).
pub const MANIFEST_FILE: &str = "manifest.json";

/// Files larger than this are not hashed (SPEC-ENGINE §4: "> 1 MB").
pub const MAX_FILE_SIZE: u64 = 1024 * 1024;

/// The desktop entries' MIME cache relative to `$HOME`, excluded like the
/// plugin directory: generated from the `.desktop` files next to it.
pub const MIME_CACHE: &str = ".local/share/applications/mimeinfo.cache";

/// Bytes probed for a NUL to call a file binary (git's heuristic).
const BINARY_PROBE: usize = 8000;

/// Directory depth below a watch path; deeper trees are not walked.
const MAX_DEPTH: usize = 32;

pub struct ConfigFiles;

/// One state of the watched files.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Generation {
    /// SHA-256 over `files` and `skipped` (the cursor names a generation by it).
    pub hash: String,
    /// `~`-path → SHA-256 of every hashed file.
    pub files: BTreeMap<String, String>,
    /// Files seen but not hashed: binary, larger than 1 MB, or unreadable.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub skipped: BTreeSet<String>,
    /// The scope the files were collected in; absent in a manifest written
    /// before WP-069. Not part of `hash`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<WatchScope>,
}

impl Generation {
    pub fn new(files: BTreeMap<String, String>, skipped: BTreeSet<String>) -> Self {
        let body = serde_json::to_vec(&(&files, &skipped)).expect("strings always serialise");
        Generation {
            hash: sys::sha256_hex(&body),
            files,
            skipped,
            scope: None,
        }
    }
}

/// The watch scope of a generation: the watch paths and the excluded
/// folders as `~`-paths, and the `[redaction] skipPaths` patterns as
/// configured, each sorted.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WatchScope {
    pub watch: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub exclude: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skip: Vec<String>,
}

impl WatchScope {
    pub fn new(dirs: &Dirs, roots: &[PathBuf], excluded: &[PathBuf], skip: &[String]) -> Self {
        let shown = |paths: &[PathBuf]| -> Vec<String> {
            let set: BTreeSet<String> = paths.iter().map(|p| dirs.display(p)).collect();
            set.into_iter().collect()
        };
        let skip: BTreeSet<String> = skip
            .iter()
            .map(|p| p.trim())
            .filter(|p| !p.is_empty())
            .map(String::from)
            .collect();
        WatchScope {
            watch: shown(roots),
            exclude: shown(excluded),
            skip: skip.into_iter().collect(),
        }
    }

    fn rules(&self, dirs: &Dirs) -> ScopeRules {
        let paths = |keys: &[String]| keys.iter().map(|k| key_path(dirs, k)).collect();
        ScopeRules {
            roots: paths(&self.watch),
            excluded: paths(&self.exclude),
            skip: SkipPaths::new(&dirs.home, &self.skip),
        }
    }
}

/// A [`WatchScope`] ready to test paths against.
struct ScopeRules {
    roots: Vec<PathBuf>,
    excluded: Vec<PathBuf>,
    skip: SkipPaths,
}

impl ScopeRules {
    /// Whether a walk in this scope reaches the file `key`: it lies under a
    /// watch path, and neither it nor a folder between it and the watch
    /// path is excluded or matches `skipPaths` (as [`Walker::ignored`]).
    fn covers(&self, dirs: &Dirs, key: &str) -> bool {
        let path = key_path(dirs, key);
        self.roots.iter().any(|root| {
            path.starts_with(root)
                && path
                    .ancestors()
                    .take_while(|a| a.starts_with(root))
                    .all(|a| {
                        !self.excluded.iter().any(|x| a.starts_with(x)) && !self.skip.matches(a)
                    })
        })
    }
}

/// A `~`-path of the manifest as an absolute path.
fn key_path(dirs: &Dirs, key: &str) -> PathBuf {
    match key.strip_prefix('~') {
        Some("") => dirs.home.clone(),
        Some(rest) if rest.starts_with('/') => dirs.home.join(&rest[1..]),
        _ => PathBuf::from(key),
    }
}

/// Size, modification time and change time (ns since 1970) and inode of a
/// hashed file. While all four are as the manifest has them, the stored
/// hash is reused and the file is not read. The change time catches an
/// edit whose modification time was put back (`touch -r`): no one can set
/// it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileStat(u64, u64, u64, u64);

/// A file modified less than this before a walk started is read again by
/// the next walk: a write right after the read may have kept the
/// modification time (timestamps are coarse; some file systems keep
/// seconds or two). A write also sets the change time, so the
/// modification time is the one to check.
const RACY: Duration = Duration::from_secs(2);

impl FileStat {
    /// `None` for a file modified less than [`RACY`] before `started` (or
    /// either time before 1970): its hash is not reused.
    fn of(meta: &std::fs::Metadata, started: SystemTime) -> Option<Self> {
        let mtime = meta.modified().ok()?;
        let ctime = UNIX_EPOCH.checked_add(Duration::new(
            u64::try_from(meta.ctime()).ok()?,
            u32::try_from(meta.ctime_nsec()).ok()?,
        ))?;
        if mtime.checked_add(RACY)? > started {
            return None;
        }
        let ns = |t: SystemTime| u64::try_from(t.duration_since(UNIX_EPOCH).ok()?.as_nanos()).ok();
        Some(FileStat(meta.len(), ns(mtime)?, ns(ctime)?, meta.ino()))
    }
}

/// `manifest.json`: the current generation, plus the one the ledger last
/// caught up with when that is older (a capture whose write failed).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    #[serde(flatten)]
    pub current: Generation,
    /// [`FileStat`] of the current generation's files, by `~`-path.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub stats: BTreeMap<String, FileStat>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous: Option<Generation>,
}

impl Manifest {
    /// `manifest.json` in the state directory.
    pub fn file(dirs: &Dirs) -> PathBuf {
        dirs.state_dir.join(MANIFEST_FILE)
    }

    /// Reads `path`: `Ok(None)` if it is missing or unreadable as a manifest
    /// (both mean: take a new baseline), `Err` on an I/O error.
    pub fn load(path: &Path) -> std::io::Result<Option<Manifest>> {
        match std::fs::read(path) {
            Ok(bytes) => Ok(serde_json::from_slice(&bytes).ok()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub fn save(&self, path: &Path) -> anyhow::Result<()> {
        let mut text = serde_json::to_string_pretty(self)?;
        text.push('\n');
        sys::write_atomic(path, text.as_bytes())
    }

    /// The stored generation called `hash`, current or previous.
    pub fn generation(&self, hash: &str) -> Option<&Generation> {
        std::iter::once(&self.current)
            .chain(&self.previous)
            .find(|g| g.hash == hash)
    }
}

/// File name of the engine's own writes in the state directory
/// (SPEC-ENGINE §2, §5 rule 7).
pub const OWNED_FILE: &str = "owned.json";

/// What the engine did to a file under a watched path.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OwnOp {
    /// Wrote it, adding what it installs (`installed by …`).
    #[default]
    Install,
    /// Wrote it, taking out what it had installed; the file stays
    /// (`removed by …`).
    Remove,
    /// Deleted it (`removed by …`); matched against the `config-remove`.
    Delete,
}

impl OwnOp {
    fn is_install(&self) -> bool {
        *self == OwnOp::Install
    }

    /// The verb of the resolution's detail: `<verb> by <command>`.
    pub fn verb(self) -> &'static str {
        match self {
            OwnOp::Install => "installed",
            OwnOp::Remove | OwnOp::Delete => "removed",
        }
    }
}

/// One file the engine wrote or deleted under a watched path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OwnWrite {
    /// SHA-256 of the content the engine wrote; for [`OwnOp::Delete`], of
    /// the content the file had when the engine deleted it.
    pub hash: String,
    /// The command that wrote it (`seldon init --theme-hook`).
    pub by: String,
    /// What it did; absent in the file for [`OwnOp::Install`].
    #[serde(default, skip_serializing_if = "OwnOp::is_install")]
    pub op: OwnOp,
}

/// `owned.json`: `~`-path → the engine's own write that the next config
/// event for that path may still report.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OwnWrites(pub BTreeMap<String, OwnWrite>);

impl OwnWrites {
    /// `owned.json` in the state directory.
    pub fn file(dirs: &Dirs) -> PathBuf {
        dirs.state_dir.join(OWNED_FILE)
    }

    /// Reads `path`: empty when it is missing or not a valid file (a lost
    /// record only means one more drift item), `Err` on an I/O error.
    pub fn load(path: &Path) -> std::io::Result<OwnWrites> {
        match std::fs::read(path) {
            Ok(bytes) => Ok(serde_json::from_slice(&bytes).unwrap_or_default()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(OwnWrites::default()),
            Err(e) => Err(e),
        }
    }

    /// Writes `path`, or removes it when nothing is recorded.
    pub fn save(&self, path: &Path) -> anyhow::Result<()> {
        if self.0.is_empty() {
            return match std::fs::remove_file(path) {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
                    Err(anyhow::Error::new(e).context(format!("cannot remove {}", path.display())))
                }
                _ => Ok(()),
            };
        }
        let mut text = serde_json::to_string_pretty(self)?;
        text.push('\n');
        sys::write_atomic(path, text.as_bytes())
    }

    /// The write that explains a config event: same `~`-path, and the
    /// event's new hash is the one the engine wrote (`config-add`,
    /// `config-change`), or its old hash the content the engine deleted
    /// (`config-remove`).
    pub fn explaining(&self, e: &Event) -> Option<&OwnWrite> {
        let w = self.0.get(&e.subject)?;
        let hash = match (w.op, &e.kind) {
            (OwnOp::Install | OwnOp::Remove, Kind::ConfigAdd | Kind::ConfigChange) => {
                e.meta.hash_to.as_deref()?
            }
            (OwnOp::Delete, Kind::ConfigRemove) => e.meta.hash_from.as_deref()?,
            _ => return None,
        };
        (w.hash == hash).then_some(w)
    }
}

/// Whether the config collector hashes `path`: it lies under a watch path
/// and matches no `[redaction] skipPaths` pattern.
pub fn is_watched(dirs: &Dirs, config: &Config, path: &Path) -> bool {
    config
        .watch_paths
        .iter()
        .filter_map(|w| dirs.expand_config(w))
        .any(|w| path.starts_with(w))
        && !SkipPaths::new(&dirs.home, &config.redaction.skip_paths).matches(path)
}

/// The hash the config collector records for `path` now, or `None` when
/// it does not hash the file (not watched, skipped, binary, too large,
/// not a regular file).
fn own_hash(dirs: &Dirs, config: &Config, path: &Path) -> Option<String> {
    if !is_watched(dirs, config, path) {
        return None;
    }
    std::fs::metadata(path)
        .ok()
        .filter(|m| m.is_file())
        .and_then(|m| hash_file(path, m.len()))
}

/// Adds `(path, hash)` records by `by` to `owned.json`; returns their
/// `~`-paths.
fn record_own(
    dirs: &Dirs,
    hashed: Vec<(&Path, String)>,
    by: &str,
    op: OwnOp,
) -> anyhow::Result<Vec<String>> {
    if hashed.is_empty() {
        return Ok(Vec::new());
    }
    let file = OwnWrites::file(dirs);
    let mut own = OwnWrites::load(&file)
        .map_err(|e| anyhow::anyhow!("cannot read {}: {e}", dirs.display(&file)))?;
    let mut recorded = Vec::new();
    for (path, hash) in hashed {
        let key = dirs.display(path);
        let by = by.to_string();
        own.0.insert(key.clone(), OwnWrite { hash, by, op });
        recorded.push(key);
    }
    own.save(&file)?;
    Ok(recorded)
}

/// Records each of `paths` that the config collector hashes as the
/// engine's own write by `by` ([`OwnOp::Install`] or [`OwnOp::Remove`]),
/// with the hash of its content now (SPEC-ENGINE §5 rule 7). Returns the
/// `~`-paths recorded; the lock proves that no capture reads the file
/// meanwhile.
pub fn record_own_writes(
    _lock: &Lock,
    dirs: &Dirs,
    config: &Config,
    paths: &[PathBuf],
    by: &str,
    op: OwnOp,
) -> anyhow::Result<Vec<String>> {
    let hashed = paths
        .iter()
        .filter_map(|p| Some((p.as_path(), own_hash(dirs, config, p)?)))
        .collect();
    record_own(dirs, hashed, by, op)
}

/// Records the deletion of the file `path` by `by` as the engine's own
/// ([`OwnOp::Delete`], with the hash of the content it has) when the
/// config collector hashes it, then deletes it. The record comes first
/// (SPEC-ENGINE §5 rule 7): a record whose deletion then fails is
/// harmless, it only explains a `config-remove` of exactly that content.
/// The outer `Err` is the failed deletion; the inner one a deletion that
/// could not be recorded (the next capture then shows it as drift).
/// `Ok(Ok(paths))` names the `~`-paths recorded.
pub fn delete_own_file(
    _lock: &Lock,
    dirs: &Dirs,
    config: &Config,
    path: &Path,
    by: &str,
) -> anyhow::Result<anyhow::Result<Vec<String>>> {
    let hashed = own_hash(dirs, config, path)
        .map(|h| vec![(path, h)])
        .unwrap_or_default();
    let recorded = record_own(dirs, hashed, by, OwnOp::Delete);
    std::fs::remove_file(path)
        .map_err(|e| anyhow::anyhow!("cannot remove {}: {e}", dirs.display(path)))?;
    Ok(recorded)
}

/// The config collector's cursor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConfigCursor {
    /// The generation the ledger has caught up with.
    hash: String,
    /// Capture time of the last check.
    checked: DateTime<FixedOffset>,
}

/// `config.toml [redaction] skipPaths`: files that are never hashed and
/// never named in the manifest (ADR-0014 §4 uses the same list for hooks).
///
/// A pattern with a `/` matches the whole path: `~/` is the home directory,
/// a pattern that does not start with `/` (after that) may match at any
/// directory boundary. A pattern without a `/` matches a file or directory
/// name. `*` and `?` match within one path component, `**` across them. A
/// match on a directory covers everything below it.
///
/// A pattern is compiled on its first use, and only for a path (or name)
/// that starts with the pattern's literal head, which every path it
/// matches does; a pattern that may match at any directory boundary has
/// no head. A hook whose command names no path below a skipped folder
/// compiles nothing (WP-092; the defaults cost about 0.45 ms to compile).
#[derive(Debug, Clone, Default)]
pub struct SkipPaths {
    paths: Vec<LazyGlob>,
    names: Vec<LazyGlob>,
}

/// An anchored glob regex, compiled when [`LazyGlob::is_match`] first gets
/// past the literal head.
#[derive(Debug, Clone)]
struct LazyGlob {
    /// What every match starts with.
    head: String,
    source: String,
    regex: OnceLock<Regex>,
}

impl LazyGlob {
    /// `pattern` as [`glob`] reads it; only a `^` prefix gives it a head.
    fn new(pattern: &str, prefix: &str, suffix: &str) -> Self {
        let head = match prefix {
            "^" => &pattern[..pattern.find(['*', '?']).unwrap_or(pattern.len())],
            _ => "",
        };
        LazyGlob {
            head: head.to_string(),
            source: glob(pattern, prefix, suffix),
            regex: OnceLock::new(),
        }
    }

    fn is_match(&self, text: &str) -> bool {
        text.starts_with(&self.head)
            && self
                .regex
                .get_or_init(|| Regex::new(&self.source).expect("an escaped glob is a valid regex"))
                .is_match(text)
    }
}

impl SkipPaths {
    pub fn new(home: &Path, patterns: &[String]) -> Self {
        let mut skip = SkipPaths::default();
        let home = home.to_string_lossy();
        for p in patterns.iter().map(|p| p.trim()).filter(|p| !p.is_empty()) {
            let p = p.trim_end_matches('/');
            if !p.contains('/') && p != "~" {
                skip.names.push(LazyGlob::new(p, "^", "$"));
                continue;
            }
            let expanded = match p.strip_prefix('~') {
                Some(rest) if rest.is_empty() || rest.starts_with('/') => format!("{home}{rest}"),
                _ => p.to_string(),
            };
            let prefix = if expanded.starts_with('/') {
                "^"
            } else {
                "(?:^|/)"
            };
            skip.paths
                .push(LazyGlob::new(&expanded, prefix, "(?:/.*)?$"));
        }
        skip
    }

    /// Whether `path` (absolute) or its last component matches.
    pub fn matches(&self, path: &Path) -> bool {
        let full = path.to_string_lossy();
        if self.paths.iter().any(|r| r.is_match(&full)) {
            return true;
        }
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy())
            .unwrap_or_default();
        self.names.iter().any(|r| r.is_match(&name))
    }
}

/// A glob as the source of an anchored regex.
fn glob(pattern: &str, prefix: &str, suffix: &str) -> String {
    let mut re = String::from(prefix);
    let mut chars = pattern.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '*' if chars.peek() == Some(&'*') => {
                chars.next();
                re.push_str(".*");
            }
            '*' => re.push_str("[^/]*"),
            '?' => re.push_str("[^/]"),
            c => re.push_str(&regex::escape(&c.to_string())),
        }
    }
    re.push_str(suffix);
    re
}

/// What one walk of the watch paths found.
#[derive(Debug, Default)]
struct Scan {
    files: BTreeMap<String, String>,
    skipped: BTreeSet<String>,
    mtimes: BTreeMap<String, SystemTime>,
    stats: BTreeMap<String, FileStat>,
    /// Files left out because their `~`-path cannot be a subject.
    unnamed: usize,
}

/// The rules of one walk.
struct Walker<'a> {
    dirs: &'a Dirs,
    excluded: &'a [PathBuf],
    skip: &'a SkipPaths,
    /// The stored manifest: hashes to reuse by [`FileStat`].
    known: Option<&'a Manifest>,
    /// When the walk started (the system clock, for [`FileStat::of`]).
    started: SystemTime,
}

impl Walker<'_> {
    fn scan(&self, roots: &[PathBuf]) -> Scan {
        let mut scan = Scan::default();
        for root in roots {
            if self.ignored(root) {
                continue;
            }
            // a watch path may itself be a symlink (dotfile managers)
            match std::fs::metadata(root) {
                Ok(m) if m.is_dir() => self.walk(root, 0, &mut scan),
                Ok(m) if m.is_file() => self.file(root, &m, &mut scan),
                _ => {}
            }
        }
        scan
    }

    fn ignored(&self, path: &Path) -> bool {
        self.excluded.iter().any(|x| path.starts_with(x)) || self.skip.matches(path)
    }

    fn walk(&self, dir: &Path, depth: usize, scan: &mut Scan) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        let mut paths: Vec<PathBuf> = entries.filter_map(|e| e.ok().map(|e| e.path())).collect();
        paths.sort();
        for path in paths {
            if path.file_name().is_some_and(|n| n == ".git") || self.ignored(&path) {
                continue;
            }
            let Ok(meta) = std::fs::symlink_metadata(&path) else {
                continue;
            };
            if meta.file_type().is_symlink() {
                // follow links to files, never into directories
                if let Ok(target) = std::fs::metadata(&path)
                    && target.is_file()
                {
                    self.file(&path, &target, scan);
                }
            } else if meta.is_dir() {
                if depth < MAX_DEPTH {
                    self.walk(&path, depth + 1, scan);
                }
            } else if meta.is_file() {
                self.file(&path, &meta, scan);
            }
        }
    }

    fn file(&self, path: &Path, meta: &std::fs::Metadata, scan: &mut Scan) {
        let key = self.dirs.display(path);
        // cannot be an event subject; a control character could also
        // rewrite the terminal that shows it
        if key.chars().count() > SUBJECT_MAX || key.chars().any(char::is_control) {
            scan.unnamed += 1;
            return;
        }
        if let Ok(t) = meta.modified() {
            scan.mtimes.insert(key.clone(), t);
        }
        let stat = FileStat::of(meta, self.started);
        let known = stat.and_then(|s| {
            let m = self.known?;
            (m.stats.get(&key) == Some(&s))
                .then(|| m.current.files.get(&key).cloned())
                .flatten()
        });
        match known.or_else(|| hash_file(path, meta.len())) {
            Some(hash) => {
                if let Some(s) = stat {
                    scan.stats.insert(key.clone(), s);
                }
                scan.files.insert(key, hash);
            }
            None => {
                scan.skipped.insert(key);
            }
        }
    }
}

/// SHA-256 of a text file of at most [`MAX_FILE_SIZE`] bytes; `None` for a
/// larger, binary or unreadable file.
fn hash_file(path: &Path, len: u64) -> Option<String> {
    if len > MAX_FILE_SIZE {
        return None;
    }
    let file = std::fs::File::open(path).ok()?;
    let mut bytes = Vec::new();
    // the file may grow between stat and read
    file.take(MAX_FILE_SIZE + 1).read_to_end(&mut bytes).ok()?;
    if bytes.len() as u64 > MAX_FILE_SIZE || bytes.iter().take(BINARY_PROBE).any(|&b| b == 0) {
        return None;
    }
    Some(sys::sha256_hex(&bytes))
}

/// The time a change happened: `mtime` in the capture's zone, whole seconds,
/// clamped to `[since, now]` (a copied mtime from the past must not land
/// an event in an old month; one from the future is now).
pub(crate) fn changed_at(
    ctx: &Ctx,
    mtime: Option<SystemTime>,
    since: DateTime<FixedOffset>,
) -> DateTime<FixedOffset> {
    let at = mtime.map_or(ctx.now, |t| {
        let at = match ctx.tz {
            Tz::Local => DateTime::<Local>::from(t).fixed_offset(),
            Tz::Fixed(off) => DateTime::<Utc>::from(t).with_timezone(&off),
        };
        at.with_nanosecond(0).unwrap_or(at)
    });
    at.min(ctx.now).max(since.min(ctx.now))
}

/// `sha256 <first 8 hex> → <first 8 hex>`, `—` for a missing side.
fn hash_detail(from: Option<&str>, to: Option<&str>) -> String {
    let short = |h: Option<&str>| h.map_or("—".to_string(), |h| h.chars().take(8).collect());
    format!("sha256 {} → {}", short(from), short(to))
}

/// Events for the step from `base` to `scan`.
fn diff(ctx: &Ctx, base: &Generation, scan: &Scan, since: DateTime<FixedOffset>) -> Vec<Event> {
    let mut events = Vec::new();
    let event = |kind, path: &str, from: Option<&String>, to: Option<&String>| {
        let ts = match kind {
            Kind::ConfigRemove => ctx.now,
            _ => changed_at(ctx, scan.mtimes.get(path).copied(), since),
        };
        Event::new(ts, Source::Config, kind, path)
            .detail(hash_detail(
                from.map(String::as_str),
                to.map(String::as_str),
            ))
            .meta(Meta {
                hash_from: from.cloned(),
                hash_to: to.cloned(),
                ..Meta::default()
            })
    };
    for (path, hash) in &scan.files {
        match base.files.get(path) {
            Some(old) if old != hash => {
                events.push(event(Kind::ConfigChange, path, Some(old), Some(hash)));
            }
            Some(_) => {}
            // a file that was too large or binary is not new
            None if base.skipped.contains(path) => {}
            None => events.push(event(Kind::ConfigAdd, path, None, Some(hash))),
        }
    }
    for (path, old) in &base.files {
        if !scan.files.contains_key(path) && !scan.skipped.contains(path) {
            events.push(event(Kind::ConfigRemove, path, Some(old), None));
        }
    }
    events.sort_by(|a, b| a.subject.cmp(&b.subject));
    events
}

/// Fits `base`, taken in its own scope, to `scope`: drops the files the
/// scope no longer reaches, and adds the files of `scan` it did not reach
/// before (when its scope is known) as they are now. A scope change is no
/// change of files. Returns how many files left and entered the scope.
fn rescope(dirs: &Dirs, base: &mut Generation, scope: &WatchScope, scan: &Scan) -> (usize, usize) {
    if base.scope.as_ref() == Some(scope) {
        return (0, 0);
    }
    let now = scope.rules(dirs);
    let before = base.files.len() + base.skipped.len();
    base.files.retain(|k, _| now.covers(dirs, k));
    base.skipped.retain(|k| now.covers(dirs, k));
    let left = before - base.files.len() - base.skipped.len();
    let Some(old) = base.scope.as_ref().map(|s| s.rules(dirs)) else {
        return (left, 0);
    };
    let known = |k: &String| base.files.contains_key(k) || base.skipped.contains(k);
    let entered_files: Vec<(String, String)> = scan
        .files
        .iter()
        .filter(|(k, _)| !known(k) && !old.covers(dirs, k))
        .map(|(k, h)| (k.clone(), h.clone()))
        .collect();
    let entered_skipped: Vec<String> = scan
        .skipped
        .iter()
        .filter(|k| !known(k) && !old.covers(dirs, k))
        .cloned()
        .collect();
    let entered = entered_files.len() + entered_skipped.len();
    base.files.extend(entered_files);
    base.skipped.extend(entered_skipped);
    (left, entered)
}

/// A recorded config event as a step of one file: the states before and
/// after it (`None`: no file), and the files its subject can name.
struct Step<'a> {
    paths: Vec<String>,
    before: Option<&'a String>,
    after: Option<&'a String>,
}

impl Step<'_> {
    /// The file this step goes to: one whose state in `base` is the one
    /// the step starts from and, of several, one whose state in `seen` is
    /// the one it leaves; `true` when it is the only such file.
    fn place(&self, base: &Generation, seen: &Generation) -> Option<(&String, bool)> {
        let fits: Vec<&String> = self
            .paths
            .iter()
            .filter(|p| base.files.get(*p) == self.before)
            .collect();
        if let [one] = fits[..] {
            return Some((one, true));
        }
        let ends: Vec<&String> = fits
            .iter()
            .copied()
            .filter(|p| seen.files.get(*p) == self.after)
            .collect();
        match ends[..] {
            [one] => Some((one, true)),
            _ => ends.first().or(fits.first()).map(|p| (*p, false)),
        }
    }

    fn apply(&self, base: &mut Generation, path: &str) {
        match self.after {
            Some(hash) => base.files.insert(path.to_string(), hash.clone()),
            None => base.files.remove(path),
        };
    }
}

/// Applies to `base`, the generation the cursor names, the config events
/// the ledger holds since `since`, until none applies. After a failed
/// cursor save they are the events of the captures since, and `seen` is
/// the generation the last of them stored; with them applied the diff
/// neither repeats them nor misses a file that went back to its old
/// content before this capture (`A→B` recorded, now `A`: the ledger gets
/// `B→A`).
///
/// An event goes to a file whose state in `base` is the one it starts
/// from (`hashFrom`, or no file for an addition). The ledger holds
/// subjects redacted, so files whose names differ only in a masked part
/// share one (WP-103); of several such files the event goes to the one
/// `seen` has in the state the event leaves. An event without a single
/// such file waits until no other event can be placed, then goes to the
/// first of them: they have the same content, and the ledger cannot tell
/// them apart. A subject names the files of `base` and `seen` whose
/// redaction it is, or, when nothing in it is redacted, its own path; an
/// event whose subject names no file is left out.
fn replay(
    ctx: &Ctx,
    base: &mut Generation,
    seen: &Generation,
    since: DateTime<FixedOffset>,
) -> anyhow::Result<()> {
    let mut recorded: Vec<Event> = ctx
        .ledger
        .read_range(since.min(ctx.now), ctx.now)?
        .into_iter()
        .filter(|r| r.source == Source::Config)
        .collect();
    if recorded.is_empty() {
        return Ok(());
    }
    recorded.sort_by_key(|r| r.ts);
    // the ledger holds subjects redacted
    let redactor = ctx.ledger.redactor();
    let mut paths: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let known: BTreeSet<&String> = base.files.keys().chain(seen.files.keys()).collect();
    for k in known {
        paths.entry(redactor.redact(k)).or_default().push(k.clone());
    }
    let mut steps: Vec<Step> = recorded
        .iter()
        .filter_map(|r| {
            let (from, to) = (r.meta.hash_from.as_ref(), r.meta.hash_to.as_ref());
            let (before, after) = match (r.kind, from, to) {
                (Kind::ConfigAdd, _, Some(_)) => (None, to),
                (Kind::ConfigChange, Some(_), Some(_)) => (from, to),
                (Kind::ConfigRemove, Some(_), _) => (from, None),
                _ => return None,
            };
            let paths = match paths.get(&r.subject) {
                Some(paths) => paths.clone(),
                None if redactor.redact(&r.subject) == r.subject => vec![r.subject.clone()],
                None => return None,
            };
            Some(Step {
                paths,
                before,
                after,
            })
        })
        .collect();
    loop {
        let before = steps.len();
        steps.retain(|s| match s.place(base, seen) {
            Some((path, true)) => {
                s.apply(base, path);
                false
            }
            _ => true, // not (yet) placed
        });
        if steps.len() < before {
            continue;
        }
        let Some((i, path)) = steps
            .iter()
            .enumerate()
            .find_map(|(i, s)| Some((i, s.place(base, seen)?.0.clone())))
        else {
            return Ok(());
        };
        steps.remove(i).apply(base, &path);
    }
}

impl ConfigFiles {
    /// [`Collector::collect`] with explicit watch paths, exclusions and
    /// manifest file.
    pub fn collect_from(
        &self,
        ctx: &Ctx,
        cursor: Option<&Value>,
        roots: &[PathBuf],
        excluded: &[PathBuf],
        manifest_file: &Path,
    ) -> Outcome {
        let stored = match Manifest::load(manifest_file) {
            Ok(m) => m,
            Err(e) => {
                let shown = ctx.dirs.display(manifest_file);
                return Outcome::degraded(format!("cannot read {shown}: {e}"), None);
            }
        };
        let skip_paths = &ctx.config.redaction.skip_paths;
        let skip = SkipPaths::new(&ctx.dirs.home, skip_paths);
        let scope = WatchScope::new(ctx.dirs, roots, excluded, skip_paths);
        let scan = Walker {
            dirs: ctx.dirs,
            excluded,
            skip: &skip,
            known: stored.as_ref(),
            started: SystemTime::now(),
        }
        .scan(roots);
        let current = Generation {
            scope: Some(scope.clone()),
            ..Generation::new(scan.files.clone(), scan.skipped.clone())
        };

        let prev = typed_cursor::<ConfigCursor>(cursor);
        let base = prev
            .as_ref()
            .and_then(|c| stored.as_ref()?.generation(&c.hash))
            .cloned();

        let manifest = Manifest {
            previous: base.clone().filter(|b| b.hash != current.hash),
            stats: scan.stats.clone(),
            current: current.clone(),
        };
        if stored.as_ref() != Some(&manifest)
            && let Err(e) = manifest.save(manifest_file)
        {
            return Outcome::degraded(format!("{e:#}"), None);
        }

        let next = to_cursor(&ConfigCursor {
            hash: current.hash.clone(),
            checked: ctx.now,
        });
        let mut notes = Vec::new();
        if scan.unnamed > 0 {
            notes.push(format!(
                "{} file(s) not watched: the name holds a control character or is longer than {SUBJECT_MAX} characters",
                scan.unnamed
            ));
        }
        let since = prev.as_ref().map(|p| p.checked);
        // the generation the last capture stored, when the cursor names an
        // older one: that capture's cursor save (or its ledger write) failed
        let behind = stored
            .as_ref()
            .map(|m| &m.current)
            .filter(|g| prev.as_ref().is_some_and(|c| g.hash != c.hash));
        let lost = match (&prev, &base) {
            (None, _) => Some(Lost::Cursor),
            (Some(_), None) => Some(Lost::Manifest),
            (Some(_), Some(_)) => None,
        };
        let events = match (prev, base) {
            (Some(prev), Some(mut base)) => {
                if let Some(seen) = behind
                    && let Err(e) = replay(ctx, &mut base, seen, prev.checked)
                {
                    return Outcome::degraded(format!("cannot read the ledger: {e:#}"), None);
                }
                let (left, entered) = rescope(ctx.dirs, &mut base, &scope, &scan);
                if left + entered > 0 {
                    notes.push(format!(
                        "watch scope changed: {left} file(s) left it, {entered} entered it; no events for them"
                    ));
                }
                diff(ctx, &base, &scan, prev.checked)
            }
            (Some(_), None) => {
                notes.push(format!(
                    "{} was missing or out of date; took a new baseline",
                    ctx.dirs.display(manifest_file)
                ));
                Vec::new()
            }
            (None, _) => Vec::new(), // baseline
        };
        Outcome {
            message: (!notes.is_empty()).then(|| notes.join("; ")),
            since,
            ..Outcome::ok(events, next)
        }
        .baseline(lost)
    }
}

impl Collector for ConfigFiles {
    fn name(&self) -> &'static str {
        "config"
    }

    fn cursor_reads(&self, cursor: &Value) -> bool {
        typed_cursor::<ConfigCursor>(Some(cursor)).is_some()
    }

    fn collect(&self, ctx: &Ctx, cursor: Option<&Value>) -> Outcome {
        let roots: Vec<PathBuf> = ctx
            .config
            .watch_paths
            .iter()
            .filter_map(|p| ctx.dirs.expand_config(p))
            .collect();
        let excluded = [
            Plugins::dir(ctx.sources, &ctx.dirs.home),
            ctx.dirs.home.join(MIME_CACHE),
        ];
        self.collect_from(ctx, cursor, &roots, &excluded, &Manifest::file(ctx.dirs))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn skip(patterns: &[&str]) -> SkipPaths {
        let patterns: Vec<String> = patterns.iter().map(|s| s.to_string()).collect();
        SkipPaths::new(Path::new("/home/user"), &patterns)
    }

    #[test]
    fn skip_paths_match_paths_names_and_globs() {
        let s = skip(&[
            "~/.config/hypr/secrets.conf",
            "~/.config/omarchy/private/",
            "*.key",
            "**/tokens/*.json",
            "id_?sa",
        ]);
        let m = |p: &str| s.matches(Path::new(p));
        assert!(m("/home/user/.config/hypr/secrets.conf"));
        assert!(!m("/home/user/.config/hypr/secrets.conf.bak"));
        assert!(!m("/home/user/.config/hypr/hyprland.conf"));
        // a directory and everything below it
        assert!(m("/home/user/.config/omarchy/private"));
        assert!(m("/home/user/.config/omarchy/private/a/b.conf"));
        assert!(!m("/home/user/.config/omarchy/privateer"));
        // names
        assert!(m("/home/user/.config/x/server.key"));
        assert!(!m("/home/user/.config/x/server.keys"));
        assert!(m("/home/user/.ssh/id_rsa"));
        // relative path patterns match at any directory boundary
        assert!(m("/home/user/.config/app/tokens/gh.json"));
        assert!(!m("/home/user/.config/app/tokens/sub/gh.json"));
        assert!(!m("/home/user/.config/app/mytokens/gh.json"));
        // also without a leading `**/`
        let rel = skip(&["app/secret.conf"]);
        assert!(rel.matches(Path::new("/home/user/.config/app/secret.conf")));
        assert!(!rel.matches(Path::new("/home/user/.config/myapp/secret.conf")));
        // regex characters in a pattern are literal
        assert!(!skip(&["a.b"]).matches(Path::new("/x/aXb")));
        assert!(!skip(&[""]).matches(Path::new("/x/y")));
    }

    /// WP-092: a pattern compiles only for a path or name that starts with
    /// its literal head; one without a head compiles on its first use.
    #[test]
    fn skip_paths_compile_on_demand() {
        let s = skip(&[
            "~/.config/omarchy/**/state.json",
            "id_?sa",
            "**/tokens/*.json",
        ]);
        let compiled = |s: &SkipPaths| -> Vec<bool> {
            s.paths
                .iter()
                .chain(&s.names)
                .map(|g| g.regex.get().is_some())
                .collect()
        };
        let mut paths = SkipPaths {
            paths: s.paths[..1].to_vec(),
            names: s.names.clone(),
        };
        assert!(!paths.matches(Path::new("/tmp/zed.pkg.tar.zst")));
        assert!(!paths.matches(Path::new("/home/user/.config/omarchy")));
        assert_eq!(compiled(&paths), [false, false]);
        assert!(paths.matches(Path::new("/home/user/.ssh/id_rsa")));
        assert_eq!(compiled(&paths), [false, true]);
        assert!(paths.matches(Path::new("/home/user/.config/omarchy/a/state.json")));
        assert_eq!(compiled(&paths), [true, true]);
        // no head: compiled on the first path
        paths.paths = s.paths[1..].to_vec();
        assert!(!paths.matches(Path::new("/tmp/x")));
        assert!(compiled(&paths)[0]);
    }

    #[test]
    fn generation_hash_covers_files_and_skipped() {
        let files: BTreeMap<String, String> = [("~/a".to_string(), "00".to_string())].into();
        let a = Generation::new(files.clone(), BTreeSet::new());
        let b = Generation::new(files.clone(), ["~/b".to_string()].into());
        let c = Generation::new(files, BTreeSet::new());
        assert_ne!(a.hash, b.hash);
        assert_eq!(a, c);
        assert_eq!(a.hash.len(), 64);
    }

    #[test]
    fn hash_detail_is_short() {
        assert_eq!(
            hash_detail(Some("3b385861a454"), Some("09688c642450")),
            "sha256 3b385861 → 09688c64"
        );
        assert_eq!(
            hash_detail(None, Some("c0243ca5c342")),
            "sha256 — → c0243ca5"
        );
        assert_eq!(
            hash_detail(Some("40ab11785348"), None),
            "sha256 40ab1178 → —"
        );
    }

    #[test]
    fn an_own_write_explains_only_its_path_and_hash() {
        let own = OwnWrites(
            [(
                "~/.config/omarchy/hooks/theme-set.d/x.sh".to_string(),
                OwnWrite {
                    hash: "aa".into(),
                    by: "seldon init --theme-hook".into(),
                    op: OwnOp::Install,
                },
            )]
            .into(),
        );
        let event = |subject: &str, hash: Option<&str>| {
            Event::new(
                DateTime::parse_from_rfc3339("2026-10-02T09:20:00+02:00").unwrap(),
                Source::Config,
                Kind::ConfigAdd,
                subject,
            )
            .meta(Meta {
                hash_to: hash.map(String::from),
                ..Meta::default()
            })
        };
        let path = "~/.config/omarchy/hooks/theme-set.d/x.sh";
        assert!(own.explaining(&event(path, Some("aa"))).is_some());
        assert!(own.explaining(&event(path, Some("bb"))).is_none());
        assert!(own.explaining(&event(path, None)).is_none(), "a removal");
        assert!(own.explaining(&event("~/.bashrc", Some("aa"))).is_none());
        let text = serde_json::to_string(&own).unwrap();
        assert!(
            !text.contains("\"op\""),
            "an install keeps the WP-038 shape: {text}"
        );
        assert_eq!(serde_json::from_str::<OwnWrites>(&text).unwrap(), own);
    }

    #[test]
    fn an_own_deletion_explains_only_the_removal_of_that_content() {
        let path = "~/.config/omarchy/hooks/theme-set.d/x.sh";
        let own = |op: OwnOp| {
            OwnWrites(
                [(
                    path.to_string(),
                    OwnWrite {
                        hash: "aa".into(),
                        by: "seldon init --remove-theme-hook".into(),
                        op,
                    },
                )]
                .into(),
            )
        };
        let event = |kind: Kind, from: Option<&str>, to: Option<&str>| {
            Event::new(
                DateTime::parse_from_rfc3339("2026-10-02T09:20:00+02:00").unwrap(),
                Source::Config,
                kind,
                path,
            )
            .meta(Meta {
                hash_from: from.map(String::from),
                hash_to: to.map(String::from),
                ..Meta::default()
            })
        };
        let deleted = own(OwnOp::Delete);
        assert!(
            deleted
                .explaining(&event(Kind::ConfigRemove, Some("aa"), None))
                .is_some()
        );
        // someone changed the file after the last capture: not what seldon removed
        assert!(
            deleted
                .explaining(&event(Kind::ConfigRemove, Some("bb"), None))
                .is_none()
        );
        // a deletion never explains a file that is there again
        assert!(
            deleted
                .explaining(&event(Kind::ConfigAdd, None, Some("aa")))
                .is_none()
        );
        // a removal that keeps the file is a change to the content written
        let removed = own(OwnOp::Remove);
        assert!(
            removed
                .explaining(&event(Kind::ConfigChange, Some("00"), Some("aa")))
                .is_some()
        );
        assert!(
            removed
                .explaining(&event(Kind::ConfigRemove, Some("aa"), None))
                .is_none()
        );
        let text = serde_json::to_string(&deleted).unwrap();
        assert!(text.contains("\"op\":\"delete\""), "{text}");
        assert_eq!(serde_json::from_str::<OwnWrites>(&text).unwrap(), deleted);
        assert_eq!(OwnOp::Delete.verb(), "removed");
        assert_eq!(OwnOp::Install.verb(), "installed");
    }

    #[test]
    fn own_writes_count_only_under_watched_paths() {
        let dirs = Dirs {
            home: PathBuf::from("/home/user"),
            xdg_config_home: PathBuf::from("/home/user/.config"),
            state_dir: PathBuf::from("/home/user/.local/state/seldon"),
        };
        let mut config = Config::default();
        config.redaction.skip_paths = vec!["*.key".into()];
        let watched = |p: &str| is_watched(&dirs, &config, Path::new(p));
        assert!(watched(
            "/home/user/.config/omarchy/hooks/theme-set.d/seldon-theme-set.sh"
        ));
        assert!(!watched("/home/user/Seldon/.claude/settings.json"));
        assert!(!watched("/home/user/.config/omarchy/x.key"));
        assert!(!watched("/home/user/.config/omarchyx/a"));
    }

    #[test]
    fn manifest_round_trips_with_previous() {
        let g1 = Generation::new([("~/a".into(), "11".into())].into(), BTreeSet::new());
        let g2 = Generation::new(
            [("~/a".into(), "22".into())].into(),
            ["~/big".into()].into(),
        );
        let m = Manifest {
            current: g2.clone(),
            previous: Some(g1.clone()),
            ..Manifest::default()
        };
        let text = serde_json::to_string(&m).unwrap();
        assert!(text.starts_with("{\"hash\":"), "{text}");
        let back: Manifest = serde_json::from_str(&text).unwrap();
        assert_eq!(back, m);
        assert_eq!(back.generation(&g1.hash), Some(&g1));
        assert_eq!(back.generation(&g2.hash), Some(&g2));
        assert_eq!(back.generation("nope"), None);
    }
}
