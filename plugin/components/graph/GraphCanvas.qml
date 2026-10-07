pragma ComponentBehavior: Bound

import QtQuick
import qs.Commons
import qs.Ui
import "../../Model.js" as Model

// The graph's canvas (ADR-0034 §5, SPEC-PLUGIN §5.4; WP-125): the layout of
// `build` (Model.graphBuild, made by the service when the index changes)
// drawn on a Canvas and stepped by a Timer.
//
// Shell thread budget: the Timer runs at most 30 times a second, only while
// `running` (the section shown in the open desk) and the layout awake; each
// tick is one Model.graphStep plus the drawing calls of one paint, timed
// (`tickMs` = stepMs + drawMs, `tickMsMax`); the Canvas rasterises on its
// own thread (Canvas.Threaded).
// The layout sleeps after GRAPH_TICKS_MAX ticks; dragging a node and the
// replay wake it, pan, zoom and hover only repaint. The layout state (typed
// arrays) lives in the service (`graphLayout`), so a reopened desk shows
// the settled graph without stepping again.
//
// Pointer: drag a node (it is held where the pointer is, the rest follows),
// drag the background to pan, the wheel zooms at the pointer; hovering a
// node lights it and its links and shows the card (top right): label, kind,
// since when, links, a folded group's changes, and "Open case" for a case
// or a change linked to one. A click on a node keeps its card; a click on
// the background lets it go. Until the view is panned or zoomed it fits the
// visible nodes (`fit()` returns to that).
Item {
  id: root

  property var build: null
  property var service: null
  property bool running: false
  property color foreground: Color.popups.text
  property color accent: Color.accent
  property color urgent: Color.urgent
  property color muted: Color.muted
  property string fontFamily: Style.font.family

  // The layout (Model.graphState); mutated in place, so nothing binds to it.
  property var sim: null
  // Mirrors of the layout for bindings and view().
  property bool sleeping: true
  property int cut: 0
  property int span: 0
  property int visibleCount: 0
  property int ticks: 0
  property real stepMs: 0
  // A paint on the shell thread: the drawing calls (drawMs); until the
  // rasterised picture is there, on the Canvas's thread (paintMs).
  property real drawMs: 0
  property real paintMs: 0
  property real tickMs: 0
  property real tickMsMax: 0
  property int tickSamples: 0
  // The first ticks over the budget: [{ tick, stepMs, drawMs }], at most 5.
  property var slowTicks: []
  property int paints: 0
  property bool tickPainted: true
  property int wakes: 0

  // The node under the pointer, the node a click (or `select`) keeps, and
  // the last one hovered (-1 none). The focus (the node lit with its links)
  // is the hovered or the kept one; the card shows the focus, else the last
  // hovered node, so the pointer can travel to the card's button.
  property int hovered: -1
  property int pinned: -1
  property int lastHovered: -1
  readonly property int focusNode: root.hovered >= 0 ? root.hovered : root.pinned
  readonly property int cardNode: root.focusNode >= 0 ? root.focusNode : root.lastHovered
  readonly property var info: root.cardNode >= 0 ? Model.graphInfo(root.build, root.cardNode) : null
  // Ticks whose tickMs went over the budget.
  property int ticksOver: 0

  // View: screen = world × viewK + (width / 2 + viewX, height / 2 + viewY).
  property real viewX: 0
  property real viewY: 0
  property real viewK: 1
  property bool autoFit: true

  // Replay: the cut moves through the days; the visible count after each
  // step of the last replay (monotonic by construction, asserted by the
  // harness).
  property bool playing: false
  property var replay: []

  readonly property real unit: Style.space(1)
  readonly property real labelSize: Style.font.caption
  readonly property string canvasFont: root.labelSize + "px \"" + root.fontFamily + "\""
  readonly property color edgeColor: Util.alpha(root.foreground, 0.22)
  readonly property color areaFill: Util.alpha(root.foreground, 0.14)
  readonly property color areaRing: Util.alpha(root.foreground, 0.9)
  readonly property color decisionColor: Util.alpha(root.foreground, 0.72)
  readonly property color changeColor: Util.alpha(root.foreground, 0.42)
  readonly property color clusterFill: Util.alpha(root.foreground, 0.3)
  readonly property color clusterRing: Util.alpha(root.foreground, 0.65)
  readonly property color doneColor: Util.alpha(root.accent, 0.5)

  signal openCase(string id)

  // The node indices of each drawing style, the label order and each
  // node's smallest radius on screen, made when the build changes.
  property var styles: ({})
  property var labelOrder: []
  property var minR: []
  // Labels as drawn (cut to 28 characters) and which pass drew one last.
  property var labelTexts: []
  property var labelMarks: []
  property int labelPass: 0
  // Reused by every paint: a paint allocates nothing on the JS heap but
  // the focus's neighbour set (a garbage collection would land in a tick).
  readonly property var drawOrder: ["area", "caseDone", "case", "decision", "change", "cluster", "crisis"]
  readonly property var fitBox: ({})
  readonly property var fitView: ({})

  function fillFor(style) {
    return style === "case" ? root.accent
      : style === "caseDone" ? root.doneColor
      : style === "area" ? root.areaFill
      : style === "decision" ? root.decisionColor
      : style === "crisis" ? root.urgent
      : style === "cluster" ? root.clusterFill
      : root.changeColor
  }

  // The legend's swatch of a kind: { fill, ring } (ring transparent: none).
  function swatch(kind) {
    return {
      fill: root.fillFor(kind),
      ring: kind === "area" ? root.areaRing : kind === "cluster" ? root.clusterRing : "transparent"
    }
  }

  function rebuild() {
    var b = root.build
    if (!b) {
      root.sim = null
      root.styles = ({})
      root.sync()
      return
    }
    // The layout to continue: this canvas's own once it has stepped, else
    // the one the service kept from the last desk (the injected service can
    // come after the build: the loader sets it once the item exists).
    var stored = root.service ? root.service.graphLayout : null
    var keep = root.sim !== null && (root.sim.total > 0 || !stored) ? root.sim : stored
    var s = Model.graphState(b, keep)
    var styles = { area: [], case: [], caseDone: [], decision: [], change: [], crisis: [], cluster: [] }
    for (var i = 0; i < b.nodes.length; i++) {
      var node = b.nodes[i]
      styles[node.kind === "case" && node.done ? "caseDone" : node.kind].push(i)
    }
    root.styles = styles
    root.labelOrder = [].concat(styles.area, styles.crisis, styles.case, styles.decision, styles.caseDone, styles.cluster)
    root.minR = b.nodes.map(function(node) { return Model.graphMinRadius(node.kind) * root.unit })
    root.labelTexts = b.nodes.map(function(node) { return node.label.length <= 28 ? node.label : node.label.slice(0, 27) + "…" })
    root.labelMarks = b.nodes.map(function() { return 0 })
    root.sim = s
    if (root.service) root.service.graphLayout = s
    // Indices are the new build's: the focus does not carry over.
    root.hovered = -1
    root.lastHovered = -1
    if (root.pinned >= 0) root.pinned = keep && keep.ids[root.pinned] !== undefined && s.at[keep.ids[root.pinned]] !== undefined
      ? s.at[keep.ids[root.pinned]] : -1
    root.sync()
    root.repaint()
  }

  function sync() {
    var s = root.sim
    root.sleeping = !s || s.sleeping
    root.cut = s ? s.cut : 0
    root.span = s ? s.span : 0
    root.visibleCount = s ? s.visCount : 0
    root.ticks = s ? s.total : 0
  }

  function repaint() {
    if (root.running) canvas.requestPaint()
  }

  function wake(alpha) {
    if (!root.sim) return
    Model.graphWake(root.sim, alpha)
    root.wakes++
    root.sync()
  }

  function tick() {
    var s = root.sim
    if (!s || !Model.graphStep(s, Model.GRAPH_TICK_BUDGET_MS)) {
      root.sync()
      return
    }
    root.stepMs = s.lastMs
    root.tickPainted = false
    root.sync()
    canvas.requestPaint()
  }

  // ---- Replay

  function setCut(day, grow) {
    if (!root.sim) return 0
    var shown = Model.graphSetCut(root.sim, day, grow === true)
    if (root.pinned >= 0 && !root.sim.vis[root.pinned]) root.pinned = -1
    if (root.hovered >= 0 && !root.sim.vis[root.hovered]) root.hovered = -1
    if (root.lastHovered >= 0 && !root.sim.vis[root.lastHovered]) root.lastHovered = -1
    root.wake(shown > 0 ? 0.5 : 0.2)
    root.repaint()
    return shown
  }

  // Play from the first day (or on from the cut when it is before the
  // last day); again: pause.
  function play() {
    if (!root.sim) return
    if (root.playing) {
      root.playing = false
      return
    }
    if (root.sim.cut >= root.sim.span) root.setCut(0, false)
    root.replay = [root.sim.visCount]
    root.playing = true
  }

  function pause() {
    root.playing = false
  }

  // ---- View

  function fit() {
    root.autoFit = true
    root.repaint()
  }

  function zoomAt(factor, sx, sy) {
    var k = Math.max(0.15, Math.min(4, root.viewK * factor))
    var wx = (sx - root.width / 2 - root.viewX) / root.viewK
    var wy = (sy - root.height / 2 - root.viewY) / root.viewK
    root.autoFit = false
    root.viewK = k
    root.viewX = sx - root.width / 2 - wx * k
    root.viewY = sy - root.height / 2 - wy * k
    root.repaint()
  }

  function toWorld(sx, sy) {
    return { x: (sx - root.width / 2 - root.viewX) / root.viewK, y: (sy - root.height / 2 - root.viewY) / root.viewK }
  }

  // Node `id` on screen (item coordinates), null when it is not visible.
  function nodePoint(id) {
    var s = root.sim
    var i = s ? s.at[String(id)] : undefined
    if (i === undefined || !s.vis[i]) return null
    return { x: root.width / 2 + root.viewX + s.x[i] * root.viewK, y: root.height / 2 + root.viewY + s.y[i] * root.viewK }
  }

  // A point of the canvas with no node near it (for a pan), null if none.
  function emptyPoint() {
    if (!root.sim) return { x: root.width / 2, y: root.height / 2 }
    for (var gy = 1; gy < 8; gy++) for (var gx = 1; gx < 8; gx++) {
      var sx = root.width * gx / 8
      var sy = root.height * gy / 8
      var w = root.toWorld(sx, sy)
      if (Model.graphPick(root.sim, w.x, w.y, Style.space(24) / root.viewK) < 0) return { x: sx, y: sy }
    }
    return null
  }

  function pick(sx, sy) {
    if (!root.sim) return -1
    var w = root.toWorld(sx, sy)
    return Model.graphPick(root.sim, w.x, w.y, Style.space(4) / root.viewK)
  }

  // Let the kept and the last hovered node go (a click beside the nodes,
  // Esc); false when there was none.
  function letGo() {
    var had = root.pinned >= 0 || root.lastHovered >= 0
    root.pinned = -1
    root.lastHovered = -1
    return had
  }

  // Keep the card of node `id`; false when there is no such node.
  function select(id) {
    var s = root.sim
    var i = s ? s.at[String(id)] : undefined
    if (i === undefined) return false
    if (!s.vis[i]) root.setCut(s.span, false)
    root.pinned = i
    root.repaint()
    return true
  }

  function view() {
    var s = root.sim
    var b = root.build
    return {
      nodes: b ? b.nodes.length : 0,
      edges: b ? b.edges.length : 0,
      folded: b ? b.numbers.folded : 0,
      clusters: b ? b.numbers.clusters : 0,
      numbers: b ? b.numbers : null,
      visible: root.visibleCount,
      cut: root.cut,
      span: root.span,
      date: b ? Model.dateOfDay(b.first + root.cut) : "",
      ticks: root.ticks,
      run: s ? s.ticks : 0,
      alpha: s ? Math.round(s.alpha * 10000) / 10000 : 0,
      sleeping: root.sleeping,
      timer: tickTimer.running,
      stepMs: root.stepMs,
      paintMs: root.paintMs,
      drawMs: root.drawMs,
      tickMs: root.tickMs,
      tickMsMax: root.tickMsMax,
      tickSamples: root.tickSamples,
      slowTicks: root.slowTicks,
      ticksOver: root.ticksOver,
      over: s ? s.over : 0,
      stepMsMax: s ? s.maxMs : 0,
      paints: root.paints,
      wakes: root.wakes,
      playing: root.playing,
      replay: root.replay,
      hovered: root.hovered >= 0 && b ? b.nodes[root.hovered].id : "",
      pinned: root.pinned >= 0 && b ? b.nodes[root.pinned].id : "",
      cardNode: root.cardNode >= 0 && b ? b.nodes[root.cardNode].id : "",
      card: root.info ? { title: root.info.title, line: root.info.line, sub: root.info.sub, caseId: root.info.caseId,
        members: root.info.members.length, more: root.info.more } : null,
      view: { x: Math.round(root.viewX), y: Math.round(root.viewY), k: Math.round(root.viewK * 1000) / 1000, fit: root.autoFit }
    }
  }

  // ---- Drawing

  // Fill (and ring) the nodes of one style; `bright` the focus and its
  // neighbours, the others when false (only drawn dimmed while a node has
  // the focus). One path per node: the raster engine fills many small
  // paths far faster than one path holding them all (20 ms against 2 ms for
  // 400 discs, measured; WP-125).
  function drawStyle(ctx, style, near, bright) {
    var s = root.sim
    var list = root.styles[style]
    if (!list || list.length === 0) return
    var shape = style === "caseDone" ? "case" : style
    var ring = style === "area" || style === "cluster"
    ctx.fillStyle = root.fillFor(style)
    if (ring) {
      ctx.lineWidth = (style === "area" ? 2 : 1.5) / root.viewK
      ctx.strokeStyle = style === "area" ? root.areaRing : root.clusterRing
    }
    for (var p = 0; p < list.length; p++) {
      var i = list[p]
      if (!s.vis[i]) continue
      if (near !== null && (near[i] === true) !== bright) continue
      ctx.beginPath()
      Model.graphShape(ctx, shape, s.x[i], s.y[i], root.screenR(i))
      ctx.fill()
      if (ring) ctx.stroke()
    }
  }

  function drawEdges(ctx, near, bright) {
    var s = root.sim
    var b = root.build
    ctx.beginPath()
    for (var e = 0; e < b.edges.length; e++) {
      var u = s.ea[e]
      var w = s.eb[e]
      if (!s.vis[u] || !s.vis[w]) continue
      if (near !== null && (u === root.focusNode || w === root.focusNode) !== bright) continue
      if (b.edges[e].dashed) {
        // Dashes by hand: 4 px on, 4 px off on screen.
        var dx = s.x[w] - s.x[u]
        var dy = s.y[w] - s.y[u]
        var len = Math.sqrt(dx * dx + dy * dy)
        var dash = 4 / root.viewK
        for (var t = 0; t < len; t += 2 * dash) {
          var t1 = Math.min(len, t + dash)
          ctx.moveTo(s.x[u] + dx * t / len, s.y[u] + dy * t / len)
          ctx.lineTo(s.x[u] + dx * t1 / len, s.y[u] + dy * t1 / len)
        }
      } else {
        ctx.moveTo(s.x[u], s.y[u])
        ctx.lineTo(s.x[w], s.y[w])
      }
    }
    ctx.lineWidth = (bright && near !== null ? 1.5 : 1) / root.viewK
    ctx.strokeStyle = bright && near !== null ? Util.alpha(root.foreground, 0.6) : root.edgeColor
    ctx.stroke()
  }

  // Labels in screen pixels, at most GRAPH_LABELS_MAX by priority (areas,
  // crises, open cases, decisions, closed cases, folded groups; a change
  // only while it has the focus), the focus and its neighbours always;
  // cases, decisions and groups only from zoom 0.5 and once the layout
  // rests.
  function drawLabels(ctx, near) {
    var s = root.sim
    var b = root.build
    var k = root.viewK
    var ox = root.width / 2 + root.viewX
    var oy = root.height / 2 + root.viewY
    // While the layout moves, only areas and crises (and the focus): text
    // is the costliest part of a tick's paint; the resting picture has all.
    var moving = !root.sleeping
    var room = moving ? Model.GRAPH_LABELS_MOVING : Model.GRAPH_LABELS_MAX
    var stamp = ++root.labelPass
    var marks = root.labelMarks
    var texts = root.labelTexts
    ctx.font = root.canvasFont
    ctx.textBaseline = "middle"
    var order = root.labelOrder
    // The focus and its neighbours first (lit), then by priority.
    for (var pass = 0; pass < 2; pass++) {
      var count = pass === 0 ? (near === null ? 0 : s.visCount) : order.length
      for (var p = 0; p < count; p++) {
        var i = pass === 0 ? s.visList[p] : order[p]
        if (marks[i] === stamp || !s.vis[i]) continue
        var node = b.nodes[i]
        var focus = i === root.focusNode
        if (pass === 0) {
          if (near[i] !== true) continue
        } else {
          if (room <= 0) break
          if ((moving || k < 0.5) && node.kind !== "area" && node.kind !== "crisis") continue
          room--
        }
        marks[i] = stamp
        ctx.globalAlpha = near === null || pass === 0 ? 1 : 0.35
        ctx.fillStyle = node.kind === "change" && !focus ? root.muted : root.foreground
        ctx.fillText(focus ? node.label : texts[i], ox + s.x[i] * k + root.screenR(i) * k + root.unit * 4, oy + s.y[i] * k)
      }
    }
    ctx.globalAlpha = 1
  }

  // A node's radius in world units, at least its kind's minimum on screen.
  function screenR(i) {
    return Math.max(root.sim.r[i], root.minR[i] / root.viewK)
  }

  function paint(ctx, w, h) {
    var s = root.sim
    if (!s || s.visCount === 0) return
    if (root.autoFit) {
      var fit = Model.graphFit(Model.graphBounds(s, root.fitBox), w, h, Style.space(32), 2.5, root.fitView)
      root.viewX = fit.x
      root.viewY = fit.y
      root.viewK = fit.k
    }
    var near = null
    if (root.focusNode >= 0) {
      near = Model.graphNeighbours(s, root.focusNode)
      near[root.focusNode] = true
    }
    ctx.setTransform(root.viewK, 0, 0, root.viewK, w / 2 + root.viewX, h / 2 + root.viewY)
    var order = root.drawOrder
    if (near !== null) {
      ctx.globalAlpha = 0.25
      root.drawEdges(ctx, near, false)
      for (var a = 0; a < order.length; a++) root.drawStyle(ctx, order[a], near, false)
      ctx.globalAlpha = 1
    }
    root.drawEdges(ctx, near, true)
    for (var o = 0; o < order.length; o++) root.drawStyle(ctx, order[o], near, true)
    ctx.setTransform(1, 0, 0, 1, 0, 0)
    root.drawLabels(ctx, near)
  }

  onBuildChanged: root.rebuild()
  onServiceChanged: {
    if (root.service) root.service.graphWanted = true
    root.rebuild()
  }
  onRunningChanged: {
    if (!root.running) {
      root.playing = false
      root.hovered = -1
    }
    root.repaint()
  }
  onFocusNodeChanged: root.repaint()
  onHoveredChanged: if (root.hovered >= 0) root.lastHovered = root.hovered
  onForegroundChanged: root.repaint()
  onAccentChanged: root.repaint()
  onUrgentChanged: root.repaint()
  onCanvasFontChanged: root.repaint()
  onWidthChanged: root.repaint()
  onHeightChanged: root.repaint()
  Component.onCompleted: {
    if (root.service) root.service.graphWanted = true
    root.rebuild()
  }

  // At most 30 Hz, only while shown and awake.
  Timer {
    id: tickTimer
    interval: 34
    repeat: true
    running: root.running && !root.sleeping
    onTriggered: root.tick()
  }

  // The replay: about 50 steps over the days, 120 ms apart.
  Timer {
    id: playTimer
    interval: 120
    repeat: true
    running: root.running && root.playing
    onTriggered: {
      var s = root.sim
      if (!s || s.cut >= s.span) {
        root.playing = false
        return
      }
      root.setCut(s.cut + Math.max(1, Math.ceil(s.span / 50)), true)
      root.replay = root.replay.concat([s.visCount])
      if (s.cut >= s.span) root.playing = false
    }
  }

  Canvas {
    id: canvas
    objectName: "graphCanvas"
    anchors.fill: parent
    // The commands are rasterised on the Canvas's own thread: the shell
    // thread only runs onPaint (recording the drawing calls). Immediate
    // rasterising of 400 nodes blocked the event loop 7 ms per frame,
    // threaded never more than 4 ms in total (measured, WP-125).
    renderStrategy: Canvas.Threaded

    property double paintStarted: 0
    onPaint: {
      var ctx = getContext("2d")
      ctx.reset()
      if (width <= 0 || height <= 0) return
      canvas.paintStarted = Date.now()
      root.paint(ctx, width, height)
      root.drawMs = Date.now() - canvas.paintStarted
      root.paints++
      if (!root.tickPainted) {
        root.tickPainted = true
        root.tickMs = root.stepMs + root.drawMs
        root.tickMsMax = Math.max(root.tickMsMax, root.tickMs)
        root.tickSamples++
        if (root.tickMs > Model.GRAPH_TICK_BUDGET_MS) root.ticksOver++
        if (root.tickMs > Model.GRAPH_TICK_BUDGET_MS && root.slowTicks.length < 5)
          root.slowTicks = root.slowTicks.concat([{ tick: root.ticks, stepMs: root.stepMs, drawMs: root.drawMs }])
      }
    }
    // Until the picture is on the canvas (off this thread): for the record.
    onPainted: {
      if (canvas.paintStarted === 0) return
      root.paintMs = Date.now() - canvas.paintStarted
      canvas.paintStarted = 0
    }
  }

  MouseArea {
    id: pointer
    anchors.fill: parent
    hoverEnabled: true
    acceptedButtons: Qt.LeftButton
    cursorShape: pointer.dragNode >= 0 || pointer.panning ? Qt.ClosedHandCursor : root.hovered >= 0 ? Qt.PointingHandCursor : Qt.OpenHandCursor

    property int dragNode: -1
    property bool panning: false
    property bool moved: false
    property real lastX: 0
    property real lastY: 0
    property real pressX: 0
    property real pressY: 0

    onPressed: function(mouse) {
      pointer.moved = false
      pointer.pressX = pointer.lastX = mouse.x
      pointer.pressY = pointer.lastY = mouse.y
      var i = root.pick(mouse.x, mouse.y)
      if (i >= 0) {
        // The view holds still while a node is dragged.
        root.autoFit = false
        pointer.dragNode = i
        var w = root.toWorld(mouse.x, mouse.y)
        Model.graphPin(root.sim, i, w.x, w.y)
      } else {
        pointer.panning = true
      }
    }
    onPositionChanged: function(mouse) {
      if (Math.abs(mouse.x - pointer.pressX) + Math.abs(mouse.y - pointer.pressY) > Style.space(3)) pointer.moved = pointer.pressed
      if (pointer.dragNode >= 0) {
        if (!pointer.moved) return
        var w = root.toWorld(mouse.x, mouse.y)
        Model.graphPin(root.sim, pointer.dragNode, w.x, w.y)
        root.wake(0.3)
        root.repaint()
      } else if (pointer.panning) {
        root.autoFit = false
        root.viewX += mouse.x - pointer.lastX
        root.viewY += mouse.y - pointer.lastY
        root.repaint()
      } else {
        root.hovered = root.pick(mouse.x, mouse.y)
      }
      pointer.lastX = mouse.x
      pointer.lastY = mouse.y
    }
    onReleased: function(mouse) {
      if (pointer.dragNode >= 0) {
        if (!pointer.moved) root.pinned = pointer.dragNode
        if (root.sim) Model.graphPin(root.sim, -1, 0, 0)
        pointer.dragNode = -1
      } else if (pointer.panning && !pointer.moved) {
        root.letGo()
      }
      pointer.panning = false
      root.hovered = root.pick(mouse.x, mouse.y)
    }
    onWheel: function(wheel) {
      var steps = wheel.angleDelta.y / 120
      if (steps !== 0) root.zoomAt(Math.pow(1.15, steps), wheel.x, wheel.y)
    }
    onExited: root.hovered = -1
  }

  // The card of the hovered or kept node, top right.
  Rectangle {
    id: card
    objectName: "graphCard"
    visible: root.info !== null
    x: root.width - width - Style.spacing.lg
    y: Style.spacing.lg
    width: Math.min(Style.space(320), root.width * 0.4)
    height: cardColumn.implicitHeight + Style.spacing.md * 2
    radius: Style.cornerRadius
    color: Color.popups.background
    border.color: Util.alpha(root.foreground, 0.18)
    border.width: Math.max(1, Style.space(1))

    // Clicks on the card stay on the card (no pan beneath it).
    MouseArea {
      anchors.fill: parent
    }

    Column {
      id: cardColumn
      x: Style.spacing.md
      y: Style.spacing.md
      width: card.width - Style.spacing.md * 2
      spacing: Style.spacing.xs

      Text {
        width: parent.width
        textFormat: Text.PlainText
        text: root.info ? root.info.title : ""
        color: root.foreground
        wrapMode: Text.Wrap
        maximumLineCount: 3
        elide: Text.ElideRight
        font.family: root.fontFamily
        font.pixelSize: Style.font.body
        font.bold: true
      }
      Text {
        width: parent.width
        textFormat: Text.PlainText
        text: root.info ? root.info.line : ""
        color: root.muted
        wrapMode: Text.Wrap
        font.family: root.fontFamily
        font.pixelSize: Style.font.caption
      }
      Text {
        width: parent.width
        visible: text !== ""
        textFormat: Text.PlainText
        text: root.info ? root.info.sub : ""
        color: root.muted
        wrapMode: Text.Wrap
        font.family: root.fontFamily
        font.pixelSize: Style.font.caption
      }
      Repeater {
        model: root.info ? root.info.members : []
        Text {
          required property var modelData
          width: cardColumn.width
          textFormat: Text.PlainText
          text: String(modelData)
          color: root.foreground
          elide: Text.ElideRight
          font.family: root.fontFamily
          font.pixelSize: Style.font.caption
        }
      }
      Text {
        visible: !!root.info && root.info.more > 0
        textFormat: Text.PlainText
        text: root.info ? "+" + root.info.more + " more" : ""
        color: root.muted
        font.family: root.fontFamily
        font.pixelSize: Style.font.caption
      }
      Button {
        visible: !!root.info && root.info.caseId !== ""
        text: "Open case"
        bordered: true
        selected: true
        foreground: root.foreground
        accent: root.accent
        fontFamily: root.fontFamily
        fontSize: Style.font.bodySmall
        onClicked: if (root.info) root.openCase(root.info.caseId)
      }
    }
  }
}
