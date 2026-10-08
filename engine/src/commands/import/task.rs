//! `seldon import task <FILE>… [--area A] [--zone Z] [--risk R]
//! [--include-done] [--dry-run]` (SPEC-ENGINE §3, WP-102, ADR-0027 §7):
//! the user's own Markdown task files become queued cases, one per open
//! `- [ ]` item, or one per file without checklist items.
//!
//! A task file is untrusted text: it is read, never written, moved or run;
//! its words go through the logbook's redaction (SPEC-ENGINE §7, the whole
//! text as a note's, so the multi-line rules apply) before anything is
//! planned, and land as escaped text in the case's *Intent* below a fixed
//! line that names the source (`Imported from … — read before you start
//! this case.`): until the user starts the case, an agent treats it like
//! fetched text (ADR-0027 §2(a)).
//! Every path is checked before the first write: a regular `.md` file of
//! at most 1 MiB under the home, never inside the logbook. The marker
//! `.seldon/imports/tasks.json` remembers what was imported (file, line,
//! hash of the redacted text, case), so a second run creates nothing; an
//! entry is written `pending` before its case and settled after it, so a
//! crash or a failed write in between never makes the case twice.

use std::collections::BTreeSet;

use chrono::Datelike as _;
use std::io::Read as _;
use std::path::{Path, PathBuf};

use clap::Args;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::commands::agent::title_of;
use crate::commands::event::{ACTOR_ENV, actor_or_env, env_actor, parse_person};
use crate::commands::plan::{SHOW_INTENT_MAX, Spec, case_json, create};
use crate::commands::{Commit, Context, Output, autocommit};
use crate::error::{Error, Result};
use crate::import::task::{Tasks, parse};
use crate::import::{Scrubber, bad_path_char, case_source, marker_path};
use crate::logbook::Logbook;
use crate::logbook::cases;
use crate::model::event::ACTOR_HUMAN;
use crate::model::{Priority, Risk, Zone, is_agent, is_slug};
use crate::redact::Redactor;
use crate::sys;

/// The tag of an imported case (CONTRACT.md rule 8).
pub use crate::commands::plan::TAG_IMPORTED;

/// The marker's source name: `.seldon/imports/tasks.json`.
const MARKER: &str = "tasks";

/// The largest task file read, in bytes.
pub const MAX_FILE_BYTES: u64 = 1024 * 1024;

/// The most cases one run creates.
pub const MAX_CASES: usize = 200;

/// The completed Log line of a `- [x]` item imported with `--include-done`.
const DONE_LINE: &str = "completed: imported as done";

/// The first line of an imported case's *Intent* (WP-102 round 2, N4).
fn provenance(source: &str) -> String {
    format!("{PROVENANCE_START}{source}{PROVENANCE_END}")
}

/// The start and the end of [`provenance`]'s line, which the index skips
/// to find the intent (ADR-0038 §2).
pub const PROVENANCE_START: &str = "Imported from ";
pub const PROVENANCE_END: &str = " — read before you start this case.";

/// `SELDON_TEST_IMPORT_CRASH=after-create:<n>`: exit with
/// [`CRASH_EXIT`] after the n-th case of the run is created, before its
/// marker entry is settled; debug builds only.
#[cfg(debug_assertions)]
pub const CRASH_ENV: &str = "SELDON_TEST_IMPORT_CRASH";

#[cfg(debug_assertions)]
pub const CRASH_EXIT: i32 = 99;

#[cfg(debug_assertions)]
fn crash_point(point: &str) {
    if std::env::var_os(CRASH_ENV).is_some_and(|v| v == point) {
        std::process::exit(CRASH_EXIT);
    }
}

#[cfg(not(debug_assertions))]
fn crash_point(_: &str) {}

#[derive(Debug, Clone, Args)]
pub struct TaskArgs {
    /// Markdown task files under your home: one case per open `- [ ]`
    /// item; a file without checklist items is one case
    #[arg(value_name = "FILE", required = true)]
    pub files: Vec<PathBuf>,

    /// Area slug of the new cases; created under areas/ on first use
    #[arg(long, value_name = "AREA")]
    pub area: Option<String>,

