//! `seldon doctor`: engine, config, logbook, cases, ledger, fences,
//! collectors, state, skills, omarchy, snapper and git checks
//! (SPEC-ENGINE §3).
//! Read-only: no lock, no write; never runs anything with privileges.
//!
//! Every check is `ok`, `degraded` (works with less, e.g. snapper without
//! permissions, ADR-0026) or `error`. Exit 0 without errors, 3 when the
//! logbook is not initialised (and `config.toml` is readable), 1 for any
//! other error (the fix is in the user's files or config).

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Serialize;
use serde_json::{Value, json};

use super::capture::{Binding, PendingReset, pending_reset};
use super::index::duplicate_cases;
use super::{Context, Output};
use crate::collectors::config::{Manifest, OwnWrites};
use crate::collectors::{
    Cursors, Lost, STATE_RESET, ShownMessages, Sources, cursors_file, snapper,
};
use crate::config::{Config, LogbookSource};
use crate::error::{Error, Exit, Result};
use crate::index::load::{FENCE_BEGIN, FENCE_END, bad_lines_warning};
use crate::index::views::{self, DECISIONS_FENCE, STATUS_FENCE};
use crate::ledger::Ledger;
use crate::logbook::{Logbook, git, layout};
use crate::model::event::{Kind, Source};
use crate::model::{self, Area, Case, CaseStatus, Decision, Journal, Memory, Record};
use crate::redact::Redactor;
use crate::sys::{self, Run};
use crate::{CONTRACT_VERSION, VERSION};

/// The one command that lets the snapper collector read the snapshots
/// without root: a read grant on the snapshot directory, whose info files
/// it then reads (ADR-0026). Printed, never run.
pub const SNAPPER_FIX: &str = "sudo setfacl -m u:$USER:rx /.snapshots";

/// What [`SNAPPER_FIX`] grants. Shown with the fix (doctor, init, the
/// plugin's banner).
pub const SNAPPER_FIX_GRANTS: &str = "The fix grants your user read access to the snapshot directory listing and the snapshot info files (files inside a snapshot keep their own permissions), nothing else: no snapshot creation, change or deletion.";

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

/// A check `seldon doctor --only` runs alone (WP-101 round 3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Only {
    /// The rules block of the logbook's `AGENTS.md`
    Rules,
}

/// `seldon doctor --only rules`: the `engine` and `rules` rows, nothing
/// else. It starts no program (no omarchy, snapper or git probe), reads no
/// collector state and takes no lock: the panel asks it when it opens
/// (SPEC-PLUGIN §3). Exit 3 when the logbook is not initialised, 1 when the
/// row is an error. The JSON is doctor's.
pub fn run_only(ctx: &Context, path: Option<&Path>, only: Only) -> Result<Output> {
    let Only::Rules = only;
    // a config.toml that cannot be read leaves the default path, as the
    // other commands would refuse; the rules row then speaks for the path
    let config = ctx.load_config().ok().flatten();
    let (root, _) = ctx.resolve_logbook(path, config.as_ref());
    let logbook = Logbook::open(&root)?;
    let checks = vec![
        Check::new(
            "engine",
            Status::Ok,
            format!("seldon {VERSION}, contract {CONTRACT_VERSION}"),
        ),
        check_rules(ctx, &logbook),
    ];
    let ok = checks.iter().all(|c| c.status != Status::Error);
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
        exit: if ok { Exit::Ok } else { Exit::UserError },
    })
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
    // why the logbook path cannot be known, when config.toml is broken
    let mut config_invalid: Option<&str> = None;
    let config = match ctx.load_config() {
        Ok(Some(c)) => {
            checks.push(check_patterns(&c, shown));
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
            checks.push(Check::new("config", Status::Error, message).fix(format!(
                "correct {shown} (the message names the key), or move it away and run seldon init"
            )));
            config_invalid = Some("config.toml is invalid");
            None
        }
        // unreadable: a row and exit 1 like an invalid file, not a bare exit 2
        Err(e) => {
            checks.push(
                Check::new("config", Status::Error, one_line(&format!("{e}")))
                    .fix(format!("chmod u+r {}", config_file.display())),
            );
            config_invalid = Some("config.toml cannot be read");
            None
        }
    };
    let effective = config.clone().unwrap_or_default();
    // collector messages and probe output, redacted (SPEC-ENGINE §7); with
    // config.toml unusable its patterns are unknown: withheld
    let shown = ShownMessages::new(config_invalid.is_none().then_some(&effective));

    // F-540: without a readable config.toml the default path is a guess
    // (every other command stops at the config error); a path from
    // `--logbook`, `--path` or the environment is still checked
    let (root, source) = ctx.resolve_logbook(path, config.as_ref());
    let known = !(config_invalid.is_some() && source == LogbookSource::Default);
    let mut not_initialised = false;
    let logbook = if known {
        let (mut logbook_check, logbook, cases) =
            check_logbook(&root, &ctx.dirs.display(&root), source);
        not_initialised = logbook_check.status == Status::Error
            && !Logbook::is_initialised(&root)
            && config_invalid.is_none();
        // `seldon init` stops at the same config error
        if config_invalid.is_some()
            && let Some(fix) = &mut logbook_check.fix
        {
            *fix = format!("after fixing config.toml: {fix}");
        }
        checks.push(logbook_check);
        if let Some(logbook) = &logbook {
            checks.push(check_cases(&cases));
            checks.push(check_ledger(logbook));
            checks.push(check_fences(logbook));
            checks.push(check_rules(ctx, logbook));
            checks.push(check_rollbacks(logbook));
            checks.extend(check_planned(logbook, &cases));
            checks.push(check_collectors(ctx, &effective, logbook, &shown));
            checks.extend(check_reset(ctx, logbook));
            checks.extend(check_pending_reset(ctx, &effective, logbook, source));
        }
        logbook
    } else {
        let why = config_invalid.unwrap_or_default();
        checks.push(Check::new(
            "logbook",
            Status::Error,
            format!("not checked: {why}, so the logbook path is not known"),
        ));
        None
    };
    checks.extend(check_state(ctx));
    checks.push(check_skills(ctx));
    checks.push(check_omarchy(&effective, &shown));
    checks.push(check_snapper(&effective, &shown));
    checks.push(check_git(&effective, logbook.as_ref()));
    // ADR-0028 §4c, §4d: rows of config.toml, last (earlier rows keep
    // their places)
    if let Some(c) = &config {
        let text = std::fs::read_to_string(&ctx.config_file).ok();
        checks.push(check_watch_paths(c, text.as_deref()));
        checks.push(check_drift_rules(c, &omarchy_evidence(ctx)));
    }

    let ok = checks.iter().all(|c| c.status != Status::Error);
    let exit = if not_initialised {
        Exit::NotInitialised
    } else if ok {
        Exit::Ok
    } else {
        Exit::UserError
    };

    let mut human = if known {
        format!("seldon doctor · {}\n", ctx.dirs.display(&root))
    } else {
        format!(
            "seldon doctor · logbook not known ({})\n",
            config_invalid.unwrap_or_default()
        )
    };
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
        json: json!({
            "ok": ok,
            "logbook": known.then_some(root),
            "checks": checks,
            "drift": drift_json(&effective),
        }),
        exit,
    })
}

