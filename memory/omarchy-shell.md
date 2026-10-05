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
  (Corrected in WP-011, below: `qs` is the config root of *that* instance.)

## WP-011 findings (2026-10-01, Omarchy 4.0.4-1, quickshell 0.3.1)

- **`qs.*` in a private instance:** Quickshell serves `qs.<Dir>` from the
  config root of the instance it runs, so `quickshell -p <root>/shell.qml`
  resolves `import qs.Commons` to `<root>/Commons/` (no qmldir needed;
  `pragma Singleton` files work). Copies of `$OMARCHY_PATH/shell/Commons`
  and `shell/Ui` in a temp root run offscreen; only `Ui/KeyboardPanel.qml`
  (a layer-shell `PanelWindow`) must be replaced. `Style.qml` runs `hyprctl
  getoption` and `fc-match` at load and keeps its defaults when they fail.
- **Real key events offscreen:** `import QtTest` and a `TestCase { when:
  false; running: false }` in any Quickshell config; `keyClick(Qt.Key_Tab)`,
  `keyClick(Qt.Key_Tab, Qt.ShiftModifier)`, `keyClick("f")` reach the item
  with active focus in a `QtQuick.Window` on the offscreen platform.
- **ListView `header` scrolls away** when the model is replaced (new index):
  the view repositions on the first delegate and the header ends up above
  the viewport. Keep headings outside the ListView (seen live, Today tab).
- **`positionViewAtIndex(i, ListView.Contain)` right after a row grows**
  (Qt.callLater) can run before the delegate's relayout; re-contain from
  the delegate's `onHeightChanged` instead.
- **Column skips invisible children**; binding `height: visible ?
  implicitHeight : 0` on a `PanelSectionHeader` inside a ListView delegate
  Column gave a binding loop. Drop the height binding.
- **QML lists are not JS arrays:** `item.children.indexOf` is a TypeError;
  index with a for loop.
- **Tab handover:** `Bar.switchPanelFrom(owner, dir)` calls `open()` on the
  neighbouring slot's widget and returns true. The popout coordinator closes
  our KeyboardPanel only when the neighbour opens a KeyboardPanel too; a
  neighbour that opens its own window (OmaSettings on the test host) leaves
  ours open.
- **`omarchy theme set`** restarts the shell (new pid, about 5 s); check
  logs of every instance under `/run/user/<uid>/quickshell/by-pid/<pid>/`
  (`quickshell log <path>/log.qslog`).
- **grim regions are logical pixels.** The test host's output is scaled 1.25:
  divide coordinates read off a full screenshot by the scale. Over ssh grim
  needs `XDG_RUNTIME_DIR=/run/user/<uid>` and `WAYLAND_DISPLAY=wayland-1`;
  `hyprctl` also needs `HYPRLAND_INSTANCE_SIGNATURE` (name of the dir in
  `/run/user/<uid>/hypr/`).
- **An index that appears after shell start** is picked up by Service.qml's
  5 s missing-file poll (verified live; the FileView watch alone was not
  tested for this case).
- **Colour tokens:** `Color.muted` exists next to `foreground`, `background`,
  `accent`, `urgent`. There is no yellow/warning token; zone yellow maps to
  `accent`. `Util.alpha(fg, a)` dims text in light and dark themes alike;
  `Qt.darker(fg)` makes a light theme's dark text darker, not dimmer.
- **Nerd Font glyphs** in the bar font (JetBrainsMono Nerd Font): verify a
  codepoint with `fc-list ":charset=f03d7" family`. `⟡` (U+27E1) is not in
  it and comes from fontconfig fallback.

## WP-012 findings (2026-10-01, Omarchy 4.0.4-1, quickshell 0.3.1)

- **A TextField inside `PanelKeyCatcher`:** set `blocked` while the field
  has focus, or h/j/k/l, digits and `c` drive the panel while typing. Esc
  must hand focus back to the catcher (`forceActiveFocus()`), as the
  weather panel does. Verified live with `wtype` and offscreen with QtTest.
- **`qs.Ui` Dropdown focus:** the focus item is its inner trigger, not the
  Dropdown root (an Item), so `dropdown.activeFocus` stays false. Wrap the
  field and the Dropdown in a `FocusScope`, whose `activeFocus` covers
  both; add `popupOpen` (the popup lives in the window overlay, outside
  the scope). Keys: Tab from the TextField reaches the trigger
  (`activeFocusOnTab`), ↓ opens, ↑/↓ move, Enter picks. The popup renders
  inside the layer-shell `KeyboardPanel` on the test host and in the
  offscreen harness alike. Once the user picks, `value` is assigned inside
  the Dropdown and a binding on it is gone; set `value` imperatively to
  reset it.
