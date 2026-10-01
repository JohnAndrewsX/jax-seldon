//! `seldon hook …` (SPEC-ENGINE §8; ADR-0014 §4, ADR-0017 §1 §5).
//!
//! - `hook claude-code` reads a Claude Code `PreToolUse` payload from stdin
//!   and records a mutating command as one `agent/command` event: `ts` is
//!   the command's start, `meta.command` the command line (heredoc bodies
//!   cut, redacted, at most 4096 characters), actor `agent:claude-code`,
//!   case from `.seldon/active-case`, `meta.toolUseId` and `meta.sessionId`
//!   for matching. `Bash` commands are classified by [`classify`]; `Edit`,
//!   `Write` and `MultiEdit` on a watched path become `subject: edit|write|
//!   multiedit`, `meta.command "<Tool> <~path>"`, the path redacted when it
//!   matches `[redaction] skipPaths`. `PostToolUse` writes nothing for a
//!   tool call that is already recorded; one that is not (a settings file
//!   with only the PostToolUse hook) is recorded with the time it arrives.
//! - `hook generic` reads `{"command","actor","cwd","startedAt"?}`: the
//!   same classification for any agent, called before the command runs.
//! - `hook session-start` prints the context block an agent starts with.
//! - `hook session-stop` writes the journal stub, runs `capture --all` and
//!   commits.
//! - `hook install claude-code` merges these hooks into the logbook's
//!   `.claude/settings.json`.
//!
//! Every hook an agent calls is silent and exits 0, whatever happens: a
//! failure goes to stderr and never blocks the agent ([`run_agent_hook`]).
//! Only `session-start` prints, and only its context block. The hooks never
//! read a command's output or a file's content, only the command line and
//! the path (SPEC-ENGINE §7).

use std::fmt::Write as _;
use std::io::{IsTerminal as _, Read as _};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use clap::{Args, Subcommand};
use serde::Deserialize;
use serde_json::{Value, json};

use super::capture::{self, CaptureArgs};
use super::event::{clip, parse_person};
use super::{Context, Output, autocommit};
use crate::attribution::{home_path, normalise};
use crate::collectors::config::SkipPaths;
use crate::config::{Config, Dirs};
use crate::error::{Error, Result};
use crate::ledger::Ledger;
use crate::logbook::cases::{self, CaseFile};
use crate::logbook::lock::{self, Lock};
use crate::logbook::{Logbook, journal};
use crate::model::event::{DETAIL_MAX, Event, Kind, Meta, Source, Zone, zone_for};
use crate::model::is_agent;
use crate::pkgcmd::{ShellLine, parse_command, parse_shell};
use crate::redact::{REDACTED, Redactor};

/// The actor of `hook claude-code`.
pub const CLAUDE_CODE: &str = "agent:claude-code";

/// How long a hook waits for the state lock before it gives up (a capture
/// may hold it for a moment).
const LOCK_PATIENCE: Duration = Duration::from_secs(2);

/// `seldon hook <command>`.
#[derive(Debug, Clone, Args)]
pub struct HookArgs {
    #[command(subcommand)]
    pub command: HookCommand,
}

#[derive(Debug, Clone, Subcommand)]
pub enum HookCommand {
    /// Merge Seldon's hooks into an agent harness's settings
    Install {
        /// The harness
        #[arg(value_parser = ["claude-code"])]
        harness: String,
        /// Settings file (default: <logbook>/.claude/settings.json)
        #[arg(long, value_name = "PATH")]
        settings: Option<PathBuf>,
    },
    /// Record a Claude Code tool call (hook payload on stdin; silent, exit 0)
    ClaudeCode,
    /// Record any agent's command ({"command","actor","cwd","startedAt"?} on stdin)
    Generic,
    /// Print the context block an agent session starts with
    SessionStart,
    /// End a session: journal stub, capture, commit (silent, exit 0)
    SessionStop {
        /// Who ends the session
        #[arg(long, value_name = "A", default_value = CLAUDE_CODE, value_parser = parse_person)]
        actor: String,
    },
}

impl HookCommand {
    /// Hooks an agent harness calls: they never fail ([`run_agent_hook`]).
    pub fn is_agent_hook(&self) -> bool {
        !matches!(self, HookCommand::Install { .. })
    }
}

/// `seldon hook install …` (the only hook that reports like a command).
pub fn run(ctx: &Context, args: HookArgs) -> Result<Output> {
    match args.command {
        HookCommand::Install { settings, .. } => install(ctx, settings),
        _ => Err(Error::user("this hook is run by an agent harness")),
    }
}

/// Runs a hook an agent calls. Prints only what the hook prints on success
/// (session-start's block); every error goes to stderr. The caller exits 0.
pub fn run_agent_hook(context: impl FnOnce() -> Result<Context>, command: HookCommand) {
    let stdin = read_stdin();
    let result = context().and_then(|ctx| match command {
        HookCommand::ClaudeCode => claude_code(&ctx, &stdin),
        HookCommand::Generic => generic(&ctx, &stdin),
        HookCommand::SessionStart => session_start(&ctx).map(|block| print!("{block}")),
        HookCommand::SessionStop { actor } => session_stop(&ctx, &actor, &stdin),
        HookCommand::Install { .. } => Ok(()),
    });
    if let Err(e) = result {
        eprintln!("seldon hook: {e}");
    }
}

/// All of stdin, or nothing when it is a terminal (a hook run by hand).
fn read_stdin() -> String {
    let mut stdin = std::io::stdin();
    if stdin.is_terminal() {
        return String::new();
    }
    let mut bytes = Vec::new();
    if let Err(e) = stdin.read_to_end(&mut bytes) {
        eprintln!("seldon hook: cannot read stdin: {e}");
    }
    String::from_utf8_lossy(&bytes).into_owned()
}

