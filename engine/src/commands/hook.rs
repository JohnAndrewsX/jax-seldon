//! `seldon hook …` (SPEC-ENGINE §8; ADR-0014 §4, ADR-0017 §1 §5).
//!
//! - `hook claude-code` reads a Claude Code `PreToolUse` payload from stdin
//!   and records a mutating command as one `agent/command` event: `ts` is
//!   the command's start, `meta.command` the command line (heredoc bodies
//!   cut, redacted, at most 4096 characters), actor `agent:claude-code`,
//!   case from `.seldon/active-case`, `meta.toolUseId` and `meta.sessionId`
//!   for matching. `Bash` commands are classified by [`classify`] (`sh -c
//!   '…'` and `eval '…'` are read as the commands inside); `Edit`, `Write`
//!   and `MultiEdit` become `subject: edit|write|multiedit`, `meta.command
//!   "<Tool> <~path>"`, the path redacted when it matches `[redaction]
//!   skipPaths`. `PostToolUse` writes nothing for a tool call that is
//!   already recorded; one that is not (a settings file with only the
//!   PostToolUse hook) is recorded with the time it arrives.
//! - Zones: what a collector tracks keeps its zone (red packages, services
//!   and updates; yellow config under `watchPaths`, plugins, themes). Any
//!   other change (a file written outside `watchPaths` and the logbook, a
//!   foreign package manager's install, `git` outside `~/.config`) is green
//!   and recorded only while a case is set (ADR-0019).
//! - `hook generic` reads `{"command","actor","cwd","startedAt"?,"case"?}`
//!   (`--case` wins over the field, either over `.seldon/active-case`):
//!   the same classification for any agent, called before the command runs.
//! - Not read yet (a follow-up): commands run by `xargs`, `find -exec` or
//!   an interpreter (`python -c`, `node -e`).
//! - `hook session-start` prints the context block an agent starts with.
//! - `hook session-stop` writes the journal stub, runs `capture --all` and
//!   commits.
//! - `hook install claude-code` merges these hooks into the logbook's
//!   `.claude/settings.json`; a settings file under a watched path is
//!   recorded as the engine's own write (SPEC-ENGINE §5 rule 7).
//!   `hook uninstall claude-code` takes exactly those hooks out again
//!   ([`unmerge_claude_hooks`]) and records its write, or the deletion of
//!   a file that is left empty, the same way.
//!
//! Every hook an agent calls is silent and exits 0, whatever happens: a
//! failure goes to stderr and never blocks the agent ([`run_agent_hook`]).
//! Only `session-start` prints, and only its context block. The hooks never
//! read a command's output or a file's content, only the command line and
//! the path (SPEC-ENGINE §7).

use std::fmt::Write as _;
use std::io::{IsTerminal as _, Read as _, Write as _};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use clap::{Args, Subcommand};
use serde::Deserialize;
use serde_json::{Value, json};

use super::capture::{self, CaptureArgs};
use super::event::{clip, parse_case_id, parse_person};
use super::{Context, Output, autocommit};
use crate::attribution::{home_path, normalise};
use crate::collectors::config::{OwnOp, SkipPaths};
use crate::config::{Config, Dirs};
use crate::error::{Error, Result};
use crate::ledger::Ledger;
use crate::logbook::cases::{self, CaseFile};
use crate::logbook::lock::{self, Lock};
use crate::logbook::{Logbook, journal};
use crate::model::event::{DETAIL_MAX, Event, Kind, Meta, Source, Zone, zone_for};
use crate::model::is_agent;
use crate::pkgcmd::{
    ShellLine, Target, parse_command, parse_shell, simple_commands, write_targets,
};
use crate::redact::{REDACTED, Redactor};

/// The actor of `hook claude-code`.
pub const CLAUDE_CODE: &str = "agent:claude-code";

/// How long a hook waits for the state lock before it gives up (a capture
/// may hold it for a moment).
const LOCK_PATIENCE: Duration = Duration::from_secs(2);

