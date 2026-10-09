//! Generated Markdown: the month views `ledger/YYYY-MM.md` (SPEC-LOGBOOK
//! §5) and `STATUS.md`. Both start with [`GENERATED_HEADER`]; headings and
//! keys are English, prose follows the logbook language (WP-002 review).
//! The `decisions.index` fence of the user's `DECISIONS.md` is filled here
//! too (WP-050); that file has no header, only the fence is the engine's.
//!
//! A view is written only when its text changed, so a rebuild does not
//! touch files (or git) for nothing.

use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;

use ulid::Ulid;

use super::build::{Built, day};
use super::load::{FENCE_BEGIN, FENCE_END, has_table_separator};
use super::model::{DecisionRow, DriftItem, IndexCase, IndexEvent};
use crate::GENERATED_HEADER;
use crate::logbook::Logbook;
use crate::model::Language;
use crate::model::event::{Event, Kind, Source};
use crate::sys;

/// Name of the generated fence in `STATUS.md`.
pub const STATUS_FENCE: &str = "status";

/// Name of the generated fence in the logbook's `DECISIONS.md` (WP-050).
pub const DECISIONS_FENCE: &str = "decisions.index";

/// Writes `ledger/<month>.md` for every month with events; returns the
/// relative paths that changed. A `ledger/` that is a symbolic link or no
/// directory is refused (exit 1, WP-168).
pub fn write_ledger_views(logbook: &Logbook, built: &Built) -> crate::error::Result<Vec<String>> {
    let mut written = Vec::new();
    for (month, text) in ledger_views(built) {
        let rel = format!("ledger/{month}.md");
        if write_if_changed(logbook, &rel, &text, Durable::No)? {
            written.push(rel);
        }
    }
    Ok(written)
}

/// The text of every month view, by month.
pub fn ledger_views(built: &Built) -> BTreeMap<String, String> {
    let folded: HashMap<Ulid, &IndexEvent> = built.folded.iter().map(|f| (f.event.id, f)).collect();
    let mut months: BTreeMap<String, Vec<&Event>> = BTreeMap::new();
    for e in &built.ledger {
        months.entry(e.month()).or_default().push(e);
    }
    months
        .into_iter()
        .map(|(month, mut events)| {
            // stable: events of the same second keep their ledger order
            events.sort_by_key(|e| (day(e), e.ts));
            let mut text = format!("{GENERATED_HEADER}\n# Ledger {month}\n");
            let mut current = None;
            for e in events {
                if current != Some(day(e)) {
                    current = Some(day(e));
                    let _ = write!(text, "\n## {}\n\n", day(e));
                }
                text.push_str(&ledger_line(e, folded.get(&e.id).copied(), built));
                text.push('\n');
            }
            (month, text)
        })
        .collect()
}

/// `- HH:MM source · kind `subject` detail · [[case]] · actor · state`
/// (the format of `fixtures/logbook/ledger/*.md`); a pacman line of a
/// transaction that did not complete adds `transaction <status>` after
/// its detail (ADR-0043). `system` is not named;
/// case events do not link the case they are about; a resolution shows
/// how it resolved; a resolved event shows how, an open drift event
/// `**drift**`.
fn ledger_line(e: &Event, folded: Option<&IndexEvent>, built: &Built) -> String {
    let event = folded.map_or(e, |f| &f.event);
    let mut line = format!(
        "- {} {} · {} `{}`",
        e.ts.format("%H:%M"),
        e.source,
        e.kind,
        e.subject
    );
    if e.kind == Kind::Resolution {
        if let Some(r) = e.resolution {
            let _ = write!(line, " {r}");
        }
    } else if shows_detail(e.kind)
        && let Some(d) = &e.detail
    {
        let _ = write!(line, " {}", one_line(d));
    }
    if let Some(status) = e.meta.tx_status
        && e.source == Source::Pacman
        && e.tx_id.is_some()
    {
        let _ = write!(line, " · transaction {status}");
    }
    if let Some(case) = &event.case
        && !is_case_kind(e.kind)
    {
        let _ = write!(line, " · [[{case}]]");
    }
    if e.actor != "system" {
        let _ = write!(line, " · {}", e.actor);
    }
    if e.kind != Kind::Resolution {
        if let Some(r) = event.resolution {
            let _ = write!(line, " · {r}");
        } else if built.open_drift.contains(&e.id) {
            line.push_str(" · **drift**");
        }
    }
    line
}

