import QtQuick
import Quickshell
import Quickshell.Wayland
import Quickshell.Hyprland
import "../../Model.js" as Model

// The desk's layer-shell window (ADR-0034 §1), the pattern of the shell's
// menu: every edge anchored, transparent, the desk an item centred inside
// (Desk.qml, Model.deskGeometry), so the area around it catches the click
// that closes. Overlay layer, exclusive keyboard focus while shown. Unlike
// the menu it keeps out of other surfaces' exclusive zones
// (ExclusionMode.Normal, no zone of its own): the bar stays visible above
// it and its pill stays clickable.
//
// One window, re-targeted on every open to the monitor Hyprland has
// focused (as Bar.qml's focusedScreenName), the first screen when none
// matches.
//
// A file of its own so that tests/plugin/desk-view.sh can replace it with
// a plain Item (tests/plugin/harness/DeskWindow.qml): an offscreen
// Quickshell cannot create a layer-shell window.
PanelWindow {
  id: root

  readonly property string screenName: root.screen ? String(root.screen.name || "") : ""

  function retarget() {
    var screens = Quickshell.screens
    if (!screens || screens.length === 0) return
    var names = []
    for (var i = 0; i < screens.length; i++) names.push(String(screens[i].name || ""))
    var focused = Hyprland.focusedMonitor ? String(Hyprland.focusedMonitor.name || "") : ""
    root.screen = screens[Model.pickScreen(names, focused)]
  }

  anchors { top: true; bottom: true; left: true; right: true }
  color: "transparent"
  exclusionMode: ExclusionMode.Normal
  exclusiveZone: 0
  WlrLayershell.namespace: "jax-seldon-desk"
  WlrLayershell.layer: WlrLayer.Overlay
  WlrLayershell.keyboardFocus: root.visible ? WlrKeyboardFocus.Exclusive : WlrKeyboardFocus.None
}
