//! `outputs/IMPORT-<source>.md`: what an import maps, renumbers, skips and
//! redacts (WP-043). English headings and engine prose (ADR-0007); kit
//! text appears only as file names and deviation paths. The body depends on
//! the plan and the mode only, so an unchanged vault renders the same bytes.

use std::fmt::Write as _;

use serde_json::{Value, json};

use super::cell;
use super::omarchy_agent::{Plan, SOURCE};
use crate::index::views;

/// The fence the report lives in; text outside it is the user's.
pub const FENCE: &str = "import-omarchy-agent";

/// Which run wrote the report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mode {
    DryRun,
    /// `--apply` at this time (`YYYY-MM-DD HH:MM`).
    Applied(String),
}

/// The counts of the report and of `--json`.
pub fn counts(plan: &Plan) -> Value {
    json!({
        "cases": plan.cases.len(),
        "renumbered": plan.collisions().count(),
        "journalSessions": plan.sessions,
        "journalDays": plan.days.len(),
        "memorySections": plan.memory_sections(),
        "memoryFiles": plan.memory.len(),
        "deviationRows": plan.rows.len(),
        "notImported": plan.skipped.iter().filter(|s| !s.error).count(),
        "errors": plan.errors(),
        "redactedLines": plan.hits.len(),
        "privatePaths": plan.private_paths,
        "rewrittenLinks": plan.rewrite_totals().0,
        "rewrittenIds": plan.rewrite_totals().1,
        "assumptions": plan.cases.iter().filter(|c| c.assumption.is_some()).count(),
    })
}

