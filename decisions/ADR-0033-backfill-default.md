# ADR-0033 — A new logbook looks back 90 days and marks that history "before Seldon", without asking

**Status:** accepted (operator decision 2026-10-06)
**Date:** 2026-10-06

> Amends the wizard defaults of `seldon init` (WP-013, WP-024) and the
> baseline wording of ADR-0012. No contract change.

## Context

The first setup on a productive laptop (2026-10-06, release 0.1.3) asked
two questions a new user cannot answer: "Backfill since (YYYY-MM-DD; empty:
record from now on)" and "Mark them as the pre-Seldon baseline?". The
answer the operator gave — a month back, then dismiss everything as the
baseline — produced 1692 events and 82 drift items, all of them crises
under 0.1.3's rules, dismissed in the same breath. The history is useful
(the timeline, the dossier, "why is this here?"); the questions are not.
The UX review of that setup (Omakase, ADR-0027 §1) proposed taking the
answer as the default.

## Decision

1. A new logbook records the last **90 days** of what the collectors can
   read (package log, snapshots) on its first capture.
2. Every drift item that backfill opens is dismissed at once with the
   reason "before Seldon"; the events stay in the ledger and the
   timeline. Nothing from before the logbook ever asks for attention.
3. No question about either. `seldon init --ask` (the full wizard) and
   the existing flags (`--since`, `--baseline`, `--no-capture`) keep the
   choice for a user who wants it.
4. Output: one line, e.g. "Looked back 90 days: 1692 changes recorded as
   history before Seldon."

## Consequences

- The zero-question setup of the 0.1.5 flow WP (`seldon init --defaults`
  from the panel) uses these defaults; plain `seldon init` asks only for
  the logbook's location.
- With ADR-0028 most of the backfill is routine anyway; the dismissal
  covers the rest and keeps a new user's first look quiet.
- 90 days is a default, not a limit: older history stays readable with
  `--since`.
