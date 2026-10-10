import QtQuick
import qs.Commons
import qs.Ui
import ".."

// One row of a desk list (prototype `.row`): a stripe at the left (crisis
// in the urgent colour, attention in the accent, none), the title over a
// meta line in the dim tone, a right-aligned small text (`aside`) (an age,
// a date). `alert`, one word in the urgent tone before the meta line, says
// what went wrong (WP-137: a pacman transaction that did not complete).
// User content is plain text and elides; Enter or the detail shows it in
// full.
//
// One cursor (Omarchy's Ui/CursorSurface.qml, WP-177): the row reads no
// hover of its own. `cursor` is the pointer's row, which the list sets
// only on a real pointer move (ListColumn.hoverIndex, PointerMoveGate) and
// clears on every key, so a keyboard move under a still pointer leaves one
// highlight; it draws Omarchy's hover cursor. `selected` is the selection
// (the desk's keyboard cursor): Omarchy's selected fill plus a second cue
// that is not a fill, an accent bar of at least 3:1 at the left and a bold
// title. A row outside a list (a detail's links) sets `pointerHover` to
// take the pointer itself, as no key moves there.
Item {
  id: root

  property string title: ""
  property string meta: ""
  property string aside: ""
  property string stripe: ""
  property string alert: ""
  property bool selected: false
  property bool cursor: false
  property bool pointerHover: false
  property color foreground: Color.popups.text
  property color accent: Color.accent
  property color urgent: Color.urgent
  property string fontFamily: Style.font.family

  readonly property Tone tone: Tone {}
  readonly property bool hasCursor: root.cursor || (root.pointerHover && hover.hovered)
  // The selection bar's width; the stripe sits beside it, so neither moves.
  readonly property int barWidth: Math.max(2, Style.space(3))

  signal clicked()

  implicitHeight: textColumn.implicitHeight + Style.spacing.lg * 2

  CursorSurface {
    objectName: "deskRow"
    anchors.fill: parent
    hasCursor: root.hasCursor
    current: root.selected
    foreground: root.foreground
    accent: root.accent
    // An idle row keeps Omarchy's normal fill (the prototype's row card).
    color: hasCursor ? fill : (current ? currentFill : Style.normalFillFor(root.foreground, root.accent))
  }

  Rectangle {
    objectName: "rowSelectionBar"
    visible: root.selected
    width: root.barWidth
    height: parent.height
    color: root.tone.accentUi
  }

  Rectangle {
    visible: root.stripe !== ""
    x: root.barWidth + Style.spacing.xxs
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
      font.bold: root.selected
    }

    Item {
      visible: root.meta !== "" || root.alert !== ""
      width: parent.width
      implicitHeight: Math.max(alertText.implicitHeight, metaText.implicitHeight)

      Text {
        id: alertText
        objectName: "rowAlert"
        visible: root.alert !== ""
        textFormat: Text.PlainText
        text: root.alert
        color: root.tone.urgentText
        font.family: root.fontFamily
        font.pixelSize: Style.font.caption
        font.bold: true
      }

      Text {
        id: metaText
        x: alertText.visible ? alertText.implicitWidth + Style.spacing.md : 0
        width: parent.width - x
        visible: root.meta !== ""
        textFormat: Text.PlainText
        text: root.meta
        color: root.tone.dim
        elide: Text.ElideRight
        font.family: root.fontFamily
        font.pixelSize: Style.font.caption
      }
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
    color: root.tone.dim
    font.family: root.fontFamily
    font.pixelSize: Style.font.caption
  }

  HoverHandler {
    id: hover
    enabled: root.pointerHover
  }

  MouseArea {
    anchors.fill: parent
    cursorShape: Qt.PointingHandCursor
    onClicked: root.clicked()
  }
}
