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
