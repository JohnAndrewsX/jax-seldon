//! Attribution: which agent command caused a collector event (ADR-0014 §1,
//! sharpened by ADR-0017 §2–§4; SPEC-ENGINE §4).
//!
//! A *cause* is a hook `command` event (`kind: command`, actor not
//! `system`) with a command line: what `seldon hook claude-code` or `hook
//! generic` recorded when the command *started*. A collector event takes
//! the cause's `actor` and `case` only with proof; time proximity alone is
//! never proof.
//!
//! - **pacman** and **omarchy** keep their own transaction-level logic in
//!   their collectors ([`find_cause`] is the shared part): a package must
//!   be named, a full upgrade only covers a full-upgrade transaction.
//! - **config**, **theme** and **plugins** go through [`attribute`], one
//!   pass that `seldon capture` runs over all events of a run before it
//!   appends them. The event must lie at most [`ATTRIBUTION_WINDOW`] after
//!   the command's start and not before it, and the command must prove it.
//!   An event stamped with the capture time (a plugin removal, enabling or
//!   disabling, a config removal) happened somewhere after the collector's
//!   last check: for it the command may start from [`ATTRIBUTION_WINDOW`]
//!   before that check up to the capture ([`attribute_capture`]), and it
//!   must be later than the newest recorded event of the same source and
//!   subject, so a command that already proved one change cannot claim a
//!   later one. Proof:
//!   - config: the event's path is a path the command *writes*
//!     ([`write_targets`]: a redirection, `tee`, `sed -i`, the destination
//!     of `cp|mv|install|ln` or a file directly in it, an `mv` source,
//!     `rm|truncate` operands, an `Edit`/`Write` hook event's path), written
//!     `~/…`, `$HOME/…`, absolute, or relative to the home directory. A path
//!     the command only reads (`cat x && pacman -S y`) is no proof;
//!   - theme: `omarchy theme set <name>` or `omarchy-theme-set <name>`,
//!     where `<name>` becomes the slug the way `omarchy-theme-set` makes it;
//!   - plugins: `omarchy plugin <verb> <id>` (or the
//!     `omarchy-plugin-<verb>` script) with the verb of the event's kind
//!     (`add` for `plugin-add` and `plugin-enable`, `remove`, `enable`,
//!     `disable`, `update` for their own kind), where the word is the id or
//!     a URL whose last path component is the id.
//!
//!   The latest proving command wins. Events that already carry an actor
//!   other than `system` or a case are left alone.
//!
//! Seldon's own changes ([`own_change`], SPEC-ENGINE §5 rule 8): its plugin
//! [`OWN_PLUGIN`] added, updated, enabled or disabled, its package
//! [`OWN_PACKAGE`] installed, upgraded, downgraded or reinstalled. The
//! event keeps the actor this pass gives it; `seldon capture` explains it
//! after the append, so it is no drift.

use std::path::{Component, Path, PathBuf};

use chrono::{DateTime, Duration, FixedOffset};

use crate::ledger::Ledger;
use crate::model::event::{ACTOR_SYSTEM, Event, Kind, Source};
use crate::pkgcmd::{Intent, parse_shell, segments_intent, simple_commands, write_targets};
use crate::redact::Redactor;

/// How long before a collector event (a pacman transaction's start) an
/// agent command still counts as its cause (ADR-0014 §1, ADR-0017 §2).
pub const ATTRIBUTION_WINDOW: Duration = Duration::minutes(10);

/// The plugin's own id (`plugin/manifest.json`).
pub const OWN_PLUGIN: &str = "jax.seldon";

/// The engine's own package (`packaging/PKGBUILD`, `pkgname`).
pub const OWN_PACKAGE: &str = "jax-seldon";

