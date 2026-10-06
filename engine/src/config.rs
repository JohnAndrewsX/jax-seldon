//! `~/.config/seldon/config.toml` and the directories the engine uses
//! (SPEC-ENGINE §2).
//!
//! Logbook path precedence: `--logbook` > `SELDON_LOGBOOK` > `logbook` in
//! config.toml > `~/Seldon` (ADR-0010).

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

use anyhow::{Context as _, anyhow};
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::model::Language;
use crate::sys;

/// Environment variable that overrides the logbook path.
pub const LOGBOOK_ENV: &str = "SELDON_LOGBOOK";

/// Environment variable naming the directory every engine directory must
/// lie under ([`Dirs::from_vars`]); set by tests and manual test runs.
pub const TEST_GUARD_ENV: &str = "SELDON_TEST_GUARD";

/// `path` made absolute, `.`/`..` folded, and symbolic links resolved as
/// far as the path exists (the rest need not exist yet).
fn resolved(path: &Path) -> PathBuf {
    let abs = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
    let mut out = PathBuf::new();
    for c in abs.components() {
        match c {
            std::path::Component::ParentDir => {
                out.pop();
            }
            std::path::Component::CurDir => {}
            c => out.push(c),
        }
        if let Ok(real) = std::fs::canonicalize(&out) {
            out = real;
        }
    }
    out
}

/// Default logbook directory name under `$HOME` (ADR-0010 option 1).
pub const DEFAULT_LOGBOOK_DIR: &str = "Seldon";

/// Engine directories, resolved once from the environment (XDG base dirs).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dirs {
    pub home: PathBuf,
    /// `$XDG_CONFIG_HOME` or `~/.config` (not the seldon sub-directory).
    pub xdg_config_home: PathBuf,
    /// `$XDG_STATE_HOME/seldon` or `~/.local/state/seldon`.
    pub state_dir: PathBuf,
}

impl Dirs {
    pub fn from_env() -> anyhow::Result<Self> {
        Self::from_vars(|name| std::env::var_os(name))
    }

    /// [`Dirs::from_env`] with the environment as a function (tests).
    ///
    /// With `SELDON_TEST_GUARD=<dir>` set, the resolved home, config and
    /// state directories must all lie under `<dir>` (symbolic links and
    /// `..` resolved), else this fails and every command exits 2 before it
    /// reads or writes anything. Tests and the manual recipes in
    /// docs/TESTING.md set it, so a run whose `HOME` or `XDG_*` override
    /// got lost on the way (WP-024: through `script`) cannot touch the
    /// real `~/.config/seldon` or `~/.local/state/seldon`.
    pub fn from_vars(var: impl Fn(&str) -> Option<std::ffi::OsString>) -> anyhow::Result<Self> {
        let home = var("HOME")
            .filter(|h| !h.is_empty())
            .map(PathBuf::from)
            .ok_or_else(|| anyhow!("HOME is not set"))?;
        let xdg = |name: &str, fallback: &str| {
            var(name)
                .map(PathBuf::from)
                .filter(|p| p.is_absolute())
                .unwrap_or_else(|| home.join(fallback))
        };
        let dirs = Dirs {
            xdg_config_home: xdg("XDG_CONFIG_HOME", ".config"),
            state_dir: xdg("XDG_STATE_HOME", ".local/state").join("seldon"),
            home,
        };
        if let Some(guard) = var(TEST_GUARD_ENV).filter(|g| !g.is_empty()) {
            dirs.check_guard(Path::new(&guard))?;
        }
        Ok(dirs)
    }

    /// Fails unless home, config and state directories lie under `guard`.
    fn check_guard(&self, guard: &Path) -> anyhow::Result<()> {
        let guard = resolved(guard);
        for (what, dir) in [
            ("home", &self.home),
            ("config", &self.xdg_config_home),
            ("state", &self.state_dir),
        ] {
            let dir = resolved(dir);
            if !dir.starts_with(&guard) {
                return Err(anyhow!(
                    "refusing to run outside the test guard: the {what} directory {} is not under {} ({TEST_GUARD_ENV})",
                    dir.display(),
                    guard.display()
                ));
            }
        }
        Ok(())
    }

    pub fn config_dir(&self) -> PathBuf {
        self.xdg_config_home.join("seldon")
    }

    pub fn config_file(&self) -> PathBuf {
        self.config_dir().join("config.toml")
    }

    pub fn index_file(&self) -> PathBuf {
        self.state_dir.join("index.json")
    }

    pub fn lock_file(&self) -> PathBuf {
        self.state_dir.join("lock")
    }

    /// The default logbook, `~/Seldon`.
    pub fn default_logbook(&self) -> PathBuf {
        self.home.join(DEFAULT_LOGBOOK_DIR)
    }

    /// The XDG `DOCUMENTS` directory if one is configured
    /// (`$XDG_DOCUMENTS_DIR`, else `user-dirs.dirs`), else `~/Documents`.
    pub fn documents(&self) -> PathBuf {
        let from_env = std::env::var("XDG_DOCUMENTS_DIR").ok();
        let from_file = || {
            let text = std::fs::read_to_string(self.xdg_config_home.join("user-dirs.dirs")).ok()?;
            text.lines().find_map(|line| {
                let value = line.trim().strip_prefix("XDG_DOCUMENTS_DIR=")?;
                Some(value.trim_matches('"').to_string())
            })
        };
        from_env
            .or_else(from_file)
            .map(|v| self.expand(&v.replacen("$HOME", "~", 1)))
            // user-dirs.dirs sets a disabled directory to $HOME itself
            .filter(|p| p.is_absolute() && *p != self.home)
            .unwrap_or_else(|| self.home.join("Documents"))
    }

