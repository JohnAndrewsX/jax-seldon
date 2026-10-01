# ADR-0020 — The index lists at most 200 open drift items, crises first

**Status:** accepted
**Date:** 2026-10-01

## Context
CONTRACT.md rule 5 gives the index a 1 MB budget and reserves
`meta.truncated` for a future contract bump. WP-007 measured the index at
×150 of the fixture (10 050 events, 900 open cases, 600 open drift items):
897 KB, of which the open cases are 480 KB. Open cases are bounded by human
behaviour; the only realistically unbounded section is `drift` — a year
of ignored drift on a machine without a planning habit.

## Decision
- `index.drift` holds the newest **200** open drift items. Crises are
  never dropped before non-crises: the list is sorted crisis-first, then
  newest first, and cut at 200.
- `summary.openDrift` and `summary.crisis` always count **all** open
  items, so the pill and the strip stay truthful when the list is cut.
- Open case groups stay uncapped.
- `meta.truncated` (rule 5) is deferred until a measured real logbook
  exceeds 1 MB.

## Consequences
Additive: no new field, `contractVersion` stays 1; the schema may gain
`maxItems: 200` on `drift` later. CONTRACT.md rule 4 gains "newest 200
open drift items, crises first". WP-007 implements the cut with a test;
the plugin shows "+N more" from `summary.openDrift − drift.length` (WP-021).
