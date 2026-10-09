import QtQuick
import qs.Commons
import qs.Ui
import ".."

// The desk's list column (ADR-0034 §2): a small-capitals title, whatever
// the section puts above the list (chips, a field: the default property),
// then a ListView of the section's rows (delegates from Model.js row
// functions), or `emptyText` when there are none. The cursor row is
// `currentIndex`; the view keeps it in sight.
//
// One cursor (SPEC-PLUGIN §5.3, WP-177): `hoverIndex` is the row under the
// pointer, set only when the pointer really moved (Omarchy's
// Ui/PointerMoveGate.qml: rows that move under a still pointer do not
// count) and cleared by a keyboard move, a scroll and the pointer leaving.
// A delegate passes `cursor: list.hoverIndex === index` to its ListRow.
Item {
  id: root

  property string title: ""
  property string emptyText: ""
  property alias model: list.model
  property alias delegate: list.delegate
  property alias currentIndex: list.currentIndex
  property color foreground: Color.popups.text
  property string fontFamily: Style.font.family
  // Draw the hairline at the right (not in the stacked layout).
  property bool divider: true
  default property alias head: headColumn.data

  readonly property ListView view: list
  property int hoverIndex: -1

  readonly property Tone tone: Tone {}

  // A key, a click or an index update moved the selection, or the list
  // scrolled: no pointer row until the pointer moves again.
  function dropPointer() {
    gate.reset()
    root.hoverIndex = -1
  }

  function pointerAt(position) {
    if (!gate.moved(root, position)) return
    var p = list.mapFromItem(root, position.x, position.y)
    root.hoverIndex = p.x < 0 || p.y < 0 || p.x >= list.width || p.y >= list.height ? -1
      : list.indexAt(p.x + list.contentX, p.y + list.contentY)
  }

  PointerMoveGate {
    id: gate
    referenceItem: list
  }

  Rectangle {
    visible: root.divider
    anchors.right: parent.right
    width: Style.spacing.hairline
    height: parent.height
    color: root.tone.divider
  }

  Column {
    id: top
    x: Style.spacing.xxl
    y: Style.spacing.xxl
    width: parent.width - Style.spacing.xxl * 2
    spacing: Style.spacing.lg

    Text {
      width: parent.width
      textFormat: Text.PlainText
      text: root.title.toUpperCase()
      color: root.tone.dim
      elide: Text.ElideRight
      font.family: root.fontFamily
      font.pixelSize: Style.font.caption
      font.letterSpacing: Style.space(1)
      font.bold: true
    }

    Column {
      id: headColumn
      width: parent.width
      spacing: Style.spacing.md
    }
  }

  ListView {
    id: list
    x: Style.spacing.xxl
    anchors.top: top.bottom
    anchors.topMargin: Style.spacing.lg
    anchors.bottom: parent.bottom
    anchors.bottomMargin: Style.spacing.xxl
    width: parent.width - Style.spacing.xxl * 2
    clip: true
    spacing: Style.spacing.md
    boundsBehavior: Flickable.StopAtBounds
    highlightFollowsCurrentItem: false
    currentIndex: -1
    onCurrentIndexChanged: {
      root.dropPointer()
      if (currentIndex >= 0) positionViewAtIndex(currentIndex, ListView.Contain)
    }
    onContentYChanged: root.dropPointer()
  }

  // On the column, not inside the Flickable: positions in the column's
  // coordinates do not change while the rows scroll under the pointer.
  HoverHandler {
    onPointChanged: root.pointerAt(point.position)
    onHoveredChanged: if (!hovered) root.dropPointer()
  }

  Text {
    visible: list.count === 0 && root.emptyText !== ""
    x: Style.spacing.xxl
    anchors.top: list.top
    width: parent.width - Style.spacing.xxl * 2
    textFormat: Text.PlainText
    text: root.emptyText
    color: root.tone.dim
    wrapMode: Text.Wrap
    font.family: root.fontFamily
    font.pixelSize: Style.font.bodySmall
  }
}
