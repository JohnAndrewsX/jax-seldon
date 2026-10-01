//! `seldon` — the Seldon engine CLI (SPEC-ENGINE.md §3).
//!
//! Every command accepts the global `--json` flag; exit codes follow
//! SPEC-ENGINE §3 (0 ok, 1 user error, 2 engine error, 3 logbook not
//! initialised, 4 lock held).

use std::ffi::OsString;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::error::ErrorKind;
use clap::{Args, CommandFactory, Parser, Subcommand};
use serde_json::json;

use seldon::commands::{self, Context, Output, capture::CaptureArgs, init::InitArgs};
use seldon::error::{Error, Exit};
use seldon::model::Language;
use seldon::{CONTRACT_VERSION, VERSION};

#[derive(Debug, Parser)]
#[command(
    name = "seldon",
    about = "Flight recorder and planning desk for your Omarchy system",
    disable_version_flag = true
)]
struct Cli {
    /// Print the engine version
    #[arg(short = 'V', long = "version")]
    version: bool,

    /// Machine-readable output
    #[arg(long, global = true)]
    json: bool,

    /// Logbook directory (overrides config.toml and SELDON_LOGBOOK)
    #[arg(long, global = true, value_name = "DIR")]
    logbook: Option<PathBuf>,

    /// No human output on success
    #[arg(long, global = true)]
    quiet: bool,

    /// Do not commit logbook changes to git
    #[arg(long, global = true)]
    no_commit: bool,

    /// Config file (overrides SELDON_CONFIG and ~/.config/seldon/config.toml)
    #[arg(long, global = true, value_name = "FILE")]
    config: Option<PathBuf>,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Print the engine/plugin contract version
    ContractVersion,

    /// Create a logbook (wizard; --non-interactive takes defaults)
    Init(InitCmd),

    /// Check engine, config, logbook, omarchy, snapper and git
    Doctor {
        /// Logbook to check (same as the global --logbook)
        #[arg(long, value_name = "DIR")]
        path: Option<PathBuf>,
    },

    /// Run collectors and append new events to the ledger
    Capture {
        /// Collectors to run, comma-separated (default: every enabled one)
        #[arg(
            long,
            value_name = "NAMES",
            value_delimiter = ',',
            conflicts_with = "all"
        )]
        source: Vec<String>,
        /// Run every enabled collector (the default)
        #[arg(long)]
        all: bool,
        /// Baseline for collectors without a cursor, RFC 3339 (default: logbook creation)
        #[arg(long, value_name = "TS")]
        since: Option<String>,
    },

    /// Write a note: a ledger event and a journal entry
    Log(commands::log::LogArgs),

    /// Record an event by hand (hooks, scripts)
    Event(commands::event::EventArgs),

    /// Cases: new, start, verify, done, drop, list, show
    Plan(commands::plan::PlanArgs),

    /// Create a decision record (ADR) and open it in the editor
    Decide(commands::decide::DecideArgs),

    /// Print the path of a logbook file; --editor opens it
    Open(commands::open::OpenArgs),

    /// Rebuild index.json and the ledger/*.md views; --check validates
    Index(commands::index::IndexArgs),

    /// Regenerate STATUS.md, the ledger views and index.json; print a summary
    Status(commands::status::StatusArgs),

    /// Agent hooks: record commands, session context, install into a harness
    Hook(commands::hook::HookArgs),

    /// Open drift; link, explain, dismiss or show a drift event
    Drift(commands::drift::DriftArgs),

    /// Start an agent on an active case
    Agent(commands::agent::AgentArgs),

    /// Write outputs/REBUILD.md: the steps to rebuild this machine
    Rebuild(commands::rebuild::RebuildArgs),

    /// Rebuild index.json when the logbook changes (feature "watch")
    Watch(commands::watch::WatchArgs),
}

#[derive(Debug, Args)]
struct InitCmd {
    /// Logbook directory (default ~/Seldon)
    #[arg(long, value_name = "DIR")]
    path: Option<PathBuf>,

    /// Ask nothing; take flags, then the existing config, then the
    /// defaults: ~/Seldon, language from the locale, all collectors, git
    /// on, first capture from now on, no backfill, no theme hook
    #[arg(long)]
    non_interactive: bool,

    /// Language of the logbook prose
    #[arg(long, value_parser = ["en", "de"])]
    language: Option<String>,

    /// Add Obsidian settings (.obsidian/)
    #[arg(long)]
    obsidian: bool,

    /// Agent harness to set up (repeatable)
    #[arg(long, value_name = "NAME", value_parser = ["claude-code", "omarchy-agent"])]
    harness: Vec<String>,

    /// Backfill: the first capture also records changes since TS
    /// (YYYY-MM-DD or RFC 3339); each one opens as drift
    #[arg(long, value_name = "TS")]
    since: Option<String>,

    /// Mark the backfilled drift as the pre-Seldon baseline (dismissed)
    #[arg(long, requires = "since")]
    baseline: bool,