- **Spinner:** `qs.Ui` Button has `iconText` and `iconSpinning` (a
  RotationAnimation); `"󰦖"` is the glyph first-party panels spin.
- **A replaced ListView model starts at the top:** keep the cursor row in
  view with `onCountChanged: Qt.callLater(keepCurrentVisible)`. The
  scrolled delegates exist only after the next layout pass, so a harness
  report in the same tick does not see them yet.
- **`omarchy-launch-editor` does not return for a terminal editor** (nvim,
  the default): it execs `omarchy-launch-tui` → `exec setsid uwsm-app --
  xdg-terminal-exec …`. A caller that is not a process-group leader is not
  forked by `setsid`, so the launcher process lives as long as the editor.
  A caller that waits with a timeout and then kills the child kills the
  editor window (seen live: `seldon open --editor` from the plugin, 10 s).
  Spawn it detached and do not wait for it.
- **`wtype` into a panel TextField** over ssh works once the panel has
  keyboard focus (`jax.seldon.panel open`, then `wtype n`); `wtype --
  "<text>"` for text starting with `-`.

## WP-020 findings (2026-10-01, Omarchy 4.0.4-1, quickshell 0.3.1)

- **Check the lock before typing over ssh.** `omarchy-shell lock status`
  (`secure: true`) means the lock screen has the keyboard: `wtype` goes to
  its password field, not to the panel, and every Return is a failed unlock
  attempt that pam_faillock counts (`faillock --user $USER`; deny = 10 on
  the test host). `jax.seldon.panel view` and the service IPC still work
  while locked; `grim` hangs (screencopy refused), so give it a `timeout`.
- **`omarchy-restart-shell` and the lock:** with a secure locker it
  refuses ("Refusing to restart … while the session is locked", exit 1);
  with a compositor lock but no secure locker it restarts and re-locks.
  After one restart a second `/usr/bin/quickshell` without arguments
  (parent `systemd --user`, no `by-pid` dir) stayed running next to the
  shell; origin unknown, left alone.
- **Inline components have their own id scope:** a `component X: Item {}`
  cannot read the enclosing file's ids (`root.…`); pass values in as
  properties.
- **`visible: childA.visible || childB.visible` on a parent never turns
  true:** a hidden parent makes its children's `visible` false. Bind to the
  data the children use instead.
