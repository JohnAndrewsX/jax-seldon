//! `seldon agent start <caseId> [--launcher NAME]` (WP-022): sends an agent
//! to work an active case. `seldon agent start --new -- "<intent>"`
//! (WP-101, ADR-0027 §6) first creates and starts the case from one
//! sentence ([`title_of`], [`super::plan::create`]) under the same lock
//! hold; the built-in launcher without an Omarchy default agent is refused
//! before anything is written ([`check_default_agent`]).
//!
//! The case becomes the active case (`.seldon/active-case`), so the agent's
//! recorded commands land on it. The prompt holds no logbook text: it
//! names the case id and the logbook path and points the agent to
//! `seldon hook session-start` and `seldon plan show <id>`
//! ([`prompt`]). It is one argument of the launcher, so it shows in the
//! process list and in a session journal that logs the launch. The
//! launcher is an argv list from `config.toml` (`[agent] launcher`, or
//! `[agent.launchers] NAME` with `--launcher NAME`; default
//! [`DEFAULT_AGENT_LAUNCHER`]); its element `{prompt}` is replaced by the
//! prompt as one argument. Nothing goes through a shell (AGENTS.md §8):
//! the launcher is checked before anything is written, and a launcher whose
//! program is known to run its arguments as code is refused (a check by
//! program name, not a sandbox).
//!
//! The launcher starts detached ([`launch_detached`], as `open --editor`)
//! where `omarchy agent prompt` would start the agent (ADR-0030 §2,
//! [`start_dir`]): in the folder `agent start` was called in, or `~/Work`
//! (else `$HOME`) when that is `$HOME`, `/` or gone; `[agent] workdir =
//! "logbook"` starts it in the logbook. Its environment holds
//! `SELDON_LOGBOOK` (the logbook), `SELDON_ACTOR=agent:<launcher name>`
//! ([`Launcher::actor`]; the actor of the agent's `seldon` calls that give
//! no `--actor`), `SELDON_ATTENDED=1` (ADR-0027 §2d: the session has a
//! user; the agent's rules read it, the engine never does) (WP-096) and
//! `SELDON_CASE=<id>`, the marker by which the hooks serve the session
//! wherever it works (ADR-0030 §1).
//! Its stderr goes to `<state>/agent-launch.log`, so a launcher that fails
//! at once is reported with its message. On a failure the previous active
//! case is restored.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use clap::{Args, Subcommand};
use serde_json::json;

use super::event::{ACTOR_ENV, actor_or_env, parse_case_id, parse_person};
use super::open::launch_detached;
use super::{CONFIG_ENV, Context, Output, autocommit, required_text};
use crate::config::{AgentConfig, AgentWorkdir, DEFAULT_AGENT_LAUNCHER, LOGBOOK_ENV};
use crate::error::{Error, Result};
use crate::logbook::{Logbook, cases};
use crate::model::event::ACTOR_HUMAN;
use crate::model::{CaseStatus, Priority, Risk, Zone};
use crate::redact::Redactor;
use crate::sys::Run;

/// The launcher element replaced by the prompt.
pub const PROMPT: &str = "{prompt}";

/// The name of `[agent] launcher` (what runs without `--launcher`).
pub const DEFAULT_NAME: &str = "default";

/// The name of the built-in launcher, [`DEFAULT_AGENT_LAUNCHER`], which
/// `--launcher omarchy` reaches even when `[agent] launcher` is changed.
pub const OMARCHY_NAME: &str = "omarchy";

/// Set to `1` for the launched agent: a session `seldon agent start`
/// launched is attended (ADR-0027 §2d). Nothing in the engine reads it.
pub const ATTENDED_ENV: &str = "SELDON_ATTENDED";

/// Set to the case id for the launched agent: the hooks serve a session
/// that carries it wherever it works (ADR-0030 §1). Nothing else in the
/// engine reads it; hook attribution stays with `.seldon/active-case`.
pub const CASE_ENV: &str = "SELDON_CASE";

/// The folder `omarchy-agent` moves to from `$HOME`, below the home
/// directory.
pub const WORK_DIR: &str = "Work";

/// Where the launcher starts (ADR-0030 §2): `here`, the folder `agent
/// start` was called in, unless it is `$HOME`, `/` or not a directory
/// (`None`: gone); then `~/Work` when it is a directory, else `home`. The
/// rule of `omarchy-agent`, so the agent gets the same trusted folder as
/// from `omarchy agent prompt` whichever launcher runs it.
pub fn start_dir(here: Option<&Path>, home: &Path) -> PathBuf {
    let same = |a: &Path, b: &Path| {
        a == b
            || matches!(
                (std::fs::canonicalize(a), std::fs::canonicalize(b)),
                (Ok(x), Ok(y)) if x == y
            )
    };
    match here {
        Some(dir) if dir.is_dir() && !same(dir, home) && !same(dir, Path::new("/")) => {
            dir.to_path_buf()
        }
        _ => {
            let work = home.join(WORK_DIR);
            if work.is_dir() {
                work
            } else {
                home.to_path_buf()
            }
        }
    }
}

