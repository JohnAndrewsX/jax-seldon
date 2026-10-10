# WP-123 — Handover: Decisions, System, Memory; Prime Radiant inside the desk

Branch `wp/123-sections-system` from `next` (38a9103, WP-121's desk
shell); merge into `next`. Plan and the decisions the WP left open:
[PLAN.md](PLAN.md).

## What was done

- **`sections/ReadingSection.qml`** — the frame of the three reading
  sections: a `ListView` of rows from a `Model.js` row function, narrowed
  by the sidebar search (`Model.deskFilter`), and the detail of the
  selected row under its sticky action bar. Selection = cursor (as the
  prototype): ↑/↓ j/k move it, the detail follows, a click selects, Enter
  shows the detail in the stacked layout; it stays on its row by id
  across index updates. `select` (IPC, payload) works on every row.
- **`sections/Decisions.qml`** — rows newest first (proposed striped),
  count and *New decision* above; detail with heading, title (superseded
  struck through), what Accept means, Status / Date / File, the CASES
  block when the index carries `decisions[].cases` (contract v2; hidden
  otherwise; a click goes to the case in Work); sticky bar *Accept* (only
  while proposed) and *Open in editor*. `d` / *New decision* show
  `components/desk/NewDecisionForm.qml` (the 0.1 `NewDecisionSheet`,
  moved and restyled; same arm-twice logic, same `Service.decide`) in the
  detail pane; the selection follows the created decision.
- **`sections/System.qml`** — five tiles (`Model.systemTiles`: Omarchy,
  Packages, Snapshots, Deviations, Collectors) → detail with the big value,
  unit, lead, key/value rows, "From the dossier; rebuilt on every
  capture."; *Open in editor* / `e` → STATUS.md.
- **`sections/Memory.qml`** — lessons and topics (`Model.memoryRows`) →
  detail with file and date; *Open in editor* / `e` → the logbook folder.
- **`sections/Radiant.qml`** — the six charts of `components/overlay/`
  unchanged, `OverlaySlot`, `ChartCanvas`, `Model.periodView` and
  `Model.overlayGrid` at the section's own size; row 1 title, the window
  caption and the period chips; 90 d on every entry (a payload's period
  wins); ←/→ and h/l walk the periods; `view()` reports period, window,
  grid mode and area, aggregation passes and the six slots like the
  overlay did. **`Desk.qml`**: `setPeriod` (shows section 7) and `hover`
  (only while shown) with the overlay's names; ←/→ go to the section
  (`Section.moveAcross`).
- **Header height at once** (`Header.qml`, `KpiStrip.figureHeight`): the
  header took its height from the KPI `Row`, which a `Row` sets only in
  its polish, one frame late — everything under the header moved 8 px
  after the first frame and every chart painted twice. Now the strip's
  height comes from its fonts; the counters match the old overlay.
- **`Model.js`** — `deskFilter`, `decisionCases`, `decisionDetail`,
  `systemTiles`, `memoryDetail` (in the Decisions/Memory block, not at the
  end, to keep the merge with WP-122 apart); `memoryRows` rows carry
  `path` and `updated`. Removed the overlay-only `overlayBanner`,
  `overlayMeta`, `overlayPayloadPeriod`, `periodForKey` (the desk's
  notices, header and `deskPayload` replaced them) and their tests. New
  unit tests for every new function.
- **Deleted**: `components/DecisionsTab.qml`, `SystemTab.qml`,
  `MemoryTab.qml` (`NewDecisionSheet.qml` moved, see above).
  `OverlayHeader.qml` and the overlay banner were already gone in WP-121;
  the banner's Model helper is gone now.
- **Harness**: `harness/desk.qml` gains `fresh[:<json>]` (drop and
  re-summon as the loader does; `firstFrame` counters on the first
  swapped frames), `hoverItem:<slot>:<i>`, `leave`, and the overflow check
  now also checks every text against its Prime Radiant slot.
  `desk-view.sh` sections 8 (radiant-ipc, radiant-fresh, radiant-hover,
  radiant-size-×3, radiant-scaled, radiant-half, radiant-reflow,
  radiant-uninit, radiant-variant-×8) and 9 (decisions, decision-cases,
  decisions-live, decisions-locked, system, system-empty, system-sparse,
  system-degraded, memory, uninit-sections, stacked-sections, fit-1366,
  fit-3840), and the sections in `DESK_SHOTS`. WP-121's `ipc` #20 now
  expects `select ADR-0004` to answer "ok" (Decisions is real).
