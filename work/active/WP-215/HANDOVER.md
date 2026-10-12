# WP-215 handover — every header button by key, every header row checked for overlap

Branch `wp/215-header-keys`, base `next` 9a9f5f7b. Plugin, harness and
docs only: no engine, schema or fixture change. The key path is a
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
  so a wrapped line ends on "·" and the next one starts with a word. The
  brand's width follows the subline, so the check sees the real box.
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
  750×700 the subline wraps beside the KPI strip ("machine · Omarchy
  version ·" / "captured …") and nothing overlaps.
- **CI**: not run (no remote run for this branch).
- **test host**: not run.
- **desktop**: not run. The dev host runs released builds only (E38).
  The harness font is monospace. The subline measures its own text
  (TextMetrics) against the strip's position, so it should follow the
  operator's proportional font, but that was not seen on a live desk.

## Not done / open points

- While the cursor is on Work's header buttons, the letter keys (`a`,
  `x`, `r`, `e`) still act on the case in the detail, as they do on any
  other head row. Enter does not. I left them unchanged; say if they
  should be inert there.
- A held ↑ reaches the header buttons the way it reaches the Changelog's
  *Agent sorts* row. Only a fresh press goes on to the notices (WP-202's
  rule).
- Work's intent rows (the import form and the new-case sheet) are not
  tagged `headerRow:`. They are forms, not header rows.
- On the old header, step 1 at 750×700 (Today, right after the summon)
  did not trip the check. Steps 2–11 did. I did not find out why the
  first frame differs. The fix covers every step.
- The overlap check still covers only rows named `headerRow:`. It does
  not sweep the whole desk (as in WP-211).
- The private runner and the comparison trees are in the private gates
  folder (`gates/dev-215/`), not in the repo.
