WP-030 HANDOVER

Branch `wp/030-overlay-skeleton`, worktree `wt/WP-030`, on top of `main`
at `cc3f00b`. Not pushed, no PR. Commits `main..HEAD`:
`61ac3f6` Model helpers · `98bfc76` overlay QML · `bea95fb` harness ·
`675cfa5` README, TESTING, memory · then `work: WP-030 handover and
renders` (this file and the renders). `just check` exits 0 at HEAD (see
Verified by).

## Done

- **`plugin/Overlay.qml`** (replaces the WP-001 stub), the Prime Radiant:
  - a fullscreen window with the scrim `Color.menu.scrim` and a card
    (`Color.popups.*`, `Border.surfaceSpec("popups", …)`);
  - **header** (`components/overlay/OverlayHeader.qml`): "Prime Radiant";
    machine · Omarchy version · generated time (`Model.overlayMeta`); on
    the right the period selector (qs.Ui `ButtonGroup`, mouse only, so the
    keys stay with the overlay), *Close*, and the period's dates
    ("90 d · 2026-07-04 – 2026-10-01");
  - the service's **banner** when the status is not ok, with only the fix
    that runs no command (*Copy*; `Model.overlayBanner`) and the hint "Fix
    it from the Seldon panel (click ⟡ in the bar)." The data still shows
    whenever an index is loaded;
  - **grid** of five slots on 12 columns (`Model.overlayGrid`): Heatmap
    (12) | Series, DriftBars, RiskDonut (4 + 4 + 4) | Timeline (12), rows
    by weight 3:4:2. Narrower cards reflow to two columns (Heatmap |
    Series DriftBars | RiskDonut Timeline), then one slot per row, and the
    grid scrolls (Flickable) when the minimum heights do not fit;
  - **slots** (`components/overlay/OverlaySlot.qml`): name, what the chart
    draws, and as placeholder the series' row count for the period plus
    one line ("90 days / 62 events", "3 samples / Explicit 323 → 327",
    "5 weeks / 13 opened · 9 resolved", "8 cases / R0 1 · R1 3 · R2 4 ·
    R3 0 · all time", "18 entries / 8 cases · 2 releases · 6 snapshots · 2
    crises"). WP-031 puts its chart into the slot's default property and
    sets `placeholder: false`;
  - **footer** with the keys.
  - Keys through the shell's `PanelKeyCatcher`: `1`–`4` (30 d / 90 d /
    365 d / All), ←/→ and `h`/`l` (previous/next, wrapping like the panel
    tabs), Esc closes. A click on the scrim or *Close* closes.
  - `open(payloadJson)` / `close()` / `opened` as before; Esc, scrim and
    *Close* go through `dismiss()` → `shell.hide(manifest.id)`.
    `summon jax.seldon '{"period":"30"}'` opens on that period.
  - Read-outs over `omarchy-shell shell call jax.seldon …` (SPEC §8 allows
    `call` on the loaded overlay): `view ""` returns JSON (opened, period,
    window, caption, status, banner, meta, grid mode, scrolls, size, each
    slot's counts and geometry in window coordinates); `setPeriod <id>`.
  - Properties for WP-031: `period` and `periodData` (`{ window, series:
    { heatmap, packages, drift, timeline, risk }, slots }`, the rows already
    cut to the period).
- **`components/overlay/OverlayWindow.qml`**: the layer-shell
  `PanelWindow` (namespace `jax-seldon-prime-radiant`, overlay layer,
  exclusive keyboard focus, every edge, no exclusive zone) in a file of its
  own, so the harness can swap it for a plain Item.
- **First frame stays cheap:** the overlay item is destroyed on every hide
  (the shell deactivates its Loader), so nothing is computed there.
  `Service.qml` got one binding, `periods: Model.periodTable(index)`:
  windows, cut series and slot counts for all four periods, recomputed
  when the index changes (about 2 ms on the sample under node). Opening
  and switching periods are lookups.
- **`plugin/Model.js`**: `PERIODS`, `PERIOD_DEFAULT` ("90"),
  `OVERLAY_SLOTS`, `isPeriod`, `periodById`, `periodForKey`, `cyclePeriod`,
  `overlayPayloadPeriod`, **`periodWindow`** (inclusive days ending on the
  index's today, ADR-0012 §10; *All* unbounded), `dateInWindow`,
  `spanInWindow`, `isoWeekMonday`, **`seriesInPeriod`** (heatmap/packages
  by date, a drift week when any of its days is in the window, case spans
  when they overlap, an open case runs on; broken rows left out),
  `riskCounts`, `slotSummary`, `periodTable`, `periodView`,
  `periodCaption`, `overlayMeta`, `overlayBanner`, `overlayGrid`.
- **Tests**
  - `model.test.js`: 62 (+9): the period helpers, ISO weeks (W53 only in
    long years), window edges (a case ending on the first day, an event at
    23:59 the day before, future rows), the sample's counts per period,
    no index, meta, banner, the grid's three modes with exact gaps.
  - **`tests/plugin/overlay-view.sh`** (new, in `just plugin-test`) with
    `harness/overlay.qml` and the stand-in `harness/OverlayWindow.qml`:
    180 checks, 210 with `OVERLAY_SHOTS`. IPC semantics through a stand-in
    shell facade (toggle open/close, summon with payload, hide via Esc,
    scrim, *Close*), keys, a chip click (keys still work after it),
    `call view` and `call setPeriod`; the sample's rows per period
    (`30,2,5,3,17` · `90,3,5,3,18` · `365,3,5,3,18` · `366,3,5,3,18`);
    layout at 1920×1080, 2560×1440, 1536×864, 2048×1152 (each for several
    periods) and with `QT_SCALE_FACTOR=1.25`: five slots inside the
    window, no text outside its slot or the window, no scrolling, three
    columns; 760×1000 two columns, 560×700 one column and scrolling; the
    not-initialised banner with *Copy* only; a clean log in every run; the
    real-home guard.
  - `justfile`: qmllint and the token check include
    `plugin/components/overlay/*.qml`; `plugin-test` runs overlay-view.sh.
- **Docs:** `plugin/README.md` (a Prime Radiant section with keys, IPC
  rows for summon with a period, `call view`, `call setPeriod`; the phase
  note), `docs/TESTING.md` (the lint/test table, layer 1 sentence, a new
  layer 3b, smoke step 4 commands).
- **Renders:** `work/active/WP-030/screenshots/offscreen-<theme>-<size>.png`
  for Osaka Jade, Tokyo Night, Catppuccin Latte at 1920×1080 and
  2560×1440, plus `testhost-offscreen-osaka-jade-1920x1080.png` (the same
  harness on the test host). These are **offscreen renders** of the real
  Overlay.qml with the shell's own Commons/Ui and each theme's
  `colors.toml`, **not live screenshots**.
- `memory/omarchy-shell.md`: WP-030 findings.

## Not done

- **Live keys, screenshots and a shell restart on the test host.** It has
  been `secure: true` since 13:13:21Z (checked before and after my runs).
  No `wtype`, no `grim`, no `omarchy-restart-shell`, and I did not copy
  the new plugin into the test host's plugin folder (a restart is refused
  while locked, so the running shell would keep the old code anyway). The
  running shell still has main's stub overlay. **Pending once unlocked:**
  copy `plugin/`, restart, then `shell toggle jax.seldon`, `shell call
  jax.seldon view ""` (check `size`, `mode`, the slot geometry at the real
  1.25 scale), `wtype 1`…`4`, `wtype -k Left/Right`, `wtype -k Escape`, a
  click on the scrim, the pill's middle click, `grim` in three themes.
- **The pill's middle click** is WP-010's wiring (`BarWidget.openOverlay`
  → `bar.shell.toggle("jax.seldon", "")`), unchanged; I checked the code
  path but could not click it live (lock).
- **SPEC-PLUGIN §6 differs from this WP** and I did not edit the spec
  (Decisions needed 1, 2).
- The period resets to 90 d on every open (the item is recreated). Keeping
  it across opens would mean storing it in the service; not asked for.

## Verified by

```
$ just check                                   → exit 0 (code as committed)
  fmt-check, clippy ok · engine tests: 321 passed, 0 failed (21 test binaries)
  validate-fixtures: ok — 109 instances (incl. 8 expected failures), 8 variants, 23 self-checks
  plugin-validate: ok · tokens: ok (424 references) · qmllint: ok (21 files), --max-warnings 0
  model.test.js: 62 passed · service-states: 189 passed · panel-view: 548 passed
  overlay-view: 180 passed, 0 failed · real-home guard: untouched · check: ok
$ node tests/plugin/model.test.js              → 62 passed
$ bash tests/plugin/overlay-view.sh            → 180 passed, 0 failed
$ OVERLAY_SHOTS=work/active/WP-030/screenshots bash tests/plugin/overlay-view.sh
                                               → 210 passed, 0 failed (six renders)
$ find plugin -type l | wc -l                  → 0
```

**Test host (Omarchy 4.0.4-1, quickshell 0.3.1, locked, `secure: true`).**

A. IPC on the running shell (main's stub overlay, jax.seldon enabled,
status engineMissing), with `OMARCHY_PATH` and `$OMARCHY_PATH/bin`
exported:

```
lock status                → {"locked":true,"secure":true}
shell call jax.seldon close ""   → unknown   (overlay not loaded)
shell toggle jax.seldon ""
shell call jax.seldon close ""   → ok        (loaded and reachable)
shell toggle jax.seldon ""  ×2
shell hide jax.seldon
shell call jax.seldon close ""   → unknown   (unloaded again)
quickshell log (by-pid/…/log.qslog): no new lines from these calls; same shell pid before and after
```

B. New code in a private offscreen Quickshell on the test host:
`plugin/`, `tests/`, `fixtures/` rsynced to a temp dir, `OMARCHY_PATH=
/usr/share/omarchy OVERLAY_SHOTS=… bash tests/plugin/overlay-view.sh` →
**208 passed, 0 failed** (before the chip-then-key check was added; the
rest identical), against the test host's own shell Commons/Ui; it never
talks to the running shell and no key reaches the seat. Temp dir removed.

## Learned

In `memory/omarchy-shell.md` (WP-030): the overlay item is destroyed on
every hide, so state and precomputation belong in the service; `shell
call` stringifies results (return JSON text) and answers `unknown` while
unloaded; `PanelWindow` is swapped through a plugin-owned wrapper file in
a plugin copy; IPC toggle/hide work and log nothing on a locked host;
`Style.space` scales with the font size, so minimum sizes need a scroll
fallback; a `focusable: false` ButtonGroup leaves the keys where they are.

## Decisions needed

1. **"All" period.** The WP asks for 30/90/365/all; SPEC-PLUGIN §6 lists
   30/90/365. Implemented as the WP says; the spec line should gain "all"
   (spec owner's edit).
2. **Row 5 "The Plan"** (active cases with step progress and agent) is in
   SPEC-PLUGIN §6 but not among the WP's five slots, and not in WP-031's
   chart list either. Which WP owns it, or does the spec drop it? The grid
   function takes rows from a table (`GRID_ROWS`), so a sixth slot is a
   small change.
3. **Default period**: I chose 90 d (`PERIOD_DEFAULT`). Change if the
   operator prefers 365 d (the heatmap's natural span).
4. **Risk is not windowed**: `series.risk` has no dates, so RiskDonut
   shows the same counts for every period, labelled "all time". A
   period-aware donut would need dated risk data (contract change) or the
   plugin counting `cases` itself.

## Touched outside WP scope

- `plugin/Service.qml`: one new binding, `periods` (the precomputed period
  table; the brief asked for the work to be bound to index changes, and
  the overlay item does not survive a close).
- `justfile`: the qmllint file list and `plugin-test` (new harness).
