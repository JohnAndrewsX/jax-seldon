//! The logbook on disk (SPEC-LOGBOOK): its root, `.seldon/logbook.toml`,
//! the paths inside it, and creating a fresh one.

pub mod cases;
pub mod git;
pub mod journal;
pub mod layout;
pub mod lock;
pub mod rules;
pub mod templates;

use std::path::{Component, Path, PathBuf};

use anyhow::Context as _;
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::model::Language;

/// The logbook format this engine reads and writes (SPEC-LOGBOOK, "Version 1").
pub const SCHEMA_VERSION: u32 = 1;

/// `.seldon/logbook.toml`, relative to the root.
pub const META_FILE: &str = ".seldon/logbook.toml";

/// `.seldon/active-case`, relative to the root.
pub const ACTIVE_CASE_FILE: &str = ".seldon/active-case";

/// `.seldon/logbook.toml` (SPEC-LOGBOOK §1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogbookMeta {
    pub schema_version: u32,
    /// RFC 3339 with offset, written as a TOML datetime.
    pub created: toml::value::Datetime,
    /// Host name plus a random suffix, e.g. `workstation-7f3a`.
    pub machine_id: String,
    pub language: Language,
}

impl LogbookMeta {
    pub fn to_toml(&self) -> String {
        toml::to_string(self).expect("logbook.toml always serialises")
    }
}

/// An initialised logbook.
#[derive(Debug, Clone)]
pub struct Logbook {
    pub root: PathBuf,
    pub meta: LogbookMeta,
}

impl Logbook {
    /// Whether `root` holds `.seldon/logbook.toml`.
    pub fn is_initialised(root: &Path) -> bool {
        root.join(META_FILE).is_file()
    }

    /// Opens the logbook at `root`. Exit 3 if it is not initialised; a
    /// malformed or newer `logbook.toml` is an engine error.
    pub fn open(root: &Path) -> Result<Logbook> {
        let meta_path = root.join(META_FILE);
        let text = match std::fs::read_to_string(&meta_path) {
            Ok(t) => t,
            // `.seldon` a file (or the root one): no logbook either (WP-168)
            Err(e)
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory
                ) =>
            {
                return Err(Error::NotInitialised(root.to_path_buf()));
            }
            Err(e) => {
                return Err(anyhow::Error::new(e)
                    .context(format!("cannot read {}", meta_path.display()))
                    .into());
            }
        };
        let meta: LogbookMeta = toml::from_str(&text)
            .with_context(|| format!("{}: invalid logbook.toml", meta_path.display()))?;
        if meta.schema_version != SCHEMA_VERSION {
            return Err(anyhow::anyhow!(
                "{}: schemaVersion {} is not supported (this engine reads {SCHEMA_VERSION})",
                meta_path.display(),
                meta.schema_version
            )
            .into());
        }
        Ok(Logbook {
            root: root.to_path_buf(),
            meta,
        })
    }

    pub fn path(&self, relative: impl AsRef<Path>) -> PathBuf {
        self.root.join(relative)
    }

    /// [`checked_dir`] below this logbook's root.
    pub fn checked_dir(&self, relative: impl AsRef<Path>) -> Result<PathBuf> {
        checked_dir(&self.root, relative.as_ref())
    }

    /// [`checked_file`] for `path`, below the root (absolute) or relative
    /// to it.
    pub fn checked_file(&self, path: impl AsRef<Path>) -> Result<PathBuf> {
        let path = path.as_ref();
        let relative = if path.is_absolute() {
            path.strip_prefix(&self.root).map_err(|_| {
                anyhow::anyhow!(
                    "{} is not in the logbook {}",
                    path.display(),
                    self.root.display()
                )
            })?
        } else {
            path
        };
        checked_file(&self.root, relative)
    }

    /// Case files `work/{queued,active,completed}/C-*.md`, sorted by path.
    pub fn case_files(&self) -> anyhow::Result<Vec<PathBuf>> {
        let mut out = Vec::new();
        for folder in ["queued", "active", "completed"] {
            out.extend(md_files(&self.path("work").join(folder), "C-")?);
        }
        Ok(out)
    }

    /// Decision files `decisions/ADR-*.md`, sorted.
    pub fn decision_files(&self) -> anyhow::Result<Vec<PathBuf>> {
        md_files(&self.path("decisions"), "ADR-")
    }

    /// Every file of decision `id` (`decisions/<id>-*.md` or `<id>.md`),
    /// sorted; more than one is a hand-made duplicate.
    pub fn decision_files_of(&self, id: &str) -> anyhow::Result<Vec<PathBuf>> {
        let prefix = format!("{id}-");
        let exact = format!("{id}.md");
        Ok(self
            .decision_files()?
            .into_iter()
            .filter(|p| {
                p.file_name().is_some_and(|n| {
                    let n = n.to_string_lossy();
                    n.starts_with(&prefix) || n == exact
                })
            })
            .collect())
    }

    /// The file of decision `id`, the first in sorted order; `None` when
    /// there is none.
    pub fn decision_file(&self, id: &str) -> anyhow::Result<Option<PathBuf>> {
        Ok(self.decision_files_of(id)?.into_iter().next())
    }

    /// Journal files `journal/YYYY/*.md`, sorted.
    pub fn journal_files(&self) -> anyhow::Result<Vec<PathBuf>> {
        let mut out = Vec::new();
        for year in sorted_entries(&self.path("journal"))? {
            if year.is_dir() {
                out.extend(md_files(&year, "")?);
            }
        }
        Ok(out)
    }

    /// Area READMEs `areas/*/README.md`, sorted.
    pub fn area_files(&self) -> anyhow::Result<Vec<PathBuf>> {
        Ok(sorted_entries(&self.path("areas"))?
            .into_iter()
            .map(|d| d.join("README.md"))
            .filter(|p| p.is_file())
            .collect())
    }

    /// Memory files `memory/*.md`, sorted.
    pub fn memory_files(&self) -> anyhow::Result<Vec<PathBuf>> {
        md_files(&self.path("memory"), "")
    }
}

