//! `theme` collector (SPEC-ENGINE §4): current theme slug diff → theme-set.
//!
//! Stub: registered so `capture` and the index know the name; WP-005
//! implements it.

use serde_json::Value;

use super::{Collector, Ctx, Outcome};

pub struct Theme;

impl Collector for Theme {
    fn name(&self) -> &'static str {
        "theme"
    }

    fn collect(&self, _ctx: &Ctx, _cursor: Option<&Value>) -> Outcome {
        Outcome {
            ok: true,
            message: Some("not implemented yet (WP-005)".into()),
            ..Outcome::default()
        }
    }
}
