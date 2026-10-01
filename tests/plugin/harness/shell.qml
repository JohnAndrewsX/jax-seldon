import QtQuick
import Quickshell

// Headless harness for plugin/Service.qml (docs/TESTING.md, "Plugin").
//
// Loads the service exactly as omarchy-shell loads a third-party service (no
// parent, injected properties left null) in a private Quickshell instance,
// prints a snapshot line on every status change and once more after
// HARNESS_MS milliseconds, then quits. It never talks to the running
// omarchy-shell. Driven by tests/plugin/service-states.sh.
//
//   HARNESS_PLUGIN_DIR  absolute path of the plugin folder (required)
//   HARNESS_MS          run time in milliseconds (default 2500)
//   HARNESS_RECHECK_MS  if set, run the "recheck" fix after this many milliseconds
//   HARNESS_FIX         comma-separated fix action ids to run after 1 s
ShellRoot {
  id: root

  property var service: null
  readonly property string pluginDir: Quickshell.env("HARNESS_PLUGIN_DIR") || ""
  readonly property int runMs: Number(Quickshell.env("HARNESS_MS") || 2500)
  readonly property int recheckMs: Number(Quickshell.env("HARNESS_RECHECK_MS") || 0)
  readonly property string fixes: Quickshell.env("HARNESS_FIX") || ""

  function emit(tag) {
    if (!root.service) return
    console.log("HARNESS " + tag + " " + JSON.stringify(root.service.snapshot()))
  }

  Component.onCompleted: {
    var component = Qt.createComponent("file://" + root.pluginDir + "/Service.qml", Component.PreferSynchronous)
    if (component.status !== Component.Ready) {
      console.log("HARNESS error " + component.errorString())
      Qt.quit()
      return
    }
    root.service = component.createObject(null)
    root.service.statusChanged.connect(function() { root.emit("change") })
    root.service.readyChanged.connect(function() { root.emit("ready") })
  }

  Timer {
    interval: root.recheckMs
    running: root.recheckMs > 0
    onTriggered: if (root.service) root.service.fix("recheck")
  }

  Timer {
    interval: 1000
    running: root.fixes !== ""
    onTriggered: {
      var ids = root.fixes.split(",")
      for (var i = 0; i < ids.length; i++)
        console.log("HARNESS fix " + ids[i] + " " + (root.service ? root.service.fix(ids[i]) : false))
    }
  }

  Timer {
    interval: root.runMs
    running: true
    onTriggered: {
      root.emit("final")
      Qt.quit()
    }
  }
}