fn is_case_kind(kind: Kind) -> bool {
    matches!(
        kind,
        Kind::CaseCreated
            | Kind::CaseStarted
            | Kind::CaseVerified
            | Kind::CaseCompleted
            | Kind::CaseDropped
            | Kind::CaseUpdated
    )
}

/// Whether the month view shows `detail`: versions, hashes, snapshot
/// descriptions, what `plan set` changed; not note texts, command lines,
/// reasons or case titles.
fn shows_detail(kind: Kind) -> bool {
    !matches!(
        kind,
        Kind::Note | Kind::Correction | Kind::Command | Kind::Resolution
    ) && (!is_case_kind(kind) || kind == Kind::CaseUpdated)
}

fn one_line(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Prose of `STATUS.md` in the logbook language.
struct Words {
    as_of: &'static str,
    last_event: &'static str,
    active: &'static str,
    verification: &'static str,
    queued: &'static str,
    open_drift: &'static str,
    crises: &'static str,
    events_today: &'static str,
    last_7_days: &'static str,
    steps: &'static str,
    crisis: &'static str,
    proposed: &'static str,
    group: &'static str,
    none: &'static str,
    degraded: &'static str,
}

const EN: Words = Words {
    as_of: "As of",
    last_event: "last event",
    active: "Active cases",
    verification: "in verification",
    queued: "queued",
    open_drift: "Open drift",
    crises: "crises",
    events_today: "Events today",
    last_7_days: "last 7 days",
    steps: "steps",
    crisis: "**Crisis**",
    proposed: "proposed",
    group: "events in this transaction",
    none: "none",
    degraded: "degraded",
};

const DE: Words = Words {
    as_of: "Stand",
    last_event: "letztes Ereignis",
    active: "Aktive Cases",
    verification: "in Prüfung",
    queued: "geplant",
    open_drift: "Offene Drift",
    crises: "davon Krise",
    events_today: "Ereignisse heute",
    last_7_days: "letzte 7 Tage",
    steps: "Schritte",
    crisis: "**Krise**",
    proposed: "Vorschlag",
    group: "Ereignisse in dieser Transaktion",
    none: "keine",
    degraded: "eingeschränkt",
};

fn words(language: Language) -> &'static Words {
    match language {
        Language::En => &EN,
        Language::De => &DE,
    }
}

/// The content of the `status` fence of `STATUS.md`.
pub fn status_text(built: &Built, language: Language) -> String {
    let w = words(language);
    let ix = &built.index;
    let s = &ix.summary;
    let mut t = format!("# Status — {}\n\n", ix.logbook.machine);

    // the day and the last event, not the build time: an unchanged
    // logbook keeps an unchanged STATUS.md (no commit per `seldon status`)
    let _ = write!(t, "{}: {}", w.as_of, ix.today.date);
    if let Some(last) = built.folded.first() {
        let ts = crate::model::event::format_ts(&last.event.ts);
        let when = if ts.starts_with(&ix.today.date) {
            ts.get(11..16).unwrap_or("").to_string()
        } else {
            ts.get(..16).unwrap_or("").replacen('T', " ", 1)
        };
        let _ = write!(t, " · {} {when}", w.last_event);
    }
    if let Some(om) = &ix.system.omarchy {
        if let Some(v) = &om.version {
            let _ = write!(t, " · Omarchy {v}");
        }
        if let Some(theme) = &om.theme {
            let _ = write!(t, " · Theme {theme}");
        }
    }
    t.push_str("\n\n## Overview\n");
    let _ = writeln!(
        t,
        "- {}: {} · {}: {} · {}: {}",
        w.active,
        s.active_cases,
        w.verification,
        ix.cases.verification.len(),
        w.queued,
        s.queued_cases
    );
    let _ = writeln!(
        t,
        "- {}: {}, {}: {}",
        w.open_drift, s.open_drift, w.crises, s.crisis
    );
    let _ = writeln!(
        t,
        "- {}: {} · {}: {}",
        w.events_today, s.events_today, w.last_7_days, s.events_7d
    );

    for (heading, cases) in [
        ("Active cases", &ix.cases.active),
        ("In verification", &ix.cases.verification),
        ("Queued", &ix.cases.queued),
    ] {
        if cases.is_empty() {
            continue;
        }
        let _ = write!(t, "\n## {heading}\n");
        for c in cases {
            t.push_str(&case_line(c, w));
        }
    }

    t.push_str("\n## Open drift\n");
    if ix.drift.is_empty() {
        let _ = writeln!(t, "- {}", w.none);
    }
    let today = &ix.today.date;
    for d in &ix.drift {
        t.push_str(&drift_line(d, today, w));
    }

    let degraded: Vec<_> = ix.state.collectors.iter().filter(|c| !c.ok).collect();
    if !degraded.is_empty() {
        t.push_str("\n## Collectors\n");
        for c in degraded {
            let _ = write!(t, "- {}: {}", c.name, w.degraded);
            if let Some(m) = &c.message {
                let _ = write!(t, " — {}", one_line(m));
            }
            t.push('\n');
        }
    }
    t
}

