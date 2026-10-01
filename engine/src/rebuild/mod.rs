//! `outputs/REBUILD.md` (SPEC-ENGINE §3 `seldon rebuild`, WP-032): what a
//! fresh Omarchy install needs to reach the state the logbook documents.
//!
//! [`collect`] reads the index derivation ([`Built`]: folded events,
//! resolutions, drift items) and the dossier fences of `system/*.md`; it
//! never derives drift, cases or resolutions a second time.
//! [`render::text`] turns the sections into Markdown (English headings,
//! prose in the logbook language), and [`merge`] puts that into the
//! `rebuild` fence of the file, keeping user text outside it.
//!
//! Every item that comes from the ledger carries its event id, so each
//! line of the document traces back to a ledger line.

pub mod render;

use std::collections::BTreeMap;
use std::path::Path;

use crate::dossier;
use crate::index::build::is_open_drift;
use crate::index::load::{fence_kv, fence_table, fences};
use crate::index::model::{DriftItem, IndexEvent};
use crate::index::{Built, views};
use crate::model::event::{Kind, Resolution, Source};
use crate::pkgcmd::{self, Op};

/// The document, relative to the logbook root (SPEC-LOGBOOK §2).
pub const REL_PATH: &str = "outputs/REBUILD.md";

/// The name of the generated fence in [`REL_PATH`].
pub const FENCE: &str = "rebuild";

/// Config files under this prefix are systemd units (section 6, not 3).
const UNIT_PREFIX: &str = "~/.config/systemd/";

/// Where an item stands and where it comes from.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Why {
    pub case: Option<String>,
    pub case_title: Option<String>,
    pub resolution: Option<Resolution>,
    /// The reason: the dossier's, else the resolution's `detail`.
    pub reason: Option<String>,
    /// Still open drift (section 7 asks for a decision).
    pub open: bool,
    /// The actor, when an agent made the change.
    pub agent: Option<String>,
    /// The ledger event the item comes from.
    pub event: Option<String>,
}

