//! Case frontmatter: `work/<folder>/C-YYYY-NNN-slug.md` (SPEC-LOGBOOK §3).

use chrono::NaiveDate;
use serde::Deserialize;

use super::{Record, field, is_agent, is_case_id, is_slug, is_ulid, null_as_empty};
use crate::frontmatter::{FmValue, FrontmatterError};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Case {
    pub id: String,
    pub title: String,
    pub status: CaseStatus,
    pub zone: Zone,
    pub risk: Risk,
    #[serde(default)]
    pub priority: Option<Priority>,
    #[serde(default)]
    pub area: Option<String>,
    pub created: NaiveDate,
    #[serde(default)]
    pub started: Option<NaiveDate>,
    #[serde(default)]
    pub closed: Option<NaiveDate>,
    #[serde(default)]
    pub snapshot_before: Option<u64>,
    #[serde(default, deserialize_with = "null_as_empty")]
    pub agents: Vec<String>,
    #[serde(default, deserialize_with = "null_as_empty")]
    pub events: Vec<String>,
    #[serde(default, deserialize_with = "null_as_empty")]
    pub tags: Vec<String>,
}

impl Record for Case {
    const TYPE: Option<&'static str> = Some("case");
    const KEYS: &'static [&'static str] = &[
        "id",
        "type",
        "title",
        "status",
        "zone",
        "risk",
        "priority",
        "area",
        "created",
        "started",
        "closed",
        "snapshotBefore",
        "agents",
        "events",
        "tags",
    ];

    fn to_values(&self) -> Vec<(&'static str, FmValue)> {
        vec![
            ("id", FmValue::str(&self.id)),
            ("type", FmValue::str("case")),
            ("title", FmValue::text(&self.title)),
            ("status", FmValue::str(self.status.as_str())),
            ("zone", FmValue::str(self.zone.as_str())),
            ("risk", FmValue::str(self.risk.as_str())),
            (
                "priority",
                FmValue::opt_str(self.priority.map(Priority::as_str)),
            ),
            ("area", FmValue::opt_str(self.area.as_ref())),
            ("created", FmValue::str(self.created.to_string())),
            ("started", FmValue::opt_str(self.started)),
            ("closed", FmValue::opt_str(self.closed)),
            (
                "snapshotBefore",
                self.snapshot_before
                    .map_or(FmValue::Null, |n| FmValue::Int(n as i64)),
            ),
            ("agents", FmValue::list(&self.agents)),
            ("events", FmValue::list(&self.events)),
            ("tags", FmValue::list(&self.tags)),
        ]
    }

    fn validate(&self) -> Result<(), FrontmatterError> {
        if !is_case_id(&self.id) {
            return Err(field("id", format!("`{}` is not C-YYYY-NNN", self.id)));
        }
        if self.title.is_empty() {
            return Err(field("title", "must not be empty"));
        }
        if let Some(area) = self.area.as_deref().filter(|a| !is_slug(a)) {
            return Err(field("area", format!("`{area}` is not a lowercase slug")));
        }
        if let Some(bad) = self.agents.iter().find(|a| !is_agent(a)) {
            return Err(field("agents", format!("`{bad}` is not agent:<name>")));
        }
        if let Some(bad) = self.events.iter().find(|e| !is_ulid(e)) {
            return Err(field("events", format!("`{bad}` is not a ULID")));
        }
        Ok(())
    }
}

impl Case {
    /// `C-YYYY-NNN-slug.md`.
    pub fn file_name(&self, slug: &str) -> String {
        format!("{}-{slug}.md", self.id)
    }
}