fn case_line(c: &IndexCase, w: &Words) -> String {
    let mut line = format!("- [[{}]] {} — {}/{}", c.id, c.title, c.zone, c.risk);
    if c.steps.total > 0 {
        let _ = write!(line, " — {}/{} {}", c.steps.done, c.steps.total, w.steps);
    }
    if !c.agents.is_empty() {
        let _ = write!(line, " — {}", c.agents.join(", "));
    }
    line.push('\n');
    line
}

fn drift_line(d: &DriftItem, today: &str, w: &Words) -> String {
    let mut line = String::from("- ");
    if d.crisis {
        let _ = write!(line, "{} ", w.crisis);
    }
    let date = d.ts.get(..10).unwrap_or("");
    let time = d.ts.get(11..16).unwrap_or("");
    if date != today {
        let _ = write!(line, "{date} ");
    }
    let _ = write!(line, "{time} {} · {} `{}`", d.source, d.kind, d.subject);
    if d.source != Source::Config.as_str()
        && let Some(detail) = &d.detail
    {
        let _ = write!(line, " {}", one_line(detail));
    }
    if d.actor != "system" {
        let _ = write!(line, " · {}", d.actor);
    }
    if let Some(n) = d.members {
        let _ = write!(line, " · {n} {}", w.group);
    }
    if let Some(case) = &d.proposed_case {
        let _ = write!(line, " · {} [[{case}]]", w.proposed);
    }
    line.push('\n');
    line
}

/// `STATUS.md` with the `status` fence replaced by `content`
/// ([`try_merge_fence`]); the `Err` says why the file must stay as it is.
/// The `init` template is the one header file without the fence that is
/// replaced.
pub fn merge_status(existing: Option<&str>, content: &str) -> Result<String, String> {
    try_merge_fence(existing, STATUS_FENCE, content, is_status_template)
}

/// Whether `text` is the `STATUS.md` that `init` wrote (any language, any
/// machine id): fully generated, so `status` may replace it.
fn is_status_template(text: &str) -> bool {
    let template = crate::logbook::templates::find("STATUS.md").expect("a built-in template");
    Language::ALL.iter().any(|&language| {
        template
            .text(language)
            .split_once("{{machineId}}")
            .and_then(|(pre, post)| text.strip_prefix(pre)?.strip_suffix(post))
            .is_some_and(|id| !id.contains('\n'))
    })
}

/// The start of every fence marker. Text written into a fence has it
/// broken by a zero-width space ([`neutralise`]).
const MARKER: &str = "<!-- seldon:";

/// `text` with every `<!-- seldon:` broken into `<!--`, U+200B, ` seldon:`
/// (F-130), so a value from the logbook (a case or decision title, a drift
/// subject, a collector message) can neither end its fence nor open one.
/// It renders the same: inside a code span the zero-width space is not
/// seen, outside one the text is still an HTML comment. A broken marker
/// stays as it is.
pub fn neutralise(text: &str) -> Cow<'_, str> {
    if text.contains(MARKER) {
        Cow::Owned(text.replace(MARKER, "<!--\u{200B} seldon:"))
    } else {
        Cow::Borrowed(text)
    }
}

