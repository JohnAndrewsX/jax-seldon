//! `config` collector (SPEC-ENGINE §4): sha256 manifest of watchPaths → config-* events.
//!
//! Stub: registered so `capture` and the index know the name; WP-005
//! implements it.

use serde_json::Value;

use super::{Collector, Ctx, Outcome};

pub struct ConfigFiles;

impl Collector for ConfigFiles {
    fn name(&self) -> &'static str {
        "config"
    }

    fn collect(&self, _ctx: &Ctx, _cursor: Option<&Value>) -> Outcome {
        Outcome {
            ok: true,
            message: Some("not implemented yet (WP-005)".into()),
            ..Outcome::default()
        }
    }
}
