import QtQuick
import qs.Commons
import qs.Ui
import ".."

// One row of a desk list (prototype `.row`): a stripe at the left, the
// title over a meta line in the dim tone, a right-aligned small text
// (`aside`) (an age, a date). `mark` is the row's status (Model.statusOf,
// WP-178: { glyph, word, tone, stripe } or null): its stripe (crisis in the
// urgent colour, attention in the accent) and, before the meta line, its
// glyph and word in the same tone, so no state is told by colour alone; a
// row without a mark has no stripe. `alert`, one word in the urgent tone,
// says what went wrong (WP-137: a pacman transaction that did not
// complete).
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
  property var mark: null
  property string alert: ""
  property bool selected: false
  property bool cursor: false
  property bool pointerHover: false
  property color foreground: Color.popups.text
  property color accent: Color.accent
  property color urgent: Color.urgent
  property string fontFamily: Style.font.family

  readonly property Tone tone: Tone {}
  readonly property string stripe: root.mark && root.mark.word ? String(root.mark.stripe || "") : ""
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

  // The selection bar and the stripe are square (SPEC-PLUGIN §7
  // "Geometry"): each is a slice of the row's own shape, a rectangle of the
  // row's size and Style.cornerRadius clipped to the bar, so it runs the full
  // height where the row is straight and follows the row's rounded corner
  // instead of sticking out past it (review F9).
  Item {
    objectName: "rowSelectionBar"
    visible: root.selected
    width: root.barWidth
    height: parent.height
    clip: true

    Rectangle {
      x: -parent.x
      width: root.width
      height: parent.height
      radius: Style.cornerRadius
      color: root.tone.accentUi
    }
  }

  Item {
    objectName: "rowStripe"
    visible: root.stripe !== ""
    x: root.barWidth + Style.spacing.xxs
    width: Math.max(2, Style.space(3))
    height: parent.height
    clip: true

    Rectangle {
      x: -parent.x
      width: root.width
      height: parent.height
      radius: Style.cornerRadius
      color: root.stripe === "crisis" ? root.urgent : root.accent
    }
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
      visible: root.meta !== "" || root.alert !== "" || statusText.visible
      width: parent.width
      implicitHeight: Math.max(statusText.implicitHeight, alertText.implicitHeight, metaText.implicitHeight)

      Text {
        id: statusText
        objectName: "rowStatus"
        visible: root.stripe !== ""
        textFormat: Text.PlainText
        text: root.mark ? (root.mark.glyph ? root.mark.glyph + " " : "") + root.mark.word : ""
        color: root.stripe === "crisis" ? root.tone.urgentText : root.tone.accentText
        font.family: root.fontFamily
        font.pixelSize: Style.font.caption
        font.bold: true
      }

      Text {
        id: alertText
        objectName: "rowAlert"
        x: statusText.visible ? statusText.implicitWidth + Style.spacing.md : 0
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
        x: alertText.visible ? alertText.x + alertText.implicitWidth + Style.spacing.md
          : statusText.visible ? statusText.implicitWidth + Style.spacing.md : 0
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
