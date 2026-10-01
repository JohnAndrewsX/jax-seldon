//! Logbook templates from `engine/templates/{en,de}/`, compiled into the
//! binary (single static binary, SPEC-ENGINE §1).
//!
//! Placeholders: `{{machineId}}`, `{{language}}`, `{{date}}` (YYYY-MM-DD).

use chrono::NaiveDate;

use crate::model::Language;

/// One template file, in every language.
#[derive(Debug, Clone, Copy)]
pub struct Template {
    /// Path relative to the logbook root.
    pub path: &'static str,
    en: &'static str,
    de: &'static str,
}

impl Template {
    pub fn text(&self, language: Language) -> &'static str {
        match language {
            Language::En => self.en,
            Language::De => self.de,
        }
    }
}

macro_rules! template {
    ($path:literal) => {
        Template {
            path: $path,
            en: include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/templates/en/", $path)),
            de: include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/templates/de/", $path)),
        }
    };
}

/// Everything `seldon init` writes from templates (SPEC-ENGINE §9).
pub const TEMPLATES: &[Template] = &[
    template!("AGENTS.md"),
    template!("PROJECT.md"),
    template!("STATUS.md"),
    template!("DECISIONS.md"),
    template!("areas/hyprland/README.md"),
    template!("areas/themes/README.md"),
    template!("areas/packages/README.md"),
    template!("areas/dev-env/README.md"),
    template!("areas/plugins/README.md"),
    template!("areas/shell/README.md"),
    template!("memory/lessons.md"),
    template!("system/hardware.md"),
    template!("system/packages.md"),
    template!("system/deviations.md"),
    template!("system/services.md"),
    template!("system/omarchy.md"),
    template!("system/plugins.md"),
];

/// Values for the placeholders.
#[derive(Debug, Clone)]
pub struct Vars<'a> {
    pub machine_id: &'a str,
    pub language: Language,
    pub date: NaiveDate,
}

/// Fills the placeholders of `text`.
pub fn render(text: &str, vars: &Vars<'_>) -> String {
    text.replace("{{machineId}}", vars.machine_id)
        .replace("{{language}}", vars.language.as_str())
        .replace("{{date}}", &vars.date.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::area::DEFAULT_AREAS;
    use crate::model::{self, Area, Memory, Project};

    fn vars(language: Language) -> Vars<'static> {
        Vars {
            machine_id: "box-1a2b",
            language,
            date: NaiveDate::from_ymd_opt(2026, 10, 1).unwrap(),
        }
    }

    #[test]
    fn every_template_renders_and_parses() {
        for language in Language::ALL {
            for t in TEMPLATES {
                let text = render(t.text(language), &vars(language));
                assert!(
                    !text.contains("{{"),
                    "{} ({language}) has a placeholder left",
                    t.path
                );
                assert!(text.ends_with('\n'), "{} ({language})", t.path);
                if t.path == "PROJECT.md" {
                    let (p, _) = model::parse::<Project>(&text).unwrap();
                    assert_eq!(p.language, language);
                    assert_eq!(p.machine_id, "box-1a2b");
                } else if t.path.starts_with("areas/") {
                    let (a, _) = model::parse::<Area>(&text).unwrap();
                    assert!(t.path.contains(&a.name));
                } else if t.path.starts_with("memory/") {
                    let (m, _) = model::parse::<Memory>(&text).unwrap();
                    assert_eq!(m.topic, "lessons");
                }
            }
        }
    }

    #[test]
    fn default_areas_have_templates() {
        for area in DEFAULT_AREAS {
            let path = format!("areas/{area}/README.md");
            assert!(TEMPLATES.iter().any(|t| t.path == path), "{path}");
        }
    }

    #[test]
    fn status_template_is_marked_generated() {
        let t = TEMPLATES.iter().find(|t| t.path == "STATUS.md").unwrap();
        for language in Language::ALL {
            assert!(t.text(language).starts_with(crate::GENERATED_HEADER));
        }
    }
}