- `service-states.sh` scenario 35 drives the moved `NewDecisionForm`.
- **Docs**: SPEC-PLUGIN §2, §5.3 (←/→, h/l), §5.4 (`moveAcross`), new
  §5.4.1 (the three sections), §5.7 (the 0.1 Decisions/Memory text and
  table rows removed), §6 (the section, its grid, periods, the
  first-frame rule), §8 (`setPeriod`, `hover`, `sectionView`);
  KEYBINDINGS.md; TESTING.md (new steps, cases, unit tests, the live
  Prime Radiant commands); COVERAGE.md rows marked done.

## Decisions (what the WP left open)

Recorded in [PLAN.md](PLAN.md) §Decisions; the short form:

1. **No decision body, no memory text** in the details: the index has
   neither, the plugin reads only the index; *Open in editor* shows them.
2. **Accept = the existing path**: no engine command accepts a decision;
   the user sets `status: accepted` in the frontmatter. Accept on a
   proposed decision opens it (`seldon open ADR-NNNN --editor --json`)
   and the detail says what to change. It writes nothing, so no arming.
3. **New decision in the detail pane**; Esc or a click on a decision
   leaves it, the title kept.
4. **System's five tiles**: Collectors also carries machine, engine,
   index time and the logbook's areas (how Seldon sees the machine); a
   tile without data says "—" / "Not in the index"; without an index no
   tiles.
5. **Periods**: the digits are the desk's, so ←/→ and h/l (the overlay's
   1–4 have no successor); `setPeriod` from another section shows section
   7; `hover` answers `{ error }` unless section 7 is shown.
6. **Search** filters Decisions (id, title, status, date), System (title,
   value, lead), Memory (title, meta, kind).
7. **Grid** = `overlayGrid` of the section's size minus padding and row
   1, the overlay's gap and minimum slot; at 50 % on 1920 the grid is
   medium, under a 960 px window narrow and scrolling (↑/↓ scroll it).
8. **Enter** shows the detail (ADR-0034 §2 "Enter selects"); opening is
   `e` or the bar's button (the 0.1 tab opened on Enter).

## How it was verified

- `omarchy plugin validate plugin/` — ok before every commit.
- `just qmllint` — ok, 0 warnings, 47 files; token check ok.
- `node tests/plugin/model.test.js` — 110 passed.
- `bash tests/plugin/desk-view.sh` — 862 passed, 0 failed (with
  `DESK_SHOTS` the shot cases add their checks). The counters on the
  fixture equal the old overlay's: first frame 0 aggregation passes and
  0 paints, every chart painted once by frame 2, `2,2,2,1,2,1` after a
  period switch, nothing for a hover, `3,3,3,2,3,2` after a resize, the
  section's own aggregation count 0 throughout and the service's
  unchanged from before the desk existed. At 50 % (960 px desk) the grid
  is medium with no text outside its slot; under 960 px narrow,
  scrolling, still none.
- `DESK_SHOTS` renders checked by eye in Tokyo Night, Kanagawa and
  Catppuccin Latte: Decisions, System, Memory, the Prime Radiant at 100 %
  and 50 % and with a 365 d hover.
- Not run: a live check on the test host (the section reuses the charts
  unchanged and the desk window of WP-121; the paint/aggregation budget is
  asserted offscreen). Worth one look when `next` goes to the test host.

## What was not done / open

- **Contract v2**: `decisions[].cases` is read wherever it is present
  (`Array.isArray`), so the CASES block works as soon as WP-120's index
  carries it; tested with the field added to a contract-1 fixture (the
  plugin still checks `contractVersion` 1 until WP-120 bumps it). No
  contract change here.
- **An engine `decide accept`** would make Accept one click; it is a new
  row in CONTRACT.md's command table — for the orchestrator to decide
  (not needed for 0.2.0).
