//! `seldon doctor`: engine, config, logbook, omarchy, snapper and git checks
//! (SPEC-ENGINE §3). Read-only; never runs anything with privileges.
//!
//! Every check is `ok`, `degraded` (works with less, e.g. snapper without
//! permissions, ADR-0011) or `error`. Exit 0 without errors, 3 when the
//! logbook is not initialised, 1 for any other error (the fix is in the
//! user's files or config).

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Serialize;
use serde_json::json;

use super::{Context, Output};
use crate::collectors::{Sources, snapper};
use crate::config::Config;
use crate::error::{Error, Exit, Result};
use crate::logbook::{Logbook, git, layout};
use crate::model::{self, Area, Case, Decision, Journal, Memory, Record};
use crate::sys::{self, Run};
use crate::{CONTRACT_VERSION, VERSION};

/// The one command that lets the user read snapper without root (ADR-0011).
/// Printed, never run.
pub const SNAPPER_FIX: &str = "sudo snapper -c root set-config ALLOW_USERS=$USER SYNC_ACL=yes";

/// What [`SNAPPER_FIX`] grants besides listing: snapper(8) has no read-only
/// level for `ALLOW_USERS`. Shown with the fix (doctor, init, the plugin's
/// banner) and when listing works for the user.
pub const SNAPPER_FIX_GRANTS: &str = "The fix adds your user to ALLOW_USERS of the root snapper config, which also lets your user create, change and delete root snapshots without a password.";

/// Appended to the `ok` message when `snapper list` works for the user.
pub const SNAPPER_LIST_GRANTS: &str = "This user may use the snapper config, which also lets it create, change and delete snapshots without a password.";

const PROBE_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Ok,
    Degraded,
    Error,
}

