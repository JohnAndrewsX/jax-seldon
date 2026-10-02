# Seldon assets (Prime Radiant)

The project's icon and graphic assets, concept 1 "Prime Radiant", delivered by the designer in design rounds 2 and 3 (`docs/DESIGN-BRIEF.md`, `work/design/ROUND-2.md`, `work/design/ROUND-3.md`). Decisions, metrics, colour values and open points are in `DELIVERY.md`; the licence is in `LICENSE`. Both are the designer's files, copied unchanged.

## Provenance

- **Masks** (the 37 SVGs of A1, A3, A4, A5, A11 and A12, and their 37 sources) are the round-3 files from `work/design/round-3/`, which moved the fallback colour to the `<svg>` root (finding F1).
- **Everything else** (rasters, brand files, overviews, specimens, the other 28 sources, `LICENSE`) is the round-2 delivery from `work/design/round-2/`; round 3 left those byte-identical.
- `work/design/round-2/` and `work/design/round-3/` stay as the record of what was delivered; this folder is what the project uses. A change to an asset goes through the designer and lands here, never as a hand edit.

## Where the project uses them

The plugin is published from `plugin/` alone (ADR-0009), so it carries **copies** of the files it shows in `plugin/assets/` (no symlinks; `tests/plugin/model.test.js` fails when a copy differs from this folder):

| Surface | Files | How |
|---|---|---|
| Bar pill (SPEC-PLUGIN §4) | `a4-bar-glyph-16.svg`, `a4-bar-glyph-20.svg`, `a4-bar-glyph.svg` | hinted file when the glyph box is 16 or 20 device pixels, the vector otherwise; centred on the digits (brief check 4) |
| Panel header (§5) | `a5-panel-mark-24.svg`, `a5-panel-mark-32.svg`, `a1-icon-mask.svg` | the A5 grid at 24 or 32 device pixels, the A1 master otherwise; metrics from `DELIVERY.md` §5 |
| Status banner (§5), Prime Radiant banner (§6) | `a11-state-{engine-missing,logbook-not-initialised,index-missing,index-stale}-{48,96}.svg` | 48 px in the panel, 96 px in the overlay |
| Today tab (§5) | `a11-state-{crisis,drift-open,case-active,all-clear}-{48,96}.svg` | the day's state, 48 px |
| Timeline legend (§6) | `a12-marker-*-{12,16}.svg` | the canvas draws the same shapes from the 16 grid's paths |
| Both READMEs | `a6-readme-hero-dark-1280x640.png` | `README.md` links this folder, `plugin/README.md` its copy |

The masks are tinted with the theme: every shape is `currentColor`, so the plugin sets the `<svg>` root's `color` to a theme colour (`plugin/components/MaskIcon.qml`). No colour is written into QML.

Not placed yet: the favicon set (A9) waits for a docs site; the GitHub avatar (A7), the social preview (A6) and the marketplace image (A8) are uploaded by the operator; the Obsidian vault icon (A10) belongs in the operator's vault.

## Files

- **SVG** files at the top level are optimised with SVGO 3.3.5. ids, viewBox, `<title>` and `<desc>` are kept, and `version="1.1"` is restored.
- **`sources/`** holds the non-optimised source of every SVG, with the same file names.
- **PNG** files are rendered from the optimised SVG with `rsvg-convert` at the exact size, then their metadata is stripped. Rendering the SVG again reproduces the PNG (acceptance check 1). The four presentation images (`a2-brand-palette`, `a3-...-comparison`, `a4-...-specimen`, `a5-...-specimen`) and the A11 / A12 overviews are exported from their Photon documents.
- **Mask** files (A1, A3, A4, A5, A10, A11, A12) use `currentColor` only, with no opacity below 1. The fallback `color="#000"` sits on the `<svg>` root only (round 3), so CSS on the root (`svg { color: ... }`) tints the whole icon.
- **Brand** files (A2, A6 to A9) use the three brand colours.

How it was produced: the geometry is defined once (Python). The same data builds the layered Photon Studio documents and the SVG. Photon's render was compared with `rsvg-convert`'s render of every master and composition, and showed 0 differing pixels.