/// Why `e` is Seldon changing itself (SPEC-ENGINE §5 rule 8), as the
/// detail of its explanation; `None` for every other event. A removal is
/// not Seldon updating itself: it stays drift.
pub fn own_change(e: &Event) -> Option<&'static str> {
    match (e.source, e.kind) {
        (
            Source::Plugins,
            Kind::PluginAdd | Kind::PluginUpdate | Kind::PluginEnable | Kind::PluginDisable,
        ) if e.subject == OWN_PLUGIN => Some("seldon's own plugin"),
        (Source::Pacman, Kind::Install | Kind::Upgrade | Kind::Downgrade | Kind::Reinstall)
            if e.subject == OWN_PACKAGE =>
        {
            Some("seldon's own package")
        }
        _ => None,
    }
}

/// A hook command that can cause collector events.
#[derive(Debug, Clone)]
pub struct Cause {
    pub ts: DateTime<FixedOffset>,
    pub actor: String,
    pub case: Option<String>,
    /// What the command asks of the package manager.
    pub intent: Intent,
    /// The argv of each simple command (`sh -c '…'` and `eval` opened), and
    /// the files each one writes by redirection, as words (quotes removed).
    pub segments: Vec<(Vec<String>, Vec<String>)>,
}

/// Hook `command` events (any actor but `system`) with a command line.
pub fn causes(events: &[Event]) -> Vec<Cause> {
    events
        .iter()
        .filter(|e| e.kind == Kind::Command && e.actor != ACTOR_SYSTEM)
        .filter_map(|e| {
            let command = e.meta.command.as_deref()?;
            // the hook's own reading of the line (WP-071)
            let segments = simple_commands(&parse_shell(command));
            Some(Cause {
                ts: e.ts,
                actor: e.actor.clone(),
                case: e.case.clone(),
                intent: segments_intent(&segments),
                segments: segments
                    .iter()
                    .map(|s| (s.argv().to_vec(), s.writes.clone()))
                    .collect(),
            })
        })
        .collect()
}

/// The latest cause for `package` in a transaction that `began` at that
/// instant (ADR-0017 §2 §3): the command started at most 10 minutes before
/// `began` and not after it, and either names `package` while the
/// transaction's own command names it too (`named_by_tx`: `explicit` is not
/// `Some(false)`, so also when the transaction has no command line), or is a
/// full upgrade while the transaction's own command is one too
/// (`tx_full_upgrade`, also true when the transaction has no command line).
///
/// A naming command never reaches a package the transaction only pulled in
/// or upgraded along the way: an agent's `yay -S zed` does not make zed's
/// later upgrade by a human's plain `-Syu` the agent's. Such members get
/// the full-upgrade path and inheritance only.
pub fn find_cause<'c>(
    causes: &'c [Cause],
    package: &str,
    began: DateTime<FixedOffset>,
    named_by_tx: bool,
    tx_full_upgrade: bool,
) -> Option<&'c Cause> {
    causes
        .iter()
        .filter(|c| c.ts <= began && began - c.ts <= ATTRIBUTION_WINDOW)
        .filter(|c| {
            (named_by_tx && c.intent.packages.iter().any(|p| p == package))
                || (c.intent.full_upgrade && tx_full_upgrade)
        })
        .max_by_key(|c| c.ts)
}

/// Whether [`attribute`] decides this event's actor: a config, theme or
/// plugins event nobody has attributed yet.
fn open_to_attribution(e: &Event) -> bool {
    matches!(e.source, Source::Config | Source::Theme | Source::Plugins)
        && e.actor == ACTOR_SYSTEM
        && e.case.is_none()
}

/// When the events of one capture happened. A collector stamps a change
/// it cannot time (a removal, enabling, disabling) with the capture time
/// `now`; such a change happened after the collector's last check
/// (`since`, per source), so its cause may have started up to
/// [`ATTRIBUTION_WINDOW`] before that check.
#[derive(Debug, Clone, Default)]
pub struct Stamps {
    pub now: Option<DateTime<FixedOffset>>,
    pub since: Vec<(Source, DateTime<FixedOffset>)>,
}

impl Stamps {
    /// The earliest instant `e`'s change may have happened: the last check
    /// of its collector when `e` carries the capture time, else `e.ts`.
    fn earliest(&self, e: &Event) -> DateTime<FixedOffset> {
        if self.now != Some(e.ts) {
            return e.ts;
        }
        self.since
            .iter()
            .find(|(source, _)| *source == e.source)
            .map_or(e.ts, |(_, since)| (*since).min(e.ts))
    }