impl Status {
    pub fn as_str(self) -> &'static str {
        match self {
            Status::Ok => "ok",
            Status::Degraded => "degraded",
            Status::Error => "error",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Check {
    pub name: &'static str,
    pub status: Status,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fix: Option<String>,
}

impl Check {
    fn new(name: &'static str, status: Status, message: impl Into<String>) -> Self {
        Check {
            name,
            status,
            message: message.into(),
            fix: None,
        }
    }

    fn fix(mut self, fix: impl Into<String>) -> Self {
        self.fix = Some(fix.into());
        self
    }
}

/// `seldon doctor [--path DIR]`.
pub fn run(ctx: &Context, path: Option<&Path>) -> Result<Output> {
    let mut checks = vec![Check::new(
        "engine",
        Status::Ok,
        format!("seldon {VERSION}, contract {CONTRACT_VERSION}"),
    )];

    let config_file = ctx.config_file.clone();
    let shown = ctx.dirs.display(&config_file);
    let config = match ctx.load_config() {
        Ok(Some(c)) => {
            checks.push(Check::new("config", Status::Ok, shown));
            Some(c)
        }
        Ok(None) => {
            checks.push(
                Check::new(
                    "config",
                    Status::Degraded,
                    format!("{shown} not found; defaults in use"),
                )
                .fix("seldon init"),
            );
            None
        }
        Err(Error::User(message)) => {
            checks.push(Check::new("config", Status::Error, message));
            None
        }
        Err(e) => return Err(e),
    };
    let effective = config.clone().unwrap_or_default();

    let (root, source) = ctx.resolve_logbook(path, config.as_ref());
    let (logbook_check, logbook) = check_logbook(&root, &ctx.dirs.display(&root), source);
    let not_initialised = logbook_check.status == Status::Error && !Logbook::is_initialised(&root);
    checks.push(logbook_check);
    checks.push(check_omarchy(&effective));
    checks.push(check_snapper(&effective));
    checks.push(check_git(&effective, logbook.as_ref()));

    let ok = checks.iter().all(|c| c.status != Status::Error);
    let exit = if not_initialised {
        Exit::NotInitialised
    } else if ok {
        Exit::Ok
    } else {
        Exit::UserError
    };

    let mut human = format!("seldon doctor · {}\n", ctx.dirs.display(&root));
    for c in &checks {
        let _ = writeln!(
            human,
            "  {:<9} {:<8} {}",
            c.status.as_str(),
            c.name,
            c.message
        );
        if let Some(fix) = &c.fix {
            let _ = writeln!(human, "  {:<9} {:<8} fix: {fix}", "", "");
        }
    }
    human.push_str(if ok {
        "doctor: ok"
    } else {
        "doctor: problems found"
    });

    Ok(Output {
        human,
        json: json!({ "ok": ok, "logbook": root, "checks": checks }),
        exit,
    })
}

/// Initialised, readable, complete layout, every record parses, every case
/// in the folder of its status. `shown` is `root` as the header prints it
/// (`~`-shortened).
fn check_logbook(
    root: &Path,
    shown: &str,
    source: crate::config::LogbookSource,
) -> (Check, Option<Logbook>) {
    let logbook = match Logbook::open(root) {
        Ok(l) => l,
        Err(Error::NotInitialised(_)) => {
            let check = Check::new(
                "logbook",
                Status::Error,
                format!("not initialised at {shown} (path from {source})"),
            )
            .fix(format!("seldon init --path {shown}"));
            return (check, None);
        }
        Err(e) => return (Check::new("logbook", Status::Error, e.to_string()), None),
    };

    let mut problems: Vec<String> = Vec::new();
    let mut misplaced: Vec<String> = Vec::new();
    let cases = check_files::<Case>(
        root,
        logbook.case_files(),
        &mut problems,
        |rel, file, case| {
            let folder = file.parent().and_then(|p| p.file_name());
            if folder.is_some_and(|f| f != case.status.folder()) {
                misplaced.push(format!("{rel} (status {})", case.status));
            }
        },
    );
    let decisions =
        check_files::<Decision>(root, logbook.decision_files(), &mut problems, |_, _, _| {});
    let journals =
        check_files::<Journal>(root, logbook.journal_files(), &mut problems, |_, _, _| {});
    check_files::<Area>(root, logbook.area_files(), &mut problems, |_, _, _| {});
    check_files::<Memory>(root, logbook.memory_files(), &mut problems, |_, _, _| {});

    let summary = format!(
        "{shown} · machine {} · {} · {} cases, {} decisions, {} journal days",
        logbook.meta.machine_id, logbook.meta.language, cases, decisions, journals
    );
    let check = if !problems.is_empty() {
        Check::new(
            "logbook",
            Status::Error,
            format!(
                "{} invalid file(s): {}",
                problems.len(),
                problems.join("; ")
            ),
        )
    } else {
        let missing = layout::missing(root);
        if !missing.is_empty() {
            Check::new(
                "logbook",
                Status::Degraded,
                format!("{summary}; missing: {}", missing.join(", ")),
            )
        } else if !misplaced.is_empty() {
            Check::new(
                "logbook",
                Status::Degraded,
                format!(
                    "{summary}; case not in the folder of its status: {}",
                    misplaced.join(", ")
                ),
            )
        } else {
            Check::new("logbook", Status::Ok, summary)
        }
    };
    (check, Some(logbook))
}

/// Parses every file of one record type; failures go to `problems`.
/// Returns how many files there are.
fn check_files<R: Record>(
    root: &Path,
    files: anyhow::Result<Vec<PathBuf>>,
    problems: &mut Vec<String>,
    mut each: impl FnMut(&str, &Path, &R),
) -> usize {
    let files = match files {
        Ok(f) => f,
        Err(e) => {
            problems.push(format!("{e:#}"));
            return 0;
        }
    };
    for file in &files {
        let rel = file
            .strip_prefix(root)
            .unwrap_or(file)
            .display()
            .to_string();
        match model::load::<R>(file) {
            Ok((record, _)) => each(&rel, file, &record),
            Err(e) => problems.push(format!("{rel}: {}", e.root_cause())),
        }
    }
    files.len()
}

/// `omarchy-version` (`omarchy --version` does not exist, memory/host.md).
fn check_omarchy(config: &Config) -> Check {
    if !config.collectors.omarchy {
        return Check::new("omarchy", Status::Ok, "collector disabled in config.toml");
    }
    match sys::run("omarchy-version", &[], None, PROBE_TIMEOUT) {
        Run::Exited {
            code: Some(0),
            stdout,
            ..
        } if !stdout.trim().is_empty() => {
            Check::new("omarchy", Status::Ok, format!("Omarchy {}", stdout.trim()))
        }
        Run::NotFound => Check::new(
            "omarchy",
            Status::Degraded,
            "omarchy-version not found; not an Omarchy system? The omarchy, plugins and theme collectors will report ok: false",
        ),
        other => Check::new(
            "omarchy",
            Status::Degraded,
            format!("omarchy-version failed: {}", describe(&other)),
        ),
    }
}

/// `snapper --jsonout list` as the user, in the C locale
/// ([`snapper::list_command`]). Without `ALLOW_USERS` it fails with
/// `No permissions.`; then the snapshot info files are read instead
/// ([`snapper::readable_info_files`]), and when they are not readable
/// either, that is degraded with the fix, never sudo (ADR-0011).
pub fn check_snapper(config: &Config) -> Check {
    if !config.collectors.snapper {
        return Check::new("snapper", Status::Ok, "collector disabled in config.toml");
    }
    let run = snapper::run_list("snapper", PROBE_TIMEOUT);
    match &run {
        Run::Exited {
            code: Some(0),
            stdout,
            ..
        } => match serde_json::from_str::<serde_json::Value>(stdout) {
            Ok(serde_json::Value::Object(configs)) => {
                let snapshots: usize = configs
                    .values()
                    .filter_map(|v| v.as_array())
                    .map(|list| {
                        list.iter()
                            .filter(|s| s["number"].as_u64() != Some(0))
                            .count()
                    })
                    .sum();
                let names: Vec<&str> = configs.keys().map(String::as_str).collect();
                Check::new(
                    "snapper",
                    Status::Ok,
                    format!(
                        "{snapshots} snapshots (config {}). {SNAPPER_LIST_GRANTS}",
                        names.join(", ")
                    ),
                )
            }
            _ => Check::new(
                "snapper",
                Status::Degraded,
                "snapper --jsonout list printed no JSON object",
            ),
        },
        Run::Exited { stderr, .. } if snapper::is_no_permissions(stderr) => {
            let dir = Sources::from_env().snapshots;
            match snapper::readable_info_files(&dir) {
                Some(info) => Check::new(
                    "snapper",
                    Status::Ok,
                    snapper::info_files_message(&dir, &info),
                ),
                None => Check::new(
                    "snapper",
                    Status::Degraded,
                    format!(
                        "No permissions. Snapshots are not recorded until you allow your user once (ADR-0011). {SNAPPER_FIX_GRANTS}"
                    ),
                )
                .fix(SNAPPER_FIX),
            }
        }
        Run::NotFound => Check::new(
            "snapper",
            Status::Degraded,
            "snapper not installed; no snapshot markers",
        ),
        other => Check::new(
            "snapper",
            Status::Degraded,
            format!("snapper failed: {}", describe(other)),
        ),
    }
}

fn check_git(config: &Config, logbook: Option<&Logbook>) -> Check {
    let Some(version) = git::version() else {
        let status = if config.git.autocommit {
            Status::Degraded
        } else {
            Status::Ok
        };
        return Check::new(
            "git",
            status,
            "git not installed; the logbook is not versioned",
        );
    };
    let Some(logbook) = logbook else {
        return Check::new("git", Status::Ok, version);
    };
    let autocommit = if config.git.autocommit { "on" } else { "off" };
    if git::is_repo(&logbook.root) {
        if let Err(e) = git::check_toplevel(&logbook.root) {
            let check = Check::new("git", Status::Degraded, format!("{version}; {e}"));
            return if e.contains("not a usable repository") {
                check.fix(format!("git -C {} init", logbook.root.display()))
            } else {
                check
            };
        }
        if config.git.autocommit
            && let Some(blocked) = autocommit_blocked(&logbook.root, &version)
        {
            return blocked;
        }
        Check::new(
            "git",
            Status::Ok,
            format!("{version}; logbook is a repository; autocommit {autocommit}"),
        )
    } else if config.git.autocommit {
        Check::new(
            "git",
            Status::Degraded,
            format!(
                "{version}; logbook is not a git repository, autocommit has nothing to commit to"
            ),
        )
        .fix(format!("git -C {} init", logbook.root.display()))
    } else {
        Check::new(
            "git",
            Status::Ok,
            format!("{version}; logbook is not a repository; autocommit off"),
        )
    }
}

/// What keeps every autocommit from committing (WP-061), first match: a
/// `.git/index.lock` (a git process killed half way leaves it behind), a
/// read-only `.git`, a detached HEAD (the autocommit skips it), a HEAD
/// that names no commit, an identity git cannot resolve. Each is degraded
/// with its fix. Nothing here takes a lock or writes into `.git`: only
/// file metadata and read-only git queries.
fn autocommit_blocked(root: &Path, version: &str) -> Option<Check> {
    let lock = root.join(".git/index.lock");
    if let Ok(meta) = std::fs::symlink_metadata(&lock) {
        let age = meta
            .modified()
            .ok()
            .and_then(|m| m.elapsed().ok())
            .map_or_else(String::new, |d| {
                format!(" (last changed {} ago)", human_age(d.as_secs()))
            });
        return Some(
            Check::new(
                "git",
                Status::Degraded,
                format!(
                    "{version}; .git/index.lock exists{age}: every autocommit fails while it is there; remove it when no git command is running in the logbook"
                ),
            )
            .fix(format!("rm {}", lock.display())),
        );
    }
    let dot_git = root.join(".git");
    if dot_git.is_dir() && std::fs::metadata(&dot_git).is_ok_and(|m| m.permissions().readonly()) {
        return Some(
            Check::new(
                "git",
                Status::Degraded,
                format!("{version}; .git is read-only: every autocommit fails"),
            )
            .fix(format!("chmod u+w {}", dot_git.display())),
        );
    }
    match git::is_detached(root) {
        Ok(true) => {
            let branches = git::branches(root);
            let branch = match branches.as_slice() {
                [one] => one.as_str(),
                _ => "<branch>",
            };
            return Some(
                Check::new(
                    "git",
                    Status::Degraded,
                    format!(
                        "{version}; {}: autocommit skips every commit until a branch is checked out",
                        git::DETACHED
                    ),
                )
                .fix(format!("git -C {} switch {branch}", root.display())),
            );
        }
        Ok(false) => {}
        Err(e) => {
            return Some(Check::new(
                "git",
                Status::Degraded,
                format!("{version}; {e}"),
            ));
        }
    }
    if let Err(e) = git::check_head(root) {
        return Some(
            Check::new(
                "git",
                Status::Degraded,
                format!("{version}; {e}: every autocommit fails"),
            )
            .fix(format!("git -C {} status", root.display())),
        );
    }
    match git::check_identity(root) {
        Ok(()) => None,
        Err(e) => Some(
            Check::new(
                "git",
                Status::Degraded,
                format!("{version}; git cannot name the committer, so every autocommit fails: {e}"),
            )
            .fix(format!(
                "git -C {} config user.name \"Your Name\"",
                root.display()
            )),
        ),
    }
}

/// `45 s`, `12 min`, `3 h`, `2 d`.
fn human_age(secs: u64) -> String {
    match secs {
        0..60 => format!("{secs} s"),
        60..3600 => format!("{} min", secs / 60),
        3600..86400 => format!("{} h", secs / 3600),
        _ => format!("{} d", secs / 86400),
    }
}

fn describe(run: &Run) -> String {
    match run {
        Run::Exited {
            code,
            stdout,
            stderr,
        } => {
            let text = if stderr.trim().is_empty() {
                stdout
            } else {
                stderr
            };
            let first = text.lines().next().unwrap_or("").trim();
            match code {
                Some(c) => format!("exit {c}: {first}"),
                None => format!("killed by a signal: {first}"),
            }
        }
        Run::NotFound => "not found".into(),
        Run::TimedOut => format!("no answer within {}s", PROBE_TIMEOUT.as_secs()),
        Run::Failed(e) => e.clone(),
    }
}
