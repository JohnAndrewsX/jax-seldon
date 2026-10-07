//! `seldon open <case|journal|ledger|status|logbook|ID> [--editor]`
//! (SPEC-ENGINE §3): prints the path; `--editor` opens it.
//!
//! The editor gets the path as one argument, never through a shell
//! (AGENTS.md §8). On a terminal it is `$VISUAL`, else `$EDITOR` (split at
//! whitespace, so `code --wait` works), else `omarchy-launch-editor
//! --inline`. Without a terminal (the plugin) it is `omarchy-launch-editor`,
//! which opens the user's default editor in its own window: started
//! detached ([`launch_detached`]), because the launcher stays in the
//! foreground while a terminal editor runs (WP-012, decision 1). That
//! launch carries `SELDON_OPEN=<path>` ([`sessions::OPEN_ENV`]); a second
//! open of the same path while a terminal window Omarchy opened for it
//! (class `org.omarchy.*`) is open focuses that window instead of starting
//! another (WP-156, ADR-0041). A GUI editor launches as before.

use std::io::IsTerminal as _;
use std::os::unix::process::CommandExt as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use clap::Args;
use serde_json::{Value, json};

use super::{Context, Output};
use crate::error::{Error, Result};
use crate::logbook::{Logbook, cases, journal};
use crate::model::{is_case_id, is_decision_id};
use crate::sessions;
use crate::sys::{self, Run};

/// Omarchy's launcher for the default editor (`omarchy-launch-editor`).
pub const OMARCHY_EDITOR: &str = "omarchy-launch-editor";

/// How long a detached launcher is watched for a start-up failure.
const LAUNCH_GRACE: Duration = Duration::from_millis(200);

#[derive(Debug, Clone, Args)]
pub struct OpenArgs {
    /// case (the active one), journal (today), ledger (this month), status,
    /// logbook, or a case or decision id
    #[arg(value_name = "WHAT")]
    pub what: String,

    /// Open it in the editor
    #[arg(long)]
    pub editor: bool,
}

pub fn run(ctx: &Context, args: OpenArgs) -> Result<Output> {
    let (_, logbook) = ctx.open_logbook()?;
    let path = resolve(ctx, &logbook, &args.what, args.editor)?;
    let editor = if args.editor {
        let opened =
            edit(&path).map_err(|e| Error::user(format!("cannot open {}: {e}", path.display())))?;
        Some(opened)
    } else {
        None
    };
    let human = match editor.as_ref().map(|e| &e.how) {
        Some(How::Focused { address, .. }) => {
            format!(
                "{} (already open; focused its window {address})",
                path.display()
            )
        }
        _ => path.display().to_string(),
    };
    Ok(Output::ok(
        human,
        json!({
            "what": args.what,
            "path": path,
            "editor": editor.map(|e| editor_json(&Ok(e))),
        }),
    ))
}

/// The file (or folder) `what` names.
fn resolve(ctx: &Context, logbook: &Logbook, what: &str, editor: bool) -> Result<PathBuf> {
    Ok(match what {
        "case" => {
            let id = cases::active_case(logbook)
                .ok_or_else(|| Error::user("no active case; name one: `seldon open C-YYYY-NNN`"))?;
            cases::find(logbook, &id)?.path
        }
        "journal" => {
            let day = if editor {
                let _lock = ctx.lock()?;
                journal::ensure_day(logbook, &ctx.now)?.path
            } else {
                crate::model::Journal::relative_path(ctx.now.date_naive())
            };
            logbook.path(day)
        }
        "ledger" => {
            // the generated view, if there is one: the .jsonl is never edited
            let month = ctx.now.format("%Y-%m");
            let view = logbook.path(format!("ledger/{month}.md"));
            if view.is_file() {
                view
            } else {
                logbook.path(format!("ledger/{month}.jsonl"))
            }
        }
        "status" => logbook.path("STATUS.md"),
        "logbook" => logbook.root.clone(),
        id if is_case_id(id) => cases::find(logbook, id)?.path,
        id if is_decision_id(id) => {
            let prefix = format!("{id}-");
            logbook
                .decision_files()?
                .into_iter()
                .find(|p| {
                    p.file_name().is_some_and(|n| {
                        let n = n.to_string_lossy();
                        n.starts_with(&prefix) || n == format!("{id}.md")
                    })
                })
                .ok_or_else(|| Error::user(format!("unknown decision {id}")))?
        }
        other => {
            return Err(Error::user(format!(
                "cannot open `{other}`: use case, journal, ledger, status, logbook, C-YYYY-NNN or ADR-NNNN"
            )));
        }
    })
}

/// What `edit` did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Editor {
    /// The program started, or the launcher of the editor found.
    pub program: String,
    pub how: How,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum How {
    /// Started.
    Launched,
    /// A terminal window of an editor this engine started on the path is
    /// open: it was focused.
    Focused { address: String, pid: i64 },
}

