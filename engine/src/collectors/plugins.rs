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
//! The cursor is the snapshot `{id: {enabled, version}}` and its SHA-256.
//! Without a cursor the collector takes a baseline (no events).

use std::collections::BTreeMap;
use std::io::Read as _;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{Collector, Ctx, Outcome, to_cursor, typed_cursor};
use crate::model::event::{Event, Kind, Meta, SUBJECT_MAX, Source};
use crate::sys::{self, Run};

/// Overrides the `omarchy` program (tests, acceptance runs).
pub const OMARCHY_ENV: &str = "SELDON_OMARCHY";

/// Overrides Omarchy's user plugin directory (tests, acceptance runs).
pub const PLUGINS_DIR_ENV: &str = "SELDON_OMARCHY_PLUGINS_DIR";

/// Omarchy's user plugin directory relative to `$HOME` (the CLI hard-codes
/// `$HOME/.config`, not XDG).
pub const PLUGINS_DIR: &str = ".config/omarchy/plugins";

/// Longest version string recorded (a manifest is user content).
const VERSION_MAX: usize = 64;

/// Largest manifest read.
const MANIFEST_MAX: u64 = 1024 * 1024;

pub struct Plugins;

/// One entry of `omarchy plugin list --json` (other fields ignored).
#[derive(Debug, Deserialize)]
struct Listed {
    id: String,
    #[serde(default)]
    enabled: bool,
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

/// The plugins collector's cursor: the last snapshot and its hash.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct PluginsCursor {
    hash: String,
    plugins: BTreeMap<String, PluginState>,
}

impl PluginsCursor {
    fn new(plugins: BTreeMap<String, PluginState>) -> Self {
        let body = serde_json::to_vec(&plugins).expect("a snapshot always serialises");
        PluginsCursor {
            hash: sys::sha256_hex(&body),
            plugins,
        }
    }
}

impl Plugins {
    /// The `omarchy` program: `SELDON_OMARCHY`, else `omarchy` on PATH.
    pub fn program() -> String {
        std::env::var(OMARCHY_ENV)
            .ok()
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| "omarchy".to_string())
    }

    /// Omarchy's user plugin directory: `SELDON_OMARCHY_PLUGINS_DIR`, else
    /// `~/.config/omarchy/plugins`. The config collector excludes it.
    pub fn dir(home: &Path) -> PathBuf {
        std::env::var_os(PLUGINS_DIR_ENV)
            .filter(|v| !v.is_empty())
            .map_or_else(|| home.join(PLUGINS_DIR), PathBuf::from)
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
        let listed = match list(ctx, omarchy) {
            Ok(l) => l,
            Err(message) => return Outcome::degraded(message, None),
        };
        if listed.is_empty() && prev.as_ref().is_some_and(|p| !p.plugins.is_empty()) {
            // a shell that lists nothing is not a shell without plugins
            return Outcome::degraded("omarchy plugin list --json returned no plugins", None);
        }

        let manifests = catalog(ctx, omarchy);
        let mut snapshot = BTreeMap::new();
        for p in listed {
            if p.id.is_empty() || p.id.chars().count() > SUBJECT_MAX {
                continue;
            }
            let fallback = plugins_dir.join(&p.id).join("manifest.json");
            let version = manifests
                .get(&p.id)
                .into_iter()
                .chain([&fallback])
                .find_map(|m| version(ctx, m))
                .or_else(|| {
                    // unreadable this time: keep the last one seen
                    prev.as_ref()?.plugins.get(&p.id)?.version.clone()
                });
            snapshot.insert(
                p.id,
                PluginState {
                    enabled: p.enabled,
                    version,
                },
            );
        }
        let next = PluginsCursor::new(snapshot);

        let events = match &prev {
            Some(prev) if prev.hash != next.hash => diff(ctx, &prev.plugins, &next.plugins),
            _ => Vec::new(), // baseline, or nothing changed
        };
        Outcome::ok(events, to_cursor(&next))
    }
}

/// `omarchy plugin list --json`, or the message to degrade with.
fn list(ctx: &Ctx, omarchy: &str) -> Result<Vec<Listed>, String> {
    const WHAT: &str = "omarchy plugin list --json";
    match ctx.run(omarchy, &["plugin", "list", "--json"]) {
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

/// Events for the step from `old` to `new`, ordered by plugin id.
fn diff(
    ctx: &Ctx,
    old: &BTreeMap<String, PluginState>,
    new: &BTreeMap<String, PluginState>,
) -> Vec<Event> {
    let event = |kind, id: &str, meta: Meta| {
        let mut e = Event::new(ctx.now, Source::Plugins, kind, id);
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
                if let (Some(from), Some(to)) = (&o.version, &n.version)
                    && from != to
                {
                    events.push(
                        Event::new(ctx.now, Source::Plugins, Kind::PluginUpdate, id.as_str())
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
            &Plugins::program(),
            &Plugins::dir(&ctx.dirs.home),
        )
    }
}
