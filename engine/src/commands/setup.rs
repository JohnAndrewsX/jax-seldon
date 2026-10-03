//! The steps `seldon init` runs once the logbook exists (SPEC-ENGINE §9):
//! the agent harnesses, the first capture with an optional backfill, the
//! pre-Seldon baseline, and Omarchy's theme-set hook.
//!
//! None of them fails the wizard: the logbook is already there, so each
//! step reports what it did, or why not, and the fix.
//!
//! - **Harnesses.** `claude-code` merges WP-009's hooks into
//!   `<logbook>/.claude/settings.json` ([`hook::merge_claude_hooks`]).
//!   `omarchy-agent` copies the Omarchy-Agent kit's guard and skills from a
//!   template directory ([`kit_dir`]) into `<logbook>/.claude/`, keeping
//!   files that are already there; without the directory it copies nothing
//!   and says what it would have done.
//! - **Baseline.** A backfill (`--since`) records changes from before the
//!   logbook. None of them has a case, so each opens as drift (WP-013
//!   FINDINGS §2.2). [`baseline`] dismisses every open drift item with the
//!   reason [`BASELINE_REASON`], through the same selection and resolution
//!   lines as `seldon drift dismiss` (`reconcile`, ADR-0013 §4): one
//!   ledger write, one `resolution` line per open member.
//! - **Theme hook.** Only on opt-in: writes the embedded
//!   `engine/hooks/theme-set.sh` to the state directory and runs
//!   `omarchy hook install theme-set <file>`, a fixed argument list
//!   (AGENTS.md §8). Tests stub `omarchy` (`SELDON_OMARCHY`, or `PATH`).
//!   The installed copy lies under the watched `~/.config/omarchy`, so it
//!   is recorded as the engine's own write: the next capture explains its
//!   `config-add` (SPEC-ENGINE §5 rule 7). [`theme_hook_step`] holds the
//!   state lock from the script to the record, so no capture sees the copy
//!   first; while another `seldon` holds it, nothing is written.
//!   `init --remove-theme-hook` ([`remove_theme_hook`]) deletes that copy
//!   and the script in the state directory; the deletion is recorded the
//!   same way ([`delete_own_file`]), so the next capture explains its
//!   `config-remove`. Omarchy has no command to remove a hook.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::Context as _;
use chrono::{DateTime, FixedOffset, Local, NaiveDate, TimeZone as _};
use serde_json::{Value, json};

use super::{Context, emit, hook};
use crate::collectors::config::{self, OwnOp};
use crate::config::{Config, Dirs};
use crate::error::{Error, Result};
use crate::index;
use crate::logbook::lock::Lock;
use crate::model::event::{ACTOR_HUMAN, Event, Resolution};
use crate::reconcile::{self, Resolve};
use crate::sys::{self, Run};

/// The reason of every resolution [`baseline`] writes.
pub const BASELINE_REASON: &str = "pre-Seldon baseline";

/// Environment variable naming the Omarchy-Agent kit's template directory.
pub const KIT_ENV: &str = "SELDON_OMARCHY_AGENT_KIT";

/// The kit's template directory under `$XDG_DATA_HOME` (`~/.local/share`).
pub const KIT_DIR: &str = "seldon/harness/omarchy-agent";

/// Where the harnesses live inside the logbook (SPEC-LOGBOOK §2).
pub const HARNESS_DIR: &str = ".claude";

/// File name of the theme hook, in the state directory and in Omarchy's
/// `~/.config/omarchy/hooks/theme-set.d/` (`omarchy hook install` keeps
/// the base name): a name of Seldon's own, so it never replaces a hook of
/// the user's.
pub const THEME_HOOK_NAME: &str = "seldon-theme-set.sh";

/// `engine/hooks/theme-set.sh`, compiled in (one static binary).
pub const THEME_HOOK_SCRIPT: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/hooks/theme-set.sh"));

