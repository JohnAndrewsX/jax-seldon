//! Cases on disk: `work/{queued,active,completed}/C-YYYY-NNN-slug.md`
//! (SPEC-LOGBOOK §3, ADR-0012 §9).
//!
//! The engine is the only one that moves case files between folders. It
//! changes a case in two ways only: frontmatter keys through the lossless
//! [`model::update`], and new lines at the end of the `## Log` section. The
//! rest of the body is never touched.

use std::ops::Range;
use std::path::{Path, PathBuf};

use anyhow::Context as _;
use chrono::{DateTime, FixedOffset};

use super::{ACTIVE_CASE_FILE, Logbook, templates};
use crate::error::{Error, Result};
use crate::frontmatter::{Document, printable};
use crate::model::{self, Area, Case, CaseStatus, Language, is_agent, is_case_id, is_slug};
use crate::sys;

/// The comment under a new `## Log` heading (as in `fixtures/logbook/`).
pub const LOG_COMMENT: &str = "<!-- append-only; engine and agents add dated lines -->";

/// Longest slug in a case file name, in bytes.
const SLUG_MAX: usize = 40;

/// A case file: where it is, its frontmatter, and the document (body).
#[derive(Debug, Clone)]
pub struct CaseFile {
    pub path: PathBuf,
    pub case: Case,
    pub doc: Document,
}

impl CaseFile {
    /// Reads a case file. Invalid frontmatter is the user's to fix (exit 1),
    /// with the file named.
    pub fn load(path: &Path) -> Result<CaseFile> {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("cannot read {}", path.display()))?;
        let (case, doc) = model::parse::<Case>(&text)
            .map_err(|e| Error::user(format!("{}: invalid case: {e}", path.display())))?;
        Ok(CaseFile {
            path: path.to_path_buf(),
            case,
            doc,
        })
    }

    /// The path relative to the logbook root, with `/` separators.
    pub fn relative(&self, logbook: &Logbook) -> String {
        relative(logbook, &self.path)
    }

    /// The `## Plan` checkboxes: (total, done).
    pub fn steps(&self) -> (usize, usize) {
        plan_steps(&self.doc.body)
    }

    /// Records a non-`seldon` ledger event attributed to this case
    /// (`events`, oldest first) and an agent actor in `agents`.
    pub fn attach(&mut self, event_id: &str, actor: &str) {
        if !self.case.events.iter().any(|e| e == event_id) {
            self.case.events.push(event_id.to_string());
        }
        self.add_agent(actor);
    }

    /// Adds `actor` to `agents` when it is an agent.
    pub fn add_agent(&mut self, actor: &str) {
        if is_agent(actor) && !self.case.agents.iter().any(|a| a == actor) {
            self.case.agents.push(actor.to_string());
        }
    }

    /// Appends one dated line to the `## Log` section.
    pub fn log(&mut self, now: &DateTime<FixedOffset>, text: &str, actor: &str) {
        self.doc.body = append_log(&self.doc.body, &log_line(now, text, actor));
    }

    /// Writes the case back: changed frontmatter keys, the body as it is,
    /// into the folder its status belongs to (moving the file if needed).
    /// Returns the previous path when the file moved.
    ///
    /// Load and save under one hold of the state lock: a copy read before
    /// it would write back a stale case. As a last guard, a file that is
    /// gone from its path (another writer moved it) is not written again,
    /// which would leave the case twice (WP-057).
    pub fn save(&mut self, logbook: &Logbook) -> Result<Option<PathBuf>> {
        let target = self.checked(logbook)?;
        let text = self.doc.render();
        if target == self.path {
            sys::write_atomic(&self.path, text.as_bytes())?;
            return Ok(None);
        }
        if let Some(dir) = target.parent() {
            crate::sys::create_dir_private(dir)
                .with_context(|| format!("cannot create {}", dir.display()))?;
        }
        // Move first, then write: there is never a second file with this id.
        std::fs::rename(&self.path, &target).with_context(|| {
            format!(
                "cannot move {} to {}",
                self.path.display(),
                target.display()
            )
        })?;
        sys::write_atomic(&target, text.as_bytes())?;
        Ok(Some(std::mem::replace(&mut self.path, target)))
    }

    /// Checks that [`save`](Self::save) will write this case once `change`
    /// is made to it, without writing anything: `change` and the save's
    /// checks run on a clone. A command that writes the ledger, then the
    /// journal, then the case prepares the case first: a save that would
    /// be refused then fails the command before the ledger or the journal
    /// changes (WP-077), as [`journal::prepare`](super::journal::prepare)
    /// does for the day file.
    pub fn prepare(&self, logbook: &Logbook, change: impl FnOnce(&mut CaseFile)) -> Result<()> {
        let mut next = self.clone();
        change(&mut next);
        next.checked(logbook).map(drop)
    }

    /// The save's checks: the file is still there, the frontmatter takes
    /// the case ([`model::update`], into `doc`), and the folder of its
    /// status has no other file of this name. Returns the target path.
    fn checked(&mut self, logbook: &Logbook) -> Result<PathBuf> {
        if !self.path.is_file() {
            return Err(Error::user(format!(
                "{} is gone (moved by another seldon?); nothing written",
                relative(logbook, &self.path)
            )));
        }
        model::update(&mut self.doc, &self.case)
            .map_err(|e| Error::user(format!("{}: {e}", relative(logbook, &self.path))))?;
        let name = self
            .path
            .file_name()
            .context("a case file has a name")?
            .to_os_string();
        let target = logbook
            .path("work")
            .join(self.case.status.folder())
            .join(name);
        if target != self.path && target.exists() {
            return Err(Error::user(format!(
                "cannot move {} to {}: the target exists",
                relative(logbook, &self.path),
                relative(logbook, &target)
            )));
        }
        Ok(target)
    }
}