    /// Whether `cause` lies in `e`'s window: not after `e`, and at most
    /// [`ATTRIBUTION_WINDOW`] before the earliest instant of its change.
    fn in_window(&self, cause: &Cause, e: &Event) -> bool {
        cause.ts <= e.ts && self.earliest(e) - cause.ts <= ATTRIBUTION_WINDOW
    }

    /// For an event that carries the capture time: the newest event in
    /// `known` with its source and subject (`subject` as the ledger holds
    /// it), not after it. A cause must be later than that event: a command
    /// that came before the last recorded change of the subject cannot
    /// have caused a newer one (an agent's `omarchy plugin disable x`,
    /// recorded, then a person enables and disables `x` again within the
    /// wider window: the second disabling is not the agent's).
    fn newest_known(
        &self,
        e: &Event,
        subject: &str,
        known: &[Event],
    ) -> Option<DateTime<FixedOffset>> {
        if self.now != Some(e.ts) {
            return None;
        }
        known
            .iter()
            .filter(|k| k.source == e.source && k.subject == subject && k.ts <= e.ts)
            .map(|k| k.ts)
            .max()
    }
}

/// The shared pass (see the module docs): sets `actor` and `case` of each
/// config, theme and plugins event in `events` from the latest cause in
/// `known` that proves it. `home` resolves the paths a command names.
pub fn attribute(events: &mut [Event], known: &[Event], home: &Path) {
    attribute_stamped(events, known, home, &Stamps::default());
}

/// [`attribute`] for the events of one capture ([`Stamps`]).
pub fn attribute_stamped(events: &mut [Event], known: &[Event], home: &Path, stamps: &Stamps) {
    attribute_in(events, known, home, stamps, None);
}

/// [`attribute_stamped`]; `redactor` gives an event's subject as the
/// ledger in `known` holds it.
fn attribute_in(
    events: &mut [Event],
    known: &[Event],
    home: &Path,
    stamps: &Stamps,
    redactor: Option<&Redactor>,
) {
    let causes = causes(known);
    if causes.is_empty() {
        return;
    }
    for e in events.iter_mut().filter(|e| open_to_attribution(e)) {
        let subject = redactor.map_or_else(|| e.subject.clone(), |r| r.redact(&e.subject));
        let after = stamps.newest_known(e, &subject, known);
        let cause = causes
            .iter()
            .filter(|c| stamps.in_window(c, e))
            .filter(|c| after.is_none_or(|t| c.ts > t))
            .filter(|c| proves(c, e, home))
            .max_by_key(|c| c.ts);
        if let Some(c) = cause {
            e.actor = c.actor.clone();
            e.case = c.case.clone();
        }
    }
}

/// [`attribute`] with the causes read from `ledger`: the hook events from
/// [`ATTRIBUTION_WINDOW`] before the earliest open event up to the latest.
pub fn attribute_from_ledger(
    ledger: &Ledger,
    events: &mut [Event],
    home: &Path,
) -> anyhow::Result<()> {
    attribute_capture(ledger, events, home, &Stamps::default())
}

/// [`attribute_from_ledger`] for the events of one capture: an event
/// stamped with the capture time reaches back to [`ATTRIBUTION_WINDOW`]
/// before its collector's last check ([`Stamps`]).
pub fn attribute_capture(
    ledger: &Ledger,
    events: &mut [Event],
    home: &Path,
    stamps: &Stamps,
) -> anyhow::Result<()> {
    let open = events.iter().filter(|e| open_to_attribution(e));
    let (Some(first), Some(last)) = (
        open.clone().map(|e| stamps.earliest(e)).min(),
        open.map(|e| e.ts).max(),
    ) else {
        return Ok(());
    };
    let known = ledger.read_range(first - ATTRIBUTION_WINDOW, last)?;
    attribute_in(events, &known, home, stamps, Some(ledger.redactor()));
    Ok(())
}

