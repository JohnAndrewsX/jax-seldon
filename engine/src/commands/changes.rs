//! `seldon changes --around <TS> [--before 2h] [--after 30m]` (WP-229): what
//! the ledger recorded around a moment — a crash, a failed boot, "it broke
//! at ten". The machine's timeline query, the one Seldon's skill gives an
//! agent that diagnoses a crash with Omarchy's `diagnose-crash`.
//!
//! Ledger only and read-only: no lock, no capture, no index, nothing
//! written. The events are the ledger lines as they were written (redacted
//! then), every kind, in time order, each with its `class` ([`class_of`]).
//! At most [`MAX_EVENTS`]: over that the ones nearest the moment are kept
//! and `omitted` counts the rest.

use std::collections::HashMap;
use std::fmt::Write as _;

use chrono::{
    DateTime, Datelike as _, Duration, FixedOffset, Local, NaiveDateTime, Offset as _,
    TimeZone as _,
};
use clap::Args;
use serde_json::{Value, json};

use super::{Context, Output, event_json};
use crate::config::AttentionMode;
use crate::error::{Error, Result};
use crate::frontmatter::printable;
use crate::index::class::{Class, Classifier, Rules};
use crate::index::drift::is_routine;
use crate::ledger::Ledger;
use crate::model::event::{Event, Source};
use crate::redact::Redactor;

/// The most events one answer lists.
pub const MAX_EVENTS: usize = 500;

/// The widest `--before` or `--after`.
const MAX_SPAN: Duration = Duration::days(31);

#[derive(Debug, Clone, Args)]
#[command(
    after_help = "TS is an RFC 3339 time, a local `YYYY-MM-DD HH:MM[:SS]` (or with `T`),
`@<seconds since 1970>`, or `now`. DURATION is a number with s, m, h or d,
and may be combined: 90m, 1h30m, 2d.

Examples:
  seldon changes --around '2026-10-01 14:20' --json
  seldon changes --around 2026-10-01T14:20:00+02:00 --before 6h --after 0m
  seldon changes --around @1790857200 --json"
)]
pub struct ChangesArgs {
    /// The moment to look around
    #[arg(long, value_name = "TS")]
    pub around: String,

    /// How far back from it
    #[arg(long, value_name = "DURATION", default_value = "2h")]
    pub before: String,

    /// How far on from it
    #[arg(long, value_name = "DURATION", default_value = "30m")]
    pub after: String,
}

pub fn run(ctx: &Context, args: ChangesArgs) -> Result<Output> {
    let around = parse_moment(&args.around, ctx.now)?;
    let before = parse_span("--before", &args.before)?;
    let after = parse_span("--after", &args.after)?;
    let (from, to) = window(&args.around, around, before, after)?;
    let (config, logbook) = ctx.open_logbook()?;
    let ledger = Ledger::new(&logbook, Redactor::builtin());
    // the class needs the ledger around the window too (a transaction's
    // explicit members, Omarchy's package history), as an index build
    let all = ledger.read_all()?;
    let mut events: Vec<&Event> = all.iter().filter(|e| e.ts >= from && e.ts <= to).collect();
    let omitted = events.len().saturating_sub(MAX_EVENTS);
    if omitted > 0 {
        events.sort_by_key(|e| (e.ts - around).num_milliseconds().abs());
        events.truncate(MAX_EVENTS);
    }
    // stable: events of one instant keep the ledger's order
    events.sort_by_key(|e| e.ts);
    let rules = Rules::new(&config.drift);
    let classes = Classes::new(&rules, &all);

    let mut human = format!(
        "{} event(s) from {} to {} (around {})",
        events.len(),
        short(&from),
        short(&to),
        short(&around)
    );
    if omitted > 0 {
        let _ = write!(human, "; {omitted} more further away not shown");
    }
    let mut listed = Vec::with_capacity(events.len());
    for e in events {
        let class = classes.of(e);
        let _ = write!(
            human,
            "\n  {}  {:<9}  {}/{}  {}  {}{}",
            short(&e.ts),
            class.map_or("-", Class::as_str),
            e.source,
            e.kind,
            printable(&e.subject),
            e.actor,
            e.case
                .as_deref()
                .map(|c| format!(" · {c}"))
                .unwrap_or_default()
        );
        let mut v = event_json(e);
        v["class"] = class.map_or(Value::Null, |c| c.as_str().into());
        listed.push(v);
    }
    Ok(Output::ok(
        human,
        json!({
            "around": around.to_rfc3339(),
            "from": from.to_rfc3339(),
            "to": to.to_rfc3339(),
            "events": listed,
            "omitted": omitted,
        }),
    ))
}

