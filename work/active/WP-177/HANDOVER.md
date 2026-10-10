# WP-177 — handover

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
foreground) composited on each theme's popup background, against that
background; `node tests/plugin/model.test.js` with `SELDON_TONE_REPORT=1`
on the dev host's 22 installed themes. "ring" is what Seldon draws
instead (`ui` tone, 2 px). No theme reaches 3:1.

| Theme | fg/bg | muted/bg (old secondary text) | dim/bg (new) | focus border/bg | Seldon ring/bg |
|---|---|---|---|---|---|
| catppuccin | 11.34 | 2.46 | 7.54 | 1.79 | 3.86 |
| catppuccin-latte | 7.06 | 1.91 | 6.44 | 1.40 | 3.64 |
| ethereal | 13.67 | 4.90 | 6.97 | 1.73 | 3.69 |
| everforest | 7.38 | 1.55 | 7.21 | 1.61 | 3.81 |
| flexoki-light | 18.62 | 2.00 | 7.26 | 1.62 | 3.80 |
| gruvbox | 8.16 | 2.26 | 7.16 | 1.64 | 3.86 |
| hackerman | 17.45 | 1.59 | 7.69 | 1.94 | 3.83 |
| kanagawa | 11.26 | 2.23 | 7.51 | 1.78 | 3.84 |
| last-horizon | 19.07 | 2.45 | 7.78 | 1.99 | 3.86 |
| lumon | 12.06 | 1.68 | 7.74 | 1.85 | 3.91 |
| lupine | 15.43 | 2.57 | 6.82 | 1.54 | 3.73 |
| matte-black | 10.08 | 1.48 | 6.91 | 1.62 | 3.67 |
| miasma | 8.82 | 2.77 | 7.09 | 1.66 | 3.83 |
| nord | 9.25 | 1.69 | 7.65 | 1.75 | 4.04 |
| osaka-jade | 9.65 | 2.91 | 7.01 | 1.64 | 3.73 |
| retro-82 | 13.39 | 2.95 | 7.58 | 1.80 | 3.80 |
| ristretto | 10.95 | 2.82 | 7.72 | 1.81 | 3.93 |
| rose-pine | 6.66 | 1.48 | 6.18 | 1.39 | 3.60 |
| solitude | 11.56 | 2.24 | 7.20 | 1.71 | 3.79 |
| tokyo-night | 8.10 | 1.91 | 6.81 | 1.58 | 3.72 |
| vantablack | 21.00 | 4.89 | 7.46 | 1.91 | 3.66 |
| white | 21.00 | 3.95 | 7.23 | 1.69 | 3.90 |

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
