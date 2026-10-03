//! `seldon init`: the wizard (SPEC-ENGINE §9, ADR-0010).
//!
//! Steps: path → language → Obsidian → collectors → watched paths →
//! harnesses → theme hook → git → backfill. Each step is skipped when its
//! flag is given; `--non-interactive` skips them all and takes flags or
//! defaults. Defaults come from an existing `config.toml` where it has a
//! value.
//!
//! Then the logbook is written, the harnesses are set up (inside the
//! first commit), the first capture runs (`capture --all`, with `--since`
//! as the backfill window), a backfill can be marked as the pre-Seldon
//! baseline, and the capture is committed; then `seldon dossier` fills
//! `system/*.md` once (WP-035, its own commit), and the theme hook is
//! installed on opt-in and recorded as the engine's own write, so the next
//! capture explains its `config-add` (SPEC-ENGINE §5 rule 7, WP-038).
//! The steps after the layout report failures; they never undo the
//! logbook ([`super::setup`]).

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
use crate::collectors::Sources;
use crate::collectors::config::OwnOp;
use crate::config::{Collectors, Config, HARNESSES};
use crate::error::{Error, Result};
use crate::logbook::layout::{self, NewLogbook};
use crate::logbook::{Logbook, git, lock};
use crate::model::Language;
use crate::sys;

/// Project folders checked for ADR-0010 option 3, in order.
pub const PROJECT_FOLDERS: [&str; 5] = ["Work", "Projects", "dev", "src", "code"];

/// `seldon init` flags.
#[derive(Debug, Clone, Default)]
pub struct InitArgs {
    pub path: Option<PathBuf>,
    pub non_interactive: bool,
    pub language: Option<Language>,
    pub obsidian: bool,
    pub harnesses: Vec<String>,
    /// `--git` / `--no-git`; `None` = default (on).
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
    let interactive = !args.non_interactive;
    if interactive && !(std::io::stdin().is_terminal() && std::io::stderr().is_terminal()) {
        return Err(Error::user(
            "not a terminal; use `seldon init --non-interactive` with --path, --language, … for defaults",
        ));
    }

