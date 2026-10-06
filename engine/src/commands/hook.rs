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
//!   skipPaths`; a command line that names such a path is recorded as
//!   `<program> ‹redacted›`. A tool call that is already
//!   recorded (by `toolUseId`) writes nothing: a `PostToolUse` after its
//!   `PreToolUse`, or a second `PreToolUse` when two settings files hold
//!   the hook (ADR-0030 §5). A `PostToolUse` whose call is not recorded (a
//!   settings file with only that hook) is recorded with the time it
//!   arrives.
//! - Scope: the hooks serve a session whose directory (`CLAUDE_PROJECT_DIR`,
//!   else the payload's `cwd`) lies inside the logbook, or that names none,
//!   and a session `seldon agent start` launched (`SELDON_CASE` holds a
//!   case id, ADR-0030 §1); `[hooks] scope = "all"` serves every session
//!   ([`in_scope`]).
//! - Zones: what a collector tracks keeps its zone (red packages, services
//!   and updates; yellow config under `watchPaths`, plugins, themes). Any
//!   other change (a file written outside `watchPaths` and the logbook, a
//!   foreign package manager's install, `git` outside `~/.config`) is green
//!   and recorded only while a case is set (ADR-0019).
//! - `hook generic` reads `{"command","actor"?,"cwd","startedAt"?,"case"?}`
//!   (`--case` wins over the field, either over `.seldon/active-case`;
//!   without `actor`, `$SELDON_ACTOR`): the same classification for any
//!   agent, called before the command runs.
//! - Not read yet (a follow-up): commands run by `xargs`, `find -exec` or
//!   an interpreter (`python -c`, `node -e`).
//! - `hook session-start` prints the context block an agent starts with.
//! - `hook session-stop` writes the journal stub, runs `capture --all` and
//!   commits.
//! - `hook install claude-code` merges these hooks into the user-wide
//!   Claude Code settings (`$CLAUDE_CONFIG_DIR/settings.json`, else
//!   `~/.claude/settings.json`; ADR-0030 §1), or into `--settings FILE`; a
//!   settings file under a watched path is recorded as the engine's own
//!   write (SPEC-ENGINE §5 rule 7).
//!   `hook uninstall claude-code` takes exactly those hooks out again
//!   ([`unmerge_claude_hooks`]) and records its write, or the deletion of
//!   a file that is left empty, the same way.
//! - `hook install skills` / `hook uninstall skills` put the Seldon agent
//!   skill into the agent skill folders that exist, or take it out again
//!   ([`super::skills`]).
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

use super::agent::CASE_ENV;
use super::capture::{self, CaptureArgs};
use super::event::{ACTOR_ENV, clip, env_actor, parse_case_id, parse_person};
use super::{Context, Output, autocommit};
use crate::attribution::{home_path, normalise};
use crate::collectors::config::{OwnOp, SkipPaths};
use crate::config::{Config, Dirs, HookScope};
use crate::error::{Error, Result};
use crate::ledger::Ledger;
use crate::logbook::cases::{self, CaseFile};
use crate::logbook::lock::{self, Lock};
use crate::logbook::{Logbook, journal};
use crate::model::CaseStatus;
use crate::model::event::{DETAIL_MAX, Event, Kind, Meta, Source, Zone, zone_for};
use crate::model::is_agent;
use crate::pkgcmd::{
    ShellLine, Target, Word, Workdir, globs_overlap, line_vars, omarchy_route, parse_command,
    parse_shell, simple_commands, workdirs, write_targets,
};
use crate::redact::{REDACTED, Redactor};

/// The actor of `hook claude-code`.
pub const CLAUDE_CODE: &str = "agent:claude-code";

/// How long a hook waits for the state lock before it gives up. A capture
/// holds it while its collectors run (each may take seconds), so the wait
/// is long, but it stays below the 10 s timeout `hook install` gives the
/// PreToolUse hook: Claude Code would otherwise cancel the hook first.
const LOCK_PATIENCE: Duration = Duration::from_secs(8);

mod context;
pub use context::{DATA_NOTE, quote, session_start};

/// `seldon hook <command>`.
#[derive(Debug, Clone, Args)]
pub struct HookArgs {
    #[command(subcommand)]
    pub command: HookCommand,
}

#[derive(Debug, Clone, Subcommand)]
pub enum HookCommand {
    /// Merge Seldon's hooks into an agent harness's settings; `skills`: put the
    /// Seldon agent skill into every agent skill folder that exists
    Install {
        /// The harness
        #[arg(value_parser = ["claude-code", "skills"])]
        harness: String,
        /// Settings file (default: the user-wide $CLAUDE_CONFIG_DIR/settings.json, else
        /// ~/.claude/settings.json; claude-code only)
        #[arg(long, value_name = "FILE")]
        settings: Option<PathBuf>,
        /// skills only: where you changed the skill, archive your copy to the
        /// logbook's archive/ and install it as shipped
        #[arg(long)]
        replace: bool,
    },
    /// Remove Seldon's hooks from an agent harness's settings, keeping the rest;
    /// `skills`: remove the Seldon agent skill, keeping files changed by hand
    Uninstall {
        /// The harness
        #[arg(value_parser = ["claude-code", "skills"])]
        harness: String,
        /// Settings file (default: the user-wide $CLAUDE_CONFIG_DIR/settings.json, else
        /// ~/.claude/settings.json; claude-code only)
        #[arg(long, value_name = "FILE")]
        settings: Option<PathBuf>,
    },
    /// Record a Claude Code tool call (hook payload on stdin; silent, exit 0)
    ClaudeCode,
    /// Record any agent's command ({"command","actor"?,"cwd","startedAt"?,"case"?} on
    /// stdin; without "actor", $SELDON_ACTOR)
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
        HookCommand::Install {
            harness,
            settings,
            replace,
        } if harness == SKILLS => {
            no_settings(settings)?;
            super::skills::install(ctx, replace)
        }
        HookCommand::Uninstall { harness, settings } if harness == SKILLS => {
            no_settings(settings)?;
            super::skills::uninstall(ctx)
        }
        HookCommand::Install { replace: true, .. } => Err(Error::user(
            "--replace is for skills; `seldon hook install claude-code` keeps the rest of the settings file anyway",
        )),
        HookCommand::Install { settings, .. } => install(ctx, settings),
        HookCommand::Uninstall { settings, .. } => uninstall(ctx, settings),
        _ => Err(Error::user("this hook is run by an agent harness")),
    }
}