/// Stand-ins for the ids the ledger assigns, for a [`CaseFile::prepare`]
/// before the ledger write: `n` distinct ULIDs of the same shape.
pub fn pending_ids(n: usize) -> Vec<String> {
    (1..=n as u128)
        .map(|i| ulid::Ulid::from_parts(1_800_000_000_000, i).to_string())
        .collect()
}

/// `path` relative to the logbook root, `/`-separated.
pub fn relative(logbook: &Logbook, path: &Path) -> String {
    path.strip_prefix(&logbook.root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

/// Checks the form of a case id given on the command line.
pub fn check_id(id: &str) -> Result<()> {
    if is_case_id(id) {
        Ok(())
    } else {
        Err(Error::user(format!(
            "`{}` is not a case id (C-YYYY-NNN)",
            id.escape_debug()
        )))
    }
}

/// The case with this id; exit 1 when there is none (or two files claim it).
pub fn find(logbook: &Logbook, id: &str) -> Result<CaseFile> {
    check_id(id)?;
    let prefix = format!("{id}-");
    let exact = format!("{id}.md");
    let candidates: Vec<PathBuf> = logbook
        .case_files()?
        .into_iter()
        .filter(|p| {
            p.file_name().is_some_and(|n| {
                let n = n.to_string_lossy();
                n.starts_with(&prefix) || n == exact
            })
        })
        .collect();
    let mut found = Vec::new();
    for path in candidates {
        let file = CaseFile::load(&path)?;
        if file.case.id == id {
            found.push(file);
        }
    }
    match found.len() {
        0 => Err(Error::user(format!("unknown case {id}"))),
        1 => Ok(found.remove(0)),
        _ => Err(Error::user(format!(
            "case {id} exists more than once: {}",
            found
                .iter()
                .map(|f| f.relative(logbook))
                .collect::<Vec<_>>()
                .join(", ")
        ))),
    }
}

/// Every case that loads, sorted by id (ADR-0012 §9 orders the open
/// groups by id), and a warning for each case file that does not, worded
/// as the index's (`<path>: invalid case: …; skipped`, WP-077).
pub fn all(logbook: &Logbook) -> Result<(Vec<CaseFile>, Vec<String>)> {
    let (mut out, mut warnings) = (Vec::new(), Vec::new());
    for path in logbook.case_files()? {
        // a file name may hold any character but `/` and NUL
        let rel = printable(&relative(logbook, &path));
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) => {
                warnings.push(format!("{rel}: cannot read: {e}; skipped"));
                continue;
            }
        };
        match model::parse::<Case>(&text) {
            Ok((case, doc)) => out.push(CaseFile { path, case, doc }),
            Err(e) => warnings.push(format!("{rel}: invalid case: {e}; skipped")),
        }
    }
    out.sort_by(|a, b| a.case.id.cmp(&b.case.id));
    Ok((out, warnings))
}

/// The next free case id of `year`. Ids are never reused: every case file
/// in the three work folders and every workpiece folder `work/C-…/` counts.
pub fn next_id(logbook: &Logbook, year: i32) -> Result<String> {
    let prefix = format!("C-{year}-");
    let mut names: Vec<String> = logbook
        .case_files()?
        .iter()
        .filter_map(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
        .collect();
    if let Ok(entries) = std::fs::read_dir(logbook.path("work")) {
        names.extend(
            entries
                .filter_map(|e| e.ok())
                .filter(|e| e.path().is_dir())
                .map(|e| e.file_name().to_string_lossy().into_owned()),
        );
    }
    let max = names
        .iter()
        .filter_map(|n| n.strip_prefix(&prefix))
        .filter_map(|rest| {
            let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
            digits.parse::<u32>().ok()
        })
        .max()
        .unwrap_or(0);
    Ok(format!("{prefix}{:03}", max + 1))
}

/// A file-name slug for a case title: ASCII, lowercase, German umlauts
/// transliterated, at most [`SLUG_MAX`] bytes cut at a hyphen; `fallback`
/// when nothing is left (a title in another script).
pub fn slug(title: &str, fallback: &str) -> String {
    let mut ascii = String::with_capacity(title.len());
    for c in title.chars() {
        match c {
            'ä' | 'Ä' => ascii.push_str("ae"),
            'ö' | 'Ö' => ascii.push_str("oe"),
            'ü' | 'Ü' => ascii.push_str("ue"),
            'ß' => ascii.push_str("ss"),
            c => ascii.push(c),
        }
    }
    let mut s = sys::slugify(&ascii);
    if s.len() > SLUG_MAX {
        let cut = s[..=SLUG_MAX].rfind('-').unwrap_or(SLUG_MAX);
        s.truncate(cut);
    }
    if s.is_empty() {
        fallback.to_string()
    } else {
        s
    }
}

/// A case lifecycle step (SPEC-LOGBOOK §3): `queued → active →
/// verification → completed`, and any open case `→ dropped`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transition {
    Start,
    Verify,
    Done,
    Drop,
}

