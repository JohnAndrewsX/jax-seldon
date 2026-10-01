//! `theme` collector (SPEC-ENGINE §4): current theme slug diff → `theme-set`.
//!
//! `omarchy-theme-set` writes the slug of the new theme to
//! `~/.local/state/omarchy/current/theme.name` and then runs
//! `omarchy-hook theme-set <slug>` (memory/host.md item 4). The collector
//! reads that file; `omarchy theme current` only prints a prettified name.
//!
//! The cursor holds the slug and the time of the last check. Without a
//! cursor the collector takes a baseline (no event). A changed slug becomes
//! one `theme-set` event with `from`/`to`, timed by the file's mtime
//! (clamped to the window since the last check).
//!
//! The optional hook `engine/hooks/theme-set.sh` records the change when it
//! happens (`seldon event theme theme-set --subject <slug>`). If the ledger
//! already holds a `theme-set` to the current slug since the last check, the
//! collector writes nothing, so a change is never recorded twice.

use std::path::{Path, PathBuf};

use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::config::changed_at;
use super::{Collector, Ctx, Outcome, Sources, to_cursor, typed_cursor};
use crate::model::event::{Event, Kind, Meta, SUBJECT_MAX, Source};

/// Where Omarchy stores the current theme slug, relative to `$HOME`
/// (`omarchy-theme-set` hard-codes `$HOME/.local/state`, not XDG).
pub const THEME_FILE: &str = ".local/state/omarchy/current/theme.name";

pub struct Theme;

/// The theme collector's cursor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ThemeCursor {
    theme: String,
    /// Capture time of the last check.
    checked: DateTime<FixedOffset>,
}

impl Theme {
    /// The theme file: [`Sources::theme_file`] (`SELDON_THEME_FILE`), else
    /// Omarchy's under `home`.
    pub fn file(sources: &Sources, home: &Path) -> PathBuf {
        sources
            .theme_file
            .clone()
            .unwrap_or_else(|| home.join(THEME_FILE))
    }

    /// [`Collector::collect`] with an explicit theme file.
    pub fn collect_from(&self, ctx: &Ctx, cursor: Option<&Value>, file: &Path) -> Outcome {
        let shown = ctx.dirs.display(file);
        let text = match std::fs::read_to_string(file) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Outcome::degraded(format!("{shown} not found: no Omarchy theme set"), None);
            }
            Err(e) => return Outcome::degraded(format!("cannot read {shown}: {e}"), None),
        };
        let slug = text.trim();
        if slug.is_empty() || slug.chars().count() > SUBJECT_MAX || slug.contains('\n') {
            return Outcome::degraded(format!("{shown} does not hold a theme name"), None);
        }

        let next = to_cursor(&ThemeCursor {
            theme: slug.to_string(),
            checked: ctx.now,
        });
        let Some(prev) = typed_cursor::<ThemeCursor>(cursor) else {
            return Outcome::ok(Vec::new(), next); // baseline
        };
        if prev.theme == slug {
            return Outcome::ok(Vec::new(), next);
        }

        // the theme-set hook may have recorded this change already
        match ctx.ledger.read_range(prev.checked.min(ctx.now), ctx.now) {
            Ok(events)
                if events
                    .iter()
                    .any(|e| e.kind == Kind::ThemeSet && e.subject == slug) =>
            {
                return Outcome::ok(Vec::new(), next);
            }
            Ok(_) => {}
            Err(e) => return Outcome::degraded(format!("cannot read the ledger: {e:#}"), None),
        }

        let mtime = std::fs::metadata(file).and_then(|m| m.modified()).ok();
        let event = Event::new(
            changed_at(ctx, mtime, prev.checked),
            Source::Theme,
            Kind::ThemeSet,
            slug,
        )
        .detail(format!("{} → {slug}", prev.theme))
        .meta(Meta {
            from: Some(prev.theme),
            to: Some(slug.to_string()),
            ..Meta::default()
        });
        Outcome::ok(vec![event], next)
    }
}

impl Collector for Theme {
    fn name(&self) -> &'static str {
        "theme"
    }

    fn collect(&self, ctx: &Ctx, cursor: Option<&Value>) -> Outcome {
        self.collect_from(ctx, cursor, &Theme::file(ctx.sources, &ctx.dirs.home))
    }
}
