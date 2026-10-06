# WP-121 — Handover: the desk shell

Branch `wp/121-desk-shell` from `next` (94e1fab); merge into `next`.
Plan and the decisions the WP left open: [PLAN.md](PLAN.md).

## What was done

- **`plugin/Desk.qml`**, the manifest's `overlay` entry point: a layer-shell
  surface on the focused monitor with the desk centred in it, width from
  the new setting `deskWidth` (ADR-0034 §1 clamp, `Model.deskGeometry`);
  header (mark, "SELDON", machine · Omarchy · captured, status chip, KPI
  strip, Settings, Esc), the notices (the 0.1 panel's banners with their
  fixes), sidebar (nine targets with icons and counts, search, fold),
  one section per target, footer. `open(payload)`, `close()`, `opened`,
  `section(id)`, `select(id)`, `view()`; IPC through `shell call`.
- **`components/desk/`**: `DeskWindow` (the only layer-shell file, replaced
  by the harness), `Header`, `KpiStrip`, `Notices`, `Sidebar`, `NavIcon`
  (the prototype's icon paths via `QtQuick.Shapes`), `Search`, `Section`
  (the section base: the interface WP-122/123/125 build on), `ListColumn`,
  `ListRow`, `DetailPane` (sticky `ActionBar` outside the Flickable),
  `KeyValues`, `Arm` (arm twice, shared).
- **`sections/`**: Today … Graph as stubs ("Coming in WP-12x.");
  **Settings** real — Appearance live (slider 50–100 in 10 % steps,
  presets 50/67/75/Full, "N px on this screen", a screen picture, sidebar
  Open/Collapsed), Capture/Agents/Quiet read-only with where each value is
  set.
- **Settings write**: `Desk.writeSetting` → the facade's
  `updateEntryInline("jax.seldon", <every key of the entry + the change>)`,
  only on release/preset/click, never with a stored value; the written
  value is held until the shell's reload comes back (≤ 5 s); a refusal
  keeps the stored value and shows "The shell did not take the change.
  Change it in Omarchy's bar settings (Seldon widget)."
- **Manifest**: `overlay: Desk.qml`; `deskWidth` (integer 50–100, step 1,
  default 100) and `deskSidebar` (enum open/collapsed, default open) in
  `defaults` and `schema`; description with the approved tagline.
- **Service**: `deskWidth`, `deskSidebar`, `entrySettings`,
  `setDeskSettings(entry)`, `desk`, `deskMemory`; `snapshot()` reports the
  two settings. **BarWidget**: no popup; `pushSettings` passes the whole
  entry; left click toggles the desk, middle click summons section 7;
  `jax.seldon.panel` is the shim (open/close/show/hide/toggle/pill/view,
  `tab` → section, `resolve` → Changelog, `filter` → Changelog).
- **Removed**: `Panel.qml`, `Overlay.qml`, `components/overlay/
  OverlayWindow.qml`, `OverlayHeader.qml`, `components/Tabs.qml`. The
  charts stay for WP-123; the old tab components stay, unreferenced, for
  WP-122/123 to port and delete.
- **`Model.js`** (appended at the end, to keep the merge with WP-120
  small): `DESK_SECTIONS`, `deskSection*`, `deskCycle`, `clampDeskWidth`,
  `deskSidebarMode`, `deskGeometry`, `deskLayout`, `deskPayload`,
  `deskKpis`, `deskCounts`, `deskSubline`, `deskSettingsWrite`,
  `pickScreen`, `deskWidthPreview`, `deskPresetLabel`, `PANEL_TABS`; ten
  new unit tests in `model.test.js`.
- **Harness**: `tests/plugin/desk-view.sh` + `harness/desk.qml` +
  `harness/DeskWindow.qml` (old step vocabulary plus `section:`,
  `select:`, `width:`, `sidebar:`, `resize:`, `pill:`, `shim:`, `drag:`;
  a facade stand-in with the shell loader's semantics and a write log);
  `tests/plugin/COVERAGE.md` (every old scenario → successor or reason);
  `panel-view.sh`, `overlay-view.sh`, `harness/{panel,overlay}.qml`,
  `OverlayWindow.qml`, `KeyboardPanel.qml` deleted; `bar-view.sh` follows
  the pill to the shim; `service-states.sh` lost a stale stand-in copy;
  `justfile` `plugin-test` and the qmllint file list updated.
- **Docs**: SPEC-PLUGIN §1, §2, §5 (new, §5.1–§5.7), §6 (section 7), §8
  (routing, desk IPC, shim), §9, §10; KEYBINDINGS.md (the desk's keys,
  pill, IPC); TESTING.md (desk-view, the live layer-shell check);
  `plugin/README.md` security bullet (see "Security" below);
  `memory/omarchy-shell.md` WP-121 findings.

## The ADR's two assumptions, checked first

1. **Facade settings update** — the public name is
   **`shell.updateEntryInline(id, settings)`** on `services/
   PluginShellApi.qml` (`_updateSettings` underneath, scoped by
   `shell.qml createScopedPluginShell` to the caller's own id; an
   overlay-kind caller is allowed). It replaces the entry with `{ id } +
   settings` and returns `false` **both when refused and when nothing
   changed** — so the desk never sends a stored value and reads `false` as
   refusal only after a real change. The ADR's fallback (preview + sentence)
   is built in for the refusal case.
2. **Centring an unanchored layer-shell axis** — not relied on. The desk
   takes Omarchy's own menu pattern instead (all four edges anchored,
   transparent, card centred inside; the click-catcher is the same
   surface). `ExclusionMode.Normal` + `exclusiveZone: 0` keeps the bar
   visible and clickable. Verified live (below). This is the ADR's "what
   would change the how" direction (compute the geometry ourselves), taken
   deliberately; SPEC-PLUGIN §5.1 says so.

## How it was verified

- `omarchy plugin validate plugin/` — ok before every commit.
- `just qmllint` — ok, 0 warnings, 49 files (the list now includes
  `components/desk/` and `sections/`); token check ok.
- `node tests/plugin/model.test.js` — all pass (107).
- `bash tests/plugin/desk-view.sh` — 346 passed, 0 failed: widths at
  50/67/75/100 % on 1366/1920/2560/3840 px windows (expected width, centred
  within 1 px, at the gaps); icons under 960, stacked under 760, collapsed
  setting, solo sections; digits and `,` over the nine, Alt+↓ ×9 and Alt+↑
  wrap; Tab inert; `/` focuses the search, Esc leaves it (cleared), Enter
  keeps the filter and the next Esc clears it before the desk closes; a
  sidebar click takes the keys back from the search; `,` opens Settings;
  a slider drag writes nothing, its release **exactly one write carrying
  all keys** (an unknown key included); preset once, same preset not
  again; sidebar switch and fold button; refusal; toggle twice closes;
  pill left/middle; shim `tab work` → section 3, `tab bogus`, `view`,
  `resolve crisis`, `filter`; section remembered across a hide;
  `{"period":"30"}` → section 7; stacked Esc order; notices (snapper,
  not initialised + chip fold, restart notice + one launch, rules update
  live with doctor beside the queue and its failure, capture warnings +
  `c`); "N notices" chip on a narrow desk; every log free of warnings,
  `TypeError`s and binding loops; no text outside the window or the desk.
  `DESK_SHOTS` renders checked by eye in Tokyo Night, Kanagawa, Catppuccin
  Latte.
- `bash tests/plugin/bar-view.sh` — 194 passed; `service-states.sh` — 316
  passed; `real-home-guard.test.sh` — 11 passed.
- **Live smoke, dev host** (read-only dev mode, `SELDON_INDEX` = the sample;
  a private `quickshell -p <scratch root>` with the real `DeskWindow`, so
  neither the running shell nor `~/.config` was touched): `hyprctl layers`
  showed the `jax-seldon-desk` surface on the focused monitor (HDMI-A-2,
  2560×1440) at y = 26 (below the bar's reserved zone), 2560×1414;
  `view`: keys focused, 100 % → 2550 px, 50 % → 1275 px at x 642
  (centred), 67 % → 1709 px; one settings write. A visual capture was not
  possible: the session locked during the run (the capture shows the lock
  screen); the captures were deleted (they held private screen content).
- **Live smoke, test host** (`pbbau-lnx-ea`, authorised; same private
  instance under `~/.cache/seldon-smoke-wp121`, removed afterwards; the
  installed plugin untouched): 1920×1080 at scale 1.25 (1536×864 logical),
  surface at y = 26, 1536×838; 100 % → 1528 px, 50 % → the 960 px floor
  at x 288, 67 % → 1024 px; the screenshot shows the desk centred under
  the visible bar in the host's theme with the engine-missing notice and
  its three fixes (no engine on the ssh PATH). The chip cut its title
  there, fixed in 977a551.
- `flock /tmp/seldon-check.lock just check` — exit 0 (last section).

## What was not done (by design or left for later)

- Sections 1–8 are stubs; WP-122 (Today, Changelog, Work), WP-123
  (Decisions, System, Memory, Prime Radiant), WP-125 (Graph) fill them.
  Until WP-123 the Prime Radiant's charts are not reachable on `next`,
  and `shell call jax.seldon setPeriod|hover` answer "unknown"; until
  WP-122 the 0.1 panel's actions (note, drift sheet, plan, agent start)
  are not reachable. `next` should not go to the test host as the user's
  plugin before WP-122/123 are in.
- `COVERAGE.md` rows marked *open* are WP-122/123's to port.
- `plugin/README.md` (beyond the security bullet), the user guides, the
  CHANGELOG **Breaking** entry (popup and overlay replaced, keyboard map,
  IPC shim and its removal in 0.3.0) and `preview.png`: WP-126.
- The font scale 1.25 sweep of the desk (the old fit scenario 23): WP-126.
- WP-123's output list still says "Deleted: `OverlayHeader.qml`" — it is
  already gone here (WP-121's output list said so too).

## Security (for stage 2)

- **The first `shell.json` write**: only `Desk.writeSetting`, only through
  the injected facade's `updateEntryInline` with the plugin's own id (the
  facade enforces the scope), only on an explicit release/preset/click,
  never with an unchanged value, never while dragging. The payload is the
  plugin's own entry (all its keys + the change), so nothing of the
  user's other settings is touched; the shell then emits one
  `config-change` per write (routine since ADR-0028). The README's
  "Writes nothing itself" was corrected to say this.
- No new process, no new command, no shell string: the desk runs nothing
  the old panel did not run (the notices call the same `Service.fix`
  ids); the shim only summons/hides through the facade.
- The desk takes exclusive keyboard focus only while visible.

## Open questions

None blocking. For the orchestrator: WP-120 changes `Model.js` in
parallel; my additions are appended at the end of the file and touch
nothing above it except the header comment's file name.

## Final check

`flock /tmp/seldon-check.lock just check` on 8197878: **exit 0** (`check:
ok`; docs-check ok, qmllint ok 49 files, service-states 316, desk-view
346, bar-view 194, plugin-test ok). HANDOVER.md is the only change after
that commit.
