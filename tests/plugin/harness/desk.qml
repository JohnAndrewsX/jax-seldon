import QtQuick
import QtQuick.Window
import QtTest
import Quickshell
import qs.Commons
import qs.Ui

// Headless harness for plugin/Desk.qml, the desk (docs/TESTING.md, "Plugin").
//
// tests/plugin/desk-view.sh copies this file as shell.qml into a temp config
// root next to copies of the installed shell's Commons/ and Ui/, and runs it
// against a copy of the plugin whose layer-shell window
// (components/desk/DeskWindow.qml) is replaced by a stand-in Item filling
// the harness window. It loads Service.qml as the shell does, and the pill
// (BarWidget.qml) in a strip with the shell's PluginBarApi, both on one
// stand-in of the plugin's scoped facade that behaves like shell.qml's
// overlay loader: summon creates Desk.qml bare (no properties), injects
// shell, manifest and service, then calls open(payload); hide calls close()
// and drops the item; toggle is hide when open, else summon. Its
// updateEntryInline records every settings write and, as shell.qml's
// does, returns false when the entry would not change; otherwise, unless
// HARNESS_REFUSE is set, it hands the new entry back to the pill as the
// shell's reload of shell.json does (the pill pushes it to the service).
// Then it runs HARNESS_STEPS and prints after each: Desk.view() (or
// {"opened":false} while unloaded), the facade's calls, the writes, the
// last call's result, the service's launch read-out (`service`: how often
// the desk stepped aside, the live sessions, the open and plan results;
// WP-156), every visible text and every text outside the
// window, the desk or the Prime Radiant slot it sits in.
//
//   HARNESS_PLUGIN_DIR  absolute path of the plugin copy (required)
//   HARNESS_W/H         window size in logical pixels (default 1920 × 1080)
//   HARNESS_SETTINGS    the widget's shell.json entry as JSON (the pill's
//                       `settings`), e.g. {"deskWidth":67}
//   HARNESS_MANIFEST    a manifest as JSON text, assigned to the service's
//                       `manifest` (the restart notice compares versions)
//   HARNESS_REFUSE      if set, updateEntryInline returns false
//   HARNESS_NO_PILL     if set, no pill: the plugin is enabled but not in
//                       the bar, so nothing pushes an entry to the service
//   HARNESS_STEPS       ";"-separated steps, each optionally "*N" repeated:
//                       summon[:<json>]  `shell summon jax.seldon <json>`
//                       fresh[:<json>]   drop the desk and summon a new one
//                                        (the loader's path from closed);
//                                        the report's `firstFrame` holds the
//                                        Prime Radiant's counters sampled on
//                                        its first swapped frame
//                       hide             `shell hide jax.seldon`
//                       toggle[:<json>]  `shell toggle jax.seldon <json>`
//                       pill:<left|middle|right>  a click on the pill
//                       shim:<method>[:<arg>]  the pill's jax.seldon.panel
//                                        IPC method (tab, resolve, view, …)
//                       call:<method>:<arg>  `shell call jax.seldon …`
//                       service:<method>:<arg>  a Service method, called
//                                        directly (its result in `call`)
//                       timedClickName:<objectName>  a click; `call` holds
//                                        the milliseconds its handlers took
//                       section:<id>     Desk.section(id)
//                       select:<id>      Desk.select(id)
//                       width:<pct>      Omarchy's bar settings set deskWidth
//                       sidebar:<mode>   … and deskSidebar
//                       resize:<W>x<H>   resize the harness window
//                       key:[Alt+]<Name> a key (Up, Down, Left, Right,
//                                        Return, Space, Escape, Tab, Backtab)
//                       text:<char>      a typed character
//                       keyDown:<Name|char>  press a key (Up, …, or a
//                                        character) and keep it down
//                       keyRepeat:<Name|char>  one auto-repeated press of
//                                        that key, as Hyprland sends while
//                                        it is held (QtTest makes none):
//                                        an event object with isAutoRepeat
//                                        handed to keyPressed(event) of the
//                                        key guard the focus is in (below)
//                       keyUp:<Name|char>  release it
//                       focusName:<objectName>  give that item the focus
//                       (every report: `keyGuard`, the key guard the focus
//                       is in — the nearest item up from the focus with a
//                       keyPressed(event) and a `keyGuard` name, else the
//                       desk — as { name, events }: a real key that counts
//                       there went through that guard's Keys handler)
//                       type:<text>      each character of text, typed
//                       click:<text>     click the first visible item whose
//                                        text is <text>
//                       clickName:<objectName>  … whose objectName is that
//                       trigger:<objectName>:<id>  that item's actionTriggered(id), no button (WP-102b)
//                       clickAt:<x>,<y>  click that window point
//                       drag:<objectName>:<f1>,<f2>  press at fraction f1 of
//                                        the item's width, move to f2 in
//                                        steps (a report per move is not
//                                        made), report, then release
//                       release          release the drag's mouse button
//                       wheel:<objectName>:<delta>  one mouse wheel event
//                                        over the item's centre (-120 is one
//                                        notch down)
//                       pause:<ms>       wait that long, then report
//                       hover:<text>     move the pointer onto that text
//                       hoverItem:<slot>:<i>  move the pointer to item i of
//                                        the Prime Radiant chart in that slot
//                                        (chart.locate(i))
//                       graphPlay        the graph's Play (Space)
//                       graphCut:<day>   the graph's cut-off day (days
//                                        since its first)
//                       graphHover:<id>  move the pointer onto graph node id
//                       graphDrag:<id|empty>:<dx>,<dy>  press on node id (or
//                                        a point with no node: a pan), move
//                                        by dx,dy in four steps, release;
//                                        `call` holds { from, to }, the
//                                        node's window point after
//                       leave            move the pointer to the window corner
//                       settle           wait (up to 15 s) until no engine
//                                        call is queued or running
//                       wait:<path>=<v>  wait (up to 15 s) until the view's
//                                        dotted path is v (`^=`: starts
//                                        with v; a step holds no `;`)
//                       shot:<name>      save the window as
//                                        $HARNESS_SHOTS/<name>.png
//                       touch:<name>     create $HOME/<name> (a-z and -):
//                                        lets a held fake engine call go
//                                        (FAKE_SELDON_HOLD_OPEN)
//                       view             no action, just report
ShellRoot {
  id: root

  property var service: null
  property var desk: null
  property var widget: null
  readonly property string pluginDir: Quickshell.env("HARNESS_PLUGIN_DIR") || ""
  readonly property var steps: (Quickshell.env("HARNESS_STEPS") || "view").split(";").filter(function(s) { return s !== "" })
  readonly property bool refuse: (Quickshell.env("HARNESS_REFUSE") || "") !== ""
  readonly property bool noPill: (Quickshell.env("HARNESS_NO_PILL") || "") !== ""
  property int step: 0
  property string lastCall: ""
  // The last creation of the desk saw no service (as the shell's loader).
  property bool bare: false
  property var entry: ({})
  property var dragItem: null
  property real dragY: 0
  property real dragX: 0
  property string waitFor: ""
  property double waitSince: 0
  // fresh: the counters before the new desk exists and on its first frame.
  property var firstFrame: null
  property bool awaitFrame: false
  property var beforeFresh: null
  readonly property var slotIds: ["heatmap", "series", "driftBars", "riskDonut", "timeline", "plan"]

  readonly property var keys: ({
    Up: Qt.Key_Up, Down: Qt.Key_Down, Left: Qt.Key_Left, Right: Qt.Key_Right, Return: Qt.Key_Return,
    Space: Qt.Key_Space, Escape: Qt.Key_Escape, Tab: Qt.Key_Tab, Backtab: Qt.Key_Backtab, Backspace: Qt.Key_Backspace
  })

  // The plugin's scoped facade (services/PluginShellApi.qml), with the
  // overlay loader's behaviour behind summon / hide / toggle.
  QtObject {
    id: fakeShell
    property var calls: []
    property var writes: []

    function record(text) { fakeShell.calls = fakeShell.calls.concat([text]) }
    function serviceFor(id) { return id === "jax.seldon" ? root.service : null }
    function isPluginOpen(id) { return !!root.desk && root.desk.opened === true }
    function summon(id, payload) {
      fakeShell.record("summon " + id + " " + payload)
      if (!root.desk) root.desk = root.createDesk()
      if (!root.desk) return false
      root.desk.open(payload)
      return true
    }
    function hide(id) {
      fakeShell.record("hide " + id)
      var d = root.desk
      if (!d) return true
      d.close()
      root.desk = null
      d.destroy()
      return true
    }
    function toggle(id, payload) {
      fakeShell.record("toggle " + id)
      return fakeShell.isPluginOpen(id) ? fakeShell.hide(id) : fakeShell.summon(id, payload)
    }
    function updateEntryInline(id, settings) {
      fakeShell.writes = fakeShell.writes.concat([{ id: id, settings: settings }])
      if (root.refuse) return false
      var next = ({})
      for (var k in settings) next[k] = settings[k]
      // shell.qml updateEntryInline: nothing changed, nothing persisted, false.
      if (root.sameEntry(root.entry, next)) return false
      reload.next = next
      reload.restart()
      return true
    }
  }

  // The shell persists shell.json, its FileView reloads it and the bar
  // hands the widget its new entry.
  Timer {
    id: reload
    property var next: null
    interval: 30
    onTriggered: root.setEntry(reload.next)
  }

  // The same keys with the same values, the id left out.
  function sameEntry(a, b) {
    var keys = function(o) { return Object.keys(o || {}).filter(function(k) { return k !== "id" }).sort() }
    var ka = keys(a)
    var kb = keys(b)
    if (ka.join(",") !== kb.join(",")) return false
    for (var i = 0; i < ka.length; i++)
      if (JSON.stringify(a[ka[i]]) !== JSON.stringify(b[ka[i]])) return false
    return true
  }

  function setEntry(next) {
    root.entry = next
    if (root.widget) root.widget.settings = next
  }

  function load(file, parent, props) {
    var component = Qt.createComponent("file://" + root.pluginDir + "/" + file, Component.PreferSynchronous)
    if (component.status !== Component.Ready) {
      console.log("HARNESS error " + component.errorString())
      return null
    }
    return component.createObject(parent, props)
  }

  // As the shell's overlay Loader does it ($OMARCHY_PATH/shell/shell.qml,
  // onLoaded): create the item bare, then inject.
  function createDesk() {
    var d = root.load("Desk.qml", win.contentItem, {})
    if (!d) return null
    root.bare = d.service === null
    d.anchors.fill = win.contentItem
    d.shell = fakeShell
    d.manifest = { id: "jax.seldon" }
    d.service = root.service
    return d
  }

  function isText(item) {
    return item.text !== undefined && item.font !== undefined && typeof item.text === "string" && item.text !== ""
  }

  function texts(item, out) {
    if (!item || item.visible === false) return out
    if (root.isText(item)) out.push(item.text)
    var kids = item.children
    for (var i = 0; kids && i < kids.length; i++) texts(kids[i], out)
    return out
  }

  // Every visible text that reaches outside the window, outside the desk
  // card it sits in, or outside its Prime Radiant slot. An elided text
  // counts by its box. Inside an item that clips (a list scrolled, a detail
  // flicked, the Prime Radiant's grid scrolled), only the part inside the
  // clip counts: rows scrolled out of a list are not on screen.
  // Tolerance 1 px.
  function overflow(item, box, out, clipBox, slot) {
    if (!item || item.visible === false) return out
    var name = String(item.objectName || "")
    if (name === "desk" || name.indexOf("slot:") === 0) {
      var at = item.mapToItem(win.contentItem, 0, 0)
      var r0 = { name: name, x: at.x, y: at.y, w: item.width, h: item.height }
      if (name === "desk") box = r0
      else slot = r0
    }
    if (item.clip === true) {
      var c = item.mapToItem(win.contentItem, 0, 0)
      clipBox = root.intersect(clipBox, { x: c.x, y: c.y, w: item.width, h: item.height })
    }
    if (root.isText(item)) {
      var p = item.mapToItem(win.contentItem, 0, 0)
      var w = item.elide !== Text.ElideNone ? item.width : Math.max(item.width, item.contentWidth)
      var h = Math.max(item.height, item.contentHeight)
      var rect = root.intersect(clipBox, { x: p.x, y: p.y, w: w, h: h })
      var boxes = [{ name: "window", x: 0, y: 0, w: win.width, h: win.height }]
      if (box) boxes.push(box)
      if (slot) boxes.push(slot)
      for (var b = 0; rect && b < boxes.length; b++) {
        var r = boxes[b]
        if (rect.x < r.x - 1 || rect.y < r.y - 1 || rect.x + rect.w > r.x + r.w + 1 || rect.y + rect.h > r.y + r.h + 1)
          out.push(item.text + " @" + r.name)
      }
    }
    var kids = item.children
    for (var i = 0; kids && i < kids.length; i++) overflow(kids[i], box, out, clipBox, slot)
    return out
  }

  // The overlap of a clip and a box: b itself without a clip (undefined),
  // null when the clip is empty (null) or they do not overlap.
  function intersect(a, b) {
    if (a === undefined) return b
    if (a === null) return null
    var x = Math.max(a.x, b.x)
    var y = Math.max(a.y, b.y)
    var w = Math.min(a.x + a.w, b.x + b.w) - x
    var h = Math.min(a.y + a.h, b.y + b.h) - y
    return w > 0 && h > 0 ? { x: x, y: y, w: w, h: h } : null
  }

  // The Prime Radiant section of the loaded desk, if it was made.
  function radiant() {
    return root.desk ? root.desk.sectionItem("radiant") : null
  }

  // The graph section (desk section 8), null before its first visit.
  function graph() {
    return root.desk ? root.desk.sectionItem("graph") : null
  }

  function chartFor(id) {
    var r = root.radiant()
    return r ? r.chartFor(id) : null
  }

  function find(item, test) {
    if (!item || item.visible === false) return null
    if (test(item)) return item
    var kids = item.children
    for (var i = 0; kids && i < kids.length; i++) {
      var found = root.find(kids[i], test)
      if (found) return found
    }
    return null
  }

  function findText(label) {
    return root.find(win.contentItem, function(it) { return it.text === label })
  }

  function findName(name) {
    return root.find(win.contentItem, function(it) { return String(it.objectName || "") === name })
  }

  function viewObject() {
    return root.desk ? JSON.parse(root.desk.view("")) : { opened: false }
  }

  function pathValue(obj, path) {
    var parts = path.split(".")
    var v = obj
    for (var i = 0; i < parts.length; i++) {
      if (v === null || v === undefined) return undefined
      v = v[parts[i]]
    }
    return v
  }

  function report(tag) {
    console.log("HARNESS step " + String(tag).replace(/\s/g, "_") + " " + JSON.stringify({
      view: root.viewObject(), calls: fakeShell.calls, keyGuard: root.keyGuardView(), writes: fakeShell.writes, entry: root.entry,
      call: root.lastCall, bare: root.bare, firstFrame: root.firstFrame,
      graphBuilds: root.service ? root.service.graphBuilds : null,
      graphDirty: root.service ? root.service.graphDirty : null,
      graphNodes: root.service && root.service.graph ? root.service.graph.nodes.length : null,
      pill: root.widget ? JSON.parse(root.widget.pillReadout()) : null,
      service: root.service ? {
        stepAsides: root.service.stepAsides,
        sessions: Object.keys(root.service.agentSessions || {}).sort(),
        open: root.service.openResult ? root.service.openResult.text : "",
        openPending: !!root.service.openResult && root.service.openResult.pending,
        plan: root.service.planResult ? root.service.planResult.text : "",
        planOk: !!root.service.planResult && root.service.planResult.ok,
        planPending: !!root.service.planResult && root.service.planResult.pending,
        busyRefusals: root.service.busyRefusals
      } : null,
      deskCalls: root.widget ? root.widget.deskCalls : 0,
      texts: texts(win.contentItem, []), overflow: overflow(win.contentItem, null, [], undefined)
    }))
  }

  // The IpcHandler inside the pill (its `jax.seldon.panel` target).
  function shimHandler() {
    var list = root.widget ? root.widget.data : []
    for (var i = 0; i < list.length; i++)
      if (list[i] && list[i].target === "jax.seldon.panel") return list[i]
    return null
  }

  // One auto-repeated press (keyRepeat:): what keyPressed(event) reads of
  // a KeyEvent. Accepted, the key stops there, as a real one would.
  function keyRepeat(name) {
    var named = root.keys[name] !== undefined
    var event = {
      key: named ? root.keys[name] : name.toUpperCase().charCodeAt(0),
      text: named ? (name === "Return" ? "\r" : name === "Space" ? " " : "") : name,
      modifiers: Qt.NoModifier,
      isAutoRepeat: true,
      count: 1,
      accepted: false
    }
    var guard = root.keyGuardItem()
    if (guard) guard.keyPressed(event)
    else console.log("HARNESS nothing to repeat: " + name)
  }

  // The key guard the focus is in (WP-173): the nearest item up from the
  // focus with keyPressed(event) and a keyGuard name, else the desk.
  function keyGuardView() {
    var g = root.keyGuardItem()
    return g ? { name: g.keyGuard, events: g.keyEvents } : null
  }

  function keyGuardItem() {
    for (var it = win.activeFocusItem; it; it = it.parent) {
      if (typeof it.keyPressed === "function" && typeof it.keyGuard === "string") return it
    }
    return root.desk
  }

  function act(spec) {
    var colon = spec.indexOf(":")
    var verb = colon === -1 ? spec : spec.slice(0, colon)
    var arg = colon === -1 ? "" : spec.slice(colon + 1)
    if (verb === "summon") {
      fakeShell.summon("jax.seldon", arg)
    } else if (verb === "fresh") {
      root.firstFrame = null
      if (root.desk) fakeShell.hide("jax.seldon")
      var started = Date.now()
      root.beforeFresh = { service: root.service.aggregationCount() }
      fakeShell.summon("jax.seldon", arg)
      root.beforeFresh.createMs = Date.now() - started
      root.awaitFrame = true
    } else if (verb === "hoverItem") {
      var hi = arg.split(":")
      var chart = root.chartFor(hi[0])
      var point = chart ? chart.locate(Number(hi[1])) : null
      if (point) driver.mouseMove(chart.plot, point.x, point.y)
      else console.log("HARNESS nothing to hover: " + arg)
    } else if (verb === "graphPlay") {
      var gp = root.graph()
      if (gp) gp.play()
      else console.log("HARNESS nothing to play")
    } else if (verb === "graphCut") {
      var gc = root.graph()
      if (gc) gc.setCut(Number(arg))
      else console.log("HARNESS nothing to cut")
    } else if (verb === "graphHover") {
      var gh = root.graph()
      var hp = gh ? gh.nodePoint(arg) : null
      if (hp) driver.mouseMove(win.contentItem, hp.x, hp.y)
      else console.log("HARNESS nothing to hover: " + arg)
    } else if (verb === "graphDrag") {
      // The id may hold ":" (area:<name>, fold:…): the delta is after the last.
      var gcut = arg.lastIndexOf(":")
      var gparts = [arg.slice(0, gcut), arg.slice(gcut + 1)]
      var gd = root.graph()
      var from = !gd ? null : gparts[0] === "empty" ? gd.emptyPoint() : gd.nodePoint(gparts[0])
      if (!from) {
        console.log("HARNESS nothing to drag: " + gparts[0])
        return
      }
      var delta = gparts[1].split(",").map(Number)
      driver.mousePress(win.contentItem, from.x, from.y)
      for (var gs = 1; gs <= 4; gs++)
        driver.mouseMove(win.contentItem, from.x + delta[0] * gs / 4, from.y + delta[1] * gs / 4)
      driver.mouseRelease(win.contentItem, from.x + delta[0], from.y + delta[1])
      var to = gparts[0] === "empty" ? null : gd.nodePoint(gparts[0])
      root.lastCall = JSON.stringify({ from: { x: Math.round(from.x), y: Math.round(from.y) },
        to: to ? { x: Math.round(to.x), y: Math.round(to.y) } : null })
    } else if (verb === "leave") {
      driver.mouseMove(win.contentItem, 0, 0)
    } else if (verb === "touch") {
      // a file the fake engine waits for (FAKE_SELDON_HOLD_OPEN)
      if (/^[a-z-]+$/.test(arg)) Quickshell.execDetached(["touch", Quickshell.env("HOME") + "/" + arg])
      else console.log("HARNESS error touch: " + arg)
    } else if (verb === "hide") {
      fakeShell.hide("jax.seldon")
    } else if (verb === "toggle") {
      fakeShell.toggle("jax.seldon", arg)
    } else if (verb === "pill") {
      var button = arg === "middle" ? Qt.MiddleButton : arg === "right" ? Qt.RightButton : Qt.LeftButton
      driver.mouseClick(root.widget, root.widget.width / 2, root.widget.height / 2, button)
    } else if (verb === "shim") {
      var sep0 = arg.indexOf(":")
      var m0 = sep0 === -1 ? arg : arg.slice(0, sep0)
      var a0 = sep0 === -1 ? undefined : arg.slice(sep0 + 1)
      var h = root.shimHandler()
      var r0 = !h ? "no handler" : a0 === undefined ? h[m0]() : h[m0](a0)
      root.lastCall = r0 === undefined || r0 === null ? "ok" : String(r0)
    } else if (verb === "service") {
      // service:<method>:<arg> — a Service method called directly (the
      // guards behind a disabled button; WP-124b)
      var ssep = arg.indexOf(":")
      var smethod = ssep === -1 ? arg : arg.slice(0, ssep)
      var sarg = ssep === -1 ? "" : arg.slice(ssep + 1)
      var sres = root.service && typeof root.service[smethod] === "function" ? root.service[smethod](sarg) : "unknown"
      root.lastCall = sres === undefined || sres === null ? "ok" : String(sres)
    } else if (verb === "timedClickName") {
      // a click and the milliseconds its handlers took (WP-124b, R2)
      var timed = root.findName(arg)
      if (timed) {
        var t0 = Date.now()
        driver.mouseClick(timed)
        root.lastCall = String(Date.now() - t0)
      } else {
        console.log("HARNESS nothing to click: " + arg)
      }
    } else if (verb === "call") {
      var sep = arg.indexOf(":")
      var method = sep === -1 ? arg : arg.slice(0, sep)
      var value = sep === -1 ? "" : arg.slice(sep + 1)
      var result = root.desk && typeof root.desk[method] === "function" ? root.desk[method](value) : "unknown"
      root.lastCall = result === undefined || result === null ? "ok" : String(result)
    } else if (verb === "section") {
      root.lastCall = root.desk ? String(root.desk.section(arg)) : "unknown"
    } else if (verb === "select") {
      root.lastCall = root.desk ? String(root.desk.select(arg)) : "unknown"
    } else if (verb === "width" || verb === "sidebar") {
      var next = ({})
      for (var k in root.entry) next[k] = root.entry[k]
      next[verb === "width" ? "deskWidth" : "deskSidebar"] = verb === "width" ? Number(arg) : arg
      root.setEntry(next)
    } else if (verb === "resize") {
      var wh = arg.split("x")
      win.width = Number(wh[0])
      win.height = Number(wh[1])
    } else if (verb === "key") {
      var mods = Qt.NoModifier
      var name = arg
      if (name.indexOf("Alt+") === 0) {
        mods = Qt.AltModifier
        name = name.slice(4)
      }
      driver.keyClick(root.keys[name], mods)
    } else if (verb === "keyDown") {
      if (root.keys[arg] !== undefined) driver.keyPress(root.keys[arg])
      else driver.keyPress(arg)
    } else if (verb === "keyUp") {
      if (root.keys[arg] !== undefined) driver.keyRelease(root.keys[arg])
      else driver.keyRelease(arg)
    } else if (verb === "keyRepeat") {
      root.keyRepeat(arg)
    } else if (verb === "focusName") {
      var focused = root.findName(arg)
      if (focused) focused.forceActiveFocus()
      else console.log("HARNESS nothing to focus: " + arg)
    } else if (verb === "text") {
      driver.keyClick(arg)
    } else if (verb === "type") {
      for (var c = 0; c < arg.length; c++) driver.keyClick(arg[c])
    } else if (verb === "click") {
      var target = root.findText(arg)
      if (target) driver.mouseClick(target)
      else console.log("HARNESS nothing to click: " + arg)
    } else if (verb === "clickName") {
      var named = root.findName(arg)
      if (named) driver.mouseClick(named)
      else console.log("HARNESS nothing to click: " + arg)
    } else if (verb === "trigger") {
      // trigger:<objectName>:<action id> — the item's actionTriggered(id), as
      // a stray trigger would (no button involved): a guard behind the bar
      // must hold on its own (WP-102b round 2)
      var at = arg.indexOf(":")
      var target = root.findName(arg.slice(0, at))
      if (target && typeof target.actionTriggered === "function") target.actionTriggered(arg.slice(at + 1))
      else console.log("HARNESS nothing to click: " + arg)
    } else if (verb === "clickAt") {
      var xy = arg.split(",")
      driver.mouseClick(win.contentItem, Number(xy[0]), Number(xy[1]))
    } else if (verb === "drag") {
      var parts = arg.split(":")
      var item = root.findName(parts[0])
      if (!item) {
        console.log("HARNESS nothing to drag: " + parts[0])
        return
      }
      var f = parts[1].split(",").map(Number)
      root.dragItem = item
      root.dragY = item.height / 2
      driver.mousePress(item, f[0] * item.width, root.dragY)
      for (var s = 1; s <= 4; s++) {
        root.dragX = (f[0] + (f[1] - f[0]) * s / 4) * item.width
        driver.mouseMove(item, root.dragX, root.dragY)
      }
    } else if (verb === "wheel") {
      var wp = arg.split(":")
      var wi = root.findName(wp[0])
      if (wi) driver.mouseWheel(wi, wi.width / 2, wi.height / 2, 0, Number(wp[1]))
      else console.log("HARNESS nothing to wheel: " + wp[0])
    } else if (verb === "release") {
      if (root.dragItem) driver.mouseRelease(root.dragItem, root.dragX, root.dragY)
      root.dragItem = null
    } else if (verb === "hover") {
      var over = root.findText(arg)
      if (over) driver.mouseMove(over, over.width / 2, over.height / 2)
      else console.log("HARNESS nothing to hover: " + arg)
    } else if (verb === "shot") {
      var dir = Quickshell.env("HARNESS_SHOTS") || ""
      if (dir !== "") win.contentItem.grabToImage(function(r) { r.saveToFile(dir + "/" + arg + ".png") })
    }
  }

  // The pill's bar, as Bar.qml binds a third-party widget.
  PluginBarApi {
    id: api
    pluginId: "jax.seldon"
    moduleName: "jax.seldon"
    shell: fakeShell
    foreground: Color.bar.text
    barForeground: Color.bar.text
    background: Color.bar.background
    urgent: Color.bar.active
    fontFamily: Style.font.family
    barSize: Style.bar.sizeHorizontal
    foregroundAnimationEnabled: false
    _moduleWidgets: function(id) { return id === "jax.seldon" && root.widget ? [root.widget] : [] }
  }

  Window {
    id: win
    visible: true
    width: Number(Quickshell.env("HARNESS_W") || 1920)
    height: Number(Quickshell.env("HARNESS_H") || 1080)

    // What lies under the desk, for `shot:` (grabToImage leaves out the
    // window colour).
    Rectangle {
      anchors.fill: parent
      color: Color.background
    }

    // The pill, in a strip off the desk's way (bottom left).
    Item {
      id: strip
      x: 0
      y: win.height - height
      width: root.widget ? root.widget.implicitWidth : 0
      height: Style.bar.sizeHorizontal
      z: 10
    }
  }

  // The first frames after a `fresh` open: what the Prime Radiant has
  // aggregated and painted by then (-1: no such chart yet).
  Connections {
    target: win
    function onFrameSwapped() {
      if (!root.awaitFrame || !root.desk) return
      var paints = root.slotIds.map(function(id) {
        var c = root.chartFor(id)
        return c ? c.paints : -1
      })
      var r = root.radiant()
      var frame = root.firstFrame
      if (frame === null) {
        frame = {
          serviceBefore: root.beforeFresh.service,
          service: root.service.aggregationCount(),
          section: r ? r.aggregationCount() : -1,
          opened: root.desk.opened,
          createMs: root.beforeFresh.createMs,
          bare: root.bare,
          paints: paints,
          frames: 0,
          paintedBy: 0
        }
      }
      frame.frames++
      if (paints.every(function(n) { return n >= 1 })) frame.paintedBy = frame.frames
      root.awaitFrame = frame.paintedBy === 0 && frame.frames < 10
      root.firstFrame = frame
    }
  }

  TestCase {
    id: driver
    name: "desk"
    when: false
    running: false
  }

  Component.onCompleted: {
    root.service = root.load("Service.qml", null, {})
    var manifestJson = Quickshell.env("HARNESS_MANIFEST") || ""
    if (manifestJson !== "") root.service.manifest = JSON.parse(manifestJson)
    var settingsJson = Quickshell.env("HARNESS_SETTINGS") || ""
    root.entry = settingsJson !== "" ? JSON.parse(settingsJson) : ({})
    if (root.noPill) return
    root.widget = root.load("BarWidget.qml", strip, { bar: api, moduleName: "jax.seldon", settings: root.entry })
    if (root.widget) root.widget.anchors.fill = strip
  }

  readonly property double startMs: Date.now()

  function engineIdle() {
    var s = root.service
    return !s || (!s.busy && s.queue.length === 0 && !s.probing)
  }

  // Let the service read the index and probe the engine (up to 15 s on a
  // loaded machine), then run the steps.
  Timer {
    id: stepper
    interval: 100
    running: true
    onTriggered: {
      var s = root.service
      var waited = Date.now() - root.startMs
      if (root.step === 0 && waited < 15000 && (waited < 1000 || !s || !s.ready || s.probing || (!root.noPill && (!root.widget || !root.widget.service)))) {
        stepper.restart()
        return
      }
      if (root.waitFor !== "") {
        var timedOut = Date.now() - root.waitSince > 15000
        var done = false
        if (root.waitFor === "settle") done = root.engineIdle()
        else if (root.waitFor.indexOf("pause:") === 0) done = Date.now() - root.waitSince >= Number(root.waitFor.slice(6))
        else {
          var eq = root.waitFor.indexOf("=")
          var prefix = eq > 0 && root.waitFor[eq - 1] === "^"
          var got = String(root.pathValue(root.viewObject(), root.waitFor.slice(0, prefix ? eq - 1 : eq)))
          var want = root.waitFor.slice(eq + 1)
          done = prefix ? got.indexOf(want) === 0 : got === want
        }
        if (!done && !timedOut) {
          stepper.interval = 100
          stepper.restart()
          return
        }
        if (!done) console.log("HARNESS wait timed out: " + root.waitFor)
        root.waitFor = ""
        root.step++
        Qt.callLater(function() { root.report(root.steps[root.step - 1]) })
        stepper.interval = 150
        stepper.restart()
        return
      }
      if (root.step >= root.steps.length) {
        root.report("final")
        Qt.quit()
        return
      }
      var spec = root.steps[root.step]
      if (spec === "settle" || spec.indexOf("wait:") === 0 || spec.indexOf("pause:") === 0) {
        root.waitFor = spec === "settle" || spec.indexOf("pause:") === 0 ? spec : spec.slice(5)
        root.waitSince = Date.now()
        stepper.interval = 100
        stepper.restart()
        return
      }
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
