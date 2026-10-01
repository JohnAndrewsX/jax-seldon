import QtQuick
import Quickshell.Io
import qs.Commons
import qs.Ui
import "Model.js" as Model

// The Seldon pill (SPEC-PLUGIN §4): `⟡ A · D`, A = active cases, D = open
// drift, zero parts hidden. Accent when cases are active, the theme's urgent
// colour when any drift is in the red zone, dimmed while the status is not ok.
// Left click toggles the panel, middle click the Prime Radiant, right click
// captures.
//
// Routing (SPEC-PLUGIN §8): the manifest declares `overlay`, so the shell
// hands jax.seldon to its panel loader. `omarchy-shell shell summon|hide|
// toggle jax.seldon` therefore opens Overlay.qml and never reaches this
// widget. The bar panel has its own IPC target, `jax.seldon.panel`, below.
// open/close/opened on this root are still read by the bar itself: Tab
// between panels (Bar.panelNavigationSlots) and the popout coordinator
// (closeForPopoutSwitch) look for them on the widget in the slot.
BarWidget {
  id: root
  moduleName: "jax.seldon"

  // The plugin's own Service.qml, through the scoped shell facade. The
  // service can mount after the bar, so keep looking until it is there; a
  // QtObject property drops back to null if the service is destroyed.
  property QtObject service: null

  readonly property var counts: service ? service.counts : null
  readonly property string status: service ? service.status : ""
  readonly property string tone: Model.pillTone(counts)
  readonly property bool dimmed: !service || (service.ready && status !== "ok")
  readonly property int captureInterval: Model.clampInterval(setting("captureIntervalMin", Model.CAPTURE_INTERVAL_MIN_DEFAULT))
  readonly property string pillText: vertical ? Model.GLYPH : Model.pillText(counts)
  readonly property string tooltip: service
    ? Model.tooltipText(status, counts, service.lastCapture, service.nowMs)
    : "Seldon — service not running"

  function findService() {
    var shell = root.bar ? root.bar.shell : null
    if (shell && typeof shell.serviceFor === "function") root.service = shell.serviceFor(root.moduleName)
  }

  function pushSettings() {
    if (root.service) root.service.setCaptureInterval(root.captureInterval)
  }

  function openOverlay() {
    root.close()
    var shell = root.bar ? root.bar.shell : null
    if (shell && typeof shell.toggle === "function") shell.toggle(root.moduleName, "")
  }

  function captureNow() {
    if (root.service) root.service.captureNow()
  }

  // ---- Panel lifecycle, forwarded to Panel.qml (see the clock widget).
  readonly property bool opened: panelLoader.item ? panelLoader.item.opened === true : false
  readonly property bool popoutSwitchClosing: panelLoader.item ? panelLoader.item.popoutSwitchClosing === true : false
  readonly property real openPanelIndicatorWidth: button.labelWidth

  function open() {
    if (panelLoader.item) panelLoader.item.open()
  }

  function close() {
    if (panelLoader.item) panelLoader.item.close()
  }

  function togglePanel() {
    if (panelLoader.item) panelLoader.item.toggle()
  }

  function closeForPopoutSwitch() {
    if (panelLoader.item) panelLoader.item.closeForPopoutSwitch()
  }

  function injectPanel() {
    var target = panelLoader.item
    if (!target) return
    target.bar = root.bar
    target.settings = root.settings
    target.anchorItem = button
    target.hostWidget = root
    target.service = root.service
  }

  implicitWidth: button.implicitWidth
  implicitHeight: button.implicitHeight

  onBarChanged: { root.findService(); root.injectPanel() }
  onSettingsChanged: root.injectPanel()
  onServiceChanged: { root.pushSettings(); root.injectPanel() }
  onCaptureIntervalChanged: root.pushSettings()

  Timer {
    interval: 1000
    repeat: true
    running: !root.service
    triggeredOnStart: true
    onTriggered: root.findService()
  }

  Loader {
    id: panelLoader
    active: true
    source: Qt.resolvedUrl("Panel.qml")
    visible: false
    onLoaded: {
      root.injectPanel()
      Qt.callLater(root.injectPanel)
    }
  }

  IpcHandler {
    target: "jax.seldon.panel"

    function open(): void { root.open() }
    function close(): void { root.close() }
    function show(): void { root.open() }
    function hide(): void { root.close() }
    function toggle(): void { root.togglePanel() }
    // What the pill shows right now, for smoke tests (docs/TESTING.md).
    function pill(): string {
      return JSON.stringify({
        text: button.text,
        tone: root.tone,
        urgent: button.active,
        dimmed: button.dimmed,
        tooltip: button.tooltipText,
        status: root.status,
        opened: root.opened
      })
    }
    // What the panel shows (tab, rows, banners, strip), for smoke tests.
    function view(): string {
      return JSON.stringify(panelLoader.item ? panelLoader.item.view() : null)
    }
    // Show one tab: today | changelog | system.
    function tab(name: string): string {
      return panelLoader.item && panelLoader.item.selectTabById(name) ? "ok" : "unknown tab"
    }
    // Set the Changelog source filter: all | pacman | snapper | …
    function filter(source: string): string {
      if (!panelLoader.item || (source !== "all" && Model.SOURCES.indexOf(source) === -1)) return "unknown source"
      panelLoader.item.setFilter(source)
      return "ok"
    }
  }

  WidgetButton {
    id: button
    anchors.fill: parent
    bar: root.bar
    text: root.pillText
    foreground: root.tone === "accent" ? Color.accent : (root.bar ? root.bar.barForeground : Color.foreground)
    active: root.tone === "urgent"
    dimmed: root.dimmed
    tooltipText: root.tooltip

    onPressed: function(b) {
      if (b === Qt.RightButton) root.captureNow()
      else if (b === Qt.MiddleButton) root.openOverlay()
      else root.togglePanel()
    }
  }
}
