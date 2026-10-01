//! `snapper` collector (SPEC-ENGINE §4): snapper --jsonout list → snapshot events.
//!
//! Stub: registered so `capture` and the index know the name; WP-004
//! implements it.

use serde_json::Value;

use super::{Collector, Ctx, Outcome};

pub struct Snapper;

impl Collector for Snapper {
    fn name(&self) -> &'static str {
        "snapper"
    }

    fn collect(&self, _ctx: &Ctx, _cursor: Option<&Value>) -> Outcome {
        Outcome {
            ok: true,
            message: Some("not implemented yet (WP-004)".into()),
            ..Outcome::default()
        }
    }
}