    /// green, yellow or red
    #[arg(long, value_name = "ZONE", default_value = "yellow")]
    pub zone: Zone,

    /// R0 to R3
    #[arg(long, value_name = "RISK", default_value = "R1")]
    pub risk: Risk,

    /// Also import `- [x]` items, as completed cases
    #[arg(long)]
    pub include_done: bool,

    /// List what would be created; write nothing
    #[arg(long)]
    pub dry_run: bool,

    /// Who imports: human or agent:NAME (default: $SELDON_ACTOR, else
    /// human)
    #[arg(long, value_name = "ACTOR", value_parser = parse_person)]
    pub actor: Option<String>,
}

/// `.seldon/imports/tasks.json`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Marker {
    version: u32,
    items: Vec<Entry>,
}

/// One imported task.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Entry {
    /// The file as `~/…`.
    file: String,
    /// The item's line; `None` for a file imported whole.
    line: Option<usize>,
    /// SHA-256 of the task's redacted text (checkbox state left out).
    hash: String,
    case: String,
    imported_at: String,
    /// Written before the case, cleared after it: a pending entry whose
    /// case exists (with this import's Log line) is settled on the next
    /// run, one whose case does not is dropped.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pending: bool,
}

impl Entry {
    fn source(&self) -> String {
        match self.line {
            Some(n) => format!("{}#{n}", self.file),
            None => self.file.clone(),
        }
    }
}

/// A task file, read and redacted.
struct Source {
    /// `~/…`, redacted.
    shown: String,
    tasks: Tasks,
    /// The file name without `.md` (redacted), the title of a file
    /// without a heading.
    stem: String,
    /// Lines the redaction changed.
    redacted: usize,
    /// Invisible characters dropped (WP-102b round 2, B2; the set WP-159).
    dropped: usize,
}

/// A task found in a file.
struct Task {
    file: String,
    line: Option<usize>,
    done: bool,
    title: String,
    /// Redacted, not escaped yet.
    intent: String,
    hash: String,
}

impl Task {
    /// The case's *Intent* as written: the provenance line, then the
    /// task's text escaped.
    fn full_intent(&self) -> String {
        format!(
            "{}\n\n{}",
            provenance(&self.source()),
            cases::escape_lines(&self.intent)
        )
    }

    fn source(&self) -> String {
        match self.line {
            Some(n) => format!("{}#{n}", self.file),
            None => self.file.clone(),
        }
    }
}

/// What happens to a task.
enum Fate {
    Create {
        replaces: Option<String>,
    },
    Skip {
        reason: &'static str,
        case: Option<String>,
    },
}

