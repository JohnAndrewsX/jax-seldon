//! One ledger line (`ledger/YYYY-MM.jsonl`, SPEC-LOGBOOK §4), typed after
//! `schema/event.schema.json`.
//!
//! Producers build an [`Event`] with [`Event::new`] and the builder methods,
//! then hand it to [`crate::ledger::Ledger::append`], which assigns the ULID,
//! redacts `detail` and `meta.command`, validates and writes it. The `id`
//! of an event that has not been appended yet is [`Ulid::nil`].
//!
//! Field order of the struct is the key order of a ledger line, and the
//! order of [`Meta`]'s fields is the order of the conventional `meta` keys,
//! so a new line looks like the lines in `fixtures/logbook/ledger/`.

use std::collections::BTreeMap;
use std::fmt;

use chrono::{DateTime, FixedOffset, SecondsFormat};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use ulid::Ulid;

pub use super::case::Zone;
use super::case::str_enum;
use super::{is_agent, is_case_id};

/// Longest `subject` the schema accepts (characters).
pub const SUBJECT_MAX: usize = 512;
/// Longest `detail` the schema accepts (characters).
pub const DETAIL_MAX: usize = 4096;

str_enum!(
    /// Who produced the event (`event.schema.json#/properties/source`).
    Source {
        Pacman = "pacman",
        Snapper = "snapper",
        Omarchy = "omarchy",
        Plugins = "plugins",
        Theme = "theme",
        Config = "config",
        Agent = "agent",
        Manual = "manual",
        Seldon = "seldon",
    }
);

str_enum!(
    /// What happened (`event.schema.json#/properties/kind`).
    Kind {
        Install = "install",
        Remove = "remove",
        Upgrade = "upgrade",
        Downgrade = "downgrade",
        Reinstall = "reinstall",
        Snapshot = "snapshot",
        SnapshotDelete = "snapshot-delete",
        Update = "update",
        PluginAdd = "plugin-add",
        PluginRemove = "plugin-remove",
        PluginEnable = "plugin-enable",
        PluginDisable = "plugin-disable",
        PluginUpdate = "plugin-update",
        ThemeSet = "theme-set",
        ConfigChange = "config-change",
        ConfigAdd = "config-add",
        ConfigRemove = "config-remove",
        Command = "command",
        Note = "note",
        Resolution = "resolution",
        Correction = "correction",
        CaseCreated = "case-created",
        CaseStarted = "case-started",
        CaseVerified = "case-verified",
        CaseCompleted = "case-completed",
        CaseDropped = "case-dropped",
    }
);

str_enum!(
    /// How a drift event was resolved (ADR-0008).
    Resolution {
        Linked = "linked",
        Explained = "explained",
        Dismissed = "dismissed",
    }
);

impl Source {
    /// Sources whose events can be drift (ADR-0012 §6).
    pub fn is_drift_eligible(self) -> bool {
        matches!(
            self,
            Source::Pacman | Source::Omarchy | Source::Plugins | Source::Theme | Source::Config
        )
    }
}

/// Actor values (`event.schema.json#/$defs/actor`).
pub const ACTOR_SYSTEM: &str = "system";
pub const ACTOR_HUMAN: &str = "human";

/// Whether `s` is a valid actor: `human`, `system` or `agent:<slug>`.
pub fn is_actor(s: &str) -> bool {
    s == ACTOR_SYSTEM || s == ACTOR_HUMAN || is_agent(s)
}

