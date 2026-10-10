//! `seldon init`: the wizard (SPEC-ENGINE §9, ADR-0010, ADR-0033).
//!
//! Four ways to ask ([`InitMode`], WP-119): `--defaults` asks nothing (the
//! panel's setup card runs it); plain `seldon init` asks only where the
//! logbook goes; `--ask` is the full wizard — path → language → Obsidian →
//! collectors → watched paths → harnesses → theme hook → git → backfill —
//! each step skipped when its flag is given; `--non-interactive` asks
//! nothing and takes flags, then the config, then the defaults, and records
//! from now on (the scripting form, as before ADR-0033). The first three
//! look back [`LOOKBACK_DAYS`] days and mark that history "before Seldon"
//! ([`setup::BASELINE_REASON`]) without a question (`--ask` offers the date
//! and asks about the baseline), and add Obsidian's settings when Obsidian
//! is installed ([`obsidian_installed`]). Defaults come from an existing
//! `config.toml` where it has a value (a file without `language` leaves the
//! language to the locale).
//!
//! Then `config.toml` is saved (with the choices, `[git] autocommit =
//! false` for no git) before anything is written into the logbook folder,
//! so a failed save leaves nothing `init` would refuse next time; the
//! logbook is written (its marker last), the harnesses are set up (inside
//! the first commit), the first capture runs (`capture --all`, with `--since`
//! as the backfill window), a backfill can be marked as the pre-Seldon
//! baseline, and the capture is committed; then `seldon dossier` fills
//! `system/*.md` once (WP-035, its own commit), and the theme hook is
//! installed on opt-in and recorded as the engine's own write under one
//! hold of the state lock, so the next capture explains its `config-add`
//! (SPEC-ENGINE §5 rule 7, WP-038, WP-074).
//! The steps after the layout report failures; they never undo the
//! logbook ([`super::setup`]).
//!
//! The result is a few aligned rows (Logbook, Config, Recording, Agents,
//! History, Snapshots), then "Next steps" only when something is left to
//! do, else [`NOTHING_TO_DO`]; the optional snapshot grant comes last
//! (WP-118). `--json` keeps every detail.

use std::io::IsTerminal as _;
use std::path::{Path, PathBuf};

use anyhow::Context as _;
use chrono::{DateTime, FixedOffset, Local};
use dialoguer::theme::ColorfulTheme;
use dialoguer::{Confirm, Input, MultiSelect, Select};
use serde_json::{Value, json};

use super::capture::{self, CaptureArgs};
use super::doctor::{self, Status};
use super::dossier::{self, DossierArgs};
use super::setup::{self, BASELINE_REASON, ThemeHook};
use super::{Commit, Context, Output, autocommit};
use crate::collectors::{ShownMessages, Sources};
use crate::config::{Collectors, Config, HARNESSES};
use crate::error::{Error, Result};
use crate::logbook::layout::{self, NewLogbook};
use crate::logbook::{Logbook, git, lock};
use crate::model::Language;
use crate::sys;

/// Project folders checked for ADR-0010 option 3, in order.
pub const PROJECT_FOLDERS: [&str; 5] = ["Work", "Projects", "dev", "src", "code"];

/// How many days a new logbook looks back on its first capture
/// (ADR-0033 §1).
pub const LOOKBACK_DAYS: u32 = 90;

/// How `init` asks (WP-119).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InitMode {
    /// Plain `seldon init`: the logbook's location only; needs a terminal.
    #[default]
    Location,
    /// `--defaults`: no question (the panel's setup card).
    Defaults,
    /// `--ask`: the full wizard; needs a terminal.
    Ask,
    /// `--non-interactive`: no question, flags and the config, records from
    /// now on; no backfill and no detection unless a flag asks for it.
    NonInteractive,
}

impl InitMode {
    /// The name `--json` reports.
    pub fn as_str(self) -> &'static str {
        match self {
            InitMode::Location => "location",
            InitMode::Defaults => "defaults",
            InitMode::Ask => "ask",
            InitMode::NonInteractive => "non-interactive",
        }
    }

    /// Whether this run asks a question: `--ask` always, plain `init`
    /// only while no `--path` or `--logbook` names the location.
    pub fn asks_with(self, location_given: bool) -> bool {
        match self {
            InitMode::Ask => true,
            InitMode::Location => !location_given,
            InitMode::Defaults | InitMode::NonInteractive => false,
        }
    }
}

/// `seldon init` flags.
#[derive(Debug, Clone, Default)]
pub struct InitArgs {
    pub path: Option<PathBuf>,
    pub mode: InitMode,
    pub language: Option<Language>,
    pub obsidian: bool,
    pub harnesses: Vec<String>,
    /// `--git` / `--no-git`; `None` = the config's `[git] autocommit`.
    pub git: Option<bool>,
    /// `--since`: the first capture's backfill window.
    pub since: Option<DateTime<FixedOffset>>,
    /// `--baseline`: dismiss the backfilled drift as the pre-Seldon baseline.
    pub baseline: bool,
    /// Run the first capture (`--no-capture` turns it off).
    pub capture: bool,
    /// `--theme-hook`: install Omarchy's theme-set hook.
    pub theme_hook: bool,
}

/// Everything the wizard decides.
#[derive(Debug, Clone)]
struct Choices {
    root: PathBuf,
    language: Language,
    obsidian: bool,
    collectors: Collectors,
    watch_paths: Vec<String>,
    harnesses: Vec<String>,
    git: bool,
    capture: bool,
    since: Option<DateTime<FixedOffset>>,
    /// `Some(days)`: `since` is the look-back of ADR-0033, not a flag's or
    /// the wizard's date (the History row says so).
    lookback: Option<u32>,
    /// `None`: ask after the first capture, when it opened drift.
    baseline: Option<bool>,
    theme_hook: bool,
}

/// One logbook location the wizard offers (ADR-0010).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathOption {
    pub label: String,
    pub path: PathBuf,
}

/// The fixed path options: `~/Seldon`, `<DOCUMENTS>/Seldon`, and
/// `<first existing project folder>/seldon` when there is one.
pub fn path_options(dirs: &crate::config::Dirs) -> Vec<PathOption> {
    let mut out = vec![PathOption {
        label: "default".into(),
        path: dirs.default_logbook(),
    }];
    out.push(PathOption {
        label: "documents".into(),
        path: dirs.documents().join("Seldon"),
    });
    if let Some(folder) = PROJECT_FOLDERS
        .iter()
        .map(|f| dirs.home.join(f))
        .find(|p| p.is_dir())
    {
        out.push(PathOption {
            label: "project folder".into(),
            path: folder.join("seldon"),
        });
    }
    out
}

