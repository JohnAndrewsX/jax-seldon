//! `seldon import task` (WP-102): what a user's Markdown task file holds.
//!
//! A file with at least one checklist item (`- [ ]`, `- [x]`, also `*`, `+`
//! and `1.` lists) outside its frontmatter and code fences is read item by
//! item: every top-level item is one task, its indented lines (nested
//! items included) belong to it, and the nearest heading above it is its
//! section. A file without any checklist item is one task: the first
//! level-1 heading is its title, the rest its text (ADR-0027 §7).
//!
//! The parser is pure: it gets the file's (already redacted) text and
//! returns what it found; the command decides what becomes a case.

use regex::Regex;
use std::sync::LazyLock;

use super::split_frontmatter;

/// One top-level checklist item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    /// 1-based line of the item in the file (frontmatter counted).
    pub line: usize,
    /// `- [x]`: done when the file was read.
    pub done: bool,
    /// The item's text after the checkbox, then its indented lines
    /// dedented; no trailing blank lines.
    pub text: String,
    /// The text of the nearest heading above the item, if any.
    pub section: Option<String>,
}

/// A file without checklist items, as one task.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Whole {
    /// The first level-1 heading's text (outside code fences).
    pub title: Option<String>,
    /// The text without the frontmatter and that heading, blank lines
    /// around it trimmed.
    pub text: String,
}

/// What a task file holds: items, or else the whole file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Tasks {
    Items(Vec<Item>),
    Whole(Whole),
}

static CHECKBOX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[ \t]*(?:[-*+]|\d{1,9}[.)])[ \t]+\[([ xX])\](?:[ \t]+(.*))?$")
        .expect("checkbox pattern compiles")
});

static HEADING: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^ {0,3}(#{1,6})(?:[ \t]+(.*?))?(?:[ \t]+#+)?[ \t]*$")
        .expect("heading pattern compiles")
});

/// Reads `text` (a whole task file).
pub fn parse(text: &str) -> Tasks {
    let (frontmatter, body) = split_frontmatter(text);
    // the body's first line in the file: after the frontmatter block and
    // its two fences
    let offset = frontmatter.map_or(0, |fm| fm.matches('\n').count() + 2);
    let lines: Vec<&str> = body
        .split('\n')
        .map(|l| l.strip_suffix('\r').unwrap_or(l))
        .collect();
    let items = items(&lines, offset);
    if items.is_empty() {
        Tasks::Whole(whole(&lines))
    } else {
        Tasks::Items(items)
    }
}

/// The width of a line's leading white space (a tab counts 4).
fn indent(line: &str) -> usize {
    line.chars()
        .take_while(|c| *c == ' ' || *c == '\t')
        .map(|c| if c == '\t' { 4 } else { 1 })
        .sum()
}

fn is_fence(line: &str) -> bool {
    let t = line.trim_start();
    t.starts_with("```") || t.starts_with("~~~")
}

/// A heading line's text (`None` for a line that is no ATX heading).
fn heading(line: &str) -> Option<(usize, String)> {
    let caps = HEADING.captures(line)?;
    let level = caps.get(1).map_or(0, |m| m.as_str().len());
    let text = caps.get(2).map_or("", |m| m.as_str()).trim().to_string();
    Some((level, text))
}

/// An item under construction.
struct Open {
    line: usize,
    indent: usize,
    done: bool,
    first: String,
    rest: Vec<String>,
    section: Option<String>,
}

impl Open {
    fn finish(self) -> Item {
        let mut rest = self.rest;
        while rest.last().is_some_and(|l| l.trim().is_empty()) {
            rest.pop();
        }
        let cut = rest
            .iter()
            .filter(|l| !l.trim().is_empty())
            .map(|l| indent(l))
            .min()
            .unwrap_or(0);
        let mut text = self.first;
        for l in rest {
            text.push('\n');
            text.push_str(dedent(&l, cut).trim_end());
        }
        Item {
            line: self.line,
            done: self.done,
            text,
            section: self.section,
        }
    }
}

/// `line` without `width` columns of leading white space (a tab counts 4).
fn dedent(line: &str, width: usize) -> &str {
    let mut taken = 0;
    for (i, c) in line.char_indices() {
        if taken >= width || !(c == ' ' || c == '\t') {
            return &line[i..];
        }
        taken += if c == '\t' { 4 } else { 1 };
    }
    ""
}

