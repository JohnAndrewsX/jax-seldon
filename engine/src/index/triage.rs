//! `triage` (ADR-0034 §6, ADR-0035 §6): the newest triage proposal of the
//! indexed logbook, from `<state>/proposals/<id>.json`
//! (`schema/proposal.schema.json`). The engine stores proposals there
//! (`seldon drift propose`, WP-124); the index only points at one.

use std::path::Path;

use serde::Deserialize;
use serde_json::Value;

use super::check::Validator;
use super::model::{Triage, TriageCounts};
use crate::config::Dirs;
use crate::model::is_ulid;

/// The proposals' directory, next to `index.json`.
pub const DIR: &str = "proposals";

#[derive(Deserialize)]
struct Proposal {
    id: String,
    at: String,
    actor: String,
    logbook: String,
    applied: Option<String>,
    items: Vec<Item>,
}

#[derive(Deserialize)]
struct Item {
    crisis: bool,
}

/// The newest valid proposal of `root`, by id. A `.json` file not named
/// `<ULID>.json`, a file that fails its schema, whose name is not its id,
/// or that cannot be read is skipped with a warning; one of another
/// logbook is skipped silently (it is not this logbook's business).
/// `None` when there is none.
pub fn read(dirs: &Dirs, root: &Path, warnings: &mut Vec<String>) -> Option<Triage> {
    let dir = dirs.state_dir.join(DIR);
    let entries = std::fs::read_dir(&dir).ok()?;
    let mut ids: Vec<String> = Vec::new();
    let mut misnamed: Vec<String> = Vec::new();
    for entry in entries.filter_map(|e| e.ok()) {
        let name = entry.file_name().to_string_lossy().into_owned();
        // a `.json` file not named `<ULID>.json` is no proposal (WP-120
        // round 2, N2): say so; the engine's own temp files (`.name.tmp-…`)
        // and other files are not proposals and pass silently
        let Some(id) = name.strip_suffix(".json") else {
            continue;
        };
        if is_ulid(id) {
            ids.push(id.to_string());
        } else if !name.starts_with('.') {
            misnamed.push(name);
        }
    }
    misnamed.sort();
    for name in misnamed {
        warnings.push(format!(
            "{}: not named <ULID>.json, so not a proposal; the index skips it",
            dirs.display(&dir.join(&name)).escape_debug()
        ));
    }
    if ids.is_empty() {
        return None;
    }
    ids.sort_unstable_by(|a, b| b.cmp(a));
    let canonical = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    let validator = Validator::new();
    for id in ids {
        let path = dir.join(format!("{id}.json"));
        let shown = dirs.display(&path);
        let value: Value = match std::fs::read_to_string(&path)
            .map_err(|e| e.to_string())
            .and_then(|t| serde_json::from_str(&t).map_err(|e| e.to_string()))
        {
            Ok(v) => v,
            Err(e) => {
                warnings.push(format!("{shown}: not read ({e}); the index skips it"));
                continue;
            }
        };
        let errors = validator.validate(&value, "proposal.schema.json");
        if let Some(first) = errors.first() {
            warnings.push(format!(
                "{shown}: not a valid proposal ({first}); the index skips it"
            ));
            continue;
        }
        let Ok(p) = serde_json::from_value::<Proposal>(value) else {
            continue;
        };
        if p.id != id {
            warnings.push(format!(
                "{shown}: its id is {}; the index skips it",
                p.id.escape_debug()
            ));
            continue;
        }
        if Path::new(&p.logbook) != canonical {
            continue;
        }
        return Some(Triage {
            counts: TriageCounts {
                items: p.items.len(),
                crises: p.items.iter().filter(|i| i.crisis).count(),
            },
            path: format!("{DIR}/{id}.json"),
            id: p.id,
            at: p.at,
            actor: p.actor,
            applied: p.applied,
        });
    }
    None
}
