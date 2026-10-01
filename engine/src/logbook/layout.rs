//! The SPEC-LOGBOOK §2 layout: creating it and checking it.

use std::io::Write as _;
use std::path::{Path, PathBuf};

use anyhow::Context as _;
use chrono::NaiveDate;
use serde_json::json;

use super::templates::{self, TEMPLATES, Vars};
use super::{LogbookMeta, META_FILE, SCHEMA_VERSION};
use crate::model::Language;

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

/// `.gitignore` of a new logbook (SPEC-LOGBOOK §2).
pub const GITIGNORE: &str = ".obsidian/workspace*\n.seldon/active-case\n";

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
/// caller checks). Never overwrites a file. Returns the files written,
/// relative to `root`, in write order.
pub fn create(root: &Path, spec: &NewLogbook) -> anyhow::Result<Vec<PathBuf>> {
    let mut written = Vec::new();
    for dir in REQUIRED_DIRS.iter().chain(KEEP_DIRS) {
        let path = root.join(dir);
        std::fs::create_dir_all(&path)
            .with_context(|| format!("cannot create {}", path.display()))?;
    }

    let meta = LogbookMeta {
        schema_version: SCHEMA_VERSION,
        created: spec.created,
        machine_id: spec.machine_id.clone(),
        language: spec.language,
    };
    write_new(root, META_FILE, &meta.to_toml(), &mut written)?;
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
        std::fs::create_dir_all(dir).with_context(|| format!("cannot create {}", dir.display()))?;
    }
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .with_context(|| format!("cannot create {}", path.display()))?;
    file.write_all(text.as_bytes())
        .with_context(|| format!("cannot write {}", path.display()))?;
    written.push(PathBuf::from(rel));
    Ok(())
}
