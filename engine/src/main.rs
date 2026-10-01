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

use seldon::commands::{self, Context, Output, init::InitArgs};
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
}

#[derive(Debug, Args)]
struct InitCmd {
    /// Logbook directory (default ~/Seldon)
    #[arg(long, value_name = "DIR")]
    path: Option<PathBuf>,

    /// Ask nothing; take flags and defaults
    #[arg(long)]
    non_interactive: bool,

    /// Language of the logbook prose
    #[arg(long, value_parser = ["en", "de"])]
    language: Option<String>,

    /// Add Obsidian settings (.obsidian/)
    #[arg(long)]
    obsidian: bool,

    /// Agent harness to set up (repeatable)
    #[arg(long, value_name = "NAME", value_parser = ["claude-code"])]
    harness: Vec<String>,

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

    let ctx = Context::from_env(cli.json, cli.quiet, cli.no_commit, cli.logbook)?;
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
                git: match (c.git, c.no_git) {
                    (true, _) => Some(true),
                    (_, true) => Some(false),
                    _ => None,
                },
            },
        ),
        Command::Doctor { path } => commands::doctor::run(&ctx, path.as_deref()),
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
/// clap reads them, so that a value is never mistaken for the flag
/// (`--detail --json` with a hyphen-accepting option, or anything after
/// `--`, as in `seldon log -- --json`). Unlike a re-parse it keeps going past
/// invalid values and unknown subcommands, which stop clap.
fn wants_json(argv: &[OsString]) -> bool {
    let mut root = Cli::command();
    root.build();
    let mut cmd = &root;
    let mut tokens = argv.iter().skip(1).map(|t| t.to_string_lossy()).peekable();
    while let Some(token) = tokens.next() {
        if token == "--" {
            return false;
        }
        if token == "--json" {
            return true;
        }
        let option = if let Some(long) = token.strip_prefix("--") {
            (!long.contains('=')).then(|| cmd.get_arguments().find(|a| a.get_long() == Some(long)))
        } else if let Some(short) = token.strip_prefix('-').filter(|s| s.chars().count() == 1) {
            let c = short.chars().next();
            Some(cmd.get_arguments().find(|a| a.get_short() == c))
        } else {
            if let Some(sub) = cmd.find_subcommand(token.as_ref()) {
                cmd = sub;
            }
            None
        };
        if let Some(Some(arg)) = option {
            let takes_value = arg.get_action().takes_values();
            let next_is_value = tokens
                .peek()
                .is_some_and(|n| !n.starts_with('-') || arg.is_allow_hyphen_values_set());
            if takes_value && next_is_value {
                tokens.next();
            }
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