/// The zone of an event (ADR-0014 §2): red for `pacman`, `omarchy` and
/// config files under `~/.config/systemd/`; yellow for other `config`,
/// `theme` and `plugins`; none for everything else (`snapper`, notes, case
/// events, resolutions). `subject` is the config path, `~`-relative.
///
/// Hook `command` events take the zone of what the command would produce;
/// the hook decides that and sets red or yellow itself. An agent command
/// no collector tracks is green (ADR-0019), so `source: agent` defaults to
/// green.
pub fn zone_for(source: Source, subject: &str) -> Option<Zone> {
    match source {
        Source::Pacman | Source::Omarchy => Some(Zone::Red),
        Source::Config if subject.starts_with("~/.config/systemd/") => Some(Zone::Red),
        Source::Config | Source::Theme | Source::Plugins => Some(Zone::Yellow),
        Source::Agent => Some(Zone::Green),
        _ => None,
    }
}

/// `meta`: the conventional keys typed, everything else in `extra`
/// (`event.schema.json#/properties/meta`). Absent keys are not written.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Meta {
    /// Redacted command line (pacman: the `[PACMAN] Running` line; hooks).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    /// install/remove/reinstall, plugin-add.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// upgrade, downgrade, update, plugin-update, theme-set.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to: Option<String>,
    /// config-*: sha256 before and after.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hash_from: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hash_to: Option<String>,
    /// snapper: `single|pre|post`.
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub snapshot_type: Option<String>,
    /// snapper cleanup algorithm (may be empty).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cleanup: Option<String>,
    /// snapper `post`: number of its `pre`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pair_of: Option<u64>,
    /// plugin-add/-enable/-disable: enabled after the change (ADR-0012 §15).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    /// resolution only: the drift group's txId (ADR-0013 §4).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tx_id: Option<String>,
    /// Any other scalar key.
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

impl Meta {
    pub fn is_empty(&self) -> bool {
        *self == Meta::default()
    }
}

/// One ledger event. See the module docs for how to write one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Event {
    #[serde(deserialize_with = "de_id")]
    pub id: Ulid,
    #[serde(serialize_with = "ser_ts", deserialize_with = "de_ts")]
    pub ts: DateTime<FixedOffset>,
    pub source: Source,
    pub kind: Kind,
    pub subject: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    pub actor: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub case: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub zone: Option<Zone>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub explicit: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tx_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refers_to: Option<Ulid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolution: Option<Resolution>,
    #[serde(default, skip_serializing_if = "Meta::is_empty")]
    pub meta: Meta,
}

impl Event {
    /// A new event with `actor: system`, the zone of `source` (ADR-0014 §2)
    /// and no id yet (the ledger assigns one).
    pub fn new(
        ts: DateTime<FixedOffset>,
        source: Source,
        kind: Kind,
        subject: impl Into<String>,
    ) -> Self {
        let subject = subject.into();
        Event {
            id: Ulid::nil(),
            ts,
            source,
            kind,
            zone: zone_for(source, &subject),
            subject,
            detail: None,
            actor: ACTOR_SYSTEM.to_string(),
            case: None,
            explicit: None,
            tx_id: None,
            refers_to: None,
            resolution: None,
            meta: Meta::default(),
        }
    }

    pub fn detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    pub fn actor(mut self, actor: impl Into<String>) -> Self {
        self.actor = actor.into();
        self
    }

    pub fn case(mut self, case: Option<String>) -> Self {
        self.case = case;
        self
    }

    pub fn meta(mut self, meta: Meta) -> Self {
        self.meta = meta;
        self
    }

    /// `ts` as written to the ledger: RFC 3339, offset kept, whole seconds
    /// unless the source had fractions.
    pub fn ts_string(&self) -> String {
        format_ts(&self.ts)
    }

    /// `YYYY-MM` of `ts` in its own offset: the ledger month file.
    pub fn month(&self) -> String {
        self.ts.format("%Y-%m").to_string()
    }

    /// The "version" part of the dedupe key `(ts, kind, subject, version)`
    /// (SPEC-ENGINE §4): `meta.to` for upgrades/downgrades, else
    /// `meta.version`.
    pub fn version_key(&self) -> Option<&str> {
        self.meta.to.as_deref().or(self.meta.version.as_deref())
    }

