//! `seldon changes --around <TS> [--before 2h] [--after 30m]` (WP-229): what
//! the ledger recorded around a moment — a crash, a failed boot, "it broke
//! at ten". The machine's timeline query, the one Seldon's skill gives an
//! agent that diagnoses a crash with Omarchy's `diagnose-crash`.
//!
//! Ledger only and read-only: no lock, no capture, no index, nothing
//! written. The events are the ledger lines as they were written (redacted
//! then), every kind, in time order. At most [`MAX_EVENTS`]: over that the
//! ones nearest the moment are kept and `omitted` counts the rest.

use std::fmt::Write as _;

use chrono::{DateTime, Duration, FixedOffset, Local, NaiveDateTime, TimeZone as _};
use clap::Args;
use serde_json::json;

use super::{Context, Output, event_json};
use crate::error::{Error, Result};
use crate::frontmatter::printable;
use crate::ledger::Ledger;
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
    let (from, to) = (around - before, around + after);
    let (_, logbook) = ctx.open_logbook()?;
    let ledger = Ledger::new(&logbook, Redactor::builtin());
    let mut events = ledger.read_range(from, to)?;
    let omitted = events.len().saturating_sub(MAX_EVENTS);
    if omitted > 0 {
        events.sort_by_key(|e| (e.ts - around).num_milliseconds().abs());
        events.truncate(MAX_EVENTS);
    }
    // stable: events of one instant keep the ledger's order
    events.sort_by_key(|e| e.ts);

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
    for e in &events {
        let _ = write!(
            human,
            "\n  {}  {}/{}  {}  {}{}",
            short(&e.ts),
            e.source,
            e.kind,
            printable(&e.subject),
            e.actor,
            e.case
                .as_deref()
                .map(|c| format!(" · {c}"))
                .unwrap_or_default()
        );
    }
    Ok(Output::ok(
        human,
        json!({
            "around": around.to_rfc3339(),
            "from": from.to_rfc3339(),
            "to": to.to_rfc3339(),
            "events": events.iter().map(event_json).collect::<Vec<_>>(),
            "omitted": omitted,
        }),
    ))
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
            // a time the clock skipped (the spring change) is no moment;
            // one it passed twice is its first
            return Local
                .from_local_datetime(&naive)
                .earliest()
                .map(|t| t.fixed_offset())
                .ok_or_else(|| {
                    Error::user(format!(
                        "--around {}: that local time does not exist (a clock change); \
                         give it with its offset",
                        printable(text)
                    ))
                });
        }
    }
    Err(refused())
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