// ---------------------------------------------------------------------------
// Classification
// ---------------------------------------------------------------------------

/// What a mutating command records: the program word and the zone of what
/// it would change (ADR-0014 §2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mutation {
    pub subject: String,
    pub zone: Option<Zone>,
}

/// Where commands change the system: the watched config paths (plus
/// `~/.config/systemd`, red by ADR-0014 §2), the XDG config home and the
/// logbook (for `git`).
#[derive(Debug, Clone)]
pub struct Scope {
    pub home: PathBuf,
    pub config_home: PathBuf,
    pub watched: Vec<PathBuf>,
    pub logbook: PathBuf,
}

impl Scope {
    pub fn new(dirs: &Dirs, config: &Config, logbook: &Path) -> Self {
        let mut watched: Vec<PathBuf> = config
            .watch_paths
            .iter()
            .map(|p| normalise(Path::new(&home_path(p, &dirs.home))))
            .collect();
        watched.push(dirs.xdg_config_home.join("systemd"));
        Scope {
            home: dirs.home.clone(),
            config_home: normalise(&dirs.xdg_config_home),
            watched,
            logbook: normalise(logbook),
        }
    }

    /// A path word as an absolute path: `~`/`$HOME` forms under the home
    /// directory, relative words against `cwd`.
    pub fn resolve(&self, word: &str, cwd: &Path) -> PathBuf {
        let home_form = word == "~"
            || ["~/", "$HOME", "${HOME}"]
                .iter()
                .any(|p| word.starts_with(p));
        if home_form || word.starts_with('/') {
            normalise(Path::new(&home_path(word, &self.home)))
        } else {
            normalise(&cwd.join(word))
        }
    }

    pub fn is_watched(&self, path: &Path) -> bool {
        self.watched.iter().any(|w| path.starts_with(w))
    }

    /// The path as `~/…` when it is under the home directory.
    pub fn display(&self, path: &Path) -> String {
        match path.strip_prefix(&self.home) {
            Ok(rest) if rest.as_os_str().is_empty() => "~".into(),
            Ok(rest) => format!("~/{}", rest.display()),
            Err(_) => path.display().to_string(),
        }
    }

    /// The zone of a change to the config file or directory `path` (the
    /// trailing `/` makes `~/.config/systemd` itself red, like its files).
    fn config_zone(&self, path: &Path) -> Option<Zone> {
        zone_for(Source::Config, &format!("{}/", self.display(path)))
    }
}

/// How much a zone weighs when a line has several mutating commands.
fn weight(zone: Option<Zone>) -> u8 {
    match zone {
        None => 0,
        Some(Zone::Green) => 1,
        Some(Zone::Yellow) => 2,
        Some(Zone::Red) => 3,
    }
}

/// Whether `line`, run in `cwd`, changes the system (SPEC-ENGINE §8), and
/// what it records: the first of its most severe mutating commands. A `cd`
/// changes the directory for the commands after it.
pub fn classify(line: &ShellLine, scope: &Scope, cwd: &Path) -> Option<Mutation> {
    let mut cwd = cwd.to_path_buf();
    let mut found: Option<Mutation> = None;
    for segment in &line.segments {
        let argv = segment.argv();
        let mutation = classify_segment(argv, &segment.writes, scope, &mut cwd);
        if let Some(m) = mutation
            && found
                .as_ref()
                .is_none_or(|f| weight(m.zone) > weight(f.zone))
        {
            found = Some(m);
        }
    }
    found
}

/// One simple command: pacman-like programs (every mutating operation is
/// red), `omarchy` routes, `systemctl` unit changes, file writers into a
/// watched path, `git` in `~/.config` or the logbook, and any redirection
/// into a watched path. Updates `cwd` on `cd`.
fn classify_segment(
    argv: &[String],
    writes: &[String],
    scope: &Scope,
    cwd: &mut PathBuf,
) -> Option<Mutation> {
    let word = argv.first().map(String::as_str).unwrap_or("");
    let program = word.rsplit('/').next().unwrap_or(word);
    let args = argv.get(1..).unwrap_or_default();
    let written = writes
        .iter()
        .map(|w| scope.resolve(w, cwd))
        .find(|p| scope.is_watched(p));
    if program == "cd" {
        *cwd = match args.iter().find(|a| !a.starts_with('-')) {
            Some(dir) => scope.resolve(dir, cwd),
            None => scope.home.clone(),
        };
    }
    let mutation = |zone| {
        Some(Mutation {
            subject: program.to_string(),
            zone,
        })
    };
    let cwd: &Path = cwd;
    let changes_config = |paths: Vec<&str>| {
        paths
            .into_iter()
            .map(|p| scope.resolve(p, cwd))
            .filter(|p| scope.is_watched(p))
            .map(|p| scope.config_zone(&p))
            .max_by_key(|z| weight(*z))
    };
    let argv_str: Vec<&str> = argv.iter().map(String::as_str).collect();
    let found = if let Some(cmd) = parse_command(&argv_str) {
        if cmd.is_mutating() {
            mutation(Some(Zone::Red))
        } else {
            None
        }
    } else {
        match program {
            "omarchy" => omarchy(&route(args)).and_then(mutation),
            p if p.starts_with("omarchy-") => {
                let mut words: Vec<&str> = p["omarchy-".len()..].split('-').collect();
                words.extend(route(args));
                omarchy(&words).and_then(mutation)
            }
            "systemctl" => args
                .iter()
                .find(|a| !a.starts_with('-'))
                .filter(|verb| SYSTEMCTL_VERBS.contains(&verb.as_str()))
                .and_then(|_| mutation(Some(Zone::Red))),
            "cp" | "mv" | "install" | "ln" => copy_target(args)
                .and_then(|t| changes_config(vec![t]))
                .and_then(mutation),
            "tee" => changes_config(operands(args)).and_then(mutation),
            "sed" => sed_in_place_files(args)
                .and_then(changes_config)
                .and_then(mutation),
            "rm" | "rmdir" | "unlink" | "truncate" => operands(args)
                .into_iter()
                .map(|p| scope.resolve(p, cwd))
                .flat_map(|p| {
                    // the path itself, or the watched paths it removes with it
                    let under = scope.watched.iter().filter(|w| w.starts_with(&p)).cloned();
                    std::iter::once(p.clone())
                        .filter(|p| scope.is_watched(p))
                        .chain(under)
                        .collect::<Vec<_>>()
                })
                .map(|p| scope.config_zone(&p))
                .max_by_key(|z| weight(*z))
                .and_then(mutation),
            "git" => git(args, scope, cwd).and_then(mutation),
            _ => None,
        }
    };
    found.or_else(|| {
        written.map(|p| Mutation {
            subject: if program.is_empty() { "sh" } else { program }.to_string(),
            zone: scope.config_zone(&p),
        })
    })
}