/// Whether `cause` names what `event` is about (per source, module docs).
fn proves(cause: &Cause, event: &Event, home: &Path) -> bool {
    match event.source {
        Source::Config => {
            let path = normalise(Path::new(&home_path(&event.subject, home)));
            cause.segments.iter().any(|(argv, writes)| {
                write_targets(argv, writes)
                    .iter()
                    .any(|t| t.covers(&normalise(Path::new(&home_path(t.word(), home))), &path))
            })
        }
        Source::Theme => cause.segments.iter().any(|(argv, _)| {
            omarchy_route(argv, "theme", &["set"]).is_some_and(|args| {
                args.iter()
                    .any(|a| !a.starts_with('-') && theme_slug(a) == event.subject)
            })
        }),
        Source::Plugins => cause.segments.iter().any(|(argv, _)| {
            omarchy_route(argv, "plugin", plugin_verbs(event.kind))
                .is_some_and(|args| args.iter().any(|a| names_plugin(a, &event.subject)))
        }),
        _ => false,
    }
}

/// The `omarchy plugin` verbs that prove a plugins event of `kind`: its
/// own, and `add` also for `plugin-enable` (`plugin add --enable` of a
/// plugin removed since the last check).
fn plugin_verbs(kind: Kind) -> &'static [&'static str] {
    match kind {
        Kind::PluginAdd => &["add"],
        Kind::PluginRemove => &["remove"],
        Kind::PluginEnable => &["enable", "add"],
        Kind::PluginDisable => &["disable"],
        Kind::PluginUpdate => &["update"],
        _ => &[],
    }
}

/// The arguments after `omarchy <group> <verb>` or `omarchy-<group>-<verb>`
/// when `verb` is one of `verbs`.
pub fn omarchy_route<'a>(argv: &'a [String], group: &str, verbs: &[&str]) -> Option<&'a [String]> {
    let (program, rest) = argv.split_first()?;
    let program = program.rsplit('/').next().unwrap_or(program);
    if program == "omarchy" {
        match rest {
            [g, v, args @ ..] if g == group && verbs.contains(&v.as_str()) => Some(args),
            _ => None,
        }
    } else {
        let verb = program.strip_prefix("omarchy-")?.strip_prefix(group)?;
        let verb = verb.strip_prefix('-')?;
        verbs.contains(&verb).then_some(rest)
    }
}

/// The slug `omarchy-theme-set` makes of a theme name: tags like `<b>`
/// removed, lowercase, spaces as `-`.
pub fn theme_slug(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut in_tag = false;
    for c in name.chars() {
        match c {
            '<' => in_tag = true,
            '>' if in_tag => in_tag = false,
            _ if in_tag => {}
            ' ' => out.push('-'),
            c => out.extend(c.to_lowercase()),
        }
    }
    out
}

/// `word` is the plugin `id`, or a URL or path whose last component is the
/// id (`.git` and a trailing `/` aside).
fn names_plugin(word: &str, id: &str) -> bool {
    let w = word.trim_end_matches('/');
    let w = w.strip_suffix(".git").unwrap_or(w);
    w == id || (w.contains('/') && w.rsplit('/').next() == Some(id))
}

/// A path word as an absolute path string: `~/x`, `$HOME/x`, `${HOME}/x`
/// and `x` (relative to the home directory) under `home`; absolute paths
/// as they are.
pub fn home_path(word: &str, home: &Path) -> String {
    let home = home.to_string_lossy();
    let rest = ["~/", "$HOME/", "${HOME}/"]
        .iter()
        .find_map(|p| word.strip_prefix(p));
    match rest {
        Some(rest) => format!("{home}/{rest}"),
        None if word == "~" || word == "$HOME" || word == "${HOME}" => home.into_owned(),
        None if word.starts_with('/') => word.to_string(),
        None => format!("{home}/{word}"),
    }
}

