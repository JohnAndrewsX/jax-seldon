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

## Round 2 (review 1: APPROVE WITH FIXES)

Commit bf501c6d on a8369670. Round 1 above stays as it was; where it
names `GroupGap` or the one-line gap text, round 2 replaces them.

- **F1, the tail gap comes into view.** `ListColumn`: when the cursor
  lands on the last row and the list has a footer, `positionViewAtEnd()`
  (else `positionViewAtIndex(…, Contain)` as before). Every desk list
  without a footer keeps its old behaviour; Work's footer holds the gaps.
  Work's test view gets `atEnd` (`ListView.atYEnd`).
- **F2, the real cut shapes.** The model test now covers rungs 16–20
  (completed and queued cut to 0, active and verification listed,
  `queuedCases` 500: `QUEUED · 0 / 500` in `tail`, no `lead`), rung 21
  under way (1 of 40 active listed, verification cut: "ACTIVE · 1 / 40",
  the queued gap, WIP "40 / 3" urgent) and rung 21 run dry. The two
  `lead` cases are labelled hand-made (no ladder state); `lead` stays as
  defence. Desk-view: `work-cut-tail` (the rungs 16–20 shape with 40
  active rows, longer than the column: the cursor starts on the first
  row with the list not at its end, ↓×45 lands on the last row,
  C-2026-008, and the list is at its end with the header, the line and
  the command in view), `work-cut-dry`, and `work-cut-hand` (the old
  `work-cut`, renamed). The block is "8d"" (8d' was taken by WP-173).
- **F3, the three surviving mutants.** New assertions: `activeCases` 1
  with 2 listed gives 2; two all-empty indexes, one with only
  `queuedCases` 5 and one with only `activeCases` 5, each give `anyCase`
  true and `firstRun` false.
- **F4, the command on a line of its own.** The gap reads "The index
  lists none of them. This lists them:" and then `seldon plan list
  --status queued` (or `active`) as its own line: no wrap, elided at the
  right only if the column is narrower than the command; plain ASCII.
- **F5, one header style.** `GroupGap.qml` is now `GroupHeader.qml`: the
  header (dim small capitals), the optional line and the optional
  command. `GroupedRow` draws its own header with it (`visible` while
  `header` is set), so Today's, the Changelog's and Work's headers and
  the gaps share one definition. Its unused `tone` went with the Text.
  Small, so done here.
- **Handover Q1** (`firstRun` with only verification or completed rows,
  all cut): waits for WP-184 (contract 3), as the orchestrator decided.
- **SPEC-PLUGIN §Work**: the real shapes, the new wording, the scroll.

### How round 2 was verified

- **fixture** — `node tests/plugin/model.test.js`: 220 passed. Mutants
  (each restored, `cmp` ok), each fails the WP-206 test: review 1's
  three survivors (`activeCases || listed`; `anyCase` with only
  `whole.active`; with only `whole.queued`), WIP back to the list's
  length, no gaps, empty `tail`, no command.
- **headless** — a cut-down copy of desk-view (the harness frame and the
  WP-206 block only, in the private gates folder; own HOME, the
  harness's private 0700 runtime dir): every new case passes; with the
  `positionViewAtEnd` branch disabled, `work-cut-tail` #2 fails (`atEnd`
  false). Shot `tail-end.png` (private gates folder, not in the repo):
  "QUEUED · 0 / 500", the line and the command at the bottom of the
  column after ↓ to the last row.
- **headless** — `SELDON_FULL_CHECK=1 flock /tmp/seldon-check.lock just
  check` on bf501c6d (private HOME, TMPDIR, JUST_TEMPDIR,
  CARGO_TARGET_DIR and a private 0700 XDG_RUNTIME_DIR, removed afterwards):
  `check: ok` — model.test.js 220, desk-view 2268, service-states 367,
  bar-view 196, ipc-restart 44, qmllint 53 files, docs-check,
  validate-fixtures, check-guard and the engine gates, each 0 failed.
  Three earlier runs of the same commit without the lock, under parallel
  gates (load about 21 on 16 cores), were red, each on a different
  timing-sensitive case outside this diff: the guard's "hook input over
  256 KB" mutant survived (alone, `just check-guard` kills it); desk-view
  `cursor-follow` #1 and `aside-openfail`; then desk-view `triage-ask`.
  Each of those desk-view cases starts from the fake engine's live index,
  so its first step can run before the index arrives. The locked run is
  the gate (orchestrator decision: one full check per host).
- **test host** — shellcheck on `tests/plugin/desk-view.sh` against
  2181b679: the same 51 findings, none new.
- **not run**: CI, desktop.

Seen, not changed (not this WP): at 1920 px, Work's WIP line wraps to
four lines over the limit ("40 / 3", "active ·", "over the", "limit")
beside the three buttons; it is the same for any over-limit state.
