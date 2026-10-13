# WP-215 handover — every header button by key, every header row checked for overlap

Branch `wp/215-header-keys`, base `next` 9a9f5f7b. Plugin, harness and
docs only: no engine, schema or fixture change. Round 2 answers review 1
(SEND BACK, F1–F4, Q1); see "Round 2" at the end. Where round 1's text
below is no longer true, it says so. The key path is a
compatible addition (new keys, nothing changed or removed), so it ships
in the next minor release; CHANGELOG [Unreleased] says so.

## What was done

- **Work's header buttons by key** (`plugin/sections/Work.qml`,
  `plugin/Model.js`). This follows SPEC-PLUGIN §5.3 and WP-202's pattern.
  - ↑ on Work's first row (or in an empty list) puts the cursor on *By
    agent*. No row stays selected, and the button wears the focus ring.
  - ←/→ walk *By agent*, *Import tasks…* and *New case*.
  - Enter or Space arms the button under the cursor, and the hint names
    it ("By agent? Press Enter again."). The second press runs exactly
    what the click runs (`headClick`). The arm ids name what they run:
    `work:agent:on|off`, `work:import:open|close`, `work:new:open|close`.
  - None of the three starts an agent, so Enter never does.
  - Where the engine cannot write (dev mode, the engine's
    `writeBlocker`), *Import tasks…* and *New case* arm nothing, and the
    line says why.
  - ↓ goes back to the first row; ↑ goes on to the notices.
  - Any other key, a click on the desk, a summon, a section change, a
    row selection or IPC `select` cancels the arm (`disarmHead`).
  - In the stacked layout, ↑ on the first row while its detail is shown
    brings the list back with the cursor on *By agent*.
  - A dim line under the buttons (`workHeadHint`) names the keys: "↑ from
    the first row reaches these buttons", and "←/→ a button · Enter twice
    runs it · ↓ back to the list" while the cursor is on them.
  - `shell call jax.seldon select header` places the cursor and never
    presses.
  - Work's test view gets `head: { button, key, armed, hint }`.
  - `Model.workHeadTargets` and `Model.workHeadHint` are unit-tested in
    `model.test.js`.
- **Header rows tagged** for WP-211's overlap check (x and height):
  - `headerRow:desk`: the desk header, i.e. the mark and subline
    (`deskBrand`), the chip, the KPI strip (`kpiStrip`), Settings and Esc;
  - `headerRow:today`: Today's date row;
  - `headerRow:changelog`: the Changelog's count row;
  - `headerRow:changelog-hidden`: the Changelog's "N hidden" row;
  - `headerRow:decisions`: Decisions' summary row;
  - `headerRow:work`: Work's WIP row (WP-211).
