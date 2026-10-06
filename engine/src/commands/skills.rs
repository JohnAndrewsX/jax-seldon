//! The Seldon agent skill (WP-094; ADR-0027 §8): `seldon hook install
//! skills` and `hook uninstall skills`, and the `skills` row of `doctor`.
//!
//! - **The skill.** Five Markdown files under `engine/assets/skills/seldon/`,
//!   compiled in ([`FILES`]). Shaped like Omarchy's own agent skills
//!   (`$OMARCHY_PATH/default/agents/skills/`), which point at it for the
//!   record and which it points at for Omarchy work.
//! - **Where.** Omarchy's agent skill folders, the ones its migrations link
//!   its skills into ([`SKILL_DIRS`], plus `~/.hermes/profiles/*/skills`).
//!   Only a folder that exists is used: Seldon never creates one, nor an
//!   agent's home. The skill goes into `<folder>/seldon/`.
//! - **Ownership.** `<folder>/seldon/` is Seldon's when it holds the
//!   manifest [`MANIFEST`]: the SHA-256 of each file as Seldon wrote it.
//!   Anything else named `seldon` there (no manifest, a symbolic link, a
//!   file) is foreign and never touched. A file of Seldon's whose content
//!   is neither the shipped one nor the one the manifest names was changed
//!   by hand: install keeps the folder as it is, uninstall keeps that file.
//! - **Writes.** Under the state lock; the manifest first, then the files,
//!   so an interrupted install is completed by the next one. Each write and
//!   deletion is recorded as the engine's own (SPEC-ENGINE §5 rule 7) where
//!   the config collector watches the path. Running either command again
//!   changes nothing.
//! - **Upgrades.** A skill nobody edited (every file its manifest names is
//!   there as Seldon wrote it) is brought up to this engine's by the next
//!   `seldon capture` ([`upgrade_unedited_under`], WP-111; ADR-0028 §4d),
//!   as the user, never from a package hook. A folder without the skill
//!   stays without it. A skill changed by hand keeps its files; `doctor`
//!   names `seldon hook install skills --replace`, which copies the
//!   changed files into the logbook's `archive/` first.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::{Context, Output};
use crate::collectors::config::OwnOp;
use crate::config::{Config, Dirs};
use crate::error::Result;
use crate::logbook::lock::Lock;
use crate::sys;

/// The skill's name: its folder in every agent skill folder.
pub const SKILL_NAME: &str = "seldon";

/// The ownership mark in `<folder>/seldon/`.
pub const MANIFEST: &str = ".seldon-skill.json";

/// The skill's files: name, content.
pub const FILES: [(&str, &str); 5] = [
    (
        "SKILL.md",
        include_str!("../../assets/skills/seldon/SKILL.md"),
    ),
    (
        "case.md",
        include_str!("../../assets/skills/seldon/case.md"),
    ),
    (
        "drift.md",
        include_str!("../../assets/skills/seldon/drift.md"),
    ),
    (
        "snapshot.md",
        include_str!("../../assets/skills/seldon/snapshot.md"),
    ),
    (
        "update.md",
        include_str!("../../assets/skills/seldon/update.md"),
    ),
];

/// The agent skill folders under the home directory, as Omarchy's
/// migrations link its own skills into them (`~/.hermes/profiles/*/skills`
/// comes on top, [`skill_dirs`]).
pub const SKILL_DIRS: [&str; 5] = [
    ".agents/skills",
    ".claude/skills",
    ".codex/skills",
    ".pi/agent/skills",
    ".hermes/skills",
];

/// The command `hook install skills` records its writes with.
pub const INSTALL_BY: &str = "seldon hook install skills";

/// What a capture's update of an unedited skill is recorded as (WP-111).
pub const CAPTURE_BY: &str = "seldon capture";

/// The command `hook uninstall skills` records its deletions with.
pub const UNINSTALL_BY: &str = "seldon hook uninstall skills";

/// The manifest's content.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
struct Manifest {
    /// The engine that wrote it (for people; never compared).
    #[serde(default)]
    by: String,
    /// File name → SHA-256 (hex) of the content Seldon wrote.
    files: BTreeMap<String, String>,
}

impl Manifest {
    fn shipped() -> Self {
        Manifest {
            by: format!("seldon {}", crate::VERSION),
            files: FILES
                .iter()
                .map(|(name, text)| (name.to_string(), sys::sha256_hex(text.as_bytes())))
                .collect(),
        }
    }
}

