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
use super::{Collector, Ctx, Outcome, to_cursor, typed_cursor};
use crate::model::event::{Event, Kind, Meta, Source};
use crate::sys::Run;

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

    fn collect(&self, ctx: &Ctx, cursor: Option<&Value>) -> Outcome {
        let Some(version) = current_version(ctx) else {
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
            _ => return Outcome::ok(Vec::new(), next),
        };
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

/// `omarchy-version`, else the version column of the package query.
fn current_version(ctx: &Ctx) -> Option<String> {
    let ok = |run: Run| match run {
        Run::Exited {
            code: Some(0),
            stdout,
            ..
        } => Some(stdout),
        _ => None,
    };
    let valid = |v: &str| !v.is_empty() && !v.contains(char::is_whitespace);
    if let Some(out) = ok(ctx.run(&ctx.sources.omarchy_version, &[])) {
        let v = out.trim();
        if valid(v) {
            return Some(v.to_string());
        }
    }
    let out = ok(ctx.run(&ctx.sources.pacman, &["-Q", "omarchy"]))?;
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
