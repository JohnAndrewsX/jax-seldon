import QtQuick
import QtQuick.Window
import Quickshell
import Quickshell.Io
import qs.Commons
import qs.Ui

// Headless harness for plugin/BarWidget.qml, the pill (docs/TESTING.md,
// "Plugin", 3c). Driven by tests/plugin/bar-view.sh.
//
// Loads Service.qml (dev mode, SELDON_INDEX) and BarWidget.qml the way the
// bar does: the widget gets the shell's own PluginBarApi facade, bound to
// the theme's bar colours and the bar font, and finds the service through
// `bar.shell.serviceFor()`. The widget sits in a strip one bar tall
// (Style.bar.sizeHorizontal) on the bar background. Once the
// service is ready and the glyph image has loaded, it prints one report and,
// with HARNESS_SHOT set, saves the window to that PNG, then quits. It never
// talks to the running omarchy-shell.
//
// The report (`HARNESS bar {json}`): the pill's IPC read-out, the device
// pixel ratio, the bar and box size, the glyph file, the glyph box and the
// counts text in window coordinates (logical px), the glyph's ink centre
// and the digits' centre as the widget computes them (logical px; for the
// record only, the glyph is placed by that formula), and the ink colour.
// bar-view.sh measures check 4 in the PNG.
//
//   HARNESS_PLUGIN_DIR  absolute path of the plugin folder (required)
//   HARNESS_SHOT        PNG path to save the window to (optional)
//   HARNESS_SETTINGS    the widget's settings (shell.json's entry) as JSON
//                       text, e.g. {"driftInBar":"none"} (ADR-0028 §4a)
//   HARNESS_IPC         config path of this harness: two widgets, as the bar
//                       builds one per monitor, and the IPC target driven
//                       through `quickshell ipc` (WP-067). Prints
//                       `HARNESS ipc {json}` instead of the bar report: which
//                       widget owns `jax.seldon.panel`, which one an IPC
//                       `open` reaches, and the same once the owner is gone.
//   HARNESS_IPC_PLACEHOLDER  with HARNESS_IPC: the first widget the bar
//                       lists is a zero-size, hidden placeholder (a module in the
//                       bar's centre section, WP-078), the second is drawn. After
//                       open/close a reconfiguration draws the placeholder
//                       and hides the other (report `ownersSwapped`,
//                       `openedSwapped`), then the owner goes.
//   HARNESS_IPC_KILL    with HARNESS_IPC: once the two widgets are ready,
//                       prints `HARNESS kill-ready {json}` (which widget owns
//                       the target) and waits to be ended from outside by
//                       `quickshell kill`, as `omarchy restart shell` ends the
//                       shell (WP-162). Every change of a widget's ownership,
//                       the teardown included, prints `HARNESS owner <i>
//                       <bool>`. Qt tears the widgets down newest first; the
//                       value `late-owner` creates the second widget before
//                       the owner, so the owner goes first while its sibling
//                       is still there, as in the shell's teardown. The value
//                       `three` lists three widgets (the owner, a hidden
//                       placeholder, a survivor): the owner and the
//                       placeholder go in one turn while both are still
//                       listed, the bar drops them afterwards; the report
//                       `three` holds `ownersAfter`, `openAfter` (an IPC
//                       `open`) and `openedAfter` before `kill-ready`.
//                       Driven by tests/plugin/ipc-restart.sh.
ShellRoot {
  id: root

  property var service: null
  property var widget: null
  // Every widget, for the facade's moduleWidgets (the bar's live instances).
  property var widgets: []
  readonly property string ipcConfig: Quickshell.env("HARNESS_IPC") || ""
  readonly property bool placeholderMode: (Quickshell.env("HARNESS_IPC_PLACEHOLDER") || "") !== ""
  readonly property bool killMode: (Quickshell.env("HARNESS_IPC_KILL") || "") !== ""
  readonly property bool threeMode: Quickshell.env("HARNESS_IPC_KILL") === "three"
  property var ipcReport: ({})
  property bool done: false
  readonly property string pluginDir: Quickshell.env("HARNESS_PLUGIN_DIR") || ""
  readonly property double startMs: Date.now()

  function load(file, parent, props) {
    var component = Qt.createComponent("file://" + root.pluginDir + "/" + file, Component.PreferSynchronous)
    if (component.status !== Component.Ready) {
      console.log("HARNESS error " + component.errorString())
      return null
    }
    return component.createObject(parent, props)
  }

  function byName(item, name) {
    if (!item) return null
    if (item.objectName === name) return item
    var kids = item.children
    for (var i = 0; kids && i < kids.length; i++) {
      var found = root.byName(kids[i], name)
      if (found) return found
    }
    return null
  }

  function rectOf(item) {
    var p = item.mapToItem(win.contentItem, 0, 0)
    return { x: p.x, y: p.y, w: item.width, h: item.height }
  }

  function report() {
    var glyph = root.byName(win.contentItem, "seldonGlyph")
    var counts = root.byName(win.contentItem, "seldonCounts")
    var pill = root.byName(win.contentItem, "seldonPill")
    var origin = pill.mapToItem(win.contentItem, 0, 0)
    console.log("HARNESS bar " + JSON.stringify({
      pill: JSON.parse(root.widget.pillReadout()),
      service: { pill: root.service.snapshot().pill, driftInBar: root.service.driftInBar },
      dpr: Screen.devicePixelRatio,
      barSize: Style.bar.sizeHorizontal,
      fontSize: Style.font.body,
      box: glyph.width,
      file: glyph.file,
      crisp: glyph.crisp,
      ready: glyph.ready,
      glyphRect: root.rectOf(glyph),
      countsRect: root.rectOf(counts),
      glyphCentre: origin.y + pill.glyphCentre,
      digitCentre: origin.y + pill.digitCentre,
      ink: String(pill.ink)
    }))
  }

  // The shell's facade for third-party bar widgets, as Bar.qml binds it.
  PluginBarApi {
    id: api
    pluginId: "jax.seldon"
    moduleName: "jax.seldon"
    shell: shellFacade
    foreground: Color.bar.text
    barForeground: Color.bar.text
    background: Color.bar.background
    urgent: Color.bar.active
    fontFamily: Style.font.family
    barSize: Style.bar.sizeHorizontal
    foregroundAnimationEnabled: false
    _moduleWidgets: function(id) { return id === "jax.seldon" ? root.widgets.filter(function(w) { return !!w }) : [] }
  }

  QtObject {
    id: shellFacade
    function serviceFor(id) { return id === "jax.seldon" ? root.service : null }
    function toggle(id, payload) {}
  }

  Window {
    id: win
    visible: true
    width: 240
    // A whole number of device pixels at 1.0 and 1.25 (26 × 1.25 = 32.5
    // would be rounded to 33 and the grab stretched by 33 / 32.5); the bar
    // strip below is the real bar height.
    height: 4 * Math.ceil(Style.bar.sizeHorizontal / 4)

    // The bar background under the widget (grabToImage leaves out the
    // window colour).
    Rectangle {
      anchors.fill: parent
      color: Color.bar.background
    }

    Item {
      id: slot
      x: 8
      width: root.widget ? root.widget.implicitWidth : 0
      height: Style.bar.sizeHorizontal
    }

    // The second monitor's bar (HARNESS_IPC only).
    Item {
      id: slot2
      x: 120
      width: 100
      height: Style.bar.sizeHorizontal
    }

    // A centre-section module's placeholder (HARNESS_IPC_PLACEHOLDER):
    // hidden and zero-size until the reconfiguration step draws it.
    Item {
      id: slot0
      x: 120
      width: 0
      height: 0
      visible: false
    }
  }

  // The second widget of HARNESS_IPC: the centre placeholder
  // (HARNESS_IPC_PLACEHOLDER) or the second monitor's.
  function loadOther() {
    if (!root.placeholderMode) return root.load("BarWidget.qml", slot2, { bar: api, moduleName: "jax.seldon" })
    return root.loadPlaceholder()
  }

  function loadPlaceholder() {
    var placeholder = root.load("BarWidget.qml", slot0, { bar: api, moduleName: "jax.seldon" })
    if (placeholder) placeholder.anchors.fill = slot0
    return placeholder
  }

  // HARNESS_IPC_KILL=three: the owner and the placeholder go in one turn,
  // both still listed while they are torn down; the bar's list drops them
  // on the next turn.
  function dropOwnerAndPlaceholder() {
    var owner = root.widgets[0]
    var placeholder = root.widgets[1]
    owner.destroy()
    placeholder.destroy()
    Qt.callLater(function() { root.widgets = [null, null, root.widgets[2]] })
  }

  function killReady(extra) {
    var r = { owners: root.owners() }
    if (extra) r.three = extra
    console.log("HARNESS kill-ready " + JSON.stringify(r))
  }

  function watchOwner(w, i) {
    w.ipcOwnerChanged.connect(function() { console.log("HARNESS owner " + i + " " + w.ipcOwner) })
  }

  Component.onCompleted: {
    root.service = root.load("Service.qml", null, {})
    var settingsJson = Quickshell.env("HARNESS_SETTINGS") || ""
    var props = { bar: api, moduleName: "jax.seldon" }
    if (settingsJson !== "") props.settings = JSON.parse(settingsJson)
    // HARNESS_IPC_KILL=late-owner: the other widget first (see the top).
    var other = root.ipcConfig !== "" && Quickshell.env("HARNESS_IPC_KILL") === "late-owner" ? root.loadOther() : null
    root.widget = root.load("BarWidget.qml", slot, props)
    if (root.widget) root.widget.anchors.fill = slot
    var all = [root.widget]
    if (root.ipcConfig !== "") {
      if (!other) other = root.loadOther()
      if (root.placeholderMode) all.unshift(other)
      else all.push(other)
    }
    if (root.threeMode) all.splice(1, 0, root.loadPlaceholder())
    if (root.killMode) all.forEach(function(w, i) { if (w) root.watchOwner(w, i) })
    root.widgets = all
  }

  // ---- HARNESS_IPC: phases on a timer, each IPC call through Process.
  property int ipcPhase: 0
  property string ipcPending: ""

  function owners() { return root.widgets.map(function(w) { return w ? w.ipcOwner : null }) }
  function openedState() { return root.widgets.map(function(w) { return w ? w.opened : null }) }

  function ipcCall(tag, args) {
    root.ipcPending = tag
    ipcProc.command = ["quickshell", "ipc", "-p", root.ipcConfig, "call", "jax.seldon.panel"].concat(args)
    ipcProc.running = true
  }

  Process {
    id: ipcProc
    stdout: StdioCollector { id: ipcOut }
    stderr: StdioCollector { id: ipcErr }
    onExited: function(code) {
      var r = root.ipcReport
      r[root.ipcPending] = { exit: code, out: ipcOut.text.trim(), err: ipcErr.text.trim() }
      root.ipcReport = r
      root.ipcPending = ""
      ipcStep.restart()
    }
  }

  // The owner's monitor goes away.
  function dropOwner() {
    var i = root.owners().indexOf(true)
    if (i === -1) return
    var gone = root.widgets[i]
    var rest = root.widgets.slice()
    rest[i] = null
    root.widgets = rest
    gone.destroy()
  }

  // Each step records what the last one left and starts the next action:
  // an IPC call (the next step follows its exit) or a local change.
  readonly property var ipcSteps: {
    var head = [
      function(r) { r.owners = root.owners(); root.ipcCall("open", ["open"]) },
      function(r) { r.opened = root.openedState(); root.ipcCall("close", ["close"]) }
    ]
    var swap = [
      function(r) {
        // A live reconfiguration: the placeholder is drawn, the other hidden.
        r.closed = root.openedState()
        slot0.width = 100
        slot0.height = Style.bar.sizeHorizontal
        slot0.visible = true
        slot.visible = false
        ipcStep.restart()
      },
      function(r) { r.ownersSwapped = root.owners(); root.ipcCall("openSwapped", ["open"]) },
      function(r) { r.openedSwapped = root.openedState(); root.ipcCall("closeSwapped", ["close"]) },
      function(r) { root.dropOwner(); ipcStep.restart() }
    ]
    var drop = [
      function(r) { r.closed = root.openedState(); root.dropOwner(); ipcStep.restart() }
    ]
    var tail = [
      function(r) { r.ownersAfter = root.owners(); root.ipcCall("openAfter", ["open"]) },
      function(r) {
        r.openedAfter = root.openedState()
        console.log("HARNESS ipc " + JSON.stringify(r))
        Qt.quit()
      }
    ]
    var three = [
      function(r) { r.owners = root.owners(); root.dropOwnerAndPlaceholder(); ipcStep.restart() },
      function(r) { r.ownersAfter = root.owners(); root.ipcCall("openAfter", ["open"]) },
      function(r) { r.openedAfter = root.openedState(); root.killReady(r) }
    ]
    if (root.threeMode) return three
    return head.concat(root.placeholderMode ? swap : drop, tail)
  }

  Timer {
    id: ipcStep
    interval: 300
    onTriggered: {
      var r = root.ipcReport
      var step = root.ipcSteps[root.ipcPhase]
      root.ipcPhase++
      if (step) step(r)
      root.ipcReport = r
    }
  }

  Timer {
    interval: 100
    repeat: true
    running: !root.done
    onTriggered: {
      var glyph = root.byName(win.contentItem, "seldonGlyph")
      var waited = Date.now() - root.startMs
      var ready = root.service && root.service.ready && root.widget && root.widget.service && glyph && glyph.ready
      if ((!ready || waited < 1000) && waited < 15000) return
      root.done = true
      if (root.ipcConfig !== "" && root.killMode && !root.threeMode) {
        root.killReady(null)
        return
      }
      if (root.ipcConfig !== "") {
        ipcStep.start()
        return
      }
      root.report()
      var shot = Quickshell.env("HARNESS_SHOT") || ""
      if (shot === "") {
        Qt.quit()
        return
      }
      win.contentItem.grabToImage(function(result) {
        result.saveToFile(shot)
        Qt.quit()
      })
    }
  }
}
