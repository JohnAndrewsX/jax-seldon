//! Keeping `index.json` under its size budget (CONTRACT.md rule 5,
//! ADR-0053). The index is built whole; only the file the plugin reads is
//! cut. Under the budget [`fit`] changes nothing. Over it, the lists of
//! [`LADDER`] lose their oldest rows, one rung after the other, until the
//! file fits. `summary` (and `series.risk`, `system.areas[].cases`) keep
//! the counts of the whole index, so the bar counts every crisis and every
//! open change; a cut row is still in the logbook. [`super::write`]
//! refuses a file that is still over the budget.

use std::collections::HashSet;

use serde::Serialize;

use super::build::SIZE_BUDGET;
use super::model::*;

/// The bytes serde_json writes for `value`, keeping none of them.
pub fn size(value: &impl Serialize) -> usize {
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
    let mut count = Count(0);
    serde_json::to_writer(&mut count, value).expect("the index always serialises");
    count.0
}

/// The bytes of `index.json` for `index`: its JSON and the newline of
/// [`super::to_text`].
pub fn file_size(index: &Index) -> usize {
    size(index) + 1
}

/// The rows one rung cuts (ADR-0053 §2), each from its oldest end.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Rows {
    /// `cases.completed`, the earliest `closed` first.
    Completed,
    /// `events`, the oldest first.
    Events,
    /// `cases.queued`, the earliest `created` first.
    Queued,
    /// `drift` items that are no crisis, the oldest first.
    Attention,
    /// `decisions` that are not `proposed`, the lowest id first.
    Decisions,
    /// `memory.topics`, the least recently updated first.
    Topics,
    /// `today.yesterday`, the earliest first.
    Yesterday,
    /// `series.timeline` releases and snapshots, the earliest first.
    Marks,
    /// `series.packages`, the earliest day first.
    PackageDays,
    /// `series.drift`, the earliest week first.
    DriftWeeks,
    /// `memory.lessons`, the first written first.
    Lessons,
    /// `system.areas`, from the end of the name order.
    Areas,
    /// `today.entries`, the earliest first.
    Entries,
    /// `cases.verification`, the earliest `created` first.
    Verification,
    /// `cases.active`, the earliest `created` first.
    Active,
    /// `decisions` that are `proposed`, the lowest id first.
    Proposed,
    /// `drift` items that are a crisis, the oldest first.
    Crises,
    /// The optional parts of `system`: snapshots, recent config files,
    /// the ignore list, the Omarchy and package summaries.
    System,
}

/// The order of the cut (ADR-0053 §2): each rung cuts its rows down to the
/// number given (rows it does not take do not count), the oldest first,
/// and only while the file is over the budget. The first seven keep a
/// screenful of each list; the rest leave history before the present and
/// crises last.
const LADDER: &[(Rows, usize)] = &[
    (Rows::Completed, 10),
    (Rows::Events, 100),
    (Rows::Queued, 50),
    (Rows::Attention, 50),
    (Rows::Decisions, 20),
    (Rows::Topics, 20),
    (Rows::Yesterday, 0),
    (Rows::Marks, 0),
    (Rows::PackageDays, 0),
    (Rows::DriftWeeks, 0),
    (Rows::Lessons, 0),
    (Rows::Topics, 0),
    (Rows::Areas, 0),
    (Rows::Completed, 0),
    (Rows::Events, 0),
    (Rows::Queued, 0),
    (Rows::Attention, 0),
    (Rows::Decisions, 0),
    (Rows::Entries, 0),
    (Rows::Verification, 0),
    (Rows::Active, 0),
    (Rows::Proposed, 0),
    (Rows::Crises, 0),
    (Rows::System, 0),
];

impl Rows {
    /// The words of the warning for one cut row and for several.
    fn words(self) -> (&'static str, &'static str) {
        match self {
            Rows::Completed => ("completed case", "completed cases"),
            Rows::Events => ("event", "events"),
            Rows::Queued => ("queued case", "queued cases"),
            Rows::Attention => ("open change", "open changes"),
            Rows::Decisions | Rows::Proposed => ("decision", "decisions"),
            Rows::Topics => ("memory topic", "memory topics"),
            Rows::Yesterday | Rows::Entries => ("journal entry", "journal entries"),
            Rows::Marks => ("timeline mark", "timeline marks"),
            Rows::PackageDays => ("package history day", "package history days"),
            Rows::DriftWeeks => ("drift week", "drift weeks"),
            Rows::Lessons => ("lesson", "lessons"),
            Rows::Areas => ("area", "areas"),
            Rows::Verification => ("case in verification", "cases in verification"),
            Rows::Active => ("active case", "active cases"),
            Rows::Crises => ("crisis row (still counted)", "crisis rows (still counted)"),
            Rows::System => ("system detail", "system details"),
        }
    }
}

