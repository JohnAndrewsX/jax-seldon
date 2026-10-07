//! `seldon decide "<title>" [--case ID] [--no-edit]` (SPEC-ENGINE §3):
//! a new `decisions/ADR-NNNN-slug.md` with status `proposed`, opened in the
//! editor unless `--no-edit`; the `decisions.index` table of `DECISIONS.md`
//! is filled in the same commit (WP-050).
//!
//! `seldon decide accept <ADR-NNNN>` (WP-135, ADR-0040): the user accepts a
//! proposed decision — `status: accepted` and the day in its frontmatter, a
//! `seldon` note in the ledger. An agent proposes; it never accepts.

use clap::{Args, Subcommand};
use serde_json::json;

use super::event::{
    ACTOR_ENV, actor_or_env, emit_one, env_actor, event_json, parse_case_id, parse_person,
};
use super::index::warnings_human;
use super::open::{edit, editor_json};
use super::{Context, Output, autocommit, one_line, write_new};
use crate::error::{Error, Result};
use crate::index::{build, load, views};
use crate::logbook::{Logbook, cases};
use crate::model::event::{ACTOR_HUMAN, Event, Kind, Source};
use crate::model::{self, Decision, DecisionStatus, is_agent, is_decision_id};
use crate::redact::Redactor;

#[derive(Debug, Clone, Args)]
#[command(args_conflicts_with_subcommands = true, subcommand_negates_reqs = true)]
pub struct DecideArgs {
    #[command(subcommand)]
    pub command: Option<DecideCommand>,

    /// The decision title, as one argument (a title `accept` goes after `--`)
    #[arg(value_name = "TITLE", allow_hyphen_values = true, required = true)]
    pub title: Option<String>,

    /// The case this decision belongs to
    #[arg(long = "case", value_name = "ID", value_parser = parse_case_id)]
    pub case_id: Option<String>,

    /// Do not open the editor
    #[arg(long)]
    pub no_edit: bool,
}

#[derive(Debug, Clone, Subcommand)]
pub enum DecideCommand {
    /// Accept a proposed decision: status accepted, today's date, a ledger
    /// note. The user's act; an agent is refused
    Accept(AcceptArgs),
}

#[derive(Debug, Clone, Args)]
pub struct AcceptArgs {
    /// The decision, ADR-NNNN
    #[arg(value_name = "ID", value_parser = parse_decision_id)]
    pub id: String,

    /// Who accepts it: human (the default); an agent is refused
    #[arg(long, value_name = "ACTOR", value_parser = parse_person)]
    pub actor: Option<String>,
}

/// clap value parser: `ADR-NNNN`.
fn parse_decision_id(s: &str) -> std::result::Result<String, String> {
    if is_decision_id(s) {
        Ok(s.to_string())
    } else {
        Err(format!("`{s}` is not a decision id (ADR-NNNN)"))
    }
}

pub fn run(ctx: &Context, args: DecideArgs) -> Result<Output> {
    match args.command {
        Some(DecideCommand::Accept(a)) => accept(ctx, a),
        None => new(ctx, args),
    }
}

fn new(ctx: &Context, args: DecideArgs) -> Result<Output> {
    let title = one_line("the title", args.title.as_deref().unwrap_or_default())?;
    let (config, logbook) = ctx.open_logbook()?;
    // the decision file, its name and DECISIONS.md get the redacted title
    let title = Redactor::for_config(&config)?.redact(&title);
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
    let warnings = fill_index(&logbook);
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
    human.push_str(&warnings_human(&warnings));
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
            "warnings": warnings,
        }),
    ))
}

/// Accepting a decision is the user's act (ADR-0040; the B2 pattern of
/// WP-102 and WP-124): an agent actor (`--actor`, or `$SELDON_ACTOR`
/// without it) is refused, and so is `--actor human` in an agent's session
/// (an agent's act is never recorded as human, ADR-0027 §5). Checked
/// before anything is read.
fn user_actor(flag: Option<String>, id: &str) -> Result<String> {
    let session = env_actor(parse_person)
        .ok()
        .flatten()
        .filter(|s| is_agent(s));
    let actor = actor_or_env(flag, parse_person, ACTOR_HUMAN)?;
    if is_agent(&actor) {
        return Err(Error::user(format!(
            "{id} is not accepted: {actor} may propose a decision (`seldon decide`), only the \
             user accepts one (ADR-0040); ask them to accept it in the desk or in their own terminal"
        )));
    }
    if let Some(agent) = session {
        return Err(Error::user(format!(
            "{id} is not accepted: `--actor {actor}` in a session of {agent} ({ACTOR_ENV}); an \
             agent's act is never recorded as human (ADR-0027 §5). Accept it from a session of \
             your own (the desk's Accept)"
        )));
    }
    Ok(actor)
}

