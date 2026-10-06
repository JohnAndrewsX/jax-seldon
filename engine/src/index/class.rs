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
    use crate::model::event::Meta;

    const AT: &str = "2026-10-01T12:00:00+02:00";

    fn at(ts: &str) -> DateTime<FixedOffset> {
        DateTime::parse_from_rfc3339(ts).unwrap()
    }

    fn ev(source: Source, kind: Kind, subject: &str) -> Event {
        Event::new(at(AT), source, kind, subject)
    }

    /// A pacman event of a transaction whose logged command is `cmd`.
    fn pac(kind: Kind, subject: &str, explicit: Option<bool>, cmd: &str) -> Event {
        let mut e = ev(Source::Pacman, kind, subject);
        e.explicit = explicit;
        e.tx_id = Some(format!("tx-{cmd}"));
        e.meta.command = Some(cmd.into());
        e
    }

    fn config(kind: Kind, subject: &str, mark: Option<&str>) -> Event {
        let mut e = ev(Source::Config, kind, subject);
        if let Some(m) = mark {
            e.meta.extra.insert(MATCHES_KEY.into(), m.into());
        }
        e
    }

    fn update(from: &str, to: &str) -> Event {
        let mut e = ev(Source::Omarchy, Kind::Update, "omarchy");
        e.meta = Meta {
            from: Some(from.into()),
            to: Some(to.into()),
            ..Meta::default()
        };
        e
    }

    /// The verdict of `members` (the first is the lead) among `ledger`.
    fn verdict(config: &DriftConfig, ledger: &[Event], members: &[usize]) -> (Class, &'static str) {
        let rules = Rules::new(config);
        let c = Classifier::new(&rules, ledger);
        let m: Vec<&Event> = members.iter().map(|&i| &ledger[i]).collect();
        let v = c.group(&m, m[0]);
        (v.class, v.rule)
    }

    use Class::{Attention as A, Crisis as C, Routine as R};

    /// ADR-0028 §2, every row: single events.
    #[test]
    fn every_row_of_the_table() {
        let omarchy_line = "pacman -Syu --noconfirm --overwrite /usr/share/omarchy/*";
        let cache = "pacman -U /var/cache/pacman/pkg/firefox-143.0.2-1-x86_64.pkg.tar.zst";
        let rows: Vec<(&str, Event, (Class, &str))> = vec![
            // plain full upgrade: every member, alwaysRed included
            (
                "-Syu upgrade",
                pac(Kind::Upgrade, "firefox", Some(false), "pacman -Syu"),
                (R, "sysupgrade"),
            ),
            (
                "-Syu kernel upgrade",
                pac(Kind::Upgrade, "linux", Some(false), "pacman -Syu"),
                (R, "sysupgrade"),
            ),
            (
                "-Syu reinstall",
                pac(Kind::Reinstall, "glibc", Some(false), "pacman -Syu"),
                (R, "sysupgrade"),
            ),
            (
                "-Syu install",
                pac(
                    Kind::Install,
                    "linux-firmware-amdgpu",
                    Some(false),
                    "pacman -Syu",
                ),
                (R, "sysupgrade"),
            ),
            (
                "-Syu :: Replace removal",
                pac(Kind::Remove, "old-pkg", Some(false), "pacman -Syu"),
                (R, "sysupgrade"),
            ),
            (
                "-Syyuu",
                pac(Kind::Upgrade, "systemd", Some(false), "pacman -Syyuu"),
                (R, "sysupgrade"),
            ),
            (
                "-Su",
                pac(Kind::Upgrade, "firefox", Some(false), "pacman -Su"),
                (R, "sysupgrade"),
            ),
            (
                "bare yay",
                pac(Kind::Upgrade, "firefox", Some(false), "yay"),
                (R, "sysupgrade"),
            ),
            (
                "Omarchy's update line",
                pac(Kind::Upgrade, "omarchy", Some(false), omarchy_line),
                (R, "sysupgrade"),
            ),
            (
                "-Syu kernel removal",
                pac(Kind::Remove, "linux-lts", Some(false), "pacman -Syu"),
                (A, "sysupgrade-red"),
            ),
            (
                "-Syu kernel downgrade",
                pac(Kind::Downgrade, "linux", Some(false), "pacman -Syyuu"),
                (A, "sysupgrade-red"),
            ),
            (
                "-Syu downgrade",
                pac(Kind::Downgrade, "firefox", Some(false), "pacman -Syyuu"),
                (A, "downgrade"),
            ),
            // named on the command line or from the cache
            (
                "named upgrade",
                pac(Kind::Upgrade, "firefox", Some(true), "pacman -S firefox"),
                (R, "upgrade"),
            ),
            (
                "named reinstall",
                pac(Kind::Reinstall, "firefox", Some(true), "pacman -S firefox"),
                (R, "upgrade"),
            ),
            (
                "-U from the cache (yay -Sua)",
                pac(Kind::Upgrade, "firefox", Some(true), cache),
                (R, "upgrade"),
            ),
            (
                "named kernel upgrade",
                pac(Kind::Upgrade, "linux", Some(true), "pacman -S linux"),
                (A, "upgrade-red"),
            ),
            (
                "named install",
                pac(Kind::Install, "htop", Some(true), "pacman -S htop"),
                (A, "package"),
            ),
            (
                "named removal",
                pac(Kind::Remove, "htop", Some(true), "pacman -Rns htop"),
                (A, "package"),
            ),
            (
                "named downgrade",
                pac(
                    Kind::Downgrade,
                    "mesa",
                    Some(true),
                    "pacman -U /c/mesa-1:26.1.0-1-x86_64.pkg.tar.zst",
                ),
                (A, "package"),
            ),
            (
                "named kernel install",
                pac(
                    Kind::Install,
                    "linux-zen",
                    Some(true),
                    "pacman -S linux-zen",
                ),
                (C, "always-red"),
            ),
            (
                "named systemd removal",
                pac(Kind::Remove, "systemd", Some(true), "pacman -R systemd"),
                (C, "always-red"),
            ),
            (
                "named boot loader downgrade",
                pac(
                    Kind::Downgrade,
                    "limine",
                    Some(true),
                    "pacman -U /c/limine-9.0-1-x86_64.pkg.tar.zst",
                ),
                (C, "always-red"),
            ),
            (
                "a full upgrade naming a package",
                pac(Kind::Install, "htop", Some(true), "pacman -Syu htop"),
                (A, "package"),
            ),
            (
                "keyring",
                pac(
                    Kind::Install,
                    "archlinux-keyring",
                    Some(true),
                    "pacman -Sy --noconfirm archlinux-keyring",
                ),
                (R, "keyring"),
            ),
            (
                "no command line",
                {
                    let mut e = pac(Kind::Upgrade, "firefox", None, "x");
                    e.meta.command = None;
                    e
                },
                (A, "other"),
            ),
            // omarchy update (the attributed one: `omarchy_update_rows`)
            ("bare dev", update("4.0.6-1", "dev"), (A, "omarchy-other")),
            (
                "unattributed",
                update("4.0.6-1", "4.0.7-1"),
                (A, "omarchy-other"),
            ),
            // plugins and theme
            (
                "plugin-add",
                ev(Source::Plugins, Kind::PluginAdd, "io.github.example.x"),
                (A, "plugin"),
            ),
            (
                "plugin-remove",
                ev(Source::Plugins, Kind::PluginRemove, "io.github.example.x"),
                (A, "plugin"),
            ),
            (
                "plugin-update",
                ev(Source::Plugins, Kind::PluginUpdate, "io.github.example.x"),
                (A, "plugin"),
            ),
            (
                "plugin-enable",
                ev(Source::Plugins, Kind::PluginEnable, "io.github.example.x"),
                (R, "plugin-toggle"),
            ),
            (
                "plugin-disable",
                ev(Source::Plugins, Kind::PluginDisable, "io.github.example.x"),
                (R, "plugin-toggle"),
            ),
            (
                "theme-set",
                ev(Source::Theme, Kind::ThemeSet, "tokyo-night"),
                (R, "theme"),
            ),
            // config: evidence marks, routine paths, persistence paths, overrides
            (
                "Omarchy's copy",
                config(
                    Kind::ConfigChange,
                    "~/.config/hypr/hyprland.lua",
                    Some(MATCHES_OMARCHY_DEFAULT),
                ),
                (R, "omarchy-default"),
            ),
            (
                "a shipped sample hook",
                config(
                    Kind::ConfigAdd,
                    "~/.config/omarchy/hooks/post-update.d/x.sample",
                    Some(MATCHES_OMARCHY_DEFAULT),
                ),
                (R, "omarchy-default"),
            ),
            (
                "Omarchy's desktop entry",
                config(
                    Kind::ConfigAdd,
                    "~/.local/share/applications/HEY.desktop",
                    Some(MATCHES_OMARCHY_DEFAULT),
                ),
                (R, "omarchy-default"),
            ),
            (
                "a link into /usr",
                config(
                    Kind::ConfigAdd,
                    "~/.config/systemd/user/graphical-session.target.wants/x.service",
                    Some(MATCHES_SYSTEM_LINK),
                ),
                (R, "system-link"),
            ),
            (
                "shell.json",
                config(Kind::ConfigChange, "~/.config/omarchy/shell.json", None),
                (R, "routine-paths"),
            ),
            (
                "refresh backup",
                config(
                    Kind::ConfigAdd,
                    "~/.config/hypr/hyprland.lua.bak.1786539345",
                    None,
                ),
                (R, "routine-paths"),
            ),
            (
                "removed shell.json",
                config(Kind::ConfigRemove, "~/.config/omarchy/shell.json", None),
                (R, "routine-paths"),
            ),
            (
                "user unit",
                config(
                    Kind::ConfigAdd,
                    "~/.config/systemd/user/miner.service",
                    None,
                ),
                (C, "always-red-paths"),
            ),
            (
                "hook in a .d",
                config(
                    Kind::ConfigAdd,
                    "~/.config/omarchy/hooks/post-update.d/x.sh",
                    None,
                ),
                (C, "always-red-paths"),
            ),
            (
                "flat hook",
                config(Kind::ConfigAdd, "~/.config/omarchy/hooks/post-boot", None),
                (C, "always-red-paths"),
            ),
            (
                "autostart",
                config(Kind::ConfigAdd, "~/.config/autostart/x.desktop", None),
                (C, "always-red-paths"),
            ),
            (
                "environment.d",
                config(Kind::ConfigChange, "~/.config/environment.d/x.conf", None),
                (C, "always-red-paths"),
            ),
            (
                "uwsm",
                config(Kind::ConfigChange, "~/.config/uwsm/env", None),
                (C, "always-red-paths"),
            ),
            (
                "~/.profile",
                config(Kind::ConfigChange, "~/.profile", None),
                (C, "always-red-paths"),
            ),
            (
                "~/.bash_profile",
                config(Kind::ConfigAdd, "~/.bash_profile", None),
                (C, "always-red-paths"),
            ),
            (
                "an edited sample hook (never runs)",
                config(
                    Kind::ConfigChange,
                    "~/.config/omarchy/hooks/theme-set.d/x.sample",
                    None,
                ),
                (A, "config"),
            ),
            (
                "hyprland lua",
                config(Kind::ConfigChange, "~/.config/hypr/autostart.lua", None),
                (A, "config"),
            ),
            (
                "waybar",
                config(Kind::ConfigChange, "~/.config/waybar/config.jsonc", None),
                (A, "config"),
            ),
            (
                "menu extension",
                config(
                    Kind::ConfigChange,
                    "~/.config/omarchy/extensions/omarchy-menu.jsonc",
                    None,
                ),
                (A, "config"),
            ),
            (
                "themed template",
                config(
                    Kind::ConfigAdd,
                    "~/.config/omarchy/themed/btop.theme.tpl",
                    None,
                ),
                (A, "config"),
            ),
            (
                ".bashrc",
                config(Kind::ConfigChange, "~/.bashrc", None),
                (A, "config"),
            ),
            (
                ".zshrc",
                config(Kind::ConfigChange, "~/.zshrc", None),
                (A, "config"),
            ),
            (
                "own desktop entry",
                config(
                    Kind::ConfigAdd,
                    "~/.local/share/applications/x.desktop",
                    None,
                ),
                (A, "config"),
            ),
            (
                "theme colours",
                config(
                    Kind::ConfigAdd,
                    "~/.config/omarchy/themes/mine/colors.toml",
                    None,
                ),
                (R, "theme-assets"),
            ),
            (
                "theme shell.toml",
                config(
                    Kind::ConfigChange,
                    "~/.config/omarchy/themes/mine/shell.toml",
                    None,
                ),
                (R, "theme-assets"),
            ),
            (
                "background",
                config(
                    Kind::ConfigAdd,
                    "~/.config/omarchy/backgrounds/mine/1.svg",
                    None,
                ),
                (R, "theme-assets"),
            ),
            (
                "theme lua",
                config(
                    Kind::ConfigAdd,
                    "~/.config/omarchy/themes/mine/hyprland.lua",
                    None,
                ),
                (A, "config"),
            ),
            (
                "theme terminal config",
                config(
                    Kind::ConfigAdd,
                    "~/.config/omarchy/themes/mine/kitty.conf",
                    None,
                ),
                (A, "config"),
            ),
            (
                "theme vscode",
                config(
                    Kind::ConfigAdd,
                    "~/.config/omarchy/themes/mine/vscode.json",
                    None,
                ),
                (A, "config"),
            ),
            (
                "cloned theme lua",
                config(
                    Kind::ConfigAdd,
                    "~/.config/omarchy/themes/repo/hyprland.lua",
                    Some(MATCHES_THEME_REPO),
                ),
                (R, "theme-repo"),
            ),
            (
                "removed hook",
                config(
                    Kind::ConfigRemove,
                    "~/.config/omarchy/hooks/post-update.d/x.sh",
                    None,
                ),
                (A, "config-remove"),
            ),
            (
                "removed lua",
                config(Kind::ConfigRemove, "~/.config/hypr/monitors.lua", None),
                (A, "config-remove"),
            ),
            (
                "a mark on a removal counts nothing",
                config(
                    Kind::ConfigRemove,
                    "~/.config/hypr/x.lua",
                    Some(MATCHES_OMARCHY_DEFAULT),
                ),
                (A, "config-remove"),
            ),
            // the total row
            (
                "anything else",
                ev(Source::Theme, Kind::ConfigChange, "x"),
                (A, "other"),
            ),
        ];
        let defaults = DriftConfig::default();
        let mut failed = Vec::new();
        for (label, e, want) in rows {
            let got = verdict(&defaults, std::slice::from_ref(&e), &[0]);
            if got != want {
                failed.push(format!("{label}: got {got:?}, want {want:?}"));
            }
        }
        assert!(failed.is_empty(), "{}", failed.join("\n"));
    }

    /// The omarchy `update` row: routine only with package-shaped versions
    /// and Omarchy's package moved to the new version by a plain full
    /// upgrade at most 31 days before.
    #[test]
    fn omarchy_update_rows() {
        let line = "pacman -Syu --noconfirm --overwrite /usr/share/omarchy/*";
        let mut moved = pac(Kind::Upgrade, "omarchy", Some(false), line);
        moved.meta.to = Some("4.0.7-1".into());
        moved.ts = at("2026-10-01T11:58:00+02:00");
        let d = DriftConfig::default();
        let ledger = [update("4.0.6-1", "4.0.7-1"), moved.clone()];
        assert_eq!(verdict(&d, &ledger, &[0]), (R, "omarchy-update"));
        let mut dev = moved.clone();
        dev.subject = "omarchy-dev".into();
        assert_eq!(
            verdict(&d, &[update("4.0.6-1", "4.0.7-1"), dev], &[0]),
            (R, "omarchy-update")
        );
        // another version, a named transaction, a downgrade, too old, later
        let mut other = moved.clone();
        other.meta.to = Some("4.0.8-1".into());
        let mut named = moved.clone();
        named.meta.command = Some("pacman -S omarchy".into());
        let mut down = moved.clone();
        down.kind = Kind::Downgrade;
        let mut old = moved.clone();
        old.ts = at("2026-08-01T12:00:00+02:00");
        let mut later = moved.clone();
        later.ts = at("2026-10-01T12:00:01+02:00");
        for (label, p) in [
            ("other", other),
            ("named", named),
            ("down", down),
            ("old", old),
            ("later", later),
        ] {
            let got = verdict(&d, &[update("4.0.6-1", "4.0.7-1"), p], &[0]);
            assert_eq!(got, (A, "omarchy-other"), "{label}");
        }
        assert_eq!(
            verdict(&d, &[update("dev", "4.0.7-1"), moved.clone()], &[0]),
            (A, "omarchy-other")
        );
    }

    /// A group's class is the highest of its members; a dependency of a
    /// named transaction follows its explicit members, also when they are
    /// no longer in the item (resolved with `--only`).
    #[test]
    fn groups_and_dependencies() {
        let d = DriftConfig::default();
        let syu = |kind, subject: &str| pac(kind, subject, Some(false), "pacman -Syu");
        let ledger = [
            syu(Kind::Upgrade, "firefox"),
            syu(Kind::Downgrade, "systemd"),
            syu(Kind::Upgrade, "linux"),
        ];
        assert_eq!(verdict(&d, &ledger, &[0, 2]), (R, "sysupgrade"));
        assert_eq!(
            verdict(&d, &ledger, &[0, 1, 2]),
            (A, "sysupgrade-red"),
            "max"
        );
        let named =
            |kind, subject: &str, explicit| pac(kind, subject, Some(explicit), "pacman -S linux");
        let ledger = [
            named(Kind::Install, "linux", true),
            named(Kind::Install, "kmod", false),
        ];
        assert_eq!(verdict(&d, &ledger, &[0, 1]), (C, "always-red"));
        assert_eq!(
            verdict(&d, &ledger, &[1]),
            (C, "always-red"),
            "the dependency alone"
        );
        let named =
            |kind, subject: &str, explicit| pac(kind, subject, Some(explicit), "pacman -S htop");
        let ledger = [
            named(Kind::Install, "htop", true),
            named(Kind::Install, "libnl", false),
        ];
        assert_eq!(verdict(&d, &ledger, &[1, 0]), (A, "package"));
        // a dependency whose transaction has no explicit member
        let lone = [pac(
            Kind::Install,
            "libnl",
            Some(false),
            "pacman -S --asdeps x",
        )];
        assert_eq!(verdict(&d, &lone, &[0]), (A, "other"));
    }

    /// `[drift]`: a routine rule left out of `routine` does not apply (the
    /// event falls to the next row); the path and package lists are the
    /// user's.
    #[test]
    fn the_config_changes_the_rules() {
        let without = |rule: &str| DriftConfig {
            routine: DriftConfig::default()
                .routine
                .into_iter()
                .filter(|r| r != rule)
                .collect(),
            ..DriftConfig::default()
        };
        let theme = [ev(Source::Theme, Kind::ThemeSet, "x")];
        assert_eq!(verdict(&without("theme"), &theme, &[0]), (A, "other"));
        let link = [config(
            Kind::ConfigAdd,
            "~/.config/systemd/user/x.service",
            Some(MATCHES_SYSTEM_LINK),
        )];
        assert_eq!(
            verdict(&without("system-link"), &link, &[0]),
            (C, "always-red-paths")
        );
        let syu = [pac(Kind::Upgrade, "linux", Some(false), "pacman -Syu")];
        assert_eq!(verdict(&without("sysupgrade"), &syu, &[0]), (A, "other"));
        let up = [pac(
            Kind::Upgrade,
            "firefox",
            Some(true),
            "pacman -S firefox",
        )];
        assert_eq!(verdict(&without("upgrade"), &up, &[0]), (A, "upgrade"));
        let mut d = DriftConfig::default();
        d.always_red_paths.push("~/.ssh/authorized_keys".into());
        d.routine_paths.push("~/.config/hypr/monitors.lua".into());
        d.routine_packages = vec!["htop".into()];
        let keys = [config(Kind::ConfigChange, "~/.ssh/authorized_keys", None)];
        assert_eq!(verdict(&d, &keys, &[0]), (C, "always-red-paths"));
        let mon = [config(
            Kind::ConfigChange,
            "~/.config/hypr/monitors.lua",
            None,
        )];
        assert_eq!(verdict(&d, &mon, &[0]), (R, "routine-paths"));
        let htop = [pac(Kind::Install, "htop", Some(true), "pacman -S htop")];
        assert_eq!(verdict(&d, &htop, &[0]), (R, "keyring"));
        let kr = [pac(
            Kind::Install,
            "archlinux-keyring",
            Some(true),
            "pacman -S archlinux-keyring",
        )];
        assert_eq!(
            verdict(&d, &kr, &[0]),
            (A, "package"),
            "the list is the user's"
        );
    }

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