    /// Do not run the first capture
    #[arg(long, conflicts_with = "since")]
    no_capture: bool,

    /// Install Omarchy's theme-set hook (`omarchy hook install theme-set`)
    #[arg(long)]
    theme_hook: bool,

    /// Make the logbook a git repository with a first commit (default)
    #[arg(long, overrides_with = "no_git")]
    git: bool,

    /// Do not use git
    #[arg(long, overrides_with = "git")]
    no_git: bool,
}

fn main() -> ExitCode {
    let argv: Vec<OsString> = std::env::args_os().collect();
    let cli = match Cli::try_parse_from(&argv) {
        Ok(cli) => cli,
        Err(err) => return parse_error(&err, &argv),
    };
    // hooks an agent harness calls never block it: errors go to stderr, exit 0
    if let Some(Command::Hook(h)) = &cli.command
        && h.command.is_agent_hook()
    {
        commands::hook::run_agent_hook(
            || {
                Context::from_env(
                    cli.json,
                    cli.quiet,
                    cli.no_commit,
                    cli.logbook.clone(),
                    cli.config.clone(),
                )
            },
            h.command.clone(),
        );
        return ExitCode::SUCCESS;
    }
    let (json, quiet) = (cli.json, cli.quiet);
    match run(cli) {
        Ok(out) => {
            if json {
                println!("{}", out.json);
            } else if !(quiet && out.exit == Exit::Ok) && !out.human.is_empty() {
                println!("{}", out.human);
            }
            out.exit.into()
        }
        Err(err) => fail(json, err.exit(), &err.to_string()),
    }
}

fn run(cli: Cli) -> Result<Output, Error> {
    if cli.version {
        return Ok(Output::ok(
            format!("seldon {VERSION}"),
            json!({ "name": "seldon", "version": VERSION }),
        ));
    }
    let Some(command) = cli.command else {
        return Err(Error::user("no command given; see `seldon --help`"));
    };
    if let Command::ContractVersion = command {
        return Ok(Output::ok(
            CONTRACT_VERSION.to_string(),
            json!({ "contractVersion": CONTRACT_VERSION }),
        ));
    }

    let ctx = Context::from_env(cli.json, cli.quiet, cli.no_commit, cli.logbook, cli.config)?;
    match command {
        Command::ContractVersion => unreachable!("handled above"),
        Command::Init(c) => commands::init::run(
            &ctx,
            InitArgs {
                path: c.path,
                non_interactive: c.non_interactive,
                language: c
                    .language
                    .map(|l| l.parse::<Language>())
                    .transpose()
                    .map_err(Error::User)?,
                obsidian: c.obsidian,
                harnesses: c.harness,
                since: c
                    .since
                    .as_deref()
                    .map(commands::setup::parse_since)
                    .transpose()?,
                baseline: c.baseline,
                capture: !c.no_capture,
                theme_hook: c.theme_hook,
                git: match (c.git, c.no_git) {
                    (true, _) => Some(true),
                    (_, true) => Some(false),
                    _ => None,
                },
            },
        ),
        Command::Doctor { path } => commands::doctor::run(&ctx, path.as_deref()),
        Command::Capture { source, all, since } => commands::capture::run(
            &ctx,
            CaptureArgs {
                sources: source,
                all,
                since,
            },
        ),

        Command::Log(a) => commands::log::run(&ctx, a),
        Command::Event(a) => commands::event::run(&ctx, a),
        Command::Plan(a) => commands::plan::run(&ctx, a),
        Command::Decide(a) => commands::decide::run(&ctx, a),
        Command::Open(a) => commands::open::run(&ctx, a),
        Command::Index(a) => commands::index::run(&ctx, a),
        Command::Status(a) => commands::status::run(&ctx, a),
        Command::Hook(a) => commands::hook::run(&ctx, a),
        Command::Drift(a) => commands::drift::run(&ctx, a),
        Command::Agent(a) => commands::agent::run(&ctx, a),
        Command::Rebuild(a) => commands::rebuild::run(&ctx, a),
        Command::Watch(a) => commands::watch::run(&ctx, a),
    }
}

/// Prints an error (JSON on stdout or text on stderr) and returns its code.
fn fail(as_json: bool, exit: Exit, message: &str) -> ExitCode {
    if as_json {
        println!(
            "{}",
            json!({ "error": { "code": exit as u8, "message": message } })
        );
    } else {
        eprintln!("seldon: {message}");
    }
    exit.into()
}

/// clap reports `--help` as an "error"; real parse errors are user errors
/// (exit 1), not clap's default exit 2, which is reserved for engine errors.
fn parse_error(err: &clap::Error, argv: &[OsString]) -> ExitCode {
    match err.kind() {
        ErrorKind::DisplayHelp | ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand => {
            let _ = err.print();
            Exit::Ok.into()
        }
        _ if wants_json(argv) => fail(true, Exit::UserError, &detail(err)),
        _ => {
            let _ = err.print();
            Exit::UserError.into()
        }
    }
}

