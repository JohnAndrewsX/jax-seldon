pragma ComponentBehavior: Bound

import QtQuick
import qs.Commons
import qs.Ui

// The panel's tab strip (SPEC-PLUGIN §5). Same pattern as the provider
// switch of the built-in agents panel: one bordered Button per tab, equal
// widths, the current one selected. It only reports clicks; Panel.qml owns
// the current index and the keyboard.
Row {
  id: root

  property var tabs: []
  // The digit that selects each tab (Model.TAB_KEYS), for the tooltip.
  property var keys: []
  property int currentIndex: 0
  property color foreground: Color.foreground
  property string fontFamily: Style.font.family
  property real fontSize: Style.font.bodySmall

  signal activated(int index)

  readonly property real cellWidth: tabs.length > 0 ? (width - spacing * (tabs.length - 1)) / tabs.length : 0

  spacing: Style.spacing.md

  Repeater {
    model: root.tabs

    Button {
      required property var modelData
      required property int index

      width: root.cellWidth
      text: modelData
      tooltipText: root.keys[index] ? "Key " + root.keys[index] : ""
      selected: index === root.currentIndex
      bordered: true
      foreground: root.foreground
      fontFamily: root.fontFamily
      fontSize: root.fontSize
      onClicked: root.activated(index)
    }
  }
}
