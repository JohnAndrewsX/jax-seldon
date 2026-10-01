import QtQuick
import qs.Commons
import "../../Model.js" as Model

// RiskDonut (SPEC-PLUGIN §6): cases by risk class R0–R3 as shares of a ring,
// clockwise from 12 o'clock, the case count in the centre, a legend with
// counts and shares beside it. R0–R2 are steps of the theme accent, R3 the
// theme's urgent colour. series.risk has no dates, so the donut is all time
// whatever the period, and says so. Hover (ring or legend): the class, its
// count and share. Data: Model.riskChart.
ChartCanvas {
  id: root

  chartId: "riskDonut"

  readonly property real legendRowH: Style.font.caption + Style.spacing.md
  readonly property real legendW: Style.font.caption * 11
  readonly property bool legendBeside: root.plot.width - root.legendW >= Style.space(60)
  readonly property real ringArea: root.legendBeside ? root.plot.width - root.legendW : root.plot.width
  readonly property real outer: Math.max(4, Math.min(root.ringArea, root.plot.height) / 2 - Style.spacing.xs)
  readonly property real inner: root.outer * 0.6
  readonly property real cx: root.ringArea / 2
  readonly property real cy: root.plot.height / 2
  readonly property real legendX: root.ringArea + Style.spacing.md
  readonly property real legendY: root.cy - 2 * root.legendRowH
  readonly property var partColors: [Util.alpha(root.accent, 0.3), Util.alpha(root.accent, 0.6), root.accent, root.urgent]

  onPaintRequested: function(ctx, w, h) {
    var parts = root.chart.parts
    for (var i = 0; i < parts.length; i++) {
      if (parts[i].count === 0) continue
      var a0 = -Math.PI / 2 + parts[i].from * 2 * Math.PI
      var a1 = -Math.PI / 2 + parts[i].to * 2 * Math.PI
      ctx.fillStyle = root.partColors[i]
      ctx.beginPath()
      ctx.arc(root.cx, root.cy, root.outer, a0, a1, false)
      ctx.arc(root.cx, root.cy, root.inner, a1, a0, true)
      ctx.closePath()
      ctx.fill()
    }

    ctx.textAlign = "center"
    ctx.textBaseline = "bottom"
    ctx.fillStyle = root.foreground
    ctx.font = Style.font.title + "px \"" + root.fontFamily + "\""
    ctx.fillText(String(root.chart.total), root.cx, root.cy + Style.spacing.xs)
    ctx.font = root.canvasFont
    ctx.textBaseline = "top"
    ctx.fillStyle = root.muted
    ctx.fillText("all time", root.cx, root.cy + Style.spacing.xs)
    ctx.textAlign = "left"

    if (!root.legendBeside) return
    ctx.textBaseline = "middle"
    var box = Math.max(4, Style.font.caption - Style.spacing.xs)
    for (var k = 0; k < parts.length; k++) {
      var y = root.legendY + (k + 0.5) * root.legendRowH
      ctx.fillStyle = root.partColors[k]
      ctx.fillRect(root.legendX, y - box / 2, box, box)
      ctx.fillStyle = parts[k].count > 0 ? root.foreground : root.muted
      ctx.fillText(parts[k].risk + "  " + parts[k].count + "  " + Math.round(parts[k].share * 100) + "%",
        root.legendX + box + Style.spacing.sm, y)
    }
  }

  // Items: R0–R3, at the middle of their arc (empty classes have none).
  onLocateRequested: function(index) {
    var parts = root.chart.parts
    var p = parts[index < 0 ? parts.length + index : index]
    if (!p || p.count === 0) return
    var a = -Math.PI / 2 + (p.from + p.to) * Math.PI
    var r = (root.inner + root.outer) / 2
    root.located = Qt.point(root.cx + r * Math.cos(a), root.cy + r * Math.sin(a))
  }

  onHoverRequested: function(x, y) {
    var i = -1
    if (root.legendBeside && x >= root.legendX) {
      var row = Math.floor((y - root.legendY) / root.legendRowH)
      if (row >= 0 && row < root.chart.parts.length) i = row
    } else {
      var dx = x - root.cx
      var dy = y - root.cy
      var r = Math.sqrt(dx * dx + dy * dy)
      if (r >= root.inner && r <= root.outer)
        i = Model.riskPartAt(root.chart, (Math.atan2(dy, dx) + Math.PI / 2) / (2 * Math.PI))
    }
    if (i < 0) {
      root.clearHover()
      return
    }
    root.hoverText = Model.riskPartText(root.chart.parts[i])
    root.highlight = root.legendBeside
      ? Qt.rect(root.legendX - Style.spacing.xs, root.legendY + i * root.legendRowH, root.legendW - Style.spacing.md, root.legendRowH)
      : Qt.rect(root.cx - root.outer, root.cy - root.outer, 2 * root.outer, 2 * root.outer)
  }
}
