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
//!   the keyrings, Omarchy's own update, a plugin toggle, Seldon's own
//!   plugin added or enabled, a theme switch,
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

use chrono::{DateTime, Duration, FixedOffset};
use ulid::Ulid;

use super::drift::AlwaysRed;
use crate::attribution::OWN_PLUGIN;
use crate::config::{AttentionMode, DriftConfig};
use crate::model::event::{Event, Kind, Source};
use crate::pkgcmd::{Op, PacmanCommand, parse_command, split_logged};

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
/// Omarchy's toggle state directory (`omarchy-toggle` touches and removes
/// empty flag files; `omarchy-hyprland-toggle` copies Omarchy's flags).
const TOGGLES_DIR: &str = "~/.local/state/omarchy/toggles/";
/// SHA-256 of no bytes: an empty flag file.
const EMPTY_SHA256: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
/// Theme files that run code: what `omarchy theme install` strips.
const THEME_CODE: [&str; 5] = [
    "alacritty.toml",
    "foot.ini",
    "ghostty.conf",
    "kitty.conf",
    "vscode.json",
];

/// The boot and login files a `.pacnew`, `.pacsave` or `.pacorig` beside
/// is a crisis (ADR-0042, WP-141): mkinitcpio, Limine (the paths
/// `omarchy-settings` ships and Omarchy writes: `/etc/default/limine`,
/// `/etc/limine-entry-tool.conf`, `/etc/limine-entry-tool.d/`), PAM.
/// Not `/etc/systemd` (Omarchy uses drop-ins) or `/etc/security` (Omarchy
/// overrides `pam`'s files there): the file in use keeps working. In the
/// [`PathGlobs`] syntax; a directory covers what lies below it.
const PACNEW_RED: [&str; 7] = [
    "/etc/mkinitcpio.conf",
    "/etc/mkinitcpio.conf.d",
    "/etc/mkinitcpio.d",
    "/etc/default/limine",
    "/etc/limine*",
    "/boot/limine*",
    "/etc/pam.d",
];

/// The suffixes of the files pacman leaves (`collectors::pacman`).
const PACNEW_SUFFIXES: [&str; 3] = [".pacnew", ".pacsave", ".pacorig"];

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
    routine_paths: PathGlobs,
    always_red_paths: PathGlobs,
    routine_packages: HashSet<String>,
    pacnew_red: PathGlobs,
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

/// `routinePaths` and `alwaysRedPaths`: the `[redaction] skipPaths` glob
/// syntax (`collectors::config::SkipPaths`, SPEC-ENGINE §7) on a subject
/// as [`glob_path`] writes it, without a regex (an index build compiles
/// nothing). A pattern with a `/` matches the whole path, anywhere at a
/// directory boundary when it is not absolute; one without matches the
/// last component; `*` and `?` stay in one component, `**` crosses them;
/// a match on a directory covers everything below it.
#[derive(Debug, Default)]
struct PathGlobs {
    paths: Vec<String>,
    names: Vec<String>,
}

impl PathGlobs {
    fn new(patterns: &[String]) -> Self {
        let mut globs = PathGlobs::default();
        for p in patterns.iter().map(|p| p.trim()).filter(|p| !p.is_empty()) {
            let p = p.trim_end_matches('/');
            if !p.contains('/') && p != "~" {
                globs.names.push(p.to_string());
            } else {
                globs.paths.push(glob_path(p));
            }
        }
        globs
    }

    fn matches(&self, path: &str) -> bool {
        let name = path.rsplit('/').next().unwrap_or(path);
        self.names
            .iter()
            .any(|n| glob_match(n.as_bytes(), name.as_bytes(), true))
            || self.paths.iter().any(|p| {
                let (p, t) = (p.as_bytes(), path.as_bytes());
                if p.first() == Some(&b'/') {
                    return glob_match(p, t, false);
                }
                // at the start or after any `/`
                std::iter::once(0)
                    .chain(
                        t.iter()
                            .enumerate()
                            .filter(|(_, c)| **c == b'/')
                            .map(|(i, _)| i + 1),
                    )
                    .any(|at| glob_match(p, &t[at..], false))
            })
    }
}