    let mut choices = if interactive {
        wizard(ctx, &args, existing.as_ref())?
    } else {
        defaults(ctx, &args, existing.as_ref())
    };
    let root = choices.root.clone();
    let lock = lock::acquire(&ctx.dirs.lock_file())?;
    if Logbook::is_initialised(&root) {
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
    let files = layout::create(&root, &spec)?;

    let mut config = existing.unwrap_or_default();
    config.logbook = Some(root.clone());
    config.language = choices.language;
    config.collectors = choices.collectors;
    config.watch_paths = choices.watch_paths.clone();
    config.harnesses = choices.harnesses.clone();
    config.save(&config_file)?;

    // inside the first commit: the harness files are part of the logbook
    let harnesses = setup::harnesses(&ctx.dirs, &root, &choices.harnesses);
    let git = setup_git(&root, choices.git, !ctx.no_commit);
    let snapper = doctor::check_snapper(&config);
    // the capture and the baseline take the lock themselves
    drop(lock);

    // every later step works on this logbook, whatever --logbook or
    // SELDON_LOGBOOK name
    let mut lb_ctx = ctx.clone();
    lb_ctx.logbook_flag = Some(root.clone());
    let capture = first_capture(&lb_ctx, &mut choices, interactive);
    // the dossier once, after the first capture (WP-035); `capture` and
    // `status` never refresh it
    let dossier = first_dossier(&lb_ctx, &capture);
    let theme_hook = if choices.theme_hook {
        setup::install_theme_hook(&ctx.dirs, &Sources::from_env().omarchy)
    } else {
        ThemeHook::NotRequested
    };
    let own_hook = match &theme_hook {
        ThemeHook::Installed { hook, .. } => Some(setup::record_own_writes(
            ctx,
            &config,
            std::slice::from_ref(hook),
            "seldon init --theme-hook",
            OwnOp::Install,
        )),
        _ => None,
    };

    let mut next = vec!["seldon doctor".to_string()];
    for h in harnesses
        .iter()
        .filter(|h| !h.done && h.name == "claude-code")
    {
        next.push(format!("seldon hook install {}", h.name));
    }
    if !capture.ran {
        next.push("seldon capture --all".to_string());
    }
    if !dossier.ran {
        next.push("seldon dossier".to_string());
    }
    if let Some((open, _)) = capture.open_after.filter(|(open, _)| *open > 0) {
        next.push(format!(
            "seldon drift   # {open} open drift item(s) to link, explain or dismiss"
        ));
    }
    if let Some(fix) = theme_hook.fix() {
        next.push(format!("{fix}   # optional: the theme hook"));
    }
    if snapper.status == Status::Degraded
        && let Some(fix) = &snapper.fix
    {
        next.push(format!(
            "{fix}   # optional: snapshots in the timeline (ADR-0011)"
        ));
    }

    let shown_root = ctx.dirs.display(&root);
    let mut human = format!(
        "Logbook created at {shown_root} (machine {machine_id}, language {}, {} files).\nConfig: {}\n{}\n",
        choices.language,
        files.len(),
        ctx.dirs.display(&config_file),
        setup::SKIP_PATHS_HINT,
    );
    for h in &harnesses {
        human.push_str(&format!("Harness {}: {}\n", h.name, h.human));
    }
    human.push_str(&format!(
        "Git: {}\nSnapper: {} — {}\nFirst capture: {}\nDossier: {}\n",
        git.describe(),
        snapper.status.as_str(),
        snapper.message,
        capture.human,
        dossier.human,
    ));
    if let Some(t) = theme_hook.human(&ctx.dirs) {
        human.push_str(&format!("Theme hook: {t}\n"));
    }
    if let Some(Err(e)) = &own_hook {
        human.push_str(&format!("Theme hook: {}\n", setup::own_writes_warning(e)));
    }
    if choices.obsidian {
        human.push_str("Obsidian: open the folder as a vault.\n");
    }
    human.push_str("Next steps:\n");
    for step in &next {
        human.push_str(&format!("  {step}\n"));
    }

    let mut theme_hook_json = theme_hook.json();
    if let Some(r) = &own_hook {
        theme_hook_json["ownWrites"] = setup::own_writes_json(r);
    }
    Ok(Output::ok(
        human.trim_end(),
        json!({
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
    /// Open drift items and crises after the step (baseline included).
    open_after: Option<(usize, usize)>,
    human: String,
    json: Value,
}

/// Runs `capture --all` (with `--since` as the backfill window), then the
/// pre-Seldon baseline when it was chosen (or, in the wizard, confirmed
/// now that the drift count is known), then commits both. Failures are
/// reported, never fatal: the logbook exists.
fn first_capture(ctx: &Context, choices: &mut Choices, interactive: bool) -> CaptureStep {
    if !choices.capture {
        return CaptureStep {
            ran: false,
            open_after: None,
            human: "skipped (--no-capture)".into(),
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
                open_after: None,
                human: format!("failed: {e}"),
                json: json!({ "ran": false, "error": e.to_string() }),
            };
        }
    };
    let written = out.json["written"].as_u64().unwrap_or(0);
    let mut json = out.json;
    json["ran"] = json!(true);
    json["since"] = json!(since);
    let mut human = format!("{written} event(s)");
    if let Some(s) = &since {
        human.push_str(&format!(" since {s}"));
    }
    let degraded: Vec<&str> = json["collectors"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|c| c["ran"] == true && c["ok"] == false)
        .filter_map(|c| c["name"].as_str())
        .collect();
    if !degraded.is_empty() {
        human.push_str(&format!(
            "; degraded: {} (see seldon doctor)",
            degraded.join(", ")
        ));
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
                human.push_str(&format!(
                    "; {} drift item(s) ({} event(s)) dismissed as \"{BASELINE_REASON}\"",
                    b.items, b.events
                ));
                json["baseline"] = json!({
                    "reason": BASELINE_REASON, "items": b.items, "events": b.events,
                });
                summary.push_str(" and pre-Seldon baseline");
                open_after = setup::open_drift(ctx);
            }
            Err(e) => {
                human.push_str(&format!("; baseline failed: {e}"));
                json["baseline"] = json!({ "reason": BASELINE_REASON, "error": e.to_string() });
            }
        }
    }
    if let Some((items, crisis)) = open_after {
        human.push_str(&format!("; {items} open drift item(s), {crisis} crisis"));
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
    human.push_str(&commit.human().replace('\n', "; "));
    json["git"] = commit.json();
    CaptureStep {
        ran: true,
        open_after,
        human,
        json,
    }
}

/// What the first `seldon dossier` did.
struct DossierStep {
    ran: bool,
    human: String,
    json: Value,
}

/// Runs `seldon dossier` (all sections) once the first capture ran; it
/// takes the lock, commits `seldon: dossier` and rebuilds the index
/// itself. A failure is reported, never fatal: the logbook exists.
fn first_dossier(ctx: &Context, capture: &CaptureStep) -> DossierStep {
    if !capture.ran {
        return DossierStep {
            ran: false,
            human: "skipped (no first capture)".into(),
            json: json!({ "ran": false, "reason": "no first capture" }),
        };
    }
    match dossier::run(ctx, DossierArgs::default()) {
        Ok(out) => {
            let mut human = out.human.lines().next().unwrap_or_default().to_string();
            let warnings = out.json["warnings"].as_array().map_or(0, Vec::len);
            if warnings > 0 {
                human.push_str(&format!("; {warnings} warning(s) (see seldon dossier)"));
            }
            let mut json = out.json;
            json["ran"] = json!(true);
            DossierStep {
                ran: true,
                human,
                json,
            }
        }
        Err(e) => DossierStep {
            ran: false,
            human: format!("failed: {e}"),
            json: json!({ "ran": false, "error": e.to_string() }),
        },
    }
}

/// The wizard's baseline question, asked once the count is known.
fn ask_baseline(items: usize, crisis: usize) -> bool {
    eprintln!(
        "The backfill opened {items} drift item(s) ({crisis} crisis): changes from before \
         Seldon, none of them in a case."
    );
    Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt(format!(
            "Mark them as the pre-Seldon baseline (dismissed, reason \"{BASELINE_REASON}\")?"
        ))
        .default(true)
        .interact()
        .unwrap_or(false)
}

/// Flags, else the existing config, else built-in defaults.
fn defaults(ctx: &Context, args: &InitArgs, existing: Option<&Config>) -> Choices {
    let base = existing.cloned().unwrap_or_default();
    let (root, _) = ctx.resolve_logbook(args.path.as_deref(), existing);
    Choices {
        root,
        language: args
            .language
            .or(existing.map(|c| c.language))
            .unwrap_or_else(locale_language),
        obsidian: args.obsidian,
        collectors: base.collectors,
        watch_paths: base.watch_paths,
        harnesses: if args.harnesses.is_empty() {
            base.harnesses
        } else {
            args.harnesses.clone()
        },
        git: args.git.unwrap_or(true),
        capture: args.capture,
        since: args.since,
        baseline: args.since.is_some().then_some(args.baseline),
        theme_hook: args.theme_hook,
    }
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

fn wizard(ctx: &Context, args: &InitArgs, existing: Option<&Config>) -> Result<Choices> {
    let mut c = defaults(ctx, args, existing);
    let theme = ColorfulTheme::default();
    let prompt_err =
        |e: dialoguer::Error| Error::Engine(anyhow::Error::new(e).context("wizard input failed"));

    if args.path.is_none() && ctx.logbook_flag.is_none() {
        let mut options = path_options(&ctx.dirs);
        if !options.iter().any(|o| o.path == c.root) {
            options.insert(
                0,
                PathOption {
                    label: "current setting".into(),
                    path: c.root.clone(),
                },
            );
        }
        let mut items: Vec<String> = options
            .iter()
            .map(|o| format!("{}  ({})", ctx.dirs.display(&o.path), o.label))
            .collect();
        items.push("custom path…".into());
        let pick = Select::with_theme(&theme)
            .with_prompt("Where should the logbook live?")
            .items(&items)
            .default(0)
            .interact()
            .map_err(prompt_err)?;
        c.root = match options.get(pick) {
            Some(o) => o.path.clone(),
            None => {
                let typed: String = Input::with_theme(&theme)
                    .with_prompt("Logbook path")
                    .interact_text()
                    .map_err(prompt_err)?;
                ctx.dirs.expand(typed.trim())
            }
        };
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
            .with_prompt("Language of the logbook prose")
            .items(&items)
            .default(default)
            .interact()
            .map_err(prompt_err)?;
        c.language = Language::ALL[pick];
    }

    if !args.obsidian {
        c.obsidian = Confirm::with_theme(&theme)
            .with_prompt("Add Obsidian settings (.obsidian/)?")
            .default(false)
            .interact()
            .map_err(prompt_err)?;
    }

    let checked: Vec<bool> = Collectors::NAMES
        .iter()
        .map(|n| c.collectors.get(n).unwrap_or(true))
        .collect();
    let picked = MultiSelect::with_theme(&theme)
        .with_prompt("Collectors (space toggles, enter confirms)")
        .items(Collectors::NAMES)
        .defaults(&checked)
        .interact()
        .map_err(prompt_err)?;
    let names: Vec<&str> = picked.iter().map(|i| Collectors::NAMES[*i]).collect();
    c.collectors = Collectors::only(&names);

    if c.collectors.config {
        let keep = MultiSelect::with_theme(&theme)
            .with_prompt("Watched config paths")
            .items(&c.watch_paths)
            .defaults(&vec![true; c.watch_paths.len()])
            .interact()
            .map_err(prompt_err)?;
        let mut paths: Vec<String> = keep.iter().map(|i| c.watch_paths[*i].clone()).collect();
        let extra: String = Input::with_theme(&theme)
            .with_prompt("More paths, comma-separated, relative to your home (empty for none)")
            .allow_empty(true)
            .interact_text()
            .map_err(prompt_err)?;
        paths.extend(typed_watch_paths(&ctx.dirs, &extra));
        c.watch_paths = paths;
    }

    if args.harnesses.is_empty() {
        let checked: Vec<bool> = HARNESSES
            .iter()
            .map(|h| c.harnesses.iter().any(|x| x == h))
            .collect();
        let kit = setup::kit_dir(&ctx.dirs);
        let kit_label = format!(
            "Omarchy-Agent kit: guard and skills into .claude/ (omarchy-agent; {} {})",
            if kit.is_dir() { "from" } else { "not found at" },
            ctx.dirs.display(&kit)
        );
        let picked = MultiSelect::with_theme(&theme)
            .with_prompt("Agent harnesses (space toggles, enter confirms)")
            .items(&["Claude Code hooks (claude-code)".to_string(), kit_label])
            .defaults(&checked)
            .interact()
            .map_err(prompt_err)?;
        c.harnesses = picked.iter().map(|i| HARNESSES[*i].to_string()).collect();
    }

    if !args.theme_hook {
        c.theme_hook = Confirm::with_theme(&theme)
            .with_prompt(
                "Record theme switches the moment they happen? \
                 (runs `omarchy hook install theme-set`; without it the next capture finds them)",
            )
            .default(false)
            .interact()
            .map_err(prompt_err)?;
    }

    if args.git.is_none() {
        c.git = Confirm::with_theme(&theme)
            .with_prompt("Make the logbook a git repository with a first commit?")
            .default(true)
            .interact()
            .map_err(prompt_err)?;
    }

    if c.capture && args.since.is_none() {
        eprintln!("{BACKFILL_NOTE}");
        let typed: String = Input::with_theme(&theme)
            .with_prompt("Backfill since (YYYY-MM-DD; empty: record from now on)")
            .allow_empty(true)
            .validate_with(|s: &String| -> std::result::Result<(), String> {
                if s.trim().is_empty() {
                    return Ok(());
                }
                setup::parse_since(s).map(|_| ()).map_err(|e| e.to_string())
            })
            .interact_text()
            .map_err(prompt_err)?;
        c.since = match typed.trim() {
            "" => None,
            s => Some(setup::parse_since(s)?),
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

/// What the wizard says before it asks for a backfill (WP-013 FINDINGS
/// §2.2: a backfill opens with a red pill).
const BACKFILL_NOTE: &str = "\
A new logbook records changes from now on. A backfill also records older
changes (the package log, snapshots). None of them belongs to a case, so
each one opens as drift: the bar pill starts red, often with crises.
After the capture you can mark the backfill as the pre-Seldon baseline:
every open item is dismissed with the reason \"pre-Seldon baseline\";
the events stay in the ledger.";

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
    fn describe(&self) -> String {
        match self {
            GitOutcome::Skipped => "skipped (--no-git)".into(),
            GitOutcome::Committed => {
                "repository initialised, first commit \"seldon: init logbook\"".into()
            }
            GitOutcome::InitialisedOnly => {
                "repository initialised, not committed (--no-commit)".into()
            }
            GitOutcome::Failed(e) => format!("not set up: {e}"),
        }
    }

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