## A1

| File | Purpose | Produced by |
|---|---|---|
| `a1-icon-mask-16.svg` | A1 mask, hand-hinted pixel version for 16 px | pixel grid -> SVG, SVGO |
| `a1-icon-mask-22.svg` | A1 mask, hand-hinted pixel version for 22 px | pixel grid -> SVG, SVGO |
| `a1-icon-mask-24.svg` | A1 mask, hand-hinted pixel version for 24 px | pixel grid -> SVG, SVGO |
| `a1-icon-mask-32.svg` | A1 mask, hand-hinted pixel version for 32 px | pixel grid -> SVG, SVGO |
| `a1-icon-mask-black-1024.png` | A1 mask PNG, black on transparent, 1024 px | rsvg-convert from the hinted SVG (16-32) or the master |
| `a1-icon-mask-black-128.png` | A1 mask PNG, black on transparent, 128 px | rsvg-convert from the hinted SVG (16-32) or the master |
| `a1-icon-mask-black-16.png` | A1 mask PNG, black on transparent, 16 px | rsvg-convert from the hinted SVG (16-32) or the master |
| `a1-icon-mask-black-22.png` | A1 mask PNG, black on transparent, 22 px | rsvg-convert from the hinted SVG (16-32) or the master |
| `a1-icon-mask-black-24.png` | A1 mask PNG, black on transparent, 24 px | rsvg-convert from the hinted SVG (16-32) or the master |
| `a1-icon-mask-black-256.png` | A1 mask PNG, black on transparent, 256 px | rsvg-convert from the hinted SVG (16-32) or the master |
| `a1-icon-mask-black-32.png` | A1 mask PNG, black on transparent, 32 px | rsvg-convert from the hinted SVG (16-32) or the master |
| `a1-icon-mask-black-48.png` | A1 mask PNG, black on transparent, 48 px | rsvg-convert from the hinted SVG (16-32) or the master |
| `a1-icon-mask-black-512.png` | A1 mask PNG, black on transparent, 512 px | rsvg-convert from the hinted SVG (16-32) or the master |
| `a1-icon-mask-black-64.png` | A1 mask PNG, black on transparent, 64 px | rsvg-convert from the hinted SVG (16-32) or the master |
| `a1-icon-mask-white-1024.png` | A1 mask PNG, white on transparent, 1024 px | rsvg-convert from the hinted SVG (16-32) or the master |
| `a1-icon-mask-white-128.png` | A1 mask PNG, white on transparent, 128 px | rsvg-convert from the hinted SVG (16-32) or the master |
| `a1-icon-mask-white-16.png` | A1 mask PNG, white on transparent, 16 px | rsvg-convert from the hinted SVG (16-32) or the master |
| `a1-icon-mask-white-22.png` | A1 mask PNG, white on transparent, 22 px | rsvg-convert from the hinted SVG (16-32) or the master |
| `a1-icon-mask-white-24.png` | A1 mask PNG, white on transparent, 24 px | rsvg-convert from the hinted SVG (16-32) or the master |
| `a1-icon-mask-white-256.png` | A1 mask PNG, white on transparent, 256 px | rsvg-convert from the hinted SVG (16-32) or the master |
| `a1-icon-mask-white-32.png` | A1 mask PNG, white on transparent, 32 px | rsvg-convert from the hinted SVG (16-32) or the master |
| `a1-icon-mask-white-48.png` | A1 mask PNG, white on transparent, 48 px | rsvg-convert from the hinted SVG (16-32) or the master |
| `a1-icon-mask-white-512.png` | A1 mask PNG, white on transparent, 512 px | rsvg-convert from the hinted SVG (16-32) or the master |
| `a1-icon-mask-white-64.png` | A1 mask PNG, white on transparent, 64 px | rsvg-convert from the hinted SVG (16-32) or the master |
| `a1-icon-mask.svg` | A1 mask master, 1024 grid, currentColor | geometry -> SVG, SVGO |

## A2

