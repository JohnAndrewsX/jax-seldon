//! `seldon rules update [--replace]` (SPEC-ENGINE §3, ADR-0027, WP-100):
//! brings the rules block of the logbook's `AGENTS.md` up to this engine's
//! rules ([`crate::logbook::rules`]). A fenced file gets the block
//! rewritten and nothing else; an unfenced one the block on top and its
//! old text below `## Your rules (kept)`, byte for byte (a file a release
//! wrote and nobody changed is replaced whole); `--replace` archives the
//! file to `archive/AGENTS-<date>.md` and writes the template. The diff is
//! printed, the change committed as `seldon: rules update`. A second run
//! changes nothing. It never runs on its own: `init` writes the template
//! once, `doctor` names this command when the block is not current.

use std::io::Write as _;

use clap::{Args, Subcommand};
use serde_json::json;

use super::{Context, Output, autocommit};
use crate::error::{Error, Result};
use crate::logbook::Logbook;
use crate::logbook::rules::{self, Action, FILE, KEPT_HEADING, VERSION};
use crate::logbook::templates::{self, Vars};
use crate::sys;

#[derive(Debug, Clone, Args)]
pub struct RulesArgs {
    #[command(subcommand)]
    pub command: RulesCommand,
}

#[derive(Debug, Clone, Subcommand)]
pub enum RulesCommand {
    /// Bring the rules block of AGENTS.md up to this release; text outside it is kept
    #[command(after_help = "Examples:
  seldon rules update
  seldon rules update --replace")]
    Update {
        /// Archive the whole file to archive/AGENTS-<date>.md and write the template
        #[arg(long)]
        replace: bool,
    },
}

pub fn run(ctx: &Context, args: RulesArgs) -> Result<Output> {
    match args.command {
        RulesCommand::Update { replace } => update(ctx, replace),
    }
}

/// The rendered `AGENTS.md` template in the logbook's language.
pub fn template(logbook: &Logbook, today: chrono::NaiveDate) -> String {
    let vars = Vars {
        machine_id: &logbook.meta.machine_id,
        language: logbook.meta.language,
        date: today,
    };
    let t = templates::find(FILE).expect("a built-in AGENTS.md template");
    templates::render(t.text(logbook.meta.language), &vars)
}

/// The rules file of `logbook`: its bytes, `None` when there is none.
pub fn read(logbook: &Logbook) -> Result<Option<Vec<u8>>> {
    let path = logbook.path(FILE);
    match std::fs::read(&path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(anyhow::Error::new(e)
            .context(format!("cannot read {}", path.display()))
            .into()),
    }
}

fn update(ctx: &Context, replace: bool) -> Result<Output> {
    let (config, logbook) = ctx.open_logbook()?;
    let lock = ctx.lock()?;
    let path = logbook.path(FILE);
    let template = template(&logbook, ctx.now.date_naive());
    let bytes = read(&logbook)?;
    let old = match &bytes {
        None => None,
        Some(b) => match std::str::from_utf8(b) {
            Ok(text) => Some(text),
            // the bytes are archived as they are
            Err(_) if replace => Some(""),
            Err(_) => {
                return Err(Error::user(format!(
                    "{FILE} is not UTF-8 text; the file is left as it is. `seldon rules update --replace` archives it and writes the template"
                )));
            }
        },
    };
    let plan = rules::update(old, &template, replace).map_err(Error::user)?;
    let from = plan.from.map(|v| format!("v{v}"));
    if plan.action == Action::Unchanged {
        drop(lock);
        return Ok(Output::ok(
            format!("{FILE}: the rules are current (v{VERSION}); nothing changed."),
            json!({
                "file": path,
                "action": plan.action.as_str(),
                "from": from,
                "version": VERSION,
                "archived": null,
                "diff": "",
                "git": { "committed": false, "reason": "nothing changed" },
            }),
        ));
    }

    let archived = match &bytes {
        Some(b) if plan.archive => Some(archive(&logbook, ctx.now.date_naive(), b)?),
        _ => None,
    };
    sys::write_atomic(&path, plan.text.as_bytes())?;
    let commit = autocommit(ctx, &config, &logbook, "rules update");
    crate::index::rebuild_if_initialised(ctx);
    drop(lock);

    let diff = rules::diff(old.unwrap_or(""), &plan.text, FILE);
    let unfenced = old.is_some_and(|t| rules::find(t) == rules::Block::Unfenced);
    let mut human = match plan.action {
        Action::Created => format!("{FILE} was missing; wrote the rules (v{VERSION})."),
        Action::Rewritten if unfenced && archived.is_none() => format!(
            "{FILE} held Seldon's older rules and nothing else; replaced them with the rules (v{VERSION})."
        ),
        Action::Rewritten if unfenced => format!(
            "{FILE} held Seldon's older rules and no line of your own; wrote the rules (v{VERSION})."
        ),
        Action::Rewritten => format!(
            "{FILE}: rewrote the rules block ({} → v{VERSION}); the text outside it is unchanged.",
            from.as_deref().unwrap_or("?")
        ),
        Action::Kept => format!(
            "{FILE}: wrote the rules (v{VERSION}); the lines of your own are kept below them, under \"{KEPT_HEADING}\"."
        ),
        Action::Replaced => format!("{FILE}: wrote the rules (v{VERSION})."),
        Action::Unchanged => unreachable!("returned above"),
    };
    if let Some(rel) = &archived {
        human.push_str(&format!(" The old file is archived as {rel}."));
    }
    if !diff.is_empty() {
        human.push('\n');
        human.push_str(diff.trim_end());
    }
    human.push_str(&commit.human());
    Ok(Output::ok(
        human,
        json!({
            "file": path,
            "action": plan.action.as_str(),
            "from": from,
            "version": VERSION,
            "archived": archived,
            "diff": diff,
            "git": commit.json(),
        }),
    ))
}

/// Writes `bytes` to `archive/AGENTS-<date>.md`, or `-2`, `-3`, … when that
/// name is taken; never overwrites. Returns the path relative to the root.
fn archive(logbook: &Logbook, today: chrono::NaiveDate, bytes: &[u8]) -> Result<String> {
    let dir = logbook.checked_dir("archive")?;
    sys::create_dir_private(&dir)
        .map_err(|e| anyhow::Error::new(e).context(format!("cannot create {}", dir.display())))?;
    let mut n = 1u32;
    loop {
        let rel = match n {
            1 => format!("archive/AGENTS-{today}.md"),
            n => format!("archive/AGENTS-{today}-{n}.md"),
        };
        let path = logbook.path(&rel);
        let written = sys::create_new_private(&path).and_then(|mut f| f.write_all(bytes));
        match written {
            Ok(()) => return Ok(rel),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => n += 1,
            Err(e) => {
                return Err(anyhow::Error::new(e)
                    .context(format!("cannot write {}", path.display()))
                    .into());
            }
        }
    }
}
