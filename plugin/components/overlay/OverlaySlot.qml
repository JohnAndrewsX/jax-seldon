pragma ComponentBehavior: Bound

import QtQuick
import qs.Commons
import qs.Ui
import ".."

// One slot of the Prime Radiant grid (WP-030): the slot's name and what it
// draws on top, the chart (WP-031) in the area under it. With `placeholder`
// on it shows the row count of its series for the selected period instead
// (a Model.slotSummary object). Every string is set as plain text.
// A slot with a `legend` (the Timeline: Model.TIMELINE_LEGEND with colours)
// shows it in place of the subtitle: each entry's A12 marker files, tinted
// like the chart draws them, then its label.
BorderSurface {
  id: root

  readonly property Tone tone: Tone {}

  property var summary: null
  property color foreground: Color.popups.text
  property color accent: Color.accent
  property string fontFamily: Style.font.family
  // Off once the slot holds its chart.
  property bool placeholder: true
  // The chart in this slot, for Overlay.view() and hover().
  property ChartCanvas chart: null
  // [{ files: [file names in plugin/assets/], label, color }]; [] shows the
  // subtitle instead.
  property var legend: []

  // The chart goes here, filling the area under the title.
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

    // The chart's caption (its summary, or the hovered item), right-aligned;
    // it wins over the subtitle, which elides or hides.
    Text {
      id: caption
      anchors.right: parent.right
      anchors.baseline: title.baseline
      width: Math.min(implicitWidth, Math.max(0, parent.width - title.width - Style.spacing.lg))
      visible: text !== ""
      horizontalAlignment: Text.AlignRight
      textFormat: Text.PlainText
      text: root.chart ? root.chart.caption : ""
      color: root.chart && root.chart.hoverText !== "" ? root.foreground : root.tone.dim
      font.family: root.fontFamily
      font.pixelSize: Style.font.bodySmall
      elide: Text.ElideRight
    }

    Text {
      id: subtitle
      anchors.left: title.right
      anchors.leftMargin: Style.spacing.lg
      anchors.right: caption.visible ? caption.left : parent.right
      anchors.rightMargin: caption.visible ? Style.spacing.xl : 0
      anchors.baseline: title.baseline
      visible: root.legend.length === 0 && width >= Style.font.bodySmall * 4
      textFormat: Text.PlainText
      text: root.summary ? root.summary.subtitle : ""
      color: root.tone.dim
      font.family: root.fontFamily
      font.pixelSize: Style.font.bodySmall
      elide: Text.ElideRight
    }
    // The legend, where the subtitle would be; hidden when it does not fit
    // beside the caption.
    Row {
      id: legendRow
      anchors.left: title.right
      anchors.leftMargin: Style.spacing.lg
      anchors.verticalCenter: title.verticalCenter
      spacing: Style.spacing.lg
      visible: root.legend.length > 0
        && implicitWidth <= parent.width - title.width - Style.spacing.lg - (caption.visible ? caption.implicitWidth + Style.spacing.xl : 0)

      Repeater {
        model: root.legend

        Row {
          id: entry

          required property var modelData

          spacing: Style.spacing.xs

          Repeater {
            model: entry.modelData.files

            MaskIcon {
              required property string modelData

              y: Math.round((entry.height - height) / 2)
              width: Style.space(12)
              height: Style.space(12)
              file: modelData
              color: entry.modelData.color
            }
          }

          Text {
            y: Math.round((entry.height - height) / 2)
            textFormat: Text.PlainText
            text: entry.modelData.label
            color: root.tone.dim
            font.family: root.fontFamily
            font.pixelSize: Style.font.bodySmall
          }
        }
      }
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
        color: root.tone.accentText
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
