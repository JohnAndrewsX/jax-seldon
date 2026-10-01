# ADR-0012 — Contract v1: schema clarifications and index derivation rules

**Status:** accepted (2026-10-01, with items 11–15 added at review; drift
grouping and zones are in ADR-0013 and ADR-0014)
**Date:** 2026-10-01

## Context
WP-002 reviewed `schema/*.json` against the specs while building a sample
logbook from which `fixtures/index.sample.json` must be derivable (the WP-007
golden test). The draft schema contradicted SPEC-LOGBOOK in one place, left
the index semantics of resolutions, drift and several derived fields open, and
accepted values no producer should write. The draft fixture had events that
could not come from any ledger. Nothing has been released, so contract v1 is
still a draft.

## Decision
`contractVersion` stays **1**. The schema is tightened and the rules below
become part of the contract.

**Schema changes**
1. `case.schema.json` accepts `type: "case"` (SPEC-LOGBOOK §3 frontmatter has
   it; `additionalProperties: false` rejected every case file). The index
   omits it.
2. Shared `$defs` in `event.schema.json` (`ulid`, `actor`, `caseId`) are
   referenced by case and index; ULIDs must start with `0`–`7`.
   `case.agents` items are `agent:<name>`, `case.events`/`proposedEvents`
   items are ULIDs, `area` is a slug, `steps` requires `total` and `done`.
3. `event`: `kind: correction` requires `refersTo`; a `linked` resolution
   requires `case`; `subject` is non-empty; `meta` key conventions are
   documented (`command`, `version`, `from`/`to`, `hashFrom`/`hashTo`,
   `type`, `cleanup`, `pairOf`).
4. `index`: every nested object is closed (`additionalProperties: false`);
   collector names, snapshot types (`single|pre|post`), heatmap `bySource`
   keys, risk keys, week labels (`YYYY-Www`), journal times and `drift.source`
   are enumerated or patterned; date and date-time fields carry `format`.
   `system.omarchy.repoHead` stays optional and is omitted on package installs
   (memory/host.md item 1).
5. Third-party input formats get their own schemas under `schema/external/`
   (snapper, `omarchy plugin list`/`catalog`, Claude Code hooks). They are not
   part of the contract and not covered by `contractVersion`.

**Semantics**
6. *Drift eligibility:* only events from system-changing collectors —
   `pacman`, `omarchy`, `plugins`, `theme`, `config` — can be drift (no `case`,
   no resolution). Snapshots, notes, hook `command` events and case events
   never are. (SPEC-ENGINE §5 says "each new event without case".)
7. *Proposals:* rule 3 of SPEC-ENGINE §5 matches against **open** cases
   (queued, active, verification), not only active ones; the subject must
   appear as a token in the case's `## Plan` section; the lowest case id
   wins. It is recomputed at index time, so it derives from the logbook.
8. *Resolutions in the index:* `index.events` lists every ledger event except
   `kind: resolution`; a resolution is folded onto its target as
   `resolution`, and as `case` when `linked`. Later resolutions win.
   Corrections stay visible as events.
9. *Case folders:* `verification` lives in `work/active/`, `dropped` in
   `work/completed/`; `index.cases.completed` holds completed and dropped
   cases, newest `closed` first, at most 50. Other groups are sorted by id.
10. *Derived fields* (all from the logbook; "today" = date of `generatedAt`;
    an event's day = the date part of its `ts`):
    - `events`: newest first by instant, ties by id descending.
    - `case.events` (frontmatter, engine-maintained): ids of non-`seldon`
      events attributed to the case, oldest first. `steps`: checkboxes in
      `## Plan`. `proposedEvents`: open drift proposed for the case.
    - `summary`: `activeCases` = status active only; `eventsToday`/`events7d`
      count index events (no resolutions) on today / the last 7 days.
    - `drift`: newest first; `crisis` iff `zone` is red.
    - `today`: journal of today and yesterday, entries from `## HH:MM · actor
      · case?` headings.
    - `decisions`: frontmatter of `decisions/ADR-*.md`, id descending.
    - `system`: dossier fences `omarchy.summary`, `packages.summary`,
      `plugins.list`, `deviations.table`; `snapshots` from ledger snapshot
      events not later deleted, newest first; `areas` alphabetical with
      case counts over all cases.
    - `memory`: `## ` headings of `memory/lessons.md`; other memory files by
      `updated` descending.
    - `series.heatmap`: 366 days ending today, oldest first, `bySource` only
      on days with events. `series.packages`: fence `packages.history`.
      `series.drift`: every ISO week from the first ledger week to today;
      opened = drift-eligible ledger lines without case, resolved =
      resolution lines. `series.risk`: all cases, keys R0–R3.
      `series.timeline`: omarchy `update` events (release), current
      snapshots, all non-dropped cases (`created` → `closed`), open crises;
      sorted by `ts` string, then kind.

**Added at review (orchestrator, 2026-10-01)**
11. *Resolution detail:* a folded event carries the index-only optional
    field `resolutionDetail` (the `detail` of the winning resolution), so
    the plugin can show *why* an event was explained or dismissed.
    Precedent for index-only fields: `case.path`, `case.steps`.
12. *Timeline timestamps:* `series.timeline[].ts` and `end` are plain
    strings in two forms — `YYYY-MM-DD` for case spans (`created` →
    `closed`), RFC 3339 date-time for snapshots, releases and crises. The
    schema description states both; the chart parses both.
13. *Proposal token rule (item 7):* "the subject appears as a token in `##
    Plan`" means a whole-word match of the subject against the Plan
    section's text, case-sensitive, where word characters are
    `[A-Za-z0-9._+-]`; the reference implementation is the regex in
    `scripts/validate-fixtures.py`, and the WP-007 golden test is the
    arbiter.
14. *Known gap:* an agent `command` without a case that produces no
    collector event is never drift (consequence of item 6). Acknowledged;
    see ADR-0014 §5.
15. *`meta.enabled`* (boolean) is a documented conventional key on
    `plugin-add` and `plugin-enable|disable` events.

## Consequences
- Every fixture case file now validates; `scripts/validate-fixtures.sh`
  checks all of the above and fails on any divergence between the sample
  logbook and `index.sample.json`.
- SPEC-ENGINE §5 (rules of 6 and 7) and SPEC-LOGBOOK §3 (case folders of 9)
  need a one-line edit each to match; the orchestrator owns docs/.
- Producers that add an index field now fail validation until the schema
  changes — that is the intent of a contract (CONTRACT.md, "Changing the contract").
- WP-004–007 implement these rules; the golden test (WP-007) is the final
  arbiter, and differences found there are fixed in this ADR's successor,
  not silently in the fixture.
