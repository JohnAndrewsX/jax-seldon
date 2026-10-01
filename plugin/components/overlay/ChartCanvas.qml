pragma ComponentBehavior: Bound

import QtQuick
import qs.Commons
import "../../Model.js" as Model

// The frame every Prime Radiant chart shares (WP-031): a Canvas over the
// chart area, the caption (the hovered item's read-out, else the chart's
// summary; OverlaySlot shows it in the slot's title row), the empty state,
// and the hover plumbing.
//
// `chart` is one entry of the service's precomputed period table
// (Model.periodTable → periods[id].charts[slot]); a chart file only turns it
// into pixels in onPaintRequested and answers onHoverRequested with
// `hoverText` and a `highlight` rectangle. The canvas paints when the chart
// data, its size or the theme colours change — never on hover (the
// highlight is a separate item) — and counts its paints in `paints`.
Item {
  id: root

  property string chartId: ""
  property var chart: null
  property color foreground: Color.popups.text
  property color accent: Color.accent
  property color urgent: Color.urgent
  property color muted: Color.muted
  property string fontFamily: Style.font.family

  // The hovered item's read-out, "" when nothing is hovered (`call view`).
  property string hoverText: ""
  // The hovered item's box in plot coordinates; a zero width hides it.
  property rect highlight: Qt.rect(0, 0, 0, 0)
  // Canvas paints since this chart was created, and the last one's time in
  // ms (the chart's own drawing calls, for the profile note).
  property int paints: 0
  property real paintMs: 0
  // Appended to the summary in the caption (e.g. what did not fit).
  property string captionSuffix: ""
  // The caption line: the hovered item's read-out, else the summary.
  readonly property string caption: root.empty ? "" : root.hoverText !== "" ? root.hoverText : root.summary + root.captionSuffix

  readonly property bool empty: !root.chart || root.chart.empty === true
  readonly property string summary: root.chart ? String(root.chart.summary) : ""
  readonly property var numbers: root.chart ? root.chart.numbers : null
  readonly property alias plot: canvas
  readonly property string canvasFont: Style.font.caption + "px \"" + root.fontFamily + "\""
  readonly property color faint: Util.alpha(root.foreground, 0.12)

  // Paint the chart into ctx, a w × h canvas already cleared.
  signal paintRequested(var ctx, real w, real h)
  // The pointer is at (x, y) in plot coordinates; set hoverText and
  // highlight, or call clearHover().
  signal hoverRequested(real x, real y)
  // Set `located` to the centre of item `index` in plot coordinates (a
  // negative index counts from the end), or leave it null.
  signal locateRequested(int index)
  property var located: null
  // Set `ownCount` to Model.aggregationCount() of the chart file's own
  // Model.js (each importing document gets its own instance per object, so
  // this file cannot read the chart's count).
  signal countRequested()
  property int ownCount: 0

  function hoverAt(x, y) {
    if (root.empty || x < 0 || y < 0 || x > canvas.width || y > canvas.height) root.clearHover()
    else root.hoverRequested(x, y)
  }

  // The read-out at a point given as fractions of the plot (IPC and tests).
  // null (and no change) unless both are finite fractions in [0, 1].
  function probe(fx, fy) {
    var x = Number(fx)
    var y = Number(fy)
    if (!isFinite(x) || !isFinite(y) || x < 0 || x > 1 || y < 0 || y > 1) return null
    root.hoverAt(x * canvas.width, y * canvas.height)
    return root.hoverText
  }

  // Where item `index` is drawn (the chart's own item order, see each chart),
  // as { x, y } in plot coordinates, or null. Tests hover there.
  function locate(index) {
    root.located = null
    if (!root.empty) root.locateRequested(index)
    return root.located
  }

  function clearHover() {
    root.hoverText = ""
    root.highlight = Qt.rect(0, 0, 0, 0)
  }

  // Aggregation passes of this object's Model.js instances (see
  // Model.aggregationCount): the one of this base file and the one of the
  // chart file, which reports it through countRequested.
  function aggregationCount() {
    root.ownCount = 0
    root.countRequested()
    return Model.aggregationCount() + root.ownCount
  }

  function repaint() {
    canvas.requestPaint()
  }

  // A canvas text width, for labels that must not collide.
  function textWidth(ctx, text) {
    return ctx.measureText(text).width
  }

  Accessible.role: Accessible.Chart
  Accessible.name: root.chartId
  Accessible.description: root.summary

  onChartChanged: {
    root.clearHover()
    canvas.requestPaint()
  }
  onForegroundChanged: canvas.requestPaint()
  onAccentChanged: canvas.requestPaint()
  onUrgentChanged: canvas.requestPaint()
  onMutedChanged: canvas.requestPaint()
  onCanvasFontChanged: canvas.requestPaint()

  Canvas {
    id: canvas
    width: root.width
    height: root.height
    visible: !root.empty
    onPaint: {
      var ctx = getContext("2d")
      ctx.reset()
      if (root.empty || width <= 0 || height <= 0) return
      var started = Date.now()
      root.paints++
      ctx.font = root.canvasFont
      root.paintRequested(ctx, width, height)
      root.paintMs = Date.now() - started
    }
  }

  Rectangle {
    x: canvas.x + root.highlight.x
    y: canvas.y + root.highlight.y
    width: root.highlight.width
    height: root.highlight.height
    visible: root.highlight.width > 0 && !root.empty
    color: Util.alpha(root.foreground, 0.08)
    border.color: root.foreground
    border.width: Math.max(1, Style.space(1))
  }

  MouseArea {
    anchors.fill: canvas
    hoverEnabled: true
    enabled: !root.empty
    acceptedButtons: Qt.NoButton
    onPositionChanged: function(mouse) { root.hoverAt(mouse.x, mouse.y) }
    onExited: root.clearHover()
  }

  Text {
    anchors.centerIn: parent
    width: parent.width
    visible: root.empty
    horizontalAlignment: Text.AlignHCenter
    textFormat: Text.PlainText
    text: root.chart ? String(root.chart.emptyText) : Model.CHART_EMPTY_TEXT
    color: root.muted
    font.family: root.fontFamily
    font.pixelSize: Style.font.body
    elide: Text.ElideRight
  }
}
