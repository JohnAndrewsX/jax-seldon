import QtQuick
import qs.Commons
import "../../Model.js" as Model

// Series (SPEC-PLUGIN §6): explicit and total package counts as step lines
// over the period, one lane each (they differ by an order of magnitude, so
// they share the time axis, not the value axis). Each lane names its
// series and latest count on the left and its range on the right; the
// period's first and last day sit under the axis. Hover: the sample that
// holds at the pointer, with both counts. Data: Model.seriesChart.
ChartCanvas {
  id: root

  chartId: "series"
  onCountRequested: root.ownCount = Model.aggregationCount()

  readonly property real labelH: Style.font.caption + Style.spacing.xs
  readonly property real valueW: Style.font.caption * 5
  readonly property real laneGap: Style.spacing.md
  readonly property int laneCount: root.chart && !root.empty ? root.chart.lanes.length : 0
  readonly property real plotW: Math.max(1, root.plot.width - root.valueW)
  readonly property real laneH: root.laneCount > 0
    ? Math.max(1, (root.plot.height - root.labelH - (root.laneCount - 1) * root.laneGap) / root.laneCount) : 0
  readonly property var lineColors: [root.accent, Util.alpha(root.foreground, 0.7)]

  function xOf(day) {
    return Model.scale(day, root.chart.x0, root.chart.x1, 0, root.plotW)
  }

  // The lane's value range inside its row, under the lane's label line.
  function yOf(lane, index, value) {
    var top = index * (root.laneH + root.laneGap) + root.labelH
    var bottom = index * (root.laneH + root.laneGap) + root.laneH - Style.spacing.xs
    return Model.scale(value, lane.lo, lane.hi, bottom, top)
  }

  onPaintRequested: function(ctx, w, h) {
    var points = root.chart.points
    var lanes = root.chart.lanes
    var dot = Math.max(2, Style.spacing.xs)
    for (var l = 0; l < lanes.length; l++) {
      var lane = lanes[l]
      var top = l * (root.laneH + root.laneGap)
      ctx.fillStyle = root.faint
      ctx.fillRect(0, top + root.laneH - 1, root.plotW, 1)

      ctx.fillStyle = root.muted
      ctx.textBaseline = "top"
      ctx.fillText(lane.label + " " + lane.last, 0, top)
      ctx.textAlign = "right"
      ctx.fillText(String(lane.max), w, top + root.labelH)
      ctx.textBaseline = "bottom"
      ctx.fillText(String(lane.min), w, top + root.laneH)
      ctx.textAlign = "left"

      // Step line: a count holds until the next sample, the last one to
      // the end of the period.
      ctx.strokeStyle = root.lineColors[l]
      ctx.lineWidth = Math.max(1.5, Style.space(2))
      ctx.beginPath()
      var prev = null
      for (var i = 0; i < points.length; i++) {
        var v = points[i][lane.key]
        if (v === null) continue
        var x = root.xOf(points[i].day)
        if (prev === null) ctx.moveTo(x, root.yOf(lane, l, v))
        else ctx.lineTo(x, root.yOf(lane, l, prev))
        ctx.lineTo(x, root.yOf(lane, l, v))
        prev = v
      }
      ctx.lineTo(root.plotW, root.yOf(lane, l, prev))
      ctx.stroke()

      ctx.fillStyle = root.lineColors[l]
      for (var p = 0; p < points.length; p++) {
        if (points[p][lane.key] === null) continue
        ctx.fillRect(root.xOf(points[p].day) - dot, root.yOf(lane, l, points[p][lane.key]) - dot, 2 * dot, 2 * dot)
      }
    }

    // The period's first and last day under the lanes.
    ctx.fillStyle = root.muted
    ctx.textBaseline = "bottom"
    ctx.fillText(Model.dateOfDay(root.chart.x0), 0, h)
    ctx.textAlign = "right"
    ctx.fillText(Model.dateOfDay(root.chart.x1 - 1), root.plotW, h)
    ctx.textAlign = "left"
  }

  // Items: the samples, oldest first (a point just after the sample).
  onLocateRequested: function(index) {
    var points = root.chart.points
    var p = points[index < 0 ? points.length + index : index]
    if (p) root.located = Qt.point(Math.min(root.plotW - 1, root.xOf(p.day) + 1), root.laneH / 2)
  }

  onHoverRequested: function(x, y) {
    if (x > root.plotW) {
      root.clearHover()
      return
    }
    var day = Model.scale(x, 0, root.plotW, root.chart.x0, root.chart.x1)
    var i = Model.seriesPointAt(root.chart, day)
    if (i < 0) {
      root.clearHover()
      return
    }
    var p = root.chart.points[i]
    root.hoverText = Model.seriesPointText(p)
    var px = root.xOf(p.day)
    root.highlight = Qt.rect(px - 1, 0, 2, root.laneCount * (root.laneH + root.laneGap) - root.laneGap)
  }
}
