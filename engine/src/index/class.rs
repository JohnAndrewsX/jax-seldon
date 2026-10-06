//! The class of a drift-eligible event (ADR-0028 §2): routine, attention
//! or crisis, computed at index time from the event, the ledger around it
//! and `config.toml [drift]`. Nothing here is written to the ledger.
//!
//! The tests of ADR-0028 §1 in table form:
//!
//! - **crisis** (harm test): a named transaction on an `alwaysRed`
//!   package, a write to a persistence path (`alwaysRedPaths`);
//! - **attention** (reason test): a package installed, removed or
//!   downgraded by name, a third-party plugin added, removed or updated,
//!   an override under a watched path, and every event no row names;
//! - **routine**: a plain full upgrade, an upgrade of what is installed,
//!   the keyrings, Omarchy's own update, a plugin toggle, a theme switch,
//!   Omarchy's own copy of a file (`meta.matches`), a link into `/usr/`,
//!   the `routinePaths`, a theme's assets.
//!
//! The caller groups: a pacman transaction's class is the highest class of
//! its members, and a dependency of a named transaction
//! ([`Verdict::follows`]) takes the highest class of the transaction's
//! explicit members. `attention = "all"` bypasses this module (the
//! derivation before ADR-0028, `build::drift_items`).

use std::cmp::Reverse;
use std::collections::{HashMap, HashSet};
use std::path::Path;

use chrono::{DateTime, Duration, FixedOffset};
use ulid::Ulid;

use super::drift::AlwaysRed;
use crate::collectors::config::SkipPaths;
use crate::config::{AttentionMode, DriftConfig};
use crate::model::event::{Event, Kind, Source};
use crate::pkgcmd::{PacmanCommand, parse_command, split_logged};

/// The `meta` key of the capture-time evidence marks (ADR-0028 §5).
pub const MATCHES_KEY: &str = "matches";
/// `meta.matches`: the new content equals Omarchy's shipped copy.
pub const MATCHES_OMARCHY_DEFAULT: &str = "omarchy-default";
/// `meta.matches`: the subject is a symlink resolving under `/usr/`.
pub const MATCHES_SYSTEM_LINK: &str = "system-link";
/// `meta.matches`: a file of a theme directory with a `.git` directory
/// (installed by `omarchy theme install`, which strips its code).
pub const MATCHES_THEME_REPO: &str = "theme-repo";

/// How far before an omarchy `update` the package upgrade that explains it
/// may lie (as the omarchy collector's attribution, `omarchy.rs`).
const OMARCHY_LOOKBACK: Duration = Duration::days(31);

/// Where a theme lives and where its extra backgrounds go (Omarchy's
/// `theming.md`).
const THEMES_DIR: &str = "~/.config/omarchy/themes/";
const BACKGROUNDS_DIR: &str = "~/.config/omarchy/backgrounds/";
/// Omarchy's hook directory; `omarchy-hook` skips `*.sample`.
const HOOKS_DIR: &str = "~/.config/omarchy/hooks/";
/// Theme files that run code: what `omarchy theme install` strips.
const THEME_CODE: [&str; 5] = [
    "alacritty.toml",
    "foot.ini",
    "ghostty.conf",
    "kitty.conf",
    "vscode.json",
];

/// The three classes, ordered by the attention they get.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Class {
    Routine,
    Attention,
    Crisis,
}

impl Class {
    pub fn as_str(self) -> &'static str {
        match self {
            Class::Routine => "routine",
            Class::Attention => "attention",
            Class::Crisis => "crisis",
        }
    }
}

/// A class and the row of the table that gave it (`drift show`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Verdict {
    pub class: Class,
    pub rule: &'static str,
}

impl Verdict {
    const fn new(class: Class, rule: &'static str) -> Self {
        Verdict { class, rule }
    }

    fn attention(rule: &'static str) -> Option<Self> {
        Some(Verdict::new(Class::Attention, rule))
    }

    fn crisis(rule: &'static str) -> Option<Self> {
        Some(Verdict::new(Class::Crisis, rule))
    }

    /// The marker of a dependency of a named transaction: it follows the
    /// transaction's explicit members (ADR-0013 grouping, rule 2).
    pub const FOLLOWS: Option<Verdict> = None;