/// `systemctl` verbs that change units (SPEC-ENGINE §8).
const SYSTEMCTL_VERBS: [&str; 6] = ["enable", "disable", "start", "stop", "mask", "unmask"];

/// `git` sub-commands that change a repository or a remote.
const GIT_MUTATING: [&str; 20] = [
    "add",
    "am",
    "apply",
    "checkout",
    "cherry-pick",
    "clean",
    "clone",
    "commit",
    "init",
    "merge",
    "mv",
    "pull",
    "push",
    "rebase",
    "reset",
    "restore",
    "revert",
    "rm",
    "stash",
    "switch",
];

/// The non-option words of an `omarchy` command line (its route and
/// arguments).
fn route(args: &[String]) -> Vec<&str> {
    args.iter()
        .map(String::as_str)
        .filter(|a| !a.starts_with('-'))
        .collect()
}

/// The zone of an `omarchy <group> <verb> …` route that changes the system
/// (SPEC-ENGINE §8: `pkg`, `plugin`, `theme`, `update`, `install`; also
/// `remove` and `reinstall`), or `None` for queries (`list`, `current`,
/// `update available`, `pkg present`, …).
fn omarchy(route: &[&str]) -> Option<Option<Zone>> {
    let (group, rest) = route.split_first()?;
    let second = rest.first().copied();
    let third = rest.get(1).copied();
    let red = Some(Some(Zone::Red));
    let yellow = Some(Some(Zone::Yellow));
    match *group {
        "update" => match second {
            None | Some("system" | "aur" | "keyring" | "firmware" | "orphan" | "pkg" | "mise") => {
                red
            }
            _ => None,
        },
        "pkg" => match (second, third) {
            (Some("add" | "install" | "drop" | "remove"), _)
            | (Some("aur"), Some("add" | "install")) => red,
            _ => None,
        },
        "install" | "remove" if second.is_some() => red,
        "reinstall" => red,
        "plugin" => match second {
            Some("add" | "clone" | "enable" | "disable" | "remove" | "update") => yellow,
            _ => None,
        },
        "theme" => match second {
            Some("set" | "install" | "remove" | "update") => yellow,
            _ => None,
        },
        _ => None,
    }
}

/// Options of `cp`, `mv`, `install` and `ln` that take the next word.
const COPY_WITH_VALUE: [&str; 8] = [
    "-m", "-o", "-g", "-S", "--mode", "--owner", "--group", "--suffix",
];

/// The destination of `cp|mv|install|ln`: `-t DIR`, `--target-directory`,
/// else the last operand when there are at least two.
fn copy_target(args: &[String]) -> Option<&str> {
    let mut operands = Vec::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        if a == "--" {
            operands.extend(it.by_ref().map(String::as_str));
            break;
        }
        if a == "-t" || a == "--target-directory" {
            return it.next().map(String::as_str);
        }
        if let Some(dir) = a.strip_prefix("--target-directory=") {
            return Some(dir);
        }
        if a.starts_with('-') && a.len() > 1 {
            if COPY_WITH_VALUE.contains(&a.as_str()) {
                it.next();
            }
            continue;
        }
        operands.push(a.as_str());
    }
    (operands.len() >= 2)
        .then(|| operands.last().copied())
        .flatten()
}

/// Words that are not options (everything after `--`).
fn operands(args: &[String]) -> Vec<&str> {
    let mut out = Vec::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        if a == "--" {
            out.extend(it.by_ref().map(String::as_str));
            break;
        }
        if !a.starts_with('-') || a == "-" {
            out.push(a.as_str());
        }
    }
    out
}

/// The files `sed` edits in place, or `None` without `-i`/`--in-place`.
fn sed_in_place_files(args: &[String]) -> Option<Vec<&str>> {
    let mut in_place = false;
    let mut script_given = false;
    let mut words = Vec::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        if a == "--" {
            words.extend(it.by_ref().map(String::as_str));
            break;
        }
        if let Some(long) = a.strip_prefix("--") {
            let name = long.split('=').next().unwrap_or(long);
            match name {
                "in-place" => in_place = true,
                "expression" | "file" => {
                    script_given = true;
                    if !long.contains('=') {
                        it.next();
                    }
                }
                "line-length" if !long.contains('=') => {
                    it.next();
                }
                _ => {}
            }
        } else if let Some(cluster) = a.strip_prefix('-').filter(|c| !c.is_empty()) {
            for (i, c) in cluster.char_indices() {
                match c {
                    // the rest of the cluster is the backup suffix
                    'i' => {
                        in_place = true;
                        break;
                    }
                    'e' | 'f' | 'l' => {
                        script_given |= c != 'l';
                        if i + 1 == cluster.len() {
                            it.next();
                        }
                        break;
                    }
                    _ => {}
                }
            }
        } else {
            words.push(a.as_str());
        }
    }
    if !in_place {
        return None;
    }
    if !script_given && !words.is_empty() {
        words.remove(0);
    }
    Some(words)
}

