# Round 3 — acceptance report on round 2 (Prime Radiant)

Report date: 2026-10-02. Round 2 is **accepted** with one correction
and a list of readings we confirm. Round 3 is small: fix F1, deliver the
corrected files with the same names, update `DELIVERY.md` §6.

## 1. Checks run (brief §6, ROUND-2 §4)

| Check | Result |
|---|---|
| 1 `rsvg-convert` on all 130 SVGs (65 optimised + 65 sources) | no warnings; delivered PNGs match fresh renders pixel for pixel (sampled: A1 16/32/1024 black, A2 1024, A6 1280, A7 1024, A9 512 — all 0 differing pixels) |
| 2 mask rules (A1, A3, A4, A5, A11, A12) | `currentColor`/`none` only, no opacity below 1, no `<style>`, no scripts, no `<image>`, no external references, kebab-case ids — but see F1 |
| 3 16 px and 22 px legibility | passed by us at 16× magnification on Tokyo Night and Catppuccin Latte; the three-person side-by-side is still to be done |
| 4 A4 in the real bar | **pending on our side** (plugin integration WP-051 on the test host); the specimen's metrics are accepted as the expected result |
| 5 metadata | no text, time or software chunks in any PNG; ICO holds 16/32/48; no personal data found by grep |
| 6 XML lint, external references | `xmllint` clean on all 130 files; 0 `href` attributes |
| 7 completeness | every row of ROUND-2 §2 present in every size and variant; viewBoxes start at `0 0`; brand files use exactly `#0d1326`, `#eef1fb`, `#b86e23` |
| names | ASCII kebab-case throughout; no spaces |
| A11/A12 at smallest size | read on both themes in the overview sheets |

## 2. Findings

**F1 — fix (round 3).** 29 of the 37 mask SVGs carry the fallback
`color="#000"` on `<g id="mask">`; the 8 hand-hinted files
(`a1-icon-mask-16/22/24/32`, `a4-bar-glyph-16/20`, `a5-panel-mark-24/32`)
do not. A consumer that tints by setting `color` on the root (CSS
`svg { color: … }`, which is how a stylesheet-driven renderer does it)
cannot override the 29: rendering `a1-icon-mask.svg` with
`svg{color:#fff}` still produces black. Please put the fallback on the
`<svg>` root in every mask (or drop it everywhere), so the whole set
behaves the same. Nothing else changes.

**F2 — watch, no change.** `index-missing`, `index-stale` and
`drift-open` differ only by the corner modifier (ring, open ring, dot).
They are distinguishable at 48 px and up, which is where the plugin
uses them. We will not use them below 48 px.

**F3 — accepted as delivered.** The crisis exclamation (and the reasons
for rejecting tips-only), the A3 recommendation (single-tone mask as
the default, A3 only for quiet large states), A10 in both tints, the A9
container, the lockup metrics (2.4 × cap height), and all four readings
in DELIVERY §8. No GitHub issues needed.

## 3. What happens next on our side

- Round-2 files are recorded under `work/design/round-2/`; after round
  3 they move to `assets/` by WP-051 (README hero, plugin bar glyph
  and header mark, state pictograms, chart markers, favicons).
- Check 4 runs on the test host as part of WP-051; the result goes into
  this file.
- The GitHub avatar, social preview and marketplace image are uploaded
  by the operator.

## 4. Round 3 received and accepted (2026-10-02)

Delivery: 76 files (37 optimised masks, 37 sources, README, DELIVERY).
Checks: every mask carries `color` on the `<svg>` root and nowhere
else (37/37, sources 37/37); a root stylesheet `svg{color:#fff}` now
tints all 37 (render identical to a `currentColor` substitution);
black renders identical to round 2 (geometry unchanged); xmllint and
rsvg-convert clean on all 74; mask rules unchanged. **F1 closed.**
Round 3 is the mask set of record; rasters and brand files stay round 2
(byte-identical per DELIVERY). Nothing further is open for the
designer; check 4 in the real bar follows from WP-051.

## 5. Check 4 result (WP-051, 2026-10-02)

**Passed offscreen; live check pending, operator.** The plugin's real
`BarWidget.qml` rendered by the headless bar harness
(`tests/plugin/bar-view.sh`, the shell's own `PluginBarApi`, sample index,
urgent tone) in Tokyo Night, Catppuccin Latte and Osaka Jade. Glyph and
digit centres measured from the pixels of each render (device px; ink rows
inclusive):

| Scale | Bar / text / box | File | Glyph ink rows | Digit ink rows | Glyph centre | Digit centre | Difference |
|---|---|---|---|---|---|---|---|
| 1.0 (font 12) | 26 / 12 / 16 | `a4-bar-glyph-16.svg` | 8–16 | 8–16 | 12.5 | 12.5 | 0.0 px |
| 1.25 (font 15) | 33 / 15 / 20 | `a4-bar-glyph-20.svg` | 11–21 | 10–21 | 16.5 | 16.0 | 0.5 px |
| 1.25 output (font 12) | 26 / 12 / 16 logical = 20 device | `a4-bar-glyph-20.svg` | 11–21 | 11–21 | 16.5 | 16.5 | 0.0 px |

Identical in all three themes; the layout's own numbers (glyph box top 5
and 7, glyph centre 12.50 and 16.50, digit centre 12.73 and 16.30 from
the bar font's metrics) agree with the specimen table in `DELIVERY.md` §5.
The hinted glyph's pixels are exactly the pill's colour in each theme and
tone. Renders: `round-3/screenshots/` (`bar-<theme>-<scale>.png`,
`bar-zoom-6x.png`, `bar-tones-zoom-6x.png`, and the panel and overlay in
the three themes). The live bar on the test host was not checked: the
session was locked; unlocking is the operator's.
