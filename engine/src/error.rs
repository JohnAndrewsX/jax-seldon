//! Engine errors and their exit codes (SPEC-ENGINE §3).

use std::path::PathBuf;

/// Process exit codes (AGENTS.md §7, SPEC-ENGINE §3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exit {
    Ok = 0,
    UserError = 1,
    EngineError = 2,
    NotInitialised = 3,
    LockHeld = 4,
}

impl From<Exit> for std::process::ExitCode {
    fn from(code: Exit) -> Self {
        std::process::ExitCode::from(code as u8)
    }
}

/// Every error a command can return. The variant decides the exit code.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Bad arguments, unknown ids, a target that already exists: exit 1.
    #[error("{0}")]
    User(String),

    /// No `.seldon/logbook.toml` at the resolved logbook path: exit 3.
    #[error("logbook not initialised at {}; run `seldon init`", .0.display())]
    NotInitialised(PathBuf),

    /// Another `seldon` process holds the state lock: exit 4.
    #[error("another seldon process holds the lock {}", .0.display())]
    LockHeld(PathBuf),

    /// Anything else (I/O, unreadable files, failed subprocesses): exit 2.
    #[error("{0:#}")]
    Engine(anyhow::Error),
}

/// A file of the logbook a reader refuses (WP-174): a ledger month that is
/// no regular file or is over its cap. It travels up as an
/// `anyhow::Error` like any read error and still ends as [`Error::User`]
/// (exit 1, as WP-171's refusals): the fix is in the user's files.
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct Refused(pub String);

impl From<anyhow::Error> for Error {
    fn from(err: anyhow::Error) -> Self {
        if err.chain().any(|c| c.is::<Refused>()) {
            Error::User(format!("{err:#}"))
        } else {
            Error::Engine(err)
        }
    }
}

impl Error {
    pub fn user(message: impl Into<String>) -> Self {
        Error::User(message.into())
    }

    pub fn exit(&self) -> Exit {
        match self {
            Error::User(_) => Exit::UserError,
            Error::NotInitialised(_) => Exit::NotInitialised,
            Error::LockHeld(_) => Exit::LockHeld,
            Error::Engine(_) => Exit::EngineError,
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Error::Engine(err.into())
    }
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
