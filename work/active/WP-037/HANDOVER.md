WP-037 HANDOVER

Branch `wp/037-overlay-live`, worktree `wt/WP-037`, on `main` `83d2435`.
Not pushed, no PR. Commits in `main..HEAD`:
- `ecc0809` plugin: empty period view without aggregation on open;
- `403610b` tests: create the overlay like the shell, service injected
  after;
- `9b3c01d` docs: service injection order, heatmap probe point;
- `8d9ee1e` docs: series probe point inside the plot, live heatmap
  example;
- `306c093` work: live results, plus the memory appends;
- this handover.

## Done

**1. `Model.periodView` without a table (plugin/Model.js).**
- `periodView(table, period)` now falls back to `emptyPeriodView(period)`
  instead of `periodTable(null)`.
- `emptyPeriodView` builds the view straight from the charts' empty
  shapes: `window`, `series` (empty rows, zero risk), `charts`, `slots`.
  It keeps one view per period, so it is built at most four times per
  Model.js instance. No aggregation counter moves.
- The empty shapes now live in one place, `emptyChart(kind)`:
  - `heatmapChart`, `seriesChart`, `driftChart`, `timelineChart`,
    `riskChart` and `planChart` start from it, so the empty view and the
    builders cannot drift apart;
  - `riskChart` fills the zeroed parts in place;
  - the output of every builder is unchanged (`model.test.js` and
    `overlay-view.sh` pass with the same expectations).
- `periodData` in Overlay.qml is unchanged. It re-evaluates when the
  shell assigns `service`, and from then on reads the service's
  precomputed table.
- New unit test (`model.test.js`): for every period, `periodView(null, p)`
  and `emptyPeriodView(p)` serialise exactly like
  `periodTable(null).periods[p]`. The kept object is returned again, and
  `aggregationCount()` does not move.

**2. Harness creation order** (tests/plugin/harness/overlay.qml,
overlay-view.sh).
- `createOverlay()` now does what the shell's Loader `onLoaded` does:
  `createObject(parent, {})`, then `shell`, `manifest` and `service`
  assigned in the shell's order.
- It records `bare` (the item had `service === null` right after
  creation), which `fresh` reports as `firstFrame.bare`.
- `overlay-view.sh` asserts `firstFrame.bare == true` at both fresh opens
  (`fresh #2`, `fresh #12`), next to the existing
  `firstFrame.overlay == 0` and `view.aggregations.overlay == 0`.
- **Proof that this path fails on the old code:**
  - I changed the harness first, with the old `Model.js`, and ran
    `bash tests/plugin/overlay-view.sh`. Full log:
    `work/active/WP-037/harness-old-code.log`.
  - Result: `overlay-view: 308 passed, 6 failed`, with these failures:
    ```
    FAIL ipc #23: .view.aggregations.overlay = 23 (want 0)
    FAIL fresh #2: .firstFrame.overlay = 23 (want 0)
    FAIL fresh #2: .view.aggregations.overlay = 23 (want 0)
    FAIL fresh #4: .view.aggregations.overlay = 23 (want 0)
    FAIL fresh #12: .firstFrame.overlay = 23 (want 0)
    FAIL reflow #6: .view.aggregations.overlay = 23 (want 0)
    ```
  - That is exactly the live number from WP-013. It confirms the
    reviewer's mechanism (one `periodTable(null)` = 1 + 4 splits + plan +
    risk + 16 charts = 23).
  - With the fix: `overlay-view: 314 passed, 0 failed`.

