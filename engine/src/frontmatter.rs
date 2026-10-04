//! YAML frontmatter of logbook Markdown files (SPEC-LOGBOOK §3).
//!
//! A [`Document`] is split into its frontmatter and its body. The body is
//! kept byte-for-byte. The frontmatter is kept as a list of entries, one per
//! top-level key, each holding the raw text of its lines, so that:
//!
//! - reading deserialises the whole block with `serde_yaml`;
//! - [`Frontmatter::set`] rewrites only the entry whose value changed and
//!   leaves comments, quoting and order of every other line alone (lossless
//!   update, keeps git diffs to one line);
//! - [`Frontmatter::canonical`] writes a fresh block in the engine's style,
//!   which is the style of `fixtures/logbook/`: one `key: value` per line,
//!   `key:` for null, flow lists `[a, b]`, free text in double quotes.
//!
//! Frontmatter stays flat (memory/pitfalls.md): scalars and lists of scalars.
//!
//! Files from other editors parse too: a leading UTF-8 BOM (kept on render),
//! spaces or tabs after a fence, quoted keys, and blank or column-0 comment
//! lines inside a block list. Before a changed block is written,
//! [`Frontmatter::check`] reads it back (WP-066).

use std::fmt::Write as _;

use serde::de::DeserializeOwned;
use serde_yaml::Value as Yaml;

const FENCE: &str = "---";
const BOM: char = '\u{feff}';

/// Errors while reading frontmatter.
#[derive(Debug, thiserror::Error)]
pub enum FrontmatterError {
    #[error("no frontmatter (the file must start with a `---` line)")]
    Missing,
    #[error("frontmatter is not closed by a `---` line")]
    Unterminated,
    /// serde names a value it refuses (`unknown variant`) as it is: the
    /// message is shown [`printable`].
    #[error("frontmatter is not valid YAML: {}", printable(&.0.to_string()))]
    Yaml(#[from] serde_yaml::Error),
    #[error("`{key}`: {message}")]
    Field { key: String, message: String },
    #[error(
        "update refused, the frontmatter would not read back ({0}); \
         the file is left as it was, rewrite the hand-edited lines in plain YAML"
    )]
    Refused(String),
}

/// `text` with every character [`char::escape_debug`] would escape, except
/// quotes and backslashes, escaped: a message that embeds a refused value
/// never prints its control or format characters (WP-077).
pub fn printable(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if matches!(c, '"' | '\'' | '\\') {
            out.push(c);
        } else {
            out.extend(c.escape_debug());
        }
    }
    out
}

/// A Markdown file: optional frontmatter plus the body after it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Document {
    pub frontmatter: Option<Frontmatter>,
    pub body: String,
}

impl Document {
    /// Splits `text` into frontmatter and body. A file that does not start
    /// with a `---` line (after an optional BOM) has no frontmatter; the
    /// whole text is the body.
    pub fn parse(text: &str) -> Result<Self, FrontmatterError> {
        let bom = if text.starts_with(BOM) {
            BOM.len_utf8()
        } else {
            0
        };
        let Some(open_len) = fence_len(&text[bom..]).map(|n| bom + n) else {
            return Ok(Document {
                frontmatter: None,
                body: text.to_string(),
            });
        };
        let mut lines = Vec::new();
        let mut pos = open_len;
        loop {
            if pos >= text.len() {
                return Err(FrontmatterError::Unterminated);
            }
            let end = text[pos..].find('\n').map_or(text.len(), |i| pos + i + 1);
            let line = &text[pos..end];
            if fence_len(line).is_some() {
                return Ok(Document {
                    frontmatter: Some(Frontmatter {
                        open: text[..open_len].to_string(),
                        entries: group(&lines),
                        close: line.to_string(),
                    }),
                    body: text[end..].to_string(),
                });
            }
            lines.push(line);
            pos = end;
        }
    }

    /// The file's text. `Document::parse(t)?.render() == t` for every `t`.
    pub fn render(&self) -> String {
        let mut out = String::with_capacity(self.body.len() + 256);
        if let Some(fm) = &self.frontmatter {
            out.push_str(&fm.render());
        }
        out.push_str(&self.body);
        out
    }

    pub fn frontmatter(&self) -> Result<&Frontmatter, FrontmatterError> {
        self.frontmatter.as_ref().ok_or(FrontmatterError::Missing)
    }