    /// Checks every rule of `event.schema.json` that serde does not
    /// already enforce. The ledger refuses an event that fails.
    pub fn validate(&self) -> Result<(), String> {
        let id = self.id.to_string();
        if self.id.is_nil() || !super::is_ulid(&id) {
            return Err(format!("id `{id}` is not an assigned ULID"));
        }
        if self.subject.is_empty() || self.subject.chars().count() > SUBJECT_MAX {
            return Err(format!("subject must be 1..={SUBJECT_MAX} characters"));
        }
        if let Some(d) = &self.detail
            && d.chars().count() > DETAIL_MAX
        {
            return Err(format!("detail is longer than {DETAIL_MAX} characters"));
        }
        if !is_actor(&self.actor) {
            return Err(format!(
                "actor `{}` is not human|system|agent:<name>",
                self.actor.escape_debug()
            ));
        }
        if let Some(c) = &self.case
            && !is_case_id(c)
        {
            return Err(format!("case `{}` is not a case id", c.escape_debug()));
        }
        if self.tx_id.as_deref() == Some("") || self.meta.tx_id.as_deref() == Some("") {
            return Err("txId must not be empty".into());
        }
        match self.kind {
            Kind::Resolution => {
                if self.refers_to.is_none() || self.resolution.is_none() {
                    return Err("a resolution needs refersTo and resolution".into());
                }
                if self.resolution == Some(Resolution::Linked) && self.case.is_none() {
                    return Err("a linked resolution needs a case".into());
                }
            }
            Kind::Correction if self.refers_to.is_none() => {
                return Err("a correction needs refersTo".into());
            }
            _ => {}
        }
        if self.meta.tx_id.is_some() && self.kind != Kind::Resolution {
            return Err("meta.txId is only allowed on a resolution".into());
        }
        if let Some((k, v)) = self
            .meta
            .extra
            .iter()
            .find(|(_, v)| v.is_array() || v.is_object())
        {
            return Err(format!("meta.{k} must be a scalar, not {v}"));
        }
        Ok(())
    }

    /// The ledger line (one JSON object, no newline).
    pub fn to_line(&self) -> String {
        serde_json::to_string(self).expect("an event always serialises")
    }
}

impl fmt::Display for Event {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} {} {} `{}`",
            self.ts_string(),
            self.source,
            self.kind,
            self.subject
        )?;
        if let Some(d) = &self.detail {
            write!(f, " {d}")?;
        }
        Ok(())
    }
}

/// RFC 3339 with the event's own offset (`+00:00`, never `Z`).
pub fn format_ts(ts: &DateTime<FixedOffset>) -> String {
    ts.to_rfc3339_opts(SecondsFormat::AutoSi, false)
}

fn ser_ts<S: Serializer>(ts: &DateTime<FixedOffset>, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(&format_ts(ts))
}

fn de_ts<'de, D: Deserializer<'de>>(d: D) -> Result<DateTime<FixedOffset>, D::Error> {
    d.deserialize_str(ParsedStr(DateTime::parse_from_rfc3339))
}

fn de_id<'de, D: Deserializer<'de>>(d: D) -> Result<Ulid, D::Error> {
    d.deserialize_str(ParsedStr(Ulid::from_string))
}

/// A string field parsed from the borrowed text: no `String` per field
/// and ledger line (WP-092). Errors read as those of `String` plus the
/// parser's.
struct ParsedStr<F>(F);