/// `[around − before, around + after]`, inside the years RFC 3339 writes
/// (0001 to 9999): past them is exit 1, never an overflow.
fn window(
    text: &str,
    around: DateTime<FixedOffset>,
    before: Duration,
    after: Duration,
) -> Result<(DateTime<FixedOffset>, DateTime<FixedOffset>)> {
    let readable = |t: Option<DateTime<FixedOffset>>| t.filter(|t| (1..=9999).contains(&t.year()));
    match (
        readable(around.checked_sub_signed(before)),
        readable(around.checked_add_signed(after)),
    ) {
        (Some(from), Some(to)) => Ok((from, to)),
        _ => Err(Error::user(format!(
            "--around {}: the window leaves the years 0001 to 9999",
            printable(text.trim())
        ))),
    }
}

/// The class of a machine change (ADR-0058 §6): ADR-0028's verdict, as an
/// index build gives it to the change's drift item — a pacman
/// transaction's members together, `attention = "all"` by zone — but
/// without the lift to attention an open case's Plan gives a routine item
/// (that is the case's state, not the change's), and whether or not the
/// change is resolved. `None` for what is no machine change: case events,
/// notes, snapshots, sessions, resolutions.
struct Classes<'a> {
    classifier: Classifier<'a>,
    transactions: HashMap<&'a str, Vec<&'a Event>>,
}

impl<'a> Classes<'a> {
    fn new(rules: &'a Rules, events: &'a [Event]) -> Self {
        let mut transactions: HashMap<&str, Vec<&Event>> = HashMap::new();
        for e in events.iter().filter(|e| changes_machine(e)) {
            if let Some(tx) = tx_of(e) {
                transactions.entry(tx).or_default().push(e);
            }
        }
        Classes {
            classifier: Classifier::new(rules, events),
            transactions,
        }
    }

    fn of(&self, e: &Event) -> Option<Class> {
        if !changes_machine(e) {
            return None;
        }
        let one = [e];
        let members: &[&Event] = tx_of(e)
            .and_then(|tx| self.transactions.get(tx))
            .map_or(&one, Vec::as_slice);
        let rules = self.classifier.rules;
        if rules.attention == AttentionMode::All {
            let red = if e.source == Source::Pacman {
                !members.iter().all(|m| is_routine(m, &rules.always_red))
            } else {
                e.zone.is_some_and(|z| z.as_str() == "red")
            };
            return Some(if red { Class::Crisis } else { Class::Attention });
        }
        let lead = members
            .iter()
            .copied()
            .filter(|m| m.explicit == Some(true))
            .min_by_key(|m| m.id)
            .or_else(|| members.iter().copied().min_by_key(|m| m.id))
            .unwrap_or(e);
        Some(self.classifier.group(members, lead).class)
    }
}

fn changes_machine(e: &Event) -> bool {
    e.source.is_drift_eligible() && e.resolution.is_none()
}

fn tx_of(e: &Event) -> Option<&str> {
    e.tx_id.as_deref().filter(|_| e.source == Source::Pacman)
}

fn short(ts: &DateTime<FixedOffset>) -> String {
    ts.format("%Y-%m-%d %H:%M:%S").to_string()
}

/// `--around`: RFC 3339, a local date and time, `@<epoch seconds>`, `now`.
pub fn parse_moment(text: &str, now: DateTime<FixedOffset>) -> Result<DateTime<FixedOffset>> {
    let text = text.trim();
    let refused = || {
        Error::user(format!(
            "--around {}: not a time; use RFC 3339 (2026-10-01T14:20:00+02:00), \
             a local `2026-10-01 14:20`, `@<seconds since 1970>` or `now`",
            printable(text)
        ))
    };
    if text == "now" {
        return Ok(now);
    }
    if let Some(secs) = text.strip_prefix('@') {
        if secs.is_empty() || !secs.bytes().all(|b| b.is_ascii_digit()) {
            return Err(refused());
        }
        let secs: i64 = secs.parse().map_err(|_| refused())?;
        let utc = DateTime::from_timestamp(secs, 0).ok_or_else(refused)?;
        return Ok(utc.with_timezone(&Local).fixed_offset());
    }
    if let Ok(t) = DateTime::parse_from_rfc3339(text) {
        return Ok(t);
    }
    for format in [
        "%Y-%m-%d %H:%M:%S",
        "%Y-%m-%dT%H:%M:%S",
        "%Y-%m-%d %H:%M",
        "%Y-%m-%dT%H:%M",
    ] {
        if let Ok(naive) = NaiveDateTime::parse_from_str(text, format) {
            return first_instant(naive).ok_or_else(|| {
                Error::user(format!(
                    "--around {}: that local time does not exist (a clock change); \
                     give it with its offset",
                    printable(text)
                ))
            });
        }
    }
    if chrono::NaiveDate::parse_from_str(text, "%Y-%m-%d").is_ok() {
        return Err(Error::user(format!(
            "--around {}: give a time with the date ({} 14:20); the forms are RFC 3339 \
             (2026-10-01T14:20:00+02:00), a local `2026-10-01 14:20`, \
             `@<seconds since 1970>` or `now`",
            printable(text),
            printable(text)
        )));
    }
    Err(refused())
}