/// The harness of `hook install|uninstall skills` (the agent skill, WP-094).
pub const SKILLS: &str = "skills";

/// `--settings` names a Claude Code settings file; the skill has none.
fn no_settings(settings: Option<PathBuf>) -> Result<()> {
    match settings {
        Some(_) => Err(Error::user(
            "--settings is for claude-code; the skill goes into the agent skill folders",
        )),
        None => Ok(()),
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
        HookCommand::SessionStart => {
            session_start(&ctx, payload_cwd(&stdin).as_deref()).map(|block| {
                let mut out = std::io::stdout().lock();
                let _ = out.write_all(block.as_bytes()).and_then(|()| out.flush());
            })
        }
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
        // as the config collector reads them: an empty entry watches nothing
        let mut watched: Vec<PathBuf> = config
            .watch_paths
            .iter()
            .filter_map(|p| dirs.expand_config(p))
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
/// `cd`, `pushd` and `popd` change the directory for the commands after
/// them, and a program's `-C DIR` or a wrapper's `env -C DIR` its own
/// ([`workdirs`]).
pub fn classify(line: &ShellLine, scope: &Scope, cwd: &Path) -> Option<Mutation> {
    let segments = simple_commands(line);
    let dirs = workdirs(&segments, cwd, &scope.home, |w, d| scope.resolve(w, d));
    let mut found: Option<Mutation> = None;
    for (segment, dirs) in segments.iter().zip(&dirs) {
        let mutation = classify_segment(segment.argv(), &segment.writes, scope, dirs);
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
/// writes ([`write_targets`]): redirections in the shell's directory, the
/// program's own operands where it runs.
fn classify_segment(
    argv: &[String],
    writes: &[String],
    scope: &Scope,
    dirs: &Workdir,
) -> Option<Mutation> {
    let word = argv.first().map(String::as_str).unwrap_or("");
    let program = word.rsplit('/').next().unwrap_or(word);
    let args = argv.get(1..).unwrap_or_default();
    // a snapshot an agent takes (ADR-0027 §3): recorded with its case, so
    // the capture after it can fill the case's `snapshotBefore` when the
    // agent did not record the number itself (WP-101); the files the same
    // command writes are classified as for any other (round 2)
    let snapshot = is_snapshot_create(program, argv);
    let subject = if snapshot {
        SNAPSHOT_SUBJECT
    } else if program.is_empty() {
        "sh"
    } else {
        program
    };
    let mutation = |(zone, needs_case): (Option<Zone>, bool)| Mutation {
        subject: subject.to_string(),
        zone,
        needs_case,
    };

    let argv_str: Vec<&str> = argv.iter().map(String::as_str).collect();
    let command = match parse_command(&argv_str) {
        _ if snapshot => Some((Some(Zone::Green), true)),
        Some(cmd) => cmd.is_mutating().then_some((Some(Zone::Red), false)),
        None => match program {
            p if p == "omarchy" || p.starts_with("omarchy-") => omarchy_route(argv)
                .and_then(|route| omarchy(&route))
                .map(|z| (z, false)),
            "systemctl" => args
                .iter()
                .find(|a| !a.starts_with('-'))
                .filter(|verb| SYSTEMCTL_VERBS.contains(&verb.as_str()))
                .map(|_| (Some(Zone::Red), false)),
            "git" => git(args, scope, &dirs.program),
            p if FOREIGN_PACKAGE_MANAGERS.contains(&p) => {
                foreign_install(p, args).then_some((Some(Zone::Green), true))
            }
            _ => None,
        },
    };
    let paths: Vec<(PathBuf, bool)> = writes
        .iter()
        .map(|w| (scope.resolve(w, &dirs.shell), false))
        .chain(write_targets(argv, &[]).iter().map(|t| {
            (
                scope.resolve(t.word(), &dirs.program),
                matches!(t, Target::Tree(_)),
            )
        }))
        .collect();
    let files = scope.writes(&paths);
    [command, files]
        .into_iter()
        .flatten()
        .map(mutation)
        .reduce(|a, b| if b.rank() > a.rank() { b } else { a })
}

/// The subject of a recorded snapshot command: `snapper … create`,
/// `omarchy-snapshot create`, `omarchy snapshot create`.
pub const SNAPSHOT_SUBJECT: &str = "snapper";

/// Whether a simple command (after its wrappers) creates a snapper
/// snapshot: `snapper [options] create …` or Omarchy's snapshot route.
fn is_snapshot_create(program: &str, argv: &[String]) -> bool {
    if program == "snapper" {
        // `-c <config>` and the like take a value; `create` is the first
        // word that is no option's value
        let mut words = argv.iter().skip(1).map(String::as_str);
        while let Some(w) = words.next() {
            match w {
                "-c" | "--config" | "-r" | "--root" => {
                    words.next();
                }
                w if w.starts_with('-') => {}
                w => return w == "create",
            }
        }
        return false;
    }
    (program == "omarchy" || program.starts_with("omarchy-"))
        && omarchy_route(argv).is_some_and(|r| r.starts_with(&["snapshot", "create"]))
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
/// `dir` is where git runs, its `-C DIR` already followed ([`workdirs`]).
fn git(args: &[String], scope: &Scope, dir: &Path) -> Option<(Option<Zone>, bool)> {
    let mut dir = dir.to_path_buf();
    let mut it = args.iter();
    let sub = loop {
        let a = it.next()?;
        match a.as_str() {
            "-C" => {
                it.next()?;
            }
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
    /// Default `$SELDON_ACTOR` (WP-096); one of the two is needed.
    #[serde(default)]
    actor: Option<String>,
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

/// The config, the logbook, its scope and the `[redaction] skipPaths`, for
/// a hook that records.
struct Setup {
    config: Config,
    logbook: Logbook,
    scope: Scope,
    skip: SkipPaths,
}

/// The config and the logbook when the hooks serve the session whose
/// directory is `cwd` ([`in_scope`]), else `None`: a session they do not
/// serve costs this check and nothing more (WP-116 round 2).
fn served(ctx: &Context, cwd: Option<&str>) -> Result<Option<(Config, Logbook)>> {
    let (config, logbook) = ctx.open_logbook()?;
    Ok(in_scope(&config, &logbook.root, cwd).then_some((config, logbook)))
}

fn setup(ctx: &Context, (config, logbook): (Config, Logbook)) -> Setup {
    let scope = Scope::new(&ctx.dirs, &config, &logbook.root);
    let skip = SkipPaths::new(&ctx.dirs.home, &config.redaction.skip_paths);
    Setup {
        config,
        logbook,
        scope,
        skip,
    }
}

/// Claude Code's project directory, set in the environment of the hook
/// commands it runs.
const PROJECT_DIR_ENV: &str = "CLAUDE_PROJECT_DIR";

/// Whether the hooks serve the session the hook is called for (ADR-0030
/// §1): (a) by its directory, or (b) because `seldon agent start` launched
/// it. (a): the session's directory is [`PROJECT_DIR_ENV`] when it is set
/// (the project stays the same while the agent's `cwd` moves), else `cwd`,
/// the directory the hook's payload names; with neither (an agent or a
/// person running the hook itself) the session is served. With `[hooks]
/// scope = "logbook"`, the default, only a session inside the logbook; with
/// `"all"` every session. A directory that is not an absolute path is
/// outside. (b): the hook's environment holds [`CASE_ENV`] naming an open
/// case of this logbook ([`launched_case`]), under either scope. Relative
/// paths in a command still resolve against `cwd`. Clause (a) first: it
/// reads no file.
fn in_scope(config: &Config, logbook: &Path, cwd: Option<&str>) -> bool {
    in_scope_by_dir(config, logbook, cwd) || launched_case(logbook).is_some()
}

/// The case `seldon agent start` launched this session on: [`CASE_ENV`]
/// in the hook's environment, when it names a case of the logbook at
/// `logbook` that is active or in verification (clause (b) of
/// [`in_scope`]; ADR-0032). Anything else is no marker: an empty value,
/// one that is not a case id, a case this logbook does not have, a
/// queued, completed or dropped case — so a server the agent started
/// (tmux, an editor server) that keeps the variable after the case is
/// done records nothing. Only the scope and the context's launch line
/// read it: hook events still take their case from `.seldon/active-case`
/// (ADR-0030 §1). It reads the folder of active cases only.
fn launched_case(logbook: &Path) -> Option<String> {
    let id = std::env::var(CASE_ENV)
        .ok()
        .filter(|id| parse_case_id(id).is_ok())?;
    let (prefix, exact) = (format!("{id}-"), format!("{id}.md"));
    let dir = logbook.join("work").join(CaseStatus::Active.folder());
    let open = std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name().is_some_and(|n| {
                let n = n.to_string_lossy();
                n.ends_with(".md") && (n.starts_with(&prefix) || n == exact)
            })
        })
        .filter_map(|p| CaseFile::load(&p).ok())
        .any(|f| {
            f.case.id == id
                && matches!(f.case.status, CaseStatus::Active | CaseStatus::Verification)
        });
    open.then_some(id)
}

/// Whether the hook's environment holds a well-formed [`CASE_ENV`],
/// whatever case it names.
fn marked() -> bool {
    std::env::var(CASE_ENV).is_ok_and(|id| parse_case_id(&id).is_ok())
}

/// Clause (a) of [`in_scope`]: the session's directory and the scope.
fn in_scope_by_dir(config: &Config, logbook: &Path, cwd: Option<&str>) -> bool {
    let project = std::env::var(PROJECT_DIR_ENV)
        .ok()
        .filter(|d| !d.is_empty());
    match (config.hooks.scope, project.as_deref().or(cwd)) {
        (HookScope::All, _) | (_, None) => true,
        (HookScope::Logbook, Some(dir)) => {
            Path::new(dir).is_absolute() && is_inside(Path::new(dir), logbook)
        }
    }
}

/// Whether the absolute `path` is `root` or lies below it: as written
/// (`.` and `..` folded), or with symbolic links resolved as far as the
/// path exists, so a logbook reached through a link counts both ways.
fn is_inside(path: &Path, root: &Path) -> bool {
    normalise(path).starts_with(normalise(root))
        || resolved(&normalise(path)).starts_with(resolved(&normalise(root)))
}

/// `path` with its longest existing ancestor's symbolic links resolved.
fn resolved(path: &Path) -> PathBuf {
    let mut rest = Vec::new();
    let mut base = path;
    loop {
        if let Ok(mut real) = std::fs::canonicalize(base) {
            real.extend(rest.iter().rev());
            return real;
        }
        match (base.parent(), base.file_name()) {
            (Some(parent), Some(name)) => {
                rest.push(name);
                base = parent;
            }
            _ => return path.to_path_buf(),
        }
    }
}

/// The `cwd` a hook payload names, if it is a JSON object that has one (a
/// `cwd` that is not a string counts as one that is outside every
/// directory).
fn payload_cwd(stdin: &str) -> Option<String> {
    let v: Value = serde_json::from_str(stdin).ok()?;
    v.get("cwd")
        .map(|c| c.as_str().unwrap_or_default().to_string())
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
    let search = match payload.hook_event_name.as_deref() {
        Some("PreToolUse") => Search::Tail,
        Some("PostToolUse") => Search::Whole,
        _ => return Ok(()),
    };
    let Some(served) = served(ctx, payload.cwd.as_deref())? else {
        return Ok(());
    };
    let tool = payload.tool_name.as_deref().unwrap_or("");
    let setup = setup(ctx, served);
    let cwd = working_dir(payload.cwd.as_deref(), &ctx.dirs);
    let records = match tool {
        "Bash" => {
            let Some(command) = payload.tool_input.get("command").and_then(Value::as_str) else {
                return Ok(());
            };
            bash_record(command, &setup, &cwd)
                .into_iter()
                .collect::<Vec<_>>()
        }
        "Edit" | "Write" | "MultiEdit" => edit_records(&setup, tool, &payload.tool_input, &cwd),
        _ => Vec::new(),
    };
    if records.is_empty() {
        return Ok(());
    }
    let ledger = Ledger::new(
        &setup.logbook,
        Redactor::with_patterns(&setup.config.redaction.patterns)?,
    );
    let tool_use_id = payload.tool_use_id.filter(|s| !s.is_empty());
    let mut extra = Vec::new();
    if let Some(id) = &tool_use_id {
        extra.push(("toolUseId", id.clone()));
    }
    if let Some(id) = payload.session_id.filter(|s| !s.is_empty()) {
        extra.push(("sessionId", id));
    }
    let entry = Entry {
        actor: CLAUDE_CODE,
        ts: ctx.now,
        case: None,
        unless_recorded: tool_use_id.as_deref().map(|id| (id, search)),
    };
    record(ctx, &setup, ledger, entry, records, &extra)
}

fn generic(ctx: &Context, stdin: &str, case_flag: Option<String>) -> Result<()> {
    let payload: GenericPayload = serde_json::from_str(stdin).map_err(|e| {
        Error::user(format!(
            "stdin is not {{\"command\",\"actor\"?,\"cwd\",\"startedAt\"?,\"case\"?}}: {e}"
        ))
    })?;
    let Some(served) = served(ctx, payload.cwd.as_deref())? else {
        return Ok(());
    };
    let actor = match &payload.actor {
        Some(actor) => parse_person(actor).map_err(Error::user)?,
        None => env_actor(parse_person)?.ok_or_else(|| {
            Error::user(format!(
                "stdin has no \"actor\" and {ACTOR_ENV} is not set; name human or agent:<name>"
            ))
        })?,
    };
    let case = match case_flag.or(payload.case) {
        Some(id) => Some(parse_case_id(&id).map_err(Error::user)?),
        None => None,
    };
    let ts = match payload.started_at.as_deref() {
        Some(s) => chrono::DateTime::parse_from_rfc3339(s).map_err(|e| {
            Error::user(format!(
                "startedAt `{}` is not RFC 3339: {e}",
                s.escape_debug()
            ))
        })?,
        None => ctx.now,
    };
    let setup = setup(ctx, served);
    let cwd = working_dir(payload.cwd.as_deref(), &ctx.dirs);
    let Some(rec) = bash_record(&payload.command, &setup, &cwd) else {
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
        unless_recorded: None,
    };
    record(ctx, &setup, ledger, entry, vec![rec], &[])
}

/// A shell command line as a record, if it is mutating. A line that names
/// a path `[redaction] skipPaths` matches is recorded as `<program>
/// ‹redacted›` (the program of [`Mutation::subject`]), as an `Edit` of such
/// a file is recorded as `Edit ‹redacted›`.
fn bash_record(command: &str, setup: &Setup, cwd: &Path) -> Option<Record> {
    let line = parse_shell(command);
    let mutation = classify(&line, &setup.scope, cwd)?;
    let command = if names_skipped_path(&line, setup, cwd) {
        format!("{} {REDACTED}", mutation.subject)
    } else {
        line.text
    };
    Some(Record { mutation, command })
}

/// Whether `line` names a path that `[redaction] skipPaths` matches. Read
/// as paths: every word of its commands (`sh -c` scripts opened) and every
/// write target, and every token of the line's text between blanks, quotes,
/// shell operators, `:` and `,` (which also holds the files read with `<`,
/// the words inside `$(…)` and the parts of a list such as `PATH=a:b`);
/// each also after its first `=` (`--file=PATH`, `VAR=PATH`). A relative
/// path is tried against `cwd` and against every directory the line's
/// commands work in ([`workdirs`]: `cd`, `pushd`, `popd`, a program's
/// `-C DIR`, `env -C DIR`), as the classifier reads them. A variable the
/// line sets to a literal is read with its value (`F=x; cat ~/d/$F`); a
/// word with a glob or an unknown part (`priv*.conf`, `$D/id_rsa`) counts
/// when a path it can name matches ([`SkipGlob::floating_overlaps`] for one
/// that starts with an unknown part; one with nothing known but `/` names
/// no path). It matches more than the shell would
/// open.
fn names_skipped_path(line: &ShellLine, setup: &Setup, cwd: &Path) -> bool {
    if setup.config.redaction.skip_paths.is_empty() {
        return false;
    }
    let scope = &setup.scope;
    let segments = simple_commands(line);
    let mut dirs = vec![cwd.to_path_buf()];
    for d in workdirs(&segments, cwd, &scope.home, |w, d| scope.resolve(w, d)) {
        dirs.extend([d.shell, d.program]);
    }
    dirs.sort_unstable();
    dirs.dedup();
    let mut words: Vec<String> = Vec::new();
    for segment in segments {
        words.extend(segment.words);
        words.extend(segment.writes);
    }
    words.extend(
        line.text
            .split(|c: char| c.is_whitespace() || "'\"`<>|&;(),:".contains(c))
            .filter(|t| !t.is_empty())
            .map(str::to_string),
    );
    let vars = line_vars(line);
    let globs = skip_globs(&setup.config.redaction.skip_paths, &scope.home);
    words.iter().any(|word| {
        let after_equals = word.split_once('=').map(|(_, value)| value);
        [Some(word.as_str()), after_equals]
            .into_iter()
            .flatten()
            .filter(|w| !w.is_empty())
            .any(|w| match vars.expand(w) {
                Word::Literal(w) => dirs
                    .iter()
                    .any(|d| setup.skip.matches(&scope.resolve(&w, d))),
                // only unknown parts: no path in it
                Word::Pattern {
                    only_unknown: true, ..
                } => false,
                Word::Pattern {
                    glob,
                    floating: true,
                    ..
                } => globs.iter().any(|g| g.floating_overlaps(&glob)),
                Word::Pattern { glob, .. } => dirs.iter().any(|d| {
                    let path = scope.resolve(&glob, d);
                    globs.iter().any(|g| g.overlaps(&path.to_string_lossy()))
                }),
            })
    })
}

/// A `[redaction] skipPaths` pattern as a glob over absolute paths, read
/// the way `SkipPaths` reads it: without `/` a last path component, else a
/// path and everything below it (`~` the home directory; a relative
/// pattern starts at any directory).
enum SkipGlob {
    Name(String),
    Path(String),
}

impl SkipGlob {
    /// Whether a path the glob `path` (absolute, or starting with an unknown
    /// part) can name matches.
    fn overlaps(&self, path: &str) -> bool {
        match self {
            SkipGlob::Name(name) => globs_overlap(path.rsplit('/').next().unwrap_or(path), name),
            SkipGlob::Path(p) => globs_overlap(path, p) || globs_overlap(path, &format!("{p}/**")),
        }
    }

    /// Whether a path the floating glob `word` (`$X/tail`, `"$(pwd)"/x`:
    /// its first component holds the unknown part) can name matches: a
    /// name pattern by the word's last component; a path pattern only by
    /// its last components, which must be literal and match the word's
    /// known tail, never by a path below the pattern. So `$D/id_rsa` is
    /// `~/.ssh/id_rsa`, while `$TMPDIR/yay.log` is no
    /// `~/.config/omarchy/**/*.log` and `$PKGDEST/x.pkg.tar.zst` is below
    /// no skipped folder.
    fn floating_overlaps(&self, word: &str) -> bool {
        let SkipGlob::Path(p) = self else {
            return self.overlaps(word);
        };
        let literal = |c: &&str| !c.contains(['*', '?']);
        let pattern: Vec<&str> = p.split('/').collect();
        let parts: Vec<&str> = word.split('/').collect();
        let (head, tail) = parts.split_first().unwrap_or((&"", &[]));
        if tail.is_empty() {
            // `$Xname`: the pattern's last component
            return pattern
                .last()
                .is_some_and(|last| literal(last) && globs_overlap(head, last));
        }
        pattern.len() > tail.len()
            && pattern[pattern.len() - tail.len()..]
                .iter()
                .zip(tail)
                .all(|(pc, wc)| literal(pc) && globs_overlap(wc, pc))
    }
}

fn skip_globs(patterns: &[String], home: &Path) -> Vec<SkipGlob> {
    let home = home.to_string_lossy();
    patterns
        .iter()
        .map(|p| p.trim().trim_end_matches('/'))
        .filter(|p| !p.is_empty())
        .map(|p| {
            if !p.contains('/') && p != "~" {
                return SkipGlob::Name(p.to_string());
            }
            match p.strip_prefix('~') {
                Some(rest) if rest.is_empty() || rest.starts_with('/') => {
                    SkipGlob::Path(format!("{home}{rest}"))
                }
                _ if p.starts_with('/') => SkipGlob::Path(p.to_string()),
                _ => SkipGlob::Path(format!("**/{p}")),
            }
        })
        .collect()
}

/// `Edit`/`Write`/`MultiEdit`: one record per path. A watched path takes
/// its config zone (ADR-0014 §4); any other path outside the logbook is
/// green and recorded only with a case (ADR-0019). Only the path is read
/// from `tool_input`, never the content.
fn edit_records(setup: &Setup, tool: &str, input: &Value, cwd: &Path) -> Vec<Record> {
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
    paths
        .into_iter()
        .map(|p| setup.scope.resolve(p, cwd))
        .filter_map(|p| {
            let (zone, needs_case) = setup.scope.writes(&[(p.clone(), false)])?;
            let shown = if setup.skip.matches(&p) {
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

/// How much of a tool call's history [`already_recorded`] reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Search {
    /// The last [`PRE_TAIL`] bytes of each month file: a second
    /// `PreToolUse` comes moments after the first (ADR-0030 §5).
    Tail,
    /// The whole month files: a `PostToolUse` may come long after its
    /// `PreToolUse`.
    Whole,
}

/// The end of a month file a `PreToolUse` searches for its tool call
/// (about 500 events).
const PRE_TAIL: u64 = 256 * 1024;

/// Whether the ledger already holds an event of the last day for this
/// tool call: a PostToolUse after its PreToolUse, or a second PreToolUse
/// from a second settings file (ADR-0030 §5). The month files are searched
/// for the id as a JSON string, and only a line that holds it is parsed:
/// asked for every recorded call, this keeps the hook within its budget
/// (SPEC-ENGINE §1).
fn already_recorded(
    ledger: &Ledger,
    ctx: &Context,
    tool_use_id: &str,
    search: Search,
) -> Result<bool> {
    let (from, to) = (
        ctx.now - chrono::Duration::days(1),
        ctx.now + chrono::Duration::days(1),
    );
    let quoted = serde_json::to_string(tool_use_id).map_err(anyhow::Error::from)?;
    // the months `Ledger::read_range` reads (a day more: an event's month
    // file goes by its own offset)
    let first = (from - chrono::Duration::days(1))
        .format("%Y-%m")
        .to_string();
    let last = (to + chrono::Duration::days(1)).format("%Y-%m").to_string();
    let tail = (search == Search::Tail).then_some(PRE_TAIL);
    for month in ledger.months()? {
        if month < first || month > last {
            continue;
        }
        let bytes = read_end(&ledger.month_file(&month), tail)?;
        let text = String::from_utf8_lossy(&bytes);
        let found = text.match_indices(&quoted).any(|(at, _)| {
            let start = text[..at].rfind('\n').map_or(0, |i| i + 1);
            let end = text[at..].find('\n').map_or(text.len(), |i| at + i);
            crate::ledger::parse_line(text[start..end].trim_end_matches('\r').as_bytes())
                .is_some_and(|e| {
                    e.ts >= from
                        && e.ts <= to
                        && e.meta.extra.get("toolUseId").and_then(Value::as_str)
                            == Some(tool_use_id)
                })
        });
        if found {
            return Ok(true);
        }
    }
    Ok(false)
}

/// The bytes of `path` from `tail` bytes before its end on (all of it with
/// `None`), starting at a line start; nothing when there is no file.
fn read_end(path: &Path, tail: Option<u64>) -> Result<Vec<u8>> {
    use std::io::{Seek as _, SeekFrom};
    let mut file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(anyhow::Error::new(e).into()),
    };
    let len = file.metadata().map_err(anyhow::Error::from)?.len();
    let start = tail.map_or(0, |t| len.saturating_sub(t));
    file.seek(SeekFrom::Start(start))
        .map_err(anyhow::Error::from)?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).map_err(anyhow::Error::from)?;
    if start > 0 {
        // the first line is cut: it starts before the tail
        let cut = bytes
            .iter()
            .position(|&b| b == b'\n')
            .map_or(bytes.len(), |i| i + 1);
        bytes.drain(..cut);
    }
    Ok(bytes)
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

/// Who recorded the commands, when they started, their case if the caller
/// named one (else `.seldon/active-case`), and for a Claude Code tool call
/// its id: nothing is written when the ledger holds it already.
struct Entry<'a> {
    actor: &'a str,
    ts: chrono::DateTime<chrono::FixedOffset>,
    case: Option<String>,
    unless_recorded: Option<(&'a str, Search)>,
}

/// Appends one `agent/command` event per record and attaches them to the
/// case. Records that need a case (ADR-0019, green) are dropped without
/// one. The command line is redacted before it is cut, so a cut never
/// leaves half a secret.
///
/// The case is read under the state lock, like every other case writer:
/// a copy read before the wait would write back what a `plan` step or a
/// second hook changed meanwhile (WP-057). A `PostToolUse` looks for its
/// tool call in the ledger under the lock too, so two of them for one call
/// write one event. The index is rebuilt after the lock is released
/// ([`rebuild_after`]); the rebuild itself is capped at 1000 ledger lines,
/// which is what keeps a recorded command cheap.
fn record(
    ctx: &Context,
    setup: &Setup,
    ledger: Ledger,
    entry: Entry,
    records: Vec<Record>,
    extra: &[(&str, String)],
) -> Result<()> {
    // green records without any case are dropped before the lock: the
    // common case of an agent working without a case takes no lock
    if entry.case.is_none()
        && records.iter().all(|r| r.mutation.needs_case)
        && cases::active_case(&setup.logbook).is_none()
    {
        return Ok(());
    }
    let lock = match lock_patiently(ctx) {
        Ok(lock) => lock,
        Err(e @ Error::LockHeld(_)) => {
            return Err(anyhow::anyhow!("{e}; command not recorded").into());
        }
        Err(e) => return Err(e),
    };
    if let Some((id, search)) = entry.unless_recorded
        && already_recorded(&ledger, ctx, id, search)?
    {
        return Ok(());
    }
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
    // a save the case would refuse fails before the ledger changes (WP-077)
    if let Some(file) = &case_file {
        let ids = cases::pending_ids(events.len());
        file.prepare(&setup.logbook, |f| {
            for (id, e) in ids.iter().zip(&events) {
                f.attach(id, &e.actor);
            }
        })?;
    }
    let written = ledger.append(&lock, events)?;
    if let Some(file) = case_file.as_mut() {
        for e in &written {
            file.attach(&e.id.to_string(), &e.actor);
        }
        file.save(&setup.logbook)?;
    }
    drop(lock);
    rebuild_after(ctx);
    Ok(())
}

/// The no-git index rebuild after a hook wrote an event, outside the
/// record's critical section: the lock is taken again without waiting. If
/// another `seldon` got it first, that writer rebuilds after its own write
/// (or, above the line threshold, the next `capture`/`status` does), and
/// its index holds this hook's event too.
fn rebuild_after(ctx: &Context) {
    if let Ok(_lock) = lock::acquire(&ctx.dirs.lock_file()) {
        crate::index::rebuild_if_initialised_fast(ctx);
    }
}

// ---------------------------------------------------------------------------
// Sessions
// ---------------------------------------------------------------------------

/// The journal stub, `capture --all`, the generated views (`STATUS.md`,
/// the `ledger/*.md` months, the decisions index), the commit and the
/// index (WP-007). Each step runs even when an earlier one failed; each
/// failure is reported on stderr as it happens (the hook still exits 0).
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
    let served = in_scope(&config, &logbook.root, payload_cwd(stdin).as_deref());
    // the agent may have closed its case before the session ends: a
    // launched session whose events the ledger holds is still ended
    let events = match (served, &session) {
        (false, Some(id)) if marked() => {
            match session_events(ctx, &config, &logbook, actor, Some(id)) {
                Ok(n) if n > 0 => Some(Ok(n)),
                _ => return Ok(()),
            }
        }
        (false, _) => return Ok(()),
        (true, _) => None,
    };
    let report = |step: &str, e: &dyn std::fmt::Display| eprintln!("seldon hook: {step}: {e}");

    let events =
        events.unwrap_or_else(|| session_events(ctx, &config, &logbook, actor, session.as_deref()));
    let text = match events {
        Ok(n) => format!(
            "session ended; {n} {} recorded",
            if n == 1 { "event" } else { "events" }
        ),
        Err(e) => {
            report("ledger", &e);
            "session ended".to_string()
        }
    };
    match lock_patiently(ctx) {
        Ok(_lock) => {
            let case = cases::active_case(&logbook);
            if let Err(e) = journal::append(&logbook, &ctx.now, actor, case.as_deref(), &text) {
                report("journal", &e);
            }
        }
        Err(e) => report("journal", &e),
    }

    match capture::run(
        ctx,
        CaptureArgs {
            all: true,
            ..CaptureArgs::default()
        },
    ) {
        // the state reset's warning (WP-081); stdout belongs to the harness
        Ok(out) => {
            for w in out.json["warnings"].as_array().into_iter().flatten() {
                eprintln!("seldon: warning: {}", w.as_str().unwrap_or_default());
            }
        }
        Err(e) => report("capture", &e),
    }

    let _lock = match lock_patiently(ctx) {
        Ok(lock) => lock,
        Err(e) => {
            report("views, git and index", &e);
            return Ok(());
        }
    };
    // the views first, so the commit holds them (SPEC-ENGINE §8)
    let built = match crate::index::derive(ctx, &config, &logbook) {
        Ok(built) => Some(built),
        Err(e) => {
            report("index", &e);
            None
        }
    };
    if let Some(built) = &built {
        for w in &built.warnings {
            eprintln!("seldon: warning: {w}");
        }
        if let Err(e) = write_views(&logbook, built) {
            report("views", &e);
        }
    }
    if let super::Commit::Failed(e) =
        autocommit(ctx, &config, &logbook, &format!("session ended ({actor})"))
    {
        report("git", &e);
    }
    // after the commit, so `logbook.git` shows it (CONTRACT rule 2)
    if let Some(mut built) = built {
        built.index.logbook.git = crate::index::git_info(&logbook.root);
        if let Err(e) = crate::index::write(&ctx.dirs.index_file(), &built.index) {
            report("index", &e);
        }
    }
    Ok(())
}

/// The generated Markdown `seldon status` writes: the `ledger/*.md`
/// months, `STATUS.md` and the `decisions.index` fence of `DECISIONS.md`
/// (each only when its text changed).
fn write_views(logbook: &Logbook, built: &crate::index::Built) -> Result<()> {
    use crate::index::views;
    views::write_ledger_views(logbook, built)?;
    if let views::Fill::Skipped(w) = views::write_status(logbook, built)? {
        eprintln!("seldon: warning: {w}");
    }
    if let views::Fill::Skipped(w) = views::write_decisions_index(logbook, &built.index.decisions)?
    {
        eprintln!("seldon: warning: {w}");
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
        if has_hook(groups, matcher, command) {
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
        // write_atomic creates a missing directory (0700)
        crate::sys::write_atomic(path, text.as_bytes())?;
    }
    Ok(merged)
}

/// `seldon hook install claude-code [--settings FILE]`: adds each of
/// [`CLAUDE_HOOKS`] that is not there yet ([`merge_claude_hooks`]) to the
/// user-wide settings by default ([`settings_file`], ADR-0030 §1),
/// records a written file under a watched path as the engine's own write
/// (so the next capture explains its config event), and commits the
/// logbook when a settings file inside it changed. The state lock is held
/// from the read to the commit: no capture sees the written file before
/// its record. The output says which sessions the hooks serve
/// ([`scope_line`]); with `[hooks] scope = "all"` that is a warning.
fn install(ctx: &Context, settings: Option<PathBuf>) -> Result<Output> {
    let (path, logbook) = settings_file(ctx, settings)?;
    let shown = ctx.dirs.display(&path);
    let (scope, all) = scope_line(ctx, &shown, logbook.as_ref());
    let warnings: Vec<String> = all.then(|| scope.clone()).into_iter().collect();
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
    if all {
        let _ = write!(human, "\nwarning: {scope}");
    } else {
        let _ = write!(human, "\n{scope}");
    }
    Ok(Output::ok(
        human,
        json!({
            "settings": path,
            "added": added,
            "present": present,
            "scope": scope,
            "warnings": warnings,
            "ownWrites": own.as_ref().map_or(Value::Null, super::setup::own_writes_json),
            "git": commit.map_or(Value::Null, |c| c.json()),
        }),
    ))
}

/// Which sessions the hooks in the settings file `shown` serve, in one
/// line (ADR-0030 §1, §5): Claude Code runs them in every session that
/// reads the file, and Seldon serves the two clauses of [`in_scope`] — or,
/// with `[hooks] scope = "all"`, every session (`true`: then the line is a
/// warning).
fn scope_line(ctx: &Context, shown: &str, logbook: Option<&(Config, Logbook)>) -> (String, bool) {
    let loaded;
    let (config, root) = match logbook {
        Some((config, logbook)) => (config, logbook.root.clone()),
        None => {
            loaded = ctx.load_config().ok().flatten().unwrap_or_default();
            (&loaded, ctx.resolve_logbook(None, Some(&loaded)).0)
        }
    };
    let root = ctx.dirs.display(&root);
    match config.hooks.scope {
        HookScope::Logbook => (
            format!(
                "Claude Code runs these hooks in every session that reads {shown}; Seldon \
                 records only the sessions inside the logbook ({root}) and those `seldon agent \
                 start` launched ({CASE_ENV}), and stays silent in every other session \
                 ([hooks] scope = \"logbook\")."
            ),
            false,
        ),
        HookScope::All => (
            format!(
                "Claude Code runs these hooks in every session that reads {shown}, and with \
                 [hooks] scope = \"all\" Seldon records the commands of each of them in the \
                 logbook ({root}) and prints the logbook context there."
            ),
            true,
        ),
    }
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

/// Whether the hook groups of one event hold `command` under `matcher`.
fn has_hook(groups: &[Value], matcher: Option<&str>, command: &str) -> bool {
    groups.iter().any(|g| {
        group_matches(g, matcher)
            && g.get("hooks")
                .and_then(Value::as_array)
                .is_some_and(|hs| hs.iter().any(|h| h.get("command") == Some(&json!(command))))
    })
}

/// How many of [`CLAUDE_HOOKS`] the Claude Code settings file at `path`
/// holds (0 without the file). Read-only, for `doctor`; `Err` is why the
/// file cannot be read as settings.
pub fn claude_hooks_in(path: &Path) -> std::result::Result<usize, String> {
    let text = match std::fs::read_to_string(path) {
        Ok(t) if t.trim().is_empty() => return Ok(0),
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(e) => return Err(format!("cannot be read: {e}")),
    };
    let root: Value =
        serde_json::from_str(&text).map_err(|e| format!("is not valid JSON ({e})"))?;
    let hooks = root.get("hooks").and_then(Value::as_object);
    Ok(CLAUDE_HOOKS
        .iter()
        .filter(|(event, matcher, command, _)| {
            hooks
                .and_then(|h| h.get(*event))
                .and_then(Value::as_array)
                .is_some_and(|groups| has_hook(groups, *matcher, command))
        })
        .count())
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
/// (`~` expanded), else the user-wide one ([`user_settings_file`],
/// ADR-0030 §1); no logbook is needed. With the config and the logbook
/// when the file lies inside the logbook, whose commit then holds it.
fn settings_file(
    ctx: &Context,
    settings: Option<PathBuf>,
) -> Result<(PathBuf, Option<(Config, Logbook)>)> {
    let path = match settings {
        Some(p) => ctx.dirs.expand(&p.to_string_lossy()),
        None => user_settings_file(&ctx.dirs),
    };
    let logbook = ctx
        .open_logbook()
        .ok()
        .filter(|(_, logbook)| is_inside(&path, &logbook.root));
    Ok((path, logbook))
}

/// The marker in the state directory that the hooks of a logbook's own
/// settings were carried to the user-wide ones, or found there
/// ([`migrate_to_user_wide`]): after it, a capture never adds them again.
pub const MIGRATED_MARKER: &str = "hooks-user-wide";

/// What [`migrate_to_user_wide`] did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Migration {
    /// The user-wide settings file the hooks were added to.
    pub added_to: Option<PathBuf>,
    pub warnings: Vec<String>,
}

/// ADR-0030 §5, WP-116 round 1b: an install from before 0.1.4 has Seldon's
/// hooks in the logbook's own `.claude/settings.json` only, which Claude
/// Code does not read in `~/Work`, where `agent start` now starts the
/// agent. Once, the hooks go into the user-wide settings with the merge of
/// `hook install` (foreign hooks and keys kept), recorded as Seldon's own
/// write under a watched path. Then, or when the user-wide file already
/// holds one of Seldon's hooks, [`MIGRATED_MARKER`] is written: a user who
/// takes the user-wide hooks out later keeps it so. Nothing without hooks
/// in the logbook's file (`doctor` names the fix). The caller holds the
/// state lock and runs as the user, never as root.
pub fn migrate_to_user_wide(
    lock: &Lock,
    ctx: &Context,
    config: &Config,
    logbook: &Logbook,
) -> Migration {
    let mut out = Migration::default();
    let marker = ctx.dirs.state_dir.join(MIGRATED_MARKER);
    if marker.exists() {
        return out;
    }
    let local = logbook.path(LOGBOOK_SETTINGS);
    if !claude_hooks_in(&local).is_ok_and(|n| n > 0) {
        return out;
    }
    let user = user_settings_file(&ctx.dirs);
    let shown = ctx.dirs.display(&user);
    match claude_hooks_in(&user) {
        Ok(0) => {}
        Ok(_) => {
            write_marker(&marker, &mut out);
            return out;
        }
        Err(why) => {
            out.warnings.push(format!(
                "Claude Code's hooks were not added to {shown}: it {why}; fix it, then seldon hook install claude-code"
            ));
            return out;
        }
    }
    match merge_claude_hooks(&user, &shown) {
        Ok(merged) => {
            if !merged.added.is_empty()
                && let Err(e) = super::setup::record_own_writes_under(
                    lock,
                    ctx,
                    config,
                    std::slice::from_ref(&user),
                    "seldon capture",
                    OwnOp::Install,
                )
            {
                out.warnings
                    .push(format!("{shown}: {}", super::setup::own_writes_warning(&e)));
            }
            out.added_to = Some(user);
            write_marker(&marker, &mut out);
        }
        Err(e) => out.warnings.push(format!(
            "Claude Code's hooks were not added to {shown}: {e}"
        )),
    }
    out
}

fn write_marker(marker: &Path, out: &mut Migration) {
    let text =
        "Seldon's Claude Code hooks are user-wide (WP-116); a capture does not add them again.\n";
    if let Err(e) = crate::sys::create_dir_private(marker.parent().unwrap_or(marker))
        .and_then(|()| std::fs::write(marker, text))
    {
        out.warnings
            .push(format!("cannot write {}: {e}", marker.display()));
    }
}

/// Claude Code's configuration folder, when not `~/.claude`.
pub const CLAUDE_CONFIG_ENV: &str = "CLAUDE_CONFIG_DIR";

/// The user-wide Claude Code settings file, where `hook install
/// claude-code` and `init --harness claude-code` put the hooks (ADR-0030
/// §1): `$CLAUDE_CONFIG_DIR/settings.json` when it is set, else
/// `~/.claude/settings.json`.
pub fn user_settings_file(dirs: &Dirs) -> PathBuf {
    std::env::var(CLAUDE_CONFIG_ENV)
        .ok()
        .filter(|d| !d.is_empty())
        .map_or_else(|| dirs.home.join(".claude"), |d| dirs.expand(&d))
        .join("settings.json")
}

/// The logbook's own Claude Code settings file, the default before
/// ADR-0030 (project settings: read only by a session in the logbook).
pub const LOGBOOK_SETTINGS: &str = ".claude/settings.json";

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

    /// An empty watch path watches nothing, as in the config collector
    /// (`Dirs::expand_config`), not the whole home directory.
    #[test]
    fn an_empty_watch_path_watches_nothing() {
        let dirs = Dirs {
            home: "/home/user".into(),
            xdg_config_home: "/home/user/.config".into(),
            state_dir: "/home/user/.local/state/seldon".into(),
        };
        let config = Config {
            watch_paths: vec![String::new(), "  ".into(), "notes".into()],
            ..Config::default()
        };
        let scope = Scope::new(&dirs, &config, Path::new("/home/user/Seldon"));
        assert!(!scope.is_watched(Path::new("/home/user/.profile")));
        assert!(scope.is_watched(Path::new("/home/user/notes/a.md")));
        assert!(scope.is_watched(Path::new("/home/user/.config/systemd/user/a.service")));
        let roots: Vec<PathBuf> = config
            .watch_paths
            .iter()
            .filter_map(|p| dirs.expand_config(p))
            .collect();
        assert_eq!(scope.watched[..scope.watched.len() - 1], roots);
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
