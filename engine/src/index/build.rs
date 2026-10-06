//! Deriving the index from the loaded logbook (ADR-0012 §6–§15, ADR-0013
//! §1–§4, ADR-0015). The reference implementation is `derive()` in
//! `scripts/validate-fixtures.py`; the golden test (`tests/index.rs`)
//! holds both to `fixtures/index.sample.json`.

use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap, HashSet};

use chrono::{DateTime, Datelike as _, Duration, FixedOffset, NaiveDate};
use ulid::Ulid;

use super::class::{Class, Classifier, Rules};
use super::drift::{is_routine, names_token};
use super::load::{Entry, Loaded, LoadedCase, fence_kv, fence_table};
use super::model::*;
use crate::config::{AttentionMode, DriftConfig};
use crate::model::event::{Event, Kind, Source, format_ts};
use crate::model::{CaseStatus, Decision, Journal};

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
/// The size budget of `index.json` in bytes (CONTRACT.md rule 5: < 1 MB).
pub const SIZE_BUDGET: usize = 1_000_000;
/// Most bytes one free text of an index event or drift item takes in
/// `index.json` (JSON-escaped, marker included): `detail`,
/// `resolutionDetail` and every string in `meta`. 500 events and 200
/// drift items then fit the [`SIZE_BUDGET`] whatever their ledger lines
/// hold; the ledger line keeps the full text.
pub const TEXT_MAX: usize = 256;

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
    /// `config.toml [drift]` (ADR-0013, ADR-0028).
    pub drift: DriftConfig,
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
    /// Ids of open drift events: every member of an item `index.drift`
    /// lists or would list past the cap (ADR-0028: not routine, or named
    /// by an open case's Plan).
    pub open_drift: HashSet<Ulid>,
    /// Ids of every event a resolution may still take (ADR-0028 §8: drift
    /// eligible, no case, no resolution), routine ones included.
    pub linkable: HashSet<Ulid>,
    /// Every item of linkable events with its class, newest first,
    /// uncapped (`drift --all`, `drift show`).
    pub items: Vec<ClassifiedItem>,
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
        mut warnings,
        ..
    } = loaded;

    let folded = fold(&events);
    let rules = Rules::new(&input.drift);
    let classifier = Classifier::new(&rules, &events);
    let items = drift_items(&folded, &cases, &classifier);
    let linkable: HashSet<Ulid> = items
        .iter()
        .flat_map(|i| i.members.iter().copied())
        .collect();
    let open_drift: HashSet<Ulid> = items
        .iter()
        .filter(|i| i.listed)
        .flat_map(|i| i.members.iter().copied())
        .collect();
    let drift: Vec<DriftItem> = items
        .iter()
        .filter(|i| i.listed)
        .map(|i| i.item.clone())
        .collect();
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

    let decision_rows = decision_rows(decisions);

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
        drift: drift_weeks(&events, today, &classifier),
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
        events: folded.iter().take(MAX_EVENTS).map(clipped).collect(),
        drift,
        cases: groups,
        decisions: decision_rows,
        system,
        memory,
        series,
    };
    warnings.extend(over_budget(&index));
    Built {
        index,
        ledger: events,
        folded,
        open_drift,
        linkable,
        items,
        warnings,
    }
}

/// The `decisions` rows of the index, newest id first (also the rows of
/// the logbook's `DECISIONS.md` table, [`super::views::decisions_index`]).
pub fn decision_rows(decisions: Vec<(String, Decision)>) -> Vec<DecisionRow> {
    let mut rows: Vec<DecisionRow> = decisions
        .into_iter()
        .map(|(path, d)| DecisionRow {
            id: d.id,
            title: d.title,
            status: d.status.as_str().to_string(),
            date: d.date.to_string(),
            path,
        })
        .collect();
    rows.sort_by(|a, b| b.id.cmp(&a.id));
    rows
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
    let mut folded: Vec<IndexEvent> = Vec::with_capacity(events.len());
    folded.extend(
        events
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
            }),
    );
    folded.sort_by(|a, b| newest_first(&a.event, &b.event));
    folded
}

/// `text` as the index carries it: unchanged when it takes at most
/// [`TEXT_MAX`] bytes in JSON, else its start followed by a visible
/// marker, `… (3744 more characters in the ledger)` (CONTRACT.md rule 5).
/// The cut falls on a character boundary.
pub fn clip(text: &str) -> Cow<'_, str> {
    // bytes of one character in JSON (serde_json escapes only these)
    let json_len = |c: char| match c {
        '"' | '\\' | '\n' | '\r' | '\t' | '\u{8}' | '\u{c}' => 2,
        c if u32::from(c) < 0x20 => 6,
        c => c.len_utf8(),
    };
    if text.len() * 6 <= TEXT_MAX || text.chars().map(json_len).sum::<usize>() <= TEXT_MAX {
        return Cow::Borrowed(text);
    }
    let marker = |left: usize| {
        let unit = if left == 1 { "character" } else { "characters" };
        format!("… ({left} more {unit} in the ledger)")
    };
    let total = text.chars().count();
    // the marker for every character is at least as long as the real one
    let room = TEXT_MAX - marker(total).len();
    let (mut used, mut end) = (0, 0);
    for (i, c) in text.char_indices() {
        used += json_len(c);
        if used > room {
            break;
        }
        end = i + c.len_utf8();
    }
    let head = text[..end].trim_end();
    Cow::Owned(format!("{head}{}", marker(total - head.chars().count())))
}