/// Where the launcher's stderr goes, in the state directory.
pub const LAUNCH_LOG: &str = "agent-launch.log";

/// Programs that can run their arguments as code: shells, interpreters,
/// programs that hand a string to a shell (`script -c`, `watch`, `flock -c`,
/// `su -c`, `ssh`, `tmux`, `screen`, `xargs`, `parallel`, the compositors'
/// `exec` messages), Omarchy launchers that join their arguments into a
/// `bash -c` or `eval` string, and `hyprctl` (`dispatch exec` takes a shell
/// string). The prompt is never handed to one of them. A heuristic by
/// program name, not a sandbox: the config is the user's own file, and a
/// program missing here is not checked. Names are compared without a
/// version suffix (`python3.12` is `python`, [`program_name`]).
const CODE_RUNNERS: &[&str] = &[
    // shells
    "sh",
    "bash",
    "rbash",
    "zsh",
    "dash",
    "ash",
    "ksh",
    "mksh",
    "oksh",
    "loksh",
    "pdksh",
    "yash",
    "posh",
    "csh",
    "tcsh",
    "fish",
    "nu",
    "xonsh",
    "elvish",
    "osh",
    "ysh",
    "rc",
    "pwsh",
    "busybox",
    "toybox",
    "eval",
    // run a string through a shell
    "script",
    "watch",
    "flock",
    "su",
    "runuser",
    "ssh",
    "mosh",
    "tmux",
    "screen",
    "xargs",
    "parallel",
    "swaymsg",
    "i3-msg",
    "niri",
    // interpreters
    "python",
    "pypy",
    "perl",
    "ruby",
    "irb",
    "node",
    "nodejs",
    "deno",
    "bun",
    "php",
    "lua",
    "luajit",
    "tclsh",
    "wish",
    "expect",
    "awk",
    "gawk",
    "mawk",
    "nawk",
    "omarchy-launch-floating-terminal-with-presentation",
    "omarchy-launch-or-focus",
    "omarchy-launch-or-focus-tui",
    "omarchy-launch-or-focus-webapp",
    // runs `tmux attach || tmux new` in `bash -c` and drops its arguments
    "omarchy-launch-terminal-tmux",
    "hyprctl",
];

/// Programs that run a string as code with one of their options: `env -S`
/// splits a string into a command line, `sudo -s`/`-i` runs it in a shell.
/// An element before `{prompt}` that is the long option, or a short option
/// cluster with one of the letters, is refused.
const CODE_OPTIONS: [(&str, &[char], &[&str]); 2] = [
    ("env", &['S'], &["--split-string"]),
    ("sudo", &['s', 'i'], &["--shell", "--login"]),
];

/// The name a program is checked by: the base name without a version
/// suffix of digits and dots (`/usr/bin/python3.12` is `python`).
fn program_name(arg: &str) -> &str {
    let base = arg.rsplit('/').next().unwrap_or(arg);
    match base.trim_end_matches(|c: char| c.is_ascii_digit() || c == '.') {
        "" => base,
        name => name,
    }
}

/// Whether `arg` is one of `long` (alone or with `=`) or a short option
/// cluster (`-xyz`, not `--`) with one of `short`.
fn has_option(arg: &str, short: &[char], long: &[&str]) -> bool {
    if long
        .iter()
        .any(|l| arg == *l || arg.strip_prefix(l).is_some_and(|r| r.starts_with('=')))
    {
        return true;
    }
    arg.strip_prefix('-')
        .filter(|r| !r.starts_with('-'))
        .is_some_and(|r| r.chars().any(|c| short.contains(&c)))
}

/// `seldon agent <command>`.
#[derive(Debug, Clone, Args)]
pub struct AgentArgs {
    #[command(subcommand)]
    pub command: AgentCommand,
}

#[derive(Debug, Clone, Subcommand)]
pub enum AgentCommand {
    /// Launch an agent on an active case, with the case as the active case
    /// and a prompt that names the case and the logbook; with --new, create
    /// and start the case from one sentence first
    #[command(after_help = "Examples:\n  seldon agent start C-2026-004\n  \
                            seldon agent start --new -- \"Install zed as a second editor\"")]
    Start(StartArgs),
}

#[derive(Debug, Clone, Args)]
pub struct StartArgs {
    /// The case (must be active)
    #[arg(
        value_name = "ID",
        value_parser = parse_case_id,
        required_unless_present = "new",
        conflicts_with = "new"
    )]
    pub id: Option<String>,

    /// Create and start a case from the text after `--` (title: its first
    /// sentence; Intent: the whole text), then launch the agent on it
    #[arg(long, requires = "intent")]
    pub new: bool,

    /// With --new: green, yellow or red
    #[arg(long, value_name = "ZONE", default_value = "yellow", requires = "new")]
    pub zone: Zone,

    /// With --new: R0 to R3
    #[arg(long, value_name = "RISK", default_value = "R1", requires = "new")]
    pub risk: Risk,

    /// With --new: area slug; created under areas/ on first use
    #[arg(long, value_name = "AREA", requires = "new")]
    pub area: Option<String>,

    /// A launcher from `[agent.launchers]` in config.toml; `omarchy`
    /// is the built-in one (default: `[agent] launcher`)
    #[arg(long, value_name = "NAME")]
    pub launcher: Option<String>,

    /// With --new: what the agent should do, as one argument after `--`
    #[arg(value_name = "INTENT", last = true, requires = "new")]
    pub intent: Option<String>,
}