macro_rules! str_enum {
    ($(#[$meta:meta])* $name:ident { $($variant:ident = $text:literal),+ $(,)? }) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, ::serde::Serialize, ::serde::Deserialize)]
        pub enum $name {
            $(#[serde(rename = $text)] $variant),+
        }

        impl $name {
            pub const ALL: &'static [$name] = &[$($name::$variant),+];

            pub fn as_str(self) -> &'static str {
                match self {
                    $($name::$variant => $text),+
                }
            }
        }

        impl ::std::fmt::Display for $name {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                f.write_str(self.as_str())
            }
        }

        impl std::str::FromStr for $name {
            type Err = String;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                match s {
                    $($text => Ok($name::$variant),)+
                    _ => Err(format!(
                        concat!("`{}` is not a valid ", stringify!($name), " ({})"),
                        s,
                        [$($text),+].join(", ")
                    )),
                }
            }
        }
    };
}
pub(crate) use str_enum;

str_enum!(
    /// Case status; the folder follows from it (ADR-0012 §9).
    CaseStatus {
        Queued = "queued",
        Active = "active",
        Verification = "verification",
        Completed = "completed",
        Dropped = "dropped",
    }
);

impl CaseStatus {
    /// The `work/` sub-folder a case with this status lives in.
    pub fn folder(self) -> &'static str {
        match self {
            CaseStatus::Queued => "queued",
            CaseStatus::Active | CaseStatus::Verification => "active",
            CaseStatus::Completed | CaseStatus::Dropped => "completed",
        }
    }

    /// queued, active and verification are open (ADR-0012 §7).
    pub fn is_open(self) -> bool {
        matches!(
            self,
            CaseStatus::Queued | CaseStatus::Active | CaseStatus::Verification
        )
    }
}

str_enum!(
    Zone {
        Green = "green",
        Yellow = "yellow",
        Red = "red",
    }
);

str_enum!(
    Risk {
        R0 = "R0",
        R1 = "R1",
        R2 = "R2",
        R3 = "R3",
    }
);

str_enum!(
    Priority {
        High = "high",
        Normal = "normal",
        Low = "low",
    }
);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{parse, render_new, update};

    const TEXT: &str = "---\nid: C-2026-004\ntype: case\ntitle: \"Zed installieren\"\nstatus: active\nzone: red\nrisk: R2\npriority: normal\narea: dev-env\ncreated: 2026-10-01\nstarted: 2026-10-01\nclosed:\nsnapshotBefore: 112\nagents: [agent:claude-code]\nevents: [01M3V4RY8GW92AWEZ8KFTHZRAW]\ntags: []\n---\n# C-2026-004\n";

    #[test]
    fn parses_and_renders_canonically() {
        let (case, doc) = parse::<Case>(TEXT).unwrap();
        assert_eq!(case.status, CaseStatus::Active);
        assert_eq!(case.status.folder(), "active");
        assert_eq!(case.snapshot_before, Some(112));
        assert_eq!(case.closed, None);
        assert_eq!(render_new(&case, &doc.body), TEXT);
    }

    #[test]
    fn update_touches_only_changed_keys() {
        let (mut case, mut doc) = parse::<Case>(TEXT).unwrap();
        case.status = CaseStatus::Completed;
        case.closed = NaiveDate::from_ymd_opt(2026, 10, 2);
        update(&mut doc, &case);
        let expected = TEXT
            .replace("status: active", "status: completed")
            .replace("closed:\n", "closed: 2026-10-02\n");
        assert_eq!(doc.render(), expected);
    }

    #[test]
    fn rejects_bad_values() {
        for (from, to) in [
            ("id: C-2026-004", "id: C-26-4"),
            ("status: active", "status: running"),
            ("type: case", "type: journal"),
            ("agents: [agent:claude-code]", "agents: [claude]"),
            ("created: 2026-10-01", "created: yesterday"),
            ("title: \"Zed installieren\"\n", ""),
        ] {
            let text = TEXT.replace(from, to);
            assert!(parse::<Case>(&text).is_err(), "{to:?} must be rejected");
        }
    }

    #[test]
    fn missing_optional_keys_are_tolerated() {
        let text = "---\nid: C-2026-009\ntitle: \"x\"\nstatus: queued\nzone: green\nrisk: R0\ncreated: 2026-10-01\ntags:\n---\n";
        let (case, _) = parse::<Case>(text).unwrap();
        assert_eq!(case.priority, None);
        assert!(case.tags.is_empty());
    }
}