- **Nested delegates:** when a new model replaces an outer Repeater's
  items, an inner ListView delegate can evaluate its bindings after the
  outer delegate's id is already null (`TypeError: Cannot read property
  … of null`); guard with `!!outer && …`.
- **qs.Ui `ButtonGroup`** is one Tab stop; ←/→ or h/l walk the chips,
  Enter/Space emit `changed(value)`. It does not assign `value` itself, so a
  binding on `value` survives. **`Button { focusable: true }`** becomes a
  Tab stop that Enter/Space click.
- **Offscreen theme renders:** `Color.qml` reads
  `$HOME/.local/state/omarchy/current/theme/colors.toml`; copying a theme's
  `colors.toml` from `$OMARCHY_PATH/themes/<theme>/` into the harness HOME
  renders the panel in that theme without touching the system.
  `grabToImage` on `Window.contentItem` leaves out the window colour; put a
  `Rectangle { color: Color.background }` under the content.

## WP-021 findings (2026-10-01, Omarchy 4.0.4-1, quickshell 0.3.1)

- **Locked test host, new plugin code:** while `omarchy-shell lock status`
  says `secure: true`, `omarchy-restart-shell` refuses, so the running shell
  keeps the plugin code it loaded last; IPC (`view`) then shows the old
  code. To run the *new* code with the real engine anyway, start a private
  offscreen Quickshell on the test host (the panel harness: copies of
  `$OMARCHY_PATH/shell/Commons` and `Ui`, `QT_QPA_PLATFORM=offscreen`,
  `env -i`, own `HOME` and `XDG_RUNTIME_DIR`, the real `seldon` first on
  `PATH`); it never touches the running shell and needs no keys on the
  real seat.
- **Deterministic drift on a real engine:** `seldon init --path X`, copy
  `fixtures/logbook/` over X (no `--delete`, init's `.seldon/templates`
  stay), set `created` in `X/.seldon/logbook.toml` to now, `seldon status`:
  the sample's four drift items, and capture baselines instead of
  importing the host's package history.
- **qs.Ui `Button { focusable: true }` clicks on Enter/Space** (its own
  `Keys` handlers). For a two-press (arm, then run) key path, wrap a
  non-focusable Button in an `Item { activeFocusOnTab: true;
  Keys.onReturnPressed: … }` and show the cursor with `hasCursor`.
- **A row-wide MouseArea declared last swallows the clicks of Buttons
  inside the row.** Declare it first (under the content); Texts let clicks
  through to it, Buttons keep their own.
- **Hiding a tab does not reliably take `activeFocus` from a FocusScope
  inside it** (a programmatic tab switch left the hidden sheet "editing",
  so the panel's key catcher stayed blocked). Hand the keys back in
  `onVisibleChanged`.
- **The FileView can deliver the rewritten index before the Process that
  wrote it has exited** (the engine rebuilds the index before it prints).
  UI that depends on both the result and the new index must handle either
  order (e.g. move focus when the later of the two arrives).
- **Harness keys: a non-ASCII `keyClick` ("ü") under `env -i` (locale C)
  crashed Quickshell.** Type ASCII in scenarios, or give the run a UTF-8
  locale.

## WP-023 findings (2026-10-01, Omarchy 4.0.4-1, quickshell 0.3.1)

- **Six tabs in `Style.space(380)`:** six bordered `qs.Ui` Buttons with
  `Style.font.bodySmall` leave "Changelog"/"Decisions" touching their
  borders in the fallback monospace font; `Style.font.caption` fits
  (Tabs.qml takes `fontSize`, Panel.qml passes caption).
- **`omarchy-launch-editor <dir>`** (the target of `seldon open logbook
  --editor`) passes the folder to the editor unchanged: terminal editors
  (nvim, the default) run in `omarchy-launch-tui` and show their directory
  view; GUI editors get it through `uwsm-app --`. Read from
  `$OMARCHY_PATH/bin/omarchy-launch-editor`; not tried on a live seat
  (the test host stayed locked).
- **A row MouseArea with `onClicked` and `onDoubleClicked`** declared first
  inside a `CursorSurface` selects on the first click and opens on the
  double click, and a `Button` declared after it keeps its own clicks
  (confirms the WP-021 finding).
- **Harness `click:<text>`** finds the first *visible* item in tree order,
  so a button label shared by several tabs ("Open") hits the one on the
  current tab; hidden tabs are skipped.
- **Harness `shows`:** the text is spliced into a jq string literal; a `"`
  in it must be written `\"`.
- **Real-engine smoke on a locked test host (repeatable):** the WP-021
  recipe (private offscreen Quickshell, `env -i`, temp `HOME` with its own
  `.gitconfig` so the engine's autocommits work, the musl `seldon` and a
  recorder named `omarchy-launch-editor` on `PATH`) runs `decide` and
  `open --editor` end to end without a window on the seat; the recorder
  receives the detached launcher's argv because the engine's child
  inherits the harness environment.

## WP-030 findings (2026-10-01, Omarchy 4.0.4-1, quickshell 0.3.1)

- **The overlay item lives only while open.** `shell.hide(id)` calls
  `close()` and then drops the id from `openPanelIds`, which deactivates
  the async Loader: the Overlay.qml instance is destroyed on every hide and
  created anew on the next summon/toggle (unless the manifest sets
  `keepLoaded`). State that must survive a close (the period) or work
  that should not run on open (aggregation) belongs in the service, which
  stays loaded; the overlay binds to it.
- **`shell call <id> <method> <arg>` stringifies the result** (`String(result)`,
  `undefined`/`null` → `"ok"`): return JSON text from a read-out method, not
  an object (`[object Object]`). It answers `unknown` while the overlay is
  not loaded or has no such method. A side-effect-free load probe does not
  exist; `call <id> close ""` answers `ok` when loaded but also closes it.
- **`PanelWindow` cannot be swapped like `KeyboardPanel`:** it comes from the
  `Quickshell` module, not from `qs.Ui`. Wrap it in a plugin file of its
  own (`components/overlay/OverlayWindow.qml`) and let the harness run a
  copy of `plugin/` with that one file replaced by a plain Item that fills
  the harness window; the overlay's geometry is then that of a window of
  the harness size.