    /// Puts `fm` in front of the body. A BOM at the start of the body moves
    /// in front of the block, where a reader expects it.
    pub fn set_frontmatter(&mut self, mut fm: Frontmatter) {
        if let Some(rest) = self.body.strip_prefix(BOM) {
            fm.open.insert(0, BOM);
            self.body = rest.to_string();
        }
        self.frontmatter = Some(fm);
    }
}

/// Length of a `---` fence line at the start of `s` (with its trailing
/// spaces or tabs and its line ending), or `None` if `s` does not start
/// with one.
fn fence_len(s: &str) -> Option<usize> {
    let rest = s.strip_prefix(FENCE)?;
    let after = rest.trim_start_matches([' ', '\t']);
    let eol = if after.is_empty() {
        0
    } else if after.starts_with('\n') {
        1
    } else if after.starts_with("\r\n") {
        2
    } else {
        return None;
    };
    Some(s.len() - after.len() + eol)
}

/// The frontmatter block, entry by entry, with its raw text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frontmatter {
    open: String,
    entries: Vec<Entry>,
    close: String,
}

/// One top-level key with its continuation lines, or a line that belongs to
/// no key (blank line, comment).
#[derive(Debug, Clone, PartialEq, Eq)]
struct Entry {
    key: Option<String>,
    raw: String,
}

impl Frontmatter {
    /// A fresh block in canonical style, keys in the given order.
    pub fn canonical(values: &[(&str, FmValue)]) -> Self {
        Frontmatter {
            open: format!("{FENCE}\n"),
            entries: values
                .iter()
                .map(|(key, value)| Entry {
                    key: Some((*key).to_string()),
                    raw: render_entry(key, value),
                })
                .collect(),
            close: format!("{FENCE}\n"),
        }
    }

    pub fn render(&self) -> String {
        let mut out = self.open.clone();
        for entry in &self.entries {
            out.push_str(&entry.raw);
        }
        out.push_str(&self.close);
        out
    }

    /// The YAML between the fences.
    pub fn yaml(&self) -> String {
        self.entries.iter().map(|e| e.raw.as_str()).collect()
    }

    /// Top-level keys in file order.
    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().filter_map(|e| e.key.as_deref())
    }

    /// Deserialises the whole block into `T`.
    pub fn deserialize<T: DeserializeOwned>(&self) -> Result<T, FrontmatterError> {
        let yaml = self.yaml();
        if yaml.trim().is_empty() {
            return Ok(serde_yaml::from_str("{}")?);
        }
        Ok(serde_yaml::from_str(&yaml)?)
    }

    /// The parsed value of one key; `None` if the key is absent.
    pub fn get(&self, key: &str) -> Option<Yaml> {
        let entry = self.entry(key)?;
        entry_value(entry).ok()
    }

    /// Sets `key` to `value`. An existing entry is rewritten only when its
    /// value differs (it keeps its column-0 comment lines, after the new
    /// `key: value` line); everything else stays byte-identical. A missing key is
    /// inserted before the first key that follows it in `order` (or at the
    /// end), unless `value` is empty (null or `[]`), in which case nothing is
    /// added. Returns whether the text changed.
    pub fn set(&mut self, key: &str, value: &FmValue, order: &[&str]) -> bool {
        if let Some(i) = self.position(key) {
            let target = value.to_yaml();
            // An entry's own lines may not hold its whole value (valid YAML
            // the line model cannot place): the whole block decides then.
            if entry_value(&self.entries[i]).is_ok_and(|current| current == target)
                || self
                    .block_value(key)
                    .is_some_and(|current| current == target)
            {
                return false;
            }
            let entry = &mut self.entries[i];
            let comments: String = entry
                .raw
                .split_inclusive('\n')
                .skip(1)
                .filter(|l| l.starts_with('#'))
                .collect();
            entry.raw = render_entry(key, value) + &comments;
            return true;
        }
        if value.is_empty() {
            return false;
        }
        let entry = Entry {
            key: Some(key.to_string()),
            raw: render_entry(key, value),
        };
        let later: Vec<&str> = order
            .iter()
            .skip_while(|k| **k != key)
            .skip(1)
            .copied()
            .collect();
        let at = self
            .entries
            .iter()
            .position(|e| e.key.as_deref().is_some_and(|k| later.contains(&k)))
            .unwrap_or(self.entries.len());
        self.entries.insert(at, entry);
        true
    }

    /// Reads the block back as a file would be read and checks that every
    /// key holds the value written (an absent key may stand for an empty
    /// one, as [`Frontmatter::set`] does not add those). A writer calls it
    /// before the text goes to disk: a hand edit the line model misplaced
    /// makes the update fail instead of the file.
    pub fn check(&self, values: &[(&str, FmValue)]) -> Result<(), FrontmatterError> {
        let refused = |e: &dyn std::fmt::Display| FrontmatterError::Refused(e.to_string());
        let doc = Document::parse(&self.render()).map_err(|e| refused(&e))?;
        let back = doc.frontmatter().map_err(|e| refused(&e))?;
        let mapping: serde_yaml::Mapping = back.deserialize().map_err(|e| refused(&e))?;
        for (key, value) in values {
            let ok = match mapping.get(*key) {
                Some(found) => *found == value.to_yaml(),
                None => value.is_empty(),
            };
            if !ok {
                return Err(refused(&format!("`{key}` reads back as another value")));
            }
        }
        Ok(())
    }

    /// Removes `key`. Returns whether it was present.
    pub fn remove(&mut self, key: &str) -> bool {
        match self.position(key) {
            Some(i) => {
                self.entries.remove(i);
                true
            }
            None => false,
        }
    }

    fn entry(&self, key: &str) -> Option<&Entry> {
        self.entries.iter().find(|e| e.key.as_deref() == Some(key))
    }

    fn position(&self, key: &str) -> Option<usize> {
        self.entries
            .iter()
            .position(|e| e.key.as_deref() == Some(key))
    }

    /// The value the whole block gives `key`, if the block parses.
    fn block_value(&self, key: &str) -> Option<Yaml> {
        let mapping: serde_yaml::Mapping = self.deserialize().ok()?;
        mapping.get(key).cloned()
    }
}

