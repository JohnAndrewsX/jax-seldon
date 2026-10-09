//! `index.json` as typed structs (`schema/index.schema.json`). Field order
//! is the key order of `fixtures/index.sample.json`; every object of the
//! schema is closed, so nothing here may grow a field without an ADR and a
//! `contractVersion` bump (docs/CONTRACT.md), except an optional field an
//! accepted ADR adds within contract 2 before 0.2.0 is tagged (ADR-0035 §6,
//! CONTRACT.md rule 9).

use std::collections::BTreeMap;

use serde::ser::SerializeMap as _;
use serde::{Serialize, Serializer};

use crate::model::event::{Event, format_ts};

/// The whole index.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Index {
    pub contract_version: u32,
    /// ADR-0051: the oldest plugin contract that can read this index.
    pub contract_readable_from: u32,
    pub generated_at: String,
    pub engine_version: String,
    pub logbook: LogbookInfo,
    pub state: State,
    pub summary: Summary,
    pub today: Today,
    pub events: Vec<IndexEvent>,
    pub drift: Vec<DriftItem>,
    pub cases: Cases,
    pub decisions: Vec<DecisionRow>,
    pub system: System,
    pub memory: MemoryInfo,
    pub series: Series,
    /// The newest triage proposal of this logbook (ADR-0035 §6).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub triage: Option<Triage>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LogbookInfo {
    pub path: String,
    pub language: String,
    pub machine: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub git: Option<GitInfo>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GitInfo {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub head: Option<String>,
    /// `None` when unknown (the fast rebuild path spawns no git).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dirty: Option<bool>,
    /// The last autocommit attempted in this logbook (ADR-0035 §2).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub autocommit: Option<AutocommitInfo>,
}

/// `logbook.git.autocommit`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AutocommitInfo {
    pub ok: bool,
    pub at: String,
    pub message: String,
}

/// `state.status`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Status {
    Ok,
    NotInitialised,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct State {
    pub status: Status,
    pub last_capture: Option<String>,
    pub collectors: Vec<CollectorRow>,
}

/// One collector's last status, from `cursors.json` (SPEC-ENGINE §2).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectorRow {
    pub name: &'static str,
    pub enabled: bool,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    pub last_run: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub active_cases: usize,
    pub queued_cases: usize,
    pub open_drift: usize,
    pub crisis: usize,
    pub events_today: usize,
    #[serde(rename = "events7d")]
    pub events_7d: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Today {
    pub date: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    pub entries: Vec<JournalRow>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub yesterday: Option<Vec<JournalRow>>,
}

/// `$defs/journalEntry`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct JournalRow {
    pub time: String,
    pub actor: String,
    pub case: Option<String>,
    pub text: String,
}

/// A ledger event as the index lists it: resolutions folded onto their
/// target (ADR-0012 §8, §11).
#[derive(Debug, Clone, PartialEq)]
pub struct IndexEvent {
    pub event: Event,
    /// The `detail` of the winning resolution (index only).
    pub resolution_detail: Option<String>,
}

impl Serialize for IndexEvent {
    /// The ledger line's keys in their order, with `resolutionDetail` next
    /// to `resolution` (`EVENT_KEYS` of scripts/validate-fixtures.py).
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let e = &self.event;
        let mut m = s.serialize_map(None)?;
        m.serialize_entry("id", &e.id)?;
        m.serialize_entry("ts", &format_ts(&e.ts))?;
        m.serialize_entry("source", &e.source)?;
        m.serialize_entry("kind", &e.kind)?;
        m.serialize_entry("subject", &e.subject)?;
        if let Some(d) = &e.detail {
            m.serialize_entry("detail", d)?;
        }
        m.serialize_entry("actor", &e.actor)?;
        if let Some(c) = &e.case {
            m.serialize_entry("case", c)?;
        }
        if let Some(z) = &e.zone {
            m.serialize_entry("zone", z)?;
        }
        if let Some(x) = &e.explicit {
            m.serialize_entry("explicit", x)?;
        }
        if let Some(t) = &e.tx_id {
            m.serialize_entry("txId", t)?;
        }
        if let Some(r) = &e.refers_to {
            m.serialize_entry("refersTo", r)?;
        }
        if let Some(r) = &e.resolution {
            m.serialize_entry("resolution", r)?;
        }
        if let Some(d) = &self.resolution_detail {
            m.serialize_entry("resolutionDetail", d)?;
        }
        if !e.meta.is_empty() {
            m.serialize_entry("meta", &e.meta)?;
        }
        m.end()
    }
}