pub fn run(ctx: &Context, args: InitArgs) -> Result<Output> {
    let config_file = ctx.config_file.clone();
    let existing = ctx.load_config()?;
    let location_given = args.path.is_some() || ctx.logbook_flag.is_some();
    if args.mode.asks_with(location_given)
        && !(std::io::stdin().is_terminal() && std::io::stderr().is_terminal())
    {
        return Err(Error::user(
            "not a terminal; use `seldon init --defaults` to ask nothing, or `seldon init --non-interactive` with --path, --language, …",
        ));
    }

    let mut choices = match args.mode {
        InitMode::Ask => wizard(ctx, &args, existing.as_ref())?,
        InitMode::Location => location(ctx, &args, existing.as_ref())?,
        InitMode::Defaults => no_questions(ctx, &args, existing.as_ref()),
        InitMode::NonInteractive => defaults(ctx, &args, existing.as_ref()),
    };
    let root = choices.root.clone();
    let lock = lock::acquire(&ctx.dirs.lock_file())?;
    if Logbook::is_initialised(&root) {
        let meta = root.join(crate::logbook::META_FILE);
        if !meta.is_file() {
            // a FIFO there: init would wait on it (WP-175)
            return Err(Error::user(format!(
                "{}: not a regular file; init does not write over it: remove it, or make it a regular file if this is a logbook",
                meta.display()
            )));
        }
        return Err(Error::user(format!(
            "{} is already a logbook",
            root.display()
        )));
    }
    if root.exists() && !root.is_dir() {
        return Err(Error::user(format!(
            "{} exists and is not a directory",
            root.display()
        )));
    }
    if !layout::is_vacant(&root).with_context(|| format!("cannot read {}", root.display()))? {
        return Err(Error::user(format!(
            "{} exists and is not empty; choose a new or empty directory",
            root.display()
        )));
    }

    let now = Local::now();
    let host = sys::slugify(&sys::hostname());
    let machine_id = format!(
        "{}-{}",
        if host.is_empty() {
            "machine"
        } else {
            host.as_str()
        },
        sys::random_hex(4)
    );
    let spec = NewLogbook {
        language: choices.language,
        machine_id: machine_id.clone(),
        created: now
            .format("%Y-%m-%dT%H:%M:%S%:z")
            .to_string()
            .parse()
            .context("cannot format the creation time")?,
        today: now.date_naive(),
        obsidian: choices.obsidian,
    };
    // the config first: if it cannot be saved, nothing is written into
    // the logbook folder and the same `init` runs again once that is fixed;
    // if the layout fails after it, both are put back ([`undo_layout`])
    let previous_config = match std::fs::read(&config_file) {
        Ok(bytes) => Some(bytes),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => {
            return Err(anyhow::Error::new(e)
                .context(format!("cannot read {}", config_file.display()))
                .into());
        }
    };
    let mut config = existing.unwrap_or_default();
    config.logbook = Some(root.clone());
    config.language = Some(choices.language);
    config.collectors = choices.collectors;
    config.watch_paths = choices.watch_paths.clone();
    config.harnesses = choices.harnesses.clone();
    // the git choice is kept: without a repository there is nothing to
    // commit to, and `doctor` reads `autocommit = false` as chosen; the
    // choice came from a flag, the wizard or this file ([`defaults`])
    config.git.autocommit = choices.git;
    // the highest folder of the logbook path that is not there yet
    let created = root
        .ancestors()
        .take_while(|p| !p.exists())
        .last()
        .map(Path::to_path_buf);
    config.save(&config_file)?;
    let files = match layout::create(&root, &spec) {
        Ok(files) => files,
        Err(e) => {
            let undo = undo_layout(
                &root,
                created.as_deref(),
                &config_file,
                previous_config.as_deref(),
            );
            return Err(anyhow::anyhow!("{e:#}; {undo}").into());
        }
    };

    // inside the first commit: the harness files are part of the logbook
    let harnesses = setup::harnesses(ctx, &lock, &config, &root, &choices.harnesses);
    let git = setup_git(&root, choices.git, !ctx.no_commit);
    let snapper = doctor::check_snapper(&config, &ShownMessages::new(Some(&config)));
    // the capture and the baseline take the lock themselves
    drop(lock);

    // every later step works on this logbook, whatever --logbook or
    // SELDON_LOGBOOK name
    let mut lb_ctx = ctx.clone();
    lb_ctx.logbook_flag = Some(root.clone());
    let capture = first_capture(&lb_ctx, &mut choices, args.mode == InitMode::Ask);
    // the dossier once, after the first capture (WP-035); `capture` and
    // `status` never refresh it
    let dossier = first_dossier(&lb_ctx, &capture);
    // under the lock from the script to the own-write record
    let (theme_hook, own_hook) = if choices.theme_hook {
        setup::theme_hook_step(&ctx.dirs, &config, &Sources::from_env().omarchy)
    } else {
        (ThemeHook::NotRequested, None)
    };

    // the result: one row per topic, then only what is left to do (WP-118)
    let mut summary = Summary::default();
    let mut logbook_row = format!("{} ({}", ctx.dirs.display(&root), choices.language.name());
    match &git {
        GitOutcome::Committed => logbook_row.push_str(", git repository"),
        GitOutcome::InitialisedOnly => {
            logbook_row.push_str(", git repository, not committed (--no-commit)")
        }
        GitOutcome::Skipped => logbook_row.push_str(", no git"),
        GitOutcome::Failed(e) => {
            logbook_row.push_str(", no git");
            summary.step(format!("seldon doctor   # git is not set up: {e}"));
        }
    }
    if choices.obsidian {
        logbook_row.push_str("; open it in Obsidian as a vault");
    }
    logbook_row.push(')');
    summary.row("Logbook", logbook_row);
    summary.row(
        "Config",
        format!(
            "{}; {}",
            ctx.dirs.display(&config_file),
            setup::SKIP_PATHS_HINT
        ),
    );

    let recording: Vec<&str> = Collectors::NAMES
        .iter()
        .filter(|n| choices.collectors.get(n) == Some(true))
        .map(|n| collector_label(n))
        .collect();
    let mut recording = if recording.is_empty() {
        "nothing (every collector is off)".to_string()
    } else {
        recording.join(", ")
    };
    if matches!(
        theme_hook,
        ThemeHook::Installed { .. } | ThemeHook::AlreadyInstalled { .. }
    ) {
        recording.push_str("; theme switches instantly (Omarchy hook)");
    }
    summary.row("Recording", recording);
    if let ThemeHook::Failed { error, .. } = &theme_hook {
        summary.more(format!("the theme hook is not installed: {error}"));
    }
    if let Some(Err(e)) = &own_hook {
        summary.more(format!("theme hook: {}", setup::own_writes_warning(e)));
    }
    if let Some(fix) = theme_hook.fix() {
        summary.step(format!("{fix}   # optional: the theme hook"));
    }

    if !harnesses.is_empty() {
        let done: Vec<String> = harnesses
            .iter()
            .filter(|h| h.done)
            .map(agent_result)
            .collect();
        summary.row(
            "Agents",
            if done.is_empty() {
                "nothing set up".to_string()
            } else {
                done.join(", ")
            },
        );
        for h in harnesses.iter().filter(|h| !h.done) {
            summary.more(format!("{}: {}", harness_label(&h.name), h.human));
            if matches!(h.name.as_str(), "claude-code" | "skills") {
                summary.step(format!("seldon hook install {}", h.name));
            }
        }
    }

    summary.row("History", capture.row.clone());
    for step in &capture.steps {
        summary.step(step.clone());
    }
    if let Some(step) = &dossier.step {
        summary.step(step.clone());
    }

    if !choices.collectors.snapper {
        summary.row("Snapshots", "off (the snapper collector is off)");
    } else {
        match (&snapper.status, &snapper.fix) {
            (Status::Degraded, Some(fix)) => {
                summary.row(
                    "Snapshots",
                    "not readable yet; optional, Seldon works without them",
                );
                summary.optional(SNAPPER_OPTIONAL, fix.clone());
            }
            (_, Some(fix)) => {
                summary.row(
                    "Snapshots",
                    "recorded, through snapper's ALLOW_USERS opt-in",
                );
                summary.optional(SNAPPER_RECOMMENDED, fix.clone());
            }
            (Status::Ok, None) => summary.row("Snapshots", "recorded"),
            (_, None) => summary.row("Snapshots", snapper.message.clone()),
        }
    }
    let human = summary.human();
    let next = summary.steps.clone();
    let optional: Vec<&String> = summary.optional.iter().map(|(_, fix)| fix).collect();

    let mut theme_hook_json = theme_hook.json();
    if let Some(r) = &own_hook {
        theme_hook_json["ownWrites"] = setup::own_writes_json(r);
    }
    Ok(Output::ok(
        human,
        json!({
            "mode": args.mode.as_str(),
            "logbook": root,
            "config": config_file,
            "machineId": machine_id,
            "language": choices.language,
            "files": files.len(),
            "obsidian": choices.obsidian,
            "collectors": choices.collectors,
            "watchPaths": choices.watch_paths,
            "skipPaths": config.redaction.skip_paths,
            "harnesses": choices.harnesses,
            "harnessSetup": harnesses
                .iter()
                .map(|h| (h.name.clone(), h.json.clone()))
                .collect::<serde_json::Map<_, _>>(),
            "git": git.json(),
            "snapper": snapper,
            "capture": capture.json,
            "dossier": dossier.json,
            "themeHook": theme_hook_json,
            "nextSteps": next,
            "optionalSteps": optional,
        }),
    ))
}

