//! `seldon index [--check]` (SPEC-ENGINE §3, §6): rebuilds
//! `index.json` and the `ledger/YYYY-MM.md` views from the logbook.
//!
//! The index is written atomically (CONTRACT.md rule 2). Before `seldon
//! init` it writes the `notInitialised` index (so the plugin shows its
//! banner) and exits 3. `--check` validates the new index against
//! `schema/index.schema.json` first and refuses to write an invalid one
//! (exit 2: the engine produced it). A case id in two files (WP-057) is
//! the user's to fix: `--check` refuses with exit 1, a plain rebuild
//! warns ([`duplicate_cases`]).

use std::path::PathBuf;

use clap::Args;
use serde_json::{Value, json};

use super::{Context, Output};
use crate::config::Config;
use crate::error::{Error, Result};
use crate::index::{self, Built, check, views};
use crate::logbook::Logbook;

#[derive(Debug, Clone, Args)]
pub struct IndexArgs {
    /// Validate the index against schema/index.schema.json before writing it
    #[arg(long)]
    pub check: bool,
}

/// What a rebuild did (shared with `seldon status`).
pub(crate) struct Rebuilt {
    pub logbook: Logbook,
    pub built: Built,
    pub index_path: PathBuf,
    /// Logbook files written, relative.
    pub files: Vec<String>,
    /// `None` without `--check`.
    pub valid: Option<bool>,
}

pub fn run(ctx: &Context, args: IndexArgs) -> Result<Output> {
    let (r, _) = rebuild_with(ctx, args.check, false, |_, _, _| None::<()>)?;
    let mut human = format!(
        "Index: {} event(s), {} open drift ({} crisis), {} active case(s) → {}",
        r.built.index.events.len(),
        r.built.index.summary.open_drift,
        r.built.index.summary.crisis,
        r.built.index.summary.active_cases,
        ctx.dirs.display(&r.index_path)
    );
    if r.valid == Some(true) {
        human.push_str("\nValid against schema/index.schema.json.");
    }
    if !r.files.is_empty() {
        human.push_str(&format!("\nWrote {}", r.files.join(", ")));
    }
    human.push_str(&warnings_human(&r.built.warnings));
    Ok(Output::ok(human, rebuilt_json(&r)))
}

/// Loads, derives, writes the views (and `STATUS.md` and the
/// `decisions.index` fence of `DECISIONS.md` when `status`),
/// optionally validates, and writes the index; under the state lock.
/// `before_index` gets the logbook files written and runs before the git
/// state is read (the autocommit of `seldon status`).
pub(crate) fn rebuild_with<T>(
    ctx: &Context,
    validate: bool,
    status: bool,
    before_index: impl FnOnce(&Config, &Logbook, &[String]) -> Option<T>,
) -> Result<(Rebuilt, Option<T>)> {
    let config = ctx.load_config()?.unwrap_or_default();
    let (root, _) = ctx.resolve_logbook(None, Some(&config));
    let index_path = ctx.dirs.index_file();
    let lock = ctx.lock()?;
    if !Logbook::is_initialised(&root) {
        index::write(&index_path, &index::not_initialised(&root, ctx.now))?;
        return Err(Error::NotInitialised(root));
    }
    let logbook = Logbook::open(&root)?;
    let mut built = index::derive(ctx, &config, &logbook)?;
    let duplicates = duplicate_cases(
        built
            .index
            .cases
            .all()
            .map(|c| (c.id.as_str(), c.path.as_str())),
    );
    let mut files = views::write_ledger_views(&logbook, &built)?;
    if status {
        match views::write_status(&logbook, &built)? {
            views::Fill::Written => files.push("STATUS.md".into()),
            views::Fill::Unchanged => {}
            views::Fill::Skipped(w) => built.warnings.push(w),
        }
        match views::write_decisions_index(&logbook, &built.index.decisions)? {
            views::Fill::Written => files.push("DECISIONS.md".into()),
            views::Fill::Unchanged => {}
            views::Fill::Skipped(w) => built.warnings.push(w),
        }
    }
    let extra = before_index(&config, &logbook, &files);
    built.index.logbook.git = index::git_info(&logbook.root);

    let valid = if validate {
        let instance = serde_json::to_value(&built.index).map_err(anyhow::Error::from)?;
        let errors = check::Validator::new().validate(&instance, "index.schema.json");
        if !errors.is_empty() {
            return Err(anyhow::anyhow!(
                "the new index does not validate against schema/index.schema.json; {} not written:\n  {}",
                ctx.dirs.display(&index_path),
                errors.join("\n  ")
            )
            .into());
        }
        if !duplicates.is_empty() {
            return Err(Error::user(format!(
                "{}; {} not written",
                duplicates.join("; "),
                ctx.dirs.display(&index_path)
            )));
        }
        Some(true)
    } else {
        built.warnings.extend(duplicates);
        None
    };
    index::write(&index_path, &built.index)?;
    drop(lock);
    Ok((
        Rebuilt {
            logbook,
            built,
            index_path,
            files,
            valid,
        },
        extra,
    ))
}

