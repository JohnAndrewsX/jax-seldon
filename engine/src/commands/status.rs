//! `seldon status` (SPEC-ENGINE §3): regenerates `STATUS.md`, the
//! `ledger/YYYY-MM.md` views and `index.json`, and prints a summary.
//!
//! `STATUS.md` keeps everything outside its `status` fence (user text).
//! When a logbook file changed, the autocommit records it as
//! `seldon: status`. Before `seldon init` it writes the `notInitialised`
//! index and exits 3 (the plugin's cue to offer `seldon init`).

use std::fmt::Write as _;

use clap::Args;
use serde_json::json;

use super::index::{rebuild_with, rebuilt_json, warnings_human};
use super::{Commit, Context, Output, autocommit};
use crate::error::Result;

#[derive(Debug, Clone, Args)]
pub struct StatusArgs {}

pub fn run(ctx: &Context, _args: StatusArgs) -> Result<Output> {
    let (r, commit) = rebuild_with(ctx, false, true, |config, logbook, files| {
        (!files.is_empty()).then(|| autocommit(ctx, config, logbook, "status"))
    })?;
    let commit = commit.unwrap_or(Commit::Skipped("nothing changed"));
    let ix = &r.built.index;
    let s = &ix.summary;

    let mut human = format!(
        "Status of {} ({})\n  cases   {} active · {} in verification · {} queued\n  drift   {} open · {} crisis\n  events  {} today · {} in 7 days",
        ix.logbook.machine,
        ix.today.date,
        s.active_cases,
        ix.cases.verification.len(),
        s.queued_cases,
        s.open_drift,
        s.crisis,
        s.events_today,
        s.events_7d,
    );
    for c in ix.state.collectors.iter().filter(|c| !c.ok) {
        let _ = write!(human, "\n  {:<7} degraded", c.name);
        if let Some(m) = &c.message {
            let _ = write!(human, ": {m}");
        }
    }
    if !r.files.is_empty() {
        let _ = write!(human, "\nWrote {}", r.files.join(", "));
    }
    let _ = write!(human, "\nIndex: {}", ctx.dirs.display(&r.index_path));
    human.push_str(&commit.human());
    human.push_str(&warnings_human(&r.built.warnings));

    let mut out = rebuilt_json(&r);
    out["status"] = json!("STATUS.md");
    out["state"] = serde_json::to_value(&ix.state).map_err(anyhow::Error::from)?;
    out["git"] = commit.json();
    Ok(Output::ok(human, out))
}
