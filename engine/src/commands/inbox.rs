//! `seldon inbox add --title T --file F|- [--tag T]… [--actor A]`
//! (SPEC-ENGINE §3, WP-166, E28 step 1): files a text — an agent's crash
//! analysis, a finding — into the logbook's `inbox/`, so the engine stays
//! the only writer of the logbook (AGENTS.md §3).
//!
//! The text is untrusted and treated as `import task` treats a task file
//! (WP-102b, WP-140): CRLF as LF, direction and format characters dropped,
//! then the whole text through the logbook's redaction keeping its lines
//! and `/home/<user>` → `~` ([`Scrubber::text`]). The title gets the same.
//!
//! The file is `inbox/<date>-<slug>.md`, created, never overwritten. A
//! text whose `# title` and body an inbox file already holds is "already
//! filed": nothing is written, so a retry never files twice. Another text
//! under a title whose name is taken gets `-2`, `-3`, … One commit of the
//! new file alone; no ledger event (the `crash` kind is E28 step 2).

use std::io::{IsTerminal as _, Read as _, Write as _};
use std::path::{Path, PathBuf};

use anyhow::Context as _;
use clap::{Args, Subcommand};
use serde_json::json;

use super::event::{actor_or_env, clip, parse_person};
use super::log::parse_tag;
use super::{Commit, Context, Output, autocommit_paths, one_line};
use crate::error::{Error, Result};
use crate::frontmatter::{Document, FmValue, Frontmatter};
use crate::import::{Scrubber, is_direction_or_format, trim_blank_lines};
use crate::logbook::{Logbook, cases};
use crate::model::event::{ACTOR_HUMAN, format_ts};
use crate::redact::Redactor;
use crate::sys;

/// The logbook folder the command writes into.
pub const INBOX: &str = "inbox";

/// The largest text read, in bytes.
pub const MAX_TEXT_BYTES: u64 = 1024 * 1024;

/// The longest title, in characters (after the redaction).
pub const MAX_TITLE_CHARS: usize = 120;

/// The highest number a taken file name gets (`-2` … `-99`).
const MAX_SUFFIX: u32 = 99;

/// The file name stem of a title without letters or digits.
const FALLBACK_SLUG: &str = "note";

#[derive(Debug, Clone, Args)]
pub struct InboxArgs {
    #[command(subcommand)]
    pub action: InboxAction,
}

#[derive(Debug, Clone, Subcommand)]
pub enum InboxAction {
    /// File a text into the logbook's inbox/, redacted; the same text
    /// again changes nothing
    #[command(after_help = "Examples:
  seldon inbox add --title \"Crash: waybar (SIGSEGV)\" --tag crash --file report.md
  printf '%s\\n' \"Zed ignores the theme\" | seldon inbox add --title \"Zed theme\" --file -")]
    Add(AddArgs),
}

#[derive(Debug, Clone, Args)]
pub struct AddArgs {
    /// The title: one line, the file's `# heading` and name
    #[arg(long, value_name = "TITLE", allow_hyphen_values = true)]
    pub title: String,

    /// The text: a Markdown file, or `-` for stdin (at most 1 MiB, UTF-8)
    #[arg(long, value_name = "FILE")]
    pub file: PathBuf,

    /// Tag the text (repeatable): `tags` in the file's frontmatter
    #[arg(long = "tag", value_name = "TAG", value_parser = parse_tag)]
    pub tags: Vec<String>,

    /// Who files it: human or agent:NAME (default: $SELDON_ACTOR, else human)
    #[arg(long, value_name = "ACTOR", value_parser = parse_person)]
    pub actor: Option<String>,
}

pub fn run(ctx: &Context, args: InboxArgs) -> Result<Output> {
    match args.action {
        InboxAction::Add(a) => add(ctx, a),
    }
}

/// `text` without direction and format characters, and how many there were.
fn drop_format(text: &str) -> (String, usize) {
    let kept: String = text
        .chars()
        .filter(|c| !is_direction_or_format(*c))
        .collect();
    let dropped = text.chars().count() - kept.chars().count();
    (kept, dropped)
}

fn add(ctx: &Context, args: AddArgs) -> Result<Output> {
    let (title, title_dropped) = drop_format(&args.title);
    let title = one_line("the title", &title)?;
    let actor = actor_or_env(args.actor, parse_person, ACTOR_HUMAN)?;
    let (config, logbook) = ctx.open_logbook()?;
    let redactor = Redactor::for_config(&config)?;
    let mut scrubber = Scrubber::new(redactor.clone());

    let raw = read_text(&args.file)?;
    let (text, text_dropped) = drop_format(&raw.replace("\r\n", "\n"));
    let text = trim_blank_lines(&scrubber.text("text", &text));
    if text.is_empty() {
        return Err(Error::user("the text must not be empty"));
    }
    let title = scrubber.text("title", &title);
    if title.chars().count() > MAX_TITLE_CHARS {
        return Err(Error::user(format!(
            "the title is longer than {MAX_TITLE_CHARS} characters; put the rest in the text"
        )));
    }
    let redacted = changed_lines(&scrubber);
    let tags: Vec<String> = args.tags.iter().map(|t| redactor.redact(t)).collect();
    let body = format!("# {title}\n\n{text}");

    let lock = ctx.lock()?;
    let (path, filed) = match already_filed(&logbook, &body) {
        Some(path) => (path, false),
        None => {
            let file = render(ctx, &actor, &tags, &body);
            (create_free(ctx, &logbook, &title, &file)?, true)
        }
    };
    let commit = if filed {
        let commit = autocommit_paths(ctx, &config, &logbook, &[&path], "inbox add");
        crate::index::rebuild_if_initialised(ctx);
        commit
    } else {
        Commit::Skipped("nothing changed")
    };
    drop(lock);

    let mut human = if filed {
        format!("Filed {path}: {}", clip(&title, 60))
    } else {
        format!("Already filed as {path}: {}", clip(&title, 60))
    };
    if redacted > 0 {
        human.push_str(&format!(" ({redacted} line(s) redacted)"));
    }
    human.push_str(&commit.human());
    Ok(Output::ok(
        human,
        json!({
            "filed": filed,
            "path": path,
            "title": title,
            "actor": actor,
            "tags": tags,
            "redactedLines": redacted,
            "privatePaths": scrubber.private_paths,
            "droppedCharacters": title_dropped + text_dropped,
            "git": commit.json(),
        }),
    ))
}

