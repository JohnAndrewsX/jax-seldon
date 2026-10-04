//! Area: `areas/<area>/README.md` (SPEC-LOGBOOK §3).

use serde::Deserialize;

use super::{Record, field, is_slug};
use crate::frontmatter::{FmValue, FrontmatterError};

/// The areas `seldon init` creates (SPEC-ENGINE §9).
pub const DEFAULT_AREAS: [&str; 6] = [
    "hyprland", "themes", "packages", "dev-env", "plugins", "shell",
];

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Area {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
}

impl Record for Area {
    const TYPE: Option<&'static str> = Some("area");
    const KEYS: &'static [&'static str] = &["type", "name", "description"];

    fn to_values(&self) -> Vec<(&'static str, FmValue)> {
        vec![
            ("type", FmValue::str("area")),
            ("name", FmValue::str(&self.name)),
            (
                "description",
                self.description
                    .as_ref()
                    .map_or(FmValue::Null, FmValue::text),
            ),
        ]
    }

    fn validate(&self) -> Result<(), FrontmatterError> {
        if is_slug(&self.name) {
            Ok(())
        } else {
            Err(field(
                "name",
                format!("`{}` is not a lowercase slug", self.name.escape_debug()),
            ))
        }
    }
}