| File | Purpose | Produced by |
|---|---|---|
| `a2-brand-palette.png` | brand palette: hex values, contrast, mark on both backgrounds | Photon document, exported |
| `a2-icon-brand-on-dark-1024.png` | A2 brand icon for dark backgrounds, 1024 px | rsvg-convert from the hinted SVG (16-32) or the 1024 SVG |
| `a2-icon-brand-on-dark-128.png` | A2 brand icon for dark backgrounds, 128 px | rsvg-convert from the hinted SVG (16-32) or the 1024 SVG |
| `a2-icon-brand-on-dark-16.png` | A2 brand icon for dark backgrounds, 16 px | rsvg-convert from the hinted SVG (16-32) or the 1024 SVG |
| `a2-icon-brand-on-dark-16.svg` | A2 brand icon for dark backgrounds, hand-hinted 16 px | pixel grid -> SVG, SVGO |
| `a2-icon-brand-on-dark-22.png` | A2 brand icon for dark backgrounds, 22 px | rsvg-convert from the hinted SVG (16-32) or the 1024 SVG |
| `a2-icon-brand-on-dark-22.svg` | A2 brand icon for dark backgrounds, hand-hinted 22 px | pixel grid -> SVG, SVGO |
| `a2-icon-brand-on-dark-24.png` | A2 brand icon for dark backgrounds, 24 px | rsvg-convert from the hinted SVG (16-32) or the 1024 SVG |
| `a2-icon-brand-on-dark-24.svg` | A2 brand icon for dark backgrounds, hand-hinted 24 px | pixel grid -> SVG, SVGO |
| `a2-icon-brand-on-dark-256.png` | A2 brand icon for dark backgrounds, 256 px | rsvg-convert from the hinted SVG (16-32) or the 1024 SVG |
| `a2-icon-brand-on-dark-32.png` | A2 brand icon for dark backgrounds, 32 px | rsvg-convert from the hinted SVG (16-32) or the 1024 SVG |
| `a2-icon-brand-on-dark-32.svg` | A2 brand icon for dark backgrounds, hand-hinted 32 px | pixel grid -> SVG, SVGO |
| `a2-icon-brand-on-dark-48.png` | A2 brand icon for dark backgrounds, 48 px | rsvg-convert from the hinted SVG (16-32) or the 1024 SVG |
| `a2-icon-brand-on-dark-512.png` | A2 brand icon for dark backgrounds, 512 px | rsvg-convert from the hinted SVG (16-32) or the 1024 SVG |
| `a2-icon-brand-on-dark-64.png` | A2 brand icon for dark backgrounds, 64 px | rsvg-convert from the hinted SVG (16-32) or the 1024 SVG |
| `a2-icon-brand-on-dark-container-1024.png` | A2 brand icon with container, dark, 1024 px | rsvg-convert from the SVG |
| `a2-icon-brand-on-dark-container.svg` | A2 brand icon in its rounded container (radius 192), dark | geometry -> SVG, SVGO |
| `a2-icon-brand-on-dark.svg` | A2 brand icon for dark backgrounds, 1024 | geometry -> SVG, SVGO |
| `a2-icon-brand-on-light-1024.png` | A2 brand icon for light backgrounds, 1024 px | rsvg-convert from the hinted SVG (16-32) or the 1024 SVG |
| `a2-icon-brand-on-light-128.png` | A2 brand icon for light backgrounds, 128 px | rsvg-convert from the hinted SVG (16-32) or the 1024 SVG |
| `a2-icon-brand-on-light-16.png` | A2 brand icon for light backgrounds, 16 px | rsvg-convert from the hinted SVG (16-32) or the 1024 SVG |
| `a2-icon-brand-on-light-16.svg` | A2 brand icon for light backgrounds, hand-hinted 16 px | pixel grid -> SVG, SVGO |
| `a2-icon-brand-on-light-22.png` | A2 brand icon for light backgrounds, 22 px | rsvg-convert from the hinted SVG (16-32) or the 1024 SVG |
| `a2-icon-brand-on-light-22.svg` | A2 brand icon for light backgrounds, hand-hinted 22 px | pixel grid -> SVG, SVGO |
| `a2-icon-brand-on-light-24.png` | A2 brand icon for light backgrounds, 24 px | rsvg-convert from the hinted SVG (16-32) or the 1024 SVG |
| `a2-icon-brand-on-light-24.svg` | A2 brand icon for light backgrounds, hand-hinted 24 px | pixel grid -> SVG, SVGO |
| `a2-icon-brand-on-light-256.png` | A2 brand icon for light backgrounds, 256 px | rsvg-convert from the hinted SVG (16-32) or the 1024 SVG |
| `a2-icon-brand-on-light-32.png` | A2 brand icon for light backgrounds, 32 px | rsvg-convert from the hinted SVG (16-32) or the 1024 SVG |
| `a2-icon-brand-on-light-32.svg` | A2 brand icon for light backgrounds, hand-hinted 32 px | pixel grid -> SVG, SVGO |
| `a2-icon-brand-on-light-48.png` | A2 brand icon for light backgrounds, 48 px | rsvg-convert from the hinted SVG (16-32) or the 1024 SVG |
| `a2-icon-brand-on-light-512.png` | A2 brand icon for light backgrounds, 512 px | rsvg-convert from the hinted SVG (16-32) or the 1024 SVG |
| `a2-icon-brand-on-light-64.png` | A2 brand icon for light backgrounds, 64 px | rsvg-convert from the hinted SVG (16-32) or the 1024 SVG |
| `a2-icon-brand-on-light-container-1024.png` | A2 brand icon with container, light, 1024 px | rsvg-convert from the SVG |
| `a2-icon-brand-on-light-container.svg` | A2 brand icon in its rounded container (radius 192), light | geometry -> SVG, SVGO |
| `a2-icon-brand-on-light.svg` | A2 brand icon for light backgrounds, 1024 | geometry -> SVG, SVGO |