/// Initialised, readable, complete layout, every record parses, every case
/// in the folder of its status. `shown` is `root` as the header prints it
/// (`~`-shortened). Also returns `(id, relative path)` of every case file
/// that parses.
fn check_logbook(
    root: &Path,
    shown: &str,
    source: LogbookSource,
) -> (Check, Option<Logbook>, Vec<(String, String)>) {
    let logbook = match Logbook::open(root) {
        Ok(l) => l,
        Err(Error::NotInitialised(_)) => {
            let check = Check::new(
                "logbook",
                Status::Error,
                format!("not initialised at {shown} (path from {source})"),
            )
            .fix(format!("seldon init --path {shown}"));
            return (check, None, Vec::new());
        }
        Err(e) => {
            let check = Check::new("logbook", Status::Error, e.to_string());
            return (check, None, Vec::new());
        }
    };

    let mut problems: Vec<String> = Vec::new();
    let mut misplaced: Vec<String> = Vec::new();
    let mut ids: Vec<(String, String)> = Vec::new();
    let cases = check_files::<Case>(
        root,
        logbook.case_files(),
        &mut problems,
        |rel, file, case| {
            let folder = file.parent().and_then(|p| p.file_name());
            if folder.is_some_and(|f| f != case.status.folder()) {
                misplaced.push(format!("{rel} (status {})", case.status));
            }
            ids.push((case.id.clone(), rel.to_string()));
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
    (check, Some(logbook), ids)
}

/// `config.toml` parsed; its `[redaction] patterns` must compile too, or
/// every command that writes free text refuses (F-541).
fn check_patterns(config: &Config, shown: String) -> Check {
    match Redactor::for_config(config) {
        Ok(_) => Check::new("config", Status::Ok, shown),
        Err(e) => Check::new(
            "config",
            Status::Error,
            format!(
                "{shown}: {}; capture, log, event and the hooks refuse to write until it compiles",
                one_line(&e.to_string())
            ),
        )
        .fix(format!(
            "fix or remove that pattern under [redaction] patterns in {shown}"
        )),
    }
}

/// A case id in two files (WP-057): every command that finds a case by id
/// refuses it, and `index --check` exits 1. `cases` are `(id, relative
/// path)` of the case files that parse.
/// ADR-0028 §4d: the persistence paths are crises only where watched. A
/// list that is an earlier engine's default gains them at the next
/// capture; a list the user wrote keeps its own, and this row names the
/// missing paths with the line to add.
fn check_watch_paths(config: &Config, text: Option<&str>) -> Check {
    let missing = config.missing_default_watch_paths();
    let n = config.watch_paths.len();
    if missing.is_empty() {
        return Check::new(
            "watch",
            Status::Ok,
            format!("watchPaths: {n} path(s), every default included"),
        );
    }
    let list = missing.join(", ");
    let owned: Vec<String> = missing.iter().map(|p| p.to_string()).collect();
    let editable = text.is_some_and(|t| crate::config::with_added_watch_paths(t, &owned).is_some());
    if config.has_earlier_default_watch_paths() && editable {
        return Check::new(
            "watch",
            Status::Ok,
            format!("watchPaths is an earlier default list; the next capture adds {list}"),
        );
    }
    let quoted: Vec<String> = missing.iter().map(|p| format!("\"{p}\"")).collect();
    let whose = if config.has_earlier_default_watch_paths() {
        "an earlier default list, but config.toml cannot be extended without changing the rest of it,"
    } else {
        "your own list"
    };
    Check::new(
        "watch",
        Status::Degraded,
        format!(
            "watchPaths ({whose}) lacks default paths: {list}; a change there records nothing, and the persistence paths among them cannot raise a crisis (ADR-0028 §4d)"
        ),
    )
    .fix(format!(
        "add to watchPaths in config.toml: {}",
        quoted.join(", ")
    ))
}

/// Whether the capture takes Omarchy's shipped files as evidence
/// (`omarchy-default`, WP-109 round 1b), and if not, why.
fn omarchy_evidence(ctx: &Context) -> String {
    use crate::collectors::config::{omarchy_trust, trusted_owner};
    let path = crate::collectors::Sources::from_env().omarchy_path;
    let shown = ctx.dirs.display(&path);
    match omarchy_trust(&path, trusted_owner()) {
        Ok(()) => format!("Omarchy's copies count as evidence ({shown})"),
        Err(why) => format!(
            "Omarchy's copies do not count as evidence: {shown} {why} (it must be root's and neither group- nor world-writable)"
        ),
    }
}

/// ADR-0028 §4c, §6: the effective `[drift]` rule set, non-default keys
/// marked (the config can silence rules; the change is shown, not
/// refused), and whether `omarchy-default` evidence is taken
/// (`evidence`). An unknown routine rule id is degraded.
fn check_drift_rules(config: &Config, evidence: &str) -> Check {
    let d = &config.drift;
    let non_default = d.non_default();
    let message = format!(
        "attention {} · routine: {} · routinePaths {} · routinePackages {} · alwaysRedPaths {} · alwaysRed {}; {}",
        d.attention.as_str(),
        if d.routine.is_empty() {
            "none".to_string()
        } else {
            d.routine.join(", ")
        },
        d.routine_paths.len(),
        d.routine_packages.len(),
        d.always_red_paths.len(),
        d.always_red.len(),
        if non_default.is_empty() {
            "all defaults".to_string()
        } else {
            format!("non-default: {}", non_default.join(", "))
        }
    );
    let message = format!("{message}; {evidence}");
    let unknown = d.unknown_routine();
    if unknown.is_empty() {
        return Check::new("drift", Status::Ok, message);
    }
    Check::new(
        "drift",
        Status::Degraded,
        format!("{message}; unknown routine rule(s): {}", unknown.join(", ")),
    )
    .fix(format!(
        "use only these in [drift] routine: {}",
        crate::config::ROUTINE_RULES.join(", ")
    ))
}

/// The effective `[drift]` set for `doctor --json`.
fn drift_json(config: &Config) -> Value {
    let d = &config.drift;
    json!({
        "attention": d.attention.as_str(),
        "routine": d.routine,
        "routinePaths": d.routine_paths,
        "routinePackages": d.routine_packages,
        "alwaysRedPaths": d.always_red_paths,
        "alwaysRed": d.always_red,
        "nonDefault": d.non_default(),
    })
}

fn check_cases(cases: &[(String, String)]) -> Check {
    let twice = duplicate_cases(cases.iter().map(|(i, p)| (i.as_str(), p.as_str())));
    if twice.is_empty() {
        return Check::new("cases", Status::Ok, "every case id has one file");
    }
    Check::new("cases", Status::Error, twice.join("; "))
        .fix("delete the stale copy of each case and keep the file in the folder of its status")
}

/// Rule 9 held back (SPEC-ENGINE §5, ADR-0029, WP-115 round 2): changes
/// without a case whose time lies in the window of a case whose file does
/// not load (or lies outside the status folders) are never linked, since
/// the engine cannot read that case's Plan. A row only when there are
/// some; `cases` are the `(id, path)` of the case files that parse.
fn check_planned(logbook: &Logbook, cases: &[(String, String)]) -> Option<Check> {
    let ledger = Ledger::new(logbook, Redactor::builtin());
    let events = ledger.read_all().ok()?;
    let known: std::collections::HashSet<&str> = cases.iter().map(|(id, _)| id.as_str()).collect();
    let held = crate::reconcile::unreadable_windows(&events, &known);
    if held.is_empty() {
        return None;
    }
    let mut by_case: std::collections::BTreeMap<&str, usize> = Default::default();
    for (_, case) in &held {
        *by_case.entry(case.as_str()).or_default() += 1;
    }
    let names: Vec<String> = by_case
        .iter()
        .map(|(case, n)| format!("{n} in the window of {case}"))
        .collect();
    Some(
        Check::new(
            "planned",
            Status::Degraded,
            format!(
                "{} change(s) not linked to the case that planned them, because a case file \
                 does not load: {}",
                held.len(),
                names.join(", ")
            ),
        )
        .fix(
            "repair the case file (the `logbook` row names it) or put it back in work/; the \
             next `seldon capture` links what it planned",
        ),
    )
}

/// The rollback snapshots of the cases (ADR-0027 §3, WP-101): a case
/// whose `snapshotBefore` the snapper collector saw deleted (a
/// `snapshot-delete` of that number on or after the case's creation day)
/// has lost its rollback. Degraded for an open case, said for a completed
/// one; dropped cases do not count.
fn check_rollbacks(logbook: &Logbook) -> Check {
    let (files, _) = match crate::logbook::cases::all(logbook) {
        Ok(found) => found,
        Err(e) => return Check::new("rollbacks", Status::Error, one_line(&format!("{e}"))),
    };
    let with: Vec<_> = files
        .iter()
        .filter(|f| f.case.status != CaseStatus::Dropped)
        .filter_map(|f| f.case.snapshot_before.map(|n| (f, n)))
        .collect();
    if with.is_empty() {
        return Check::new("rollbacks", Status::Ok, "no case has a rollback snapshot");
    }
    let events = match Ledger::new(logbook, Redactor::builtin()).read_all() {
        Ok(events) => events,
        Err(e) => return Check::new("rollbacks", Status::Error, format!("{e:#}")),
    };
    let (mut open, mut closed) = (Vec::new(), Vec::new());
    for (f, n) in &with {
        let subject = n.to_string();
        let pruned = events.iter().any(|e| {
            e.source == Source::Snapper
                && e.kind == Kind::SnapshotDelete
                && e.subject == subject
                && e.ts.date_naive() >= f.case.created
        });
        if pruned {
            let line = format!("{}: snapshot {n} pruned", f.case.id);
            match f.case.status {
                CaseStatus::Completed => closed.push(line),
                _ => open.push(line),
            }
        }
    }
    let mut message = if open.is_empty() && closed.is_empty() {
        format!(
            "{} case(s) with a rollback snapshot, none pruned",
            with.len()
        )
    } else {
        open.iter()
            .chain(&closed)
            .cloned()
            .collect::<Vec<_>>()
            .join("; ")
    };
    if open.is_empty() {
        if !closed.is_empty() {
            message.push_str(" (completed: the case's work stands, its snapshot rollback is gone)");
        }
        return Check::new("rollbacks", Status::Ok, message);
    }
    Check::new("rollbacks", Status::Degraded, message).fix(
        "snapper's number cleanup removed it (5 numbered snapshots); before the case's next \
         red change take a new snapshot and write its number into the case's Log",
    )
}

/// Ledger lines that are not events (a torn write, a hand edit; F-132):
/// every reader skips them, so their events are missing from the index
/// and the views.
fn check_ledger(logbook: &Logbook) -> Check {
    let ledger = Ledger::new(logbook, Redactor::builtin());
    let (months, bad) = match ledger.months().and_then(|m| Ok((m, ledger.bad_lines()?))) {
        Ok(found) => found,
        Err(e) => return Check::new("ledger", Status::Error, format!("{e:#}")),
    };
    if bad.is_empty() {
        let noun = if months.len() == 1 { "month" } else { "months" };
        return Check::new(
            "ledger",
            Status::Ok,
            format!("{} {noun}, every line an event", months.len()),
        );
    }
    let lines: Vec<String> = bad
        .iter()
        .filter_map(|(month, lines)| bad_lines_warning(month, lines))
        .collect();
    Check::new(
        "ledger",
        Status::Degraded,
        format!(
            "{}; their events are missing from the index and the views",
            lines.join("; ")
        ),
    )
    .fix(
        "repair or delete those lines by hand (the ledger is plain JSON Lines, one event per line)",
    )
}

/// The generated fences of `STATUS.md` and `DECISIONS.md`. A damaged one
/// (no end marker of its own, or `STATUS.md` with the header but without
/// the fence) is left alone by `status` with a warning (WP-065); doctor
/// asks the same question the writer asks. An end marker that closes no
/// fence is a trap: if the fence's own end marker is (or was) removed,
/// the writer takes the text up to that stray one as the fence body and
/// replaces it. Doctor cannot tell a removed end marker followed by a
/// stray one from an intact fence, and says so. A file that cannot be read
/// as text stops `status` (exit 2): an error.
fn check_fences(logbook: &Logbook) -> Check {
    let mut unreadable: Vec<String> = Vec::new();
    let mut damaged: Vec<(String, String)> = Vec::new();
    let mut stray: Vec<String> = Vec::new();
    for (rel, name) in [
        ("STATUS.md", STATUS_FENCE),
        ("DECISIONS.md", DECISIONS_FENCE),
    ] {
        let text = match std::fs::read_to_string(logbook.path(rel)) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => {
                unreadable.push(format!("{rel}: cannot read ({e})"));
                continue;
            }
        };
        let broken = if name == STATUS_FENCE {
            views::merge_status(Some(&text), "").err()
        } else {
            views::fence_damaged(&text, name)
                .then(|| format!("the {name} fence has no end marker of its own"))
        };
        if let Some(why) = broken {
            damaged.push((
                format!("{rel}: {why}; `seldon status` leaves the file as it is"),
                format!(
                    "restore the marker lines `{FENCE_BEGIN}{name} -->` and `{FENCE_END}` in {rel}"
                ),
            ));
            continue;
        }
        let lines = stray_end_markers(&text);
        if !lines.is_empty() {
            let at: Vec<String> = lines.iter().map(usize::to_string).collect();
            stray.push(format!(
                "{rel}: the end marker on line {} closes no fence. If the {name} fence's own end marker was removed, `seldon status` takes the text up to the next end marker as the fence body and replaces it, your text included; doctor cannot tell a removed end marker from an intact fence",
                at.join(", ")
            ));
        }
    }
    if !unreadable.is_empty() {
        return Check::new(
            "fences",
            Status::Error,
            format!("{}; `seldon status` stops on it", unreadable.join("; ")),
        )
        .fix("make the file readable UTF-8 text again");
    }
    if !damaged.is_empty() {
        let (messages, fixes): (Vec<String>, Vec<String>) = damaged.into_iter().unzip();
        let messages = messages.into_iter().chain(stray).collect::<Vec<_>>();
        return Check::new("fences", Status::Degraded, messages.join("; ")).fix(fixes.join("; "));
    }
    if !stray.is_empty() {
        return Check::new("fences", Status::Degraded, stray.join("; ")).fix(format!(
            "delete the stray `{FENCE_END}` line(s), and check that each generated fence ends where its generated text ends"
        ));
    }
    Check::new(
        "fences",
        Status::Ok,
        "STATUS.md and DECISIONS.md: every generated fence has its end marker",
    )
}

/// The Seldon agent skill in the agent skill folders (WP-094).
fn check_skills(ctx: &Context) -> Check {
    let (status, message, fix) = super::skills::doctor_row(&ctx.dirs);
    let check = Check::new("skills", status, message);
    match fix {
        Some(f) => check.fix(f),
        None => check,
    }
}

/// The rules block of `AGENTS.md` against this engine's (ADR-0027,
/// WP-100): ok when current, else degraded with the fix the panel and the
/// user run (`seldon rules update`, or `--replace` for a damaged block).
fn check_rules(ctx: &Context, logbook: &Logbook) -> Check {
    use crate::logbook::rules::{self, State};
    let template = super::rules::template(logbook, ctx.now.date_naive());
    let state = match super::rules::read(logbook) {
        Ok(None) => rules::state(None, &template),
        Ok(Some(bytes)) => match String::from_utf8(bytes) {
            Ok(text) => rules::state(Some(&text), &template),
            Err(_) => State::NotUtf8,
        },
        Err(e) => {
            return Check::new("rules", Status::Degraded, one_line(&format!("{e}")))
                .fix("make AGENTS.md readable, then seldon rules update");
        }
    };
    let label = state.label();
    match state {
        State::Current => Check::new("rules", Status::Ok, label),
        State::Newer(_) => Check::new("rules", Status::Degraded, label)
            .fix("update seldon (the rules come with it)"),
        State::Damaged(_) => Check::new("rules", Status::Degraded, label).fix(
            "restore the rules block's marker lines in AGENTS.md, or seldon rules update --replace (archives the file)",
        ),
        State::NotUtf8 => Check::new("rules", Status::Degraded, label)
            .fix("seldon rules update --replace (archives the file)"),
        State::Changed => Check::new("rules", Status::Degraded, label)
            .fix("seldon rules update (archives your copy)"),
        State::Outdated(_) | State::Missing => {
            Check::new("rules", Status::Degraded, label).fix("seldon rules update")
        }
    }
}

/// 1-based line numbers of end markers that close no fence: one before
/// any begin marker, or a second one after a fence was closed.
fn stray_end_markers(text: &str) -> Vec<usize> {
    let mut markers: Vec<(usize, bool)> = text
        .match_indices(FENCE_BEGIN)
        .map(|(at, _)| (at, true))
        .chain(text.match_indices(FENCE_END).map(|(at, _)| (at, false)))
        .collect();
    markers.sort_unstable();
    let mut open = false;
    let mut out = Vec::new();
    for (at, begin) in markers {
        if begin {
            open = true;
        } else if open {
            open = false;
        } else {
            out.push(text[..at].matches('\n').count() + 1);
        }
    }
    out
}

/// The last capture's result per collector, from `cursors.json` (SPEC-ENGINE
/// §2 keeps each collector's `ok`, `message` and `fix` there): every
/// enabled collector whose last run failed is listed with its message,
/// degraded, with the fixes the collectors gave. Only cursors of this
/// logbook count (another logbook's are not this one's state). A file
/// that cannot be read is the `state` row's error.
fn check_collectors(
    ctx: &Context,
    config: &Config,
    logbook: &Logbook,
    shown: &ShownMessages,
) -> Check {
    let Ok(cursors) = Cursors::load(&cursors_file(&ctx.dirs)) else {
        return Check::new(
            "collectors",
            Status::Degraded,
            "not checked: cursors.json cannot be read (see the state row)",
        );
    };
    let canonical = std::fs::canonicalize(&logbook.root).unwrap_or_else(|_| logbook.root.clone());
    if cursors.logbook.as_deref() != Some(canonical.as_path()) {
        return Check::new("collectors", Status::Ok, "no capture for this logbook yet");
    }
    let mut failing = Vec::new();
    let mut fixes = Vec::new();
    for (name, state) in &cursors.collectors {
        if state.ok || !config.collectors.get(name).unwrap_or(true) {
            continue;
        }
        // an older engine saved it unredacted (WP-105)
        let message = state.message.as_deref().map(|m| shown.show(m));
        failing.push(format!(
            "{name}: {}",
            one_line(message.as_deref().unwrap_or("failed"))
        ));
        if let Some(fix) = &state.fix {
            fixes.push(fix.clone());
        }
    }
    if failing.is_empty() {
        return Check::new(
            "collectors",
            Status::Ok,
            "the last capture of every enabled collector succeeded",
        );
    }
    let check = Check::new(
        "collectors",
        Status::Degraded,
        format!("last capture failed: {}", failing.join("; ")),
    );
    if fixes.is_empty() {
        check
    } else {
        check.fix(fixes.join("; "))
    }
}

/// A state reset the last capture recorded (WP-081): the ledger's newest
/// `seldon` note `state-reset` is as new as the newest `lastRun` in
/// `cursors.json` for this logbook. Degraded, with the restore hint; no
/// row otherwise (the `state` rows check the files themselves).
fn check_reset(ctx: &Context, logbook: &Logbook) -> Option<Check> {
    let cursors = Cursors::load(&cursors_file(&ctx.dirs)).ok()?;
    let canonical = std::fs::canonicalize(&logbook.root).unwrap_or_else(|_| logbook.root.clone());
    if cursors.logbook.as_deref() != Some(canonical.as_path()) {
        return None;
    }
    let last = cursors
        .collectors
        .values()
        .filter_map(|s| chrono::DateTime::parse_from_rfc3339(s.last_run.as_deref()?).ok())
        .max()?;
    let ledger = Ledger::new(logbook, Redactor::builtin());
    let reset = ledger.months().ok()?.iter().rev().find_map(|m| {
        let events = ledger.read_month(m).ok()?.events;
        events
            .into_iter()
            .filter(|e| {
                e.source == Source::Seldon && e.kind == Kind::Note && e.subject == STATE_RESET
            })
            .max_by_key(|e| e.ts)
    })?;
    if reset.ts < last {
        return None;
    }
    let meta = |key: &str| -> Vec<String> {
        let value = reset.meta.extra.get(key).and_then(|v| v.as_str());
        let value = value.unwrap_or_default();
        value
            .split(',')
            .filter(|v| !v.is_empty())
            .map(String::from)
            .collect()
    };
    let state = ctx.dirs.display(&ctx.dirs.state_dir);
    let (rebound, files): (Vec<String>, Vec<String>) =
        meta("files").into_iter().partition(|f| f == "logbook");
    let mut why = Vec::new();
    if !rebound.is_empty() {
        why.push(format!("the state in {state} belonged to another logbook"));
    }
    if !files.is_empty() {
        why.push(format!(
            "{} missing or unreadable in {state}",
            files.join(", ")
        ));
    }
    let fix = if rebound.is_empty() {
        format!(
            "restore a backup of {state} and run seldon capture (user guide: Back up and restore the state directory); without a backup, run seldon capture to clear this row"
        )
    } else {
        "nothing to restore: the state belonged to another logbook path and the new baseline is this logbook's; run seldon capture to clear this row".to_string()
    };
    Some(
        Check::new(
            "state",
            Status::Degraded,
            format!(
                "the last capture recorded a state reset: {} took a new baseline ({}), so changes made in between may be missing",
                meta("sources").join(", "),
                why.join("; ")
            ),
        )
        .fix(fix),
    )
}

/// A state reset the next capture will record (WP-083): the cursors of
/// collectors whose source the ledger holds events of are missing,
/// unreadable or another logbook's. Predicted by the capture's own rule
/// ([`pending_reset`]), so it shows while a restored backup still prevents
/// the gap; after the capture, [`check_reset`] takes over. No row when
/// nothing is lost, `cursors.json` cannot be read (the `state` rows) or
/// the ledger cannot be read (the `ledger` row). A logbook from
/// `--path`/`--logbook` is not the one a plain `seldon capture` captures,
/// so the fix names it. A collector whose baseline waits since it
/// degraded or was not run when the state was lost (`pendingBaseline`,
/// WP-088/WP-091) has its own row ([`waiting_row`]). So has a loss whose
/// note a capture that stopped before saving its state already wrote
/// (`recorded`, WP-104): the next capture only warns of it.
fn check_pending_reset(
    ctx: &Context,
    config: &Config,
    logbook: &Logbook,
    source: LogbookSource,
) -> Vec<Check> {
    let Ok(cursors) = Cursors::load(&cursors_file(&ctx.dirs)) else {
        return Vec::new();
    };
    let canonical = std::fs::canonicalize(&logbook.root).unwrap_or_else(|_| logbook.root.clone());
    let ledger = Ledger::new(logbook, Redactor::builtin());
    let Ok(PendingReset {
        binding,
        lost,
        recorded,
    }) = pending_reset(&ledger, config, &cursors, &canonical)
    else {
        return Vec::new();
    };
    // a mark counts only in cursors bound here (`loss`)
    let mark = |name: &str| {
        let state = cursors
            .collectors
            .get(name)
            .filter(|_| binding == Binding::This);
        state.and_then(|s| s.pending_baseline)
    };
    let (waiting, lost): (Vec<_>, Vec<_>) = lost.into_iter().partition(|(n, _)| mark(n).is_some());
    let state = ctx.dirs.display(&ctx.dirs.state_dir);
    let capture = match source {
        LogbookSource::Flag => format!(
            "seldon --logbook {} capture",
            ctx.dirs.display(&logbook.root)
        ),
        _ => "seldon capture".to_string(),
    };
    let restore = format!(
        "restore {state} from a backup now (user guide: Back up and restore the state directory), or run {capture} to accept the new baseline"
    );
    let (why, fix) = match binding {
        Binding::None => (format!("cursors missing in {state}"), restore),
        Binding::This => (format!("cursors unreadable in {state}"), restore),
        Binding::Other => (
            format!("cursors in {state} bound to another logbook"),
            format!(
                "nothing to restore: the state belongs to another logbook path; run {capture} to accept the new baseline (user guide: Moving or copying the logbook)"
            ),
        ),
    };
    let names = |l: &[(&str, Lost)]| l.iter().map(|(n, _)| *n).collect::<Vec<_>>().join(", ");
    let mut checks = Vec::new();
    if !lost.is_empty() {
        checks.push(
            Check::new(
                "state",
                Status::Degraded,
                format!(
                    "the next capture will record a state reset for {}: {why}, so changes made since the last capture may not be recorded",
                    names(&lost)
                ),
            )
            .fix(fix.clone()),
        );
    }
    if !recorded.is_empty() {
        checks.push(
            Check::new(
                "state",
                Status::Degraded,
                format!(
                    "the next capture will warn of the state reset for {} that a capture recorded before it stopped without saving its state: {why}, so changes made since the last completed capture may not be recorded",
                    names(&recorded)
                ),
            )
            .fix(fix),
        );
    }
    if !waiting.is_empty() {
        checks.push(waiting_row(&waiting, &state, &capture));
    }
    checks
}

/// The row for collectors whose baseline waits (WP-091): each degraded or
/// was not run in the capture that lost its state, and what the mark says
/// was lost (`waiting`, from [`pending_reset`]): `cursors`, or `logbook`
/// when the state was another logbook's. A capture clears the row only by
/// running them successfully, so the fix names that capture.
fn waiting_row(waiting: &[(&'static str, Lost)], state: &str, capture: &str) -> Check {
    let why = |l: Lost| match l {
        Lost::Logbook => format!("the state in {state} belonged to another logbook"),
        _ => format!("cursors missing or unreadable in {state}"),
    };
    let names: Vec<&str> = waiting.iter().map(|(n, _)| *n).collect();
    let why = if waiting.iter().all(|(_, l)| *l == waiting[0].1) {
        why(waiting[0].1)
    } else {
        let each: Vec<String> = waiting
            .iter()
            .map(|(n, l)| format!("{n}: {}", why(*l)))
            .collect();
        each.join("; ")
    };
    let (whose, it) = if names.len() == 1 {
        ("its", "it")
    } else {
        ("each one's", "they")
    };
    Check::new(
        "state",
        Status::Degraded,
        format!(
            "{} degraded or not run since a state reset ({why}); {whose} next successful capture records the gap, so changes made in between may not be recorded",
            names.join(", ")
        ),
    )
    .fix(format!(
        "run {capture} --source {} once {it} can run (a degraded collector: the collectors row's fix first)",
        names.join(",")
    ))
}

/// The collector state files, loaded as strictly as `capture` loads
/// `cursors.json` (F-541); `manifest.json` and `owned.json` are read
/// leniently by the collector, so a corrupt one costs reports silently.
/// Missing files are fine (a new baseline). One row per broken file.
fn check_state(ctx: &Context) -> Vec<Check> {
    type Parse = fn(&[u8]) -> std::result::Result<(), serde_json::Error>;
    // (file, parser, what a corrupt one does, what an unreadable one does)
    let files: [(PathBuf, Parse, &str, &str); 3] = [
        (
            cursors_file(&ctx.dirs),
            |b| serde_json::from_slice::<Cursors>(b).map(drop),
            "every capture fails until it is moved away; the next capture then takes a new baseline for every collector, recorded as a state reset for the collectors the ledger holds events of",
            "every capture fails until it can be read again",
        ),
        (
            Manifest::file(&ctx.dirs),
            |b| serde_json::from_slice::<Manifest>(b).map(drop),
            "the next capture takes a new config baseline, recorded as a state reset if the ledger holds config events; the config changes since the last capture are not reported",
            "the config collector reports degraded until it can be read again",
        ),
        (
            OwnWrites::file(&ctx.dirs),
            |b| serde_json::from_slice::<OwnWrites>(b).map(drop),
            "the engine's own writes under the watched paths are reported as drift; the next capture moves it to owned.json.bad and records a state reset if the ledger holds config events",
            "the engine's own writes are neither recorded nor explained until it can be read again, so they are reported as drift",
        ),
    ];
    let mut present = Vec::new();
    let mut broken = Vec::new();
    for (path, parse, effect, unreadable) in files {
        let shown = ctx.dirs.display(&path);
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        match std::fs::read(&path) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => broken.push(
                Check::new(
                    "state",
                    Status::Error,
                    format!("{shown}: cannot read: {e}; {unreadable}"),
                )
                .fix(format!("chmod u+rw {}", path.display())),
            ),
            Ok(bytes) => match parse(&bytes) {
                Ok(()) => present.push(name),
                Err(e) => broken.push(
                    Check::new(
                        "state",
                        Status::Error,
                        format!("{shown} is corrupt ({e}); {effect}"),
                    )
                    .fix(format!("mv {0} {0}.bad", path.display())),
                ),
            },
        }
    }
    if !broken.is_empty() {
        return broken;
    }
    let dir = ctx.dirs.display(&ctx.dirs.state_dir);
    let message = if present.is_empty() {
        format!("{dir}: no collector state yet")
    } else {
        format!("{dir}: {} readable", present.join(", "))
    };
    vec![Check::new("state", Status::Ok, message)]
}

/// `text` on one line: a regex error spans several.
fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
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

/// `omarchy-version` (`omarchy --version` does not exist, memory/host.md),
/// with `OMARCHY_PATH` defaulted as the collector runs it
/// ([`sys::omarchy_command`]).
fn check_omarchy(config: &Config, shown: &ShownMessages) -> Check {
    if !config.collectors.omarchy {
        return Check::new("omarchy", Status::Ok, "collector disabled in config.toml");
    }
    // the program the collector runs (`SELDON_OMARCHY_VERSION`)
    let program = Sources::from_env().omarchy_version;
    match sys::run_command(sys::omarchy_command(&program, &[]), PROBE_TIMEOUT) {
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
            format!(
                "{program} not found; not an Omarchy system? The omarchy, plugins and theme collectors will report ok: false"
            ),
        ),
        other => Check::new(
            "omarchy",
            Status::Degraded,
            format!("{program} failed: {}", shown.show(&describe(&other))),
        ),
    }
}

/// `snapper --jsonout list` as the user, in the C locale
/// ([`snapper::list_command`]). For a user the snapper config does not list
/// it fails with `No permissions.`; then the snapshot info files are read
/// instead ([`snapper::readable_info_files`]), and when they are not
/// readable either, that is degraded with the read grant, never sudo
/// (ADR-0026). When listing works and the root config still lists this
/// user (the old opt-in of ADR-0011), the fix reverts that first. What
/// snapper printed on a failure is shown through `shown` (WP-105).
pub fn check_snapper(config: &Config, shown: &ShownMessages) -> Check {
    if !config.collectors.snapper {
        return Check::new("snapper", Status::Ok, "collector disabled in config.toml");
    }
    // the program the collector runs (`SELDON_SNAPPER`, WP-053 follow-up)
    let program = Sources::from_env().snapper;
    let run = snapper::run_list(&program, PROBE_TIMEOUT);
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
                let message = format!(
                    "{snapshots} snapshots (config {}). {SNAPPER_LIST_GRANTS}",
                    names.join(", ")
                );
                if snapper::lists_current_user(&program, PROBE_TIMEOUT) {
                    Check::new(
                        "snapper",
                        Status::Ok,
                        format!(
                            "{message} Your user is in ALLOW_USERS of the root snapper config, the opt-in that ADR-0026 replaces by a read grant: revert it (this empties ALLOW_USERS and turns SYNC_ACL off), then grant read access. {SNAPPER_FIX_GRANTS}"
                        ),
                    )
                    .fix(format!("{} && {SNAPPER_FIX}", snapper::REVERT_OPT_IN))
                } else {
                    Check::new("snapper", Status::Ok, message)
                }
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
                        "No permissions. Snapshots are not recorded until you grant your user read access to the snapshot directory once (ADR-0026). {SNAPPER_FIX_GRANTS}"
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
            format!("snapper failed: {}", shown.show(&describe(other))),
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

#[cfg(test)]
mod tests {
    use super::*;

    /// WP-091: one reason for all, or each collector with its own; the
    /// fix names every collector for `--source`.
    #[test]
    fn a_waiting_row_names_each_reason() {
        let state = "~/.local/state/seldon";
        let both = waiting_row(
            &[("snapper", Lost::Cursor), ("theme", Lost::Cursor)],
            state,
            "seldon capture",
        );
        assert_eq!(
            both.message,
            "snapper, theme degraded or not run since a state reset (cursors missing or unreadable in ~/.local/state/seldon); each one's next successful capture records the gap, so changes made in between may not be recorded"
        );
        assert_eq!(
            both.fix.as_deref(),
            Some(
                "run seldon capture --source snapper,theme once they can run (a degraded collector: the collectors row's fix first)"
            )
        );
        let mixed = waiting_row(
            &[("snapper", Lost::Logbook), ("theme", Lost::Cursor)],
            state,
            "seldon --logbook ~/lb capture",
        );
        assert!(
            mixed.message.starts_with(
                "snapper, theme degraded or not run since a state reset (snapper: the state in ~/.local/state/seldon belonged to another logbook; theme: cursors missing or unreadable in ~/.local/state/seldon); "
            ),
            "{}",
            mixed.message
        );
        assert!(
            mixed
                .fix
                .as_deref()
                .unwrap()
                .starts_with("run seldon --logbook ~/lb capture --source snapper,theme once"),
            "{:?}",
            mixed.fix
        );
    }
}