/// Which `omarchy pkg` command installs a package.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    /// The logged command was `pacman -S`: `omarchy pkg add`.
    Repo,
    /// The logged command was `pacman -U <file>`, how an AUR helper
    /// installs what it built: `omarchy pkg aur add`.
    Aur,
    /// No command, or another one: the document says so.
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Package {
    pub name: String,
    pub version: Option<String>,
    pub origin: Origin,
    pub why: Why,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Deviation {
    /// `~`-relative path.
    pub path: String,
    /// The file was removed (its last event is `config-remove`).
    pub removed: bool,
    pub why: Why,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plugin {
    pub id: String,
    pub enabled: Option<bool>,
    /// `clonedFrom` of the dossier: made with `omarchy plugin clone`.
    pub cloned_from: Option<String>,
    /// `meta.url` of the `plugin-add` event, when a producer recorded it.
    pub url: Option<String>,
    pub why: Why,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Theme {
    pub name: String,
    pub why: Why,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    User,
    System,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unit {
    /// The unit file (`~/.config/systemd/…`) when a config event has it.
    pub path: Option<String>,
    pub unit: String,
    /// `path` is a drop-in (`<unit>.d/<file>`) of `unit`.
    pub drop_in: bool,
    /// The file existed before the logbook and was removed.
    pub removed: bool,
    pub scope: Scope,
    /// Listed as enabled in the dossier's `services.enabled`.
    pub enabled: bool,
    pub why: Why,
}

/// The unit a file under `~/.config/systemd/` belongs to, and whether it
/// is a drop-in: `…/foo.service.d/override.conf` → (`foo.service`, true).
pub fn unit_of(path: &str) -> (String, bool) {
    let mut parts = path.rsplit('/');
    let file = parts.next().unwrap_or(path);
    match parts.next().and_then(|dir| dir.strip_suffix(".d")) {
        Some(unit) if !unit.is_empty() => (unit.to_string(), true),
        _ => (file.to_string(), false),
    }
}

/// A dismissed change: "deliberately not reproduced".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dismissed {
    pub source: String,
    pub kind: String,
    pub subject: String,
    pub reason: Option<String>,
    /// Events of the same pacman transaction dismissed with it.
    pub members: usize,
    pub event: String,
}

/// Explicit packages that predate the logbook (the dossier's
/// `packages.explicit` lines marked `pre-logbook`): the user's own by
/// origin and name, Omarchy's (class `omarchy-base`) only counted.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Before {
    pub repo: Vec<String>,
    pub aur: Vec<String>,
    /// Pre-logbook packages Omarchy's package lists name.
    pub omarchy: usize,
    /// The Omarchy version those lists belong to (`omarchy.summary`, else
    /// the base of section 1).
    pub version: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Base {
    pub version: Option<String>,
    /// Day of the last update.
    pub updated: Option<String>,
    pub event: Option<String>,
}

/// Everything the document says, section by section.
#[derive(Debug, Clone, PartialEq)]
pub struct Rebuild {
    pub machine: String,
    /// The day the logbook began (`logbook.toml created`).
    pub since: String,
    /// `ts` of the newest event: the document's "as of".
    pub last_event: Option<String>,
    pub base: Base,
    /// `explicit` of the dossier's `packages.summary`.
    pub explicit_total: Option<i64>,
    /// `None` when the dossier has no `packages.explicit` fence, or an
    /// empty one (written by `seldon dossier`, WP-035).
    pub before: Option<Before>,
    pub packages: Vec<Package>,
    pub deviations: Vec<Deviation>,
    pub plugins: Vec<Plugin>,
    pub first_party_disabled: Vec<String>,
    pub theme: Option<Theme>,
    pub units: Vec<Unit>,
    /// The index's drift items (crises first when capped).
    pub open: Vec<DriftItem>,
    /// All open drift items, also those the index leaves out.
    pub open_total: usize,
    pub dismissed: Vec<Dismissed>,
}

/// The counts of `seldon rebuild --json`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct Counts {
    pub packages: usize,
    pub deviations: usize,
    pub plugins: usize,
    pub units: usize,
    pub open: usize,
}

impl Rebuild {
    pub fn counts(&self) -> Counts {
        Counts {
            packages: self.packages.len(),
            deviations: self.deviations.len(),
            plugins: self.plugins.len(),
            units: self.units.len(),
            open: self.open_total,
        }
    }
}

/// The generated fences of `system/*.md` by name, first one wins (as the
/// index reads them, SPEC-ENGINE §6); unreadable files are skipped.
pub fn read_dossier(root: &Path) -> BTreeMap<String, String> {
    let mut files: Vec<_> = std::fs::read_dir(root.join("system"))
        .map(|r| {
            r.filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| p.is_file() && p.extension().is_some_and(|e| e == "md"))
                .collect()
        })
        .unwrap_or_default();
    files.sort();
    let mut out = BTreeMap::new();
    for path in files {
        if let Ok(text) = std::fs::read_to_string(&path) {
            for (name, body) in fences(&text) {
                out.entry(name).or_insert(body);
            }
        }
    }
    out
}

/// Builds the sections from the index derivation and the dossier.
/// `since` is the day the logbook began; `title` looks up a case title
/// (the index caps completed cases, so the caller may read the file).
pub fn collect(
    built: &Built,
    dossier: &BTreeMap<String, String>,
    since: &str,
    title: impl Fn(&str) -> Option<String>,
) -> Rebuild {
    let why = |f: &IndexEvent| why_of(f, &title);
    // oldest first; `folded` is newest first with ties by id descending
    let chrono: Vec<&IndexEvent> = built.folded.iter().rev().collect();
    let dismissed_res = |f: &IndexEvent| f.event.resolution == Some(Resolution::Dismissed);
    let table = |name: &str| {
        dossier
            .get(name)
            .map(|f| fence_table(f))
            .unwrap_or_default()
    };
    let cell = |row: &BTreeMap<String, String>, key: &str| {
        row.get(key)
            .map(|v| v.trim())
            .filter(|v| !v.is_empty() && *v != "—" && *v != "-")
            .map(String::from)
    };

    // 1. base
    let summary = dossier
        .get("omarchy.summary")
        .map(|f| fence_kv(f))
        .unwrap_or_default();
    let base = match built
        .folded
        .iter()
        .find(|f| f.event.source == Source::Omarchy && f.event.kind == Kind::Update)
    {
        Some(f) => Base {
            version: f.event.meta.to.clone(),
            updated: Some(f.event.ts.date_naive().to_string()),
            event: Some(f.event.id.to_string()),
        },
        None => Base {
            version: summary.get("version").cloned(),
            updated: summary
                .get("lastUpdate")
                .and_then(|t| t.get(..10))
                .map(String::from),
            event: None,
        },
    };

    // 2. explicit installs not removed later
    let mut installed: BTreeMap<&str, &IndexEvent> = BTreeMap::new();
    for f in chrono.iter().filter(|f| f.event.source == Source::Pacman) {
        match f.event.kind {
            Kind::Install if f.event.explicit == Some(true) => {
                installed.insert(&f.event.subject, f);
            }
            Kind::Remove => {
                installed.remove(f.event.subject.as_str());
            }
            _ => {}
        }
    }
    let mut packages: Vec<Package> = installed
        .into_values()
        .filter(|f| !dismissed_res(f))
        .map(|f| Package {
            name: f.event.subject.clone(),
            version: f.event.meta.version.clone().or(f.event.detail.clone()),
            origin: origin(f.event.meta.command.as_deref()),
            why: why(f),
        })
        .collect();
    // by case, then name; packages without a case last
    packages.sort_by(|a, b| {
        (a.why.case.is_none(), &a.why.case, &a.name).cmp(&(
            b.why.case.is_none(),
            &b.why.case,
            &b.name,
        ))
    });

    // the last config event of each path, and whether the path existed
    // before the logbook saw it (a file added and removed again is gone)
    let mut config: BTreeMap<&str, (&IndexEvent, bool)> = BTreeMap::new();
    for f in chrono.iter().filter(|f| f.event.source == Source::Config) {
        let e = &f.event;
        if !matches!(
            e.kind,
            Kind::ConfigAdd | Kind::ConfigChange | Kind::ConfigRemove
        ) {
            continue;
        }
        let existed = config
            .get(e.subject.as_str())
            .map_or(e.kind != Kind::ConfigAdd, |(_, existed)| *existed);
        config.insert(&e.subject, (f, existed));
    }
    let config_live = |f: &IndexEvent, existed: bool| {
        !dismissed_res(f) && (f.event.kind != Kind::ConfigRemove || existed)
    };

    // 3. deviations: the dossier table, then config events it lacks
    let mut deviations = Vec::new();
    let mut listed: Vec<String> = Vec::new();
    for row in table("deviations.table") {
        let Some(path) = cell(&row, "path") else {
            continue;
        };
        let mut w = Why {
            case: cell(&row, "case").map(|c| c.trim_matches(['[', ']']).to_string()),
            reason: cell(&row, "reason"),
            ..Why::default()
        };
        let mut removed = false;
        if let Some((f, existed)) = config.get(path.as_str())
            && config_live(f, *existed)
        {
            let ev = why(f);
            removed = f.event.kind == Kind::ConfigRemove;
            w.case = w.case.or(ev.case);
            w.reason = w.reason.or(ev.reason);
            w.resolution = ev.resolution;
            w.open = ev.open;
            w.agent = ev.agent;
            w.event = ev.event;
        }
        w.case_title = w.case.as_deref().and_then(&title);
        listed.push(path.clone());
        deviations.push(Deviation {
            path,
            removed,
            why: w,
        });
    }
    for (path, (f, existed)) in &config {
        if path.starts_with(UNIT_PREFIX)
            || listed.iter().any(|p| p == path)
            || !config_live(f, *existed)
        {
            continue;
        }
        deviations.push(Deviation {
            path: path.to_string(),
            removed: f.event.kind == Kind::ConfigRemove,
            why: why(f),
        });
    }

    // 4. plugins: non-first-party rows of the dossier, plus plugins the
    // ledger added; the ledger's removal and enabled state win
    #[derive(Default)]
    struct Seen<'a> {
        add: Option<&'a IndexEvent>,
        removed: bool,
        enabled: Option<bool>,
    }
    let mut seen: BTreeMap<&str, Seen> = BTreeMap::new();
    for f in chrono.iter().filter(|f| f.event.source == Source::Plugins) {
        let s = seen.entry(&f.event.subject).or_default();
        match f.event.kind {
            Kind::PluginAdd => {
                s.add = Some(f);
                s.removed = false;
                s.enabled = f.event.meta.enabled.or(s.enabled);
            }
            Kind::PluginRemove => s.removed = true,
            Kind::PluginEnable => s.enabled = Some(true),
            Kind::PluginDisable => s.enabled = Some(false),
            _ => {}
        }
    }
    let plugin_rows = table("plugins.list");
    let yes = |v: Option<String>| v.map(|v| v == "yes");
    let mut ids: Vec<String> = plugin_rows
        .iter()
        .filter(|r| cell(r, "firstParty").as_deref() != Some("yes"))
        .filter_map(|r| cell(r, "id"))
        .collect();
    for (id, s) in &seen {
        if s.add.is_some() && !ids.iter().any(|i| i == id) {
            ids.push(id.to_string());
        }
    }
    let mut plugins = Vec::new();
    for id in ids {
        let row = plugin_rows
            .iter()
            .find(|r| cell(r, "id").as_deref() == Some(&id));
        let s = seen.get(id.as_str());
        if s.is_some_and(|s| s.removed || s.add.is_some_and(dismissed_res)) {
            continue;
        }
        let add = s.and_then(|s| s.add);
        plugins.push(Plugin {
            enabled: s
                .and_then(|s| s.enabled)
                .or_else(|| yes(row.and_then(|r| cell(r, "enabled")))),
            cloned_from: row.and_then(|r| cell(r, "clonedFrom")),
            url: add.and_then(|f| f.event.meta.extra.get("url")?.as_str().map(String::from)),
            why: add.map(why).unwrap_or_default(),
            id,
        });
    }
    // a clone's source is switched off by `omarchy plugin clone` itself
    let first_party_disabled = plugin_rows
        .iter()
        .filter(|r| cell(r, "firstParty").as_deref() == Some("yes"))
        .filter(|r| cell(r, "enabled").as_deref() == Some("no"))
        .filter_map(|r| cell(r, "id"))
        .filter(|id| {
            !plugins
                .iter()
                .any(|p| p.cloned_from.as_deref() == Some(id.as_str()))
        })
        .collect();

    // 5. theme: the last theme-set not dismissed, else the dossier's
    let theme = built
        .folded
        .iter()
        .find(|f| {
            f.event.source == Source::Theme && f.event.kind == Kind::ThemeSet && !dismissed_res(f)
        })
        .map(|f| Theme {
            name: f.event.subject.clone(),
            why: why(f),
        })
        .or_else(|| {
            summary.get("theme").map(|t| Theme {
                name: t.clone(),
                why: Why::default(),
            })
        });

    // 6. units: unit files from config events, and the dossier's enabled
    // units that a case set up
    let services = table("services.enabled");
    let enabled_as = |unit: &str, scope: &str| {
        services.iter().any(|r| {
            cell(r, "unit").as_deref() == Some(unit) && cell(r, "scope").as_deref() == Some(scope)
        })
    };
    let mut units = Vec::new();
    for (path, (f, existed)) in &config {
        if !path.starts_with(UNIT_PREFIX) || !config_live(f, *existed) {
            continue;
        }
        let (unit, drop_in) = unit_of(path);
        units.push(Unit {
            path: Some(path.to_string()),
            enabled: enabled_as(&unit, "user"),
            unit,
            drop_in,
            removed: f.event.kind == Kind::ConfigRemove,
            scope: Scope::User,
            why: why(f),
        });
    }
    for row in &services {
        let (Some(unit), Some(case)) = (cell(row, "unit"), cell(row, "case")) else {
            continue;
        };
        let scope = match cell(row, "scope").as_deref() {
            Some("user") => Scope::User,
            Some("system") => Scope::System,
            _ => continue,
        };
        if units.iter().any(|u| u.unit == unit && u.scope == scope) {
            continue;
        }
        let case = case.trim_matches(['[', ']']).to_string();
        units.push(Unit {
            path: None,
            unit,
            drop_in: false,
            removed: false,
            scope,
            enabled: true,
            why: Why {
                case_title: title(&case),
                case: Some(case),
                ..Why::default()
            },
        });
    }

    // 7. dismissed changes of state (version moves are left out: a fresh
    // install gets current versions anyway); a pacman transaction once,
    // with the count of every dismissed member (dependencies included)
    let mut tx_members: BTreeMap<&str, usize> = BTreeMap::new();
    for f in chrono.iter().filter(|f| dismissed_res(f)) {
        if let Some(tx) = pacman_tx(f) {
            *tx_members.entry(tx).or_default() += 1;
        }
    }
    let mut dismissed: Vec<Dismissed> = Vec::new();
    let mut seen_tx: Vec<&str> = Vec::new();
    for f in chrono.iter().filter(|f| dismissed_res(f)) {
        let e = &f.event;
        let stateful = match e.kind {
            Kind::Install => e.source != Source::Pacman || e.explicit == Some(true),
            Kind::Remove
            | Kind::PluginAdd
            | Kind::PluginRemove
            | Kind::PluginEnable
            | Kind::PluginDisable
            | Kind::ThemeSet
            | Kind::ConfigAdd
            | Kind::ConfigChange
            | Kind::ConfigRemove => true,
            _ => false,
        };
        if !stateful {
            continue;
        }
        let tx = pacman_tx(f);
        if let Some(t) = tx {
            if seen_tx.contains(&t) {
                continue;
            }
            seen_tx.push(t);
        }
        dismissed.push(Dismissed {
            source: e.source.as_str().to_string(),
            kind: e.kind.as_str().to_string(),
            subject: e.subject.clone(),
            reason: f.resolution_detail.clone().filter(|d| !d.trim().is_empty()),
            members: tx.and_then(|t| tx_members.get(t).copied()).unwrap_or(1),
            event: e.id.to_string(),
        });
    }

    let before = dossier
        .get("packages.explicit")
        .filter(|f| !f.trim().is_empty())
        .map(|f| {
            let mut before = Before {
                version: summary
                    .get("version")
                    .cloned()
                    .or_else(|| base.version.clone()),
                ..Before::default()
            };
            for p in dossier::parse_explicit(f)
                .into_iter()
                .filter(|p| p.pre_logbook)
            {
                if p.omarchy {
                    before.omarchy += 1;
                } else if p.aur {
                    before.aur.push(p.name);
                } else {
                    before.repo.push(p.name);
                }
            }
            before.repo.sort();
            before.aur.sort();
            before
        });

    let ix = &built.index;
    Rebuild {
        machine: ix.logbook.machine.clone(),
        since: since.to_string(),
        last_event: built
            .folded
            .first()
            .map(|f| crate::model::event::format_ts(&f.event.ts)),
        base,
        explicit_total: dossier
            .get("packages.summary")
            .and_then(|f| fence_kv(f).get("explicit")?.parse().ok()),
        before,
        packages,
        deviations,
        plugins,
        first_party_disabled,
        theme,
        units,
        open: ix.drift.clone(),
        open_total: ix.summary.open_drift,
        dismissed,
    }
}

