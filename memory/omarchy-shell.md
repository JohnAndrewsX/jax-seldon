# Omarchy shell findings (QML API, plugin contract)

Source of truth: `/usr/share/omarchy/shell/README.md`, `shell/plugins/README.md`,
`shell/services/PluginRegistry.qml`, `bin/omarchy-plugin-validate`. Verified
on Omarchy 4.0.4-1, quickshell 0.3.1. Workers append; the Reviewer prunes.

## Manifest contract (what the shell and `omarchy plugin validate` enforce)

- `schemaVersion` must be the JSON **number** `1`.
- Required: `id`, `name`, `version`, `kinds`, `entryPoints`.
- `id`: `^[A-Za-z0-9][A-Za-z0-9._-]*$`, no `..`, no `/`, not `omarchy.*`.
- `kinds`: non-empty array of `bar-widget | panel | overlay | menu | service | bar`.
- `entryPoints`: object; each value a relative path without `..` that
  **exists**; every declared kind needs its key:
  `bar→bar`, `bar-widget→barWidget`, `menu→menu`, `overlay→overlay`,
  `panel→panel`, `service→service`.
- `barWidget.defaultSection` ∈ `left|center|right` if present.
- No symlinks anywhere in the plugin folder (`.git` is skipped).
- **Unknown top-level keys are accepted** (`validateManifest` does not reject
  them) → the SPEC-PLUGIN `seldon: { contractVersion, engineMin }` block is
  fine. `omarchy` is a reserved metadata key (capabilities, `clonedFrom`).
- Optional keys seen in first-party manifests: `author`, `license`,
  `description`, `activation: "on-demand"` (omarchy.agents), `keepLoaded`,
  `barWidget.{displayName, description, category, aliases, allowMultiple,
  defaultSection, defaults, schema[]}`. `schema[]` items:
  `{ key, type: string|integer|enum|path|..., label, min, max, step,
  options, defaultValue, description }` → rendered in Setup > Plugins.
- Install path: `~/.config/omarchy/plugins/<id>/`. Saving any file under it
  hot-reloads plugin code; `keepLoaded` services need `omarchy-restart-shell`.
- Third-party plugins land **disabled** after `omarchy plugin add`; use
  `--enable --yes` for scripts.

## Loading and IPC

- `omarchy-shell shell summon|hide|toggle <id> '<json>'`, `call <id> <method>
  <arg>`, `listPlugins`, `rescanPlugins`, `reloadConfig`, `ping`.
- Bar widgets must expose `open()`, `close()`, `opened` on the widget root
  for `shell.summon/hide/toggle` routing (`Bar.findPanelWidget`), see the
  clock widget comment.
- **Open question for WP-010/WP-030:** with several kinds under one id
  (`service` + `bar-widget` + `overlay`), which entry does
  `toggle jax.seldon` hit? Check how `omarchy.menu` (menu + bar-widget) and
  `omarchy.media` (service + bar-widget) route, and whether `call jax.seldon
  openOverlay ""` is the cleaner path for the Prime Radiant.
- Third-party entry points get capability-scoped facades, not host objects.
  Entry points may declare `omarchyPath`, `shell`, `manifest`,
  `pluginRegistry`, `barWidgetRegistry` properties for host injection.

## Patterns to copy

Bar widget (`shell/plugins/panels/clock/BarWidget.qml`):

```qml
import QtQuick
import Quickshell
import Quickshell.Io
import qs.Commons
import qs.Ui
import "Model.js" as Model

BarWidget {
  id: root
  moduleName: "omarchy.clock"
  // settings from the inline shell.json entry:
  readonly property string fmt: setting("format", "dddd HH:mm")
  readonly property bool opened: panelLoader.item ? panelLoader.item.opened === true : false
  function open() {...}  function close() {...}  function togglePanel() {...}
  // write a setting back:
  // root.bar.shell.updateEntryInline(root.moduleName, entry)
}
```

Panel (`shell/plugins/agents/Panel.qml`): `Panel { moduleName; ipcTarget;
manageIpc: false }`, colours from `bar.foreground`, `bar.urgent`,
`Color.accent`, `Color.popups.background`, fills from
`Style.selectedFillFor(fg, accent)`, font `bar.fontFamily` /
`Style.font.family`, `Process` + `Quickshell.Io` for CLI calls.

## qs.Ui and qs.Commons inventory

