# Design brief — Seldon icon and graphic assets

Audience: the designer and the design agents producing Seldon's visual
identity. This document is the complete specification. Where it says
"must", the deliverable is rejected without it; where it says "should",
deviate only with a stated reason in the delivery notes.

Deliver by pull request against `JohnAndrewsX/jax-seldon` (folder
`assets/`), or as a zip with the same folder layout. Every file name,
layer name and note in English.

## 1. What Seldon is (read this first)

Seldon is a **flight recorder and planning desk for an Omarchy Linux
system**. It records every change to the machine (packages, configs,
themes, plugins, Omarchy updates) in a Markdown logbook, reconciles those
facts against *planned* work (cases), and shows the picture in the
Omarchy shell: a small **pill** in the top bar, a **panel** with tabs, and
a full-screen overlay called the **Prime Radiant** with charts.

The name comes from Hari Seldon in Asimov's *Foundation*: the Seldon Plan
predicts the future; the *Seldon Crises* are the moments where reality
deviates from the plan. In our product a **crisis** is a change in the
red zone with no explanation; **drift** is any change nobody planned; a
**case** is a planned piece of work. The Prime Radiant in the novels is
the device that displays the Plan's equations; ours shows the machine's
history as charts.

Tone of the product: calm, precise, technical, trustworthy. It records,
it does not nag. Think instrument, not mascot.

Current placeholder: the bar pill shows the Unicode glyph `⟡` (U+27E1,
white concave-sided diamond) before the counts, e.g. `⟡ 2 · 3`.

Reference material in the repository:
- `plugin/preview.png` — the panel and the Prime Radiant as they look today
- `docs/images/` — further renders of the panel
- `work/completed/WP-03*/screenshots/` — renders in three Omarchy themes
- `/usr/share/omarchy/logo.svg` and `icon.png` on any Omarchy machine —
  the host platform's own identity (we live next to it, we do not copy it)

## 2. Concept direction

Pick one primary concept and show two alternates in the first round.
The primary should come from this list, in order of preference:

1. **Prime Radiant** — a concave-sided diamond (the `⟡` shape) as the
   core, with a restrained sense of light or radiating structure. Links
   the icon to the glyph users already see in the bar.
2. **Plan versus reality** — a straight (dashed) reference line and a
   solid line that departs from it and returns; the departure is the
   drift, the return is the explanation. Communicates the idea without
   words.
3. **Flight recorder** — a stylised recorder trace or black-box form;
   sober, less tied to the Foundation reference.

Constraints on the concept:
- Geometric, constructed on a grid; no gradients that only read in colour,
  no 3D, no shadows, no glossy effects, no mascot, no literal book or
  clipboard, no robot.
