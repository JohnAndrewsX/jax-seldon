//! `plugins` collector (SPEC-ENGINE §4): omarchy plugin list --json diff → plugin-* events.
//!
//! Stub: registered so `capture` and the index know the name; WP-005
//! implements it.

use serde_json::Value;

use super::{Collector, Ctx, Outcome};

pub struct Plugins;

impl Collector for Plugins {
    fn name(&self) -> &'static str {
        "plugins"
    }

    fn collect(&self, _ctx: &Ctx, _cursor: Option<&Value>) -> Outcome {
        Outcome {
            ok: true,
            message: Some("not implemented yet (WP-005)".into()),
            ..Outcome::default()
        }
    }
}
