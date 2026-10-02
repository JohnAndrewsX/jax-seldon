# Delivery notes, round 2

This round delivers the full asset set for the chosen concept, Concept 1 "Prime Radiant". It follows the brief (`docs/DESIGN-BRIEF.md`, including §6a) and the round-2 order (`ROUND-2.md`). Every file is listed in `README.md`.

## 1. Decisions

### The mark

The construction is exactly as on round-1 sheet 1; nothing was redrawn.

- **Hollow envelope.** It stays hollow everywhere. The filled four-point star is never used (the "AI sparkle" problem).
- **Mask is the default.** Use the single-tone mask (A1) everywhere in the shell.
  - Deliverable `a3-icon-two-tone.svg` exists as ordered, but at 16 to 32 px a dimmed core loses the point of the mark: the core is the "present state" at its centre.
  - On the dark themes a dimmed core reads as switched off rather than as depth (see `a3-icon-two-tone-comparison.png`).
  - **Recommendation:** use A3 only if the shell needs a quieter large version (48 px and up), for example in an empty state.

### Brand

- **Palette.** Three colours (section 4). Ember appears once, on the core: the light at the centre of the Prime Radiant.
- **Variants.** Each brand asset has a version for dark backgrounds (primary) and one for light backgrounds, using the same three colours.
  - The suffix names the background the file is made for: `on-dark` / `on-light` for icons, `dark` / `light` for compositions.
  - So `a2-icon-brand-on-dark` has a pale envelope for dark backgrounds.

### Lockup (A5, A6, A8)

- **Mark size.** Icon box = 2.4 × the wordmark's cap height, which makes the mark's ink 2.1 × the cap height. At 2.0 the thin envelope looked lighter than the bold wordmark.
- **Alignment.** The icon's centre sits on the cap-height centre.
- **Spacing.** The wordmark starts 0.45 × cap height after the mark's ink.
- **Wordmark.** "Seldon" set in JetBrains Mono Bold, the system monospace and the weight the panel header already uses. No custom lettering.

### A11 state pictograms

They form one family built only from the mark's parts. The concave sides leave the four corners of the square empty, so a single modifier sits in the bottom-right corner without shrinking the envelope.

| State | Picture | Reading |
|---|---|---|
| all clear | envelope + core | the mark: nothing pending |
| engine missing | envelope broken at the four waists (22° gaps), no core | the frame is there, the instrument is not |
| logbook not initialised | envelope + hollow core | an empty record |
| index missing | mark + ring in the corner | an empty index |
| index stale | mark + open ring (80° gap) in the corner | needs a refresh |
| drift open | mark + dot in the corner, outside the envelope | a point outside the plan |
| case active | mark + bar in the corner | a span of planned work, the same picture as the case spans in the timeline |
| crisis | exclamation mark made of the mark's south tip (26°) and its diamond core | the strongest shape for the most urgent state |

The order allowed crisis to use "the tips only". That was tried and rejected:

- Four tips around an empty centre read as the "fullscreen / expand" icon, and with the core as the "move" cursor.
- Two separated tips read as "sort".
- Longer tips merge into the filled star.

The exclamation keeps the language (one tip and the core) and is unambiguous at 48 px and 24 px.

### A12 legend markers

Distinguishable by shape alone:

- **Release:** convex diamond (as the chart draws it today).
- **Snapshot:** dot (as today).
- **Case span start / end:** brackets `[` and `]`.
- **Crisis:** the Prime Radiant tip pair, a narrow concave spindle.

Today release and crisis are both diamonds and differ only in colour; that is fixed. A first version drew the case markers as a tick with the span's bar, which read as a hammer at 16 px, so it was replaced by brackets.

### Other decisions

- **A10.** The brief asks for one PNG. It is delivered in white and in black like A1, because vault switchers are themed both ways.
- **A9.** The favicons sit in the A2 container (Night, radius 192/1024), so they read on light and dark browser chrome. The marks inside are the hand-hinted grids:
  - 16 px: the 11 × 11 bar glyph
  - 32 px: the 22 px grid
  - 48 px: the 32 px grid

## 2. Construction

| Item | Value |
|---|---|
| Master | 1024 grid, 64 safe margin, key points on the 32 grid |
| Tips | on the margin: (512, 64), (960, 512), (512, 960), (64, 512) |
| Sides | circular arcs through two tips and the point inset 64 from the straight side, e.g. (672, 352); tip angle 26° |
| Envelope | outline offset 64 inward (= the one stroke width) |
| Core | diamond, 192 across, centred |
| Radius family | 32 / 64 / 128; the container radius 192 is the order's value for A2 |
| Pictograms (A11) | same construction scaled: stroke = margin = arc inset = grid / 16 (6 on the 96 grid, 3 on the 48 grid); core 18 / 9 |
| Markers (A12) | on whole pixels where straight: brackets 2 px |