    /// The rows every pacman member of a transaction without an explicit
    /// member falls to.
    pub const ORPHAN: Verdict = Verdict::new(Class::Attention, "other");
}

/// `[drift]`, compiled for one index build.
#[derive(Debug)]
pub struct Rules {
    pub attention: AttentionMode,
    pub always_red: AlwaysRed,
    routine: HashSet<String>,
    routine_paths: SkipPaths,
    always_red_paths: SkipPaths,
    routine_packages: HashSet<String>,
}

/// The home directory as the path globs see a `~`-path subject: `~/x` is
/// matched as `/~/x` (a real `/~` directory would be misread; none exists).
const HOME_KEY: &str = "/~";

/// A config subject as the path globs read it.
fn glob_path(subject: &str) -> String {
    match subject.strip_prefix('~') {
        Some(rest) if rest.is_empty() || rest.starts_with('/') => format!("{HOME_KEY}{rest}"),
        _ => subject.to_string(),
    }
}

impl Rules {
    pub fn new(config: &DriftConfig) -> Self {
        let home = Path::new(HOME_KEY);
        Rules {
            attention: config.attention,
            always_red: AlwaysRed::new(&config.always_red),
            routine: config.routine.iter().cloned().collect(),
            routine_paths: SkipPaths::new(home, &config.routine_paths),
            always_red_paths: SkipPaths::new(home, &config.always_red_paths),
            routine_packages: config.routine_packages.iter().cloned().collect(),
        }
    }

    /// Whether the routine rule `id` applies (`[drift] routine`). A rule
    /// that does not apply is skipped: the event falls to the next row.
    fn on(&self, id: &str) -> bool {
        self.routine.contains(id)
    }

    fn routine(&self, id: &'static str) -> Option<Verdict> {
        self.on(id).then_some(Verdict::new(Class::Routine, id))
    }

    /// The verdict of one event, [`Verdict::FOLLOWS`] for a dependency of
    /// a named pacman transaction. `cmd` is the parsed `meta.command` of a
    /// pacman event (shared by its transaction; the caller caches it).
    pub fn event(
        &self,
        e: &Event,
        cmd: Option<&PacmanCommand>,
        history: &History,
    ) -> Option<Verdict> {
        match e.source {
            Source::Pacman => self.pacman(e, cmd),
            Source::Omarchy => Some(self.omarchy(e, history)),
            Source::Plugins => Some(self.plugins(e)),
            Source::Theme => Some(self.theme(e)),
            Source::Config => Some(self.config(e)),
            _ => Verdict::attention("other"),
        }
    }

    fn pacman(&self, e: &Event, cmd: Option<&PacmanCommand>) -> Option<Verdict> {
        let red = self.always_red.matches(&e.subject);
        if cmd.is_some_and(PacmanCommand::is_plain_full_upgrade) {
            // a distro-driven upgrade, `alwaysRed` included (ADR-0028 §2
            // row 1); removals are pacman's `:: Replace`
            let routine = match e.kind {
                Kind::Upgrade | Kind::Reinstall | Kind::Install => true,
                Kind::Remove => !red,
                _ => false,
            };
            if routine && let Some(v) = self.routine("sysupgrade") {
                return Some(v);
            }
            if red && matches!(e.kind, Kind::Downgrade | Kind::Remove) {
                return Verdict::attention("sysupgrade-red");
            }
            if e.kind == Kind::Downgrade {
                return Verdict::attention("downgrade");
            }
            // every member is a dependency: none is named
            return Some(Verdict::ORPHAN);
        }
        if let Some(c) = cmd
            && !c.targets.is_empty()
            && c.targets.iter().all(|t| self.routine_packages.contains(t))
            && let Some(v) = self.routine("keyring")
        {
            return Some(v);
        }
        match e.explicit {
            Some(false) => Verdict::FOLLOWS,
            Some(true) => match (e.kind, red) {
                (Kind::Upgrade | Kind::Reinstall, false) => {
                    self.routine("upgrade").or(Verdict::attention("upgrade"))
                }
                (Kind::Upgrade | Kind::Reinstall, true) => Verdict::attention("upgrade-red"),
                (Kind::Install | Kind::Remove | Kind::Downgrade, true) => {
                    Verdict::crisis("always-red")
                }
                (Kind::Install | Kind::Remove | Kind::Downgrade, false) => {
                    Verdict::attention("package")
                }
                _ => Verdict::attention("other"),
            },
            // no command line: nothing says what was named
            None => Verdict::attention("other"),
        }
    }

