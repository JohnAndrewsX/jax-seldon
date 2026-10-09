//! The SPEC-LOGBOOK §2 layout: creating it and checking it.

use std::io::Write as _;
use std::path::{Path, PathBuf};

use anyhow::Context as _;
use chrono::NaiveDate;
use serde_json::json;

use super::templates::{self, TEMPLATES, Vars};
use super::{LogbookMeta, META_FILE, SCHEMA_VERSION};
use crate::model::Language;
use crate::sys;

/// Directories every logbook has.
pub const REQUIRED_DIRS: &[&str] = &[
    "inbox",
    "journal",
    "ledger",
    "decisions",
    "work/queued",
    "work/active",
    "work/completed",
    "areas",
    "system",
    "memory",
    "resources",
    "outputs",
    "archive",
    ".seldon",
];

/// Files every logbook has. `STATUS.md` is generated and may be missing.
pub const REQUIRED_FILES: &[&str] = &["AGENTS.md", "PROJECT.md", "DECISIONS.md", META_FILE];

/// Directories that start empty get a `.gitkeep` so git keeps them.
const KEEP_DIRS: &[&str] = &[
    "inbox",
    "journal",
    "ledger",
    "decisions",
    "work/queued",
    "work/active",
    "work/completed",
    "resources",
    "outputs",
    "archive",
];

/// `.gitignore` of a new logbook (SPEC-LOGBOOK §2); `.*.tmp-*` is the temp
/// file of `sys::write_atomic`.
pub const GITIGNORE: &str = ".obsidian/workspace*\n.seldon/active-case\n.*.tmp-*\n";

/// What a new logbook is made of.
#[derive(Debug, Clone)]
pub struct NewLogbook {
    pub language: Language,
    pub machine_id: String,
    pub created: toml::value::Datetime,
    pub today: NaiveDate,
    /// Write `.obsidian/` (SPEC-LOGBOOK §6).
    pub obsidian: bool,
}

/// Creates the layout under `root`, which must not exist or be empty (the
/// caller checks). Never overwrites a file. New directories are 0700 and
/// files 0600 (`sys::NEW_DIR_MODE`, `sys::NEW_FILE_MODE`). The marker
/// `.seldon/logbook.toml` is written last. Returns the files written,
/// relative to `root`, in write order.
pub fn create(root: &Path, spec: &NewLogbook) -> anyhow::Result<Vec<PathBuf>> {
    let mut written = Vec::new();
    for dir in REQUIRED_DIRS.iter().chain(KEEP_DIRS) {
        let path = root.join(dir);
        sys::create_dir_private(&path)
            .with_context(|| format!("cannot create {}", path.display()))?;
    }

    write_new(root, ".gitignore", GITIGNORE, &mut written)?;

    let vars = Vars {
        machine_id: &spec.machine_id,
        language: spec.language,
        date: spec.today,
    };
    for t in TEMPLATES {
        let text = templates::render(t.text(spec.language), &vars);
        write_new(root, t.path, &text, &mut written)?;
    }
    for dir in KEEP_DIRS {
        write_new(root, &format!("{dir}/.gitkeep"), "", &mut written)?;
    }
    if spec.obsidian {
        for (path, value) in obsidian_config() {
            let text = serde_json::to_string_pretty(&value)? + "\n";
            write_new(root, path, &text, &mut written)?;
        }
    }
    // the marker last: a layout that stopped half-way is no logbook
    // (`Logbook::is_initialised`), so `doctor` does not call it ok
    let meta = LogbookMeta {
        schema_version: SCHEMA_VERSION,
        created: spec.created,
        machine_id: spec.machine_id.clone(),
        language: spec.language,
    };
    write_new(root, META_FILE, &meta.to_toml(), &mut written)?;
    Ok(written)
}

/// `.obsidian/` settings: daily notes in `journal/YYYY/`, engine and harness
/// folders and case workpieces (`work/C-…/`) excluded. No community plugins.
fn obsidian_config() -> [(&'static str, serde_json::Value); 2] {
    [
        (
            ".obsidian/app.json",
            json!({ "userIgnoreFilters": [".seldon/", ".claude/", ".codex/", "/^work\\/C-/"] }),
        ),
        (
            ".obsidian/daily-notes.json",
            json!({ "folder": "journal", "format": "YYYY/YYYY-MM-DD" }),
        ),
    ]
}

/// What the layout check of `root` is missing, as relative paths.
pub fn missing(root: &Path) -> Vec<&'static str> {
    let dirs = REQUIRED_DIRS.iter().filter(|d| !root.join(d).is_dir());
    let files = REQUIRED_FILES.iter().filter(|f| !root.join(f).is_file());
    dirs.chain(files).copied().collect()
}

