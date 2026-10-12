# WP-206 handover — counts from list lengths when the index is cut

Branch `wp/206-cut-counts`, base `next` 2181b679. Plugin only: no engine,
schema or fixture-contract change.

## What was done

- **Work keeps an empty group's header** (`plugin/Model.js` `workView`,
  `plugin/sections/Work.qml`, new `plugin/components/desk/GroupGap.qml`).
  A group the summary counts but the index lists no row of (ADR-0053 rung
  21: `cases.active` empty, `summary.activeCases` 40) is a *gap*:
  `workView` returns `gaps`, `lead[i]` (the gaps before shown row i) and
  `tail` (the gaps after the last row, or all of them when no row is
  listed). Work draws each as its header, "ACTIVE · 0 / 40", over one dim
  line: "The index lists none of them; seldon plan list --status active
  shows them." Before a row it sits inside that row's delegate
  (`GroupedRow.gaps`, above the header); after the last row in the list's
  footer (`ListColumn.footer`). It holds no row, so the cursor, the
  selection and `hoverIndex` are unchanged. The empty text "No cases
  yet…" is not shown while a tail gap stands there. During a search there
  is no gap (the search covers the listed rows, as WP-204 review 1 had
  it). Work's test view gets `gaps` (the gap headers).
- **`wipStatus`** counts `max(listed active, summary.activeCases)`: on rung
  21 the WIP line reads "40 / 3 active · over the limit", not "0 / 3".
- **`anyCase`** is true also when `summary.queuedCases` or
  `summary.activeCases` is above 0, so Today's first-run card does not
  show on a cut index that still counts open cases.
- **SPEC-PLUGIN** §5 Today (first-run card) and §Work (the gap, the WIP
  count) say the same.

### Deviation from the WP text

The WP names `seldon case list`; the engine has no `case` command. The
command that lists cases is `seldon plan list [--status S]`
(engine/src/commands/plan.rs, SPEC-ENGINE, AGENT-GUIDE), so the line names
`seldon plan list --status active` / `--status queued`.

## ADR-0053 §5's list-length counts, one by one

`summary` carries six counts: `activeCases`, `queuedCases`, `openDrift`,
`crisis`, `eventsToday`, `events7d`. A count can read the summary only
where one of these is the same thing.

| ADR-0053 §5 row | now | why |
|---|---|---|
| Work's column headers | summary for queued and active (WP-204); a group with none listed keeps its header and line (this WP) | verification and completed have no summary count; they stay the listed rows |
| `wipStatus` | summary (`activeCases`, never below the list) | this WP |
| `anyCase` / `firstRun` | summary for queued and active | this WP. Verification and completed have no summary count: a logbook with neither queued nor active cases whose verification and completed rows were all cut (rungs 14 and 20) and with no open drift would still show the card. That needs an index over budget with no open case and no open change; noted, not fixed (it needs a contract-3 count, WP-184) |
| `cases.verification` count (the header's KPI, the sidebar's middle Work figure) | stays the list's length | no summary count; the rows go only on rung 20, after every rung before it ran dry |
| proposed decisions (sidebar), `decisionSummary` total | stays | no summary count; proposed rows stay until rung 22 |
| Memory's sidebar count | stays | no summary count (rungs 6, 11, 12) |
| Yesterday toggle count | stays | no summary count (rung 7) |
| the Changelog's chips (open, attention, sources) | stays | a chip filters the listed rows: its figure must match the rows it shows (as the search's count against listed rows, WP-204 review 1). The whole open count is already shown where the plugin shows counts: the pill, the header's KPIs and the sidebar read `summary.openDrift`/`crisis`. The crisis chip loses nothing before rung 24 |
| Today's active case tiles | stays the listed rows | tiles are rows; there is nothing to draw for an unlisted case. Today's counts and sentence already read the summary |

## How it was verified

- **fixture** — `node tests/plugin/model.test.js` (own HOME and TMPDIR under
  the private gates folder): 220 passed. New test "WP-206: a cut index
  (rung 21) …": a whole index (no gap, WIP "2 / 3", `anyCase` true);
  rung 21 (ACTIVE · 0 / 40 and its line before the first listed row, WIP
  "40 / 3 active" urgent, `anyCase` true, By agent and search make no
  gap); rung 21 run dry (no row, both gaps in `tail`, `firstRun` false);
  a gap between rows (queued unlisted, completed listed); a summary below
  the list never lowers the WIP count.
- **fixture, mutations** (each restored, `cmp` ok): WIP back to the
  list's length, `anyCase` without the summary, no gaps, `tail` empty:
  each fails the new test. `<` → `<=` in the lead walk survives and is
  equivalent (a gap group never has a shown row).
- **headless** — `tests/plugin/desk-view.sh` (offscreen Quickshell, own
  HOME, private 0700 runtime dir removed afterwards): new cases
  `work-cut` (from the sample with jq in the harness's work dir: ACTIVE ·
  0 / 40 and its line shown, groups, "40 / 3 active · over the limit",
  the cursor on the first listed row) and `work-cut-dry` (no first-run
  card on Today; both headers and the queued line in Work, no "No cases
  yet"); every existing case unchanged (the whole sample renders as
  before). `desk-view: 2256 passed, 0 failed`.
- **headless** — `SELDON_FULL_CHECK=1 just check` on eae75d34 (private
  HOME, TMPDIR, JUST_TEMPDIR and CARGO_TARGET_DIR; rustup's and cargo's
  own homes passed through): `check: ok` — model.test.js 220, desk-view
  2256, service-states 366, bar-view 195, ipc-restart 43, qmllint 53
  files, plugin-validate, docs-check, validate-fixtures, the engine gates
  all ok, each 0 failed.
- **test host** — shellcheck (the orchestrator kit's wrapper) on
  `tests/plugin/desk-view.sh`, against the same file at 2181b679: the
  same 51 findings, none new. Locally `just check` has no shellcheck
  (bash -n only). The wrapper's default output crashes on a Unicode quote
  in an existing line of the script (`commitBuffer: invalid argument`);
  `-f gcc` runs through.
- **not run**: CI, desktop; on the test host only shellcheck ran.

## Open questions

1. `firstRun` on an index whose only cases are verification or completed
   rows that were all cut (table above): leave for contract 3 (WP-184)?