/// The line `init` prints under the config file: how to keep a file that
/// changes all the time, or holds secrets, out of the config collector.
pub const SKIP_PATHS_HINT: &str = "Watched files that change all the time or hold secrets: list them in [redaction] skipPaths there.";

/// How long `omarchy hook install` may take.
const OMARCHY_TIMEOUT: Duration = Duration::from_secs(30);

/// `--since` of `init`: RFC 3339, or a date (`YYYY-MM-DD`, local midnight).
pub fn parse_since(s: &str) -> Result<DateTime<FixedOffset>> {
    let s = s.trim();
    if let Ok(t) = DateTime::parse_from_rfc3339(s) {
        return Ok(t);
    }
    let date = NaiveDate::parse_from_str(s, "%Y-%m-%d").map_err(|_| {
        Error::user(format!(
            "--since `{s}` is neither a date (YYYY-MM-DD) nor an RFC 3339 time"
        ))
    })?;
    let midnight = date.and_hms_opt(0, 0, 0).expect("midnight exists");
    Local
        .from_local_datetime(&midnight)
        .earliest()
        .map(|t| t.fixed_offset())
        .ok_or_else(|| Error::user(format!("--since `{s}`: midnight does not exist here")))
}

// ---------------------------------------------------------------------------
// Harnesses
// ---------------------------------------------------------------------------

/// What setting up one harness did, as `init` reports it.
#[derive(Debug, Clone)]
pub struct HarnessReport {
    pub name: String,
    /// One line for the human output.
    pub human: String,
    pub json: Value,
    /// Whether the harness is fully set up (else `init` keeps a next step).
    pub done: bool,
}

/// Sets up `names` in the logbook at `root`, the Omarchy-Agent kit first
/// so that Claude Code's hooks are merged into a settings file the kit
/// may bring.
pub fn harnesses(dirs: &Dirs, root: &Path, names: &[String]) -> Vec<HarnessReport> {
    let mut ordered: Vec<&String> = names.iter().collect();
    ordered.sort_by_key(|n| n.as_str() != "omarchy-agent");
    ordered.dedup();
    ordered
        .into_iter()
        .map(|name| match name.as_str() {
            "omarchy-agent" => omarchy_agent(dirs, root),
            "claude-code" => claude_code(dirs, root),
            other => HarnessReport {
                name: other.to_string(),
                human: format!("unknown harness `{other}`; nothing set up"),
                json: json!({ "error": "unknown harness" }),
                done: false,
            },
        })
        .collect()
}

fn claude_code(dirs: &Dirs, root: &Path) -> HarnessReport {
    let path = root.join(HARNESS_DIR).join("settings.json");
    let shown = format!("{HARNESS_DIR}/settings.json");
    match hook::merge_claude_hooks(&path, &shown) {
        Ok(m) => HarnessReport {
            name: "claude-code".into(),
            human: format!(
                "{shown}: {} hook(s) added, {} already there",
                m.added.len(),
                m.present.len()
            ),
            json: json!({ "settings": path, "added": m.added, "present": m.present }),
            done: true,
        },
        Err(e) => HarnessReport {
            name: "claude-code".into(),
            human: format!(
                "{shown}: not set up: {e} (fix: seldon hook install claude-code; {})",
                dirs.display(&path)
            ),
            json: json!({ "settings": path, "error": e.to_string() }),
            done: false,
        },
    }
}

/// The Omarchy-Agent kit's template directory: `$SELDON_OMARCHY_AGENT_KIT`,
/// else `${XDG_DATA_HOME:-~/.local/share}/seldon/harness/omarchy-agent`.
/// Its layout is that of `<logbook>/.claude/` (e.g. `hooks/guard.py`,
/// `skills/<name>/SKILL.md`, `settings.json`).
pub fn kit_dir(dirs: &Dirs) -> PathBuf {
    if let Some(p) = std::env::var(KIT_ENV).ok().filter(|p| !p.is_empty()) {
        return dirs.expand(&p);
    }
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| dirs.home.join(".local/share"))
        .join(KIT_DIR)
}