- **A notice settling in the first frame** (index stale, snapper not
  readable) still moves the grid once and each chart paints twice: the
  notices (`Notices.qml`, `Banner.qml`) size by their Columns' polish.
  The old overlay had the same with its banner; making the notices'
  heights synchronous is a WP-121/126 polish, not done here.
- The RiskDonut's centre label touches the ring in a narrow slot (50 %
  desk); the chart is reused unchanged as the WP says.
- README, guides, preview.png and the CHANGELOG **Breaking** entry for
  the overlay's 1–4 keys: WP-126.

## Security (for stage 2)

- No new engine command, no new process: Accept, `e` and Open in editor
  run the existing `seldon open <ADR-NNNN|status|logbook> --editor
  --json` (fixed argv, ids validated by `Model.openArgs`); New decision
  is the existing `decide --no-edit --json -- <title>`. A case link from
  `decisions[].cases` only switches sections (`Desk.section`/`select`)
  and only for ids matching `CASE_ID`. All user content is plain text.

## Final check

`flock /tmp/seldon-check.lock just check` on 62670fb: **exit 0** (`check:
ok`; engine tests ok, install.test 209, deploy-test-host.test 190,
docs-check ok, qmllint ok 47 files, model.test.js 110, real-home-guard
11, service-states 316, desk-view 862, bar-view 194). The first run on
60b5675 failed: `service-states.sh` scenario 35 still loaded
`components/NewDecisionSheet.qml`, which this WP moved to
`components/desk/NewDecisionForm.qml`; fixed in 62670fb. Only this
handover and PLAN.md were added after the green run.

## Round 2

Review 1 (stage 1): APPROVE with N1–N6 non-blocking; the orchestrator
asked for N1–N5 in a round 2 while WP-120 and WP-122 merge first (no
merge of `next` here). Commits 865414b (plugin), 1136ba0 (tests),
382bd8d (docs).

- **N1 — first ↓/j after a search.** `ReadingSection.move` now starts
  from the row the detail shows (`Math.max(0, cursor)`), so with the
  selection filtered out the first `j` moves on from the first row. A
  search that only hid the selection gives it back when cleared (the
  selection changes only on a move or a click).
- **N2 — System and Memory search tested.** New case `search-sections`:
  System `/aur` (by the lead) and `/installed` (by the value) → Packages;
  Memory `/hyprland` → the lesson and the topic, `j`/`k` from the shown
  row, Esc gives the old selection back; `/memory/hyp` → the topic by its
  path. TESTING.md's "the search" now says which fields per section.
- **N3 — `setPeriod` with an unknown id changes nothing.** `Desk.setPeriod`
  checks `Model.isPeriod` before showing section 7; it returns section
  7's period ("" before its first visit). New case `radiant-setperiod`.
  SPEC-PLUGIN §8 says so.
- **N4 — the period keys on screen.** "←/→ period" beside the chips while
  the row has room for the title too (`view.sectionView.keyHint`);
  `radiant-ipc` #1 checks it; SPEC §6 row 1 names it.
- **N5 — RiskDonut's centre label.** "all time" is drawn under the count
  only where the hole holds it (`labelFits`, from a `TextMetrics`; the
  caption says "all time" always). `ChartCanvas.readout` reports a
  chart's own drawing decision in `view` (RiskDonut: `{ centreLabel }`);
  `radiant-half` checks true at 100 %, false at 50 %. Checked by eye in
  the Latte radiant-50 render (`DESK_SHOTS`): the ring is clean. The
  chart's data, paints and hover are unchanged.
- **N6 — note only.** The header sizes by `Style.spacing.controlHeight`,
  not `tools.height`; a theme whose fonts make the Esc button taller than
  the brand block and the KPI figure would clip it. Not changed (every
  theme in the sweep has the brand and KPIs taller); WP-126's font-scale
  sweep is the place to look.

Mutants on an archive copy (scratchpad `mutants-r2.sh`), each killed:
M7 (System search on the title only) → `search-sections` #5, #9; N1's
old `move` → `search-sections` #15; N3's old `setPeriod` →
`radiant-setperiod` #2, #4, #7, #8; no hint → `radiant-ipc` #1 (×2);
the label always drawn → `radiant-half` #2.