pub fn run(ctx: &Context, args: TaskArgs) -> Result<Output> {
    let actor = actor_or_env(args.actor.clone(), parse_person, ACTOR_HUMAN)?;
    if args.include_done {
        // as `plan done`: an agent's session cannot close as a person
        // (ADR-0027 §5; an agent close is never recorded as human)
        let session = env_actor(parse_person).ok().flatten();
        if let Some(agent) = Some(&actor)
            .filter(|a| is_agent(a))
            .or(session.as_ref().filter(|s| is_agent(s)))
        {
            return Err(Error::user(format!(
                "--include-done makes completed cases, and an agent closes a case only with a Result, never as a person (ADR-0027 §5); this is {agent}'s session (--actor or ${ACTOR_ENV}): import the done items as a person, outside it"
            )));
        }
    }
    if let Some(area) = args.area.as_deref()
        && !is_slug(area)
    {
        return Err(Error::user(format!(
            "area `{area}` is not a lowercase slug ([a-z0-9][a-z0-9-]*)"
        )));
    }
    let (config, logbook) = ctx.open_logbook()?;
    let redactor = Redactor::for_config(&config)?;
    let mut scrubber = Scrubber::new(redactor.clone());

    // every path is read and checked before anything is planned or written
    let mut seen = BTreeSet::new();
    let mut sources = Vec::new();
    for arg in &args.files {
        let path = resolve(ctx, &logbook, arg)?;
        if !seen.insert(path.clone()) {
            continue;
        }
        sources.push(read_source(ctx, &redactor, &mut scrubber, &path)?);
    }
    let tasks: Vec<Task> = sources.iter().flat_map(tasks_of).collect();

    let lock = if args.dry_run {
        None
    } else {
        Some(ctx.lock()?)
    };
    let marker_rel = marker_path(MARKER);
    let mut marker = read_marker(&logbook.path(&marker_rel))?;
    // what an earlier run left pending: settled in the file only when this
    // run writes (never in a dry run)
    let settled = settle(&logbook, &mut marker);
    if settled && lock.is_some() {
        write_marker(&logbook.path(&marker_rel), &marker)?;
    }
    let redacted: usize = sources.iter().map(|s| s.redacted).sum();
    let dropped: usize = sources.iter().map(|s| s.dropped).sum();
    let fates = decide(&tasks, &marker, args.include_done);
    let planned = fates
        .iter()
        .filter(|f| matches!(f, Fate::Create { .. }))
        .count();
    if planned > MAX_CASES {
        return Err(Error::user(format!(
            "{planned} cases to create; one import makes at most {MAX_CASES}: split the file or import fewer files at once"
        )));
    }
    let skipped: Vec<Value> = tasks
        .iter()
        .zip(&fates)
        .filter_map(|(t, f)| match f {
            Fate::Skip { reason, case } => {
                Some(json!({"source": t.source(), "reason": reason, "case": case}))
            }
            Fate::Create { .. } => None,
        })
        .collect();
    let status_of = |t: &Task| if t.done { "completed" } else { "queued" };

    let Some(lock) = lock else {
        let created: Vec<Value> = tasks
            .iter()
            .zip(&fates)
            .filter_map(|(t, f)| match f {
                Fate::Create { replaces } => Some(json!({
                    "id": null,
                    "title": t.title,
                    "status": status_of(t),
                    "source": t.source(),
                    "path": null,
                    "replaces": replaces,
                })),
                Fate::Skip { .. } => None,
            })
            .collect();
        let human = report("Dry run: would create", &created, &skipped);
        return Ok(Output::ok(
            human,
            json!({
                "mode": "dry-run",
                "created": created,
                "skipped": skipped,
                "redactedLines": redacted,
                "droppedCharacters": dropped,
                "areaCreated": null,
                "files": [],
                "marker": null,
                "git": Commit::Skipped("dry run").json(),
            }),
        ));
    };

    let mut created = Vec::new();
    let mut files = Vec::new();
    let mut area_created = None;
    let mut failure = None;
    for (task, fate) in tasks.iter().zip(&fates) {
        let Fate::Create { replaces } = fate else {
            continue;
        };
        let mut note = format!("imported from {}", task.source());
        if let Some(earlier) = replaces {
            note.push_str(&format!(", changed since {earlier}"));
        }
        let intent = task.full_intent();
        // the entry first, pending: a crash or a failed write after the
        // case never makes it again (the next run settles it)
        let next = match cases::next_id(&logbook, ctx.now.year()) {
            Ok(id) => id,
            Err(e) => {
                failure = Some(e);
                break;
            }
        };
        marker.items.push(Entry {
            file: task.file.clone(),
            line: task.line,
            hash: task.hash.clone(),
            case: next,
            imported_at: crate::model::event::format_ts(&ctx.now),
            pending: true,
        });
        if let Err(e) = write_marker(&logbook.path(&marker_rel), &marker) {
            marker.items.pop();
            failure = Some(e);
            break;
        }
        let spec = Spec {
            title: task.title.clone(),
            zone: args.zone,
            risk: args.risk,
            area: args.area.clone(),
            priority: Priority::Normal,
            actor: actor.clone(),
            intent: Some(intent),
            tags: vec![TAG_IMPORTED.to_string()],
            note: Some(note),
            start: false,
            point: false,
            done: task.done.then(|| DONE_LINE.to_string()),
            // the case says where it came from (ADR-0038 §3); the marker
            // stays the only idempotency key
            source: Some(case_source(&task.source())),
        };
        let made = match create(ctx, &config, &logbook, &lock, spec) {
            Ok(made) => made,
            Err(e) => {
                // no case: the pending entry goes (the next run would
                // drop it anyway)
                marker.items.pop();
                let _ = write_marker(&logbook.path(&marker_rel), &marker);
                failure = Some(e);
                break;
            }
        };
        let id = made.file.case.id.clone();
        area_created = area_created.or(made.area_created.clone());
        if let Some(entry) = marker.items.last_mut() {
            entry.case = id.clone();
            entry.pending = false;
        }
        crash_point(&format!("after-create:{}", created.len() + 1));
        if let Err(e) = write_marker(&logbook.path(&marker_rel), &marker) {
            failure = Some(e);
        }
        let case = case_json(&logbook, &made.file);
        files.push(case["path"].clone());
        created.push(json!({
            "id": id,
            "title": case["title"],
            "status": case["status"],
            "source": task.source(),
            "path": case["path"],
            "replaces": replaces,
        }));
        if failure.is_some() {
            break;
        }
    }
    let commit = if created.is_empty() && !settled {
        Commit::Skipped("nothing changed")
    } else {
        files.push(Value::String(marker_rel.clone()));
        let commit = autocommit(ctx, &config, &logbook, "import task");
        crate::index::rebuild_if_initialised(ctx);
        commit
    };
    drop(lock);
    if let Some(e) = failure {
        let made = created
            .iter()
            .filter_map(|c| c["id"].as_str())
            .collect::<Vec<_>>()
            .join(", ");
        if made.is_empty() {
            return Err(e);
        }
        let more = format!("created before the failure, and kept: {made}; a second run skips them");
        return Err(match e {
            Error::User(m) => Error::user(format!("{m} ({more})")),
            other => Error::from(anyhow::anyhow!("{other} ({more})")),
        });
    }
    let mut human = if created.is_empty() {
        report("Nothing imported", &created, &skipped)
    } else {
        report("Imported", &created, &skipped)
    };
    human.push_str(&commit.human());
    Ok(Output::ok(
        human,
        json!({
            "mode": "apply",
            "created": created,
            "skipped": skipped,
            "redactedLines": redacted,
            "droppedCharacters": dropped,
            "areaCreated": area_created,
            "files": files,
            "marker": (!created.is_empty() || settled).then_some(marker_rel),
            "git": commit.json(),
        }),
    ))
}

