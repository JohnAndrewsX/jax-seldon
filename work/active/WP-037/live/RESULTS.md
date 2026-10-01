# WP-037 live session on the test host — results

2026-10-01, 23:47–00:00 CEST.
- Branch `wp/037-overlay-live`. The plugin copy at the end equals `plugin/`
  at the handover commit (tree hash `1fde983c5e80`; it was `644fd01ae2c8`).
- Omarchy 4.0.4-1, quickshell 0.3.1, output HDMI-A-2 1920×1080 at scale
  1.25 (1536×864 logical).
- The session was unlocked throughout. `omarchy-shell lock status` ran
  before the plugin copy, the restart, every theme change and every
  screenshot, inside the same remote script where it gated an action.
- No keys and no pointer were sent. Everything went through IPC
  (`shell summon|hide`, `shell call jax.seldon view|hover`), so the §11
  `opened`/`editing` gate had nothing to guard. The screenshot script also
  required `view.opened == true` before `grim`.

**Setup** (docs/TESTING.md, runtime smoke step 3):
- `plugin/` copied with `rsync -rp --checksum --delete`. Only the changed
  files were rewritten (`Model.js`, `Overlay.qml`, `README.md`), and
  `omarchy plugin validate` gave exit 0.
- A stand-in engine in `~/.local/bin/seldon`. It answers `--version` and
  writes nothing.
- `fixtures/index.sample.json` as `~/.local/state/seldon/index.json`,
  with `generatedAt` and `state.lastCapture` set to now. The machine name
  in it is `workstation-7f3a`.
- After `jax.seldon.service refresh`, the service reported `ok`, pill
  `⟡ 2 · 4`.
- `omarchy-restart-shell` ran after `settle` (ping, then 5 s) and an
  unlocked check. It exited 0. There was still one shell instance and 4
  crash reports, the same as before.

| # | Step | Result | Evidence |
|---|---|---|---|
| 1 | Fresh open after the restart: `shell summon jax.seldon`, then `shell call jax.seldon view ""` | **pass**: `aggregations.overlay` 0 (WP-013: 23), `service` 92, every chart `paints` 1, `paintMs` 0–2, wide mode, 1536×864 | `view-fresh-open.json` |
| 2 | `hover "heatmap 0.15,0.5"`, a point from `heatmapLayout` (w 1410, h 108 → pitch 13, 14 columns, today = column 13) | **pass**: "Thu 2026-10-01 · 30 events · pacman 7 · …" | `hover.log` |
| 3 | `hover "heatmap 0.035,0.5"` (column 1, row 3) | pass: "Thu 2026-07-09 · 0 events" | `hover.log` |
| 4 | `hover "heatmap 0.9,0.5"` | empty, as documented now (right of the grid) | `hover.log` |
| 5 | `hover "series 0.9,0.5"` | empty: 0.9 lies in Series' value-label column (`x > plotW`). The docs' generic example is now `series 0.5,0.5` | `hover.log` |
| 6 | `hover "series 0.5,0.5"`, `driftBars 0.97,0.5`, `""` | pass: "2026-09-01 · explicit 323 · total 2004"; "2026-W40 · …"; cleared | `hover.log` |
| 7 | After the hovers | `overlay` 0, every chart `paints` 1 (hover repaints nothing) | `hover.log` |
| 8 | Theme Tokyo Night via the bare `ssh <host> 'OMARCHY_PATH=/usr/share/omarchy omarchy theme set "Tokyo Night"'` | pass: ssh exit 0; a separate call read `tokyo-night`. The overlay stayed open and repainted each chart once (paints 2), `overlay` 0 | `tokyo-night-overlay.png` |
| 9 | Theme Catppuccin Latte, same form | pass: `catppuccin-latte`, paints 3, `overlay` 0 | `catppuccin-latte-overlay.png` |
| 10 | Theme Osaka Jade, same form (restore) | pass: `osaka-jade`, paints 4, `overlay` 0 | `osaka-jade-overlay.png` |
| 11 | Second fresh open: `shell hide`, then `shell summon jax.seldon '{"period":"365"}'`, `view` | pass: period 365, `overlay` 0, every chart `paints` 1 | `view-second-open-365.json` |
| 12 | Log of the live shell instance (`quickshell log --pid`) | clean: "Configuration Loaded" present, no WARN/ERROR line names jax.seldon | — |

**Screenshots.**
- Each one is `grim -o HDMI-A-2` at the output's real resolution, with the
  overlay open on 90 d.
- The top 28 px (the bar: workspace list, window title, the operator's
  media title) are cropped away, so each PNG is 1920×1052 at real pixel
  scale. Metadata stripped.

**Restore.**
- The overlay was closed, and the stand-in engine and
  `~/.local/state/seldon` removed (both were absent before). The service
  is back to `engineMissing`.
- The final plugin copy (comment and README change only, hot-reloaded)
  was followed by `settle`.
- Fingerprint after vs before (`~/.local/bin/seldon`,
  `~/.local/state/seldon`, `~/.config/seldon`, `~/Seldon-e2e` all absent;
  plugin enabled; theme `osaka-jade`; one shell instance; 4 crash reports;
  same `ls -A ~` hash; session unlocked): **identical except the plugin dir
  hash**, which is the fresh copy the WP asks for.