/// The transaction of a pacman event.
fn pacman_tx(f: &IndexEvent) -> Option<&str> {
    f.event
        .tx_id
        .as_deref()
        .filter(|_| f.event.source == Source::Pacman)
}

fn why_of(f: &IndexEvent, title: &impl Fn(&str) -> Option<String>) -> Why {
    let e = &f.event;
    Why {
        case: e.case.clone(),
        case_title: e.case.as_deref().and_then(title),
        resolution: e.resolution,
        reason: f.resolution_detail.clone().filter(|d| !d.trim().is_empty()),
        open: is_open_drift(e),
        agent: e.actor.starts_with("agent:").then(|| e.actor.clone()),
        event: Some(e.id.to_string()),
    }
}

/// `omarchy pkg add` or `omarchy pkg aur add`, from pacman's logged
/// command line (`meta.command`, parsed by [`pkgcmd`]).
pub fn origin(command: Option<&str>) -> Origin {
    let Some(cmd) = command.and_then(|c| pkgcmd::parse_command(&pkgcmd::split_logged(c))) else {
        return Origin::Unknown;
    };
    match (cmd.program.as_str(), cmd.op) {
        ("pacman", Some(Op::Sync)) => Origin::Repo,
        ("pacman", Some(Op::Upgrade)) => Origin::Aur,
        _ => Origin::Unknown,
    }
}

