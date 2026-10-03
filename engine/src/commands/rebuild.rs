//! `seldon rebuild` (SPEC-ENGINE §3, WP-032): writes `outputs/REBUILD.md`,
//! the steps that take a fresh Omarchy install to the documented state.
//!
//! The sections come from the index derivation and the dossier
//! ([`crate::rebuild`]). The file is replaced atomically and only when its
//! bytes change; text outside the `rebuild` fence is the user's and kept.
//! A change is committed as `seldon: rebuild`. Nothing is written to the
//! ledger, so the index is not rebuilt.

use std::cell::RefCell;
use std::collections::HashMap;

use clap::Args;
use serde_json::json;

use super::index::warnings_human;
use super::{Commit, Context, Output, autocommit};
use crate::error::Result;
use crate::index;
use crate::logbook::cases;
use crate::rebuild::{self, REL_PATH};
use crate::sys;

#[derive(Debug, Clone, Args)]
pub struct RebuildArgs {}

pub fn run(ctx: &Context, _args: RebuildArgs) -> Result<Output> {
    let (config, logbook) = ctx.open_logbook()?;
    let lock = ctx.lock()?;
    let built = index::derive(ctx, &config, &logbook)?;

    // the index lists at most 50 closed cases; read older ones from file
    let titles: RefCell<HashMap<String, String>> = RefCell::new(
        built
            .index
            .cases
            .all()
            .map(|c| (c.id.clone(), c.title.clone()))
            .collect(),
    );
    let title = |id: &str| {
        if let Some(t) = titles.borrow().get(id) {
            return Some(t.clone());
        }
        let t = cases::find(&logbook, id).ok()?.case.title;
        titles.borrow_mut().insert(id.to_string(), t.clone());
        Some(t)
    };

    let dossier = rebuild::read_dossier(&logbook.root);
    let since = logbook.meta.created.to_string();
    let doc = rebuild::collect(&built, &dossier, since.get(..10).unwrap_or(&since), title);
    let counts = doc.counts();
    let (content, skipped) = rebuild::render::text(&doc, logbook.meta.language);

    let path = logbook.path(REL_PATH);
    let existing = match std::fs::read_to_string(&path) {
        Ok(t) => Some(t),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => {
            return Err(anyhow::Error::new(e)
                .context(format!("cannot read {}", path.display()))
                .into());
        }
    };
    let text = rebuild::merge(existing.as_deref(), &content);
    let changed = existing.as_deref() != Some(text.as_str());
    if changed {
        sys::write_generated(&path, text.as_bytes())?;
    }
    let commit = if changed {
        autocommit(ctx, &config, &logbook, "rebuild")
    } else {
        Commit::Skipped("nothing changed")
    };
    drop(lock);

    let files: Vec<&str> = if changed { vec![REL_PATH] } else { Vec::new() };
    let mut human = format!(
        "{} {REL_PATH}: {} package(s), {} deviation(s), {} plugin(s), {} unit(s), {} open",
        if changed { "Wrote" } else { "Unchanged:" },
        counts.packages,
        counts.deviations,
        counts.plugins,
        counts.units,
        counts.open
    );
    human.push_str(&commit.human());
    let warnings: Vec<String> = built.warnings.iter().cloned().chain(skipped).collect();
    human.push_str(&warnings_human(&warnings));
    Ok(Output::ok(
        human,
        json!({
            "path": path,
            "sections": counts,
            "files": files,
            "git": commit.json(),
            "warnings": warnings,
        }),
    ))
}