## 3. Optical corrections

- **Hinted A1 grids.** Every hinted grid is mirrored in all eight symmetries of the square, so the north and east tips are identical.
  - 16 px: odd 15 × 15 with a centre pixel, which leaves a 0.5 px offset (approved in round 1).
  - 22 / 24 / 32 px: even designs centred on the pixel boundary, with no offset.
  - 32 px: the envelope is 2 px, exactly 64 units.
  - 22 / 24 px: the envelope is 1 px, slightly lighter than 64 units, because 1.4 px is not drawable and 2 px was too heavy.
- **Tips at small sizes.** Lengthened to 2 to 3 px so they do not grey out.
- **Bar glyph size.** It matches the digit height (0.73 em) instead of the cap height of a pointed shape, so it carries no overshoot.
- **Lockup mark size.** 2.4 × cap height (see above).
- **Pictogram balance.** The corner modifiers move the optical centre down and right by about 5 % of the grid. This is intended: the modifier is the message.
- **Crisis weight and balance.** The exclamation has less ink than the median pictogram (8.3 % against 15.8 %); its strength comes from its shape. It is centred by its bounding box (31 × 70 on the 96 grid). Its optical centre sits higher (y 37 of 96), as an exclamation's always does.

## 4. Colour values

| Name | Hex | Use |
|---|---|---|
| night | `#0d1326` | brand background (deep night blue) |
| pale | `#eef1fb` | the mark and wordmark on night; the background of the light versions |
| ember | `#b86e23` | warm accent, the core only; amber, far from any theme's `urgent` red |

Contrast (WCAG, graphical objects need 3 : 1):

| Pair | Ratio |
|---|---|
| pale on night | 16.4 : 1 |
| ember on night | 4.6 : 1 |
| ember on pale | 3.5 : 1 |

All pass on both backgrounds; see `a2-brand-palette.png`.

The mask files carry no colour. Specimens and comparisons use the real Omarchy theme colours (background / foreground from `colors.toml`):

| Theme | Background / foreground |
|---|---|
| Tokyo Night | `#1a1b26` / `#a9b1d6` |
| Catppuccin Latte | `#eff1f5` / `#4c4f69` |
| Osaka Jade | `#111c18` / `#c1c497` |

The A3 comparison renders the "dimmed foreground" as 50 % between foreground and background.

## 5. Metrics

### A4 bar glyph

Measured from the Omarchy shell:

- bar 26 px tall
- pill text JetBrainsMono Nerd Font, 12 px
- icon canvas 16 px

| Scale | Bar | Text | Baseline | Glyph box (top) | Glyph | Glyph centre | Digit centre | Difference |
|---|---|---|---|---|---|---|---|---|
| 1.0 | 26 | 12 px | 17 | 16 (5) | 9 × 9 hinted | 12.50 | 12.62 | 0.12 px |
| 1.25 | 33 | 15 px | 22 | 20 (7) | 11 × 11 hinted | 16.50 | 16.53 | 0.03 px |
| 1.5 | 39 | 18 px | 26 | 24 (7) | vector `a4-bar-glyph.svg` | 19.00 | 19.43 | 0.43 px |
| 2.0 | 52 | 24 px | 35 | 32 (10) | vector | 26.00 | 26.24 | 0.24 px |

- **Glyph box.** Placed 2 px before the counts, as the shell would draw it.
- **Vector glyph.** `a4-bar-glyph.svg` is a 16-unit box; scale the box with the bar. Its glyph is 9 units tall: one unit of envelope and a 1.5 unit core.
- **Before.** The current `⟡` comes from Adwaita Mono (the fallback) at about 5 px. It is reproduced in the specimen as "current".

### A5 panel header

The mark is the hinted A1 grid of the box size.

