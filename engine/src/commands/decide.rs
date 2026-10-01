//! `seldon decide "<title>" [--case ID] [--no-edit]` (SPEC-ENGINE §3):
//! a new `decisions/ADR-NNNN-slug.md` with status `proposed`, opened in the
//! editor unless `--no-edit`.

use clap::Args;
use serde_json::json;

use super::event::parse_case_id;
use super::open::{edit, editor_json};
use super::{Context, Output, autocommit, one_line, write_new};
use crate::error::Result;
use crate::logbook::{Logbook, cases};
use crate::model::{self, Decision, DecisionStatus};

#[derive(Debug, Clone, Args)]
pub struct DecideArgs {
    /// The decision title, as one argument
    #[arg(value_name = "TITLE", allow_hyphen_values = true)]
    pub title: String,

    /// The case this decision belongs to
    #[arg(long = "case", value_name = "ID", value_parser = parse_case_id)]
    pub case_id: Option<String>,

    /// Do not open the editor
    #[arg(long)]
    pub no_edit: bool,
}

pub fn run(ctx: &Context, args: DecideArgs) -> Result<Output> {
    let title = one_line("the title", &args.title)?;
    let (config, logbook) = ctx.open_logbook()?;
    let lock = ctx.lock()?;
    if let Some(id) = &args.case_id {
        cases::find(&logbook, id)?;
    }
    let id = next_id(&logbook)?;
    let decision = Decision {
        id: id.clone(),
        title: title.clone(),
        status: DecisionStatus::Proposed,
        date: ctx.now.date_naive(),
        supersedes: None,
        cases: args.case_id.iter().cloned().collect(),
    };
    let template = cases::logbook_template(&logbook, "decision.md")?;
    let body = cases::fill(&template, &[("id", &id), ("title", &title)]);
    let rel = format!("decisions/{id}-{}.md", cases::slug(&title, "decision"));
    let path = logbook.path(&rel);
    write_new(&path, &model::render_new(&decision, &body))?;
    let commit = autocommit(ctx, &config, &logbook, &format!("{id} proposed"));
    crate::index::rebuild_if_initialised(ctx);
    drop(lock);

    // after the lock is released: the editor may stay open for a long time
    let editor = (!args.no_edit).then(|| edit(&path));
    let mut human = format!("Created {id} \"{title}\" in {rel}");
    if let Some(Err(e)) = &editor {
        human.push_str(&format!("\nEditor: {e}"));
    }
    human.push_str(&commit.human());
    Ok(Output::ok(
        human,
        json!({
            "decision": {
                "id": id,
                "title": title,
                "status": decision.status,
                "date": decision.date.to_string(),
                "cases": decision.cases,
                "path": rel,
            },
            "editor": editor.as_ref().map(editor_json),
            "git": commit.json(),
        }),
    ))
}

/// The next `ADR-NNNN` after every file in `decisions/`.
fn next_id(logbook: &Logbook) -> Result<String> {
    let max = logbook
        .decision_files()?
        .iter()
        .filter_map(|p| {
            let name = p.file_name()?.to_string_lossy().into_owned();
            let digits: String = name
                .strip_prefix("ADR-")?
                .chars()
                .take_while(char::is_ascii_digit)
                .collect();
            digits.parse::<u32>().ok()
        })
        .max()
        .unwrap_or(0);
    Ok(format!("ADR-{:04}", max + 1))
}
