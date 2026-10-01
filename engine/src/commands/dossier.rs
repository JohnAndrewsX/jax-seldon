//! `seldon dossier [--section …]` (SPEC-ENGINE §3, WP-035): rewrites the
//! generated fences of `system/*.md` from read-only host queries and the
//! ledger ([`crate::dossier`]).
//!
//! Only the selected fences are touched, and only their bodies: the text
//! outside the fences is the user's. A section whose query fails is
//! skipped with a warning and keeps its fences as they were. Files are
//! replaced atomically and only when their bytes change; a change is
//! committed as `seldon: dossier` and the index is rebuilt (it reads the
//! fences). Nothing is written to the ledger.

use std::collections::BTreeMap;

use clap::Args;
use serde_json::{Value, json};

use super::index::warnings_human;
use super::{Commit, Context, Output, autocommit};
use crate::dossier::query::{self, Hosts, Scope};
use crate::dossier::{self, FENCES, Facts, Fence, Files, Section};
use crate::error::Result;
use crate::index;
use crate::redact::Redactor;

#[derive(Debug, Clone, Default, Args)]
pub struct DossierArgs {
    /// Sections to write, comma-separated or repeated (default: all)
    #[arg(
        long = "section",
        value_name = "SECTION",
        value_enum,
        value_delimiter = ','
    )]
    pub sections: Vec<Section>,
}

/// What happened to one fence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Status {
    Written,
    Unchanged,
    Skipped,
}

impl Status {
    fn as_str(self) -> &'static str {
        match self {
            Status::Written => "written",
            Status::Unchanged => "unchanged",
            Status::Skipped => "skipped",
        }
    }
}

/// The numbers a run found (`None`: that section did not run or failed).
#[derive(Debug, Clone, Default)]
struct Counts {
    explicit: Option<usize>,
    pre_logbook: Option<usize>,
    omarchy_base: Option<usize>,
    total: Option<usize>,
    aur: Option<usize>,
    units: Option<usize>,
    plugins: Option<usize>,
}

