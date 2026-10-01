# ADR-0021 — The index folds `case` from any resolution that carries one

**Status:** accepted (clarifies ADR-0012 §8)
**Date:** 2026-10-01

## Context
`seldon drift explain` (WP-008) creates a retroactive, completed case and
writes `explained` resolution lines that carry that case. ADR-0012 §8
folds `case` onto the index event only for `linked`, so an explained
event would show `resolution: explained` without the case the human can
open, although the case lists the event in `events:`. The fixture's
explained lines carry no case ("deliberately without a case") and are
unaffected.

## Decision
When the winning resolution line carries `case`, the index folds it onto
the event regardless of `resolution` (`linked` or `explained`). An
explained event without a case stays without one. `drift explain`
always creates the case (status completed, `created`/`started` on the
day of the earliest resolved event, `closed` today, zone from `--zone`
else the item's computed zone, risk default R1); `--only` and the group
fan-out apply as for `link`.

## Consequences
One line each in the index builder (`engine/src/index/build.rs`) and the
reference derivation (`scripts/validate-fixtures.py`), with a self-check
that an explained line with a case folds it. The plugin's EventRow shows
"explained · C-…" (WP-021). No `contractVersion` bump.
