//! `seldon open` (the app) and `seldon edit <case|journal|ledger|status|logbook|ID>
//! [--editor]` (SPEC-ENGINE §3; WP-229, ADR-0058).
//!
//! **The app.** `seldon open` with no target starts the Seldon app, the
//! program `$SELDON_DESK` names (an absolute path, for development) or
//! [`DESK`]. Without `--json` it runs in the foreground: the engine
//! replaces itself with it (`exec`), so the `.desktop` file's `Exec=seldon
//! open` and a terminal see the app. With `--json` (the bar's pill) it is
//! started detached in a session of its own, stdio on `/dev/null`, and the
//! engine answers `{"started": true}` and exits: the app never lives under
//! the caller (`omarchy-shell`). A missing app is exit 1, "install the
//! seldon package". No logbook is needed: the app shows its own states.
//!
//! **The editor.** `seldon edit <target>` prints the path; `--editor`
//! opens it. `seldon open <target>` is the same command under its old
//! name, kept for the 0.5.x plugins and agents (same JSON, a note on
//! stderr) and removed in 0.6.0.
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
use std::os::unix::fs::PermissionsExt as _;
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

/// The Seldon app's binary, as the `seldon` package installs it.
pub const DESK: &str = "/usr/lib/seldon/seldon-desk";

/// Another app binary in its place (development): an absolute path.
pub const DESK_ENV: &str = "SELDON_DESK";

/// The note `open <target>` prints on stderr.
pub const OPEN_DEPRECATED: &str = "`seldon open <target>` is now `seldon edit <target>`; \
     the old form goes in 0.6.0";

#[derive(Debug, Clone, Args)]
#[command(
    after_help = "Without a target: the Seldon app (--json: started detached).
With a target: the old name of `seldon edit <target>`, removed in 0.6.0.

Examples:
  seldon open
  seldon open --json"
)]
pub struct OpenArgs {
    /// Deprecated: use `seldon edit <WHAT>`
    #[arg(value_name = "WHAT")]
    pub what: Option<String>,

    /// Deprecated: use `seldon edit <WHAT> --editor`
    #[arg(long, requires = "what")]
    pub editor: bool,
}

#[derive(Debug, Clone, Args)]
#[command(after_help = "Examples:
  seldon edit case
  seldon edit journal --editor
  seldon edit ADR-0003 --editor --json")]
pub struct EditArgs {
    /// case (the active one), journal (today), ledger (this month), status,
    /// logbook, or a case or decision id
    #[arg(value_name = "WHAT")]
    pub what: String,

    /// Open it in the editor
    #[arg(long)]
    pub editor: bool,
}

/// `seldon open [<target>]`: the app, or the deprecated `edit`.
pub fn run_open(ctx: &Context, args: OpenArgs) -> Result<Output> {
    let Some(what) = args.what else {
        return desk(ctx);
    };
    if !ctx.quiet {
        eprintln!("seldon: note: {OPEN_DEPRECATED}");
    }
    run(
        ctx,
        EditArgs {
            what,
            editor: args.editor,
        },
    )
}

/// The app binary: `$SELDON_DESK` when set (not empty), else [`DESK`].
fn desk_path() -> Result<PathBuf> {
    match std::env::var_os(DESK_ENV).filter(|v| !v.is_empty()) {
        Some(v) => {
            let path = PathBuf::from(v);
            if path.is_absolute() {
                Ok(path)
            } else {
                Err(Error::user(format!(
                    "{DESK_ENV}={} is not an absolute path; name the app binary by its full path",
                    path.display()
                )))
            }
        }
        None => Ok(PathBuf::from(DESK)),
    }
}

/// Starts the app (the module comment): `exec` without `--json`, detached
/// in its own session with it.
fn desk(ctx: &Context) -> Result<Output> {
    let path = desk_path()?;
    let shown = path.display();
    match std::fs::metadata(&path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(Error::user(format!(
                "The Seldon window is not installed ({shown} is missing). \
                 Install the seldon package."
            )));
        }
        Err(e) => return Err(Error::user(format!("cannot read {shown}: {e}"))),
        Ok(m) if !m.is_file() || m.permissions().mode() & 0o111 == 0 => {
            return Err(Error::user(format!(
                "{shown} is not a regular executable file, so the Seldon window cannot start. \
                 Reinstall the seldon package."
            )));
        }
        Ok(_) => {}
    }
    let mut cmd = Command::new(&path);
    if !ctx.json {
        // returns only when the exec failed
        let e = cmd.exec();
        return Err(Error::user(if e.kind() == std::io::ErrorKind::NotFound {
            no_loader(&path)
        } else {
            format!("The Seldon window cannot start ({shown}): {e}.")
        }));
    }
    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    detach(&mut cmd);
    match watch_start(cmd) {
        // an exit 0 at once is a start too: a second `seldon open` hands
        // over to the window that is open (ADR-0058 §1)
        Run::Exited { code: Some(0), .. } => Ok(Output::ok(
            format!("Started {shown}"),
            json!({ "started": true }),
        )),
        Run::Exited { code, .. } => Err(Error::user(format!(
            "The Seldon window stopped at once: {shown} exited with {}.",
            code.map_or("a signal".to_string(), |c| c.to_string())
        ))),
        Run::NotFound => Err(Error::user(no_loader(&path))),
        other => {
            let e = outcome(&shown.to_string(), other).err().unwrap_or_default();
            Err(Error::user(format!(
                "The Seldon window cannot start ({shown}): {e}."
            )))
        }
    }
}

