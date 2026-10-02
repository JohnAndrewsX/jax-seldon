WP-051 HANDOVER

Branch `wp/051-assets`, worktree `wt/WP-051`. Commits: `c310169` assets,
`75b00c8` plugin, `9f0f22d` docs, `73f67ad` design renders, plus this
handover. No PR, no push.

## Done

1. **`assets/`** (139 files + `sources/` 65): the 37 round-3 mask SVGs and
   their 37 sources over the round-2 delivery (rasters, brand files,
   overviews, specimens, the other 28 sources, `LICENSE`); `DELIVERY.md`
   copied unchanged (the round-3 version, which covers rounds 2 and 3);
   `LICENSE` copied unchanged from round 2; `assets/README.md` adapted
   from the designer's README: provenance, where the project uses each
   file, then the designer's file tables. Every file is listed (checked by
   script). `work/design/round-2/` and `round-3/` untouched as the record.
   2.4 MB in total; no file over 1 MB.
2. **Plugin** (`plugin/assets/`: 33 copies, 148 KB, no symlinks; a unit
   test fails if a copy differs from `assets/`):
   - `components/MaskIcon.qml`: one tinted mask. The SVG is read once via
     `FileView` (async, like index.json) and handed to `Image` as a data
     URL with the theme colour on the `<svg>` root (`Model.tintedSvg`). No
     colour constant in QML.
   - **Pill** (`BarWidget.qml`): A4 glyph + counts `2 · 3`. Box =
     `Style.bar.iconCanvas`; hinted `-16`/`-20` when the box in device
     px is 16/20, vector otherwise; glyph ink centre on the digits'
     centre, snapped to device pixels; glyph in the counts' colour (tone
     default/accent/urgent, dimmed). The shell's own label is hidden
     (`labelVisible: false`), so the WidgetButton keeps clicks, tooltip,
     dimming and the panel indicator. IPC `pill` gains `glyph`.
   - **Panel header** (`Panel.qml`): A5 lockup, `Model.panelMark` from
     the heading's cap height → 24 / 6 / 18 at the default font (the
     delivered metrics; measured in the render: box 24, wordmark origin
     x 30, baseline 18, mark centre = cap centre = 12.0).
   - **A11 pictograms**: status banner (`Banner.qml`) 48 px in the panel,
     96 px in the overlay; Today tab: the day's state (crisis › drift
     open › case active › all clear).
   - **A12**: Timeline canvas draws release diamond, snapshot dot, crisis
     spindle from the 16-grid path data (`Model.MARKER_PATHS`, a test
     keeps it equal to the files) and case spans between brackets
     (`[` start, `]` close; open cases none); legend with the marker files
     in the slot's title row in place of the subtitle.
   - The overlay hint now reads "Fix it from the Seldon panel (click the
     Seldon mark in the bar)."
   - `Model.pillText` returns the counts only (`""`, `2`, `· 3`,
     `2 · 3`); `Model.GLYPH` is gone. The Changelog source glyph for
     `seldon` events stays the text `⟡` (see "Not done").
3. **README hero**: `assets/a6-readme-hero-dark-1280x640.png` at the top
   of `README.md`; the plugin copy at the top of `plugin/README.md`; both
   replace the WP-046 "mark" and "hero" placeholder comments; the
   screenshot stays below the badges. Pill wording without `⟡` in both
   READMEs (outside the install sections). The WP-044 install sections are
   byte-identical (diffed).
4. **SPEC-PLUGIN** §4, §5, §6, §10 updated (wording below). TESTING.md:
   new §3c bar harness, `python3` for plugin-test, uninit renders, preview
   crop 538 / frame 500×578. CHANGELOG `[Unreleased]`: new `### Plugin`
   section and one `Packaging and docs` bullet. `fixtures/README.md` pill
   string.
5. **Bar harness** `tests/plugin/bar-view.sh` + `harness/bar.qml` +
   `png-ink.py` (stdlib PNG reader), in `just plugin-test`: the real
   BarWidget on the shell's own `PluginBarApi`, three themes × scale 1.0 /
   1.25 / 1.25 output, plus accent and default tone per theme and the
   not-initialised variant; check 4 from layout and from pixels, the tint
   (glyph pixels == pill colour), clean logs, real-home guard. 129 checks.