/// Groups the lines between the fences into entries: a `key:` line at
/// column 0 starts one; indented and `- ` lines continue it. Blank lines
/// and column-0 comments continue it too when the next other line does
/// (a comment or blank line inside a block list); otherwise they are an
/// entry without a key.
fn group(lines: &[&str]) -> Vec<Entry> {
    let mut entries: Vec<Entry> = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        if let Some(key) = key_of(lines[i]) {
            entries.push(Entry {
                key: Some(key),
                raw: lines[i].to_string(),
            });
            i += 1;
            continue;
        }
        let filler = lines[i..].iter().take_while(|l| is_filler(l)).count();
        let joins = lines.get(i + filler).is_some_and(|l| continues(l));
        // the filler run, plus the continuation line when there is no filler
        let n = if filler == 0 { 1 } else { filler };
        let raw = lines[i..i + n].concat();
        match entries.last_mut() {
            Some(last) if joins && last.key.is_some() => last.raw.push_str(&raw),
            _ => entries.push(Entry { key: None, raw }),
        }
        i += n;
    }
    entries
}

/// A blank line or a comment at column 0.
fn is_filler(line: &str) -> bool {
    line.trim().is_empty() || line.starts_with('#')
}

/// An indented line or a block list item: part of the entry above.
fn continues(line: &str) -> bool {
    line.starts_with([' ', '\t']) && !line.trim().is_empty()
        || line.starts_with("- ")
        || line.trim_end() == "-"
}

/// The key of a `key: value` line at column 0, if it is one. A quoted key
/// (`"title": …`, `'title': …`) is returned unquoted.
fn key_of(line: &str) -> Option<String> {
    let first = line.chars().next()?;
    let (key, after) = match first {
        '"' | '\'' => {
            let end = quoted_end(line, first)?;
            let key: String = serde_yaml::from_str(&line[..end]).ok()?;
            let after = line[end..].trim_start_matches([' ', '\t']);
            (key, after.strip_prefix(':')?)
        }
        c if c.is_whitespace() || matches!(c, '#' | '-' | '[' | '{') => return None,
        _ => {
            let colon = line.find(':')?;
            (line[..colon].trim_end().to_string(), &line[colon + 1..])
        }
    };
    if !(after.is_empty() || after.starts_with([' ', '\t', '\n', '\r'])) {
        return None;
    }
    (!key.is_empty()).then_some(key)
}

/// The byte index after the closing quote of the quoted scalar that
/// `line` starts with (`"…"` with `\` escapes, `'…'` with `''`).
fn quoted_end(line: &str, quote: char) -> Option<usize> {
    let mut chars = line.char_indices().skip(1);
    while let Some((i, c)) = chars.next() {
        match c {
            '\\' if quote == '"' => {
                chars.next();
            }
            c if c == quote => {
                if quote == '\'' && line[i + 1..].starts_with('\'') {
                    chars.next();
                } else {
                    return Some(i + 1);
                }
            }
            _ => {}
        }
    }
    None
}

