WP-031 HANDOVER

Branch `wp/031-charts`, worktree `wt/WP-031`, on top of `main` at
`a651a76`. Not pushed, no PR. Commits `main..HEAD`:
`e1fb988` Model helpers and one-pass period table · `f927f55` charts and
The Plan · `a913219` harness · `370adae` README, TESTING, memory ·
`731652c` renders · then `work: WP-031 handover` (this file).
`just check` exits 0 at `731652c` (see Verified by).

## Done

- **Six slots, drawn from precomputed data only** (`plugin/components/overlay/`):
  - `ChartCanvas.qml` (new, shared base): a `Canvas` over the chart area,
    hover plumbing (a `MouseArea` with `hoverEnabled`), a highlight item
    (hover never repaints), the empty state, `paints`/`paintMs` counters,
    `probe(fx, fy)` and `locate(i)` for IPC and tests. Charts handle its
    signals `paintRequested(ctx, w, h)`, `hoverRequested(x, y)` and
    `locateRequested(i)`. The canvas repaints on chart data, size or theme
    colour change only.
  - `Heatmap.qml`: the period's days as ISO weeks × weekdays (Monday on
    top; 5 columns at 30 d, 14 at 90 d, 53 at 365 d / All), five steps of
    the accent via `Util.alpha` (`Model.CHART_STEP_ALPHAS`, square root of
    count/max) and a faint cell for none, month names, Mon/Wed/Fri, a
    less–more legend. Hover: "Thu 2026-10-01 · 30 events · pacman 7 · agent
    6 · …" (sources by count).
  - `Series.qml`: explicit and total as step lines, one lane each (they
    differ ×6, so they share the time axis, not the value axis), with
    range and first/last day. Hover: the sample that holds at the pointer.
  - `DriftBars.qml`: opened (accent) / resolved (foreground) per ISO week,
    gaps between weeks filled with zeros. Hover: "2026-W40 · 28 Sep – 4 Oct
    · opened 6 · resolved 2".
  - `RiskDonut.qml`: R0–R2 accent steps, R3 `Color.urgent`, the count and
    "all time" in the centre, a legend. Hover on ring or legend: "R2 · 4
    cases · 50% · all time".
  - `Timeline.qml`: releases, snapshots, crises (urgent) on a marker band;
    case spans in lanes below (open ones bright, to the end of today;
    clipped to the window); month lines. Hover: "release · Omarchy 4.0.6-1 ·
    2026-09-15 20:13", "case · C-2026-003 … · 2026-09-26 – open".
  - `ThePlan.qml`: `cases.active` as cards (zone stripe as on CaseCard, id
    and risk, title, a steps bar, "4/5 steps · agent: claude-code"); cards
    that do not fit are counted in the caption.
  - Each chart's **caption** sits in its slot's title row
    (`OverlaySlot.qml`): the summary ("62 events on 13 of 90 days · busiest
    2026-10-01 (30)"), replaced by the hover read-out. Empty states: "no
    data in this period"; the donut "no cases yet · all time"; The Plan "no
    active cases". `Accessible.role: Chart` with the summary as
    description.
- **Wiring (`Overlay.qml`):** six Components bound to
  `periodData.charts.<slot>`, a Loader per slot (`placeholder: false`);
  `view()` adds `aggregations { service, overlay }` and per slot `chart {
  summary, numbers, empty, hover, paints, paintMs, w, h }`; new IPC method
  `hover "<slot> <fx>,<fy>"` (`hover ""` clears).
- **Grid:** `GRID_ROWS` has The Plan as a fourth row in every mode (wide
  weights 3:4:2:2). `overlayGrid` now gives rows that would fall under the
  minimum height their minimum and shares the rest by weight (new
  `rowHeights`, filling the height exactly), so the grid scrolls only when
  the minimums alone do not fit. Result: no scroll at 1920×1080 **and** at
  1.25 scale (1536×864 logical) and 2560×1440.
- **`plugin/Model.js`** (pure, node-tested): `periodTable` now walks each
  series once for all four periods (`splitSeries`, identical rows to
  `seriesInPeriod`, asserted) and builds the chart data: `heatmapChart`,
  `seriesChart`, `driftChart`, `riskChart` (one object for all periods),
  `timelineChart` (rows parsed once per table, markers shared across
  periods, lanes by interval partitioning with a min-heap), `planChart`;
  geometry/hit helpers `heatmapLayout`, `heatmapCellAt`, `seriesPointAt`,
  `riskPartAt`, `timelineLayout`, `timelineItemAt`, `planColumns`, `scale`;
  texts `heatmapCellText`, `seriesPointText`, `driftWeekText`,
  `riskPartText`, `timelineItemText`; dates `dayNumber`/`dateOfDay`
  (calendar arithmetic, no `Date.parse`), `isoWeekOf`, `weekdayOfDay`;
  `colourStep`; `aggregationCount()` (counts table/chart passes per script
  instance). `isDate` now rejects impossible dates (2026-02-30), which V8's
  `Date.parse` rolled over.
- **Tests**
  - `model.test.js`: 72 (+10): six slots, `rowHeights`, every chart helper
    on the sample and edge rows, date arithmetic against `Date` for every
    day 1899–2101, `splitSeries` ≡ `seriesInPeriod`, the aggregation count.
  - `model.bench.js` (new, in `just plugin-test`): see the profile note.
  - `overlay-view.sh`: 302 checks (332 with `OVERLAY_SHOTS`), new harness
    steps `fresh[:json]` (a new Overlay.qml per open, as the shell's Loader
    does; `firstFrame` sampled on the window's first `frameSwapped`),
    `hover`, `hoverItem` (real mouse moves onto `chart.locate(i)`),
    `leave`, `resize`. Asserted: every chart's summary per period (30/90/
    365/All), hover texts from real mouse moves on all six charts and from
    `call hover`, **first frame after a fresh open: service aggregation
    count unchanged, overlay's 0, paints 0; every chart painted exactly once
    by frame 2**; period switches aggregate nothing and repaint only the
    windowed charts; hovering repaints nothing; a resize repaints once; the
    medium/narrow reflow with The Plan; not-initialised: all six empty
    states, nothing painted; every other index variant renders every chart;
    layouts at 1920×1080, 2560×1440, 1536×864, 2048×1152, QT_SCALE_FACTOR
    1.25 without scroll or text overflow; clean logs; real-home guard.
- **Docs:** `plugin/README.md` (charts, captions, empty states, the `hover`
  IPC row, the phase note), `docs/TESTING.md` (bench row, layer 1 and 3b
  text, smoke step 4 `hover` line).
- **Renders:** `work/active/WP-031/screenshots/offscreen-<theme>-<size>.png`
  (90 d) and `…-365d-hover.png` (365 d, pointer on the heatmap's last day)
  for Osaka Jade, Tokyo Night, Catppuccin Latte (light) at 1920×1080 and
  2560×1440. **Offscreen renders** of the real Overlay.qml with the shell's
  Commons/Ui and each theme's `colors.toml`, not live screenshots.
- `memory/omarchy-shell.md`, `memory/pitfalls.md`: WP-031 findings.

## Profile note

- **Harness (offscreen, dev host, software render loop,
  `QSG_RENDER_TIMING=1` + `QT_LOGGING_RULES=qt.scenegraph.time.renderloop=true`):**
  fresh open at 1920×1080: frame 1 4 ms (render 3), frame 2 6 ms (polish 4
  = the six canvas paints); at 2560×1440: 5 ms and 7 ms. Period switch: one
  frame of 4–7 ms. Creating and opening the overlay (QML instantiation,
  before frame 1): 8–10 ms (`firstFrame.createMs`); in the shell that
  Loader is asynchronous. Each chart's own drawing calls: 0–2 ms
  (`paintMs` in `call view`). No aggregation in any of these frames
  (asserted).
- **Precomputation under node (`node tests/plugin/model.bench.js`,
  median of 31):**

  | index | periodTable sandbox / plain | WP-030 cut alone sandbox / plain |
  |---|---|---|
  | sample | 4.6 / 0.55 ms | 2.0 / 0.19 ms |
  | sample ×10 (3660/30/50/180 rows) | **14.1** / 1.8 ms | 19.9 / 1.5 ms |
  | 7000 timeline rows | 34.9 / 5.4 ms | 43.3 / 3.9 ms |

  "sandbox" is Model.js in a node `vm` context as model.test.js loads it
  (every top-level name a slow contextified lookup, ~8× slower); "plain" is
  one function scope. The budget (×10 under 20 ms) holds in the stricter
  sandbox and is a gate in `just plugin-test` (fastest of 31 runs). The new
  table does strictly more than the old cut (it also builds all chart data)
  and is still faster than the old cut alone on ×10 and at 7000 rows in the
  sandbox. 7000 timeline rows exceed 20 ms in the sandbox (5.4 ms plain); it
  was not the stated budget, and the engine's real timeline (releases,
  current snapshots, non-dropped cases, open crises) is far smaller.
- **Live frame timing on the test host: not done (locked).** Recipe for an
  unlocked host: `call view` gives `paintMs`/`paints`; for
  `QSG_RENDER_TIMING` the variable must be in Hyprland's environment
  (`omarchy-launch-shell` is spawned through `hyprctl dispatch exec`):
  `hyprctl keyword env QSG_RENDER_TIMING,1`, `hyprctl keyword env
  QT_LOGGING_RULES,qt.scenegraph.time.renderloop=true`, `omarchy-restart-shell`,
  toggle the overlay, read `journalctl --user -t omarchy-shell`. That is a
  runtime change on the test host — operator's decision (Decisions 2).

## Not done

- **Live steps on the test host.** Locked, `secure: true` since
  13:13:21Z (checked 16:23Z before any work and 16:58Z after). No `wtype`,
  no `grim`, no `omarchy-restart-shell`, no copy into its plugin folder,
  and I did not run the private offscreen harness there either this time
  (the dev-host harness covers the same code; WP-030 showed both hosts
  agree). **Pending once unlocked:** copy `plugin/`, restart, `shell
  toggle jax.seldon`, `shell call jax.seldon view ""` (check `aggregations`,
  each chart's `paints` = 1, `paintMs`, the slot sizes at the real 1.25
  scale), `shell call jax.seldon hover "heatmap 0.9,0.5"`, a real mouse
  hover over each chart, `wtype 1`…`4`, `grim` in three themes, the frame
  profile above.
- Dev host: I did not copy the plugin into
  `~/.config/omarchy/plugins/jax.seldon` (allowed, not needed for the
  checks).
- **One known extra paint:** when a status banner is shown (index-stale),
  the banner's Column settles during frame 1 and Qt's Canvas paints once
  more at the same size (no plugin code requests it; `renderStrategy` and
  `renderTarget` make no difference). The harness asserts ≤ 2 paints there
  and exactly 1 everywhere else. Removing it would mean giving the shared
  `Banner.qml` a non-positioner height (outside this WP).

## Verified by

```
$ just check                                   → exit 0 (at 731652c's code)
  validate-fixtures: ok — 109 instances, 8 variants, 23 self-checks
  plugin-validate: ok · tokens: ok (520 references) · qmllint: ok (28 files)
  model.test.js: 72 passed · model.bench: ×10 14.09 ms sandbox (median)
  service-states: 189 passed · panel-view: 548 passed
  overlay-view: 302 passed, 0 failed · plugin-test: ok · check: ok
$ OVERLAY_SHOTS=work/active/WP-031/screenshots bash tests/plugin/overlay-view.sh
                                               → 332 passed, 0 failed (12 renders)
$ find plugin -type l | wc -l                  → 0
$ grep -nE '"#[0-9a-fA-F]{3,8}"|Qt\.(rgba|rgb|hsla|hsva|color)\(|olor\s*[:=]\s*"[a-z]+"|Style\s*=\s*"' \
    plugin/*.qml plugin/components/*.qml plugin/components/overlay/*.qml
                                               → only OverlayWindow.qml `color: "transparent"` (accepted)
$ ssh test 'omarchy-shell lock status'         → {"locked":true,"secure":true} at 16:23Z and 16:58Z
```

The token check (`check-tokens.py`) checks token *names* only; the
literal-colour rule was checked with the grep above (no new literals; the
charts' canvas colours are `Util.alpha(Color.*)` values).

## Learned

In `memory/omarchy-shell.md` (WP-031): Canvas paints on frame 2 and
repaints itself on geometry changes; a Column's implicitHeight lands one
frame late and resizes what is below it; `ctx.fillStyle` takes QML colour
values; base-component signals instead of function overriding keep qmllint
clean; non-library JS imports have per-instance state; offscreen
`frameSwapped`/`QSG_RENDER_TIMING` work; the live shell takes Hyprland's
environment. In `memory/pitfalls.md`: the node `vm` sandbox ~8× slowdown;
V8 `Date.parse` rolls impossible dates over; first-fit lane packing is
quadratic; the harness's `toggle` overlay is long-lived (use `fresh`); jq
`input` in `expect`.

## Decisions needed

1. **SPEC-PLUGIN §6 wording** (spec owner's edit; code and spec differ in
   detail). Proposed text for rows 2–5:
   - Row 2 (full width): **Heatmap** — events per day of the period as ISO
     weeks × 7 days (53 × 7 at 365 d and All), five steps of the theme
     accent; hover shows the date and counts by source.
   - Row 3: **Series** explicit and total packages over time (step lines,
     one lane each) · **DriftBars** drift opened vs resolved per ISO week ·
     **RiskDonut** cases by risk, all time.
   - Row 4 (full width): **Timeline** — Omarchy releases, snapshots and
     crisis markers on one band; cases as spans from created to closed
     (open cases run to today), packed in lanes.
   - Row 5 (full width): **The Plan** — active cases (`cases.active`) as
     cards with step progress and agent (no period; the sixth slot, WP-031).
   - New paragraph: "Each chart's summary is its caption in the slot's
     title row (also its accessible description and in `call view`); while
     the pointer is on the chart the caption shows the hovered item. A chart
     without data in the period says 'no data in this period'. Chart data
     is prepared by the service (`Model.periodTable`) when the index
     changes; the overlay only draws (one paint per chart per data or size
     change)."
   - §7: "error for crisis" → "`Color.urgent` for crises and R3".
   - §8 IPC: add `shell call jax.seldon hover "<slot> <fx>,<fy>"`.
2. **Live frame profile on the test host** needs `QSG_RENDER_TIMING` in
   Hyprland's runtime environment (`hyprctl keyword env …`) plus a shell
   restart once the host is unlocked. Allowed, or keep the offscreen
   profile above as the record?
3. The 7000-timeline-row case is 35 ms in the node sandbox (5 ms plain).
   Fine as is, or should it get a budget of its own?

## Touched outside WP scope

- `plugin/Service.qml`: only a comment on `periods` (precomputation now
  includes chart data) and a read-only `aggregationCount()` function for
  the harness/`call view`. No new binding, no I/O.
- `plugin/components/overlay/OverlayHeader.qml` (WP-030's file): its
  height is now summed from its texts instead of a Column's polish-time
  `implicitHeight`, which resized the charts after their first paint.
- `plugin/components/overlay/OverlaySlot.qml`: `chart` property and the
  caption in the title row (the slot wiring).
- `plugin/components/overlay/ChartCanvas.qml`: a seventh file, the shared
  chart base.
- `tests/plugin/harness/overlay.qml` (WP-030's harness): the new steps.
- `justfile`: `plugin-test` runs `node tests/plugin/model.bench.js`.
