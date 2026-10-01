import QtQuick
import Quickshell
import Quickshell.Io
import "Model.js" as Model

// Headless data service for jax.seldon (SPEC-PLUGIN §3, docs/CONTRACT.md).
//
// Watches index.json, publishes `index` and `status`, detects the engine,
// and runs it on a timer. Everything is asynchronous: FileView reads and
// Process calls never block the shell thread. The index is parsed here on the
// shell thread; the contract keeps it under 1 MB.
//
// The engine is only ever started with an argument list from CONTRACT.md,
// checked by Model.validateArgs; fix commands are constants from Model.js.
//
// The index lives at ${XDG_STATE_HOME:-$HOME/.local/state}/seldon/index.json
// (CONTRACT.md rule 1). The engine is probed once at start and again only on
// "Check again"; the capture timer never probes.
//
// Development overrides (never set them in a real session):
//   SELDON_INDEX  read this file instead of the state index.
//                 Dev mode is read-only: the engine is probed, never run.
//   SELDON_NOW    with SELDON_INDEX, the clock used for staleness (RFC 3339).
//                 Without it the clock is pinned to the index's generatedAt,
//                 so a fixture never turns stale on its own.
Item {
  id: root

  // Injected by omarchy-shell (a capability-scoped facade for third parties).
  property var shell: null
  property var manifest: null

  readonly property int contractVersion: Model.CONTRACT_VERSION

  // ---- Index location.
  readonly property string home: Quickshell.env("HOME") || ""
  readonly property string devIndex: String(Quickshell.env("SELDON_INDEX") || "")
  readonly property bool devMode: devIndex !== ""
  readonly property string devNow: devMode ? String(Quickshell.env("SELDON_NOW") || "") : ""
  readonly property string indexPath: devMode
    ? Model.resolvePath(devIndex, Quickshell.workingDirectory)
    : Model.stateIndexPath(Quickshell.env("XDG_STATE_HOME"), home)

  // ---- Engine.
  property string engineState: "unknown"   // unknown | present | missing
  property string engineVersion: ""
  property string engineDetail: ""
  readonly property bool probing: probe.running
  // Set when the engine exits 3 (logbook not initialised); cleared by the
  // next successful call or by an index written after it was set.
  property bool engineNotInitialised: false
  property double notInitialisedAtMs: 0

  // ---- Index file.
  property string fileState: "loading"     // loading | loaded | missing | invalid
  property var parsed: null
  readonly property var index: parsed && parsed.ok ? parsed.index : null
  readonly property int indexContractVersion: parsed ? parsed.contractVersion : 0
  readonly property bool ready: fileState !== "loading" && engineState !== "unknown"

  // ---- Derived state.
  property double liveNowMs: Date.now()
  readonly property double nowMs: Model.effectiveNowMs(liveNowMs, devMode, devNow, index ? index.generatedAt : "")
  readonly property string status: Model.deriveStatus({
    engine: engineState,
    file: fileState,
    parse: parsed,
    engineNotInitialised: engineNotInitialised,
    nowMs: nowMs
  })
  readonly property var counts: Model.counts(index)
  readonly property string lastCapture: Model.lastCapture(index)
  readonly property var banner: Model.bannerFor(status, {
    indexContractVersion: indexContractVersion,
    parseError: parsed ? parsed.error : "",
    generatedAt: index ? index.generatedAt : "",
    nowMs: nowMs
  })
  // What the index itself reports, shown under the status banner on every tab
  // (SPEC-PLUGIN §5), only while its counts mean something.
  readonly property bool indexShown: index !== null && Model.showsCounts(status)
  readonly property string crisisText: indexShown ? Model.crisisText(index) : ""
  readonly property var snapperBanner: indexShown ? Model.snapperBanner(index) : null

  // ---- Engine calls: one at a time, in order.
  property bool busy: false
  property var queue: []
  property var currentArgs: []
  property string lastError: ""
  property int captureIntervalMin: Model.CAPTURE_INTERVAL_MIN_DEFAULT

  // Emitted after every engine call, for panels that wait on a result.
  signal finished(var args, int exitCode, string output)

  function setCaptureInterval(minutes) {
    root.captureIntervalMin = Model.clampInterval(minutes)
  }

  // ---- Index.

  function ingest(text) {
    var result = Model.parseIndex(text)
    if (root.engineNotInitialised && result.ok
        && Model.timeMs(result.index.generatedAt) > root.notInitialisedAtMs)
      root.engineNotInitialised = false
    root.parsed = result
    root.fileState = result.ok || result.error === "contract" ? "loaded" : "invalid"
    if (result.error === "parse" || result.error === "shape")
      console.warn("jax.seldon: index.json unreadable: " + result.detail)
  }

  function ingestFailure(error) {
    root.parsed = null
    root.fileState = error === FileViewError.FileNotFound ? "missing" : "invalid"
  }

  function reloadIndex() {
    indexFile.reload()
  }

  // ---- Engine.

  function probeEngine() {
    if (probe.running) return
    probe.launch(["seldon", "--version", "--json"])
  }

  function probeDone(exitCode, out, err) {
    if (exitCode === 0) {
      root.engineVersion = Model.engineVersion(out)
      root.engineDetail = ""
      root.engineState = "present"
      if (!root.devMode) root.captureCycle()
    } else {
      // A seldon that cannot even report its version is no usable engine.
      root.engineVersion = ""
      root.engineDetail = Model.engineError(out, err, exitCode)
      root.lastError = ""
      root.engineState = "missing"
    }
  }

  function probeFailedToStart() {
    root.lastError = ""
    root.engineVersion = ""
    root.engineDetail = "seldon not found on PATH"
    root.engineState = "missing"
  }

  // Queue one engine call. `args` excludes the program name and must be one
  // of the CONTRACT.md command forms. Returns false when refused.
  function run(args) {
    var reason = Model.validateArgs(args)
    if (reason !== "") {
      console.warn("jax.seldon: refused engine call: " + reason)
      return false
    }
    if (root.devMode) {
      root.lastError = "dev mode (SELDON_INDEX): engine calls are disabled"
      return false
    }
    if (root.engineState !== "present") {
      root.lastError = "engine not available"
      return false
    }
    root.queue = root.queue.concat([args.slice()])
    root.pump()
    return true
  }

  function queued(head) {
    if (root.busy && root.currentArgs[0] === head) return true
    for (var i = 0; i < root.queue.length; i++)
      if (root.queue[i][0] === head) return true
    return false
  }

  function pump() {
    if (root.busy || root.queue.length === 0) return
    var next = root.queue[0]
    root.queue = root.queue.slice(1)
    root.currentArgs = next
    root.busy = true
    watchdog.restart()
    runner.launch(["seldon"].concat(next))
  }

  function runnerDone(exitCode, out, err) {
    var args = root.currentArgs
    watchdog.stop()
    root.busy = false
    root.currentArgs = []
    if (exitCode === 0) {
      root.lastError = ""
      root.engineNotInitialised = false
    } else if (exitCode === 3) {
      // Nothing else can succeed until `seldon init` has run.
      root.notInitialisedAtMs = Date.now()
      root.engineNotInitialised = true
      root.queue = []
      root.lastError = ""
    } else {
      root.lastError = "seldon " + args[0] + ": " + Model.engineError(out, err, exitCode)
    }
    // The engine rewrites index.json atomically; reload in case the watch
    // missed the rename.
    indexFile.reload()
    root.finished(args, exitCode, out)
    root.pump()
  }

  function runnerFailedToStart() {
    watchdog.stop()
    root.busy = false
    root.currentArgs = []
    root.queue = []
    root.lastError = ""
    root.engineVersion = ""
    root.engineDetail = "seldon not found on PATH"
    root.engineState = "missing"
  }

  // Capture, then status (which rewrites the index). Never queued twice.
  function captureNow() {
    if (root.queued("capture")) return false
    if (!root.run(["capture", "--all", "--json", "--quiet"])) return false
    root.run(["status", "--json"])
    return true
  }

  function captureCycle() {
    if (root.devMode || root.engineState !== "present") return
    // Until `seldon init` has run, only ask whether it has.
    if (root.engineNotInitialised) {
      if (!root.queued("status")) root.run(["status", "--json"])
      return
    }
    root.captureNow()
  }

  // ---- Banner fixes (AGENTS.md §7: every non-ok state has a one-click fix).
  // bannerId picks the banner whose constant command copy/terminal use:
  // "status" (default) or "snapper" (ADR-0011).
  function fix(actionId, bannerId) {
    var source = bannerId === "snapper" ? root.snapperBanner : root.banner
    var command = source ? source.command : ""
    if (actionId === "copy" && command !== "") {
      Quickshell.execDetached(["wl-copy", "--", command])
    } else if (actionId === "terminal" && command !== "") {
      // Opens on the explicit click only (ADR-0004); `command` is a constant.
      Quickshell.execDetached(["omarchy-launch-floating-terminal-with-presentation", command])
    } else if (actionId === "recheck") {
      root.probeEngine()
      root.reloadIndex()
    } else if (actionId === "build") {
      root.run(["status", "--json"])
    } else if (actionId === "capture") {
      root.captureNow()
    } else {
      return false
    }
    return true
  }

  function snapshot() {
    return {
      status: root.status,
      ready: root.ready,
      devMode: root.devMode,
      indexPath: root.indexPath,
      fileState: root.fileState,
      indexContractVersion: root.indexContractVersion,
      engine: root.engineState,
      engineVersion: root.engineVersion,
      engineDetail: root.engineDetail,
      busy: root.busy,
      lastError: root.lastError,
      pill: Model.pillText(root.counts),
      tone: Model.pillTone(root.counts),
      tooltip: Model.tooltipText(root.status, root.counts, root.lastCapture, root.nowMs),
      banner: root.banner ? root.banner.title : "",
      crisis: root.crisisText,
      snapper: root.snapperBanner ? root.snapperBanner.title : ""
    }
  }

  // ---- Machinery.

  // One engine invocation. A missing binary never emits `exited`: Quickshell
  // logs "Process failed to start" and drops `running` without `started`.
  component EngineCall: Item {
    id: call

    readonly property bool running: proc.running
    property bool didStart: false
    property bool exitSeen: false
    property bool outDone: false
    property bool errDone: false
    property int code: -1

    signal done(int exitCode, string out, string err)
    signal failedToStart()

    function launch(argv) {
      call.didStart = false
      call.exitSeen = false
      call.outDone = false
      call.errDone = false
      call.code = -1
      proc.command = argv
      proc.running = true
    }

    function stop() {
      proc.running = false
    }

    function settle() {
      if (call.exitSeen && call.outDone && call.errDone)
        call.done(call.code, outText.text, errText.text)
    }

    Process {
      id: proc
      stdout: StdioCollector {
        id: outText
        onStreamFinished: { call.outDone = true; call.settle() }
      }
      stderr: StdioCollector {
        id: errText
        onStreamFinished: { call.errDone = true; call.settle() }
      }
      onStarted: call.didStart = true
      onRunningChanged: if (!proc.running && !call.didStart) call.failedToStart()
    }

    // A Connections handler, because qmllint cannot resolve the
    // QProcess::ExitStatus parameter of an inline onExited handler.
    Connections {
      target: proc
      function onExited(exitCode, exitStatus) {
        call.code = exitCode
        call.exitSeen = true
        call.settle()
      }
    }
  }

  EngineCall {
    id: probe
    onDone: function(exitCode, out, err) { root.probeDone(exitCode, out, err) }
    onFailedToStart: root.probeFailedToStart()
  }

  EngineCall {
    id: runner
    onDone: function(exitCode, out, err) { root.runnerDone(exitCode, out, err) }
    onFailedToStart: root.runnerFailedToStart()
  }

  FileView {
    id: indexFile
    path: root.indexPath
    watchChanges: true
    printErrors: false
    onLoaded: root.ingest(indexFile.text())
    onLoadFailed: function(error) { root.ingestFailure(error) }
    onFileChanged: indexFile.reload()
  }

  // A watch cannot sit on a file that does not exist yet; look again until
  // it does.
  Timer {
    interval: 5000
    repeat: true
    running: root.fileState === "missing" || root.fileState === "invalid"
    onTriggered: indexFile.reload()
  }

  // Capture cycle (ADR-0005). A missing engine is looked for again only on
  // "Check again", so a machine without it logs one probe per shell start.
  Timer {
    interval: root.captureIntervalMin * 60000
    repeat: true
    running: root.engineState === "present"
    onTriggered: root.captureCycle()
  }

  // Staleness and "N min ago" follow the clock.
  Timer {
    interval: 60000
    repeat: true
    running: true
    onTriggered: root.liveNowMs = Date.now()
  }

  // A hung engine must not stall the queue forever.
  Timer {
    id: watchdog
    interval: 300000
    onTriggered: {
      console.warn("jax.seldon: engine call timed out: " + root.currentArgs.join(" "))
      runner.stop()
    }
  }

  IpcHandler {
    target: "jax.seldon.service"

    function status(): string { return JSON.stringify(root.snapshot()) }
    function refresh(): string { root.probeEngine(); root.reloadIndex(); return "ok" }
    function capture(): string { return root.captureNow() ? "ok" : "refused" }
  }

  Component.onCompleted: root.probeEngine()
}
