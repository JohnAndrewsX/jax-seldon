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

use std::fmt::Write as _;

use serde::de::DeserializeOwned;
use serde_yaml::Value as Yaml;

const FENCE: &str = "---";

/// Errors while reading frontmatter.
#[derive(Debug, thiserror::Error)]
pub enum FrontmatterError {
    #[error("no frontmatter (the file must start with a `---` line)")]
    Missing,
    #[error("frontmatter is not closed by a `---` line")]
    Unterminated,
    #[error("frontmatter is not valid YAML: {0}")]
    Yaml(#[from] serde_yaml::Error),
    #[error("`{key}`: {message}")]
    Field { key: String, message: String },
}

/// A Markdown file: optional frontmatter plus the body after it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Document {
    pub frontmatter: Option<Frontmatter>,
    pub body: String,
}

impl Document {
    /// Splits `text` into frontmatter and body. A file that does not start
    /// with a `---` line has no frontmatter; the whole text is the body.
    pub fn parse(text: &str) -> Result<Self, FrontmatterError> {
        let Some(open_len) = fence_len(text) else {
            return Ok(Document {
                frontmatter: None,
                body: text.to_string(),
            });
        };
        let mut fm = Frontmatter {
            open: text[..open_len].to_string(),
            entries: Vec::new(),
            close: String::new(),
        };
        let mut pos = open_len;
        loop {
            if pos >= text.len() {
                return Err(FrontmatterError::Unterminated);
            }
            let end = text[pos..].find('\n').map_or(text.len(), |i| pos + i + 1);
            let line = &text[pos..end];
            if fence_len(line).is_some() {
                fm.close = line.to_string();
                return Ok(Document {
                    frontmatter: Some(fm),
                    body: text[end..].to_string(),
                });
            }
            fm.push_line(line);
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
}

/// Length of a `---` fence line at the start of `s` (with its line ending),
/// or `None` if `s` does not start with one.
fn fence_len(s: &str) -> Option<usize> {
    let rest = s.strip_prefix(FENCE)?;
    if rest.is_empty() {
        Some(FENCE.len())
    } else if rest.starts_with('\n') {
        Some(FENCE.len() + 1)
    } else if rest.starts_with("\r\n") {
        Some(FENCE.len() + 2)
    } else {
        None
    }
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
    /// value differs; everything else stays byte-identical. A missing key is
    /// inserted before the first key that follows it in `order` (or at the
    /// end), unless `value` is empty (null or `[]`), in which case nothing is
    /// added. Returns whether the text changed.
    pub fn set(&mut self, key: &str, value: &FmValue, order: &[&str]) -> bool {
        if let Some(i) = self.position(key) {
            let entry = &mut self.entries[i];
            if entry_value(entry).is_ok_and(|current| current == value.to_yaml()) {
                return false;
            }
            entry.raw = render_entry(key, value);
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

    fn push_line(&mut self, line: &str) {
        if let Some(key) = key_of(line) {
            self.entries.push(Entry {
                key: Some(key.to_string()),
                raw: line.to_string(),
            });
            return;
        }
        let continues = line.starts_with([' ', '\t']) && !line.trim().is_empty()
            || line.starts_with("- ")
            || line.trim_end() == "-";
        match self.entries.last_mut() {
            Some(last) if continues && last.key.is_some() => last.raw.push_str(line),
            _ => self.entries.push(Entry {
                key: None,
                raw: line.to_string(),
            }),
        }
    }
}

/// The key of a `key: value` line at column 0, if it is one.
fn key_of(line: &str) -> Option<&str> {
    let first = line.chars().next()?;
    if first.is_whitespace() || matches!(first, '#' | '-' | '"' | '\'' | '[' | '{') {
        return None;
    }
    let colon = line.find(':')?;
    let after = &line[colon + 1..];
    if !(after.is_empty() || after.starts_with([' ', '\t', '\n', '\r'])) {
        return None;
    }
    let key = line[..colon].trim_end();
    (!key.is_empty()).then_some(key)
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