mod context;
pub use context::session_start;

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
        #[arg(long, value_name = "FILE")]
        settings: Option<PathBuf>,
    },
    /// Remove Seldon's hooks from an agent harness's settings, keeping the rest
    Uninstall {
        /// The harness
        #[arg(value_parser = ["claude-code"])]
        harness: String,
        /// Settings file (default: <logbook>/.claude/settings.json)
        #[arg(long, value_name = "FILE")]
        settings: Option<PathBuf>,
    },
    /// Record a Claude Code tool call (hook payload on stdin; silent, exit 0)
    ClaudeCode,
    /// Record any agent's command ({"command","actor","cwd","startedAt"?,"case"?} on stdin)
    Generic {
        /// The case the command belongs to (default: `.seldon/active-case`)
        #[arg(long = "case", value_name = "ID", value_parser = parse_case_id)]
        case_id: Option<String>,
    },
    /// Print the context block an agent session starts with
    SessionStart,
    /// End a session: journal stub, capture, commit (silent, exit 0)
    SessionStop {
        /// Who ends the session: human or agent:NAME
        #[arg(long, value_name = "ACTOR", default_value = CLAUDE_CODE, value_parser = parse_person)]
        actor: String,
    },
}

impl HookCommand {
    /// Hooks an agent harness calls: they never fail ([`run_agent_hook`]).
    pub fn is_agent_hook(&self) -> bool {
        !matches!(
            self,
            HookCommand::Install { .. } | HookCommand::Uninstall { .. }
        )
    }
}

/// `seldon hook install|uninstall …` (the hooks that report like a
/// command).
pub fn run(ctx: &Context, args: HookArgs) -> Result<Output> {
    match args.command {
        HookCommand::Install { settings, .. } => install(ctx, settings),
        HookCommand::Uninstall { settings, .. } => uninstall(ctx, settings),
        _ => Err(Error::user("this hook is run by an agent harness")),
    }
}