fn entry_value(entry: &Entry) -> Result<Yaml, serde_yaml::Error> {
    let mapping: serde_yaml::Mapping = serde_yaml::from_str(&entry.raw)?;
    Ok(mapping.into_iter().next().map_or(Yaml::Null, |(_, v)| v))
}

fn render_entry(key: &str, value: &FmValue) -> String {
    match value {
        FmValue::Null => format!("{key}:\n"),
        _ => format!("{key}: {}\n", value.render()),
    }
}

/// A frontmatter value as the engine writes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FmValue {
    /// Written as `key:` with nothing after the colon.
    Null,
    Bool(bool),
    Int(i64),
    /// An identifier-like string: plain when YAML reads it back unchanged,
    /// otherwise double-quoted.
    Str(String),
    /// Free text (titles, descriptions): always double-quoted.
    Text(String),
    /// A flow list `[a, b]`; items follow the rules of [`FmValue::Str`].
    List(Vec<String>),
}

impl FmValue {
    pub fn str(s: impl Into<String>) -> Self {
        FmValue::Str(s.into())
    }

    pub fn text(s: impl Into<String>) -> Self {
        FmValue::Text(s.into())
    }

    pub fn opt_str<T: ToString>(v: Option<T>) -> Self {
        v.map_or(FmValue::Null, |v| FmValue::Str(v.to_string()))
    }

    pub fn list<T: ToString>(items: &[T]) -> Self {
        FmValue::List(items.iter().map(ToString::to_string).collect())
    }

    /// Null and `[]` are "empty": [`Frontmatter::set`] does not add them.
    pub fn is_empty(&self) -> bool {
        matches!(self, FmValue::Null) || matches!(self, FmValue::List(v) if v.is_empty())
    }

    pub fn to_yaml(&self) -> Yaml {
        match self {
            FmValue::Null => Yaml::Null,
            FmValue::Bool(b) => Yaml::Bool(*b),
            FmValue::Int(i) => Yaml::Number((*i).into()),
            FmValue::Str(s) | FmValue::Text(s) => Yaml::String(s.clone()),
            FmValue::List(items) => {
                Yaml::Sequence(items.iter().cloned().map(Yaml::String).collect())
            }
        }
    }

    fn render(&self) -> String {
        match self {
            FmValue::Null => String::new(),
            FmValue::Bool(b) => b.to_string(),
            FmValue::Int(i) => i.to_string(),
            FmValue::Str(s) if is_plain(s, false) => s.clone(),
            FmValue::Str(s) | FmValue::Text(s) => quote(s),
            FmValue::List(items) => {
                let items: Vec<String> = items
                    .iter()
                    .map(|s| {
                        if is_plain(s, true) {
                            s.clone()
                        } else {
                            quote(s)
                        }
                    })
                    .collect();
                format!("[{}]", items.join(", "))
            }
        }
    }
}

/// Whether `s` can be written as a plain YAML scalar and reads back as the
/// same string (not a number, bool, null or date-typed value). YAML 1.1
/// booleans (`yes`, `off`, …) are quoted too, for older readers.
fn is_plain(s: &str, in_flow: bool) -> bool {
    let Some(first) = s.chars().next() else {
        return false;
    };
    const YAML11_BOOLS: [&str; 8] = ["y", "n", "yes", "no", "on", "off", "true", "false"];
    if YAML11_BOOLS.contains(&s.to_ascii_lowercase().as_str()) {
        return false;
    }
    if !(first.is_ascii_alphanumeric() || matches!(first, '.' | '/' | '~' | '_'))
        || s.ends_with(':')
        || !s
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "._-/+:@~".contains(c))
    {
        return false;
    }
    if in_flow {
        serde_yaml::from_str::<Yaml>(&format!("[{s}]"))
            .is_ok_and(|v| v == Yaml::Sequence(vec![Yaml::String(s.to_string())]))
    } else {
        serde_yaml::from_str::<Yaml>(s).is_ok_and(|v| v == Yaml::String(s.to_string()))
    }
}

