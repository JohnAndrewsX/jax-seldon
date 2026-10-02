# Seldon icon concepts, round 1

Three concept sheets as the brief asks for in §7, one page each. Every page shows:

- the idea
- the icon at 1024 (with its construction) and at 16 px
- the mask tinted on a dark and a light theme
- the bar glyph specimen at 1.0 and 1.25 next to `2 · 3`

The primary comes first, per the order of preference in §2.

| Sheet | Concept | Role |
|---|---|---|
| `concept-1-prime-radiant.png` | Prime Radiant: hollow concave diamond (the `⟡` shape) with a diamond core | primary |
| `concept-2-plan-vs-reality.png` | Plan versus reality: dashed plan, solid line that drifts and returns | alternate A |
| `concept-3-flight-recorder.png` | Flight recorder: recorder dial, stepped trace, recording head | alternate B |

Sheets are 2560 × 1600 PNG.

## Files

| Path | What it is | How it was produced |
|---|---|---|
| `concept-*.png` | the three concept sheets | flattened full-size export of the layered Photon sheet (`document.save`, PNG) |
| `sources/concept-*-mask-1024.svg` | vector mask master per concept: SVG 1.1, viewBox `0 0 1024 1024`, `currentColor` only, presentation attributes, no style blocks, no external references | written from the shared geometry in `tools/concepts.py`, verified against Photon's render (0 differing pixels beyond anti-aliasing) |
| `sources/concept-*-master.psd` | layered master per concept: `mask` group (one named shape layer per element), hidden `construction` group, guides at 64 / 512 / 960 | Photon Studio, editable Bézier shape layers |
| `sources/concept-*-sheet.psd` | layered concept sheet (groups `paper`, `master-1024`, `idea`, `hinted-16`, `mask-themes`, `bar-specimen`, `header`, `footer`) | Photon Studio |
| `sources/concept-*-bar-specimen.psd` | the bar specimen at true pixel size | Photon Studio, text in JetBrainsMono Nerd Font and Adwaita Mono |
| `sources/concept-*-mask-1024-photon.png` | Photon render of each master, the reference for the SVG comparison | Photon `render` |
| `sketches/` | ideation thumbnails and intermediate renders; not deliverables | `tools/sketch.py`, `tools/preview_pixels.py`, rsvg-convert, ImageMagick |
| `../tools/` | the build scripts: geometry, hand-hinted pixel grids, Photon builders | Python 3 (standard library only) driving the Photon CLI |

To rebuild, keep Photon Studio running, then:

```sh
python3 tools/build_masters.py
python3 tools/build_sheets.py
```

## Construction

All three concepts share one set of rules, so the chosen one carries straight into round 2:

- **Grid:** 1024 grid, 64-unit safe margin, key points on the 32-unit grid. At 32 px, one grid step is exactly one pixel.
- **Stroke:** one width, 64 units (2 px at 32 px, 1 px at 16 px). Round caps and round joins throughout.
- **Corner radii:** one family, 32 / 64 / 128. The 32 radius is half the stroke, as at caps and joins; concept 2's fillets use 128.
- **Colour:** single colour, no opacity steps, no gradients, no effects.

Each concept:

1. **Prime Radiant.** Tips sit on the safe margin. Each side is a circular arc through two tips and a 32-grid point inset 64 units from the straight side, e.g. (672, 352). That gives a 26° tip angle. With an inset of 96 the tips became 3° needles that vanish at 16 px, so it was rejected. The envelope is that outline offset 64 inward, and the core is a 192-unit diamond.
2. **Plan versus reality.** The reality line is built from straight runs joined by r128 fillets. A plain S-curve collapsed the stroke's inner edge, which is why it was replaced. The plan's dashes are 32 units long with round caps, on a 160 period. The return node (r64) sits on that dash lattice. Dashes are separate segments rather than a dash pattern, so every renderer draws them identically.
3. **Flight recorder.** A ring at 448 / 384, and a trace on the 32-unit grid that starts inside the ring band (a clean junction, no notch). The recording head is r80.

## Small sizes

**The 16 px icons are drawn by hand on the pixel grid** (`tools/pixels.py`), not downsampled. Each sheet shows the hinted 16 px next to a plain downsample of the master to show the difference. The plain downsample greys out concept 1's tips and turns concepts 2 and 3 to mush.

- **Concept 1 at 16 px:** the shape is 15 × 15 with a 1 px envelope. The tips are 3 px long, and the stepped sides go steep, then 45°, then flat, which keeps the concavity visible. The core is a 3 px plus (a 3 px diamond).
- **Concept 3 at 16 px:** the recording head is a 3 × 3 block, because a plus reads as "add" at that size.

## Bar glyph (A4) specimen

Measured from the shell (`/usr/share/omarchy/shell`):

- the bar is 26 px tall
- the pill text is the theme font (JetBrainsMono Nerd Font) at 12 px
- the icon canvas token is 16 px

The specimen draws the strip at true pixel size, then magnifies it 3× with nearest neighbour. Placement:

- **Glyph box:** 16 px at 1.0, 20 px at 1.25, centred on the bar.
- **Gap:** 2 px between the box and the counts.
- **Counts:** `2 · 3` in JetBrainsMono Nerd Font, 12 px at 1.0 and 15 px at 1.25.

| Scale | Text | Digit height (0.73 em) | Concept 1 glyph | Glyph centre vs digit centre |
|---|---|---|---|---|
| 1.0 | 12 px | 8.8 px | 9 × 9 px, hand-hinted | 12.5 vs 12.6 (within 1 px) |
| 1.25 | 15 px | 11.0 px | 11 × 11 px, hand-hinted | 16.5 vs 16.5 |

The orange dashed line on the sheets marks the digit centre.

**Finding:** JetBrains Mono has no U+27E1. The current pill therefore draws `⟡` from the fontconfig fallback (Adwaita Mono), where it is about 5 px tall and visibly smaller and lighter than the digits. The "current" column of the specimen reproduces exactly that.

## Notes and open points

**Brief ambiguities.** Two readings for the client to confirm. One GitHub issue each is the way to ask (brief §10), but we have not opened them yet.

- **A4 "fits a 16–20 px cap height, 1:1 or 1:1.2".** We read this as a 16 px glyph box at 1.0 and a 20 px box at 1.25, and we used the 1:1 box.
- **Check 4, "optical size matches the digits' x-height".** Lining digits have no x-height, so we matched the digit height.

**Concept 2 at small sizes.** It is honestly the weakest of the three. The silhouette is wide and low, and in the bar it reads as a small hill rather than as a symbol. Both sheets say so.

**Photon Studio 0.1.31 findings.** All were worked around:

- `layer.code.read` writes stroke dash arrays in units of stroke width without scaling them, which would give wrong SVG dashes. We avoid dash patterns in the masters.
- `render` caps previews at 1536 px, so the sheets are exported with `document.save` instead.
- A transparent new document starts with an empty raster layer "Layer 1". The builders delete it so every layer name stays kebab-case.

**Metadata.** No personal data was found in the PSDs or PNGs (no author fields, no home paths). The PNGs carry an sRGB ICC profile and dates only. Round 2 strips the dates as well.

**Not in this round.** These belong to round 2, after the pick:

- brand colours and palette
- lockup, README hero, avatar
- favicon, Obsidian icon
- pictograms (A11) and legend markers (A12)
- 22 / 24 / 32 px hinting
- SVGO-optimised files
- `assets/` folder layout, `assets/LICENSE`, `DELIVERY.md`