/// `text` with the body of its first fence `name` replaced by `content`
/// (which ends in a newline, or is empty); the marker lines and every
/// byte outside them stay as they are. In a file with `\r\n` line ends
/// (the begin marker's line) `content` gets them too. `None` when `text`
/// has no complete fence `name`.
pub fn replace_fence(text: &str, name: &str, content: &str) -> Option<String> {
    let (start, len, crlf) = fence_span(text, name)?;
    let content = if crlf {
        Cow::Owned(content.replace('\n', "\r\n"))
    } else {
        Cow::Borrowed(content)
    };
    Some(format!(
        "{}{content}{}",
        &text[..start],
        &text[start + len..]
    ))
}

/// The body of the first fence `name` in `text`, found exactly the way
/// [`replace_fence`] finds it, so "has the fence" and "can replace it"
/// never disagree (a damaged marker elsewhere in the file cannot hide it).
pub fn fence_body<'a>(text: &'a str, name: &str) -> Option<&'a str> {
    let (start, len, _) = fence_span(text, name)?;
    Some(&text[start..start + len])
}

/// `(start, length, crlf)` of the body of the first fence `name`: the
/// first begin marker that ends its line (`\n` or `\r\n`), up to the next
/// end marker.
fn fence_span(text: &str, name: &str) -> Option<(usize, usize, bool)> {
    let begin = format!("{FENCE_BEGIN}{name} -->");
    let (start, crlf) = text.match_indices(&begin).find_map(|(at, _)| {
        let rest = &text[at + begin.len()..];
        if rest.starts_with('\n') {
            Some((at + begin.len() + 1, false))
        } else if rest.starts_with("\r\n") {
            Some((at + begin.len() + 2, true))
        } else {
            None
        }
    })?;
    let len = text[start..].find(FENCE_END)?;
    Some((start, len, crlf))
}

/// Whether `text` has a begin marker of fence `name` whose body cannot be
/// told: no end marker after it, or another fence's begin marker before
/// the end (the end then belongs to that fence). A writer must leave such
/// a fence alone: appending a second one would let the next run replace
/// everything between the old begin and the new end, the user's text
/// included (WP-050).
pub fn fence_damaged(text: &str, name: &str) -> bool {
    text.contains(&format!("{FENCE_BEGIN}{name} -->"))
        && fence_body(text, name).is_none_or(|b| b.contains(FENCE_BEGIN))
}

/// A generated file (`STATUS.md`, `outputs/REBUILD.md`) with its fence
/// `name` replaced by `content` ([`neutralise`]d); the rest of an existing
/// file is kept (WP-032 review item 6: one merge, several callers). A file
/// without the fence is replaced when it is blank (the header aside) or
/// `template` says it is fully generated; one without the header is user
/// text and is kept below the fence. The header is always the first line.
///
/// `Err` (the file must stay as it is, F-131): a damaged fence
/// ([`fence_damaged`]), or a file with the generated header but without
/// the fence (its markers were removed by hand): which part is the user's
/// cannot be told.
pub fn try_merge_fence(
    existing: Option<&str>,
    name: &str,
    content: &str,
    template: impl Fn(&str) -> bool,
) -> Result<String, String> {
    let content = neutralise(content);
    let fresh = || format!("{GENERATED_HEADER}\n{FENCE_BEGIN}{name} -->\n{content}{FENCE_END}\n");
    let Some(old) = existing else {
        return Ok(fresh());
    };
    if fence_damaged(old, name) {
        return Err(format!("the {name} fence has no end marker of its own"));
    }
    if let Some(merged) = replace_fence(old, name, &content) {
        return Ok(if merged.starts_with(GENERATED_HEADER) {
            merged
        } else {
            format!("{GENERATED_HEADER}\n{merged}")
        });
    }
    let below_header = old.strip_prefix(GENERATED_HEADER).unwrap_or(old);
    if below_header.trim().is_empty() || template(old) {
        return Ok(fresh());
    }
    if old.starts_with(GENERATED_HEADER) {
        return Err(format!(
            "the generated header is there but the {name} fence is not"
        ));
    }
    Ok(format!("{}\n{old}", fresh()))
}

/// [`try_merge_fence`] for files the user asked for (`outputs/REBUILD.md`,
/// the import report): a file it would leave alone gets a fresh fence on
/// top and keeps all of its old text below (without a second header
/// line), so nothing is lost and the next run finds the new fence first.
pub fn merge_fence(existing: Option<&str>, name: &str, content: &str) -> String {
    try_merge_fence(existing, name, content, |_| false).unwrap_or_else(|_| {
        let old = existing.unwrap_or_default();
        let old = old
            .strip_prefix(GENERATED_HEADER)
            .map_or(old, |rest| rest.trim_start_matches(['\r', '\n']));
        let fresh = try_merge_fence(None, name, content, |_| false).unwrap_or_default();
        format!("{fresh}\n{old}")
    })
}

