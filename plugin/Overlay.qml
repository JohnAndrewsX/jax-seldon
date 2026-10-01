pragma ComponentBehavior: Bound

import QtQuick
import qs.Commons
import qs.Ui
import "components"
import "components/overlay"
import "Model.js" as Model

// Prime Radiant, the fullscreen overlay (SPEC-PLUGIN §6).
//
// Routing (SPEC-PLUGIN §8): `omarchy-shell shell summon|toggle jax.seldon
// ['{"period":"30"}']` and the pill's middle click load this file through the
// shell's overlay loader and call open(payloadJson); `hide` calls close().
// The loader drops the item on hide, so every open starts from here.
//
// Layout: a scrim, then a card with the header (title, machine, Omarchy
// version, index time, period selector, close), the status banner when the
// service is not ok, a 12-column grid of six slots (Model.overlayGrid) and
// the keys. The slots hold the charts (WP-031, components/overlay/):
// Heatmap, Series, DriftBars, RiskDonut, Timeline and The Plan, each drawn
// from its entry in periodData.charts, with a caption that reads out the
// hovered item or the chart's summary, and an empty state.
//
// Keys: 1–4 pick 30 d / 90 d / 365 d / All, ←/→ (h/l) the previous or next
// period, Esc closes. A click on the scrim closes.
//
// Cheap to open: the service computes every period's rows, counts and chart
// data when the index changes (Service.periods, Model.periodTable); this
// file and the charts only look them up and draw. view() reports the
// aggregation passes so the harness can prove that opening and switching
// periods add none. It reads only the service's index and runs no engine
// command (WP-030).
//
// Read-out for tests and the test host: `omarchy-shell shell call jax.seldon
// view ""` (JSON, see view()); `shell call jax.seldon setPeriod 30`;
// `shell call jax.seldon hover "heatmap 0.9,0.5"` (the read-out at that
// point of a chart, as fractions of its plot; "" clears every hover).
Item {
  id: root

  // Injected by omarchy-shell's panel loader.
  property var shell: null
  property var manifest: null
  property var service: null

  property bool opened: false
  // The selected period (a Model.PERIODS id). WP-031's charts bind to it and
  // to periodData.series.
  property string period: Model.PERIOD_DEFAULT

  readonly property var index: root.service ? root.service.index : null
  readonly property var periodData: Model.periodView(root.service ? root.service.periods : null, root.period)
  readonly property var periodOptions: Model.PERIODS.map(function(p) { return { value: p.id, label: p.label } })
  readonly property var banner: Model.overlayBanner(root.service ? root.service.banner : null)
  readonly property var grid: Model.overlayGrid(gridArea.width, gridArea.height, Style.spacing.panelGap,
    Style.space(240), Style.space(120))

  readonly property color foreground: Color.popups.text
  readonly property string fontFamily: Style.font.family

  function open(payloadJson) {
    root.period = Model.overlayPayloadPeriod(payloadJson, root.period)
    root.opened = true
    Qt.callLater(function() { keyCatcher.forceActiveFocus() })
  }

  function close() {
    root.opened = false
  }

  function dismiss() {
    root.opened = false
    if (root.shell && typeof root.shell.hide === "function")
      root.shell.hide((root.manifest && root.manifest.id) || "jax.seldon")
  }

  // Returns the period now selected; an unknown id changes nothing.
  function setPeriod(id) {
    var value = String(id)
    if (Model.isPeriod(value)) root.period = value
    return root.period
  }

  function summaryFor(id) {
    var slots = root.periodData.slots
    for (var i = 0; i < slots.length; i++) if (slots[i].id === id) return slots[i]
    return null
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

  // "heatmap 0.9,0.5" → the read-out at that point of the chart's plot
  // (fractions), as JSON { slot, hover }; "" clears every chart's hover.
  function hover(arg) {
    var m = /^\s*(\w+)\s+([0-9.]+),([0-9.]+)\s*$/.exec(String(arg))
    if (!m) {
      for (var i = 0; i < slotRepeater.count; i++) {
        var item = slotRepeater.itemAt(i) as OverlaySlot
        if (item && item.chart) item.chart.clearHover()
      }
      return JSON.stringify({ slot: "", hover: "" })
    }
    var chart = root.chartFor(m[1])
    return JSON.stringify({ slot: chart ? m[1] : "", hover: chart ? chart.probe(Number(m[2]), Number(m[3])) : "" })
  }

  function rectFor(id) {
    var slots = root.grid.slots
    for (var i = 0; i < slots.length; i++) if (slots[i].id === id) return slots[i]
    return { x: 0, y: 0, w: 0, h: 0 }
  }

  // What the overlay shows, as JSON: state, period and window, banner, grid
  // mode, aggregation passes, and each slot's counts, geometry in window
  // coordinates and chart (summary, numbers, empty, hover, paints, paintMs).
  function view(arg) {
    var slots = []
    for (var i = 0; i < slotRepeater.count; i++) {
      var item = slotRepeater.itemAt(i) as OverlaySlot
      if (!item || !item.summary) continue
      var at = item.mapToItem(frame, 0, 0)
      var s = item.summary
      var c = item.chart
      slots.push({ id: s.id, title: s.title, rows: s.rows, count: s.count, detail: s.detail, windowed: s.windowed,
        x: Math.round(at.x), y: Math.round(at.y), w: Math.round(item.width), h: Math.round(item.height),
        chart: c ? { summary: c.summary, numbers: c.numbers, empty: c.empty, hover: c.hoverText, paints: c.paints, paintMs: c.paintMs,
          w: Math.round(c.plot.width), h: Math.round(c.plot.height) } : null })
    }
    return JSON.stringify({
      opened: root.opened,
      period: root.period,
      window: root.periodData.window,
      caption: Model.periodCaption(root.periodData.window),
      status: root.service ? root.service.status : "",
      banner: root.banner ? root.banner.title : "",
      meta: Model.overlayMeta(root.index),
      mode: root.grid.mode,
      scrolls: gridArea.contentHeight > gridArea.height,
      size: { w: Math.round(frame.width), h: Math.round(frame.height) },
      aggregations: { service: root.service ? root.service.aggregationCount() : 0, overlay: root.aggregationCount() },
      slots: slots
    })
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

  OverlayWindow {
    id: window
    visible: root.opened

    Item {
      id: frame
      anchors.fill: parent

      Rectangle {
        anchors.fill: parent
        color: Color.menu.scrim
      }

      MouseArea {
        anchors.fill: parent
        onClicked: root.dismiss()
      }

      BorderSurface {
        id: card
        anchors.fill: parent
        anchors.margins: Math.max(Style.gapsOut, Style.space(24))
        radius: Style.cornerRadius
        color: Color.popups.background
        borderSpec: Border.surfaceSpec("popups", "border", Color.popups.border, Math.max(1, Style.space(2)))
        padding: Style.spacing.panelPadding

        // Clicks on the card stay on the card (declared first, so the
        // header's buttons keep theirs).
        MouseArea {
          anchors.fill: parent
        }

        PanelKeyCatcher {
          id: keyCatcher
          anchors.fill: parent
          onCloseRequested: root.dismiss()
          onMoveRequested: function(dx, dy) {
            if (dx !== 0) root.period = Model.cyclePeriod(root.period, dx)
          }
          onTextKey: function(text) {
            var picked = Model.periodForKey(text)
            if (picked !== "") root.period = picked
          }
        }

        Item {
          id: content
          x: card.contentLeftInset
          y: card.contentTopInset
          width: card.width - card.contentLeftInset - card.contentRightInset
          height: card.height - card.contentTopInset - card.contentBottomInset

          OverlayHeader {
            id: header
            width: parent.width
            meta: Model.overlayMeta(root.index)
            caption: Model.periodCaption(root.periodData.window)
            periods: root.periodOptions
            period: root.period
            foreground: root.foreground
            fontFamily: root.fontFamily
            onPeriodRequested: function(id) { root.setPeriod(id) }
            onCloseRequested: root.dismiss()
          }

          // A non-ok service: its banner, with the fix that runs no engine
          // command; the rest of the fixes are in the panel.
          Column {
            id: notice
            anchors.top: header.bottom
            anchors.topMargin: visible ? Style.spacing.panelGap : 0
            width: parent.width
            spacing: Style.spacing.sm
            visible: root.banner !== null
            height: visible ? implicitHeight : 0

            Banner {
              width: parent.width
              banner: root.banner
              foreground: root.foreground
              fontFamily: root.fontFamily
              onActionRequested: function(actionId) { if (root.service) root.service.fix(actionId, "status") }
            }

            Text {
              width: parent.width
              textFormat: Text.PlainText
              text: "Fix it from the Seldon panel (click ⟡ in the bar)."
              color: Color.muted
              font.family: root.fontFamily
              font.pixelSize: Style.font.bodySmall
              elide: Text.ElideRight
            }
          }

          Flickable {
            id: gridArea
            anchors.top: notice.bottom
            anchors.topMargin: Style.spacing.panelGap
            anchors.bottom: footer.top
            anchors.bottomMargin: Style.spacing.panelGap
            width: parent.width
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

          Text {
            id: footer
            anchors.bottom: parent.bottom
            width: parent.width
            horizontalAlignment: Text.AlignHCenter
            textFormat: Text.PlainText
            text: "1–4 period · ←/→ previous / next · Esc close"
            color: Color.muted
            font.family: root.fontFamily
            font.pixelSize: Style.font.bodySmall
            elide: Text.ElideRight
          }
        }
      }
    }
  }
}
