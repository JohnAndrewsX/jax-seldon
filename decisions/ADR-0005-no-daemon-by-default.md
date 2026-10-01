# ADR-0005 — No daemon by default; shell-timer driven capture

**Status:** accepted · **Date:** 2026-10-01

## Context
Capture must happen regularly. Options: a resident watcher process, a systemd
user timer, or a timer inside the plugin's `Service.qml` (as
`omarchy-agent-collectors` does: at shell start and every 15 minutes).

## Decision
- v1: `Service.qml` runs `seldon capture --all --json` at shell start and
  every 15 minutes, and after any panel action.
- The engine keeps per-source cursors so each run is cheap and idempotent.
- `seldon watch` (inotify on the logbook, feature-gated) and a systemd user
  unit are optional, documented, off by default.

## Consequences
Zero resident footprint when the shell is the only consumer. Agents and
scripts that need freshness call `seldon capture` themselves (hooks do).
