//! Typed frontmatter of logbook files (SPEC-LOGBOOK §3, `schema/case.schema.json`).
//!
//! Each record type knows its canonical key order and how to turn itself
//! into [`FmValue`]s. Reading goes through `serde_yaml`; writing goes through
//! [`crate::frontmatter`], either as a fresh canonical block
//! ([`render_new`]) or as a lossless update of an existing file ([`update`]).

pub mod area;
pub mod case;
pub mod decision;
pub mod event;
pub mod journal;
pub mod memory;
pub mod project;

use std::fmt;
use std::path::Path;
use std::str::FromStr;

use anyhow::Context as _;
use serde::de::{DeserializeOwned, Deserializer};
use serde::{Deserialize, Serialize};

use crate::frontmatter::{Document, FmValue, Frontmatter, FrontmatterError};

pub use area::Area;
pub use case::{Case, CaseStatus, Priority, Risk, Zone};
pub use decision::{Decision, DecisionStatus};
pub use event::{Event, Kind, Meta, Source};
pub use journal::{Journal, JournalEntry};
pub use memory::Memory;
pub use project::Project;

/// A logbook file type with YAML frontmatter.
pub trait Record: DeserializeOwned {
    /// Value of the `type` key, or `None` for files without one (PROJECT.md).
    const TYPE: Option<&'static str>;
    /// Canonical key order.
    const KEYS: &'static [&'static str];

    /// All keys, canonical order, canonical values.
    fn to_values(&self) -> Vec<(&'static str, FmValue)>;

    /// Checks beyond what serde enforces (id patterns and the like).
    fn validate(&self) -> Result<(), FrontmatterError> {
        Ok(())
    }

    /// Reads and validates a record from a frontmatter block.
    fn from_frontmatter(fm: &Frontmatter) -> Result<Self, FrontmatterError> {
        if let (Some(expected), Some(found)) = (Self::TYPE, fm.get("type"))
            && found.as_str() != Some(expected)
        {
            return Err(field("type", format!("expected `{expected}`")));
        }
        let record: Self = fm.deserialize()?;
        record.validate()?;
        Ok(record)
    }
}

/// Parses a file's text into its record and the document (for the body).
pub fn parse<R: Record>(text: &str) -> Result<(R, Document), FrontmatterError> {
    let doc = Document::parse(text)?;
    let record = R::from_frontmatter(doc.frontmatter()?)?;
    Ok((record, doc))
}

/// Reads a record from a file; errors name the file.
pub fn load<R: Record>(path: &Path) -> anyhow::Result<(R, Document)> {
    let text =
        std::fs::read_to_string(path).with_context(|| format!("cannot read {}", path.display()))?;
    parse(&text).with_context(|| format!("{}: invalid frontmatter", path.display()))
}

/// A new file: canonical frontmatter followed by `body`.
pub fn render_new<R: Record>(record: &R, body: &str) -> String {
    Document {
        frontmatter: Some(Frontmatter::canonical(&record.to_values())),
        body: body.to_string(),
    }
    .render()
}

/// Writes `record` into an existing document, touching only changed keys.
/// A document without frontmatter gets a canonical block.
pub fn update<R: Record>(doc: &mut Document, record: &R) {
    let values = record.to_values();
    match doc.frontmatter.as_mut() {
        Some(fm) => {
            for (key, value) in &values {
                fm.set(key, value, R::KEYS);
            }
        }
        None => doc.frontmatter = Some(Frontmatter::canonical(&values)),
    }
}

pub(crate) fn field(key: &str, message: impl Into<String>) -> FrontmatterError {
    FrontmatterError::Field {
        key: key.to_string(),
        message: message.into(),
    }
}

/// Deserialises a list that may be written as `key:` (null) or omitted.
pub(crate) fn null_as_empty<'de, D, T>(de: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Ok(Option::<Vec<T>>::deserialize(de)?.unwrap_or_default())
}

/// Logbook language (ADR-0007). Templates exist for each variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    #[default]
    En,
    De,
}

impl Language {
    pub const ALL: [Language; 2] = [Language::En, Language::De];

    pub fn as_str(self) -> &'static str {
        match self {
            Language::En => "en",
            Language::De => "de",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Language::En => "English",
            Language::De => "Deutsch",
        }
    }

    /// `de` for a `LANG`/`LC_ALL` like `de_DE.UTF-8`, else `en`.
    pub fn from_locale(locale: Option<&str>) -> Language {
        match locale {
            Some(l) if l.starts_with("de") => Language::De,
            _ => Language::En,
        }
    }
}

impl fmt::Display for Language {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Language {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "en" => Ok(Language::En),
            "de" => Ok(Language::De),
            _ => Err(format!("unsupported language `{s}` (en, de)")),
        }
    }
}

/// `C-YYYY-NNN` (`event.schema.json#/$defs/caseId`).
pub fn is_case_id(s: &str) -> bool {
    let Some(rest) = s.strip_prefix("C-") else {
        return false;
    };
    let Some((year, num)) = rest.split_once('-') else {
        return false;
    };
    year.len() == 4 && all_digits(year) && num.len() >= 3 && all_digits(num)
}

/// `ADR-NNNN`.
pub fn is_decision_id(s: &str) -> bool {
    s.strip_prefix("ADR-")
        .is_some_and(|n| n.len() >= 4 && all_digits(n))
}

/// ULID as the schema allows it: 26 Crockford base32 chars, first `0`–`7`.
pub fn is_ulid(s: &str) -> bool {
    s.len() == 26
        && s.starts_with(['0', '1', '2', '3', '4', '5', '6', '7'])
        && s.chars()
            .all(|c| c.is_ascii_digit() || (c.is_ascii_uppercase() && !"ILOU".contains(c)))
}

/// `agent:<name>` with a lowercase slug name.
pub fn is_agent(s: &str) -> bool {
    s.strip_prefix("agent:").is_some_and(is_slug)
}

/// Lowercase slug `[a-z0-9][a-z0-9-]*`, used for areas and agent names.
pub fn is_slug(s: &str) -> bool {
    s.starts_with(|c: char| c.is_ascii_lowercase() || c.is_ascii_digit())
        && s.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

fn all_digits(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn id_patterns() {
        assert!(is_case_id("C-2026-004"));
        assert!(is_case_id("C-2026-1004"));
        assert!(!is_case_id("C-26-004"));
        assert!(!is_case_id("C-2026-04"));
        assert!(is_decision_id("ADR-0004"));
        assert!(!is_decision_id("ADR-4"));
        assert!(is_ulid("01M3V4RY8GW92AWEZ8KFTHZRAW"));
        assert!(!is_ulid("81M3V4RY8GW92AWEZ8KFTHZRAW"));
        assert!(!is_ulid("01M3V4RY8GW92AWEZ8KFTHZRAI"));
        assert!(is_agent("agent:claude-code"));
        assert!(!is_agent("agent:Claude"));
        assert!(is_slug("dev-env"));
        assert!(!is_slug("-dev"));
    }

    #[test]
    fn language_from_locale() {
        assert_eq!(Language::from_locale(Some("de_DE.UTF-8")), Language::De);
        assert_eq!(Language::from_locale(Some("C")), Language::En);
        assert_eq!(Language::from_locale(None), Language::En);
    }
}
