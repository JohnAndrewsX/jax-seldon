import QtQuick
import QtQuick.Window
import Quickshell
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
// and the digits' centre as the widget computes them (logical px), and the
// ink colour. bar-view.sh measures the same centres in the PNG.
//
//   HARNESS_PLUGIN_DIR  absolute path of the plugin folder (required)
//   HARNESS_SHOT        PNG path to save the window to (optional)
ShellRoot {
  id: root

  property var service: null
  property var widget: null
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
  }

  Component.onCompleted: {
    root.service = root.load("Service.qml", null, {})
    root.widget = root.load("BarWidget.qml", slot, { bar: api, moduleName: "jax.seldon" })
    if (root.widget) root.widget.anchors.fill = slot
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