/// Cuts `index` to under [`SIZE_BUDGET`] (ADR-0053). `None` when it fits
/// as built (nothing changes); else the warning that names the size, the
/// largest section and what was cut.
pub fn fit(index: &mut Index) -> Option<String> {
    let before = file_size(index);
    if before < SIZE_BUDGET {
        return None;
    }
    let (name, bytes) = largest_section(index);
    let mut cut: Vec<((&'static str, &'static str), usize)> = Vec::new();
    'ladder: for &(rows, floor) in LADDER {
        loop {
            let now = file_size(index);
            if now < SIZE_BUDGET {
                break 'ladder;
            }
            let n = trim(index, rows, floor, now - SIZE_BUDGET + 1);
            if n == 0 {
                break;
            }
            match cut.iter_mut().find(|(w, _)| *w == rows.words()) {
                Some((_, sum)) => *sum += n,
                None => cut.push((rows.words(), n)),
            }
        }
    }
    let after = file_size(index);
    let what = cut
        .iter()
        .map(|&((one, many), n)| format!("{n} {}", if n == 1 { one } else { many }))
        .collect::<Vec<_>>()
        .join(", ");
    let end = if after < SIZE_BUDGET {
        format!(
            "written at {after} bytes without the oldest {what}; the summary still counts every case, change and crisis, the logbook keeps every row"
        )
    } else {
        format!("still {after} bytes with every list cut; not written")
    };
    Some(format!(
        "index.json would be {before} bytes, over its budget of {SIZE_BUDGET} (CONTRACT.md rule 5); \
         the largest section is `{name}` ({bytes} bytes); {end}"
    ))
}

