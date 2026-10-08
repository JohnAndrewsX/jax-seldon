//! The engine-maintained rules block of the logbook's `AGENTS.md`
//! (ADR-0027, WP-100): `<!-- seldon:begin rules vN -->` … `<!-- seldon:end -->`.
//!
//! `seldon init` writes the template, the block first and a section for
//! the user's own rules after it. `seldon rules update` brings the block of
//! an existing file up to this engine's rules and keeps everything outside
//! it; a file from before the block keeps only the user's own lines (WP-100
//! round 2). `seldon doctor` reports the block's [`State`]. Every
//! `seldon capture` brings a block nobody edited (one an earlier engine
//! shipped, word for word) up to this engine's ([`silent_upgrade`],
//! WP-111; ADR-0028 §4d: an unchanged default is upgraded, the user's own
//! text is kept). Pure functions: the commands do the reading, archiving,
//! writing and committing.
//!
//! Marker lines count only as whole lines (`\n` or `\r\n`); the block ends
//! at the first `<!-- seldon:end -->` line after its begin marker. The
//! template's block holds no marker text of its own (a test checks), so
//! the user's text below it, which may quote markers, never ends it.

use std::borrow::Cow;

use crate::index::load::{FENCE_BEGIN, FENCE_END};

/// The rules version this engine writes.
pub const VERSION: u32 = 4;

/// The rules file, relative to the logbook root.
pub const FILE: &str = "AGENTS.md";

/// The heading above the user's own lines of an unfenced file that
/// `rules update` keeps below the template.
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

/// Every `AGENTS.md` text Seldon wrote before the rules had a block: the
/// four released ones of [`RELEASED_V1`] and the pre-release renderings of
/// WP-003, WP-024 and WP-047 (a dev logbook, and `fixtures/logbook/`, may
/// carry those). A line of an unfenced file that occurs in none of them is
/// the user's.
const V1_TEXTS: [&str; 10] = [
    include_str!("../../templates/rules-v1/AGENTS-v0.1.0-en.md"),
    include_str!("../../templates/rules-v1/AGENTS-v0.1.0-de.md"),
    include_str!("../../templates/rules-v1/AGENTS-v0.1.1-en.md"),
    include_str!("../../templates/rules-v1/AGENTS-v0.1.1-de.md"),
    include_str!("../../templates/rules-v1/AGENTS-wp003-en.md"),
    include_str!("../../templates/rules-v1/AGENTS-wp003-de.md"),
    include_str!("../../templates/rules-v1/AGENTS-wp024-en.md"),
    include_str!("../../templates/rules-v1/AGENTS-wp024-de.md"),
    include_str!("../../templates/rules-v1/AGENTS-wp047-en.md"),
    include_str!("../../templates/rules-v1/AGENTS-wp047-de.md"),
];

