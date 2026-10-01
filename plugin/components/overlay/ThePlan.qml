pragma ComponentBehavior: Bound

import QtQuick
import qs.Commons
import "../../Model.js" as Model

// The Plan (SPEC-PLUGIN §6, the sixth slot): the active cases
// (`cases.active`, no period) as compact cards side by side — the Work
// tab's zone stripe (CaseCard), id and risk, title, a step progress bar
// with done/total, and the agent. Cards that do not fit the slot are
// counted in the caption. Hover: the case's id, title, steps and agent.
// Data: Model.planChart (precomputed; the period does not change it).
ChartCanvas {
  id: root

  chartId: "plan"

  readonly property var rows: root.chart && !root.empty ? root.chart.rows : []
  readonly property real gap: Style.spacing.md
  readonly property int columns: Model.planColumns(root.plot.width, Style.space(240), root.gap, root.rows.length)
  readonly property real cardW: Math.max(1, (root.plot.width - (root.columns - 1) * root.gap) / root.columns)
  readonly property real cardH: Style.font.caption * 2 + Style.font.body + Style.spacing.lg * 4
  readonly property int fits: root.columns * Math.max(1, Math.floor((root.plot.height + root.gap) / (root.cardH + root.gap)))
  readonly property int shown: Math.min(root.rows.length, root.fits)

  captionSuffix: root.shown < root.rows.length ? " · " + (root.rows.length - root.shown) + " not shown" : ""

  function toneColor(tone) {
    return tone === "urgent" ? root.urgent : tone === "accent" ? root.accent : root.muted
  }

  // Items: the cards in order (only the shown ones).
  onLocateRequested: function(index) {
    var i = index < 0 ? root.shown + index : index
    if (i >= 0 && i < root.shown)
      root.located = Qt.point((i % root.columns + 0.5) * (root.cardW + root.gap), (Math.floor(i / root.columns) + 0.5) * (root.cardH + root.gap))
  }

  onHoverRequested: function(x, y) {
    var col = Math.floor(x / (root.cardW + root.gap))
    var row = Math.floor(y / (root.cardH + root.gap))
    var i = row * root.columns + col
    var inside = x - col * (root.cardW + root.gap) <= root.cardW && y - row * (root.cardH + root.gap) <= root.cardH
    if (!inside || col >= root.columns || i < 0 || i >= root.shown) {
      root.clearHover()
      return
    }
    var c = root.rows[i]
    root.hoverText = [c.id, c.title, c.stepsText, c.agent, [c.zone, c.risk].filter(function(p) { return p !== "" }).join(" ")]
      .filter(function(p) { return p !== "" }).join(" · ")
    root.highlight = Qt.rect(col * (root.cardW + root.gap), row * (root.cardH + root.gap), root.cardW, root.cardH)
  }

  Repeater {
    model: root.rows

    Item {
      id: card

      required property var modelData
      required property int index

      x: root.plot.x + (card.index % root.columns) * (root.cardW + root.gap)
      y: root.plot.y + Math.floor(card.index / root.columns) * (root.cardH + root.gap)
      width: root.cardW
      height: root.cardH
      visible: card.index < root.shown

      Rectangle {
        anchors.fill: parent
        color: root.faint
        opacity: 0.5
      }

      Rectangle {
        id: stripe
        x: Style.spacing.sm
        y: Style.spacing.lg
        width: Style.spacing.xs
        height: card.height - Style.spacing.lg * 2
        radius: width / 2
        color: root.toneColor(card.modelData.tone)
      }

      Column {
        x: stripe.x + stripe.width + Style.spacing.lg
        y: Style.spacing.lg
        width: card.width - x - Style.spacing.lg
        spacing: Style.spacing.xs

        Text {
          width: parent.width
          textFormat: Text.PlainText
          text: [card.modelData.id, card.modelData.risk].filter(function(p) { return p !== "" }).join(" · ")
          color: root.muted
          font.family: root.fontFamily
          font.pixelSize: Style.font.caption
          elide: Text.ElideRight
        }

        Text {
          width: parent.width
          textFormat: Text.PlainText
          text: card.modelData.title
          color: root.foreground
          font.family: root.fontFamily
          font.pixelSize: Style.font.body
          elide: Text.ElideRight
        }

        Item {
          width: parent.width
          height: Math.max(steps.implicitHeight, Style.spacing.sm)

          Rectangle {
            id: track
            anchors.left: parent.left
            anchors.right: steps.left
            anchors.rightMargin: Style.spacing.md
            anchors.verticalCenter: parent.verticalCenter
            height: Math.max(2, Style.spacing.xs)
            color: root.faint

            Rectangle {
              width: track.width * card.modelData.progress
              height: parent.height
              color: root.accent
            }
          }

          Text {
            id: steps
            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter
            width: Math.min(implicitWidth, parent.width * 0.7)
            textFormat: Text.PlainText
            text: card.modelData.stepsText + " · " + card.modelData.agent
            color: root.muted
            font.family: root.fontFamily
            font.pixelSize: Style.font.caption
            elide: Text.ElideRight
          }
        }
      }
    }
  }
}
