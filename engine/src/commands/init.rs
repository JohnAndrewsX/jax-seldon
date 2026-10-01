//! `seldon init`: the wizard (SPEC-ENGINE §9, ADR-0010).
//!
//! Steps: path → language → Obsidian → collectors → watched paths →
//! harnesses → git. Each step is skipped when its flag is given;
//! `--non-interactive` skips them all and takes flags or defaults. Defaults
//! come from an existing `config.toml` where it has a value.

use std::io::IsTerminal as _;
use std::path::{Path, PathBuf};

use anyhow::Context as _;
use chrono::Local;
use dialoguer::theme::ColorfulTheme;
use dialoguer::{Confirm, Input, MultiSelect, Select};
use serde_json::json;

use super::doctor::{self, Status};
use super::{Context, Output};
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
    let config_file = ctx.dirs.config_file();
    let existing = ctx.load_config()?;
    let interactive = !args.non_interactive;
    if interactive && !(std::io::stdin().is_terminal() && std::io::stderr().is_terminal()) {
        return Err(Error::user(
            "not a terminal; use `seldon init --non-interactive` with --path, --language, … for defaults",
        ));
    }

    let choices = if interactive {
        wizard(ctx, &args, existing.as_ref())?
    } else {
        defaults(ctx, &args, existing.as_ref())
    };
    let root = &choices.root;
    let _lock = lock::acquire(&ctx.dirs.lock_file())?;
    if Logbook::is_initialised(root) {
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
    if !layout::is_vacant(root).with_context(|| format!("cannot read {}", root.display()))? {
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
    let files = layout::create(root, &spec)?;

    let mut config = existing.unwrap_or_default();
    config.logbook = Some(root.clone());
    config.language = choices.language;
    config.collectors = choices.collectors;
    config.watch_paths = choices.watch_paths.clone();
    config.harnesses = choices.harnesses.clone();
    config.save(&config_file)?;

    let git = setup_git(root, choices.git, !ctx.no_commit);
    let snapper = doctor::check_snapper(&config);

    let mut next = vec!["seldon doctor".to_string()];
    for h in &choices.harnesses {
        next.push(format!("seldon hook install {h}"));
    }
    next.push("seldon capture --all".to_string());
    if snapper.status == Status::Degraded
        && let Some(fix) = &snapper.fix
    {
        next.push(format!(
            "{fix}   # optional: snapshots in the timeline (ADR-0011)"
        ));
    }

    let shown_root = ctx.dirs.display(root);
    let mut human = format!(
        "Logbook created at {shown_root} (machine {machine_id}, language {}, {} files).\nConfig: {}\nGit: {}\nSnapper: {} — {}\nFirst capture: skipped (no collectors in this engine version yet).\n",
        choices.language,
        files.len(),
        ctx.dirs.display(&config_file),
        git.describe(),
        snapper.status.as_str(),
        snapper.message,
    );
    if choices.obsidian {
        human.push_str("Obsidian: open the folder as a vault.\n");
    }
    human.push_str("Next steps:\n");
    for step in &next {
        human.push_str(&format!("  {step}\n"));
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
            "harnesses": choices.harnesses,
            "git": git.json(),
            "snapper": snapper,
            "capture": { "ran": false, "reason": "no collectors in this engine version" },
            "nextSteps": next,
        }),
    ))
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
    }
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
            .with_prompt("More paths, comma-separated (empty for none)")
            .allow_empty(true)
            .interact_text()
            .map_err(prompt_err)?;
        paths.extend(
            extra
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(String::from),
        );
        c.watch_paths = paths;
    }

    if args.harnesses.is_empty() {
        let checked: Vec<bool> = HARNESSES
            .iter()
            .map(|h| c.harnesses.iter().any(|x| x == h))
            .collect();
        let picked = MultiSelect::with_theme(&theme)
            .with_prompt("Agent harnesses")
            .items(["Claude Code hooks (claude-code)"])
            .defaults(&checked)
            .interact()
            .map_err(prompt_err)?;
        c.harnesses = picked.iter().map(|i| HARNESSES[*i].to_string()).collect();
    }

    if args.git.is_none() {
        c.git = Confirm::with_theme(&theme)
            .with_prompt("Make the logbook a git repository with a first commit?")
            .default(true)
            .interact()
            .map_err(prompt_err)?;
    }
    Ok(c)
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
}