/// Whether `pat` matches all of `text` (`whole`), or a prefix of it that
/// ends at its end or before a `/` (a directory and what lies below).
fn glob_match(pat: &[u8], text: &[u8], whole: bool) -> bool {
    match pat.split_first() {
        None => text.is_empty() || (!whole && text[0] == b'/'),
        Some((b'*', rest)) if rest.first() == Some(&b'*') => {
            let rest = &rest[1..];
            (0..=text.len()).any(|i| glob_match(rest, &text[i..], whole))
        }
        Some((b'*', rest)) => {
            let span = text.iter().position(|&c| c == b'/').unwrap_or(text.len());
            (0..=span).any(|i| glob_match(rest, &text[i..], whole))
        }
        Some((b'?', rest)) => {
            text.first().is_some_and(|&c| c != b'/') && glob_match(rest, &text[1..], whole)
        }
        Some((&c, rest)) => text.first() == Some(&c) && glob_match(rest, &text[1..], whole),
    }
}

impl Rules {
    pub fn new(config: &DriftConfig) -> Self {
        Rules {
            attention: config.attention,
            always_red: AlwaysRed::new(&config.always_red),
            routine: config.routine.iter().cloned().collect(),
            routine_paths: PathGlobs::new(&config.routine_paths),
            always_red_paths: PathGlobs::new(&config.always_red_paths),
            routine_packages: config.routine_packages.iter().cloned().collect(),
            pacnew_red: PathGlobs::new(&PACNEW_RED.map(String::from)),
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
            Source::Config => Some(self.config(e, history)),
            _ => Verdict::attention("other"),
        }
    }

