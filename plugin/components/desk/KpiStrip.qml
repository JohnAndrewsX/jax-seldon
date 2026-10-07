pragma ComponentBehavior: Bound

import QtQuick
import qs.Commons

// The header's KPI strip (ADR-0034 §2): active · verification · queued ·
// crises · attention, from Model.deskKpis. Big tabular numbers over small
// capitals; active in the accent, crises in the urgent colour while any is
// open. A click on a figure asks for its section.
Row {
  id: root

  property var kpis: []
  property color foreground: Color.popups.text
  property color accent: Color.accent
  property color urgent: Color.urgent
  property string fontFamily: Style.font.family

  signal picked(string id)

  spacing: Style.spacing.huge

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
        color: kpi.modelData.tone === "urgent" ? root.urgent
          : kpi.modelData.tone === "accent" ? root.accent
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
        color: Color.muted
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