impl Transition {
    pub fn name(self) -> &'static str {
        match self {
            Transition::Start => "start",
            Transition::Verify => "verify",
            Transition::Done => "done",
            Transition::Drop => "drop",
        }
    }

    /// The status after this step from `from`, or why it is not allowed.
    pub fn target(self, from: CaseStatus) -> Result<CaseStatus, String> {
        use CaseStatus::*;
        let (allowed, to): (&[CaseStatus], CaseStatus) = match self {
            Transition::Start => (&[Queued], Active),
            Transition::Verify => (&[Active], Verification),
            Transition::Done => (&[Verification], Completed),
            Transition::Drop => (&[Queued, Active, Verification], Dropped),
        };
        if allowed.contains(&from) {
            return Ok(to);
        }
        let hint = match (self, from) {
            (Transition::Done, Active) => "; run `seldon plan verify` first",
            (Transition::Verify, Queued) => "; run `seldon plan start` first",
            _ => "",
        };
        let wants: Vec<&str> = allowed.iter().map(|s| s.as_str()).collect();
        Err(format!(
            "is {from}; `seldon plan {}` needs a case that is {}{hint}",
            self.name(),
            wants.join(", ")
        ))
    }

    /// The word in the case's Log line (as in `fixtures/logbook/`).
    pub fn log_word(self) -> &'static str {
        match self {
            Transition::Start => "started",
            Transition::Verify => "verification",
            Transition::Done => "completed",
            Transition::Drop => "dropped",
        }
    }
}

/// `- YYYY-MM-DD HH:MM · text · actor`. Line breaks in `text` become spaces,
/// so one entry is always one line.
pub fn log_line(now: &DateTime<FixedOffset>, text: &str, actor: &str) -> String {
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    format!("- {} · {text} · {actor}", now.format("%Y-%m-%d %H:%M"))
}

/// `body` with `line` appended as the last line of its `## Log` section.
/// Everything before the insertion point and everything after it stays
/// byte-identical. A body without a Log section gets one, before
/// `## Result` if there is one, else at the end.
pub fn append_log(body: &str, line: &str) -> String {
    let nl = if body.contains("\r\n") { "\r\n" } else { "\n" };
    if let Some(range) = section(body, "Log") {
        let content = &body[range.clone()];
        let insert_at = match content.trim_end_matches([' ', '\t', '\r', '\n']).len() {
            0 => range.start,
            n => {
                let after = range.start + n;
                // past the line ending of the last non-blank line
                body[after..range.end]
                    .find('\n')
                    .map_or(range.end, |i| after + i + 1)
            }
        };
        let mut out = String::with_capacity(body.len() + line.len() + 2);
        out.push_str(&body[..insert_at]);
        if !out.ends_with('\n') {
            out.push_str(nl);
        }
        out.push_str(line);
        out.push_str(nl);
        out.push_str(&body[insert_at..]);
        return out;
    }
    let block = format!("## Log{nl}{LOG_COMMENT}{nl}{line}{nl}");
    if let Some(at) = heading_start(body, "Result") {
        return format!("{}{block}{nl}{}", &body[..at], &body[at..]);
    }
    let mut out = body.to_string();
    if !out.is_empty() && !out.ends_with('\n') {
        out.push_str(nl);
    }
    if !out.is_empty() && !out.ends_with(&format!("{nl}{nl}")) {
        out.push_str(nl);
    }
    out.push_str(&block);
    out
}

/// `body` with the content of its `## Intent` section replaced by `text`
/// (a new case's body, WP-101). A body without an Intent section gets one
/// before `## Plan`, else before `## Log`, else at the end.
pub fn put_intent(body: &str, text: &str) -> String {
    let nl = if body.contains("\r\n") { "\r\n" } else { "\n" };
    let text: String = text.trim().lines().collect::<Vec<_>>().join(nl);
    if let Some(range) = section(body, "Intent") {
        let tail = if range.end == body.len() { "" } else { nl };
        return format!(
            "{}{text}{nl}{tail}{}",
            &body[..range.start],
            &body[range.end..]
        );
    }
    let block = format!("## Intent{nl}{text}{nl}{nl}");
    match heading_start(body, "Plan").or_else(|| heading_start(body, "Log")) {
        Some(at) => format!("{}{block}{}", &body[..at], &body[at..]),
        None => {
            let mut out = body.to_string();
            if !out.is_empty() && !out.ends_with('\n') {
                out.push_str(nl);
            }
            if !out.is_empty() && !out.ends_with(&format!("{nl}{nl}")) {
                out.push_str(nl);
            }
            out.push_str(block.trim_end());
            out.push_str(nl);
            out
        }
    }
}

