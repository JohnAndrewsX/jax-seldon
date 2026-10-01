# WP-013 live session on the test host — results

2026-10-01, 22:54–23:30 CEST.
- Branch `wp/013-integration` rebased onto `main` `f54e382`.
- Omarchy 4.0.4-1, quickshell 0.3.1, output 1920×1080 at scale 1.25
  (1536×864 logical), theme Osaka Jade.
- The session was unlocked throughout (`omarchy-shell lock status` before
  every key and screenshot).

**Setup:**
- the static musl engine in `~/.local/bin/seldon`;
- `seldon init --non-interactive --path ~/Seldon-e2e --no-capture`, then
  `fixtures/logbook/` copied over it, with `created` set to now. The
  logbook's own machine id (`workstation-7f3a`) keeps the real host name
  out of the screenshots;
- `[agent] launcher = ["/usr/bin/true", "{prompt}"]` in the scratch
  `config.toml`, so Start agent spawns nothing real;
- `plugin/` from `main` rsynced and validated, then a shell restart after
  the reloads had settled.

The index after `seldon status` equals the sample's summary: 2 active,
3 queued, 4 open drift, 2 crises.

**Logs in this folder:**
- `e2e-engine-only.log`, `e2e-full-run1.log`, `e2e-full-run2.log`;
- `keys-*.log`, `overlay.log`, `themes.log`, `shell-log.txt`,
  `restore.txt`;
- `INCIDENT.md`.

**Screenshots:** `screenshots/osaka-jade-*.png`.

| # | Step | WP | Result | Note |
|---|---|---|---|---|
| 1 | `just e2e --engine-only` after the rebase | 013 | pass (24/24) | `init` runs the first capture now (WP-024); the script backfills through `init --since` |
| 2 | `just e2e` full run 1 | 013 | pass (47/47) | host restored; crash reports unchanged (4) |
| 3 | `just e2e` full run 2, right after | 013 | pass (47/47) | same |
| 4 | Panel opens: status `ok`, pill `⟡ 2 · 4`, urgent, strip "2 changes in the red zone need a reason", snapper banner | 010–012 | pass | pill = index summary |
| 5 | Digits 1–6 select today, changelog, work, decisions, system, memory | 011/020/023 | pass | `keys-tabs.log` |
| 6 | `l` wraps memory → today, `h` today → memory; ←/→ switch tabs | 011 | pass | |
| 7 | First ↓ only shows the cursor, the second moves it | 011 | pass | |
| 8 | `n` focuses the QuickEntry; a note starting with `--help` and quotes is saved verbatim; Esc gives the keys back | 012 | pass | ledger `detail` byte-exact |
| 9 | `c` Capture now | 012 | pass | "nothing new · failing: snapper" |
| 10 | `e` Open in editor (Today) | 012 | pass | nvim stays open after 16 s: WP-012 Decision 1 (killed after 10 s) is fixed on main |
| 11 | `+` new-case sheet, title, Enter → `Created C-2026-009` | 020 | pass | |
| 12 | Start → verify → done by Enter twice (arming hint "Press Enter again: …") | 020 | pass | the file moves `queued → active → completed`; the columns follow without a restart |
| 13 | `x` on a queued case | 020 | pass (no-op) | queued has no Drop (`ACTIONS_BY_STATUS`) |
| 14 | `x x` on an active case → dropped; a cursor move disarms | 020 | pass | |
| 15 | `a a` Start agent on an active case | 022 | pass | "Agent started on C-2026-003 · launcher default (/usr/bin/true)"; `.seldon/active-case` written. The real default launcher was deliberately not run |
| 16 | `d`, title, Enter twice → `Created ADR-0005`, `status: proposed`, editor opens | 023 | pass | |
| 17 | Memory: Enter opens the logbook folder | 023 | pass | |
| 18 | Drift sheet, Explain on the red crisis (Enter twice) → case C-2026-010 created and completed, `resolution` line with `refersTo` | 021 | pass | pill `⟡ 1 · 3`, strip down to 1 |
| 19 | Drift sheet, Link `tokyo-night` → C-2026-005 (the proposed case preselected) | 021 | pass | |
| 20 | Drift sheet, Dismiss the firefox group (Backtab×2, →, Enter, Tab×2, reason, Enter twice) → "Dismissed 3 events" | 021 | pass | the badge reads `+2` (members − 1), as settled |
| 21 | Drift sheet, Dismiss by a pointer click on the button | 021 | **fail (my driving)** | the click closed the panel and the typed text went to a terminal: `INCIDENT.md` |
| 22 | Overlay `shell toggle jax.seldon`: wide mode, 6 slots, every `chart.paints` = 1, `paintMs` 0–2 | 030/031 | pass | |
| 23 | Overlay `aggregations.overlay` on a fresh open | 030 | **mismatch** | live 23; the headless harness gives 0 on the same index (FINDINGS §5.1) |
| 24 | `call hover "heatmap 0.9,0.5"` | 031 | empty (as headless) | the 13-week grid fills only the left ~15 % of the slot; inside it the probe answers (FINDINGS §5.2) |
| 25 | `call hover` on series, driftBars, riskDonut, plan; `""` clears; a malformed argument → `{error}` | 031 | pass | |
| 26 | `setPeriod 30`; keys 1–4; ←/→ and h/l wrap | 030 | pass | windowed charts repaint once per change; riskDonut and plan never |
| 27 | Real pointer hover (`ydotool`) on heatmap, series and plan | 031 | pass | hovering repaints nothing (paints stay 1) |
| 28 | Esc closes the overlay | 030 | pass | |
| 29 | Click on the scrim closes the overlay | 030 | pass | |
| 30 | Middle click on the pill opens the overlay | 030 | pass | |
| 31 | Tab / Shift-Tab hand over to the next bar panel | 011 | pass | OmaSettings / Navbar Cat open; ours stays open (known); closed again |
| 32 | Screenshots, Osaka Jade (dark) | 020–031 | pass | 11 PNGs |
| 33 | Screenshots, Tokyo Night and Catppuccin Latte | 020–031 | **not done** | the guard blocked `omarchy theme set` (my command had `; echo` after the ssh); reported, not retried |
| 34 | Shell log of the live instance (pid 1957859) | all | pass | "Configuration Loaded" present; no WARN or ERROR naming jax.seldon |
| 35 | No crash during the session | all | pass | crash reports 4 before and after (the 4th dates from 15:13, before this session) |
| 36 | Restore | 013 | pass | `restore.txt`: Seldon paths absent, home listing unchanged, theme Osaka Jade, service `engineMissing`, plugin folder = fresh copy of `main` |
| — | WP-040 `makepkg` on the test host | 040 | not run | package build; the guard blocks it (needs an operator exception) |
| — | WP-024 theme hook (`omarchy hook install`) | 024 | not run | the guard blocks it |
| — | WP-041 live grab for `preview.png` | 041 | partly | `screenshots/` has the pill and the panels; `plugin/preview.png` is not mine to change |