## A3

| File | Purpose | Produced by |
|---|---|---|
| `a3-icon-two-tone-comparison.png` | A3 one tone versus two-tone on three themes (recommendation in DELIVERY.md) | Photon document, exported |
| `a3-icon-two-tone.svg` | A3 two-tone mask: groups primary (envelope) and muted (core) | geometry -> SVG, SVGO |

## A4

| File | Purpose | Produced by |
|---|---|---|
| `a4-bar-glyph-16.svg` | A4 bar glyph, hand-hinted, 16 px box | pixel grid -> SVG, SVGO |
| `a4-bar-glyph-20.svg` | A4 bar glyph, hand-hinted, 20 px box | pixel grid -> SVG, SVGO |
| `a4-bar-glyph-specimen.png` | A4 specimen next to 2 · 3: Tokyo Night, Catppuccin Latte, Osaka Jade; scales 1.0 / 1.25 / 1.5 / 2.0 | Photon documents, exported |
| `a4-bar-glyph.svg` | A4 bar glyph, vector for scales other than 1.0 / 1.25 (16-unit box) | geometry -> SVG, SVGO |

## A5

| File | Purpose | Produced by |
|---|---|---|
| `a5-panel-lockup-specimen.png` | A5 mark + wordmark placeholder, baseline-aligned, three themes | Photon document, exported |
| `a5-panel-mark-24.svg` | A5 panel header mark, 24 px box (metrics in DELIVERY.md) | pixel grid -> SVG, SVGO |
| `a5-panel-mark-32.svg` | A5 panel header mark, 32 px box (metrics in DELIVERY.md) | pixel grid -> SVG, SVGO |

## A6

