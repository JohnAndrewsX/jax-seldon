//! `pacman` collector (SPEC-ENGINE §4): pacman.log → package events.
//!
//! Stub: registered so `capture` and the index know the name; WP-004
//! implements it.

use serde_json::Value;

use super::{Collector, Ctx, Outcome};

pub struct Pacman;

impl Collector for Pacman {
    fn name(&self) -> &'static str {
        "pacman"
    }

    fn collect(&self, _ctx: &Ctx, _cursor: Option<&Value>) -> Outcome {
        Outcome {
            ok: true,
            message: Some("not implemented yet (WP-004)".into()),
            ..Outcome::default()
        }
    }
}
