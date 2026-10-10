import QtQuick
import qs.Commons
import "../../Model.js" as Model

// Timeline (SPEC-PLUGIN §6): one time axis over the period. The top band
// carries the markers — Omarchy releases (accent diamonds), snapshots
// (foreground dots) and crises (urgent spindles), the A12 shapes from
// Model.MARKER_PATHS — and the lanes under it the case spans from created
// to closed, each between brackets: `[` at the start, `]` at the close
// (open cases, in accent, run to the end of today without one; closed ones
// are dimmer). The slot's title row carries the legend (Overlay.qml). The period's first and last
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

  // One A12 marker centred on (x, y), its 16-unit grid scaled so the
  // release diamond (12 units wide) is 2 r across.
  function drawMarker(ctx, kind, x, y, r) {
    var u = 2 * r / 12
    ctx.save()
    ctx.translate(x - 8 * u, y - 8 * u)
    ctx.scale(u, u)
    ctx.beginPath()
    ctx.path = Model.MARKER_PATHS[kind]
    ctx.fill()
    ctx.restore()
  }

  // A case span: brackets the lane's bar height tall at its ends (2 of 16
  // units thick, arms 6 of 16, as the A12 files), a bar a third of that
  // between them; an open span has no closing bracket. Too short for both
  // brackets, it stays a plain bar.
  function drawSpan(ctx, x0, x1, y, h, open) {
    var w = Math.max(2, x1 - x0)
    var t = Math.max(1, Math.round(h / 6))
    var arm = Math.max(2 * t, Math.round(h * 3 / 8))
    if (w < 2 * arm + 2) {
      ctx.fillRect(x0, y, w, h)
      return
    }
    var mid = Math.max(1, Math.round(h / 3))
    ctx.fillRect(x0, y + Math.round((h - mid) / 2), w, mid)
    ctx.fillRect(x0, y, t, h)
    ctx.fillRect(x0, y, arm, t)
    ctx.fillRect(x0, y + h - t, arm, t)
    if (open) return
    ctx.fillRect(x0 + w - t, y, t, h)
    ctx.fillRect(x0 + w - arm, y, arm, t)
    ctx.fillRect(x0 + w - arm, y + h - t, arm, t)
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
    ctx.fillStyle = root.dim
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
        ctx.fillStyle = root.dim
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
      root.drawSpan(ctx, x0, root.xOf(sp.x1), L.laneY0 + sp.lane * L.laneH, barH, sp.open)
    }

    // Markers: snapshots first, then releases, crises on top.
    var order = ["snapshot", "release", "crisis"]
    var r = root.markerR
    for (var o = 0; o < order.length; o++) {
      ctx.fillStyle = root.markerColor(order[o])
      for (var m = 0; m < c.markers.length; m++) {
        var mk = c.markers[m]
        if (mk.kind !== order[o]) continue
        root.drawMarker(ctx, mk.kind, root.xOf(mk.x), L.bandY, r)
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
