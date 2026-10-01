import QtQuick
import qs.Commons
import "../../Model.js" as Model

// DriftBars (SPEC-PLUGIN §6): drift items opened (accent) and resolved
// (foreground) per ISO week as paired bars on one count axis, the highest
// count on top. Week labels under the bars where they fit, a legend on the
// top line. Hover: the week, its dates and both counts. Data:
// Model.driftChart.
ChartCanvas {
  id: root

  chartId: "driftBars"

  readonly property real labelH: Style.font.caption + Style.spacing.xs
  readonly property real axisW: Style.font.caption * 3
  readonly property real plotX: root.axisW
  readonly property real plotW: Math.max(1, root.plot.width - root.axisW)
  readonly property real plotTop: root.labelH + Style.spacing.xs
  readonly property real plotBottom: Math.max(root.plotTop + 1, root.plot.height - root.labelH)
  readonly property int weekCount: root.chart && !root.empty ? root.chart.weeks.length : 0
  readonly property real slotW: root.weekCount > 0 ? root.plotW / root.weekCount : 0
  readonly property color openedColor: root.accent
  readonly property color resolvedColor: Util.alpha(root.foreground, 0.45)

  function yOf(value) {
    return Model.scale(value, 0, Math.max(1, root.chart.max), root.plotBottom, root.plotTop)
  }

  onPaintRequested: function(ctx, w, h) {
    var weeks = root.chart.weeks
    var max = Math.max(1, root.chart.max)

    // Count axis: 0 and the highest count, with faint rules.
    ctx.fillStyle = root.faint
    ctx.fillRect(root.plotX, root.yOf(max), root.plotW, 1)
    ctx.fillRect(root.plotX, root.plotBottom, root.plotW, 1)
    ctx.fillStyle = root.muted
    ctx.textAlign = "right"
    ctx.textBaseline = "middle"
    ctx.fillText(String(max), root.axisW - Style.spacing.sm, root.yOf(max))
    ctx.fillText("0", root.axisW - Style.spacing.sm, root.plotBottom)
    ctx.textAlign = "left"

    var inner = root.slotW * 0.8
    var barW = Math.max(1, inner / 2)
    for (var i = 0; i < weeks.length; i++) {
      var x = root.plotX + i * root.slotW + (root.slotW - inner) / 2
      ctx.fillStyle = root.openedColor
      ctx.fillRect(x, root.yOf(weeks[i].opened), barW, root.plotBottom - root.yOf(weeks[i].opened))
      ctx.fillStyle = root.resolvedColor
      ctx.fillRect(x + barW, root.yOf(weeks[i].resolved), barW, root.plotBottom - root.yOf(weeks[i].resolved))
    }

    // Week labels ("W40") under the bars, every n-th so they do not touch.
    ctx.fillStyle = root.muted
    ctx.textBaseline = "bottom"
    ctx.textAlign = "center"
    var labelW = root.textWidth(ctx, "W00") + Style.spacing.md
    var every = Math.max(1, Math.ceil(labelW / Math.max(1, root.slotW)))
    for (var k = weeks.length - 1; k >= 0; k -= every)
      ctx.fillText(weeks[k].week.slice(5), root.plotX + (k + 0.5) * root.slotW, h)

    // Legend: ■ opened ■ resolved, right-aligned on the top line.
    ctx.textAlign = "left"
    ctx.textBaseline = "top"
    var box = Math.max(4, Style.font.caption - Style.spacing.xs)
    var gap = Style.spacing.xs
    var items = [["opened", root.openedColor], ["resolved", root.resolvedColor]]
    var legendW = 0
    for (var a = 0; a < items.length; a++) legendW += box + gap + root.textWidth(ctx, items[a][0]) + Style.spacing.md
    var lx = Math.max(root.plotX, w - legendW + Style.spacing.md)
    for (var b = 0; b < items.length; b++) {
      ctx.fillStyle = items[b][1]
      ctx.fillRect(lx, 1, box, box)
      lx += box + gap
      ctx.fillStyle = root.muted
      ctx.fillText(items[b][0], lx, 0)
      lx += root.textWidth(ctx, items[b][0]) + Style.spacing.md
    }
  }

  // Items: the weeks, oldest first.
  onLocateRequested: function(index) {
    var n = root.weekCount
    var i = index < 0 ? n + index : index
    if (i >= 0 && i < n) root.located = Qt.point(root.plotX + (i + 0.5) * root.slotW, (root.plotTop + root.plotBottom) / 2)
  }

  onHoverRequested: function(x, y) {
    var i = Math.floor((x - root.plotX) / Math.max(1, root.slotW))
    if (x < root.plotX || i < 0 || i >= root.weekCount) {
      root.clearHover()
      return
    }
    root.hoverText = Model.driftWeekText(root.chart.weeks[i])
    root.highlight = Qt.rect(root.plotX + i * root.slotW, root.plotTop, root.slotW, root.plotBottom - root.plotTop + 1)
  }
}
