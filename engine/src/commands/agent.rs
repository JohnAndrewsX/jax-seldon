//! `seldon agent start <caseId> [--launcher NAME]` (WP-022): sends an agent
//! to work an active case.
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
//! The launcher starts detached in the logbook directory, with
//! `SELDON_LOGBOOK` set to it ([`launch_detached`], as `open --editor`).
//! Its stderr goes to `<state>/agent-launch.log`, so a launcher that fails
//! at once is reported with its message. On a failure the previous active
//! case is restored.

use std::path::Path;
use std::process::{Command, Stdio};

use clap::{Args, Subcommand};
use serde_json::json;

use super::event::parse_case_id;
use super::open::launch_detached;
use super::{CONFIG_ENV, Context, Output};
use crate::config::{AgentConfig, DEFAULT_AGENT_LAUNCHER, LOGBOOK_ENV};
use crate::error::{Error, Result};
use crate::logbook::{Logbook, cases};
use crate::model::CaseStatus;
use crate::sys::Run;

/// The launcher element replaced by the prompt.
pub const PROMPT: &str = "{prompt}";

/// The name of `[agent] launcher` (what runs without `--launcher`).
pub const DEFAULT_NAME: &str = "default";

/// The name of the built-in launcher, [`DEFAULT_AGENT_LAUNCHER`], which
/// `--launcher omarchy` reaches even when `[agent] launcher` is changed.
pub const OMARCHY_NAME: &str = "omarchy";

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
    /// and `seldon hook session-start` as its prompt
    Start {
        /// The case (must be active)
        #[arg(value_name = "ID", value_parser = parse_case_id)]
        id: String,
        /// A launcher from `[agent.launchers]` in config.toml; `omarchy`
        /// is the built-in one (default: `[agent] launcher`)
        #[arg(long, value_name = "NAME")]
        launcher: Option<String>,
    },
}

pub fn run(ctx: &Context, args: AgentArgs) -> Result<Output> {
    match args.command {
        AgentCommand::Start { id, launcher } => start(ctx, &id, launcher.as_deref()),
    }
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

    /// The program's arguments, the prompt in place of `{prompt}`.
    pub fn args<'a>(&'a self, prompt: &'a str) -> impl Iterator<Item = &'a str> {
        self.argv[1..]
            .iter()
            .map(move |a| if a == PROMPT { prompt } else { a.as_str() })
    }
}

fn start(ctx: &Context, id: &str, name: Option<&str>) -> Result<Output> {
    let (config, logbook) = ctx.open_logbook()?;
    // the launcher is checked before anything is written
    let launcher = Launcher::resolve(&config.agent, name)?;
    let lock = ctx.lock()?;
    let file = cases::find(&logbook, id)?;
    match file.case.status {
        CaseStatus::Active => {}
        CaseStatus::Queued => {
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

    let previous = cases::active_case(&logbook);
    cases::set_active_case(&logbook, id)?;
    let launched = launch(ctx, &logbook, &launcher, &prompt(id, &logbook.root));
    if let Err(e) = launched.map_err(Error::User) {
        restore(&logbook, id, previous.as_deref());
        return Err(e);
    }
    drop(lock);

    let program = &launcher.argv[0];
    Ok(Output::ok(
        format!(
            "Agent started on {id} with launcher `{}` ({program}) in {}",
            launcher.name,
            ctx.dirs.display(&logbook.root)
        ),
        json!({
            "launched": true,
            "launcher": launcher.name,
            "program": program,
            "argv": launcher.argv,
            "case": id,
            "cwd": logbook.root,
            "previousActiveCase": previous,
        }),
    ))
}

/// The launcher's prompt: the case id (checked by `cases::find`), the
/// logbook path from the config and fixed text. The agent reads the
/// logbook context itself, so no logbook text is in the process arguments.
pub fn prompt(id: &str, root: &Path) -> String {
    format!(
        "Work case {id} in the Seldon logbook at {}. First run `seldon hook session-start` \
         (the logbook context) and `seldon plan show {id}` (the case file). Every mutating \
         command is recorded.",
        root.display()
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

/// Starts the launcher detached in the logbook. `Err` is the message.
fn launch(
    ctx: &Context,
    logbook: &Logbook,
    launcher: &Launcher,
    prompt: &str,
) -> Result<(), String> {
    let program = &launcher.argv[0];
    let mut cmd = Command::new(program);
    cmd.args(launcher.args(prompt))
        .current_dir(&logbook.root)
        .env(LOGBOOK_ENV, &logbook.root);
    // the agent's own `seldon` calls read the config this one read
    if ctx.config_file != ctx.dirs.config_file() {
        cmd.env(CONFIG_ENV, &ctx.config_file);
    }
    // appended, never truncated: an earlier launcher may still write to it;
    // only what this launch adds after `start` is read back
    let log = ctx.dirs.state_dir.join(LAUNCH_LOG);
    let opened = std::fs::create_dir_all(&ctx.dirs.state_dir).and_then(|()| {
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
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
    fn the_prompt_names_the_case_and_the_logbook_only() {
        let p = prompt("C-2026-007", Path::new("/home/u/Seldon"));
        assert_eq!(
            p,
            "Work case C-2026-007 in the Seldon logbook at /home/u/Seldon. First run \
             `seldon hook session-start` (the logbook context) and `seldon plan show C-2026-007` \
             (the case file). Every mutating command is recorded."
        );
        assert!(!p.contains('\n'));
    }
}