/// The lines the scrubber's redaction changed so far, per source.
fn changed_lines(scrubber: &Scrubber) -> usize {
    let lines: std::collections::BTreeSet<(&str, usize)> = scrubber
        .hits
        .iter()
        .map(|h| (h.file.as_str(), h.line))
        .collect();
    lines.len()
}

/// The text of `--file`: stdin for `-` (never a terminal), else a regular
/// file (no symbolic link, FIFO or device) of at most [`MAX_TEXT_BYTES`],
/// UTF-8.
fn read_text(file: &Path) -> Result<String> {
    if file == Path::new("-") {
        let stdin = std::io::stdin();
        if stdin.is_terminal() {
            return Err(Error::user(
                "pipe the text on stdin (`--file -`), or name a file with --file",
            ));
        }
        let mut bytes = Vec::new();
        stdin
            .lock()
            .take(MAX_TEXT_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| Error::user(format!("cannot read stdin: {e}")))?;
        if bytes.len() as u64 > MAX_TEXT_BYTES {
            return Err(Error::user(format!(
                "the text is larger than {} KiB",
                MAX_TEXT_BYTES / 1024
            )));
        }
        return String::from_utf8(bytes).map_err(|_| Error::user("the text on stdin is not UTF-8"));
    }
    let shown = file.display();
    match sys::read_small_file(file, MAX_TEXT_BYTES) {
        Ok(Some(text)) => Ok(text),
        Ok(None) => Err(Error::user(format!("{shown}: no such file"))),
        Err(why) => Err(Error::user(format!(
            "{shown}: cannot file it: {why} (a regular file of at most {} KiB, UTF-8; or pipe it with `--file -`)",
            MAX_TEXT_BYTES / 1024
        ))),
    }
}

/// The file as written: frontmatter, then `body`.
fn render(ctx: &Context, actor: &str, tags: &[String], body: &str) -> String {
    let fm = Frontmatter::canonical(&[
        ("type", FmValue::str(INBOX)),
        ("created", FmValue::str(format_ts(&ctx.now))),
        ("actor", FmValue::str(actor)),
        ("tags", FmValue::list(tags)),
    ]);
    format!("{}{body}", fm.render())
}

/// The inbox file (`inbox/<name>.md`, top level) whose body is `body`.
fn already_filed(logbook: &Logbook, body: &str) -> Option<String> {
    let entries = std::fs::read_dir(logbook.path(INBOX)).ok()?;
    let mut names: Vec<String> = entries
        .filter_map(|e| e.ok())
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| n.ends_with(".md"))
        .collect();
    names.sort();
    names.into_iter().find_map(|name| {
        let path = logbook.path(format!("{INBOX}/{name}"));
        // a link, a large or unreadable file is not this text
        let text = sys::read_small_file(&path, 2 * MAX_TEXT_BYTES).ok()??;
        let doc = Document::parse(&text).ok()?;
        (doc.body == body).then(|| format!("{INBOX}/{name}"))
    })
}

/// Creates `inbox/<date>-<slug>.md` (or the first free `-N` of it) with
/// `text`; its path relative to the logbook.
fn create_free(ctx: &Context, logbook: &Logbook, title: &str, text: &str) -> Result<String> {
    let dir = logbook.path(INBOX);
    sys::create_dir_private(&dir).with_context(|| format!("cannot create {}", dir.display()))?;
    let stem = format!(
        "{}-{}",
        ctx.now.format("%Y-%m-%d"),
        cases::slug(title, FALLBACK_SLUG)
    );
    for n in 1..=MAX_SUFFIX {
        let name = match n {
            1 => format!("{stem}.md"),
            n => format!("{stem}-{n}.md"),
        };
        if create_new(&dir.join(&name), text)? {
            return Ok(format!("{INBOX}/{name}"));
        }
    }
    Err(Error::user(format!(
        "{INBOX}/{stem}.md and {} more of that name are taken; file it under another title",
        MAX_SUFFIX - 1
    )))
}

/// Writes `text` to the new file `path`; `false` when something (a file,
/// a link) is there already. A failed write removes the file it began.
fn create_new(path: &Path, text: &str) -> Result<bool> {
    let mut file = match sys::create_new_private(path) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => return Ok(false),
        Err(e) => {
            return Err(anyhow::Error::new(e)
                .context(format!("cannot create {}", path.display()))
                .into());
        }
    };
    let written = file
        .write_all(text.as_bytes())
        .and_then(|()| file.sync_all());
    if let Err(e) = written {
        drop(file);
        let _ = std::fs::remove_file(path);
        return Err(anyhow::Error::new(e)
            .context(format!("cannot write {}", path.display()))
            .into());
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_characters_are_dropped_and_counted() {
        assert_eq!(
            drop_format("to\u{200B}ken\u{202E}=x\u{E0041}"),
            ("token=x".to_string(), 3)
        );
        assert_eq!(drop_format("plain"), ("plain".to_string(), 0));
    }
}
