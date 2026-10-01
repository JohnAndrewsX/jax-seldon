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

## WP-010 findings (2026-10-01, Omarchy 4.0.4-1, quickshell 0.3.1)

Verified in the shell source and live on the test host.

- **Service lookup from a third-party bar widget:** `root.bar.shell.serviceFor(<own id>)`.
  `bar.shell` is the scoped facade from `shell.pluginShellForId` (only the
  trusted built-in bar hands it out); it returns the plugin's own service and
  nothing else. `firstPartyServiceFor` (media) only serves an allowlist of
  first-party ids. The service can mount after the widget: poll until found,
  and hold it in a `property QtObject` so it drops back to null when the
  service is destroyed on a reload.
- **`bar.run(cmd)` is a shell string** (`Util.execDetached` → `bash -lc cmd`).
  Never use it with anything assembled. `Quickshell.execDetached([argv])`
  takes a list; `Util.execArgv(argv)` wraps a login shell around a constant
  `exec "$@"`.
- **Process that cannot start:** Quickshell logs `WARN: Process failed to
  start, likely because the binary could not be found. Command: …`, drops
  `running` and emits **neither `started` nor `exited`**. Detect it with a
  `started` flag checked in `onRunningChanged`.
- **qmllint vs `Process.onExited`:** `onExited: function(exitCode) {…}` gives
  a `signal-handler-parameters` warning (`QProcess::ExitStatus` is not
  resolvable; first-party `SystemUpdate.qml` has the same). A
  `Connections { target: proc; function onExited(exitCode, exitStatus) {} }`
  lints clean. Inline components that use outer ids from a `Repeater`
  delegate need `pragma ComponentBehavior: Bound`.
- **Hot reload does not load new code.** `finishPluginReload` calls
  `Qt.clearComponentCache` only `if (typeof … === "function")`, and it is
  `undefined` in Quickshell 0.3.1. Saving files re-creates the plugin from
  cached components. After changing plugin code: `omarchy-restart-shell`
  (works over ssh).
- **FileView:** `printErrors: false` keeps a missing file out of the log;
  `loadFailed(FileViewError.FileNotFound)` reports it. `watchChanges` follows
  an atomic temp+rename replace. Whether the watch also sees a file that did
  not exist at start is not verified: Service.qml polls every 5 s while the
  index is missing.
- **IPC to plugin targets:** `omarchy-shell <target> <method> [args]` reaches
  any `IpcHandler` target, e.g. `omarchy-shell jax.seldon.panel pill`.
  Non-interactive ssh has no `OMARCHY_PATH` and no `$OMARCHY_PATH/bin` on
  `PATH`; export both first.
- **Routing quirk:** `shell togglePanelAt <section> <n>` resolves the Nth bar
  panel to its *id* and calls `shell.toggle(id)`. For `jax.seldon` (has an
  overlay) that opens the Prime Radiant, not the bar panel. Tab between
  panels calls `slot.activeItem.open()` directly and does reach our panel;
  a nested Panel must pass the slot's widget (`barIdentity`) to
  `bar.switchPanelFrom`, as clock and weather do.
- **Environment:** the shell is spawned from Hyprland (`omarchy-launch-shell`,
  with `QS_DISABLE_FILE_WATCHER=1`) and inherits Hyprland's environment, so
  env overrides like `SELDON_INDEX` cannot reach it from a terminal. Logs:
  `quickshell log --pid <pid>` or `journalctl --user -t omarchy-shell`.
- **Log noise that is not ours:** every bar rebuild (enable, disable, reload)
  logs two `QObject::connect(QJSEngine, QtObject): invalid nullptr parameter`
  lines and "Handler was registered but will not be used" for other plugins'
  IPC targets. Both appear with jax.seldon disabled.
- **`omarchy pkg add` is official repositories only** (`pacman -S`); AUR
  packages need `omarchy pkg aur add` (`yay -S`, `--needed`, no upgrade).
  ADR-0004 and SPEC-PLUGIN §5 name the wrong command for `jax-seldon`.
- **Terminal for a fix:** `omarchy-launch-floating-terminal-with-presentation
  "<cmd>"` runs `<cmd>` through `bash -c` in a floating Omarchy terminal;
  pass constants only.
- **Headless harness:** `QT_QPA_PLATFORM=offscreen quickshell -p file.qml`
  runs non-visual QML (Service.qml) in a private instance. Files importing
  `qs.*` cannot load there: `qs` is the config root of the running shell.
