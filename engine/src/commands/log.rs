//! `seldon log "<text>" [--case ID] [--actor human|agent:NAME] [--tag T]…`
//! (SPEC-ENGINE §3): a `manual/note` event plus a journal entry
//! `## HH:MM · actor · case?` with the text.

use clap::Args;
use serde_json::{Value, json};

use super::event::{clip, emit_one, event_json, parse_case_id, parse_person};
use super::{Context, Output, autocommit, required_text};
use crate::error::Result;
use crate::logbook::{cases, journal};
use crate::model::event::{Event, Kind, Meta, Source};

/// Subject of a note without a case (`event.schema.json`).
pub const JOURNAL_SUBJECT: &str = "journal";

#[derive(Debug, Clone, Args)]
#[command(after_help = "Examples:
  seldon log -- \"Switched the terminal font to Iosevka\"
  seldon log --case C-2026-004 --tag fonts -- \"Tried two fonts, kept the first\"")]
pub struct LogArgs {
    /// The note, as one argument; after `--` when it starts with `-`
    #[arg(value_name = "TEXT", allow_hyphen_values = true)]
    pub text: String,

    /// The case the note belongs to
    #[arg(long = "case", value_name = "ID", value_parser = parse_case_id)]
    pub case_id: Option<String>,

    /// Who writes the note: human or agent:NAME
    #[arg(long, value_name = "ACTOR", default_value = "human", value_parser = parse_person)]
    pub actor: String,

    /// Tag the note (repeatable): `#tag` in the journal, `meta.tags` in the ledger
    #[arg(long = "tag", value_name = "TAG", value_parser = parse_tag)]
    pub tags: Vec<String>,
}

/// An Obsidian-style tag: letters, digits, `_`, `-`, `/`; not only digits.
fn parse_tag(s: &str) -> Result<String, String> {
    let s = s.strip_prefix('#').unwrap_or(s);
    let ok = !s.is_empty()
        && !s.chars().all(|c| c.is_ascii_digit())
        && s.chars().all(|c| c.is_alphanumeric() || "_-/".contains(c));
    if ok {
        Ok(s.to_string())
    } else {
        Err(format!("`{s}` is not a tag (letters, digits, _ - /)"))
    }
}

pub fn run(ctx: &Context, args: LogArgs) -> Result<Output> {
    let text = required_text("the note", &args.text)?;
    let (config, logbook) = ctx.open_logbook()?;
    let lock = ctx.lock()?;
    let mut case_file = args
        .case_id
        .as_deref()
        .map(|id| cases::find(&logbook, id))
        .transpose()?;

    let mut meta = Meta::default();
    if !args.tags.is_empty() {
        meta.extra
            .insert("tags".into(), Value::String(args.tags.join(",")));
    }
    let event = Event::new(
        ctx.now,
        Source::Manual,
        Kind::Note,
        args.case_id.as_deref().unwrap_or(JOURNAL_SUBJECT),
    )
    .detail(text.clone())
    .actor(&args.actor)
    .case(args.case_id.clone())
    .meta(meta);
    // the ledger first: it assigns the id the case file records
    let event = emit_one(&lock, &config, &logbook, event)?;

    let mut entry = text.clone();
    if !args.tags.is_empty() {
        let tags: Vec<String> = args.tags.iter().map(|t| format!("#{t}")).collect();
        entry.push('\n');
        entry.push_str(&tags.join(" "));
    }
    let day = journal::append(
        &logbook,
        &ctx.now,
        &args.actor,
        args.case_id.as_deref(),
        &entry,
    )?;
    if let Some(file) = case_file.as_mut() {
        file.attach(&event.id.to_string(), &event.actor);
        file.save(&logbook)?;
    }
    let summary = match &args.case_id {
        Some(id) => format!("note {id}"),
        None => "note".to_string(),
    };
    let commit = autocommit(ctx, &config, &logbook, &summary);
    crate::index::rebuild_if_initialised(ctx);
    drop(lock);

    let mut human = format!(
        "Noted in {}{}: {}",
        day.path,
        args.case_id
            .as_deref()
            .map(|c| format!(" ({c})"))
            .unwrap_or_default(),
        clip(text.lines().next().unwrap_or(""), 60)
    );
    human.push_str(&commit.human());
    Ok(Output::ok(
        human,
        json!({
            "event": event_json(&event),
            "ledger": format!("ledger/{}.jsonl", event.month()),
            "journal": { "path": day.path, "created": day.created },
            "case": args.case_id,
            "git": commit.json(),
        }),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tags() {
        assert_eq!(parse_tag("#zed").unwrap(), "zed");
        assert_eq!(parse_tag("dev/editor").unwrap(), "dev/editor");
        assert!(parse_tag("two words").is_err());
        assert!(parse_tag("2026").is_err());
        assert!(parse_tag("").is_err());
    }
}
