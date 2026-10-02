# Round 2 — go for Concept 1, Prime Radiant

Decision (operator, 2026-10-02): **Concept 1, Prime Radiant, is the
Seldon mark.** Concepts 2 and 3 are closed; keep their sources in
`round-1/sources/` for the record.

This file is the round-2 order. The brief (`docs/DESIGN-BRIEF.md`,
incl. §6a clarifications) stays the specification; this file adds the
concept-specific decisions and the exact deliverable list for this
round. Deliver into `round-2/` with the layout of brief §4 (`assets/`
is the final destination in the repository; we move the files there).

## 1. Keep from round 1

- The construction exactly as on sheet 1: 1024 grid, 64 safe margin,
  stroke 64, round caps and joins, radius family 32/64/128, sides as
  arcs through the tips and the 32-grid point inset 64 (26° tips),
  envelope offset 64 inward, core diamond 192.
- The hollow envelope. Never a filled four-point star (the "AI sparkle"
  problem named on the sheet).
- The hand-hinted 16 px (15 × 15 with 1 px envelope, 3 px tips, 3 px
  plus core).
- The bar specimen method and measurements (16/20 px box, 2 px gap,
  9 × 9 and 11 × 11 glyphs, digit height as the reference).

## 2. Deliverables this round (brief §4, concept 1)

| # | Asset | Notes for concept 1 |
|---|---|---|
| A1 | Mask icon, SVG master + PNG 16/22/24/32/48/64/128/256/512/1024 | 22, 24, 32 hand-hinted like the 16; white-on-transparent and black-on-transparent PNGs |
| A2 | Brand icon, colour | see §3; dark and light background variants; 1024 with and without the rounded container (container radius 192 at 1024) |
| A3 | Two-tone mask | optional: `primary` = envelope + tips, `muted` = core; only if the core reads better dimmed — show both and recommend |
| A4 | Bar glyph SVG | the 9 × 9 (1.0) and 11 × 11 (1.25) hinted glyphs as SVG on their boxes, plus the vector for other scales; specimen next to `2 · 3` in Tokyo Night, Catppuccin Latte, Osaka Jade |
| A5 | Panel header mark | icon + "Seldon" wordmark, baseline metrics; the wordmark in JetBrainsMono Nerd Font as placeholder |
| A6 | README hero / social preview | 1280×640 and 1600×800; lockup centred-left on the brand background, right 45 % empty for a screenshot overlay |
| A7 | GitHub avatar | 500 and 1024; brand icon on brand background, circle-safe |
| A8 | Marketplace listing image | 1200×630 and 800×600 |
| A9 | Favicons | ICO 16/32/48, apple-touch 180, maskable 192/512 |
| A10 | Obsidian vault icon | 256, mask on transparent |
| A11 | State pictograms, 48 and 96 | eight: engine missing, logbook not initialised, index missing, index stale, drift open, crisis, case active, all clear — built from the same envelope language (e.g. the envelope with a gap, a dot, a bar); crisis may use the tips only |
| A12 | Chart legend markers, 12 and 16 | release, snapshot, case span start, case span end, crisis — distinguishable in monochrome by shape alone; crisis = a small Prime Radiant tip pair |

Plus: `round-2/README.md` (file list, production notes), `DELIVERY.md`
(decisions, optical corrections, colour values, tools), `LICENSE`
(MIT with copyright assigned to JohnAndrewsX, or CC0 for the artwork —
say which), SVGO-optimised files next to the sources, no metadata with
personal data or dates.

## 3. Brand colours (for A2, A6, A7, A8 only)

Propose a palette of at most three colours with hex values and show it
on the hero. Direction: a deep night-blue family for the background
(the Prime Radiant glows in the dark), the mark in a near-white, one
warm accent used sparingly (never the UI's `urgent` red). Provide the
dark-background version as primary and a light-background version with
the same three colours. Check contrast of the mark on both backgrounds
(≥ 3:1).

## 4. Acceptance (brief §6, plus)

- `rsvg-convert` renders every SVG without warnings; PNGs match.
- Mask SVGs: `currentColor` only, no opacity below 1, no `<style>`.
- A4 placed in the real bar (we do this on the test host): glyph
  centre on the digit centre within 1 px at 1.0 and 1.25.
- A11 and A12 read at their smallest size in both themes.
- No file names with spaces or non-ASCII; `exiftool` clean.

## 5. Questions

As before: one GitHub issue per question on `JohnAndrewsX/jax-seldon`,
label `design`, asset id in the title. Known answers are in brief §6a.
