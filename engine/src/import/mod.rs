//! `seldon import` (SPEC-ENGINE §3, WP-043): bring an earlier logbook into
//! this one, dry run first.
//!
//! An importer reads its source read-only and builds a plan
//! ([`omarchy_agent::Plan`]): every file it will create or extend, with its
//! final bytes, plus a report of what it maps, skips and redacts. The dry
//! run writes only the report; `--apply` writes the plan.
//!
//! The helpers here know Markdown only as far as an import needs it:
//! level-2 sections outside code fences, heading demotion, and the
//! [`Scrubber`] that every imported line passes (SPEC-ENGINE §7 redaction
//! plus private home paths rewritten to `~`).

pub mod omarchy_agent;
pub mod report;

use std::collections::BTreeMap;

use regex::Regex;

use crate::redact::Redactor;

/// `outputs/IMPORT-<source>.md`, relative to the logbook root.
pub fn report_path(source: &str) -> String {
    format!("outputs/IMPORT-{source}.md")
}

/// `.seldon/imports/<source>.json`: written by `--apply`; its presence
/// makes a second apply a no-op.
pub fn marker_path(source: &str) -> String {
    format!(".seldon/imports/{source}.json")
}

/// One redacted line of a source file (the report names the rule, never
/// the text).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Hit {
    /// Source path, relative to the vault.
    pub file: String,
    /// 1-based line number in the source file.
    pub line: usize,
    pub rule: &'static str,
}

/// Redaction and private-path rewriting over every imported line.
#[derive(Debug, Clone)]
pub struct Scrubber {
    redactor: Redactor,
    home: Regex,
    /// Redacted lines, in the order they were seen.
    pub hits: Vec<Hit>,
    /// Home paths rewritten to `~`.
    pub private_paths: usize,
}

impl Scrubber {
    pub fn new(redactor: Redactor) -> Self {
        Scrubber {
            redactor,
            // `/home/<user>` at the start of a path: not inside another
            // path (`backups/home/x` stays), so the character before it is
            // the start, white space, a quote or punctuation
            home: Regex::new(r#"(^|[\s`'"(\[<=:,;*])/home/[A-Za-z0-9._-]+"#)
                .expect("home pattern compiles"),
            hits: Vec::new(),
            private_paths: 0,
        }
    }

    /// `text` (a whole source file) with every line scrubbed; `file` and
    /// the line numbers go into [`Scrubber::hits`]. Line endings are kept.
    pub fn text(&mut self, file: &str, text: &str) -> String {
        let mut out = String::with_capacity(text.len());
        for (n, line) in text.split_inclusive('\n').enumerate() {
            let (content, ending) = line.split_at(line.trim_end_matches(['\n', '\r']).len());
            out.push_str(&self.line(file, n + 1, content));
            out.push_str(ending);
        }
        out
    }

    /// One line, scrubbed.
    pub fn line(&mut self, file: &str, line: usize, text: &str) -> String {
        let mut out = text.to_string();
        let rules = self.redactor.matching_rules(&out);
        if !rules.is_empty() {
            out = self.redactor.redact(&out);
            for rule in rules {
                self.hits.push(Hit {
                    file: file.to_string(),
                    line,
                    rule,
                });
            }
        }
        let found = self.home.find_iter(&out).count();
        if found > 0 {
            self.private_paths += found;
            out = self.home.replace_all(&out, "${1}~").into_owned();
        }
        out
    }

    /// Redacted lines per rule, sorted by rule name.
    pub fn by_rule(&self) -> BTreeMap<&'static str, usize> {
        let mut out = BTreeMap::new();
        for h in &self.hits {
            *out.entry(h.rule).or_insert(0) += 1;
        }
        out
    }
}

/// Rewrites the ids of renumbered cases in imported text: `[[C-OLD…` and
/// a bare `C-OLD` become `C-NEW` (a slug after the id stays), so a link or
/// a mention never points at the logbook's own case of that id. One pass
/// over each text, so a new id is never rewritten again.
#[derive(Debug, Clone)]
pub struct Rewriter {
    map: BTreeMap<String, String>,
    re: Regex,
    /// Per source file: (wikilinks, bare ids) rewritten.
    pub by_file: BTreeMap<String, (usize, usize)>,
}

