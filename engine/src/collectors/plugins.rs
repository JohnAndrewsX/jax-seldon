//! `plugins` collector (SPEC-ENGINE §4): `omarchy plugin list --json` diff →
//! `plugin-add|plugin-remove|plugin-enable|plugin-disable|plugin-update`.
//!
//! `omarchy plugin list --json` is the shell IPC call `listPlugins`
//! (`omarchy-shell shell listPlugins`): it fails when the Omarchy shell is
//! not running, and the collector then degrades (`ok: false`, the message of
//! `omarchy-shell`, cursor kept). Its entries carry `id` and `enabled` (for a
//! bar widget: placed in the bar) but no version, and neither does
//! `omarchy plugin catalog` (memory/host.md item 3). The version comes from
//! the plugin's manifest (ADR-0014 §3): the `manifestPath` that
//! `omarchy plugin catalog` reports for the id, else
//! `~/.config/omarchy/plugins/<id>/manifest.json`; when the manifest has no
//! `version`, the short git HEAD of the plugin directory (`plugin add` is a
//! `git clone`). A version that cannot be read this time keeps the last one
//! seen, so a failing catalog never looks like an update.
//!
//! `plugin-update` fires only for plugins with `firstParty: false`
//! (ADR-0018): first-party plugins ship inside the `omarchy` package, whose
//! `update` event covers them; their versions are tracked in the cursor
//! only. Enabling and disabling fire for every plugin.
//!
//! `plugin-add` and `plugin-update` are timed by the later mtime of the
//! plugin directory and its manifest, clamped to `[last check, now]`, so
//! attribution (ADR-0017) can match the agent command that cloned or
//! pulled it. Removal, enabling and disabling get the capture time.
//!
//! The cursor is the snapshot `{id: {enabled, version}}`, its SHA-256 and the
//! time of the last check. Without a cursor the collector takes a baseline
//! (no events). Events the ledger already holds since the last check (same
//! kind, id, version, enabled state, update step) are dropped: a capture
//! whose cursor save failed after its ledger write left the old snapshot in
//! the cursor, and the next diff would repeat them.

use std::collections::BTreeMap;
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::config::changed_at;
use super::{Collector, Ctx, Lost, Outcome, RUN_TIMEOUT, Sources, to_cursor, typed_cursor};
use crate::model::event::{Event, Kind, Meta, SUBJECT_MAX, Source};
use crate::sys::{self, Run};

/// Omarchy's user plugin directory relative to `$HOME` (the CLI hard-codes
/// `$HOME/.config`, not XDG).
pub const PLUGINS_DIR: &str = ".config/omarchy/plugins";

/// Longest version string recorded (a manifest is user content).
const VERSION_MAX: usize = 64;

/// Largest manifest read.
const MANIFEST_MAX: u64 = 1024 * 1024;

pub struct Plugins;

/// One entry of `omarchy plugin list --json` (other fields ignored).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Listed {
    pub id: String,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default, rename = "firstParty")]
    pub first_party: bool,
    /// The source id of a clone (`omarchy plugin clone`), else empty.
    #[serde(default, rename = "clonedFrom")]
    pub cloned_from: String,
}

/// One entry of `omarchy plugin catalog` (other fields ignored).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Cataloged {
    id: String,
    #[serde(default)]
    manifest_path: Option<PathBuf>,
}

/// A plugin as the cursor remembers it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct PluginState {
    enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    version: Option<String>,
}

/// The plugins collector's cursor: the last snapshot, its hash and the
/// time of the last check.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct PluginsCursor {
    hash: String,
    plugins: BTreeMap<String, PluginState>,
    /// Capture time of the last check (missing in older cursors: the
    /// capture time is used then).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    checked: Option<DateTime<FixedOffset>>,
}

impl PluginsCursor {
    fn new(plugins: BTreeMap<String, PluginState>, checked: DateTime<FixedOffset>) -> Self {
        let body = serde_json::to_vec(&plugins).expect("a snapshot always serialises");
        PluginsCursor {
            hash: sys::sha256_hex(&body),
            plugins,
            checked: Some(checked),
        }
    }
}

/// What this run saw of a plugin beyond its cursor state.
#[derive(Debug, Clone, Copy, Default)]
struct Seen {
    first_party: bool,
    /// Later mtime of the plugin directory and its manifest.
    touched: Option<SystemTime>,
}

impl Plugins {
    /// Omarchy's user plugin directory: [`Sources::plugins_dir`]
    /// (`SELDON_OMARCHY_PLUGINS_DIR`), else `~/.config/omarchy/plugins`.
    /// The config collector excludes it.
    pub fn dir(sources: &Sources, home: &Path) -> PathBuf {
        sources
            .plugins_dir
            .clone()
            .unwrap_or_else(|| home.join(PLUGINS_DIR))
    }

