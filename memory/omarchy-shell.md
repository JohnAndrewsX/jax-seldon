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