/// Whether the failed command line asked for `--json`. Not a search for
/// the string: the tokens are read with the CLI's own definition, the way
/// clap reads them, so that a value is never mistaken for the flag:
///
/// - the value of an option (`--reason --json` when the option accepts
///   hyphen values; `--path json`);
/// - a positional value (`seldon log -x`: free text accepts hyphen values
///   unless the token is a known option, exactly as clap decides);
/// - anything after `--` (`seldon log -- --json`).
///
/// Unlike a re-parse it keeps going past invalid values and unknown
/// subcommands, which stop clap.
fn wants_json(argv: &[OsString]) -> bool {
    let mut root = Cli::command();
    root.build();
    let mut cmd = &root;
    // positional slots of `cmd` filled so far
    let mut filled = 0;
    let mut tokens = argv.iter().skip(1).map(|t| t.to_string_lossy()).peekable();
    while let Some(token) = tokens.next() {
        if token == "--" {
            return false;
        }
        let pending = cmd.get_positionals().nth(filled);
        let (known, attached) = if let Some(long) = token.strip_prefix("--") {
            let (name, value) = long
                .split_once('=')
                .map_or((long, None), |(n, v)| (n, Some(v)));
            if name == "json" {
                return true;
            }
            (
                cmd.get_arguments().find(|a| a.get_long() == Some(name)),
                value.is_some(),
            )
        } else if let Some(short) = token.strip_prefix('-').filter(|s| !s.is_empty()) {
            let c = short.chars().next();
            (
                cmd.get_arguments()
                    .find(|a| a.get_short() == c && !a.is_positional()),
                short.chars().count() > 1,
            )
        } else {
            if let Some(sub) = cmd.find_subcommand(token.as_ref()) {
                cmd = sub;
                filled = 0;
            } else if pending.is_some() {
                filled += 1;
            }
            continue;
        };
        match known {
            Some(arg) => {
                let next_is_value = tokens
                    .peek()
                    .is_some_and(|n| !n.starts_with('-') || arg.is_allow_hyphen_values_set());
                if arg.get_action().takes_values() && !attached && next_is_value {
                    tokens.next();
                }
            }
            // an unknown option-like token is the value of a positional
            // that accepts hyphen values (clap's rule), else an error clap
            // has already reported
            None if pending.is_some_and(|p| p.is_allow_hyphen_values_set()) => filled += 1,
            None => {}
        }
    }
    false
}

/// clap's message without the `error:` prefix and the usage footer, e.g.
/// "unrecognized subcommand 'bogus'" (SPEC-ENGINE §3 "JSON shapes").
fn detail(err: &clap::Error) -> String {
    let text = err.to_string();
    let text = text.strip_prefix("error: ").unwrap_or(&text);
    let end = ["\nUsage:", "\nFor more information"]
        .iter()
        .filter_map(|marker| text.find(marker))
        .min()
        .unwrap_or(text.len());
    text[..end].trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wants(args: &[&str]) -> bool {
        let argv: Vec<OsString> = std::iter::once("seldon")
            .chain(args.iter().copied())
            .map(OsString::from)
            .collect();
        wants_json(&argv)
    }

    #[test]
    fn json_flag_is_found_where_clap_reads_it() {
        assert!(wants(&["--json", "log"]));
        assert!(wants(&["log", "text", "--json"]));
        assert!(wants(&["log", "--json", "text"]));
        assert!(wants(&["plan", "new", "t", "--zone", "purple", "--json"]));
        assert!(wants(&["log", "-x", "--actor", "Bad", "--json"]));
        assert!(wants(&["--config", "c.toml", "log", "--json"]));
    }

    #[test]
    fn values_are_not_the_flag() {
        // after `--`
        assert!(!wants(&["log", "--", "--json"]));
        assert!(!wants(&["log", "--actor", "Bad", "--", "--json"]));
        // option values that accept hyphens
        assert!(!wants(&[
            "plan",
            "drop",
            "C-2026-001",
            "--reason",
            "--json"
        ]));
        assert!(!wants(&[
            "event",
            "pacman",
            "install",
            "--subject",
            "s",
            "--detail",
            "--json"
        ]));
        assert!(!wants(&[
            "event",
            "theme",
            "theme-set",
            "--subject",
            "--json"
        ]));
        // `json` as the value of an option, and as a positional
        assert!(!wants(&["init", "--path", "json", "--language", "xx"]));
        assert!(!wants(&["log", "json", "--actor", "Bad"]));
        // a positional value that is a subcommand name elsewhere
        assert!(!wants(&["plan", "new", "list", "--zone", "purple"]));
        // `--json=…` attached to another option's name is not the flag
        assert!(!wants(&["log", "--reason=--json"]));
    }
}
