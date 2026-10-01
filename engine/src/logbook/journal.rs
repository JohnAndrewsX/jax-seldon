//! Appending to the journal: `journal/YYYY/YYYY-MM-DD.md` (SPEC-LOGBOOK §3).
//!
//! An entry is a `## HH:MM · actor · case?` heading and its text, added at
//! the end of the day's file. Existing entries are never rewritten: the
//! only other change to the file is a new id in the `cases:` frontmatter
//! list.

use anyhow::Context as _;
use chrono::{DateTime, FixedOffset};

use super::Logbook;
use crate::error::{Error, Result};
use crate::frontmatter::Document;
use crate::model::{self, Journal, JournalEntry};
use crate::sys;

/// One appended entry: where it went.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Appended {
    /// Relative to the logbook root.
    pub path: String,
    /// Whether the day's file was created.
    pub created: bool,
}

/// Appends an entry for `now` to the day's journal, creating the file
/// (with frontmatter) when it is the first entry of the day.
pub fn append(
    logbook: &Logbook,
    now: &DateTime<FixedOffset>,
    actor: &str,
    case: Option<&str>,
    text: &str,
) -> Result<Appended> {
    let date = now.date_naive();
    let rel = Journal::relative_path(date);
    let path = logbook.path(&rel);
    let entry = JournalEntry {
        time: now.time(),
        actor: actor.to_string(),
        case: case.map(str::to_string),
        text: String::new(),
    };
    let block = format!("{}\n{}\n", entry.heading(), escape(text.trim_end()));

    let (text, created) = match std::fs::read_to_string(&path) {
        Ok(existing) => (append_to(&existing, &block, case, &rel)?, false),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            let record = Journal {
                date,
                cases: case.map(|c| vec![c.to_string()]).unwrap_or_default(),
            };
            (model::render_new(&record, &block), true)
        }
        Err(e) => {
            return Err(anyhow::Error::new(e)
                .context(format!("cannot read {}", path.display()))
                .into());
        }
    };
    sys::write_atomic(&path, text.as_bytes())?;
    Ok(Appended { path: rel, created })
}

/// The day's text with `block` at the end and `case` in `cases:`.
fn append_to(existing: &str, block: &str, case: Option<&str>, rel: &str) -> Result<String> {
    let (mut journal, mut doc): (Journal, Document) = model::parse(existing)
        .map_err(|e| Error::user(format!("{rel}: invalid journal frontmatter: {e}")))?;
    if let Some(case) = case
        && !journal.cases.iter().any(|c| c == case)
    {
        journal.cases.push(case.to_string());
        model::update(&mut doc, &journal);
    }
    let nl = if doc.body.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    let body = &mut doc.body;
    if !body.is_empty() && !body.ends_with('\n') {
        body.push_str(nl);
    }
    if !body.trim().is_empty() && !body.ends_with(&format!("{nl}{nl}")) {
        body.push_str(nl);
    }
    body.push_str(&block.replace('\n', nl));
    Ok(doc.render())
}

/// Text lines that would read as an entry heading get a backslash, so a
/// pasted `## 10:00 · human` stays part of this entry.
fn escape(text: &str) -> String {
    text.lines()
        .map(|line| {
            if JournalEntry::parse_heading(line).is_some() {
                format!("\\{line}")
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Ensures the day's file exists (for `seldon open journal --editor`).
/// Returns its relative path and whether it was created.
pub fn ensure_day(logbook: &Logbook, now: &DateTime<FixedOffset>) -> Result<Appended> {
    let date = now.date_naive();
    let rel = Journal::relative_path(date);
    let path = logbook.path(&rel);
    if path.is_file() {
        return Ok(Appended {
            path: rel,
            created: false,
        });
    }
    let record = Journal {
        date,
        cases: Vec::new(),
    };
    sys::write_atomic(&path, model::render_new(&record, "").as_bytes())
        .with_context(|| format!("cannot create {rel}"))?;
    Ok(Appended {
        path: rel,
        created: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn append_keeps_the_text_before() {
        let day = "---\ntype: journal\ndate: 2026-10-01\ncases: [C-2026-003]\n---\n## 09:25 · agent:claude-code · C-2026-003\nline\n";
        let out = append_to(day, "## 10:40 · human\nnew\n", None, "x").unwrap();
        assert_eq!(out, format!("{day}\n## 10:40 · human\nnew\n"));
        let out = append_to(
            day,
            "## 10:40 · human · C-2026-004\nnew\n",
            Some("C-2026-004"),
            "x",
        )
        .unwrap();
        assert_eq!(
            out,
            day.replace("[C-2026-003]", "[C-2026-003, C-2026-004]")
                + "\n## 10:40 · human · C-2026-004\nnew\n"
        );
    }

    #[test]
    fn escape_heading_lookalikes() {
        assert_eq!(
            escape("a\n## 10:00 · human\n## Notes"),
            "a\n\\## 10:00 · human\n## Notes"
        );
    }
}