    fn omarchy(&self, e: &Event, history: &History) -> Verdict {
        let shaped = |v: Option<&String>| v.is_some_and(|v| package_shaped(v));
        if e.kind == Kind::Update
            && shaped(e.meta.from.as_ref())
            && let Some(to) = e.meta.to.as_deref()
            && shaped(e.meta.to.as_ref())
            && history.omarchy_upgraded_to(to, e.ts)
            && let Some(v) = self.routine("omarchy-update")
        {
            return v;
        }
        Verdict::new(Class::Attention, "omarchy-other")
    }

    fn plugins(&self, e: &Event) -> Verdict {
        if matches!(e.kind, Kind::PluginEnable | Kind::PluginDisable)
            && let Some(v) = self.routine("plugin-toggle")
        {
            return v;
        }
        Verdict::new(Class::Attention, "plugin")
    }

    fn theme(&self, e: &Event) -> Verdict {
        if e.kind == Kind::ThemeSet
            && let Some(v) = self.routine("theme")
        {
            return v;
        }
        Verdict::new(Class::Attention, "other")
    }

    fn config(&self, e: &Event) -> Verdict {
        let mark = e.meta.extra.get(MATCHES_KEY).and_then(|v| v.as_str());
        let removed = e.kind == Kind::ConfigRemove;
        if !removed {
            for id in [MATCHES_OMARCHY_DEFAULT, MATCHES_SYSTEM_LINK] {
                if mark == Some(id)
                    && let Some(v) = self.routine(id)
                {
                    return v;
                }
            }
        }
        let path = glob_path(&e.subject);
        if self.routine_paths.matches(Path::new(&path))
            && let Some(v) = self.routine("routine-paths")
        {
            return v;
        }
        if removed {
            return Verdict::new(Class::Attention, "config-remove");
        }
        if self.always_red_paths.matches(Path::new(&path)) && !inert_hook(&e.subject) {
            return Verdict::new(Class::Crisis, "always-red-paths");
        }
        if let Some(rest) = e.subject.strip_prefix(THEMES_DIR)
            && rest.contains('/')
        {
            if mark == Some(MATCHES_THEME_REPO)
                && let Some(v) = self.routine("theme-repo")
            {
                return v;
            }
            let name = rest.rsplit('/').next().unwrap_or(rest);
            if !name.ends_with(".lua")
                && !THEME_CODE.contains(&name)
                && let Some(v) = self.routine("theme-assets")
            {
                return v;
            }
        }
        if e.subject.starts_with(BACKGROUNDS_DIR)
            && let Some(v) = self.routine("theme-assets")
        {
            return v;
        }
        Verdict::new(Class::Attention, "config")
    }
}

/// A `*.sample` file in Omarchy's hook directory: `omarchy-hook` never
/// runs it, so it is no persistence (WP-109, Omarchy's `hooks.md`).
fn inert_hook(subject: &str) -> bool {
    subject.starts_with(HOOKS_DIR) && subject.ends_with(".sample")
}

/// A pacman version (`N…-N`: pkgver starting with a digit, `-`, a numeric
/// pkgrel); not `dev` or `dev (3f9c2e1)` of a git checkout.
pub fn package_shaped(v: &str) -> bool {
    let Some((ver, rel)) = v.rsplit_once('-') else {
        return false;
    };
    let rel_ok = !rel.is_empty()
        && rel.split('.').count() <= 2
        && rel
            .split('.')
            .all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()));
    rel_ok
        && ver.starts_with(|c: char| c.is_ascii_digit())
        && !ver.contains(|c: char| c.is_whitespace() || c == '-')
}

/// What the omarchy `update` row needs from the rest of the ledger: the
/// pacman installs and upgrades of `omarchy`/`omarchy-dev` inside a plain
/// full upgrade.
#[derive(Debug, Default)]
pub struct History<'a> {
    omarchy: Vec<(&'a str, DateTime<FixedOffset>)>,
}