/// `seldon init --remove-theme-hook`: removes what `--theme-hook`
/// installed ([`setup::remove_theme_hook`]); the logbook is not needed.
/// The config's watch paths decide what is recorded as the engine's own
/// deletion, as for `hook install --settings`.
pub fn remove_theme_hook(ctx: &Context) -> Result<Output> {
    let config = ctx.load_config().ok().flatten().unwrap_or_default();
    let r = setup::remove_theme_hook(ctx, &config)?;
    let (hook, script) = (ctx.dirs.display(&r.hook), ctx.dirs.display(&r.script));
    let mut human = if r.hook_removed {
        format!("Removed the theme hook {hook}.")
    } else {
        format!("The theme hook is not installed ({hook}); nothing to remove.")
    };
    if r.script_removed {
        human.push_str(&format!("\nRemoved its script {script}."));
    }
    if let Some(Err(e)) = &r.own {
        human.push_str(&format!("\n{hook}: {}", setup::own_writes_warning(e)));
    }
    Ok(Output::ok(
        human,
        json!({
            "hook": r.hook,
            "removed": r.hook_removed,
            "script": r.script,
            "scriptRemoved": r.script_removed,
            "ownWrites": r.own.as_ref().map_or(Value::Null, setup::own_writes_json),
        }),
    ))
}

/// What the first capture step did.
struct CaptureStep {
    ran: bool,
    /// The summary's History row.
    row: String,
    /// What is left to do after it (`nextSteps`).
    steps: Vec<String>,
    json: Value,
}

/// Runs `capture --all` (with `--since` as the backfill window), then the
/// pre-Seldon baseline when it was chosen (or, in the wizard, confirmed
/// now that the drift count is known), then commits both. Failures are
/// reported, never fatal: the logbook exists.
fn first_capture(ctx: &Context, choices: &mut Choices, interactive: bool) -> CaptureStep {
    let capture_all = || vec!["seldon capture --all".to_string()];
    if !choices.capture {
        return CaptureStep {
            ran: false,
            row: "nothing recorded yet (--no-capture)".into(),
            steps: capture_all(),
            json: json!({ "ran": false, "reason": "--no-capture" }),
        };
    }
    let since = choices.since.map(|t| t.to_rfc3339());
    let out = match capture::run(
        ctx,
        CaptureArgs {
            sources: Vec::new(),
            all: true,
            since: since.clone(),
        },
    ) {
        Ok(out) => out,
        Err(e) => {
            return CaptureStep {
                ran: false,
                row: format!("nothing recorded: the first capture failed: {e}"),
                steps: capture_all(),
                json: json!({ "ran": false, "error": e.to_string() }),
            };
        }
    };
    let written = out.json["written"].as_u64().unwrap_or(0);
    let mut json = out.json;
    json["ran"] = json!(true);
    json["since"] = json!(since);
    json["lookbackDays"] = json!(choices.lookback);
    let mut steps = Vec::new();
    // ADR-0033 §4: one line for the look-back, which the baseline below
    // marks "before Seldon"
    let mut row = match (choices.lookback, choices.since) {
        (Some(days), _) => format!(
            "Looked back {days} days: {written} {} recorded as history before Seldon",
            if written == 1 { "change" } else { "changes" }
        ),
        (None, Some(t)) => format!("{written} event(s) since {}", shown_since(t)),
        (None, None) => format!("from now on; the first capture recorded {written} event(s)"),
    };
    // snapper has the Snapshots row of its own
    let degraded: Vec<&str> = json["collectors"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|c| c["ran"] == true && c["ok"] == false)
        .filter_map(|c| c["name"].as_str())
        .filter(|name| *name != "snapper")
        .collect();
    if !degraded.is_empty() {
        let names = degraded.join(", ");
        row.push_str(&format!("; {names} degraded"));
        steps.push(format!("seldon doctor   # degraded: {names}"));
    }

    let open = setup::open_drift(ctx);
    if choices.since.is_some()
        && choices.baseline.is_none()
        && let Some((items, crisis)) = open.filter(|(items, _)| *items > 0)
    {
        choices.baseline = Some(interactive && ask_baseline(items, crisis));
    }
    let mut summary = "first capture".to_string();
    let mut open_after = open;
    json["baseline"] = Value::Null;
    if choices.baseline == Some(true) {
        match setup::baseline(ctx) {
            Ok(b) => {
                // the look-back's line already says it
                if b.items > 0 && choices.lookback.is_none() {
                    row.push_str(&format!(
                        "; {} drift item(s) dismissed as \"{BASELINE_REASON}\"",
                        b.items
                    ));
                }
                json["baseline"] = json!({
                    "reason": BASELINE_REASON, "items": b.items, "events": b.events,
                });
                summary.push_str(" and pre-Seldon baseline");
                open_after = setup::open_drift(ctx);
            }
            Err(e) => {
                row.push_str(&format!("; the baseline failed: {e}"));
                json["baseline"] = json!({ "reason": BASELINE_REASON, "error": e.to_string() });
            }
        }
    }
    if let Some((items, crisis)) = open_after {
        if items > 0 {
            row.push_str(&format!("; {items} open drift item(s)"));
            if crisis > 0 {
                row.push_str(&format!(", {crisis} crisis"));
            }
            steps.push(format!(
                "seldon drift   # {items} open drift item(s) to link, explain or dismiss"
            ));
        }
        json["openDrift"] = json!(items);
        json["crisis"] = json!(crisis);
    }

    // commit, then rebuild under the same lock, so `logbook.git` in the
    // index names the new head and a clean tree (like `log` and `drift`)
    let commit = match ctx.lock().and_then(|lock| Ok((lock, ctx.open_logbook()?))) {
        Ok((lock, (config, logbook))) => {
            let commit = autocommit(ctx, &config, &logbook, &summary);
            crate::index::rebuild_if_initialised(ctx);
            drop(lock);
            commit
        }
        Err(e) => Commit::Failed(e.to_string()),
    };
    if let Commit::Failed(e) = &commit {
        row.push_str(&format!("; not committed: {e}"));
    }
    json["git"] = commit.json();
    CaptureStep {
        ran: true,
        row,
        steps,
        json,
    }
}