| Icon box | Wordmark (placeholder) | Cap height | Box right edge to text origin | Baseline below box top |
|---|---|---|---|---|
| 24 px | JetBrains Mono Bold 16 px (the panel's `Style.font.heading`) | 11.7 px | 6 px | 17.84 px (round to 18) |
| 32 px | JetBrains Mono Bold 22 px | 16.1 px | 8 px | 24.03 px (round to 24) |

In both, the icon centre sits on the cap-height centre: baseline = box top + box / 2 + cap / 2.

### A6 hero and A8 listing image

**A6, 1280 × 640.**

- The lockup is vertically centred at y = 320, with cap height 76.
- The mark spans x 107 to 267; the wordmark spans x 307 to 668.
- Everything right of x = 704 (the right 45 %) is empty.

**A6, 1600 × 800.** The 1280 × 640 composition unchanged, centred. The safe area is x 160 to 1440, y 80 to 720. The wordmark ends at x = 828, and the right 45 % (from x 880) is empty.

**A8, empty tagline area (one line):**

| Size | Tagline area | Lockup |
|---|---|---|
| 1200 × 630 | x 150, y 385, 900 × 44 | cap height 60, centred above |
| 800 × 600 | x 80, y 356, 640 × 36 | cap height 48, centred above |

### Other sizes

- **A7.** The mark's ink is 56 % of the width, so the tips stay at 56 % of the radius of a circular crop.
- **A9 apple-touch.** Mark ink 55 % of the width, full-bleed night.
- **A9 maskable.** Mark ink 48 %, inside the 80 % safe zone.

## 6. Checks run

| Check | Result |
|---|---|
| `rsvg-convert` renders every SVG (65 optimised + 65 sources) | no warnings |
| PNGs rendered from the optimised SVGs | they match by construction; masters and compositions also show 0 differing pixels between Photon and rsvg |
| Masks | `currentColor` only, no opacity below 1, no `<style>`, no scripts, no `<image>`, no external references, kebab-case ids |
| SVG | `version="1.1"`, viewBox starting at `0 0` |
| Names and metadata | ASCII kebab-case names; no metadata chunks or dates in any PNG or ICO; no home paths or user names in any file |
| Checker | `tools/check.py` (round-2-work), run on the whole folder including these notes: 204 files, 0 failing |
| A11 / A12 legibility | read at their smallest size (A11 at 48 and 24, A12 at 16 and 12) on Tokyo Night and Catppuccin Latte; see the overview PNGs |

`exiftool` is not installed on the build machine. The PNG check reads the chunks directly, and every PNG and the ICO were stripped with ImageMagick.

Not done by us: check 4 in the real bar on the test host.

## 7. Tools

- **Photon Studio 0.1.31** for the layered design documents: masters, compositions, specimens, overviews.
- **Python 3**, standard library only, for the geometry. This includes a small TrueType reader for the wordmark outlines, which come from JetBrains Mono Bold under the SIL OFL.
- **SVGO 3.3.5** (via `npx`) for optimisation.
- **rsvg-convert 2.x** for rendering.
- **ImageMagick 7** for magnification, the ICO and metadata stripping.

### Tool findings

- **Photon text to shape.** It produces flattened outlines, about 111 KB for "Seldon". We read the font's own quadratic outlines instead (2.4 KB), and they match Photon's text rendering with 0 differing pixels.
- **Photon layer stacking.** Groups created before a batch end up below the batch's layers. The build creates one group per part, in order.
- **SVGO.** It drops `version="1.1"`, so the build restores it.

## 8. Ambiguities

Each has the reading we chose. They are not filed as GitHub issues yet; we can open one per question if you want them on record.

- **A6 / A8 and "viewBox `0 0 N N`" (§4).** Read as "origin 0 0". The hero and listing images are not square by definition: they use `0 0 1280 640` and so on. Icons and masks are square.
- **A5 "24–32 px cap height".** Read like the A4 clarification in §6a, as icon boxes of 24 and 32 px. A literal cap height of 24 to 32 px would mean a 33 to 44 px wordmark, far larger than the panel's 16 px heading. The metrics above scale either way: box = 2 × cap height at the panel sizes.
- **A6 "1600×800 safe-area version".** Read as the 1280 × 640 composition centred in a 1600 × 800 canvas, so crops keep the lockup.
- **A2 "dark- and light-background variants".** Read as variants made for dark and for light backgrounds: the mark's colours flip, the background stays transparent (or is the container).

## 9. Not in this delivery

- The layered Photon documents (`.psd`) and the build scripts stay with the designer, like the round-1 sketches. They are available on request, in `round-2-work/` (`design.py`, `build.py`, `tools/`, `photon/`, `review/`).
- Check 4 in the real bar is for your test host. The specimen shows the expected result.
- Round 3 (corrections from your acceptance checks) follows your report.

## 10. Rebuild

In `round-2-work/`, run:

```sh
python3 tools/set_sheet.py .
python3 build.py
python3 build.py extras
python3 docs.py
```

Photon Studio must be running.