/// The report's fence body.
pub fn render(plan: &Plan, mode: &Mode) -> String {
    let mut t = String::new();
    let _ = writeln!(t, "# Import from {SOURCE}\n");
    let _ = writeln!(t, "- Vault: `{}`", plan.vault);
    match mode {
        Mode::DryRun => {
            let _ = writeln!(
                t,
                "- Mode: dry run. Nothing was imported; this report is the only file written. Apply with `seldon import {SOURCE} <vault> --apply`."
            );
        }
        Mode::Applied(at) => {
            let _ = writeln!(
                t,
                "- Mode: applied {at}. The files below were written in one commit; a second apply changes nothing."
            );
        }
    }
    let _ = writeln!(t, "- Errors: {}", plan.errors());

    t.push_str("\n## Counts\n\n");
    let mut by_status: Vec<(String, usize)> = Vec::new();
    for c in &plan.cases {
        let s = c.case.status.to_string();
        match by_status.iter_mut().find(|(k, _)| *k == s) {
            Some((_, n)) => *n += 1,
            None => by_status.push((s, 1)),
        }
    }
    by_status.sort();
    let _ = writeln!(
        t,
        "- cases: {}{}, renumbered {}",
        plan.cases.len(),
        if by_status.is_empty() {
            String::new()
        } else {
            format!(
                " ({})",
                by_status
                    .iter()
                    .map(|(s, n)| format!("{s} {n}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        },
        plan.collisions().count()
    );
    let _ = writeln!(
        t,
        "- journal: {} session(s) on {} day(s), {} of them appended to an existing day",
        plan.sessions,
        plan.days.len(),
        plan.days.iter().filter(|d| d.exists).count()
    );
    let _ = writeln!(
        t,
        "- memory: {} section(s) in {} file(s), {} of them new",
        plan.memory_sections(),
        plan.memory.len(),
        plan.memory.iter().filter(|m| !m.exists).count()
    );
    let _ = writeln!(t, "- deviations: {} row(s)", plan.rows.len());
    let _ = writeln!(
        t,
        "- not imported: {}; errors: {}",
        plan.skipped.iter().filter(|s| !s.error).count(),
        plan.errors()
    );
    let rules = plan
        .by_rule
        .iter()
        .map(|(r, n)| format!("{r} {n}"))
        .collect::<Vec<_>>();
    let _ = writeln!(
        t,
        "- redaction: {} line(s){}; {} private path(s) rewritten to `~`",
        plan.hits.len(),
        if rules.is_empty() {
            String::new()
        } else {
            format!(" ({})", rules.join(", "))
        },
        plan.private_paths
    );

    let (links, ids) = plan.rewrite_totals();
    let _ = writeln!(
        t,
        "- id rewrites: {links} wikilink(s) and {ids} bare id(s) in {} file(s)",
        plan.rewrites.len()
    );
    let _ = writeln!(
        t,
        "- assumptions: {}",
        plan.cases.iter().filter(|c| c.assumption.is_some()).count()
    );

    t.push_str("\n## Cases\n\n");
    if plan.cases.is_empty() {
        t.push_str("None.\n");
    } else {
        t.push_str("| kit id | id here | status | file | source |\n|---|---|---|---|---|\n");
        for c in &plan.cases {
            let _ = writeln!(
                t,
                "| {} | {} | {} → {} | {} | {} |",
                c.from,
                c.to,
                cell(&c.kit_status),
                c.case.status,
                cell(&c.path),
                cell(&c.source)
            );
        }
    }

    t.push_str("\n## Collisions\n\n");
    if plan.collisions().count() == 0 {
        t.push_str("None: every kit id was free.\n");
    } else {
        t.push_str("| kit id | already used by | new id |\n|---|---|---|\n");
        for c in plan.collisions() {
            let _ = writeln!(
                t,
                "| {} | {} | {} |",
                c.from,
                cell(c.collision.as_deref().unwrap_or_default()),
                c.to
            );
        }
    }

    t.push_str("\n## Id rewrites\n\n");
    if plan.rewrites.is_empty() {
        t.push_str("None: no imported text names a renumbered id.\n");
    } else {
        t.push_str(
            "Imported text names renumbered cases by their new id (`[[C-OLD…` and bare `C-OLD` → `C-NEW`); the old id stays in the tag `omarchy-agent/C-OLD`, the line under the title and `meta.originalId`.\n\n| file | wikilinks | bare ids |\n|---|---|---|\n",
        );
        for (file, (links, ids)) in &plan.rewrites {
            let _ = writeln!(t, "| {} | {links} | {ids} |", cell(file));
        }
    }

    t.push_str("\n## Assumptions\n\n");
    if plan.cases.iter().all(|c| c.assumption.is_none()) {
        t.push_str("None.\n");
    } else {
        t.push_str("| id here | source | assumption |\n|---|---|---|\n");
        for c in &plan.cases {
            if let Some(a) = &c.assumption {
                let _ = writeln!(t, "| {} | {} | {} |", c.to, cell(&c.source), cell(a));
            }
        }
    }

    t.push_str("\n## Journal\n\n");
    if plan.days.is_empty() {
        t.push_str("None.\n");
    } else {
        t.push_str("| day | sessions | file |\n|---|---|---|\n");
        for d in &plan.days {
            let _ = writeln!(
                t,
                "| {} | {} | {}{} |",
                d.date,
                d.sessions,
                d.path,
                if d.exists { " (appended)" } else { "" }
            );
        }
    }

    t.push_str("\n## Memory\n\n");
    if plan.memory.is_empty() {
        t.push_str("None.\n");
    } else {
        t.push_str("| file | sections | sources |\n|---|---|---|\n");
        for m in &plan.memory {
            let _ = writeln!(
                t,
                "| {}{} | {} | {} |",
                m.path,
                if m.exists { " (appended)" } else { "" },
                m.sources.len(),
                m.sources
                    .iter()
                    .map(|s| format!("`{}`", cell(s)))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
    }

    t.push_str("\n## Deviations\n\n");
    if plan.rows.is_empty() {
        t.push_str("None.\n");
    } else {
        t.push_str(
            "New user rows in `system/deviations.md` (`deviations.table`, no case):\n\n| path | date | kit entry |\n|---|---|---|\n",
        );
        for r in &plan.rows {
            let date = if r.date.is_empty() { "—" } else { &r.date };
            let _ = writeln!(t, "| {} | {date} | {} |", cell(&r.path), cell(&r.entry));
        }
    }

    t.push_str("\n## Not imported\n\n");
    if plan.skipped.is_empty() {
        t.push_str("Nothing.\n");
    } else {
        t.push_str("| file | reason |\n|---|---|\n");
        for s in &plan.skipped {
            let _ = writeln!(
                t,
                "| {} | {}{} |",
                cell(&s.path),
                if s.error { "**error:** " } else { "" },
                cell(&s.reason)
            );
        }
    }

    t.push_str("\n## Redaction\n\n");
    if plan.hits.is_empty() {
        t.push_str("No line matched a redaction rule.\n");
    } else {
        t.push_str("Each line was written with the match replaced by `‹redacted›`.\n\n| file | line | rule |\n|---|---|---|\n");
        for h in &plan.hits {
            let _ = writeln!(t, "| {} | {} | {} |", cell(&h.file), h.line, h.rule);
        }
    }
    t
}

/// The report file's text: `content` in the fence, text outside kept.
pub fn merge(existing: Option<&str>, content: &str) -> String {
    views::merge_fence(existing, FENCE, content)
}