/// An event as `index.events` lists it: every free text [`clip`]ped.
fn clipped(f: &IndexEvent) -> IndexEvent {
    let mut f = f.clone();
    let meta = &mut f.event.meta;
    let texts = [
        &mut f.event.detail,
        &mut f.resolution_detail,
        &mut meta.command,
        &mut meta.version,
        &mut meta.from,
        &mut meta.to,
        &mut meta.hash_from,
        &mut meta.hash_to,
        &mut meta.snapshot_type,
        &mut meta.cleanup,
        &mut meta.tx_id,
    ];
    for text in texts.into_iter().flatten() {
        if let Cow::Owned(short) = clip(text) {
            *text = short;
        }
    }
    for value in meta.extra.values_mut() {
        if let serde_json::Value::String(text) = value
            && let Cow::Owned(short) = clip(text)
        {
            *text = short;
        }
    }
    f
}

/// A warning when the serialised index is not under [`SIZE_BUDGET`]
/// (CONTRACT.md rule 5), naming its largest section. Events and drift are
/// clipped and capped; open cases, decisions and memory topics are not.
fn over_budget(index: &Index) -> Option<String> {
    /// Counts the bytes serde_json writes, keeping none of them.
    struct Count(usize);
    impl std::io::Write for Count {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0 += buf.len();
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    fn size(value: &impl serde::Serialize) -> usize {
        let mut count = Count(0);
        serde_json::to_writer(&mut count, value).expect("the index always serialises");
        count.0
    }
    let total = size(index) + 1; // the newline of `index::to_text`
    if total < SIZE_BUDGET {
        return None;
    }
    let (name, bytes) = [
        ("events", size(&index.events)),
        ("drift", size(&index.drift)),
        ("cases", size(&index.cases)),
        ("decisions", size(&index.decisions)),
        ("memory", size(&index.memory)),
        ("series", size(&index.series)),
        ("system", size(&index.system)),
        ("today", size(&index.today)),
    ]
    .into_iter()
    .max_by_key(|s| s.1)
    .expect("sections");
    Some(format!(
        "index.json is {total} bytes, over its budget of {SIZE_BUDGET} (CONTRACT.md rule 5); \
         the largest section is `{name}` ({bytes} bytes); the plugin reads the whole file"
    ))
}

/// Whether a folded event can still be resolved (ADR-0012 §6, ADR-0028
/// §8): from a system-changing collector, no case, no resolution. Whether
/// it is open drift depends on its item's class ([`Built::open_drift`]).
pub fn is_linkable(e: &Event) -> bool {
    e.source.is_drift_eligible() && e.case.is_none() && e.resolution.is_none()
}

/// One item of linkable events and its class (ADR-0028 §2).
#[derive(Debug, Clone)]
pub struct ClassifiedItem {
    /// The item as `index.drift` lists it; `crisis` follows the class.
    pub item: DriftItem,
    /// The class shown: routine, or attention for a routine item an open
    /// case's Plan names (ADR-0028 §3), attention, crisis.
    pub class: Class,
    /// The row of the table that gave the class (`attention-all` under
    /// `[drift] attention = "all"`).
    pub rule: &'static str,
    /// Its events, newest first.
    pub members: Vec<Ulid>,
    /// Whether it is open drift (`index.drift` lists it, up to the cap).
    pub listed: bool,
}

/// Every item of linkable events, newest first. Caseless pacman events of
/// one transaction form one item (ADR-0013 §1); its fields and proposal
/// are the leader's (ADR-0015 §1). Its class is the highest of its members
/// (ADR-0028 §2); `zone` is the leader's ledger zone, `crisis` the class.
/// Under `attention = "all"` every item is open drift and the pacman zone
/// is computed, crisis iff red (ADR-0013 §3, the rule before ADR-0028).
fn drift_items(
    folded: &[IndexEvent],
    cases: &[LoadedCase],
    classifier: &Classifier,
) -> Vec<ClassifiedItem> {
    let open_cases: Vec<&LoadedCase> = cases.iter().filter(|c| c.case.status.is_open()).collect();
    // one scan of the open Plans per subject: routine items need their
    // proposal too, and subjects repeat (themes, packages, config files)
    let mut proposals: HashMap<String, Option<String>> = HashMap::new();
    let mut proposed = |subject: &str| -> Option<String> {
        if let Some(p) = proposals.get(subject) {
            return p.clone();
        }
        let p = open_cases
            .iter()
            .find(|c| names_token(&c.plan, subject))
            .map(|c| c.case.id.clone());
        proposals.insert(subject.to_string(), p.clone());
        p
    };
    let rules = classifier.rules;
    let legacy = rules.attention == AttentionMode::All;

    let mut items: Vec<Vec<&Event>> = Vec::new();
    let mut by_tx: HashMap<&str, usize> = HashMap::new();
    for f in folded {
        let e = &f.event;
        if !is_linkable(e) {
            continue;
        }
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

    let mut out: Vec<(DateTime<FixedOffset>, Ulid, ClassifiedItem)> = items
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
            let proposed_case = proposed(&lead.subject);
            let (zone, class, rule) = if legacy {
                let zone = if lead.source == Source::Pacman {
                    Some(
                        if members.iter().all(|m| is_routine(m, &rules.always_red)) {
                            "yellow"
                        } else {
                            "red"
                        },
                    )
                } else {
                    lead.zone.map(|z| z.as_str())
                };
                let class = if zone == Some("red") {
                    Class::Crisis
                } else {
                    Class::Attention
                };
                (zone, class, "attention-all")
            } else {
                let v = classifier.group(&members, lead);
                let class = match v.class {
                    Class::Routine if proposed_case.is_some() => Class::Attention,
                    c => c,
                };
                (lead.zone.map(|z| z.as_str()), class, v.rule)
            };
            let group = members.len() > 1;
            let item = DriftItem {
                event_id: lead.id.to_string(),
                ts: format_ts(&lead.ts),
                source: lead.source.as_str().to_string(),
                kind: lead.kind.as_str().to_string(),
                subject: lead.subject.clone(),
                detail: lead.detail.as_deref().map(|d| clip(d).into_owned()),
                actor: lead.actor.clone(),
                zone: zone.map(String::from),
                crisis: class == Class::Crisis,
                proposed_case,
                tx_id: group.then(|| lead.tx_id.clone()).flatten(),
                members: group.then_some(members.len()),
            };
            let classified = ClassifiedItem {
                item,
                class,
                rule,
                members: members.iter().map(|m| m.id).collect(),
                listed: class != Class::Routine,
            };
            (lead.ts, lead.id, classified)
        })
        .collect();
    out.sort_by_key(|d| std::cmp::Reverse((d.0, d.1)));
    out.into_iter().map(|(_, _, d)| d).collect()
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
/// and resolution writes (ADR-0012 §10, ADR-0013 §4, ADR-0015 §5,
/// ADR-0028 §5). A caseless pacman transaction opens once, in the week of
/// its earliest line, unless its class is routine; a routine event opens
/// nothing. The resolution lines of one group write (same `meta.txId`,
/// `ts`, `actor`) count once, and only when the event they resolve opened
/// an item, so the curve cannot go negative. The class here ignores
/// proposals (they depend on today's open cases).
fn drift_weeks(events: &[Event], today: NaiveDate, classifier: &Classifier) -> Vec<DriftWeek> {
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

    let caseless =
        |e: &Event| e.source.is_drift_eligible() && e.case.is_none() && e.kind != Kind::Resolution;
    // which caseless events opened an item: the members of every group
    // whose class is not routine (every group under `attention = "all"`)
    let mut groups: Vec<Vec<&Event>> = Vec::new();
    let mut by_tx: HashMap<&str, usize> = HashMap::new();
    for e in sorted.iter().copied().filter(|e| caseless(e)) {
        match e.tx_id.as_deref().filter(|_| e.source == Source::Pacman) {
            Some(tx) => match by_tx.get(tx) {
                Some(&i) => groups[i].push(e),
                None => {
                    by_tx.insert(tx, groups.len());
                    groups.push(vec![e]);
                }
            },
            None => groups.push(vec![e]),
        }
    }
    let legacy = classifier.rules.attention == AttentionMode::All;
    let opened_ids: HashSet<Ulid> = groups
        .iter()
        .filter(|members| {
            legacy || {
                let lead = members
                    .iter()
                    .copied()
                    .filter(|m| m.explicit == Some(true))
                    .min_by_key(|m| m.id)
                    .or_else(|| members.iter().copied().min_by_key(|m| m.id))
                    .expect("a group has members");
                classifier.group(members, lead).class != Class::Routine
            }
        })
        .flatten()
        .map(|e| e.id)
        .collect();
    let mut seen: HashSet<Key> = HashSet::new();
    let mut opened: HashMap<String, usize> = HashMap::new();
    let mut resolved: HashMap<String, usize> = HashMap::new();
    for e in sorted {
        let (key, counter) = if caseless(e) {
            if !opened_ids.contains(&e.id) {
                continue;
            }
            let key = match e.tx_id.as_deref().filter(|_| e.source == Source::Pacman) {
                Some(tx) => Key::Tx(tx),
                None => Key::Event(e.id),
            };
            (key, &mut opened)
        } else if e.kind == Kind::Resolution {
            if !e.refers_to.is_some_and(|t| opened_ids.contains(&t)) {
                continue;
            }
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