6. **Renders**: `plugin/preview.png` (155 KB) and `docs/images/*`
   re-rendered (panel tabs now show the header mark; crops +2 px, the
   shift the header causes); `work/design/round-3/screenshots/` (28
   files, 620 KB); result in `work/design/ROUND-3.md` §5.

## Check 4 (brief §6, offscreen record)

Harness renders, sample index (urgent tone), identical in Tokyo Night,
Catppuccin Latte and Osaka Jade. Pixel measurement, device px, ink rows
inclusive:

| Scale | Bar / text / box | File | Glyph rows | Digit rows | Glyph centre | Digit centre | Δ |
|---|---|---|---|---|---|---|---|
| 1.0 (font 12) | 26 / 12 / 16 | `a4-bar-glyph-16.svg` | 8–16 | 8–16 | 12.5 | 12.5 | 0.0 |
| 1.25 (font 15) | 33 / 15 / 20 | `a4-bar-glyph-20.svg` | 11–21 | 10–21 | 16.5 | 16.0 | 0.5 |
| 1.25 output, font 12 | 26 / 12 / 16 log. = 20 dev. | `a4-bar-glyph-20.svg` | 11–21 | 11–21 | 16.5 | 16.5 | 0.0 |

Layout side (logical px): glyph box top 5 / 7 / 5.6, glyph centre
12.50 / 16.50 / 13.20, digit centre (bar font metrics) 12.73 / 16.30 /
13.23 — matches the specimen table in DELIVERY §5 (box top 5 and 7,
glyph centre 12.50 and 16.50). Tint: glyph pixels exactly the pill colour,
e.g. urgent `#f7768e` / `#d20f39` / `#ff5345`, accent `#7aa2f7` /
`#1e66f5` / `#509475`, default `#a9b1d6` / `#4c4f69` / `#c1c497`.

**Live check pending, operator.** Read-only probe of the test host
(`omarchy-shell lock status`): `"locked":true`. Not unlocked, nothing
deployed there.

## SPEC wording (SPEC-PLUGIN, new or changed)