/// A backfill start for the summary: the date when it is local midnight
/// (what `--since YYYY-MM-DD` gives), else the RFC 3339 time.
fn shown_since(t: DateTime<FixedOffset>) -> String {
    if t.time() == chrono::NaiveTime::MIN {
        t.format("%Y-%m-%d").to_string()
    } else {
        t.to_rfc3339()
    }
}

/// What the first `seldon dossier` did.
struct DossierStep {
    /// What is left to do (`nextSteps`); `None` after a clean run.
    step: Option<String>,
    json: Value,
}

/// Runs `seldon dossier` (all sections) once the first capture ran; it
/// takes the lock, commits `seldon: dossier` and rebuilds the index
/// itself. A failure is reported, never fatal: the logbook exists.
fn first_dossier(ctx: &Context, capture: &CaptureStep) -> DossierStep {
    if !capture.ran {
        return DossierStep {
            step: Some("seldon dossier".into()),
            json: json!({ "ran": false, "reason": "no first capture" }),
        };
    }
    match dossier::run(ctx, DossierArgs::default()) {
        Ok(out) => {
            let warnings = out.json["warnings"].as_array().map_or(0, Vec::len);
            let mut json = out.json;
            json["ran"] = json!(true);
            DossierStep {
                step: (warnings > 0)
                    .then(|| format!("seldon dossier   # the first run had {warnings} warning(s)")),
                json,
            }
        }
        Err(e) => DossierStep {
            step: Some(format!("seldon dossier   # the first run failed: {e}")),
            json: json!({ "ran": false, "error": e.to_string() }),
        },
    }
}

/// The wizard's baseline question, asked once the count is known.
fn ask_baseline(items: usize, crisis: usize) -> bool {
    eprintln!("{}", baseline_note(items, crisis));
    Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt(prompts::BASELINE)
        .default(true)
        .interact()
        .unwrap_or(false)
}

/// The line above the baseline question.
fn baseline_note(items: usize, crisis: usize) -> String {
    let crisis = if crisis > 0 {
        format!(" ({crisis} crisis)")
    } else {
        String::new()
    };
    format!(
        "The backfill opened {items} drift item(s){crisis}:\n\
         changes from before Seldon. The baseline dismisses them with the\n\
         reason \"{BASELINE_REASON}\"."
    )
}

/// Flags, else the existing config, else built-in defaults.
fn defaults(ctx: &Context, args: &InitArgs, existing: Option<&Config>) -> Choices {
    let base = existing.cloned().unwrap_or_default();
    let (root, _) = ctx.resolve_logbook(args.path.as_deref(), existing);
    Choices {
        root,
        language: args
            .language
            .or(existing.and_then(|c| c.language))
            .unwrap_or_else(locale_language),
        obsidian: args.obsidian,
        collectors: base.collectors,
        watch_paths: base.watch_paths,
        harnesses: if args.harnesses.is_empty() {
            base.harnesses
        } else {
            args.harnesses.clone()
        },
        // `--git`/`--no-git`, else the config's autocommit (default on):
        // a re-run keeps a hand-set `autocommit = false`
        git: args.git.unwrap_or(base.git.autocommit),
        capture: args.capture,
        since: args.since,
        lookback: None,
        baseline: args.since.is_some().then_some(args.baseline),
        theme_hook: args.theme_hook,
    }
}

/// `--defaults` (and plain `init` after its one question): [`defaults`],
/// plus Obsidian's settings when Obsidian is installed, and, unless a flag
/// says otherwise, the look-back of ADR-0033: the first capture records
/// the last [`LOOKBACK_DAYS`] days and the baseline marks what that opens
/// "before Seldon", without a question. `--since` sets another start, its
/// backfill marked the same way; `--no-capture` records nothing.
fn no_questions(ctx: &Context, args: &InitArgs, existing: Option<&Config>) -> Choices {
    let mut c = defaults(ctx, args, existing);
    c.obsidian = c.obsidian || obsidian_installed(&ctx.dirs);
    if c.capture && c.since.is_none() {
        c.since = Some(lookback_start(ctx.now, LOOKBACK_DAYS));
        c.lookback = Some(LOOKBACK_DAYS);
    }
    if c.since.is_some() {
        c.baseline = Some(true);
    }
    c
}

/// Plain `seldon init`: [`no_questions`], and the one question where the
/// logbook goes (unless `--path` or `--logbook` names it).
fn location(ctx: &Context, args: &InitArgs, existing: Option<&Config>) -> Result<Choices> {
    let mut c = no_questions(ctx, args, existing);
    if args.path.is_none() && ctx.logbook_flag.is_none() {
        c.root = ask_path(ctx, &c.root)?;
    }
    Ok(c)
}

/// Local midnight `days` days before `now`'s day, in `now`'s offset: where
/// the look-back starts.
pub fn lookback_start(now: DateTime<FixedOffset>, days: u32) -> DateTime<FixedOffset> {
    let day = now.date_naive() - chrono::Days::new(u64::from(days));
    day.and_time(chrono::NaiveTime::MIN)
        .and_local_timezone(*now.offset())
        .single()
        .unwrap_or(now)
}