- **Locked test host:** `shell toggle jax.seldon` and `hide` over IPC work
  while `secure: true` (the overlay loads, `call … close` reaches it, `hide`
  unloads it) and log nothing; whether the surface would show above the
  lock is untested (no screenshots while locked).
- **`Style.space(n)` scales with the font size** (`effectiveSpacingScale` =
  `spacingScale × fontBaseSize/12`), so minimum slot sizes given in
  `Style.space` grow with a large font; a fixed-size layout needs a scroll
  fallback rather than a hard minimum.
- **qs.Ui `ButtonGroup` with `focusable: false`** works as a mouse-only
  selector next to a key catcher: a chip click emits `changed(value)`
  without taking keyboard focus, and a binding on `value` survives.

## WP-031 findings (2026-10-01, Omarchy 4.0.4-1, quickshell 0.3.1, Qt 6.11)

- **`Canvas` paints on frame 2 after it is created** (its 2D context comes
  up on frame 1), then once per `requestPaint()` per frame. It repaints by
  itself on a geometry change; `onWidthChanged: requestPaint()` is
  redundant. When the layout above it settles during frame 1 (a status
  banner), it paints twice at the same size; nothing in plugin code
  triggers the second one.
- **A Column's `implicitHeight` is set in its polish, one frame late.** A
  header whose height comes from `titles.implicitHeight` (a Column) grew by
  3 px after the charts' first paint and made them paint again. Sum the
  children's `implicitHeight`s instead where a Canvas sits below.
- **`ctx.fillStyle`/`strokeStyle` take QML colour values** (`Util.alpha(…)`,
  `Color.accent`) directly; no colour string is needed, so the token rule
  (no literal colours) holds inside `onPaint` too. `ctx.font` takes
  `"<px>px \"<family>\""` built from `Style.font.*` and `Style.font.family`.
- **Shared chart behaviour without function overriding:** a base Item
  (`ChartCanvas.qml`) emits signals (`paintRequested(ctx, w, h)`,
  `hoverRequested(x, y)`, `locateRequested(i)`) that the derived file
  handles with `onX: function(…) {…}`. qmllint stays at zero warnings;
  redeclaring a base function in a derived file does not.
- **A non-`.pragma library` JS import has one state per importing
  document per object.** A derived file (`Heatmap.qml`, whose root is
  `ChartCanvas {}`) that imports Model.js gets an instance of its own,
  separate from the one its base file `ChartCanvas.qml` imports, in the
  same object. A base-class function reading `Model.aggregationCount()`
  therefore never sees the derived file's calls (review of WP-031 caught
  exactly that). The base asks through a signal (`countRequested`) and each
  derived file answers with its own count (`ownCount`); `call view` sums
  the service's, the overlay's and every chart's.
- **Offscreen harness:** `Window.frameSwapped` fires with
  `QT_QPA_PLATFORM=offscreen`; `QSG_RENDER_TIMING=1` with
  `QT_LOGGING_RULES=qt.scenegraph.time.renderloop=true` logs each frame
  ("software" render loop: polish/sync/render ms). `TestCase.mouseMove`
  without a button reaches a `MouseArea { hoverEnabled: true }` as hover.
  Setting `win.width` and then `win.height` is two geometry changes (two
  paints).
- **`omarchy-launch-shell` runs the shell with Hyprland's environment**
  (spawned through `hyprctl dispatch exec`), so a `QSG_RENDER_TIMING=1` for
  the live shell has to go into Hyprland's environment before
  `omarchy-restart-shell` (`hyprctl keyword env …`, runtime only). That is
  a change on the test host: the operator's call.

## WP-041 findings (2026-10-01, Omarchy 4.0.4-1; marketplace template)

- **Marketplace README template** (https://plugins.omarchy.org/develop.html,
  read 2026-10-01): the finished example's README has, in order, a title
  with a one-line description, `## Install` (`omarchy plugin add <git-url>
  --enable`), `## Usage`, `## Configure` (`omarchy bar move <id> --section
  …`), `## Remove` (`omarchy plugin remove <id>`). The page's one rule:
  "Document every external dependency, setup step, privilege boundary,
  service, installer, or remote build used by your plugin." It says
  nothing about security scanning, preview size or dimensions; it calls
  `preview.png` optional and "beside these files". Built-in READMEs
  (`agents/`, `bar/`) are not a template: intro, Panel, Data,
  Interactions, Settings (agents). Settings examples use `omarchy bar set
  <id> <key> <value> [--json]` (numbers need `--json`).
