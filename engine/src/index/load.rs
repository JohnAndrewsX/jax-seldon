//! Reading the logbook for the index (SPEC-ENGINE §6): every
//! `ledger/*.jsonl`, every case file, the journal of today and yesterday,
//! `decisions/*.md` frontmatter, the generated fences of `system/*.md`,
//! `memory/*.md` frontmatter and lesson headings, and `areas/*/`.
//!
//! The index is a view: a file that cannot be read (a hand-broken case, a
//! torn ledger line) is skipped with a warning instead of failing the
//! whole build, so the plugin keeps a useful picture of the rest.

use std::collections::BTreeMap;
use std::path::Path;

use chrono::{Duration, NaiveDate};

use crate::frontmatter::Document;
use crate::ledger::Ledger;
use crate::logbook::{Logbook, cases};
use crate::model::event::{Event, is_actor};
use crate::model::{self, Case, Decision, Journal, Memory};
use crate::redact::Redactor;

/// A case file as the index needs it.
#[derive(Debug, Clone)]
pub struct LoadedCase {
    /// Relative to the logbook root, `/`-separated.
    pub path: String,
    pub case: Case,
    /// The text of the `## Plan` section (empty without one).
    pub plan: String,
    /// `## Plan` checkboxes: (total, done).
    pub steps: (usize, usize),
}

/// One journal entry heading `## HH:MM · actor · case?` and its text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub time: String,
    pub actor: String,
    pub case: Option<String>,
    pub text: String,
}

/// A memory file other than `lessons.md`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Topic {
    pub topic: String,
    pub updated: Option<String>,
    pub path: String,
}

/// Everything the index derives from, read once.
#[derive(Debug, Clone, Default)]
pub struct Loaded {
    /// Every ledger event, months ascending, file order within a month.
    pub events: Vec<Event>,
    /// Cases sorted by id.
    pub cases: Vec<LoadedCase>,
    /// The day the journal was read for ("today").
    pub today: Option<NaiveDate>,
    pub journal_today: Vec<Entry>,
    pub journal_yesterday: Vec<Entry>,
    /// `(relative path, decision)`, in file order.
    pub decisions: Vec<(String, Decision)>,
    /// `<!-- seldon:begin NAME -->` fences of `system/*.md` by name.
    pub fences: BTreeMap<String, String>,
    /// `## ` headings of `memory/lessons.md`; `None` without the file.
    pub lessons: Option<Vec<String>>,
    pub topics: Vec<Topic>,
    /// `(area, has AGENTS.md)`, sorted.
    pub areas: Vec<(String, bool)>,
    /// What could not be read, one line each (English).
    pub warnings: Vec<String>,
}

/// Reads the logbook; `today` picks the journal days.
pub fn load(logbook: &Logbook, today: NaiveDate) -> anyhow::Result<Loaded> {
    let mut out = Loaded {
        today: Some(today),
        ..Loaded::default()
    };

    let ledger = Ledger::new(logbook, Redactor::builtin());
    for month in ledger.months()? {
        let file = ledger.read_month(&month)?;
        for n in file.bad_lines {
            out.warnings.push(format!(
                "ledger/{month}.jsonl:{n}: not a valid event, skipped"
            ));
        }
        out.events.extend(file.events);
    }

    for path in logbook.case_files()? {
        let rel = cases::relative(logbook, &path);
        match read(&path)
            .and_then(|t| model::parse::<Case>(&t).map_err(|e| format!("invalid case: {e}")))
        {
            Ok((case, doc)) => {
                let plan = cases::section(&doc.body, "Plan")
                    .map(|r| doc.body[r].to_string())
                    .unwrap_or_default();
                out.cases.push(LoadedCase {
                    steps: cases::plan_steps(&doc.body),
                    path: rel,
                    case,
                    plan,
                });
            }
            Err(e) => out.warnings.push(format!("{rel}: {e}; skipped")),
        }
    }
    out.cases.sort_by(|a, b| a.case.id.cmp(&b.case.id));

    for (day, entries) in [
        (today, &mut out.journal_today),
        (today - Duration::days(1), &mut out.journal_yesterday),
    ] {
        let rel = Journal::relative_path(day);
        let path = logbook.path(&rel);
        if !path.is_file() {
            continue;
        }
        match read(&path) {
            Ok(text) => *entries = journal_entries(&text),
            Err(e) => out.warnings.push(format!("{rel}: {e}")),
        }
    }

    for path in logbook.decision_files()? {
        let rel = cases::relative(logbook, &path);
        match read(&path).and_then(|t| {
            model::parse::<Decision>(&t).map_err(|e| format!("invalid decision: {e}"))
        }) {
            Ok((d, _)) => out.decisions.push((rel, d)),
            Err(e) => out.warnings.push(format!("{rel}: {e}; skipped")),
        }
    }

    for path in md_files(&logbook.path("system")) {
        match read(&path) {
            Ok(text) => {
                for (name, body) in fences(&text) {
                    out.fences.entry(name).or_insert(body);
                }
            }
            Err(e) => out
                .warnings
                .push(format!("{}: {e}; skipped", cases::relative(logbook, &path))),
        }
    }

    for path in logbook.memory_files()? {
        let rel = cases::relative(logbook, &path);
        let text = match read(&path) {
            Ok(t) => t,
            Err(e) => {
                out.warnings.push(format!("{rel}: {e}; skipped"));
                continue;
            }
        };
        if rel == "memory/lessons.md" {
            let body = Document::parse(&text).map_or(text.clone(), |d| d.body);
            out.lessons = Some(
                body.lines()
                    .filter_map(|l| l.strip_prefix("## "))
                    .map(|h| h.trim().to_string())
                    .collect(),
            );
            continue;
        }
        match model::parse::<Memory>(&text) {
            Ok((m, _)) => out.topics.push(Topic {
                topic: m.topic,
                updated: m.updated.map(|d| d.to_string()),
                path: rel,
            }),
            Err(e) => out
                .warnings
                .push(format!("{rel}: invalid memory file: {e}")),
        }
    }

    for readme in logbook.area_files()? {
        let dir = readme.parent().expect("areas/<name>/README.md");
        let name = dir
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        out.areas.push((name, dir.join("AGENTS.md").is_file()));
    }
    Ok(out)
}

