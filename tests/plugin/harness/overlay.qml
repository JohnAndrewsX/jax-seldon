import QtQuick
import QtQuick.Window
import QtTest
import Quickshell
import qs.Commons

// Headless harness for plugin/Overlay.qml, the Prime Radiant
// (docs/TESTING.md, "Plugin").
//
// tests/plugin/overlay-view.sh copies this file as shell.qml into a temp
// config root next to copies of the installed shell's Commons/ and Ui/, and
// runs it against a copy of the plugin whose layer-shell window
// (components/overlay/OverlayWindow.qml) is replaced by a stand-in Item. It
// loads Service.qml as the shell does, puts Overlay.qml in an offscreen
// window of HARNESS_W × HARNESS_H, hands it a stand-in shell facade (whose
// hide() records the id and calls close(), as the shell's hide does), then
// runs HARNESS_STEPS and prints after each: Overlay.view(), the ids hidden
// through the facade, every visible text and every text that leaves its
// slot or the window.
//
//   HARNESS_PLUGIN_DIR  absolute path of the plugin copy (required)
//   HARNESS_W/H         window size in logical pixels (default 1920 × 1080)
//   HARNESS_STEPS       ";"-separated steps, each optionally "*N" repeated:
//                       toggle[:<json>]  what `shell toggle jax.seldon` does:
//                                        hide when opened, else open(json)
//                       fresh[:<json>]   what the shell's overlay Loader does
//                                        on summon: drop the overlay, create a
//                                        new one, open(json); the report's
//                                        `firstFrame` holds the counters
//                                        sampled on its first swapped frame
//                                        and the ms creating and opening it took
//                       hover:<slot>:<fx>,<fy>  move the mouse to that point of
//                                        the slot's chart plot (fractions)
//                       hoverItem:<slot>:<i>  move the mouse to item i of the
//                                        slot's chart (chart.locate(i))
//                       leave            move the mouse to the window corner
//                       resize:<W>x<H>   resize the harness window
//                       summon[:<json>]  open(json), as `shell summon`
//                       hide             close(), as `shell hide`
//                       key:<Left|Right|Escape|Return|Tab>
//                       text:<character> a typed character (1, h, l, …)
//                       click:<text>     click the first visible item whose
//                                        text is <text>
//                       clickAt:<x>,<y>  click that window point
//                       call:<method>:<arg>  what `shell call jax.seldon
//                                        <method> <arg>` does
//                       shot:<name>      save the window as
//                                        $HARNESS_SHOTS/<name>.png
//                       view             no action, just report
ShellRoot {
  id: root

  property var service: null
  property var overlay: null
  readonly property string pluginDir: Quickshell.env("HARNESS_PLUGIN_DIR") || ""
  readonly property var steps: (Quickshell.env("HARNESS_STEPS") || "view").split(";").filter(function(s) { return s !== "" })
  property int step: 0
  property string lastCall: ""
  // fresh: counters before the new overlay exists and on its first frame.
  property var firstFrame: null
  property bool awaitFrame: false
  readonly property var slotIds: ["heatmap", "series", "driftBars", "riskDonut", "timeline", "plan"]
  property var beforeFresh: null

  readonly property var keys: ({
    Left: Qt.Key_Left, Right: Qt.Key_Right, Escape: Qt.Key_Escape, Return: Qt.Key_Return, Tab: Qt.Key_Tab
  })

  // The part of the shell facade Overlay.qml uses.
  QtObject {
    id: fakeShell
    property var hides: []
    function hide(id) {
      fakeShell.hides = fakeShell.hides.concat([id])
      if (root.overlay) root.overlay.close()
      return true
    }
  }

  function load(file, parent, props) {
    var component = Qt.createComponent("file://" + root.pluginDir + "/" + file, Component.PreferSynchronous)
    if (component.status !== Component.Ready) {
      console.log("HARNESS error " + component.errorString())
      return null
    }
    return component.createObject(parent, props)
  }

  function isText(item) {
    return item.text !== undefined && item.font !== undefined && typeof item.text === "string" && item.text !== ""
  }

  // Every visible, non-empty Text under `item`, in tree order.
  function texts(item, out) {
    if (!item || item.visible === false) return out
    if (root.isText(item)) out.push(item.text)
    var kids = item.children
    for (var i = 0; kids && i < kids.length; i++) texts(kids[i], out)
    return out
  }

  // Every visible text that reaches outside the window, or outside the
  // slot it sits in (an elided text counts by its width, any other by
  // what it draws). Tolerance: one pixel.
  function overflow(item, slot, out) {
    if (!item || item.visible === false) return out
    var name = String(item.objectName || "")
    if (name.indexOf("slot:") === 0) {
      var at = item.mapToItem(win.contentItem, 0, 0)
      slot = { name: name, x: at.x, y: at.y, w: item.width, h: item.height }
    }
    if (root.isText(item)) {
      var p = item.mapToItem(win.contentItem, 0, 0)
      var w = item.elide !== Text.ElideNone ? item.width : Math.max(item.width, item.contentWidth)
      var h = Math.max(item.height, item.contentHeight)
      var boxes = [{ name: "window", x: 0, y: 0, w: win.width, h: win.height }]
      if (slot) boxes.push(slot)
      for (var b = 0; b < boxes.length; b++) {
        var r = boxes[b]
        if (p.x < r.x - 1 || p.y < r.y - 1 || p.x + w > r.x + r.w + 1 || p.y + h > r.y + r.h + 1)
          out.push(item.text + " @" + r.name)
      }
    }
    var kids = item.children
    for (var i = 0; kids && i < kids.length; i++) overflow(kids[i], slot, out)
    return out
  }

  // The first visible item, in tree order, whose text is `label`.
  function findText(item, label) {
    if (!item || item.visible === false) return null
    if (item.text === label) return item
    var kids = item.children
    for (var i = 0; kids && i < kids.length; i++) {
      var found = root.findText(kids[i], label)
      if (found) return found
    }
    return null
  }

  function report(tag) {
    var view = root.overlay ? JSON.parse(root.overlay.view("")) : null
    // The tag is one word: overlay-view.sh cuts the line at the first space after it.
    console.log("HARNESS step " + String(tag).replace(/\s/g, "_") + " " + JSON.stringify({
      view: view, hides: fakeShell.hides, call: root.lastCall, firstFrame: root.firstFrame,
      texts: texts(win.contentItem, []), overflow: overflow(win.contentItem, null, [])
    }))
  }

  function act(spec) {
    var colon = spec.indexOf(":")
    var verb = colon === -1 ? spec : spec.slice(0, colon)
    var arg = colon === -1 ? "" : spec.slice(colon + 1)
    if (verb === "fresh") {
      root.firstFrame = null
      if (root.overlay) root.overlay.destroy()
      var started = Date.now()
      root.beforeFresh = { service: root.service.aggregationCount() }
      root.overlay = root.createOverlay()
      root.overlay.open(arg)
      root.beforeFresh.createMs = Date.now() - started
      root.awaitFrame = true
    } else if (verb === "hover") {
      var parts = arg.split(":")
      var xy0 = parts[1].split(",")
      var chart = root.overlay.chartFor(parts[0])
      if (chart) driver.mouseMove(chart.plot, Number(xy0[0]) * chart.plot.width, Number(xy0[1]) * chart.plot.height)
      else console.log("HARNESS nothing to hover: " + parts[0])
    } else if (verb === "hoverItem") {
      var at = arg.split(":")
      var target = root.overlay.chartFor(at[0])
      var point = target ? target.locate(Number(at[1])) : null
      if (point) driver.mouseMove(target.plot, point.x, point.y)
      else console.log("HARNESS nothing to hover: " + arg)
    } else if (verb === "leave") {
      driver.mouseMove(win.contentItem, 0, 0)
    } else if (verb === "resize") {
      var wh = arg.split("x")
      win.width = Number(wh[0])
      win.height = Number(wh[1])
    } else if (verb === "toggle") {
      if (root.overlay.opened) fakeShell.hide("jax.seldon")
      else root.overlay.open(arg)
    } else if (verb === "summon") {
      root.overlay.open(arg)
    } else if (verb === "hide") {
      root.overlay.close()
    } else if (verb === "key") {
      driver.keyClick(root.keys[arg])
    } else if (verb === "text") {
      driver.keyClick(arg)
    } else if (verb === "click") {
      var target = root.findText(win.contentItem, arg)
      if (target) driver.mouseClick(target)
      else console.log("HARNESS nothing to click: " + arg)
    } else if (verb === "clickAt") {
      var xy = arg.split(",")
      driver.mouseClick(win.contentItem, Number(xy[0]), Number(xy[1]))
    } else if (verb === "call") {
      var sep = arg.indexOf(":")
      var method = sep === -1 ? arg : arg.slice(0, sep)
      var value = sep === -1 ? "" : arg.slice(sep + 1)
      var result = typeof root.overlay[method] === "function" ? root.overlay[method](value) : "unknown"
      root.lastCall = result === undefined || result === null ? "ok" : String(result)
    } else if (verb === "shot") {
      var dir = Quickshell.env("HARNESS_SHOTS") || ""
      if (dir !== "") win.contentItem.grabToImage(function(r) { r.saveToFile(dir + "/" + arg + ".png") })
    }
  }

  Window {
    id: win
    visible: true
    width: Number(Quickshell.env("HARNESS_W") || 1920)
    height: Number(Quickshell.env("HARNESS_H") || 1080)

    // What lies under the scrim, for `shot:` (grabToImage leaves out the
    // window colour).
    Rectangle {
      anchors.fill: parent
      color: Color.background
    }
  }

  TestCase {
    id: driver
    name: "overlay"
    when: false
    running: false
  }

  function createOverlay() {
    var o = root.load("Overlay.qml", win.contentItem, { service: root.service, shell: fakeShell,
      manifest: { id: "jax.seldon" } })
    if (o) o.anchors.fill = win.contentItem
    return o
  }

  // The first frame after a `fresh` open: what has been aggregated and
  // painted by then.
  Connections {
    target: win
    function onFrameSwapped() {
      if (!root.awaitFrame || !root.overlay) return
      var paints = root.slotIds.map(function(id) {
        var c = root.overlay.chartFor(id)
        return c ? c.paints : -1
      })
      var frame = root.firstFrame
      if (frame === null) {
        frame = {
          serviceBefore: root.beforeFresh.service,
          service: root.service.aggregationCount(),
          overlay: root.overlay.aggregationCount(),
          opened: root.overlay.opened,
          createMs: root.beforeFresh.createMs,
          paints: paints,
          frames: 0,
          paintedBy: 0
        }
      }
      // Frames since the open, and the one by which every chart has painted.
      frame.frames++
      if (paints.every(function(n) { return n >= 1 })) frame.paintedBy = frame.frames
      root.awaitFrame = frame.paintedBy === 0 && frame.frames < 10
      root.firstFrame = frame
    }
  }

  Component.onCompleted: {
    root.service = root.load("Service.qml", null, {})
    root.overlay = root.createOverlay()
  }

  readonly property double startMs: Date.now()

  // Let the service read the index and probe the engine (up to 15 s on a
  // loaded machine), then run the steps.
  Timer {
    id: stepper
    interval: 100
    running: true
    onTriggered: {
      var s = root.service
      var waited = Date.now() - root.startMs
      if (root.step === 0 && waited < 15000 && (waited < 1000 || !s || !s.ready || s.probing)) {
        stepper.restart()
        return
      }
      if (root.step >= root.steps.length) {
        root.report("final")
        Qt.quit()
        return
      }
      var spec = root.steps[root.step]
      var times = 1
      var star = spec.lastIndexOf("*")
      if (star !== -1) {
        times = Number(spec.slice(star + 1))
        spec = spec.slice(0, star)
      }
      for (var i = 0; i < times; i++) root.act(spec)
      root.step++
      stepper.interval = 150
      // Report after the bindings and Qt.callLater work of this step settled.
      Qt.callLater(function() { root.report(root.steps[root.step - 1]) })
      stepper.restart()
    }
  }
}