/// The zone of a mutating `git` command in `~/.config` (by the config zone
/// of its directory) or in the logbook (no zone), else `None`.
fn git(args: &[String], scope: &Scope, cwd: &Path) -> Option<Option<Zone>> {
    let mut dir = cwd.to_path_buf();
    let mut it = args.iter();
    let sub = loop {
        let a = it.next()?;
        match a.as_str() {
            "-C" => dir = scope.resolve(it.next()?, &dir),
            "--work-tree" | "--git-dir" => {
                let d = scope.resolve(it.next()?, &dir);
                dir = if a == "--git-dir" {
                    parent_of_git(d)
                } else {
                    d
                };
            }
            "-c" | "--namespace" | "--exec-path" | "--config-env" => {
                it.next();
            }
            _ if a.starts_with("--work-tree=") => {
                dir = scope.resolve(&a["--work-tree=".len()..], &dir);
            }
            _ if a.starts_with("--git-dir=") => {
                dir = parent_of_git(scope.resolve(&a["--git-dir=".len()..], &dir));
            }
            _ if a.starts_with('-') => {}
            _ => break a,
        }
    };
    if !GIT_MUTATING.contains(&sub.as_str()) {
        return None;
    }
    if dir.starts_with(&scope.config_home) {
        Some(scope.config_zone(&dir))
    } else if dir.starts_with(&scope.logbook) {
        Some(None)
    } else {
        None
    }
}

/// `…/repo/.git` → `…/repo`.
fn parent_of_git(dir: PathBuf) -> PathBuf {
    match dir.file_name() {
        Some(n) if n == ".git" => dir.parent().map(Path::to_path_buf).unwrap_or(dir),
        _ => dir,
    }
}

// ---------------------------------------------------------------------------
// Recording
// ---------------------------------------------------------------------------

/// The fields of a Claude Code hook payload Seldon reads. `tool_response`
/// (the command's output) is never read.
#[derive(Debug, Default, Deserialize)]
struct ToolPayload {
    #[serde(default)]
    session_id: Option<String>,
    #[serde(default)]
    cwd: Option<String>,
    #[serde(default)]
    hook_event_name: Option<String>,
    #[serde(default)]
    tool_name: Option<String>,
    #[serde(default)]
    tool_use_id: Option<String>,
    #[serde(default)]
    tool_input: Value,
}

/// `seldon hook generic` stdin.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GenericPayload {
    command: String,
    actor: String,
    #[serde(default)]
    cwd: Option<String>,
    #[serde(default)]
    started_at: Option<String>,
}

/// One command to record, before the event is built.
struct Record {
    mutation: Mutation,
    command: String,
}

/// The config, the logbook and its scope, for a hook that records.
struct Setup {
    config: Config,
    logbook: Logbook,
    scope: Scope,
}

fn setup(ctx: &Context) -> Result<Setup> {
    let (config, logbook) = ctx.open_logbook()?;
    let scope = Scope::new(&ctx.dirs, &config, &logbook.root);
    Ok(Setup {
        config,
        logbook,
        scope,
    })
}

/// The directory a payload's `cwd` names, else the process's.
fn working_dir(cwd: Option<&str>, dirs: &Dirs) -> PathBuf {
    cwd.filter(|c| c.starts_with('/'))
        .map(PathBuf::from)
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| dirs.home.clone())
}

fn claude_code(ctx: &Context, stdin: &str) -> Result<()> {
    let payload: ToolPayload = serde_json::from_str(stdin)
        .map_err(|e| Error::user(format!("stdin is not a Claude Code hook payload: {e}")))?;
    let post = match payload.hook_event_name.as_deref() {
        Some("PreToolUse") => false,
        Some("PostToolUse") => true,
        _ => return Ok(()),
    };
    let tool = payload.tool_name.as_deref().unwrap_or("");
    let setup = setup(ctx)?;
    let cwd = working_dir(payload.cwd.as_deref(), &ctx.dirs);
    let records = match tool {
        "Bash" => {
            let Some(command) = payload.tool_input.get("command").and_then(Value::as_str) else {
                return Ok(());
            };
            bash_record(command, &setup.scope, &cwd)
                .into_iter()
                .collect::<Vec<_>>()
        }
        "Edit" | "Write" | "MultiEdit" => {
            edit_records(ctx, &setup, tool, &payload.tool_input, &cwd)
        }
        _ => Vec::new(),
    };
    if records.is_empty() {
        return Ok(());
    }
    let ledger = Ledger::new(
        &setup.logbook,
        Redactor::with_patterns(&setup.config.redaction.patterns)?,
    );
    if post
        && let Some(id) = &payload.tool_use_id
        && already_recorded(&ledger, ctx, id)?
    {
        return Ok(());
    }
    let mut extra = Vec::new();
    if let Some(id) = payload.tool_use_id.filter(|s| !s.is_empty()) {
        extra.push(("toolUseId", id));
    }
    if let Some(id) = payload.session_id.filter(|s| !s.is_empty()) {
        extra.push(("sessionId", id));
    }
    record(ctx, &setup, ledger, CLAUDE_CODE, ctx.now, records, &extra)
}