impl<T, E: fmt::Display, F: FnOnce(&str) -> Result<T, E>> serde::de::Visitor<'_> for ParsedStr<F> {
    type Value = T;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("a string")
    }

    fn visit_str<Er: serde::de::Error>(self, v: &str) -> Result<T, Er> {
        (self.0)(v).map_err(Er::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// WP-092: `id` and `ts` read from the borrowed text give the values
    /// and the errors that reading a `String` first gave, escapes included.
    /// Only the error's column may differ (raised before the closing quote
    /// is consumed); the one reader, `Ledger::read_month`, drops the error.
    #[test]
    fn id_and_ts_read_as_through_a_string() {
        #[derive(Debug, PartialEq, Deserialize)]
        struct Borrowed {
            #[serde(deserialize_with = "de_id")]
            id: Ulid,
            #[serde(deserialize_with = "de_ts")]
            ts: DateTime<FixedOffset>,
        }
        #[derive(Debug, PartialEq, Deserialize)]
        struct Owned {
            #[serde(deserialize_with = "owned_id")]
            id: Ulid,
            #[serde(deserialize_with = "owned_ts")]
            ts: DateTime<FixedOffset>,
        }
        fn owned_id<'de, D: Deserializer<'de>>(d: D) -> Result<Ulid, D::Error> {
            Ulid::from_string(&String::deserialize(d)?).map_err(serde::de::Error::custom)
        }
        fn owned_ts<'de, D: Deserializer<'de>>(d: D) -> Result<DateTime<FixedOffset>, D::Error> {
            DateTime::parse_from_rfc3339(&String::deserialize(d)?).map_err(serde::de::Error::custom)
        }
        let (id, ts) = (
            r#""01M1MB2M1GWZYF485HTGVZ1KS3""#,
            r#""2026-09-03T21:14:06+02:00""#,
        );
        for (id, ts) in [
            (id, ts),
            // JSON escapes for the first `0` and the `+`
            (
                r#""\u00301M1MB2M1GWZYF485HTGVZ1KS3""#,
                r#""2026-09-03T21:14:06\u002b02:00""#,
            ),
            ("5", ts),
            ("null", ts),
            (r#""01M1""#, ts),
            (id, "5"),
            (id, "[]"),
            (id, r#""2026-09-03""#),
            (id, r#""2026-09-03T21:14:06\u002b0200""#),
        ] {
            let line = format!(r#"{{"id":{id},"ts":{ts}}}"#);
            let message = |e: serde_json::Error| {
                let text = e.to_string();
                text.rsplit_once(" at line ")
                    .map_or(text.clone(), |(m, _)| m.to_string())
            };
            let borrowed = serde_json::from_str::<Borrowed>(&line).map_err(message);
            let owned = serde_json::from_str::<Owned>(&line).map_err(message);
            assert_eq!(
                format!("{borrowed:?}"),
                format!("{owned:?}").replace("Owned", "Borrowed"),
                "{line}"
            );
        }
        let ok: Borrowed = serde_json::from_str(&format!(r#"{{"id":{id},"ts":{ts}}}"#)).unwrap();
        assert_eq!(ok.id.to_string(), "01M1MB2M1GWZYF485HTGVZ1KS3");
    }

    /// An actor or case it refuses is named escaped (WP-077).
    #[test]
    fn a_refused_value_is_named_escaped() {
        let event: Event = serde_json::from_str(PACMAN_LINE).unwrap();
        let mut bad_actor = event.clone();
        bad_actor.actor = "agent:\u{1b}[31mX".into();
        let mut bad_case = event;
        bad_case.case = Some("C-2026-\u{1b}[31mX".into());
        for bad in [bad_actor, bad_case] {
            let err = bad.validate().unwrap_err();
            assert!(!err.chars().any(char::is_control), "{err:?}");
            assert!(err.contains("\\u{1b}[31mX"), "{err:?}");
        }
    }

    const PACMAN_LINE: &str = r#"{"id":"01M1MB2M1GWZYF485HTGVZ1KS3","ts":"2026-09-03T21:14:06+02:00","source":"pacman","kind":"install","subject":"btop","detail":"1.4.5-1","actor":"system","zone":"red","explicit":true,"txId":"tx-20260903T211406","meta":{"command":"pacman -S btop","version":"1.4.5-1"}}"#;

    #[test]
    fn fixture_ledger_lines_round_trip_byte_for_byte() {
        let dir =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../fixtures/logbook/ledger");
        let mut n = 0;
        for month in ["2026-09", "2026-10"] {
            let text = std::fs::read_to_string(dir.join(format!("{month}.jsonl"))).unwrap();
            for line in text.lines() {
                let event: Event = serde_json::from_str(line).unwrap();
                event.validate().unwrap();
                assert_eq!(event.month(), month);
                assert_eq!(event.to_line(), line);
                n += 1;
            }
        }
        assert_eq!(n, 85, "fixtures/README.md: 85 ledger lines");
    }

    #[test]
    fn builder_defaults() {
        let ts = DateTime::parse_from_rfc3339("2026-09-03T21:14:06+02:00").unwrap();
        let mut e = Event::new(ts, Source::Pacman, Kind::Install, "btop")
            .detail("1.4.5-1")
            .meta(Meta {
                command: Some("pacman -S btop".into()),
                version: Some("1.4.5-1".into()),
                ..Meta::default()
            });
        e.explicit = Some(true);
        e.tx_id = Some("tx-20260903T211406".into());
        e.id = "01M1MB2M1GWZYF485HTGVZ1KS3".parse().unwrap();
        assert_eq!(e.to_line(), PACMAN_LINE);
        assert_eq!(e.version_key(), Some("1.4.5-1"));
    }

    #[test]
    fn zones() {
        assert_eq!(zone_for(Source::Pacman, "x"), Some(Zone::Red));
        assert_eq!(zone_for(Source::Omarchy, "omarchy"), Some(Zone::Red));
        assert_eq!(
            zone_for(Source::Config, "~/.config/systemd/user/a.service"),
            Some(Zone::Red)
        );
        assert_eq!(
            zone_for(Source::Config, "~/.config/hypr/a.conf"),
            Some(Zone::Yellow)
        );
        assert_eq!(zone_for(Source::Theme, "kanagawa"), Some(Zone::Yellow));
        assert_eq!(zone_for(Source::Plugins, "x"), Some(Zone::Yellow));
        assert_eq!(
            zone_for(Source::Agent, "npm"),
            Some(Zone::Green),
            "ADR-0019"
        );
        assert_eq!(zone_for(Source::Snapper, "112"), None);
        assert_eq!(zone_for(Source::Seldon, "C-2026-001"), None);
    }

    #[test]
    fn validation_rules() {
        let ts = DateTime::parse_from_rfc3339("2026-09-03T21:14:06+00:00").unwrap();
        let ok = |mut e: Event| {
            e.id = Ulid::from_parts(1, 1);
            e
        };
        let e = ok(Event::new(ts, Source::Seldon, Kind::Resolution, "btop"));
        assert!(e.validate().unwrap_err().contains("refersTo"));
        let mut e = ok(Event::new(ts, Source::Seldon, Kind::Resolution, "btop"));
        e.refers_to = Some(Ulid::from_parts(1, 2));
        e.resolution = Some(Resolution::Linked);
        assert!(e.validate().unwrap_err().contains("case"));
        e.case = Some("C-2026-001".into());
        e.validate().unwrap();
        let e = ok(Event::new(ts, Source::Pacman, Kind::Install, "x").actor("agent:Claude"));
        assert!(e.validate().is_err());
        let e = ok(Event::new(ts, Source::Pacman, Kind::Install, ""));
        assert!(e.validate().is_err());
        let e = Event::new(ts, Source::Pacman, Kind::Install, "x");
        assert!(
            e.validate().unwrap_err().contains("ULID"),
            "nil id is refused"
        );
        let mut e = ok(Event::new(ts, Source::Pacman, Kind::Install, "x"));
        e.meta.tx_id = Some("tx-1".into());
        assert!(e.validate().is_err());
        assert_eq!(e.ts_string(), "2026-09-03T21:14:06+00:00");
    }
}
