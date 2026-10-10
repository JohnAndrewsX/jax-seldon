pragma ComponentBehavior: Bound

import QtQuick
import qs.Commons
import qs.Ui
import ".."

// The sticky action bar at the top of a detail (ADR-0034 §2, prototype
// round 2): the actions as buttons (`primary` drawn selected, `armed` with
// the desk's focus ring while it waits for its second press), the item's
// id and risk at the right, and the arm hint under them while a writing
// action waits for its second press (Arm.qml). It sits outside the
// detail's Flickable, so it stays while the detail scrolls.
Item {
  id: root

  readonly property Tone tone: Tone {}

  // [{ id, label, primary, enabled, armed }]
  property var actions: []
  property string meta: ""
  property string hint: ""
  property color foreground: Color.popups.text
  property color accent: Color.accent
  property string fontFamily: Style.font.family

  signal triggered(string id)

  visible: root.actions.length > 0 || root.meta !== ""
  implicitHeight: visible ? column.implicitHeight + Style.spacing.xxl * 2 : 0

  Rectangle {
    anchors.bottom: parent.bottom
    width: parent.width
    height: Style.spacing.hairline
    color: root.tone.divider
  }

  Column {
    id: column
    x: Style.spacing.huge
    y: Style.spacing.xxl
    width: parent.width - Style.spacing.huge * 2
    spacing: Style.spacing.sm

    Item {
      width: parent.width
      height: Math.max(buttons.height, metaText.implicitHeight)

      Flow {
        id: buttons
        width: parent.width - (metaText.visible ? metaText.implicitWidth + Style.spacing.xl : 0)
        spacing: Style.spacing.md

        Repeater {
          model: root.actions

          Button {
            id: actionButton
            required property var modelData
            text: modelData.label
            bordered: true
            selected: modelData.primary === true
            enabled: modelData.enabled !== false
            foreground: root.foreground
            accent: root.accent
            fontFamily: root.fontFamily
            fontSize: Style.font.bodySmall
            onClicked: root.triggered(modelData.id)

            FocusRing {
              shown: actionButton.modelData.armed === true
            }
          }
        }
      }

      Text {
        id: metaText
        visible: root.meta !== ""
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        textFormat: Text.PlainText
        text: root.meta
        color: root.tone.dim
        font.family: root.fontFamily
        font.pixelSize: Style.font.caption
      }
    }

    Text {
      visible: root.hint !== ""
      width: parent.width
      textFormat: Text.PlainText
      text: root.hint
      color: root.tone.accentText
      wrapMode: Text.Wrap
      font.family: root.fontFamily
      font.pixelSize: Style.font.caption
    }
  }
}