/// Runs a hook an agent calls. Prints only what the hook prints on success
/// (session-start's block, whose write errors such as a closed pipe are
/// ignored); every error goes to stderr. The caller exits 0.
///
/// A panic exits 0 too: the panic hook reports it on stderr and ends the
/// process before the release profile's `panic = "abort"` would (where
/// `catch_unwind` catches nothing).
pub fn run_agent_hook(context: impl FnOnce() -> Result<Context>, command: HookCommand) {
    std::panic::set_hook(Box::new(|info| {
        let _ = writeln!(std::io::stderr(), "seldon hook: internal error: {info}");
        std::process::exit(0);
    }));
    #[cfg(debug_assertions)]
    if std::env::var_os("SELDON_TEST_HOOK_PANIC").is_some() {
        panic!("SELDON_TEST_HOOK_PANIC is set");
    }
    let stdin = read_stdin();
    let result = context().and_then(|ctx| match command {
        HookCommand::ClaudeCode => claude_code(&ctx, &stdin),
        HookCommand::Generic { case_id } => generic(&ctx, &stdin, case_id),
        HookCommand::SessionStart => session_start(&ctx).map(|block| {
            let mut out = std::io::stdout().lock();
            let _ = out.write_all(block.as_bytes()).and_then(|()| out.flush());
        }),
        HookCommand::SessionStop { actor } => session_stop(&ctx, &actor, &stdin),
        HookCommand::Install { .. } | HookCommand::Uninstall { .. } => Ok(()),
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

/// What a mutating command records: the program word, the zone of what it
/// would change (ADR-0014 §2, ADR-0019), and whether it is recorded only
/// while a case is set (ADR-0019 §1: green commands no collector tracks).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mutation {
    pub subject: String,
    pub zone: Option<Zone>,
    pub needs_case: bool,
}

impl Mutation {
    /// The order in which a line's mutations compete: the more severe zone,
    /// then the one recorded without a case.
    fn rank(&self) -> (u8, bool) {
        (zone_weight(self.zone), !self.needs_case)
    }
}

/// How much a zone weighs when a line has several mutating commands.
fn zone_weight(zone: Option<Zone>) -> u8 {
    match zone {
        None => 0,
        Some(Zone::Green) => 1,
        Some(Zone::Yellow) => 2,
        Some(Zone::Red) => 3,
    }
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

    /// Paths whose changes are no news for the green class: devices
    /// (`2>/dev/null`) and the logbook, which Seldon itself records.
    fn is_untracked_noise(&self, path: &Path) -> bool {
        path.starts_with("/dev") || path.starts_with(&self.logbook)
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

    /// What writing `paths` amounts to: the most severe config zone when one
    /// is watched (a removal also counts the watched paths below it);
    /// otherwise green, recorded only with a case, when any path is outside
    /// the noise (ADR-0019); `None` when nothing is written.
    fn writes(&self, paths: &[(PathBuf, bool)]) -> Option<(Option<Zone>, bool)> {
        let mut watched: Option<Option<Zone>> = None;
        let mut untracked = false;
        for (p, tree) in paths {
            let mut zones: Vec<Option<Zone>> = Vec::new();
            if self.is_watched(p) {
                zones.push(self.config_zone(p));
            }
            if *tree {
                zones.extend(
                    self.watched
                        .iter()
                        .filter(|w| w.starts_with(p))
                        .map(|w| self.config_zone(w)),
                );
            }
            if zones.is_empty() {
                untracked |= !self.is_untracked_noise(p);
            }
            for z in zones {
                if watched.is_none_or(|w| zone_weight(z) > zone_weight(w)) {
                    watched = Some(z);
                }
            }
        }
        match watched {
            Some(zone) => Some((zone, false)),
            None if untracked => Some((Some(Zone::Green), true)),
            None => None,
        }
    }
}

/// Whether `line`, run in `cwd`, changes anything (SPEC-ENGINE §8,
/// ADR-0019), and what it records: the first of its most severe mutating
/// commands. `sh -c '…'` and `eval '…'` are read as the commands inside;
/// a `cd` changes the directory for the commands after it.
pub fn classify(line: &ShellLine, scope: &Scope, cwd: &Path) -> Option<Mutation> {
    let mut cwd = cwd.to_path_buf();
    let mut found: Option<Mutation> = None;
    for segment in simple_commands(line) {
        let mutation = classify_segment(segment.argv(), &segment.writes, scope, &mut cwd);
        if let Some(m) = mutation
            && found.as_ref().is_none_or(|f| m.rank() > f.rank())
        {
            found = Some(m);
        }
    }
    found
}

/// One simple command: pacman-like programs (every mutating operation is
/// red), `omarchy` routes, `systemctl` unit changes, `git` sub-commands
/// that change a repository, foreign package managers, and the files it
/// writes ([`write_targets`]). Updates `cwd` on `cd`.
fn classify_segment(
    argv: &[String],
    writes: &[String],
    scope: &Scope,
    cwd: &mut PathBuf,
) -> Option<Mutation> {
    let word = argv.first().map(String::as_str).unwrap_or("");
    let program = word.rsplit('/').next().unwrap_or(word);
    let args = argv.get(1..).unwrap_or_default();
    if program == "cd" {
        *cwd = match args.iter().find(|a| !a.starts_with('-')) {
            Some(dir) => scope.resolve(dir, cwd),
            None => scope.home.clone(),
        };
    }
    let cwd: &Path = cwd;
    let subject = if program.is_empty() { "sh" } else { program };
    let mutation = |(zone, needs_case): (Option<Zone>, bool)| Mutation {
        subject: subject.to_string(),
        zone,
        needs_case,
    };

    let argv_str: Vec<&str> = argv.iter().map(String::as_str).collect();
    let command = match parse_command(&argv_str) {
        Some(cmd) => cmd.is_mutating().then_some((Some(Zone::Red), false)),
        None => match program {
            "omarchy" => omarchy(&route(args)).map(|z| (z, false)),
            p if p.starts_with("omarchy-") => {
                let mut words: Vec<&str> = p["omarchy-".len()..].split('-').collect();
                words.extend(route(args));
                omarchy(&words).map(|z| (z, false))
            }
            "systemctl" => args
                .iter()
                .find(|a| !a.starts_with('-'))
                .filter(|verb| SYSTEMCTL_VERBS.contains(&verb.as_str()))
                .map(|_| (Some(Zone::Red), false)),
            "git" => git(args, scope, cwd),
            p if FOREIGN_PACKAGE_MANAGERS.contains(&p) => {
                foreign_install(p, args).then_some((Some(Zone::Green), true))
            }
            _ => None,
        },
    };
    let paths: Vec<(PathBuf, bool)> = write_targets(argv, writes)
        .iter()
        .map(|t| (scope.resolve(t.word(), cwd), matches!(t, Target::Tree(_))))
        .collect();
    let files = scope.writes(&paths);
    [command, files]
        .into_iter()
        .flatten()
        .map(mutation)
        .reduce(|a, b| if b.rank() > a.rank() { b } else { a })
}

/// `systemctl` verbs that change units (SPEC-ENGINE §8).
const SYSTEMCTL_VERBS: [&str; 6] = ["enable", "disable", "start", "stop", "mask", "unmask"];

/// Package managers no collector tracks (ADR-0019 §2): their installs are
/// green, recorded only with a case.
const FOREIGN_PACKAGE_MANAGERS: [&str; 10] = [
    "npm", "pnpm", "yarn", "pip", "pip3", "pipx", "uv", "cargo", "go", "bun",
];

/// Sub-commands of [`FOREIGN_PACKAGE_MANAGERS`] that change what is
/// installed.
const FOREIGN_VERBS: [&str; 12] = [
    "install",
    "i",
    "add",
    "remove",
    "rm",
    "uninstall",
    "un",
    "update",
    "upgrade",
    "up",
    "get",
    "ci",
];

/// Whether a foreign package manager's arguments install or remove
/// something: the first operand is a verb of [`FOREIGN_VERBS`] (after `pip`
/// or `tool` for `uv pip install`, `uv tool install`); bare `yarn` installs.
fn foreign_install(program: &str, args: &[String]) -> bool {
    let words = route(args);
    match words.as_slice() {
        [] => program == "yarn",
        ["pip" | "tool", verb, ..] if program == "uv" => FOREIGN_VERBS.contains(verb),
        [verb, ..] => FOREIGN_VERBS.contains(verb),
    }
}

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

/// The non-option words of a command line (its route and arguments).
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

/// A `git` sub-command that changes a repository: in `~/.config` the config
/// zone of its directory; in the logbook green and always recorded
/// (SPEC-ENGINE §8); anywhere else green, only with a case (ADR-0019).
fn git(args: &[String], scope: &Scope, cwd: &Path) -> Option<(Option<Zone>, bool)> {
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
    Some(if dir.starts_with(&scope.config_home) {
        (scope.config_zone(&dir), false)
    } else if dir.starts_with(&scope.logbook) {
        (Some(Zone::Green), false)
    } else {
        (Some(Zone::Green), true)
    })
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
    /// The command's case (`--case` wins; default `.seldon/active-case`).
    #[serde(default)]
    case: Option<String>,
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
    let entry = Entry {
        actor: CLAUDE_CODE,
        ts: ctx.now,
        case: None,
    };
    record(ctx, &setup, ledger, entry, records, &extra)
}

fn generic(ctx: &Context, stdin: &str, case_flag: Option<String>) -> Result<()> {
    let payload: GenericPayload = serde_json::from_str(stdin).map_err(|e| {
        Error::user(format!(
            "stdin is not {{\"command\",\"actor\",\"cwd\",\"startedAt\"?,\"case\"?}}: {e}"
        ))
    })?;
    let actor = parse_person(&payload.actor).map_err(Error::user)?;
    let case = match case_flag.or(payload.case) {
        Some(id) => Some(parse_case_id(&id).map_err(Error::user)?),
        None => None,
    };
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
    let entry = Entry {
        actor: &actor,
        ts,
        case,
    };
    record(ctx, &setup, ledger, entry, vec![rec], &[])
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

/// `Edit`/`Write`/`MultiEdit`: one record per path. A watched path takes
/// its config zone (ADR-0014 §4); any other path outside the logbook is
/// green and recorded only with a case (ADR-0019). Only the path is read
/// from `tool_input`, never the content.
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
        .filter_map(|p| {
            let (zone, needs_case) = setup.scope.writes(&[(p.clone(), false)])?;
            let shown = if skip.matches(&p) {
                REDACTED.to_string()
            } else {
                setup.scope.display(&p)
            };
            Some(Record {
                mutation: Mutation {
                    subject: tool.to_lowercase(),
                    zone,
                    needs_case,
                },
                command: format!("{tool} {shown}"),
            })
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

/// Who recorded the commands, when they started, and their case if the
/// caller named one (else `.seldon/active-case`).
struct Entry<'a> {
    actor: &'a str,
    ts: chrono::DateTime<chrono::FixedOffset>,
    case: Option<String>,
}

/// Appends one `agent/command` event per record and attaches them to the
/// case. Records that need a case (ADR-0019, green) are dropped without
/// one. The command line is redacted before it is cut, so a cut never
/// leaves half a secret.
fn record(
    ctx: &Context,
    setup: &Setup,
    ledger: Ledger,
    entry: Entry,
    records: Vec<Record>,
    extra: &[(&str, String)],
) -> Result<()> {
    let (id, origin) = match entry.case {
        Some(id) => (Some(id), "case"),
        None => (cases::active_case(&setup.logbook), "active case"),
    };
    let mut case_file: Option<CaseFile> =
        id.and_then(|id| match cases::find(&setup.logbook, &id) {
            Ok(file) => Some(file),
            Err(e) => {
                eprintln!("seldon hook: {origin} {id} ignored: {e}");
                None
            }
        });
    let records: Vec<Record> = records
        .into_iter()
        .filter(|r| case_file.is_some() || !r.mutation.needs_case)
        .collect();
    if records.is_empty() {
        return Ok(());
    }
    let (actor, ts) = (entry.actor, entry.ts);
    let lock = lock_patiently(ctx)?;
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
    // the hook wrote an event: the no-git rebuild keeps the 5 ms budget
    crate::index::rebuild_if_initialised_fast(ctx);
    drop(lock);
    Ok(())
}

// ---------------------------------------------------------------------------
// Sessions
// ---------------------------------------------------------------------------

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
    let _lock = lock_patiently(ctx)?;
    if let super::Commit::Failed(e) =
        autocommit(ctx, &config, &logbook, &format!("session ended ({actor})"))
    {
        eprintln!("seldon hook: git: {e}");
    }
    // after the commit, so `logbook.git` shows it (CONTRACT rule 2)
    crate::index::rebuild_if_initialised(ctx);
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
    ("SessionEnd", None, "seldon hook session-stop", 60),
];

/// What [`merge_claude_hooks`] did: one label per hook of [`CLAUDE_HOOKS`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Merged {
    pub added: Vec<String>,
    pub present: Vec<String>,
}

/// Merges [`CLAUDE_HOOKS`] into the Claude Code settings file at `path`
/// (created when missing): adds each hook that is not there yet and keeps
/// everything else; the file is written only when something was added.
/// A file that is not a JSON object is refused, never overwritten. `shown`
/// names the file in messages. Used by `hook install` and `seldon init`.
pub fn merge_claude_hooks(path: &Path, shown: &str) -> Result<Merged> {
    let text = match std::fs::read_to_string(path) {
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

    let mut merged = Merged::default();
    for (event, matcher, command, timeout) in CLAUDE_HOOKS {
        let groups = hooks
            .entry(event)
            .or_insert_with(|| json!([]))
            .as_array_mut()
            .ok_or_else(|| refuse(format!("`hooks.{event}` is not a list")))?;
        let label = hook_label(event, matcher, command);
        let has = groups.iter().any(|g| {
            group_matches(g, matcher)
                && g.get("hooks")
                    .and_then(Value::as_array)
                    .is_some_and(|hs| hs.iter().any(|h| h.get("command") == Some(&json!(command))))
        });
        if has {
            merged.present.push(label);
            continue;
        }
        let mut group = json!({
            "hooks": [{ "type": "command", "command": command, "timeout": timeout }]
        });
        if let Some(m) = matcher {
            group["matcher"] = json!(m);
        }
        groups.push(group);
        merged.added.push(label);
    }

    if !merged.added.is_empty() {
        let mut text = serde_json::to_string_pretty(&root).map_err(anyhow::Error::from)?;
        text.push('\n');
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        crate::sys::write_atomic(path, text.as_bytes())?;
    }
    Ok(merged)
}

/// `seldon hook install claude-code [--settings FILE]`: adds each of
/// [`CLAUDE_HOOKS`] that is not there yet ([`merge_claude_hooks`]),
/// records a written file under a watched path as the engine's own write
/// (so the next capture explains its config event), and commits the
/// logbook when its own settings file changed. The state lock is held from
/// the read to the commit: no capture sees the written file before its
/// record.
fn install(ctx: &Context, settings: Option<PathBuf>) -> Result<Output> {
    let (path, logbook) = settings_file(ctx, settings)?;
    let shown = ctx.dirs.display(&path);
    let lock = ctx.lock()?;
    let Merged { added, present } = merge_claude_hooks(&path, &shown)?;
    let own = (!added.is_empty()).then(|| {
        let config = match &logbook {
            Some((config, _)) => config.clone(),
            // `--settings` works without a config; then the default paths
            None => ctx.load_config().ok().flatten().unwrap_or_default(),
        };
        super::setup::record_own_writes_under(
            &lock,
            ctx,
            &config,
            std::slice::from_ref(&path),
            "seldon hook install claude-code",
            OwnOp::Install,
        )
    });
    let mut commit = None;
    if !added.is_empty()
        && let Some((config, logbook)) = &logbook
    {
        commit = Some(autocommit(ctx, config, logbook, "hook install claude-code"));
    }
    drop(lock);

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
    if let Some(Err(e)) = &own {
        let _ = write!(human, "\n{shown}: {}", super::setup::own_writes_warning(e));
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
            "ownWrites": own.as_ref().map_or(Value::Null, super::setup::own_writes_json),
            "git": commit.map_or(Value::Null, |c| c.json()),
        }),
    ))
}

/// What [`unmerge_claude_hooks`] found: one label per hook of
/// [`CLAUDE_HOOKS`], and what becomes of the file.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Unmerged {
    pub removed: Vec<String>,
    pub absent: Vec<String>,
    pub after: After,
}

