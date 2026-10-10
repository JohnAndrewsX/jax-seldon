import QtQuick
import qs.Commons
import ".."

// The desk's detail pane (ADR-0034 §2): in the stacked layout a "back to
// the list" row, then the sticky ActionBar, then the scrolling content
// (the default property: a Column of the section's blocks, a small-capitals
// `title` first). The action bar is outside the Flickable, so it stays.
Item {
  id: root

  readonly property Tone tone: Tone {}

  property string title: ""
  property bool backVisible: false
  property alias actions: bar.actions
  property alias meta: bar.meta
  property alias hint: bar.hint
  property color foreground: Color.popups.text
  property string fontFamily: Style.font.family
  default property alias content: body.data

  readonly property Flickable flickable: flick
  readonly property ActionBar actionBar: bar

  signal backRequested()
  signal actionTriggered(string id)

  Item {
    id: back
    objectName: "deskBack"
    visible: root.backVisible
    width: parent.width
    height: visible ? backText.implicitHeight + Style.spacing.lg * 2 : 0

    Text {
      id: backText
      x: Style.spacing.huge
      anchors.verticalCenter: parent.verticalCenter
      textFormat: Text.PlainText
      text: "‹ Back to the list"
      color: root.tone.accentText
      font.family: root.fontFamily
      font.pixelSize: Style.font.bodySmall
    }

    MouseArea {
      anchors.fill: parent
      cursorShape: Qt.PointingHandCursor
      onClicked: root.backRequested()
    }
  }

  ActionBar {
    id: bar
    anchors.top: back.bottom
    width: parent.width
    height: implicitHeight
    foreground: root.foreground
    fontFamily: root.fontFamily
    onTriggered: function(id) { root.actionTriggered(id) }
  }

  Flickable {
    id: flick
    objectName: "deskDetailScroll"
    anchors.top: bar.bottom
    anchors.bottom: parent.bottom
    width: parent.width
    clip: true
    contentWidth: width
    contentHeight: body.implicitHeight + Style.spacing.huge * 2
    boundsBehavior: Flickable.StopAtBounds
    interactive: contentHeight > height

    Column {
      id: body
      x: Style.spacing.huge + Style.spacing.lg
      y: Style.spacing.huge
      width: flick.width - (Style.spacing.huge + Style.spacing.lg) * 2
      spacing: Style.spacing.huge

      Text {
        visible: root.title !== ""
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
    }
  }
}
