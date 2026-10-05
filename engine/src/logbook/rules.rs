//! The engine-maintained rules block of the logbook's `AGENTS.md`
//! (ADR-0027, WP-100): `<!-- seldon:begin rules vN -->` … `<!-- seldon:end -->`.
//!
//! `seldon init` writes the template, the block first and a section for
//! the user's own rules after it. `seldon rules update` brings the block of
//! an existing file up to this engine's rules and keeps everything outside
//! it; `seldon doctor` reports the block's [`State`]. Pure functions: the
//! command does the reading, writing and committing.
//!
//! Marker lines count only as whole lines (`\n` or `\r\n`); the block ends
//! at the first `<!-- seldon:end -->` line after its begin marker. The
//! template's block holds no marker text of its own (a test checks), so
//! the user's text below it, which may quote markers, never ends it.

use std::borrow::Cow;

use crate::index::load::{FENCE_BEGIN, FENCE_END};

/// The rules version this engine writes.
pub const VERSION: u32 = 2;

/// The rules file, relative to the logbook root.
pub const FILE: &str = "AGENTS.md";

/// The heading above the text of an unfenced file that `rules update`
/// keeps below the new block.
pub const KEPT_HEADING: &str = "## Your rules (kept)";

/// The begin marker up to the version number.
const BEGIN_PREFIX: &str = "<!-- seldon:begin rules v";

/// sha256 of every `AGENTS.md` a release wrote before the rules had a
/// block (v1): v0.1.0, and v0.1.1 to v0.1.3, en and de. A file that is
/// still one of them holds nothing of the user's, so `rules update`
/// replaces it whole instead of keeping the old rules below the new ones.
pub const RELEASED_V1: [&str; 4] = [
    "8b17f2dc3f4deac6c94605f04c2fbef345708ab7e264b392d1669be8cc5308ef",
    "477062ef4a92ea0862fd231547ea07cf0fe35414715331e4230058321dcabb5c",
    "2241d260418bb1f84b3fc1d570dbb42cfbeac7416a4178609b4b43e3af7dcfa6",
    "6b752670b9be2307354941682c0b803637c5c49a97946a7eb6e3b0cf873ad3be",
];

/// Where the rules block of a text is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Block {
    /// `start..end` spans the begin marker line through the end marker
    /// line, its line end included; `crlf` when the begin line ends in
    /// `\r\n`.
    Fenced {
        version: u32,
        start: usize,
        end: usize,
        crlf: bool,
    },
    /// A begin marker whose block cannot be told; the reason.
    Damaged(&'static str),
    /// No begin marker line: a file from before the block (v1), or one
    /// the user wrote.
    Unfenced,
}

/// Finds the first rules block of `text`.
pub fn find(text: &str) -> Block {
    let mut offset = 0;
    // (version, start, crlf) of the begin marker line, once seen
    let mut open: Option<(u32, usize, bool)> = None;
    for line in text.split_inclusive('\n') {
        let at = offset;
        offset += line.len();
        let (content, crlf) = strip_eol(line);
        match open {
            None => {
                if content.starts_with(BEGIN_PREFIX) {
                    match marker_version(content) {
                        Some(version) => open = Some((version, at, crlf)),
                        None => return Block::Damaged("has a begin marker without a version"),
                    }
                }
            }
            Some((version, start, crlf)) => {
                if content == FENCE_END {
                    return Block::Fenced {
                        version,
                        start,
                        end: offset,
                        crlf,
                    };
                }
                if content.contains(FENCE_BEGIN) || content.contains(FENCE_END) {
                    return Block::Damaged("has a marker inside it before its end marker line");
                }
            }
        }
    }
    match open {
        Some(_) => Block::Damaged("has no end marker line"),
        None => Block::Unfenced,
    }
}

/// `line` without its `\n` or `\r\n`, and whether it was `\r\n`.
fn strip_eol(line: &str) -> (&str, bool) {
    match line.strip_suffix('\n') {
        Some(l) => match l.strip_suffix('\r') {
            Some(l) => (l, true),
            None => (l, false),
        },
        None => (line, false),
    }
}

/// `N` of a whole line `<!-- seldon:begin rules vN -->`.
fn marker_version(line: &str) -> Option<u32> {
    let digits = line.strip_prefix(BEGIN_PREFIX)?.strip_suffix(" -->")?;
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok()
}