/// `path` with `.` and `..` resolved lexically (symlinks are not followed).
pub fn normalise(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in path.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugs_like_omarchy_theme_set() {
        assert_eq!(theme_slug("Tokyo Night"), "tokyo-night");
        assert_eq!(theme_slug("<i>Kanagawa</i>"), "kanagawa");
        assert_eq!(theme_slug("tokyo-night"), "tokyo-night");
    }

    #[test]
    fn own_changes_are_seldons_plugin_and_package_but_no_removal() {
        let at = DateTime::parse_from_rfc3339("2026-10-01T10:00:00+02:00").unwrap();
        let own = |source, kind, subject: &str| own_change(&Event::new(at, source, kind, subject));
        for kind in [
            Kind::PluginAdd,
            Kind::PluginUpdate,
            Kind::PluginEnable,
            Kind::PluginDisable,
        ] {
            assert_eq!(
                own(Source::Plugins, kind, OWN_PLUGIN),
                Some("seldon's own plugin"),
                "{kind:?}"
            );
        }
        for kind in [
            Kind::Install,
            Kind::Upgrade,
            Kind::Downgrade,
            Kind::Reinstall,
        ] {
            assert_eq!(
                own(Source::Pacman, kind, OWN_PACKAGE),
                Some("seldon's own package"),
                "{kind:?}"
            );
        }
        // removing Seldon is somebody's change to the system
        assert_eq!(own(Source::Plugins, Kind::PluginRemove, OWN_PLUGIN), None);
        assert_eq!(own(Source::Pacman, Kind::Remove, OWN_PACKAGE), None);
        // other plugins and packages, and the names in another source
        for (source, kind, subject) in [
            (
                Source::Plugins,
                Kind::PluginUpdate,
                "io.github.example.tyme",
            ),
            (Source::Plugins, Kind::PluginUpdate, "jax.seldon-extra"),
            (Source::Plugins, Kind::PluginUpdate, OWN_PACKAGE),
            (Source::Pacman, Kind::Upgrade, "jax-seldon-git"),
            (Source::Pacman, Kind::Upgrade, OWN_PLUGIN),
            (Source::Config, Kind::ConfigChange, OWN_PLUGIN),
            (Source::Omarchy, Kind::Upgrade, OWN_PACKAGE),
        ] {
            assert_eq!(own(source, kind, subject), None, "{source:?} {subject}");
        }
    }

    #[test]
    fn plugin_words() {
        let id = "io.github.example.tyme";
        assert!(names_plugin(id, id));
        assert!(names_plugin(
            "https://github.com/example/io.github.example.tyme.git",
            id
        ));
        assert!(names_plugin(
            "https://example.org/x/io.github.example.tyme/",
            id
        ));
        assert!(!names_plugin("https://github.com/example/tyme", id));
        assert!(!names_plugin("io.github.example.tyme-extra", id));
    }

    #[test]
    fn path_words() {
        let home = Path::new("/home/user");
        for w in [
            "~/.config/hypr/a.conf",
            "$HOME/.config/hypr/a.conf",
            "${HOME}/.config/hypr/a.conf",
            "/home/user/.config/hypr/a.conf",
            ".config/hypr/a.conf",
            "~/.config/hypr/../hypr/./a.conf",
        ] {
            assert_eq!(
                normalise(Path::new(&home_path(w, home))),
                Path::new("/home/user/.config/hypr/a.conf"),
                "{w}"
            );
        }
    }

    #[test]
    fn routes() {
        let argv = |s: &str| -> Vec<String> { s.split(' ').map(String::from).collect() };
        let a = argv("omarchy theme set tokyo-night");
        assert_eq!(omarchy_route(&a, "theme", &["set"]), Some(&a[3..]));
        let a = argv("omarchy-theme-set Tokyo");
        assert_eq!(omarchy_route(&a, "theme", &["set"]), Some(&a[1..]));
        let a = argv("omarchy theme list");
        assert_eq!(omarchy_route(&a, "theme", &["set"]), None);
        let a = argv("omarchy-theme-set-gnome x");
        assert_eq!(omarchy_route(&a, "theme", &["set"]), None);
    }
}