- §4: "`WidgetButton` showing the bar glyph (A4, the Prime Radiant mark)
  and the counts `A · D` … (hidden parts when 0: the glyph alone, `2`,
  `· 3`, `2 · 3`; a vertical bar shows the glyph only) … the glyph always
  takes the counts' colour." Plus a "Glyph (WP-051)" paragraph: box =
  `Style.bar.iconCanvas`, `Style.space(2)` before the counts, file by the
  box in device px (16 / 20 hinted, else vector), counts placed like the
  shell's label, glyph ink centre (row 7.5 of 16, 9.5 of 20, vector
  middle) on the digits' centre (baseline − half the ten digits' height),
  snapped to device px; check 4 within 1 px, `bar-view.sh`. Plus the
  MaskIcon paragraph: copies under `plugin/assets/` (ADR-0009), tint via
  the SVG root `color` as data URL, no colour in QML, and why not
  `MultiEffect` (not painted by the harness's software renderer).
- §5: "Header (WP-051)": A5 lockup, box = 2 × cap height rounded to an
  even pixel count, wordmark `round(cap / 2)` after it, baseline
  `box / 2 + cap / 2`; 24 / 6 / 18 at the default font; files by device
  px (24, 32, else `a1-icon-mask.svg`). "State pictograms": 48/96 grids,
  never below 48 px, the 48 grid up to 72 px; banner pictogram per status
  (contract mismatch and snapper: none); Today's day state with tones.
- §6: Row 1 gains the overlay banner (Copy only, the hint, 96 px
  pictogram). Row 4 gains the marker shapes (diamond / dot / spindle from
  `Model.MARKER_PATHS`, brackets for spans, open span without `]`) and the
  legend ("releases", "snapshots", "cases", "crises"; hides when it does
  not fit beside the caption).
- §10: "Reads one JSON file, and its own images under `plugin/assets/`
  (SVG and PNG artwork, no scripts; WP-051)."

## Not done

- **Live bar check on the test host: pending, operator's decision after
  this handover.** Read-only probe: session locked; not unlocked, nothing
  deployed. The offscreen check 4 above is the record.
- `docs/user/en|de` still show `⟡` in pill examples (03-daily-use,
  01-getting-started, 13-glossary): WP-049 owns `docs/user`. They need
  the same wording as the READMEs; `docs/images/panel-tokyo-night-*.png`
  they embed are already re-rendered.
- The Changelog's source glyph for `seldon` events is still the text
  `⟡` (a Nerd-Font-style text column next to the other sources' glyphs);
  not one of the brief's surfaces. Replacing it with the A4 image would
  mean an image column in EventRow/DriftSheet.
- `docs/DESIGN-BRIEF.md`, `docs/CONCEPT.md`, `docs/seldon-concept.html`
  still describe `⟡` as the placeholder; they are the brief/concept of
  record, left unchanged.
- Favicons (A9): only in `assets/`, as the WP says. A7/A8/A6 uploads,
  A10 vault icon: operator.
- The A6 hero's right 45 % is empty by design ("we overlay screenshots");
  the READMEs show it as delivered with the screenshot below. A composed
  hero (lockup + screenshot) would be a new asset.

## Verified by

- `just check` → **still running at handover** (finish was ordered
  first). Passed in that run so far: fmt-check, clippy, test (52 cargo
  test results ok, 0 failed), check-watch, check-packaging (shellcheck
  not installed, `bash -n` only), check-install (106 passed),
  schema-validate, docs-check, plugin-validate, qmllint (29 files),
  model.test.js (80), model bench, real-home-guard.test (11), and
  service-states up to `fix-contract` with 0 FAIL lines. Not reached
  then: the rest of service-states, panel-view, overlay-view, bar-view —
  each passed on its own on the final code (below). The reviewer should
  re-run `just check`.
- `omarchy plugin validate plugin/` → exit 0, no output.
- `just qmllint` → `qmllint: ok (29 files)`.
- `node tests/plugin/model.test.js` → 80 passed (6 new asset tests).
- `bash tests/plugin/bar-view.sh` → 129 passed, 0 failed.
- `bash tests/plugin/panel-view.sh` (with `PANEL_FIT_SHOTS`) → 886
  passed; with `PANEL_SHOTS` → 715 + the uninit renders; plain run part
  of `just check`.
- `bash tests/plugin/overlay-view.sh` (with `OVERLAY_SHOTS`) → 355
  passed, 0 failed.
- `bash tests/plugin/service-states.sh` → 189 passed.
- Real-home guard green in every harness run ("the real
  ~/.local/state/seldon and ~/.config/seldon are untouched").
- `find plugin -type l` → 0; `find plugin assets -size +1M` → none.
- `just docs-check` → ok (378 links); install sections diffed identical.

## Learned (memory/pitfalls.md, WP-051 section)

Software-renderer harness paints no `MultiEffect`; an id that shadows a
property name breaks every binding silently (qmllint blind); fractional
window heights stretch grabs; Repeater delegates must not anchor to
`parent`; assert rule outcomes, not font metrics; header changes shift
every panel crop.

## Decisions needed

1. **Tint mechanism.** The WP said to use the shell's mechanism. Code is
   truth: the shell tints symbolic tray icons with `MultiEffect {
   colorization: 1.0 }` (`plugins/bar/widgets/Tray.qml`). In the
   headless harness (`QT_QPA_PLATFORM=offscreen`) Qt Quick uses the
   software backend, where `MultiEffect` paints nothing (tested: empty
   output; Vulkan/GL RHI cannot initialise offscreen), so the three-theme
   renders the acceptance asks for would show no mark. I used the tint the
   designer built the masks for instead (F1: the root `color`), which
   renders identically in the harness and the shell, with theme colours
   only. Confirm, or ask for `MultiEffect` in the live path plus this as
   the software fallback (two code paths, the harness would test only one).
2. **Where the four non-banner pictograms live.** The brief names the
   states, not the places. I put crisis / drift open / case active / all
   clear on the Today tab (the day's state) and nothing in the crisis strip
   (48 px would triple its height). Confirm or redirect.

## Touched outside WP scope

`fixtures/README.md` (one pill string in the variants table); the
`docs/images/panel-tokyo-night-*.png` renders embedded by `docs/user`
(images only, no `docs/user` file edited); `tests/plugin/*` and
`justfile` `plugin-test` (the new harness). No engine, packaging,
install.sh or docs/user change.