    /// [`Collector::collect`] with an explicit program and plugin directory.
    pub fn collect_from(
        &self,
        ctx: &Ctx,
        cursor: Option<&Value>,
        omarchy: &str,
        plugins_dir: &Path,
    ) -> Outcome {
        let prev = typed_cursor::<PluginsCursor>(cursor);
        let listed = match list(omarchy) {
            Ok(l) => l,
            Err(message) => return Outcome::degraded(message, None),
        };
        if listed.is_empty() && prev.as_ref().is_some_and(|p| !p.plugins.is_empty()) {
            // a shell that lists nothing is not a shell without plugins
            return Outcome::degraded("omarchy plugin list --json returned no plugins", None);
        }

        let manifests = catalog(ctx, omarchy);
        let mut snapshot = BTreeMap::new();
        let mut seen = BTreeMap::new();
        for p in listed {
            if p.id.is_empty() || p.id.chars().count() > SUBJECT_MAX {
                continue;
            }
            let fallback = plugins_dir.join(&p.id).join("manifest.json");
            let candidates: Vec<&PathBuf> = manifests
                .get(&p.id)
                .into_iter()
                .chain([&fallback])
                .collect();
            let found = candidates
                .iter()
                .find_map(|m| version(ctx, m).map(|v| (v, *m)));
            // the manifest that answered, else the first one that exists
            let manifest = found
                .as_ref()
                .map(|(_, m)| *m)
                .or_else(|| candidates.iter().copied().find(|m| m.exists()));
            let touched = manifest.and_then(|m| touched(m));
            let version = found.map(|(v, _)| v).or_else(|| {
                // unreadable this time: keep the last one seen
                prev.as_ref()?.plugins.get(&p.id)?.version.clone()
            });
            seen.insert(
                p.id.clone(),
                Seen {
                    first_party: p.first_party,
                    touched,
                },
            );
            snapshot.insert(
                p.id,
                PluginState {
                    enabled: p.enabled,
                    version,
                },
            );
        }
        let next = PluginsCursor::new(snapshot, ctx.now);

        let (events, since) = match &prev {
            Some(prev) if prev.hash != next.hash => {
                let since = prev.checked.unwrap_or(ctx.now);
                let events = diff(ctx, &prev.plugins, &next.plugins, &seen, since);
                match unrecorded(ctx, events, since) {
                    Ok(events) => (events, Some(since)),
                    Err(e) => {
                        return Outcome::degraded(format!("cannot read the ledger: {e:#}"), None);
                    }
                }
            }
            _ => (Vec::new(), None), // baseline, or nothing changed
        };
        Outcome {
            since,
            ..Outcome::ok(events, to_cursor(&next))
        }
        .baseline(prev.is_none().then_some(Lost::Cursor))
    }
}

/// `events` without those the ledger holds since `since`: same kind, id,
/// version, enabled state and update step. A capture whose cursor save
/// failed after its ledger write left the cursor on the snapshot before
/// them (the config collector does the same).
fn unrecorded(
    ctx: &Ctx,
    events: Vec<Event>,
    since: DateTime<FixedOffset>,
) -> anyhow::Result<Vec<Event>> {
    if events.is_empty() {
        return Ok(events);
    }
    let recorded: Vec<Event> = ctx
        .ledger
        .read_range(since.min(ctx.now), ctx.now)?
        .into_iter()
        .filter(|r| r.source == Source::Plugins)
        .collect();
    if recorded.is_empty() {
        return Ok(events);
    }
    // the ledger holds subject and meta values redacted
    let redactor = ctx.ledger.redactor();
    let same = |held: &Option<String>, new: &Option<String>| {
        held.as_deref() == new.as_deref().map(|v| redactor.redact(v)).as_deref()
    };
    Ok(events
        .into_iter()
        .filter(|e| {
            let subject = redactor.redact(&e.subject);
            !recorded.iter().any(|r| {
                r.kind == e.kind
                    && r.subject == subject
                    && r.meta.enabled == e.meta.enabled
                    && same(&r.meta.version, &e.meta.version)
                    && same(&r.meta.from, &e.meta.from)
                    && same(&r.meta.to, &e.meta.to)
            })
        })
        .collect())
}

/// `omarchy plugin list --json`, or the message to degrade with (also
/// read by `seldon dossier`).
pub fn list(omarchy: &str) -> Result<Vec<Listed>, String> {
    const WHAT: &str = "omarchy plugin list --json";
    match sys::run(omarchy, &["plugin", "list", "--json"], None, RUN_TIMEOUT) {
        Run::Exited {
            code: Some(0),
            stdout,
            ..
        } => serde_json::from_str(&stdout).map_err(|e| format!("{WHAT}: unexpected output ({e})")),
        Run::Exited { code, stderr, .. } => {
            let reason = stderr
                .lines()
                .map(str::trim)
                .find(|l| !l.is_empty())
                .map_or_else(
                    || {
                        format!(
                            "exit code {}",
                            code.map_or("none".into(), |c| c.to_string())
                        )
                    },
                    str::to_string,
                );
            Err(format!(
                "{WHAT}: {reason} (it needs the running Omarchy shell)"
            ))
        }
        Run::NotFound => Err(format!("{WHAT}: `{omarchy}` not found")),
        Run::TimedOut => Err(format!("{WHAT}: timed out")),
        Run::Failed(e) => Err(format!("{WHAT}: {e}")),
    }
}

