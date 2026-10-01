//! Deriving the index from the loaded logbook (ADR-0012 §6–§15, ADR-0013
//! §1–§4, ADR-0015). The reference implementation is `derive()` in
//! `scripts/validate-fixtures.py`; the golden test (`tests/index.rs`)
//! holds both to `fixtures/index.sample.json`.

use std::collections::{BTreeMap, HashMap, HashSet};

use chrono::{DateTime, Datelike as _, Duration, FixedOffset, NaiveDate};
use ulid::Ulid;

use super::drift::{AlwaysRed, is_routine, names_token};
use super::load::{Entry, Loaded, LoadedCase, fence_kv, fence_table};
use super::model::*;
use crate::model::event::{Event, Kind, Source, format_ts};
use crate::model::{CaseStatus, Journal};

/// Most events the index lists (CONTRACT.md rule 4).
pub const MAX_EVENTS: usize = 500;
/// Most completed/dropped cases the index lists.
pub const MAX_COMPLETED: usize = 50;
/// Days of the heatmap, ending today.
pub const HEATMAP_DAYS: i64 = 366;
/// Most snapshots the index lists.
pub const MAX_SNAPSHOTS: usize = 10;
/// Most open drift items the index lists (ADR-0020): crises first, then
/// the newest; `summary` still counts all of them.
pub const MAX_DRIFT: usize = 200;

/// What the index needs besides the logbook.
#[derive(Debug, Clone)]
pub struct Input {
    /// `generatedAt`; its date is "today".
    pub now: DateTime<FixedOffset>,
    pub logbook_path: String,
    pub language: String,
    pub machine: String,
    pub git: Option<GitInfo>,
    pub state: State,
    /// `config.toml [drift] alwaysRed`.
    pub always_red: Vec<String>,
}

/// The index and what the generated Markdown views need besides it.
#[derive(Debug, Clone)]
pub struct Built {
    pub index: Index,
    /// Every ledger event as read (resolutions included), file order.
    pub ledger: Vec<Event>,
    /// Every non-resolution event, folded, newest first (`index.events`
    /// is the first [`MAX_EVENTS`] of these).
    pub folded: Vec<IndexEvent>,
    /// Ids of open drift events (every member of a group).
    pub open_drift: HashSet<Ulid>,
    /// What could not be read while loading.
    pub warnings: Vec<String>,
}

