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
/// "Linked folders and files"), in the order [`misplaced`] reports them.
/// Each is refused by the commands that write there when it, or a part of
/// it, is a link or no directory. The year folders `journal/<YYYY>/` and
/// the area folders `areas/<area>/` are found in `journal/` and `areas/`.
const WRITTEN_FOLDERS: &[&str] = &[
    "decisions",
    "work",
    "work/queued",
    "work/active",
    "work/completed",
    "journal",
    "ledger",
    "areas",
    "system",
    "outputs",
    "archive",
    "memory",
    ".seldon",
    ".seldon/imports",
    ".claude",
];

/// The folders whose every entry [`misplaced`] names when it is a link or
/// neither a folder nor a regular file (a link there is the same escape,
/// even under a name the engine does not write yet). In the other folders
/// it names only the files the engine writes ([`written`]); workpieces
/// (`work/C-…/`), the archives and the setup kit are not looked into.
const EVERY_ENTRY: &[&str] = &[
    "decisions",
    "work/queued",
    "work/active",
    "work/completed",
    "ledger",
    "system",
    "memory",
];

/// What a writer does with a file of its own that is a link or no regular
/// file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Written {
    /// It refuses, exit 1 (ADR-0049 §1).
    Refused,
    /// A generated view: skipped with a warning (`STATUS.md`,
    /// `DECISIONS.md`, `ledger/<month>.md`; WP-171 round 2).
    Skipped,
}

/// Whether the engine writes the file `rel` (relative to the root, `/`
/// separated), and what it does when it cannot: the list of every writer
/// of a logbook file (SPEC-ENGINE §2), tied to them by
/// `tests/linked_files.rs`, where every refused or skipped file of a
/// writer must be named here.
pub fn written(rel: &str) -> Option<Written> {
    use crate::model::is_slug;
    let parts: Vec<&str> = rel.split('/').collect();
    let md = |n: &str, prefix: &str| n.starts_with(prefix) && n.ends_with(".md");
    let month = |n: &str, ext: &str| {
        n.strip_suffix(ext)
            .is_some_and(|m| NaiveDate::parse_from_str(&format!("{m}-01"), "%Y-%m-%d").is_ok())
    };
    let refused = match parts.as_slice() {
        // rules update and the capture's rules upgrade
        [file] if *file == super::rules::FILE => true,
        // `status`, `index`, `decide`'s fill: views, skipped
        ["STATUS.md" | "DECISIONS.md"] => false,
        ["ledger", n] if month(n, ".jsonl") => true,
        ["ledger", n] if month(n, ".md") => false,
        // `journal::prepare`, `ensure_day`, the import's days
        ["journal", year, day] => {
            return day
                .strip_suffix(".md")
                .and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok())
                .filter(|d| d.format("%Y").to_string() == *year)
                .map(|_| Written::Refused);
        }
        // case files: `write_new`, `CaseFile::save`
        ["work", "queued" | "active" | "completed", n] if md(n, "C-") => true,
        // `decide`, `decide accept`
        ["decisions", n] if md(n, "ADR-") => true,
        // `cases::ensure_area`
        ["areas", area, "README.md"] if is_slug(area) => true,
        // `dossier` and the import's dossier rows
        ["system", n] if crate::dossier::FENCES.iter().any(|f| f.file == *n) => true,
        // the import's memory files
        ["memory", n] if md(n, "") => true,
        // `rebuild`, the import's report
        ["outputs", n] if *n == "REBUILD.md" || md(n, "IMPORT-") => true,
        // `plan start|done|drop`
        [".seldon", "active-case"] => true,
        // the import's marker and undo file, the task import's marker
        [".seldon", "imports", n] => {
            let agent = crate::import::omarchy_agent::SOURCE;
            let names = [
                format!("{agent}.json"),
                format!("{agent}.undo.json"),
                "tasks.json".to_string(),
            ];
            if !names.iter().any(|m| m == n) {
                return None;
            }
            true
        }
        _ => return None,
    };
    Some(if refused {
        Written::Refused
    } else {
        Written::Skipped
    })
}