impl<'a> History<'a> {
    pub fn new(events: impl IntoIterator<Item = &'a Event>) -> Self {
        let omarchy = events
            .into_iter()
            .filter(|e| {
                e.source == Source::Pacman
                    && matches!(e.subject.as_str(), "omarchy" | "omarchy-dev")
                    && matches!(e.kind, Kind::Install | Kind::Upgrade)
                    && e.meta
                        .command
                        .as_deref()
                        .and_then(|c| parse_command(&split_logged(c)))
                        .is_some_and(|c| c.is_plain_full_upgrade())
            })
            .filter_map(|e| Some((e.version_key()?, e.ts)))
            .collect();
        History { omarchy }
    }

    /// Whether a plain full upgrade moved Omarchy's package to `version`
    /// at most [`OMARCHY_LOOKBACK`] before `at` (and not after it).
    fn omarchy_upgraded_to(&self, version: &str, at: DateTime<FixedOffset>) -> bool {
        self.omarchy
            .iter()
            .any(|&(v, ts)| v == version && ts <= at && ts >= at - OMARCHY_LOOKBACK)
    }
}

/// [`Rules`] with what a verdict needs from the ledger around an event:
/// the omarchy package history and the explicit members of every pacman
/// transaction (a dependency follows them).
#[derive(Debug)]
pub struct Classifier<'a> {
    pub rules: &'a Rules,
    history: History<'a>,
    explicit: HashMap<&'a str, Vec<&'a Event>>,
}

impl<'a> Classifier<'a> {
    /// Over every ledger event (`events`, any order).
    pub fn new(rules: &'a Rules, events: &'a [Event]) -> Self {
        let mut explicit: HashMap<&str, Vec<&Event>> = HashMap::new();
        for e in events {
            if e.source == Source::Pacman
                && e.explicit == Some(true)
                && let Some(tx) = e.tx_id.as_deref()
            {
                explicit.entry(tx).or_default().push(e);
            }
        }
        Classifier {
            rules,
            history: History::new(events),
            explicit,
        }
    }

    /// The verdict of a drift item or series group: one event, or the
    /// members of one pacman transaction (sharing its command line). The
    /// highest class wins; of several members with it, the rule is the
    /// lead's, else the lowest id's.
    pub fn group(&self, members: &[&Event], lead: &Event) -> Verdict {
        let cmd = lead
            .meta
            .command
            .as_deref()
            .filter(|_| lead.source == Source::Pacman)
            .and_then(|c| parse_command(&split_logged(c)));
        let mut followed: Option<Verdict> = None;
        let mut best: Option<(Class, bool, Reverse<Ulid>, Verdict)> = None;
        for m in members {
            let v = match self.rules.event(m, cmd.as_ref(), &self.history) {
                Some(v) => v,
                None => *followed.get_or_insert_with(|| self.follow(m, cmd.as_ref())),
            };
            let key = (v.class, m.id == lead.id, Reverse(m.id));
            if best.as_ref().is_none_or(|b| key > (b.0, b.1, b.2)) {
                best = Some((key.0, key.1, key.2, v));
            }
        }
        best.map_or(Verdict::ORPHAN, |b| b.3)
    }

    /// The highest verdict of the explicit members of `dep`'s transaction.
    fn follow(&self, dep: &Event, cmd: Option<&PacmanCommand>) -> Verdict {
        dep.tx_id
            .as_deref()
            .and_then(|tx| self.explicit.get(tx))
            .into_iter()
            .flatten()
            .filter_map(|x| self.rules.event(x, cmd, &self.history))
            .max_by_key(|v| v.class)
            .unwrap_or(Verdict::ORPHAN)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn package_shaped_versions() {
        for v in ["4.0.4-1", "1:4.0-2", "4.1.0.r12.g3f9c2e1-1", "3.0-1.1"] {
            assert!(package_shaped(v), "{v}");
        }
        for v in [
            "dev",
            "dev (3f9c2e1)",
            "4.0.4",
            "v4.0.4-1",
            "4.0-",
            "-1",
            "4.0-x",
            "4.0-1.2.3",
            "",
        ] {
            assert!(!package_shaped(v), "{v}");
        }
    }

    #[test]
    fn glob_paths_read_the_home_as_a_key() {
        assert_eq!(glob_path("~/.profile"), "/~/.profile");
        assert_eq!(glob_path("~"), "/~");
        assert_eq!(glob_path("/etc/pacman.conf"), "/etc/pacman.conf");
        assert_eq!(glob_path("~x/y"), "~x/y");
    }
}
