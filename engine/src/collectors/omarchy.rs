//! `omarchy` collector (SPEC-ENGINE §4, memory/host.md item 1–2).
//!
//! The version comes from `omarchy-version` (prints e.g. `4.0.4-1`).
//! `omarchy --version` does not exist and `$OMARCHY_PATH/version` is
//! stale. The fallback is `pacman -Q omarchy`. Omarchy is a package install,
//! not a git checkout, so nothing here looks at git.
//!
//! A version change becomes an `update` event (`subject: omarchy`,
//! `meta.from`/`to`, red) at capture time. Without a cursor, the version is
//! recorded and no event is written.
//!
//! Attribution (ADR-0014 §1): the event takes `actor` and `case` from the
//! pacman event that moved the `omarchy` package to the new version (same
//! capture or the ledger). Without such an event, it takes them from a hook
//! command that ran a full upgrade at most 10 minutes before the capture.

use chrono::Duration;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::pacman::{ATTRIBUTION_WINDOW, causes};
use super::{Collector, Ctx, Lost, Outcome, RUN_TIMEOUT, Sources, to_cursor, typed_cursor};
use crate::model::event::{Event, Kind, Meta, Source};
use crate::sys::{self, Run};

pub struct Omarchy;

/// `cursors.json` → `omarchy.cursor`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OmarchyCursor {
    pub version: String,
}

/// How far back the ledger is searched for the pacman upgrade of `omarchy`.
const LOOKBACK: Duration = Duration::days(31);

impl Collector for Omarchy {
    fn name(&self) -> &'static str {
        "omarchy"
    }

    fn cursor_reads(&self, cursor: &Value) -> bool {
        typed_cursor::<OmarchyCursor>(Some(cursor)).is_some()
    }

    fn collect(&self, ctx: &Ctx, cursor: Option<&Value>) -> Outcome {
        let Some(version) = current_version(ctx.sources) else {
            return Outcome::degraded(
                "cannot read the Omarchy version (`omarchy-version` and the package query both failed)",
                None,
            );
        };
        let previous: Option<OmarchyCursor> = typed_cursor(cursor);
        let next = to_cursor(&OmarchyCursor {
            version: version.clone(),
        });
        let from = match previous {
            Some(p) if p.version != version => p.version,
            Some(_) => return Outcome::ok(Vec::new(), next),
            None => return Outcome::ok(Vec::new(), next).baseline(Some(Lost::Cursor)),
        };
        match already_recorded(ctx, &from, &version) {
            Ok(true) => return Outcome::ok(Vec::new(), next),
            Ok(false) => {}
            Err(err) => return Outcome::degraded(format!("{err:#}"), None),
        }
        let mut e = Event::new(ctx.now, Source::Omarchy, Kind::Update, "omarchy")
            .detail(format!("{from} → {version}"))
            .meta(Meta {
                from: Some(from),
                to: Some(version.clone()),
                ..Meta::default()
            });
        match attribution(ctx, &version) {
            Ok(Some((actor, case))) => e = e.actor(actor).case(case),
            Ok(None) => {}
            Err(err) => return Outcome::degraded(format!("{err:#}"), None),
        }
        Outcome::ok(vec![e], next)
    }
}

/// Whether the ledger's latest `update` within [`LOOKBACK`] is already
/// `from → to`: the ledger was written but the cursor was not saved (a
/// crash in between). The latest one, so that a real re-upgrade after a
/// downgrade is still recorded.
fn already_recorded(ctx: &Ctx, from: &str, to: &str) -> anyhow::Result<bool> {
    let latest = ctx
        .ledger
        .read_range(ctx.now - LOOKBACK, ctx.now)?
        .into_iter()
        .filter(|e| e.source == Source::Omarchy && e.kind == Kind::Update)
        .max_by_key(|e| e.ts);
    Ok(latest
        .is_some_and(|e| e.meta.from.as_deref() == Some(from) && e.meta.to.as_deref() == Some(to)))
}

/// `omarchy-version` (with `OMARCHY_PATH` defaulted,
/// [`sys::omarchy_command`]), else the version column of the package query
/// (also read by `seldon dossier`).
pub fn current_version(sources: &Sources) -> Option<String> {
    let ok = |run: Run| match run {
        Run::Exited {
            code: Some(0),
            stdout,
            ..
        } => Some(stdout),
        _ => None,
    };
    let valid = |v: &str| !v.is_empty() && !v.contains(char::is_whitespace);
    let omarchy_version = sys::omarchy_command(&sources.omarchy_version, &[]);
    if let Some(out) = ok(sys::run_command(omarchy_version, RUN_TIMEOUT)) {
        let v = out.trim();
        if valid(v) {
            return Some(v.to_string());
        }
    }
    let out = ok(sys::run(
        &sources.pacman,
        &["-Q", "omarchy"],
        None,
        RUN_TIMEOUT,
    ))?;
    let mut words = out.split_whitespace();
    match (words.next(), words.next()) {
        (Some("omarchy"), Some(v)) if valid(v) => Some(v.to_string()),
        _ => None,
    }
}

/// Actor and case of what caused the update, if anything did.
fn attribution(ctx: &Ctx, version: &str) -> anyhow::Result<Option<(String, Option<String>)>> {
    let mut known = ctx.ledger.read_range(ctx.now - LOOKBACK, ctx.now)?;
    known.extend(ctx.earlier.iter().cloned());
    let upgrade = known
        .iter()
        .filter(|e| {
            e.source == Source::Pacman
                && e.subject == "omarchy"
                && matches!(e.kind, Kind::Install | Kind::Upgrade | Kind::Downgrade)
                && e.version_key() == Some(version)
        })
        .max_by_key(|e| e.ts);
    if let Some(u) = upgrade {
        return Ok(Some((u.actor.clone(), u.case.clone())));
    }
    Ok(causes(&known)
        .into_iter()
        .filter(|c| {
            c.intent.full_upgrade && c.ts <= ctx.now && c.ts >= ctx.now - ATTRIBUTION_WINDOW
        })
        .max_by_key(|c| c.ts)
        .map(|c| (c.actor, c.case)))
}
