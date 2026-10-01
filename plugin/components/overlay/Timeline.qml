import QtQuick
import qs.Commons
import "../../Model.js" as Model

// Timeline (SPEC-PLUGIN §6): one time axis over the period. The top band
// carries the markers — Omarchy releases (accent diamonds), snapshots
// (foreground dots) and crises (urgent diamonds) — and the lanes under it
// the case spans from created to closed (open cases, in accent, run to
// the end of today; closed ones are dimmer). The period's first and last
// day and the month starts sit under the axis. Hover: the item's kind,
// label and date. Data: Model.timelineChart (lanes packed there).
ChartCanvas {
  id: root

  chartId: "timeline"
  onCountRequested: root.ownCount = Model.aggregationCount()

  readonly property real labelH: Style.font.caption + Style.spacing.xs
  readonly property real band: Math.max(Style.space(14), Style.font.caption + Style.spacing.sm)
  readonly property real laneArea: Math.max(0, root.plot.height - root.labelH)
  readonly property var layout: Model.timelineLayout(root.laneArea, root.chart && !root.empty ? root.chart.lanes : 0,
    root.band, Style.space(14))
  readonly property real markerR: Math.max(3, Style.space(5))
  readonly property color closedColor: Util.alpha(root.accent, 0.4)
  readonly property color snapshotColor: Util.alpha(root.foreground, 0.7)

  function xOf(day) {
    return Model.scale(day, root.chart.x0, root.chart.x1, 0, root.plot.width)
  }

  function markerColor(kind) {
    return kind === "crisis" ? root.urgent : kind === "release" ? root.accent : root.snapshotColor
  }

  onPaintRequested: function(ctx, w, h) {
    var L = root.layout
    var c = root.chart

    // Axis and month starts.
    ctx.fillStyle = root.faint
    ctx.fillRect(0, L.bandY, w, 1)
    ctx.fillRect(0, root.laneArea, w, 1)
    ctx.fillStyle = root.muted
    ctx.textBaseline = "bottom"
    var startLabel = Model.dateOfDay(c.x0)
    var endLabel = Model.dateOfDay(c.x1 - 1)
    var free = root.textWidth(ctx, startLabel) + Style.spacing.lg
    var stop = w - root.textWidth(ctx, endLabel) - Style.spacing.lg
    ctx.fillText(startLabel, 0, h)
    ctx.textAlign = "right"
    ctx.fillText(endLabel, w, h)
    ctx.textAlign = "left"
    for (var d = 0; d < c.months.length; d++) {
      var mx = root.xOf(c.months[d].day)
      ctx.fillStyle = root.faint
      ctx.fillRect(mx, 0, 1, root.laneArea)
      var label = c.months[d].label
      if (mx >= free && mx + root.textWidth(ctx, label) <= stop) {
        ctx.fillStyle = root.muted
        ctx.fillText(label, mx, h)
        free = mx + root.textWidth(ctx, label) + Style.spacing.lg
      }
    }

    // Case spans, one row per lane.
    var barH = Math.max(1, L.laneH - Math.max(1, Style.spacing.xs))
    for (var s = 0; s < c.spans.length; s++) {
      var sp = c.spans[s]
      var x0 = root.xOf(sp.x0)
      ctx.fillStyle = sp.open ? root.accent : root.closedColor
      ctx.fillRect(x0, L.laneY0 + sp.lane * L.laneH, Math.max(2, root.xOf(sp.x1) - x0), barH)
    }

    // Markers: snapshots first, then releases, crises on top.
    var order = ["snapshot", "release", "crisis"]
    var r = root.markerR
    for (var o = 0; o < order.length; o++) {
      ctx.fillStyle = root.markerColor(order[o])
      for (var m = 0; m < c.markers.length; m++) {
        var mk = c.markers[m]
        if (mk.kind !== order[o]) continue
        var x = root.xOf(mk.x)
        ctx.beginPath()
        if (mk.kind === "snapshot") {
          ctx.arc(x, L.bandY, r * 0.7, 0, 2 * Math.PI, false)
        } else {
          ctx.moveTo(x, L.bandY - r)
          ctx.lineTo(x + r, L.bandY)
          ctx.lineTo(x, L.bandY + r)
          ctx.lineTo(x - r, L.bandY)
          ctx.closePath()
        }
        ctx.fill()
      }
    }
  }

  // Items: the markers in index order, then the case spans.
  onLocateRequested: function(index) {
    var c = root.chart
    var n = c.markers.length + c.spans.length
    var i = index < 0 ? n + index : index
    if (i < 0 || i >= n) return
    if (i < c.markers.length) {
      root.located = Qt.point(root.xOf(c.markers[i].x), root.layout.bandY)
    } else {
      var sp = c.spans[i - c.markers.length]
      root.located = Qt.point((root.xOf(sp.x0) + root.xOf(sp.x1)) / 2, root.layout.laneY0 + (sp.lane + 0.5) * root.layout.laneH)
    }
  }

  onHoverRequested: function(x, y) {
    var hit = Model.timelineItemAt(root.chart, root.layout, root.plot.width, x, y, root.markerR + Style.spacing.xs)
    if (!hit) {
      root.clearHover()
      return
    }
    var L = root.layout
    var r = root.markerR
    if (hit.kind === "marker") {
      var mk = root.chart.markers[hit.index]
      var mx = root.xOf(mk.x)
      root.hoverText = Model.timelineItemText(mk)
      root.highlight = Qt.rect(mx - r - 2, L.bandY - r - 2, 2 * r + 4, 2 * r + 4)
    } else {
      var sp = root.chart.spans[hit.index]
      var x0 = root.xOf(sp.x0)
      root.hoverText = Model.timelineItemText(sp)
      root.highlight = Qt.rect(x0 - 1, L.laneY0 + sp.lane * L.laneH - 1, Math.max(2, root.xOf(sp.x1) - x0) + 2, L.laneH + 1)
    }
  }
}
