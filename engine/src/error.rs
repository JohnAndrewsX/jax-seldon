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
    Engine(#[from] anyhow::Error),
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
