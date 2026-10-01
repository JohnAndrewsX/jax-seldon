import QtQuick
import QtQuick.Window
import QtTest
import Quickshell

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
//                       key:<Tab|Backtab|Up|Down|Left|Right|Return|Space|Escape>
//                       text:<character>   a typed character (f, F, c, 1, …)
//                       tab:<id>           Panel.selectTabById(id)
//                       filter:<source>    Panel.setFilter(source)
//                       view               no action, just report
ShellRoot {
  id: root

  property var service: null
  property var panel: null
  readonly property string pluginDir: Quickshell.env("HARNESS_PLUGIN_DIR") || ""
  readonly property var steps: (Quickshell.env("HARNESS_STEPS") || "view").split(";").filter(function(s) { return s !== "" })
  property int step: 0

  readonly property var keys: ({
    Tab: Qt.Key_Tab, Backtab: Qt.Key_Backtab, Up: Qt.Key_Up, Down: Qt.Key_Down,
    Left: Qt.Key_Left, Right: Qt.Key_Right, Return: Qt.Key_Return, Space: Qt.Key_Space, Escape: Qt.Key_Escape
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

  function report(tag) {
    var view = root.panel ? root.panel.view() : null
    console.log("HARNESS step " + tag + " " + JSON.stringify({ view: view, texts: texts(win.contentItem, []) }))
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
    } else if (verb === "tab") {
      root.panel.selectTabById(arg)
    } else if (verb === "filter") {
      root.panel.setFilter(arg)
    }
  }

  Window {
    id: win
    visible: true
    width: 700
    height: 1000
  }

  TestCase {
    id: driver
    name: "panel"
    when: false
    running: false
  }

  Component.onCompleted: {
    root.service = root.load("Service.qml", null, {})
    root.panel = root.load("Panel.qml", win.contentItem, { service: root.service })
    if (root.panel) root.panel.open()
  }

  // Let the service read the index and probe the engine, then run the steps.
  Timer {
    id: stepper
    interval: 1500
    running: true
    onTriggered: {
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
