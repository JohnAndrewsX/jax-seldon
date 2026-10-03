//! `seldon hook session-start` context block (SPEC-ENGINE §8).

use std::fmt::Write as _;

use super::super::Context;
use crate::error::Result;
use crate::logbook::cases::{self, CaseFile};

/// The context block (SPEC-ENGINE §8): STATUS summary, the active case
/// with its plan steps, the last 5 journal lines, the lessons' headings.
pub fn session_start(ctx: &Context) -> Result<String> {
    let (_, logbook) = ctx.open_logbook()?;
    let mut out = String::from("# Seldon logbook context\n");

    out.push_str("\n## Status\n");
    match std::fs::read_to_string(logbook.path("STATUS.md")) {
        Ok(text) => out.push_str(&status_summary(&text)),
        Err(_) => out.push_str("No STATUS.md yet (`seldon status` writes it).\n"),
    }

    out.push_str("\n## Active case\n");
    match cases::active_case(&logbook).map(|id| cases::find(&logbook, &id)) {
        Some(Ok(file)) => out.push_str(&case_block(&file)),
        Some(Err(e)) => {
            let _ = writeln!(out, "Unreadable: {e}");
        }
        None => out.push_str("None (`seldon plan start <id>` sets one).\n"),
    }

    let today = ctx.now.date_naive().format("%Y-%m-%d").to_string();
    let journal = logbook.journal_files()?.into_iter().rfind(|p| {
        p.file_stem()
            .is_some_and(|s| *s.to_string_lossy() <= *today)
    });
    match journal {
        Some(path) => {
            let rel = cases::relative(&logbook, &path);
            let _ = writeln!(out, "\n## Journal ({rel}, last 5 lines)");
            let text = std::fs::read_to_string(&path)?;
            let body = crate::frontmatter::Document::parse(&text)
                .map(|d| d.body)
                .unwrap_or(text);
            let lines: Vec<&str> = body.lines().filter(|l| !l.trim().is_empty()).collect();
            for line in &lines[lines.len().saturating_sub(5)..] {
                let _ = writeln!(out, "{line}");
            }
        }
        None => out.push_str("\n## Journal\nNo entries yet.\n"),
    }

    out.push_str("\n## Lessons (memory/lessons.md)\n");
    let lessons = std::fs::read_to_string(logbook.path("memory/lessons.md")).unwrap_or_default();
    let headings: Vec<&str> = lessons
        .lines()
        .filter_map(|l| l.strip_prefix("## "))
        .collect();
    if headings.is_empty() {
        out.push_str("None yet.\n");
    }
    for h in headings {
        let _ = writeln!(out, "- {}", h.trim());
    }
    Ok(out)
}

/// The first section of STATUS.md: the lines between its title and the
/// second `## ` heading, without the generated header and blank lines.
fn status_summary(text: &str) -> String {
    let mut out = String::new();
    let mut sections = 0;
    for line in text.lines() {
        if line.starts_with("## ") {
            sections += 1;
            if sections > 1 {
                break;
            }
            continue;
        }
        if line.trim().is_empty() || line.starts_with("# ") || line.starts_with("<!--") {
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

/// `C-… — title (status, zone/risk, done/total steps)` and the plan's
/// checkbox lines.
fn case_block(file: &CaseFile) -> String {
    let c = &file.case;
    let (total, done) = file.steps();
    let mut out = format!(
        "{} — {} ({}, {}/{}, {done}/{total} steps)\n",
        c.id,
        c.title,
        c.status.as_str(),
        c.zone.as_str(),
        c.risk.as_str()
    );
    if let Some(range) = cases::section(&file.doc.body, "Plan") {
        for line in file.doc.body[range].lines() {
            let item = line.trim();
            if item.starts_with("- [") || item.starts_with("* [") {
                let _ = writeln!(out, "{item}");
            }
        }
    }
    out
}
