pragma ComponentBehavior: Bound

import QtQuick
import qs.Commons
import qs.Ui
import "../components/desk"
import "../components/overlay"
import "../Model.js" as Model

// Section 7, Prime Radiant (ADR-0034 §4, SPEC-PLUGIN §6; WP-123): the 0.1
// overlay's six charts inside the desk, solo (no list column). On top the
// title, the period's window and the period selector (30 d / 90 d / 365 d /
// All; 90 d on every entry of the section unless a payload names one),
// then the six slots on the 12-column grid of Model.overlayGrid at the
// section's own size: Heatmap | Series DriftBars RiskDonut | Timeline |
// The Plan (wide), fewer columns as the desk narrows, scrolling only when
// the minimum heights do not fit. The charts (components/overlay/) are the
// overlay's, unchanged: each draws its entry of the service's precomputed
// period table (Model.periodTable → Model.periodView) and shows its
// summary, or the hovered item, as the caption in its slot's title row.
//
// Cheap to enter: the service aggregates when the index changes; this
// file and the charts only look up and draw — no aggregation on the first
// frame, one paint per chart per data or size change (the harness counts
// both through view()).
//
// Keys: ←/→ and h/l the previous / next period (wrapping); the digits are
// the desk's sections. IPC through the desk: `setPeriod <id>`, `hover
// "<slot> <x>,<y>"` (Desk.qml).
Section {
  id: root

  // The selected period (a Model.PERIODS id).
  property string period: Model.PERIOD_DEFAULT
  // A payload's period for the next entry of the section ("" none).
  property string entryPeriod: ""

  readonly property var periodData: Model.periodView(root.service ? root.service.periods : null, root.period)
  readonly property var periodOptions: Model.PERIODS.map(function(p) { return { value: p.id, label: p.label } })
  readonly property real pad: Style.spacing.huge
  readonly property var grid: Model.overlayGrid(gridArea.width, gridArea.height, Style.spacing.panelGap,
    Style.space(240), Style.space(120))
  readonly property color foreground: Color.popups.text
  readonly property string fontFamily: Style.font.family
  // The Timeline's legend (A12): the markers in the colours its canvas
  // uses (Timeline.markerColor), shown in the slot's title row.
  readonly property var timelineLegend: Model.TIMELINE_LEGEND.map(function(e) {
    return {
      files: e.markers.map(function(k) { return Model.markerFile(k, Style.space(12)) }),
      label: e.label,
      color: e.tone === "urgent" ? Color.urgent : e.tone === "snapshot" ? Util.alpha(root.foreground, 0.7) : Color.accent
    }
  })

  // Returns the period now selected; an unknown id changes nothing.
  function setPeriod(id) {
    var value = String(id)
    if (Model.isPeriod(value)) root.period = value
    return root.period
  }

  function moveAcross(dx) {
    root.period = Model.cyclePeriod(root.period, dx)
    return true
  }

  function textKey(t) {
    if (t === "h" || t === "l") return root.moveAcross(t === "h" ? -1 : 1)
    return false
  }

  // ↑/↓ scroll a grid that does not fit.
  function move(dy) {
    if (gridArea.contentHeight > gridArea.height)
      gridArea.contentY = Math.max(0, Math.min(gridArea.contentHeight - gridArea.height, gridArea.contentY + dy * Style.space(120)))
    return true
  }

  function applyPayload(payload) {
    if (!payload || payload.period === "") return
    if (root.active) root.setPeriod(payload.period)
    else root.entryPeriod = payload.period
  }

  function summaryFor(id) {
    var slots = root.periodData.slots
    for (var i = 0; i < slots.length; i++) if (slots[i].id === id) return slots[i]
    return null
  }

  function rectFor(id) {
    var slots = root.grid.slots
    for (var i = 0; i < slots.length; i++) if (slots[i].id === id) return slots[i]
    return { x: 0, y: 0, w: 0, h: 0 }
  }

  // The chart in slot `id`, or null.
  function chartFor(id) {
    for (var i = 0; i < slotRepeater.count; i++) {
      var item = slotRepeater.itemAt(i) as OverlaySlot
      if (item && item.summary && item.summary.id === id) return item.chart
    }
    return null
  }

  // Aggregation passes of this file's and the charts' Model.js instances.
  function aggregationCount() {
    var n = Model.aggregationCount()
    for (var i = 0; i < slotRepeater.count; i++) {
      var item = slotRepeater.itemAt(i) as OverlaySlot
      if (item && item.chart) n += item.chart.aggregationCount()
    }
    return n
  }

  // "series 0.5,0.5" → the read-out at that point of the chart's plot
  // (fractions in [0, 1]), as JSON { slot, hover }; "" clears every chart's
  // hover. Anything else (no such slot, a malformed or out-of-range point)
  // changes nothing and returns JSON { error }.
  function hover(arg) {
    var text = String(arg).trim()
    if (text === "") {
      for (var i = 0; i < slotRepeater.count; i++) {
        var item = slotRepeater.itemAt(i) as OverlaySlot
        if (item && item.chart) item.chart.clearHover()
      }
      return JSON.stringify({ slot: "", hover: "" })
    }
    var usage = "expected \"<slot> <x>,<y>\" with x and y in [0, 1], or \"\""
    var m = /^(\w+)\s+(\d+(?:\.\d+)?|\.\d+),(\d+(?:\.\d+)?|\.\d+)$/.exec(text)
    if (!m) return JSON.stringify({ error: usage })
    var chart = root.chartFor(m[1])
    if (!chart) return JSON.stringify({ error: "no chart " + m[1] })
    var read = chart.probe(Number(m[2]), Number(m[3]))
    if (read === null) return JSON.stringify({ error: usage })
    return JSON.stringify({ slot: m[1], hover: read })
  }

  // The section's read-out (Desk.view's sectionView): period and window,
  // grid mode, aggregation passes, and each slot's counts, geometry in
  // window coordinates and chart (summary, numbers, empty, hover, paints,
  // paintMs, plot size).
  function view() {
    var slots = []
    for (var i = 0; i < slotRepeater.count; i++) {
      var item = slotRepeater.itemAt(i) as OverlaySlot
      if (!item || !item.summary) continue
      var at = item.mapToItem(null, 0, 0)
      var s = item.summary
      var c = item.chart
      slots.push({ id: s.id, title: s.title, rows: s.rows, count: s.count, detail: s.detail, windowed: s.windowed,
        x: Math.round(at.x), y: Math.round(at.y), w: Math.round(item.width), h: Math.round(item.height),
        chart: c ? { summary: c.summary, numbers: c.numbers, empty: c.empty, hover: c.hoverText, paints: c.paints, paintMs: c.paintMs,
          w: Math.round(c.plot.width), h: Math.round(c.plot.height) } : null })
    }
    var origin = gridArea.mapToItem(null, 0, 0)
    return {
      period: root.period,
      window: root.periodData.window,
      caption: caption.text,
      mode: root.grid.mode,
      scrolls: gridArea.contentHeight > gridArea.height,
      area: { x: Math.round(origin.x), y: Math.round(origin.y), w: Math.round(gridArea.width), h: Math.round(gridArea.height) },
      aggregations: { service: root.service ? root.service.aggregationCount() : 0, section: root.aggregationCount() },
      slots: slots
    }
  }

  // Every entry of the section starts on 90 d, or on the payload's period.
  onActiveChanged: if (root.active) {
    root.period = root.entryPeriod !== "" ? root.entryPeriod : Model.PERIOD_DEFAULT
    root.entryPeriod = ""
  }

  // The charts, each bound to its precomputed entry for the period.
  Component {
    id: heatmapChart
    Heatmap { chart: root.periodData.charts.heatmap; foreground: root.foreground; fontFamily: root.fontFamily }
  }
  Component {
    id: seriesChart
    Series { chart: root.periodData.charts.series; foreground: root.foreground; fontFamily: root.fontFamily }
  }
  Component {
    id: driftChart
    DriftBars { chart: root.periodData.charts.driftBars; foreground: root.foreground; fontFamily: root.fontFamily }
  }
  Component {
    id: riskChart
    RiskDonut { chart: root.periodData.charts.riskDonut; foreground: root.foreground; fontFamily: root.fontFamily }
  }
  Component {
    id: timelineChart
    Timeline { chart: root.periodData.charts.timeline; foreground: root.foreground; fontFamily: root.fontFamily }
  }
  Component {
    id: planChart
    ThePlan { chart: root.periodData.charts.plan; foreground: root.foreground; fontFamily: root.fontFamily }
  }

  // Row 1: title and window on the left, the periods on the right. Its
  // height comes from the texts and the selector directly, not from a
  // Column's polish one frame late, which would resize the charts after
  // their first paint (WP-031).
  Item {
    id: head
    x: root.pad
    y: root.pad
    width: Math.max(0, root.width - root.pad * 2)
    height: Math.max(titleText.implicitHeight + Style.spacing.xs + caption.implicitHeight, periods.implicitHeight)

    Text {
      id: titleText
      width: Math.max(0, parent.width - periods.implicitWidth - Style.spacing.panelGap)
      textFormat: Text.PlainText
      text: "Prime Radiant"
      color: root.foreground
      elide: Text.ElideRight
      font.family: root.fontFamily
      font.pixelSize: Style.font.title
      font.bold: true
    }

    Text {
      id: caption
      y: titleText.implicitHeight + Style.spacing.xs
      width: titleText.width
      textFormat: Text.PlainText
      text: Model.periodCaption(root.periodData.window)
      color: Color.muted
      elide: Text.ElideRight
      font.family: root.fontFamily
      font.pixelSize: Style.font.bodySmall
    }

    ButtonGroup {
      id: periods
      anchors.right: parent.right
      anchors.verticalCenter: parent.verticalCenter
      options: root.periodOptions
      value: root.period
      focusable: false
      foreground: root.foreground
      fontFamily: root.fontFamily
      onChanged: function(value) {
        root.setPeriod(value)
        if (root.desk) root.desk.giveKeys()
      }
    }
  }

  Flickable {
    id: gridArea
    x: root.pad
    y: head.y + head.height + Style.spacing.panelGap
    width: Math.max(0, root.width - root.pad * 2)
    height: Math.max(0, root.height - y - root.pad)
    contentWidth: width
    contentHeight: root.grid.contentHeight
    interactive: contentHeight > height
    boundsBehavior: Flickable.StopAtBounds
    clip: true

    Repeater {
      id: slotRepeater
      model: Model.OVERLAY_SLOTS

      OverlaySlot {
        id: slot

        required property var modelData
        readonly property var rect: root.rectFor(modelData.id)

        objectName: "slot:" + modelData.id
        x: rect.x
        y: rect.y
        width: rect.w
        height: rect.h
        summary: root.summaryFor(modelData.id)
        legend: modelData.id === "timeline" ? root.timelineLegend : []
        foreground: root.foreground
        fontFamily: root.fontFamily
        placeholder: false
        chart: chartLoader.item as ChartCanvas

        Loader {
          id: chartLoader
          anchors.fill: parent
          sourceComponent: slot.modelData.id === "heatmap" ? heatmapChart
            : slot.modelData.id === "series" ? seriesChart
            : slot.modelData.id === "driftBars" ? driftChart
            : slot.modelData.id === "riskDonut" ? riskChart
            : slot.modelData.id === "timeline" ? timelineChart
            : planChart
        }
      }
    }
  }
}