/// `root/relative`, a folder of the logbook the engine creates or writes
/// a file in, checked (WP-168, as `triage::checked_dir` for the state
/// folder, WP-124): every part of `relative` that exists is a real
/// directory inside the logbook, never a symbolic link (followed, a write
/// would land wherever it points) or anything else. The first part that
/// does not exist ends the check: the writer creates it, 0700. A link or
/// a non-directory is a user error (exit 1) naming the part relative to
/// the root. The root itself is not checked: a logbook kept behind a link
/// is the user's choice. `relative` holds plain names only (a caller's
/// bug otherwise).
pub fn checked_dir(root: &Path, relative: &Path) -> Result<PathBuf> {
    if !relative
        .components()
        .all(|c| matches!(c, Component::Normal(_)))
    {
        return Err(
            anyhow::anyhow!("{} is not a folder below the logbook", relative.display()).into(),
        );
    }
    let mut dir = root.to_path_buf();
    let mut shown = PathBuf::new();
    for name in relative.components() {
        dir.push(name);
        shown.push(name);
        match std::fs::symlink_metadata(&dir) {
            Ok(m) if m.file_type().is_dir() => {}
            Ok(m) => {
                return Err(Error::user(format!(
                    "{} is {}, not a folder of the logbook; make it a folder and run the command again",
                    shown.display(),
                    if m.file_type().is_symlink() {
                        "a symbolic link"
                    } else {
                        "no directory"
                    }
                )));
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => break,
            Err(e) => return Err(anyhow::anyhow!("{}: {e}", dir.display()).into()),
        }
    }
    Ok(root.join(relative))
}

/// `root/relative`, a file of the logbook the engine writes (replaces or
/// appends to), checked (WP-171, ADR-0049): its folder with
/// [`checked_dir`] first, then the file itself without following a link.
/// A regular file or none passes (the writer creates it, 0600); a
/// symbolic link (followed, the write would land wherever it points) or
/// anything else (a directory, FIFO, socket or device in its place) is a
/// user error (exit 1) naming the file relative to the root. A file
/// directly in the root has no folder to check.
pub fn checked_file(root: &Path, relative: &Path) -> Result<PathBuf> {
    let Some(name) = relative.file_name() else {
        return Err(
            anyhow::anyhow!("{} is not a file below the logbook", relative.display()).into(),
        );
    };
    let dir = checked_dir(root, relative.parent().unwrap_or(Path::new("")))?;
    let path = dir.join(name);
    match std::fs::symlink_metadata(&path) {
        Ok(m) if m.file_type().is_file() => {}
        Ok(m) => {
            return Err(Error::user(format!(
                "{} is {}, not a file of the logbook; make it a file and run the command again",
                relative.display(),
                if m.file_type().is_symlink() {
                    "a symbolic link"
                } else {
                    "no regular file"
                }
            )));
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(anyhow::anyhow!("{}: {e}", path.display()).into()),
    }
    Ok(path)
}

/// Entries of `dir`, sorted; an absent directory has none.
fn sorted_entries(dir: &Path) -> anyhow::Result<Vec<PathBuf>> {
    let read = match std::fs::read_dir(dir) {
        Ok(r) => r,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => {
            return Err(anyhow::Error::new(e).context(format!("cannot list {}", dir.display())));
        }
    };
    let mut out = read
        .map(|e| e.map(|e| e.path()))
        .collect::<std::io::Result<Vec<_>>>()
        .with_context(|| format!("cannot list {}", dir.display()))?;
    out.sort();
    Ok(out)
}

/// `*.md` files in `dir` whose name starts with `prefix`, sorted.
fn md_files(dir: &Path, prefix: &str) -> anyhow::Result<Vec<PathBuf>> {
    Ok(sorted_entries(dir)?
        .into_iter()
        .filter(|p| {
            p.is_file()
                && p.extension().is_some_and(|e| e == "md")
                && p.file_name()
                    .is_some_and(|n| n.to_string_lossy().starts_with(prefix))
        })
        .collect())
}

/// A fresh directory under `TMPDIR` for one unit test, removed first if a
/// run left it and again when the test ends, passed or not (WP-168).
#[cfg(test)]
pub(crate) mod scratch {
    use std::path::{Path, PathBuf};

    pub struct Scratch(PathBuf);

    impl std::ops::Deref for Scratch {
        type Target = PathBuf;
        fn deref(&self) -> &PathBuf {
            &self.0
        }
    }

    impl AsRef<Path> for Scratch {
        fn as_ref(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// `<TMPDIR>/<name>-<pid>`, empty.
    pub fn scratch(name: &str) -> Scratch {
        let dir = std::env::temp_dir().join(format!("{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Scratch(dir)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::logbook::scratch::{Scratch, scratch as fresh};

    fn scratch(tag: &str) -> Scratch {
        fresh(&format!("seldon-checked-dir-{tag}"))
    }

    /// Longest path a Unix socket can be bound at (`sun_path` less its NUL).
    const SOCKET_PATH_MAX: usize = 107;

    fn user_message(r: Result<PathBuf>) -> String {
        match r {
            Err(Error::User(m)) => m,
            other => panic!("expected a user error, got {other:?}"),
        }
    }

    fn engine_error(r: Result<PathBuf>) -> String {
        match r {
            Err(e @ Error::Engine(_)) => e.to_string(),
            other => panic!("expected an engine error, got {other:?}"),
        }
    }

    #[test]
    fn checked_dir_takes_real_and_missing_folders() {
        let root = scratch("real");
        std::fs::create_dir_all(root.join("work/active")).unwrap();
        let check = |rel: &str| checked_dir(&root, Path::new(rel));
        assert_eq!(check("work/active").unwrap(), root.join("work/active"));
        assert_eq!(check("work").unwrap(), root.join("work"));
        // missing: the writer creates it
        assert_eq!(check("decisions").unwrap(), root.join("decisions"));
        assert_eq!(check("work/queued").unwrap(), root.join("work/queued"));
        assert_eq!(check("journal/2026").unwrap(), root.join("journal/2026"));
        // nothing to check: the root itself
        assert_eq!(check("").unwrap(), *root);
    }

    #[test]
    fn checked_dir_refuses_a_link_and_a_non_directory() {
        let root = scratch("refused");
        let outside = scratch("refused-outside");
        std::fs::create_dir_all(root.join("work")).unwrap();
        std::os::unix::fs::symlink(&outside, root.join("decisions")).unwrap();
        std::os::unix::fs::symlink("nowhere", root.join("inbox")).unwrap();
        std::os::unix::fs::symlink(&outside, root.join("work/active")).unwrap();
        std::fs::write(root.join("ledger"), "").unwrap();
        std::fs::write(root.join("work/queued"), "").unwrap();
        // a socket: neither link nor directory; it can be bound only at a
        // short path, so a long TMPDIR leaves this case out
        let socket = root.join("system");
        let _socket = (socket.as_os_str().len() <= SOCKET_PATH_MAX)
            .then(|| std::os::unix::net::UnixListener::bind(&socket).unwrap());
        if _socket.is_none() {
            eprintln!(
                "note: the socket case is skipped: {} is longer than {SOCKET_PATH_MAX} bytes",
                socket.display()
            );
        }
        let check = |rel: &str| checked_dir(&root, Path::new(rel));

        for (rel, shown) in [
            ("decisions", "decisions"),
            ("inbox", "inbox"),
            ("work/active", "work/active"),
            ("work/active/x", "work/active"),
        ] {
            assert_eq!(
                user_message(check(rel)),
                format!(
                    "{shown} is a symbolic link, not a folder of the logbook; make it a folder and run the command again"
                ),
                "{rel}"
            );
        }
        let socket_cases = [("system", "system"), ("system/x", "system")];
        let socket_cases = if _socket.is_some() {
            &socket_cases[..]
        } else {
            &[]
        };
        for (rel, shown) in [("ledger", "ledger"), ("work/queued", "work/queued")]
            .iter()
            .chain(socket_cases)
        {
            assert_eq!(
                user_message(check(rel)),
                format!(
                    "{shown} is no directory, not a folder of the logbook; make it a folder and run the command again"
                ),
                "{rel}"
            );
        }
        // a link in the middle: the part is named, not the folder asked for
        std::fs::remove_dir_all(root.join("work")).unwrap();
        std::os::unix::fs::symlink(&outside, root.join("work")).unwrap();
        std::fs::create_dir_all(outside.join("active")).unwrap();
        assert!(user_message(check("work/active")).starts_with("work is a symbolic link,"));
        assert!(std::fs::read_dir(&*outside).unwrap().count() == 1);
    }

    #[test]
    fn checked_dir_takes_plain_names_only() {
        let root = scratch("names");
        for rel in ["../x", "a/../b", "/etc", "./a"] {
            let e = engine_error(checked_dir(&root, Path::new(rel)));
            assert!(
                e.ends_with("is not a folder below the logbook"),
                "{rel}: {e}"
            );
        }
    }

    #[test]
    fn checked_dir_does_not_check_the_root() {
        let base = scratch("root-link");
        std::fs::create_dir_all(base.join("real/decisions")).unwrap();
        let root = base.join("logbook");
        std::os::unix::fs::symlink("real", &root).unwrap();
        assert_eq!(
            checked_dir(&root, Path::new("decisions")).unwrap(),
            root.join("decisions")
        );
    }

    #[test]
    fn checked_dir_reports_an_unreadable_part_as_an_engine_error() {
        let root = scratch("unreadable");
        std::fs::write(root.join("file"), "").unwrap();
        // ENOTDIR below a file is not "missing": the walk stops at the file
        assert!(
            user_message(checked_dir(&root, Path::new("file/x")))
                .starts_with("file is no directory")
        );
        // a name too long for the file system: neither missing nor a part
        let long = "x".repeat(300);
        let e = engine_error(checked_dir(&root, Path::new(&long)));
        assert!(e.contains(&long), "{e}");
    }

    #[test]
    fn checked_file_checks_the_folder_of_the_file() {
        let root = scratch("file");
        let outside = scratch("file-outside");
        std::os::unix::fs::symlink(&outside, root.join("decisions")).unwrap();
        let logbook = Logbook {
            root: root.to_path_buf(),
            meta: LogbookMeta {
                schema_version: 1,
                created: "2026-09-01T19:00:42+02:00".parse().unwrap(),
                machine_id: "workstation-7f3a".into(),
                language: Language::En,
            },
        };
        assert_eq!(
            logbook.checked_file("AGENTS.md").unwrap(),
            root.join("AGENTS.md")
        );
        assert_eq!(
            logbook
                .checked_file(root.join("journal/2026/2026-10-09.md"))
                .unwrap(),
            root.join("journal/2026/2026-10-09.md")
        );
        assert_eq!(
            logbook.checked_file("journal/2026/2026-10-09.md").unwrap(),
            root.join("journal/2026/2026-10-09.md")
        );
        assert!(
            user_message(logbook.checked_file("decisions/ADR-0001-x.md"))
                .starts_with("decisions is a symbolic link,")
        );
        assert!(
            user_message(logbook.checked_file(root.join("decisions/ADR-0001-x.md")))
                .starts_with("decisions is a symbolic link,")
        );
        assert!(
            user_message(logbook.checked_dir("decisions"))
                .starts_with("decisions is a symbolic link,")
        );
        let e = engine_error(logbook.checked_file(outside.join("x.md")));
        assert!(e.contains("is not in the logbook"), "{e}");
    }

    /// WP-171: the file itself, after its folder, never followed.
    #[test]
    fn checked_file_refuses_a_link_and_a_non_regular_file() {
        let root = scratch("file-itself");
        let outside = scratch("file-itself-outside");
        std::fs::create_dir_all(root.join("journal/2026")).unwrap();
        std::fs::write(outside.join("day.md"), "outside\n").unwrap();
        std::fs::write(root.join("journal/2026/2026-10-08.md"), "x\n").unwrap();
        std::os::unix::fs::symlink(
            outside.join("day.md"),
            root.join("journal/2026/2026-10-09.md"),
        )
        .unwrap();
        std::os::unix::fs::symlink("nowhere.md", root.join("journal/2026/2026-10-10.md")).unwrap();
        std::os::unix::fs::symlink(&*outside, root.join("journal/2026/2026-10-11.md")).unwrap();
        std::fs::create_dir(root.join("journal/2026/2026-10-12.md")).unwrap();
        std::os::unix::fs::symlink(outside.join("day.md"), root.join("STATUS.md")).unwrap();
        let check = |rel: &str| checked_file(&root, Path::new(rel));

        // a regular file and a missing one pass, as the absolute path
        assert_eq!(
            check("journal/2026/2026-10-08.md").unwrap(),
            root.join("journal/2026/2026-10-08.md")
        );
        assert_eq!(
            check("journal/2026/2026-10-13.md").unwrap(),
            root.join("journal/2026/2026-10-13.md")
        );
        assert_eq!(check("AGENTS.md").unwrap(), root.join("AGENTS.md"));
        // a link to a file, a dangling one, one to a folder: named
        for rel in [
            "journal/2026/2026-10-09.md",
            "journal/2026/2026-10-10.md",
            "journal/2026/2026-10-11.md",
            "STATUS.md",
        ] {
            assert_eq!(
                user_message(check(rel)),
                format!(
                    "{rel} is a symbolic link, not a file of the logbook; make it a file and run the command again"
                )
            );
        }
        assert_eq!(
            user_message(check("journal/2026/2026-10-12.md")),
            "journal/2026/2026-10-12.md is no regular file, not a file of the logbook; make it a file and run the command again"
        );
        // the folder first: a linked year is named, not the day in it
        std::fs::rename(root.join("journal/2026"), outside.join("2026")).unwrap();
        std::os::unix::fs::symlink(outside.join("2026"), root.join("journal/2026")).unwrap();
        assert!(
            user_message(check("journal/2026/2026-10-08.md"))
                .starts_with("journal/2026 is a symbolic link, not a folder of the logbook;")
        );
        assert_eq!(
            std::fs::read_to_string(outside.join("day.md")).unwrap(),
            "outside\n"
        );
        // plain names only, and a name at the end
        for rel in ["", "../x.md", "/etc/x.md", "a/../b.md"] {
            let e = engine_error(check(rel));
            assert!(e.contains("below the logbook"), "{rel}: {e}");
        }
    }

    #[test]
    fn checked_file_reports_an_unreadable_file_as_an_engine_error() {
        let root = scratch("file-unreadable");
        let long = "x".repeat(300);
        let e = engine_error(checked_file(&root, Path::new(&long)));
        assert!(e.contains(&long), "{e}");
    }

    #[test]
    fn checked_file_of_a_logbook_checks_the_file_too() {
        let root = scratch("file-method");
        std::os::unix::fs::symlink("nowhere", root.join("AGENTS.md")).unwrap();
        let logbook = Logbook {
            root: root.to_path_buf(),
            meta: LogbookMeta {
                schema_version: 1,
                created: "2026-09-01T19:00:42+02:00".parse().unwrap(),
                machine_id: "workstation-7f3a".into(),
                language: Language::En,
            },
        };
        for path in [PathBuf::from("AGENTS.md"), root.join("AGENTS.md")] {
            assert!(
                user_message(logbook.checked_file(&path))
                    .starts_with("AGENTS.md is a symbolic link, not a file of the logbook;")
            );
        }
    }

    #[test]
    fn meta_toml_style() {
        let meta = LogbookMeta {
            schema_version: 1,
            created: "2026-09-01T19:00:42+02:00".parse().unwrap(),
            machine_id: "workstation-7f3a".into(),
            language: Language::De,
        };
        assert_eq!(
            meta.to_toml(),
            "schemaVersion = 1\ncreated = 2026-09-01T19:00:42+02:00\nmachineId = \"workstation-7f3a\"\nlanguage = \"de\"\n"
        );
    }
}