fn read(path: &Path) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| format!("cannot read: {e}"))
}

/// `*.md` directly in `dir`, sorted.
fn md_files(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut out: Vec<_> = std::fs::read_dir(dir)
        .map(|r| {
            r.filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| p.is_file() && p.extension().is_some_and(|e| e == "md"))
                .collect()
        })
        .unwrap_or_default();
    out.sort();
    out
}

/// Journal entries with a valid actor (`event.schema.json#/$defs/actor`);
/// any other `## ` line is text of the entry before it.
pub fn journal_entries(text: &str) -> Vec<Entry> {
    let body = Document::parse(text).map_or_else(|_| text.to_string(), |d| d.body);
    Journal::entries(&body)
        .into_iter()
        .filter(|e| is_actor(&e.actor))
        .map(|e| Entry {
            time: e.time.format("%H:%M").to_string(),
            actor: e.actor,
            case: e.case,
            text: e.text,
        })
        .collect()
}

pub const FENCE_BEGIN: &str = "<!-- seldon:begin ";
pub const FENCE_END: &str = "<!-- seldon:end -->";

/// Generated fences `<!-- seldon:begin NAME -->\n…<!-- seldon:end -->` in
/// file order (SPEC-LOGBOOK §3); the content excludes the marker lines.
pub fn fences(text: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find(FENCE_BEGIN) {
        let after = &rest[start + FENCE_BEGIN.len()..];
        let Some(close) = after.find(" -->\n") else {
            break;
        };
        let name = &after[..close];
        let content = &after[close + " -->\n".len()..];
        if name.is_empty()
            || !name
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '.' || c == '-')
        {
            rest = after;
            continue;
        }
        let Some(end) = content.find(FENCE_END) else {
            break;
        };
        out.push((name.to_string(), content[..end].to_string()));
        rest = &content[end + FENCE_END.len()..];
    }
    out
}

/// `- key: value` lines of a fence.
pub fn fence_kv(text: &str) -> BTreeMap<String, String> {
    text.lines()
        .filter_map(|l| l.strip_prefix("- "))
        .filter_map(|l| l.split_once(": "))
        .filter(|(k, _)| {
            k.starts_with(|c: char| c.is_ascii_alphabetic())
                && k.chars().all(|c| c.is_ascii_alphanumeric())
        })
        .map(|(k, v)| (k.to_string(), v.trim().to_string()))
        .collect()
}

/// The rows of a Markdown table in a fence, keyed by the header cells.
pub fn fence_table(text: &str) -> Vec<BTreeMap<String, String>> {
    let rows: Vec<Vec<String>> = text
        .lines()
        .filter(|l| l.starts_with('|'))
        .map(|l| {
            l.trim()
                .trim_matches('|')
                .split('|')
                .map(|c| c.trim().to_string())
                .collect()
        })
        .collect();
    if rows.len() < 2 {
        return Vec::new();
    }
    rows[2..]
        .iter()
        .map(|r| rows[0].iter().cloned().zip(r.iter().cloned()).collect())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fences_kv_and_tables() {
        let text = "# x\n\n<!-- seldon:begin omarchy.summary -->\n- version: 4.0.7-1\n- theme: tokyo-night\n<!-- seldon:end -->\nfree\n<!-- seldon:begin packages.history -->\n| date | explicit | total |\n|---|---|---|\n| 2026-09-01 | 323 | 2004 |\n<!-- seldon:end -->\n<!-- seldon:begin empty -->\n<!-- seldon:end -->\n";
        let f = fences(text);
        assert_eq!(f.len(), 3);
        assert_eq!(f[0].0, "omarchy.summary");
        assert_eq!(fence_kv(&f[0].1)["theme"], "tokyo-night");
        let rows = fence_table(&f[1].1);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["explicit"], "323");
        assert_eq!(f[2].1, "");
        assert!(fence_table("").is_empty());
    }

    #[test]
    fn journal_entries_skip_invalid_actors() {
        let text = "---\ntype: journal\ndate: 2026-10-01\ncases: []\n---\n## 09:25 · agent:claude-code · C-2026-003\nA\n\n## 10:00 · Robot\nB\n## 11:00 · human\nC\n";
        let e = journal_entries(text);
        assert_eq!(e.len(), 2);
        assert_eq!(e[0].time, "09:25");
        assert_eq!(e[0].case.as_deref(), Some("C-2026-003"));
        assert_eq!(e[1].actor, "human");
    }
}