pub fn run(ctx: &Context, args: AgentArgs) -> Result<Output> {
    match args.command {
        AgentCommand::Start(a) => match (a.new, a.id, a.intent) {
            (true, _, Some(intent)) => {
                let new = New {
                    intent,
                    zone: a.zone,
                    risk: a.risk,
                    area: a.area,
                };
                start(ctx, Target::New(new), a.launcher.as_deref())
            }
            (false, Some(id), _) => start(ctx, Target::Case(id), a.launcher.as_deref()),
            // clap's requires/conflicts rule the other shapes out
            _ => Err(Error::user(
                "give a case id, or --new -- \"<intent>\"".to_string(),
            )),
        },
    }
}

/// What `agent start` works on: an active case, or a new one.
enum Target {
    Case(String),
    New(New),
}

/// `agent start --new`: the case to create and start.
struct New {
    intent: String,
    zone: Zone,
    risk: Risk,
    area: Option<String>,
}

/// The longest title `--new` derives, in characters.
pub const TITLE_MAX: usize = 72;

/// The title of a case made from an intent (ADR-0027 §6): its first
/// sentence — up to the first line break, or the first `.`, `!` or `?`
/// followed by white space or the end — without a final `.`, at most
/// [`TITLE_MAX`] characters, cut at a word with `…` when longer.
pub fn title_of(intent: &str) -> String {
    let text = intent.trim();
    let line = text.split('\n').next().unwrap_or("");
    // a lone `\r`, a tab or any other control character is a space, and a
    // run of white space one (`plan new` refuses `\r` in a title)
    let line = line
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let chars: Vec<char> = line.chars().collect();
    let mut end = chars.len();
    for (i, c) in chars.iter().enumerate() {
        if matches!(c, '.' | '!' | '?') && chars.get(i + 1).is_none_or(|n| n.is_whitespace()) {
            end = if *c == '.' { i } else { i + 1 };
            break;
        }
    }
    let sentence: String = chars[..end].iter().collect();
    let sentence = sentence.trim();
    if sentence.chars().count() <= TITLE_MAX {
        return sentence.to_string();
    }
    let head: Vec<char> = sentence.chars().take(TITLE_MAX - 1).collect();
    let cut = head
        .iter()
        .rposition(|c| c.is_whitespace())
        .filter(|&i| i > 0)
        .unwrap_or(head.len());
    let mut title: String = head[..cut].iter().collect();
    title = title.trim_end().to_string();
    title.push('…');
    title
}

/// Omarchy's default agent file, which `omarchy agent prompt` reads
/// (`omarchy-default-agent`): without it, the built-in launcher exits at
/// once.
const OMARCHY_DEFAULT_AGENT: &str = ".config/omarchy/defaults/agent";

/// Refuses the built-in launcher when Omarchy has no default agent, before
/// `--new` writes anything (ADR-0027 §6). Read-only; any other launcher is
/// not checked.
fn check_default_agent(ctx: &Context, launcher: &Launcher) -> Result<()> {
    if launcher.argv != DEFAULT_AGENT_LAUNCHER {
        return Ok(());
    }
    // `omarchy-default-agent` reads the first line (`read -r agent`)
    let file = ctx.dirs.home.join(OMARCHY_DEFAULT_AGENT);
    let set = std::fs::read_to_string(&file)
        .is_ok_and(|t| t.lines().next().is_some_and(|l| !l.trim().is_empty()));
    if set {
        return Ok(());
    }
    Err(Error::user(format!(
        "no default agent: Omarchy has none set, so `omarchy agent prompt` cannot start one; \
         nothing was created. Fix: `omarchy default agent <name>` (e.g. claude), or set \
         `[agent] launcher` in {}",
        ctx.dirs.display(&ctx.config_file)
    )))
}

/// A launcher from the config, checked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Launcher {
    pub name: String,
    pub argv: Vec<String>,
}

impl Launcher {
    /// The launcher `name` names (`None`: `[agent] launcher`), checked.
    pub fn resolve(agent: &AgentConfig, name: Option<&str>) -> Result<Launcher> {
        let name = name.unwrap_or(DEFAULT_NAME);
        let argv = match agent.launchers.get(name) {
            Some(argv) => argv.clone(),
            None if name == DEFAULT_NAME => agent.launcher.clone(),
            None if name == OMARCHY_NAME => DEFAULT_AGENT_LAUNCHER.map(String::from).to_vec(),
            None => {
                let mut known = vec![DEFAULT_NAME.to_string(), OMARCHY_NAME.to_string()];
                known.extend(agent.launchers.keys().cloned());
                return Err(Error::user(format!(
                    "unknown launcher `{name}`; known: {} (`[agent.launchers]` in config.toml)",
                    known.join(", ")
                )));
            }
        };
        let launcher = Launcher {
            name: name.to_string(),
            argv,
        };
        launcher.check()?;
        Ok(launcher)
    }