fn omarchy_agent(dirs: &Dirs, root: &Path) -> HarnessReport {
    let template = kit_dir(dirs);
    let shown = dirs.display(&template);
    let report = |human: String, json: Value, done: bool| HarnessReport {
        name: "omarchy-agent".into(),
        human,
        json,
        done,
    };
    if !template.is_dir() {
        return report(
            format!(
                "no kit at {shown}; nothing copied. With the kit there (the layout of \
                 {HARNESS_DIR}/: guard, skills, settings.json), its files would be copied \
                 into {HARNESS_DIR}/, existing files kept (template: {KIT_ENV})"
            ),
            json!({ "template": template, "present": false, "copied": [], "kept": [] }),
            false,
        );
    }
    match copy_tree(&template, &root.join(HARNESS_DIR)) {
        Ok(c) => report(
            format!(
                "{} file(s) copied from {shown} into {HARNESS_DIR}/{}",
                c.copied.len(),
                if c.kept.is_empty() {
                    String::new()
                } else {
                    format!(", {} kept as they were", c.kept.len())
                }
            ),
            json!({
                "template": template,
                "present": true,
                "copied": c.copied.iter().map(|p| format!("{HARNESS_DIR}/{p}")).collect::<Vec<_>>(),
                "kept": c.kept.iter().map(|p| format!("{HARNESS_DIR}/{p}")).collect::<Vec<_>>(),
            }),
            true,
        ),
        Err(e) => report(
            format!("copying from {shown} failed: {e:#}"),
            json!({ "template": template, "present": true, "error": format!("{e:#}") }),
            false,
        ),
    }
}

/// Files [`copy_tree`] copied and kept, relative to the target, sorted.
#[derive(Debug, Default, PartialEq, Eq)]
struct Copied {
    copied: Vec<String>,
    kept: Vec<String>,
}

/// Copies the regular files under `from` into `to` (permissions kept, so
/// an executable guard stays executable). A file that exists in `to` is
/// kept, never overwritten; symbolic links are not followed or copied.
fn copy_tree(from: &Path, to: &Path) -> anyhow::Result<Copied> {
    let mut out = Copied::default();
    let mut stack = vec![PathBuf::new()];
    while let Some(rel) = stack.pop() {
        let dir = from.join(&rel);
        let entries =
            std::fs::read_dir(&dir).with_context(|| format!("cannot read {}", dir.display()))?;
        for entry in entries {
            let entry = entry?;
            let kind = entry.file_type()?;
            let rel = rel.join(entry.file_name());
            if kind.is_dir() {
                stack.push(rel);
            } else if kind.is_file() {
                let target = to.join(&rel);
                let name = rel.to_string_lossy().into_owned();
                if target.exists() {
                    out.kept.push(name);
                    continue;
                }
                if let Some(parent) = target.parent() {
                    sys::create_dir_private(parent)
                        .with_context(|| format!("cannot create {}", parent.display()))?;
                }
                std::fs::copy(entry.path(), &target)
                    .with_context(|| format!("cannot copy {name}"))?;
                out.copied.push(name);
            }
        }
    }
    out.copied.sort();
    out.kept.sort();
    Ok(out)
}

// ---------------------------------------------------------------------------
// The engine's own writes
// ---------------------------------------------------------------------------

/// What recording the engine's own writes gave: the recorded `~`-paths,
/// or a line for the report.
pub type OwnRecord = std::result::Result<Vec<String>, String>;

/// Records `paths` as written by the engine with the command `by` and
/// `op` (SPEC-ENGINE §5 rule 7); only paths the config collector watches
/// are recorded. The caller holds the state lock across the write and the
/// record, so no capture sees the file in between. `Ok` names them; `Err`
/// is a line for the report, never fatal: the next capture then shows the
/// file as drift.
pub fn record_own_writes_under(
    lock: &Lock,
    ctx: &Context,
    config: &Config,
    paths: &[PathBuf],
    by: &str,
    op: OwnOp,
) -> OwnRecord {
    config::record_own_writes(lock, &ctx.dirs, config, paths, by, op).map_err(|e| format!("{e:#}"))
}