/// The largest top-level section of `index` and its bytes.
fn largest_section(index: &Index) -> (&'static str, usize) {
    [
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
    .expect("sections")
}

/// One step of a rung: cuts rows of `rows` until `excess` bytes are gone
/// or `floor` of them are left. Returns how many went.
fn trim(index: &mut Index, rows: Rows, floor: usize, excess: usize) -> usize {
    let newest_first = |n: usize| (0..n).rev().collect::<Vec<_>>();
    let oldest_first = |n: usize| (0..n).collect::<Vec<_>>();
    match rows {
        Rows::Events => {
            let order = newest_first(index.events.len());
            cut(&mut index.events, order, floor, excess).len()
        }
        Rows::Completed | Rows::Queued | Rows::Verification | Rows::Active => {
            let timeline = index.series.timeline.as_ref();
            let list = match rows {
                Rows::Completed => &mut index.cases.completed,
                Rows::Queued => &mut index.cases.queued,
                Rows::Verification => &mut index.cases.verification,
                _ => &mut index.cases.active,
            };
            let mut order = oldest_first(list.len());
            if rows == Rows::Completed {
                order.reverse();
            } else {
                order.sort_by(|&a, &b| {
                    (&list[a].created, &list[a].id).cmp(&(&list[b].created, &list[b].id))
                });
            }
            let span = |c: &IndexCase| marks(timeline, "case", &c.id);
            let gone = cut_with(list, order, floor, excess, span);
            if let Some(timeline) = &mut index.series.timeline {
                let ids: HashSet<&str> = gone.iter().map(|c| c.id.as_str()).collect();
                timeline.retain(|t| !(t.kind == "case" && ids.contains(t.reference.as_str())));
            }
            gone.len()
        }
        Rows::Attention | Rows::Crises => {
            let crisis = rows == Rows::Crises;
            let order = newest_first(index.drift.len())
                .into_iter()
                .filter(|&i| index.drift[i].crisis == crisis)
                .collect();
            let timeline = index.series.timeline.as_ref();
            let mark = |d: &DriftItem| {
                if crisis {
                    marks(timeline, "crisis", &d.event_id)
                } else {
                    0
                }
            };
            let gone = cut_with(&mut index.drift, order, floor, excess, mark);
            if crisis && let Some(timeline) = &mut index.series.timeline {
                let ids: HashSet<&str> = gone.iter().map(|d| d.event_id.as_str()).collect();
                timeline.retain(|t| !(t.kind == "crisis" && ids.contains(t.reference.as_str())));
            }
            gone.len()
        }
        Rows::Decisions | Rows::Proposed => {
            let proposed = rows == Rows::Proposed;
            let order = newest_first(index.decisions.len())
                .into_iter()
                .filter(|&i| (index.decisions[i].status == "proposed") == proposed)
                .collect();
            cut(&mut index.decisions, order, floor, excess).len()
        }
        Rows::Topics => index.memory.topics.as_mut().map_or(0, |list| {
            cut(list, newest_first(list.len()), floor, excess).len()
        }),
        Rows::Lessons => index.memory.lessons.as_mut().map_or(0, |list| {
            cut(list, oldest_first(list.len()), floor, excess).len()
        }),
        Rows::Yesterday => index.today.yesterday.as_mut().map_or(0, |list| {
            cut(list, oldest_first(list.len()), floor, excess).len()
        }),
        Rows::Entries => {
            let list = &mut index.today.entries;
            cut(list, oldest_first(list.len()), floor, excess).len()
        }
        Rows::Marks => index.series.timeline.as_mut().map_or(0, |list| {
            let order = oldest_first(list.len())
                .into_iter()
                .filter(|&i| matches!(list[i].kind, "release" | "snapshot"))
                .collect();
            cut(list, order, floor, excess).len()
        }),
        Rows::PackageDays => {
            let list = &mut index.series.packages;
            cut(list, oldest_first(list.len()), floor, excess).len()
        }
        Rows::DriftWeeks => {
            let list = &mut index.series.drift;
            cut(list, oldest_first(list.len()), floor, excess).len()
        }
        Rows::Areas => index.system.areas.as_mut().map_or(0, |list| {
            cut(list, newest_first(list.len()), floor, excess).len()
        }),
        Rows::System => {
            let s = &mut index.system;
            let parts = [
                s.snapshots.take().is_some(),
                s.recent_config.take().is_some(),
                s.pacman_ignore.take().is_some(),
                s.omarchy.take().is_some(),
                s.packages.take().is_some(),
            ];
            parts.into_iter().filter(|&p| p).count()
        }
    }
}

/// Takes rows out of `list` in `order` (indices, the first to go first)
/// until their bytes reach `excess` or `floor` of the rows `order` names
/// are left; returns them. A row's bytes are its JSON, its comma and
/// `also` (what goes with it elsewhere: a case's timeline span), so the
/// count is at most a byte high, and [`fit`] measures again.
fn cut_with<T: Serialize>(
    list: &mut Vec<T>,
    order: Vec<usize>,
    floor: usize,
    excess: usize,
    also: impl Fn(&T) -> usize,
) -> Vec<T> {
    let mut gone = vec![false; list.len()];
    let (mut left, mut freed) = (order.len(), 0);
    for i in order {
        if freed >= excess || left <= floor {
            break;
        }
        gone[i] = true;
        left -= 1;
        freed += size(&list[i]) + 1 + also(&list[i]);
    }
    let (mut kept, mut taken) = (Vec::new(), Vec::new());
    for (row, gone) in std::mem::take(list).into_iter().zip(gone) {
        if gone {
            taken.push(row);
        } else {
            kept.push(row);
        }
    }
    *list = kept;
    taken
}

/// [`cut_with`] for rows nothing else goes with.
fn cut<T: Serialize>(list: &mut Vec<T>, order: Vec<usize>, floor: usize, excess: usize) -> Vec<T> {
    cut_with(list, order, floor, excess, |_| 0)
}

/// The bytes of the timeline marks of `kind` that name `reference`.
fn marks(timeline: Option<&Vec<TimelineItem>>, kind: &str, reference: &str) -> usize {
    timeline
        .into_iter()
        .flatten()
        .filter(|t| t.kind == kind && t.reference == reference)
        .map(|t| size(t) + 1)
        .sum()
}