/// `seldon decide accept <ADR-NNNN> [--actor A]`: a proposed decision
/// becomes accepted with today's date; one `seldon` note (subject the id)
/// records it. Accepted already: nothing is written (exit 0, `already`).
/// Superseded: refused. The ledger first, as a plan step: when it cannot be
/// written, the file stays proposed.
fn accept(ctx: &Context, args: AcceptArgs) -> Result<Output> {
    let id = args.id;
    let actor = user_actor(args.actor, &id)?;
    let (config, logbook) = ctx.open_logbook()?;
    let lock = ctx.lock()?;
    let path = logbook
        .decision_file(&id)?
        .ok_or_else(|| Error::user(format!("unknown decision {id}")))?;
    let rel = cases::relative(&logbook, &path);
    let (mut decision, mut doc) =
        model::load::<Decision>(&path).map_err(|e| Error::user(format!("{e:#}")))?;
    if decision.id != id {
        return Err(Error::user(format!(
            "{rel} names {} in its frontmatter, not {id}; fix the file first",
            decision.id
        )));
    }
    match decision.status {
        DecisionStatus::Proposed => {}
        DecisionStatus::Accepted => {
            drop(lock);
            return Ok(accept_output(&rel, &decision, true, None, None, Vec::new()));
        }
        DecisionStatus::Superseded => {
            return Err(Error::user(format!(
                "{id} is superseded; only a proposed decision is accepted"
            )));
        }
    }
    decision.status = DecisionStatus::Accepted;
    decision.date = ctx.now.date_naive();
    // read back before anything is written (WP-066): a frontmatter the
    // update cannot carry fails here, the ledger untouched
    model::update(&mut doc, &decision)
        .map_err(|e| Error::user(format!("{rel}: the status is not changed: {e}")))?;
    let event = Event::new(ctx.now, Source::Seldon, Kind::Note, &id)
        .actor(&actor)
        .detail(format!("accepted: {}", decision.title));
    let event = emit_one(&lock, &config, &logbook, event)?;
    crate::sys::write_atomic(&path, doc.render().as_bytes())?;
    let warnings = fill_index(&logbook);
    let commit = autocommit(ctx, &config, &logbook, &format!("{id} accepted"));
    crate::index::rebuild_if_initialised(ctx);
    drop(lock);
    Ok(accept_output(
        &rel,
        &decision,
        false,
        Some(&event),
        Some(&commit),
        warnings,
    ))
}

fn accept_output(
    rel: &str,
    decision: &Decision,
    already: bool,
    event: Option<&Event>,
    commit: Option<&super::Commit>,
    warnings: Vec<String>,
) -> Output {
    let id = &decision.id;
    let mut human = if already {
        format!("{id} is accepted already; nothing changed")
    } else {
        format!("Accepted {id} \"{}\" in {rel}", decision.title)
    };
    if let Some(c) = commit {
        human.push_str(&c.human());
    }
    human.push_str(&warnings_human(&warnings));
    Output::ok(
        human,
        json!({
            "decision": {
                "id": id,
                "title": decision.title,
                "status": decision.status,
                "date": decision.date.to_string(),
                "cases": decision.cases,
                "path": rel,
            },
            "already": already,
            "event": event.map(event_json),
            "git": commit.map_or_else(
                || json!({ "committed": false, "reason": "nothing changed" }),
                super::Commit::json,
            ),
            "warnings": warnings,
        }),
    )
}

/// Fills the `decisions.index` fence of `DECISIONS.md` (WP-050), in the
/// same commit as the new file. The decision is written already, so a
/// failure here is a warning, not an error; an invalid decision file is
/// skipped and named in the warnings.
fn fill_index(logbook: &Logbook) -> Vec<String> {
    let mut warnings = Vec::new();
    let rows = match load::decisions(logbook, &mut warnings) {
        Ok(d) => build::decision_rows(d, |_| None),
        Err(e) => {
            warnings.push(format!("DECISIONS.md not updated: {e:#}"));
            return warnings;
        }
    };
    match views::write_decisions_index(logbook, &rows) {
        Ok(views::Fill::Written | views::Fill::Unchanged) => {}
        Ok(views::Fill::Skipped(w)) => warnings.push(w),
        Err(e) => warnings.push(format!("DECISIONS.md not updated: {e:#}")),
    }
    warnings
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