    fn pacman(&self, e: &Event, cmd: Option<&PacmanCommand>) -> Option<Verdict> {
        if e.kind == Kind::Note {
            return Some(self.pacnew(&e.subject));
        }
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
            && matches!(c.op, Some(Op::Sync | Op::Upgrade))
            && !c.query
            && !c.stdin_targets
            && !c.targets.is_empty()
            && c.targets.iter().all(|t| self.routine_packages.contains(t))
            && let Some(v) = self.routine("keyring")
        {
            return Some(v);
        }
        match e.explicit {
            Some(false) => Verdict::FOLLOWS,
            Some(true) => match (e.kind, red) {
                // `-U` counts as an upgrade of what is installed only from a
                // package cache; another file is anyone's package
                (Kind::Upgrade | Kind::Reinstall, false)
                    if cmd.is_some_and(|c| c.op == Some(Op::Upgrade) && !c.from_cache) =>
                {
                    Verdict::attention("package")
                }
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

    /// A file pacman left beside a configuration file (WP-141): never
    /// routine — the new default was not applied, or the user's file was
    /// moved aside, and nothing but a merge changes that; a crisis beside a
    /// boot or login file ([`PACNEW_RED`], ADR-0042). Its own item, never
    /// its transaction's: it carries no `txId`.
    fn pacnew(&self, subject: &str) -> Verdict {
        let file = PACNEW_SUFFIXES
            .iter()
            .find_map(|s| subject.strip_suffix(s))
            .unwrap_or(subject);
        if self.pacnew_red.matches(file) {
            Verdict::new(Class::Crisis, "pacnew-red")
        } else {
            Verdict::new(Class::Attention, "pacnew")
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
        // Seldon's own plugin added or enabled: the user installing Seldon
        // (ADR-0050); before the toggle row, so its rule says so
        if e.subject == OWN_PLUGIN
            && matches!(e.kind, Kind::PluginAdd | Kind::PluginEnable)
            && let Some(v) = self.routine("seldon-self")
        {
            return v;
        }
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

    fn config(&self, e: &Event, history: &History) -> Verdict {
        let mark = e.meta.extra.get(MATCHES_KEY).and_then(|v| v.as_str());
        let removed = e.kind == Kind::ConfigRemove;
        // ADR-0037 §1: a toggle is routine both ways — a flag file (empty,
        // or Omarchy's shipped copy by capture evidence) created, changed
        // to or removed in the toggles directory
        if e.subject.starts_with(TOGGLES_DIR) {
            let hash = if removed {
                e.meta.hash_from.as_deref()
            } else {
                e.meta.hash_to.as_deref()
            };
            if hash == Some(EMPTY_SHA256)
                && let Some(v) = self.routine("toggle-flag")
            {
                return v;
            }
            if removed
                && mark == Some(MATCHES_OMARCHY_DEFAULT)
                && let Some(v) = self.routine(MATCHES_OMARCHY_DEFAULT)
            {
                return v;
            }
        }
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
        // the persistence paths come right after the evidence rows (ADR-0028
        // §2): a file there runs at login whatever its name, so a backup
        // is routine only with evidence that it is one (WP-109 round 2)
        if !removed && self.always_red_paths.matches(&path) && !inert_hook(&e.subject) {
            // not in Omarchy's hook directories: `omarchy-hook` runs every
            // file there not named `*.sample`, a backup included (B5)
            if self.routine_paths.matches(&path)
                && !e.subject.starts_with(HOOKS_DIR)
                && history.holds_base_content(e)
                && let Some(v) = self.routine("routine-paths")
            {
                return v;
            }
            return Verdict::new(Class::Crisis, "always-red-paths");
        }
        if self.routine_paths.matches(&path)
            && let Some(v) = self.routine("routine-paths")
        {
            return v;
        }
        if removed {
            return Verdict::new(Class::Attention, "config-remove");
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
    /// The whole ledger, read only for the rare backup in a persistence
    /// path ([`History::holds_base_content`]).
    events: &'a [Event],
}

impl<'a> History<'a> {
    pub fn new(events: &'a [Event]) -> Self {
        let omarchy = events
            .iter()
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
        History { omarchy, events }
    }

    /// Whether the backup event `e` (subject `<base>.bak.<…>`) holds the
    /// content the ledger last recorded for `<base>` (WP-109 round 2): the
    /// `hashTo` of the latest config event of `<base>` before `e`, or the
    /// `hashFrom` of the first one at or after it — the copy `omarchy
    /// refresh` makes before it puts the default back. Records at the
    /// backup's own second count as after it. A base the ledger never
    /// recorded gives no evidence.
    fn holds_base_content(&self, e: &Event) -> bool {
        let Some(hash) = e.meta.hash_to.as_deref() else {
            return false;
        };
        let Some(at) = e.subject.rfind(".bak.") else {
            return false;
        };
        let base = &e.subject[..at];
        if base.is_empty() || base.ends_with('/') || at + 5 == e.subject.len() {
            return false;
        }
        let mut before: Option<&Event> = None;
        let mut after: Option<&Event> = None;
        for x in self
            .events
            .iter()
            .filter(|x| x.source == Source::Config && x.subject == base)
        {
            // by time only: `omarchy refresh` writes the backup and the
            // default within a second, and the base sorts first by name
            if x.ts < e.ts {
                if before.is_none_or(|b| (b.ts, b.id) < (x.ts, x.id)) {
                    before = Some(x);
                }
            } else if after.is_none_or(|a| (x.ts, x.id) < (a.ts, a.id)) {
                after = Some(x);
            }
        }
        before.is_some_and(|b| b.meta.hash_to.as_deref() == Some(hash))
            || after.is_some_and(|a| a.meta.hash_from.as_deref() == Some(hash))
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
    use std::path::Path;

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

    /// A file pacman left (`collectors::pacman`, WP-141) in a transaction
    /// whose logged command is `cmd`.
    fn left(subject: &str, cmd: &str) -> Event {
        let mut e = ev(Source::Pacman, Kind::Note, subject);
        e.meta.command = Some(cmd.into());
        e.meta
            .extra
            .insert("transaction".into(), format!("tx-{cmd}").into());
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
                "the keyring and another package",
                pac(
                    Kind::Install,
                    "htop",
                    Some(true),
                    "pacman -Sy --noconfirm archlinux-keyring htop",
                ),
                (A, "package"),
            ),
            (
                "a keyring removed by name (N2)",
                pac(
                    Kind::Remove,
                    "archlinux-keyring",
                    Some(true),
                    "pacman -Rdd archlinux-keyring",
                ),
                (A, "package"),
            ),
            (
                "-Syu with targets from stdin (B4)",
                pac(Kind::Install, "linux-zen", Some(false), "pacman -Syu -"),
                (A, "other"),
            ),
            (
                "-S -u with targets from stdin (B4)",
                pac(Kind::Install, "evilpkg", Some(false), "pacman -S -u -"),
                (A, "other"),
            ),
            (
                "a keyring transaction with targets from stdin",
                pac(
                    Kind::Install,
                    "evilpkg",
                    Some(false),
                    "pacman -S archlinux-keyring -",
                ),
                (A, "other"),
            ),
            (
                "-U from outside a cache is an install-like attention (N1)",
                pac(
                    Kind::Upgrade,
                    "sudo",
                    Some(true),
                    "pacman -U /tmp/evil/sudo-9.9-1-x86_64.pkg.tar.zst",
                ),
                (A, "package"),
            ),
            (
                "-U from yay's cache",
                pac(
                    Kind::Upgrade,
                    "zed-bin",
                    Some(true),
                    "pacman -U /home/user/.cache/yay/zed-bin/zed-bin-1.0-1-x86_64.pkg.tar.zst",
                ),
                (R, "upgrade"),
            ),
            (
                "-U from paru's cache",
                pac(
                    Kind::Reinstall,
                    "zed-bin",
                    Some(true),
                    "pacman -U /home/user/.cache/paru/clone/zed-bin/zed-bin-1.0-1-x86_64.pkg.tar.zst",
                ),
                (R, "upgrade"),
            ),
            (
                "-U with one file outside the cache",
                pac(
                    Kind::Upgrade,
                    "firefox",
                    Some(true),
                    "pacman -U /var/cache/pacman/pkg/firefox-1-1-x86_64.pkg.tar.zst /tmp/x-1-1-x86_64.pkg.tar.zst",
                ),
                (A, "package"),
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
            // Seldon's own plugin (ADR-0050): its add and enable; the rest
            // is rule 8's (update, disable) or stays attention (remove)
            (
                "own plugin-add",
                ev(Source::Plugins, Kind::PluginAdd, "jax.seldon"),
                (R, "seldon-self"),
            ),
            (
                "own plugin-enable",
                ev(Source::Plugins, Kind::PluginEnable, "jax.seldon"),
                (R, "seldon-self"),
            ),
            (
                "own plugin-disable",
                ev(Source::Plugins, Kind::PluginDisable, "jax.seldon"),
                (R, "plugin-toggle"),
            ),
            (
                "own plugin-remove",
                ev(Source::Plugins, Kind::PluginRemove, "jax.seldon"),
                (A, "plugin"),
            ),
            (
                "own plugin-update",
                ev(Source::Plugins, Kind::PluginUpdate, "jax.seldon"),
                (A, "plugin"),
            ),
            (
                "a look-alike id's add",
                ev(Source::Plugins, Kind::PluginAdd, "jax.seldon-extra"),
                (A, "plugin"),
            ),
            (
                "a look-alike id's enable",
                ev(Source::Plugins, Kind::PluginEnable, "jax.seldon.x"),
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
            // B1: a backup name does not make a persistence file harmless
            (
                "a .bak. hook (B1)",
                config(
                    Kind::ConfigAdd,
                    "~/.config/omarchy/hooks/post-update.d/evil.bak.sh",
                    None,
                ),
                (C, "always-red-paths"),
            ),
            (
                "a .bak. user unit (B1)",
                config(
                    Kind::ConfigAdd,
                    "~/.config/systemd/user/evil.bak.service",
                    None,
                ),
                (C, "always-red-paths"),
            ),
            (
                "a .bak. autostart entry (B1)",
                config(
                    Kind::ConfigAdd,
                    "~/.config/autostart/evil.bak.desktop",
                    None,
                ),
                (C, "always-red-paths"),
            ),
            (
                "a .bak. environment.d file (B1)",
                config(
                    Kind::ConfigAdd,
                    "~/.config/environment.d/50-evil.bak.conf",
                    None,
                ),
                (C, "always-red-paths"),
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
            // a file pacman left (WP-141): attention, beside a boot, login
            // or security file a crisis; whatever the transaction was
            (
                ".pacnew in a plain full upgrade",
                left("/etc/pacman.conf.pacnew", "pacman -Syu"),
                (A, "pacnew"),
            ),
            (
                ".pacsave of a named removal",
                left("/etc/ssh/sshd_config.pacsave", "pacman -Rns openssh"),
                (A, "pacnew"),
            ),
            (
                ".pacorig in a keyring transaction",
                left("/etc/x.conf.pacorig", "pacman -Sy archlinux-keyring"),
                (A, "pacnew"),
            ),
            (
                ".pacnew of mkinitcpio.conf, Omarchy's update",
                left("/etc/mkinitcpio.conf.pacnew", omarchy_line),
                (C, "pacnew-red"),
            ),
            (
                ".pacnew in mkinitcpio.conf.d",
                left(
                    "/etc/mkinitcpio.conf.d/omarchy_hooks.conf.pacnew",
                    "pacman -Syu",
                ),
                (C, "pacnew-red"),
            ),
            (
                ".pacnew of a mkinitcpio preset",
                left("/etc/mkinitcpio.d/linux.preset.pacnew", "pacman -Syu"),
                (C, "pacnew-red"),
            ),
            (
                ".pacsave of /etc/default/limine",
                left("/etc/default/limine.pacsave", "pacman -Rns limine"),
                (C, "pacnew-red"),
            ),
            (
                ".pacnew in limine-entry-tool.d",
                left(
                    "/etc/limine-entry-tool.d/omarchy-defaults.conf.pacnew",
                    "pacman -Syu",
                ),
                (C, "pacnew-red"),
            ),
            (
                ".pacnew of /boot/limine.conf",
                left("/boot/limine.conf.pacnew", "pacman -Syu"),
                (C, "pacnew-red"),
            ),
            (
                ".pacnew of a systemd file (Omarchy uses drop-ins)",
                left("/etc/systemd/logind.conf.pacnew", "pacman -Syu"),
                (A, "pacnew"),
            ),
            (
                ".pacorig in pam.d",
                left("/etc/pam.d/system-login.pacorig", "pacman -S pambase"),
                (C, "pacnew-red"),
            ),
            (
                ".pacnew in /etc/security (Omarchy overrides pam's files)",
                left("/etc/security/faillock.conf.pacnew", "pacman -Syu"),
                (A, "pacnew"),
            ),
            (
                ".pacnew of fstab",
                left("/etc/fstab.pacnew", "pacman -Syu"),
                (A, "pacnew"),
            ),
            (
                ".pacnew of crypttab",
                left("/etc/crypttab.pacnew", "pacman -Syu"),
                (A, "pacnew"),
            ),
            (
                ".pacnew of sudoers",
                left("/etc/sudoers.pacnew", "pacman -Syu"),
                (A, "pacnew"),
            ),
            (
                ".pacsave in pam.d",
                left("/etc/pam.d/sudo.pacsave", "pacman -Rns sudo"),
                (C, "pacnew-red"),
            ),
            (
                ".pacnew without a command line",
                {
                    let mut e = left("/etc/pam.d/system-auth.pacnew", "");
                    e.meta.command = None;
                    e
                },
                (C, "pacnew-red"),
            ),
            (
                "a look-alike of a system path",
                left("/etc/pam.dx/a.pacnew", "pacman -Syu"),
                (A, "pacnew"),
            ),
            (
                "another root (pacman -r /mnt)",
                left(
                    "/mnt/etc/mkinitcpio.conf.pacnew",
                    "pacman -r /mnt -S mkinitcpio",
                ),
                (A, "pacnew"),
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
        // WP-141: a file the transaction left is no member and lends it no
        // class; it is classed alone
        let mut pacnew = left("/etc/pam.d/system-login.pacnew", "pacman -S htop");
        pacnew.meta.extra.insert(
            "transaction".into(),
            ledger[0].tx_id.clone().unwrap().into(),
        );
        let ledger = [ledger[0].clone(), ledger[1].clone(), pacnew];
        assert_eq!(verdict(&d, &ledger, &[1]), (A, "package"));
        assert_eq!(verdict(&d, &ledger, &[2]), (C, "pacnew-red"));
        // K19: of mixed explicit members the highest counts, also when the
        // crisis member is no longer in the item (resolved)
        let mixed = |kind, subject: &str, explicit| {
            pac(kind, subject, Some(explicit), "pacman -S htop linux")
        };
        let ledger = [
            mixed(Kind::Install, "htop", true),
            mixed(Kind::Install, "linux", true),
            mixed(Kind::Install, "kmod", false),
        ];
        assert_eq!(verdict(&d, &ledger, &[2]), (C, "always-red"));
        assert_eq!(verdict(&d, &ledger, &[0, 2]), (C, "always-red"));
        // a dependency whose transaction has no explicit member
        let lone = [pac(
            Kind::Install,
            "libnl",
            Some(false),
            "pacman -S --asdeps x",
        )];
        assert_eq!(verdict(&d, &lone, &[0]), (A, "other"));
    }

    /// B1 (WP-109 round 2): a backup in a persistence path is routine only
    /// with evidence that it is one — it holds the content the ledger last
    /// recorded for its base file (the copy `omarchy refresh` makes before
    /// it restores the default); the base, restored to Omarchy's copy,
    /// carries `omarchy-default`.
    #[test]
    fn a_refresh_backup_in_a_persistence_path_needs_its_base() {
        let d = DriftConfig::default();
        let at_ts = |e: Event, ts: &str| Event { ts: at(ts), ..e };
        let mut old = config(Kind::ConfigAdd, "~/.config/uwsm/env", None);
        old.meta.hash_to = Some("H-old".into());
        let old = at_ts(old, "2026-09-01T10:00:00+02:00");
        let mut backup = config(Kind::ConfigAdd, "~/.config/uwsm/env.bak.1786539345", None);
        backup.meta.hash_to = Some("H-old".into());
        let mut base = config(
            Kind::ConfigChange,
            "~/.config/uwsm/env",
            Some(MATCHES_OMARCHY_DEFAULT),
        );
        base.meta.hash_from = Some("H-old".into());
        base.meta.hash_to = Some("H-default".into());
        // the base recorded earlier, the pair in one capture
        let ledger = [old.clone(), backup.clone(), base.clone()];
        assert_eq!(verdict(&d, &ledger, &[1]), (R, "routine-paths"));
        assert_eq!(verdict(&d, &ledger, &[2]), (R, "omarchy-default"));
        // the base never recorded before: its change's hashFrom is the evidence
        let ledger = [backup.clone(), base.clone()];
        assert_eq!(verdict(&d, &ledger, &[0]), (R, "routine-paths"));
        // the latest record of the base before the backup wins
        let mut newer = old.clone();
        newer.kind = Kind::ConfigChange;
        newer.meta.hash_from = Some("H-old".into());
        newer.meta.hash_to = Some("H-newer".into());
        let newer = at_ts(newer, "2026-09-20T10:00:00+02:00");
        let ledger = [old.clone(), newer, backup.clone()];
        assert_eq!(verdict(&d, &ledger, &[2]), (C, "always-red-paths"));
        // other content than the base had, or no base at all: a crisis
        let mut forged = backup.clone();
        forged.meta.hash_to = Some("H-evil".into());
        let ledger = [old.clone(), forged, base.clone()];
        assert_eq!(verdict(&d, &ledger, &[1]), (C, "always-red-paths"));
        let ledger = [backup.clone()];
        assert_eq!(verdict(&d, &ledger, &[0]), (C, "always-red-paths"));
        // a name that ends in `.bak.` has no epoch part: no base
        let mut bare = config(Kind::ConfigAdd, "~/.config/uwsm/env.bak.", None);
        bare.meta.hash_to = Some("H-old".into());
        let ledger = [old, bare];
        assert_eq!(verdict(&d, &ledger, &[1]), (C, "always-red-paths"));
        // B5: in a hook directory a backup runs too (`omarchy-hook` skips
        // only `*.sample`): a crisis even with its base's content
        let mut sample = config(
            Kind::ConfigChange,
            "~/.config/omarchy/hooks/post-update.d/x.sample",
            None,
        );
        sample.meta.hash_to = Some("H-run".into());
        let sample = at_ts(sample, "2026-09-01T10:00:00+02:00");
        let mut copy = config(
            Kind::ConfigAdd,
            "~/.config/omarchy/hooks/post-update.d/x.sample.bak.1786539345",
            None,
        );
        copy.meta.hash_to = Some("H-run".into());
        let ledger = [sample, copy];
        assert_eq!(
            verdict(&d, &ledger, &[0]),
            (A, "config"),
            "the sample stays quiet"
        );
        assert_eq!(verdict(&d, &ledger, &[1]), (C, "always-red-paths"));
        // outside the persistence paths a backup stays routine without evidence
        let ledger = [config(Kind::ConfigAdd, "~/.config/hypr/x.lua.bak.1", None)];
        assert_eq!(verdict(&d, &ledger, &[0]), (R, "routine-paths"));
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
        // Seldon's own plugin falls to the plugin rows
        let own = [
            ev(Source::Plugins, Kind::PluginAdd, "jax.seldon"),
            ev(Source::Plugins, Kind::PluginEnable, "jax.seldon"),
        ];
        assert_eq!(verdict(&without("seldon-self"), &own, &[0]), (A, "plugin"));
        assert_eq!(
            verdict(&without("seldon-self"), &own, &[1]),
            (R, "plugin-toggle")
        );
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

    /// The regex-free matcher agrees with `SkipPaths` (the `skipPaths`
    /// syntax it promises) on the defaults and on the syntax's cases.
    #[test]
    fn path_globs_agree_with_skip_paths() {
        use crate::collectors::config::SkipPaths;
        let mut patterns: Vec<String> = DriftConfig::default().routine_paths;
        patterns.extend(DriftConfig::default().always_red_paths);
        patterns.extend(
            [
                "*.key",
                "id_?sa",
                "**/tokens/*.json",
                "app/secret.conf",
                "~/.config/omarchy/private/",
                "a.b",
                "~",
                "~/.config/a?b",
            ]
            .map(String::from),
        );
        let subjects = [
            "~/.config/omarchy/shell.json",
            "~/.config/omarchy/shell.json.x",
            "~/.config/hypr/hyprland.lua.bak.1786539345",
            "~/.bak.x",
            "x.bak.1",
            "~/.config/systemd/user/a.service",
            "~/.config/systemd/user/default.target.wants/b.service",
            "~/.config/systemd/userx/a",
            "~/.config/systemd/user",
            "~/.config/omarchy/hooks/post-boot",
            "~/.config/omarchy/hooks/post-update.d/x.sh",
            "~/.profile",
            "~/.profile.d/x",
            "~/.profiles",
            "~/.bash_profile",
            "~/.config/x/server.key",
            "~/.config/x/server.keys",
            "~/.ssh/id_rsa",
            "~/.config/app/tokens/gh.json",
            "~/.config/app/tokens/sub/gh.json",
            "~/.config/app/secret.conf",
            "~/.config/myapp/secret.conf",
            "~/.config/omarchy/private/a/b",
            "~/.config/omarchy/privateer",
            "/x/aXb",
            "/x/a.b",
            "/etc/pacman.conf",
            "~",
            "~/.config/a/b",
            "~/.config/axb",
        ];
        let ours = PathGlobs::new(&patterns);
        for p in &patterns {
            let one = PathGlobs::new(std::slice::from_ref(p));
            let theirs = SkipPaths::new(Path::new(HOME_KEY), std::slice::from_ref(p));
            for s in subjects {
                let path = glob_path(s);
                assert_eq!(
                    one.matches(&path),
                    theirs.matches(Path::new(&path)),
                    "{p} on {s}"
                );
            }
        }
        assert!(ours.matches(&glob_path("~/.config/uwsm/env")));
    }

    #[test]
    fn glob_paths_read_the_home_as_a_key() {
        assert_eq!(glob_path("~/.profile"), "/~/.profile");
        assert_eq!(glob_path("~"), "/~");
        assert_eq!(glob_path("/etc/pacman.conf"), "/etc/pacman.conf");
        assert_eq!(glob_path("~x/y"), "~x/y");
    }
}