/// A year folder `journal/<YYYY>` or an area folder `areas/<area>`: a
/// folder the engine writes in, found inside another.
fn is_subfolder(rel: &str) -> bool {
    match rel.split('/').collect::<Vec<_>>().as_slice() {
        ["journal", y] => y.len() == 4 && y.bytes().all(|b| b.is_ascii_digit()),
        ["areas", a] => crate::model::is_slug(a),
        _ => false,
    }
}

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

/// One place [`misplaced`] names, relative to the root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    pub rel: String,
    pub what: Misplaced,
    /// Whether a command refuses it (exit 1); else a writer skips it with
    /// a warning, or none writes it (a link in a folder of [`EVERY_ENTRY`]
    /// under another name).
    pub refused: bool,
}

/// Every place where the engine writes and would refuse to, or skip
/// (WP-168, WP-171): a part of a [`WRITTEN_FOLDERS`] folder or a year or
/// area folder that is a link or no directory (refused); a file the engine
/// writes ([`written`]) that is a link or no regular file, a directory
/// included (refused or skipped as its writer does); in the folders of
/// [`EVERY_ENTRY`] also any other entry that is a link or neither a folder
/// nor a regular file (not refused). Folder by folder, by name within a
/// folder, then the root files. Nothing is followed; what cannot be read
/// is left out (`doctor`'s other rows say so).
pub fn misplaced(root: &Path) -> Vec<Found> {
    let mut out: Vec<Found> = Vec::new();
    for folder in WRITTEN_FOLDERS {
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
                    note(&mut out, &rel.to_string_lossy(), what, true);
                    real = false;
                    break;
                }
                Err(_) => {
                    real = false;
                    break;
                }
            }
        }
        if real {
            entries(root, folder, &mut out);
        }
    }
    for file in ["AGENTS.md", "STATUS.md", "DECISIONS.md"] {
        if let Ok(m) = std::fs::symlink_metadata(root.join(file)) {
            let kind = m.file_type();
            let refused = written(file) == Some(Written::Refused);
            if kind.is_symlink() {
                note(&mut out, file, Misplaced::Link, refused);
            } else if !kind.is_file() {
                note(&mut out, file, Misplaced::NoRegularFile, refused);
            }
        }
    }
    out
}

/// `rel` once (`work` for its three status folders).
fn note(out: &mut Vec<Found>, rel: &str, what: Misplaced, refused: bool) {
    if !out.iter().any(|f| f.rel == rel) {
        out.push(Found {
            rel: rel.to_string(),
            what,
            refused,
        });
    }
}