**3. SPEC-PLUGIN §6 and TESTING, the injection order.**
- SPEC-PLUGIN §6 gets this sentence, appended to the paragraph on chart
  data:
  > The shell creates the overlay item first and injects `service`
  > afterwards (its Loader's `onLoaded`: `if ("service" in item)
  > item.service = …`), so every binding first runs with
  > `service === null` and must tolerate that without work
  > (`Model.periodView` then returns a kept empty view, no aggregation
  > pass); the harness creates the overlay in the same order.
- It also gets this layout note:
  > The Heatmap is a square grid bound by the slot's height, left-aligned
  > with its legend beside it; at 30 d and 90 d it fills only the left
  > part of a full-width slot (WP-037).
- TESTING, "Plugin" (overlay-view.sh) gets this sentence:
  > The harness creates Overlay.qml the way the shell's overlay Loader
  > does: without properties, then it assigns `shell`, `manifest` and
  > `service`, so the overlay's bindings first run with `service === null`
  > and must do no work then (SPEC-PLUGIN §6; with `service` as a creation
  > property the harness missed the 23 aggregation passes the live shell
  > showed, WP-013 FINDINGS §5.1). `fresh` reports that as
  > `firstFrame.bare`, and the script asserts it next to
  > `firstFrame.overlay == 0`.

**4. Heatmap probe docs.**
- TESTING, runtime smoke step 4, has a "Heatmap probe" paragraph:
  - the point comes from `Model.heatmapLayout`, using the slot's
    `chart.w`/`chart.h` from `view`, the week columns, `labelW` =
    3 × caption and `labelH` = caption + spacing.sm;
  - `fx = (labelW + (c + ½)·pitch)/w`, `fy = (labelH + (r + ½)·pitch)/h`;
  - the worked live example: w 1410, h 108 → pitch 13, 14 columns,
    today = `0.15,0.5`;
  - in the harness, use `hoverItem:heatmap:<i>` (`chart.locate(i)`).
- The command list there now has `hover "heatmap 0.15,0.5"`.
- plugin/README.md's IPC table says why a fixed fraction misses, and
  points to that paragraph.
- **The generic example changed from `heatmap 0.9,0.5` to
  `series 0.5,0.5`** (README, TESTING, Overlay.qml comments). The live
  test showed that `series 0.9,0.5` is empty too: Series keeps a
  value-label column on the right (`x > plotW` → no hover).
- **Design decision: the grid is not centred.** It stays left-aligned
  with the legend beside it:
  - it reads like a calendar, with the legend next to the cells;
  - it lines up with the slot title and the other charts' left edge;
  - centring would leave empty space on both sides instead of one, and it
    would move the legend's fallback placement;
  - stretching the cells would break the square grid.

  The docs say so, and SPEC §6 records it.

**5. Live verification on the test host.** Details:
`work/active/WP-037/live/RESULTS.md`.
- Setup:
  - `plugin/` copied with `rsync -rp --checksum` (only the 3 changed
    files rewritten), and `omarchy plugin validate` exit 0;
  - a stand-in engine plus `fixtures/index.sample.json` with
    `generatedAt` = now (TESTING step 3). Service `ok`, pill `⟡ 2 · 4`;
  - `omarchy-restart-shell` after `settle`, and only while unlocked. No
    new crash report.
- Fresh open (`shell summon`) → `call view`: **`aggregations.overlay` 0**
  (WP-013: 23), `service` 92, **every chart `paints` 1**, `paintMs` 0–2.
- A second fresh open on 365 d after the theme sweep gave the same:
  overlay 0, paints 1 each.
- `hover`:
  - heatmap `0.15,0.5` → "Thu 2026-10-01 · 30 events · …";
  - `0.035,0.5` → "Thu 2026-07-09 · 0 events";
  - `0.9,0.5` → empty, as now documented;
  - `series 0.5,0.5` → "2026-09-01 · explicit 323 · total 2004";
  - `driftBars 0.97,0.5` → "2026-W40 …";
  - `""` clears;
  - hovering repainted nothing.
- Themes:
  - Each change used exactly
    `ssh <host> 'OMARCHY_PATH=/usr/share/omarchy omarchy theme set "<theme>"'`,
    with no suffix, preceded by a lock-status call.
  - ssh exit 0 each time. A separate ssh call read `theme.name`:
    `tokyo-night`, `catppuccin-latte`, then `osaka-jade` (restored).
  - The open overlay repainted each chart once per theme and aggregated
    nothing.
- Screenshots:
  - files: `tokyo-night-overlay.png`, `catppuccin-latte-overlay.png`,
    `osaka-jade-overlay.png`;
  - each is `grim -o HDMI-A-2` at 1920×1080 (scale 1.25), taken only
    after a lock-status check and `view.opened == true`;
  - the top 28 px (bar with window and media titles) are cropped →
    1920×1052 at real pixel scale.
- Shell log: "Configuration Loaded", no WARN/ERROR line naming
  jax.seldon.
- No keys and no pointer events were sent; everything went through IPC.

## Not done

- **No `QSG_RENDER_TIMING` or other live frame timing.** It needs a
  Hyprland env change, and that is the operator's call. The first-frame
  claim stays proven headless; live, I sampled `view` ~2 s after the open.
- **The shell's Loader is `asynchronous: true`; the harness creates the
  item synchronously.** The order of injection is the same, and that is
  what mattered here. Incubation timing is not reproduced.

## Verified by

```
$ node tests/plugin/model.test.js                 → model.test.js: 73 passed
$ bash tests/plugin/overlay-view.sh (new harness, OLD Model.js)
                                                  → 308 passed, 6 failed (overlay = 23, list above)
$ bash tests/plugin/overlay-view.sh (new harness, fix)
                                                  → 314 passed, 0 failed
$ just check                                      → exit 0, "check: ok" (before the live session, and again on the final tree)
live (test host): summon + call view              → {"aggregations":{"service":92,"overlay":0}, paints [1,1,1,1,1,1]}
live: hover "heatmap 0.15,0.5"                    → "Thu 2026-10-01 · 30 events · pacman 7 · …"
live: theme set ×3 (bare form), theme.name        → tokyo-night, catppuccin-latte, osaka-jade
```

**Test host left as found, apart from the fresh plugin copy:**
- `~/.local/bin/seldon`, `~/.local/state/seldon`, `~/.config/seldon` and
  `~/Seldon-e2e` are absent;
- the plugin is enabled, and its dir hash `1fde983c5e80` equals `plugin/`
  at HEAD (it was `644fd01ae2c8`);
- the theme is Osaka Jade;
- there is one shell instance, and 4 crash reports (unchanged);
- the `ls -A ~` hash is unchanged;
- the service is `engineMissing`, as found;
- the session is unlocked.

The dev host: no plugin install, and the real Seldon dirs were untouched
(`real-home-guard` in the plugin tests).

## Learned (appended to memory/omarchy-shell.md and memory/pitfalls.md, "WP-037")

- The shell's Loader assigns `omarchyPath`, `shell`, `manifest`, …,
  `service` after creation. Every binding first runs with them `null`, so
  the null path must be a kept constant.
- A harness must create items in the shell's order. Changing the harness
  first and keeping the old code reproduced the live 23 exactly.
- Probe fractions depend on chart geometry. The heatmap is height-bound;
  Series keeps a label column on the right, so `series 0.9` is empty.
- `omarchy theme set` over plain ssh with only `OMARCHY_PATH` works. An
  open overlay repaints once per theme change.
- Crop the test host's bar (window and media titles) from live shots. A
  stand-in engine plus the fixture index is enough for live chart shots.
- `rsync -a --checksum` still sets the mtime on every file;
  `-rp --checksum` rewrites only the changed files.

## Decisions needed

None. The heatmap grid stays left-aligned; that is my design decision,
stated above and in SPEC §6, and open to review.

## Touched outside WP scope

- `memory/omarchy-shell.md` and `memory/pitfalls.md`: appends, as the
  brief asked.
- `riskChart` and the other chart builders were refactored to start from
  `emptyChart(kind)`. Their output did not change.
- Nothing under `engine/`, `schema/` or `fixtures/`.