| File | Purpose | Produced by |
|---|---|---|
| `a6-readme-hero-dark-1280x640.png` | A6 README hero / social preview, dark, 1280x640 | rsvg-convert from the SVG |
| `a6-readme-hero-dark-1280x640.svg` | A6 README hero / social preview, dark, 1280x640 | geometry + wordmark outlines -> SVG, SVGO |
| `a6-readme-hero-dark-1600x800.png` | A6 README hero / social preview, dark, 1600x800 | rsvg-convert from the SVG |
| `a6-readme-hero-dark-1600x800.svg` | A6 README hero / social preview, dark, 1600x800 | geometry + wordmark outlines -> SVG, SVGO |
| `a6-readme-hero-light-1280x640.png` | A6 README hero / social preview, light, 1280x640 | rsvg-convert from the SVG |
| `a6-readme-hero-light-1280x640.svg` | A6 README hero / social preview, light, 1280x640 | geometry + wordmark outlines -> SVG, SVGO |
| `a6-readme-hero-light-1600x800.png` | A6 README hero / social preview, light, 1600x800 | rsvg-convert from the SVG |
| `a6-readme-hero-light-1600x800.svg` | A6 README hero / social preview, light, 1600x800 | geometry + wordmark outlines -> SVG, SVGO |

## A7

| File | Purpose | Produced by |
|---|---|---|
| `a7-avatar-dark-1024.png` | A7 GitHub avatar, dark, 1024 px (circle-safe) | rsvg-convert from the SVG |
| `a7-avatar-dark-500.png` | A7 GitHub avatar, dark, 500 px (circle-safe) | rsvg-convert from the SVG |
| `a7-avatar-dark.svg` | A7 GitHub avatar source, dark | geometry -> SVG, SVGO |
| `a7-avatar-light-1024.png` | A7 GitHub avatar, light, 1024 px (circle-safe) | rsvg-convert from the SVG |
| `a7-avatar-light-500.png` | A7 GitHub avatar, light, 500 px (circle-safe) | rsvg-convert from the SVG |
| `a7-avatar-light.svg` | A7 GitHub avatar source, light | geometry -> SVG, SVGO |

## A8

| File | Purpose | Produced by |
|---|---|---|
| `a8-marketplace-dark-1200x630.png` | A8 marketplace listing image, dark, 1200x630 (tagline area empty) | rsvg-convert from the SVG |
| `a8-marketplace-dark-1200x630.svg` | A8 marketplace listing image source, dark, 1200x630 | geometry + wordmark outlines -> SVG, SVGO |
| `a8-marketplace-dark-800x600.png` | A8 marketplace listing image, dark, 800x600 (tagline area empty) | rsvg-convert from the SVG |
| `a8-marketplace-dark-800x600.svg` | A8 marketplace listing image source, dark, 800x600 | geometry + wordmark outlines -> SVG, SVGO |
| `a8-marketplace-light-1200x630.png` | A8 marketplace listing image, light, 1200x630 (tagline area empty) | rsvg-convert from the SVG |
| `a8-marketplace-light-1200x630.svg` | A8 marketplace listing image source, light, 1200x630 | geometry + wordmark outlines -> SVG, SVGO |
| `a8-marketplace-light-800x600.png` | A8 marketplace listing image, light, 800x600 (tagline area empty) | rsvg-convert from the SVG |
| `a8-marketplace-light-800x600.svg` | A8 marketplace listing image source, light, 800x600 | geometry + wordmark outlines -> SVG, SVGO |

## A9

| File | Purpose | Produced by |
|---|---|---|
| `a9-apple-touch-180.png` | A9 apple-touch icon, 180 px | rsvg-convert from the SVG |
| `a9-apple-touch.svg` | A9 apple-touch icon source | geometry -> SVG, SVGO |
| `a9-favicon-16.png` | A9 favicon, 16 px | rsvg-convert from the SVG |
| `a9-favicon-16.svg` | A9 favicon source, 16 px, hand-hinted mark in the container | pixel grid -> SVG, SVGO |
| `a9-favicon-32.png` | A9 favicon, 32 px | rsvg-convert from the SVG |
| `a9-favicon-32.svg` | A9 favicon source, 32 px, hand-hinted mark in the container | pixel grid -> SVG, SVGO |
| `a9-favicon-48.png` | A9 favicon, 48 px | rsvg-convert from the SVG |
| `a9-favicon-48.svg` | A9 favicon source, 48 px, hand-hinted mark in the container | pixel grid -> SVG, SVGO |
| `a9-favicon.ico` | A9 favicon, ICO with 16 / 32 / 48 | ImageMagick from the three PNGs |
| `a9-favicon.svg` | A9 favicon for the docs site (SVG) | geometry -> SVG, SVGO |
| `a9-maskable-192.png` | A9 maskable icon, 192 px | rsvg-convert from the SVG |
| `a9-maskable-512.png` | A9 maskable icon, 512 px | rsvg-convert from the SVG |
| `a9-maskable.svg` | A9 maskable icon source (mark inside the 80 % safe zone) | geometry -> SVG, SVGO |

