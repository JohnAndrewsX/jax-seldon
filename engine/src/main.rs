//! `seldon` — the Seldon engine (SPEC-ENGINE.md).
//!
//! Scaffold only (WP-001): `--version` and `contract-version`. Every command
//! accepts the global `--json` flag; exit codes follow AGENTS.md §7.

use std::process::ExitCode;

use clap::{Parser, Subcommand};
use serde_json::json;

/// Engine version, from Cargo.toml.
const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The contract version this engine writes (`index.json.contractVersion`).
/// Must equal `manifest.json.seldon.contractVersion` (docs/CONTRACT.md).
const CONTRACT_VERSION: u32 = 1;

/// Process exit codes (AGENTS.md §7, SPEC-ENGINE.md §3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)] // variants are used by later work packages
enum Exit {
    Ok = 0,
    UserError = 1,
    EngineError = 2,
    NotInitialised = 3,
    LockHeld = 4,
}

impl From<Exit> for ExitCode {
    fn from(code: Exit) -> Self {
        ExitCode::from(code as u8)
    }
}

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

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Print the engine/plugin contract version
    ContractVersion,
}

fn main() -> ExitCode {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(err) => return parse_error(err),
    };

    if cli.version {
        return emit(
            cli.json,
            &format!("seldon {VERSION}"),
            json!({ "name": "seldon", "version": VERSION }),
        );
    }

    match cli.command {
        Some(Command::ContractVersion) => emit(
            cli.json,
            &CONTRACT_VERSION.to_string(),
            json!({ "contractVersion": CONTRACT_VERSION }),
        ),
        None => usage_error(cli.json, "no command given; see `seldon --help`"),
    }
}

/// Prints human or JSON output and returns success.
fn emit(as_json: bool, human: &str, value: serde_json::Value) -> ExitCode {
    if as_json {
        println!("{value}");
    } else {
        println!("{human}");
    }
    Exit::Ok.into()
}

/// clap reports `--help` as an "error"; real parse errors are user errors
/// (exit 1), not clap's default exit 2, which is reserved for engine errors.
fn parse_error(err: clap::Error) -> ExitCode {
    use clap::error::ErrorKind;
    match err.kind() {
        ErrorKind::DisplayHelp | ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand => {
            let _ = err.print();
            Exit::Ok.into()
        }
        _ => {
            if std::env::args().any(|a| a == "--json") {
                let message = err.kind().to_string();
                usage_error(true, &message)
            } else {
                let _ = err.print();
                Exit::UserError.into()
            }
        }
    }
}

fn usage_error(as_json: bool, message: &str) -> ExitCode {
    if as_json {
        println!(
            "{}",
            json!({ "error": { "code": Exit::UserError as u8, "message": message } })
        );
    } else {
        eprintln!("seldon: {message}");
    }
    Exit::UserError.into()
}