    /// Expands a leading `~` and makes the path absolute (against the
    /// current directory).
    pub fn expand(&self, path: &str) -> PathBuf {
        let p = if path == "~" {
            self.home.clone()
        } else if let Some(rest) = path.strip_prefix("~/") {
            self.home.join(rest)
        } else {
            PathBuf::from(path)
        };
        std::path::absolute(&p).unwrap_or(p)
    }

    /// A path value of `config.toml` (`logbook`, `watchPaths`) as an
    /// absolute path: `~`, `$HOME` and `${HOME}` forms, and a relative
    /// value, lie under the home directory, never under the current
    /// directory (the plugin captures from the shell's directory, a hook
    /// from the agent's); `.` and `..` are folded. `None` for an empty
    /// value. The hooks read `watchPaths` the same way
    /// ([`crate::attribution::home_path`]).
    pub fn expand_config(&self, value: &str) -> Option<PathBuf> {
        if value.trim().is_empty() {
            return None;
        }
        let path = crate::attribution::home_path(value, &self.home);
        Some(crate::attribution::normalise(Path::new(&path)))
    }

    /// `path` with the home directory written as `~` (for display only).
    pub fn display(&self, path: &Path) -> String {
        match path.strip_prefix(&self.home) {
            Ok(rest) if rest.as_os_str().is_empty() => "~".to_string(),
            Ok(rest) => format!("~/{}", rest.display()),
            Err(_) => path.display().to_string(),
        }
    }
}

/// Where the logbook path came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogbookSource {
    Flag,
    Env,
    Config,
    Default,
}

impl LogbookSource {
    pub fn as_str(self) -> &'static str {
        match self {
            LogbookSource::Flag => "flag",
            LogbookSource::Env => "env",
            LogbookSource::Config => "config",
            LogbookSource::Default => "default",
        }
    }
}

impl fmt::Display for LogbookSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            LogbookSource::Flag => "--logbook",
            LogbookSource::Env => LOGBOOK_ENV,
            LogbookSource::Config => "config.toml",
            LogbookSource::Default => "default",
        })
    }
}

/// Resolves the logbook path by precedence: flag, env, config, default.
pub fn resolve_logbook(
    dirs: &Dirs,
    flag: Option<&Path>,
    env: Option<&str>,
    config: Option<&Config>,
) -> (PathBuf, LogbookSource) {
    if let Some(p) = flag {
        return (dirs.expand(&p.to_string_lossy()), LogbookSource::Flag);
    }
    if let Some(p) = env.filter(|p| !p.is_empty()) {
        return (dirs.expand(p), LogbookSource::Env);
    }
    if let Some(p) = config
        .and_then(|c| c.logbook.as_deref())
        .and_then(|p| dirs.expand_config(&p.to_string_lossy()))
    {
        return (p, LogbookSource::Config);
    }
    (dirs.default_logbook(), LogbookSource::Default)
}

/// `config.toml`. Unknown keys are ignored on read and kept on write.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Config {
    /// The logbook path; a relative one lies under the home directory
    /// ([`Dirs::expand_config`]).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logbook: Option<PathBuf>,
    /// The language `init` gives a new logbook (ADR-0007); `None` (no key)
    /// = the locale. An existing logbook keeps its own, in
    /// `.seldon/logbook.toml`, which every later command reads.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<Language>,
    /// Paths the config collector hashes; expanded at use
    /// ([`Dirs::expand_config`]).
    pub watch_paths: Vec<String>,
    /// Agent harnesses chosen in the wizard (`claude-code`, `omarchy-agent`,
    /// `skills`).
    pub harnesses: Vec<String>,
    pub collectors: Collectors,
    pub git: GitConfig,
    pub redaction: Redaction,
    pub drift: DriftConfig,
    /// `seldon agent start` (WP-022); not written while it is the default.
    #[serde(skip_serializing_if = "AgentConfig::is_default")]
    pub agent: AgentConfig,
    /// Which agent sessions the hooks serve (SPEC-ENGINE §8); not written
    /// while it is the default.
    #[serde(skip_serializing_if = "HooksConfig::is_default")]
    pub hooks: HooksConfig,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            logbook: None,
            language: None,
            watch_paths: DEFAULT_WATCH_PATHS.iter().map(|s| s.to_string()).collect(),
            harnesses: Vec::new(),
            collectors: Collectors::default(),
            git: GitConfig::default(),
            redaction: Redaction::default(),
            drift: DriftConfig::default(),
            agent: AgentConfig::default(),
            hooks: HooksConfig::default(),
        }
    }
}

/// SPEC-ENGINE §4 (config collector). `~/.config/omarchy/plugins/` and
/// the desktop entries' `mimeinfo.cache` are excluded by the collector;
/// missing paths are skipped. The last six are the persistence paths of
/// ADR-0028 §4d (`[drift] alwaysRedPaths`).
pub const DEFAULT_WATCH_PATHS: [&str; 12] = [
    "~/.config/hypr",
    "~/.config/omarchy",
    "~/.config/waybar",
    "~/.bashrc",
    "~/.zshrc",
    "~/.local/share/applications",
    "~/.config/systemd/user",
    "~/.config/autostart",
    "~/.config/environment.d",
    "~/.config/uwsm",
    "~/.profile",
    "~/.bash_profile",
];