/// Regenerates `STATUS.md`. A damaged fence, or the header without the
/// fence, leaves the file as it is ([`Fill::Skipped`], F-131).
pub fn write_status(logbook: &Logbook, built: &Built) -> anyhow::Result<Fill> {
    const REL: &str = "STATUS.md";
    let path = logbook.path(REL);
    let existing = match std::fs::read_to_string(&path) {
        Ok(t) => Some(t),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => {
            return Err(anyhow::Error::new(e).context(format!("cannot read {}", path.display())));
        }
    };
    let text = match merge_status(
        existing.as_deref(),
        &status_text(built, logbook.meta.language),
    ) {
        Ok(text) => text,
        Err(why) => {
            return Ok(Fill::Skipped(format!(
                "{REL}: {why}; file not updated (restore the marker lines \
                 `{FENCE_BEGIN}{STATUS_FENCE} -->` and `{FENCE_END}`)"
            )));
        }
    };
    Ok(if write_if_changed(logbook, REL, &text, Durable::No)? {
        Fill::Written
    } else {
        Fill::Unchanged
    })
}

/// The body of the `decisions.index` fence of `DECISIONS.md`: the table
/// head of `old` when it has one (a translated head stays), else the
/// English one; then one row per decision in `rows` order (newest first).
pub fn decisions_index(old: Option<&str>, rows: &[DecisionRow]) -> String {
    let mut text = String::new();
    let head: Vec<&str> = old
        .filter(|b| has_table_separator(b))
        .map(|b| {
            let lines: Vec<&str> = b.lines().collect();
            let sep = lines
                .iter()
                .position(|l| has_table_separator(l))
                .unwrap_or(0);
            lines[..=sep].to_vec()
        })
        .unwrap_or_default();
    if head.is_empty() {
        text.push_str("| ID | Title | Status | Date |\n|---|---|---|---|\n");
    } else {
        for line in head {
            text.push_str(line);
            text.push('\n');
        }
    }
    for r in rows {
        let _ = writeln!(
            text,
            "| [[{}]] | {} | {} | {} |",
            r.id,
            r.title.replace('|', "\\|"),
            r.status,
            r.date
        );
    }
    text
}

/// What [`write_decisions_index`] did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Fill {
    Written,
    Unchanged,
    /// Not touched; the reason is a warning.
    Skipped(String),
}

/// Fills the `decisions.index` fence of `DECISIONS.md` from `rows`
/// (WP-050); every byte outside the fence stays. A file without the
/// fence gets it appended under `## Index`, a missing file is created.
/// A damaged fence ([`fence_damaged`]) is left alone.
pub fn write_decisions_index(logbook: &Logbook, rows: &[DecisionRow]) -> anyhow::Result<Fill> {
    const REL: &str = "DECISIONS.md";
    let path = logbook.path(REL);
    let old = match std::fs::read_to_string(&path) {
        Ok(t) => Some(t),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => {
            return Err(anyhow::Error::new(e).context(format!("cannot read {}", path.display())));
        }
    };
    if old
        .as_deref()
        .is_some_and(|t| fence_damaged(t, DECISIONS_FENCE))
    {
        return Ok(Fill::Skipped(format!(
            "{REL}: the {DECISIONS_FENCE} fence has no end marker of its own; table not updated"
        )));
    }
    let body = old.as_deref().and_then(|t| fence_body(t, DECISIONS_FENCE));
    let content = neutralise(&decisions_index(body, rows)).into_owned();
    let text = match old.as_deref() {
        Some(t) => match replace_fence(t, DECISIONS_FENCE, &content) {
            Some(text) => text,
            None => {
                let sep = if t.is_empty() || t.ends_with('\n') {
                    ""
                } else {
                    "\n"
                };
                format!(
                    "{t}{sep}\n## Index\n\n{FENCE_BEGIN}{DECISIONS_FENCE} -->\n{content}{FENCE_END}\n"
                )
            }
        },
        None => {
            format!("# Decisions\n\n{FENCE_BEGIN}{DECISIONS_FENCE} -->\n{content}{FENCE_END}\n")
        }
    };
    Ok(if write_if_changed(logbook, REL, &text, Durable::Yes)? {
        Fill::Written
    } else {
        Fill::Unchanged
    })
}