/// Records the deletion of the file `path` as the engine's own with the
/// command `by` when the config collector watches it, then deletes it,
/// under the state lock ([`config::delete_own_file`]). `Err` when the lock
/// is held or the file cannot be deleted (a record written first stays,
/// harmless); the [`OwnRecord`] as for [`record_own_writes_under`].
pub fn delete_own_file(ctx: &Context, config: &Config, path: &Path, by: &str) -> Result<OwnRecord> {
    let lock = ctx.lock()?;
    delete_own_file_under(&lock, ctx, config, path, by)
}

/// [`delete_own_file`] for a caller that already holds the state lock.
pub fn delete_own_file_under(
    lock: &Lock,
    ctx: &Context,
    config: &Config,
    path: &Path,
    by: &str,
) -> Result<OwnRecord> {
    let recorded = config::delete_own_file(lock, &ctx.dirs, config, path, by)?;
    Ok(recorded.map_err(|e| format!("{e:#}")))
}

/// The report of [`record_own_writes_under`] as one JSON value: the recorded
/// `~`-paths, or `{"error": …}`.
pub fn own_writes_json(r: &OwnRecord) -> Value {
    match r {
        Ok(paths) => json!(paths),
        Err(e) => json!({ "error": e }),
    }
}

/// The warning for a failed [`record_own_writes_under`].
pub fn own_writes_warning(error: &str) -> String {
    format!("not recorded as seldon's own write ({error}); the next capture shows it as drift")
}

// ---------------------------------------------------------------------------
// Drift after the first capture
// ---------------------------------------------------------------------------

/// Open drift items and crises, from the index model (`None`: unreadable).
pub fn open_drift(ctx: &Context) -> Option<(usize, usize)> {
    let (config, logbook) = ctx.open_logbook().ok()?;
    let built = index::derive(ctx, &config, &logbook).ok()?;
    Some((built.index.summary.open_drift, built.index.summary.crisis))
}

/// What [`baseline`] resolved.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Baseline {
    /// Drift items dismissed (a pacman transaction is one item).
    pub items: usize,
    /// `resolution` lines written, one per open member.
    pub events: usize,
}

/// Marks every open drift item as the pre-Seldon baseline: a `dismissed`
/// resolution with [`BASELINE_REASON`] for each open member, oldest item
/// first, in one ledger write; then the index is rebuilt. Every open item
/// counts, also those past the index's 200-item cap (ADR-0020). Nothing
/// open, nothing written.
pub fn baseline(ctx: &Context) -> Result<Baseline> {
    let (config, logbook) = ctx.open_logbook()?;
    let lock = ctx.lock()?;
    let built = index::derive(ctx, &config, &logbook)?;
    let mut open: Vec<&Event> = built
        .folded
        .iter()
        .map(|f| &f.event)
        .filter(|e| built.open_drift.contains(&e.id))
        .collect();
    open.sort_by_key(|e| (e.ts, e.id));

    let resolve = Resolve {
        resolution: Resolution::Dismissed,
        ts: ctx.now,
        actor: ACTOR_HUMAN.to_string(),
        detail: Some(BASELINE_REASON.to_string()),
        case: None,
    };
    let mut resolved = HashSet::new();
    let mut lines = Vec::new();
    let mut items = 0;
    for e in open {
        if resolved.contains(&e.id) {
            continue;
        }
        let sel = reconcile::select(&built, &e.id.to_string(), false)?;
        if sel.members.is_empty() {
            continue;
        }
        resolved.extend(sel.members.iter().map(|m| m.id));
        items += 1;
        lines.extend(reconcile::resolutions(&sel, &resolve));
    }
    let events = lines.len();
    if events > 0 {
        emit(&lock, &config, &logbook, lines)?;
        index::rebuild_if_initialised(ctx);
    }
    drop(lock);
    Ok(Baseline { items, events })
}

// ---------------------------------------------------------------------------
// Theme hook
// ---------------------------------------------------------------------------