/// id → manifest path from `omarchy plugin catalog`; empty if it fails (the
/// manifest fallback and the remembered versions cover that).
fn catalog(ctx: &Ctx, omarchy: &str) -> BTreeMap<String, PathBuf> {
    let Run::Exited {
        code: Some(0),
        stdout,
        ..
    } = ctx.run(omarchy, &["plugin", "catalog"])
    else {
        return BTreeMap::new();
    };
    serde_json::from_str::<Vec<Cataloged>>(&stdout)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|c| Some((c.id, c.manifest_path?)))
        .collect()
}

/// The plugin's version: `version` of the manifest at `manifest`, else the
/// short git HEAD of the manifest's directory when that is a git clone.
fn version(ctx: &Ctx, manifest: &Path) -> Option<String> {
    let from_manifest = std::fs::File::open(manifest).ok().and_then(|f| {
        let mut text = String::new();
        f.take(MANIFEST_MAX).read_to_string(&mut text).ok()?;
        let v: Value = serde_json::from_str(&text).ok()?;
        let version = v.get("version")?.as_str()?.trim();
        (!version.is_empty()).then(|| version.chars().take(VERSION_MAX).collect())
    });
    from_manifest.or_else(|| {
        // only the plugin's own clone; never a repository further up (a
        // dotfiles repo in ~/.config would answer for every plugin)
        let dir = manifest.parent()?;
        if !dir.join(".git").exists() {
            return None;
        }
        let dir = dir.to_str()?;
        match ctx.run("git", &["-C", dir, "rev-parse", "--short", "HEAD"]) {
            Run::Exited {
                code: Some(0),
                stdout,
                ..
            } => {
                let head = stdout.trim();
                (!head.is_empty() && head.chars().all(|c| c.is_ascii_hexdigit()))
                    .then(|| head.to_string())
            }
            _ => None,
        }
    })
}

/// The later mtime of `manifest` and its directory (a clone or a pull
/// touches at least one of them).
fn touched(manifest: &Path) -> Option<SystemTime> {
    let mtime = |p: &Path| std::fs::metadata(p).and_then(|m| m.modified()).ok();
    let dir = manifest.parent().and_then(mtime);
    mtime(manifest).max(dir)
}

/// Events for the step from `old` to `new`, ordered by plugin id. `seen`
/// holds what this run saw of the plugins in `new`; `since` is the last
/// check.
fn diff(
    ctx: &Ctx,
    old: &BTreeMap<String, PluginState>,
    new: &BTreeMap<String, PluginState>,
    seen: &BTreeMap<String, Seen>,
    since: DateTime<FixedOffset>,
) -> Vec<Event> {
    let seen_of = |id: &str| seen.get(id).copied().unwrap_or_default();
    // add and update: when the plugin directory changed (ADR-0017 window)
    let changed = |id: &str| changed_at(ctx, seen_of(id).touched, since);
    let event = |kind, id: &str, meta: Meta| {
        let ts = match kind {
            Kind::PluginAdd => changed(id),
            _ => ctx.now,
        };
        let mut e = Event::new(ts, Source::Plugins, kind, id);
        e.detail = meta.version.clone();
        e.meta(meta)
    };
    let mut ids: Vec<&String> = old.keys().chain(new.keys()).collect();
    ids.sort();
    ids.dedup();
    let mut events = Vec::new();
    for id in ids {
        match (old.get(id), new.get(id)) {
            (None, Some(n)) => events.push(event(
                Kind::PluginAdd,
                id,
                Meta {
                    version: n.version.clone(),
                    enabled: Some(n.enabled),
                    ..Meta::default()
                },
            )),
            (Some(o), None) => events.push(event(
                Kind::PluginRemove,
                id,
                Meta {
                    version: o.version.clone(),
                    ..Meta::default()
                },
            )),
            (Some(o), Some(n)) => {
                // first-party versions belong to the omarchy package (ADR-0018)
                if let (Some(from), Some(to)) = (&o.version, &n.version)
                    && from != to
                    && !seen_of(id).first_party
                {
                    events.push(
                        Event::new(
                            changed(id),
                            Source::Plugins,
                            Kind::PluginUpdate,
                            id.as_str(),
                        )
                        .detail(format!("{from} → {to}"))
                        .meta(Meta {
                            from: Some(from.clone()),
                            to: Some(to.clone()),
                            ..Meta::default()
                        }),
                    );
                }
                if o.enabled != n.enabled {
                    let kind = if n.enabled {
                        Kind::PluginEnable
                    } else {
                        Kind::PluginDisable
                    };
                    events.push(event(
                        kind,
                        id,
                        Meta {
                            version: n.version.clone(),
                            enabled: Some(n.enabled),
                            ..Meta::default()
                        },
                    ));
                }
            }
            (None, None) => {}
        }
    }
    events
}

impl Collector for Plugins {
    fn name(&self) -> &'static str {
        "plugins"
    }

    fn collect(&self, ctx: &Ctx, cursor: Option<&Value>) -> Outcome {
        self.collect_from(
            ctx,
            cursor,
            &ctx.sources.omarchy,
            &Plugins::dir(ctx.sources, &ctx.dirs.home),
        )
    }
}
