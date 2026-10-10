# WP-121 — Plan: the desk shell

Branch `wp/121-desk-shell` from `next` (94e1fab), merges into `next`.

## What I verified first (ADR-0034 "Assumptions" and "What would change the how")

1. **The facade's settings update.** Third-party UI entries receive
   `services/PluginShellApi.qml` as `shell`. Its public method is
   **`updateEntryInline(id, settings)`** (backed by `_updateSettings`, which
   `shell.qml createScopedPluginShell` closes over the plugin id: allowed
   when `pluginOwnsTarget(key, id)`, i.e. our own id, for every kind,
   overlay included). It calls the host's `updateEntryInline`, which
   replaces the entry (bar layout first, else `plugins[]`) with
   `{ id } + settings` and returns **`false` both when refused and when
   nothing changed** (`dirty` false). The desk therefore never calls it with
   unchanged values, and treats `false` (or a missing method) as refused.
   The pill's `bar.shell` is the same kind of facade (`pluginShellForId`).
2. **Centring an unanchored layer-shell axis.** The layer-shell protocol
   centres a surface on an axis without both anchors, and Hyprland follows
   it — but the desk does not depend on it. Omarchy's own menu
   (`plugins/menu/Menu.qml`) anchors its `PanelWindow` on all four edges,
   transparent, and centres the card inside; the desk does the same. That
   also gives the ADR's outside click-catcher for free (a `MouseArea` in the
   same surface) and makes the width a plain item width the harness can
   measure. `ExclusionMode.Normal` + `exclusiveZone: 0` keeps the bar's
   zone free, so the bar stays visible and its pill clickable. This is the
   ADR's fallback direction ("compute the geometry ourselves"), taken
   deliberately; checked live in the smoke test.
3. **Loader lifecycle.** The overlay entry is created bare, then `shell`,
   `manifest`, `service` are injected; `hide` calls `close()` and unloads
   the item (no `keepLoaded`), so the desk keeps its section and selection
   in the service (`Service.deskMemory`) between opens.
4. **Focused monitor.** `Hyprland.focusedMonitor.name` as in
   `plugins/bar/Bar.qml focusedScreenName()`.

## Build

- `Desk.qml` (entry, keys, IPC methods, `view`), `components/desk/`
  `DeskWindow` (layer shell; harness stand-in), `Header`, `KpiStrip`,
  `Sidebar` (+ `NavIcon`), `ListColumn`, `DetailPane`, `ActionBar`,
  `Search`, `Notices` (today's banners under the header), `Arm` (arm-twice).
- `sections/{Today,Changelog,Work,Decisions,System,Memory,Radiant,Graph}.qml`
  stubs on a shared `SectionStub.qml`; `sections/Settings.qml` real.
- `Model.js`: `DESK_SECTIONS`, `deskGeometry`, `deskLayout`, `deskPayload`,
  `deskKpis`, `deskCounts`, `deskSettingsWrite`, `pickScreen`, unit tests.
- `Service.qml`: `deskWidth`, `deskSidebar`, `entrySettings`,
  `setDeskSettings()`, `desk`, `deskMemory`. `BarWidget.qml`: no popup;
  left click toggles, middle click summons section 7, shim.
- Removed: `Panel.qml`, `Overlay.qml`, `OverlayWindow.qml`,
  `OverlayHeader.qml`, `Tabs.qml`. Old tab components stay unreferenced
  until WP-122/123 port and delete them.
- Harness: `tests/plugin/desk-view.sh`, `harness/desk.qml`,
  `harness/DeskWindow.qml`; `COVERAGE.md`; `bar-view.sh` adapted;
  `panel-view.sh`, `overlay-view.sh` and their harness files deleted.
- Docs: SPEC-PLUGIN §1, §2, §5 (new), §6, §8; KEYBINDINGS.md.

## Decisions (left open by the WP)

1. **Window**: all four edges anchored, transparent, the desk an item
   centred in it (verified point 2). Width base = the window's width (the
   screen minus vertical bars), margins `Style.gapsOut`:
   `avail = W − 2·gapsOut`, `width = clamp(round(avail·pct/100),
   min(960, avail), avail)`, `x = (W − width)/2`.
2. **Thresholds**: desk width < 960 → sidebar icons-only regardless of
   `deskSidebar`; < 760 → stacked (list or detail, a "‹ Back to the list"
   row). Sidebar 210 / 56 (`Style.space`), list column
   `clamp(round(0.3·rest), 260, 360)`.
3. **What "all keys" means**: the write carries every key of the current
   entry (unknown keys included) plus the changed one; defaults the user
   never set are not materialised. Unchanged value → no call.
4. **The sidebar's collapse button** writes `deskSidebar` (explicit user
   action, one write per click). The Appearance page has the same switch.
5. **Slider**: 50–100 in steps of 10 while dragging (presets 67/75 are
   reachable by click and keep their value); the desk previews the live
   value while dragging; one write on release. Until the shell's
   `shell.json` reload arrives back through the pill, the desk holds the
   written value (no flicker). Refused → the stored value returns and the
   sentence "Change it in Omarchy's bar settings (Seldon widget)." shows.
6. **Search**: `/` focuses; Enter leaves the field and keeps the filter;
   Esc in the field clears it and leaves. Sections receive `searchText`.
7. **Letters** go to the current section's `textKey` first; unhandled,
   the desk falls back to `c` capture, `n` → Today, `+` → Work (the key is
   then offered to that section). Tab/Shift-Tab do nothing.
8. **Notices**: today's banners (restart, status, snapper, rules, rules
   result, capture warnings) sit in one strip under the header, each with
   its fixes, as today; the header carries a status chip with the first
   one's title and the count. Click on the chip folds the strip.
9. **Section icons**: the prototype's 24-unit paths drawn with
   `QtQuick.Shapes` in the theme colour (no font glyph dependence).
10. **Old IPC `filter`** on the shim: opens the Changelog with the filter
    in the payload; the section takes it from WP-122 on.