/// The first instant the local clock showed `naive`: in the autumn
/// overlap the earlier of the two (still summer time), in the spring gap
/// none. Found from the offsets in force a day either side, not from
/// chrono's ambiguity report, which names the later one for some zones.
fn first_instant(naive: NaiveDateTime) -> Option<DateTime<FixedOffset>> {
    let as_utc = naive.and_utc();
    let mut found: Option<DateTime<FixedOffset>> = None;
    for probe in [
        as_utc.checked_sub_signed(Duration::days(1)),
        Some(as_utc),
        as_utc.checked_add_signed(Duration::days(1)),
    ]
    .into_iter()
    .flatten()
    {
        let offset = Local.offset_from_utc_datetime(&probe.naive_utc()).fix();
        let Some(t) = offset.from_local_datetime(&naive).single() else {
            continue;
        };
        // the offset holds at that instant: the clock showed `naive` then
        if Local.offset_from_utc_datetime(&t.naive_utc()).fix() == offset
            && found.is_none_or(|f| t < f)
        {
            found = Some(t);
        }
    }
    found
}

/// `--before` / `--after`: one or more `<number><s|m|h|d>`, at most
/// [`MAX_SPAN`] in all.
pub fn parse_span(flag: &str, text: &str) -> Result<Duration> {
    let refused = || {
        Error::user(format!(
            "{flag} {}: not a duration; use a number with s, m, h or d (30m, 1h30m, 2d), \
             at most 31d",
            printable(text)
        ))
    };
    let mut total: i64 = 0;
    let mut digits = String::new();
    let mut parts = 0;
    for c in text.chars() {
        if c.is_ascii_digit() {
            digits.push(c);
            continue;
        }
        let unit: i64 = match c {
            's' => 1,
            'm' => 60,
            'h' => 3600,
            'd' => 86_400,
            _ => return Err(refused()),
        };
        if digits.is_empty() || digits.len() > 9 {
            return Err(refused());
        }
        let n: i64 = digits.parse().map_err(|_| refused())?;
        total = n
            .checked_mul(unit)
            .and_then(|s| total.checked_add(s))
            .ok_or_else(refused)?;
        digits.clear();
        parts += 1;
    }
    if parts == 0 || !digits.is_empty() || total > MAX_SPAN.num_seconds() {
        return Err(refused());
    }
    Ok(Duration::seconds(total))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn now() -> DateTime<FixedOffset> {
        DateTime::parse_from_rfc3339("2026-10-01T17:00:00+02:00").unwrap()
    }

    #[test]
    fn spans() {
        let s = |t: &str| parse_span("--before", t).map(|d| d.num_seconds()).ok();
        assert_eq!(s("2h"), Some(7200));
        assert_eq!(s("30m"), Some(1800));
        assert_eq!(s("0m"), Some(0));
        assert_eq!(s("1h30m"), Some(5400));
        assert_eq!(s("90s"), Some(90));
        assert_eq!(s("31d"), Some(31 * 86_400));
        for bad in [
            "",
            "2",
            "h",
            "2x",
            "-2h",
            "2 h",
            "32d",
            "1d1s1d30d",
            "9999999999s",
            "2H",
        ] {
            assert_eq!(s(bad), None, "{bad}");
        }
    }

    #[test]
    fn moments() {
        let m = |t: &str| parse_moment(t, now()).ok();
        assert_eq!(m("now"), Some(now()));
        assert_eq!(
            m("2026-10-01T14:20:00+02:00").map(|t| t.to_rfc3339()),
            Some("2026-10-01T14:20:00+02:00".to_string())
        );
        assert_eq!(m("@1790857200").map(|t| t.timestamp()), Some(1_790_857_200));
        for local in [
            "2026-10-01 14:20",
            "2026-10-01T14:20",
            "2026-10-01 14:20:00",
        ] {
            let t = m(local).unwrap_or_else(|| panic!("{local}"));
            assert_eq!(t.format("%Y-%m-%d %H:%M").to_string(), "2026-10-01 14:20");
        }
        for bad in ["", "yesterday", "@", "@-5", "@1.5", "2026-10-01", "14:20"] {
            assert_eq!(m(bad), None, "{bad}");
        }
    }
}
