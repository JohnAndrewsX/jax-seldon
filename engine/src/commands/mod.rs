//! Command implementations. Each returns an [`Output`] with a human and a
//! JSON rendering (SPEC-ENGINE §1.4); `main.rs` picks one.

pub mod capture;
pub mod doctor;
pub mod init;

use std::path::{Path, PathBuf};

use crate::config::{self, Config, Dirs, LogbookSource};
use crate::error::{Exit, Result};

/// Global flags and the environment, read once per process.
#[derive(Debug, Clone)]
pub struct Context {
    pub dirs: Dirs,
    pub json: bool,
    pub quiet: bool,
    pub no_commit: bool,
    /// `--logbook DIR`.
    pub logbook_flag: Option<PathBuf>,
    /// `$SELDON_LOGBOOK`.
    pub logbook_env: Option<String>,
}

impl Context {
    pub fn from_env(
        json: bool,
        quiet: bool,
        no_commit: bool,
        logbook: Option<PathBuf>,
    ) -> Result<Self> {
        Ok(Context {
            dirs: Dirs::from_env()?,
            json,
            quiet,
            no_commit,
            logbook_flag: logbook,
            logbook_env: std::env::var(config::LOGBOOK_ENV).ok(),
        })
    }

    pub fn load_config(&self) -> Result<Option<Config>> {
        Config::load(&self.dirs.config_file())
    }

    /// The logbook path; `explicit` (a command's own `--path`) wins over
    /// everything else.
    pub fn resolve_logbook(
        &self,
        explicit: Option<&Path>,
        config: Option<&Config>,
    ) -> (PathBuf, LogbookSource) {
        config::resolve_logbook(
            &self.dirs,
            explicit.or(self.logbook_flag.as_deref()),
            self.logbook_env.as_deref(),
            config,
        )
    }
}

/// What a command prints, and how it exits.
#[derive(Debug, Clone)]
pub struct Output {
    pub human: String,
    pub json: serde_json::Value,
    pub exit: Exit,
}

impl Output {
    pub fn ok(human: impl Into<String>, json: serde_json::Value) -> Self {
        Output {
            human: human.into(),
            json,
            exit: Exit::Ok,
        }
    }
}
