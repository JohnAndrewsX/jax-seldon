import QtQuick
import Quickshell

// Headless harness for plugin/Service.qml (docs/TESTING.md, "Plugin").
//
// Loads the service exactly as omarchy-shell loads a third-party service (no
// parent, injected properties left null) in a private Quickshell instance,
// prints a snapshot line on every status change and a final one, then quits.
// It never talks to the running omarchy-shell. Driven by
// tests/plugin/service-states.sh.
//
// Timing is event-driven where it can be, so a loaded machine (parallel
// cargo builds) only makes a run slower, not wrong: fixes run once the
// service is ready, and the final snapshot waits until the service has
// settled (ready, no probe or engine call in flight, fixes and recheck done,
// HARNESS_UNTIL met), at the earliest after HARNESS_MS and at the latest
// 15 s after that.
//
//   HARNESS_PLUGIN_DIR  absolute path of the plugin folder (required)
//   HARNESS_MS          earliest final snapshot, ms after start (default 2500)
//   HARNESS_RECHECK_MS  if set, run the "recheck" fix after this many milliseconds
//   HARNESS_FIX         comma-separated fix action ids, run once the service is
//                       ready; an id may name its banner: "snapper:copy"
//   HARNESS_UNTIL       "field=value": also wait until the snapshot's field
//                       has that value (e.g. "status=ok")
ShellRoot {
  id: root

  property var service: null
  readonly property string pluginDir: Quickshell.env("HARNESS_PLUGIN_DIR") || ""
  readonly property int runMs: Number(Quickshell.env("HARNESS_MS") || 2500)
  readonly property int recheckMs: Number(Quickshell.env("HARNESS_RECHECK_MS") || 0)
  readonly property string fixes: Quickshell.env("HARNESS_FIX") || ""
  readonly property string until: Quickshell.env("HARNESS_UNTIL") || ""
  readonly property int graceMs: 15000

  readonly property double startMs: Date.now()
  readonly property bool serviceReady: !!root.service && root.service.ready
  property bool fixesDone: fixes === ""
  property bool recheckDone: recheckMs === 0

  function emit(tag) {
    if (!root.service) return
    console.log("HARNESS " + tag + " " + JSON.stringify(root.service.snapshot()))
  }

  function settled() {
    var s = root.service
    if (!s) return true
    if (!s.ready || s.busy || s.probing || s.queue.length > 0) return false
    if (!root.fixesDone || !root.recheckDone) return false
    if (root.until !== "") {
      var eq = root.until.indexOf("=")
      if (String(s.snapshot()[root.until.slice(0, eq)]) !== root.until.slice(eq + 1)) return false
    }
    return true
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
    onTriggered: {
      if (root.service) root.service.fix("recheck")
      root.recheckDone = true
    }
  }

  // Fixes need the banner, and the banner needs a settled status.
  Timer {
    interval: 300
    running: root.fixes !== "" && root.serviceReady && !root.fixesDone
    onTriggered: {
      var ids = root.fixes.split(",")
      for (var i = 0; i < ids.length; i++) {
        var parts = ids[i].split(":")
        var banner = parts.length > 1 ? parts[0] : "status"
        var action = parts[parts.length - 1]
        console.log("HARNESS fix " + ids[i] + " " + (root.service ? root.service.fix(action, banner) : false))
      }
      root.fixesDone = true
    }
  }

  Timer {
    id: finalTimer
    interval: root.runMs
    running: true
    onTriggered: {
      if (root.settled() || Date.now() - root.startMs > root.runMs + root.graceMs) {
        if (!root.settled()) console.log("HARNESS unsettled after grace period")
        root.emit("final")
        Qt.quit()
        return
      }
      finalTimer.interval = 200
      finalTimer.restart()
    }
  }
}