/// The default `watchPaths` of earlier engines: 0.1.0 to 0.1.3, and the
/// unreleased list of WP-089. A config whose list still equals one of
/// them (in any order) gains the current defaults (ADR-0028 §4d).
pub const EARLIER_DEFAULT_WATCH_PATHS: [&[&str]; 2] = [
    &[
        "~/.config/hypr",
        "~/.config/omarchy",
        "~/.config/waybar",
        "~/.bashrc",
        "~/.zshrc",
    ],
    &[
        "~/.config/hypr",
        "~/.config/omarchy",
        "~/.config/waybar",
        "~/.bashrc",
        "~/.zshrc",
        "~/.local/share/applications",
    ],
];

impl Config {
    /// ADR-0028 §4d: when `watchPaths` equals an earlier engine's default
    /// list, appends the current defaults it lacks and returns them; a
    /// list the user changed is never widened (empty result).
    pub fn upgrade_watch_paths(&mut self) -> Vec<String> {
        if !self.has_earlier_default_watch_paths() {
            return Vec::new();
        }
        let added: Vec<String> = DEFAULT_WATCH_PATHS
            .iter()
            .filter(|p| !self.watch_paths.iter().any(|w| w == *p))
            .map(|p| p.to_string())
            .collect();
        self.watch_paths.extend(added.iter().cloned());
        added
    }

    /// Whether `watchPaths` equals an earlier engine's default list, in
    /// any order ([`EARLIER_DEFAULT_WATCH_PATHS`]).
    pub fn has_earlier_default_watch_paths(&self) -> bool {
        let mut have: Vec<&str> = self.watch_paths.iter().map(String::as_str).collect();
        have.sort_unstable();
        have.dedup();
        EARLIER_DEFAULT_WATCH_PATHS.iter().any(|list| {
            let mut list = list.to_vec();
            list.sort_unstable();
            list == have
        })
    }

    /// The current default `watchPaths` a user-changed list lacks (for
    /// `doctor`); empty for a list that holds them all.
    pub fn missing_default_watch_paths(&self) -> Vec<&'static str> {
        DEFAULT_WATCH_PATHS
            .into_iter()
            .filter(|p| !self.watch_paths.iter().any(|w| w == p))
            .collect()
    }
}

/// Harnesses the wizard can set up: Claude Code's hooks
/// (`.claude/settings.json`), the Omarchy-Agent kit's guard and skills
/// (copied into `.claude/` from a template directory, `commands::setup`)
/// and the Seldon agent skill (into the agent skill folders that exist,
/// `commands::skills`).
pub const HARNESSES: [&str; 3] = ["claude-code", "omarchy-agent", "skills"];

/// Collectors on/off, all on by default (SPEC-ENGINE §9).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Collectors {
    pub snapper: bool,
    pub pacman: bool,
    pub omarchy: bool,
    pub plugins: bool,
    pub theme: bool,
    pub config: bool,
}

