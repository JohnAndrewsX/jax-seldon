import QtQuick
import qs.Commons
import "../../Model.js" as Model

// Heatmap (SPEC-PLUGIN §6): events per day of the period as a calendar grid,
// ISO weeks as columns, Monday on top, five colour steps of the theme accent
// (Model.CHART_STEP_ALPHAS through Util.alpha) and a faint cell for days
// without events. Month names above, Mon/Wed/Fri on the left, a less–more
// legend beside the grid when it fits. Hover: the day and its counts by
// source. The cells come from Model.heatmapChart (precomputed).
ChartCanvas {
  id: root

  chartId: "heatmap"
  onCountRequested: root.ownCount = Model.aggregationCount()

  readonly property real labelW: Style.font.caption * 3
  readonly property real labelH: Style.font.caption + Style.spacing.sm
  readonly property var layout: Model.heatmapLayout(root.plot.width, root.plot.height, root.chart ? root.chart.weeks : 0,
    root.labelW, root.labelH)
  readonly property var stepColors: [Util.alpha(root.foreground, Model.CHART_ZERO_ALPHA)].concat(
    Model.CHART_STEP_ALPHAS.map(function(a) { return Util.alpha(root.accent, a) }))

  onPaintRequested: function(ctx, w, h) {
    var L = root.layout
    var cells = root.chart.cells
    for (var i = 0; i < cells.length; i++) {
      var c = cells[i]
      ctx.fillStyle = root.stepColors[c.step]
      ctx.fillRect(L.x0 + c.col * L.pitch, L.y0 + c.row * L.pitch, L.cell, L.cell)
    }

    ctx.fillStyle = root.muted
    ctx.textBaseline = "middle"
    var days = [[0, "Mon"], [2, "Wed"], [4, "Fri"]]
    for (var d = 0; d < days.length; d++)
      ctx.fillText(days[d][1], 0, L.y0 + days[d][0] * L.pitch + L.cell / 2)

    // Month names where a month starts, skipped where they would collide.
    ctx.textBaseline = "top"
    var free = L.x0
    var months = root.chart.months
    for (var m = 0; m < months.length; m++) {
      var x = L.x0 + months[m].col * L.pitch
      if (x < free) continue
      ctx.fillText(months[m].label, x, 0)
      free = x + root.textWidth(ctx, months[m].label) + Style.spacing.md
    }

    // Legend: less ▪▪▪▪▪▪ more, beside the grid's top row when there is
    // room, else right-aligned on the month row, else left out.
    var box = Math.max(4, Math.min(L.cell, root.labelH - Style.spacing.xs))
    var less = root.textWidth(ctx, "less")
    var more = root.textWidth(ctx, "more")
    var gap = Style.spacing.xs
    var legendW = less + more + 2 * gap + root.stepColors.length * (box + gap)
    var beside = L.x0 + L.width + Style.spacing.xxl
    var lx = beside + legendW <= w ? beside : w - legendW >= free ? w - legendW : -1
    if (lx < 0) return
    var ly = lx === beside ? L.y0 + (L.cell - box) / 2 : 0
    ctx.textBaseline = "middle"
    ctx.fillStyle = root.muted
    ctx.fillText("less", lx, ly + box / 2)
    lx += less + gap
    for (var s = 0; s < root.stepColors.length; s++) {
      ctx.fillStyle = root.stepColors[s]
      ctx.fillRect(lx, ly, box, box)
      lx += box + gap
    }
    ctx.fillStyle = root.muted
    ctx.fillText("more", lx, ly + box / 2)
  }

  // Items: the cells, oldest day first.
  onLocateRequested: function(index) {
    var cells = root.chart.cells
    var c = cells[index < 0 ? cells.length + index : index]
    if (c) root.located = Qt.point(root.layout.x0 + (c.col + 0.5) * root.layout.pitch, root.layout.y0 + (c.row + 0.5) * root.layout.pitch)
  }

  onHoverRequested: function(x, y) {
    var i = Model.heatmapCellAt(root.chart, root.layout, x, y)
    if (i < 0) {
      root.clearHover()
      return
    }
    var c = root.chart.cells[i]
    var L = root.layout
    root.hoverText = Model.heatmapCellText(c)
    root.highlight = Qt.rect(L.x0 + c.col * L.pitch - 1, L.y0 + c.row * L.pitch - 1, L.cell + 2, L.cell + 2)
  }
}
