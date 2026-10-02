WP-051 HANDOVER

Branch `wp/051-assets`, worktree `wt/WP-051`, rebased onto main
(`b28265b`, after "release: prepare 0.1.1"). Commits: `f7a9216` assets,
`2ceac89` plugin, `a041d28` docs, `76aa96e` design renders, `0d50661`
handover; fix round: `5b712f1` user guide en+de, `94ed1b7` German source
lines, `3f6df35` review fixes, plus this update. No PR, no push.

## Fix round (review SEND BACK, 2026-10-02)

- **B2** Rebased onto main. CHANGELOG: my Plugin bullets moved into the
  existing `[0.1.1]` `### Plugin` list (after the WP-039/ADR-0024 ones),
  my Packaging bullet into its `### Packaging and docs` list (after
  WP-049's); `[Unreleased]` is empty, one `### Plugin` heading per
  version, no blank line inside a list;
  `bash packaging/release-notes.sh 0.1.1 CHANGELOG.md` prints one tight
  body. `memory/pitfalls.md`: both sides kept, main's WP-049 section
  first.
- **B1** `tests/integration/e2e.sh` `pill_of` builds the counts only,
  `[a, "· d"] | join(" ")` with zero parts left out (checked by hand for
  `2 · 4`, `· 3`, `2`, `""`); comment and `docs/TESTING.md` (the e2e
  compare step) say so; the asserts (`plugin pill`, `panel pill text`,
  `service pill`) compare against `pill_of`, so they now expect the
  counts. `bash -n` clean. **`just e2e` not run: it needs the test host.**
- **B3** User guide en and de (01-getting-started, 03-daily-use,
  13-glossary): the README wording, the Seldon mark ("Seldon-Zeichen")
  then `A · D`; no `⟡` left in `docs/user`. German source lines re-stamped
  at `5b712f1` in a second commit; `bash scripts/docs-check.sh` → ok, no
  warning.
- **M1** `bar-view.sh`: the layout half of check 4 is gone (the glyph is
  placed by that formula); check 4 is the pixel measurement only, now also
  in the accent/default tone cases. The widget's centres stay in the
  report, labelled "for the record only". TESTING §3c updated.
- **M2** `Model.pictogramFile` / `markerFile` comments and SPEC-PLUGIN
  §5/§6: sizes in logical pixels (the vector scales with the DPR). No code
  change. (`barGlyph` and `panelMark` do take device pixels: their callers
  multiply by the DPR to choose the hinted file.)
- **N1** The five `docs/images/panel-tokyo-night-*` tab crops are palette
  PNGs again (`-strip -colors 64 PNG8:`, 7.6–15.2 KB each, like main);
  the overlay and Today renders stay truecolor as on main.
- Orchestrator decisions recorded: root-colour tint accepted (no
  MultiEffect path); Today-tab placement accepted; the live bar check is
  not a release blocker; the hero ships as delivered in 0.1.1; the
  Changelog text glyph `⟡` for seldon events stays.

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
   crop 538 / frame 500×578. CHANGELOG `[0.1.1]`: four `### Plugin`
   bullets and one `Packaging and docs` bullet (see B2).
   `fixtures/README.md` pill string.
5. **Bar harness** `tests/plugin/bar-view.sh` + `harness/bar.qml` +
   `png-ink.py` (stdlib PNG reader), in `just plugin-test`: the real
   BarWidget on the shell's own `PluginBarApi`, three themes × scale 1.0 /
   1.25 / 1.25 output, plus accent and default tone per theme and the
   not-initialised variant; check 4 from the pixels, the tint (glyph
   pixels == pill colour), clean logs, real-home guard.
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

This pixel measurement is check 4. For the record only (not a check,
the glyph is placed by this formula): the widget's own numbers, logical
px — glyph box top 5 / 7 / 5.6, glyph centre 12.50 / 16.50 / 13.20,
digit centre from the bar font's metrics 12.73 / 16.30 / 13.23 — agree
with the specimen table in DELIVERY §5 (box top 5 and 7, glyph centre
12.50 and 16.50). Tint: glyph pixels exactly the pill colour,
e.g. urgent `#f7768e` / `#d20f39` / `#ff5345`, accent `#7aa2f7` /
`#1e66f5` / `#509475`, default `#a9b1d6` / `#4c4f69` / `#c1c497`.

**Live check pending, operator** (not a release blocker, orchestrator
decision). Read-only probe of the test host (`omarchy-shell lock
status`): `"locked":true`. Not unlocked, nothing deployed there.

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
  never below 48 px, by logical size the 48 grid up to 72 px; banner pictogram per status
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
- `just e2e` (B1's change) not run: it needs the test host.
- The Changelog's source glyph for `seldon` events stays the text `⟡`
  (orchestrator decision).
- `docs/DESIGN-BRIEF.md`, `docs/CONCEPT.md`, `docs/seldon-concept.html`
  still describe `⟡` as the placeholder; they are the brief/concept of
  record, left unchanged.
- Favicons (A9): only in `assets/`, as the WP says. A7/A8/A6 uploads,
  A10 vault icon: operator.
- The A6 hero ships as delivered in 0.1.1 (right 45 % empty by design);
  a composed hero with a screenshot is a later WP (orchestrator).

## Verified by

Fix round, on the rebased branch (`3f6df35`):

- `just check` → `check: ok`, exit 0. In it: fmt-check, clippy, 54 cargo
  test results ok and 0 failed (incl. `--features watch`),
  check-packaging ok (shellcheck not installed, `bash -n` only),
  install.test 132 passed, validate-fixtures ok, docs-check ok (380
  links, 14 translated pages, no warning), plugin-validate ok, qmllint ok
  (29 files), model.test.js 80 passed, real-home-guard.test 11 passed,
  service-states 189, panel-view 688, overlay-view 319, bar-view 120 —
  all 0 failed; the real-home guard green in every harness.
- `bash packaging/release-notes.sh 0.1.1 CHANGELOG.md` → one body, one
  `### Plugin` list including the four WP-051 bullets, no blank line
  inside a list.
- `bash -n tests/integration/e2e.sh` clean; `pill_of` checked by hand on
  four summaries. `just e2e` not run (test host).
- `bash scripts/docs-check.sh` → ok, no warning (German source lines at
  `5b712f1`).

First round (still valid): `PANEL_FIT_SHOTS` panel-view 886 passed,
`OVERLAY_SHOTS` overlay-view 355 passed, `omarchy plugin validate
plugin/` exit 0, `find plugin -type l` → 0, `find plugin assets -size
+1M` → none, install sections of both READMEs diffed identical.

## Learned (memory/pitfalls.md, WP-051 section)

Software-renderer harness paints no `MultiEffect`; an id that shadows a
property name breaks every binding silently (qmllint blind); fractional
window heights stretch grabs; Repeater delegates must not anchor to
`parent`; assert rule outcomes, not font metrics; header changes shift
every panel crop (and fresh crops are truecolor: shrink to palette); a
rebase onto a release commit moves `[Unreleased]` bullets into the dated
section.

## Decisions needed

None open. Both questions of the first round were decided by the
orchestrator (accepted as built); kept below for the record.

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
`docs/images/panel-tokyo-night-*.png` renders embedded by `docs/user`;
`docs/user/en|de` 01/03/13 (B3, after WP-049 merged, by review order);
`tests/integration/e2e.sh` `pill_of` (B1); `tests/plugin/*` and
`justfile` `plugin-test` (the new harness). No engine, packaging or
install.sh change.