/// The settings file after [`unmerge_claude_hooks`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum After {
    /// None of Seldon's hooks was there (or no file): nothing to write.
    #[default]
    Unchanged,
    /// The new content, without Seldon's hooks.
    Write(String),
    /// Nothing but Seldon's hooks was in it: the file goes.
    Delete,
}

/// The label of one hook of [`CLAUDE_HOOKS`] in reports.
fn hook_label(event: &str, matcher: Option<&str>, command: &str) -> String {
    match matcher {
        Some(m) => format!("{event} ({m}): {command}"),
        None => format!("{event}: {command}"),
    }
}

/// Whether a hook group has the matcher `matcher` (none or empty: `None`),
/// as [`merge_claude_hooks`] wrote it.
fn group_matches(group: &Value, matcher: Option<&str>) -> bool {
    group
        .get("matcher")
        .and_then(Value::as_str)
        .filter(|m| !m.is_empty())
        == matcher
}

/// The inverse of [`merge_claude_hooks`]: takes each hook of
/// [`CLAUDE_HOOKS`] (same event, matcher and command) out of the Claude
/// Code settings file at `path` and keeps everything else. A group, an
/// event list or the `hooks` object that this leaves empty goes too; a
/// file left as `{}` is deleted ([`After::Delete`]). Nothing is written
/// here: the caller writes or deletes, so that it can record what it did.
/// A file that is not a JSON object is refused, as by the merge.
pub fn unmerge_claude_hooks(path: &Path, shown: &str) -> Result<Unmerged> {
    let all_absent = || Unmerged {
        absent: CLAUDE_HOOKS
            .iter()
            .map(|(e, m, c, _)| hook_label(e, *m, c))
            .collect(),
        ..Unmerged::default()
    };
    let text = match std::fs::read_to_string(path) {
        Ok(t) if t.trim().is_empty() => return Ok(all_absent()),
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(all_absent()),
        Err(e) => {
            return Err(anyhow::Error::new(e)
                .context(format!("cannot read {shown}"))
                .into());
        }
    };
    let refuse = |why: String| Error::user(format!("{shown}: {why}; nothing was changed"));
    let mut root: Value =
        serde_json::from_str(&text).map_err(|e| refuse(format!("not valid JSON ({e})")))?;
    let object = root
        .as_object_mut()
        .ok_or_else(|| refuse("not a JSON object".into()))?;
    let mut out = Unmerged::default();
    let Some(hooks) = object.get_mut("hooks") else {
        return Ok(all_absent());
    };
    let hooks = hooks
        .as_object_mut()
        .ok_or_else(|| refuse("`hooks` is not an object".into()))?;
    for (event, matcher, command, _) in CLAUDE_HOOKS {
        let label = hook_label(event, matcher, command);
        let Some(groups) = hooks.get_mut(event) else {
            out.absent.push(label);
            continue;
        };
        let groups = groups
            .as_array_mut()
            .ok_or_else(|| refuse(format!("`hooks.{event}` is not a list")))?;
        let mut found = false;
        for group in groups.iter_mut().filter(|g| group_matches(g, matcher)) {
            if let Some(list) = group.get_mut("hooks").and_then(Value::as_array_mut) {
                let before = list.len();
                list.retain(|h| h.get("command") != Some(&json!(command)));
                if list.len() < before {
                    found = true;
                    // a group of Seldon's hook alone goes with it
                    if list.is_empty() {
                        *group = Value::Null;
                    }
                }
            }
        }
        if !found {
            out.absent.push(label);
            continue;
        }
        groups.retain(|g| !g.is_null());
        if groups.is_empty() {
            hooks.remove(event);
        }
        out.removed.push(label);
    }
    if out.removed.is_empty() {
        return Ok(out);
    }
    if hooks.is_empty() {
        object.remove("hooks");
    }
    out.after = if object.is_empty() {
        After::Delete
    } else {
        let mut text = serde_json::to_string_pretty(&root).map_err(anyhow::Error::from)?;
        text.push('\n');
        After::Write(text)
    };
    Ok(out)
}

