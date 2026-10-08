import QtQuick
import QtQuick.Window
import Quickshell.Io
import qs.Commons
import qs.Ui
import "components"
import "Model.js" as Model

// The Seldon pill (SPEC-PLUGIN §4): the bar glyph (A4) and `A · D`, A =
// active cases, D = the crisis count by default, zero parts hidden; the
// setting `driftInBar` (ADR-0028 §4a) makes D all open drift (`all`) or
// hides it (`none`). Accent when cases are active, the theme's urgent
// colour when any crisis (in every mode), dimmed while the status is not
// ok; the glyph takes the text's colour.
// Left click toggles the panel, middle click the Prime Radiant, right click
// captures.
//
// The glyph box is the shell's icon canvas (Style.bar.iconCanvas: 16 px at
// scale 1.0, 20 at 1.25), 2 px before the counts; the hinted file when the
// box in device pixels is 16 or 20, the vector otherwise (Model.barGlyph).
// Its ink centre sits on the digits' centre (half the digit height above
// the baseline, from the bar font's own metrics), snapped to device pixels
// so the hinted grid stays crisp (brief check 4: within 1 px).
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
  readonly property string driftInBar: Model.driftInBarMode(setting("driftInBar", Model.DRIFT_IN_BAR_DEFAULT))
  readonly property string pillText: vertical ? "" : Model.pillText(counts, driftInBar)
  readonly property string tooltip: service
    ? Model.tooltipText(status, counts, service.lastCapture, service.nowMs)
    : "Seldon — service not running"

  function findService() {
    var shell = root.bar ? root.bar.shell : null
    if (shell && typeof shell.serviceFor === "function") root.service = shell.serviceFor(root.moduleName)
  }

  function pushSettings() {
    if (!root.service) return
    root.service.setCaptureInterval(root.captureInterval)
    root.service.setDriftInBar(root.driftInBar)
  }

  function openOverlay() {
    root.close()
    var shell = root.bar ? root.bar.shell : null
    if (shell && typeof shell.toggle === "function") shell.toggle(root.moduleName, "")
  }

  // The pill's read-out: IPC `pill` and the bar harness.
  function pillReadout() {
    return JSON.stringify({
      text: button.text,
      glyph: glyph.file,
      tone: root.tone,
      urgent: button.active,
      dimmed: button.dimmed,
      tooltip: button.tooltipText,
      status: root.status,
      driftInBar: root.driftInBar,
      opened: root.opened
    })
  }

  function captureNow() {
    if (root.service) root.service.captureNow()
  }

  // ---- Panel lifecycle, forwarded to Panel.qml (see the clock widget).
  readonly property bool opened: panelLoader.item ? panelLoader.item.opened === true : false
  readonly property bool popoutSwitchClosing: panelLoader.item ? panelLoader.item.popoutSwitchClosing === true : false
  readonly property real openPanelIndicatorWidth: pill.width

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

  // ---- One handler for `jax.seldon.panel` (WP-067). The bar builds this
  // widget once per monitor, plus a zero-size, hidden placeholder in the
  // bar's centre section (once a centre anchor is set, the default, the
  // shell mounts the centre list a second time), and an IPC target takes
  // one handler: every further instance made the shell log "another
  // handler is registered".
  // The first drawn instance the bar lists owns the target, a placeholder
  // only when none is drawn (Model.pickDrawnWidget, as the shell's
  // pickDrawnSlot routes a panel hotkey; WP-078). When an instance comes,
  // goes, or is drawn or hidden, every instance looks again on a later turn
  // of the event loop, the owner first, so it lets go before the next one
  // takes over. Without the bar's list (a harness) the widget owns it alone.
  //
  // An owner that goes only lets go (WP-162). When the shell exits
  // (`omarchy restart shell`, `quickshell kill`), Quickshell tears the whole
  // engine generation down within one event; a handler enabled in the middle
  // of it looks up the generation's IPC registry, which is already gone, and
  // Quickshell 0.3.1 crashes (SIGSEGV in IpcHandler::updateRegistration). So
  // the hand-over runs later, through each sibling's own method
  // (Qt.callLater): after a teardown the siblings are destroyed too and Qt
  // drops the calls; when one instance goes at run time (a monitor
  // unplugged, the widget removed from the bar) the survivors live on and
  // take the target over. Every sibling gets the call, so one that dies in
  // the same turn (a placeholder) cannot drop the hand-over.
  property bool ipcOwner: false
  readonly property bool drawn: Model.isDrawnWidget(root)

  function liveWidgets() {
    return root.bar && typeof root.bar.moduleWidgets === "function" ? root.bar.moduleWidgets(root.moduleName) : []
  }

  function claimIpc() {
    var pick = Model.pickDrawnWidget(root.liveWidgets(), null)
    root.ipcOwner = pick === null || pick === root
  }

  function reclaimIpc() {
    var items = root.liveWidgets()
    if (items.indexOf(root) === -1) items = items.concat([root])
    var owners = items.filter(function(w) { return !!w && w.ipcOwner === true })
    var others = items.filter(function(w) { return !!w && w.ipcOwner !== true })
    var order = owners.concat(others)
    for (var i = 0; i < order.length; i++)
      if (typeof order[i].claimIpc === "function") order[i].claimIpc()
  }

  onDrawnChanged: Qt.callLater(root.reclaimIpc)
  Component.onCompleted: Qt.callLater(root.reclaimIpc)
  Component.onDestruction: {
    if (!root.ipcOwner) return
    root.ipcOwner = false
    var siblings = root.liveWidgets().filter(function(w) { return !!w && w !== root && typeof w.reclaimIpc === "function" })
    siblings.forEach(function(w) { Qt.callLater(w.reclaimIpc) })
  }

  implicitWidth: button.implicitWidth
  implicitHeight: button.implicitHeight

  onBarChanged: { root.findService(); root.injectPanel(); Qt.callLater(root.reclaimIpc) }
  onSettingsChanged: root.injectPanel()
  onServiceChanged: { root.pushSettings(); root.injectPanel() }
  onCaptureIntervalChanged: root.pushSettings()
  onDriftInBarChanged: root.pushSettings()

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
    enabled: root.ipcOwner

    function open(): void { root.open() }
    function close(): void { root.close() }
    function show(): void { root.open() }
    function hide(): void { root.close() }
    function toggle(): void { root.togglePanel() }
    // What the pill shows right now, for smoke tests (docs/TESTING.md).
    function pill(): string { return root.pillReadout() }
    // What the panel shows (tab, rows, banners, strip), for smoke tests.
    function view(): string {
      return JSON.stringify(panelLoader.item ? panelLoader.item.view() : null)
    }
    // Show one tab: today | changelog | work | decisions | system | memory.
    function tab(name: string): string {
      return panelLoader.item && panelLoader.item.selectTabById(name) ? "ok" : "unknown tab"
    }
    // Open the Changelog's drift sheet: `crisis` (the red strip's target) or
    // an event id. Navigation only; the sheet's actions need a key or click.
    function resolve(target: string): string {
      if (!panelLoader.item || (target !== "crisis" && !Model.EVENT_ID.test(target))) return "unknown target"
      return panelLoader.item.resolve(target) ? "ok" : "not open drift"
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
    // The shell's label is replaced by the glyph and counts below.
    labelVisible: false
    hasVisualContent: true
    fixedWidth: root.vertical ? -1 : pill.width + button.scaledHorizontalMargin * 2
    fixedHeight: root.vertical ? pill.height + button.scaledVerticalPadding * 2 : -1
    foreground: root.tone === "accent" ? Color.accent : (root.bar ? root.bar.barForeground : Color.foreground)
    active: root.tone === "urgent"
    dimmed: root.dimmed
    tooltipText: root.tooltip

    onPressed: function(b) {
      if (b === Qt.RightButton) root.captureNow()
      else if (b === Qt.MiddleButton) root.openOverlay()
      else root.togglePanel()
    }

    Item {
      id: pill
      objectName: "seldonPill"

      readonly property color ink: button.active && button.useActiveColor ? button.activeColor : button.foreground
      readonly property real dpr: Screen.devicePixelRatio > 0 ? Screen.devicePixelRatio : 1
      readonly property real box: Style.bar.iconCanvas
      readonly property var spec: Model.barGlyph(box * dpr)
      readonly property real gap: countsText.text === "" ? 0 : Style.space(2)
      // The digits' centre in this item: the baseline minus half the digit
      // height (the tight box of the ten digits in the bar font).
      readonly property real digitCentre: countsText.y + countsText.baselineOffset + digits.tightBoundingRect.y + digits.tightBoundingRect.height / 2
      readonly property real glyphCentre: glyph.y + spec.centre * box

      anchors.centerIn: parent
      width: root.vertical ? box : box + gap + (countsText.text === "" ? 0 : countsText.implicitWidth)
      height: root.vertical ? box : button.height

      function snap(v) {
        return Math.round(v * pill.dpr) / pill.dpr
      }

      TextMetrics {
        id: digits
        font: countsText.font
        text: "0123456789"
      }

      MaskIcon {
        id: glyph
        objectName: "seldonGlyph"
        x: 0
        y: root.vertical ? 0 : pill.snap(pill.digitCentre - pill.spec.centre * pill.box)
        width: pill.box
        height: pill.box
        file: pill.spec.file
        crisp: pill.spec.crisp
        color: pill.ink
      }

      // Placed like the shell's own label (vertically centred), so the
      // digits share the baseline of the neighbouring widgets.
      Text {
        id: countsText
        objectName: "seldonCounts"
        x: pill.box + pill.gap
        anchors.verticalCenter: parent.verticalCenter
        visible: !root.vertical
        textFormat: Text.PlainText
        text: button.text
        color: pill.ink
        font.family: button.fontFamily
        font.pixelSize: button.fontSize
        renderType: Text.NativeRendering
      }
    }
  }
}