impl Rewriter {
    /// `map`: old id → new id, renumbered cases only.
    pub fn new(map: BTreeMap<String, String>) -> Self {
        Rewriter {
            map,
            re: Regex::new(r"(\[\[)?\bC-\d{4}-\d{3,}\b").expect("id pattern compiles"),
            by_file: BTreeMap::new(),
        }
    }

    /// `text` with the renumbered ids replaced; counted under `file`.
    pub fn text(&mut self, file: &str, text: &str) -> String {
        if self.map.is_empty() {
            return text.to_string();
        }
        let mut out = String::with_capacity(text.len());
        let mut last = 0;
        let (mut links, mut bare) = (0, 0);
        for caps in self.re.captures_iter(text) {
            let whole = caps.get(0).expect("group 0");
            let link = caps.get(1).is_some();
            let id = &whole.as_str()[if link { 2 } else { 0 }..];
            let Some(new) = self.map.get(id) else {
                continue;
            };
            out.push_str(&text[last..whole.start()]);
            if link {
                out.push_str("[[");
                links += 1;
            } else {
                bare += 1;
            }
            out.push_str(new);
            last = whole.end();
        }
        out.push_str(&text[last..]);
        if links + bare > 0 {
            let e = self.by_file.entry(file.to_string()).or_insert((0, 0));
            e.0 += links;
            e.1 += bare;
        }
        out
    }

    /// (wikilinks, bare ids) over every file.
    pub fn totals(&self) -> (usize, usize) {
        self.by_file
            .values()
            .fold((0, 0), |(l, b), (x, y)| (l + x, b + y))
    }
}

/// Splits `text` into its YAML frontmatter block (without the `---` lines)
/// and the body. No frontmatter: `None` and the whole text. Like the
/// logbook's reader (`frontmatter::Document::parse`, WP-066), a leading
/// UTF-8 BOM is skipped (and never part of the result) and a fence may
/// have spaces or tabs after its `---`.
pub fn split_frontmatter(text: &str) -> (Option<&str>, &str) {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let Some(open) = text
        .split_inclusive('\n')
        .next()
        .filter(|l| l.ends_with('\n') && is_fence_line(l))
    else {
        return (None, text);
    };
    let rest = &text[open.len()..];
    let mut pos = 0;
    for line in rest.split_inclusive('\n') {
        if is_fence_line(line) {
            return (Some(&rest[..pos]), &rest[pos + line.len()..]);
        }
        pos += line.len();
    }
    (None, text)
}

/// `---`, then only spaces or tabs up to the line end.
fn is_fence_line(line: &str) -> bool {
    line.trim_end_matches(['\n', '\r'])
        .strip_prefix("---")
        .is_some_and(|pad| pad.chars().all(|c| c == ' ' || c == '\t'))
}

/// A frontmatter block as a YAML mapping; anything else (no block, not a
/// mapping, invalid YAML) is an empty mapping.
pub fn yaml_map(block: Option<&str>) -> serde_yaml::Mapping {
    block
        .and_then(|b| serde_yaml::from_str::<serde_yaml::Value>(b).ok())
        .and_then(|v| match v {
            serde_yaml::Value::Mapping(m) => Some(m),
            _ => None,
        })
        .unwrap_or_default()
}

/// A scalar of `map` as text: strings as they are, numbers and booleans
/// printed; null, empty and nested values are `None`.
pub fn yaml_str(map: &serde_yaml::Mapping, key: &str) -> Option<String> {
    let text = match map.get(key)? {
        serde_yaml::Value::String(s) => s.clone(),
        serde_yaml::Value::Number(n) => n.to_string(),
        serde_yaml::Value::Bool(b) => b.to_string(),
        _ => return None,
    };
    let text = text.trim().to_string();
    (!text.is_empty()).then_some(text)
}

