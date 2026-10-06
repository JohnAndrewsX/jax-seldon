import QtQuick
import qs.Commons
import qs.Ui

// One row of a desk list (prototype `.row`): a stripe at the left (crisis
// in the urgent colour, attention in the accent, none), the title over a
// muted meta line, a right-aligned small text (`aside`) (an age, a date), the
// selection's accent border and the cursor's hover fill. User content is
// plain text and elides; Enter or the detail shows it in full.
Item {
  id: root

  property string title: ""
  property string meta: ""
  property string aside: ""
  property string stripe: ""
  property bool selected: false
  property bool cursor: false
  property color foreground: Color.popups.text
  property color accent: Color.accent
  property color urgent: Color.urgent
  property string fontFamily: Style.font.family

  signal clicked()

  implicitHeight: textColumn.implicitHeight + Style.spacing.lg * 2

  BorderSurface {
    anchors.fill: parent
    radius: Style.cornerRadius
    color: root.cursor || hover.hovered ? Style.hoverFill : Style.normalFill
    borderSpec: Border.flat(root.selected ? root.accent : Util.alpha(root.foreground, 0.0), Math.max(1, Style.space(1)))
  }

  Rectangle {
    visible: root.stripe !== ""
    width: Math.max(2, Style.space(3))
    height: parent.height
    radius: width / 2
    color: root.stripe === "crisis" ? root.urgent : root.accent
  }

  Column {
    id: textColumn
    x: Style.spacing.xxl
    anchors.verticalCenter: parent.verticalCenter
    width: parent.width - x - Style.spacing.xxl - (rightText.visible ? rightText.implicitWidth + Style.spacing.lg : 0)
    spacing: Style.spacing.xxs

    Text {
      width: parent.width
      textFormat: Text.PlainText
      text: root.title
      color: root.foreground
      elide: Text.ElideRight
      font.family: root.fontFamily
      font.pixelSize: Style.font.body
    }

    Text {
      visible: root.meta !== ""
      width: parent.width
      textFormat: Text.PlainText
      text: root.meta
      color: Color.muted
      elide: Text.ElideRight
      font.family: root.fontFamily
      font.pixelSize: Style.font.caption
    }
  }

  Text {
    id: rightText
    visible: root.aside !== ""
    anchors.right: parent.right
    anchors.rightMargin: Style.spacing.xxl
    anchors.verticalCenter: parent.verticalCenter
    textFormat: Text.PlainText
    text: root.aside
    color: Color.muted
    font.family: root.fontFamily
    font.pixelSize: Style.font.caption
  }

  HoverHandler {
    id: hover
  }

  MouseArea {
    anchors.fill: parent
    cursorShape: Qt.PointingHandCursor
    onClicked: root.clicked()
  }
}