/// The text of the `## Intent` section, trimmed ("" when there is none).
pub fn intent(body: &str) -> &str {
    section(body, "Intent").map_or("", |r| body[r].trim())
}

/// Free text as section content of a case: a line that Markdown or
/// [`section`] would read as a heading or a code fence (`#`, three
/// backticks or tildes, after leading blanks) gets a `\` in front, so the
/// text can never end its section or hide the ones after it.
pub fn escape_lines(text: &str) -> String {
    text.lines()
        .map(|line| {
            let t = line.trim_start();
            if t.starts_with('#') || t.starts_with("```") || t.starts_with("~~~") {
                format!("{}\\{t}", &line[..line.len() - t.len()])
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// `text` without HTML comments (`<!-- … -->`; an unclosed one runs to the
/// end).
pub fn strip_comments(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(open) = rest.find("<!--") {
        out.push_str(&rest[..open]);
        match rest[open + 4..].find("-->") {
            Some(close) => rest = &rest[open + 4 + close + 3..],
            None => return out,
        }
    }
    out.push_str(rest);
    out
}

/// The first paragraph of section `name` (ADR-0038 §2): its text without
/// HTML comments, blank and heading lines before it skipped, then every
/// line up to the next blank one, each trimmed at the end, joined by
/// `\n`. `None` without the section or without such a line.
pub fn first_paragraph(body: &str, name: &str) -> Option<String> {
    paragraphs(&body[section(body, name)?], 1)
        .into_iter()
        .next()
}

/// The first `max` paragraphs of `text` as [`first_paragraph`] reads them:
/// HTML comments left out (an unclosed one runs to the end, as in
/// [`strip_comments`]), then blocks of non-blank lines; a heading line
/// (`#` to `######` and a space) ends one and is no paragraph text. The
/// scan stops after the `max`-th paragraph, and only the paragraphs
/// returned are copied: a whole task file imported as an Intent costs no
/// more than its first paragraphs on every index build (WP-127 round 2).
pub fn paragraphs(text: &str, max: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut block: Vec<String> = Vec::new();
    let mut line = String::new();
    let mut pos = 0;
    // one line of text without comments ends: text joins the block, a
    // blank or heading line closes it
    fn end_line(line: &mut String, block: &mut Vec<String>, out: &mut Vec<String>) {
        let l = line.trim_end();
        if l.trim().is_empty() || is_heading(l.trim_start()) {
            if !block.is_empty() {
                out.push(block.join("\n").trim_start().to_string());
                block.clear();
            }
        } else {
            block.push(l.to_string());
        }
        line.clear();
    }
    while pos < text.len() && out.len() < max {
        let rest = &text[pos..];
        let nl = rest.find('\n').unwrap_or(rest.len());
        // a comment opening on this line: everything up to its close is
        // left out, line breaks inside it too
        match rest[..nl].find("<!--") {
            Some(open) => {
                line.push_str(&rest[..open]);
                match rest[open + 4..].find("-->") {
                    Some(close) => pos += open + 4 + close + 3,
                    None => pos = text.len(),
                }
            }
            None => {
                line.push_str(&rest[..nl]);
                pos += (nl + 1).min(rest.len());
                end_line(&mut line, &mut block, &mut out);
            }
        }
    }
    if out.len() < max {
        if !line.is_empty() {
            end_line(&mut line, &mut block, &mut out);
        }
        if !block.is_empty() {
            out.push(block.join("\n").trim_start().to_string());
        }
    }
    out.truncate(max);
    out
}

/// An ATX heading line: `#` to `######` followed by a space or nothing.
fn is_heading(line: &str) -> bool {
    let rest = line.trim_start_matches('#');
    let level = line.len() - rest.len();
    (1..=6).contains(&level) && (rest.is_empty() || rest.starts_with([' ', '\t']))
}

/// What an agent's close lacks (ADR-0027 §5): the *Result* has no text,
/// and *Plan › Verification* — the `Verification:` item of the Plan with
/// its indented continuation lines — has none. Text is [`has_text`]: a
/// line that is no heading and holds a letter or digit, HTML comments
/// left out. Empty when both are filled.
pub fn close_gaps(body: &str) -> Vec<&'static str> {
    let mut gaps = Vec::new();
    let result = section(body, "Result").map_or("", |r| &body[r]);
    if !has_text(result) {
        gaps.push("its Result is empty");
    }
    if !verification_filled(body) {
        gaps.push("its Plan › Verification is not filled in");
    }
    gaps
}

/// Whether `text` says something: without its HTML comments, a line that
/// is not a heading (`#`) holds a letter or digit (white space, zero-width
/// characters and punctuation alone do not count; WP-101 round 2).
pub fn has_text(text: &str) -> bool {
    strip_comments(text).lines().any(|line| {
        let line = line.trim();
        !line.starts_with('#') && line.chars().any(char::is_alphanumeric)
    })
}

/// Whether the Plan's `Verification:` item (case-insensitive, as a list
/// item or a plain line, the label bold or not: `**Verification:**`,
/// `**Verification**:`) has text after the colon or in the lines indented
/// below it.
fn verification_filled(body: &str) -> bool {
    let Some(range) = section(body, "Plan") else {
        return false;
    };
    let indent = |l: &str| l.len() - l.trim_start().len();
    let lines: Vec<&str> = body[range].lines().collect();
    for (i, line) in lines.iter().enumerate() {
        let item = line.trim_start();
        let item = ["- ", "* ", "+ "]
            .iter()
            .find_map(|m| item.strip_prefix(m))
            .unwrap_or(item)
            .trim_start()
            .trim_start_matches(['*', '_']);
        let Some(head) = item.get(..12) else {
            continue;
        };
        if !head.eq_ignore_ascii_case("verification") {
            continue;
        }
        let Some(rest) = item[12..].trim_start_matches(['*', '_']).strip_prefix(':') else {
            continue;
        };
        let mut text = rest.trim_start_matches(['*', '_']).to_string();
        for next in &lines[i + 1..] {
            if !next.trim().is_empty() && indent(next) <= indent(line) {
                break;
            }
            text.push('\n');
            text.push_str(next);
        }
        return has_text(&text);
    }
    false
}

/// The `## Plan` checkboxes (`- [ ]`, `- [x]`): (total, done).
pub fn plan_steps(body: &str) -> (usize, usize) {
    let Some(range) = section(body, "Plan") else {
        return (0, 0);
    };
    let mut total = 0;
    let mut done = 0;
    for line in body[range].lines() {
        let item = line.trim_start();
        let Some(rest) = ["- [", "* [", "+ ["]
            .iter()
            .find_map(|p| item.strip_prefix(p))
        else {
            continue;
        };
        match rest.get(..2) {
            Some(" ]") => total += 1,
            Some("x]") | Some("X]") => {
                total += 1;
                done += 1;
            }
            _ => {}
        }
    }
    (total, done)
}

/// The byte range of the content of the `## <name>` section: from after
/// its heading line to the next heading of level 1 or 2 (or the end).
/// Headings inside fenced code blocks do not count.
pub fn section(body: &str, name: &str) -> Option<Range<usize>> {
    let headings = headings(body);
    let i = headings.iter().position(|(_, _, text)| *text == name)?;
    let (_, content_start, _) = headings[i];
    let end = headings
        .get(i + 1)
        .map_or(body.len(), |(start, _, _)| *start);
    Some(content_start..end)
}

fn heading_start(body: &str, name: &str) -> Option<usize> {
    headings(body)
        .into_iter()
        .find(|(_, _, text)| *text == name)
        .map(|(start, _, _)| start)
}

/// Level-1 and level-2 headings: (line start, line end incl. newline, text
/// of a level-2 heading or "" for level 1).
fn headings(body: &str) -> Vec<(usize, usize, &str)> {
    let mut out = Vec::new();
    let mut in_fence = false;
    let mut pos = 0;
    while pos < body.len() {
        let end = body[pos..].find('\n').map_or(body.len(), |i| pos + i + 1);
        let line = body[pos..end].trim_end_matches(['\n', '\r']);
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_fence = !in_fence;
        } else if !in_fence {
            if let Some(text) = line.strip_prefix("## ") {
                out.push((pos, end, text.trim_end()));
            } else if line.starts_with("# ") || line == "#" || line == "##" {
                out.push((pos, end, ""));
            }
        }
        pos = end;
    }
    out
}

/// The body of a new case: `.seldon/templates/case.md` of the logbook if
/// the user has one, else the built-in template of the logbook language.
/// Placeholders `{{id}}` and `{{title}}`.
pub fn new_body(logbook: &Logbook, id: &str, title: &str) -> Result<String> {
    let template = logbook_template(logbook, "case.md")?;
    Ok(fill(&template, &[("id", id), ("title", title)]))
}

/// sha256 of every body template an earlier engine shipped into
/// `.seldon/templates/` (`seldon init` copies them): a logbook copy that
/// is still one of them, byte for byte, holds nothing of the user's and
/// counts as the built-in template, so an existing logbook gets the
/// current one without a write (WP-143). The case template of WP-006, en
/// and de.
const SHIPPED_TEMPLATES: [&str; 2] = [
    "9e8d70708793925822b12806394e3d0dc991d16f71b1b0d91d2d49c6aaf59f06",
    "a0561f366ad32a90f903b1f079cd7934b71224b71769e5244c2f56c63c56cf4c",
];

/// `.seldon/templates/<name>` of the logbook, else the built-in one; a
/// copy an earlier engine shipped unchanged is the built-in one too
/// ([`SHIPPED_TEMPLATES`]).
pub fn logbook_template(logbook: &Logbook, name: &str) -> Result<String> {
    let rel = format!(".seldon/templates/{name}");
    let path = logbook.path(&rel);
    let built_in = || -> Result<String> {
        let language: Language = logbook.meta.language;
        templates::find(&rel)
            .map(|t| t.text(language).to_string())
            .ok_or_else(|| anyhow::anyhow!("no built-in template {rel}").into())
    };
    match std::fs::read_to_string(&path) {
        Ok(text) if SHIPPED_TEMPLATES.contains(&sys::sha256_hex(text.as_bytes()).as_str()) => {
            built_in()
        }
        Ok(text) => Ok(text),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => built_in(),
        Err(e) => Err(anyhow::Error::new(e)
            .context(format!("cannot read {}", path.display()))
            .into()),
    }
}

/// Replaces `{{key}}` placeholders in one pass: a value is never scanned
/// again, so a title that contains `{{id}}` stays as typed.
pub fn fill(template: &str, vars: &[(&str, &str)]) -> String {
    let mut out = String::with_capacity(template.len() + 64);
    let mut rest = template;
    while let Some(open) = rest.find("{{") {
        out.push_str(&rest[..open]);
        let after = &rest[open + 2..];
        match after
            .find("}}")
            .and_then(|close| Some((close, vars.iter().find(|(k, _)| *k == &after[..close])?)))
        {
            Some((close, (_, value))) => {
                out.push_str(value);
                rest = &after[close + 2..];
            }
            None => {
                out.push_str("{{");
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// `.seldon/active-case`: the id of the case started last, if any.
pub fn active_case(logbook: &Logbook) -> Option<String> {
    let text = std::fs::read_to_string(logbook.path(ACTIVE_CASE_FILE)).ok()?;
    let id = text.trim();
    is_case_id(id).then(|| id.to_string())
}

/// Writes `.seldon/active-case`.
pub fn set_active_case(logbook: &Logbook, id: &str) -> Result<()> {
    sys::write_atomic(
        &logbook.path(ACTIVE_CASE_FILE),
        format!("{id}\n").as_bytes(),
    )?;
    Ok(())
}

/// Removes `.seldon/active-case` if it names `id`. Returns whether it did.
pub fn clear_active_case(logbook: &Logbook, id: &str) -> Result<bool> {
    if active_case(logbook).as_deref() != Some(id) {
        return Ok(false);
    }
    let path = logbook.path(ACTIVE_CASE_FILE);
    std::fs::remove_file(&path).with_context(|| format!("cannot remove {}", path.display()))?;
    Ok(true)
}

/// Creates `areas/<area>/README.md` on first use. Returns the new file's
/// relative path, or `None` when the area exists.
pub fn ensure_area(logbook: &Logbook, area: &str) -> Result<Option<String>> {
    if !is_slug(area) {
        return Err(Error::user(format!(
            "area `{area}` is not a lowercase slug ([a-z0-9][a-z0-9-]*)"
        )));
    }
    let rel = format!("areas/{area}/README.md");
    let path = logbook.path(&rel);
    if path.is_file() {
        return Ok(None);
    }
    let record = Area {
        name: area.to_string(),
        description: None,
    };
    let text = model::render_new(&record, &format!("# {area}\n"));
    sys::write_atomic(&path, text.as_bytes())?;
    Ok(Some(rel))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone as _;

    /// Every caller checks the id first today; the message is escaped
    /// anyway (WP-077).
    #[test]
    fn a_refused_id_is_named_escaped() {
        let err = check_id("C-\u{1b}[31mX").unwrap_err().to_string();
        assert_eq!(err, "`C-\\u{1b}[31mX` is not a case id (C-YYYY-NNN)");
    }

    fn now() -> DateTime<FixedOffset> {
        FixedOffset::east_opt(7200)
            .unwrap()
            .with_ymd_and_hms(2026, 10, 1, 9, 5, 12)
            .unwrap()
    }

    const BODY: &str = "# C-2026-004 — Zed\n\n## Intent\nWhy.\n\n## Plan\n- Steps:\n  - [x] one\n  - [ ] two\n  - [X] three\n\n## Log\n<!-- append-only; engine and agents add dated lines -->\n- 2026-09-28 20:11 · created (zone red, risk R2) · human\n\n## Result\n";

    #[test]
    fn state_machine() {
        use CaseStatus::*;
        use Transition::*;
        let ok = [
            (Start, Queued, Active),
            (Verify, Active, Verification),
            (Done, Verification, Completed),
            (Drop, Queued, Dropped),
            (Drop, Active, Dropped),
            (Drop, Verification, Dropped),
        ];
        for (t, from, to) in ok {
            assert_eq!(t.target(from), Ok(to), "{t:?} from {from}");
        }
        for t in [Start, Verify, Done, Drop] {
            for from in CaseStatus::ALL {
                if !ok.iter().any(|(t2, f, _)| *t2 == t && f == from) {
                    let err = t.target(*from).unwrap_err();
                    assert!(err.contains(from.as_str()), "{err}");
                }
            }
        }
        assert!(Done.target(Active).unwrap_err().contains("plan verify"));
    }

    #[test]
    fn log_lines_append_at_the_end_of_the_section() {
        let line = log_line(&now(), "started (snapshot 111)", "human");
        assert_eq!(line, "- 2026-10-01 09:05 · started (snapshot 111) · human");
        let once = append_log(BODY, &line);
        let (before, after) = BODY.split_at(BODY.find("\n\n## Result").unwrap() + 1);
        assert_eq!(once, format!("{before}{line}\n{after}"));
        let twice = append_log(&once, "- x");
        assert!(twice.starts_with(&format!("{before}{line}\n- x\n")));
        assert!(twice.ends_with(after));
    }

    #[test]
    fn log_section_edge_cases() {
        // empty section, at the end, without a final newline
        assert_eq!(append_log("## Log", "- a"), "## Log\n- a\n");
        assert_eq!(append_log("## Log\n", "- a"), "## Log\n- a\n");
        assert_eq!(append_log("## Log\n- a", "- b"), "## Log\n- a\n- b\n");
        // missing section: before Result, else at the end
        assert_eq!(
            append_log("# T\n\n## Result\nok\n", "- a"),
            format!("# T\n\n## Log\n{LOG_COMMENT}\n- a\n\n## Result\nok\n")
        );
        assert_eq!(
            append_log("# T\n", "- a"),
            format!("# T\n\n## Log\n{LOG_COMMENT}\n- a\n")
        );
        // CRLF stays CRLF
        assert_eq!(
            append_log("## Log\r\n- a\r\n\r\n## Result\r\n", "- b"),
            "## Log\r\n- a\r\n- b\r\n\r\n## Result\r\n"
        );
        // a heading inside a code fence does not end the section
        let fenced = "## Log\n```\n## Result\n```\n\n## Result\n";
        assert_eq!(
            append_log(fenced, "- a"),
            "## Log\n```\n## Result\n```\n- a\n\n## Result\n"
        );
        // ### sub-headings belong to the section
        assert_eq!(
            append_log("## Log\n### old\n- a\n## Result\n", "- b"),
            "## Log\n### old\n- a\n- b\n## Result\n"
        );
    }

    #[test]
    fn log_line_is_one_line() {
        let line = log_line(&now(), "a\nb\r\n  c", "agent:x");
        assert_eq!(line, "- 2026-10-01 09:05 · a b c · agent:x");
    }

    #[test]
    fn the_intent_replaces_the_section_content() {
        let template = "# C-1 — T\n\n## Intent\n<!-- Why? -->\n\n## Plan\n- Goal:\n";
        assert_eq!(
            put_intent(template, "  Do it.\nNow.  \n"),
            "# C-1 — T\n\n## Intent\nDo it.\nNow.\n\n## Plan\n- Goal:\n"
        );
        assert_eq!(intent(&put_intent(template, "x")), "x");
        // CRLF stays CRLF; an Intent at the end of the body
        assert_eq!(
            put_intent("## Intent\r\nold\r\n", "a\nb"),
            "## Intent\r\na\r\nb\r\n"
        );
        // no Intent section: before Plan, else Log, else at the end
        assert_eq!(
            put_intent("# T\n\n## Plan\n", "x"),
            "# T\n\n## Intent\nx\n\n## Plan\n"
        );
        assert_eq!(
            put_intent("# T\n\n## Log\n", "x"),
            "# T\n\n## Intent\nx\n\n## Log\n"
        );
        assert_eq!(put_intent("# T", "x"), "# T\n\n## Intent\nx\n");
        assert_eq!(intent("# T\n"), "");
    }

    #[test]
    fn escaped_lines_open_no_section_and_no_fence() {
        let text = "Do it.\n## Log\n  # x\n```\n~~~sh\nok # not first";
        let escaped = escape_lines(text);
        assert_eq!(
            escaped,
            "Do it.\n\\## Log\n  \\# x\n\\```\n\\~~~sh\nok # not first"
        );
        let body = put_intent(
            "## Intent\n\n## Plan\n- Verification: x\n\n## Log\n\n## Result\nr\n",
            &escaped,
        );
        assert_eq!(headings(&body).len(), 4, "{body}");
        assert!(close_gaps(&body).is_empty());
    }

    #[test]
    fn comments_are_not_text() {
        assert_eq!(strip_comments("a<!-- b -->c<!-- d"), "ac");
        assert_eq!(strip_comments("<!--\nx\n-->\n"), "\n");
        assert_eq!(strip_comments("plain"), "plain");
    }

    #[test]
    fn close_gaps_name_what_an_agent_close_lacks() {
        let body = |verification: &str, result: &str| {
            format!(
                "## Intent\nx\n\n## Plan\n- Goal: g\n{verification}- Rollback: r\n\n## Log\n\n## Result\n{result}"
            )
        };
        let both = [
            "its Result is empty",
            "its Plan › Verification is not filled in",
        ];
        assert_eq!(close_gaps(&body("- Verification:\n", "")), both);
        assert_eq!(
            close_gaps(&body("- Verification: <!-- later -->\n", "<!-- x -->\n")),
            both
        );
        assert_eq!(close_gaps(&body("", "done\n")), [both[1]]);
        assert!(close_gaps(&body("- Verification: `zed --version`\n", "ok\n")).is_empty());
        assert!(close_gaps(&body("* verification:\n  - `pacman -Q zed`\n", "ok\n")).is_empty());
        assert!(close_gaps(&body("Verification: it runs\n", "ok\n")).is_empty());
        // a sibling item below is not the verification's text
        assert_eq!(close_gaps(&body("- Verification:\n", "ok\n")), [both[1]]);
        // a Verification outside the Plan does not count
        assert_eq!(
            close_gaps("## Plan\n- Goal:\n\n## Log\n- Verification: x\n\n## Result\nok\n"),
            [both[1]]
        );
        assert_eq!(close_gaps("no sections"), both);
        // round 2: a heading, zero-width or punctuation is no text
        for result in [
            "### Evidence\n",
            "\u{200b}\u{feff}\n",
            "- …\n",
            "<!-- x -->\n# y\n",
        ] {
            let gaps = close_gaps(&body("- Verification: v\n", result));
            assert_eq!(gaps, [both[0]], "{result:?}");
        }
        assert!(close_gaps(&body("- Verification: v\n", "### Evidence\nexit 0\n")).is_empty());
        // a bold label
        for item in [
            "- **Verification:** `zed --version`\n",
            "- **Verification**: it runs\n",
            "__verification:__ ok\n",
        ] {
            assert!(close_gaps(&body(item, "ok\n")).is_empty(), "{item:?}");
        }
        for item in [
            "- **Verification:**\n",
            "- **Verification:** …\n",
            "- Verifications: x\n",
        ] {
            assert_eq!(close_gaps(&body(item, "ok\n")), [both[1]], "{item:?}");
        }
    }

    #[test]
    fn steps_count_plan_checkboxes_only() {
        assert_eq!(plan_steps(BODY), (3, 2));
        assert_eq!(plan_steps("## Log\n- [x] not a step\n"), (0, 0));
    }

    #[test]
    fn slugs() {
        assert_eq!(
            slug("Zed als zweiten Editor installieren", "case"),
            "zed-als-zweiten-editor-installieren"
        );
        assert_eq!(slug("Größe ändern", "case"), "groesse-aendern");
        assert_eq!(slug("日本語", "case"), "case");
        let long = slug(
            "Theme-Wechsel auf Tokyo Night durchziehen (Zed, Neovim)",
            "case",
        );
        assert!(long.len() <= SLUG_MAX && !long.ends_with('-'), "{long}");
        assert_eq!(long, "theme-wechsel-auf-tokyo-night");
    }

    #[test]
    fn fill_is_one_pass() {
        assert_eq!(
            fill(
                "# {{id}} — {{title}} {{other}}",
                &[("id", "C-1"), ("title", "{{id}}")]
            ),
            "# C-1 — {{id}} {{other}}"
        );
        assert_eq!(fill("{{unclosed", &[("unclosed", "x")]), "{{unclosed");
    }

    #[test]
    fn first_paragraph_reads_one_block_of_a_section() {
        let body = "# C — t\n\n## Intent\n<!-- Why? -->\n\n  First line \nsecond line\n\nlater\n\n## Plan\n- x\n\n## Result\n";
        assert_eq!(
            first_paragraph(body, "Intent").as_deref(),
            Some("First line\nsecond line")
        );
        // an empty section, a comment only, no section at all
        assert_eq!(first_paragraph(body, "Result"), None);
        assert_eq!(
            first_paragraph("## Intent\n<!-- a\n\nb -->\n", "Intent"),
            None
        );
        assert_eq!(first_paragraph(body, "Log"), None);
        // a heading is no text; a `#` word is
        assert_eq!(
            first_paragraph("## Result\n### Done\n#3 merged\n", "Result").as_deref(),
            Some("#3 merged")
        );
        // CRLF lines lose their `\r`
        assert_eq!(
            first_paragraph("## Result\r\nok\r\n\r\nmore\r\n", "Result").as_deref(),
            Some("ok")
        );
        assert_eq!(paragraphs("a\n\n\nb\nc\n", 9), ["a", "b\nc"]);
        assert_eq!(paragraphs("a\n\n\nb\nc\n", 1), ["a"]);
        assert_eq!(paragraphs("a\n\nb", 0), Vec::<String>::new());
    }

    /// The streaming [`paragraphs`] reads as `strip_comments` and then the
    /// lines would (WP-127 round 2, N5): comments within a line, over line
    /// breaks, unclosed, back to back, at a paragraph's edge.
    #[test]
    fn paragraphs_leave_comments_out_as_strip_comments_does() {
        fn whole(text: &str) -> Vec<String> {
            let text = strip_comments(text);
            let mut out = Vec::new();
            let mut block: Vec<&str> = Vec::new();
            for line in text.lines() {
                let line = line.trim_end();
                if line.trim().is_empty() || is_heading(line.trim_start()) {
                    if !block.is_empty() {
                        out.push(block.join("\n").trim_start().to_string());
                        block.clear();
                    }
                    continue;
                }
                block.push(line);
            }
            if !block.is_empty() {
                out.push(block.join("\n").trim_start().to_string());
            }
            out
        }
        for text in [
            "a<!-- x -->b\nc",
            "a<!-- x\n\ny -->b\n\nc",
            "<!-- only -->\n\nreal\n",
            "a\n<!-- open\nnever closed\n\nmore",
            "a<!--1--><!--2-->b<!--3\n-->\n\nc\r\nd\r\n",
            "x\n<!--\n-->\ny\n\n## H\nz",
            "  lead\n\t\n# h\n<!-- c -->tail\n\n",
            "a-->b<!--c-->d<!--",
            "",
        ] {
            assert_eq!(paragraphs(text, usize::MAX), whole(text), "{text:?}");
            for max in 0..3 {
                let want: Vec<String> = whole(text).into_iter().take(max).collect();
                assert_eq!(paragraphs(text, max), want, "{text:?} max {max}");
            }
        }
    }
}
