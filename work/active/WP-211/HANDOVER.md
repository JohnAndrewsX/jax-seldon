# WP-211 handover — Work's WIP line on a narrow desk

Branch `wp/211-wip-line`, base `next` 50bbd290. Plugin and harness only:
no engine, schema or fixture change. Round 2 answers review 1
(SEND BACK, F1–F5); see "Round 2" at the end.

**Status: PAUSED (wind-down, 2026-10-12 12:46).** F1–F5 are done and
committed (cff236d5, a0e20b27), and the mutation runs are recorded. Round
2's full gate has **not run**. It waited on `/tmp/seldon-check.lock`
behind other agents' checks and could not finish before the account
switch, so I stopped it before it started. **Next:** run, on this
branch's head, `SELDON_FULL_CHECK=1 flock /tmp/seldon-check.lock just
check` with the usual private env. Put its result where "How it was
verified" says *not run*. Then reply `WP-211 ready <sha>`.

## What was done

- **Work's header row** (`plugin/sections/Work.qml`). The WIP line sits
  beside *By agent*, *Import tasks…* and *New case* where it fits: its
  advance (TextMetrics) plus `Style.spacing.sm` plus the buttons' implicit
  widths, rounded up to whole pixels, is no wider than the row. Otherwise
  it takes its own line above the buttons, and the buttons wrap in a
  `Flow`. That is the same pattern as the desk's ActionBar and the
  Changelog's chips. A list column is at most 360 px (`listMax`,
  `plugin/Desk.qml`) and the line plus the buttons need about 353 px or
  more. So **the line is above the buttons on every desk with a list
  column, and beside them only in the stacked layout**, where the list is
  the desk's width. The orchestrator accepted this as the behaviour (F1).
  There is no elision, so the line keeps all its words. The row uses
  `Style` tokens and `qs.Ui` `Button`s only. Work's test view gets
  `wipBeside`. The row is named `headerRow:work`.
- **Harness: `overlap`** (`tests/plugin/harness/desk.qml`). Every report
  lists, for each visible Item named `headerRow:<name>`, the items that
  draw over each other, leave the row sideways or leave the row's height.
  A Row's or Flow's items count one by one. The whole row is laid out
  first (`layOut`: `forceLayout()` on each positioner and text), because
  the report runs before their polish and every frame is drawn after it.
  A text counts by its box, as in `overflow`. The tolerance is 1 px. Every
  report also names the header rows it found (`headerRows`).
- **Cases** (`tests/plugin/desk-view.sh` §8d‴). Each runs over the limit
  ("40 / 3 active · over the limit", from the cut index) and with the
  short line ("2 / 3 active", the sample). The windows are 750×700
  (stacked), 900×700 (the sidebar's icons), 1366×768, 1920×1080 and
  2048×1152 at `QT_SCALE_FACTOR` 1.25, each at 100 % and 50 %. The last
  is the operator's 2560×1440 screen at scale 1.25 on 0.2.0, with the
  desk at 1019 px, which is checked. Every step checks four things:
  - the row is there (`headerRows`);
  - `wipBeside` is true at 750×700 and false at every other size;
  - `overlap` and `overflow` are empty;
  - the whole line is shown.

  The existing `work` case checks `overlap` too.
- **Docs.** SPEC-PLUGIN §Work says where the line goes: above the
  buttons in a list column, beside them when stacked. TESTING.md
  describes `overlap`, `headerRows` and the sizes, including 2048×1152 at
  scale 1.25. COVERAGE.md row 23 says the same.

## How it was verified

- **headless**: the overlap check fails on the old layout. On af6d36aa
  (the check, the row still the old one), `desk-view.sh` failed
  `work-wip-900x700` #1/#3, `1366x768` #3 (the 960 px floor) and
  `1920x1080` #3. The report read: "headerRow:work: 40 / 3 active · over
  the limit × By agent | … × Import tasks… | … × New case | By agent
  leaves the row". This run was outside the lock (my mistake, early on);
  all later runs were under a lock.
- **headless**: the check fails on 0.2.0's layout at the operator's
  geometry. See "0.2.0 proof" below.
- **headless**: mutation runs (F5). See "Round 2" below.
- **headless**: `SELDON_FULL_CHECK=1 just check` on a0e20b27 under
  `flock /tmp/seldon-check.lock`. It used a private HOME, a 0700
  XDG_RUNTIME_DIR removed afterwards, TMPDIR, JUST_TEMPDIR and
  CARGO_TARGET_DIR under the private gates folder, and the real
  RUSTUP_HOME and CARGO_HOME: **not run** (paused, see the top). The
  last green full gate is round 1's, on 2d65c9b5 under
  `/tmp/seldon-gate.lock`: `check: ok`, desk-view 2471 passed, 0 failed.
  Since then, the mutation baseline (round 2's case 8d‴ only, on
  a0e20b27's plugin and harness, headless, under the check lock) gave
  113 passed, 0 failed. qmllint and plugin-validate are ok on cff236d5.
- **headless**: shots of every WIP case (`DESK_SHOTS`, private, not
  committed). At 2048×1152 at 50 % the line is above the buttons and
  *New case* wraps. At 1366 and 1920, at 100 % and 50 %, the line is also
  above the buttons. At 750×700 (stacked) it is beside them. (These are
  round 1's shots; round 2 did not change the layout, and `wipBeside`
  asserts the same in the mutation baseline.)
  (Round 1 said the line was beside the buttons at 1366 and 1920 at
  100 %. That was wrong: review 1, F1.)
- **desktop**: not run. The dev host runs released builds only (E38).
  The operator's font is proportional and the harness's is monospace. The
  row measures its own text and buttons, so it follows the font, but that
  was not seen on a live desk. (The reviewer ran a JetBrainsMono Nerd Font
  sweep headless; review 1 §3.)
- **test host**: not run.

## 0.2.0 proof

This was headless, under `flock /tmp/seldon-gate.lock` (the
orchestrator's go in round 1), from a private tree: this branch's
`tests/` and `fixtures/` (2d65c9b5) and `git archive v0.2.0 plugin`. The
only change to 0.2.0 is the marker line `objectName: "headerRow:work"` on
the row, the same line af6d36aa adds to the old row. A first run without
that marker passed every case, because the check found no row to test.
That is why each case now requires the row.

With the marker, `headerRows` held `headerRow:work` at every step, and
`overlap` failed at:

- 2048×1152 at scale 1.25, 50 % (the desk 1019 px, the operator's
  geometry), both lines: "headerRow:work: 0 / 3 active × By agent |
  headerRow:work: 0 / 3 active × Import tasks… | headerRow:work: By agent
  leaves the row". It is "0 / 3" because 0.2.0 counts the WIP from the
  list's length (before WP-206). This is the operator's picture.
- 900×700 at 100 % and 50 %, 1366×768 at 50 % and 1920×1080 at 50 %: the
  same three findings, for both lines.
- The cases passed at 750×700 (stacked) and at 100 % on 1366, 1920 and
  2048.

That run used round 1's x-only check. It was not repeated with round 2's
height check: the row is the same, and its findings are sideways.

## Round 2 (review 1)

- **F1**: SPEC-PLUGIN §Work, the Work.qml header comment, the row's
  comment, the §8d‴ case comment and this handover now say what the code
  does: the line is above the buttons in a list column (at most 360 px)
  and beside them only in the stacked layout. The layout is unchanged
  (orchestrator decision).
- **F2**: every WIP step asserts `wipBeside`: true at 750×700, false at
  900, 1366, 1920 and 2048 at 100 % and 50 %.
- **F3**: `overlap` also reports an item that leaves the row's height
  ("… leaves the row's height").

  The first baseline with that check failed at 750×700, step 1. The shot
  showed the row drawn correctly. The cause was the harness's measuring
  order: it measured the WIP text while the row was still as tall as the
  stale two-line Flow, then laid the Flow out, which shrank the row under
  the text already measured. a0e20b27 lays out the whole row first, then
  measures.
- **F4**: TESTING.md and COVERAGE.md name the 2048×1152 at scale 1.25
  cases and the height check.
- **F5, mutation runs** (headless, each under
  `flock /tmp/seldon-check.lock`). The command was the literal path
  `gates/dev-211/mut/<m>/tests/plugin/wip-only.sh`. Each tree is
  `git archive cff236d5 plugin tests fixtures` with a0e20b27's harness and
  one mutation. `wip-only.sh` is desk-view.sh's setup plus its case 8d‴
  only (private, not committed), like the reviewer's `rv-211-wip.sh`, but
  rebuilt from round 2's case so the new assertions run. I re-made the
  reviewer's five mutations on this code rather than run the
  `gates/rv-211/mut-*` trees, which hold round 1's case and harness.

  | Tree | Mutation | Result | Killed by |
  |---|---|---|---|
  | base | none | 113 passed, 0 failed | — |
  | force | no `forceLayout()` on the positioner | **killed**: 107 / 6 | "New case leaves the row" at 1366, 1920 and 2048, at 50 % (#3), both lines |
  | name | no `objectName: "headerRow:work"` | **killed**: 93 / 20 | `headerRows` lacks the row, every size and step |
  | bfalse | `beside: false` | **killed**: 109 / 4 | `wipBeside` false at 750×700, #1 and #3, both lines (F2) |
  | btrue | `beside: true` | **killed**: 87 / 26 | `wipBeside` true in a list column, and overlap "… × By agent \| … × Import tasks…" |
  | height | the row's height without `wipButtons` | **killed**: 97 / 16 | "By agent / Import tasks… / New case leave the row's height", at every size with a list column (F3) |
  | height-xonly | the same, with round 1's x-only harness (2d65c9b5) | **survives**: 113 / 0 | nothing: the case for F3 |

  The reviewer's sixth tree, `mut-rows` (tagging the Changelog's and
  Decisions' rows), was not run. It is the follow-up WP (orchestrator
  decision).

## Open points

- Follow-up WP (orchestrator decision), not done here: key paths for *By
  agent* and *Import tasks…* (WP-202's "every button has a key path"),
  and `headerRow:` tags on the Changelog's count row and Decisions'
  summary row. Both rows have the same anchors pattern that broke here.
- Seen in the 750×700 shot, not in this WP: at a 740 px desk, the desk
  header's KPI strip ("ACTIVE · VERIFICATION") draws over the subline
  ("captured just now"). The header (`components/desk/Header.qml`) is
  not a tagged row. Tagging it `headerRow:desk` would make the check see
  it.
- The overlap check covers only the rows named `headerRow:`. It does not
  sweep the whole desk.