/// The block of `text` (begin through end marker line), `None` unless it
/// is [`Block::Fenced`].
fn block(text: &str) -> Option<&str> {
    match find(text) {
        Block::Fenced { start, end, .. } => Some(&text[start..end]),
        _ => None,
    }
}

/// The rules block of the rendered template.
pub fn template_block(template: &str) -> &str {
    block(template).expect("the AGENTS.md template has a rules block")
}

/// What the rules file holds, for `doctor`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum State {
    /// The block is this engine's, word for word.
    Current,
    /// An older block, or none (`1`, a file from before the block).
    Outdated(u32),
    /// A block of this version whose text differs from this engine's.
    Changed,
    /// A block newer than this engine's.
    Newer(u32),
    Damaged(&'static str),
    Missing,
}

impl State {
    /// The doctor row's wording.
    pub fn label(&self) -> String {
        match self {
            State::Current => format!("current (v{VERSION})"),
            State::Outdated(v) => format!("outdated (v{v})"),
            State::Changed => {
                format!("outdated (v{VERSION}, its text differs from this seldon's rules)")
            }
            State::Newer(v) => format!("newer (v{v}) than this seldon's rules (v{VERSION})"),
            State::Damaged(why) => format!("damaged: the rules block {why}"),
            State::Missing => "missing".into(),
        }
    }
}

/// The state of `text` (`None`: no file) against the rendered `template`.
pub fn state(text: Option<&str>, template: &str) -> State {
    let Some(text) = text else {
        return State::Missing;
    };
    match find(text) {
        Block::Unfenced => State::Outdated(1),
        Block::Damaged(why) => State::Damaged(why),
        Block::Fenced { version, .. } if version > VERSION => State::Newer(version),
        Block::Fenced { version, .. } if version < VERSION => State::Outdated(version),
        Block::Fenced { start, end, .. } => {
            if lf(&text[start..end]) == template_block(template) {
                State::Current
            } else {
                State::Changed
            }
        }
    }
}

fn lf(text: &str) -> Cow<'_, str> {
    if text.contains("\r\n") {
        Cow::Owned(text.replace("\r\n", "\n"))
    } else {
        Cow::Borrowed(text)
    }
}

/// What `rules update` does to the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// The block is current; nothing is written.
    Unchanged,
    /// There was no file; the template is written.
    Created,
    /// The block (or a released v1 file, whole) is replaced by this
    /// engine's; nothing outside it changes.
    Rewritten,
    /// An unfenced file the user changed: the block goes on top, the old
    /// text below [`KEPT_HEADING`], byte for byte.
    Inserted,
    /// `--replace`: the old file is archived, the template written.
    Replaced,
}

impl Action {
    pub fn as_str(self) -> &'static str {
        match self {
            Action::Unchanged => "unchanged",
            Action::Created => "created",
            Action::Rewritten => "rewritten",
            Action::Inserted => "inserted",
            Action::Replaced => "replaced",
        }
    }
}

/// The outcome of [`update`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Update {
    pub action: Action,
    /// The version the file had: `None` without a file, `1` unfenced.
    pub from: Option<u32>,
    pub text: String,
}

/// The new text of the rules file `old` (`None`: no file) with the
/// rendered `template`. `replace` writes the template whatever the file
/// holds (the caller archives the old one). `Err` is the user error: a
/// damaged block or a newer one, where the file stays as it is.
pub fn update(old: Option<&str>, template: &str, replace: bool) -> Result<Update, String> {
    let Some(old) = old else {
        return Ok(Update {
            action: Action::Created,
            from: None,
            text: template.to_string(),
        });
    };
    let found = find(old);
    let from = match found {
        Block::Fenced { version, .. } => Some(version),
        Block::Unfenced => Some(1),
        Block::Damaged(_) => None,
    };
    if replace {
        let action = if old == template {
            Action::Unchanged
        } else {
            Action::Replaced
        };
        return Ok(Update {
            action,
            from,
            text: template.to_string(),
        });
    }
    let new_block = template_block(template);
    match found {
        Block::Damaged(why) => Err(format!(
            "{FILE}: the rules block {why}, so which text is the user's cannot be told; the file is left as it is. Restore the marker lines `{BEGIN_PREFIX}{VERSION} -->` and `{FENCE_END}` around the rules, or run `seldon rules update --replace` (archives the file, then writes the template)"
        )),
        Block::Fenced { version, .. } if version > VERSION => Err(format!(
            "{FILE} has rules v{version}, newer than this seldon's (v{VERSION}); update seldon. The file is left as it is"
        )),
        Block::Fenced {
            start, end, crlf, ..
        } => {
            let block = if crlf {
                Cow::Owned(new_block.replace('\n', "\r\n"))
            } else {
                Cow::Borrowed(new_block)
            };
            let text = format!("{}{block}{}", &old[..start], &old[end..]);
            let action = if text == old {
                Action::Unchanged
            } else {
                Action::Rewritten
            };
            Ok(Update { action, from, text })
        }
        Block::Unfenced if is_released_v1(old) => Ok(Update {
            action: Action::Rewritten,
            from,
            text: template.to_string(),
        }),
        Block::Unfenced => Ok(Update {
            action: Action::Inserted,
            from,
            text: format!("{new_block}\n{KEPT_HEADING}\n\n{old}"),
        }),
    }
}