/// Desktop entries that say Obsidian is installed: the package's and the
/// Flatpak's.
pub const OBSIDIAN_DESKTOP_FILES: [&str; 2] = ["obsidian.desktop", "md.obsidian.Obsidian.desktop"];

/// Whether Obsidian is installed: one of [`OBSIDIAN_DESKTOP_FILES`] is in
/// an application folder of the XDG data dirs (`$XDG_DATA_HOME`, default
/// `~/.local/share`, then `$XDG_DATA_DIRS`, default `/usr/local/share` and
/// `/usr/share`; a Flatpak's exports are on that list where Flatpak is set
/// up). Only whether the file is there; nothing is read.
pub fn obsidian_installed(dirs: &crate::config::Dirs) -> bool {
    let var = |name: &str| std::env::var_os(name).filter(|v| !v.is_empty());
    let data_home = var("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| dirs.home.join(".local/share"));
    let data_dirs = var("XDG_DATA_DIRS").unwrap_or_else(|| "/usr/local/share:/usr/share".into());
    std::iter::once(data_home)
        .chain(std::env::split_paths(&data_dirs).filter(|p| p.is_absolute()))
        .any(|dir| {
            OBSIDIAN_DESKTOP_FILES
                .iter()
                .any(|f| dir.join("applications").join(f).is_file())
        })
}

/// The wizard's "more paths" answer as `watchPaths` entries: comma
/// separated, each resolved like a config value (relative to the home
/// folder, whatever the directory `init` runs in) and stored as `~/…`.
fn typed_watch_paths(dirs: &crate::config::Dirs, text: &str) -> Vec<String> {
    text.split(',')
        .filter_map(|s| dirs.expand_config(s.trim()))
        .map(|p| dirs.display(&p))
        .collect()
}

fn locale_language() -> Language {
    let locale = ["LC_ALL", "LC_MESSAGES", "LANG"]
        .iter()
        .find_map(|v| std::env::var(v).ok().filter(|s| !s.is_empty()));
    Language::from_locale(locale.as_deref())
}

fn prompt_err(e: dialoguer::Error) -> Error {
    Error::Engine(anyhow::Error::new(e).context("wizard input failed"))
}

/// The location question (ADR-0010's options; `current` first when it is
/// none of them).
fn ask_path(ctx: &Context, current: &Path) -> Result<PathBuf> {
    let theme = ColorfulTheme::default();
    let mut options = path_options(&ctx.dirs);
    if !options.iter().any(|o| o.path == current) {
        options.insert(
            0,
            PathOption {
                label: "current setting".into(),
                path: current.to_path_buf(),
            },
        );
    }
    let mut items: Vec<String> = options
        .iter()
        .map(|o| format!("{}  ({})", ctx.dirs.display(&o.path), o.label))
        .collect();
    items.push("custom path…".into());
    let pick = Select::with_theme(&theme)
        .with_prompt(prompts::PATH)
        .items(&items)
        .default(0)
        .interact()
        .map_err(prompt_err)?;
    Ok(match options.get(pick) {
        Some(o) => o.path.clone(),
        None => {
            let typed: String = Input::with_theme(&theme)
                .with_prompt(prompts::CUSTOM_PATH)
                .interact_text()
                .map_err(prompt_err)?;
            ctx.dirs.expand(typed.trim())
        }
    })
}

fn wizard(ctx: &Context, args: &InitArgs, existing: Option<&Config>) -> Result<Choices> {
    let mut c = defaults(ctx, args, existing);
    let theme = ColorfulTheme::default();

    if args.path.is_none() && ctx.logbook_flag.is_none() {
        c.root = ask_path(ctx, &c.root)?;
    }

    if args.language.is_none() {
        let items: Vec<String> = Language::ALL
            .iter()
            .map(|l| format!("{} ({l})", l.name()))
            .collect();
        let default = Language::ALL
            .iter()
            .position(|l| *l == c.language)
            .unwrap_or(0);
        let pick = Select::with_theme(&theme)
            .with_prompt(prompts::LANGUAGE)
            .items(&items)
            .default(default)
            .interact()
            .map_err(prompt_err)?;
        c.language = Language::ALL[pick];
    }

    if !args.obsidian {
        c.obsidian = Confirm::with_theme(&theme)
            .with_prompt(prompts::OBSIDIAN)
            .default(obsidian_installed(&ctx.dirs))
            .interact()
            .map_err(prompt_err)?;
    }

    let checked: Vec<bool> = Collectors::NAMES
        .iter()
        .map(|n| c.collectors.get(n).unwrap_or(true))
        .collect();
    let picked = MultiSelect::with_theme(&theme)
        .with_prompt(prompts::COLLECTORS)
        .items(Collectors::NAMES)
        .defaults(&checked)
        .interact()
        .map_err(prompt_err)?;
    let names: Vec<&str> = picked.iter().map(|i| Collectors::NAMES[*i]).collect();
    c.collectors = Collectors::only(&names);

    if c.collectors.config {
        let keep = MultiSelect::with_theme(&theme)
            .with_prompt(prompts::WATCH_PATHS)
            .items(&c.watch_paths)
            .defaults(&vec![true; c.watch_paths.len()])
            .interact()
            .map_err(prompt_err)?;
        let mut paths: Vec<String> = keep.iter().map(|i| c.watch_paths[*i].clone()).collect();
        let extra: String = Input::with_theme(&theme)
            .with_prompt(prompts::MORE_PATHS)
            .allow_empty(true)
            .interact_text()
            .map_err(prompt_err)?;
        paths.extend(typed_watch_paths(&ctx.dirs, &extra));
        c.watch_paths = paths;
    }

    if args.harnesses.is_empty() {
        // the kit is a private template, offered only where it is installed
        let items = harness_items(setup::kit_dir(&ctx.dirs).is_dir());
        let checked: Vec<bool> = items
            .iter()
            .map(|(name, _)| c.harnesses.iter().any(|x| x == name))
            .collect();
        let labels: Vec<&str> = items.iter().map(|(_, label)| *label).collect();
        let picked = MultiSelect::with_theme(&theme)
            .with_prompt(prompts::HARNESSES)
            .items(&labels)
            .defaults(&checked)
            .interact()
            .map_err(prompt_err)?;
        c.harnesses = picked.iter().map(|i| items[*i].0.to_string()).collect();
    }

    if !args.theme_hook {
        // the explanation above a short question: a prompt that wraps is
        // drawn twice by dialoguer
        eprintln!("{THEME_HOOK_NOTE}");
        c.theme_hook = Confirm::with_theme(&theme)
            .with_prompt(prompts::THEME_HOOK)
            .default(false)
            .interact()
            .map_err(prompt_err)?;
    }

    if args.git.is_none() {
        c.git = Confirm::with_theme(&theme)
            .with_prompt(prompts::GIT)
            .default(c.git)
            .interact()
            .map_err(prompt_err)?;
    }

    if c.capture && args.since.is_none() {
        eprintln!("{BACKFILL_NOTE}");
        // Enter takes the look-back of ADR-0033
        let start = lookback_start(ctx.now, LOOKBACK_DAYS);
        let typed: String = Input::with_theme(&theme)
            .with_prompt(prompts::BACKFILL)
            .default(start.format("%Y-%m-%d").to_string())
            .validate_with(|s: &String| -> std::result::Result<(), String> {
                if no_backfill(s) {
                    return Ok(());
                }
                setup::parse_since(s).map(|_| ()).map_err(|e| e.to_string())
            })
            .interact_text()
            .map_err(prompt_err)?;
        c.since = if no_backfill(&typed) {
            None
        } else {
            Some(setup::parse_since(&typed)?)
        };
    }
    // asked after the capture, with the number of items it opened
    c.baseline = match (c.since, args.baseline) {
        (None, _) => None,
        (Some(_), true) => Some(true),
        (Some(_), false) => None,
    };
    Ok(c)
}

/// The wizard's answer for "record from now on only".
fn no_backfill(typed: &str) -> bool {
    let t = typed.trim();
    t.is_empty() || t.eq_ignore_ascii_case("none")
}

/// The wizard's questions (WP-118). Each is drawn on one line of a
/// 70-column terminal, dialoguer's marks included ([`rendered_width`]):
/// a prompt or list item that wraps is drawn twice, because dialoguer
/// clears one line per logical line. Omarchy's presentation terminal is
/// wider (about 120 columns); 70 leaves room for a narrow window.
/// Explanations go above a question as plain lines ([`THEME_HOOK_NOTE`],
/// [`BACKFILL_NOTE`], [`baseline_note`]).
pub mod prompts {
    pub const PATH: &str = "Where should the logbook live?";
    pub const CUSTOM_PATH: &str = "Logbook path";
    pub const LANGUAGE: &str = "Language of the logbook prose";
    pub const OBSIDIAN: &str = "Add Obsidian settings (.obsidian/)?";
    pub const COLLECTORS: &str = "Collectors (space toggles, enter confirms)";
    pub const WATCH_PATHS: &str = "Watched config paths";
    pub const MORE_PATHS: &str = "More paths, comma-separated (empty for none)";
    pub const HARNESSES: &str = "Agent setup (space toggles, enter confirms)";
    pub const THEME_HOOK: &str = "Record theme switches instantly?";
    pub const GIT: &str = "Keep the logbook in git, with a first commit?";
    pub const BACKFILL: &str = "Backfill since (YYYY-MM-DD, or none)";
    pub const BASELINE: &str = "Dismiss them as \"before Seldon\"?";
}

/// The terminal width every wizard line fits in ([`prompts`]).
pub const WIZARD_COLUMNS: usize = 70;

/// How dialoguer's `ColorfulTheme` draws a question.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptKind {
    /// `? <prompt> (y/n) › yes`
    Confirm,
    /// `? <prompt> › <answer>`, with an answer of `answer` columns
    Input { answer: usize },
    /// `? <prompt> ›` above `❯ <item>` / `⬚ <item>` lines
    List,
}