/// A file name a manifest may name: one plain path component that is not
/// hidden, so a manifest can never point outside its folder.
fn plain_name(name: &str) -> bool {
    !name.is_empty() && !name.starts_with('.') && !name.contains('/') && !name.contains('\0')
}

/// The candidate folders: [`SKILL_DIRS`], then every
/// `~/.hermes/profiles/<p>/skills` that exists, in name order.
pub fn candidates(home: &Path) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = SKILL_DIRS.iter().map(|d| home.join(d)).collect();
    if let Ok(entries) = std::fs::read_dir(home.join(".hermes/profiles")) {
        let mut profiles: Vec<PathBuf> = entries
            .filter_map(|e| e.ok())
            .map(|e| e.path().join("skills"))
            .filter(|p| p.is_dir())
            .collect();
        profiles.sort();
        out.extend(profiles);
    }
    out
}

/// The candidate folders that exist, and those that do not.
pub fn skill_dirs(home: &Path) -> (Vec<PathBuf>, Vec<PathBuf>) {
    candidates(home).into_iter().partition(|d| d.is_dir())
}

/// What `<folder>/seldon/` holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum State {
    /// Nothing named `seldon`.
    Missing,
    /// Seldon's, every file as shipped.
    Current,
    /// Seldon's, unchanged since it was written, older than this engine's
    /// (or a file is missing).
    Outdated,
    /// Seldon's, but these files were changed by hand.
    Changed(Vec<String>),
    /// Something named `seldon` that Seldon did not write.
    Foreign,
}

impl State {
    pub fn as_str(&self) -> &'static str {
        match self {
            State::Missing => "missing",
            State::Current => "current",
            State::Outdated => "outdated",
            State::Changed(_) => "changed",
            State::Foreign => "foreign",
        }
    }
}

/// A regular file at `path`, not through a symbolic link.
fn plain_file(path: &Path) -> Option<bool> {
    match std::fs::symlink_metadata(path) {
        Ok(m) => Some(m.file_type().is_file()),
        Err(_) => None,
    }
}

/// The manifest of `target`, if `target` is a real folder that has one.
fn read_manifest(target: &Path) -> Option<Manifest> {
    let meta = std::fs::symlink_metadata(target).ok()?;
    if !meta.file_type().is_dir() {
        return None;
    }
    let path = target.join(MANIFEST);
    if plain_file(&path) != Some(true) {
        return None;
    }
    let text = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

/// One file of Seldon's folder against the shipped content and the
/// manifest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FileState {
    /// Not there.
    Absent,
    /// The shipped content.
    Shipped,
    /// The content the manifest names (an older version).
    Written,
    /// Neither, or not a regular file: the user's now.
    Edited,
}

fn file_state(path: &Path, shipped: Option<&str>, written: Option<&String>) -> FileState {
    match plain_file(path) {
        None => return FileState::Absent,
        Some(false) => return FileState::Edited,
        Some(true) => {}
    }
    let Ok(bytes) = std::fs::read(path) else {
        return FileState::Edited;
    };
    if shipped.is_some_and(|s| s.as_bytes() == bytes) {
        FileState::Shipped
    } else if written.is_some_and(|h| *h == sys::sha256_hex(&bytes)) {
        FileState::Written
    } else {
        FileState::Edited
    }
}

/// Every file name Seldon's folder may hold: the shipped ones, then those
/// an older manifest names.
fn names(manifest: &Manifest) -> Vec<String> {
    let mut out: Vec<String> = FILES.iter().map(|(n, _)| n.to_string()).collect();
    for name in manifest.files.keys() {
        if plain_name(name) && !out.contains(name) {
            out.push(name.clone());
        }
    }
    out
}

fn shipped(name: &str) -> Option<&'static str> {
    FILES.iter().find(|(n, _)| *n == name).map(|(_, t)| *t)
}

/// A real, empty folder (what an install stopped right after creating it
/// leaves): writing into it overwrites nothing.
fn is_empty_dir(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_dir())
        && std::fs::read_dir(path).is_ok_and(|mut d| d.next().is_none())
}

/// The state of `<folder>/seldon/` (read-only).
pub fn state(folder: &Path) -> State {
    let target = folder.join(SKILL_NAME);
    if std::fs::symlink_metadata(&target).is_err() || is_empty_dir(&target) {
        return State::Missing;
    }
    let Some(manifest) = read_manifest(&target) else {
        return State::Foreign;
    };
    let mut edited = Vec::new();
    let mut current = manifest.files == Manifest::shipped().files;
    for name in names(&manifest) {
        let s = file_state(
            &target.join(&name),
            shipped(&name),
            manifest.files.get(&name),
        );
        match (s, shipped(&name).is_some()) {
            (FileState::Edited, _) => edited.push(name),
            (FileState::Shipped, true) | (FileState::Absent, false) => {}
            _ => current = false,
        }
    }
    if !edited.is_empty() {
        State::Changed(edited)
    } else if current {
        State::Current
    } else {
        State::Outdated
    }
}