- Ui: `BarWidget`, `WidgetButton` (text, foreground, activeColor, active,
  tooltipText, `signal pressed(int button)`, `signal wheelMoved(int delta)`),
  `KeyboardPanel` (anchorItem, bar, open, contentWidth/Height via
  `Style.space(n)`, padding, margin, centerOnBar), `Panel`, `PanelController`,
  `PanelHero`, `PanelSectionHeader`, `PanelSeparator`, `PanelActionButton`,
  `PanelSlider`, `PanelToolTip`, `PanelKeyCatcher`, `PopupCard`, `Button`,
  `ButtonGroup`, `Dropdown`, `SearchableDropdown`, `MultiSelect`,
  `TextField`, `NumberField`, `Toggle`, `ToggleSwitch`, `ConfirmDialog`,
  `BarIconButton`, `BarIndicator`, `OpticalGlyph`, `BorderOverlay`,
  `BorderSurface`, `CursorSurface`, `PointerMoveGate`, `ScreenMoveRemap`.
- Commons: `Style` (`space(n)`, `gapsOut`, `cornerRadius`, `spacing.*`,
  `font.family/body`, state fill/border helpers), `Color` (`foreground`,
  `accent`, `urgent`, `popups.background/border`), `Util` (`alpha()`),
  `Border`, `BorderGeometry.js`.
- The theme's "error colour" in SPEC-PLUGIN is the shell token **`urgent`**.
- `omarchy dev ui preview [section]` opens the Ui kit gallery.
- Lint: `/usr/lib/qt6/bin/qmllint -I /usr/share/omarchy/shell <files>`
  (dev host; `/usr/bin/qmllint` on the test host). The justfile must resolve
  the path.

## WP-001 findings (2026-10-01, Omarchy 4.0.4-1)

- **Answer to the routing question above** (`shell.qml`
  `isBarWidgetPanelPlugin`, `summon`, `computePanelEntries`): a plugin whose
  `kinds` contain `bar-widget` **and** any of `panel|overlay|menu` is owned by
  the panel loader, not the bar. So `omarchy-shell shell summon|hide|toggle
  jax.seldon` goes to **Overlay.qml**; the bar widget's `open/close/opened`
  are never reached through `shell.*` for `jax.seldon`. The loader picks one
  UI kind per id, in the order `panel > overlay > menu`, so declaring `panel`
  as well would hide the overlay from `summon`. Opening the bar widget's
  panel by IPC needs its own route (an `IpcHandler` in the widget, or
  `call jax.seldon <method>`); WP-010/WP-030 decide.
- **Overlay loader contract:** async `Loader`, active while open (or always
  with `keepLoaded`). After load it injects `omarchyPath`, `shell`,
  `manifest`, `barWidgetRegistry`, `pluginRegistry` and **`service`** (=
  `shell.serviceFor(id)`, the plugin's own Service.qml instance) if the root
  declares them. `summon` calls `open(payloadJson)` once per queued payload;
  `hide` calls `close()`; `toggle` reads `item.opened`. Self-dismiss pattern
  (emojis): `shell.hide(manifest.id)`.
- **Overlay window pattern** (`plugins/emojis/Emojis.qml`): root `Item`,
  `PanelWindow { visible: root.opened; anchors {top;bottom;left;right: true};
  WlrLayershell.layer: WlrLayer.Overlay; WlrLayershell.keyboardFocus:
  WlrKeyboardFocus.Exclusive; exclusionMode: ExclusionMode.Ignore }`, scrim
  `Color.menu.scrim`, card `BorderSurface` with
  `Border.surfaceSpec(section, key, color, width)`.
- **Third-party services** are created with **no parent**
  (`createObject(null)`); only first-party ones go under `serviceHost`. An
  `Item` root works. Same injection list as above, minus `service`.
- A root `Item` must not declare `property string state` (clashes with
  `Item.state`, qmllint `property-override`); Service.qml uses `status`.
- **qmllint:**
  - Exits 0 on warnings by default; use `--max-warnings 0` to gate.
  - `-I $OMARCHY_PATH/shell` alone does **not** resolve `qs.Ui` /
    `qs.Commons`: Quickshell serves the shell root as prefix `qs`, but there
    is no `qs/` dir and no `.qmlls.ini` in the package install. Fix used in
    the justfile: a temp import root with `qs/<Module>/qmldir` files whose
    entries are **relative** paths back into the shell (absolute paths in
    qmldir entries are rejected; qmllint joins them onto the qmldir's dir).
  - Unavoidable even for first-party plugins: `missing-property` on nested
    tokens (`Style.font.body`, `Color.popups.text`: declared as `QtObject`
    properties, so typed `QObject`) and `uncreatable-type` on `PanelWindow`
    (`Quickshell/_Window/quickshell-window.qmltypes` marks it
    `isCreatable: false`). The justfile demotes exactly these two to info.
- `bin/omarchy-plugin-validate` is plain bash + jq (118 lines) and prints
  nothing on success. It would run in CI if fetched from a pinned Omarchy
  tag; today CI skips it.
- SPEC-PLUGIN §1 points at `~/.local/share/omarchy/shell/README.md`; on a
  package install the README is at `$OMARCHY_PATH/shell/README.md`
  (`/usr/share/omarchy`).