/// Columns the line of a question takes on screen, dialoguer's marks
/// included (dialoguer 0.12, `ColorfulTheme`).
pub fn rendered_width(kind: PromptKind, prompt: &str) -> usize {
    let text = prompt.chars().count();
    match kind {
        PromptKind::Confirm => "? ".len() + text + " (y/n) › yes".chars().count(),
        PromptKind::Input { answer } => "? ".len() + text + " › ".chars().count() + answer,
        PromptKind::List => "? ".len() + text + " ›".chars().count(),
    }
}

/// Columns a list item takes on screen (`⬚ <item>`).
pub fn rendered_item_width(item: &str) -> usize {
    "⬚ ".chars().count() + item.chars().count()
}

/// What the wizard says before it asks for the theme hook.
pub const THEME_HOOK_NOTE: &str = "\
The next capture records a theme switch anyway. Yes installs
Omarchy's theme-set hook (`omarchy hook install theme-set`), which
records it the moment it happens.";

/// What the wizard says before it asks for a backfill. Under ADR-0028
/// most of an older history is routine; what is left opens as drift and
/// can be dismissed "before Seldon" right after the capture (ADR-0033).
pub const BACKFILL_NOTE: &str = "\
A new logbook looks back 90 days: the package log and snapshots
since the date below. Most of it is routine history; the rest you
can dismiss as \"before Seldon\" in one step after the capture.
Enter takes the date; none records from now on. The events stay
in the ledger.";

/// The wizard's agent items, in [`HARNESSES`] order, as (name, label).
/// The Omarchy-Agent kit is a private template, not Omarchy's agent: it
/// is offered only when its directory exists (`kit_present`).
pub fn harness_items(kit_present: bool) -> Vec<(&'static str, &'static str)> {
    HARNESSES
        .iter()
        .filter(|name| **name != "omarchy-agent" || kit_present)
        .map(|name| {
            let label = match *name {
                "claude-code" => "Claude Code hooks (user-wide)",
                "omarchy-agent" => "Omarchy-Agent kit (private template)",
                "skills" => "Seldon agent skill (into existing skill folders)",
                other => other,
            };
            (*name, label)
        })
        .collect()
}

/// A harness in the summary.
fn harness_label(name: &str) -> &str {
    match name {
        "claude-code" => "Claude Code hooks",
        "omarchy-agent" => "Omarchy-Agent kit",
        "skills" => "Seldon agent skill",
        other => other,
    }
}

/// A harness that is set up, for the summary's Agents row.
fn agent_result(h: &setup::HarnessReport) -> String {
    match h.name.as_str() {
        "claude-code" => "Claude Code hooks (user-wide)".into(),
        // done without a folder: nothing to put the skill in yet
        "skills" if h.json["dirs"].as_array().is_some_and(Vec::is_empty) => {
            "Seldon agent skill (no agent skill folder yet)".into()
        }
        other => harness_label(other).into(),
    }
}

/// A collector in the summary's Recording row.
fn collector_label(name: &str) -> &str {
    match name {
        "snapper" => "snapshots",
        "pacman" => "packages",
        "omarchy" => "Omarchy updates",
        "plugins" => "plugins",
        "theme" => "themes",
        "config" => "config files",
        other => other,
    }
}

/// The lines above the read grant when snapshots are not readable.
pub const SNAPPER_OPTIONAL: &str = "\
Optional, snapshots in the timeline: read access to the snapshot list
and info files, nothing else. Asks for your password once:";

/// The lines above the revert and the read grant for a user in snapper's
/// `ALLOW_USERS` (ADR-0026).
pub const SNAPPER_RECOMMENDED: &str = "\
Recommended: a read grant instead of the snapper opt-in. It empties
ALLOW_USERS, turns SYNC_ACL off and asks for your password once:";

