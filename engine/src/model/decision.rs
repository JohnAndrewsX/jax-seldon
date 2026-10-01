//! Decision: `decisions/ADR-NNNN-slug.md` (SPEC-LOGBOOK §3).

use chrono::NaiveDate;
use serde::Deserialize;

use super::case::str_enum;
use super::{Record, field, is_case_id, is_decision_id, null_as_empty};
use crate::frontmatter::{FmValue, FrontmatterError};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Decision {
    pub id: String,
    pub title: String,
    pub status: DecisionStatus,
    pub date: NaiveDate,
    #[serde(default)]
    pub supersedes: Option<String>,
    #[serde(default, deserialize_with = "null_as_empty")]
    pub cases: Vec<String>,
}

str_enum!(
    DecisionStatus {
        Proposed = "proposed",
        Accepted = "accepted",
        Superseded = "superseded",
    }
);

impl Record for Decision {
    const TYPE: Option<&'static str> = Some("decision");
    const KEYS: &'static [&'static str] = &[
        "id",
        "type",
        "title",
        "status",
        "date",
        "supersedes",
        "cases",
    ];

    fn to_values(&self) -> Vec<(&'static str, FmValue)> {
        vec![
            ("id", FmValue::str(&self.id)),
            ("type", FmValue::str("decision")),
            ("title", FmValue::text(&self.title)),
            ("status", FmValue::str(self.status.as_str())),
            ("date", FmValue::str(self.date.to_string())),
            ("supersedes", FmValue::opt_str(self.supersedes.as_ref())),
            ("cases", FmValue::list(&self.cases)),
        ]
    }

    fn validate(&self) -> Result<(), FrontmatterError> {
        if !is_decision_id(&self.id) {
            return Err(field("id", format!("`{}` is not ADR-NNNN", self.id)));
        }
        if self.title.is_empty() {
            return Err(field("title", "must not be empty"));
        }
        if let Some(s) = self.supersedes.as_deref().filter(|s| !is_decision_id(s)) {
            return Err(field("supersedes", format!("`{s}` is not ADR-NNNN")));
        }
        match self.cases.iter().find(|c| !is_case_id(c)) {
            Some(bad) => Err(field("cases", format!("`{bad}` is not C-YYYY-NNN"))),
            None => Ok(()),
        }
    }
}
