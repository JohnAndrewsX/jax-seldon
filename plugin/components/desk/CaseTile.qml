import QtQuick
import qs.Commons
import qs.Ui
import ".."

// An active case on Today's overview (prototype `.tile`): "id · risk", the
// title, the plan's progress, "2/4 steps · claude-code". A click opens the
// case in Work. Plain text; the title wraps to two lines.
BorderSurface {
  id: root

  readonly property Tone tone: Tone {}

  // { id, risk, title, progress, text } from Model.deskToday().cases
  property var caseData: null
  property color foreground: Color.popups.text
  property string fontFamily: Style.font.family

  signal clicked()

  implicitHeight: column.implicitHeight + Style.spacing.lg * 2
  radius: Style.cornerRadius
  color: hover.hovered ? Style.hoverFill : Style.normalFill
  borderSpec: Border.flat(root.tone.divider, Math.max(1, Style.space(1)))

  Column {
    id: column
    x: Style.spacing.xl
    y: Style.spacing.lg
    width: root.width - Style.spacing.xl * 2
    spacing: Style.spacing.sm

    Text {
      width: parent.width
      textFormat: Text.PlainText
      text: root.caseData ? root.caseData.id + " · " + root.caseData.risk : ""
      color: root.tone.dim
      elide: Text.ElideRight
      font.family: root.fontFamily
      font.pixelSize: Style.font.caption
    }

    Text {
      width: parent.width
      textFormat: Text.PlainText
      text: root.caseData ? root.caseData.title : ""
      color: root.foreground
      wrapMode: Text.Wrap
      maximumLineCount: 2
      elide: Text.ElideRight
      font.family: root.fontFamily
      font.pixelSize: Style.font.body
      font.bold: true
    }

    Progress {
      width: parent.width
      value: root.caseData ? root.caseData.progress : 0
      foreground: root.foreground
    }

    Text {
      width: parent.width
      textFormat: Text.PlainText
      text: root.caseData ? root.caseData.text : ""
      color: root.tone.dim
      elide: Text.ElideRight
      font.family: root.fontFamily
      font.pixelSize: Style.font.caption
    }
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