Verified: `omarchy plugin validate plugin/` ok; `just qmllint` ok (47
files); `model.test.js` 110; `desk-view.sh` 904 passed with
`DESK_SHOTS`, 886 without. `flock /tmp/seldon-check.lock just check` on
382bd8d: **exit 0** (`check: ok`; install.test 209, deploy-test-host
190, docs-check ok, qmllint ok 47 files, model.test.js 110,
real-home-guard 11, service-states 316, desk-view 886, bar-view 194).
Only this handover changed after that commit.

For the merge onto `next` after WP-122 (the reviewer's trial merge): keep
WP-122's `clipBox`/`intersect` in the harness's `overflow()` and add the
`slot` box, then tighten `rfits`' scroll exemption; renumber this WP's
desk-view sections to 9 and 10; take all three form moves in
`service-states.sh`; `decision-cases` step 5 can then also expect
`.view.selected == "C-2026-003"`; consider WP-122's `takeKeys()` in
Decisions' `closeForm`. Full `just check` on the merged `next`.

## Merge of next

`origin/next` (a1198b2: WP-120 contract v2, WP-122 sections 1–3) merged
into this branch in 4357414. The conflicts were the seven docs/test
files the review's trial merge predicted; no plugin code conflicted
(`Desk.qml`, `Model.js`, `Section.qml` merged on their own). Both sides
kept:

- `harness/desk.qml`: WP-122's clip-aware `overflow(item, box, out,
  clipBox)` and `intersect()` with WP-123's Prime Radiant `slot` box as a
  fifth parameter; the texts inside a scrolled chart grid now count only
  where the clip shows them, so `rfits` dropped its "if the grid scrolls,
  only @slot" exemption and checks the plain overflow list.
- `desk-view.sh`: WP-122 keeps section 8; WP-123's become 9 (Prime
  Radiant, 9a–9g) and 10 (Decisions, System, Memory, 10a–10i); one
  offscreen-render header for both.
- `service-states.sh` scenario 35 loads all three moved forms
  (`desk/NewCaseSheet`, `desk/DriftForm`, `desk/NewDecisionForm`).
- SPEC-PLUGIN: the file tree lists both sections' parts (no 0.1 tab
  components left); §5.3's keys row and §5.4's table name both; §5.4
  holds WP-122's Today/Changelog/Work and WP-123's "Decisions, System,
  Memory (4–6; WP-123)" (was §5.4.1); the Enter row says sections 4–6
  have no first action (theirs open the editor, `e`); §5.7 is superseded
  for every section and kept as the 0.1 record. KEYBINDINGS, TESTING and
  COVERAGE.md take both rows and paragraphs.

Then to the contract-2 sample and WP-122's sections:

- The sample's `decisions[].cases` is present: Decisions shows the CASES
  block for every decision (ADR-0004 "CASES · 0 / This decision names no
  case.", ADR-0003 C-2026-004 and C-2026-005); `decisions` expects it,
  the new `decision-nocases` case checks that an index without the field
  hides the block; the `decisionCases` unit test reads the sample.
- Two more events today: the Prime Radiant's heatmap summaries ("75
  events on 15 of 90 days · busiest 2026-10-01 (32)", "70 … of 30 days …
  (32)") and the 2026-10-01 hover read-out ("32 events · seldon 8 · …").
  The other counters (rows per slot, paints, aggregation) are unchanged.
- Work is real now: `decision-cases` #5 expects the case selected there
  (`.view.selected == "C-2026-003"`).
- Decisions' `closeForm` hands the keys back with WP-122's
  `Desk.takeKeys()`.

Verified: `omarchy plugin validate plugin/` ok; `just qmllint` ok (46
files); `model.test.js` 124; `desk-view.sh` 1364 passed with
`DESK_SHOTS` (the Kanagawa Decisions render shows the CASES block),
1346 without. `flock /tmp/seldon-check.lock just check` on 4357414:
**exit 0** (`check: ok`; install.test 209, deploy-test-host 190,
docs-check ok, qmllint ok 46 files, model.test.js 124, real-home-guard
11, service-states 328, desk-view 1346, bar-view 194). Only this
handover changed after that commit.
