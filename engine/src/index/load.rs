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

use crate::commands::import::task::{PROVENANCE_END, PROVENANCE_START};
use crate::commands::plan::TAG_IMPORTED;
use crate::frontmatter::{Document, printable};
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
    /// The first paragraph of `## Intent` (after an imported case's
    /// provenance line) and of `## Result`, as written: the build
    /// redacts and clips them (ADR-0038 §2).
    pub intent: Option<String>,
    pub result: Option<String>,
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
    /// `(relative path, decision, the first paragraph of its ##
    /// Decision as written)`, in file order.
    pub decisions: Vec<(String, Decision, Option<String>)>,
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
        if let Some(w) = bad_lines_warning(&month, &file.bad_lines) {
            out.warnings.push(w);
        }
        if out.events.is_empty() {
            // the first month's list moves instead of being copied (WP-092)
            out.events = file.events;
        } else {
            out.events.extend(file.events);
        }
    }

    for path in logbook.case_files()? {
        let rel = cases::relative(logbook, &path);
        match read(&path)
            .and_then(|t| model::parse::<Case>(&t).map_err(|e| format!("invalid case: {e}")))
        {
            Ok((case, doc)) => {
                // a template placeholder in a comment is no plan (rule 3
                // and rule 9 alike, WP-115 round 2)
                let plan = cases::section(&doc.body, "Plan")
                    .map(|r| cases::strip_comments(&doc.body[r]))
                    .unwrap_or_default();
                out.cases.push(LoadedCase {
                    steps: cases::plan_steps(&doc.body),
                    intent: intent(&case, &doc.body),
                    result: cases::first_paragraph(&doc.body, "Result"),
                    path: rel,
                    case,
                    plan,
                });
            }
            Err(e) => out
                .warnings
                .push(format!("{}: {e}; skipped", printable(&rel))),
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

    out.decisions = decisions(logbook, &mut out.warnings)?;

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

/// One warning per month with bad lines: the month, how many, and the
/// first line numbers (F-132); `None` without bad lines.
pub fn bad_lines_warning(month: &str, lines: &[usize]) -> Option<String> {
    const SHOWN: usize = 5;
    if lines.is_empty() {
        return None;
    }
    let shown: Vec<String> = lines.iter().take(SHOWN).map(usize::to_string).collect();
    let more = match lines.len().saturating_sub(SHOWN) {
        0 => String::new(),
        n => format!(" and {n} more"),
    };
    let (count, noun) = match lines.len() {
        1 => (1, "line"),
        n => (n, "lines"),
    };
    Some(format!(
        "ledger/{month}.jsonl: {count} {noun} skipped, not a valid event \
         (not UTF-8, not JSON, or a bad actor or case): line {}{more}",
        shown.join(", ")
    ))
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

/// The first paragraph of a case's `## Intent` (ADR-0038 §2). An imported
/// case's Intent begins with the engine's provenance line (WP-102); when
/// that line is the whole first paragraph, the next one is the intent:
/// the index carries the source in `source`.
fn intent(case: &Case, body: &str) -> Option<String> {
    let section = cases::section(body, "Intent")?;
    // at most two: the provenance line and the intent after it
    let mut paragraphs = cases::paragraphs(&body[section], 2).into_iter();
    let first = paragraphs.next()?;
    let imported = case.tags.iter().any(|t| t == TAG_IMPORTED);
    if imported && is_provenance(&first) {
        return paragraphs.next();
    }
    Some(first)
}

/// `Imported from <source> — read before you start this case.` on one
/// line (`commands::import::task::provenance`).
fn is_provenance(paragraph: &str) -> bool {
    !paragraph.contains('\n')
        && paragraph.starts_with(PROVENANCE_START)
        && paragraph.ends_with(PROVENANCE_END)
}

/// `(relative path, decision, lead)` of every `decisions/ADR-*.md`, in
/// file order, the lead the first paragraph of its `## Decision` (ADR-0038
/// §2); an unreadable or invalid file is a warning and skipped.
pub fn decisions(
    logbook: &Logbook,
    warnings: &mut Vec<String>,
) -> anyhow::Result<Vec<(String, Decision, Option<String>)>> {
    let mut out = Vec::new();
    for path in logbook.decision_files()? {
        let rel = cases::relative(logbook, &path);
        match read(&path).and_then(|t| {
            model::parse::<Decision>(&t).map_err(|e| format!("invalid decision: {e}"))
        }) {
            Ok((d, doc)) => {
                let lead = cases::first_paragraph(&doc.body, "Decision");
                out.push((rel, d, lead));
            }
            Err(e) => warnings.push(format!("{rel}: {e}; skipped")),
        }
    }
    Ok(out)
}

pub const FENCE_BEGIN: &str = "<!-- seldon:begin ";
pub const FENCE_END: &str = "<!-- seldon:end -->";

/// Generated fences `<!-- seldon:begin NAME -->\n…<!-- seldon:end -->` in
/// file order (SPEC-LOGBOOK §3); the content excludes the marker lines.
/// A `\r\n` marker line is a marker line too (WP-065 review).
pub fn fences(text: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find(FENCE_BEGIN) {
        let after = &rest[start + FENCE_BEGIN.len()..];
        // the marker line ends in `\n` or `\r\n` (a CRLF dossier file)
        let Some((name, content)) = after.match_indices(" -->").find_map(|(i, _)| {
            let rest = &after[i + " -->".len()..];
            let content = rest
                .strip_prefix('\n')
                .or_else(|| rest.strip_prefix("\r\n"))?;
            Some((&after[..i], content))
        }) else {
            break;
        };
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

/// Whether `text` has a Markdown table separator row: `|---|---|` as the
/// engine writes it, and the forms editors write (`| --- | --- |`,
/// `|:---|---:|`, Obsidian's table editor). Every cell is three or more
/// `-` with an optional `:` on either side.
pub fn has_table_separator(text: &str) -> bool {
    text.lines().any(|line| {
        let t = line.trim();
        let Some(inner) = t.strip_prefix('|') else {
            return false;
        };
        let inner = inner.strip_suffix('|').unwrap_or(inner);
        !inner.trim().is_empty()
            && inner.split('|').all(|cell| {
                let c = cell.trim();
                let dashes = c.strip_prefix(':').unwrap_or(c);
                let dashes = dashes.strip_suffix(':').unwrap_or(dashes);
                dashes.len() >= 3 && dashes.chars().all(|ch| ch == '-')
            })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_separators_in_every_editor_form() {
        for sep in [
            "|---|---|",
            "| --- | --- |",
            "|:---|---:|",
            "| :---: | ---- |",
            "  |---|  ",
        ] {
            assert!(
                has_table_separator(&format!("| a | b |\n{sep}\n| 1 | 2 |\n")),
                "{sep}"
            );
        }
        for not in ["| a | b |", "| -- | -- |", "|---|x|", "||", "---", "| - |"] {
            assert!(!has_table_separator(not), "{not}");
        }
    }

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

    /// WP-065 review: a CRLF dossier file has the same fences.
    #[test]
    fn crlf_fences_are_fences() {
        let lf = "# x\n<!-- seldon:begin omarchy.summary -->\n- theme: tokyo-night\n<!-- seldon:end -->\n<!-- seldon:begin a --> no\n<!-- seldon:begin packages.history -->\n| date | total |\n|---|---|\n| 2026-09-01 | 2004 |\n<!-- seldon:end -->\n";
        let crlf = lf.replace('\n', "\r\n");
        let (a, b) = (fences(lf), fences(&crlf));
        let names = |f: &[(String, String)]| f.iter().map(|(n, _)| n.clone()).collect::<Vec<_>>();
        assert_eq!(names(&a), ["omarchy.summary", "packages.history"]);
        assert_eq!(names(&b), names(&a));
        assert_eq!(fence_kv(&b[0].1)["theme"], "tokyo-night");
        assert_eq!(fence_table(&b[1].1), fence_table(&a[1].1));
        assert_eq!(fence_table(&b[1].1)[0]["total"], "2004");
    }

    /// F-132: the month, the count, five line numbers, then how many more.
    #[test]
    fn bad_lines_warning_names_month_and_count() {
        assert_eq!(bad_lines_warning("2026-10", &[]), None);
        let one = bad_lines_warning("2026-10", &[2]).unwrap();
        assert!(
            one.starts_with("ledger/2026-10.jsonl: 1 line skipped, ") && one.ends_with(": line 2"),
            "{one}"
        );
        let seven = bad_lines_warning("2026-09", &[1, 3, 5, 7, 9, 11, 13]).unwrap();
        assert!(
            seven.starts_with("ledger/2026-09.jsonl: 7 lines skipped, "),
            "{seven}"
        );
        assert!(
            seven.ends_with(": line 1, 3, 5, 7, 9 and 2 more"),
            "{seven}"
        );
        assert!(!seven.contains("11"), "{seven}");
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