/// Settles the pending entries an earlier run left (a crash or a failed
/// write between an entry and its case): an entry whose case exists and
/// names this import in its Log is complete; any other is dropped, so its
/// task is imported again. Whether anything changed.
fn settle(logbook: &Logbook, marker: &mut Marker) -> bool {
    let before = marker.items.len();
    let mut changed = false;
    marker.items.retain_mut(|e| {
        if !e.pending {
            return true;
        }
        changed = true;
        // the Log line's words, up to what follows the source (` · actor`
        // or `, changed since …`): `#1` is not `#12`
        let line = format!("imported from {}", e.source());
        let made = cases::find(logbook, &e.case).is_ok_and(|f| {
            f.case.tags.iter().any(|t| t == TAG_IMPORTED)
                && [" ·", ","]
                    .iter()
                    .any(|end| f.doc.body.contains(&format!("{line}{end}")))
        });
        e.pending = false;
        made
    });
    changed || marker.items.len() != before
}

/// The human report: what was (or would be) created, then what was
/// skipped and why.
fn report(head: &str, created: &[Value], skipped: &[Value]) -> String {
    let mut out = if created.is_empty() {
        format!("{head}: no new tasks.")
    } else {
        format!("{head} {} case(s):", created.len())
    };
    for c in created {
        let id = c["id"]
            .as_str()
            .map_or(String::new(), |id| format!("{id} "));
        out.push_str(&format!(
            "\n  {id}\"{}\" ({}) from {}",
            c["title"].as_str().unwrap_or_default(),
            c["status"].as_str().unwrap_or_default(),
            c["source"].as_str().unwrap_or_default()
        ));
    }
    if !skipped.is_empty() {
        out.push_str(&format!("\nSkipped {}:", skipped.len()));
        for s in skipped {
            let case = s["case"]
                .as_str()
                .map_or(String::new(), |c| format!(" as {c}"));
            out.push_str(&format!(
                "\n  {} ({}{case})",
                s["source"].as_str().unwrap_or_default(),
                s["reason"].as_str().unwrap_or_default()
            ));
        }
    }
    out
}