- Must read as one shape at 16 px and as something richer at 512 px.
- Must not resemble the Omarchy logo, the Obsidian logo, or any
  distribution logo (Arch's arrow, etc.).
- The wordmark "Seldon" is set in the system's monospace typeface (the
  Omarchy shell uses the active theme's font); do not design a custom
  typeface. A lockup icon + wordmark is required only for the README
  hero and the social preview.

## 3. Colour and theming (the hard constraint)

Omarchy ships many themes (dark and light: Tokyo Night, Catppuccin
Latte, Osaka Jade, Gruvbox, Flexoki Light, …) and users switch them at
runtime. **The in-shell icon therefore cannot carry its own colours.**

- **Monochrome master.** The icon must be a single-colour vector that
  works as a *mask*: the shell tints it with the theme's foreground or
  accent colour. Design for that: strokes and fills in one colour, with
  transparency for everything else. No colour, no opacity steps below
  100 % in the mask version (the shell applies opacity itself).
- **Two-tone variant (optional).** If the concept needs a second tone,
  provide a variant with exactly two colour roles, `primary` and `muted`,
  as named layers; the shell maps them to foreground and a dimmed
  foreground. Never a third colour.
- **Brand version.** For GitHub, the marketplace and the README, a
  coloured version is allowed. Define a palette of at most three colours
  with hex values and a dark and a light background version. Suggested
  character: deep night blue background family with one warm accent
  (the "crisis" red used in the UI is the theme's `urgent` colour, do not
  hard-code it into the brand).
- Contrast: the mask icon on both `#1a1b26` (dark) and `#eff1f5` (light)
  must pass WCAG AA for graphical objects (3:1) when tinted with the
  theme foreground, which we guarantee; what you must guarantee is that
  the shape itself does not rely on thin lines that vanish at 16 px.

## 4. Deliverables

All vector sources as SVG 1.1, viewBox `0 0 N N`, no embedded raster,
no external fonts (text converted to paths only where text is part of
the artwork), no scripts, no `<style>` blocks (presentation attributes
only), ids and layer names in English `kebab-case`. Optimise with SVGO
or equivalent; keep a non-optimised source file too.

| # | Asset | Format | Sizes | Notes |
|---|---|---|---|---|
| A1 | App icon, mask (monochrome) | SVG + PNG | SVG master 1024 grid; PNG 16, 22, 24, 32, 48, 64, 128, 256, 512, 1024 | PNGs white-on-transparent and black-on-transparent; pixel-hinted versions for 16, 22, 24, 32 |
| A2 | App icon, brand (colour) | SVG + PNG | same sizes | dark-background and light-background variants; 1024 PNG with and without rounded container |
| A3 | Two-tone mask (optional) | SVG | 1024 grid | layers `primary`, `muted` |
| A4 | Bar glyph | SVG | a 16 px glyph box at scale 1.0 and a 20 px box at 1.25, square (1:1) | the `⟡` successor shown before counts in the top bar; must align optically with monospace digits; provide a specimen at 1.0 and 1.25 scale next to `2 · 3` |
| A5 | Panel header mark | SVG | 24–32 px cap height | small lockup icon + "Seldon" wordmark baseline-aligned (wordmark rendered in a monospace font as a placeholder; deliver the icon and alignment metrics, not the font) |
| A6 | README hero / social preview | PNG + SVG | 1280×640 (GitHub social preview), 1600×800 safe-area version | lockup on brand background, no screenshots inside (we overlay those) |
| A7 | GitHub avatar | PNG | 500×500, 1024×1024 | brand icon on brand background, works when cropped to a circle |
| A8 | Marketplace listing image | PNG | 1200×630 and 800×600 | lockup + one-line tagline area left empty (we add text) |
| A9 | Favicon / docs site | ICO + PNG + SVG | 16, 32, 48 ICO; 180 apple-touch; 192, 512 maskable | from A2 |
| A10 | Obsidian vault icon | PNG | 256×256 | from A1 mask on transparent |
| A11 | Empty-state and status pictograms (set) | SVG | 48 and 96 grid | eight pictograms, mask style: engine missing, logbook not initialised, index missing, index stale, drift open, crisis, case active, all clear; same stroke width and corner radius as A1 |
| A12 | Chart legend markers (set) | SVG | 12 and 16 grid | five markers for the Prime Radiant timeline: release, snapshot, case span start/end, crisis; distinguishable in monochrome by shape alone |

Naming: `assets/<asset-id>-<name>[-<variant>][-<size>].<ext>`, e.g.
`assets/a1-icon-mask-white-32.png`, `assets/a4-bar-glyph.svg`,
`assets/a11-state-crisis-48.svg`. One `assets/README.md` listing every
file, its purpose, and how it was produced.

## 5. Construction rules

- Grid: 1024 units for the master, with a 64-unit safe margin on every
  side; key shapes snap to a 32-unit grid; optical corrections allowed
  and listed in the notes.
- Stroke: if the concept uses strokes, one stroke width for the whole
  set, between 56 and 80 units at 1024 (i.e. 1.75–2.5 px at 32 px);
  round or square caps consistently; no hairlines.
- Corner radius: one radius family (e.g. 48/96/192 units).
- Small sizes: the 16, 22, 24 and 32 px PNGs must be hand-adjusted
  (hinted) versions, not downsampled; the SVG may carry `<desc>` notes on
  what changed.
- Negative space: the icon must survive being placed on a 2 px gap next
  to monospace text in the bar (see A4 specimen).
- Nothing in the artwork depends on text rendering.

## 6. Acceptance checks (we run these; design against them)

1. `rsvg-convert` renders every SVG without warnings; the output matches
   the delivered PNG at the same size within anti-aliasing.
2. The mask SVG contains a single fill colour (or none with
   `currentColor`) and no opacity attribute below 1.
3. The A1 16 px and 22 px PNGs are legible as the same shape in a
   side-by-side with the 512 px version (we ask three people).
4. A4 placed in the real bar next to `⟡ 2 · 3` at 1.0 and 1.25 scale:
   optical size matches the digit height (lining digits, 0.73 em of the
   bar font) and the vertical centre sits on the digits' centre within
   1 px.
5. No file carries metadata with personal data (author fields, GPS,
   software paths); run `exiftool` or equivalent before delivery.
6. SVG files pass an XML lint and have no external references.
7. Each asset in the table exists in every listed size and variant.

## 6a. Clarifications after round 1 (2026-10-02)

- A4 box: 16 px at 1.0, 20 px at 1.25, square; the designer's reading
  is confirmed.
- Check 4: "x-height" meant the digit height; the table in the round-1
  README (9 × 9 px glyph at 1.0, 11 × 11 at 1.25) is the reference.
- The current `⟡` comes from a fallback font (JetBrains Mono has no
  U+27E1) and is smaller than the digits; the new glyph fixes that.
- Round-1 deliverables live in `work/design/round-1/` (sheets, README,
  sources); sketches stay with the designer.

## 7. Process

- Round 1: three concept sheets (one page each: the idea, the icon at
  1024 and 16, the mask on dark and light, the bar glyph specimen). We
  pick one within a day.
- Round 2: full deliverable set for the chosen concept, plus the
  pictogram set (A11) and markers (A12) in the same language.
- Round 3: corrections from the acceptance checks.
- Delivery notes (`assets/DELIVERY.md`): decisions taken, optical
  corrections, colour values, tools used, and anything not done.

## 8. Rights and licensing

The work is delivered under the project's MIT licence with the
copyright assigned to the project owner (JohnAndrewsX), or under CC0 for
the artwork, stated in `assets/LICENSE`. No stock elements, no fonts
with restrictive licences embedded, no AI-generated raster art passed
off as vector (vector sources are the deliverable). Reference images
from the repository may be used for context only.

## 9. Do not

- Do not use the Omarchy, Arch, Hyprland or Obsidian marks or shapes.
- Do not add colour to the mask icon.
- Do not ship rasters as the only source.
- Do not design a typeface or a mascot.
- Do not put text inside the icon.
- Do not deliver files whose names contain spaces or non-ASCII characters.

## 10. Questions

Ask in a GitHub issue on `JohnAndrewsX/jax-seldon` with the label
`design`, one question per issue, with the asset id in the title.