/// A list of scalars (`[a, b]`); a single scalar is a list of one.
pub fn yaml_list(map: &serde_yaml::Mapping, key: &str) -> Vec<String> {
    match map.get(key) {
        Some(serde_yaml::Value::Sequence(items)) => items
            .iter()
            .filter_map(|v| match v {
                serde_yaml::Value::String(s) => Some(s.trim().to_string()),
                serde_yaml::Value::Number(n) => Some(n.to_string()),
                serde_yaml::Value::Bool(b) => Some(b.to_string()),
                _ => None,
            })
            .filter(|s| !s.is_empty())
            .collect(),
        Some(_) => yaml_str(map, key).into_iter().collect(),
        None => Vec::new(),
    }
}

/// Whether `line` opens or closes a fenced code block.
fn is_fence(line: &str) -> bool {
    let t = line.trim_start();
    t.starts_with("```") || t.starts_with("~~~")
}

/// The lines of `text` with whether each lies inside a code fence (a
/// fence line itself counts as inside).
fn lines_with_fence(text: &str) -> impl Iterator<Item = (&str, bool)> {
    let mut in_fence = false;
    text.split_inclusive('\n').map(move |line| {
        if is_fence(line) {
            in_fence = !in_fence;
            return (line, true);
        }
        (line, in_fence)
    })
}

/// A Markdown body split at its level-2 headings (outside code fences):
/// the text before the first one, then `(heading text, content)` pairs.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Sections {
    pub preamble: String,
    pub sections: Vec<(String, String)>,
}

pub fn sections(body: &str) -> Sections {
    let mut out = Sections::default();
    for (line, fenced) in lines_with_fence(body) {
        let heading = (!fenced)
            .then(|| line.trim_end_matches(['\n', '\r']).strip_prefix("## "))
            .flatten();
        match (heading, out.sections.last_mut()) {
            (Some(h), _) => out.sections.push((h.trim().to_string(), String::new())),
            (None, Some((_, content))) => content.push_str(line),
            (None, None) => out.preamble.push_str(line),
        }
    }
    out
}

/// `text` with every ATX heading outside code fences one level deeper
/// (`#` → `##`, …; level 6 stays), so imported text nests under the
/// heading it is put below.
pub fn demote(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 32);
    for (line, fenced) in lines_with_fence(text) {
        let level = line.chars().take_while(|c| *c == '#').count();
        let rest = &line[level..];
        let is_heading = (1..6).contains(&level)
            && (rest.is_empty() || rest.starts_with([' ', '\t', '\n', '\r']));
        if !fenced && is_heading {
            out.push('#');
        }
        out.push_str(line);
    }
    out
}

/// `text` without leading and trailing blank lines (inner text as it is),
/// with a final newline unless it is empty.
pub fn trim_blank_lines(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let first = lines.iter().position(|l| !l.trim().is_empty());
    let last = lines.iter().rposition(|l| !l.trim().is_empty());
    match (first, last) {
        (Some(a), Some(b)) => {
            let mut s = lines[a..=b].join("\n");
            s.push('\n');
            s
        }
        _ => String::new(),
    }
}

/// `text` without its first line when that line is a level-1 heading
/// (leading blank lines skipped); returns the heading text too.
pub fn strip_title(text: &str) -> (Option<String>, String) {
    let trimmed = text.trim_start_matches(['\n', '\r']);
    let (first, rest) = trimmed.split_once('\n').unwrap_or((trimmed, ""));
    match first.trim_end().strip_prefix("# ") {
        Some(title) => (Some(title.trim().to_string()), rest.to_string()),
        None => (None, text.to_string()),
    }
}