/// Whether Seldon's skill in `folder` is outdated and nobody touched it:
/// every file its manifest names is there, a regular file, as that
/// manifest or this engine names it. Only then does a capture update it
/// without asking ([`upgrade_unedited_under`]); a file the user deleted
/// counts as a change.
pub fn unedited(folder: &Path) -> bool {
    if state(folder) != State::Outdated {
        return false;
    }
    let target = folder.join(SKILL_NAME);
    let Some(manifest) = read_manifest(&target) else {
        return false;
    };
    manifest.files.iter().all(|(name, hash)| {
        plain_name(name)
            && matches!(
                file_state(&target.join(name), shipped(name), Some(hash)),
                FileState::Shipped | FileState::Written
            )
    })
}

/// What install or uninstall did in one folder.
#[derive(Debug, Clone, Default)]
struct DirReport {
    folder: PathBuf,
    before: &'static str,
    /// `installed`, `updated`, `replaced`, `unchanged`, `kept`, `removed`,
    /// `absent`, `failed`.
    action: &'static str,
    written: Vec<String>,
    removed: Vec<String>,
    kept: Vec<String>,
    own: Vec<String>,
    own_errors: Vec<String>,
    /// `--replace`: where the changed files were copied, relative to the
    /// logbook, and their names.
    archived: Option<String>,
    archived_files: Vec<String>,
    /// Why this folder failed (`action: failed`); the other folders go on.
    error: Option<String>,
    /// The command the own writes are recorded with ([`INSTALL_BY`],
    /// [`CAPTURE_BY`], [`UNINSTALL_BY`]).
    by: &'static str,
}

impl DirReport {
    fn json(&self, dirs: &Dirs) -> Value {
        json!({
            "dir": dirs.display(&self.folder),
            "path": self.folder.join(SKILL_NAME),
            "state": self.before,
            "action": self.action,
            "written": self.written,
            "removed": self.removed,
            "kept": self.kept,
            "archived": self.archived,
            "archivedFiles": self.archived_files,
            "error": self.error,
        })
    }
}

fn own_record(
    lock: &Lock,
    ctx: &Context,
    config: &Config,
    paths: &[PathBuf],
    report: &mut DirReport,
) {
    if paths.is_empty() {
        return;
    }
    match super::setup::record_own_writes_under(lock, ctx, config, paths, report.by, OwnOp::Install)
    {
        Ok(p) => report.own.extend(p),
        Err(e) => report.own_errors.push(e),
    }
}

fn delete_own(
    lock: &Lock,
    ctx: &Context,
    config: &Config,
    path: &Path,
    by: &str,
    report: &mut DirReport,
) -> Result<()> {
    match super::setup::delete_own_file_under(lock, ctx, config, path, by)? {
        Ok(p) => report.own.extend(p),
        Err(e) => report.own_errors.push(e),
    }
    Ok(())
}

/// Debug builds only: `SELDON_TEST_SKILL_STOP_AFTER=N` stops an install
/// in a folder with an error after its N-th write, as a crash would (the
/// tests' interrupted install).
fn stop_after() -> Option<usize> {
    #[cfg(debug_assertions)]
    if let Some(n) = std::env::var("SELDON_TEST_SKILL_STOP_AFTER")
        .ok()
        .and_then(|v| v.parse().ok())
    {
        return Some(n);
    }
    None
}

/// Writes `bytes` to `path` and notes it in `written`, unless
/// [`stop_after`] says the install stops first.
fn write_step(path: &Path, bytes: &[u8], written: &mut Vec<PathBuf>) -> Result<()> {
    if stop_after() == Some(written.len()) {
        return Err(anyhow::anyhow!("stopped after {} write(s) (test)", written.len()).into());
    }
    sys::write_atomic(path, bytes)?;
    written.push(path.to_path_buf());
    Ok(())
}

/// Where `hook install skills --replace` copies the files of a skill that
/// were changed by hand: one folder `archive/skill-<date>[-N]/` of the
/// logbook per run, made with the first copy, never one that exists.
pub struct Archive {
    root: PathBuf,
    today: chrono::NaiveDate,
    /// The folder, relative to the logbook, once made.
    rel: Option<String>,
}

