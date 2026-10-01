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
// service is not ok, a 12-column grid of five chart slots (Model.overlayGrid)
// and the keys. WP-031 puts the charts into the slots; until then each slot
// shows its series' row count for the selected period.
//
// Keys: 1–4 pick 30 d / 90 d / 365 d / All, ←/→ (h/l) the previous or next
// period, Esc closes. A click on the scrim closes.
//
// Cheap to open: the service computes every period's rows and counts when
// the index changes (Service.periods, Model.periodTable); this file only
// looks them up. It reads only the service's index and runs no engine
// command (WP-030).
//
// Read-out for tests and the test host: `omarchy-shell shell call jax.seldon
// view ""` (JSON, see view()); `shell call jax.seldon setPeriod 30`.
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

  function rectFor(id) {
    var slots = root.grid.slots
    for (var i = 0; i < slots.length; i++) if (slots[i].id === id) return slots[i]
    return { x: 0, y: 0, w: 0, h: 0 }
  }

  // What the overlay shows, as JSON: state, period and window, banner, grid
  // mode, and each slot's counts and geometry in window coordinates.
  function view(arg) {
    var slots = []
    for (var i = 0; i < slotRepeater.count; i++) {
      var item = slotRepeater.itemAt(i) as OverlaySlot
      if (!item || !item.summary) continue
      var at = item.mapToItem(frame, 0, 0)
      var s = item.summary
      slots.push({ id: s.id, title: s.title, rows: s.rows, count: s.count, detail: s.detail, windowed: s.windowed,
        x: Math.round(at.x), y: Math.round(at.y), w: Math.round(item.width), h: Math.round(item.height) })
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
      slots: slots
    })
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