/// What the theme hook step did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ThemeHook {
    /// The user did not opt in.
    NotRequested,
    /// `omarchy hook install` ran and succeeded.
    Installed { script: PathBuf, hook: PathBuf },
    /// Omarchy's hook directory already has Seldon's hook; nothing ran.
    AlreadyInstalled { hook: PathBuf },
    /// Writing the script or running `omarchy` failed.
    Failed { script: PathBuf, error: String },
}

impl ThemeHook {
    /// The command the user can run by hand after a failure; none when
    /// the script was never written (a held lock, a failed write).
    pub fn fix(&self) -> Option<String> {
        match self {
            ThemeHook::Failed { script, .. } if is_file(script) => Some(format!(
                "omarchy hook install theme-set {}",
                script.display()
            )),
            _ => None,
        }
    }

    pub fn human(&self, dirs: &Dirs) -> Option<String> {
        match self {
            ThemeHook::NotRequested => None,
            ThemeHook::Installed { hook, .. } => {
                Some(format!("installed ({})", dirs.display(hook)))
            }
            ThemeHook::AlreadyInstalled { hook } => {
                Some(format!("already installed ({})", dirs.display(hook)))
            }
            ThemeHook::Failed { error, .. } => Some(format!("not installed: {error}")),
        }
    }

    pub fn json(&self) -> Value {
        match self {
            ThemeHook::NotRequested => json!({ "requested": false, "installed": false }),
            ThemeHook::Installed { script, hook } => json!({
                "requested": true, "installed": true, "script": script, "hook": hook,
            }),
            ThemeHook::AlreadyInstalled { hook } => json!({
                "requested": true, "installed": true, "already": true, "hook": hook,
            }),
            ThemeHook::Failed { script, error } => json!({
                "requested": true, "installed": false, "script": script, "error": error,
                "fix": self.fix(),
            }),
        }
    }
}

/// Where `omarchy hook install theme-set` puts Seldon's hook (it uses
/// `$HOME/.config`, whatever `XDG_CONFIG_HOME` says).
pub fn theme_hook_target(dirs: &Dirs) -> PathBuf {
    dirs.home
        .join(".config/omarchy/hooks/theme-set.d")
        .join(THEME_HOOK_NAME)
}

/// The command [`theme_hook_step`] records as the installer.
pub const INSTALL_THEME_HOOK: &str = "seldon init --theme-hook";

/// The theme hook step of `init --theme-hook`: takes the state lock
/// before anything is written, installs the hook ([`install_theme_hook`])
/// and records the installed copy as the engine's own write (SPEC-ENGINE
/// §5 rule 7), then lets the lock go. No capture can see the copy before
/// its record, so the next capture's `config-add` is explained. While
/// another `seldon` holds the lock, nothing is written and the step fails
/// without a fix (the next capture finds theme switches anyway). The
/// [`OwnRecord`] is `Some` only after an install.
pub fn theme_hook_step(
    dirs: &Dirs,
    config: &Config,
    omarchy: &str,
) -> (ThemeHook, Option<OwnRecord>) {
    let lock = match crate::logbook::lock::acquire(&dirs.lock_file()) {
        Ok(lock) => lock,
        Err(e) => {
            return (
                ThemeHook::Failed {
                    script: theme_hook_script(dirs),
                    error: format!(
                        "{e}; nothing was written (the next capture still records theme switches)"
                    ),
                },
                None,
            );
        }
    };
    let theme_hook = install_theme_hook(&lock, dirs, omarchy);
    let own = match &theme_hook {
        ThemeHook::Installed { hook, .. } => Some(
            config::record_own_writes(
                &lock,
                dirs,
                config,
                std::slice::from_ref(hook),
                INSTALL_THEME_HOOK,
                OwnOp::Install,
            )
            .map_err(|e| format!("{e:#}")),
        ),
        _ => None,
    };
    drop(lock);
    (theme_hook, own)
}

