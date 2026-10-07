pragma ComponentBehavior: Bound

import QtQuick
import qs.Commons

// Key/value rows of a detail (prototype `.kv`): the key muted in a fixed
// column, the value wrapped (anywhere: paths) beside it. `rows`: [[key,
// value], …], plain text.
Column {
  id: root

  property var rows: []
  property real keyWidth: Style.space(160)
  property color foreground: Color.popups.text
  property string fontFamily: Style.font.family

  spacing: Style.spacing.md

  Repeater {
    model: root.rows

    Item {
      id: kvRow

      required property var modelData

      width: root.width
      height: Math.max(key.implicitHeight, value.implicitHeight)

      Text {
        id: key
        width: Math.min(root.keyWidth, root.width / 2)
        textFormat: Text.PlainText
        text: kvRow.modelData[0]
        color: Color.muted
        wrapMode: Text.WrapAnywhere
        font.family: root.fontFamily
        font.pixelSize: Style.font.bodySmall
      }

      Text {
        id: value
        x: key.width + Style.spacing.xxl
        width: parent.width - x
        textFormat: Text.PlainText
        text: kvRow.modelData[1]
        color: root.foreground
        wrapMode: Text.WrapAnywhere
        font.family: root.fontFamily
        font.pixelSize: Style.font.bodySmall
      }
    }
  }
}