pub fn build(loaded: Loaded, input: &Input) -> Built {
    let today = input.now.date_naive();
    let Loaded {
        events,
        cases,
        journal_today,
        journal_yesterday,
        decisions,
        fences,
        lessons,
        mut topics,
        areas,
        warnings,
        ..
    } = loaded;

    let folded = fold(&events);
    let (drift, open_drift) = drift_items(&folded, &cases, &AlwaysRed::new(&input.always_red));
    let (drift_total, crisis_total) = (drift.len(), drift.iter().filter(|d| d.crisis).count());
    let drift = cap_drift(drift);
    let groups = case_groups(&cases, &drift);
    let all_cases: Vec<&IndexCase> = groups.all().collect();

    let summary = Summary {
        active_cases: groups.active.len(),
        queued_cases: groups.queued.len(),
        open_drift: drift_total,
        crisis: crisis_total,
        events_today: folded.iter().filter(|f| day(&f.event) == today).count(),
        events_7d: folded
            .iter()
            .filter(|f| (today - Duration::days(6)..=today).contains(&day(&f.event)))
            .count(),
    };

    let today_obj = Today {
        date: today.to_string(),
        path: Some(Journal::relative_path(today)),
        entries: rows(journal_today),
        yesterday: Some(rows(journal_yesterday)),
    };

    let mut decision_rows: Vec<DecisionRow> = decisions
        .into_iter()
        .map(|(path, d)| DecisionRow {
            id: d.id,
            title: d.title,
            status: d.status.as_str().to_string(),
            date: d.date.to_string(),
            path,
        })
        .collect();
    decision_rows.sort_by(|a, b| b.id.cmp(&a.id));

    let snapshots = snapshots(&events);
    let system = System {
        omarchy: fences.get("omarchy.summary").and_then(|f| {
            let kv = fence_kv(f);
            (!kv.is_empty()).then(|| OmarchyInfo {
                version: kv.get("version").cloned(),
                repo_head: kv.get("repoHead").cloned(),
                theme: kv.get("theme").cloned(),
                last_update: kv.get("lastUpdate").filter(|v| !v.is_empty()).cloned(),
            })
        }),
        packages: fences.get("packages.summary").and_then(|f| {
            let kv = fence_kv(f);
            let int = |k: &str| kv.get(k).and_then(|v| v.parse().ok());
            let p = Packages {
                explicit: int("explicit"),
                total: int("total"),
                aur: int("aur"),
            };
            (p != Packages::default()).then_some(p)
        }),
        deviations: fences.get("deviations.table").map(|f| fence_table(f).len()),
        snapshots: Some(snapshots.clone()),
        plugins: fences.get("plugins.list").map(|f| {
            let rows = fence_table(f);
            PluginCounts {
                enabled: rows
                    .iter()
                    .filter(|r| r.get("enabled").is_some_and(|v| v == "yes"))
                    .count(),
                installed: rows.len(),
            }
        }),
        areas: Some(
            areas
                .into_iter()
                .map(|(name, has_agents_md)| AreaRow {
                    cases: all_cases
                        .iter()
                        .filter(|c| c.area.as_deref() == Some(name.as_str()))
                        .count(),
                    name,
                    has_agents_md,
                })
                .collect(),
        ),
    };

    // other memory files by `updated` descending, then by topic
    topics.sort_by(|a, b| {
        b.updated
            .cmp(&a.updated)
            .then_with(|| a.topic.cmp(&b.topic))
    });
    let memory = MemoryInfo {
        lessons: Some(lessons.unwrap_or_default()),
        topics: Some(
            topics
                .into_iter()
                .map(|t| TopicRow {
                    topic: t.topic,
                    updated: t.updated,
                    path: t.path,
                })
                .collect(),
        ),
    };

    let series = Series {
        heatmap: heatmap(&folded, today),
        packages: fences
            .get("packages.history")
            .map(|f| {
                fence_table(f)
                    .into_iter()
                    .filter_map(|r| {
                        Some(PackagesDay {
                            date: r.get("date")?.clone(),
                            explicit: r.get("explicit")?.parse().ok()?,
                            total: r.get("total").and_then(|t| t.parse().ok()),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default(),
        drift: drift_weeks(&events, today),
        risk: Some(
            ["R0", "R1", "R2", "R3"]
                .into_iter()
                .map(|r| (r, all_cases.iter().filter(|c| c.risk == r).count()))
                .collect(),
        ),
        timeline: Some(timeline(&folded, &snapshots, &all_cases, &drift)),
    };

    let index = Index {
        contract_version: crate::CONTRACT_VERSION,
        generated_at: format_ts(&input.now),
        engine_version: crate::VERSION.to_string(),
        logbook: LogbookInfo {
            path: input.logbook_path.clone(),
            language: input.language.clone(),
            machine: input.machine.clone(),
            git: input.git.clone(),
        },
        state: input.state.clone(),
        summary,
        today: today_obj,
        events: folded.iter().take(MAX_EVENTS).cloned().collect(),
        drift,
        cases: groups,
        decisions: decision_rows,
        system,
        memory,
        series,
    };
    Built {
        index,
        ledger: events,
        folded,
        open_drift,
        warnings,
    }
}

/// An event's day: the date of its `ts` in its own offset.
pub fn day(e: &Event) -> NaiveDate {
    e.ts.date_naive()
}

/// Newest first by instant, ties by id descending.
fn newest_first(a: &Event, b: &Event) -> std::cmp::Ordering {
    (b.ts, b.id).cmp(&(a.ts, a.id))
}

/// Every event except resolutions, with the latest resolution of each
/// folded onto its target: `resolution`, `resolutionDetail` and, when
/// the resolution carries one, `case` (ADR-0012 §8, §11, ADR-0021). A resolution counts only for an
/// event earlier in the ledger.
fn fold(events: &[Event]) -> Vec<IndexEvent> {
    let mut seen: HashSet<Ulid> = HashSet::with_capacity(events.len());
    let mut resolutions: HashMap<Ulid, &Event> = HashMap::new();
    for e in events {
        if e.kind == Kind::Resolution
            && let Some(target) = e.refers_to
            && seen.contains(&target)
        {
            resolutions.insert(target, e);
        }
        seen.insert(e.id);
    }
    let mut folded: Vec<IndexEvent> = events
        .iter()
        .filter(|e| e.kind != Kind::Resolution)
        .map(|e| {
            let mut event = e.clone();
            let mut resolution_detail = None;
            if let Some(r) = resolutions.get(&e.id) {
                event.resolution = r.resolution;
                resolution_detail = r.detail.clone();
                // ADR-0021: any resolution that carries a case folds it
                if r.case.is_some() {
                    event.case = r.case.clone();
                }
            }
            IndexEvent {
                event,
                resolution_detail,
            }
        })
        .collect();
    folded.sort_by(|a, b| newest_first(&a.event, &b.event));
    folded
}

/// Whether a folded event is open drift (ADR-0012 §6): from a
/// system-changing collector, no case, no resolution.
pub fn is_open_drift(e: &Event) -> bool {
    e.source.is_drift_eligible() && e.case.is_none() && e.resolution.is_none()
}

/// Open drift items, newest first, and the ids of every open drift event.
/// Caseless pacman events of one transaction form one item (ADR-0013 §1);
/// its fields and proposal are the leader's (ADR-0015 §1), its zone is
/// computed (ADR-0013 §3, ADR-0015 §3).
fn drift_items(
    folded: &[IndexEvent],
    cases: &[LoadedCase],
    always_red: &AlwaysRed,
) -> (Vec<DriftItem>, HashSet<Ulid>) {
    let open_cases: Vec<&LoadedCase> = cases.iter().filter(|c| c.case.status.is_open()).collect();
    let proposed = |subject: &str| {
        open_cases
            .iter()
            .find(|c| names_token(&c.plan, subject))
            .map(|c| c.case.id.clone())
    };

    let mut items: Vec<Vec<&Event>> = Vec::new();
    let mut by_tx: HashMap<&str, usize> = HashMap::new();
    let mut open = HashSet::new();
    for f in folded {
        let e = &f.event;
        if !is_open_drift(e) {
            continue;
        }
        open.insert(e.id);
        match e.tx_id.as_deref().filter(|_| e.source == Source::Pacman) {
            Some(tx) => match by_tx.get(tx) {
                Some(&i) => items[i].push(e),
                None => {
                    by_tx.insert(tx, items.len());
                    items.push(vec![e]);
                }
            },
            None => items.push(vec![e]),
        }
    }

    let mut drift: Vec<(DateTime<FixedOffset>, Ulid, DriftItem)> = items
        .into_iter()
        .map(|members| {
            let explicit: Vec<&Event> = members
                .iter()
                .copied()
                .filter(|m| m.explicit == Some(true))
                .collect();
            let pool = if explicit.is_empty() {
                &members
            } else {
                &explicit
            };
            let lead = *pool
                .iter()
                .min_by_key(|m| m.id)
                .expect("an item has members");
            let zone = if lead.source == Source::Pacman {
                Some(if members.iter().all(|m| is_routine(m, always_red)) {
                    "yellow"
                } else {
                    "red"
                })
            } else {
                lead.zone.map(|z| z.as_str())
            };
            let group = members.len() > 1;
            let item = DriftItem {
                event_id: lead.id.to_string(),
                ts: format_ts(&lead.ts),
                source: lead.source.as_str().to_string(),
                kind: lead.kind.as_str().to_string(),
                subject: lead.subject.clone(),
                detail: lead.detail.clone(),
                actor: lead.actor.clone(),
                zone: zone.map(String::from),
                crisis: zone == Some("red"),
                proposed_case: proposed(&lead.subject),
                tx_id: group.then(|| lead.tx_id.clone()).flatten(),
                members: group.then_some(members.len()),
            };
            (lead.ts, lead.id, item)
        })
        .collect();
    drift.sort_by_key(|d| std::cmp::Reverse((d.0, d.1)));
    (drift.into_iter().map(|(_, _, d)| d).collect(), open)
}

/// At most [`MAX_DRIFT`] items (ADR-0020): every crisis before any other
/// item, the newest of each first; the kept items stay newest first.
fn cap_drift(drift: Vec<DriftItem>) -> Vec<DriftItem> {
    if drift.len() <= MAX_DRIFT {
        return drift;
    }
    let crises = drift.iter().filter(|d| d.crisis).count();
    let mut others_left = MAX_DRIFT.saturating_sub(crises);
    let mut crises_left = MAX_DRIFT;
    drift
        .into_iter()
        .filter(|d| {
            let left = if d.crisis {
                &mut crises_left
            } else {
                &mut others_left
            };
            let keep = *left > 0;
            *left = left.saturating_sub(1);
            keep
        })
        .collect()
}

/// `index.cases` (ADR-0012 §9): open groups by id, completed and dropped
/// together, newest `closed` first, at most [`MAX_COMPLETED`].
fn case_groups(cases: &[LoadedCase], drift: &[DriftItem]) -> Cases {
    let mut groups = Cases::default();
    for lc in cases {
        let c = &lc.case;
        let row = IndexCase {
            id: c.id.clone(),
            title: c.title.clone(),
            status: c.status.as_str().to_string(),
            zone: c.zone.as_str().to_string(),
            risk: c.risk.as_str().to_string(),
            priority: c.priority.map(|p| p.as_str().to_string()),
            area: c.area.clone(),
            created: c.created.to_string(),
            started: c.started.map(|d| d.to_string()),
            closed: c.closed.map(|d| d.to_string()),
            snapshot_before: c.snapshot_before,
            agents: c.agents.clone(),
            events: c.events.clone(),
            tags: c.tags.clone(),
            path: lc.path.clone(),
            steps: Steps {
                total: lc.steps.0,
                done: lc.steps.1,
            },
            proposed_events: drift
                .iter()
                .filter(|d| d.proposed_case.as_deref() == Some(c.id.as_str()))
                .map(|d| d.event_id.clone())
                .collect(),
        };
        match c.status {
            CaseStatus::Queued => groups.queued.push(row),
            CaseStatus::Active => groups.active.push(row),
            CaseStatus::Verification => groups.verification.push(row),
            CaseStatus::Completed | CaseStatus::Dropped => groups.completed.push(row),
        }
    }
    groups.completed.sort_by(|a, b| {
        (b.closed.as_deref().unwrap_or(""), &b.id).cmp(&(a.closed.as_deref().unwrap_or(""), &a.id))
    });
    groups.completed.truncate(MAX_COMPLETED);
    groups
}

fn rows(entries: Vec<Entry>) -> Vec<JournalRow> {
    entries
        .into_iter()
        .map(|e| JournalRow {
            time: e.time,
            actor: e.actor,
            case: e.case,
            text: e.text,
        })
        .collect()
}

/// Snapshot events not deleted later, newest first, at most
/// [`MAX_SNAPSHOTS`].
fn snapshots(events: &[Event]) -> Vec<SnapshotRow> {
    let deleted: HashSet<&str> = events
        .iter()
        .filter(|e| e.source == Source::Snapper && e.kind == Kind::SnapshotDelete)
        .map(|e| e.subject.as_str())
        .collect();
    let mut snaps: Vec<&Event> = events
        .iter()
        .filter(|e| {
            e.source == Source::Snapper
                && e.kind == Kind::Snapshot
                && !deleted.contains(e.subject.as_str())
        })
        .collect();
    snaps.sort_by(|a, b| newest_first(a, b));
    snaps
        .into_iter()
        .filter_map(|e| {
            Some(SnapshotRow {
                number: e.subject.parse().ok()?,
                ts: format_ts(&e.ts),
                description: e.detail.clone(),
                snapshot_type: e
                    .meta
                    .snapshot_type
                    .clone()
                    .filter(|t| matches!(t.as_str(), "single" | "pre" | "post")),
            })
        })
        .take(MAX_SNAPSHOTS)
        .collect()
}

/// [`HEATMAP_DAYS`] days ending today, oldest first; `bySource` only on
/// days with events.
fn heatmap(folded: &[IndexEvent], today: NaiveDate) -> Vec<HeatDay> {
    let first = today - Duration::days(HEATMAP_DAYS - 1);
    let mut by_day: HashMap<NaiveDate, BTreeMap<&'static str, usize>> = HashMap::new();
    for f in folded {
        let d = day(&f.event);
        if d >= first && d <= today {
            *by_day
                .entry(d)
                .or_default()
                .entry(f.event.source.as_str())
                .or_default() += 1;
        }
    }
    (0..HEATMAP_DAYS)
        .map(|i| {
            let d = first + Duration::days(i);
            let by = by_day.remove(&d);
            HeatDay {
                date: d.to_string(),
                total: by.as_ref().map_or(0, |b| b.values().sum()),
                by_source: by,
            }
        })
        .collect()
}

/// `YYYY-Www` of a date.
pub fn iso_week(d: NaiveDate) -> String {
    let w = d.iso_week();
    format!("{:04}-W{:02}", w.year(), w.week())
}

/// Every ISO week from the first ledger week to today: drift items opened
/// and resolution writes (ADR-0012 §10, ADR-0013 §4, ADR-0015 §5). A
/// caseless pacman transaction opens once, in the week of its earliest
/// line; the resolution lines of one group write (same `meta.txId`, `ts`,
/// `actor`) count once.
fn drift_weeks(events: &[Event], today: NaiveDate) -> Vec<DriftWeek> {
    #[derive(Hash, PartialEq, Eq)]
    enum Key<'a> {
        Event(Ulid),
        Tx(&'a str),
        Write(&'a str, String, &'a str),
    }
    let Some(first) = events.iter().map(day).min() else {
        return Vec::new();
    };
    let mut sorted: Vec<&Event> = events.iter().collect();
    sorted.sort_by(|a, b| newest_first(b, a));
    let mut seen: HashSet<Key> = HashSet::new();
    let mut opened: HashMap<String, usize> = HashMap::new();
    let mut resolved: HashMap<String, usize> = HashMap::new();
    for e in sorted {
        let (key, counter) =
            if e.source.is_drift_eligible() && e.case.is_none() && e.kind != Kind::Resolution {
                let key = match e.tx_id.as_deref().filter(|_| e.source == Source::Pacman) {
                    Some(tx) => Key::Tx(tx),
                    None => Key::Event(e.id),
                };
                (key, &mut opened)
            } else if e.kind == Kind::Resolution {
                let key = match e.meta.tx_id.as_deref() {
                    Some(tx) => Key::Write(tx, format_ts(&e.ts), e.actor.as_str()),
                    None => Key::Event(e.id),
                };
                (key, &mut resolved)
            } else {
                continue;
            };
        if seen.insert(key) {
            *counter.entry(iso_week(day(e))).or_default() += 1;
        }
    }
    let mut weeks = Vec::new();
    let mut d = first - Duration::days(i64::from(first.weekday().num_days_from_monday()));
    while d <= today {
        let week = iso_week(d);
        weeks.push(DriftWeek {
            opened: opened.get(&week).copied().unwrap_or(0),
            resolved: resolved.get(&week).copied().unwrap_or(0),
            week,
        });
        d += Duration::days(7);
    }
    weeks
}

/// Releases (omarchy `update`), current snapshots, non-dropped cases
/// (`created` → `closed`) and open crises, sorted by `ts` string, then
/// kind (ADR-0012 §10, §12).
fn timeline(
    folded: &[IndexEvent],
    snapshots: &[SnapshotRow],
    cases: &[&IndexCase],
    drift: &[DriftItem],
) -> Vec<TimelineItem> {
    let mut out = Vec::new();
    for f in folded {
        let e = &f.event;
        if e.source == Source::Omarchy && e.kind == Kind::Update {
            let to = e.meta.to.clone().unwrap_or_default();
            out.push(TimelineItem {
                kind: "release",
                ts: format_ts(&e.ts),
                end: None,
                label: format!("Omarchy {to}"),
                reference: to,
            });
        }
    }
    for s in snapshots {
        out.push(TimelineItem {
            kind: "snapshot",
            ts: s.ts.clone(),
            end: None,
            label: format!("{} {}", s.number, s.description.as_deref().unwrap_or(""))
                .trim()
                .to_string(),
            reference: s.number.to_string(),
        });
    }
    for c in cases.iter().filter(|c| c.status != "dropped") {
        out.push(TimelineItem {
            kind: "case",
            ts: c.created.clone(),
            end: Some(c.closed.clone()),
            label: format!("{} {}", c.id, c.title),
            reference: c.id.clone(),
        });
    }
    for d in drift.iter().filter(|d| d.crisis) {
        out.push(TimelineItem {
            kind: "crisis",
            ts: d.ts.clone(),
            end: None,
            label: format!("{} {} {}", d.source, d.kind, d.subject),
            reference: d.event_id.clone(),
        });
    }
    let order = |k: &str| match k {
        "release" => 0,
        "snapshot" => 1,
        "case" => 2,
        _ => 3,
    };
    out.sort_by(|a, b| {
        (&a.ts, order(a.kind), &a.reference).cmp(&(&b.ts, order(b.kind), &b.reference))
    });
    out
}
