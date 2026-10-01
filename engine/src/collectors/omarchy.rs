//! `omarchy` collector (SPEC-ENGINE §4): omarchy-version → update events.
//!
//! Stub: registered so `capture` and the index know the name; WP-004
//! implements it.

use serde_json::Value;

use super::{Collector, Ctx, Outcome};

pub struct Omarchy;

impl Collector for Omarchy {
    fn name(&self) -> &'static str {
        "omarchy"
    }

    fn collect(&self, _ctx: &Ctx, _cursor: Option<&Value>) -> Outcome {
        Outcome {
            ok: true,
            message: Some("not implemented yet (WP-004)".into()),
            ..Outcome::default()
        }
    }
}