/// Where `init --theme-hook` writes the script that `omarchy hook install`
/// copies: `<state>/hooks/`.
fn theme_hook_script(dirs: &Dirs) -> PathBuf {
    dirs.state_dir.join("hooks").join(THEME_HOOK_NAME)
}

/// Writes the theme hook script to `<state>/hooks/` and runs
/// `<omarchy> hook install theme-set <script>` once, unless Omarchy's hook
/// directory has it already. The caller holds the state lock until the
/// installed copy is recorded ([`theme_hook_step`]).
pub fn install_theme_hook(_lock: &Lock, dirs: &Dirs, omarchy: &str) -> ThemeHook {
    let hook = theme_hook_target(dirs);
    if hook.exists() {
        return ThemeHook::AlreadyInstalled { hook };
    }
    let script = theme_hook_script(dirs);
    let failed = |error: String| ThemeHook::Failed {
        script: script.clone(),
        error,
    };
    if let Err(e) = write_script(&script) {
        return failed(format!("{e:#}"));
    }
    let Some(arg) = script.to_str() else {
        return failed(format!("{} is not UTF-8", script.display()));
    };
    match sys::run(
        omarchy,
        &["hook", "install", "theme-set", arg],
        None,
        OMARCHY_TIMEOUT,
    ) {
        Run::Exited { code: Some(0), .. } => ThemeHook::Installed { script, hook },
        Run::Exited {
            code,
            stdout,
            stderr,
        } => {
            let said = [stderr.trim(), stdout.trim()]
                .into_iter()
                .find(|s| !s.is_empty())
                .unwrap_or("no output");
            failed(format!(
                "`omarchy hook install` exited with {}: {said}",
                code.map_or("a signal".into(), |c| c.to_string())
            ))
        }
        Run::NotFound => failed(format!("`{omarchy}` is not installed")),
        Run::TimedOut => failed("`omarchy hook install` timed out".into()),
        Run::Failed(e) => failed(format!("cannot run `{omarchy}`: {e}")),
    }
}

/// The command [`remove_theme_hook`] records as the remover.
pub const REMOVE_THEME_HOOK: &str = "seldon init --remove-theme-hook";

/// What [`remove_theme_hook`] did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeHookRemoval {
    /// Omarchy's copy in `~/.config/omarchy/hooks/theme-set.d/`.
    pub hook: PathBuf,
    /// The script in the state directory that `omarchy hook install` copied.
    pub script: PathBuf,
    pub hook_removed: bool,
    pub script_removed: bool,
    /// The record of the hook's deletion (`None`: nothing deleted).
    pub own: Option<OwnRecord>,
}

/// `seldon init --remove-theme-hook`: deletes what `init --theme-hook`
/// installed, Omarchy's copy of the hook ([`theme_hook_target`], a name of
/// Seldon's own) and the script in `<state>/hooks/`, and records the
/// hook's deletion as the engine's own ([`delete_own_file`]). Directories
/// stay. Nothing installed, nothing changed.
pub fn remove_theme_hook(ctx: &Context, config: &Config) -> Result<ThemeHookRemoval> {
    let hook = theme_hook_target(&ctx.dirs);
    let script = theme_hook_script(&ctx.dirs);
    let own = if is_file(&hook) {
        Some(delete_own_file(ctx, config, &hook, REMOVE_THEME_HOOK)?)
    } else {
        None
    };
    let script_removed = is_file(&script);
    if script_removed {
        std::fs::remove_file(&script)
            .with_context(|| format!("cannot remove {}", ctx.dirs.display(&script)))?;
    }
    Ok(ThemeHookRemoval {
        hook,
        script,
        hook_removed: own.is_some(),
        script_removed,
        own,
    })
}

/// A regular file (or a symbolic link to one) at `path`.
fn is_file(path: &Path) -> bool {
    std::fs::metadata(path).is_ok_and(|m| m.is_file())
}

