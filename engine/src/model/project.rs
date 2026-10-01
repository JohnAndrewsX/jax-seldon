//! `PROJECT.md` frontmatter (SPEC-LOGBOOK §2): language and machine id.

use serde::Deserialize;

use super::{Language, Record};
use crate::frontmatter::FmValue;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub language: Language,
    pub machine_id: String,
}

impl Record for Project {
    const TYPE: Option<&'static str> = None;
    const KEYS: &'static [&'static str] = &["language", "machineId"];

    fn to_values(&self) -> Vec<(&'static str, FmValue)> {
        vec![
            ("language", FmValue::str(self.language.as_str())),
            ("machineId", FmValue::str(&self.machine_id)),
        ]
    }
}
