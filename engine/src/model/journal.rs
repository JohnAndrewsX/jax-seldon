//! Journal day: `journal/YYYY/YYYY-MM-DD.md` (SPEC-LOGBOOK §3).
//!
//! Entries are appended as `## HH:MM · actor · case?` headings and are never
//! rewritten.

use chrono::{NaiveDate, NaiveTime};
use serde::Deserialize;

use super::{Record, field, is_case_id, null_as_empty};
use crate::frontmatter::{FmValue, FrontmatterError};

/// Separator in entry headings (U+00B7 with spaces).
pub const SEPARATOR: &str = " · ";

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Journal {
    pub date: NaiveDate,
    #[serde(default, deserialize_with = "null_as_empty")]
    pub cases: Vec<String>,
}

impl Record for Journal {
    const TYPE: Option<&'static str> = Some("journal");
    const KEYS: &'static [&'static str] = &["type", "date", "cases"];

    fn to_values(&self) -> Vec<(&'static str, FmValue)> {
        vec![
            ("type", FmValue::str("journal")),
            ("date", FmValue::str(self.date.to_string())),
            ("cases", FmValue::list(&self.cases)),
        ]
    }

    fn validate(&self) -> Result<(), FrontmatterError> {
        match self.cases.iter().find(|c| !is_case_id(c)) {
            Some(bad) => Err(field("cases", format!("`{bad}` is not C-YYYY-NNN"))),
            None => Ok(()),
        }
    }
}

/// One `## HH:MM · actor · case?` entry and the text under it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JournalEntry {
    pub time: NaiveTime,
    pub actor: String,
    pub case: Option<String>,
    /// The lines under the heading, trimmed.
    pub text: String,
}

impl JournalEntry {
    /// The heading line without the trailing newline.
    pub fn heading(&self) -> String {
        let mut h = format!("## {}{SEPARATOR}{}", self.time.format("%H:%M"), self.actor);
        if let Some(case) = &self.case {
            h.push_str(SEPARATOR);
            h.push_str(case);
        }
        h
    }

    /// Parses a heading line; `None` if it is not an entry heading.
    pub fn parse_heading(line: &str) -> Option<(NaiveTime, String, Option<String>)> {
        let rest = line.trim_end().strip_prefix("## ")?;
        let mut parts = rest.split(SEPARATOR);
        let time = NaiveTime::parse_from_str(parts.next()?, "%H:%M").ok()?;
        let actor = parts.next()?.trim();
        if actor.is_empty() {
            return None;
        }
        let case = match parts.next() {
            Some(c) if is_case_id(c.trim()) => Some(c.trim().to_string()),
            Some(_) => return None,
            None => None,
        };
        if parts.next().is_some() {
            return None;
        }
        Some((time, actor.to_string(), case))
    }
}

impl Journal {
    /// The entries of a journal body, in file order. Text before the first
    /// entry heading is not an entry.
    pub fn entries(body: &str) -> Vec<JournalEntry> {
        let mut entries: Vec<JournalEntry> = Vec::new();
        let mut text = String::new();
        let flush = |entries: &mut Vec<JournalEntry>, text: &mut String| {
            if let Some(last) = entries.last_mut() {
                last.text = text.trim().to_string();
            }
            text.clear();
        };
        for line in body.lines() {
            if let Some((time, actor, case)) = JournalEntry::parse_heading(line) {
                flush(&mut entries, &mut text);
                entries.push(JournalEntry {
                    time,
                    actor,
                    case,
                    text: String::new(),
                });
            } else {
                text.push_str(line);
                text.push('\n');
            }
        }
        flush(&mut entries, &mut text);
        entries
    }

    /// `journal/YYYY/YYYY-MM-DD.md`, relative to the logbook root.
    pub fn relative_path(date: NaiveDate) -> String {
        format!(
            "journal/{}/{}.md",
            date.format("%Y"),
            date.format("%Y-%m-%d")
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entries_from_body() {
        let body = "## 09:25 · agent:claude-code · C-2026-003\nline one\nline two\n\n## 14:40 · human\nfree\n## not an entry\nstill free\n";
        let entries = Journal::entries(body);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].actor, "agent:claude-code");
        assert_eq!(entries[0].case.as_deref(), Some("C-2026-003"));
        assert_eq!(entries[0].text, "line one\nline two");
        assert_eq!(
            entries[0].heading(),
            "## 09:25 · agent:claude-code · C-2026-003"
        );
        assert_eq!(entries[1].case, None);
        assert_eq!(entries[1].text, "free\n## not an entry\nstill free");
    }

    #[test]
    fn relative_path() {
        let d = NaiveDate::from_ymd_opt(2026, 10, 1).unwrap();
        assert_eq!(Journal::relative_path(d), "journal/2026/2026-10-01.md");
    }
}
