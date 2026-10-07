//! The logbook on disk (SPEC-LOGBOOK): its root, `.seldon/logbook.toml`,
//! the paths inside it, and creating a fresh one.

pub mod cases;
pub mod git;
pub mod journal;
pub mod layout;
pub mod lock;
pub mod rules;
pub mod templates;

use std::path::{Path, PathBuf};

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
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
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

#[cfg(test)]
mod tests {
    use super::*;

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