    /// The rules of the config: a program name without `/` or an absolute
    /// path first, `{prompt}` exactly once as a whole element, and no
    /// program before it that is known to run its arguments as code.
    fn check(&self) -> Result<()> {
        let bad = |why: String| {
            Err(Error::user(format!(
                "launcher `{}` in config.toml: {why}",
                self.name
            )))
        };
        let Some(program) = self.argv.first() else {
            return bad("it is empty".into());
        };
        if program.is_empty()
            || program == PROMPT
            || (program.contains('/') && !program.starts_with('/'))
        {
            return bad(format!(
                "`{program}` must be a program name without `/` or an absolute path"
            ));
        }
        let at = match self
            .argv
            .iter()
            .enumerate()
            .filter(|(_, a)| a.contains(PROMPT))
            .collect::<Vec<_>>()
            .as_slice()
        {
            [(i, a)] if *a == PROMPT => *i,
            [] => return bad(format!("it has no `{PROMPT}` element")),
            [_] => return bad(format!("`{PROMPT}` must be a whole element")),
            _ => return bad(format!("`{PROMPT}` must appear once")),
        };
        let before = &self.argv[..at];
        const HEURISTIC: &str =
            "the prompt is never handed to it (a heuristic check by program name, not a sandbox)";
        if let Some(runner) = before
            .iter()
            .find(|a| CODE_RUNNERS.contains(&program_name(a)))
        {
            return bad(format!(
                "`{runner}` can run its arguments as code; {HEURISTIC}"
            ));
        }
        for (i, a) in before.iter().enumerate() {
            let Some((program, short, long)) =
                CODE_OPTIONS.iter().find(|(p, ..)| program_name(a) == *p)
            else {
                continue;
            };
            if let Some(option) = before[i + 1..].iter().find(|o| has_option(o, short, long)) {
                return bad(format!(
                    "`{program} {option}` can run its arguments as code; {HEURISTIC}"
                ));
            }
        }
        // `omarchy launch …` dispatches to `omarchy-launch-*` by route, so
        // the list above cannot see which one: name the launcher directly
        if before
            .windows(2)
            .any(|w| program_name(&w[0]) == "omarchy" && w[1] == "launch")
        {
            return bad(
                "`omarchy launch …` is refused; name the launcher itself, e.g. `omarchy-launch-tui`"
                    .into(),
            );
        }
        Ok(())
    }

    /// The actor of the launched agent: `agent:` and the launcher's name,
    /// lowercased, every run of characters other than `a-z` and `0-9`
    /// one `-`, none at either end (`Claude Code` is `agent:claude-code`,
    /// the built-in names `agent:default` and `agent:omarchy`). A name
    /// with no letter or digit `a-z`/`0-9` is refused.
    pub fn actor(&self) -> Result<String> {
        let mut slug = String::new();
        for c in self.name.chars().map(|c| c.to_ascii_lowercase()) {
            if c.is_ascii_lowercase() || c.is_ascii_digit() {
                slug.push(c);
            } else if !slug.is_empty() && !slug.ends_with('-') {
                slug.push('-');
            }
        }
        let slug = slug.trim_end_matches('-');
        if slug.is_empty() {
            return Err(Error::user(format!(
                "launcher `{}` in config.toml: its name gives no actor (agent:<name>, \
                 a name with a-z or 0-9); rename it in `[agent.launchers]`",
                self.name.escape_debug()
            )));
        }
        Ok(format!("agent:{slug}"))
    }

    /// The program's arguments, the prompt in place of `{prompt}`.
    pub fn args<'a>(&'a self, prompt: &'a str) -> impl Iterator<Item = &'a str> {
        self.argv[1..]
            .iter()
            .map(move |a| if a == PROMPT { prompt } else { a.as_str() })
    }
}