/// The file `arg` names, resolved with its symbolic links, checked: under
/// the home, outside the logbook, a regular `.md` file.
fn resolve(ctx: &Context, logbook: &Logbook, arg: &Path) -> Result<PathBuf> {
    let refuse_chars = || {
        Error::user(
            "a task file's path has a control, text-direction or invisible format character; rename the file or its folder"
                .to_string(),
        )
    };
    if arg.to_string_lossy().chars().any(bad_path_char) {
        return Err(refuse_chars());
    }
    let path = match arg.strip_prefix("~") {
        Ok(rest) => ctx.dirs.home.join(rest),
        Err(_) => std::path::absolute(arg).unwrap_or_else(|_| arg.to_path_buf()),
    };
    let shown = ctx.dirs.display(&path);
    let real = std::fs::canonicalize(&path)
        .map_err(|e| Error::user(format!("{shown}: cannot read the task file: {e}")))?;
    // the resolved path too: a linked folder may bring one in
    if real.to_string_lossy().chars().any(bad_path_char) {
        return Err(refuse_chars());
    }
    let home = std::fs::canonicalize(&ctx.dirs.home).unwrap_or_else(|_| ctx.dirs.home.clone());
    if real == home || !real.starts_with(&home) {
        return Err(Error::user(format!(
            "{shown} is outside your home directory; import reads task files under {} only",
            ctx.dirs.display(&ctx.dirs.home)
        )));
    }
    let root = std::fs::canonicalize(&logbook.root).unwrap_or_else(|_| logbook.root.clone());
    if real.starts_with(&root) {
        return Err(Error::user(format!(
            "{shown} is inside the logbook; import reads your own task files, never the logbook"
        )));
    }
    let meta = std::fs::metadata(&real)
        .map_err(|e| Error::user(format!("{shown}: cannot read the task file: {e}")))?;
    if meta.is_dir() {
        return Err(Error::user(format!(
            "{shown} is a directory; name the Markdown files in it"
        )));
    }
    if !meta.is_file() {
        return Err(Error::user(format!("{shown} is not a regular file")));
    }
    if !real
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("md"))
    {
        return Err(Error::user(format!("{shown} is not a Markdown file (.md)")));
    }
    Ok(real)
}

/// Reads and redacts the checked file `path` (at most [`MAX_FILE_BYTES`],
/// UTF-8).
fn read_source(
    ctx: &Context,
    redactor: &Redactor,
    scrubber: &mut Scrubber,
    path: &Path,
) -> Result<Source> {
    let home = std::fs::canonicalize(&ctx.dirs.home).unwrap_or_else(|_| ctx.dirs.home.clone());
    let rest = path.strip_prefix(&home).unwrap_or(path);
    let shown = redactor.redact(&format!("~/{}", rest.to_string_lossy()));
    let file = std::fs::File::open(path)
        .map_err(|e| Error::user(format!("{shown}: cannot read the task file: {e}")))?;
    let mut bytes = Vec::new();
    file.take(MAX_FILE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| Error::user(format!("{shown}: cannot read the task file: {e}")))?;
    if bytes.len() as u64 > MAX_FILE_BYTES {
        return Err(Error::user(format!(
            "{shown} is larger than {} KiB; a task file is smaller",
            MAX_FILE_BYTES / 1024
        )));
    }
    let text =
        String::from_utf8(bytes).map_err(|_| Error::user(format!("{shown} is not UTF-8 text")))?;
    // CRLF as LF first (round 3): the rules read `\r\n` as `\n` since
    // WP-128, but the parser and the marker's task hashes take LF text, so
    // a CRLF file already imported is not imported again; the line count
    // does not change
    let text = text.replace("\r\n", "\n");
    // invisible characters out (WP-102b round 2, B2; the set WP-159): an
    // invisible instruction would reach the case, its title and an agent
    // while the desk's review drops it (the redaction reads the text
    // without them in any case)
    let before = text.chars().count();
    let text = crate::redact::without_invisible(&text).into_owned();
    let dropped = before - text.chars().count();
    // the scrubber redacts the whole text, as a note's, keeping its line
    // breaks (WP-140), then rewrites home paths line by line
    let first_hit = scrubber.hits.len();
    let text = scrubber.text(&shown, &text);
    let changed: BTreeSet<usize> = scrubber.hits[first_hit..].iter().map(|h| h.line).collect();
    let stem = path
        .file_stem()
        .map(|s| redactor.redact(&s.to_string_lossy()))
        .unwrap_or_default();
    Ok(Source {
        shown,
        tasks: parse(&text),
        stem,
        redacted: changed.len(),
        dropped,
    })
}