- **CLI forms** (`omarchy plugin --help`, `omarchy bar --help`): `plugin add
  [git-url] [--enable] [--yes]`, `enable <id> [placement]`, `disable`,
  `remove [id] [--yes]`, `update [id] [--yes]`, `list [--json]`,
  `validate <folder>`; `bar move <id> [placement]`, placement like
  `--section center --index 0`.
- **`omarchy-launch-floating-terminal-with-presentation <command>`** joins
  its arguments and runs them with `bash -c` in a floating terminal (after
  `omarchy-show-logo`). So a banner's *Run in terminal* is a constant shell
  command shown to the user; document it as such in a security section.
- **Panel renders without a dev-mode footer:** a `panel-view.sh` run with
  `SELDON_INDEX` empty and `FAKE_SELDON_FIXTURE` set renders the live panel;
  a dev-mode run prints the absolute index path (a private path in a
  committed image). `PANEL_SHOTS=<dir>` does this for the Today tab.

## WP-037 findings (2026-10-01, Omarchy 4.0.4-1, quickshell 0.3.1)

- **The shell's panel/overlay Loader injects every property after
  creation.** `onLoaded` assigns `omarchyPath`, `shell`, `manifest`,
  `barWidgetRegistry`, `pluginRegistry` and `service` (`if ("service" in
  item) item.service = shell.serviceFor(id)`) once the async Loader has
  built the item. Every binding therefore first evaluates with all of them
  `null`, and then once more after each assignment. A fallback that does
  real work for `null` (here `periodTable(null)`: 23 aggregation passes)
  runs on every open. Make the null path a kept constant. A harness that
  passes these as `createObject` properties never sees this; create the
  item bare and assign afterwards, in the shell's order.
- **A theme change repaints a Canvas chart once and aggregates nothing.**
  The open overlay stays open through `omarchy theme set`, and `Color.*`
  changes repaint each chart once.