impl Collectors {
    /// Names in run order (SPEC-ENGINE §4).
    pub const NAMES: [&'static str; 6] =
        ["snapper", "pacman", "omarchy", "plugins", "theme", "config"];

    pub fn get(&self, name: &str) -> Option<bool> {
        Some(match name {
            "snapper" => self.snapper,
            "pacman" => self.pacman,
            "omarchy" => self.omarchy,
            "plugins" => self.plugins,
            "theme" => self.theme,
            "config" => self.config,
            _ => return None,
        })
    }

    /// Enables exactly the named collectors.
    pub fn only(names: &[&str]) -> Self {
        let on = |n: &str| names.contains(&n);
        Collectors {
            snapper: on("snapper"),
            pacman: on("pacman"),
            omarchy: on("omarchy"),
            plugins: on("plugins"),
            theme: on("theme"),
            config: on("config"),
        }
    }
}

impl Default for Collectors {
    fn default() -> Self {
        Collectors::only(&Collectors::NAMES)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct GitConfig {
    /// Commit after every logbook write (SPEC-LOGBOOK §1).
    pub autocommit: bool,
}

impl Default for GitConfig {
    fn default() -> Self {
        GitConfig { autocommit: true }
    }
}

/// Extra redaction (SPEC-ENGINE §7).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Redaction {
    pub patterns: Vec<String>,
    /// Default [`DEFAULT_SKIP_PATHS`], also for an empty list (the
    /// `skipPaths = []` that `init` wrote before WP-069); a list of the
    /// user's own replaces them. `init` writes the list into the file.
    #[serde(deserialize_with = "skip_paths_or_defaults")]
    pub skip_paths: Vec<String>,
}

/// `[redaction] skipPaths` as read: an empty list is the defaults.
fn skip_paths_or_defaults<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> std::result::Result<Vec<String>, D::Error> {
    let paths = Vec::<String>::deserialize(d)?;
    Ok(if paths.is_empty() {
        Redaction::default().skip_paths
    } else {
        paths
    })
}

impl Default for Redaction {
    fn default() -> Self {
        Redaction {
            patterns: Vec::new(),
            skip_paths: DEFAULT_SKIP_PATHS.map(String::from).to_vec(),
        }
    }
}

/// State, history, cache and log files that shell plugins keep in a folder
/// of their own under `~/.config/omarchy/` and rewrite every few minutes:
/// watched, each rewrite would be one more drift item (WP-069). Omarchy's
/// own files there (`shell.json`, `extensions/`, `hooks/`, `themed/`) do
/// not match.
pub const DEFAULT_SKIP_PATHS: [&str; 5] = [
    "~/.config/omarchy/**/history.json",
    "~/.config/omarchy/**/history/",
    "~/.config/omarchy/**/state.json",
    "~/.config/omarchy/**/cache/",
    "~/.config/omarchy/**/*.log",
];

/// The text of `config.toml` with `added` appended to its `watchPaths`
/// array and every other byte as it was (ADR-0028 §4d, WP-109 round 2:
/// the upgrade keeps comments and order). `None` when that cannot be done
/// safely: no single top-level `watchPaths = [ … ]` with at least one
/// string, a nested array, or a result that does not read back as the
/// same file with exactly these paths added.
pub fn with_added_watch_paths(text: &str, added: &[String]) -> Option<String> {
    // the key, once, before the first table header
    let mut key_at = None;
    let mut offset = 0;
    for line in text.split_inclusive('\n') {
        let trimmed = line.trim_start();
        if trimmed.starts_with('[') {
            break;
        }
        if let Some(rest) = trimmed.strip_prefix("watchPaths")
            && rest.trim_start().starts_with('=')
        {
            if key_at.is_some() {
                return None;
            }
            key_at = Some(offset + (line.len() - trimmed.len()));
        }
        offset += line.len();
    }
    let key_at = key_at?;
    let eq = key_at + text[key_at..].find('=')?;
    let open = eq + 1 + text[eq + 1..].find(|c: char| !c.is_whitespace())?;
    if !text[open..].starts_with('[') {
        return None;
    }
    // to the closing `]`, past strings and comments; the end of the last
    // string element is where the new ones go
    let (mut last_end, mut close) = (None, None);
    let mut chars = text[open + 1..].char_indices();
    while let Some((i, c)) = chars.next() {
        let at = open + 1 + i;
        match c {
            '"' | '\'' => {
                let mut escaped = false;
                let mut end = None;
                for (j, d) in chars.by_ref() {
                    if c == '"' && !escaped && d == '\\' {
                        escaped = true;
                        continue;
                    }
                    if d == c && !escaped {
                        end = Some(open + 1 + j + 1);
                        break;
                    }
                    if d == '\n' {
                        return None;
                    }
                    escaped = false;
                }
                last_end = Some(end?);
            }
            '#' => {
                for (_, d) in chars.by_ref() {
                    if d == '\n' {
                        break;
                    }
                }
            }
            '[' => return None,
            ']' => {
                close = Some(at);
                break;
            }
            _ => {}
        }
    }
    close?;
    let at = last_end?;
    let mut insert = String::new();
    for p in added {
        insert.push_str(", ");
        insert.push_str(&toml::Value::String(p.clone()).to_string());
    }
    let new = format!("{}{insert}{}", &text[..at], &text[at..]);
    // it must read back as the same file with exactly these paths added
    let mut old: toml::Table = text.parse().ok()?;
    let mut got: toml::Table = new.parse().ok()?;
    let mut want: Vec<toml::Value> = old.get("watchPaths")?.as_array()?.clone();
    want.extend(added.iter().map(|p| toml::Value::String(p.clone())));
    let got_paths = got.remove("watchPaths")?;
    old.remove("watchPaths");
    (got_paths == toml::Value::Array(want) && got == old).then_some(new)
}

/// `[drift]`: grouping (ADR-0013) and the classification by consequence
/// (ADR-0028 §2, §4c), read at index time. The keys ADR-0028 added are
/// written only when they differ from the default, so a later engine's
/// new defaults reach a config `init` wrote.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DriftConfig {
    pub always_red: Vec<String>,
    /// `normal`: the §2 table; `all`: every drift-eligible event without a
    /// case is open drift, crisis iff red (the derivation before ADR-0028,
    /// its rollback).
    #[serde(skip_serializing_if = "AttentionMode::is_default")]
    pub attention: AttentionMode,
    /// The routine rule ids that apply ([`ROUTINE_RULES`]); drop one to
    /// make its events attention again.
    #[serde(skip_serializing_if = "is_default_routine")]
    pub routine: Vec<String>,
    /// `config-*` subjects that are routine (`skipPaths` glob syntax).
    #[serde(skip_serializing_if = "is_default_routine_paths")]
    pub routine_paths: Vec<String>,
    /// Packages whose explicit transactions are routine (`keyring`).
    #[serde(skip_serializing_if = "is_default_routine_packages")]
    pub routine_packages: Vec<String>,
    /// `config-*` subjects that are crises: the persistence paths
    /// (`skipPaths` glob syntax).
    #[serde(skip_serializing_if = "is_default_always_red_paths")]
    pub always_red_paths: Vec<String>,
}

/// `[drift] attention`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AttentionMode {
    #[default]
    Normal,
    All,
}

impl AttentionMode {
    fn is_default(&self) -> bool {
        *self == AttentionMode::default()
    }

    pub fn as_str(self) -> &'static str {
        match self {
            AttentionMode::Normal => "normal",
            AttentionMode::All => "all",
        }
    }
}

