//! `seldon open <case|journal|ledger|status|logbook|ID> [--editor]`
//! (SPEC-ENGINE §3): prints the path; `--editor` opens it.
//!
//! The editor gets the path as one argument, never through a shell
//! (AGENTS.md §8). On a terminal it is `$VISUAL`, else `$EDITOR` (split at
//! whitespace, so `code --wait` works), else `omarchy-launch-editor
//! --inline`. Without a terminal (the plugin) it is `omarchy-launch-editor`,
//! which opens the user's default editor in its own window: started
//! detached ([`launch_detached`]), because the launcher stays in the
//! foreground while a terminal editor runs (WP-012, decision 1).

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
        let launched =
            edit(&path).map_err(|e| Error::user(format!("cannot open {}: {e}", path.display())))?;
        Some(launched)
    } else {
        None
    };
    Ok(Output::ok(
        path.display().to_string(),
        json!({
            "what": args.what,
            "path": path,
            "editor": editor.map(|program| json!({ "launched": true, "program": program })),
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

/// Opens `path` in the editor. `Ok` names the program that was started.
pub fn edit(path: &Path) -> Result<String, String> {
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
        return outcome(&program, sys::run_attached(&program, &argv));
    }
    let mut cmd = Command::new(OMARCHY_EDITOR);
    cmd.arg(&*path_arg);
    outcome(OMARCHY_EDITOR, launch_detached(cmd, Stdio::null()))
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
        // an editor on the terminal: no output is captured
        Run::Cut => Err(format!("{program}: output over the limit")),
        Run::Failed(e) => Err(format!("cannot start {program}: {e}")),
    }
}

/// `{"launched": true, "program": …}` or `{"launched": false, "error": …}`.
pub fn editor_json(result: &Result<String, String>) -> Value {
    match result {
        Ok(program) => json!({ "launched": true, "program": program }),
        Err(e) => json!({ "launched": false, "error": e }),
    }
}