fn start(ctx: &Context, target: Target, name: Option<&str>) -> Result<Output> {
    // the creator of a --new case: the caller (default human)
    let creator = match &target {
        Target::New(_) => Some(actor_or_env(None, parse_person, ACTOR_HUMAN)?),
        Target::Case(_) => None,
    };
    let (config, logbook) = ctx.open_logbook()?;
    // the launcher is checked before anything is written
    let launcher = Launcher::resolve(&config.agent, name)?;
    let actor = launcher.actor()?;
    let new = match target {
        Target::Case(id) => {
            let lock = ctx.lock()?;
            return launch_on(
                ctx,
                &logbook,
                &launcher,
                config.agent.workdir,
                &actor,
                &id,
                lock,
                None,
            );
        }
        Target::New(new) => new,
    };
    check_default_agent(ctx, &launcher)?;
    let intent = required_text("the intent", &new.intent)?;
    let redactor = Redactor::for_config(&config)?;
    let intent = redactor.redact(&intent);
    let title = title_of(&intent);
    let intent = cases::escape_lines(&intent);
    // `.`, `!` or `…` alone say nothing to do (WP-101 round 2)
    if !title.chars().any(char::is_alphanumeric) {
        return Err(Error::user(
            "the intent's first sentence has no letter or digit; start it with what to do"
                .to_string(),
        ));
    }
    let lock = ctx.lock()?;
    let created = super::plan::create(
        ctx,
        &config,
        &logbook,
        &lock,
        super::plan::Spec {
            title,
            zone: new.zone,
            risk: new.risk,
            area: new.area,
            priority: Priority::Normal,
            actor: creator.unwrap_or_else(|| ACTOR_HUMAN.to_string()),
            intent: Some(intent),
            tags: Vec::new(),
            note: None,
            start: true,
            point: true,
            done: None,
        },
    )?;
    let id = created.file.case.id.clone();
    let commit = autocommit(ctx, &config, &logbook, &format!("{id} created and started"));
    crate::index::rebuild_if_initialised(ctx);
    launch_on(
        ctx,
        &logbook,
        &launcher,
        config.agent.workdir,
        &actor,
        &id,
        lock,
        Some((&created, commit)),
    )
}

/// Launches `launcher` on case `id` under `lock`. `created`: the case
/// `--new` made under the same lock hold (it stays active when the launch
/// fails, and the error says how to retry).
#[allow(clippy::too_many_arguments)]
fn launch_on(
    ctx: &Context,
    logbook: &Logbook,
    launcher: &Launcher,
    workdir: AgentWorkdir,
    actor: &str,
    id: &str,
    lock: crate::logbook::lock::Lock,
    created: Option<(&super::plan::Created, super::Commit)>,
) -> Result<Output> {
    let file = cases::find(logbook, id)?;
    match file.case.status {
        CaseStatus::Active => {}
        CaseStatus::Queued => {
            // an imported case is started by the user, never on an
            // agent's launch (WP-102 round 3)
            super::plan::refuse_agent_start_of_imported(&file, actor)?;
            return Err(Error::user(format!(
                "{id} is queued; start it first: `seldon plan start {id}`"
            )));
        }
        other => {
            return Err(Error::user(format!(
                "{id} has status {other}; an agent starts on an active case only"
            )));
        }
    }

    let cwd = match workdir {
        AgentWorkdir::Logbook => logbook.root.clone(),
        AgentWorkdir::Inherit => start_dir(std::env::current_dir().ok().as_deref(), &ctx.dirs.home),
    };
    let previous = cases::active_case(logbook);
    cases::set_active_case(logbook, id)?;
    let launched = launch(
        ctx,
        logbook,
        launcher,
        Launch {
            actor,
            case: id,
            cwd: &cwd,
            prompt: &prompt(id, &logbook.root),
        },
    );
    if let Err(e) = launched {
        if created.is_some() {
            // the case holds the user's intent: it stays, active
            return Err(Error::user(format!(
                "{e}; {id} was created and started and stays active: fix the launcher, then \
                 run `seldon agent start {id}` (or Start agent on its card)"
            )));
        }
        restore(logbook, id, previous.as_deref());
        return Err(Error::User(e));
    }
    drop(lock);

    let program = &launcher.argv[0];
    let mut human = String::new();
    if let Some((c, _)) = &created {
        human.push_str(&format!(
            "Created and started {id} \"{}\" in {}\n",
            c.file.case.title,
            c.file.relative(logbook)
        ));
    }
    human.push_str(&format!(
        "Agent started on {id} with launcher `{}` ({program}) in {}, as {actor}",
        launcher.name,
        ctx.dirs.display(&cwd)
    ));
    let mut out = json!({
        "launched": true,
        "launcher": launcher.name,
        "program": program,
        "argv": launcher.argv,
        "actor": actor,
        "case": id,
        "cwd": cwd,
        "previousActiveCase": previous,
    });
    if let Some((c, commit)) = created {
        human.push_str(&commit.human());
        out["created"] = json!({
            "case": super::plan::case_json(logbook, &c.file),
            "events": c.events.iter().map(super::event::event_json).collect::<Vec<_>>(),
            "areaCreated": c.area_created,
            "git": commit.json(),
        });
    }
    Ok(Output::ok(human, out))
}

/// The launcher's prompt (ADR-0030 §3): the case id (checked by
/// `cases::find`), the logbook path from the config and fixed text. It
/// names the skill, and the rules file for a harness without skills (the
/// agent may start outside the logbook), as `omarchy-agent-crash` names its
/// skill. The agent reads the logbook context itself, so no logbook text is
/// in the process arguments.
pub fn prompt(id: &str, root: &Path) -> String {
    format!(
        "Work case {id} in the Seldon logbook at {}. Use the seldon skill; if your harness has \
         no skill mechanism, read {} (Seldon's rules) instead. First run `seldon hook \
         session-start` unless your harness already gave you the block `# Seldon logbook \
         context`, then `seldon plan show {id}`. Every mutating command is recorded.",
        root.display(),
        root.join(crate::logbook::rules::FILE).display()
    )
}

