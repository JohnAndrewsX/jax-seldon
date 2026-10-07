import QtQuick
import Quickshell

// The graph's live measurement (WP-125, ADR-0034 §5; tests/plugin/graph-live.sh):
// the real desk in its real layer-shell window on a Hyprland session, read
// only (SELDON_INDEX, dev mode), opened at section 8. It reports
// Desk.view().graph (ticks, tickMs, tickMsMax, slowTicks, stepMsMax,
// drawMs, paintMs) after the layout settles, after a replay from day 0 to
// the last, after a cut to the middle day, and after a switch to another
// section — and beside it an event-loop probe: a 1 ms Timer on the same
// thread whose largest gap is how long the shell thread was busy at once
// (the tick, the frame's sync, everything else the instance did).
//
// Lines: "GRAPH-LIVE <phase> <json>". The instance quits after the last.
ShellRoot {
  id: root

  readonly property string pluginDir: Quickshell.env("HARNESS_PLUGIN_DIR") || ""
  property var service: null
  property var desk: null
  property string phase: "load"
  property double phaseSince: Date.now()

  // The probe: the largest gap between two 1 ms timeouts, and the gaps
  // over 8 ms, since the last reset.
  property double lastBeat: 0
  property double maxGap: 0
  property var gaps: []

  QtObject {
    id: fakeShell
    function serviceFor(id) { return id === "jax.seldon" ? root.service : null }
    function isPluginOpen(id) { return !!root.desk && root.desk.opened === true }
    function summon(id, payload) { return true }
    function hide(id) { return true }
    function toggle(id, payload) { return true }
    function updateEntryInline(id, settings) { return false }
  }

  function load(file) {
    var component = Qt.createComponent("file://" + root.pluginDir + "/" + file, Component.PreferSynchronous)
    if (component.status !== Component.Ready) {
      console.log("GRAPH-LIVE error " + component.errorString())
      Qt.quit()
      return null
    }
    return component.createObject(null, {})
  }

  function graph() {
    return root.desk ? root.desk.sectionItem("graph") : null
  }

  function resetProbe() {
    root.maxGap = 0
    root.gaps = []
    root.lastBeat = 0
  }

  function report(tag) {
    var v = root.desk ? JSON.parse(root.desk.view("")) : null
    var worst = root.gaps.slice().sort(function(a, b) { return b - a }).slice(0, 8)
    console.log("GRAPH-LIVE " + tag + " " + JSON.stringify({
      status: v ? v.status : "",
      section: v ? v.section : "",
      screen: v ? v.screen : "",
      desk: v ? v.desk : null,
      graph: v ? v.graph : null,
      loop: { maxGap: root.maxGap, over8: root.gaps.length, worst: worst }
    }))
  }

  function next(phase) {
    root.phase = phase
    root.phaseSince = Date.now()
  }

  Timer {
    interval: 1
    repeat: true
    running: root.phase !== "load"
    onTriggered: {
      var now = Date.now()
      if (root.lastBeat > 0) {
        var gap = now - root.lastBeat
        if (gap > root.maxGap) root.maxGap = gap
        if (gap > 8) root.gaps.push(gap)
      }
      root.lastBeat = now
    }
  }

  // The script, one phase at a time, polled every 100 ms.
  Timer {
    interval: 100
    repeat: true
    running: true
    onTriggered: {
      var waited = Date.now() - root.phaseSince
      var g = root.graph()
      var gv = g ? g.graphView() : null
      if (root.phase === "load") {
        if (!root.service || !root.service.ready) return
        root.desk = root.load("Desk.qml")
        if (!root.desk) return
        root.desk.shell = fakeShell
        root.desk.manifest = { id: "jax.seldon" }
        root.desk.service = root.service
        root.desk.open(JSON.stringify({ section: "graph" }))
        root.resetProbe()
        root.next("settle")
      } else if (root.phase === "settle") {
        if (!(gv && gv.sleeping && gv.ticks > 0) && waited < 30000) return
        root.report("settled")
        root.resetProbe()
        g.play()
        root.next("replay")
      } else if (root.phase === "replay") {
        if (!(gv && !gv.playing && gv.sleeping) && waited < 40000) return
        root.report("replayed")
        root.resetProbe()
        g.setCut(Math.round(gv.span / 2))
        root.next("cut")
      } else if (root.phase === "cut") {
        if (!(gv && gv.sleeping) && waited < 30000) return
        root.report("cut")
        root.desk.section("today")
        g.setCut(gv.span)
        root.resetProbe()
        root.next("hidden")
      } else if (root.phase === "hidden") {
        if (waited < 2000) return
        root.report("hidden")
        root.desk.close()
        root.next("done")
        Qt.quit()
      }
    }
  }

  Component.onCompleted: root.service = root.load("Service.qml")
}