/// Whether `text` is an `AGENTS.md` exactly as a release wrote it before
/// the block ([`RELEASED_V1`]).
pub fn is_released_v1(text: &str) -> bool {
    RELEASED_V1.contains(&crate::sys::sha256_hex(text.as_bytes()).as_str())
}

/// A unified diff of `old` and `new` for `name` with one hunk and no
/// context lines (`diff -U0`): the lines between the common first and the
/// common last lines. Empty when they are equal.
pub fn diff(old: &str, new: &str, name: &str) -> String {
    let a: Vec<&str> = old.split_inclusive('\n').collect();
    let b: Vec<&str> = new.split_inclusive('\n').collect();
    let head = a.iter().zip(&b).take_while(|(x, y)| x == y).count();
    let tail = a[head..]
        .iter()
        .rev()
        .zip(b[head..].iter().rev())
        .take_while(|(x, y)| x == y)
        .count();
    let (gone, came) = (&a[head..a.len() - tail], &b[head..b.len() - tail]);
    if gone.is_empty() && came.is_empty() {
        return String::new();
    }
    // `-U0`: a side with no lines names the line before the hunk
    let range = |n: usize| {
        if n == 0 {
            format!("{head},0")
        } else {
            format!("{},{n}", head + 1)
        }
    };
    let mut out = format!(
        "--- {name}\n+++ {name}\n@@ -{} +{} @@\n",
        range(gone.len()),
        range(came.len())
    );
    for (sign, lines) in [('-', gone), ('+', came)] {
        for line in lines {
            out.push(sign);
            out.push_str(strip_eol(line).0);
            out.push('\n');
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logbook::templates::{self, Vars};
    use crate::model::Language;

    fn template(language: Language) -> String {
        let vars = Vars {
            machine_id: "box-1a2b",
            language,
            date: chrono::NaiveDate::from_ymd_opt(2026, 10, 5).unwrap(),
        };
        templates::render(templates::find(FILE).unwrap().text(language), &vars)
    }

    fn golden(name: &str) -> String {
        let path = format!(
            "{}/tests/golden/rules-v1/AGENTS-{name}.md",
            env!("CARGO_MANIFEST_DIR")
        );
        std::fs::read_to_string(&path).unwrap()
    }

    #[test]
    fn the_templates_carry_a_current_block_without_marker_text_inside() {
        for language in Language::ALL {
            let t = template(language);
            assert!(t.starts_with(&format!("{BEGIN_PREFIX}{VERSION} -->\n")));
            let Block::Fenced { start, end, .. } = find(&t) else {
                panic!("{language}: no block");
            };
            assert_eq!(start, 0);
            let block = &t[start..end];
            let body = &block[block.find('\n').unwrap() + 1..block.len() - FENCE_END.len() - 1];
            assert!(
                !body.contains("<!-- seldon:") && !body.contains("seldon:end -->"),
                "{language}: marker text inside the block"
            );
            assert_eq!(state(Some(&t), &t), State::Current, "{language}");
            // the user's part after the block
            assert!(t[end..].contains("\n## Your rules\n"), "{language}");
        }
    }

    #[test]
    fn markers_count_as_whole_lines_only() {
        assert_eq!(find("no block\n"), Block::Unfenced);
        // quoted in prose: not a marker
        assert_eq!(
            find("see `<!-- seldon:begin rules v2 -->` here\n"),
            Block::Unfenced
        );
        assert_eq!(
            find("a\n<!-- seldon:begin rules v12 -->\nx\n<!-- seldon:end -->\nb\n"),
            Block::Fenced {
                version: 12,
                start: 2,
                end: 2 + 32 + 2 + 20,
                crlf: false
            }
        );
        // the end marker at the end of the file without a line end
        assert_eq!(
            find("<!-- seldon:begin rules v2 -->\n<!-- seldon:end -->"),
            Block::Fenced {
                version: 2,
                start: 0,
                end: 50,
                crlf: false
            }
        );
        assert_eq!(
            find("<!-- seldon:begin rules v2 -->\r\nx\r\n<!-- seldon:end -->\r\n"),
            Block::Fenced {
                version: 2,
                start: 0,
                end: 56,
                crlf: true
            }
        );
        // an end marker inside a line does not end the block
        assert_eq!(
            find("<!-- seldon:begin rules v2 -->\nx <!-- seldon:end -->\n"),
            Block::Damaged("has a marker inside it before its end marker line")
        );
        assert_eq!(
            find("<!-- seldon:begin rules v2 -->\nx\n"),
            Block::Damaged("has no end marker line")
        );
        assert_eq!(
            find(
                "<!-- seldon:begin rules v2 -->\n<!-- seldon:begin status -->\n<!-- seldon:end -->\n"
            ),
            Block::Damaged("has a marker inside it before its end marker line")
        );
        for bad in ["v", "vx", "v-1", "v+2", "v 2", "v99999999999"] {
            let text = format!("<!-- seldon:begin rules {bad} -->\n<!-- seldon:end -->\n");
            assert_eq!(
                find(&text),
                Block::Damaged("has a begin marker without a version"),
                "{bad}"
            );
        }
        // a marker with trailing text is no marker line
        assert_eq!(
            find("<!-- seldon:begin rules v2 --> x\n<!-- seldon:end -->\n"),
            Block::Damaged("has a begin marker without a version")
        );
    }

    #[test]
    fn state_tells_every_case() {
        let t = template(Language::En);
        assert_eq!(state(None, &t), State::Missing);
        assert_eq!(state(Some(&golden("v0.1.1-en")), &t), State::Outdated(1));
        let v1 = "<!-- seldon:begin rules v1 -->\nold\n<!-- seldon:end -->\n";
        assert_eq!(state(Some(v1), &t), State::Outdated(1));
        let v3 = "<!-- seldon:begin rules v3 -->\nnew\n<!-- seldon:end -->\n";
        assert_eq!(state(Some(v3), &t), State::Newer(3));
        let edited = t.replacen("Rules for every agent", "Rules for any agent", 1);
        assert_eq!(state(Some(&edited), &t), State::Changed);
        // the user's part may change freely
        let own = format!("{t}- Never touch ~/Music.\n");
        assert_eq!(state(Some(&own), &t), State::Current);
        assert_eq!(state(Some(&t.replace('\n', "\r\n")), &t), State::Current);
        assert_eq!(
            state(Some("<!-- seldon:begin rules v2 -->\n"), &t),
            State::Damaged("has no end marker line")
        );
        // the other language's block is not this logbook's
        assert_eq!(state(Some(&template(Language::De)), &t), State::Changed);
    }

    #[test]
    fn released_v1_files_are_known_by_their_hash() {
        for name in ["v0.1.0-en", "v0.1.0-de", "v0.1.1-en", "v0.1.1-de"] {
            assert!(is_released_v1(&golden(name)), "{name}");
        }
        let edited = format!("{}\n- my rule\n", golden("v0.1.1-en"));
        assert!(!is_released_v1(&edited));
    }

    #[test]
    fn update_rewrites_the_block_only() {
        let t = template(Language::En);
        let block = template_block(&t);
        let old = "# Mine\n\nbefore\n<!-- seldon:begin rules v1 -->\nold rules\n<!-- seldon:end -->\nafter `<!-- seldon:end -->`\n";
        let u = update(Some(old), &t, false).unwrap();
        assert_eq!(u.action, Action::Rewritten);
        assert_eq!(u.from, Some(1));
        assert_eq!(
            u.text,
            format!("# Mine\n\nbefore\n{block}after `<!-- seldon:end -->`\n")
        );
        // idempotent
        let again = update(Some(&u.text), &t, false).unwrap();
        assert_eq!(
            (again.action, again.text.as_str()),
            (Action::Unchanged, u.text.as_str())
        );
        // a CRLF file keeps its line ends in the block
        let crlf = old.replace('\n', "\r\n");
        let u = update(Some(&crlf), &t, false).unwrap();
        assert!(u.text.contains(&block.replace('\n', "\r\n")));
        assert!(u.text.starts_with("# Mine\r\n\r\nbefore\r\n"));
        assert_eq!(state(Some(&u.text), &t), State::Current);
    }

    #[test]
    fn update_inserts_above_a_changed_v1_file_and_keeps_it_byte_for_byte() {
        let t = template(Language::En);
        let old = format!("{}\n- Never touch ~/Music.\n", golden("v0.1.1-en"));
        let u = update(Some(&old), &t, false).unwrap();
        assert_eq!((u.action, u.from), (Action::Inserted, Some(1)));
        let (top, kept) = u.text.split_once(&format!("\n{KEPT_HEADING}\n\n")).unwrap();
        assert_eq!(kept, old);
        assert_eq!(top, template_block(&t));
        assert_eq!(state(Some(&u.text), &t), State::Current);
        let again = update(Some(&u.text), &t, false).unwrap();
        assert_eq!(again.action, Action::Unchanged);
        // a file without the old rules, too
        let u = update(Some("my own\n"), &t, false).unwrap();
        assert!(u.text.ends_with(&format!("{KEPT_HEADING}\n\nmy own\n")));
    }

    #[test]
    fn update_replaces_an_unchanged_released_v1_file_whole() {
        for language in Language::ALL {
            let t = template(language);
            for release in ["v0.1.0", "v0.1.1"] {
                let old = golden(&format!("{release}-{language}"));
                let u = update(Some(&old), &t, false).unwrap();
                assert_eq!((u.action, u.from), (Action::Rewritten, Some(1)));
                assert_eq!(u.text, t);
            }
        }
    }

    #[test]
    fn update_refuses_a_damaged_or_newer_block_and_replace_takes_any() {
        let t = template(Language::En);
        let damaged = "<!-- seldon:begin rules v2 -->\nno end\n";
        let e = update(Some(damaged), &t, false).unwrap_err();
        assert!(
            e.contains("has no end marker line") && e.contains("--replace"),
            "{e}"
        );
        let newer = "<!-- seldon:begin rules v3 -->\nx\n<!-- seldon:end -->\n";
        let e = update(Some(newer), &t, false).unwrap_err();
        assert!(e.contains("v3") && e.contains("update seldon"), "{e}");
        for old in [damaged, newer, "mine\n"] {
            let u = update(Some(old), &t, true).unwrap();
            assert_eq!((u.action, u.text.as_str()), (Action::Replaced, t.as_str()));
        }
        assert_eq!(
            update(Some(&t), &t, true).unwrap().action,
            Action::Unchanged
        );
        let u = update(None, &t, false).unwrap();
        assert_eq!(
            (u.action, u.from, u.text.as_str()),
            (Action::Created, None, t.as_str())
        );
    }

    #[test]
    fn diff_shows_the_changed_lines_only() {
        assert_eq!(diff("a\nb\n", "a\nb\n", "F"), "");
        assert_eq!(
            diff("a\nb\nc\n", "a\nx\ny\nc\n", "F"),
            "--- F\n+++ F\n@@ -2,1 +2,2 @@\n-b\n+x\n+y\n"
        );
        // pure insertion at the top
        assert_eq!(
            diff("old\n", "new\n\nold\n", "F"),
            "--- F\n+++ F\n@@ -0,0 +1,2 @@\n+new\n+\n"
        );
        // pure deletion
        assert_eq!(
            diff("a\nb\nc\n", "a\nc\n", "F"),
            "--- F\n+++ F\n@@ -2,1 +1,0 @@\n-b\n"
        );
        // a repeated line next to the change is not counted twice
        assert_eq!(
            diff("x\nx\n", "x\n", "F"),
            "--- F\n+++ F\n@@ -2,1 +1,0 @@\n-x\n"
        );
    }
}