/// A table cell: one line, `|` escaped.
pub fn cell(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .replace('|', "\\|")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scrubber_redacts_and_rewrites_home_paths() {
        let mut s = Scrubber::new(Redactor::builtin());
        let out = s.text(
            "a.md",
            "ok\ncurl -H 'Authorization: Bearer x' token=abc\ncd /home/alice/x and `/home/bob`\nbackups/home/alice/.config stays\n",
        );
        assert_eq!(
            out,
            "ok\ncurl -H 'Authorization: ‹redacted›' token=‹redacted›\ncd ~/x and `~`\nbackups/home/alice/.config stays\n"
        );
        assert_eq!(s.private_paths, 2);
        assert_eq!(
            s.hits.iter().map(|h| (h.line, h.rule)).collect::<Vec<_>>(),
            [(2, "token-assignment"), (2, "authorization-header")]
        );
        assert_eq!(s.by_rule().len(), 2);
    }

    #[test]
    fn rewriter_replaces_renumbered_ids_once() {
        let map = BTreeMap::from([
            ("C-2026-001".to_string(), "C-2026-039".to_string()),
            ("C-2026-039".to_string(), "C-2026-050".to_string()),
        ]);
        let mut r = Rewriter::new(map);
        let out = r.text(
            "a.md",
            "[[C-2026-001-quattro]] und C-2026-001, nicht C-2026-0012 oder XC-2026-001; [[C-2026-002]] bleibt.",
        );
        assert_eq!(
            out,
            "[[C-2026-039-quattro]] und C-2026-039, nicht C-2026-0012 oder XC-2026-001; [[C-2026-002]] bleibt."
        );
        assert_eq!(r.by_file["a.md"], (1, 1));
        assert_eq!(r.text("b.md", "nichts"), "nichts");
        assert_eq!(r.totals(), (1, 1));
        assert!(!r.by_file.contains_key("b.md"));
    }

    #[test]
    fn frontmatter_split_and_lenient_yaml() {
        let (fm, body) = split_frontmatter("---\nid: C-1\ntags: [a, 2]\nclosed:\n---\n# T\n");
        assert_eq!(body, "# T\n");
        let map = yaml_map(fm);
        assert_eq!(yaml_str(&map, "id").as_deref(), Some("C-1"));
        assert_eq!(yaml_list(&map, "tags"), ["a", "2"]);
        assert_eq!(yaml_str(&map, "closed"), None);
        assert_eq!(split_frontmatter("# no\n"), (None, "# no\n"));
        assert_eq!(split_frontmatter("---\nopen\n"), (None, "---\nopen\n"));
        // a BOM and padded fences, as other editors write them (WP-066)
        for text in [
            "\u{feff}---\nid: C-1\n---\n# T\n",
            "---  \nid: C-1\n---\t\n# T\n",
            "\u{feff}---\t \r\nid: C-1\r\n--- \r\n# T\n",
        ] {
            let (fm, body) = split_frontmatter(text);
            assert_eq!(fm.map(str::trim_end), Some("id: C-1"), "{text:?}");
            assert_eq!(body, "# T\n", "{text:?}");
        }
        // a BOM without frontmatter is not part of the body either
        assert_eq!(split_frontmatter("\u{feff}# T\n"), (None, "# T\n"));
        // not a fence: text after the dashes, more dashes, no line end
        for text in [
            "---x\nid: x\n---\n",
            "--- x\nid: x\n---\n",
            "----\nid: x\n---\n",
            "---",
        ] {
            assert_eq!(split_frontmatter(text), (None, text), "{text:?}");
        }
        assert!(yaml_map(Some(": : :\n  - [")).is_empty());
    }

    #[test]
    fn sections_skip_headings_in_fences() {
        let s = sections("# T\nintro\n## A\na\n```\n## not\n```\n## B\nb\n");
        assert_eq!(s.preamble, "# T\nintro\n");
        assert_eq!(
            s.sections,
            [
                ("A".to_string(), "a\n```\n## not\n```\n".to_string()),
                ("B".to_string(), "b\n".to_string())
            ]
        );
    }

    #[test]
    fn demote_headings_outside_fences() {
        assert_eq!(
            demote("# a\n## b\n###### six\n#tag\n```\n## code\n```\n##\n"),
            "## a\n### b\n###### six\n#tag\n```\n## code\n```\n###\n"
        );
    }

    #[test]
    fn trim_and_title() {
        assert_eq!(trim_blank_lines("\n\n a\n\nb\n\n"), " a\n\nb\n");
        assert_eq!(trim_blank_lines("\n \n"), "");
        assert_eq!(
            strip_title("\n# Title\nbody\n"),
            (Some("Title".to_string()), "body\n".to_string())
        );
        assert_eq!(strip_title("body\n"), (None, "body\n".to_string()));
        assert_eq!(cell("a |\nb"), "a \\| b");
    }
}