/// The routine rule ids of ADR-0028 §2 (and WP-109's theme rules), the
/// default of `[drift] routine`.
pub const ROUTINE_RULES: [&str; 11] = [
    "sysupgrade",
    "upgrade",
    "keyring",
    "omarchy-update",
    "plugin-toggle",
    "theme",
    "omarchy-default",
    "system-link",
    "routine-paths",
    "theme-assets",
    "theme-repo",
];

/// Default `[drift] routinePaths`: the shell's own state file and backups
/// (`omarchy refresh` writes `<file>.bak.<epoch>`).
pub const DEFAULT_ROUTINE_PATHS: [&str; 2] = ["~/.config/omarchy/shell.json", "**/*.bak.*"];

/// Default `[drift] routinePackages`.
pub const DEFAULT_ROUTINE_PACKAGES: [&str; 2] = ["archlinux-keyring", "omarchy-keyring"];

/// Default `[drift] alwaysRedPaths`: code that runs at login or on events
/// without being configuration (ADR-0028 §2).
pub const DEFAULT_ALWAYS_RED_PATHS: [&str; 7] = [
    "~/.config/systemd/user/**",
    "~/.config/omarchy/hooks/**",
    "~/.config/autostart/**",
    "~/.config/environment.d/**",
    "~/.config/uwsm/**",
    "~/.profile",
    "~/.bash_profile",
];

fn strings(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| s.to_string()).collect()
}

fn is_default_routine(v: &Vec<String>) -> bool {
    *v == strings(&ROUTINE_RULES)
}

fn is_default_routine_paths(v: &Vec<String>) -> bool {
    *v == strings(&DEFAULT_ROUTINE_PATHS)
}

fn is_default_routine_packages(v: &Vec<String>) -> bool {
    *v == strings(&DEFAULT_ROUTINE_PACKAGES)
}

fn is_default_always_red_paths(v: &Vec<String>) -> bool {
    *v == strings(&DEFAULT_ALWAYS_RED_PATHS)
}

impl DriftConfig {
    /// The keys whose value differs from the default (`doctor`, ADR-0028
    /// §6: the config can silence rules, so the change is shown).
    pub fn non_default(&self) -> Vec<&'static str> {
        let d = DriftConfig::default();
        let mut keys = Vec::new();
        if self.always_red != d.always_red {
            keys.push("alwaysRed");
        }
        if self.attention != d.attention {
            keys.push("attention");
        }
        if self.routine != d.routine {
            keys.push("routine");
        }
        if self.routine_paths != d.routine_paths {
            keys.push("routinePaths");
        }
        if self.routine_packages != d.routine_packages {
            keys.push("routinePackages");
        }
        if self.always_red_paths != d.always_red_paths {
            keys.push("alwaysRedPaths");
        }
        keys
    }

    /// Entries of `routine` that name no rule ([`ROUTINE_RULES`]).
    pub fn unknown_routine(&self) -> Vec<&str> {
        self.routine
            .iter()
            .map(String::as_str)
            .filter(|r| !ROUTINE_RULES.contains(r))
            .collect()
    }
}

impl Default for DriftConfig {
    /// `alwaysRed`: the R3 subjects of ADR-0023 as package globs (WP-050):
    /// the kernels (not firmware or headers), systemd, glibc, Hyprland,
    /// Omarchy itself, the shell, the boot loader and initramfs, the login
    /// path (`pam`, `sddm`, `uwsm`); `/etc` through `omarchy-settings`
    /// (Omarchy's `/etc` layer) and `filesystem` (the base `/etc` files).
    /// The rest: ADR-0028 §2.
    fn default() -> Self {
        DriftConfig {
            always_red: [
                "linux",
                "linux-lts",
                "linux-zen",
                "linux-hardened",
                "linux-rt",
                "linux-rt-lts",
                "linux-omarchy",
                "systemd",
                "glibc",
                "hyprland",
                "omarchy",
                "omarchy-settings",
                "quickshell",
                "limine*",
                "grub",
                "mkinitcpio*",
                "filesystem",
                "pam",
                "sddm",
                "uwsm",
            ]
            .map(String::from)
            .to_vec(),
            attention: AttentionMode::Normal,
            routine: strings(&ROUTINE_RULES),
            routine_paths: strings(&DEFAULT_ROUTINE_PATHS),
            routine_packages: strings(&DEFAULT_ROUTINE_PACKAGES),
            always_red_paths: strings(&DEFAULT_ALWAYS_RED_PATHS),
        }
    }
}

/// The agent launcher of `seldon agent start` (WP-022): argv lists, never
/// a shell string; the element `{prompt}` becomes the prompt, as one
/// argument. Validated where it is used (`commands::agent`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AgentConfig {
    /// The launcher `agent start` runs without `--launcher`.
    pub launcher: Vec<String>,
    /// More launchers, by name, for `agent start --launcher NAME`.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub launchers: BTreeMap<String, Vec<String>>,
}

/// `omarchy agent prompt <prompt>`: Omarchy's default coding agent in a
/// terminal window of its own (no `--inline`: that runs the agent in the
/// caller's terminal, and a detached launch has none).
pub const DEFAULT_AGENT_LAUNCHER: [&str; 4] = ["omarchy", "agent", "prompt", "{prompt}"];