/// Puts `.seldon/active-case` back as it was before `id` was set.
fn restore(logbook: &Logbook, id: &str, previous: Option<&str>) {
    let restored = match previous {
        Some(p) if p == id => Ok(()),
        Some(p) => cases::set_active_case(logbook, p),
        None => cases::clear_active_case(logbook, id).map(|_| ()),
    };
    if let Err(e) = restored {
        eprintln!("seldon: cannot restore the active case: {e}");
    }
}

/// What one launch passes to the launcher besides its argv.
struct Launch<'a> {
    actor: &'a str,
    case: &'a str,
    cwd: &'a Path,
    prompt: &'a str,
}

/// Starts the launcher detached in `cwd`. `Err` is the message.
fn launch(
    ctx: &Context,
    logbook: &Logbook,
    launcher: &Launcher,
    how: Launch,
) -> Result<(), String> {
    let program = &launcher.argv[0];
    let mut cmd = Command::new(program);
    cmd.args(launcher.args(how.prompt))
        .current_dir(how.cwd)
        // a launcher that is no shell reads the folder from PWD
        .env("PWD", how.cwd)
        .env(LOGBOOK_ENV, &logbook.root)
        .env(ACTOR_ENV, how.actor)
        .env(ATTENDED_ENV, "1")
        .env(CASE_ENV, how.case);
    // the agent's own `seldon` calls read the config this one read
    if ctx.config_file != ctx.dirs.config_file() {
        cmd.env(CONFIG_ENV, &ctx.config_file);
    }
    // appended, never truncated: an earlier launcher may still write to it;
    // only what this launch adds after `start` is read back
    let log = ctx.dirs.state_dir.join(LAUNCH_LOG);
    let opened = crate::sys::create_dir_private(&ctx.dirs.state_dir).and_then(|()| {
        use std::os::unix::fs::OpenOptionsExt as _;
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .mode(crate::sys::NEW_FILE_MODE)
            .open(&log)
    });
    let start = opened
        .as_ref()
        .ok()
        .and_then(|f| f.metadata().ok())
        .map_or(0, |m| m.len());
    let stderr = opened.map_or_else(|_| Stdio::null(), Stdio::from);
    match launch_detached(cmd, stderr) {
        Run::Exited { code: Some(0), .. } => Ok(()),
        Run::Exited { code, .. } => Err(format!(
            "launcher `{}` ({program}) exited with {}{}",
            launcher.name,
            code.map_or("a signal".to_string(), |c| c.to_string()),
            last_lines(&log, start)
                .map(|t| format!(": {t}"))
                .unwrap_or_default()
        )),
        Run::NotFound => Err(format!(
            "launcher `{}`: `{program}` not found; set `[agent] launcher` in config.toml",
            launcher.name
        )),
        Run::TimedOut => Err(format!("{program} did not return")),
        Run::Failed(e) => Err(format!("cannot start {program}: {e}")),
    }
}

