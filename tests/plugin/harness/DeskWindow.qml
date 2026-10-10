import QtQuick

// Test stand-in for plugin/components/desk/DeskWindow.qml
// (tests/plugin/desk-view.sh).
//
// The real one is a layer-shell PanelWindow over the focused monitor, which
// an offscreen Quickshell cannot create. This fills the item it is put in
// (the harness window), so Desk.qml and everything inside it run unchanged
// and their geometry is that of a screen of the harness size. The screen's
// name counts the re-targets ("harness-2" after the second), so a case can
// see that an open desk is re-targeted on a summon.
Item {
  property int retargets: 0
  readonly property string screenName: "harness-" + retargets

  function retarget() { retargets++ }

  anchors.fill: parent
}
