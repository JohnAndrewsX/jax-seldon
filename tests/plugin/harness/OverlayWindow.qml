import QtQuick

// Test stand-in for plugin/components/overlay/OverlayWindow.qml
// (tests/plugin/overlay-view.sh).
//
// The real one is a fullscreen layer-shell PanelWindow, which an offscreen
// Quickshell cannot create. This fills the item it is put in (the harness
// window), so Overlay.qml and everything inside it run unchanged and their
// geometry is the geometry of a window of the harness size.
Item {
  anchors.fill: parent
}