/// The last lines the launch log got after byte `start`, joined, if any.
fn last_lines(log: &Path, start: u64) -> Option<String> {
    let bytes = std::fs::read(log).ok()?;
    let text = String::from_utf8_lossy(bytes.get(start as usize..)?);
    let lines: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();
    let text = lines[lines.len().saturating_sub(3)..].join(" · ");
    (!text.is_empty()).then(|| text.chars().take(500).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn agent(launcher: &[&str]) -> AgentConfig {
        AgentConfig {
            launcher: launcher.iter().map(|s| s.to_string()).collect(),
            ..AgentConfig::default()
        }
    }

    fn refused(launcher: &[&str]) -> String {
        Launcher::resolve(&agent(launcher), None)
            .unwrap_err()
            .to_string()
    }

    #[test]
    fn the_default_is_omarchy_agent_prompt() {
        let l = Launcher::resolve(&AgentConfig::default(), None).unwrap();
        assert_eq!(l.name, "default");
        assert_eq!(l.argv, ["omarchy", "agent", "prompt", "{prompt}"]);
        let args: Vec<&str> = l.args("two words\nand a line").collect();
        assert_eq!(args, ["agent", "prompt", "two words\nand a line"]);
    }

    #[test]
    fn named_launchers() {
        let mut config = agent(&["/opt/bin/agent", "{prompt}"]);
        config
            .launchers
            .insert("claude".into(), vec!["claude".into(), "{prompt}".into()]);
        let get = |n| Launcher::resolve(&config, n);
        assert_eq!(get(None).unwrap().argv[0], "/opt/bin/agent");
        assert_eq!(get(Some("default")).unwrap().argv[0], "/opt/bin/agent");
        assert_eq!(get(Some("omarchy")).unwrap().argv[0], "omarchy");
        assert_eq!(get(Some("claude")).unwrap().name, "claude");
        let e = get(Some("codex")).unwrap_err().to_string();
        assert!(e.contains("known: default, omarchy, claude"), "{e}");
    }

    #[test]
    fn bad_launchers_are_refused() {
        assert!(refused(&[]).contains("empty"));
        assert!(refused(&["bin/agent", "{prompt}"]).contains("without `/`"));
        assert!(refused(&["./agent", "{prompt}"]).contains("without `/`"));
        assert!(refused(&["{prompt}"]).contains("program name"));
        assert!(refused(&["agent"]).contains("no `{prompt}`"));
        assert!(refused(&["agent", "--p={prompt}"]).contains("whole element"));
        assert!(refused(&["agent", "{prompt}", "{prompt}"]).contains("once"));
        for shell in [
            &["bash", "-c", "{prompt}"][..],
            &["/usr/bin/sh", "-c", "{prompt}"],
            &["env", "X=1", "zsh", "-c", "{prompt}"],
            &["omarchy-launch-tui", "bash", "-c", "{prompt}"],
            &[
                "omarchy-launch-floating-terminal-with-presentation",
                "claude",
                "{prompt}",
            ],
            &["omarchy-launch-or-focus-tui", "claude", "{prompt}"],
            &["omarchy-launch-or-focus-webapp", "{prompt}"],
            &[
                "/usr/share/omarchy/bin/omarchy-launch-or-focus",
                "x",
                "{prompt}",
            ],
            &["omarchy-launch-terminal-tmux", "{prompt}"],
            &["hyprctl", "dispatch", "exec", "{prompt}"],
        ] {
            assert!(
                refused(shell).contains("can run its arguments as code"),
                "{shell:?}"
            );
        }
        // the omarchy CLI's route to the same launchers, refused outright
        for route in [
            &["omarchy", "launch", "or-focus-tui", "claude", "{prompt}"][..],
            &["omarchy", "launch", "or", "focus", "tui", "{prompt}"],
            &["/usr/bin/omarchy", "launch", "tui", "claude", "{prompt}"],
            &["env", "X=1", "omarchy", "launch", "tui", "{prompt}"],
        ] {
            assert!(
                refused(route).contains("`omarchy launch …` is refused"),
                "{route:?}"
            );
        }
        // other omarchy routes stay allowed, as does `launch` after the prompt
        assert!(
            Launcher::resolve(&agent(&["omarchy", "agent", "prompt", "{prompt}"]), None).is_ok()
        );
        assert!(Launcher::resolve(&agent(&["omarchy", "{prompt}", "launch"]), None).is_ok());
        // after the prompt a shell name is only an argument
        assert!(Launcher::resolve(&agent(&["agent", "{prompt}", "bash"]), None).is_ok());
        assert!(
            Launcher::resolve(
                &agent(&["omarchy-launch-tui", "claude", "--", "{prompt}"]),
                None
            )
            .is_ok()
        );
    }

    #[test]
    fn programs_that_run_code_are_refused_by_name() {
        for name in [
            "rbash", "ash", "oksh", "loksh", "pdksh", "yash", "posh", "csh", "tcsh", "elvish",
            "osh", "ysh", "rc", "pwsh", "busybox", "toybox", "script", "watch", "flock", "su",
            "runuser", "ssh", "mosh", "tmux", "screen", "xargs", "parallel", "swaymsg", "i3-msg",
            "niri", "python", "pypy", "perl", "ruby", "irb", "node", "nodejs", "deno", "bun",
            "php", "lua", "luajit", "tclsh", "wish", "expect", "awk", "gawk", "mawk", "nawk",
        ] {
            for launcher in [
                &[name, "{prompt}"][..],
                &[name, "-c", "{prompt}"],
                &["env", "X=1", name, "{prompt}"],
                &["alacritty", "-e", name, "{prompt}"],
            ] {
                let e = refused(launcher);
                assert!(
                    e.contains(&format!("`{name}` can run its arguments as code"))
                        && e.contains("a heuristic check by program name, not a sandbox"),
                    "{launcher:?}: {e}"
                );
            }
        }
        // a version suffix or a path does not hide the name
        for (program, name) in [
            ("python3", "python3"),
            ("/usr/bin/python3.12", "/usr/bin/python3.12"),
            ("perl5.40", "perl5.40"),
            ("lua5.4", "lua5.4"),
            ("/bin/rbash", "/bin/rbash"),
        ] {
            let e = refused(&[program, "-c", "{prompt}"]);
            assert!(e.contains(&format!("`{name}` can run")), "{e}");
        }
        assert_eq!(program_name("/usr/bin/python3.12"), "python");
        assert_eq!(program_name("i3-msg"), "i3-msg");
        assert_eq!(program_name("2048"), "2048");
        // an agent whose name ends in digits is still an agent
        assert!(Launcher::resolve(&agent(&["claude2", "{prompt}"]), None).is_ok());
    }

    #[test]
    fn options_that_run_code_are_refused() {
        for (launcher, shown) in [
            (&["env", "-S", "{prompt}"][..], "`env -S`"),
            (&["/usr/bin/env", "-vS", "{prompt}"], "`env -vS`"),
            (
                &["env", "--split-string", "{prompt}"],
                "`env --split-string`",
            ),
            (
                &["env", "--split-string=x", "{prompt}"],
                "`env --split-string=x`",
            ),
            (&["sudo", "-s", "{prompt}"], "`sudo -s`"),
            (&["sudo", "-i", "{prompt}"], "`sudo -i`"),
            (&["sudo", "-Es", "{prompt}"], "`sudo -Es`"),
            (&["sudo", "--login", "{prompt}"], "`sudo --login`"),
            (&["sudo", "--shell", "{prompt}"], "`sudo --shell`"),
            (&["alacritty", "-e", "env", "-S", "{prompt}"], "`env -S`"),
        ] {
            let e = refused(launcher);
            assert!(
                e.contains(&format!("{shown} can run its arguments as code"))
                    && e.contains("heuristic"),
                "{launcher:?}: {e}"
            );
        }
        // plain `env` and `sudo` run an argv; options after the prompt are arguments
        for ok in [
            &["env", "X=1", "claude", "{prompt}"][..],
            &["env", "-u", "X", "claude", "{prompt}"],
            &["sudo", "-u", "agent", "claude", "{prompt}"],
            &["env", "claude", "{prompt}", "-S"],
            &["claude", "-s", "{prompt}"],
        ] {
            assert!(Launcher::resolve(&agent(ok), None).is_ok(), "{ok:?}");
        }
    }

    #[test]
    fn the_actor_is_the_launcher_name_as_a_slug() {
        let actor = |name: &str| {
            Launcher {
                name: name.into(),
                argv: vec!["agent".into(), PROMPT.into()],
            }
            .actor()
        };
        for (name, want) in [
            ("default", "agent:default"),
            ("omarchy", "agent:omarchy"),
            ("claude", "agent:claude"),
            ("claude-code", "agent:claude-code"),
            ("Claude Code", "agent:claude-code"),
            ("  Codex_CLI 2 ", "agent:codex-cli-2"),
            ("-x--y-", "agent:x-y"),
            ("gpt5", "agent:gpt5"),
            ("Agent/Ünï", "agent:agent-n"),
        ] {
            let got = actor(name).unwrap();
            assert_eq!(got, want, "{name:?}");
            assert!(crate::model::event::is_actor(&got), "{got}");
        }
        for name in ["", "-", "___", "ÄÖÜ", " \n"] {
            let e = actor(name).unwrap_err();
            assert!(matches!(e.exit(), crate::error::Exit::UserError));
            let e = e.to_string();
            assert!(
                e.contains("its name gives no actor (agent:<name>"),
                "{name:?}: {e}"
            );
        }
    }

    #[test]
    fn the_prompt_names_the_case_and_the_logbook_only() {
        let p = prompt("C-2026-007", Path::new("/home/u/Seldon"));
        assert_eq!(
            p,
            "Work case C-2026-007 in the Seldon logbook at /home/u/Seldon. Use the seldon skill; \
             if your harness has no skill mechanism, read /home/u/Seldon/AGENTS.md (Seldon's \
             rules) instead. First run `seldon hook session-start` unless your harness already \
             gave you the block `# Seldon logbook context`, then `seldon plan show C-2026-007`. \
             Every mutating command is recorded."
        );
        assert!(!p.contains('\n'));
    }

    /// `omarchy-agent`'s folder rule, extended to `/` and a folder that is
    /// gone (ADR-0030 §2).
    #[test]
    fn the_start_folder_follows_omarchy_agent() {
        let base = std::env::temp_dir().join(format!("seldon-start-dir-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let home = base.join("home");
        let project = home.join("src/project");
        std::fs::create_dir_all(&project).unwrap();
        let file = home.join("file");
        std::fs::write(&file, "").unwrap();
        let gone = home.join("gone");
        // without ~/Work: home
        assert_eq!(start_dir(Some(&project), &home), project);
        assert_eq!(start_dir(Some(&home), &home), home);
        assert_eq!(start_dir(Some(Path::new("/")), &home), home);
        assert_eq!(start_dir(None, &home), home);
        assert_eq!(start_dir(Some(&gone), &home), home);
        assert_eq!(start_dir(Some(&file), &home), home);
        // with ~/Work
        let work = home.join("Work");
        std::fs::create_dir(&work).unwrap();
        assert_eq!(start_dir(Some(&project), &home), project);
        assert_eq!(start_dir(Some(&home), &home), work);
        assert_eq!(start_dir(Some(&home.join("src/..")), &home), work);
        assert_eq!(start_dir(Some(Path::new("/")), &home), work);
        assert_eq!(start_dir(None, &home), work);
        assert_eq!(start_dir(Some(&gone), &home), work);
        assert_eq!(start_dir(Some(&work), &home), work);
        // home reached through a link is home
        let link = base.join("link");
        std::os::unix::fs::symlink(&home, &link).unwrap();
        assert_eq!(start_dir(Some(&link), &home), work);
        assert_eq!(start_dir(Some(&home), &link), link.join("Work"));
        std::fs::remove_dir_all(&base).unwrap();
    }
}