fn generic(ctx: &Context, stdin: &str) -> Result<()> {
    let payload: GenericPayload = serde_json::from_str(stdin).map_err(|e| {
        Error::user(format!(
            "stdin is not {{\"command\",\"actor\",\"cwd\",\"startedAt\"?}}: {e}"
        ))
    })?;
    let actor = parse_person(&payload.actor).map_err(Error::user)?;
    let ts = match payload.started_at.as_deref() {
        Some(s) => chrono::DateTime::parse_from_rfc3339(s)
            .map_err(|e| Error::user(format!("startedAt `{s}` is not RFC 3339: {e}")))?,
        None => ctx.now,
    };
    let setup = setup(ctx)?;
    let cwd = working_dir(payload.cwd.as_deref(), &ctx.dirs);
    let Some(rec) = bash_record(&payload.command, &setup.scope, &cwd) else {
        return Ok(());
    };
    let ledger = Ledger::new(
        &setup.logbook,
        Redactor::with_patterns(&setup.config.redaction.patterns)?,
    );
    record(ctx, &setup, ledger, &actor, ts, vec![rec], &[])
}

/// A shell command line as a record, if it is mutating.
fn bash_record(command: &str, scope: &Scope, cwd: &Path) -> Option<Record> {
    let line = parse_shell(command);
    let mutation = classify(&line, scope, cwd)?;
    Some(Record {
        mutation,
        command: line.text,
    })
}

/// `Edit`/`Write`/`MultiEdit` on watched paths (ADR-0014 §4): one record
/// per path. Only the path is read from `tool_input`, never the content.
fn edit_records(
    ctx: &Context,
    setup: &Setup,
    tool: &str,
    input: &Value,
    cwd: &Path,
) -> Vec<Record> {
    let mut paths: Vec<&str> = input
        .get("file_path")
        .and_then(Value::as_str)
        .into_iter()
        .collect();
    // defensively, per-file edit lists that carry their own path
    for key in ["edits", "file_edits"] {
        if let Some(list) = input.get(key).and_then(Value::as_array) {
            paths.extend(
                list.iter()
                    .filter_map(|e| e.get("file_path").and_then(Value::as_str)),
            );
        }
    }
    paths.sort_unstable();
    paths.dedup();
    let skip = SkipPaths::new(&ctx.dirs.home, &setup.config.redaction.skip_paths);
    paths
        .into_iter()
        .map(|p| setup.scope.resolve(p, cwd))
        .filter(|p| setup.scope.is_watched(p))
        .map(|p| {
            let shown = if skip.matches(&p) {
                REDACTED.to_string()
            } else {
                setup.scope.display(&p)
            };
            Record {
                mutation: Mutation {
                    subject: tool.to_lowercase(),
                    zone: setup.scope.config_zone(&p),
                },
                command: format!("{tool} {shown}"),
            }
        })
        .collect()
}

/// Whether the ledger already holds an event for this tool call (a
/// PostToolUse after its PreToolUse).
fn already_recorded(ledger: &Ledger, ctx: &Context, tool_use_id: &str) -> Result<bool> {
    let since = ctx.now - chrono::Duration::days(1);
    Ok(ledger
        .read_range(since, ctx.now + chrono::Duration::days(1))?
        .iter()
        .any(|e| e.meta.extra.get("toolUseId").and_then(Value::as_str) == Some(tool_use_id)))
}

/// Takes the state lock, waiting up to [`LOCK_PATIENCE`] for another
/// writer (a hook must not lose a command to a running capture).
fn lock_patiently(ctx: &Context) -> Result<Lock> {
    let start = Instant::now();
    loop {
        match lock::acquire(&ctx.dirs.lock_file()) {
            Err(Error::LockHeld(_)) if start.elapsed() < LOCK_PATIENCE => {
                std::thread::sleep(Duration::from_millis(20));
            }
            other => return other,
        }
    }
}

