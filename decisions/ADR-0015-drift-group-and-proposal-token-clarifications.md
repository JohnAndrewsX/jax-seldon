# ADR-0015 — Drift-group schema rules and the proposal token rule

**Status:** accepted (clarifies ADR-0013 §2 §3; supersedes ADR-0012 §13)
**Date:** 2026-10-01

## Context
WP-014 implemented ADR-0012 items 11–15 and ADR-0013 in the schema and the
fixture derivation and raised five readings that the ADRs left open. The
WP-014 reviewer confirmed all five as orchestrator calls and asked for one
of them (the token rule) to be decided in an ADR, not in a script.

## Decision
1. **`proposedCase` of a group** is the leader's proposal only. A case whose
   Plan names one upgraded package never proposes, and never fan-out-links,
   a whole full-upgrade transaction.
2. **`txId` and `members` appear together and only on groups**, and
   `members` requires `source: pacman`. Single-event pacman items carry
   neither. The plugin tests one thing: `members` present.
3. **The zone of every pacman drift item is computed** by the routine rule
   of ADR-0013 §3, single-event transactions included; a pacman event
   without `explicit` is never routine (errs red).
4. **Proposal token rule** (replaces ADR-0012 §13): the subject must match
   as a whole word in the case's `## Plan` text, case-sensitive, where word
   characters are `[A-Za-z0-9._+-]`, **except that a final `.` not followed
   by a word character is punctuation**, so a Plan line `Install zed.`
   proposes `zed`. `extra/zed` matches `zed` because `/` is not a word
   character. The reference implementation is the regex in
   `scripts/validate-fixtures.py`; the WP-007 golden test is the arbiter.
5. **`series.drift` counting:** a transaction opens once, in the ISO week of
   its earliest line; one resolution write (same `meta.txId`, `ts`, `actor`)
   counts once. A straggler that reappears after a rotation (ADR-0013 §4)
   is a new item and counts as a new opening.

## Consequences
No `contractVersion` bump; the schema already enforces 2. The fixture
derivation implements 4 with a self-check for the trailing-period case.
WP-007 and WP-008 implement 1, 3, 5 as written.
