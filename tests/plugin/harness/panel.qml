import QtQuick
import QtQuick.Window
import QtTest
import Quickshell
import qs.Commons

// Headless harness for plugin/Panel.qml (docs/TESTING.md, "Plugin").
//
// tests/plugin/panel-view.sh copies this file as shell.qml into a temp config
// root next to copies of the installed shell's Commons/ and Ui/ (Ui's
// KeyboardPanel replaced by a stand-in), so `import qs.*` resolves to the real
// shell code. It loads Service.qml as the shell does, puts Panel.qml in an
// offscreen window, opens it, then runs HARNESS_STEPS one by one and prints
// what the panel shows after each: Panel.view() plus every visible text.
//
//   HARNESS_PLUGIN_DIR  absolute path of the plugin folder (required)
//   HARNESS_STEPS       ";"-separated steps, each optionally "*N" repeated:
//                       key:<Tab|Backtab|Up|Down|Left|Right|Return|Space|Escape|Backspace>
//                       text:<character>   a typed character (f, F, c, 1, …)
//                       type:<text>        each character of text, typed
//                       tab:<id>           Panel.selectTabById(id)
//                       filter:<source>    Panel.setFilter(source)
//                       resolve:<target>   Panel.resolve(target): the drift
//                                          sheet for an event id, or "crisis"
//                                          (a click on the red strip)
//                       click:<text>       click the centre of the first
//                                          visible item whose text is <text>
//                                          (a Button, or a label over a
//                                          MouseArea such as the red strip)
//                       hover:<text>       move the pointer to the centre of
//                                          the first visible item whose text
//                                          is <text> (a banner's tooltip)
//                       shot:<name>        save the window as
//                                          $HARNESS_SHOTS/<name>.png
//                       close / open       close or open the panel again
//                       view               no action, just report
//                       settle             wait (up to 15 s) until no engine
//                                          call is queued or running
//                       wait:<path>=<v>    wait (up to 15 s) until Panel.view()
//                                          at the dotted path is v, e.g.
//                                          wait:changelog.rows=59
//   SELDON_INDEX        empty for a live run: the service then reads the
//                       state index the fake engine writes and runs actions
//   HARNESS_MANIFEST    a manifest as JSON text, assigned to the service's
//                       `manifest` after creation, as the shell injects it
//   HARNESS_SETTINGS    the bar widget's settings (shell.json's entry) as
//                       JSON text, handed to the panel as the widget does,
//                       e.g. {"driftInBar":"all"} (ADR-0028 §4a)
//   HARNESS_BAR         if set, give the panel a stand-in bar whose
//                       switchPanelFrom() records its direction; each report
//                       then carries `switches` (Tab hands over to the bar)
ShellRoot {
  id: root

  property var service: null
  property var panel: null
  readonly property string pluginDir: Quickshell.env("HARNESS_PLUGIN_DIR") || ""
  readonly property var steps: (Quickshell.env("HARNESS_STEPS") || "view").split(";").filter(function(s) { return s !== "" })
  property int step: 0

  readonly property var keys: ({
    Tab: Qt.Key_Tab, Backtab: Qt.Key_Backtab, Up: Qt.Key_Up, Down: Qt.Key_Down,
    Left: Qt.Key_Left, Right: Qt.Key_Right, Return: Qt.Key_Return, Space: Qt.Key_Space, Escape: Qt.Key_Escape,
    Backspace: Qt.Key_Backspace
  })

  function load(file, parent, props) {
    var component = Qt.createComponent("file://" + root.pluginDir + "/" + file, Component.PreferSynchronous)
    if (component.status !== Component.Ready) {
      console.log("HARNESS error " + component.errorString())
      return null
    }
    return component.createObject(parent, props)
  }

  // Every visible, non-empty Text under `item`, in tree order.
  function texts(item, out) {
    if (!item || item.visible === false) return out
    if (item.text !== undefined && item.font !== undefined && typeof item.text === "string" && item.text !== "")
      out.push(item.text)
    var kids = item.children
    for (var i = 0; kids && i < kids.length; i++) texts(kids[i], out)
    return out
  }

  // Every visible text that does not fit (WP-039), as "<kind>:<text>":
  //   elided   a Text elided or cut at its line limit (`truncated`)
  //   wide     a Text whose content is wider than its explicit width
  //   button   a qs.Ui Button narrower than its label and padding (the
  //            Button's own label is never elided; it spills over)
  //   outside  text painted past the panel's right edge (a Row that grew
  //            wider than the panel)
  // `frame` is the panel's content item (the key catcher).
  function overflow(item, frame, out) {
    if (!item || item.visible === false) return out
    var text = typeof item.text === "string" ? item.text : ""
    var isButton = item.bordered !== undefined && item.horizontalPadding !== undefined
    var isText = !isButton && item.truncated !== undefined && item.font !== undefined
    if (text !== "" && (isButton || isText)) {
      var painted = isText && item.horizontalAlignment !== Text.AlignRight
        ? Math.min(item.width, item.contentWidth) : item.width
      var left = item.mapToItem(frame, 0, 0).x
      if (item.horizontalAlignment === Text.AlignRight && isText) left += item.width - painted
      if (isButton && item.implicitWidth > item.width + 0.5) out.push("button:" + text)
      else if (isText && item.truncated) out.push("elided:" + text)
      else if (isText && item.width > 0 && item.contentWidth > item.width + 0.5) out.push("wide:" + text)
      else if (left + painted > frame.width + 0.5) out.push("outside:" + text)
    }
    var kids = item.children
    for (var i = 0; kids && i < kids.length; i++) overflow(kids[i], frame, out)
    return out
  }

  function report(tag) {
    var view = root.panel ? root.panel.view() : null
    var frame = root.byName(win.contentItem, "seldonKeys")
    // The tag is one word: panel-view.sh cuts the line at the first space after it.
    console.log("HARNESS step " + String(tag).replace(/\s/g, "_") + " " + JSON.stringify({
      view: view, switches: fakeBar.switches, texts: texts(win.contentItem, []),
      overflow: frame ? overflow(frame, frame, []) : [], contentWidth: frame ? frame.width : 0
    }))
  }

  // The part of the bar facade Panel.qml uses; colours are test values.
  QtObject {
    id: fakeBar
    property color foreground: "#c0c0c0"
    property color barForeground: "#c0c0c0"
    property color urgent: "#c04040"
    property string fontFamily: "monospace"
    property string position: "top"
    property var switches: []
    function switchPanelFrom(owner, direction) {
      fakeBar.switches = fakeBar.switches.concat([direction])
      return true
    }
  }

  function act(spec) {
    var colon = spec.indexOf(":")
    var verb = colon === -1 ? spec : spec.slice(0, colon)
    var arg = colon === -1 ? "" : spec.slice(colon + 1)
    if (verb === "key") {
      if (arg === "Backtab") driver.keyClick(Qt.Key_Tab, Qt.ShiftModifier)
      else driver.keyClick(root.keys[arg])
    } else if (verb === "text") {
      driver.keyClick(arg)
    } else if (verb === "type") {
      for (var i = 0; i < arg.length; i++) {
        if (arg[i] === " ") driver.keyClick(Qt.Key_Space)
        else driver.keyClick(arg[i])
      }
    } else if (verb === "tab") {
      root.panel.selectTabById(arg)
    } else if (verb === "filter") {
      root.panel.setFilter(arg)
    } else if (verb === "resolve") {
      root.panel.resolve(arg)
    } else if (verb === "click") {
      var target = root.findText(win.contentItem, arg)
      if (target) driver.mouseClick(target)
      else console.log("HARNESS nothing to click: " + arg)
    } else if (verb === "hover") {
      var over = root.findText(win.contentItem, arg)
      if (over) driver.mouseMove(over)
      else console.log("HARNESS nothing to hover: " + arg)
    } else if (verb === "close") {
      root.panel.close()
    } else if (verb === "open") {
      root.panel.open()
    } else if (verb === "shot") {
      var dir = Quickshell.env("HARNESS_SHOTS") || ""
      if (dir !== "") win.contentItem.grabToImage(function(result) { result.saveToFile(dir + "/" + arg + ".png") })
    }
  }

  // The first item, in tree order, with this objectName.
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

  Window {
    id: win
    visible: true
    width: 700
    height: 1000

    // The theme's background under the panel, for `shot:` (grabToImage
    // leaves out the window colour).
    Rectangle {
      anchors.fill: parent
      color: Color.background
    }
  }

  TestCase {
    id: driver
    name: "panel"
    when: false
    running: false
  }

  function idle() {
    var s = root.service
    return !s || (s.ready && !s.probing && !s.busy && s.queue.length === 0)
  }

  property double settleStartMs: 0

  // "a.b=v": Panel.view().a.b, as a string, is v.
  function viewHas(cond) {
    var eq = cond.indexOf("=")
    var value = root.panel ? root.panel.view() : null
    var path = cond.slice(0, eq).split(".")
    for (var i = 0; value !== null && value !== undefined && i < path.length; i++) value = value[path[i]]
    return String(value) === cond.slice(eq + 1)
  }

  Component.onCompleted: {
    root.service = root.load("Service.qml", null, {})
    var manifestJson = Quickshell.env("HARNESS_MANIFEST") || ""
    if (root.service && manifestJson !== "") root.service.manifest = JSON.parse(manifestJson)
    var props = { service: root.service }
    var settingsJson = Quickshell.env("HARNESS_SETTINGS") || ""
    if (settingsJson !== "") props.settings = JSON.parse(settingsJson)
    if (Quickshell.env("HARNESS_BAR")) props.bar = fakeBar
    root.panel = root.load("Panel.qml", win.contentItem, props)
    if (root.panel) root.panel.open()
  }

  readonly property double startMs: Date.now()

  // Let the service read the index and probe the engine (event-driven, up to
  // 15 s on a loaded machine), then run the steps.
  Timer {
    id: stepper
    interval: 100
    running: true
    onTriggered: {
      // At least 1 s, so the window has laid out and drawn its first frames.
      // A live run also waits for the start-up capture.
      var waited = Date.now() - root.startMs
      if (root.step === 0 && waited < 15000 && (waited < 1000 || !root.idle())) {
        stepper.restart()
        return
      }
      if (root.step >= root.steps.length) {
        root.report("final")
        Qt.quit()
        return
      }
      var spec = root.steps[root.step]
      if (spec === "settle" || spec.indexOf("wait:") === 0) {
        if (root.settleStartMs === 0) root.settleStartMs = Date.now()
        var met = spec === "settle" ? root.idle() : root.viewHas(spec.slice(5))
        if (!met && Date.now() - root.settleStartMs < 15000) {
          stepper.restart()
          return
        }
        root.settleStartMs = 0
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
