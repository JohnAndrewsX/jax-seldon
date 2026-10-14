//! `seldon skill [install]` (WP-229, ADR-0058): the Hype-shaped front of
//! the Seldon agent skill. `seldon skill` prints the skill's `SKILL.md`
//! as this engine ships it; `seldon skill install` is `seldon hook install
//! skills` ([`super::skills::install`]): every agent skill folder that
//! exists, idempotent, files changed by hand kept. Replacing an edited
//! copy and uninstalling stay `hook install skills --replace` and `hook
//! uninstall skills`.

use clap::{Args, Subcommand};
use serde_json::json;

use super::skills::{self, FILES, SKILL_NAME};
use super::{Context, Output};
use crate::error::Result;

#[derive(Debug, Clone, Args)]
#[command(after_help = "Examples:
  seldon skill
  seldon skill install --json")]
pub struct SkillArgs {
    #[command(subcommand)]
    pub command: Option<SkillCommand>,
}

#[derive(Debug, Clone, Subcommand)]
pub enum SkillCommand {
    /// Put the skill into every agent skill folder that exists (as `hook install skills`)
    Install,
}

pub fn run(ctx: &Context, args: SkillArgs) -> Result<Output> {
    match args.command {
        Some(SkillCommand::Install) => skills::install(ctx, false),
        None => Ok(print()),
    }
}

/// The skill's `SKILL.md`; `--json` adds the names of its topic files.
fn print() -> Output {
    let (name, text) = FILES[0];
    Output::ok(
        text.trim_end().to_string(),
        json!({
            "skill": SKILL_NAME,
            "file": name,
            "text": text,
            "files": FILES.iter().map(|(n, _)| *n).collect::<Vec<_>>(),
        }),
    )
}