/// The result `init` prints: a few aligned rows, then what is left to do
/// (or that nothing is), then the optional steps.
#[derive(Debug, Default)]
struct Summary {
    rows: Vec<(&'static str, String)>,
    steps: Vec<String>,
    optional: Vec<(&'static str, String)>,
}

impl Summary {
    fn row(&mut self, label: &'static str, value: impl Into<String>) {
        self.rows.push((label, value.into()));
    }

    /// One more line under the last row.
    fn more(&mut self, value: String) {
        self.rows.push(("", value));
    }

    fn step(&mut self, step: String) {
        self.steps.push(step);
    }

    fn optional(&mut self, intro: &'static str, command: String) {
        self.optional.push((intro, command));
    }

    fn human(&self) -> String {
        let mut out: Vec<String> = self
            .rows
            .iter()
            .map(|(label, value)| format!("{label:<12}{value}"))
            .collect();
        out.push(String::new());
        if self.steps.is_empty() {
            out.push(NOTHING_TO_DO.into());
        } else {
            out.push("Next steps:".into());
            out.extend(self.steps.iter().map(|s| format!("  {s}")));
        }
        for (intro, command) in &self.optional {
            out.push(String::new());
            out.push((*intro).into());
            out.push(format!("  {command}"));
        }
        out.join("\n")
    }
}

/// The last line of a clean `init`.
pub const NOTHING_TO_DO: &str = "Seldon is recording. Nothing else to do.";

/// Undoes a layout that failed: removes what `init` created for the
/// logbook (`created`, the highest folder that was not there; else the
/// contents of `root`, which was empty: both checked under the lock) and
/// puts `config.toml` back as it was (`previous`; removed when there was
/// none). Returns the sentence for the error message.
fn undo_layout(
    root: &Path,
    created: Option<&Path>,
    config_file: &Path,
    previous: Option<&[u8]>,
) -> String {
    let remove = |path: &Path| -> std::io::Result<()> {
        let result = if path.is_dir() && !path.is_symlink() {
            std::fs::remove_dir_all(path)
        } else {
            std::fs::remove_file(path)
        };
        match result {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        }
    };
    let mut failed = Vec::new();
    let folder = match created {
        Some(top) => remove(top).map_err(|e| format!("cannot remove {}: {e}", top.display())),
        None => std::fs::read_dir(root)
            .and_then(|mut entries| entries.try_for_each(|e| remove(&e?.path())))
            .map_err(|e| format!("cannot empty {}: {e}", root.display())),
    };
    failed.extend(folder.err());
    let config = match previous {
        Some(bytes) => sys::write_atomic(config_file, bytes).map_err(|e| format!("{e:#}")),
        None => {
            remove(config_file).map_err(|e| format!("cannot remove {}: {e}", config_file.display()))
        }
    };
    failed.extend(config.err());
    if failed.is_empty() {
        let folder = match created {
            Some(top) => format!("{} removed again", top.display()),
            None => format!("{} emptied again", root.display()),
        };
        format!("nothing was kept: {folder}, the config file as before")
    } else {
        format!("the undo failed: {}", failed.join("; "))
    }
}

/// What happened with git. Failures are reported, not fatal: the logbook
/// works without git, and `seldon doctor` shows the fix.
#[derive(Debug, Clone, PartialEq, Eq)]
enum GitOutcome {
    Skipped,
    Committed,
    InitialisedOnly,
    Failed(String),
}

impl GitOutcome {
    fn json(&self) -> serde_json::Value {
        let (repository, committed, error) = match self {
            GitOutcome::Skipped => (false, false, None),
            GitOutcome::Committed => (true, true, None),
            GitOutcome::InitialisedOnly => (true, false, None),
            GitOutcome::Failed(e) => (false, false, Some(e.as_str())),
        };
        json!({ "repository": repository, "committed": committed, "error": error })
    }
}

fn setup_git(root: &Path, wanted: bool, commit: bool) -> GitOutcome {
    if !wanted {
        return GitOutcome::Skipped;
    }
    if let Err(e) = git::init(root) {
        return GitOutcome::Failed(e);
    }
    if !commit {
        return GitOutcome::InitialisedOnly;
    }
    match git::commit_all(root, "init logbook") {
        Ok(()) => GitOutcome::Committed,
        Err(e) => GitOutcome::Failed(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Dirs;

    #[test]
    fn path_options_follow_adr_0010() {
        let home = std::env::temp_dir().join(format!("seldon-init-home-{}", std::process::id()));
        let dirs = Dirs {
            home: home.clone(),
            xdg_config_home: home.join(".config"),
            state_dir: home.join(".local/state/seldon"),
        };
        std::fs::create_dir_all(home.join("src")).unwrap();
        std::fs::create_dir_all(home.join("Projects")).unwrap();
        let opts = path_options(&dirs);
        assert_eq!(opts[0].path, home.join("Seldon"));
        assert!(opts[1].path.ends_with("Seldon"));
        // Projects comes before src in the ADR's order
        assert_eq!(opts[2].path, home.join("Projects/seldon"));
        std::fs::remove_dir_all(home.join("Projects")).unwrap();
        std::fs::remove_dir_all(home.join("src")).unwrap();
        assert_eq!(path_options(&dirs).len(), 2);
        std::fs::remove_dir_all(&home).unwrap();
    }

    /// WP-118: the wizard's texts, pinned, each drawn on one line of a
    /// 70-column terminal with dialoguer's marks; the notes say nothing
    /// about a red pill.
    #[test]
    fn wizard_texts_fit_and_say_what_0_1_4_does() {
        use PromptKind::{Confirm, Input, List};
        // a date for the backfill, a short answer for the rest
        let questions = [
            (prompts::PATH, List),
            (prompts::CUSTOM_PATH, Input { answer: 20 }),
            (prompts::LANGUAGE, List),
            (prompts::OBSIDIAN, Confirm),
            (prompts::COLLECTORS, List),
            (prompts::WATCH_PATHS, List),
            (prompts::MORE_PATHS, Input { answer: 20 }),
            (prompts::HARNESSES, List),
            (prompts::THEME_HOOK, Confirm),
            (prompts::GIT, Confirm),
            (prompts::BACKFILL, Input { answer: 10 }),
            (prompts::BASELINE, Confirm),
        ];
        // one column short of the edge: a line that fills it puts the
        // cursor past it on some terminals
        for (prompt, kind) in questions {
            assert!(
                rendered_width(kind, prompt) < WIZARD_COLUMNS,
                "{prompt}: {} columns",
                rendered_width(kind, prompt)
            );
        }
        // dialoguer's marks, counted as on screen
        assert_eq!(rendered_width(Confirm, prompts::THEME_HOOK), 46);
        assert_eq!(
            rendered_width(Confirm, "x"),
            "? x (y/n) › yes".chars().count()
        );
        assert_eq!(rendered_width(List, "x"), "? x ›".chars().count());
        assert_eq!(
            rendered_width(Input { answer: 1 }, "x"),
            "? x › a".chars().count()
        );
        let items: Vec<&str> = harness_items(true).into_iter().map(|(_, l)| l).collect();
        let watch = Config::default().watch_paths;
        for item in items
            .iter()
            .copied()
            .chain(Collectors::NAMES)
            .chain(watch.iter().map(String::as_str))
        {
            assert!(rendered_item_width(item) < WIZARD_COLUMNS, "{item}");
        }
        for note in [
            THEME_HOOK_NOTE,
            BACKFILL_NOTE,
            SNAPPER_OPTIONAL,
            SNAPPER_RECOMMENDED,
            &baseline_note(123_456, 123_456),
        ] {
            for line in note.lines() {
                assert!(line.chars().count() < WIZARD_COLUMNS, "{line}");
            }
        }
        assert_eq!(prompts::THEME_HOOK, "Record theme switches instantly?");
        assert_eq!(
            THEME_HOOK_NOTE,
            "The next capture records a theme switch anyway. Yes installs\n\
             Omarchy's theme-set hook (`omarchy hook install theme-set`), which\n\
             records it the moment it happens."
        );
        assert_eq!(
            BACKFILL_NOTE,
            "A new logbook looks back 90 days: the package log and snapshots\n\
             since the date below. Most of it is routine history; the rest you\n\
             can dismiss as \"before Seldon\" in one step after the capture.\n\
             Enter takes the date; none records from now on. The events stay\n\
             in the ledger."
        );
        assert!(BACKFILL_NOTE.contains(&format!("looks back {LOOKBACK_DAYS} days")));
        assert!(!BACKFILL_NOTE.contains("red") && !BACKFILL_NOTE.contains("crises"));
        assert_eq!(
            prompts::GIT,
            "Keep the logbook in git, with a first commit?"
        );
        assert_eq!(prompts::BACKFILL, "Backfill since (YYYY-MM-DD, or none)");
        assert_eq!(prompts::BASELINE, "Dismiss them as \"before Seldon\"?");
        assert_eq!(
            baseline_note(40, 0),
            "The backfill opened 40 drift item(s):\n\
             changes from before Seldon. The baseline dismisses them with the\n\
             reason \"before Seldon\"."
        );
        assert!(baseline_note(5, 2).starts_with("The backfill opened 5 drift item(s) (2 crisis):"));
    }

    /// WP-119: the look-back starts at local midnight 90 days before the
    /// day, in the clock's offset; the wizard's "none" (or nothing) records
    /// from now on.
    #[test]
    fn the_look_back_starts_90_days_back_at_midnight() {
        let now = DateTime::parse_from_rfc3339("2026-10-15T12:34:56+02:00").unwrap();
        assert_eq!(
            lookback_start(now, LOOKBACK_DAYS).to_rfc3339(),
            "2026-07-17T00:00:00+02:00"
        );
        // across a year and a leap day
        let now = DateTime::parse_from_rfc3339("2028-03-01T00:00:01-05:00").unwrap();
        assert_eq!(
            lookback_start(now, 90).to_rfc3339(),
            "2027-12-02T00:00:00-05:00"
        );
        for typed in ["", "  ", "none", "None", " NONE "] {
            assert!(no_backfill(typed), "{typed:?}");
        }
        assert!(!no_backfill("2026-07-17"));
    }

    /// WP-119: which modes ask, and what `--json` calls them.
    #[test]
    fn only_plain_init_and_ask_need_a_terminal() {
        // plain init with its location named asks nothing
        assert!(InitMode::Location.asks_with(false) && !InitMode::Location.asks_with(true));
        assert!(InitMode::Ask.asks_with(true) && InitMode::Ask.asks_with(false));
        assert!(!InitMode::Defaults.asks_with(false) && !InitMode::NonInteractive.asks_with(false));
        assert_eq!(InitMode::default(), InitMode::Location);
        assert_eq!(
            [
                InitMode::Location,
                InitMode::Defaults,
                InitMode::Ask,
                InitMode::NonInteractive
            ]
            .map(InitMode::as_str),
            ["location", "defaults", "ask", "non-interactive"]
        );
    }

    /// WP-118: the kit is a private template; without its directory the
    /// wizard does not offer it.
    #[test]
    fn the_kit_item_shows_only_with_the_kit() {
        assert_eq!(
            harness_items(false),
            [
                ("claude-code", "Claude Code hooks (user-wide)"),
                ("skills", "Seldon agent skill (into existing skill folders)"),
            ]
        );
        assert_eq!(
            harness_items(true),
            [
                ("claude-code", "Claude Code hooks (user-wide)"),
                ("omarchy-agent", "Omarchy-Agent kit (private template)"),
                ("skills", "Seldon agent skill (into existing skill folders)"),
            ]
        );
    }

    /// WP-118: a clean run ends with one line and no next steps; the
    /// optional grant comes after it.
    #[test]
    fn the_summary_lists_next_steps_only_when_some_are_left() {
        let mut s = Summary::default();
        s.row("Logbook", "~/Seldon (English, git repository)");
        s.row("Snapshots", "recorded");
        assert_eq!(
            s.human(),
            "Logbook     ~/Seldon (English, git repository)\n\
             Snapshots   recorded\n\
             \n\
             Seldon is recording. Nothing else to do."
        );
        s.optional(
            SNAPPER_OPTIONAL,
            "sudo setfacl -m u:$USER:rx /.snapshots".into(),
        );
        assert!(s.human().ends_with(&format!(
            "Nothing else to do.\n\n{SNAPPER_OPTIONAL}\n  sudo setfacl -m u:$USER:rx /.snapshots"
        )));
        s.more("Claude Code hooks: not set up".into());
        s.step("seldon hook install claude-code".into());
        let text = s.human();
        assert!(
            text.contains("\n            Claude Code hooks: not set up\n"),
            "{text}"
        );
        assert!(
            text.contains("\n\nNext steps:\n  seldon hook install claude-code\n\n"),
            "{text}"
        );
        assert!(!text.contains(NOTHING_TO_DO), "{text}");
    }

    #[test]
    fn typed_watch_paths_are_stored_under_home() {
        let dirs = Dirs {
            home: "/home/user".into(),
            xdg_config_home: "/home/user/.config".into(),
            state_dir: "/home/user/.local/state/seldon".into(),
        };
        assert_eq!(
            typed_watch_paths(
                &dirs,
                ".config/nvim, ~/.config/kitty ,$HOME/bin/,, /etc/keyd/default.conf"
            ),
            [
                "~/.config/nvim",
                "~/.config/kitty",
                "~/bin",
                "/etc/keyd/default.conf"
            ]
        );
        assert!(typed_watch_paths(&dirs, " ").is_empty());
    }
}
