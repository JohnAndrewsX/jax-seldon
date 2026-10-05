# WP-031 — Live frame profile of the Prime Radiant (test host, 2026-10-05)

The live half of WP-031's profile note, done by the orchestrator on the
operator's word. Engine and plugin 0.1.3, Omarchy 4 shell, one monitor
1920×1080 at scale 1.25 (logical 1536×864, layout `wide`), Intel UHD 630
(i5-8400), threaded render loop, 60 Hz (swap ≈ 16 ms).

## How

- No change to Hyprland's environment: the shell was stopped like
  `omarchy-restart-shell` does and relaunched once with
  `hyprctl dispatch 'hl.dsp.exec_cmd("env QSG_RENDER_TIMING=1
  QT_LOGGING_RULES=qt.scenegraph.time.renderloop=true omarchy-launch-shell")'`;
  the frame log went to `journalctl --user -t omarchy-shell`. Afterwards a
  plain `omarchy-restart-shell` (variables checked absent in the new
  process) and the original `index.json` restored byte-identical.
- Three indexes, each swapped in with `cp` + `mv`: the test logbook's own
  index (small), `fixtures/index.sample.json`, and the sample ×10 (every
  series and case group repeated ten times, as `tests/plugin/model.bench.js`
  builds it); `generatedAt` set to now.
- Per index five rounds: `shell summon jax.seldon`, 2 s, `call view`,
  `setPeriod` 30 → 365 → all → 90 (1 s each), `call view`, `shell hide`.
  Only IPC calls, no keys. The overlay's window was identified as the one
  not rendering before the command.

## Results (max over 5 rounds per row)

| index | open: frames | open: sync+render | open: polish | open: GUI blocked for sync | first frame after the command | period switch: sync+render / polish |
|---|---|---|---|---|---|---|
| test logbook | 6 | 23 ms | 11 ms | 41 ms | 88–162 ms | 2 / 7 ms |
| sample | 6 | 25 ms | 12 ms | 41 ms | 91–101 ms | 1 / 7 ms |
| sample ×10 | 6 | 26 ms | 15 ms | 37 ms | 92–103 ms | 1 / 8 ms |

`call view`: overlay aggregations 0 in every view (the first-frame rule
holds live), each chart 1 paint of 0–2 ms.

## Reading

- **Period switches** stay far inside a 16.7 ms frame at every size: no
  dropped frame.
- **Open:** the overlay's first frame costs 23–26 ms on the render thread,
  almost independent of the data (×10 adds 3 ms): it is the first frame of
  a new layer surface (glyph distance fields and atlas uploads in the same
  frame), not Seldon's aggregation (0) or painting (≤ 2 ms). Nothing is on
  screen before it, so it shows as a window appearing ~100 ms after the
  command (IPC, async Loader, surface configure), not as a stutter inside
  the overlay. The GUI thread waits up to 41 ms for that first frame once
  per open; other windows driven by that thread (the bar) can miss two
  frames at that moment.
- Offscreen profile (WP-031 handover, software loop): 4–7 ms frames — the
  live first frame is higher because of the GPU uploads.
- Seen on the side: one other shell window rendered continuously at 60 fps
  in idle during the whole run — the bar with a scrolling media title (a
  player was active). The plugin has no endless animation (no
  `Animation.Infinite`, its only always-running timer ticks once a minute).

Verdict: acceptance "no frame drops on open" holds for the overlay's own
frames after the first; the first frame of a new surface costs one to two
frame periods, independent of the index size. Possible follow-up (not
queued): keep the overlay's item alive between opens to skip the
first-surface cost — trades memory for the 100 ms; only worth it if users
notice.