/// `seldon hook uninstall claude-code [--settings FILE]`: takes Seldon's
/// hooks out of the settings file ([`unmerge_claude_hooks`]), writes it or
/// deletes a file that held nothing else, records that as the engine's own
/// write under a watched path (so the next capture explains its config
/// event), and commits the logbook when its own settings file changed;
/// under the state lock from the read to the commit, as `install`.
fn uninstall(ctx: &Context, settings: Option<PathBuf>) -> Result<Output> {
    const BY: &str = "seldon hook uninstall claude-code";
    let (path, logbook) = settings_file(ctx, settings)?;
    let shown = ctx.dirs.display(&path);
    let lock = ctx.lock()?;
    let Unmerged {
        removed,
        absent,
        after,
    } = unmerge_claude_hooks(&path, &shown)?;
    let config = match &logbook {
        Some((config, _)) => config.clone(),
        None => ctx.load_config().ok().flatten().unwrap_or_default(),
    };
    let own = match &after {
        After::Unchanged => None,
        After::Write(text) => {
            crate::sys::write_atomic(&path, text.as_bytes())?;
            Some(super::setup::record_own_writes_under(
                &lock,
                ctx,
                &config,
                std::slice::from_ref(&path),
                BY,
                OwnOp::Remove,
            ))
        }
        After::Delete => Some(super::setup::delete_own_file_under(
            &lock, ctx, &config, &path, BY,
        )?),
    };
    let mut commit = None;
    if after != After::Unchanged
        && let Some((config, logbook)) = &logbook
    {
        commit = Some(autocommit(
            ctx,
            config,
            logbook,
            "hook uninstall claude-code",
        ));
    }
    drop(lock);

    let mut human = match &after {
        After::Unchanged => {
            format!("{shown}: the Seldon hooks are not installed; nothing changed.")
        }
        After::Write(_) => format!("{shown}: removed the Seldon hooks."),
        After::Delete => {
            format!("{shown}: removed the Seldon hooks and the file, which held nothing else.")
        }
    };
    for r in &removed {
        let _ = write!(human, "\n  removed  {r}");
    }
    if after != After::Unchanged {
        for a in &absent {
            let _ = write!(human, "\n  absent   {a}");
        }
    }
    if let Some(Err(e)) = &own {
        let _ = write!(human, "\n{shown}: {}", super::setup::own_writes_warning(e));
    }
    if let Some(c) = &commit {
        human.push_str(&c.human());
    }
    Ok(Output::ok(
        human,
        json!({
            "settings": path,
            "removed": removed,
            "absent": absent,
            "deleted": after == After::Delete,
            "ownWrites": own.as_ref().map_or(Value::Null, super::setup::own_writes_json),
            "git": commit.map_or(Value::Null, |c| c.json()),
        }),
    ))
}

