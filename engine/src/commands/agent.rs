//! `seldon agent start <caseId> [--launcher NAME]` (WP-022): sends an agent
//! to work an active case.
//!
//! The case becomes the active case (`.seldon/active-case`), so the agent's
//! recorded commands land on it. The prompt is one line naming the case and
//! the logbook, then the context block of `seldon hook session-start`. The
//! launcher is an argv list from `config.toml` (`[agent] launcher`, or
//! `[agent.launchers] NAME` with `--launcher NAME`; default
//! [`DEFAULT_AGENT_LAUNCHER`]); its element `{prompt}` is replaced by the
//! prompt as one argument. Nothing goes through a shell (AGENTS.md §8):
//! the launcher is checked before anything is written, and a launcher that
//! runs its arguments as shell code is refused.
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
use super::{CONFIG_ENV, Context, Output, hook};
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

/// Programs that run their arguments as shell code: shells, Omarchy
/// launchers that join their arguments into a `bash -c` or `eval` string,
/// and `hyprctl` (`dispatch exec` takes a shell string). The prompt carries
/// logbook text, so it is never handed to one of them. A heuristic, not a
/// sandbox: interpreters (`python -c`, `perl -e`) and `xargs` are not
/// listed; the config is the user's own file.
const SHELL_STRING_RUNNERS: [&str; 16] = [
    "sh",
    "bash",
    "zsh",
    "dash",
    "ksh",
    "mksh",
    "fish",
    "nu",
    "xonsh",
    "eval",
    "omarchy-launch-floating-terminal-with-presentation",
    "omarchy-launch-or-focus",
    "omarchy-launch-or-focus-tui",
    "omarchy-launch-or-focus-webapp",
    // runs `tmux attach || tmux new` in `bash -c` and drops its arguments
    "omarchy-launch-terminal-tmux",
    "hyprctl",
];

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
    /// path first, `{prompt}` exactly once as a whole element, and no shell
    /// that would read the prompt as code.
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
        let base = |a: &str| a.rsplit('/').next().unwrap_or(a).to_string();
        if let Some(shell) = before
            .iter()
            .find(|a| SHELL_STRING_RUNNERS.contains(&base(a).as_str()))
        {
            return bad(format!(
                "`{shell}` runs its arguments as shell code; the prompt is never handed to it"
            ));
        }
        // `omarchy launch …` dispatches to `omarchy-launch-*` by route, so
        // the list above cannot see which one: name the launcher directly
        if before
            .windows(2)
            .any(|w| base(&w[0]) == "omarchy" && w[1] == "launch")
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
    let launched = hook::session_start(ctx).and_then(|block| {
        let prompt = format!(
            "Work case {id} in the Seldon logbook at {}; every mutating command is recorded.\n\n{block}",
            logbook.root.display()
        );
        launch(ctx, &logbook, &launcher, &prompt).map_err(Error::User)
    });
    if let Err(e) = launched {
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
            assert!(refused(shell).contains("shell code"), "{shell:?}");
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
}