/// sha256 of every rules block an earlier engine shipped (LF line ends),
/// en and de: rewriting one of them loses nothing, any other block was
/// edited and its file is archived first; a file whose block is one of
/// them is upgraded by the next capture ([`silent_upgrade`]). A change of
/// the block text adds the old one here, with its rendering under
/// `templates/rules-v<N>/`: the v2 blocks of WP-100 (rounds 1, 2 and the
/// merged one) and of WP-101 (on `main` and the test host, in no release);
/// the v3 blocks of WP-111 (stage 1, round 2 and its follow-up, the merged
/// one; reachable from `main`, in no release); the v4 block of WP-116,
/// released in 0.1.4 (WP-143 changed the text within v4: the block's
/// version says what an agent must do, not which wording).
const RELEASED_BLOCKS: [&str; 17] = [
    // WP-100 round 1 (6625cf9), en, de
    "8246c602f96980427956697d995bec2500aa8f66cc5b8b502ca12a86b4dc5d23",
    "7831a764354213f6b847bc329f8f9a5830d7f04ac0b1421067cd1b29ca57919a",
    // WP-100 round 2 (d201048)
    "21b2c0eb5a036ce5ee9c55e48226f63afe39141b01f0ac0bb08f1c2a056f10a3",
    "66fdb593ed07d4fbfdf41fdadbaf783414649d203ec927a0651bfd0a5cd1ef3a",
    // WP-100 as merged (1a0a043)
    "b0ddf1fddb392159c078597b9d03aa7593125e0ef2a398086f4fb034f3d13bba",
    "68e9aaf90fc7b4d38bc65c859f5623ee39bd093d05f3a6f1e42824e8ca386bef",
    // WP-101 (f89381a), the last v2
    "cb59e3b806b8b1adfc2cdfd45dbb5c91c3871b92cfa7e00fc1d14d635c427292",
    "0267b6a12155d2e65efcebb49b2568afe8b52151a66cc55f4e0883c6c88b95a3",
    // WP-111 stage 1 (243dac3), the first v3
    "c308ea5d646d851a2f1ff1d5eba0d6a5080d9241f29744b4dd466f1325c28d12",
    "ce8e3aaa1c04d5938cbe3d3b3900341a74ddb39db9d9e7a56fad83b71e96e024",
    // WP-111 round 2 (9c3a7c4)
    "9759dd0c7a78e47eb478cadb87ee6f3f8837d6adcb1d98188da621ed5fe753fa",
    "738417c5601534f1c0dec752b534f12631ed1ad2349b08f67a6e0ece9851d202",
    // WP-111 round 2, `--noconfirm` (0f09a3e): de only, en as merged
    "82fb14c9c30533371878c174157469e0f7343e4d490c52cf5928f8fac23522f1",
    // WP-111 as merged (070ff1f, dd41fe2), the last v3
    "25ea43fb9358d3ab3e566ab07dd87f1d434ddd52153de49e6668d3776ee42eb4",
    "20ec344f024ae13642d296fd1f5fa248fc86fa37497584ad8fbedc5cbaa61875",
    // WP-116 as merged (868c5da), released in 0.1.4, the first v4
    "5d2c4839131378943e7c29afd149d940709e2724f22a6e0961369a63c5938056",
    "f84f7a803fe42b16b58d6bde822964aca3d26e57694ff6715edde8ec7613d5b5",
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
    /// An older block, or a block of this version with other text,
    /// exactly as an earlier engine shipped it; or a file from before the
    /// block exactly as a release wrote it (`1`). Nobody edited it: the
    /// next capture upgrades it ([`silent_upgrade`]).
    Unedited(u32),
    /// An older block that was edited, or none (`1`, a file from before
    /// the block that is not a released one).
    Outdated(u32),
    /// A block of this version whose text differs from this engine's.
    Changed,
    /// A block newer than this engine's.
    Newer(u32),
    Damaged(&'static str),
    Missing,
    /// The file is not UTF-8 text (doctor reads the bytes).
    NotUtf8,
}

impl State {
    /// The doctor row's wording.
    pub fn label(&self) -> String {
        match self {
            State::Current => format!("current (v{VERSION})"),
            State::Unedited(v) => {
                format!("v{v} as Seldon wrote it; the next capture updates it to v{VERSION}")
            }
            State::Outdated(v) => format!("outdated (v{v})"),
            State::Changed => {
                format!("outdated (v{VERSION}, its text differs from this seldon's rules)")
            }
            State::Newer(v) => format!("newer (v{v}) than this seldon's rules (v{VERSION})"),
            State::Damaged(why) => format!("damaged: the rules block {why}"),
            State::Missing => "missing".into(),
            State::NotUtf8 => "invalid (not UTF-8)".into(),
        }
    }
}

/// The state of `text` (`None`: no file) against the rendered `template`.
pub fn state(text: Option<&str>, template: &str) -> State {
    let Some(text) = text else {
        return State::Missing;
    };
    match find(text) {
        Block::Unfenced if is_released_v1(text) => State::Unedited(1),
        Block::Unfenced => State::Outdated(1),
        Block::Damaged(why) => State::Damaged(why),
        Block::Fenced { version, .. } if version > VERSION => State::Newer(version),
        Block::Fenced {
            version,
            start,
            end,
            ..
        } => {
            let block = lf(&text[start..end]);
            if version == VERSION && block == template_block(template) {
                State::Current
            } else if is_released_block(&block) {
                State::Unedited(version)
            } else if version < VERSION {
                State::Outdated(version)
            } else {
                State::Changed
            }
        }
    }
}

/// The new text of the rules file `old` when the next capture upgrades it
/// without asking: its block is one an earlier engine shipped, word for
/// word, or the file is a released v1 file ([`State::Unedited`]). The
/// text outside the block stays byte for byte; nothing is archived,
/// because nothing in it is the user's. `None` for every other file: an
/// edited block keeps the doctor row and its fix, a missing file stays
/// missing.
pub fn silent_upgrade(old: &str, template: &str) -> Option<Update> {
    if !matches!(state(Some(old), template), State::Unedited(_)) {
        return None;
    }
    update(Some(old), template, false)
        .ok()
        .filter(|u| u.action == Action::Rewritten && !u.archive)
}

/// Whether `block` (LF line ends) is a block an earlier engine shipped.
fn is_released_block(block: &str) -> bool {
    RELEASED_BLOCKS.contains(&crate::sys::sha256_hex(block.as_bytes()).as_str())
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
    /// The block is replaced by this engine's and nothing outside it
    /// changes; or a file from before the block that holds no line of the
    /// user's is replaced by the template.
    Rewritten,
    /// A file from before the block with lines of the user's: the
    /// template, then those lines below [`KEPT_HEADING`].
    Kept,
    /// `--replace`: the old file is archived, the template written.
    Replaced,
}

impl Action {
    pub fn as_str(self) -> &'static str {
        match self {
            Action::Unchanged => "unchanged",
            Action::Created => "created",
            Action::Rewritten => "rewritten",
            Action::Kept => "kept",
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
    /// The old file goes to `archive/` before the write: it held text of
    /// the user's that the new file does not keep as it was.
    pub archive: bool,
}

/// The new text of the rules file `old` (`None`: no file) with the
/// rendered `template`, and whether the caller archives the old file
/// first. `replace` writes the template whatever the file holds. A fenced
/// file gets its block rewritten (archived first when the block is no
/// block Seldon wrote: the user edited it); an unfenced one that is not a
/// released v1 file is archived and becomes the template plus its own
/// lines ([`own_lines`]). `Err` is the user error: a damaged block or a
/// newer one, where the file stays as it is.
pub fn update(old: Option<&str>, template: &str, replace: bool) -> Result<Update, String> {
    let Some(old) = old else {
        return Ok(Update {
            action: Action::Created,
            from: None,
            text: template.to_string(),
            archive: false,
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
            archive: action == Action::Replaced,
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
            let archive = action == Action::Rewritten && !is_seldon_block(&old[start..end]);
            Ok(Update {
                action,
                from,
                text,
                archive,
            })
        }
        Block::Unfenced if is_released_v1(old) => Ok(Update {
            action: Action::Rewritten,
            from,
            text: template.to_string(),
            archive: false,
        }),
        Block::Unfenced => {
            let own = own_lines(old);
            let (action, text) = if own.is_empty() {
                (Action::Rewritten, template.to_string())
            } else {
                (Action::Kept, format!("{template}\n{KEPT_HEADING}\n\n{own}"))
            };
            Ok(Update {
                action,
                from,
                text,
                // a blank file holds nothing to keep
                archive: !old.trim().is_empty(),
            })
        }
    }
}

/// Whether `block` (begin through end marker line) is a block Seldon
/// wrote: this engine's, in any language, or a released one.
fn is_seldon_block(block: &str) -> bool {
    let block = lf(block);
    let ours = crate::model::Language::ALL.iter().any(|&language| {
        let t = crate::logbook::templates::find(FILE).expect("a built-in AGENTS.md template");
        template_block(t.text(language)) == block
    });
    ours || is_released_block(&block)
}

/// The lines of an unfenced `old` that occur in no text Seldon wrote
/// before the block ([`V1_TEXTS`]), in their order, with `\n` line ends;
/// blank lines only between kept lines, runs of them as one. A run of the
/// user's lines that follows Seldon's lines gets the last `## ` heading of
/// Seldon's above it, once, unless that heading already stands over it in
/// the output or the run starts with a heading of its own: a bullet the
/// user added under v1's `## Never` stays a "never" (WP-100 round 3).
/// Empty when nothing is the user's.
pub fn own_lines(old: &str) -> String {
    let known: std::collections::HashSet<&str> = V1_TEXTS
        .iter()
        .flat_map(|t| t.lines())
        .filter(|l| !l.trim().is_empty())
        .collect();
    let mut out = String::new();
    let mut gap = false;
    // the last `## ` heading of Seldon's lines, and the heading the output
    // stands under (Seldon's, emitted, or the user's own)
    let mut seldon_heading: Option<&str> = None;
    let mut shown_heading: Option<&str> = None;
    let mut after_known = false;
    for line in old.lines() {
        if line.trim().is_empty() {
            gap = !out.is_empty();
            continue;
        }
        if known.contains(line) {
            if line.starts_with("## ") {
                seldon_heading = Some(line);
            }
            after_known = true;
            continue;
        }
        if line.starts_with("## ") {
            shown_heading = Some(line);
        } else if after_known
            && let Some(heading) = seldon_heading
            && shown_heading != Some(heading)
        {
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str(heading);
            out.push('\n');
            shown_heading = Some(heading);
            gap = false;
        }
        after_known = false;
        if gap {
            out.push('\n');
            gap = false;
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

/// Whether `text` is an `AGENTS.md` exactly as a release wrote it before
/// the block ([`RELEASED_V1`]); line ends do not count (a CRLF copy is
/// the same file).
pub fn is_released_v1(text: &str) -> bool {
    RELEASED_V1.contains(&crate::sys::sha256_hex(lf(text).as_bytes()).as_str())
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
            "{}/templates/rules-v1/AGENTS-{name}.md",
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

    /// Every v2 rendering on `main` (WP-100 rounds 1 and 2, WP-100 as
    /// merged, WP-101), en and de.
    const V2_FILES: [&str; 4] = ["wp100r1", "wp100r2", "wp100", "wp101"];

    /// Every v3 rendering reachable from `main` (WP-111 stage 1, round 2,
    /// its `--noconfirm` follow-up, WP-111 as merged), by language: the
    /// follow-up's en block is the merged one.
    const V3_FILES: [(&str, &str); 7] = [
        ("wp111r1", "en"),
        ("wp111r1", "de"),
        ("wp111r2", "en"),
        ("wp111r2", "de"),
        ("wp111r2b", "de"),
        ("wp111", "en"),
        ("wp111", "de"),
    ];

    /// Every v4 rendering reachable from `main` (WP-116 as merged, 0.1.4).
    const V4_FILES: [&str; 1] = ["wp116"];

    fn rendering(version: u32, name: &str, language: &str) -> String {
        let path = format!(
            "{}/templates/rules-v{version}/AGENTS-{name}-{language}.md",
            env!("CARGO_MANIFEST_DIR")
        );
        std::fs::read_to_string(&path).unwrap()
    }

    /// Every earlier rendering with a block: (version, file, language).
    fn shipped() -> Vec<(u32, &'static str, &'static str)> {
        V2_FILES
            .iter()
            .flat_map(|name| ["en", "de"].map(|l| (2, *name, l)))
            .chain(V3_FILES.iter().map(|(name, l)| (3, *name, *l)))
            .chain(
                V4_FILES
                    .iter()
                    .flat_map(|name| ["en", "de"].map(|l| (4, *name, l))),
            )
            .collect()
    }

    /// Every block an earlier engine shipped is known by its hash, and
    /// the list holds nothing else (WP-111, WP-116).
    #[test]
    fn the_released_blocks_are_exactly_the_shipped_blocks() {
        let mut hashes: Vec<String> = shipped()
            .into_iter()
            .map(|(v, name, l)| rendering(v, name, l))
            .map(|text| crate::sys::sha256_hex(block(&text).unwrap().as_bytes()))
            .collect();
        hashes.sort();
        hashes.dedup();
        assert_eq!(
            hashes.len(),
            RELEASED_BLOCKS.len(),
            "one rendering per hash"
        );
        let mut listed: Vec<String> = RELEASED_BLOCKS.iter().map(|h| h.to_string()).collect();
        listed.sort();
        assert_eq!(hashes, listed);
        // this engine's own block is not among them: it is current
        for language in Language::ALL {
            assert!(!is_released_block(template_block(&template(language))));
        }
    }

    /// A shipped v2, v3 or v4 block nobody edited: unedited, upgraded by
    /// the next capture without an archive, the user's part kept; an
    /// edited one is outdated (an edited v4 one: changed), left to `rules
    /// update`, which archives it (WP-111, WP-143).
    #[test]
    fn a_shipped_block_is_upgraded_silently_an_edited_one_is_not() {
        for (version, file, name) in shipped() {
            let language = if name == "en" {
                Language::En
            } else {
                Language::De
            };
            let t = template(language);
            {
                let old = format!("{}- my own rule\n", rendering(version, file, name));
                assert_ne!(old, t, "{file}-{name}: the block text changed");
                assert_eq!(
                    state(Some(&old), &t),
                    State::Unedited(version),
                    "{file}-{name}"
                );
                let u = silent_upgrade(&old, &t).unwrap();
                assert_eq!(
                    (u.action, u.from, u.archive),
                    (Action::Rewritten, Some(version), false)
                );
                assert_eq!(u.text, format!("{t}- my own rule\n"), "{file}-{name}");
                assert_eq!(state(Some(&u.text), &t), State::Current);
                assert!(silent_upgrade(&u.text, &t).is_none(), "idempotent");
                // CRLF: the same block
                let crlf = old.replace('\n', "\r\n");
                let u = silent_upgrade(&crlf, &t).unwrap();
                assert!(u.text.ends_with("- my own rule\r\n"));
                assert_eq!(state(Some(&u.text), &t), State::Current);

                let edited = old.replacen("\n# AGENTS.md\n", "\n# AGENTS\n", 1);
                let expected = if version < VERSION {
                    State::Outdated(version)
                } else {
                    State::Changed
                };
                assert_eq!(state(Some(&edited), &t), expected, "{file}-{name}");
                assert!(silent_upgrade(&edited, &t).is_none());
                assert!(update(Some(&edited), &t, false).unwrap().archive);
            }
        }
    }

    #[test]
    fn only_an_unedited_default_is_upgraded_silently() {
        let t = template(Language::En);
        // a released v1 file: replaced whole
        let u = silent_upgrade(&golden("v0.1.1-en"), &t).unwrap();
        assert_eq!(
            (u.action, u.from, u.text.as_str()),
            (Action::Rewritten, Some(1), t.as_str())
        );
        assert_eq!(state(Some(&golden("v0.1.0-de")), &t), State::Unedited(1));
        // left alone: a v1 file with a line of the user's, a pre-release
        // rendering, a blank file, an edited block of this version, a
        // damaged or newer block, this engine's block in another language,
        // and the current file
        let edited = t.replacen("Rules for every agent", "Rules for any agent", 1);
        for old in [
            format!("{}- mine\n", golden("v0.1.1-en")),
            golden("wp003-en"),
            String::new(),
            edited,
            "<!-- seldon:begin rules v2 -->\nno end\n".to_string(),
            "<!-- seldon:begin rules v9 -->\nx\n<!-- seldon:end -->\n".to_string(),
            template(Language::De),
            t.clone(),
        ] {
            assert!(silent_upgrade(&old, &t).is_none(), "{old:?}");
        }
        assert_eq!(state(Some(&golden("wp003-en")), &t), State::Outdated(1));
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
        // an end marker with text after it does not end the block (the
        // text would be swallowed)
        assert_eq!(
            find("<!-- seldon:begin rules v2 -->\n<!-- seldon:end --> keep me\n"),
            Block::Damaged("has a marker inside it before its end marker line")
        );
        // a begin marker without its closing ` -->`
        assert_eq!(
            find("<!-- seldon:begin rules v2\n<!-- seldon:end -->\n"),
            Block::Damaged("has a begin marker without a version")
        );
    }

    #[test]
    fn state_tells_every_case() {
        let t = template(Language::En);
        assert_eq!(state(None, &t), State::Missing);
        assert_eq!(state(Some(&golden("v0.1.1-en")), &t), State::Unedited(1));
        let v1 = "<!-- seldon:begin rules v1 -->\nold\n<!-- seldon:end -->\n";
        assert_eq!(state(Some(v1), &t), State::Outdated(1));
        let v5 = "<!-- seldon:begin rules v5 -->\nnew\n<!-- seldon:end -->\n";
        assert_eq!(state(Some(v5), &t), State::Newer(5));
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
        // the pre-release renderings are known lines, not released files
        for name in ["wp003-en", "wp024-de", "wp047-en"] {
            assert!(!is_released_v1(&golden(name)), "{name}");
            assert_eq!(own_lines(&golden(name)), "", "{name}");
        }
        let edited = format!("{}\n- my rule\n", golden("v0.1.1-en"));
        assert!(!is_released_v1(&edited));
        // CRLF line ends: the same file (round 2, N5)
        let crlf = golden("v0.1.0-de").replace('\n', "\r\n");
        assert!(is_released_v1(&crlf));
        let t = template(Language::De);
        assert_eq!(state(Some(&crlf), &t), State::Unedited(1));
        let u = silent_upgrade(&crlf, &t).unwrap();
        assert_eq!(
            (u.action, u.archive, u.text.as_str()),
            (Action::Rewritten, false, t.as_str())
        );
    }

    #[test]
    fn update_rewrites_the_block_only() {
        let t = template(Language::En);
        let block = template_block(&t);
        let old = "# Mine\n\nbefore\n<!-- seldon:begin rules v1 -->\nold rules\n<!-- seldon:end -->\nafter `<!-- seldon:end -->`\n";
        let u = update(Some(old), &t, false).unwrap();
        assert_eq!(u.action, Action::Rewritten);
        assert_eq!(u.from, Some(1));
        // no Seldon release wrote that block: the user's copy is archived
        assert!(u.archive);
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
    fn a_seldon_block_is_rewritten_without_archive_an_edited_one_with() {
        for language in Language::ALL {
            let t = template(language);
            // this engine's block in the other language: Seldon's text
            let other = template(match language {
                Language::En => Language::De,
                Language::De => Language::En,
            });
            let u = update(Some(&other), &t, false).unwrap();
            assert_eq!(
                (u.action, u.archive),
                (Action::Rewritten, false),
                "{language}"
            );
            // a line added inside the block (N1): archived before the rewrite
            let edited = t.replacen(
                "## Never\n",
                "## Never\n\n- MY RULE: never touch /opt.\n",
                1,
            );
            assert_eq!(state(Some(&edited), &t), State::Changed);
            let u = update(Some(&edited), &t, false).unwrap();
            assert_eq!(
                (u.action, u.archive),
                (Action::Rewritten, true),
                "{language}"
            );
            assert_eq!(u.text, t);
        }
    }

    #[test]
    fn an_edited_v1_file_keeps_only_the_users_lines_and_is_archived() {
        let t = template(Language::En);
        let old = format!(
            "{}\n## Mine\n\n- Never touch ~/Music.\n\n\n\n- Ask before AUR installs.\n",
            golden("v0.1.1-en")
        );
        let u = update(Some(&old), &t, false).unwrap();
        assert_eq!((u.action, u.from, u.archive), (Action::Kept, Some(1), true));
        assert_eq!(
            u.text,
            format!(
                "{t}\n{KEPT_HEADING}\n\n## Mine\n\n- Never touch ~/Music.\n\n- Ask before AUR installs.\n"
            )
        );
        assert_eq!(state(Some(&u.text), &t), State::Current);
        let again = update(Some(&u.text), &t, false).unwrap();
        assert_eq!(again.action, Action::Unchanged);
        // lines of any v1 text Seldon wrote count as Seldon's, in either
        // language and wherever they stand; CRLF does not matter
        let mixed = format!("my own\r\n{}", golden("wp024-de").replace('\n', "\r\n"));
        let u = update(Some(&mixed), &t, false).unwrap();
        assert_eq!(u.text, format!("{t}\n{KEPT_HEADING}\n\nmy own\n"));
    }

    #[test]
    fn a_users_lines_keep_the_v1_heading_they_stand_under() {
        // B1 (round 3): bullets appended to v1's `## Never` stay limits
        let t = template(Language::En);
        let old = format!(
            "{}- Install anything from the AUR.\n- Touch /opt.\n",
            golden("v0.1.1-en")
        );
        assert_eq!(
            own_lines(&old),
            "## Never\n- Install anything from the AUR.\n- Touch /opt.\n"
        );
        let u = update(Some(&old), &t, false).unwrap();
        assert!(
            u.text.ends_with(&format!(
                "{KEPT_HEADING}\n\n## Never\n- Install anything from the AUR.\n- Touch /opt.\n"
            )),
            "{}",
            &u.text[t.len()..]
        );
        // two runs under two v1 headings; a second run under the same
        // heading gets no second copy of it (the blank line before it is
        // v1's, before "Area rules:")
        let v1 = golden("v0.1.1-en");
        let old = v1
            .replacen("## Drift\n", "## Drift\n\n- Ask me about drift.\n", 1)
            .replacen("## Never\n", "## Never\n\n- Touch /opt.\n", 1)
            + "- Install from the AUR.\n";
        assert_eq!(
            own_lines(&old),
            "## Drift\n- Ask me about drift.\n\n## Never\n- Touch /opt.\n\n- Install from the AUR.\n"
        );
        // a run that starts with its own heading gets none of Seldon's
        let old = format!("{v1}\n## Mine\n- Touch /opt.\n");
        assert_eq!(own_lines(&old), "## Mine\n- Touch /opt.\n");
        // own lines before any v1 heading: no heading
        let old = format!("- First.\n{v1}");
        assert_eq!(own_lines(&old), "- First.\n");
    }

    #[test]
    fn a_v1_file_without_own_lines_becomes_the_template() {
        let t = template(Language::De);
        // an older rendering with blank lines around it:
        // not a released file, so archived, but nothing of the user's
        let old = format!("\n\n{}\n\n", golden("wp003-de"));
        let u = update(Some(&old), &t, false).unwrap();
        assert_eq!(
            (u.action, u.archive, u.text.as_str()),
            (Action::Rewritten, true, t.as_str())
        );
        // an empty or blank file: nothing to keep, nothing to archive (N3)
        for blank in ["", "\n", "  \n\t\n"] {
            let u = update(Some(blank), &t, false).unwrap();
            assert_eq!(
                (u.action, u.archive, u.text.as_str()),
                (Action::Rewritten, false, t.as_str()),
                "{blank:?}"
            );
        }
    }

    #[test]
    fn the_fixture_logbook_keeps_exactly_its_own_line() {
        // fixtures/logbook/AGENTS.md is WP-003's rendering plus one line of
        // its own (WP-100 round 2)
        let fixture = std::fs::read_to_string(format!(
            "{}/../fixtures/logbook/AGENTS.md",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap();
        assert!(!is_released_v1(&fixture));
        let t = template(Language::De);
        assert_eq!(state(Some(&fixture), &t), State::Outdated(1));
        let golden = std::fs::read_to_string(format!(
            "{}/tests/golden/rules-kept-fixture.md",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap();
        assert_eq!(own_lines(&fixture), golden);
        let u = update(Some(&fixture), &t, false).unwrap();
        assert_eq!(u.text, format!("{t}\n{KEPT_HEADING}\n\n{golden}"));
    }

    #[test]
    fn update_replaces_an_unchanged_released_v1_file_whole() {
        for language in Language::ALL {
            let t = template(language);
            for release in ["v0.1.0", "v0.1.1"] {
                let old = golden(&format!("{release}-{language}"));
                let u = update(Some(&old), &t, false).unwrap();
                assert_eq!((u.action, u.from), (Action::Rewritten, Some(1)));
                assert!(!u.archive);
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
        let newer = "<!-- seldon:begin rules v5 -->\nx\n<!-- seldon:end -->\n";
        let e = update(Some(newer), &t, false).unwrap_err();
        assert!(e.contains("v5") && e.contains("update seldon"), "{e}");
        for old in [damaged, newer, "mine\n"] {
            let u = update(Some(old), &t, true).unwrap();
            assert_eq!((u.action, u.text.as_str()), (Action::Replaced, t.as_str()));
            assert!(u.archive);
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
