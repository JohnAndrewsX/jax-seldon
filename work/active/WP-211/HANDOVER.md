# WP-211 handover — Work's WIP line on a narrow desk

Branch `wp/211-wip-line`, base `next` 50bbd290. Plugin and harness only:
no engine, schema or fixture change.

## What was done

- **Work's header row** (`plugin/sections/Work.qml`). The WIP line stays
  beside *By agent*, *Import tasks…* and *New case* while it fits there:
  its advance (TextMetrics) plus `Style.spacing.sm` plus the buttons'
  implicit widths, rounded up to whole pixels, is no wider than the row.
  On a narrower list column it takes its own line above the buttons and
  the buttons wrap in a `Flow`. That is the same pattern as the desk's
  ActionBar and the Changelog's chips. No elision, so the line keeps all
  its words; `Style` tokens and `qs.Ui` `Button`s only. Work's test view
  gets `wipBeside`. The row is named `headerRow:work`.
- **Harness: `overlap`** (`tests/plugin/harness/desk.qml`). Every report
  lists, for each visible Item named `headerRow:<name>`, the items that
  draw over each other or leave the row. A Row's or Flow's items count one
  by one, and the positioner is laid out first: the report runs before
  its polish, and every frame is drawn after it. A text counts by its box,
  as in `overflow` (an elided one by its width, any other by what it
  draws). The tolerance is 1 px. Every report also names the header rows
  it found (`headerRows`). Each WIP case requires `headerRow:work` among
  them, so a row that is not marked fails instead of passing unchecked.
- **Cases** (`tests/plugin/desk-view.sh` §8d‴). Each runs over the limit
  ("40 / 3 active · over the limit", from the cut index) and with the
  short line ("2 / 3 active", the sample). The windows are 750×700
  (stacked), 900×700 (the sidebar's icons), 1366×768, 1920×1080 and
  2048×1152 at `QT_SCALE_FACTOR` 1.25, each at 100 % and 50 %. The last
  is the operator's 2560×1440 screen at scale 1.25 on 0.2.0, with the
  desk at 1019 px, which is checked. Every step checks that the row is
  there, that `overlap` and `overflow` are empty and that the whole line
  is shown. The existing `work`
  case checks `overlap` too.
- **Docs.** SPEC-PLUGIN §Work (where the line goes), TESTING.md (the
  report's `overlap`), COVERAGE.md row 23.

## How it was verified

- **headless**: the overlap check fails on the old layout. On
  af6d36aa (the check, the row still the old one), `desk-view.sh` failed
  `work-wip-900x700` #1/#3, `1366x768` #3 (the 960 px floor) and
  `1920x1080` #3. The report read: "headerRow:work: 40 / 3 active · over
  the limit × By agent | … × Import tasks… | … × New case | By agent
  leaves the row". 750 px (stacked) and 100 % passed. The shots show
  the review's picture. This run was outside the lock (my mistake, early
  on). The later runs were all under a lock.
- **headless**: the check on 0.2.0's layout at the operator's geometry.
  See "0.2.0 proof" below. 0.2.0's Work row is identical to the old row
  of `next` (diff of the row: none).
- **headless**: a first run of the new layout found "New case leaves
  the row" at the 960 px floor. That was the report reading the Flow
  before its polish (the shot showed *New case* on its second line, inside
  the column). The harness now calls `forceLayout()` before it measures.
- **headless**: `SELDON_FULL_CHECK=1 just check` on 2d65c9b5 under
  `flock /tmp/seldon-gate.lock` (the orchestrator's go). It used a
  private HOME, a 0700 XDG_RUNTIME_DIR removed afterwards, TMPDIR,
  JUST_TEMPDIR and CARGO_TARGET_DIR under the private gates folder, and
  the real RUSTUP_HOME and CARGO_HOME. Result `check: ok`: desk-view 2471
  passed, 0 failed (all 91 WIP-row checks ok, the 2048 desk 1019 px),
  service-states 367, bar-view 196, ipc-restart 44, model.test.js 221,
  qmllint ok, plugin-validate ok. It started after the orchestrator's
  shell restarts (08:41–08:50) and did not trip the real-home guards.
- **headless**: shots of every WIP case (`DESK_SHOTS`, private, not
  committed). At 2048×1152 at 50 % the line is above the buttons and
  *New case* wraps; at 1366 and 1920 at 100 % the line sits beside them.
- **desktop**: not run. The dev host runs released builds only (E38).
  The operator's font is proportional and the harness's is monospace.
  The row measures its own text and buttons, so it follows the font, but
  that was not seen on a live desk.
- **test host**: not run.

## 0.2.0 proof

This was headless, under `flock /tmp/seldon-gate.lock`, from a private
tree: this branch's `tests/` and `fixtures/` (2d65c9b5) and
`git archive v0.2.0 plugin`. The only change to 0.2.0 is the marker line
`objectName: "headerRow:work"` on the row, the same line af6d36aa adds to
the old row. A first run without that marker passed every case, because
the check found no row to test. That is why each case now requires the
row (2d65c9b5).

With the marker, `headerRows` held `headerRow:work` at every step, and
`overlap` failed at:

- 2048×1152 at scale 1.25, 50 % (the desk 1019 px, the operator's
  geometry), both lines: "headerRow:work: 0 / 3 active × By agent |
  headerRow:work: 0 / 3 active × Import tasks… | headerRow:work: By agent
  leaves the row". It is "0 / 3" because 0.2.0 counts the WIP from the
  list's length (before WP-206). This is the operator's picture.
- 900×700 at 100 % and 50 %, 1366×768 at 50 %, 1920×1080 at 50 %: the
  same three findings, for both lines.
- The cases passed at 750×700 (stacked) and at 100 % on 1366, 1920 and
  2048.

The other desk-view cases were stopped after the WIP cases: 0.2.0's
plugin predates cases the harness has today, so their results say
nothing here.

## Open points

- Other rows have the same anchors pattern (text left, buttons anchored
  right): the Changelog's count row (*Ledger*, *Capture*) and Decisions'
  summary row (*New decision*). They are not tagged `headerRow:` and not
  checked here. Tagging them is one line each; a follow-up WP can do that
  and fix whatever it finds.
- The overlap check covers only the rows named `headerRow:`. It is not a
  whole-desk sweep.
