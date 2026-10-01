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
  property int currentIndex: 0
  property color foreground: Color.foreground
  property string fontFamily: Style.font.family

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
      tooltipText: "Key " + (index + 1)
      selected: index === root.currentIndex
      bordered: true
      foreground: root.foreground
      fontFamily: root.fontFamily
      fontSize: Style.font.bodySmall
      onClicked: root.activated(index)
    }
  }
}