/// Whether `dir` is absent or an empty directory.
pub fn is_vacant(dir: &Path) -> std::io::Result<bool> {
    match std::fs::read_dir(dir) {
        Ok(mut entries) => Ok(entries.next().is_none()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(true),
        Err(e) => Err(e),
    }
}

fn write_new(root: &Path, rel: &str, text: &str, written: &mut Vec<PathBuf>) -> anyhow::Result<()> {
    let path = root.join(rel);
    if let Some(dir) = path.parent() {
        sys::create_dir_private(dir).with_context(|| format!("cannot create {}", dir.display()))?;
    }
    let mut file = sys::create_new_private(&path)
        .with_context(|| format!("cannot create {}", path.display()))?;
    file.write_all(text.as_bytes())
        .with_context(|| format!("cannot write {}", path.display()))?;
    written.push(PathBuf::from(rel));
    Ok(())
}

/// The folders of the logbook the engine writes in (SPEC-ENGINE §2,
/// "Linked folders and files"), and how many folder levels below each
/// [`misplaced`] looks into for entries: `None` checks the folder only
/// (workpiece folders under `work/`, the archives, the setup kit are the
/// user's or written with `O_EXCL`).
const WRITTEN_FOLDERS: &[(&str, Option<u8>)] = &[
    ("decisions", Some(0)),
    ("work", None),
    ("work/queued", Some(0)),
    ("work/active", Some(0)),
    ("work/completed", Some(0)),
    ("journal", Some(1)),
    ("ledger", Some(0)),
    ("areas", Some(1)),
    ("system", Some(0)),
    ("outputs", Some(0)),
    ("archive", None),
    ("memory", Some(0)),
    (".seldon", Some(1)),
    (".claude", None),
];

/// The files directly in the root the engine writes.
const WRITTEN_ROOT_FILES: &[&str] = &["AGENTS.md", "STATUS.md", "DECISIONS.md"];

/// What [`misplaced`] found in a place the engine writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Misplaced {
    /// A symbolic link.
    Link,
    /// A file (or FIFO, socket, device) where a folder is.
    NoDirectory,
    /// A directory, FIFO, socket or device where a file is.
    NoRegularFile,
}

impl Misplaced {
    pub fn as_str(self) -> &'static str {
        match self {
            Misplaced::Link => "symbolic link",
            Misplaced::NoDirectory => "no directory",
            Misplaced::NoRegularFile => "no regular file",
        }
    }
}

/// Every place where the engine writes and would refuse to (WP-168,
/// WP-171): a part of a [`WRITTEN_FOLDERS`] folder that is a link or no
/// directory, an entry in one of them that is a link or neither a folder
/// nor a regular file, and a [`WRITTEN_ROOT_FILES`] file that is a link or
/// no regular file; relative to `root`, in the order found. Nothing is
/// followed; what cannot be read is left out (`doctor`'s other rows say
/// so).
pub fn misplaced(root: &Path) -> Vec<(String, Misplaced)> {
    // `work` once, for `work/queued`, `work/active` and `work/completed`
    fn note(rel: &Path, what: Misplaced, out: &mut Vec<(String, Misplaced)>) {
        let rel = rel.to_string_lossy().into_owned();
        if !out.iter().any(|(r, _)| *r == rel) {
            out.push((rel, what));
        }
    }
    let mut out: Vec<(String, Misplaced)> = Vec::new();
    for &(folder, depth) in WRITTEN_FOLDERS {
        let mut rel = PathBuf::new();
        let mut real = true;
        for part in Path::new(folder) {
            rel.push(part);
            match std::fs::symlink_metadata(root.join(&rel)) {
                Ok(m) if m.file_type().is_dir() => {}
                Ok(m) => {
                    let what = if m.file_type().is_symlink() {
                        Misplaced::Link
                    } else {
                        Misplaced::NoDirectory
                    };
                    note(&rel, what, &mut out);
                    real = false;
                    break;
                }
                Err(_) => {
                    real = false;
                    break;
                }
            }
        }
        if let (true, Some(depth)) = (real, depth) {
            entries(root, &rel, depth, &mut |rel, what| {
                note(rel, what, &mut out)
            });
        }
    }
    for file in WRITTEN_ROOT_FILES {
        if let Ok(m) = std::fs::symlink_metadata(root.join(file)) {
            let kind = m.file_type();
            if kind.is_symlink() {
                note(Path::new(file), Misplaced::Link, &mut out);
            } else if !kind.is_file() {
                note(Path::new(file), Misplaced::NoRegularFile, &mut out);
            }
        }
    }
    out
}

/// The entries of the real folder `root/rel` that are links or neither a
/// folder nor a regular file, sorted; `depth` more levels of real folders
/// below it the same way.
fn entries(root: &Path, rel: &Path, depth: u8, found: &mut dyn FnMut(&Path, Misplaced)) {
    let Ok(read) = std::fs::read_dir(root.join(rel)) else {
        return;
    };
    let mut names: Vec<_> = read.filter_map(|e| e.ok().map(|e| e.file_name())).collect();
    names.sort();
    for name in names {
        let rel = rel.join(name);
        let Ok(m) = std::fs::symlink_metadata(root.join(&rel)) else {
            continue;
        };
        let kind = m.file_type();
        if kind.is_symlink() {
            found(&rel, Misplaced::Link);
        } else if kind.is_dir() {
            if depth > 0 {
                entries(root, &rel, depth - 1, found);
            }
        } else if !kind.is_file() {
            found(&rel, Misplaced::NoRegularFile);
        }
    }
}