/// The settings file `hook install|uninstall` works on: `--settings`
/// (`~` expanded; no logbook needed), else the logbook's own
/// `.claude/settings.json`, with the config and logbook for the commit.
fn settings_file(
    ctx: &Context,
    settings: Option<PathBuf>,
) -> Result<(PathBuf, Option<(Config, Logbook)>)> {
    Ok(match settings {
        Some(p) => (ctx.dirs.expand(&p.to_string_lossy()), None),
        None => {
            let (config, logbook) = ctx.open_logbook()?;
            (
                logbook.path(".claude/settings.json"),
                Some((config, logbook)),
            )
        }
    })
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

    /// What the hook records for `line` (cwd = the logbook) with a case set.
    fn with_case(line: &str) -> Option<(String, Option<Zone>)> {
        classify(&parse_shell(line), &scope(), Path::new("/home/user/Seldon"))
            .map(|m| (m.subject, m.zone))
    }

    /// … and without a case (ADR-0019: green commands need one).
    fn class(line: &str) -> Option<(String, Option<Zone>)> {
        classify(&parse_shell(line), &scope(), Path::new("/home/user/Seldon"))
            .filter(|m| !m.needs_case)
            .map(|m| (m.subject, m.zone))
    }

    fn red(s: &str) -> Option<(String, Option<Zone>)> {
        Some((s.to_string(), Some(Zone::Red)))
    }

    fn yellow(s: &str) -> Option<(String, Option<Zone>)> {
        Some((s.to_string(), Some(Zone::Yellow)))
    }

    fn green(s: &str) -> Option<(String, Option<Zone>)> {
        Some((s.to_string(), Some(Zone::Green)))
    }

    #[test]
    fn green_needs_a_case() {
        for (line, subject) in [
            ("tee ~/.config/zed/settings.json", "tee"),
            ("npm install", "npm"),
            ("pnpm add -D vite", "pnpm"),
            ("uv pip install requests", "uv"),
            ("cargo install ripgrep", "cargo"),
            ("yarn", "yarn"),
            ("cd /tmp/project && git push origin main", "git"),
            ("echo x > /tmp/notes.txt", "echo"),
            ("rm -rf /tmp/project/target", "rm"),
            ("cp a.txt ~/Documents/", "cp"),
        ] {
            assert_eq!(with_case(line), green(subject), "{line}");
            assert_eq!(class(line), None, "{line}: not without a case");
        }
        // not package changes, not writes
        for line in [
            "npm run build",
            "cargo build --release",
            "go test ./...",
            "uv run pytest",
            "cargo test 2>/dev/null",
            "ls > /dev/null",
        ] {
            assert_eq!(with_case(line), None, "{line}");
        }
        // the logbook is Seldon's own record: writes there are no news
        assert_eq!(with_case("echo x >> /home/user/Seldon/inbox/a.md"), None);
        // tracked changes keep their zone and need no case
        assert_eq!(
            class("npm install && sed -i s/a/b/ ~/.bashrc"),
            yellow("sed")
        );
    }

    #[test]
    fn review_round_1_writers() {
        // an `mv` out of a watched path removes the file there
        assert_eq!(class("mv ~/.config/hypr/old.conf /tmp/"), yellow("mv"));
        assert_eq!(
            class("install -d ~/.config/hypr/scripts"),
            yellow("install")
        );
        assert_eq!(
            class("install -d -m 700 ~/.config/systemd/user"),
            red("install")
        );
        // a copy into a watched directory
        assert_eq!(class("cp new.conf ~/.config/hypr/"), yellow("cp"));
        // shells and eval are read as the commands inside
        assert_eq!(class("bash -c 'sudo pacman -S zed'"), red("pacman"));
        assert_eq!(
            class("sh -c \"sed -i s/a/b/ ~/.config/hypr/a.conf\""),
            yellow("sed")
        );
        assert_eq!(class("eval 'yay -S zed'"), red("yay"));
        assert_eq!(class("bash -c 'command -v yay'"), None);
        assert_eq!(class("bash script.sh"), None);
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
        // in the logbook: recorded without a case (SPEC-ENGINE §8), green
        assert_eq!(class("git commit -am x"), green("git"), "cwd = logbook");
        // anywhere else: green, only with a case (ADR-0019)
        assert_eq!(class("cd /tmp/repo && git commit -am x"), None);
        assert_eq!(with_case("cd /tmp/repo && git commit -am x"), green("git"));
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
}
