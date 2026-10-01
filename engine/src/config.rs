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
    if let Some(p) = config.and_then(|c| c.logbook.as_deref()) {
        return (dirs.expand(&p.to_string_lossy()), LogbookSource::Config);
    }
    (dirs.default_logbook(), LogbookSource::Default)
}

/// `config.toml`. Unknown keys are ignored on read and kept on write.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Config {
    /// Absolute logbook path.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logbook: Option<PathBuf>,
    /// Language of the logbook prose (ADR-0007).
    pub language: Language,
    /// Paths the config collector hashes; `~` is expanded at use.
    pub watch_paths: Vec<String>,
    /// Agent harnesses chosen in the wizard (`claude-code`, `omarchy-agent`).
    pub harnesses: Vec<String>,
    pub collectors: Collectors,
    pub git: GitConfig,
    pub redaction: Redaction,
    pub drift: DriftConfig,
    /// `seldon agent start` (WP-022); not written while it is the default.
    #[serde(skip_serializing_if = "AgentConfig::is_default")]
    pub agent: AgentConfig,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            logbook: None,
            language: Language::default(),
            watch_paths: DEFAULT_WATCH_PATHS.iter().map(|s| s.to_string()).collect(),
            harnesses: Vec::new(),
            collectors: Collectors::default(),
            git: GitConfig::default(),
            redaction: Redaction::default(),
            drift: DriftConfig::default(),
            agent: AgentConfig::default(),
        }
    }
}

/// SPEC-ENGINE §4 (config collector). `~/.config/omarchy/plugins/` is
/// excluded by the collector; missing paths are skipped.
pub const DEFAULT_WATCH_PATHS: [&str; 5] = [
    "~/.config/hypr",
    "~/.config/omarchy",
    "~/.config/waybar",
    "~/.bashrc",
    "~/.zshrc",
];

/// Harnesses the wizard can set up: Claude Code's hooks
/// (`.claude/settings.json`) and the Omarchy-Agent kit's guard and skills
/// (copied into `.claude/` from a template directory, `commands::setup`).
pub const HARNESSES: [&str; 2] = ["claude-code", "omarchy-agent"];

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
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Redaction {
    pub patterns: Vec<String>,
    pub skip_paths: Vec<String>,
}

/// Drift grouping (ADR-0013).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DriftConfig {
    pub always_red: Vec<String>,
}

impl Default for DriftConfig {
    fn default() -> Self {
        DriftConfig {
            always_red: [
                "linux*",
                "systemd",
                "glibc",
                "hyprland",
                "omarchy",
                "quickshell",
            ]
            .map(String::from)
            .to_vec(),
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
    fn defaults_round_trip_through_toml() {
        let config = Config {
            logbook: Some(PathBuf::from("/home/user/Seldon")),
            language: Language::De,
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
        assert_eq!(config.language, Language::De);
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