/// The file with its `rebuild` fence replaced by `content`; everything
/// outside the fence is kept ([`views::merge_fence`]).
pub fn merge(existing: Option<&str>, content: &str) -> String {
    views::merge_fence(existing, FENCE, content)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::GENERATED_HEADER;

    #[test]
    fn origin_from_the_logged_command() {
        assert_eq!(origin(Some("pacman -S btop")), Origin::Repo);
        assert_eq!(
            origin(Some(
                "pacman -S --needed --noconfirm --config /etc/pacman.conf -- extra/zed"
            )),
            Origin::Repo
        );
        assert_eq!(
            origin(Some(
                "pacman -U --noconfirm --config /etc/pacman.conf -- /home/u/.cache/yay/brave-bin/brave-bin-1.80.1-1-x86_64.pkg.tar.zst"
            )),
            Origin::Aur
        );
        assert_eq!(origin(None), Origin::Unknown);
        assert_eq!(origin(Some("yay -S foo")), Origin::Unknown);
        assert_eq!(origin(Some("make install")), Origin::Unknown);
    }

    #[test]
    fn a_drop_in_belongs_to_its_unit() {
        assert_eq!(
            unit_of("~/.config/systemd/user/foo.service.d/override.conf"),
            ("foo.service".to_string(), true)
        );
        assert_eq!(
            unit_of("~/.config/systemd/user/ollama.service"),
            ("ollama.service".to_string(), false)
        );
    }

    #[test]
    fn merge_keeps_user_text_outside_the_fence() {
        let fresh = merge(None, "A\n");
        assert_eq!(
            fresh,
            format!("{GENERATED_HEADER}\n<!-- seldon:begin rebuild -->\nA\n<!-- seldon:end -->\n")
        );
        let edited = format!("{fresh}\n## My notes\nKeep.\n");
        let again = merge(Some(&edited), "B\n");
        assert_eq!(
            again,
            format!(
                "{GENERATED_HEADER}\n<!-- seldon:begin rebuild -->\nB\n<!-- seldon:end -->\n\n## My notes\nKeep.\n"
            )
        );
        assert_eq!(merge(Some(&again), "B\n"), again, "idempotent");
        // a hand-written file is kept below the fence
        let user = "# Mine\ntext\n";
        assert_eq!(
            merge(Some(user), "C\n"),
            format!("{}\n{user}", merge(None, "C\n"))
        );
        // an empty generated file is replaced
        assert_eq!(
            merge(Some(&format!("{GENERATED_HEADER}\n")), "D\n"),
            merge(None, "D\n")
        );
    }
}
