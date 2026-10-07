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
// Left click toggles the desk (ADR-0034), middle click opens it at the
// Prime Radiant, right click captures.
//
// The glyph box is the shell's icon canvas (Style.bar.iconCanvas: 16 px at
// scale 1.0, 20 at 1.25), 2 px before the counts; the hinted file when the
// box in device pixels is 16 or 20, the vector otherwise (Model.barGlyph).
// Its ink centre sits on the digits' centre (half the digit height above
// the baseline, from the bar font's own metrics), snapped to device pixels
// so the hinted grid stays crisp (brief check 4: within 1 px).
//
// Routing (SPEC-PLUGIN §8): the manifest declares `overlay`, so the shell
// hands jax.seldon to its panel loader: `omarchy-shell shell summon|hide|
// toggle jax.seldon` opens Desk.qml and never reaches this widget, and the
// widget's clicks go the same way, through the scoped facade (`bar.shell`).
// The widget has no popup of its own any more, so it offers the bar no
// open/close: Tab between bar panels passes it by.
// `jax.seldon.panel`, the 0.1.x panel's IPC target, stays one minor
// release as a shim to the desk (ADR-0034 §7; removed in 0.3.0).
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
    root.service.setDeskSettings(root.settings)
  }

  // ---- The desk, through the facade (summon / hide / toggle jax.seldon).
  readonly property bool deskOpened: !!root.service && !!root.service.desk && root.service.desk.opened === true
  // Calls this instance forwarded to the desk (the bar harness checks that
  // the IPC owner is the one that acts).
  property int deskCalls: 0

  function shellCall(method, payload) {
    var shell = root.bar ? root.bar.shell : null
    if (!shell || typeof shell[method] !== "function") return false
    root.deskCalls++
    return method === "hide" ? shell.hide(root.moduleName) === true : shell[method](root.moduleName, payload || "") === true
  }

  function toggleDesk() {
    return root.shellCall("toggle", "")
  }

  // Open the desk (or re-target an open one) with a payload.
  function summonDesk(payload) {
    return root.shellCall("summon", JSON.stringify(payload || {}))
  }

  function hideDesk() {
    return root.shellCall("hide", "")
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
      opened: root.deskOpened
    })
  }

  function captureNow() {
    if (root.service) root.service.captureNow()
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
  // goes, or is drawn or hidden, every instance looks again, the owner
  // first, so it lets go before the next one takes over. Without the bar's
  // list (a harness) the widget owns it alone.
  property bool ipcOwner: false
  readonly property bool drawn: Model.isDrawnWidget(root)

  function liveWidgets() {
    return root.bar && typeof root.bar.moduleWidgets === "function" ? root.bar.moduleWidgets(root.moduleName) : []
  }

  function claimIpc(leaving) {
    if (root === leaving) {
      root.ipcOwner = false
      return
    }
    var pick = Model.pickDrawnWidget(root.liveWidgets(), leaving)
    root.ipcOwner = pick === null || pick === root
  }

  function reclaimIpc(leaving) {
    var items = root.liveWidgets()
    if (items.indexOf(root) === -1) items = items.concat([root])
    var owners = items.filter(function(w) { return !!w && w.ipcOwner === true })
    var others = items.filter(function(w) { return !!w && w.ipcOwner !== true })
    var order = owners.concat(others)
    for (var i = 0; i < order.length; i++)
      if (order[i] !== leaving && typeof order[i].claimIpc === "function") order[i].claimIpc(leaving)
  }

  onDrawnChanged: Qt.callLater(root.reclaimIpc, null)
  Component.onCompleted: Qt.callLater(root.reclaimIpc, null)
  Component.onDestruction: {
    if (!root.ipcOwner) return
    // Let go first, so the next owner's handler is the only one.
    root.ipcOwner = false
    root.reclaimIpc(root)
  }

  implicitWidth: button.implicitWidth
  implicitHeight: button.implicitHeight

  onBarChanged: { root.findService(); Qt.callLater(root.reclaimIpc, null) }
  onSettingsChanged: root.pushSettings()
  onServiceChanged: root.pushSettings()
  onCaptureIntervalChanged: root.pushSettings()
  onDriftInBarChanged: root.pushSettings()

  Timer {
    interval: 1000
    repeat: true
    running: !root.service
    triggeredOnStart: true
    onTriggered: root.findService()
  }

  // The shim (ADR-0034 §7): the 0.1.x panel's methods, forwarded to the
  // desk. `tab <name>` and `resolve <target>` open the desk at a section
  // (and an event); `view` reports the desk while it is loaded. None runs
  // the engine.
  IpcHandler {
    target: "jax.seldon.panel"
    enabled: root.ipcOwner

    function open(): void { root.summonDesk({}) }
    function close(): void { root.hideDesk() }
    function show(): void { root.summonDesk({}) }
    function hide(): void { root.hideDesk() }
    function toggle(): void { root.toggleDesk() }
    // What the pill shows right now, for smoke tests (docs/TESTING.md).
    function pill(): string { return root.pillReadout() }
    // What the desk shows (Desk.view), or {"opened":false} while unloaded.
    function view(): string {
      var desk = root.service ? root.service.desk : null
      return desk ? desk.view("") : JSON.stringify({ opened: false })
    }
    // The 0.1.x tabs are desk sections of the same name.
    function tab(name: string): string {
      if (Model.PANEL_TABS.indexOf(name) === -1) return "unknown tab"
      return root.summonDesk({ section: name }) ? "ok" : "unknown tab"
    }
    // The Changelog at an event (or its crisis filter); navigation only.
    function resolve(target: string): string {
      if (target === "crisis") return root.summonDesk({ section: "changelog", filter: "crisis" }) ? "ok" : "unknown target"
      if (!Model.EVENT_ID.test(target)) return "unknown target"
      return root.summonDesk({ section: "changelog", select: target }) ? "ok" : "unknown target"
    }
    // The Changelog with a source filter: all | pacman | snapper | …
    function filter(source: string): string {
      if (source !== "all" && Model.SOURCES.indexOf(source) === -1) return "unknown source"
      return root.summonDesk({ section: "changelog", filter: source }) ? "ok" : "unknown source"
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
      else if (b === Qt.MiddleButton) root.summonDesk({ section: "radiant" })
      else root.toggleDesk()
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
