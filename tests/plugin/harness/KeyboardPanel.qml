import QtQuick
import Quickshell

// Test stand-in for qs.Ui KeyboardPanel (tests/plugin/panel-view.sh).
//
// The real one is a layer-shell PanelWindow anchored to the bar, which an
// offscreen Quickshell cannot create. This keeps its properties and sizing
// functions and shows the content in place, so Panel.qml and everything
// inside it (tabs, rows, the shell's own PanelKeyCatcher) run unchanged.
Item {
  id: root

  property Item anchorItem: null
  property QtObject bar: null
  property var owner: null
  property int margin: 0
  property int padding: 0
  property int contentWidth: 280
  property int contentHeight: 200
  property var borderSpec: null
  property bool centerOnBar: false
  property bool open: false
  property bool popoutSwitching: false
  property bool popoutSwitchClosing: false
  property Item focusTarget: null

  default property alias contentItem: holder.data

  // HARNESS_CARD_WIDTH stands in for the real panel's `availableCardWidth`
  // (the room the screen leaves); unset, the room is unlimited.
  readonly property int availableCardWidth: Number(Quickshell.env("HARNESS_CARD_WIDTH") || 0)

  function fittedContentWidth(width, cap) {
    var max = root.availableCardWidth > 0 ? root.availableCardWidth : width
    if (cap) max = Math.min(max, cap)
    return Math.round(Math.min(width, max))
  }
  function fittedContentHeight(height, cap) { return cap ? Math.min(height, cap) : height }
  function close() { root.open = false }

  visible: open
  width: contentWidth
  height: contentHeight

  onOpenChanged: if (open && focusTarget) Qt.callLater(function() { root.focusTarget.forceActiveFocus() })

  Item {
    id: holder
    anchors.fill: parent
  }
}
