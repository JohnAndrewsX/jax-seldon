import QtQuick

// Test stand-in for plugin/components/desk/DeskWindow.qml
// (tests/plugin/desk-view.sh).
//
// The real one is a layer-shell PanelWindow over the focused monitor, which
// an offscreen Quickshell cannot create. This fills the item it is put in
// (the harness window), so Desk.qml and everything inside it run unchanged
// and their geometry is that of a screen of the harness size.
Item {
  readonly property string screenName: "harness"

  function retarget() {}

  anchors.fill: parent
}