fn items(lines: &[&str], offset: usize) -> Vec<Item> {
    let mut out = Vec::new();
    let mut open: Option<Open> = None;
    let mut section: Option<String> = None;
    let mut in_fence = false;
    for (i, line) in lines.iter().enumerate() {
        if let Some(cur) = open.as_mut() {
            // the item's block: blank lines, deeper lines, and a fence it
            // opened up to its close
            if in_fence || line.trim().is_empty() || indent(line) > cur.indent {
                if is_fence(line) {
                    in_fence = !in_fence;
                }
                cur.rest.push((*line).to_string());
                continue;
            }
            out.push(open.take().expect("open item").finish());
        }
        if is_fence(line) {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        if let Some((_, text)) = heading(line) {
            section = (!text.is_empty()).then_some(text);
            continue;
        }
        if let Some(caps) = CHECKBOX.captures(line) {
            open = Some(Open {
                line: offset + i + 1,
                indent: indent(line),
                done: caps.get(1).is_some_and(|m| m.as_str() != " "),
                first: caps.get(2).map_or("", |m| m.as_str()).trim().to_string(),
                rest: Vec::new(),
                section: section.clone(),
            });
        }
    }
    out.extend(open.map(Open::finish));
    out
}

fn whole(lines: &[&str]) -> Whole {
    let mut in_fence = false;
    let mut title = None;
    let mut kept: Vec<&str> = Vec::with_capacity(lines.len());
    for line in lines {
        if is_fence(line) {
            in_fence = !in_fence;
        } else if !in_fence
            && title.is_none()
            && let Some((1, text)) = heading(line)
        {
            title = Some(text);
            continue;
        }
        kept.push(line);
    }
    Whole {
        title: title.filter(|t| !t.is_empty()),
        text: super::trim_blank_lines(&kept.join("\n")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn items_of(text: &str) -> Vec<Item> {
        match parse(text) {
            Tasks::Items(items) => items,
            Tasks::Whole(w) => panic!("expected items, got {w:?}"),
        }
    }

    #[test]
    fn top_level_items_with_their_blocks_and_sections() {
        let text = "\
---
title: x
---
# Backlog
Intro text.

- [ ] Fix the bar. It flickers.
  On the second monitor only.

  - [ ] nested step
- [x] Done thing
## Later
* [X] star done
+ [ ] plus open
1. [ ] numbered
not an item: - [ ] mid-line
```
- [ ] in a fence
```
";
        let items = items_of(text);
        let got: Vec<(usize, bool, &str, Option<&str>)> = items
            .iter()
            .map(|i| (i.line, i.done, i.text.as_str(), i.section.as_deref()))
            .collect();
        assert_eq!(
            got,
            [
                (
                    7,
                    false,
                    "Fix the bar. It flickers.\nOn the second monitor only.\n\n- [ ] nested step",
                    Some("Backlog")
                ),
                (11, true, "Done thing", Some("Backlog")),
                (13, true, "star done", Some("Later")),
                (14, false, "plus open", Some("Later")),
                (15, false, "numbered", Some("Later")),
            ]
        );
    }

    #[test]
    fn an_item_block_keeps_its_fence_and_ends_at_a_shallower_line() {
        let text = "- [ ] a\n  ```\n# not a heading\n  ```\n- [ ] b\ntext\n- [ ] c\n";
        let items = items_of(text);
        assert_eq!(items.len(), 3);
        // the fence's inner line is not indented: nothing is cut
        assert_eq!(items[0].text, "a\n  ```\n# not a heading\n  ```");
        assert_eq!(items[0].section, None);
        assert_eq!(items[1].text, "b");
        assert_eq!(items[2].line, 7);
    }

    #[test]
    fn crlf_tabs_and_empty_items() {
        let items = items_of("- [ ]\r\n\t- [ ] tabbed\r\n-   [ ]   spaced  \r\n");
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].text, "\n- [ ] tabbed");
        assert_eq!(items[1].text, "spaced");
        // `[ ]` without a list marker, or `[]`, is no item
        assert!(matches!(parse("[ ] x\n- [] y\n"), Tasks::Whole(_)));
    }

    #[test]
    fn a_file_without_items_is_one_task() {
        let text = "---\na: 1\n---\n\nSome words first.\n\n# Make backups work\n\nUse restic.\n## Steps\n# second h1 stays\n";
        assert_eq!(
            parse(text),
            Tasks::Whole(Whole {
                title: Some("Make backups work".to_string()),
                text: "Some words first.\n\n\nUse restic.\n## Steps\n# second h1 stays\n"
                    .to_string(),
            })
        );
        // a heading in a fence is no title; no title at all
        assert_eq!(
            parse("```\n# x\n```\n"),
            Tasks::Whole(Whole {
                title: None,
                text: "```\n# x\n```\n".to_string()
            })
        );
        assert_eq!(
            parse("# Title only #\n"),
            Tasks::Whole(Whole {
                title: Some("Title only".to_string()),
                text: String::new()
            })
        );
    }

    #[test]
    fn dedent_counts_tabs() {
        assert_eq!(dedent("\t  x", 4), "  x");
        assert_eq!(dedent("  x", 4), "x");
        assert_eq!(dedent("   ", 2), " ");
    }
}
