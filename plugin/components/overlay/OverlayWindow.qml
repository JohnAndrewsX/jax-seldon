import QtQuick
import Quickshell
import Quickshell.Wayland

// The Prime Radiant's fullscreen layer-shell window (SPEC-PLUGIN §6), the
// pattern of the shell's emojis overlay: every edge anchored, overlay layer,
// exclusive keyboard focus, no exclusive zone. The window itself is
// transparent; Overlay.qml paints the scrim.
//
// A file of its own so that tests/plugin/overlay-view.sh can replace it with
// a plain Item (tests/plugin/harness/OverlayWindow.qml): an offscreen
// Quickshell cannot create a layer-shell window.
PanelWindow {
  anchors { top: true; bottom: true; left: true; right: true }
  color: "transparent"
  WlrLayershell.namespace: "jax-seldon-prime-radiant"
  WlrLayershell.layer: WlrLayer.Overlay
  WlrLayershell.keyboardFocus: WlrKeyboardFocus.Exclusive
  exclusionMode: ExclusionMode.Ignore
}
