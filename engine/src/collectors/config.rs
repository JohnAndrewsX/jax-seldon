//! `config` collector (SPEC-ENGINE §4): sha256 manifest of `watchPaths` →
//! `config-add|config-change|config-remove`.
//!
//! Every regular file under the watch paths (`config.toml watchPaths`, `~`
//! expanded; missing paths are skipped) is hashed with SHA-256 and recorded
//! under its `~`-path (SPEC-LOGBOOK §7: no absolute private paths). Not
//! hashed, never read beyond the first bytes:
//!
//! - Omarchy's plugin directory `~/.config/omarchy/plugins/` (the `plugins`
//!   collector covers it) and every `.git` directory;
//! - files and directories matching `config.toml [redaction] skipPaths`
//!   ([`SkipPaths`]); they do not appear in the manifest at all;
//! - binary files (a NUL byte in the first 8000 bytes, as git decides) and
//!   files larger than 1 MB: listed as `skipped` without a hash, so a file
//!   that grows past the limit is not reported as removed;
//! - sockets, FIFOs and devices; symlinks to directories (no loops).
//!   Symlinks to files are followed (stow-style dotfiles).
//!
//! The manifest lives in `$XDG_STATE_HOME/seldon/manifest.json`
//! ([`Manifest`]); the cursor holds the hash of the generation the ledger has
//! caught up with. `capture` saves cursors only after the ledger write, so
//! the file keeps the generation before the newest one too: when a write
//! fails, the next run diffs against the generation its cursor names and
//! nothing is lost. The first run (no cursor) is a baseline: no events.
//!
//! The engine's own writes (SPEC-ENGINE §5 rule 7): when `init` or `hook
//! install` writes a file this collector would hash, it records the file's
//! hash and the command in `$XDG_STATE_HOME/seldon/owned.json`
//! ([`OwnWrites`], [`record_own_writes`]). The next capture that runs this
//! collector explains the matching event (`crate::reconcile::explain_own_writes`).

use std::collections::{BTreeMap, BTreeSet};
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use chrono::{DateTime, FixedOffset, Local, Timelike as _, Utc};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::plugins::Plugins;
use super::{Collector, Ctx, Outcome, Tz, to_cursor, typed_cursor};
use crate::config::{Config, Dirs};
use crate::logbook::lock::Lock;
use crate::model::event::{Event, Kind, Meta, SUBJECT_MAX, Source};
use crate::sys;

/// File name of the manifest in the state directory (SPEC-ENGINE §2).
pub const MANIFEST_FILE: &str = "manifest.json";

/// Files larger than this are not hashed (SPEC-ENGINE §4: "> 1 MB").
pub const MAX_FILE_SIZE: u64 = 1024 * 1024;

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
}

impl Generation {
    pub fn new(files: BTreeMap<String, String>, skipped: BTreeSet<String>) -> Self {
        let body = serde_json::to_vec(&(&files, &skipped)).expect("strings always serialise");
        Generation {
            hash: sys::sha256_hex(&body),
            files,
            skipped,
        }
    }
}

/// `manifest.json`: the current generation, plus the one the ledger last
/// caught up with when that is older (a capture whose write failed).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    #[serde(flatten)]
    pub current: Generation,
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

/// One file the engine wrote under a watched path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OwnWrite {
    /// SHA-256 of the content the engine wrote.
    pub hash: String,
    /// The command that wrote it (`seldon init --theme-hook`).
    pub by: String,
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
    /// event's new hash is the one the engine wrote.
    pub fn explaining(&self, e: &Event) -> Option<&OwnWrite> {
        let hash = e.meta.hash_to.as_deref()?;
        self.0.get(&e.subject).filter(|w| w.hash == hash)
    }
}

/// Whether the config collector hashes `path`: it lies under a watch path
/// and matches no `[redaction] skipPaths` pattern.
pub fn is_watched(dirs: &Dirs, config: &Config, path: &Path) -> bool {
    config
        .watch_paths
        .iter()
        .any(|w| path.starts_with(dirs.expand(w)))
        && !SkipPaths::new(&dirs.home, &config.redaction.skip_paths).matches(path)
}