- **`omarchy theme set` works over plain ssh with only `OMARCHY_PATH`
  set.** The shell (and the overlay's colours) picked up Tokyo Night,
  Catppuccin Latte and Osaka Jade without `WAYLAND_DISPLAY` or `HYPRLAND_INSTANCE_SIGNATURE`
  in the ssh environment. Read `~/.local/state/omarchy/current/theme.name`
  in a separate call to confirm.

## WP-039 findings (2026-10-02, Omarchy 4.0.4-1, quickshell 0.3.1)

- **First-party panel widths** (`contentWidth: panel.fittedContentWidth(
  Style.space(n))`): 380 for the list panels (agents, audio, bluetooth,
  network, power, tailscale, monitor, dropbox), 480 weather, 560 clock;
  `KeyboardPanel`'s default is 280. `fittedContentWidth` caps at
  `availableCardWidth` (the screen). Seldon's panel is 460.
- **qs.Ui `Button` never elides its label.** `implicitWidth` = label +
  `controlPaddingX` (10 units) × 2 + reserved border; a smaller `width`
  lets the text spill past the border (no clip). The label is bold only
  while `selected`, so a cell sized from `implicitWidth` changes width
  when the selection moves; add the bold/plain `TextMetrics` difference.
- **Font scale in an offscreen harness:** `[font] base-size = 15` in
  `$HOME/.config/omarchy/shell.toml` (the user override Color.qml reads)
  gives scale 1.25; `Style.space` follows the font unless
  `[spacing] scale-with-font = false`.
- **`Text.truncated`** is true for an elided Text and for one cut at
  `maximumLineCount`; with `contentWidth > width` and `mapToItem` it makes
  a generic "does any label not fit" probe over the item tree.
- **`Flow` wraps when `x + child.width > width`**, so cells whose widths
  are rounded down and sum exactly to the width stay on one line; give
  the rounding rest to the last cell.

## Live frame timing without touching Hyprland's environment (2026-10-05)

- Omarchy 4's Hyprland takes Lua dispatchers: `hyprctl dispatch
  'hl.dsp.exec_cmd("…")'` (what `omarchy-restart-shell` uses). Stop the
  shell with `quickshell kill -p "$OMARCHY_PATH/shell" --any-display` (loop
  until it fails), then `hl.dsp.exec_cmd("env QSG_RENDER_TIMING=1
  QT_LOGGING_RULES=qt.scenegraph.time.renderloop=true omarchy-launch-shell")`:
  the variables reach only that shell process (`/proc/<pid>/environ`), the
  frame log goes to `journalctl --user -t omarchy-shell` (threaded loop:
  per-window "frame rendered in N ms, sync, render, swap" and "Frame
  prepared, polish …, blockedForSync …"). A plain `omarchy-restart-shell`
  afterwards drops them. Over ssh export `HYPRLAND_INSTANCE_SIGNATURE`
  (newest dir under `$XDG_RUNTIME_DIR/hypr`) or `hyprctl` prints nothing.
- Lines are tagged by window pointer, not name: identify a surface as the
  window that starts rendering after the IPC command. A bar with a
  scrolling media title renders at 60 fps in idle.

## WP-090 findings (2026-10-05, Omarchy shell tree at `$OMARCHY_PATH` 4.0.0.alpha `version` file, quickshell 0.3.1)

- **`omarchy plugin update` is meant to reload, and does not load new
  code.** `bin/omarchy-plugin-update` pulls each git checkout and, if any
  changed, runs `omarchy-shell shell rescanPlugins` → `shell.reloadPlugins()`
  (also what the `inotifywait` watch on `~/.config/omarchy/plugins` triggers
  for any file change outside `.git`): it destroys panels, non-`keepLoaded`
  services and widget registrations, then `finishPluginReload()` calls
  `Qt.clearComponentCache()` **only if it is a function**, and rescans.
  In quickshell 0.3.1 `typeof Qt.clearComponentCache` and
  `typeof Qt.trimComponentCache` are both `"undefined"`, so
  `Qt.createComponent(url)` returns the cached compiled type: **old QML and
  old JS imports** (`Model.js`). Reproduced offscreen: a QML file and its JS
  import edited on disk, `createComponent` again → both still old. Only
  `omarchy-restart-shell` loads the new code.
- **The manifest, unlike the code, is fresh after a rescan.** The registry's
  scan `cat`s every `manifest.json`; a new service instance gets
  `inst.manifest = publicPluginManifest(m)` (a third party's copy without
  `__sourceDir`), and a kept instance is handed the fresh manifest too
  (`_syncServices`). So `manifest.version` ≠ a version constant compiled
  into the code ⇔ the plugin was updated under a running shell. The plugin
  cannot learn its own folder from the manifest (`__sourceDir` is
  stripped); the injected manifest is the way to read it.
- **`omarchy-restart-shell`** kills every quickshell of the config dir
  (`quickshell kill -p … --any-display`), relaunches via `hyprctl dispatch
  exec omarchy-launch-shell` and waits for `ping`. It refuses while a secure
  lock is up. First-party QML runs it the same way: the Omarchy menu's
  *Update > Process > Shell* (`default/omarchy/omarchy-menu.jsonc`,
  `update.process.shell`) goes through `plugins/menu/Menu.qml`
  `runAction` → `Commons/Util.qml` `execDetached` →
  `Quickshell.execDetached(["bash", "-lc", command])`. The plugin starts it
  with `Quickshell.execDetached(["omarchy-restart-shell"])`. **Verified
  live** on the test host (orchestrator, WP-090 round 1): a click on
  *Restart shell* gave a new shell within 1 s, one process, the notice
  gone — the detached process outlives the shell it kills.
- A second restart started while the first one runs can kill the *new*
  shell: the plugin's restart action is one-shot per service instance
  (WP-090 round 2).

## WP-098 findings (2026-10-05, Omarchy 4.0.4)

- **Every dir under `~/.config/omarchy/plugins` with a `manifest.json` is
  a plugin** (`omarchy-plugin-catalog`: `find -L … -mindepth 2 -maxdepth
  2 -name manifest.json`, dot dirs skipped). A backup of `jax.seldon`
  next to it would be a second plugin with the same id (`omarchy plugin
  add` refuses "already used by …"); keep backups outside, e.g.
  `~/.local/state/seldon-dev/`.
- **`omarchy plugin update` needs a git checkout** (`fetch origin HEAD`,
  `merge --ff-only`); a plain copy fails with "not a git checkout". A
  clone at a tag (detached HEAD) still fast-forwards.