/// The entries of the real folder `root/folder` [`misplaced`] names,
/// sorted; a real year or area folder in it the same way, in place.
fn entries(root: &Path, folder: &str, out: &mut Vec<Found>) {
    let Ok(read) = std::fs::read_dir(root.join(folder)) else {
        return;
    };
    let mut names: Vec<String> = read
        .filter_map(|e| e.ok().map(|e| e.file_name().to_string_lossy().into_owned()))
        .collect();
    names.sort();
    let every = EVERY_ENTRY.contains(&folder);
    for name in names {
        let rel = format!("{folder}/{name}");
        let Ok(m) = std::fs::symlink_metadata(root.join(&rel)) else {
            continue;
        };
        let kind = m.file_type();
        let file = written(&rel);
        let refused = file == Some(Written::Refused);
        if is_subfolder(&rel) {
            if kind.is_symlink() {
                note(out, &rel, Misplaced::Link, true);
            } else if kind.is_dir() {
                entries(root, &rel, out);
            } else {
                note(out, &rel, Misplaced::NoDirectory, true);
            }
        } else if file.is_some() || every {
            if kind.is_symlink() {
                note(out, &rel, Misplaced::Link, refused);
            } else if kind.is_dir() {
                // a folder where the engine writes a file; another
                // folder there is not looked into
                if file.is_some() {
                    note(out, &rel, Misplaced::NoRegularFile, refused);
                }
            } else if !kind.is_file() {
                note(out, &rel, Misplaced::NoRegularFile, refused);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logbook::scratch::scratch;
    use std::os::unix::fs::symlink;

    unsafe extern "C" {
        fn mkfifo(path: *const std::ffi::c_char, mode: u32) -> i32;
    }

    fn fifo(path: &Path) {
        let c = std::ffi::CString::new(path.as_os_str().as_encoded_bytes()).unwrap();
        assert_eq!(unsafe { mkfifo(c.as_ptr(), 0o600) }, 0);
    }

    fn found(root: &Path) -> Vec<(String, &'static str, bool)> {
        misplaced(root)
            .into_iter()
            .map(|f| (f.rel, f.what.as_str(), f.refused))
            .collect()
    }

    #[test]
    fn a_real_layout_has_nothing_misplaced() {
        let root = scratch("seldon-layout-real");
        for dir in REQUIRED_DIRS {
            std::fs::create_dir_all(root.join(dir)).unwrap();
        }
        std::fs::create_dir_all(root.join("journal/2026")).unwrap();
        std::fs::create_dir_all(root.join(".seldon/imports")).unwrap();
        std::fs::write(root.join("journal/2026/2026-10-09.md"), "").unwrap();
        std::fs::write(root.join("AGENTS.md"), "").unwrap();
        assert!(found(&root).is_empty());
        // nothing at all: nothing to name either
        let empty = scratch("seldon-layout-empty");
        assert!(found(&empty).is_empty());
    }

    #[test]
    fn what_is_named_and_whether_it_is_refused() {
        let root = scratch("seldon-layout-misplaced");
        let outside = scratch("seldon-layout-outside");
        std::fs::create_dir_all(outside.join("dir")).unwrap();
        std::fs::write(outside.join("file"), "").unwrap();
        // what a linked folder holds is not looked into
        symlink("nowhere", outside.join("dir/inner.md")).unwrap();
        for dir in [
            "decisions/old",
            "work/queued",
            "work/C-2026-001-x",
            "journal/2026",
            "areas/editors",
            "outputs",
            "archive",
            "memory",
            ".seldon/imports",
            ".claude",
        ] {
            std::fs::create_dir_all(root.join(dir)).unwrap();
        }
        let link = |rel: &str| symlink(outside.join("file"), root.join(rel)).unwrap();
        let dir = |rel: &str| std::fs::create_dir(root.join(rel)).unwrap();
        // folders: a link, a file in a folder's place
        symlink(outside.join("dir"), root.join("ledger")).unwrap();
        std::fs::write(root.join("system"), "").unwrap();
        symlink(outside.join("dir"), root.join("journal/2027")).unwrap();
        symlink(outside.join("dir"), root.join("areas/hyprland")).unwrap();
        // every entry of decisions/, the status folders, memory/
        link("decisions/ADR-0001-x.md");
        dir("decisions/ADR-0002-y.md");
        link("decisions/notes.md");
        link("work/queued/C-2026-001-x.md");
        link("work/queued/README");
        fifo(&root.join("memory/pipe"));
        fifo(&root.join("memory/pipe.md"));
        // only the files the engine writes elsewhere
        symlink("nowhere", root.join("journal/2026/2026-10-09.md")).unwrap();
        dir("journal/2026/2026-10-10.md");
        link("journal/2026/attachments");
        link("journal/notes.md");
        link("areas/editors/README.md");
        link("areas/editors/notes.md");
        dir("outputs/IMPORT-x.md");
        link("outputs/REBUILD.md");
        link("outputs/my-report.md");
        link(".seldon/active-case");
        std::fs::create_dir(root.join(".seldon/templates")).unwrap();
        link(".seldon/templates/case.md");
        link(".seldon/imports/tasks.json");
        link(".seldon/imports/other.json");
        // the user's places: not looked into
        link("work/C-2026-001-x/notes.md");
        link("archive/AGENTS-2026-10-09.md");
        link(".claude/settings.json");
        // root files: a link refused, the views skipped
        link("AGENTS.md");
        link("STATUS.md");
        dir("DECISIONS.md");
        let named = |rel: &str, what, refused| (rel.to_string(), what, refused);
        let (link, nodir, nofile) = ("symbolic link", "no directory", "no regular file");
        assert_eq!(
            found(&root),
            [
                named("decisions/ADR-0001-x.md", link, true),
                named("decisions/ADR-0002-y.md", nofile, true),
                named("decisions/notes.md", link, false),
                named("work/queued/C-2026-001-x.md", link, true),
                named("work/queued/README", link, false),
                named("journal/2026/2026-10-09.md", link, true),
                named("journal/2026/2026-10-10.md", nofile, true),
                named("journal/2027", link, true),
                named("ledger", link, true),
                named("areas/editors/README.md", link, true),
                named("areas/hyprland", link, true),
                named("system", nodir, true),
                named("outputs/IMPORT-x.md", nofile, true),
                named("outputs/REBUILD.md", link, true),
                named("memory/pipe", nofile, false),
                named("memory/pipe.md", nofile, true),
                named(".seldon/active-case", link, true),
                named(".seldon/imports/tasks.json", link, true),
                named("AGENTS.md", link, true),
                named("STATUS.md", link, false),
                named("DECISIONS.md", nofile, false),
            ]
        );
    }

    #[test]
    fn the_ledger_and_folders_named_once() {
        let root = scratch("seldon-layout-ledger");
        let outside = scratch("seldon-layout-ledger-outside");
        std::fs::create_dir_all(root.join("ledger")).unwrap();
        std::fs::create_dir(root.join("ledger/2026-10.jsonl")).unwrap();
        symlink(outside.join("x"), root.join("ledger/2026-10.md")).unwrap();
        symlink(outside.join("x"), root.join("ledger/2026-13.md")).unwrap();
        std::fs::create_dir(root.join("journal")).unwrap();
        std::fs::write(root.join("journal/2026"), "").unwrap();
        // a linked `work`, `archive`, `.seldon/imports` and a `.claude`
        // file are named once each
        symlink(&*outside, root.join("work")).unwrap();
        symlink(&*outside, root.join("archive")).unwrap();
        std::fs::create_dir(root.join(".seldon")).unwrap();
        symlink(&*outside, root.join(".seldon/imports")).unwrap();
        std::fs::write(root.join(".claude"), "").unwrap();
        let names: Vec<(String, &str, bool)> = found(&root);
        assert_eq!(
            names,
            [
                ("work".to_string(), "symbolic link", true),
                ("journal/2026".to_string(), "no directory", true),
                ("ledger/2026-10.jsonl".to_string(), "no regular file", true),
                ("ledger/2026-10.md".to_string(), "symbolic link", false),
                // not a month: no view, but every entry of ledger/ counts
                ("ledger/2026-13.md".to_string(), "symbolic link", false),
                ("archive".to_string(), "symbolic link", true),
                (".seldon/imports".to_string(), "symbolic link", true),
                (".claude".to_string(), "no directory", true),
            ]
        );
    }

    #[test]
    fn written_names_the_files_of_the_writers() {
        for (rel, w) in [
            ("AGENTS.md", Some(Written::Refused)),
            ("STATUS.md", Some(Written::Skipped)),
            ("DECISIONS.md", Some(Written::Skipped)),
            ("PROJECT.md", None),
            ("ledger/2026-10.jsonl", Some(Written::Refused)),
            ("ledger/2026-10.md", Some(Written::Skipped)),
            ("ledger/notes.md", None),
            ("journal/2026/2026-10-09.md", Some(Written::Refused)),
            ("journal/2025/2026-10-09.md", None),
            ("journal/2026/notes.md", None),
            ("work/active/C-2026-001-zed.md", Some(Written::Refused)),
            ("work/C-2026-001/notes.md", None),
            ("decisions/ADR-0001-zed.md", Some(Written::Refused)),
            ("decisions/notes.md", None),
            ("areas/editors/README.md", Some(Written::Refused)),
            ("areas/Editors/README.md", None),
            ("system/hardware.md", Some(Written::Refused)),
            ("system/notes.md", None),
            ("memory/lessons.md", Some(Written::Refused)),
            ("outputs/REBUILD.md", Some(Written::Refused)),
            ("outputs/IMPORT-omarchy-agent.md", Some(Written::Refused)),
            ("outputs/report.md", None),
            (".seldon/active-case", Some(Written::Refused)),
            (".seldon/logbook.toml", None),
            (".seldon/imports/omarchy-agent.json", Some(Written::Refused)),
            (
                ".seldon/imports/omarchy-agent.undo.json",
                Some(Written::Refused),
            ),
            (".seldon/imports/tasks.json", Some(Written::Refused)),
            (".seldon/imports/other.json", None),
            (".seldon/templates/case.md", None),
        ] {
            assert_eq!(written(rel), w, "{rel}");
        }
    }
}