/// How [`write_if_changed`] writes: `Yes` syncs (`DECISIONS.md`, whose
/// text outside the fence is the user's), `No` does not (a view rebuilt
/// from the ledger).
#[derive(Clone, Copy)]
enum Durable {
    Yes,
    No,
}

fn write_if_changed(
    logbook: &Logbook,
    rel: &str,
    text: &str,
    durable: Durable,
) -> crate::error::Result<bool> {
    let path = logbook.checked_file(rel)?;
    if std::fs::read(&path).is_ok_and(|old| old == text.as_bytes()) {
        return Ok(false);
    }
    match durable {
        Durable::Yes => sys::write_atomic_nofollow(&path, text.as_bytes())?,
        Durable::No => sys::write_generated_nofollow(&path, text.as_bytes())?,
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn merge(existing: Option<&str>, content: &str) -> String {
        merge_status(existing, content).unwrap()
    }

    #[test]
    fn merge_keeps_user_text_outside_the_fence() {
        let fresh = merge(None, "A\n");
        assert_eq!(
            fresh,
            format!("{GENERATED_HEADER}\n<!-- seldon:begin status -->\nA\n<!-- seldon:end -->\n")
        );
        let edited = format!("{fresh}\nMy notes.\n");
        let again = merge(Some(&edited), "B\n");
        assert_eq!(
            again,
            format!(
                "{GENERATED_HEADER}\n<!-- seldon:begin status -->\nB\n<!-- seldon:end -->\n\nMy notes.\n"
            )
        );
        // user text above the fence, header removed by hand
        let above = "Intro\n<!-- seldon:begin status -->\nold\n<!-- seldon:end -->\nTail";
        assert_eq!(
            merge(Some(above), "C\n"),
            format!(
                "{GENERATED_HEADER}\nIntro\n<!-- seldon:begin status -->\nC\n<!-- seldon:end -->\nTail"
            )
        );
        // the init template (any language) is fully generated; a user file
        // is kept below
        for language in Language::ALL {
            let template = crate::logbook::templates::find("STATUS.md")
                .unwrap()
                .text(language)
                .replace("{{machineId}}", "box-1a2b");
            assert_eq!(merge(Some(&template), "D\n"), merge(None, "D\n"));
        }
        let user = "# My status\nhand-written\n";
        assert_eq!(
            merge(Some(user), "E\n"),
            format!("{}\n{user}", merge(None, "E\n"))
        );
        // idempotent
        let once = merge(Some(user), "E\n");
        assert_eq!(merge(Some(&once), "E\n"), once);
    }

    /// F-131: a fence whose markers were edited away leaves the file alone.
    #[test]
    fn a_damaged_status_file_is_not_merged() {
        let fresh = merge(None, "A\n");
        let notes = format!("{fresh}\n## My notes\nKeep this.\n");
        for damaged in [
            notes.replace("<!-- seldon:end -->\n", ""),
            notes.replace("<!-- seldon:begin status -->\n", ""),
            notes
                .replace("<!-- seldon:begin status -->\n", "")
                .replace("<!-- seldon:end -->\n", ""),
            // the template with notes added is no longer the template
            format!("{GENERATED_HEADER}\n# Status — x\n\nNo status yet.\nMine.\n"),
            // the end marker is gone, but a complete block pasted below
            // would lend the first begin marker its end
            notes.replace("<!-- seldon:end -->\n", "")
                + "\n<!-- seldon:begin status -->\npasted\n<!-- seldon:end -->\n",
        ] {
            let err = merge_status(Some(&damaged), "B\n").unwrap_err();
            assert!(!err.is_empty(), "{damaged}");
        }
        // REBUILD.md and the import report: the old text stays below
        let open = notes.replace("<!-- seldon:end -->\n", "");
        let kept = merge_fence(Some(&open), STATUS_FENCE, "B\n");
        assert_eq!(
            kept,
            format!(
                "{GENERATED_HEADER}\n<!-- seldon:begin status -->\nB\n<!-- seldon:end -->\n\n\
                 <!-- seldon:begin status -->\nA\n\n## My notes\nKeep this.\n"
            )
        );
        assert_eq!(
            merge_fence(Some(&kept), STATUS_FENCE, "B\n"),
            kept,
            "stable"
        );
    }

    /// F-131: `\r\n` line ends are a fence too; the new body gets them.
    #[test]
    fn a_crlf_fence_is_merged_with_crlf() {
        let crlf = merge(None, "A\nB\n").replace('\n', "\r\n") + "\r\nMine.\r\n";
        let merged = merge(Some(&crlf), "C\n");
        assert_eq!(
            merged,
            format!(
                "{GENERATED_HEADER}\r\n<!-- seldon:begin status -->\r\nC\r\n<!-- seldon:end -->\r\n\r\nMine.\r\n"
            )
        );
        assert_eq!(fence_body(&merged, STATUS_FENCE), Some("C\r\n"));
        assert!(!fence_damaged(&merged, STATUS_FENCE));
    }

    /// F-130: a value with a fence marker cannot end or open the fence.
    #[test]
    fn values_cannot_close_the_fence() {
        let content = "- [[C-2026-001]] Document the <!-- seldon:end --> marker\n\
                       - <!-- seldon:begin status --> twice\n";
        let once = merge(None, content);
        let body = fence_body(&once, STATUS_FENCE).unwrap();
        assert_eq!(body, neutralise(content));
        assert!(!body.contains(MARKER), "{body}");
        assert!(body.contains("Document the <!--\u{200B} seldon:end --> marker"));
        let mut text = once.clone();
        for _ in 0..3 {
            text = merge(Some(&text), content);
        }
        assert_eq!(text, once, "stable");
        assert_eq!(neutralise(&neutralise(content)), neutralise(content));
        assert!(matches!(neutralise("plain <!-- x -->"), Cow::Borrowed(_)));
    }

    #[test]
    fn a_fence_is_damaged_without_an_end_of_its_own() {
        let ok = "<!-- seldon:begin a -->\nx\n<!-- seldon:end -->\n";
        assert!(!fence_damaged(ok, "a"));
        assert!(!fence_damaged(ok, "b"), "no fence is not a damaged fence");
        assert!(fence_damaged("<!-- seldon:begin a -->\nx\n", "a"));
        assert!(
            fence_damaged("t <!-- seldon:begin a -->", "a"),
            "marker at the end"
        );
        let borrowed =
            "<!-- seldon:begin a -->\nmine\n<!-- seldon:begin b -->\ny\n<!-- seldon:end -->\n";
        assert!(fence_damaged(borrowed, "a"));
        assert!(!fence_damaged(borrowed, "b"));
    }

    #[test]
    fn decisions_index_keeps_the_head_and_writes_every_row() {
        let row = |id: &str, title: &str| DecisionRow {
            id: id.into(),
            title: title.into(),
            status: "accepted".into(),
            date: "2026-10-01".into(),
            path: format!("decisions/{id}-x.md"),
            cases: Vec::new(),
            lead: None,
        };
        let rows = [row("ADR-0002", "a | b"), row("ADR-0001", "One")];
        let fresh = "| ID | Title | Status | Date |\n|---|---|---|---|\n\
                     | [[ADR-0002]] | a \\| b | accepted | 2026-10-01 |\n\
                     | [[ADR-0001]] | One | accepted | 2026-10-01 |\n";
        assert_eq!(decisions_index(None, &rows), fresh);
        assert_eq!(decisions_index(Some(""), &rows), fresh);
        assert_eq!(
            decisions_index(Some("hand-written, no table\n"), &rows),
            fresh
        );
        // an editor-formatted, translated head stays; old rows go
        let old = "| ID | Titel | Status | Datum |\n| --- | :--- | --- | --- |\n| [[ADR-0009]] | gone | x | y |\n";
        let kept = decisions_index(Some(old), &rows[1..]);
        assert_eq!(
            kept,
            "| ID | Titel | Status | Datum |\n| --- | :--- | --- | --- |\n\
             | [[ADR-0001]] | One | accepted | 2026-10-01 |\n"
        );
        assert_eq!(decisions_index(Some(&kept), &rows[1..]), kept, "stable");
        assert_eq!(
            decisions_index(None, &[]),
            "| ID | Title | Status | Date |\n|---|---|---|---|\n",
            "the init template's table"
        );
    }
}
