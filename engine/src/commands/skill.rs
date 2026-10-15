//! `seldon skill [install]` (WP-229, ADR-0058): the Hype-shaped front of
//! the Seldon agent skill. `seldon skill` prints the skill's `SKILL.md`
//! as this engine ships it; `seldon skill install` is `seldon hook install
//! skills` ([`super::skills::install`]): every agent skill folder that
//! exists, idempotent, files changed by hand kept; `--replace` archives
//! such a copy and installs the skill as shipped, the same code as `hook
//! install skills --replace`. Uninstalling stays `hook uninstall skills`.

use clap::{Args, Subcommand};
use serde_json::json;

use super::skills::{self, FILES, SKILL_NAME};
use super::{Context, Output};
use crate::error::Result;

#[derive(Debug, Clone, Args)]
#[command(after_help = "Examples:
  seldon skill
  seldon skill install --json
  seldon skill install --replace")]
pub struct SkillArgs {
    #[command(subcommand)]
    pub command: Option<SkillCommand>,
}

#[derive(Debug, Clone, Subcommand)]
pub enum SkillCommand {
    /// Put the skill into every agent skill folder that exists (as `hook install skills`)
    Install {
        /// Where you changed the skill, archive your copy to the logbook's
        /// archive/ and install it as shipped
        #[arg(long)]
        replace: bool,
    },
}

pub fn run(ctx: &Context, args: SkillArgs) -> Result<Output> {
    match args.command {
        Some(SkillCommand::Install { replace }) => skills::install(ctx, replace),
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
