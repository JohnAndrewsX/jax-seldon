//! Memory: `memory/<topic>.md` (SPEC-LOGBOOK §3).

use chrono::NaiveDate;
use serde::Deserialize;

use super::Record;
use crate::frontmatter::FmValue;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Memory {
    pub topic: String,
    #[serde(default)]
    pub updated: Option<NaiveDate>,
}

impl Record for Memory {
    const TYPE: Option<&'static str> = Some("memory");
    const KEYS: &'static [&'static str] = &["type", "topic", "updated"];

    fn to_values(&self) -> Vec<(&'static str, FmValue)> {
        vec![
            ("type", FmValue::str("memory")),
            ("topic", FmValue::str(&self.topic)),
            ("updated", FmValue::opt_str(self.updated)),
        ]
    }
}
