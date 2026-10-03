# ADR-0025 — The index clips long texts of events and drift items with a visible marker

**Status:** accepted
**Date:** 2026-10-04

## Context
CONTRACT.md rule 5 gives `index.json` a budget of < 1 MB. ADR-0020
capped `index.drift` at 200 items and deferred `meta.truncated` on the
assumption that only `drift` is realistically unbounded. The review of
0.1.1 found another unbounded part: texts. An event's `detail` may hold
4 096 characters, and the agent hook stores command lines up to that
length in `meta.command` (heredocs, long pipelines). 500 such events
made an index of about 2.1 MB, and `index --check` called it valid. The
plugin parses the whole file on the shell thread.

## Decision
This extends ADR-0020; its decisions stand.
- In `index.events` and `index.drift`, `detail`, `resolutionDetail` and
  every string value of `meta` are clipped to 256 bytes of JSON (escapes
  counted, marker included). The cut falls on a character boundary, and
  the text ends in `… (N more characters in the ledger)`, where N is the
  number of characters left out. `subject` is not clipped (at most 512
  characters by `event.schema.json`).
- The ledger keeps every text whole. The `ledger/*.md` views and the
  member events of `drift show` keep it whole too; `drift list` and the
  `item` of `drift show` come from `index.drift` and are clipped.
- Cases, decisions and memory topics are not cut.
- An index of 1 000 000 bytes or more makes the engine warn (`warnings`
  in `--json`, the warning lines of `index` and `status`, stderr after a
  writing command) and name the largest section.
- `meta.truncated` stays deferred: it is a new field and needs a contract
  bump.

## Consequences
- No schema change; `contractVersion` stays 1.
- The plugin may see the marker at the end of a text and shows it as
  text.
- With the counts of CONTRACT.md rule 4, events and drift stay bounded:
  500 events and 200 drift items with 4 096-character texts come to about
  520 KB (WP-076).
- The reference `derive()` in `scripts/validate-fixtures.py` does not
  clip yet (follow-up WP-077). Every fixture text is under 256 bytes, so
  the two agree today.
- Whether the plugin should see the over-budget warning needs a contract
  field; that is the operator's decision.