/// Appends one `agent/command` event per record and attaches them to the
/// active case. The command line is redacted before it is cut, so a cut
/// never leaves half a secret.
fn record(
    ctx: &Context,
    setup: &Setup,
    ledger: Ledger,
    actor: &str,
    ts: chrono::DateTime<chrono::FixedOffset>,
    records: Vec<Record>,
    extra: &[(&str, String)],
) -> Result<()> {
    let lock = lock_patiently(ctx)?;
    let mut case_file: Option<CaseFile> = match cases::active_case(&setup.logbook) {
        Some(id) => match cases::find(&setup.logbook, &id) {
            Ok(file) => Some(file),
            Err(e) => {
                eprintln!("seldon hook: active case {id} ignored: {e}");
                None
            }
        },
        None => None,
    };
    let events: Vec<Event> = records
        .into_iter()
        .map(|r| {
            let command = clip(&ledger.redactor().redact(&r.command), DETAIL_MAX);
            let mut meta = Meta {
                command: Some(command),
                ..Meta::default()
            };
            for (k, v) in extra {
                meta.extra
                    .insert((*k).to_string(), Value::String(v.clone()));
            }
            let mut e = Event::new(ts, Source::Agent, Kind::Command, r.mutation.subject)
                .actor(actor)
                .case(case_file.as_ref().map(|f| f.case.id.clone()))
                .meta(meta);
            e.zone = r.mutation.zone;
            e
        })
        .collect();
    let written = ledger.append(&lock, events)?;
    if let Some(file) = case_file.as_mut() {
        for e in &written {
            file.attach(&e.id.to_string(), &e.actor);
        }
        file.save(&setup.logbook)?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Sessions
// ---------------------------------------------------------------------------

/// The context block (SPEC-ENGINE §8): STATUS summary, the active case
/// with its plan steps, the last 5 journal lines, the lessons' headings.
pub fn session_start(ctx: &Context) -> Result<String> {
    let (_, logbook) = ctx.open_logbook()?;
    let mut out = String::from("# Seldon logbook context\n");

    out.push_str("\n## Status\n");
    match std::fs::read_to_string(logbook.path("STATUS.md")) {
        Ok(text) => out.push_str(&status_summary(&text)),
        Err(_) => out.push_str("No STATUS.md yet (`seldon status` writes it).\n"),
    }

    out.push_str("\n## Active case\n");
    match cases::active_case(&logbook).map(|id| cases::find(&logbook, &id)) {
        Some(Ok(file)) => out.push_str(&case_block(&file)),
        Some(Err(e)) => {
            let _ = writeln!(out, "Unreadable: {e}");
        }
        None => out.push_str("None (`seldon plan start <id>` sets one).\n"),
    }

    let today = ctx.now.date_naive().format("%Y-%m-%d").to_string();
    let journal = logbook.journal_files()?.into_iter().rfind(|p| {
        p.file_stem()
            .is_some_and(|s| *s.to_string_lossy() <= *today)
    });
    match journal {
        Some(path) => {
            let rel = cases::relative(&logbook, &path);
            let _ = writeln!(out, "\n## Journal ({rel}, last 5 lines)");
            let text = std::fs::read_to_string(&path)?;
            let body = crate::frontmatter::Document::parse(&text)
                .map(|d| d.body)
                .unwrap_or(text);
            let lines: Vec<&str> = body.lines().filter(|l| !l.trim().is_empty()).collect();
            for line in &lines[lines.len().saturating_sub(5)..] {
                let _ = writeln!(out, "{line}");
            }
        }
        None => out.push_str("\n## Journal\nNo entries yet.\n"),
    }

    out.push_str("\n## Lessons (memory/lessons.md)\n");
    let lessons = std::fs::read_to_string(logbook.path("memory/lessons.md")).unwrap_or_default();
    let headings: Vec<&str> = lessons
        .lines()
        .filter_map(|l| l.strip_prefix("## "))
        .collect();
    if headings.is_empty() {
        out.push_str("None yet.\n");
    }
    for h in headings {
        let _ = writeln!(out, "- {}", h.trim());
    }
    Ok(out)
}

/// The first section of STATUS.md: the lines between its title and the
/// second `## ` heading, without the generated header and blank lines.
fn status_summary(text: &str) -> String {
    let mut out = String::new();
    let mut sections = 0;
    for line in text.lines() {
        if line.starts_with("## ") {
            sections += 1;
            if sections > 1 {
                break;
            }
            continue;
        }
        if line.trim().is_empty() || line.starts_with("# ") || line.starts_with("<!--") {
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

/// `C-… — title (status, zone/risk, done/total steps)` and the plan's
/// checkbox lines.
fn case_block(file: &CaseFile) -> String {
    let c = &file.case;
    let (total, done) = file.steps();
    let mut out = format!(
        "{} — {} ({}, {}/{}, {done}/{total} steps)\n",
        c.id,
        c.title,
        c.status.as_str(),
        c.zone.as_str(),
        c.risk.as_str()
    );
    if let Some(range) = cases::section(&file.doc.body, "Plan") {
        for line in file.doc.body[range].lines() {
            let item = line.trim();
            if item.starts_with("- [") || item.starts_with("* [") {
                let _ = writeln!(out, "{item}");
            }
        }
    }
    out
}

/// The journal stub, `capture --all`, the index rebuild (WP-007) and the
/// commit. Each step runs even when an earlier one failed; failures are
/// reported on stderr.
fn session_stop(ctx: &Context, actor: &str, stdin: &str) -> Result<()> {
    if !is_agent(actor) {
        return Err(Error::user(format!(
            "session-stop --actor `{actor}` is not agent:<name>"
        )));
    }
    let (config, logbook) = ctx.open_logbook()?;
    let session = serde_json::from_str::<ToolPayload>(stdin)
        .ok()
        .and_then(|p| p.session_id)
        .filter(|s| !s.is_empty());

    let n = session_events(ctx, &config, &logbook, actor, session.as_deref())?;
    let case = cases::active_case(&logbook);
    let text = format!(
        "session ended; {n} {} recorded",
        if n == 1 { "event" } else { "events" }
    );
    {
        let _lock = lock_patiently(ctx)?;
        journal::append(&logbook, &ctx.now, actor, case.as_deref(), &text)?;
    }

    if let Err(e) = capture::run(
        ctx,
        CaptureArgs {
            all: true,
            ..CaptureArgs::default()
        },
    ) {
        eprintln!("seldon hook: capture: {e}");
    }
    // WP-007: rebuild index.json and STATUS.md here (`commands::status`)
    // once it lands; one call, before the commit below.
    if let super::Commit::Failed(e) =
        autocommit(ctx, &config, &logbook, &format!("session ended ({actor})"))
    {
        eprintln!("seldon hook: git: {e}");
    }
    Ok(())
}

/// Events `actor` recorded in this session: hook events carrying the
/// session id, or without one, the actor's events of today.
fn session_events(
    ctx: &Context,
    config: &Config,
    logbook: &Logbook,
    actor: &str,
    session: Option<&str>,
) -> Result<usize> {
    let ledger = Ledger::new(
        logbook,
        Redactor::with_patterns(&config.redaction.patterns)?,
    );
    let today = ctx.now.date_naive();
    Ok(ledger
        .read_range(ctx.now - chrono::Duration::days(2), ctx.now)?
        .iter()
        .filter(|e| e.actor == actor)
        .filter(|e| match session {
            Some(id) => e.meta.extra.get("sessionId").and_then(Value::as_str) == Some(id),
            None => e.ts.date_naive() == today,
        })
        .count())
}

// ---------------------------------------------------------------------------
// Install
// ---------------------------------------------------------------------------

/// The hooks `hook install claude-code` writes: event, matcher, command,
/// timeout in seconds. `SessionEnd`, not `Stop`: Claude Code runs `Stop`
/// after every reply, `SessionEnd` once when the session ends.
pub const CLAUDE_HOOKS: [(&str, Option<&str>, &str, u64); 3] = [
    (
        "PreToolUse",
        Some("Bash|Edit|Write|MultiEdit"),
        "seldon hook claude-code",
        10,
    ),
    ("SessionStart", None, "seldon hook session-start", 10),
    ("SessionEnd", None, "seldon hook session-stop", 120),
];

/// `seldon hook install claude-code [--settings PATH]`: adds each of
/// [`CLAUDE_HOOKS`] that is not there yet; everything else in the file is
/// kept. A file that is not a JSON object is refused, never overwritten.
fn install(ctx: &Context, settings: Option<PathBuf>) -> Result<Output> {
    let (path, logbook) = match settings {
        Some(p) => (ctx.dirs.expand(&p.to_string_lossy()), None),
        None => {
            let (config, logbook) = ctx.open_logbook()?;
            (
                logbook.path(".claude/settings.json"),
                Some((config, logbook)),
            )
        }
    };
    let shown = ctx.dirs.display(&path);
    let text = match std::fs::read_to_string(&path) {
        Ok(t) if t.trim().is_empty() => "{}".to_string(),
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => "{}".to_string(),
        Err(e) => {
            return Err(anyhow::Error::new(e)
                .context(format!("cannot read {shown}"))
                .into());
        }
    };
    let refuse = |why: String| Error::user(format!("{shown}: {why}; nothing was changed"));
    let mut root: Value =
        serde_json::from_str(&text).map_err(|e| refuse(format!("not valid JSON ({e})")))?;
    let hooks = root
        .as_object_mut()
        .ok_or_else(|| refuse("not a JSON object".into()))?
        .entry("hooks")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .ok_or_else(|| refuse("`hooks` is not an object".into()))?;

    let mut added = Vec::new();
    let mut present = Vec::new();
    for (event, matcher, command, timeout) in CLAUDE_HOOKS {
        let groups = hooks
            .entry(event)
            .or_insert_with(|| json!([]))
            .as_array_mut()
            .ok_or_else(|| refuse(format!("`hooks.{event}` is not a list")))?;
        let label = match matcher {
            Some(m) => format!("{event} ({m}): {command}"),
            None => format!("{event}: {command}"),
        };
        let has = groups.iter().any(|g| {
            g.get("matcher")
                .and_then(Value::as_str)
                .filter(|m| !m.is_empty())
                == matcher
                && g.get("hooks")
                    .and_then(Value::as_array)
                    .is_some_and(|hs| hs.iter().any(|h| h.get("command") == Some(&json!(command))))
        });
        if has {
            present.push(label);
            continue;
        }
        let mut group = json!({
            "hooks": [{ "type": "command", "command": command, "timeout": timeout }]
        });
        if let Some(m) = matcher {
            group["matcher"] = json!(m);
        }
        groups.push(group);
        added.push(label);
    }

    let mut commit = None;
    if !added.is_empty() {
        let mut text = serde_json::to_string_pretty(&root).map_err(anyhow::Error::from)?;
        text.push('\n');
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        crate::sys::write_atomic(&path, text.as_bytes())?;
        if let Some((config, logbook)) = &logbook {
            commit = Some(autocommit(ctx, config, logbook, "hook install claude-code"));
        }
    }

    let mut human = if added.is_empty() {
        format!("{shown}: the Seldon hooks are already installed.")
    } else {
        format!("{shown}: installed the Seldon hooks.")
    };
    for a in &added {
        let _ = write!(human, "\n  added    {a}");
    }
    for p in &present {
        let _ = write!(human, "\n  present  {p}");
    }
    if let Some(c) = &commit {
        human.push_str(&c.human());
    }
    Ok(Output::ok(
        human,
        json!({
            "settings": path,
            "added": added,
            "present": present,
            "git": commit.map_or(Value::Null, |c| c.json()),
        }),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scope() -> Scope {
        let dirs = Dirs {
            home: "/home/user".into(),
            xdg_config_home: "/home/user/.config".into(),
            state_dir: "/home/user/.local/state/seldon".into(),
        };
        Scope::new(&dirs, &Config::default(), Path::new("/home/user/Seldon"))
    }

    fn class(line: &str) -> Option<(String, Option<Zone>)> {
        classify(&parse_shell(line), &scope(), Path::new("/home/user/Seldon"))
            .map(|m| (m.subject, m.zone))
    }

    fn red(s: &str) -> Option<(String, Option<Zone>)> {
        Some((s.to_string(), Some(Zone::Red)))
    }

    fn yellow(s: &str) -> Option<(String, Option<Zone>)> {
        Some((s.to_string(), Some(Zone::Yellow)))
    }

    #[test]
    fn package_commands() {
        assert_eq!(class("yay -S --noconfirm zed"), red("yay"));
        assert_eq!(class("sudo pacman -S --noconfirm ollama"), red("pacman"));
        assert_eq!(
            class("sudo -u root /usr/bin/pacman -Rns zed"),
            red("pacman")
        );
        assert_eq!(class("yay"), red("yay"));
        for q in [
            "pacman -Qi zed | head -5",
            "pacman -Ss zed",
            "yay -Si zed",
            "pacman -T zed",
            "paru -Sc",
            // probes run nothing; `yay` alone would be a full upgrade
            "command -v yay",
            "command -v pacman",
            "type yay",
            "which pacman",
            "command -v yay >/dev/null && echo ok",
        ] {
            assert_eq!(class(q), None, "{q}");
        }
    }

    #[test]
    fn omarchy_commands() {
        assert_eq!(class("omarchy update"), red("omarchy"));
        assert_eq!(class("omarchy update -y"), red("omarchy"));
        assert_eq!(class("omarchy pkg add zed"), red("omarchy"));
        assert_eq!(class("omarchy pkg aur add foo-bin"), red("omarchy"));
        assert_eq!(class("omarchy-pkg-add zed"), red("omarchy-pkg-add"));
        assert_eq!(class("omarchy install editor zed"), red("omarchy"));
        assert_eq!(class("omarchy theme set tokyo-night"), yellow("omarchy"));
        assert_eq!(
            class("omarchy-theme-set kanagawa"),
            yellow("omarchy-theme-set")
        );
        assert_eq!(
            class("omarchy plugin add https://x/y.git"),
            yellow("omarchy")
        );
        for q in [
            "omarchy theme list",
            "omarchy theme current",
            "omarchy plugin list --json",
            "omarchy plugin catalog",
            "omarchy update available",
            "omarchy-update-available",
            "omarchy pkg present zed",
            "omarchy-version",
            "omarchy",
            "omarchy menu",
        ] {
            assert_eq!(class(q), None, "{q}");
        }
    }

    #[test]
    fn systemctl() {
        assert_eq!(
            class("systemctl --user enable --now ollama"),
            red("systemctl")
        );
        assert_eq!(class("sudo systemctl mask foo"), red("systemctl"));
        assert_eq!(class("systemctl status foo"), None);
        assert_eq!(class("systemctl --user list-units"), None);
    }

    #[test]
    fn file_writes_into_watched_paths() {
        assert_eq!(
            class("sed -i 's/^bindd = SUPER, E, .*/x/' ~/.config/hypr/bindings.conf"),
            yellow("sed")
        );
        assert_eq!(class("sed -i.bak -e s/a/b/ ~/.bashrc"), yellow("sed"));
        assert_eq!(
            class("sed --in-place=.bak s/a/b/ /home/user/.zshrc"),
            yellow("sed")
        );
        assert_eq!(
            class("sed s/a/b/ ~/.config/hypr/bindings.conf"),
            None,
            "not in place"
        );
        assert_eq!(class("sed -i s/a/b/ /tmp/x"), None, "not watched");
        assert_eq!(
            class("tee ~/.config/systemd/user/ollama.service"),
            red("tee")
        );
        assert_eq!(
            class("echo x | sudo tee -a ~/.config/hypr/a.conf"),
            yellow("tee")
        );
        assert_eq!(
            class("cp new.conf ~/.config/hypr/hyprland.conf"),
            yellow("cp")
        );
        assert_eq!(class("cp -t ~/.config/hypr a b"), yellow("cp"));
        assert_eq!(
            class("cp ~/.config/hypr/hyprland.conf /tmp/"),
            None,
            "source only"
        );
        assert_eq!(class("mv a.conf $HOME/.config/waybar/config"), yellow("mv"));
        assert_eq!(
            class("install -m 644 a ~/.config/omarchy/x"),
            yellow("install")
        );
        assert_eq!(class("ln -sf /tmp/a ~/.config/hypr/a.conf"), yellow("ln"));
        assert_eq!(class("rm ~/.config/hypr/old.conf"), yellow("rm"));
        assert_eq!(class("rm -rf ~/.config"), red("rm"), "contains systemd");
        assert_eq!(class("echo x > ~/.config/hypr/a.conf"), yellow("echo"));
        assert_eq!(
            class("cat >> ~/.bashrc <<'EOF'\nexport A=1\nEOF"),
            yellow("cat")
        );
        assert_eq!(class("echo x > /tmp/a"), None);
        // a `cd` changes what relative paths mean
        assert_eq!(
            class("cd ~/.config/hypr && sed -i s/a/b/ bindings.conf"),
            yellow("sed")
        );
        assert_eq!(class("sed -i s/a/b/ bindings.conf"), None);
        // the most severe command wins
        assert_eq!(
            class("sed -i s/a/b/ ~/.bashrc && sudo pacman -S zed"),
            red("pacman")
        );
    }

    #[test]
    fn git_in_config_or_logbook() {
        assert_eq!(
            class("AWS_ACCESS_KEY_ID=x git -C ~/.config/hypr push https://u:p@h/r.git main"),
            yellow("git")
        );
        assert_eq!(class("git -C ~/.config/hypr status --short"), None);
        assert_eq!(
            class("git commit -am x"),
            Some(("git".into(), None)),
            "cwd = logbook"
        );
        assert_eq!(class("cd /tmp/repo && git commit -am x"), None);
        assert_eq!(
            class("git --git-dir=/home/user/.config/.git add -A"),
            yellow("git")
        );
        assert_eq!(class("git log --oneline"), None);
    }

    #[test]
    fn harmless_commands() {
        for q in [
            "ls -la ~/.config/hypr",
            "cat ~/.config/hypr/bindings.conf",
            "grep -r foo ~/.config",
            "cargo test",
            "echo hello",
            "",
            "   ",
        ] {
            assert_eq!(class(q), None, "{q}");
        }
    }

    #[test]
    fn sed_files() {
        let args = |s: &str| -> Vec<String> { s.split(' ').map(String::from).collect() };
        let a = args("-i -e s/a/b/ -e s/c/d/ f1 f2");
        assert_eq!(sed_in_place_files(&a).unwrap(), [&a[5], &a[6]]);
        let a = args("-Ei s/a/b/ f1");
        assert_eq!(sed_in_place_files(&a).unwrap(), [&a[2]]);
        let a = args("-n s/a/b/p f1");
        assert_eq!(sed_in_place_files(&a), None);
    }
}