impl Default for AgentConfig {
    fn default() -> Self {
        AgentConfig {
            launcher: DEFAULT_AGENT_LAUNCHER.map(String::from).to_vec(),
            launchers: BTreeMap::new(),
        }
    }
}

impl AgentConfig {
    fn is_default(&self) -> bool {
        *self == AgentConfig::default()
    }
}

/// `[hooks]` (SPEC-ENGINE §8).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct HooksConfig {
    pub scope: HookScope,
}

impl HooksConfig {
    fn is_default(&self) -> bool {
        *self == HooksConfig::default()
    }
}

/// The sessions whose commands the agent hooks record and which get the
/// logbook context: the session's directory is the `cwd` of the hook's
/// payload.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HookScope {
    /// Sessions whose directory lies inside the logbook.
    #[default]
    Logbook,
    /// Every session that runs the hooks, wherever it works.
    All,
}

const CONFIG_HEADER: &str = "# Seldon engine configuration (docs/SPEC-ENGINE.md §2). Edit freely;\n# `seldon init` rewrites this file and drops comments.\n\n";

impl Config {
    /// Reads `path`; `Ok(None)` if it does not exist. A malformed file is a
    /// user error (exit 1) naming the file.
    pub fn load(path: &Path) -> Result<Option<Config>> {
        let text = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => {
                return Err(anyhow::Error::new(e)
                    .context(format!("cannot read {}", path.display()))
                    .into());
            }
        };
        toml::from_str(&text)
            .map(Some)
            .map_err(|e| Error::user(format!("{}: {}", path.display(), e.message())))
    }

    /// Writes the config atomically. Keys this engine does not know (from a
    /// newer engine or the user) are carried over from the existing file.
    pub fn save(&self, path: &Path) -> anyhow::Result<()> {
        let mut table = toml::Table::try_from(self).context("cannot serialise config")?;
        if let Ok(old) = std::fs::read_to_string(path)
            && let Ok(old) = old.parse::<toml::Table>()
        {
            merge_unknown(&mut table, old);
        }
        let text = format!("{CONFIG_HEADER}{}", toml::to_string(&table)?);
        sys::write_atomic(path, text.as_bytes())
    }
}