## A10

| File | Purpose | Produced by |
|---|---|---|
| `a10-obsidian-vault-black-256.png` | A10 Obsidian vault icon, black mask on transparent, 256 px | rsvg-convert from a1-icon-mask.svg |
| `a10-obsidian-vault-white-256.png` | A10 Obsidian vault icon, white mask on transparent, 256 px | rsvg-convert from a1-icon-mask.svg |

## A11

| File | Purpose | Produced by |
|---|---|---|
| `a11-state-all-clear-48.svg` | A11 state pictogram “all-clear”, 48 grid, mask | geometry -> SVG, SVGO |
| `a11-state-all-clear-96.svg` | A11 state pictogram “all-clear”, 96 grid, mask | geometry -> SVG, SVGO |
| `a11-state-case-active-48.svg` | A11 state pictogram “case-active”, 48 grid, mask | geometry -> SVG, SVGO |
| `a11-state-case-active-96.svg` | A11 state pictogram “case-active”, 96 grid, mask | geometry -> SVG, SVGO |
| `a11-state-crisis-48.svg` | A11 state pictogram “crisis”, 48 grid, mask | geometry -> SVG, SVGO |
| `a11-state-crisis-96.svg` | A11 state pictogram “crisis”, 96 grid, mask | geometry -> SVG, SVGO |
| `a11-state-drift-open-48.svg` | A11 state pictogram “drift-open”, 48 grid, mask | geometry -> SVG, SVGO |
| `a11-state-drift-open-96.svg` | A11 state pictogram “drift-open”, 96 grid, mask | geometry -> SVG, SVGO |
| `a11-state-engine-missing-48.svg` | A11 state pictogram “engine-missing”, 48 grid, mask | geometry -> SVG, SVGO |
| `a11-state-engine-missing-96.svg` | A11 state pictogram “engine-missing”, 96 grid, mask | geometry -> SVG, SVGO |
| `a11-state-index-missing-48.svg` | A11 state pictogram “index-missing”, 48 grid, mask | geometry -> SVG, SVGO |
| `a11-state-index-missing-96.svg` | A11 state pictogram “index-missing”, 96 grid, mask | geometry -> SVG, SVGO |
| `a11-state-index-stale-48.svg` | A11 state pictogram “index-stale”, 48 grid, mask | geometry -> SVG, SVGO |
| `a11-state-index-stale-96.svg` | A11 state pictogram “index-stale”, 96 grid, mask | geometry -> SVG, SVGO |
| `a11-state-logbook-not-initialised-48.svg` | A11 state pictogram “logbook-not-initialised”, 48 grid, mask | geometry -> SVG, SVGO |
| `a11-state-logbook-not-initialised-96.svg` | A11 state pictogram “logbook-not-initialised”, 96 grid, mask | geometry -> SVG, SVGO |
| `a11-state-overview-48.png` | A11 overview, 48 grid, both themes, consistency figures | Photon document (set_sheet), exported |
| `a11-state-overview-96.png` | A11 overview, 96 grid, both themes, consistency figures | Photon document (set_sheet), exported |

## A12