/// A YAML double-quoted scalar. Non-ASCII text stays as UTF-8.
fn quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            c if c.is_control() => {
                let _ = write!(out, "\\u{:04X}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const CASE: &str = "---\nid: C-2026-001\ntitle: \"A\"\nstatus: queued            # comment\nclosed:\ntags:\n  - a\n  - b\n---\n# Body\n\ntext\n";

    #[test]
    fn parse_and_render_is_identity() {
        let doc = Document::parse(CASE).unwrap();
        assert_eq!(doc.render(), CASE);
        assert_eq!(doc.body, "# Body\n\ntext\n");
        let fm = doc.frontmatter.unwrap();
        let keys: Vec<_> = fm.keys().collect();
        assert_eq!(keys, ["id", "title", "status", "closed", "tags"]);
        assert_eq!(
            fm.get("tags"),
            Some(Yaml::Sequence(vec![Yaml::from("a"), Yaml::from("b")]))
        );
        assert_eq!(fm.get("closed"), Some(Yaml::Null));
    }

    #[test]
    fn no_frontmatter_is_all_body() {
        let doc = Document::parse("# Title\n---\n").unwrap();
        assert!(doc.frontmatter.is_none());
        assert_eq!(doc.render(), "# Title\n---\n");
    }

    #[test]
    fn unterminated_is_an_error() {
        assert!(matches!(
            Document::parse("---\nid: x\n"),
            Err(FrontmatterError::Unterminated)
        ));
    }

    #[test]
    fn crlf_and_eof_fence_round_trip() {
        for text in ["---\r\nid: x\r\n---\r\nbody\r\n", "---\nid: x\n---"] {
            assert_eq!(Document::parse(text).unwrap().render(), text);
        }
    }

    #[test]
    fn set_rewrites_only_changed_entries() {
        let mut doc = Document::parse(CASE).unwrap();
        let fm = doc.frontmatter.as_mut().unwrap();
        let order = ["id", "title", "status", "started", "closed", "tags"];
        assert!(!fm.set("status", &FmValue::str("queued"), &order));
        assert!(!fm.set("tags", &FmValue::list(&["a", "b"]), &order));
        assert!(!fm.set("started", &FmValue::Null, &order));
        assert!(fm.set("status", &FmValue::str("active"), &order));
        assert!(fm.set("started", &FmValue::str("2026-10-01"), &order));
        assert_eq!(
            doc.render(),
            "---\nid: C-2026-001\ntitle: \"A\"\nstatus: active\nstarted: 2026-10-01\nclosed:\ntags:\n  - a\n  - b\n---\n# Body\n\ntext\n"
        );
    }

    #[test]
    fn a_bom_and_padded_fences_round_trip() {
        for text in [
            "\u{feff}---\nid: x\n---\nbody\n",
            "--- \nid: x\n---\t \nbody\n",
            "\u{feff}---  \r\nid: x\r\n--- \r\nbody\r\n",
            "---\nid: x\n--- ",
        ] {
            let doc = Document::parse(text).unwrap();
            let fm = doc.frontmatter.as_ref().expect(text);
            assert_eq!(fm.get("id"), Some(Yaml::from("x")), "{text:?}");
            assert_eq!(doc.render(), text);
        }
        for text in ["\u{feff}# Title\n", "---x\nid: x\n---\n", "--- x\n"] {
            let doc = Document::parse(text).unwrap();
            assert!(doc.frontmatter.is_none(), "{text:?}");
            assert_eq!(doc.render(), text);
        }
        assert!(matches!(
            Document::parse("---\nid: x\n---x\n"),
            Err(FrontmatterError::Unterminated)
        ));
    }

    #[test]
    fn set_frontmatter_moves_a_bom_to_the_front() {
        let mut doc = Document::parse("\u{feff}# Day\n").unwrap();
        doc.set_frontmatter(Frontmatter::canonical(&[("id", FmValue::str("x"))]));
        assert_eq!(doc.render(), "\u{feff}---\nid: x\n---\n# Day\n");
        assert!(
            Document::parse(&doc.render())
                .unwrap()
                .frontmatter
                .is_some()
        );
    }

    const HAND: &str = "---\ntags:\n# keep sorted\n\n  - a\n# more\n  - b\n\n# about the id\nid: x\n\"title\": \"Old\"\n'it''s': y\n---\n";

    #[test]
    fn blank_and_comment_lines_inside_a_list_stay_with_it() {
        let mut doc = Document::parse(HAND).unwrap();
        assert_eq!(doc.render(), HAND);
        let fm = doc.frontmatter.as_mut().unwrap();
        assert_eq!(
            fm.entries[0].raw,
            "tags:\n# keep sorted\n\n  - a\n# more\n  - b\n"
        );
        assert_eq!(fm.entries[1].key, None);
        let order = ["tags", "id", "title"];
        assert!(!fm.set("tags", &FmValue::list(&["a", "b"]), &order));
        assert!(fm.set("id", &FmValue::str("z"), &order));
        assert_eq!(doc.render(), HAND.replace("id: x", "id: z"));
        // a rewritten list keeps its comments, the list's lines go
        let fm = doc.frontmatter.as_mut().unwrap();
        assert!(fm.set("tags", &FmValue::list(&["c"]), &order));
        let tail = "\n# about the id\nid: z\n\"title\": \"Old\"\n'it''s': y\n---\n";
        assert_eq!(
            doc.render(),
            format!("---\ntags: [c]\n# keep sorted\n# more\n{tail}")
        );
    }

    #[test]
    fn quoted_keys_are_keys() {
        let mut doc = Document::parse(HAND).unwrap();
        let fm = doc.frontmatter.as_mut().unwrap();
        let keys: Vec<_> = fm.keys().collect();
        assert_eq!(keys, ["tags", "id", "title", "it's"]);
        assert!(!fm.set("title", &FmValue::text("Old"), &["title"]));
        assert!(fm.set("title", &FmValue::text("New"), &["title"]));
        assert_eq!(
            doc.render(),
            HAND.replace("\"title\": \"Old\"", "title: \"New\"")
        );
        for (line, key) in [
            ("\"a\\\"b\": 1\n", Some("a\"b")),
            ("'a:b' : 1\n", Some("a:b")),
            ("\"a\":1\n", None),
            ("\"a\n", None),
            ("\"\": 1\n", None),
        ] {
            assert_eq!(key_of(line).as_deref(), key, "{line:?}");
        }
    }

    /// Valid YAML the line model cannot place: a flow list continued at
    /// column 0. Unchanged, it is left alone; changed, the old tail would
    /// stay behind, and the read-back check refuses the block.
    #[test]
    fn check_refuses_a_block_that_does_not_read_back() {
        let text = "---\nagents: [a,\nb]\nid: x\n---\n";
        let mut doc = Document::parse(text).unwrap();
        let fm = doc.frontmatter.as_mut().unwrap();
        let unchanged = [
            ("agents", FmValue::list(&["a", "b"])),
            ("id", FmValue::str("x")),
        ];
        assert!(!fm.set("agents", &unchanged[0].1, &[]));
        fm.check(&unchanged).unwrap();
        assert!(fm.set("agents", &FmValue::list(&["c"]), &[]));
        assert!(matches!(
            fm.check(&[("agents", FmValue::list(&["c"]))]),
            Err(FrontmatterError::Refused(_))
        ));
        // a block that parses but holds another value is refused too
        let fm = Frontmatter::canonical(&[("id", FmValue::str("x"))]);
        assert!(fm.check(&[("id", FmValue::str("y"))]).is_err());
        assert!(fm.check(&[("tags", FmValue::List(vec![]))]).is_ok());
    }

    #[test]
    fn canonical_style() {
        let fm = Frontmatter::canonical(&[
            ("id", FmValue::str("C-2026-001")),
            ("title", FmValue::text("Say \"hi\" · ü")),
            ("closed", FmValue::Null),
            ("n", FmValue::Int(112)),
            ("agents", FmValue::list(&["agent:claude-code", "two words"])),
            ("events", FmValue::List(vec![])),
            ("weird", FmValue::str("true")),
            ("date", FmValue::str("2026-10-01")),
        ]);
        assert_eq!(
            fm.render(),
            "---\nid: C-2026-001\ntitle: \"Say \\\"hi\\\" · ü\"\nclosed:\nn: 112\nagents: [agent:claude-code, \"two words\"]\nevents: []\nweird: \"true\"\ndate: 2026-10-01\n---\n"
        );
        // and it reads back to the same values
        let doc = Document::parse(&fm.render()).unwrap();
        let back = doc.frontmatter.unwrap();
        assert_eq!(back.get("title"), Some(Yaml::from("Say \"hi\" · ü")));
        assert_eq!(back.get("weird"), Some(Yaml::from("true")));
        assert_eq!(back.get("n"), Some(Yaml::Number(112.into())));
    }

    #[test]
    fn quoting_rules() {
        for (s, plain) in [
            ("C-2026-004", true),
            ("agent:claude-code", true),
            ("2026-10-01", true),
            ("null", false),
            ("~", false),
            ("123", false),
            ("1.5", false),
            ("yes", false),
            ("Off", false),
            ("", false),
            ("a b", false),
            ("-x", false),
            ("x:", false),
            ("ü", false),
        ] {
            assert_eq!(is_plain(s, false), plain, "{s:?}");
        }
    }
}