pub(crate) fn rebuilt_json(r: &Rebuilt) -> Value {
    let ix = &r.built.index;
    let mut out = json!({
        "ok": true,
        "logbook": r.logbook.root,
        "index": r.index_path,
        "generatedAt": ix.generated_at,
        "events": ix.events.len(),
        "summary": ix.summary,
        "files": r.files,
        "warnings": r.built.warnings,
    });
    if let Some(valid) = r.valid {
        out["valid"] = json!(valid);
    }
    out
}

/// One line per case id that appears more than once in `cases` (`(id,
/// relative path)`), with the paths of its files and the fix: a stale
/// copy written back to its old folder (WP-057). Every command that finds
/// a case by id refuses such an id. Not a schema rule: `index --check`
/// exits 1 on it, `index` and `status` warn, `doctor` reports it from the
/// case files.
pub(crate) fn duplicate_cases<'a>(
    cases: impl IntoIterator<Item = (&'a str, &'a str)>,
) -> Vec<String> {
    let mut seen: Vec<(&str, Vec<&str>)> = Vec::new();
    for (id, path) in cases {
        match seen.iter_mut().find(|(i, _)| *i == id) {
            Some((_, paths)) => paths.push(path),
            None => seen.push((id, vec![path])),
        }
    }
    seen.into_iter()
        .filter(|(_, paths)| paths.len() > 1)
        .map(|(id, paths)| {
            format!(
                "case {id} exists more than once ({}); keep one file",
                paths.join(", ")
            )
        })
        .collect()
}

pub(crate) fn warnings_human(warnings: &[String]) -> String {
    warnings.iter().map(|w| format!("\nwarning: {w}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_case_id_twice_is_named_with_both_files() {
        let sample: Value =
            serde_json::from_str(include_str!("../../../fixtures/index.sample.json")).unwrap();
        let pairs = |ix: &Value| -> Vec<(String, String)> {
            ["queued", "active", "verification", "completed"]
                .iter()
                .flat_map(|g| ix["cases"][g].as_array().unwrap().clone())
                .map(|c| {
                    let s = |k: &str| c[k].as_str().unwrap().to_string();
                    (s("id"), s("path"))
                })
                .collect()
        };
        let run = |p: &[(String, String)]| {
            duplicate_cases(p.iter().map(|(i, p)| (i.as_str(), p.as_str())))
        };
        let mut cases = pairs(&sample);
        assert_eq!(run(&cases), Vec::<String>::new());
        let (id, path) = cases[0].clone();
        cases.push((id.clone(), format!("work/completed/{id}-copy.md")));
        assert_eq!(
            run(&cases),
            [format!(
                "case {id} exists more than once ({path}, work/completed/{id}-copy.md); keep one file"
            )]
        );
    }
}