/// ENOENT from the exec of an executable file that is there: its
/// interpreter (`#!`) or its loader is missing.
fn no_loader(path: &Path) -> String {
    format!(
        "The Seldon window cannot start: {} needs an interpreter or loader that is missing. \
         Reinstall the seldon package.",
        path.display()
    )
}

/// The highest descriptor this process has open (`/proc/self/fd`), for the
/// fallback of [`detach`]; 1023 when it cannot be read.
fn highest_fd() -> i32 {
    std::fs::read_dir("/proc/self/fd")
        .ok()
        .and_then(|dir| {
            dir.filter_map(|e| e.ok()?.file_name().to_str()?.parse::<i32>().ok())
                .max()
        })
        .unwrap_or(1023)
}

/// The child calls `setsid(2)` before it runs its program: a session and
/// process group of its own, no controlling terminal, so neither a signal
/// to the caller's group nor the end of its terminal reaches it. And every
/// descriptor from 3 up is marked close-on-exec: one the caller passed
/// without the flag never reaches the long-lived app.
fn detach(cmd: &mut Command) {
    unsafe extern "C" {
        fn setsid() -> i32;
        fn syscall(number: std::ffi::c_long, ...) -> std::ffi::c_long;
        fn fcntl(fd: i32, cmd: i32, ...) -> i32;
    }
    // close_range(2) is 436 on every Linux architecture (the unified table)
    const SYS_CLOSE_RANGE: std::ffi::c_long = 436;
    const CLOSE_RANGE_CLOEXEC: std::ffi::c_uint = 1 << 2;
    const F_GETFD: i32 = 1;
    const F_SETFD: i32 = 2;
    const FD_CLOEXEC: i32 = 1;
    let highest = highest_fd();
    // SAFETY: the closure runs in the forked child before exec and calls
    // only setsid(2), close_range(2) and fcntl(2), which are
    // async-signal-safe and allocate nothing; the child is never a group
    // leader here (no `process_group`), so setsid cannot fail with EPERM.
    // Marking descriptors close-on-exec closes none before the exec, so
    // the standard library's own exec-error pipe keeps working.
    unsafe {
        cmd.pre_exec(move || {
            if setsid() < 0 {
                return Err(std::io::Error::last_os_error());
            }
            // kernels before 5.11 lack the flag (EINVAL) or the call
            // (ENOSYS): the descriptors one by one
            if syscall(
                SYS_CLOSE_RANGE,
                3 as std::ffi::c_uint,
                !0 as std::ffi::c_uint,
                CLOSE_RANGE_CLOEXEC,
            ) != 0
            {
                for fd in 3..=highest {
                    let flags = fcntl(fd, F_GETFD);
                    if flags >= 0 {
                        fcntl(fd, F_SETFD, flags | FD_CLOEXEC);
                    }
                }
            }
            Ok(())
        });
    }
}

pub fn run(ctx: &Context, args: EditArgs) -> Result<Output> {
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
                .ok_or_else(|| Error::user("no active case; name one: `seldon edit C-YYYY-NNN`"))?;
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
        id if is_decision_id(id) => logbook
            .decision_file(id)?
            .ok_or_else(|| Error::user(format!("unknown decision {id}")))?,
        other => {
            return Err(Error::user(format!(
                "cannot edit `{other}`: use case, journal, ledger, status, logbook, C-YYYY-NNN or ADR-NNNN"
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
    watch_start(cmd)
}

/// Spawns `cmd` as the caller set it up and watches it for
/// [`LAUNCH_GRACE`] ([`launch_detached`]); never kills or reaps it later.
fn watch_start(mut cmd: Command) -> Run {
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
