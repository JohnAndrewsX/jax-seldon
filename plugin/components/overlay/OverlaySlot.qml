import QtQuick
import qs.Commons
import qs.Ui

// One chart slot of the Prime Radiant grid (WP-030). Until WP-031 puts the
// chart in, it shows the slot's name, what the chart will draw and the row
// count of its series for the selected period (a Model.slotSummary object).
// Every string is set as plain text.
BorderSurface {
  id: root

  property var summary: null
  property color foreground: Color.popups.text
  property color accent: Color.accent
  property string fontFamily: Style.font.family
  // WP-031 turns this off once the slot holds its chart.
  property bool placeholder: true

  // WP-031: the chart goes here, filling the area under the title.
  default property alias content: chartArea.data
  readonly property alias chartArea: chartArea

  radius: Style.cornerRadius
  color: Style.normalFillFor(foreground, accent, Color.urgent)
  borderSpec: Border.controlSpec("normal", foreground, accent)
  padding: Style.spacing.panelPadding

  Item {
    id: titleRow
    x: root.contentLeftInset
    y: root.contentTopInset
    width: root.width - root.contentLeftInset - root.contentRightInset
    height: title.implicitHeight

    Text {
      id: title
      anchors.left: parent.left
      width: Math.min(implicitWidth, parent.width)
      textFormat: Text.PlainText
      text: root.summary ? root.summary.title : ""
      color: root.foreground
      font.family: root.fontFamily
      font.pixelSize: Style.font.title
      font.bold: true
      elide: Text.ElideRight
    }

    Text {
      anchors.left: title.right
      anchors.leftMargin: Style.spacing.lg
      anchors.right: parent.right
      anchors.baseline: title.baseline
      textFormat: Text.PlainText
      text: root.summary ? root.summary.subtitle : ""
      color: Color.muted
      font.family: root.fontFamily
      font.pixelSize: Style.font.bodySmall
      elide: Text.ElideRight
    }
  }

  Item {
    id: chartArea
    x: root.contentLeftInset
    y: titleRow.y + titleRow.height + Style.spacing.md
    width: titleRow.width
    height: Math.max(0, root.height - y - root.contentBottomInset)

    // The placeholder: the series' row count for the period, and one line
    // of what is in it.
    Column {
      anchors.centerIn: parent
      width: parent.width
      spacing: Style.spacing.sm
      visible: root.placeholder

      Text {
        width: parent.width
        horizontalAlignment: Text.AlignHCenter
        textFormat: Text.PlainText
        text: root.summary ? root.summary.count : ""
        color: root.accent
        font.family: root.fontFamily
        font.pixelSize: Style.font.display
        elide: Text.ElideRight
      }

      Text {
        width: parent.width
        horizontalAlignment: Text.AlignHCenter
        textFormat: Text.PlainText
        text: root.summary ? root.summary.detail : ""
        color: root.foreground
        font.family: root.fontFamily
        font.pixelSize: Style.font.body
        elide: Text.ElideRight
      }
    }
  }
}