/// Records each of `paths` that the config collector hashes as the
/// engine's own write by `by`, with the hash of its content now
/// (SPEC-ENGINE §5 rule 7). Returns the `~`-paths recorded; the lock
/// proves that no capture reads the file meanwhile.
pub fn record_own_writes(
    _lock: &Lock,
    dirs: &Dirs,
    config: &Config,
    paths: &[PathBuf],
    by: &str,
) -> anyhow::Result<Vec<String>> {
    let mut recorded = Vec::new();
    let file = OwnWrites::file(dirs);
    let mut own = OwnWrites::load(&file)
        .map_err(|e| anyhow::anyhow!("cannot read {}: {e}", dirs.display(&file)))?;
    for path in paths.iter().filter(|p| is_watched(dirs, config, p)) {
        let Some(hash) = std::fs::metadata(path)
            .ok()
            .filter(|m| m.is_file())
            .and_then(|m| hash_file(path, m.len()))
        else {
            continue; // not hashed, so never reported either
        };
        let key = dirs.display(path);
        own.0.insert(
            key.clone(),
            OwnWrite {
                hash,
                by: by.to_string(),
            },
        );
        recorded.push(key);
    }
    if !recorded.is_empty() {
        own.save(&file)?;
    }
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
#[derive(Debug, Clone, Default)]
pub struct SkipPaths {
    paths: Vec<Regex>,
    names: Vec<Regex>,
}

impl SkipPaths {
    pub fn new(home: &Path, patterns: &[String]) -> Self {
        let mut skip = SkipPaths::default();
        let home = home.to_string_lossy();
        for p in patterns.iter().map(|p| p.trim()).filter(|p| !p.is_empty()) {
            let p = p.trim_end_matches('/');
            if !p.contains('/') && p != "~" {
                skip.names.push(glob(p, "^", "$"));
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
            skip.paths.push(glob(&expanded, prefix, "(?:/.*)?$"));
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

/// A glob as an anchored regex.
fn glob(pattern: &str, prefix: &str, suffix: &str) -> Regex {
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
    Regex::new(&re).expect("an escaped glob is a valid regex")
}

/// What one walk of the watch paths found.
#[derive(Debug, Default)]
struct Scan {
    files: BTreeMap<String, String>,
    skipped: BTreeSet<String>,
    mtimes: BTreeMap<String, SystemTime>,
}

/// The rules of one walk.
struct Walker<'a> {
    dirs: &'a Dirs,
    excluded: &'a [PathBuf],
    skip: &'a SkipPaths,
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
        if key.chars().count() > SUBJECT_MAX {
            return; // cannot be an event subject
        }
        if let Ok(t) = meta.modified() {
            scan.mtimes.insert(key.clone(), t);
        }
        match hash_file(path, meta.len()) {
            Some(hash) => {
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
        let skip = SkipPaths::new(&ctx.dirs.home, &ctx.config.redaction.skip_paths);
        let scan = Walker {
            dirs: ctx.dirs,
            excluded,
            skip: &skip,
        }
        .scan(roots);
        let current = Generation::new(scan.files.clone(), scan.skipped.clone());

        let stored = match Manifest::load(manifest_file) {
            Ok(m) => m,
            Err(e) => {
                let shown = ctx.dirs.display(manifest_file);
                return Outcome::degraded(format!("cannot read {shown}: {e}"), None);
            }
        };
        let prev = typed_cursor::<ConfigCursor>(cursor);
        let base = prev
            .as_ref()
            .and_then(|c| stored.as_ref()?.generation(&c.hash))
            .cloned();

        let manifest = Manifest {
            previous: base.clone().filter(|b| b.hash != current.hash),
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
        match (prev, base) {
            (Some(prev), Some(base)) => Outcome::ok(diff(ctx, &base, &scan, prev.checked), next),
            (Some(_), None) => Outcome {
                message: Some(format!(
                    "{} was missing or out of date; took a new baseline",
                    ctx.dirs.display(manifest_file)
                )),
                ..Outcome::ok(Vec::new(), next)
            },
            (None, _) => Outcome::ok(Vec::new(), next), // baseline
        }
    }
}

impl Collector for ConfigFiles {
    fn name(&self) -> &'static str {
        "config"
    }

    fn collect(&self, ctx: &Ctx, cursor: Option<&Value>) -> Outcome {
        let roots: Vec<PathBuf> = ctx
            .config
            .watch_paths
            .iter()
            .map(|p| ctx.dirs.expand(p))
            .collect();
        let excluded = [Plugins::dir(ctx.sources, &ctx.dirs.home)];
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
        // regex characters in a pattern are literal
        assert!(!skip(&["a.b"]).matches(Path::new("/x/aXb")));
        assert!(!skip(&[""]).matches(Path::new("/x/y")));
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
        assert_eq!(serde_json::from_str::<OwnWrites>(&text).unwrap(), own);
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