/// Copies keys from `old` that `new` does not have, one table level deep.
fn merge_unknown(new: &mut toml::Table, old: toml::Table) {
    for (key, old_value) in old {
        match (new.get_mut(&key), old_value) {
            (None, v) => {
                new.insert(key, v);
            }
            (Some(toml::Value::Table(n)), toml::Value::Table(o)) => {
                for (k, v) in o {
                    n.entry(k).or_insert(v);
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `Dirs::from_vars` over a fixed environment.
    fn dirs_with(vars: &[(&str, &Path)]) -> anyhow::Result<Dirs> {
        let vars: Vec<(String, std::ffi::OsString)> = vars
            .iter()
            .map(|(k, v)| (k.to_string(), v.as_os_str().to_os_string()))
            .collect();
        Dirs::from_vars(|name| vars.iter().find(|(k, _)| k == name).map(|(_, v)| v.clone()))
    }

    #[test]
    fn the_test_guard_checks_the_resolved_dirs() {
        let tmp = std::env::temp_dir().join(format!("seldon-guard-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let (guard, home, outside) = (tmp.join("guard"), tmp.join("guard/home"), tmp.join("out"));
        std::fs::create_dir_all(&home).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        let refused = |r: anyhow::Result<Dirs>| {
            let e = r.expect_err("outside the guard").to_string();
            assert!(e.contains("refusing to run outside the test guard"), "{e}");
        };

        // without the variable nothing is checked
        assert!(dirs_with(&[("HOME", &outside)]).is_ok());
        // home under the guard; XDG dirs default under home
        let d = dirs_with(&[("HOME", &home), (TEST_GUARD_ENV, &guard)]).unwrap();
        assert_eq!(d.state_dir, home.join(".local/state/seldon"));
        // the real-home case: HOME lost, XDG still redirected
        refused(dirs_with(&[
            ("HOME", &outside),
            ("XDG_CONFIG_HOME", &guard.join("config")),
            ("XDG_STATE_HOME", &guard.join("state")),
            (TEST_GUARD_ENV, &guard),
        ]));
        // home inside, one XDG dir outside
        refused(dirs_with(&[
            ("HOME", &home),
            ("XDG_STATE_HOME", &outside),
            (TEST_GUARD_ENV, &guard),
        ]));
        // `..` and a symbolic link out of the guard are resolved, not trusted
        refused(dirs_with(&[
            ("HOME", &guard.join("home/../../out")),
            (TEST_GUARD_ENV, &guard),
        ]));
        std::os::unix::fs::symlink(&outside, guard.join("escape")).unwrap();
        refused(dirs_with(&[
            ("HOME", &guard.join("escape")),
            (TEST_GUARD_ENV, &guard),
        ]));
        // a guard given through a link still contains what lies under it
        std::os::unix::fs::symlink(&guard, tmp.join("guard-link")).unwrap();
        assert!(dirs_with(&[("HOME", &home), (TEST_GUARD_ENV, &tmp.join("guard-link"))]).is_ok());
        std::fs::remove_dir_all(&tmp).unwrap();
    }

    fn dirs() -> Dirs {
        Dirs {
            home: PathBuf::from("/home/user"),
            xdg_config_home: PathBuf::from("/home/user/.config"),
            state_dir: PathBuf::from("/home/user/.local/state/seldon"),
        }
    }

    #[test]
    fn precedence() {
        let d = dirs();
        let config = Config {
            logbook: Some(PathBuf::from("/from/config")),
            ..Config::default()
        };
        let flag = PathBuf::from("/from/flag");
        let r = |f, e, c| resolve_logbook(&d, f, e, c);
        assert_eq!(
            r(Some(&flag), Some("/from/env"), Some(&config)),
            (flag.clone(), LogbookSource::Flag)
        );
        assert_eq!(
            r(None, Some("/from/env"), Some(&config)),
            (PathBuf::from("/from/env"), LogbookSource::Env)
        );
        assert_eq!(
            r(None, Some(""), Some(&config)),
            (PathBuf::from("/from/config"), LogbookSource::Config)
        );
        assert_eq!(
            r(None, None, None),
            (PathBuf::from("/home/user/Seldon"), LogbookSource::Default)
        );
        assert_eq!(
            r(None, Some("~/lb"), None).0,
            PathBuf::from("/home/user/lb")
        );
    }

    #[test]
    fn config_paths_resolve_under_home_whatever_the_cwd() {
        let d = dirs();
        let e = |v: &str| d.expand_config(v);
        let home = |rel: &str| Some(PathBuf::from("/home/user").join(rel));
        assert_eq!(e("dotfiles"), home("dotfiles"));
        assert_eq!(e("./dotfiles/../.config/nvim"), home(".config/nvim"));
        assert_eq!(e("~/.config/nvim"), home(".config/nvim"));
        assert_eq!(e("$HOME/.config/nvim"), home(".config/nvim"));
        assert_eq!(e("${HOME}/.bashrc"), home(".bashrc"));
        assert_eq!(e("~"), Some(PathBuf::from("/home/user")));
        assert_eq!(e("$HOME"), Some(PathBuf::from("/home/user")));
        assert_eq!(
            e("/etc/pacman.conf"),
            Some(PathBuf::from("/etc/pacman.conf"))
        );
        assert_eq!(
            e(""),
            None,
            "an empty value is no path, not the home folder"
        );
        assert_eq!(e("  "), None);
        // the logbook key: relative to home; the flag and the variable are
        // typed in a shell and stay relative to its directory
        let config = Config {
            logbook: Some(PathBuf::from("Logbooks/seldon")),
            ..Config::default()
        };
        assert_eq!(
            resolve_logbook(&d, None, None, Some(&config)),
            (
                PathBuf::from("/home/user/Logbooks/seldon"),
                LogbookSource::Config
            )
        );
        let cwd = std::env::current_dir().unwrap();
        assert_eq!(
            resolve_logbook(&d, None, Some("lb"), Some(&config)).0,
            cwd.join("lb")
        );
        // an empty logbook value is the default
        let empty = Config {
            logbook: Some(PathBuf::new()),
            ..Config::default()
        };
        assert_eq!(
            resolve_logbook(&d, None, None, Some(&empty)),
            (PathBuf::from("/home/user/Seldon"), LogbookSource::Default)
        );
    }

    #[test]
    fn default_skip_paths_skip_plugin_state_not_omarchy_config() {
        use crate::collectors::config::SkipPaths;
        let skip = SkipPaths::new(
            Path::new("/home/user"),
            &Config::default().redaction.skip_paths,
        );
        let m = |rel: &str| skip.matches(&Path::new("/home/user").join(rel));
        // made-up plugin data under a folder of its own
        assert!(m(".config/omarchy/example-timer/history.json"));
        assert!(m(".config/omarchy/example-timer/state.json"));
        assert!(m(".config/omarchy/example-timer/data/state.json"));
        assert!(m(".config/omarchy/example-timer/history/2026-10.json"));
        assert!(m(".config/omarchy/example-timer/cache"));
        assert!(m(".config/omarchy/example-timer/debug.log"));
        // Omarchy's own config and other folders stay watched
        for rel in [
            ".config/omarchy/shell.json",
            ".config/omarchy/history.json",
            ".config/omarchy/extensions/omarchy-menu.jsonc",
            ".config/omarchy/hooks/theme-set.d/seldon-theme-set.sh",
            ".config/omarchy/example-timer/settings.json",
            ".config/hypr/state.json",
        ] {
            assert!(!m(rel), "{rel} is skipped");
        }
        // written into a new file; an empty list (what `init` wrote before)
        // and a missing key are the defaults; a list of one's own replaces them
        let text = toml::to_string(&Config::default()).unwrap();
        assert!(
            text.contains("skipPaths = [\"~/.config/omarchy/**/history.json\""),
            "{text}"
        );
        let read = |text: &str| toml::from_str::<Config>(text).unwrap().redaction.skip_paths;
        assert_eq!(read("[redaction]\nskipPaths = []\n"), DEFAULT_SKIP_PATHS);
        assert_eq!(read("[redaction]\npatterns = []\n"), DEFAULT_SKIP_PATHS);
        assert_eq!(read(""), DEFAULT_SKIP_PATHS);
        assert_eq!(read("[redaction]\nskipPaths = [\"*.key\"]\n"), ["*.key"]);
    }

    #[test]
    fn defaults_round_trip_through_toml() {
        let config = Config {
            logbook: Some(PathBuf::from("/home/user/Seldon")),
            language: Some(Language::De),
            ..Config::default()
        };
        let text = toml::to_string(&config).unwrap();
        assert!(text.contains("watchPaths"));
        assert!(text.contains("[drift]\nalwaysRed"));
        assert!(text.contains("[git]\nautocommit = true"));
        assert_eq!(toml::from_str::<Config>(&text).unwrap(), config);
        // an empty file is all defaults
        assert_eq!(toml::from_str::<Config>("").unwrap(), Config::default());
    }

    #[test]
    fn a_missing_language_key_is_no_value() {
        // `init` then takes the locale (SPEC-ENGINE §9), not a default `en`
        let read = |text: &str| toml::from_str::<Config>(text).unwrap().language;
        assert_eq!(read("[redaction]\npatterns = [\"mysecret\"]\n"), None);
        assert_eq!(read("language = \"en\"\n"), Some(Language::En));
        assert!(
            !toml::to_string(&Config::default())
                .unwrap()
                .contains("language")
        );
    }

    #[test]
    fn save_keeps_unknown_keys() {
        let dir = std::env::temp_dir().join(format!("seldon-config-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.toml");
        std::fs::write(
            &path,
            "future = 1\nlanguage = \"de\"\n[git]\nautocommit = false\nsignoff = true\n[agent]\nlauncher = [\"x\"]\n",
        )
        .unwrap();
        let mut config = Config::load(&path).unwrap().unwrap();
        assert_eq!(config.language, Some(Language::De));
        assert!(!config.git.autocommit);
        config.logbook = Some(PathBuf::from("/tmp/lb"));
        config.save(&path).unwrap();
        let table: toml::Table = std::fs::read_to_string(&path).unwrap().parse().unwrap();
        assert_eq!(table["future"].as_integer(), Some(1));
        assert_eq!(table["git"]["signoff"].as_bool(), Some(true));
        assert_eq!(table["agent"]["launcher"][0].as_str(), Some("x"));
        assert_eq!(config.agent.launcher, ["x"]);
        assert_eq!(table["logbook"].as_str(), Some("/tmp/lb"));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// WP-109 round 2 (N5): the watch-path upgrade changes only the
    /// `watchPaths` array; comments, order and every other byte stay.
    #[test]
    fn watch_paths_are_added_by_a_minimal_edit() {
        let add =
            |text: &str| with_added_watch_paths(text, &["~/.profile".into(), "~/a\"b".into()]);
        let text = "# mine\nlogbook = \"/x\" # here\nwatchPaths = [\"~/.config/hypr\", 'lit'] # tail\n\n[git]\nautocommit = false\n";
        assert_eq!(
            add(text).unwrap(),
            "# mine\nlogbook = \"/x\" # here\nwatchPaths = [\"~/.config/hypr\", 'lit', \"~/.profile\", '~/a\"b'] # tail\n\n[git]\nautocommit = false\n"
        );
        // multi-line, a comment after the last element, a trailing comma
        let text =
            "watchPaths = [\n  \"~/.config/hypr\", # one\n  \"~/.zshrc\", # two ] [\n] # done\n";
        assert_eq!(
            add(text).unwrap(),
            "watchPaths = [\n  \"~/.config/hypr\", # one\n  \"~/.zshrc\", \"~/.profile\", '~/a\"b', # two ] [\n] # done\n"
        );
        // refused: no key, the key only in a table, twice, quoted, empty,
        // nested, unclosed
        for text in [
            "logbook = \"/x\"\n",
            "[x]\nwatchPaths = [\"a\"]\n",
            "watchPaths = [\"a\"]\nwatchPaths = [\"b\"]\n",
            "\"watchPaths\" = [\"a\"]\n",
            "watchPaths = []\n",
            "watchPaths = [[\"a\"]]\n",
            "watchPaths = [\"a\"\n",
            "watchPaths = \"a\"\n",
            // the key's text inside a string: only the read-back sees it
            "logbook = \"\"\"\nwatchPaths = [\"a\"]\n\"\"\"\n",
        ] {
            assert_eq!(add(text), None, "{text:?}");
        }
    }

    #[test]
    fn hook_scope() {
        let parse = |text: &str| toml::from_str::<Config>(text).map(|c| c.hooks.scope);
        assert_eq!(parse("").unwrap(), HookScope::Logbook);
        assert_eq!(
            parse("[hooks]\nscope = \"logbook\"\n").unwrap(),
            HookScope::Logbook
        );
        assert_eq!(parse("[hooks]\nscope = \"all\"\n").unwrap(), HookScope::All);
        assert!(parse("[hooks]\nscope = \"everywhere\"\n").is_err());
        // the default is not written; the other value is
        let text = |config: &Config| toml::to_string(config).unwrap();
        let mut config = Config::default();
        assert!(!text(&config).contains("[hooks]"), "{}", text(&config));
        config.hooks.scope = HookScope::All;
        assert!(
            text(&config).contains("[hooks]\nscope = \"all\""),
            "{}",
            text(&config)
        );
    }

    #[test]
    fn malformed_config_is_a_user_error() {
        let dir = std::env::temp_dir().join(format!("seldon-config-bad-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.toml");
        std::fs::write(&path, "language = \"fr\"\n").unwrap();
        let err = Config::load(&path).unwrap_err();
        assert_eq!(err.exit(), crate::error::Exit::UserError);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