fn write_script(path: &Path) -> anyhow::Result<()> {
    sys::write_atomic_mode(path, THEME_HOOK_SCRIPT.as_bytes(), 0o755)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn since_takes_a_date_or_rfc_3339() {
        let t = parse_since("2026-09-01T10:00:00+02:00").unwrap();
        assert_eq!(t.to_rfc3339(), "2026-09-01T10:00:00+02:00");
        let d = parse_since("2026-09-01").unwrap();
        assert_eq!(d.date_naive(), NaiveDate::from_ymd_opt(2026, 9, 1).unwrap());
        assert_eq!(d.time(), chrono::NaiveTime::MIN);
        for bad in ["", "yesterday", "2026-13-01", "01.09.2026"] {
            assert!(parse_since(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn the_theme_hook_is_the_shipped_script() {
        assert!(THEME_HOOK_SCRIPT.starts_with("#!/bin/bash\n"));
        assert!(THEME_HOOK_SCRIPT.contains("seldon event theme theme-set --subject \"$1\""));
    }

    #[test]
    fn the_theme_hook_step_writes_nothing_while_the_lock_is_held() {
        // the lock comes before the script, `omarchy` and the record: with
        // it held elsewhere the step fails at once, without a fix
        let tmp = std::env::temp_dir().join(format!("seldon-theme-hook-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let home = tmp.join("home");
        let dirs = Dirs {
            xdg_config_home: home.join(".config"),
            state_dir: home.join(".local/state/seldon"),
            home,
        };
        let held = crate::logbook::lock::acquire(&dirs.lock_file()).unwrap();
        // a program that is not there: the step must not get as far as it
        let omarchy = tmp.join("no-omarchy");
        let (step, own) = theme_hook_step(&dirs, &Config::default(), omarchy.to_str().unwrap());
        assert!(!theme_hook_script(&dirs).exists(), "the script was written");
        assert!(!theme_hook_target(&dirs).exists());
        assert!(!dirs.state_dir.join("owned.json").exists());
        match &step {
            ThemeHook::Failed { error, .. } => {
                assert!(error.contains("holds the lock"), "{error}");
                assert!(error.contains("; nothing was written"), "{error}");
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(own, None);
        assert_eq!(
            step.fix(),
            None,
            "no command for a script that is not there"
        );
        drop(held);
        std::fs::remove_dir_all(&tmp).unwrap();
    }

    #[test]
    fn copy_tree_keeps_existing_files_and_modes() {
        use std::os::unix::fs::PermissionsExt as _;
        let tmp = std::env::temp_dir().join(format!("seldon-copy-tree-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let (from, to) = (tmp.join("kit"), tmp.join("logbook/.claude"));
        std::fs::create_dir_all(from.join("hooks")).unwrap();
        std::fs::create_dir_all(from.join("skills/zones")).unwrap();
        std::fs::create_dir_all(&to).unwrap();
        std::fs::write(from.join("hooks/guard.py"), "guard").unwrap();
        std::fs::set_permissions(
            from.join("hooks/guard.py"),
            std::fs::Permissions::from_mode(0o755),
        )
        .unwrap();
        std::fs::write(from.join("skills/zones/SKILL.md"), "skill").unwrap();
        std::fs::write(from.join("settings.json"), "{\"kit\":true}").unwrap();
        std::os::unix::fs::symlink("/etc/hostname", from.join("link")).unwrap();
        std::fs::write(to.join("settings.json"), "{}").unwrap();

        let c = copy_tree(&from, &to).unwrap();
        assert_eq!(c.copied, ["hooks/guard.py", "skills/zones/SKILL.md"]);
        assert_eq!(c.kept, ["settings.json"]);
        assert_eq!(
            std::fs::read_to_string(to.join("settings.json")).unwrap(),
            "{}"
        );
        assert!(!to.join("link").exists());
        let mode = std::fs::metadata(to.join("hooks/guard.py"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o111, 0o111);
        // a second run copies nothing
        let again = copy_tree(&from, &to).unwrap();
        assert!(again.copied.is_empty());
        std::fs::remove_dir_all(&tmp).unwrap();
    }
}