pub fn run(ctx: &Context, args: DossierArgs) -> Result<Output> {
    let (config, logbook) = ctx.open_logbook()?;
    // host strings go through the user's `[redaction]` patterns too
    // (SPEC-ENGINE §7); an invalid pattern is a user error before any write
    let redactor = Redactor::with_patterns(&config.redaction.patterns)?;
    let lock = ctx.lock()?;
    let built = index::derive(ctx, &config, &logbook)?;
    let facts = dossier::facts(&built);
    let hosts = Hosts::from_env();
    let mut files = Files::read(&logbook.path("system"), logbook.meta.language)?;

    let mut warnings: Vec<String> = Vec::new();
    let mut counts = Counts::default();
    let today = ctx.now.date_naive().to_string();
    let selected: Vec<&Fence> = FENCES
        .iter()
        .filter(|f| Section::selects(&args.sections, f))
        .collect();
    let mut status: BTreeMap<&'static str, Status> = BTreeMap::new();
    let mut set = |files: &mut Files, fence: &Fence, content: Option<String>| {
        let s = match content {
            Some(c) if files.set(fence, &c) => Status::Written,
            Some(_) => Status::Unchanged,
            None => Status::Skipped,
        };
        status.insert(fence.name, s);
    };

    // the queries run once per section, the first time one of its fences
    // needs them
    let mut packages: Option<std::result::Result<query::Packages, String>> = None;
    for fence in selected {
        let body = files.body(fence.name);
        let content = match fence.name {
            "packages.summary" | "packages.history" | "packages.explicit" => {
                let p = packages.get_or_insert_with(|| {
                    let p = query::packages(&hosts.sources.pacman).map(|p| {
                        // classified by the real names, then redacted
                        let (lists, missing) = query::omarchy_packages(&hosts.omarchy_packages);
                        if !missing.is_empty() {
                            let missing: Vec<String> =
                                missing.iter().map(|m| m.display().to_string()).collect();
                            warnings.push(format!(
                                "packages: Omarchy's package list(s) {} not readable; their packages count as `user`",
                                missing.join(", ")
                            ));
                        }
                        p.classify(&lists).redacted(&redactor)
                    });
                    match &p {
                        Ok(p) => {
                            counts.explicit = Some(p.explicit.len());
                            counts.omarchy_base = Some(p.omarchy.len());
                            counts.pre_logbook = Some(
                                p.explicit
                                    .iter()
                                    .filter(|n| !facts.installs.contains_key(*n))
                                    .count(),
                            );
                            counts.total = Some(p.total);
                            counts.aur = Some(p.aur());
                        }
                        Err(e) => warnings.push(format!("packages: {e}; fences kept")),
                    }
                    p
                });
                p.as_ref().ok().map(|p| match fence.name {
                    "packages.summary" => dossier::packages_summary(p),
                    "packages.history" => dossier::packages_history(body.as_deref(), &today, p),
                    _ => dossier::packages_explicit(p, &facts.installs),
                })
            }
            "services.enabled" => {
                services(&hosts, &facts, &redactor, body.as_deref(), &mut warnings).map(
                    |(text, n)| {
                        counts.units = Some(n);
                        text
                    },
                )
            }
            "omarchy.summary" => {
                let text = dossier::omarchy_summary(
                    query::omarchy_version(&hosts.sources).map(|v| redactor.redact(&v)),
                    query::theme(&hosts.sources, &ctx.dirs.home).map(|t| redactor.redact(&t)),
                    &facts,
                    body.as_deref(),
                );
                if text.is_empty() {
                    warnings.push("omarchy: no version and no theme found; fence kept".into());
                }
                (!text.is_empty()).then_some(text)
            }
            "hardware.summary" => {
                let hw = query::hardware(&hosts.hardware_root).redacted(&redactor);
                if hw.is_empty() {
                    warnings.push(format!(
                        "hardware: nothing readable under {}; fence kept",
                        hosts.hardware_root.display()
                    ));
                }
                (!hw.is_empty()).then(|| dossier::hardware_summary(&hw, body.as_deref()))
            }
            "plugins.list" => match query::plugins(&hosts.sources.omarchy, &redactor) {
                // a shell that lists nothing is not a shell without plugins
                Ok(list)
                    if list.is_empty()
                        && body
                            .as_deref()
                            .is_some_and(|b| !crate::index::load::fence_table(b).is_empty()) =>
                {
                    warnings.push(
                        "plugins: omarchy plugin list --json returned no plugins; fence kept"
                            .into(),
                    );
                    None
                }
                Ok(list) => {
                    counts.plugins = Some(list.len());
                    Some(dossier::plugins_list(&list))
                }
                Err(e) => {
                    warnings.push(format!("plugins: {e}; fence kept"));
                    None
                }
            },
            "deviations.table" => {
                let cased: Vec<dossier::Cased> = facts
                    .cased_config
                    .iter()
                    .map(|c| dossier::Cased {
                        path: redactor.redact(&c.path),
                        ..c.clone()
                    })
                    .collect();
                Some(dossier::deviations_table(body.as_deref(), &cased))
            }
            other => unreachable!("fence {other} has no renderer"),
        };
        set(&mut files, fence, content);
    }

    let written = files.write()?;
    let commit = if written.is_empty() {
        Commit::Skipped("nothing changed")
    } else {
        let commit = autocommit(ctx, &config, &logbook, "dossier");
        index::rebuild_if_initialised(ctx);
        commit
    };
    drop(lock);

    warnings.extend(built.warnings);
    let mut human = if written.is_empty() {
        format!("Nothing changed ({} fence(s) checked)", status.len())
    } else {
        let n = status.values().filter(|s| **s == Status::Written).count();
        format!("Wrote {} ({n} fence(s) changed)", written.join(", "))
    };
    if let (Some(e), Some(pre), Some(base), Some(t), Some(a)) = (
        counts.explicit,
        counts.pre_logbook,
        counts.omarchy_base,
        counts.total,
        counts.aur,
    ) {
        human.push_str(&format!(
            "\nPackages: {e} explicit ({pre} from before the logbook, {base} on Omarchy's lists), {t} total, {a} AUR"
        ));
    }
    human.push_str(&commit.human());
    human.push_str(&warnings_human(&warnings));

    let sections: serde_json::Map<String, Value> = status
        .iter()
        .map(|(name, s)| (name.to_string(), json!(s.as_str())))
        .collect();
    Ok(Output::ok(
        human,
        json!({
            "files": written,
            "sections": sections,
            "counts": {
                "explicit": counts.explicit,
                "preLogbook": counts.pre_logbook,
                "omarchyBase": counts.omarchy_base,
                "total": counts.total,
                "aur": counts.aur,
                "units": counts.units,
                "plugins": counts.plugins,
            },
            "git": commit.json(),
            "warnings": warnings,
        }),
    ))
}

/// The `services.enabled` body and its row count; `None` (with a warning)
/// when either scope cannot be listed, so no row is lost.
fn services(
    hosts: &Hosts,
    facts: &Facts,
    redactor: &Redactor,
    body: Option<&str>,
    warnings: &mut Vec<String>,
) -> Option<(String, usize)> {
    let mut units = Vec::new();
    for scope in [Scope::System, Scope::User] {
        match query::enabled_units(&hosts.systemctl, scope) {
            Ok(list) => units.extend(list.into_iter().map(|u| (redactor.redact(&u), scope))),
            Err(e) => {
                warnings.push(format!("services: {e}; fence kept"));
                return None;
            }
        }
    }
    let n = units.len();
    Some((
        dossier::services_enabled(&units, &facts.unit_cases, body),
        n,
    ))
}
