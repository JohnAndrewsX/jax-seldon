pragma ComponentBehavior: Bound

import QtQuick
import qs.Commons
import qs.Ui
import "../components/desk"
import "../components/graph"
import "../Model.js" as Model

// Section 8, Graph (ADR-0034 §5, SPEC-PLUGIN §5.4; WP-125), solo: the
// machine's memory as a growing network, from the index alone — areas,
// cases, decisions and changes, linked as the logbook links them. Row 1:
// title and what is shown, Play and the date slider (the replay's cut-off
// day) with the date and node count; row 2: the legend and the keys; then
// the canvas (components/graph/GraphCanvas.qml) and the footer, which says
// how much of the logbook the index holds.
//
// The service builds the graph when the index changes (Service.graph); the
// canvas only lays it out and draws, and only while this section is shown.
//
// Keys: ←/→ the cut-off day, Space / Enter / p play or pause the replay,
// 0 fits the view, - and = zoom out and in, Esc pauses, then lets a kept
// card go. `select <id>` keeps the card of that node.
Section {
  id: root

  readonly property var build: root.service ? root.service.graph : null
  readonly property bool hasGraph: !!root.build && !root.build.empty
  readonly property real pad: Style.spacing.huge
  readonly property color foreground: Color.popups.text
  readonly property string fontFamily: Style.font.family
  readonly property var legend: Model.GRAPH_LEGEND.filter(function(e) {
    return e.kind !== "cluster" || (!!root.build && root.build.numbers.clusters > 0)
  })
  readonly property string dateText: !root.hasGraph ? ""
    : Model.dateOfDay(root.build.first + canvas.cut) + " · " + Model.plural(canvas.visibleCount, "node", "nodes")
      + (canvas.visibleCount < root.build.nodes.length ? " of " + root.build.nodes.length : "")

  function moveAcross(dx) {
    if (!root.hasGraph) return true
    canvas.pause()
    canvas.setCut(canvas.cut + dx, dx > 0)
    return true
  }

  function activate() {
    if (root.hasGraph) canvas.play()
    return true
  }

  function textKey(t) {
    if (t === "p" || t === "P") return root.activate()
    if (t === "0") {
      canvas.fit()
      return true
    }
    if (t === "-" || t === "=") {
      canvas.zoomAt(t === "=" ? 1.25 : 0.8, canvas.width / 2, canvas.height / 2)
      return true
    }
    return false
  }

  function select(id) {
    if (!canvas.select(id)) return false
    root.selectedId = String(id)
    return true
  }

  function back() {
    if (canvas.playing) {
      canvas.pause()
      return true
    }
    if (canvas.letGo()) {
      root.selectedId = ""
      return true
    }
    return false
  }

  // For the harness and the replay key.
  function play() {
    root.activate()
  }

  function setCut(day) {
    canvas.pause()
    canvas.setCut(Number(day), false)
  }

  function graphView() {
    return canvas.view()
  }

  // The service builds the graph only for a shown section 8, and again
  // when the index changes while it is shown.
  function refresh() {
    if (root.active && root.service) root.service.graphRefresh()
  }
  onActiveChanged: root.refresh()
  onServiceChanged: root.refresh()
  Connections {
    target: root.service
    // After the index's other bindings (indexShown) have settled.
    function onGraphDirtyChanged() { Qt.callLater(root.refresh) }
  }

  function nodePoint(id) {
    var p = canvas.nodePoint(id)
    return p ? canvas.mapToItem(null, p.x, p.y) : null
  }

  function emptyPoint() {
    var p = canvas.emptyPoint()
    return p ? canvas.mapToItem(null, p.x, p.y) : null
  }

  function view() {
    return {
      graph: canvas.view(),
      date: root.dateText,
      footer: footer.text,
      caption: caption.text,
      still: root.hasGraph && root.build.still,
      legend: root.legend.map(function(e) { return e.label }),
      empty: emptyText.visible ? emptyText.text : ""
    }
  }

  // Row 1: title and caption left; Play, the slider and the date right.
  Item {
    id: head
    x: root.pad
    y: root.pad
    width: Math.max(0, root.width - root.pad * 2)
    height: Math.max(titleText.implicitHeight + Style.spacing.xs + caption.implicitHeight, controls.height)

    Text {
      id: titleText
      width: Math.max(0, parent.width - controls.width - Style.spacing.panelGap)
      textFormat: Text.PlainText
      text: "Graph"
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
      // A still picture (Model.graphBuild's `still`) says why.
      text: root.hasGraph && root.build.still
        ? "A still picture: " + (root.build.numbers.areas + root.build.numbers.cases + root.build.numbers.decisions
          + root.build.numbers.crises) + " areas, cases, decisions and crises are more than the "
          + Model.GRAPH_CAP + " nodes the layout moves"
        : "This machine since its logbook began · cases, changes, decisions, areas"
      color: Color.muted
      elide: Text.ElideRight
      font.family: root.fontFamily
      font.pixelSize: Style.font.bodySmall
    }

    Item {
      id: controls
      anchors.right: parent.right
      anchors.verticalCenter: parent.verticalCenter
      visible: root.hasGraph
      width: playButton.implicitWidth + slider.width + dateLabel.width + Style.spacing.md * 2
      height: Math.max(playButton.implicitHeight, slider.implicitHeight)

      Button {
        id: playButton
        objectName: "graphPlay"
        anchors.verticalCenter: parent.verticalCenter
        text: canvas.playing ? "Pause" : "Play growth"
        bordered: true
        foreground: root.foreground
        accent: Color.accent
        fontFamily: root.fontFamily
        fontSize: Style.font.bodySmall
        onClicked: {
          canvas.play()
          if (root.desk) root.desk.giveKeys()
        }
      }

      PanelSlider {
        id: slider
        objectName: "graphSlider"
        x: playButton.implicitWidth + Style.spacing.md
        anchors.verticalCenter: parent.verticalCenter
        width: Math.max(Style.space(120), Math.min(Style.space(360), head.width * 0.3))
        minimum: 0
        maximum: Math.max(1, canvas.span)
        step: 1
        integer: true
        value: canvas.cut
        trackColor: Style.selectedFillFor(root.foreground, Color.accent)
        fillColor: Color.accent
        knobColor: Color.accent
        // No knob animation while the replay moves it: a running QML
        // animation throttles the shell thread to the display's frames
        // (gaps of a frame, ~17 ms, measured on the test host; WP-125).
        // PanelSlider animates only while not `dragging`; its own press
        // sets that, so the release hands it back to the replay.
        dragging: canvas.playing
        onMoved: function(v) {
          canvas.pause()
          canvas.setCut(Math.round(v), Math.round(v) > canvas.cut)
        }
        onReleased: function(v) {
          slider.dragging = Qt.binding(function() { return canvas.playing })
          if (root.desk) root.desk.giveKeys()
        }
      }

      Text {
        id: dateLabel
        x: slider.x + slider.width + Style.spacing.md
        anchors.verticalCenter: parent.verticalCenter
        width: dateMetrics.advanceWidth
        textFormat: Text.PlainText
        text: root.dateText
        color: Color.muted
        font.family: root.fontFamily
        font.pixelSize: Style.font.bodySmall
        font.features: { "tnum": 1 }
      }

      // Wide enough for the longest date text, so the slider stays put.
      TextMetrics {
        id: dateMetrics
        font: dateLabel.font
        text: "0000-00-00 · 000 nodes of 000"
      }
    }
  }

  // Row 2: the legend; the keys at the right while there is room.
  Item {
    id: legendRow
    x: root.pad
    y: head.y + head.height + Style.spacing.md
    width: head.width
    height: legendFlow.implicitHeight
    visible: root.hasGraph

    Row {
      id: legendFlow
      spacing: Style.spacing.lg

      Repeater {
        model: root.legend

        Row {
          id: entry
          required property var modelData
          spacing: Style.spacing.xs
          readonly property var swatch: canvas.swatch(entry.modelData.kind)

          Canvas {
            id: shape
            width: Style.space(12)
            height: Style.space(12)
            anchors.verticalCenter: parent.verticalCenter
            property var swatch: entry.swatch
            onSwatchChanged: requestPaint()
            onPaint: {
              var ctx = getContext("2d")
              ctx.reset()
              var r = Math.min(width, height) / 2 - 1.5
              ctx.beginPath()
              Model.graphShape(ctx, entry.modelData.kind, width / 2, height / 2, entry.modelData.kind === "crisis" ? r * 0.75 : r)
              ctx.fillStyle = shape.swatch.fill
              ctx.fill()
              if (entry.modelData.kind === "area" || entry.modelData.kind === "cluster") {
                ctx.lineWidth = 1.5
                ctx.strokeStyle = shape.swatch.ring
                ctx.stroke()
              }
            }
          }

          Text {
            anchors.verticalCenter: parent.verticalCenter
            textFormat: Text.PlainText
            text: entry.modelData.label
            color: Color.muted
            font.family: root.fontFamily
            font.pixelSize: Style.font.caption
          }
        }
      }
    }

    Text {
      anchors.right: parent.right
      anchors.verticalCenter: parent.verticalCenter
      visible: parent.width >= legendFlow.implicitWidth + implicitWidth + Style.spacing.panelGap
      textFormat: Text.PlainText
      text: "←/→ day · Space play · drag, scroll · 0 fit"
      color: Color.muted
      font.family: root.fontFamily
      font.pixelSize: Style.font.caption
    }
  }

  GraphCanvas {
    id: canvas
    x: root.pad
    y: legendRow.y + legendRow.height + Style.spacing.md
    width: head.width
    height: Math.max(0, footer.y - y - Style.spacing.md)
    visible: root.hasGraph
    build: root.hasGraph ? root.build : null
    service: root.service
    running: root.active && root.hasGraph
    foreground: root.foreground
    fontFamily: root.fontFamily
    onOpenCase: function(id) {
      if (!root.desk) return
      root.desk.section("work")
      root.desk.select(id)
    }
    onPinnedChanged: root.selectedId = canvas.pinned >= 0 && root.build ? root.build.nodes[canvas.pinned].id : ""
  }

  Text {
    id: emptyText
    anchors.centerIn: canvas
    width: canvas.width
    visible: !root.hasGraph
    horizontalAlignment: Text.AlignHCenter
    textFormat: Text.PlainText
    text: !root.index ? "No index to show" : "Nothing to draw yet: no areas, cases, decisions or changes in the index"
    color: Color.muted
    wrapMode: Text.Wrap
    font.family: root.fontFamily
    font.pixelSize: Style.font.body
  }

  Text {
    id: footer
    x: root.pad
    y: root.height - root.pad - implicitHeight
    width: head.width
    textFormat: Text.PlainText
    text: root.hasGraph ? root.build.footer : ""
    color: Color.muted
    elide: Text.ElideRight
    font.family: root.fontFamily
    font.pixelSize: Style.font.caption
  }
}
