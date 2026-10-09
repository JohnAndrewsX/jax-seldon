pragma ComponentBehavior: Bound

import QtQuick
import qs.Commons
import ".."

// The header's KPI strip (ADR-0034 §2): active · verification · queued ·
// crises · attention, from Model.deskKpis. Big tabular numbers over small
// capitals; active in the accent, crises in the urgent colour while any is
// open. A click on a figure asks for its section.
Row {
  id: root

  readonly property Tone tone: Tone {}

  property var kpis: []
  property color foreground: Color.popups.text
  property color accent: Color.accent
  property color urgent: Color.urgent
  property string fontFamily: Style.font.family

  signal picked(string id)

  // The strip's height from its fonts, at once: a Row sets its own height
  // only in its polish, one frame late, which would move everything under
  // the header after the first frame (and repaint the Prime Radiant's
  // charts, WP-123). The header sizes itself by this.
  readonly property real figureHeight: root.kpis.length > 0 ? numberProbe.implicitHeight + labelProbe.implicitHeight : 0

  spacing: Style.spacing.huge

  Text {
    id: numberProbe
    visible: false
    textFormat: Text.PlainText
    text: "0"
    font.family: root.fontFamily
    font.pixelSize: Style.font.display
    font.bold: true
    font.features: { "tnum": 1 }
  }

  Text {
    id: labelProbe
    visible: false
    textFormat: Text.PlainText
    text: "A"
    font.family: root.fontFamily
    font.pixelSize: Style.font.caption
    font.letterSpacing: Style.space(1)
  }

  Repeater {
    model: root.kpis

    Item {
      id: kpi

      required property var modelData

      objectName: "kpi:" + kpi.modelData.id
      width: Math.max(number.implicitWidth, label.implicitWidth)
      height: number.implicitHeight + label.implicitHeight

      Text {
        id: number
        anchors.right: parent.right
        textFormat: Text.PlainText
        text: String(kpi.modelData.value)
        color: kpi.modelData.tone === "urgent" ? root.tone.urgentText
          : kpi.modelData.tone === "accent" ? root.tone.accentText
          : root.foreground
        font.family: root.fontFamily
        font.pixelSize: Style.font.display
        font.bold: true
        font.features: { "tnum": 1 }
      }

      Text {
        id: label
        anchors.right: parent.right
        anchors.top: number.bottom
        textFormat: Text.PlainText
        text: kpi.modelData.label.toUpperCase()
        color: root.tone.dim
        font.family: root.fontFamily
        font.pixelSize: Style.font.caption
        font.letterSpacing: Style.space(1)
      }

      MouseArea {
        anchors.fill: parent
        cursorShape: Qt.PointingHandCursor
        onClicked: root.picked(kpi.modelData.id)
      }
    }
  }
}