/// The tasks of one file.
fn tasks_of(source: &Source) -> Vec<Task> {
    let hash = |kind: &str, text: &str| sys::sha256_hex(format!("{kind}\n{text}").as_bytes());
    match &source.tasks {
        Tasks::Items(items) => items
            .iter()
            .map(|item| {
                let mut intent = item.text.clone();
                if let Some(section) = &item.section {
                    intent.push_str(&format!("\n\nSection: {section}"));
                }
                Task {
                    file: source.shown.clone(),
                    line: Some(item.line),
                    done: item.done,
                    title: title_of(&item.text),
                    intent,
                    hash: hash("item", &item.text),
                }
            })
            .collect(),
        Tasks::Whole(whole) => {
            // an empty file is no task: no title, so it is skipped as empty
            let title = match (&whole.title, whole.text.trim().is_empty()) {
                (None, true) => String::new(),
                (title, _) => title_of(title.as_deref().unwrap_or(&source.stem)),
            };
            vec![Task {
                file: source.shown.clone(),
                line: None,
                done: false,
                hash: hash("file", &format!("{title}\n{}", whole.text)),
                title,
                intent: whole.text.clone(),
            }]
        }
    }
}

/// What happens to each task, in order.
fn decide(tasks: &[Task], marker: &Marker, include_done: bool) -> Vec<Fate> {
    let mut taken: BTreeSet<(&str, &str)> = BTreeSet::new();
    tasks
        .iter()
        .map(|t| {
            if t.done && !include_done {
                return Fate::Skip {
                    reason: "done",
                    case: None,
                };
            }
            if !t.title.chars().any(char::is_alphanumeric) {
                return Fate::Skip {
                    reason: "empty",
                    case: None,
                };
            }
            // what the desk shows whole before a Start (round 2, B1)
            if t.full_intent().len() > SHOW_INTENT_MAX {
                return Fate::Skip {
                    reason: "too-long",
                    case: None,
                };
            }
            if let Some(e) = marker
                .items
                .iter()
                .find(|e| e.file == t.file && e.hash == t.hash)
            {
                return Fate::Skip {
                    reason: "already-imported",
                    case: Some(e.case.clone()),
                };
            }
            if !taken.insert((&t.file, &t.hash)) {
                return Fate::Skip {
                    reason: "duplicate",
                    case: None,
                };
            }
            // the earlier task at this place whose text is gone from the
            // file: this one is its changed form
            let replaces = marker
                .items
                .iter()
                .rev()
                .find(|e| {
                    e.file == t.file
                        && e.line == t.line
                        && !tasks.iter().any(|o| o.file == e.file && o.hash == e.hash)
                })
                .map(|e| e.case.clone());
            Fate::Create { replaces }
        })
        .collect()
}

/// The marker, or an empty one when there is none yet; one that cannot
/// be read is a user error (nothing is written).
fn read_marker(path: &Path) -> Result<Marker> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Marker {
                version: 1,
                items: Vec::new(),
            });
        }
        Err(e) => {
            return Err(Error::user(format!(
                "{}: cannot read the import marker: {e}",
                path.display()
            )));
        }
    };
    let marker: Marker = serde_json::from_str(&text).map_err(|e| {
        Error::user(format!(
            "{} is not a valid import marker ({e}); restore it from the logbook's git history, nothing was imported",
            path.display()
        ))
    })?;
    if marker.version != 1 {
        return Err(Error::user(format!(
            "{} has version {}; this engine knows version 1",
            path.display(),
            marker.version
        )));
    }
    Ok(marker)
}

fn write_marker(path: &Path, marker: &Marker) -> Result<()> {
    let text = serde_json::to_string_pretty(marker).expect("json");
    sys::write_atomic(path, format!("{text}\n").as_bytes())?;
    Ok(())
}