| File | Purpose | Produced by |
|---|---|---|
| `a12-marker-case-span-end-12.svg` | A12 legend marker “case-span-end”, 12 grid, mask | geometry -> SVG, SVGO |
| `a12-marker-case-span-end-16.svg` | A12 legend marker “case-span-end”, 16 grid, mask | geometry -> SVG, SVGO |
| `a12-marker-case-span-start-12.svg` | A12 legend marker “case-span-start”, 12 grid, mask | geometry -> SVG, SVGO |
| `a12-marker-case-span-start-16.svg` | A12 legend marker “case-span-start”, 16 grid, mask | geometry -> SVG, SVGO |
| `a12-marker-crisis-12.svg` | A12 legend marker “crisis”, 12 grid, mask | geometry -> SVG, SVGO |
| `a12-marker-crisis-16.svg` | A12 legend marker “crisis”, 16 grid, mask | geometry -> SVG, SVGO |
| `a12-marker-overview-12.png` | A12 overview, 12 grid, both themes | Photon document (set_sheet), exported |
| `a12-marker-overview-16.png` | A12 overview, 16 grid, both themes | Photon document (set_sheet), exported |
| `a12-marker-release-12.svg` | A12 legend marker “release”, 12 grid, mask | geometry -> SVG, SVGO |
| `a12-marker-release-16.svg` | A12 legend marker “release”, 16 grid, mask | geometry -> SVG, SVGO |
| `a12-marker-snapshot-12.svg` | A12 legend marker “snapshot”, 12 grid, mask | geometry -> SVG, SVGO |
| `a12-marker-snapshot-16.svg` | A12 legend marker “snapshot”, 16 grid, mask | geometry -> SVG, SVGO |

## sources/

65 non-optimised SVG sources, one for every optimised SVG above:

`a1-icon-mask-16.svg`, `a1-icon-mask-22.svg`, `a1-icon-mask-24.svg`, `a1-icon-mask-32.svg`, `a1-icon-mask.svg`, `a11-state-all-clear-48.svg`, `a11-state-all-clear-96.svg`, `a11-state-case-active-48.svg`, `a11-state-case-active-96.svg`, `a11-state-crisis-48.svg`, `a11-state-crisis-96.svg`, `a11-state-drift-open-48.svg`, `a11-state-drift-open-96.svg`, `a11-state-engine-missing-48.svg`, `a11-state-engine-missing-96.svg`, `a11-state-index-missing-48.svg`, `a11-state-index-missing-96.svg`, `a11-state-index-stale-48.svg`, `a11-state-index-stale-96.svg`, `a11-state-logbook-not-initialised-48.svg`, `a11-state-logbook-not-initialised-96.svg`, `a12-marker-case-span-end-12.svg`, `a12-marker-case-span-end-16.svg`, `a12-marker-case-span-start-12.svg`, `a12-marker-case-span-start-16.svg`, `a12-marker-crisis-12.svg`, `a12-marker-crisis-16.svg`, `a12-marker-release-12.svg`, `a12-marker-release-16.svg`, `a12-marker-snapshot-12.svg`, `a12-marker-snapshot-16.svg`, `a2-icon-brand-on-dark-16.svg`, `a2-icon-brand-on-dark-22.svg`, `a2-icon-brand-on-dark-24.svg`, `a2-icon-brand-on-dark-32.svg`, `a2-icon-brand-on-dark-container.svg`, `a2-icon-brand-on-dark.svg`, `a2-icon-brand-on-light-16.svg`, `a2-icon-brand-on-light-22.svg`, `a2-icon-brand-on-light-24.svg`, `a2-icon-brand-on-light-32.svg`, `a2-icon-brand-on-light-container.svg`, `a2-icon-brand-on-light.svg`, `a3-icon-two-tone.svg`, `a4-bar-glyph-16.svg`, `a4-bar-glyph-20.svg`, `a4-bar-glyph.svg`, `a5-panel-mark-24.svg`, `a5-panel-mark-32.svg`, `a6-readme-hero-dark-1280x640.svg`, `a6-readme-hero-dark-1600x800.svg`, `a6-readme-hero-light-1280x640.svg`, `a6-readme-hero-light-1600x800.svg`, `a7-avatar-dark.svg`, `a7-avatar-light.svg`, `a8-marketplace-dark-1200x630.svg`, `a8-marketplace-dark-800x600.svg`, `a8-marketplace-light-1200x630.svg`, `a8-marketplace-light-800x600.svg`, `a9-apple-touch.svg`, `a9-favicon-16.svg`, `a9-favicon-32.svg`, `a9-favicon-48.svg`, `a9-favicon.svg`, `a9-maskable.svg`