/// Opens `path` in the editor (see the module comment).
pub fn edit(path: &Path) -> Result<Editor, String> {
    let path_arg = path.to_string_lossy();
    let terminal = std::io::stdin().is_terminal() && std::io::stdout().is_terminal();
    if terminal {
        let configured = ["VISUAL", "EDITOR"]
            .iter()
            .find_map(|v| std::env::var(v).ok().filter(|s| !s.trim().is_empty()));
        let (program, mut args): (String, Vec<String>) = match configured {
            Some(cmd) => {
                let mut words = cmd.split_whitespace().map(String::from);
                let program = words.next().unwrap_or_default();
                (program, words.collect())
            }
            None => (OMARCHY_EDITOR.to_string(), vec!["--inline".to_string()]),
        };
        args.push(path_arg.into_owned());
        let argv: Vec<&str> = args.iter().map(String::as_str).collect();
        return outcome(&program, sys::run_attached(&program, &argv)).map(launched);
    }
    if let Some(found) = already_open(path) {
        return Ok(found);
    }
    let mut cmd = Command::new(OMARCHY_EDITOR);
    cmd.arg(&*path_arg).env(sessions::OPEN_ENV, path);
    outcome(OMARCHY_EDITOR, launch_detached(cmd, Stdio::null())).map(launched)
}

fn launched(program: String) -> Editor {
    Editor {
        program,
        how: How::Launched,
    }
}

/// The terminal window of an editor an earlier `open --editor` started
/// on `path`, focused ([`sessions::marked_windows`]: class `org.omarchy.*`,
/// the marker in its tree). `None` (launch as usual) when there is none,
/// when Hyprland cannot be asked, or when the focus fails.
fn already_open(path: &Path) -> Option<Editor> {
    let windows = sessions::windows().ok()?;
    let found = sessions::marked_windows(
        Path::new(sessions::PROC),
        &windows,
        |c| c.starts_with(sessions::TUI_CLASS_PREFIX) && c != sessions::AGENT_CLASS,
        &[(sessions::OPEN_ENV, path.as_os_str())],
        &[],
    );
    let window = &found.first()?.window;
    sessions::focus(&window.address).ok()?;
    Some(Editor {
        program: OMARCHY_EDITOR.to_string(),
        how: How::Focused {
            address: window.address.clone(),
            pid: window.pid,
        },
    })
}

/// Starts `cmd` (program, arguments, and whatever directory or environment
/// the caller set) detached: stdin and stdout null, stderr `stderr` (null
/// or a file, never a pipe: an inherited pipe would keep the caller, e.g.
/// the plugin's process queue, waiting for the editor or agent), its own
/// process group (a signal to ours does not
/// reach it), never killed, never waited for. It is watched for
/// [`LAUNCH_GRACE`]: an exit in that time is its result (a non-zero exit
/// is an error), otherwise it counts as launched (`code: Some(0)`).
pub(crate) fn launch_detached(mut cmd: Command, stderr: Stdio) -> Run {
    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(stderr)
        .process_group(0);
    // ETXTBSY (as in `sys::run`): a just-written program still open for
    // writing in another thread's forked child; brief, retry
    let mut spawned = cmd.spawn();
    for _ in 0..20 {
        match &spawned {
            Err(e) if e.raw_os_error() == Some(26) => {
                std::thread::sleep(Duration::from_millis(5));
                spawned = cmd.spawn();
            }
            _ => break,
        }
    }
    let mut child = match spawned {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Run::NotFound,
        Err(e) => return Run::Failed(e.to_string()),
    };
    let deadline = Instant::now() + LAUNCH_GRACE;
    let code = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status.code(),
            Ok(None) if Instant::now() >= deadline => break Some(0),
            Ok(None) => std::thread::sleep(Duration::from_millis(10)),
            Err(e) => return Run::Failed(e.to_string()),
        }
    };
    Run::Exited {
        code,
        stdout: String::new(),
        stderr: String::new(),
    }
}

fn outcome(program: &str, run: Run) -> Result<String, String> {
    match run {
        Run::Exited { code: Some(0), .. } => Ok(program.to_string()),
        Run::Exited { code, stderr, .. } => Err(format!(
            "{program} exited with {}{}",
            code.map_or("a signal".to_string(), |c| c.to_string()),
            if stderr.trim().is_empty() {
                String::new()
            } else {
                format!(": {}", stderr.trim())
            }
        )),
        Run::NotFound if program == OMARCHY_EDITOR => Err(format!(
            "no terminal and no {OMARCHY_EDITOR}; set $EDITOR or open the path yourself"
        )),
        Run::NotFound => Err(format!("{program} not found (check $VISUAL / $EDITOR)")),
        Run::TimedOut => Err(format!("{program} did not return")),
        Run::Failed(e) => Err(format!("cannot start {program}: {e}")),
    }
}

/// `{"launched": true, "program": …}`; an editor already open on the path:
/// `{"launched": false, "focused": true, "address", "pid", "program"}`; a
/// failure `{"launched": false, "error": …}`.
pub fn editor_json(result: &Result<Editor, String>) -> Value {
    match result {
        Ok(Editor {
            program,
            how: How::Launched,
        }) => json!({ "launched": true, "program": program }),
        Ok(Editor {
            program,
            how: How::Focused { address, pid },
        }) => json!({
            "launched": false, "focused": true, "address": address, "pid": pid, "program": program
        }),
        Err(e) => json!({ "launched": false, "error": e }),
    }
}