impl Archive {
    pub fn new(root: &Path, today: chrono::NaiveDate) -> Self {
        Archive {
            root: root.to_path_buf(),
            today,
            rel: None,
        }
    }

    /// The run's folder, made on first use.
    fn dir(&mut self) -> Result<String> {
        if let Some(rel) = &self.rel {
            return Ok(rel.clone());
        }
        let parent = self.root.join("archive");
        sys::create_dir_private(&parent)
            .map_err(|e| anyhow::anyhow!("cannot create {}: {e}", parent.display()))?;
        let mut n = 1u32;
        loop {
            let rel = match n {
                1 => format!("archive/skill-{}", self.today),
                n => format!("archive/skill-{}-{n}", self.today),
            };
            match std::fs::create_dir(self.root.join(&rel)) {
                Ok(()) => {
                    self.rel = Some(rel.clone());
                    return Ok(rel);
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => n += 1,
                Err(e) => {
                    return Err(anyhow::anyhow!("cannot create {}: {e}", rel).into());
                }
            }
        }
    }

    /// Copies `bytes` as `<run folder>/<label>/<name>`; returns
    /// `<run folder>/<label>`.
    fn copy(&mut self, label: &str, name: &str, bytes: &[u8]) -> Result<String> {
        let rel = format!("{}/{label}", self.dir()?);
        let dir = self.root.join(&rel);
        sys::create_dir_private(&dir).map_err(|e| anyhow::anyhow!("cannot create {rel}: {e}"))?;
        let mut file = sys::create_new_private(&dir.join(name))
            .map_err(|e| anyhow::anyhow!("cannot write {rel}/{name}: {e}"))?;
        std::io::Write::write_all(&mut file, bytes)
            .map_err(|e| anyhow::anyhow!("cannot write {rel}/{name}: {e}"))?;
        Ok(rel)
    }

    /// Whether anything was copied.
    pub fn used(&self) -> bool {
        self.rel.is_some()
    }
}

/// The archive's name for an agent skill folder: its path below the home
/// directory, each part without its leading dots, joined by `-`
/// (`~/.claude/skills` → `claude-skills`).
fn archive_label(home: &Path, folder: &Path) -> String {
    let rel = folder.strip_prefix(home).unwrap_or(folder);
    let parts: Vec<String> = rel
        .components()
        .filter_map(|c| match c {
            std::path::Component::Normal(s) => {
                Some(s.to_string_lossy().trim_start_matches('.').to_string())
            }
            _ => None,
        })
        .filter(|s| !s.is_empty())
        .collect();
    if parts.is_empty() {
        "skills".to_string()
    } else {
        parts.join("-")
    }
}

/// Installs or updates the skill in `folder` (which exists), under the
/// caller's state lock. With `archive` (`--replace`), a skill changed by
/// hand has its changed files copied there and is then installed as
/// shipped. A failure is this folder's result (`failed`, with the error);
/// what was written before it is recorded all the same.
fn install_into(
    lock: &Lock,
    ctx: &Context,
    config: &Config,
    folder: &Path,
    archive: Option<&mut Archive>,
    by: &'static str,
) -> DirReport {
    let before = state(folder);
    let mut report = DirReport {
        folder: folder.to_path_buf(),
        before: before.as_str(),
        by,
        ..DirReport::default()
    };
    let mut written = Vec::new();
    if let Err(e) = install_steps(
        lock,
        ctx,
        config,
        &before,
        archive,
        &mut report,
        &mut written,
    ) {
        report.action = "failed";
        report.error = Some(e.to_string());
    }
    own_record(lock, ctx, config, &written, &mut report);
    report
}

fn install_steps(
    lock: &Lock,
    ctx: &Context,
    config: &Config,
    before: &State,
    archive: Option<&mut Archive>,
    report: &mut DirReport,
    written: &mut Vec<PathBuf>,
) -> Result<()> {
    let target = report.folder.join(SKILL_NAME);
    // the changed files `--replace` archived: no longer the user's here
    let mut archived: Vec<String> = Vec::new();
    match before {
        State::Current => {
            report.action = "unchanged";
            return Ok(());
        }
        State::Foreign => {
            report.action = "kept";
            return Ok(());
        }
        State::Changed(files) => {
            // only regular files can be copied; anything else stays as it is
            let copyable = files
                .iter()
                .all(|n| plain_file(&target.join(n)) == Some(true));
            let Some(archive) = archive.filter(|_| copyable) else {
                report.action = "kept";
                report.kept = files.clone();
                return Ok(());
            };
            let label = archive_label(&ctx.dirs.home, &report.folder);
            for name in files {
                let bytes = std::fs::read(target.join(name)).map_err(|e| {
                    anyhow::anyhow!("cannot read {}: {e}", ctx.dirs.display(&target.join(name)))
                })?;
                report.archived = Some(archive.copy(&label, name, &bytes)?);
            }
            archived = files.clone();
            report.archived_files = files.clone();
            report.action = "replaced";
        }
        State::Missing => {
            // the skill's own folder, never the agent's: `folder` exists
            if !target.exists() {
                std::fs::create_dir(&target).map_err(|e| {
                    anyhow::anyhow!("cannot create {}: {e}", ctx.dirs.display(&target))
                })?;
            }
            report.action = "installed";
        }
        State::Outdated => report.action = "updated",
    }
    let old = read_manifest(&target).unwrap_or_default();
    let manifest = Manifest::shipped();
    let manifest_path = target.join(MANIFEST);
    let write_manifest = |written: &mut Vec<PathBuf>| -> Result<()> {
        if old.files == manifest.files && plain_file(&manifest_path) == Some(true) {
            return Ok(());
        }
        let mut text = serde_json::to_string_pretty(&manifest).map_err(anyhow::Error::from)?;
        text.push('\n');
        write_step(&manifest_path, text.as_bytes(), written)
    };
    // A new folder gets the manifest first: an install that stops half way
    // leaves a folder that is still Seldon's. An update writes it last: a
    // file still as the old manifest names it is not mistaken for one
    // changed by hand.
    let fresh = *before == State::Missing;
    if fresh {
        write_manifest(written)?;
    }
    for (name, text) in FILES {
        let path = target.join(name);
        if file_state(&path, Some(text), old.files.get(name)) == FileState::Shipped {
            continue;
        }
        write_step(&path, text.as_bytes(), written)?;
        report.written.push(name.to_string());
    }
    // files an older skill had and this one does not, if still as written
    // (or archived just now), before the new manifest forgets them
    for name in names(&old).into_iter().filter(|n| shipped(n).is_none()) {
        let path = target.join(&name);
        if file_state(&path, None, old.files.get(&name)) == FileState::Written
            || archived.contains(&name)
        {
            let by = report.by;
            delete_own(lock, ctx, config, &path, by, report)?;
            report.removed.push(name);
        }
    }
    if !fresh {
        write_manifest(written)?;
    }
    Ok(())
}

/// Removes Seldon's files from `folder`, under the caller's state lock. A
/// failure is this folder's result (`failed`, with the error).
fn uninstall_from(lock: &Lock, ctx: &Context, config: &Config, folder: &Path) -> DirReport {
    let before = state(folder);
    let mut report = DirReport {
        folder: folder.to_path_buf(),
        before: before.as_str(),
        by: UNINSTALL_BY,
        ..DirReport::default()
    };
    if let Err(e) = uninstall_steps(lock, ctx, config, before, &mut report) {
        report.action = "failed";
        report.error = Some(e.to_string());
    }
    report
}

fn uninstall_steps(
    lock: &Lock,
    ctx: &Context,
    config: &Config,
    before: State,
    report: &mut DirReport,
) -> Result<()> {
    let target = report.folder.join(SKILL_NAME);
    let manifest = match before {
        State::Missing => {
            report.action = "absent";
            return Ok(());
        }
        State::Foreign => {
            report.action = "kept";
            return Ok(());
        }
        _ => read_manifest(&target).unwrap_or_default(),
    };
    for name in names(&manifest) {
        let path = target.join(&name);
        match file_state(&path, shipped(&name), manifest.files.get(&name)) {
            FileState::Absent => {}
            FileState::Shipped | FileState::Written => {
                delete_own(lock, ctx, config, &path, UNINSTALL_BY, report)?;
                report.removed.push(name);
            }
            FileState::Edited => report.kept.push(name),
        }
    }
    // the manifest stays with a kept file: the folder is still Seldon's,
    // and `doctor` names the changed file
    if report.kept.is_empty() {
        delete_own(
            lock,
            ctx,
            config,
            &target.join(MANIFEST),
            UNINSTALL_BY,
            report,
        )?;
        // only when empty: a file the user put there stays
        if std::fs::remove_dir(&target).is_ok() {
            report.action = "removed";
        } else {
            report.action = "kept";
        }
    } else {
        report.action = "kept";
    }
    Ok(())
}

/// The `skills` row of `seldon doctor` (read-only): the state of the
/// skill in every agent skill folder that exists. Installed everywhere, or
/// no folder: ok. Missing somewhere: ok, the skill is optional, with the
/// fix. Outdated and unedited: ok, the next capture updates it (WP-111).
/// Outdated otherwise, changed by hand or foreign: degraded, with the fix;
/// a skill changed by hand reads "outdated" and its fix is the one command
/// that archives the user's copy ([`REPLACE_FIX`]).
pub fn doctor_row(dirs: &Dirs) -> (super::doctor::Status, String, Option<String>) {
    use super::doctor::Status;
    let (present, absent) = skill_dirs(&dirs.home);
    if present.is_empty() {
        let none: Vec<String> = absent.iter().map(|p| dirs.display(p)).collect();
        return (
            Status::Ok,
            format!(
                "no agent skill folder ({}); nothing to install",
                none.join(", ")
            ),
            None,
        );
    }
    // key → folders as shown; a changed folder carries its files
    let mut by: BTreeMap<&'static str, Vec<String>> = BTreeMap::new();
    for folder in &present {
        let shown = dirs.display(folder);
        let (key, shown) = match state(folder) {
            State::Outdated if unedited(folder) => ("pending", shown),
            State::Changed(files) => (
                "changed",
                format!("{shown} (changed by hand: {})", files.join(", ")),
            ),
            s => (s.as_str(), shown),
        };
        by.entry(key).or_default().push(shown);
    }
    let label = |key: &str, word: &str| by.get(key).map(|v| format!("{word} in {}", v.join(", ")));
    let message = [
        label("current", "installed"),
        label("pending", "updated at the next capture"),
        label("outdated", "outdated"),
        label("changed", "outdated"),
        label("foreign", "another skill named seldon"),
        label("missing", "not installed"),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join("; ");
    let status = if ["outdated", "changed", "foreign"]
        .iter()
        .any(|k| by.contains_key(k))
    {
        Status::Degraded
    } else {
        Status::Ok
    };
    let mut fixes = Vec::new();
    if by.contains_key("foreign") {
        fixes.push(format!(
            "move the folder named seldon away where it is not seldon's, then {INSTALL_BY}"
        ));
    }
    if by.contains_key("changed") {
        fixes.push(REPLACE_FIX.to_string());
    } else if by.contains_key("outdated") || by.contains_key("missing") {
        fixes.push(INSTALL_BY.to_string());
    }
    let fix = (!fixes.is_empty()).then(|| fixes.join("; "));
    (status, format!("seldon agent skill: {message}"), fix)
}

/// What [`install_under`] did, for `hook install skills` and `init`.
#[derive(Debug, Clone)]
pub struct Installed {
    reports: Vec<DirReport>,
    /// Candidate folders that do not exist (never created).
    absent: Vec<PathBuf>,
}

impl Installed {
    /// Whether every folder that exists has the current skill.
    pub fn done(&self) -> bool {
        self.reports
            .iter()
            .all(|r| matches!(r.action, "installed" | "updated" | "unchanged"))
    }

    /// One line for `init`'s report.
    pub fn summary(&self, dirs: &Dirs) -> String {
        if self.reports.is_empty() {
            return format!(
                "no agent skill folder ({}); nothing installed",
                self.absent_list(dirs)
            );
        }
        let lines: Vec<String> = self
            .reports
            .iter()
            .map(|r| format!("{} {}", r.action, dirs.display(&r.folder.join(SKILL_NAME))))
            .collect();
        lines.join(", ")
    }

    fn absent_list(&self, dirs: &Dirs) -> String {
        self.absent
            .iter()
            .map(|p| dirs.display(p))
            .collect::<Vec<_>>()
            .join(", ")
    }

    pub fn json(&self, dirs: &Dirs) -> Value {
        json!({
            "skill": SKILL_NAME,
            "dirs": self.reports.iter().map(|r| r.json(dirs)).collect::<Vec<_>>(),
            "absent": self.absent.iter().map(|p| dirs.display(p)).collect::<Vec<_>>(),
            "ownWrites": own_json(&self.reports),
        })
    }
}

fn own_json(reports: &[DirReport]) -> Value {
    let errors: Vec<&String> = reports.iter().flat_map(|r| &r.own_errors).collect();
    if let Some(e) = errors.first() {
        return json!({ "error": e });
    }
    json!(reports.iter().flat_map(|r| &r.own).collect::<Vec<_>>())
}

/// Installs the skill into every agent skill folder that exists, under the
/// caller's state lock; with `archive` (`--replace`) also where it was
/// changed by hand.
pub fn install_under(
    lock: &Lock,
    ctx: &Context,
    config: &Config,
    mut archive: Option<&mut Archive>,
) -> Result<Installed> {
    let (present, absent) = skill_dirs(&ctx.dirs.home);
    let reports = present
        .iter()
        .map(|folder| {
            install_into(
                lock,
                ctx,
                config,
                folder,
                archive.as_deref_mut(),
                INSTALL_BY,
            )
        })
        .collect();
    Ok(Installed { reports, absent })
}

/// What a capture's upgrade of unedited skills did ([`upgrade_unedited_under`]).
#[derive(Debug, Clone, Default)]
pub struct Upgraded {
    /// The folders whose skill was updated, as `doctor` shows them.
    pub updated: Vec<String>,
    /// One line per folder that failed, and per own-write record that
    /// failed.
    pub warnings: Vec<String>,
}

/// The capture's part (WP-111): updates the skill in every agent skill
/// folder where it is outdated and unedited ([`unedited`]), under the
/// caller's state lock. Nothing else: no folder gets a skill it does not
/// have, a skill changed by hand or not Seldon's is left as it is. Never
/// as root: a root process's home is not the user's.
pub fn upgrade_unedited_under(lock: &Lock, ctx: &Context, config: &Config) -> Upgraded {
    let mut out = Upgraded::default();
    if sys::runs_as_root() {
        return out;
    }
    let (present, _) = skill_dirs(&ctx.dirs.home);
    for folder in present.iter().filter(|f| unedited(f)) {
        let report = install_into(lock, ctx, config, folder, None, CAPTURE_BY);
        let shown = ctx.dirs.display(&folder.join(SKILL_NAME));
        match report.action {
            "updated" => out.updated.push(ctx.dirs.display(folder)),
            "failed" => out.warnings.push(format!(
                "the agent skill in {shown} was not updated: {}",
                report.error.as_deref().unwrap_or_default()
            )),
            _ => {}
        }
        for e in &report.own_errors {
            out.warnings.push(super::setup::own_writes_warning(e));
        }
    }
    out
}

fn config_of(ctx: &Context) -> Config {
    ctx.load_config().ok().flatten().unwrap_or_default()
}

/// The fix line for a folder install keeps.
fn kept_fix(dirs: &Dirs, r: &DirReport) -> String {
    let path = dirs.display(&r.folder.join(SKILL_NAME));
    match r.before {
        "foreign" => format!("{path} was not written by seldon; left alone"),
        _ if r
            .kept
            .iter()
            .any(|n| plain_file(&r.folder.join(SKILL_NAME).join(n)) != Some(true)) =>
        {
            format!(
                "{path}: {} changed by hand and not a plain file; kept (fix: move {path} away, then {INSTALL_BY})",
                r.kept.join(", ")
            )
        }
        _ => format!(
            "{path}: {} changed by hand; kept (fix: {REPLACE_FIX})",
            r.kept.join(", ")
        ),
    }
}

/// The one-command fix for a skill changed by hand.
pub const REPLACE_FIX: &str = "seldon hook install skills --replace (archives your copy)";

/// `seldon hook install skills [--replace]`. `--replace` copies the files
/// of a skill changed by hand into the logbook's `archive/` and installs
/// the skill as shipped there too; it commits when it copied anything.
pub fn install(ctx: &Context, replace: bool) -> Result<Output> {
    let (config, logbook) = if replace {
        let (config, logbook) = ctx.open_logbook()?;
        (config, Some(logbook))
    } else {
        (config_of(ctx), None)
    };
    let mut archive = logbook
        .as_ref()
        .map(|l| Archive::new(&l.root, ctx.now.date_naive()));
    let lock = ctx.lock()?;
    let installed = install_under(&lock, ctx, &config, archive.as_mut())?;
    let commit = match (&logbook, &archive) {
        (Some(logbook), Some(a)) if a.used() => Some(super::autocommit(
            ctx,
            &config,
            logbook,
            "hook install skills --replace",
        )),
        _ => None,
    };
    drop(lock);
    let dirs = &ctx.dirs;
    let mut human = if installed.reports.is_empty() {
        format!(
            "No agent skill folder exists ({}); nothing installed. Seldon never creates one.",
            installed.absent_list(dirs)
        )
    } else {
        "The Seldon agent skill:".to_string()
    };
    for r in &installed.reports {
        let path = dirs.display(&r.folder.join(SKILL_NAME));
        match r.action {
            "kept" => {
                let _ = write!(human, "\n  kept       {}", kept_fix(dirs, r));
            }
            "failed" => {
                let _ = write!(
                    human,
                    "\n  failed     {path}: {}",
                    r.error.as_deref().unwrap_or_default()
                );
            }
            "replaced" => {
                let _ = write!(
                    human,
                    "\n  replaced   {path}; your copy of {} is in {}",
                    r.archived_files.join(", "),
                    r.archived.as_deref().unwrap_or_default()
                );
            }
            action => {
                let _ = write!(human, "\n  {action:<10} {path}");
            }
        }
    }
    if !installed.reports.is_empty() && !installed.absent.is_empty() {
        let _ = write!(
            human,
            "\nNot there, not created: {}",
            installed.absent_list(dirs)
        );
    }
    for e in installed.reports.iter().flat_map(|r| &r.own_errors) {
        let _ = write!(human, "\n{}", super::setup::own_writes_warning(e));
    }
    let mut json = installed.json(dirs);
    if let Some(commit) = &commit {
        human.push_str(&commit.human());
        json["git"] = commit.json();
    }
    Ok(with_failures(human, json, &installed.reports))
}

/// `seldon hook uninstall skills`.
pub fn uninstall(ctx: &Context) -> Result<Output> {
    let config = config_of(ctx);
    let (present, absent) = skill_dirs(&ctx.dirs.home);
    let lock = ctx.lock()?;
    let reports: Vec<DirReport> = present
        .iter()
        .map(|folder| uninstall_from(&lock, ctx, &config, folder))
        .collect();
    drop(lock);
    let dirs = &ctx.dirs;
    let mut human = if reports.iter().all(|r| r.action == "absent") {
        "The Seldon agent skill is not installed; nothing changed.".to_string()
    } else {
        "The Seldon agent skill:".to_string()
    };
    for r in reports.iter().filter(|r| r.action != "absent") {
        let path = dirs.display(&r.folder.join(SKILL_NAME));
        let _ = match (r.action, r.before) {
            ("removed", _) => write!(human, "\n  removed    {path}"),
            ("failed", _) => write!(
                human,
                "\n  failed     {path}: {}",
                r.error.as_deref().unwrap_or_default()
            ),
            (_, "foreign") => write!(
                human,
                "\n  kept       {path} was not written by seldon; left alone"
            ),
            _ if !r.kept.is_empty() => write!(
                human,
                "\n  kept       {path}: {} changed by hand, so kept (delete it to remove it)",
                r.kept.join(", ")
            ),
            _ => write!(
                human,
                "\n  kept       {path}: holds files that are not seldon's"
            ),
        };
    }
    for e in reports.iter().flat_map(|r| &r.own_errors) {
        let _ = write!(human, "\n{}", super::setup::own_writes_warning(e));
    }
    let json = json!({
        "skill": SKILL_NAME,
        "dirs": reports.iter().map(|r| r.json(dirs)).collect::<Vec<_>>(),
        "absent": absent.iter().map(|p| dirs.display(p)).collect::<Vec<_>>(),
        "ownWrites": own_json(&reports),
    });
    Ok(with_failures(human, json, &reports))
}

/// The output of install or uninstall: exit 1 when a folder failed (the
/// report lists what was done in the others), else 0.
fn with_failures(mut human: String, json: Value, reports: &[DirReport]) -> Output {
    let failed = reports.iter().filter(|r| r.action == "failed").count();
    if failed == 0 {
        return Output::ok(human, json);
    }
    let _ = write!(
        human,
        "\n{failed} folder(s) failed; the others are as listed. Fix the folder's \
         permissions, then run the command again."
    );
    Output {
        human,
        json,
        exit: crate::error::Exit::UserError,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_file_is_shipped_once_and_the_skill_has_its_frontmatter() {
        let names: Vec<&str> = FILES.iter().map(|(n, _)| *n).collect();
        assert_eq!(
            names,
            [
                "SKILL.md",
                "case.md",
                "drift.md",
                "snapshot.md",
                "update.md"
            ]
        );
        let skill = FILES[0].1;
        assert!(skill.starts_with("---\nname: seldon\ndescription: >\n"));
        for (name, text) in FILES {
            assert!(plain_name(name), "{name}");
            assert!(text.ends_with('\n') && !text.contains('\r'), "{name}");
        }
    }

    #[test]
    fn a_manifest_names_only_plain_files() {
        for bad in ["", ".seldon-skill.json", "../x", "a/b", ".hidden", "x\0"] {
            assert!(!plain_name(bad), "{bad:?}");
        }
        assert!(plain_name("old.md"));
    }
}
