# WP-177 — handover

## Round 2 (review 1: SEND BACK, B1, M1–M4)

- **B1.** `model.test.js` pins `TONE_TEXT_TARGET === 4.7`,
  `TONE_UI_TARGET === 3.2` (and `TONE_FOCUS_MIN`, `TONE_STEPS`), and
  `checkTheme` asserts the literal WCAG floors, 4.5 for text and 3 for UI
  parts, unless the tone is listed as limited. Mutants: text target 3.5 →
  3 FAIL; UI target 2.0 → 3 FAIL; the linearisation branch dropped → 1
  FAIL (a new test on `#090909`; the reviewer's survivor).
- **M1.** `ui` (the ring and lines) is derived on all four surfaces, the
  selected fill included, so the ring is ≥ 3:1 also on an armed primary
  button (ring/selected 3.20–3.34 on the 22 themes, table below); the
  test checks `ui` like the other tones (mutant `plain.slice(0, 3)` → 2
  FAIL). The ring moved from ~3.7:1 to ~4.6–5.3:1 on the background.
- **M2.** Every key the desk gets clears the pointer's row in code:
  `Section.keyEvents` (Desk.keyPressed's count) → `ListColumn.keyEvents`
  → `dropPointer()`; the sidebar the same. `desk-view.sh one-cursor-keys`:
  Up on the first row and Return (keys that move nothing) clear it; the
  mutant without the handler fails #5 and #8. SPEC §5.3 unchanged ("any
  key").
- **M3.** SPEC §7: "Text never takes `accent`, `urgent` or `muted` as
  they are (primary text is the raw foreground, `Color.popups.text`)".
- **M4.** Strict zero: GraphCanvas `clusterRing` is `tone.ui`; 0 uses of
  `alpha(…, 0.65)` in `plugin/`. `check-tokens.py`'s allow-list is per
  named colour property now (file → property → alphas); the graph's
  inline neighbourhood edge and the Radiant legend's snapshot marker
  became named properties (`edgeBright`, `snapshotMarker`). Self-test
  14/14, incl. a listed property passing and the same alpha on another
  property, another alpha on a listed one and a literal outside a property
  failing (`CHECK_TOKENS_PLUGIN_ROOT` points the check at a copy).
- Notes taken: (b) the E54 table states its definitions and adds the
  border over its own focus fill; (c) the fallbacks cite Color.qml's
  defaults. Not taken: (a) check-tokens blind spots (`property var` alias,
  value on the next line), (d) sidebar/popups-surface harness cases, (e)
  dim close to fg — as the review says, no action for merge.
- Live look on the test host: the orchestrator's, after the merge.

## Round 1

Branch `wp/177-readability` (from `next` at e75c83f4), worktree `wt/WP-177`.
Plugin only; no contract change, no ADR (SPEC-PLUGIN §7 rule).

## What was done

- **Tones (A9/C4).** `Model.textOn(colour, surfaces, target, toward)` and
  `Model.deskTones(theme)` (plugin/Model.js, end of file): a role is mixed
  towards the theme's own foreground in steps of 2 % until it reaches the
  target on every surface; no black, no white, no fallback (where the
  foreground itself cannot reach it, the tone is the foreground and
  `limited` names it). One text target 4.7:1 (`TONE_TEXT_TARGET`), one UI
  target 3.2:1 (`TONE_UI_TARGET`). Surfaces: `Color.popups.background`
  (alpha < 1 taken as opaque), Style's normal, hover and selected fills
  over it, and the accent and urgent tints at the selected alpha; each as
  drawn (8 bits). Tones: `dim`, `accentText`, `urgentText`, `accentUi`,
  `ui`, `focusRing` (+ `themeFocus`, `focusRatio`), `divider` (fg 12 %).
  `plugin/Tones.js` is a `.pragma library`, so every
  `components/Tone.qml` shares one memo: the derivation runs once per
  theme change for the whole plugin, not per row. Components bind
  `readonly property Tone tone: Tone {}` (Section, DetailPane and
  ChartCanvas carry it for their subclasses).
- **The sweep.** 0 text uses of `Color.muted` (was 72 + the canvas labels
  of the charts and the graph); the 8 text dims `Util.alpha(fg, 0.65)` are
  `tone.dim`; text in the raw urgent/accent (49 Text colours, the banner
  title, the header chip's label) takes `urgentText`/`accentText`; the 9
  fg-0.12 hairlines are `tone.divider`. Stripes, pictograms, progress
  fills, charts, the graph and the pill keep the raw roles (ThePlan's
  card stripe keeps raw `Color.muted`: a stripe, not text). UI-state
  literals became Omarchy's tokens: Settings' screen outline and the
  graph card border `Style.normalBorderFor`, the chart highlight
  `Style.hoverFillFor`, Work's intent box `Border.controlSpec("normal")`,
  Changelog's proposal row the selected accent fill with an `accentUi`
  border while shown.
- **One cursor (C2+B9).** `ListRow` is Omarchy's `CursorSurface` and reads
  no hover of its own; `ListColumn.hoverIndex` is set only after a real
  pointer move (`PointerMoveGate`, reference the column, so rows scrolling
  under a still pointer do not count) and cleared by every selection
  change (keys, clicks, index updates), a scroll and the pointer leaving.
  Every delegate passes `cursor: list.hoverIndex === index`. The selection
  is the selected fill plus an `accentUi` bar (≥ 3:1) and a bold title;
  the stripe sits beside the bar, so neither moves. Rows outside a list
  (Decisions' case links) set `pointerHover`. The sidebar gates its hover
  the same way; its current section's bar and icon are `accentUi`.
  Operator decision via the orchestrator: option 2 (hover look only after
  a real move, any key clears it, clicks select); SPEC-PLUGIN §5.3 now
  says "one cursor highlight at a time; the selection has its own look".
- **Seldon's own controls.** `components/desk/FocusRing.qml` on
  `KeyButton`, DriftForm's and NewDecisionForm's submit keys (focus or
  armed) and an armed action-bar button (`armed` added to the action
  objects of Decisions, Work, TriageDetail): the theme's focus border where
  it reaches 3:1, else `ui` at 2 px, drawn over the button's own border
  (one frame, size unchanged). `qs.Ui` fields and the Cancel buttons keep
  Omarchy's focus look. The chosen Changelog chip gets an `accentUi` bar
  along its bottom.
- **Tokens (B11).** Named spacing tokens where a gap or inset has a name
  (BarWidget's count gap `xxs`, Settings' preview inset `sm`). Kept as
  `Style.space(N)` on purpose: line widths, bar heights, icon and marker
  sizes, pointer thresholds and letter spacing (they are sizes, not
  spacing; a theme's `xxl` override should not resize a 12 px marker).
- **Checks (B13a).** `tests/plugin/model.test.js`: colour helpers,
  `textOn`, the memo (once per theme), and every tone against its targets
  on `fixtures/themes/roles.json` (tokyo-night, rose-pine, miasma; five
  roles, as Color.qml reads them) and on every host theme; a weak theme is
  reported, not failed (a synthetic weak theme proves the report and the
  "foreground, not black/white" rule). `tests/plugin/check-tokens.py`: no
  `Color.muted` as a Text colour (also through a colour property set to
  it, inline Text components included), `Util.alpha(…, <number>)` only on
  `ALPHA_ALLOWED` (the charts' and the graph's data colours, with the
  reason); `--rules` runs them without a shell tree, in `plugin-test`
  everywhere (CI too, python3 is already required there);
  `tests/plugin/check-tokens.test.sh` (10 cases) proves each rule fires.
  `desk-view.sh`: `one-cursor` (a pointer row only after a real move; a
  key under the still pointer leaves one highlight, the selection; the
  same point again draws nothing), `tones-default`/`tones-tokyo` (QML's
  tones equal node's for Color.qml's defaults and Tokyo Night; the ring
  `ui 2` on a focused KeyButton, none on a focused field). The harness
  reports `marks.rows`, `marks.rings` and `tones`.
- **Docs.** SPEC-PLUGIN §7 "Text tones" (rule, surface, targets, opaque
  alpha, report-not-fail), States (focus-border numbers), §5.3 rule
  reworded; TESTING.md, fixtures/README.md, the omarchy-ux skill (two
  lines, still 100 lines).

## Not done / open

- One `Util.alpha(…, 0.65)` remains: GraphCanvas `clusterRing`, a graph
  data colour beside `areaRing` 0.9 (not text; allow-listed). Switching it
  to `tone.ui` is a one-line change if the reviewer wants a strict zero.
- `dim` must read on the selected fill and both tints, so in low-contrast
  themes it lands close to the foreground (everforest fg/dim 1.02,
  rose-pine 1.08, catppuccin-latte 1.10): readable, but secondary text
  there differs mostly by size. A separate dim for tints would widen the
  gap; not done (one tone, as the WP asks).
- The selection bar sits right beside an attention/crisis stripe; in the
  shots it reads as one thicker bar (accent) or blue beside red (crisis).
  For WP-126's stripe/rounding sweep to look at.
- Nothing filed upstream (E54 is the operator's); numbers below.
- Accepted exception (orchestrator): the harness scripts keep their short
  private 0700 runtime dir under `/tmp` (`mktemp -d /tmp/seldon-rt.XXXXXX`,
  unix socket path length); not changed. Everything else of mine ran in
  on-disk dirs under `jax-seldon-private/gates/` (`TMPDIR`, a private
  0700 `XDG_RUNTIME_DIR`, `CARGO_TARGET_DIR=…/target-wp177`).

## Focus-border numbers for the operator's upstream report (E54)

Omarchy's default focus border (`focus-border-alpha = 0.25`, 1 px, the
foreground) on each of the dev host's 22 installed themes (node over
`Model.deskTones`; `SELDON_TONE_REPORT=1 node tests/plugin/model.test.js`
prints the first five columns). Definitions, so the report can state them:

- *focus/bg*: the border composited on the popup background; the lower of
  its contrast against that background and against Omarchy's normal fill
  (0.04) — `focusRatio`, what Seldon's yield rule tests against 3:1.
- *focus on its fill*: the border composited over the focus fill (0.08)
  that Omarchy draws inside it, against that fill.
- *ring/bg*, *ring/selected*: Seldon's ring (`ui`, 2 px) against the
  background and, since round 2, against the selected fill (0.18) of a
  primary button.

No theme's focus border reaches 3:1 either way; every ring does on all
four surfaces.

| Theme | fg/bg | muted/bg (old secondary text) | dim/bg (new) | focus/bg | focus on its fill | ring/bg | ring/selected |
|---|---|---|---|---|---|---|---|
| catppuccin | 11.34 | 2.46 | 7.54 | 1.79 | 1.92 | 5.18 | 3.26 |
| catppuccin-latte | 7.06 | 1.91 | 6.44 | 1.40 | 1.44 | 4.19 | 3.20 |
| ethereal | 13.67 | 4.90 | 6.97 | 1.73 | 1.94 | 4.91 | 3.32 |
| everforest | 7.38 | 1.55 | 7.21 | 1.61 | 1.71 | 4.85 | 3.23 |
| flexoki-light | 18.62 | 2.00 | 7.26 | 1.62 | 1.74 | 4.83 | 3.25 |
| gruvbox | 8.16 | 2.26 | 7.16 | 1.64 | 1.74 | 4.92 | 3.28 |
| hackerman | 17.45 | 1.59 | 7.69 | 1.94 | 2.17 | 5.20 | 3.24 |
| kanagawa | 11.26 | 2.23 | 7.51 | 1.78 | 1.92 | 5.16 | 3.26 |
| last-horizon | 19.07 | 2.45 | 7.78 | 1.99 | 2.26 | 5.33 | 3.26 |
| lumon | 12.06 | 1.68 | 7.74 | 1.85 | 1.97 | 5.30 | 3.24 |
| lupine | 15.43 | 2.57 | 6.82 | 1.54 | 1.65 | 4.61 | 3.21 |
| matte-black | 10.08 | 1.48 | 6.91 | 1.62 | 1.77 | 4.61 | 3.20 |
| miasma | 8.82 | 2.77 | 7.09 | 1.66 | 1.76 | 4.93 | 3.27 |
| nord | 9.25 | 1.69 | 7.65 | 1.75 | 1.83 | 5.29 | 3.28 |
| osaka-jade | 9.65 | 2.91 | 7.01 | 1.64 | 1.78 | 4.80 | 3.25 |
| retro-82 | 13.39 | 2.95 | 7.58 | 1.80 | 1.98 | 5.14 | 3.29 |
| ristretto | 10.95 | 2.82 | 7.72 | 1.81 | 1.93 | 5.24 | 3.24 |
| rose-pine | 6.66 | 1.48 | 6.18 | 1.39 | 1.43 | 4.25 | 3.26 |
| solitude | 11.56 | 2.24 | 7.20 | 1.71 | 1.88 | 4.88 | 3.27 |
| tokyo-night | 8.10 | 1.91 | 6.81 | 1.58 | 1.69 | 4.64 | 3.24 |
| vantablack | 21.00 | 4.89 | 7.46 | 1.91 | 2.25 | 4.96 | 3.21 |
| white | 21.00 | 3.95 | 7.23 | 1.69 | 1.82 | 5.10 | 3.34 |

No theme's own fg/bg is below 4.5; no tone was limited in any of the 22.

## How it was verified

- `node tests/plugin/model.test.js`: 203 passed — *fixture* (3 committed
  themes) and the 22 host themes on the dev host.
- `check-tokens.py --rules` on all 51 plugin QML files and
  `check-tokens.test.sh` 10/10 — *fixture*.
- `just qmllint` (incl. check-tokens with the shell) and
  `omarchy plugin validate plugin/` — dev host, pass.
- `bash tests/plugin/desk-view.sh` full run after the last plugin change:
  1982 passed, 0 failed, incl. `one-cursor` and the tone/ring cases —
  *headless* (offscreen Quickshell, private HOME and runtime dir).
- `DESK_SHOTS` renders in tokyo-night, kanagawa, catppuccin-latte, looked
  at (Changelog in tokyo-night and catppuccin-latte: dim text readable,
  selection = fill + bar + bold, chip bar) — *headless*.
- `SELDON_FULL_CHECK=1 just check` on the dev host, private HOMEs, a
  private 0700 `XDG_RUNTIME_DIR` and `TMPDIR` on disk, `CARGO_TARGET_DIR`
  on disk, `CARGO_BUILD_JOBS=4`: **check: ok** (23 min) — engine tests,
  clippy, fmt, fixtures (151 instances), qmllint 51 files, model.test 203,
  check-tokens.test 10, service-states 358, desk-view 1943 (without the
  shot cases), bar-view 196, ipc-restart 44 — *headless* for the
  harnesses. A first run failed in `schema-validate` (the new
  `fixtures/themes/roles.json` had no mapping); fixed in
  `scripts/validate-fixtures.py` (skipped like `state/recent-config.json`,
  its shape is checked by model.test.js), then the run above.
- shellcheck on `tests/plugin/check-tokens.test.sh` and the desk-view
  additions: *not run* locally (not installed); CI runs it.
- Test host, desktop: *not run* (the live look across themes is WP-126's
  sweep).

### Round 2, how it was verified

- `node tests/plugin/model.test.js`: 204 passed — *fixture* (3 committed
  themes) and the 22 host themes; the four mutants above each fail it.
- `bash tests/plugin/check-tokens.test.sh` 14/14 and `check-tokens.py
  --rules` on the 51 plugin QML files — *fixture*; `just qmllint` (with
  the shell token check) and `omarchy plugin validate plugin/` — dev
  host, pass; `validate-fixtures.sh` ok.
- `bash tests/plugin/desk-view.sh` full: 1948 passed, 0 failed, incl.
  `one-cursor`, `one-cursor-keys`, `tones-default`, `tones-tokyo` —
  *headless* (private HOME, on-disk `TMPDIR` and 0700 `XDG_RUNTIME_DIR`;
  the harness's own short `/tmp/seldon-rt.*` is the accepted exception).
  The no-handler mutant of M2 fails `one-cursor-keys` #5 and #8.
- `just check` not rerun this round (not asked); shellcheck *not run*
  (not installed; CI); test host *not run* (the orchestrator's live look).