/// `$defs/drift`: one open drift item, a single event or a pacman group.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DriftItem {
    pub event_id: String,
    pub ts: String,
    pub source: String,
    pub kind: String,
    pub subject: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    pub actor: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub zone: Option<String>,
    pub crisis: bool,
    pub proposed_case: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tx_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub members: Option<usize>,
    /// `Some(true)` when `detail` was clipped (ADR-0035 §3).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub truncated: Option<bool>,
    /// The ADR-0028 §2 rule that classified the item, as `drift show`
    /// reports it (ADR-0038 §1).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rule: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Cases {
    pub queued: Vec<IndexCase>,
    pub active: Vec<IndexCase>,
    pub verification: Vec<IndexCase>,
    /// completed and dropped, newest `closed` first, at most 50.
    pub completed: Vec<IndexCase>,
}

impl Cases {
    pub fn all(&self) -> impl Iterator<Item = &IndexCase> {
        self.queued
            .iter()
            .chain(&self.active)
            .chain(&self.verification)
            .chain(&self.completed)
    }
}

/// `case.schema.json` as the index has it: no `type`, plus `path`,
/// `steps`, `proposedEvents`, `intent` and `result`.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexCase {
    pub id: String,
    pub title: String,
    pub status: String,
    pub zone: String,
    pub risk: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub area: Option<String>,
    pub created: String,
    pub started: Option<String>,
    pub closed: Option<String>,
    pub snapshot_before: Option<u64>,
    pub agents: Vec<String>,
    pub events: Vec<String>,
    pub tags: Vec<String>,
    pub path: String,
    pub steps: Steps,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub proposed_events: Vec<String>,
    /// The first paragraph of `## Intent` and of `## Result`, redacted
    /// and clipped (ADR-0038 §2).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub intent: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<String>,
    /// An imported case's task, `~/…/file.md#line` (ADR-0038 §3).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct Steps {
    pub total: usize,
    pub done: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DecisionRow {
    pub id: String,
    pub title: String,
    pub status: String,
    pub date: String,
    pub path: String,
    /// The frontmatter's `cases`, as written, without repeats (ADR-0035 §5).
    pub cases: Vec<String>,
    /// The first paragraph of `## Decision`, redacted and clipped
    /// (ADR-0038 §2).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lead: Option<String>,
}

/// `triage`: the newest proposal of this logbook (ADR-0035 §6).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Triage {
    pub id: String,
    pub at: String,
    pub actor: String,
    pub counts: TriageCounts,
    /// `proposals/<id>.json`, relative to the directory of `index.json`.
    pub path: String,
    pub applied: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct TriageCounts {
    pub items: usize,
    pub crises: usize,
}

/// `system`: every member is optional; an empty logbook has `{}`.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct System {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub omarchy: Option<OmarchyInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub packages: Option<Packages>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deviations: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snapshots: Option<Vec<SnapshotRow>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plugins: Option<PluginCounts>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub areas: Option<Vec<AreaRow>>,
    /// Recently edited files under `~/.config` outside the watch paths
    /// (ADR-0046).
    #[serde(rename = "recentConfig", skip_serializing_if = "Option::is_none")]
    pub recent_config: Option<RecentConfig>,
}

/// `system.recentConfig` (ADR-0046): the last scan's time and its files,
/// newest first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentConfig {
    pub scanned_at: String,
    pub files: Vec<RecentFile>,
    /// The scan stopped early or left deep folders out: the list may be
    /// incomplete (ADR-0046 §2). Written only when true.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub partial: bool,
}

/// One recently edited file: its `~`-path and modification time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, serde::Deserialize)]
pub struct RecentFile {
    pub path: String,
    pub mtime: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OmarchyInfo {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repo_head: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub theme: Option<String>,
    pub last_update: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Packages {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub explicit: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aur: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SnapshotRow {
    pub number: i64,
    pub ts: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub snapshot_type: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct PluginCounts {
    pub enabled: usize,
    pub installed: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AreaRow {
    pub name: String,
    pub has_agents_md: bool,
    pub cases: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct MemoryInfo {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lessons: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub topics: Option<Vec<TopicRow>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TopicRow {
    pub topic: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated: Option<String>,
    pub path: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Series {
    pub heatmap: Vec<HeatDay>,
    pub packages: Vec<PackagesDay>,
    pub drift: Vec<DriftWeek>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub risk: Option<BTreeMap<&'static str, usize>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeline: Option<Vec<TimelineItem>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HeatDay {
    pub date: String,
    pub total: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub by_source: Option<BTreeMap<&'static str, usize>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PackagesDay {
    pub date: String,
    pub explicit: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DriftWeek {
    pub week: String,
    pub opened: usize,
    pub resolved: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TimelineItem {
    pub kind: &'static str,
    pub ts: String,
    /// Case spans only: `Some(None)` is `null` (open case).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end: Option<Option<String>>,
    pub label: String,
    #[serde(rename = "ref")]
    pub reference: String,
}
