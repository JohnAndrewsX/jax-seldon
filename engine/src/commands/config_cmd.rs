//! `seldon config watch <PATH>` (SPEC-ENGINE §3, ADR-0046, WP-139): adds
//! one path to `config.toml watchPaths`, the desk's *Watch* click on a
//! recently edited file. Only the `watchPaths` array changes (the minimal
//! edit of WP-109, [`crate::config::with_added_watch_paths`]); every other
//! byte of the file stays. The next capture takes the files the path
//! brings in as they are, without an event (WP-069 scope change).

use clap::{Args, Subcommand};
use serde_json::json;

use super::{Context, Output};
use crate::collectors::config::SkipPaths;
use crate::collectors::recent;
use crate::config::Config;
use crate::error::{Error, Result};
use crate::model::event::SUBJECT_MAX;
use crate::sys;

#[derive(Debug, Clone, Args)]
pub struct ConfigArgs {
    #[command(subcommand)]
    pub command: ConfigCommand,
}

#[derive(Debug, Clone, Subcommand)]
pub enum ConfigCommand {
    /// Add a path under your home directory to watchPaths; the rest of config.toml stays as it is
    #[command(after_help = "Examples:
  seldon config watch ~/.config/alacritty/alacritty.toml
  seldon config watch --json -- ~/.config/starship.toml")]
    Watch {
        /// The file or folder (`~/…`; a relative path lies under the home directory)
        path: String,
    },
}

pub fn run(ctx: &Context, args: ConfigArgs) -> Result<Output> {
    match args.command {
        ConfigCommand::Watch { path } => watch(ctx, &path),
    }
}

fn watch(ctx: &Context, value: &str) -> Result<Output> {
    let dirs = &ctx.dirs;
    let shown_value = value.escape_debug();
    let Some(path) = dirs.expand_config(value) else {
        return Err(Error::user("config watch: the path is empty"));
    };
    let key = dirs.display(&path);
    if key
        .chars()
        .any(|c| c.is_control() || crate::redact::is_invisible(c))
        || key.chars().count() > SUBJECT_MAX
    {
        return Err(Error::user(format!(
            "config watch: `{shown_value}` cannot be a watch path (a control or format character, \
             or longer than {SUBJECT_MAX} characters)"
        )));
    }
    if !path.starts_with(&dirs.home) || path == dirs.home {
        return Err(Error::user(format!(
            "config watch: `{key}` is not below your home directory; Seldon watches only files there"
        )));
    }
    // a path that is not there is a typo or a name the list showed wrong
    // (B1); the walk lists only what exists
    if std::fs::symlink_metadata(&path).is_err() {
        return Err(Error::user(format!(
            "config watch: `{key}` does not exist; nothing to watch"
        )));
    }
    // the config as it is under the lock: a concurrent edit is seen (N2)
    let lock = ctx.lock()?;
    let config = ctx.load_config()?.unwrap_or_default();
    // Seldon's own files change with every capture: never watched, nor a
    // folder that holds them
    let (logbook, _) = ctx.resolve_logbook(None, Some(&config));
    let own = [
        logbook,
        dirs.state_dir.clone(),
        dirs.config_dir(),
        ctx.config_file.clone(),
    ];
    if let Some(o) = own
        .iter()
        .map(|o| crate::attribution::normalise(&std::path::absolute(o).unwrap_or(o.clone())))
        .find(|o| o.starts_with(&path) || path.starts_with(o))
    {
        return Err(Error::user(format!(
            "config watch: `{key}` holds or lies in Seldon's own files ({}); they are never watched",
            dirs.display(&o)
        )));
    }
    let skip = SkipPaths::new(&dirs.home, &config.redaction.skip_paths);
    if recent::skipped(&skip, &dirs.home, &path) {
        return Err(Error::user(format!(
            "config watch: `{key}` matches [redaction] skipPaths, so it is never opened or \
             hashed; remove the pattern from config.toml first"
        )));
    }
    let covered = config
        .watch_paths
        .iter()
        .find(|w| dirs.expand_config(w).is_some_and(|w| path.starts_with(w)))
        .cloned();
    let file = dirs.display(&ctx.config_file);
    if let Some(by) = covered {
        let human = format!("{key} is watched already (watchPaths: {by}); nothing changed");
        return Ok(Output::ok(
            human,
            json!({"added": false, "path": key, "coveredBy": by, "config": file}),
        ));
    }

    match std::fs::read_to_string(&ctx.config_file) {
        Ok(text) => {
            let edited = crate::config::with_added_watch_paths(&text, std::slice::from_ref(&key))
                .ok_or_else(|| not_extended(&text, &file, &key))?;
            sys::write_atomic(&ctx.config_file, edited.as_bytes())?;
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            let mut config = Config::default();
            config.watch_paths.push(key.clone());
            config.save(&ctx.config_file)?;
        }
        Err(e) => {
            return Err(anyhow::Error::new(e)
                .context(format!("cannot read {}", ctx.config_file.display()))
                .into());
        }
    }
    // the list's row goes at once, not at the next capture
    crate::index::rebuild_if_initialised(ctx);
    drop(lock);
    let human = format!(
        "{key} added to watchPaths in {file}; the next capture takes it as it is (no event)"
    );
    Ok(Output::ok(
        human,
        json!({"added": true, "path": key, "coveredBy": null, "config": file}),
    ))
}

/// Why `config.toml` was left as it is: its `watchPaths` cannot take one
/// more path by the minimal edit, or it has none, so the defaults apply
/// and one path alone would replace them.
fn not_extended(text: &str, file: &str, key: &str) -> Error {
    let has_key = text
        .parse::<toml::Table>()
        .is_ok_and(|t| t.contains_key("watchPaths"));
    Error::user(if has_key {
        format!(
            "config watch: {file} was left as it is: its watchPaths cannot be extended without \
             changing the rest of the file; add \"{key}\" to watchPaths by hand"
        )
    } else {
        format!(
            "config watch: {file} was left as it is: it has no top-level watchPaths, so the \
             defaults apply; write watchPaths with the defaults (`seldon init` writes them) and \
             \"{key}\" by hand"
        )
    })
}