- **Desk header at 740 px** (`plugin/components/desk/Header.qml`). The
  subline is as wide as its text but no wider than the room left of the
  KPI strip. Where it does not fit, it wraps to at most two lines, then
  elides. Its " · " separators get a non-breaking space before the dot,
  so no line starts with "·". (Round 1 said a wrapped line ends on "·";
  that was wrong at 740 px, where the break falls inside "Omarchy
  4.0.7-1": review 1, F4.) The brand's width follows the subline, so the
  check sees the real box. Round 2: while the chip is up, the subline
  also leaves it the room for its count (F1).
- **Cases** (`tests/plugin/desk-view.sh`).
  - §8d⁗ `header-rows-<size>` runs at WP-211's sizes: 750×700, 900×700,
    1366×768, 1920×1080, and 2048×1152 at scale 1.25. It walks Today,
    the Changelog (with one change hidden), Decisions and Work at 100 %,
    then again at 50 %. Each of the 11 steps requires its rows by name
    (`headerRows`; no row, no pass). Each step also checks `overlap` and
    `overflow` are empty.
  - `keys-work` runs on the sample in dev mode, 18 steps. It covers
    reach, arm, disarm by ↓, the filter by Enter twice, *Import tasks…*
    disabled with its reason, ←, a row click, `select header`, a summon,
    and Space twice.
  - `keys-work-live` runs live. *Import tasks…* by Enter twice opens the
    form, and a path and Return make the dry run. *New case* by Enter
    twice opens the sheet, and a title and Return create the case. The
    engine's argv is checked exactly: those two calls, no agent start.
  - `keys-work-stacked` runs at 700×900: ↑ from a shown detail.
- **Docs.**
  - SPEC-PLUGIN covers the keys in §5.3 (↑/↓, Enter, ←/→, the house
    rule's arm ids, `enterFromTop`) and in §8 (`select header`).
  - SPEC-PLUGIN §5.2 covers the subline wrap.
  - SPEC-PLUGIN §5.4 Work has a "By key" paragraph and the WIP sentence
    the WP-211 review asked for. The code measures: the buttons wrap only
    where the column is too narrow, and a narrower font can put the line
    beside them.
  - KEYBINDINGS.md has the key row, the arm paragraph and `header` among
    the select targets.
  - Also updated: `plugin/README.md`, `docs/TESTING.md` (the tagged
    rows), `tests/plugin/COVERAGE.md` row 23, guide 03 in English and
    German, and the CHANGELOG.

## How it was verified

- **fixture**: `model.test.js` 221 passed; `just qmllint` ok (53 files);
  `docs-check` ok.
- **headless**: the WP-215 cases on the final code (5fec02e1). The
  command was a private runner: desk-view.sh's setup, case 8d‴, case
  8d⁗ and the three keys-work cases, unchanged. It ran under
  `flock /tmp/seldon-check.lock`, with `RUNNER_ROOT` pointing at the
  tree. Result: **316 passed, 0 failed**. The real-home guard was ok,
  and the by-id runtime count was unchanged.
- **headless**: the same runner against the old code (fresh tree:
  9a9f5f7b's `plugin` and `fixtures`, this branch's `tests/plugin`).
  Result: **235 passed, 81 failed**.
  - `keys-work` (24), `keys-work-live` (9) and `keys-work-stacked` (3)
    fail. ↑ on the first row reaches no button (`head` is null, the
    cursor stays on row 0, no ring).
  - In `keys-work-live`, the same keys act on the selected case instead.
    The engine's argv held `plan verify C-2026-003`, `plan done
    C-2026-008`, `open C-2026-008 --editor`, `plan reopen C-2026-008`,
    where it should have held the dry run and `plan new`.
  - Every `header-rows-*` step fails, because the old code names no rows.
- **headless**: the old code plus only the five `objectName` tags
  (oldtags). Result: **268 passed, 48 failed**. The 36 key failures are
  as above. The other 12 are at 750×700: `header-rows` steps 2–11 and
  `work-wip-*-750x700` #3 each report "headerRow:desk: QQuickItem ×
  KpiStrip", i.e. the old, unnamed brand under the KPI strip. That is
  the 740 px bug, now seen by the check.
- **headless**: overlap mutants. Each tree is a fresh
  `git archive 5fec02e1 plugin fixtures tests/plugin` with one line
  changed. Each ran under the lock with the same runner. Every one was
  **killed**:

  | Tree | Mutation | Result | Killed by |
  |---|---|---|---|
  | final | none | 316 / 0 | — |
  | m-sub | subline `width` uncapped (no `brand.room`) | 304 / 12 | "headerRow:desk: deskBrand × kpiStrip" at 750×700 (`header-rows` #2–#11, `work-wip-*` #3) |
  | m-dec | Decisions summary `anchors.right: parent.right` | 306 / 10 | "headerRow:decisions: 4 decisions · 1 proposed × New decision", every size, 100 % and 50 % |
  | m-cl | Changelog count line `anchors.right: parent.right` | 298 / 18 | "headerRow:changelog: N changes · newest first × Ledger \| … × Capture now", every size |
  | m-hidden | "N hidden" line `anchors.right: parent.right` | 307 / 9 | "headerRow:changelog-hidden: 1 change hidden this session × Show", every size |
  | m-today | date flow `anchors.left: parent.left` (over the state icon) | 306 / 10 | "headerRow:today: MaskIcon × Thursday, 1 Oct 2026" (and "× Open in editor"), every size |

- **headless**: `SELDON_FULL_CHECK=1 just check` on 5fec02e1 under
  `flock /tmp/seldon-check.lock`, with a private HOME and a 0700
  XDG_RUNTIME_DIR (removed afterwards); TMPDIR, JUST_TEMPDIR and
  CARGO_TARGET_DIR under the private gates folder; the real RUSTUP_HOME
  and CARGO_HOME. Result: **`check: ok`**. desk-view 2655 passed,
  0 failed; service-states 367, bar-view 196, ipc-restart 44,
  model.test.js 221, every cargo test binary ok (112 result lines, none
  failed), qmllint ok (53 files), plugin-validate ok, docs-check ok,
  validate-fixtures ok. The real-home guard was ok, and no by-id leftover
  appeared in the session's or the private runtime dir. The private
  runtime dir was removed afterwards.
- **headless**: shots (`DESK_SHOTS`, private, not committed). At
  750×700 with no notice up, the subline wraps beside the KPI strip
  ("workstation-7f3a · Omarchy" / "4.0.7-1 · captured just now"), and
  nothing overlaps. Round 2 shots: see "Round 2".
- **CI**: not run (no remote run for this branch).
- **test host**: not run.
- **desktop**: not run. The dev host runs released builds only (E38).
  The harness font is monospace. The subline measures its own text
  (TextMetrics) against the strip's position, so it should follow the
  operator's proportional font, but that was not seen on a live desk.

## Not done / open points

- (Round 1 asked whether `a`, `x`, `r` and `e` should act on the case
  while the cursor is on Work's header buttons. The orchestrator decided
  they do nothing there; done in round 2, Q1.)
- A held ↑ reaches the header buttons the way it reaches the Changelog's
  *Agent sorts* row. Only a fresh press goes on to the notices (WP-202's
  rule).
- Work's intent rows (the import form and the new-case sheet) are not
  tagged `headerRow:`. They are forms, not header rows.
- (Round 1: on the old header, step 1 at 750×700 did not trip the
  check. The cause was the harness: it did not lay out the KPI strip, a
  Row subtype, first. Fixed in round 2, F3.)
- With a notice up on a 750 px window, the subline's second line elides
  ("Omarchy 4.0.7-1 · captu…"), because the chip's count now takes its
  room. SPEC §5.2 allows this: "two lines at most, then elided".
- The overlap check still covers only rows named `headerRow:`. It does
  not sweep the whole desk (as in WP-211).
- The private runner and the comparison trees are in the private gates
  folder (`gates/dev-215/`), not in the repo.

## Round 2 (review 1)

- **F1, the chip on the KPI strip.** `Header.qml`: while the chip is up,
  the subline's room (`brand.room`) is also less the chip's count form:
  its advance, rounded up, plus the chip's padding and gap
  (`chipCountMetrics`, with the label's own "▾"/"▸"). This uses metrics,
  not `chip.width`, so there is no binding loop. So the chip shows at
  least "▾ 1", whole, and never lies on the strip (SPEC §5.2: "the count
  alone where that does not fit either").
  - Harness: a text that an item draws outside its own box now counts
    as its own part (`strayTexts`). This covers the chip's label in a
    chip squeezed to no room, which is not clipped. Such a text is not
    checked against its own item.
  - A text with no width left (the label's width is -20 there) draws
    whole, while Qt reports its `contentWidth` as 0. It counts by its
    natural width (`implicitWidth`). The first version of the stray-text
    rule used `contentWidth` and missed the label (m-chip2 below).
  - New cases `header-chip-750x700` and `header-chip-900x700`: the
    sample with the restart notice (manifest 99), on Today and Work at
    100 %, then on Today and Work at 50 %. Each step checks four things:
    the row is there, the chip shows something and is not truncated
    (`chipShown`, `chipTruncated`), `overlap` is empty and `overflow` is
    empty.
  - SPEC §5.2, TESTING.md, COVERAGE.md row 23 and the CHANGELOG say so.
- **F2, ↑ to the notices and back.** New case `keys-work-notice`, the
  reviewer's steps: with the restart notice up, ↑ ↑ goes from the
  buttons to the notices (`noticeKeys.id` = restart, no head), ↓ comes
  back onto *By agent*, and Enter arms it.
- **F3, the first frame.** `layOut` now forces the layout of every
  visible item that has `forceLayout`, inner ones first. That includes a
  type built on a Row, such as the KPI strip. It runs twice, because a
  positioner's new width moves what is bound to it (the subline's room).
  Before, it matched only type names starting `QQuickRow`/`QQuickFlow`.
- **F4, the wrap wording.** The Header.qml comment now says what
  happens: "the '·' keeps to the word before it: no line starts with
  it". This handover says so above.
- **Q1, case keys on the buttons.** `Work.textKey`: while the cursor is
  on the header's buttons, `a`, `x`, `r` and `e` do nothing (they are
  consumed, so they reach no case). `i` and `+` are unchanged. New live
  case `keys-work-letters`:
  - on the buttons, `a a x x r r e` change nothing: no arm, the cursor
    stays on *By agent*;
  - the engine's argv is the startup only;
  - ↓ back on the first row, `a` arms `case:C-2026-003:agent` again.

  SPEC-PLUGIN §5.4, KEYBINDINGS.md, guide 03 (en, de; the de source line
  is at eb182538) and the CHANGELOG say so.

### How round 2 was verified

- **headless**: the private runner, rebuilt from this branch's
  desk-view.sh. It holds the setup, 8d‴, 8d⁗ with `header-chip-*`, and
  the five keys-work cases, with its lines unchanged. It ran under
  `flock /tmp/seldon-check.lock` on the final code (3218050f): **375
  passed, 0 failed**. The real-home guard was ok, and the by-id runtime
  count was unchanged.
- **headless**: mutants. Each tree is a fresh `git archive <commit>
  plugin fixtures tests/plugin` with one line changed, and each ran
  under the lock with the same runner:

  | Tree | Base | Mutation | Result | Killed by |
  |---|---|---|---|---|
  | m-up | 284a38a1 | `if (dy < 0) return true` on the buttons (the reviewer's) | 372 / 3, **killed** | keys-work-notice #3–#5: ↑ stays on *By agent*, no notice |
  | m-keys | 284a38a1 | no case-key guard on the buttons | 366 / 9, **killed** | keys-work-letters: `a` arms `case:C-2026-003:agent`, the second `a` runs `agent start C-2026-003` (argv) |
  | m-sub | 284a38a1 | subline width uncapped | 347 / 28, **killed** | "deskBrand × kpiStrip" at 750, now **also at step 1** (`header-rows` #1–#11, `work-wip-*` #1 and #3, `header-chip-750` #1–#6), and `chipTruncated` at 900 (F3) |
  | m-chip | 284a38a1 | no room kept for the chip | 367 / 8, killed, but only by `chipTruncated` | that harness did not see the label: the chip's width was 0, the label's `contentWidth` 0 |
  | m-chip-oldharness | 284a38a1 + round 1's harness | the same | 371 / 4 | only `chipTruncated` at 900; round 1's check is blind to it (the review's F1) |
  | m-chip2 | ffe57eea | the same | 367 / 8 | still only `chipTruncated`: the stray-text rule used `contentWidth` (0) |
  | m-chip3 | 3218050f | the same | 362 / 13, **killed** | "headerRow:desk: ▾ 1 × kpiStrip" at 750, every step; `chipTruncated` at 750 and 900 from step 2 |

  A private probe on the m-chip2 tree printed the chip's geometry: the
  chip at x 286.2 (the strip's x), width 0; the label's width -20,
  `contentWidth` 0, truncated. 3218050f's `implicitWidth` rule followed
  from that.
- **headless**: shots of `header-chip-*` at 100 % (private). At 750 px
  the chip reads "▾ 1" between the subline ("workstation-7f3a ·" /
  "Omarchy 4.0.7-1 · captu…") and ACTIVE. At 900 px it reads "▾ 1"
  beside a two-line subline. Nothing overlaps.
- **headless**: `SELDON_FULL_CHECK=1 just check` on 3218050f under
  `flock /tmp/seldon-check.lock`, with a private HOME and a 0700
  XDG_RUNTIME_DIR (removed afterwards); TMPDIR, JUST_TEMPDIR and
  CARGO_TARGET_DIR under the private gates folder; the real RUSTUP_HOME
  and CARGO_HOME. Result: **`check: ok`**. desk-view 2714 passed,
  0 failed; service-states 367, bar-view 196, ipc-restart 44,
  model.test.js 221, every cargo test binary ok (112 result lines, none
  failed), qmllint ok (53 files), plugin-validate ok, docs-check ok,
  validate-fixtures ok. The real-home guard was ok, and no by-id leftover
  appeared. An earlier start of this gate (on c2074d51) was stopped by
  me before its result, because I changed the harness in the worktree
  while it ran; its partial log does not count.
- **fixture**: model.test.js 221 passed; qmllint and docs-check, inside
  the gate above.
- **CI**: not run. **test host**: not run. **desktop**: not run (E38;
  the dev host runs released builds only).
