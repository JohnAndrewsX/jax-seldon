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
// checked by Model.validateArgs; fix commands and the terminal scripts
// behind them are constants from Model.js.
//
// The index lives at ${XDG_STATE_HOME:-$HOME/.local/state}/seldon/index.json
// (CONTRACT.md rule 1). The engine is probed once at start, again on the
// status banner's "Check again", when the desk opens while it is missing,
// and every few seconds after the setup card opened its install terminal
// (WP-119); the capture timer never probes.
//
// Development overrides (never set them in a real session):
//   SELDON_INDEX  read this file instead of the state index.
//                 Dev mode is read-only: the engine is probed, never run.
//   SELDON_NOW    with SELDON_INDEX, the clock used for staleness (RFC 3339).
//                 Without it the clock is pinned to the index's generatedAt,
//                 so a fixture never turns stale on its own.
//   SELDON_LOCK_RETRY_MS  the wait before a capture or status that found the
//                 lock held runs again (default 30000; the harness's).
//   SELDON_SETUP_CAPTURE_MS  the wait before the setup card's own capture
//                 after the snapshot grant's terminal, and between two
//                 (default Model.SETUP_CAPTURE_MS; the harness's).
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
  // What the last exit 3 said about the logbook's folder (CONTRACT.md rule
  // 10, Model.notInitialisedInfo): { reason, path }; the setup card's step
  // 2 offers another folder when `reason` says this one cannot be used.
  property var notInitialisedInfo: ({ reason: "", path: "" })
  // The lowest engine this plugin works with (manifest `seldon.engineMin`,
  // docs/VERSIONING.md); "" until the shell has injected the manifest.
  readonly property string engineMin: Model.engineMinOf(root.manifest)

  // ---- Plugin code (WP-090): the version this code is, and the one the
  // manifest on disk names (the shell re-reads it on every rescan, "" until
  // injected). When they differ, the plugin was updated under a running
  // shell, which keeps the old code until it restarts.
  readonly property string pluginVersion: Model.PLUGIN_VERSION
  readonly property string manifestVersion: Model.pluginVersionOf(root.manifest)
  readonly property var restartNotice: Model.restartShellNotice(pluginVersion, manifestVersion)
  // Set by the first restart this service starts. A second one (a double
  // click) could kill the new shell while it starts, so the action is
  // one-shot; the next service instance (the new shell's) starts clear.
  property bool restartStarted: false

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
  // A newer contract this plugin still reads (ADR-0051): the quiet notice
  // that asks for a plugin update; null otherwise and without an engine.
  readonly property var contractNotice: Model.contractNewerNotice(parsed, status)
  readonly property var counts: Model.counts(index)
  readonly property string lastCapture: Model.lastCapture(index)
  readonly property var banner: Model.engineOutdatedBanner(status, engineVersion, engineMin)
    || Model.bannerFor(status, {
      indexContractVersion: indexContractVersion,
      parseError: parsed ? parsed.error : "",
      generatedAt: index ? index.generatedAt : "",
      nowMs: nowMs,
      indexExists: fileState === "loaded" || fileState === "invalid"
    })
  // What the index itself reports, shown under the status banner on every tab
  // (SPEC-PLUGIN §5), only while its counts mean something.
  readonly property bool indexShown: index !== null && Model.showsCounts(status)
  readonly property string crisisText: indexShown ? Model.crisisText(index) : ""
  readonly property var snapperBanner: indexShown ? Model.snapperBanner(index) : null
  // The setup card (WP-119): engine → logbook → snapshots, from the same
  // states as the banners (Model.setupCard), null when nothing is left.
  // *Not now* on its snapshot step is stored in the shell.json entry
  // (Model.SETUP_LATER_KEY, written by the desk); `setupLaterOverride`
  // holds the click (true) or Settings' *Offer again* (false) until the
  // entry the shell sends back says the same, and for this shell's life
  // when the write did not happen.
  property var setupLaterOverride: null
  readonly property bool setupLater: root.setupLaterOverride !== null ? root.setupLaterOverride === true
    : Model.setupLaterStored(root.entryKnown ? root.entrySettings : root.localEntry)
  readonly property var setup: Model.setupCard({
    status: root.status,
    indexExists: root.fileState === "loaded" || root.fileState === "invalid",
    index: root.indexShown ? root.index : null,
    later: root.setupLater,
    bannerStatus: root.banner ? root.banner.status : "",
    bannerTitle: root.banner ? root.banner.title : "",
    snapperReady: !!root.snapperBanner,
    // the folder `init --defaults` creates: what exit 3 named, else the
    // notInitialised index's own logbook path
    logbookPath: Model.displayPath(root.notInitialisedInfo.path !== "" ? root.notInitialisedInfo.path
      : root.status === "notInitialised" && root.index && root.index.logbook && typeof root.index.logbook.path === "string"
        ? root.index.logbook.path : "", root.home),
    logbookBlocked: root.status === "notInitialised" ? root.notInitialisedInfo.reason : ""
  })
  // After a step's terminal opened: { step, untilMs, captureAtMs }, while
  // the service looks again by itself (setupTick); null otherwise.
  property var setupWatch: null
  // Why Not now or Offer again holds only until the shell restarts (no
  // bar entry, or the shell refused the write); "" when it was stored.
  // Settings › Capture shows it (the card is gone after Not now).
  property string setupResult: ""
  // The Prime Radiant's windows, series rows, slot counts and chart data for
  // every period (Model.periodTable: one pass per series, then the charts),
  // computed when the index changes: the overlay is created anew on each
  // open and then only looks them up (WP-030, WP-031).
  readonly property var periods: Model.periodTable(index)
  // The desk's sections Today, Changelog and Work (ADR-0034 §3, WP-122):
  // their rows, built once when the index changes; the sections filter
  // them by chip and search and look details up by id. Only while the
  // index's contents mean something (indexShown), as the desk shows it.
  readonly property var deskChangelog: Model.deskChangelog(root.indexShown ? root.index : null)
  readonly property var deskToday: Model.deskToday(root.indexShown ? root.index : null, root.deskChangelog)
  readonly property var deskWork: Model.deskWork(root.indexShown ? root.index : null)
  // The graph (section 8, ADR-0034 §5, WP-125): nodes, edges, day index and
  // folding (Model.graphBuild); the section only lays it out. Built when
  // section 8 is shown and the index changed since the last build
  // (`graphDirty`; graphRefresh, called by the section): the build takes
  // about 5 ms of the shell thread on 500 events, which no capture should
  // pay while the graph is not on screen. `graphBuilds` counts them.
  property var graph: Model.graphEmpty()
  property bool graphDirty: true
  property int graphBuilds: 0
  // The graph's layout (Model.graphState), kept here so a reopened desk
  // shows the settled layout; written by components/graph/GraphCanvas.qml.
  property var graphLayout: null
  // Hide in the Changelog (WP-122): the attention items kept out of the
  // open list for this shell session, by Model.hideKey; nothing written.
  property var deskHidden: ({})
  // Ask agent in an event's or a case's bar, and the Changelog's "Agent
  // sorts N open changes" (WP-124b, ADR-0036): `agent ask` needs an engine
  // that can run. Whether a default agent exists only the engine knows
  // (the index does not say; the plugin reads nothing else): a refusal
  // names the fix in the result line.
  readonly property bool askAgentAvailable: root.canWrite
  readonly property var triageButton: Model.triageButton(root.indexShown ? root.index : null, root.canWrite)
  // The proposal index.triage names, next to the index (ADR-0035 §6): read
  // as a file the index points to, checked (Model.parseProposal), shown as
  // plain text. "" path: no proposal.
  readonly property string proposalPath: Model.triagePath(root.indexPath, root.indexShown ? root.index : null)
  property string proposalText: ""
  readonly property var proposal: root.proposalPath === "" ? null
    : Model.parseProposal(root.proposalText, root.index ? root.index.triage : null)
  readonly property var triageView: Model.triageView(root.indexShown ? root.index : null, root.proposal,
    root.deskChangelog, root.triageResult)
  // WP-156: the agents `seldon agent start` launched whose window is open
  // (or that are starting), by case ({ <caseId>: { starting, workspace,
  // actor } }, Model.sessionsResult), from `seldon agent sessions --json` in
  // its own process beside the queue; asked when the desk opens, after
  // every agent or open answer, when the index changes and every
  // Model.SESSIONS_POLL_MS while the desk is open.
  property var agentSessions: ({})
  property bool sessionsAgain: false
  // WP-138: before the logbook exists, what the machine remembers on its
  // own (`seldon preview --json`, Model.previewResult): asked in its own
  // read-only process when the status becomes notInitialised and when the
  // desk opens, at most every Model.PREVIEW_REFRESH_MS; null otherwise.
  property var preview: null
  property double previewAtMs: 0
  // The last successful `open` ({ what, atMs }): the same target is not
  // sent again within Model.OPEN_REPEAT_MS.
  property var lastOpen: null
  // How often the desk stepped aside for a window it opened.
  property int stepAsides: 0
  readonly property bool deskOpen: !!root.desk && root.desk.opened

  // Build the graph if the index changed since the last build.
  function graphRefresh() {
    if (!root.graphDirty) return
    root.graphDirty = false
    root.graphBuilds++
    root.graph = Model.graphBuild(root.indexShown ? root.index : null, Model.GRAPH_CAP)
  }
  // A new index may name another proposal, or the same file rewritten
  // (applied): read it again; and ask for the agent sessions again while
  // the desk is open (WP-156).
  onIndexChanged: {
    root.graphDirty = true
    if (root.proposalPath !== "") proposalFile.reload()
    if (root.deskOpen) root.refreshSessions()
  }
  onIndexShownChanged: root.graphDirty = true

  // How many aggregation passes this service's Model.js ran (periodTable and
  // its chart builders); the overlay reports it so the harness can show that
  // opening it and switching periods aggregate nothing (WP-031).
  function aggregationCount() {
    return Model.aggregationCount()
  }

  // ---- Engine calls: one at a time, in order.
  property bool busy: false
  property var queue: []
  property var currentArgs: []
  property string lastError: ""
  property int captureIntervalMin: Model.CAPTURE_INTERVAL_MIN_DEFAULT
  // The bar widget setting `driftInBar` (ADR-0028 §4a), pushed by the
  // widget like the capture interval; only the IPC read-out's `pill` uses it.
  property string driftInBar: Model.DRIFT_IN_BAR_DEFAULT
  // The desk's settings (ADR-0034 §1), pushed by the widget the same way:
  // the width in per cent of the screen and the sidebar's state, and the
  // whole shell.json entry, which the desk's settings write carries back
  // (the facade's updateEntryInline replaces the entry).
  property int deskWidth: Model.DESK_WIDTH_DEFAULT
  property string deskSidebar: Model.DESK_SIDEBAR_DEFAULT
  property var entrySettings: ({})
  // The widget has pushed an entry: the plugin is in the bar, so the shell
  // has a place to keep the desk's settings. Without it (the plugin listed
  // only under plugins[]) the desk keeps a change here, for this shell's
  // lifetime (localEntry), and writes nothing.
  property bool entryKnown: false
  property var localEntry: null
  // The loaded desk (Desk.qml registers itself), for the pill's
  // `jax.seldon.panel` shim; null while the shell has it unloaded.
  property var desk: null
  // What the desk remembers between opens within one shell session (the
  // shell unloads it on hide): { section, selected: { <section>: id } }.
  property var deskMemory: ({ section: Model.DESK_SECTION_DEFAULT, selected: {} })

  // Exit 4 (lock held) of a capture or status: what runs again when
  // lockRetry fires ("capture", which brings its status, or "status"), and
  // how many retries this run of the lock has had (Model.LOCK_RETRIES).
  property string lockRetryHead: ""
  property int lockRetries: 0
  readonly property int lockRetryMs: Number(Quickshell.env("SELDON_LOCK_RETRY_MS")) > 0
    ? Number(Quickshell.env("SELDON_LOCK_RETRY_MS")) : Model.LOCK_RETRY_MS
  readonly property int setupCaptureMs: Number(Quickshell.env("SELDON_SETUP_CAPTURE_MS")) > 0
    ? Number(Quickshell.env("SELDON_SETUP_CAPTURE_MS")) : Model.SETUP_CAPTURE_MS

  // Capture (and the status that follows it) is queued, running or waiting
  // for a held lock.
  readonly property bool capturing: root.queued("capture") || root.queued("status") || root.lockRetryHead !== ""

  // Panel actions write through the engine; this says whether they can, and
  // if not, why (shown in place of the action).
  readonly property string writeBlocker: root.devMode ? "Dev mode is read-only"
    : root.engineState !== "present" ? "Needs the Seldon engine"
    : root.status === "notInitialised" ? "Run seldon init first"
    : ""
  readonly property bool canWrite: root.writeBlocker === ""

  // The last call a one-at-a-time guard refused because another call of
  // its family was pending: { family, action, caseId, eventId, text } or
  // null, shown by the sheet that asked (Model.BUSY_TEXT). busyRefusals
  // counts them, so a sheet can tell that its own call was the one refused.
  property var busyRefusal: null
  property int busyRefusals: 0

  // The last result of each panel action, { ok, pending, text } or null:
  // QuickEntry (`log`), Open in editor (`open`), Capture now (`capture`),
  // the Work tab's case actions, Start agent and new-case sheet (`plan`;
  // also `action` — a plan verb, "new" or "agent" — and `caseId`, the case
  // the result is about), the drift sheet (`drift`;
  // also `action`, `eventId`, `caseId` — the linked or created case — and
  // `already` for a no-op re-run), the new-decision sheet (`decide`; also
  // `decisionId`, the created decision, which is then opened in the editor),
  // Accept on a decision (`decide accept`; also `decisionId` and `already`).
  property var logResult: null
  property var openResult: null
  property var captureResult: null
  // The warnings of the last capture the plugin ran that exited 0 (the
  // engine's text, Model.captureWarnings), and the panel's notice for them.
  // A failed or locked capture leaves them: only a capture that finished
  // can say a warning no longer holds (WP-085).
  property var captureWarnings: []
  readonly property var captureNotice: Model.captureWarningNotice(root.captureWarnings)
  property var planResult: null
  // WP-101: the rules row of `seldon doctor --json` (read-only, its own
  // process, never the write queue), checked when the panel opens, at most
  // every Model.RULES_CHECK_MS unless forced; the answer of the banner's
  // one-click `seldon rules update --json`.
  property string doctorText: ""
  property double rulesCheckedAtMs: 0
  property var rulesResult: null
  readonly property var rulesBanner: Model.rulesBannerWith(Model.rulesBanner(root.doctorText), root.rulesResult)
  // WP-111: one line for a finished *Update rules* click.
  readonly property var rulesNotice: Model.rulesNotice(root.rulesResult)
  property var driftResult: null
  property var decideResult: null
  property var acceptResult: null
  // WP-139: the engine's answer to Watch on a recently edited file
  property var watchResult: null
  // `agent ask …` (WP-124b): { ok, pending, text, what, target }.
  property var askResult: null
  // `drift apply|discard …` of a proposal: { ok, pending, text, action,
  // proposalId, eventId, gone, done, skipped, refused, markedApplied }.
  property var triageResult: null
  // The drift form's `seldon drift show` answer: { eventId, pending, ok,
  // text, members, rule, cls } — a group's members beyond what index.events
  // lists, and the item's rule (WP-122: the "why loud" callout).
  property var driftShown: null
  // The rules `drift show` named: { <eventId>: { rule, cls } }, kept while
  // the item is an open crisis (Model.keptDriftRules).
  property var driftRules: ({})
  // WP-102b, *Import tasks…*: the last `import task` call, { ok, pending,
  // text, dryRun, path, area, created, skipped, redactedLines, caseIds }
  // (Model.importResult); `path` and `area` are what was sent.
  property var importResult: null
  // WP-102b: `plan show <id> --json` of the selected imported case, { ok,
  // pending, text, caseId, intent, lines, truncated } (Model.caseShowResult):
  // the whole Intent the detail shows before its Start.
  property var caseShown: null

  // Emitted after every engine call, for panels that wait on a result.
  signal finished(var args, int exitCode, string output)

  function setCaptureInterval(minutes) {
    root.captureIntervalMin = Model.clampInterval(minutes)
  }

  function setDriftInBar(mode) {
    root.driftInBar = Model.driftInBarMode(mode)
  }

  // The widget's shell.json entry (its `settings`), as it is.
  function setDeskSettings(entry) {
    var copy = ({})
    if (entry && typeof entry === "object")
      for (var k in entry) copy[k] = entry[k]
    root.entrySettings = copy
    root.entryKnown = true
    // the shell brought the setup card's choice back: the entry decides
    if (root.setupLaterOverride !== null && Model.setupLaterStored(copy) === root.setupLaterOverride)
      root.setupLaterOverride = null
    root.deskWidth = Model.clampDeskWidth(copy.deskWidth)
    root.deskSidebar = Model.deskSidebarMode(copy.deskSidebar)
  }

  // ---- Index.

  function ingest(text) {
    var result = Model.parseIndex(text)
    // A rule holds while its item is still an open crisis; a rewritten
    // index drops the others (resolved, or reclassified), which are asked
    // again if they are selected as a crisis.
    root.driftRules = Model.keptDriftRules(root.driftRules, result.ok ? result.index : null)
    // The engine writes whole seconds: an index written in the second of
    // the exit 3 is the new one (`seldon init` right after the setup
    // card's probe, WP-119), an older one is a leftover.
    if (root.engineNotInitialised && result.ok
        && Model.timeMs(result.index.generatedAt) >= Math.floor(root.notInitialisedAtMs / 1000) * 1000)
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
    root.warnFailure(["--version"], exitCode, out, err)
    if (exitCode === 0) {
      root.engineVersion = Model.engineVersion(out)
      root.engineDetail = ""
      root.engineState = "present"
      if (Model.versionBelow(root.engineVersion, root.engineMin))
        console.warn("jax.seldon: engine " + root.engineVersion + " is older than engineMin " + root.engineMin)
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

  // One journal line per engine call that exited above 0 (Model.callWarning).
  function warnFailure(args, exitCode, out, err) {
    var line = Model.callWarning(args, exitCode, out, err)
    if (line !== "") console.warn(line)
  }

  // A one-at-a-time guard refused: tell the caller why (Model.BUSY_TEXT).
  function refuseBusy(family, action, caseId, eventId) {
    root.busyRefusal = { family: family, action: action, caseId: caseId, eventId: eventId, text: Model.BUSY_TEXT }
    root.busyRefusals++
    return false
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

  // ---- Panel actions (SPEC-PLUGIN §5). Each takes user input, builds a fixed
  // argument list in Model.js and reports into its result property.

  // QuickEntry: `seldon log [--case <id>] --json -- <text>`.
  function log(text, caseId) {
    var built = Model.logArgs(text, caseId)
    if (built.error) {
      root.logResult = { ok: false, pending: false, text: built.error }
      return false
    }
    if (!root.canWrite || !root.run(built.args)) {
      root.logResult = { ok: false, pending: false, text: root.writeBlocker || root.lastError }
      return false
    }
    root.logResult = { ok: true, pending: true, text: "Saving…" }
    return true
  }

  // Open in editor: journal | ledger | status | logbook | <caseId> | <ADR id>.
  // One open at a time, and not the same target again within
  // Model.OPEN_REPEAT_MS of a successful one (WP-156: three clicks were
  // three editors).
  function openInEditor(what) {
    var args = Model.openArgs(what)
    if (args === null) {
      console.warn("jax.seldon: refused to open " + JSON.stringify(String(what)))
      return false
    }
    if (root.openResult && root.openResult.pending) return root.refuseBusy("open", "open", "", "")
    if (Model.openRepeated(root.lastOpen, what, Date.now())) return false
    if (!root.run(args)) {
      root.openResult = { ok: false, pending: false, text: root.lastError }
      return false
    }
    root.openResult = { ok: true, pending: true, text: "Opening " + what + "…" }
    return true
  }

  // Work tab: `seldon plan new …` (form: { title, zone, risk, area,
  // priority }) or `seldon plan start|verify|done|drop <caseId>`. One plan
  // call at a time; the moved case arrives with the index (FileView).
  function plan(action, input) {
    var caseId = action === "new" ? "" : String(input || "")
    if (root.planResult && root.planResult.pending) return root.refuseBusy("plan", action, caseId, "")
    var built = Model.planArgs(action, input)
    if (built.error) {
      root.planResult = { ok: false, pending: false, text: built.error, action: action, caseId: caseId }
      return false
    }
    if (!root.canWrite || !root.run(built.args)) {
      root.planResult = { ok: false, pending: false, text: root.writeBlocker || root.lastError, action: action, caseId: caseId }
      return false
    }
    var doing = { new: "Creating the case", start: "Starting", verify: "Moving to verification:", done: "Completing", drop: "Dropping",
      reopen: "Reopening" }
    root.planResult = { ok: true, pending: true, text: doing[action] + (caseId !== "" ? " " + caseId : "") + "…",
      action: action, caseId: caseId }
    return true
  }

  // Work tab, an active case's *Start agent* (WP-022): `seldon agent start
  // <caseId> --json`. The engine launches the configured agent detached and
  // answers at once; the answer shares the plan result line (`action`
  // "agent") and its one-at-a-time rule.
  function startAgent(caseId) {
    var id = String(caseId || "")
    if (root.planResult && root.planResult.pending) return root.refuseBusy("plan", "agent", id, "")
    var built = Model.agentArgs(caseId)
    if (built.error) {
      root.planResult = { ok: false, pending: false, text: built.error, action: "agent", caseId: id }
      return false
    }
    if (!root.canWrite || !root.run(built.args)) {
      root.planResult = { ok: false, pending: false, text: root.writeBlocker || root.lastError, action: "agent", caseId: id }
      return false
    }
    root.planResult = { ok: true, pending: true, text: "Starting an agent on " + id + "…", action: "agent", caseId: id }
    return true
  }

  // WP-156: *Focus* on an active case an agent works on: `seldon agent
  // focus <caseId> --json` brings its window to the front; shares the plan
  // result line and its one-at-a-time rule like Hand to agent.
  function focusAgent(caseId) {
    var id = String(caseId || "")
    if (root.planResult && root.planResult.pending) return root.refuseBusy("plan", "focus", id, "")
    var built = Model.agentFocusArgs(caseId)
    if (built.error) {
      root.planResult = { ok: false, pending: false, text: built.error, action: "focus", caseId: id }
      return false
    }
    if (!root.canWrite || !root.run(built.args)) {
      root.planResult = { ok: false, pending: false, text: root.writeBlocker || root.lastError, action: "focus", caseId: id }
      return false
    }
    root.planResult = { ok: true, pending: true, text: "Bringing the agent on " + id + " to the front…", action: "focus", caseId: id }
    return true
  }

  // WP-156: ask the engine which agents it launched still run. Read-only,
  // its own process (never the queue); one at a time — a request while one
  // runs asks once more after it.
  function refreshSessions() {
    if (root.devMode || root.engineState !== "present" || root.status === "notInitialised") return false
    if (sessionsCall.running) {
      root.sessionsAgain = true
      return false
    }
    var args = ["agent", "sessions", "--json"]
    if (Model.validateArgs(args) !== "") return false
    root.sessionsAgain = false
    sessionsCall.launch(["seldon"].concat(args))
    return true
  }

  // WP-138: ask the engine for the preview (read-only, its own process,
  // never the queue). Only while the logbook is not initialised, with an
  // engine, outside dev mode; within Model.PREVIEW_REFRESH_MS of the last
  // only when `force`d. The last answer stays on screen while it runs.
  function refreshPreview(force) {
    if (root.devMode || root.engineState !== "present" || root.status !== "notInitialised" || previewCall.running)
      return false
    if (force !== true && root.previewAtMs > 0 && Date.now() - root.previewAtMs < Model.PREVIEW_REFRESH_MS) return false
    if (Model.validateArgs(Model.PREVIEW_ARGS) !== "") return false
    root.previewAtMs = Date.now()
    if (!root.preview) root.preview = { ok: false, pending: true, text: "" }
    previewCall.launch(["seldon"].concat(Model.PREVIEW_ARGS))
    return true
  }

  function previewDone(exitCode, out, err) {
    if (exitCode !== 0) root.warnFailure(["preview"], exitCode, out, err)
    // the logbook appeared meanwhile: the answer is no longer shown
    root.preview = root.status === "notInitialised" ? Model.previewResult(exitCode, out, err) : null
  }

  onStatusChanged: {
    if (root.status === "notInitialised") root.refreshPreview(true)
    else root.preview = null
  }

  function sessionsDone(exitCode, out, err) {
    if (exitCode !== 0) root.warnFailure(["agent", "sessions"], exitCode, out, err)
    var found = Model.sessionsResult(exitCode, out)
    if (found !== null) root.agentSessions = found
    if (root.sessionsAgain) root.refreshSessions()
  }

  // WP-156: a window the desk opened would appear under it; close the desk
  // (through the facade, as Esc does), as Omarchy's menus close for what
  // they launch. Nothing when the desk is not open.
  function stepAside() {
    if (!root.deskOpen) return false
    root.stepAsides++
    root.desk.dismiss()
    return true
  }

  // Work tab, *Run* (WP-101, ADR-0027 §6): `seldon agent start --new --json
  // -- <intent>`. The engine creates and starts the case from the sentence
  // and launches the configured agent; the answer shares the plan result
  // line (`action` "agent-new") and its one-at-a-time rule.
  function startAgentNew(intent) {
    if (root.planResult && root.planResult.pending) return root.refuseBusy("plan", "agent-new", "", "")
    var built = Model.agentNewArgs(intent)
    if (built.error) {
      root.planResult = { ok: false, pending: false, text: built.error, action: "agent-new", caseId: "" }
      return false
    }
    if (!root.canWrite || !root.run(built.args)) {
      root.planResult = { ok: false, pending: false, text: root.writeBlocker || root.lastError, action: "agent-new", caseId: "" }
      return false
    }
    root.planResult = { ok: true, pending: true, text: "Creating the case and starting an agent…", action: "agent-new", caseId: "" }
    return true
  }

  // *Import tasks…* (WP-102b): `seldon import task --json [--dry-run]
  // [--area <slug>] -- <path>`, the path one argument after `--`. One import
  // at a time; the created cases arrive with the index.
  function importTasks(path, area, dryRun) {
    var p = String(path || "")
    var a = String(area || "")
    var dry = dryRun === true
    if (root.importResult && root.importResult.pending) return root.refuseBusy("import", dry ? "dry-run" : "import", "", "")
    var fail = function(text) {
      root.importResult = { ok: false, pending: false, text: text, dryRun: dry, path: p, area: a, created: [], skipped: [],
        redactedLines: 0, caseIds: [] }
      return false
    }
    var built = Model.importArgs(p, a, dry)
    if (built.error) return fail(built.error)
    if (!root.canWrite || !root.run(built.args)) return fail(root.writeBlocker || root.lastError)
    root.importResult = { ok: true, pending: true, text: dry ? "Reading the task file…" : "Importing…", dryRun: dry,
      path: p, area: a, created: [], skipped: [], redactedLines: 0, caseIds: [] }
    return true
  }

  // The whole Intent of a case (WP-102b): `seldon plan show <id> --json`,
  // read-only. A call for the case already shown or pending is not repeated
  // unless `again`.
  // Asked again (a new index), the last text stays on screen, pending (so
  // Start is off until the answer), and an unchanged answer changes no text
  // (WP-102b round 2: a long Intent keeps its layout and scroll position).
  // A new index while the answer is still in flight is remembered
  // (`reaskWanted`) and asked once more when it arrives (WP-102b stage 2).
  function showCase(caseId, again) {
    var id = String(caseId || "")
    var c = root.caseShown
    if (c && c.caseId === id && c.pending) {
      if (again === true) c.reaskWanted = true
      return false
    }
    if (c && c.caseId === id && again !== true) return false
    return root.askCase(id)
  }

  function askCase(id) {
    var c = root.caseShown
    var built = Model.caseShowArgs(id)
    if (built.error || !root.canWrite || !root.run(built.args)) {
      root.caseShown = { ok: false, pending: false, text: built.error || root.writeBlocker || root.lastError, caseId: id,
        intent: "", lines: 0, truncated: false, hidden: 0 }
      return false
    }
    root.caseShown = c && c.caseId === id && c.ok
      ? { ok: true, pending: true, text: "Asking the engine again…", caseId: id, intent: c.intent, lines: c.lines,
          truncated: c.truncated, hidden: c.hidden }
      : { ok: false, pending: true, text: "Loading the whole Intent…", caseId: id, intent: "", lines: 0,
          truncated: false, hidden: 0 }
    return true
  }

  // The rules check (WP-101): `seldon doctor --json` in its own process.
  // Read-only; skipped in dev mode, without an engine, while one runs, and
  // within Model.RULES_CHECK_MS of the last unless `force`.
  function checkRules(force) {
    if (root.devMode || root.engineState !== "present" || doctorCall.running) return false
    if (!force && root.rulesCheckedAtMs > 0 && Date.now() - root.rulesCheckedAtMs < Model.RULES_CHECK_MS) return false
    var args = ["doctor", "--only", "rules", "--json"]
    if (Model.validateArgs(args) !== "") return false
    root.rulesCheckedAtMs = Date.now()
    doctorCall.launch(["seldon"].concat(args))
    return true
  }

  // The panel opened again: the last *Update rules* answer has been seen.
  function clearRulesResult() {
    if (root.rulesResult && !root.rulesResult.pending) root.rulesResult = null
  }

  // doctor exits 1 when a row is an error; its JSON is the answer either way.
  function doctorDone(exitCode, out, err) {
    if (exitCode > 1) root.warnFailure(["doctor"], exitCode, out, err)
    root.doctorText = exitCode <= 1 ? out : ""
  }

  // Drift sheet: `seldon drift link|explain|dismiss …` (input: { eventId,
  // caseId, only, text, zone, risk, area, itemZone }, see Model.driftArgs).
  // One drift call at a time; the resolved rows arrive with the index.
  function drift(action, input) {
    var eventId = input && typeof input.eventId === "string" ? input.eventId : ""
    if (root.driftResult && root.driftResult.pending) return root.refuseBusy("drift", action, "", eventId)
    var built = Model.driftArgs(action, input)
    if (built.error) {
      root.driftResult = { ok: false, pending: false, text: built.error, action: action, eventId: eventId, caseId: "", already: false }
      return false
    }
    if (!root.canWrite || !root.run(built.args)) {
      root.driftResult = { ok: false, pending: false, text: root.writeBlocker || root.lastError, action: action,
        eventId: eventId, caseId: "", already: false }
      return false
    }
    var doing = { link: "Linking", explain: "Explaining", dismiss: "Dismissing" }
    root.driftResult = { ok: true, pending: true, text: doing[action] + "…", action: action, eventId: eventId, caseId: "", already: false }
    return true
  }

  // New decision: `seldon decide --no-edit --json -- <title>`, then, once the
  // engine has created it, `seldon open <id> --editor --json` with the id
  // from its answer (checked against the schema pattern by Model.decideResult).
  // One decide call at a time.
  function decide(title) {
    if (root.decideResult && root.decideResult.pending) return root.refuseBusy("decide", "decide", "", "")
    var built = Model.decideArgs(title)
    if (built.error) {
      root.decideResult = { ok: false, pending: false, text: built.error, decisionId: "" }
      return false
    }
    if (!root.canWrite || !root.run(built.args)) {
      root.decideResult = { ok: false, pending: false, text: root.writeBlocker || root.lastError, decisionId: "" }
      return false
    }
    root.decideResult = { ok: true, pending: true, text: "Creating the decision…", decisionId: "" }
    return true
  }

  // Accept on a proposed decision (WP-135, ADR-0040): `seldon decide accept
  // <ADR-NNNN> --json`, the id checked against the schema pattern. One
  // accept at a time; the accepted decision arrives with the index.
  function acceptDecision(decisionId) {
    var id = String(decisionId || "")
    if (root.acceptResult && root.acceptResult.pending) return root.refuseBusy("accept", "accept", "", "")
    var built = Model.acceptArgs(id)
    if (built.error) {
      root.acceptResult = { ok: false, pending: false, text: built.error, decisionId: id, already: false }
      return false
    }
    if (!root.canWrite || !root.run(built.args)) {
      root.acceptResult = { ok: false, pending: false, text: root.writeBlocker || root.lastError, decisionId: id, already: false }
      return false
    }
    root.acceptResult = { ok: true, pending: true, text: "Accepting " + id + "…", decisionId: id, already: false }
    return true
  }

  // System › Recently edited, *Watch* (WP-139, ADR-0046): `seldon config
  // watch --json -- <path>`, the path one argument after `--`, checked as
  // the engine lists them. One at a time; the row goes with the index the
  // engine rebuilds.
  function watchPath(path) {
    var p = String(path || "")
    if (root.watchResult && root.watchResult.pending) return root.refuseBusy("watch", "watch", "", "")
    var built = Model.watchArgs(p)
    if (built.error) {
      root.watchResult = { ok: false, pending: false, text: built.error, path: p, added: false }
      return false
    }
    if (!root.canWrite || !root.run(built.args)) {
      root.watchResult = { ok: false, pending: false, text: root.writeBlocker || root.lastError, path: p, added: false }
      return false
    }
    root.watchResult = { ok: true, pending: true, text: "Watching " + p + "…", path: p, added: false }
    return true
  }

  // `seldon drift show <id> --json`: the full member list of a group whose
  // members index.events no longer lists all of (ADR-0013 §2). Read-only.
  function driftShow(eventId) {
    var id = String(eventId || "")
    if (root.driftShown && root.driftShown.pending) return false
    if (!Model.EVENT_ID.test(id) || !root.canWrite || !root.run(["drift", "show", id, "--json"])) return false
    root.driftShown = { eventId: id, pending: true, ok: true, text: "", members: [] }
    return true
  }

  // Ask agent (WP-124b, ADR-0036 §1): `seldon agent ask triage|drift
  // <eventId>|case <caseId> --json`. The engine launches the agent and
  // answers at once; one ask at a time.
  function askAgent(what, id) {
    var target = String(id || "")
    if (root.askResult && root.askResult.pending) return root.refuseBusy("ask", what, what === "case" ? target : "",
      what === "drift" ? target : "")
    var built = Model.askArgs(what, target)
    if (built.error) {
      root.askResult = { ok: false, pending: false, text: built.error, what: what, target: target }
      return false
    }
    if (!root.canWrite || !root.run(built.args)) {
      root.askResult = { ok: false, pending: false, text: root.writeBlocker || root.lastError, what: what, target: target }
      return false
    }
    root.askResult = { ok: true, pending: true, what: what, target: target,
      text: what === "triage" ? "Starting an agent to sort the open changes…" : "Asking an agent about " + target + "…" }
    return true
  }

  // Apply the proposal the user saw (`proposalId`, the detail's id): only
  // while the index still names it, so a replaced proposal is never
  // applied unseen. `eventId`: one crisis (`--item`), else the rest.
  function applyProposal(proposalId, eventId) {
    return root.triageCall("apply", proposalId, eventId || "")
  }

  // Discard the proposal the user saw.
  function discardProposal(proposalId) {
    return root.triageCall("discard", proposalId, "")
  }

  function triageCall(action, proposalId, eventId) {
    var id = String(proposalId || "")
    if (root.triageResult && root.triageResult.pending) return root.refuseBusy("triage", action, "", eventId)
    var fail = function(text) {
      root.triageResult = { ok: false, pending: false, text: text, action: action, proposalId: id, eventId: eventId,
        gone: false, done: [], skipped: [], refused: [] }
      return false
    }
    var current = root.index && root.index.triage && typeof root.index.triage.id === "string" ? root.index.triage.id : ""
    if (current !== id) return fail("This proposal is not the current one any more; review what the Changelog shows now")
    var built = action === "apply" ? Model.applyArgs(id, eventId) : Model.discardArgs(id)
    if (built.error) return fail(built.error)
    if (!root.canWrite || !root.run(built.args)) return fail(root.writeBlocker || root.lastError)
    root.triageResult = { ok: true, pending: true, action: action, proposalId: id, eventId: eventId, gone: false,
      done: [], skipped: [], refused: [],
      text: action === "discard" ? "Discarding the proposal…" : eventId !== "" ? "Applying " + eventId + "…" : "Applying the proposal…" }
    return true
  }

  function setResult(args, result) {
    if (args[0] === "log") root.logResult = result
    else if (args[0] === "open") root.openResult = result
    else if (args[0] === "capture") root.captureResult = result
    else if (args[0] === "plan" && args[1] === "show") {
      result.caseId = args[2]
      if (result.intent === undefined) result.intent = ""
      if (result.lines === undefined) result.lines = 0
      if (result.truncated === undefined) result.truncated = false
      if (result.hidden === undefined) result.hidden = 0
      var last = root.caseShown
      // the same text again: keep the string the box already lays out
      if (last && result.ok && last.ok && last.caseId === result.caseId && last.intent === result.intent)
        result.intent = last.intent
      if (Model.reaskAfter(last, result)) {
        // a new index came meanwhile: this answer enables nothing; ask again
        result.pending = true
        root.caseShown = result
        root.askCase(result.caseId)
        return
      }
      root.caseShown = result
    } else if (args[0] === "import") {
      var sep = args.indexOf("--")
      var at = args.indexOf("--area")
      result.dryRun = args.indexOf("--dry-run") !== -1
      result.path = sep !== -1 ? args[sep + 1] : ""
      result.area = at !== -1 && at < sep ? args[at + 1] : ""
      if (result.created === undefined) result.created = []
      if (result.skipped === undefined) result.skipped = []
      if (result.caseIds === undefined) result.caseIds = []
      if (result.redactedLines === undefined) result.redactedLines = 0
      root.importResult = result
    } else if (args[0] === "plan") {
      result.action = args[1]
      if (result.caseId === undefined || result.caseId === "") result.caseId = args[1] === "new" ? "" : args[2]
      root.planResult = result
    } else if (args[0] === "agent" && args[1] === "ask") {
      result.what = args[2]
      result.target = args.length > 4 ? args[3] : ""
      root.askResult = result
    } else if (args[0] === "drift" && (args[1] === "apply" || args[1] === "discard")) {
      result.action = args[1]
      result.proposalId = args[2]
      result.eventId = args[3] === "--item" ? args[4] : ""
      if (result.gone === undefined) result.gone = false
      if (result.done === undefined) result.done = []
      if (result.skipped === undefined) result.skipped = []
      if (result.refused === undefined) result.refused = []
      root.triageResult = result
    } else if (args[0] === "agent") {
      var isNew = args[2] === "--new"
      result.action = args[1] === "focus" ? "focus" : isNew ? "agent-new" : "agent"
      if (result.caseId === undefined || result.caseId === "") result.caseId = isNew ? "" : args[2]
      root.planResult = result
    } else if (args[0] === "config") {
      // the path asked for, also when the engine refused it
      result.path = args[args.length - 1]
      root.watchResult = result
    } else if (args[0] === "rules") {
      root.rulesResult = result
    } else if (args[0] === "drift" && args[1] === "show") {
      result.eventId = args[2]
      if (result.members === undefined) result.members = []
      if (result.ok && result.rule) {
        var rules = ({})
        for (var k in root.driftRules) rules[k] = root.driftRules[k]
        rules[args[2]] = { rule: result.rule, cls: result.cls }
        root.driftRules = rules
      }
      root.driftShown = result
    } else if (args[0] === "decide" && args[1] === "accept") {
      if (result.decisionId === undefined || result.decisionId === "") result.decisionId = args[2]
      if (result.already === undefined) result.already = false
      root.acceptResult = result
    } else if (args[0] === "decide") {
      if (result.decisionId === undefined) result.decisionId = ""
      root.decideResult = result
    } else if (args[0] === "drift") {
      result.action = args[1]
      result.eventId = args[2]
      if (result.caseId === undefined) result.caseId = ""
      if (result.already === undefined) result.already = false
      root.driftResult = result
    }
  }

  // Calls that will never run still owe their result line an answer.
  function dropQueue(reason) {
    for (var i = 0; i < root.queue.length; i++)
      root.setResult(root.queue[i], { ok: false, pending: false, text: "Not run: " + reason })
    root.queue = []
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
    root.warnFailure(args, exitCode, out, err)
    if (exitCode === 4 && root.retryLater(args)) {
      root.finished(args, exitCode, out)
      root.pump()
      return
    }
    if (args[0] === "capture" || args[0] === "status") root.lockRetries = 0
    var result = args[0] === "log" ? Model.logResult(exitCode, out, err)
      : args[0] === "open" ? Model.openResult(exitCode, out, err)
      : args[0] === "capture" ? Model.captureResult(exitCode, out, err)
      : args[0] === "plan" && args[1] === "show" ? Model.caseShowResult(exitCode, out, err)
      : args[0] === "plan" ? Model.planResult(exitCode, out, err)
      : args[0] === "agent" && args[1] === "focus" ? Model.focusResult(exitCode, out, err)
      : args[0] === "import" ? Model.importResult(exitCode, out, err)
      : args[0] === "agent" && args[1] === "ask" ? Model.askResult(exitCode, out, err)
      : args[0] === "agent" ? Model.agentResult(exitCode, out, err)
      : args[0] === "drift" && args[1] === "apply" ? Model.applyResult(exitCode, out, err)
      : args[0] === "drift" && args[1] === "discard" ? Model.discardResult(exitCode, out, err)
      : args[0] === "drift" && args[1] === "show" ? Model.driftShowResult(exitCode, out, err)
      : args[0] === "drift" ? Model.driftResult(args[1], exitCode, out, err)
      : args[0] === "decide" && args[1] === "accept" ? Model.acceptResult(exitCode, out, err)
      : args[0] === "decide" ? Model.decideResult(exitCode, out, err)
      : args[0] === "rules" ? Model.rulesUpdateResult(exitCode, out, err)
      : args[0] === "config" ? Model.watchResult(exitCode, out, err)
      : null
    if (result) {
      result.pending = false
      root.setResult(args, result)
    }
    if (args[0] === "capture" && exitCode === 0) root.captureWarnings = result.warnings
    if (exitCode === 0) {
      root.lastError = ""
      root.engineNotInitialised = false
    } else if (exitCode === 3) {
      // Nothing else can succeed until `seldon init` has run.
      root.notInitialisedAtMs = Date.now()
      root.engineNotInitialised = true
      root.notInitialisedInfo = Model.notInitialisedInfo(out)
      root.dropQueue("the logbook is not initialised")
      root.lastError = ""
    } else if (args[0] !== "log" && args[0] !== "plan" && args[0] !== "agent" && args[0] !== "drift" && args[0] !== "decide"
        && args[0] !== "rules" && args[0] !== "import" && args[0] !== "config") {
      // QuickEntry, the Work tab (case actions, Start agent), the drift sheet
      // and the new-decision sheet, Accept and Watch show their own errors in place.
      root.lastError = "seldon " + args[0] + ": " + Model.engineError(out, err, exitCode)
    }
    // The engine rewrites index.json atomically; reload in case the watch
    // missed the rename.
    indexFile.reload()
    // A created decision opens in the editor (the id is checked).
    if (args[0] === "decide" && args[1] !== "accept" && result && result.ok && result.decisionId !== "") root.openInEditor(result.decisionId)
    // Updated rules: ask doctor again, so the banner goes.
    if (args[0] === "rules") root.checkRules(true)
    if (args[0] === "open" && result && result.ok) root.lastOpen = { what: args[1], atMs: Date.now() }
    // An agent started, refused as already working, or gone, an editor
    // opened or focused: ask again (SPEC-PLUGIN §5.4).
    if (args[0] === "agent" || args[0] === "open") root.refreshSessions()
    root.finished(args, exitCode, out)
    // The window it opened comes up in front (WP-156).
    if (Model.opensWindow(args, result)) root.stepAside()
    root.pump()
  }

  // Exit 4 of a capture or status: another seldon holds the lock for a
  // moment (a hook, the CLI). Run it again after lockRetryMs, at most
  // Model.LOCK_RETRIES times, with a neutral capture result meanwhile; a
  // locked capture takes its queued status along, which would only find
  // the lock too. False when this is not such a call or the retries are
  // used up: then the exit is an error like any other.
  function retryLater(args) {
    var head = args[0]
    if (head !== "capture" && head !== "status") return false
    if (root.lockRetries >= Model.LOCK_RETRIES) {
      root.lockRetries = 0
      return false
    }
    if (head === "capture") {
      var rest = []
      var dropped = false
      for (var i = 0; i < root.queue.length; i++) {
        if (!dropped && root.queue[i][0] === "status") dropped = true
        else rest.push(root.queue[i])
      }
      root.queue = rest
      root.captureResult = { ok: true, pending: false, text: Model.LOCK_WAIT_TEXT }
    }
    if (root.lockRetryHead !== "capture") root.lockRetryHead = head
    root.lockRetries++
    lockRetry.interval = root.lockRetryMs
    lockRetry.restart()
    return true
  }

  function retryLocked() {
    var head = root.lockRetryHead
    root.lockRetryHead = ""
    if (root.engineState !== "present" || root.devMode) {
      root.lockRetries = 0
      return
    }
    if (head === "capture") root.captureNow()
    else if (head === "status" && !root.queued("status")) root.run(["status", "--json"])
  }

  function runnerFailedToStart() {
    var args = root.currentArgs
    watchdog.stop()
    root.busy = false
    root.currentArgs = []
    root.setResult(args, { ok: false, pending: false, text: "seldon not found on PATH" })
    root.dropQueue("seldon not found on PATH")
    root.lastError = ""
    root.engineVersion = ""
    root.engineDetail = "seldon not found on PATH"
    root.engineState = "missing"
  }

  // Capture, then status (which rewrites the index). Never queued twice.
  function captureNow() {
    if (root.queued("capture")) return false
    // An explicit capture replaces a pending lock retry (its counter stays).
    if (root.lockRetryHead !== "") {
      lockRetry.stop()
      root.lockRetryHead = ""
    }
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
  // bannerId picks the banner whose constants copy and terminal use:
  // "status" (default), "snapper" (ADR-0026) or "contract" (the newer
  // contract's notice, ADR-0051). Copy puts the plain command
  // on the clipboard; terminal opens the banner's terminal script (WP-117),
  // only one of Model.TERMINAL_SCRIPTS (Model.terminalArgv). "restart" is the restart
  // notice's own action (WP-090): the fixed argv, only while it shows, and
  // only once per service (restartStarted).
  function fix(actionId, bannerId) {
    if (bannerId === "restart") {
      if (actionId !== "restart" || !root.restartNotice || root.restartStarted) return false
      root.restartStarted = true
      Quickshell.execDetached(Model.RESTART_SHELL_ARGV)
      return true
    }
    if (bannerId === "rules") {
      // One click (WP-101): the engine rewrites only its own block.
      if (actionId !== "update" || !root.rulesBanner || root.queued("rules")) return false
      if (!root.canWrite || !root.run(["rules", "update", "--json"])) return false
      root.rulesResult = { ok: true, pending: true, text: "Updating the rules…" }
      return true
    }
    var source = bannerId === "snapper" ? root.snapperBanner
      : bannerId === "contract" ? root.contractNotice
      // the setup card's Choose a folder (WP-119): only before init
      : bannerId === "initAsk" ? (root.status === "notInitialised" ? Model.INIT_ASK_FIX : null)
      : root.banner
    var command = source ? source.command : ""
    var terminal = Model.terminalArgv(source)
    if (actionId === "copy" && command !== "") {
      Quickshell.execDetached(["wl-copy", "--", command])
    } else if (actionId === "terminal" && terminal) {
      // Opens on the explicit click only (ADR-0004); the script is a constant.
      Quickshell.execDetached(terminal)
      // The install, the logbook and the grant: look again by itself
      // (WP-119), the engine's install also from the urgent notice.
      var stepId = Model.setupStepOf(source)
      if (stepId !== "") root.watchSetup(stepId)
      // No answer comes back from a detached launch: step aside at once,
      // as Omarchy's menus do (WP-156).
      root.stepAside()
    } else if (actionId === "recheck") {
      root.probeEngine()
      root.reloadIndex()
    } else if (actionId === "build") {
      root.run(["status", "--json"])
    } else if (actionId === "capture") {
      // Also the snapper banner's "Check again" (WP-054).
      root.captureNow()
    } else {
      return false
    }
    return true
  }

  // ---- Setup card (WP-119). One of the current step's actions: the
  // terminal fix of the banner behind it (then the service looks again by
  // itself, setupTick), Copy, or Not now on the snapshot step, which the
  // desk stores in the shell.json entry (Desk.writeSetting). Returns
  // whether it was taken.
  function setupAction(stepId, actionId) {
    var card = root.setup
    var steps = card ? card.steps : []
    var step = null
    for (var i = 0; i < steps.length; i++) if (steps[i].id === stepId && steps[i].current) step = steps[i]
    if (!step || !step.ready) return false
    if (actionId === "later") {
      if (stepId !== "snapshots") return false
      return root.setSetupLater(true)
    }
    if (actionId === "copy") return root.fix("copy", step.banner)
    if (actionId !== "terminal") return false
    return root.fix("terminal", step.banner)
  }

  // A setup terminal opened (the card's or a notice's): look again by
  // itself until the step is done (setupTick).
  function watchSetup(stepId) {
    root.setupResult = ""
    root.setupWatch = { step: stepId, untilMs: Date.now() + Model.SETUP_WATCH_MS,
      captureAtMs: Date.now() + root.setupCaptureMs, probes: 0 }
  }

  // Not now (true) or Settings' Offer again (false): held here at once,
  // stored by the desk in the shell.json entry; without a desk or an
  // entry it holds for this shell's life (the line says so).
  function setSetupLater(later) {
    root.setupLaterOverride = later === true
    var written = root.desk ? root.desk.writeSetting(Model.SETUP_LATER_KEY, later === true ? Model.SETUP_LATER_VALUE : undefined) : "session"
    root.setupResult = written === "written" || written === "unchanged" ? ""
      : written === "refused" ? "The shell did not take the change; it holds until the shell restarts."
      : Model.DESK_NO_ENTRY_TEXT
    if (later !== true) root.setupWatch = null
    return true
  }

  // Every Model.SETUP_PROBE_MS while setupWatch is set: the engine probe
  // for step 1, the index for step 2 (the FileView watches it too), a
  // capture every Model.SETUP_CAPTURE_MS for step 3 (only a capture
  // rewrites the collector state; the grant's own capture comes first).
  // Ends when the step is done, the card is gone, or after
  // Model.SETUP_WATCH_MS.
  function setupTick() {
    var w = root.setupWatch
    if (!w) return
    var steps = root.setup ? root.setup.steps : []
    var step = null
    for (var i = 0; i < steps.length; i++) if (steps[i].id === w.step) step = steps[i]
    var now = Date.now()
    var done = w.step === "engine" ? root.engineState === "present"
      : w.step === "logbook" ? root.status !== "notInitialised"
      : !step || step.done || step.later
    if (done || now > w.untilMs) {
      root.setupWatch = null
      return
    }
    // what runs slowly: the step-3 capture, step 2's status (it tells
    // why a failed init failed, CONTRACT.md rule 10), and step 1's probe
    // after its first Model.SETUP_PROBES_FAST (each failed probe is a line
    // in the shell's log: at most about 40 per watch)
    var slow = now >= w.captureAtMs
    var next = slow ? now + root.setupCaptureMs : w.captureAtMs
    var probes = w.probes || 0
    if (w.step === "engine") {
      if (probes < Model.SETUP_PROBES_FAST || slow) {
        root.probeEngine()
        probes++
      }
    } else if (w.step === "logbook") {
      root.reloadIndex()
      if (slow && root.engineState === "present" && !root.queued("status")) root.run(["status", "--json"])
    } else if (w.step === "snapshots" && slow && !root.capturing) {
      root.captureNow()
    }
    root.setupWatch = { step: w.step, untilMs: w.untilMs, captureAtMs: next, probes: probes }
  }

  function snapshot() {
    return {
      status: root.status,
      ready: root.ready,
      devMode: root.devMode,
      indexPath: root.indexPath,
      fileState: root.fileState,
      indexContractVersion: root.indexContractVersion,
      indexReadableFrom: root.parsed ? root.parsed.readableFrom : 0,
      contractNotice: root.contractNotice ? root.contractNotice.detail : "",
      contractActions: root.contractNotice ? root.contractNotice.actions.map(function(a) { return a.id + ":" + a.label }) : [],
      engine: root.engineState,
      engineVersion: root.engineVersion,
      engineDetail: root.engineDetail,
      busy: root.busy,
      preview: root.preview ? Model.previewSummary(root.preview) : "",
      setup: root.setup ? {
        headline: root.setup.headline,
        current: root.setup.current,
        open: root.setup.open,
        total: root.setup.total,
        steps: root.setup.steps.map(function(s) {
          return s.id + ":" + (s.done ? "done" : s.later ? "later" : s.current ? "current" : "waiting")
        }),
        ready: root.setup.steps.some(function(s) { return s.current && s.ready })
      } : null,
      setupLater: root.setupLater,
      setupWatch: root.setupWatch ? root.setupWatch.step : "",
      setupResult: root.setupResult,
      capturing: root.capturing,
      lockRetries: root.lockRetries,
      engineMin: root.engineMin,
      pluginVersion: root.pluginVersion,
      manifestVersion: root.manifestVersion,
      restartNotice: root.restartNotice ? root.restartNotice.title : "",
      restartStarted: root.restartStarted,
      restartActions: root.restartNotice ? root.restartNotice.actions.map(function(a) { return a.id + ":" + a.label }) : [],
      busyRefusal: root.busyRefusal,
      canWrite: root.canWrite,
      lastError: root.lastError,
      logResult: root.logResult,
      openResult: root.openResult,
      captureResult: root.captureResult,
      captureWarnings: root.captureWarnings,
      captureNotice: root.captureNotice ? root.captureNotice.detail : "",
      planResult: root.planResult,
      driftResult: root.driftResult,
      driftShown: root.driftShown,
      decideResult: root.decideResult,
      acceptResult: root.acceptResult,
      watchResult: root.watchResult,
      askResult: root.askResult,
      triageResult: root.triageResult,
      importResult: root.importResult,
      caseShown: root.caseShown ? { caseId: root.caseShown.caseId, ok: root.caseShown.ok, pending: root.caseShown.pending,
        lines: root.caseShown.lines, truncated: root.caseShown.truncated, hidden: root.caseShown.hidden,
        text: root.caseShown.text } : null,
      triageButton: root.triageButton,
      proposalPath: root.proposalPath,
      proposalRead: !!root.proposal,
      agentSessions: root.agentSessions,
      stepAsides: root.stepAsides,
      lastOpen: root.lastOpen ? root.lastOpen.what : "",
      pill: Model.pillText(root.counts, root.driftInBar),
      driftInBar: root.driftInBar,
      deskWidth: root.deskWidth,
      deskSidebar: root.deskSidebar,
      tone: Model.pillTone(root.counts),
      tooltip: Model.tooltipText(root.status, root.counts, root.lastCapture, root.nowMs),
      banner: root.banner ? root.banner.title : "",
      bannerTone: root.banner ? root.banner.tone : "",
      bannerDetail: root.banner ? root.banner.detail : "",
      bannerActions: root.banner ? root.banner.actions.map(function(a) { return a.id + ":" + a.label }) : [],
      crisis: root.crisisText,
      snapper: root.snapperBanner ? root.snapperBanner.title : "",
      snapperDetail: root.snapperBanner ? root.snapperBanner.detail : "",
      snapperActions: root.snapperBanner ? root.snapperBanner.actions.map(function(a) { return a.id + ":" + a.label }) : [],
      snapperFull: root.snapperBanner ? root.snapperBanner.full : "",
      rules: root.rulesBanner ? root.rulesBanner.title : "",
      rulesResult: root.rulesResult,
      rulesNotice: root.rulesNotice ? root.rulesNotice.title : ""
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
    id: doctorCall
    onDone: function(exitCode, out, err) { root.doctorDone(exitCode, out, err) }
    onFailedToStart: root.doctorText = ""
  }

  EngineCall {
    id: previewCall
    onDone: function(exitCode, out, err) { root.previewDone(exitCode, out, err) }
    onFailedToStart: root.preview = null
  }

  EngineCall {
    id: sessionsCall
    onDone: function(exitCode, out, err) { root.sessionsDone(exitCode, out, err) }
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

  // The proposal index.triage points to (WP-124b).
  FileView {
    id: proposalFile
    path: root.proposalPath
    watchChanges: true
    printErrors: false
    onLoaded: root.proposalText = proposalFile.text()
    onLoadFailed: root.proposalText = ""
    onFileChanged: proposalFile.reload()
  }

  // The setup card's own look again after a step's terminal (setupTick).
  Timer {
    interval: Model.SETUP_PROBE_MS
    repeat: true
    running: root.setupWatch !== null
    onTriggered: root.setupTick()
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
  // the status banner's "Check again", a desk open and the setup card's
  // watch, so a machine without it logs one probe per shell start and
  // per desk open.
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

  // The live sessions while the desk is open (WP-156): an agent's window
  // closed by hand ends its session without an index change.
  Timer {
    interval: Model.SESSIONS_POLL_MS
    repeat: true
    running: root.deskOpen && root.engineState === "present"
    onTriggered: root.refreshSessions()
  }
  onDeskOpenChanged: if (root.deskOpen) {
    // the setup card's first step may have been done outside the desk
    if (root.engineState === "missing" && !root.devMode) root.probeEngine()
    root.refreshSessions()
    root.refreshPreview(false)
  }
  onEngineStateChanged: {
    if (root.deskOpen) root.refreshSessions()
    // the index may have said notInitialised before the probe answered
    root.refreshPreview(true)
  }

  // The retry of a capture or status that found the lock held.
  Timer {
    id: lockRetry
    repeat: false
    onTriggered: root.retryLocked()
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
