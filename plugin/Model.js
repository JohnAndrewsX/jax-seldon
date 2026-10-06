// Pure helpers for jax.seldon: index parsing, status derivation, pill and
// banner text, and validation of engine argument lists.
//
// Nothing here touches Qt, files or processes, so the same file runs under
// node (tests/plugin/model.test.js). Service.qml owns all I/O; BarWidget.qml,
// Desk.qml and components/ only render what these functions return,
// including the rows of the Today, Changelog and System tabs.

var CONTRACT_VERSION = 1

// The version of the code that is running: always equal to `version` in
// plugin/manifest.json, set by hand with it (docs/VERSIONING.md, tag flow;
// model.test.js and the harness fail when they differ). The shell reads the
// manifest from disk on every rescan but keeps running the plugin code it
// compiled first (WP-090), so a manifest that says otherwise means the
// plugin was updated under a running shell (restartShellNotice).
var PLUGIN_VERSION = "0.1.3"

// SPEC-PLUGIN §3: an index older than two hours is stale.
var STALE_AFTER_MS = 2 * 60 * 60 * 1000

// ADR-0005: capture at shell start and every 15 minutes.
var CAPTURE_INTERVAL_MIN_DEFAULT = 15
var CAPTURE_INTERVAL_MIN_MIN = 5
var CAPTURE_INTERVAL_MIN_MAX = 120

// ADR-0028 §4a: what the pill's D counts, the bar widget setting
// `driftInBar`: `crisis` (default; the crisis count), `all` (every open
// drift item, the behaviour before 0.1.4) or `none` (D hidden). The colour
// is the same in every mode: urgent while any crisis is open.
var DRIFT_IN_BAR_MODES = ["crisis", "all", "none"]
var DRIFT_IN_BAR_DEFAULT = "crisis"

// The service's `status` (named so because `state` clashes with Item.state).
var STATUSES = ["ok", "engineMissing", "notInitialised", "indexMissing", "indexStale", "contractMismatch"]

// Patterns from schema/event.schema.json $defs.
var CASE_ID = /^C-[0-9]{4}-[0-9]{3,}$/
var EVENT_ID = /^[0-7][0-9A-HJKMNP-TV-Z]{25}$/
var ZONES = ["green", "yellow", "red"]
var RISKS = ["R0", "R1", "R2", "R3"]
// `seldon open` targets the panel uses; `logbook` (the logbook folder) stands
// in for the memory files until the engine can open them (WP-023).
var OPEN_TARGETS = ["journal", "ledger", "status", "logbook"]
// schema/index.schema.json decisions[].id.
var DECISION_ID = /^ADR-[0-9]{4}$/
// schema/case.schema.json: priority, area; the steps of `seldon plan`.
var PRIORITIES = ["high", "normal", "low"]
var AREA = /^[a-z0-9][a-z0-9-]*$/
var PLAN_STEPS = ["start", "verify", "done", "drop"]

// Fix commands shown in banners. Constants only: nothing from the index is
// ever spliced into a command (AGENTS.md §8).
// The engine install is the GitHub one-liner while the AUR package does not
// exist (ADR-0024): it runs only on the user's click, in the floating
// terminal (whose bash -c runs the pipe), fetches install.sh over TLS from
// this project's release, and the script checks the engine against
// SHA256SUMS. Flip back to "omarchy pkg aur add jax-seldon" (ADR-0004,
// ADR-0016: `omarchy pkg add` only reaches the official repositories) when
// the AUR package is live, together with ENGINE_MISSING_DETAIL below.
var INSTALL_ENGINE_COMMAND = "curl -fsSL https://github.com/JohnAndrewsX/jax-seldon/releases/latest/download/install.sh | bash"
// While the AUR package does not exist, updating the engine is the same
// installer (ADR-0024); flip back together with INSTALL_ENGINE_COMMAND.
var UPDATE_ENGINE_COMMAND = INSTALL_ENGINE_COMMAND
var UPDATE_PLUGIN_COMMAND = "omarchy plugin update jax.seldon"
var INIT_COMMAND = "seldon init"
// ADR-0026: the one-time read grant on the snapshot directory that lets the
// snapper collector read the snapshot info files. `$USER` is expanded by the
// shell the user pastes it into (or by the terminal launcher's bash -c);
// nothing else in it varies.
var SNAPPER_FIX_COMMAND = "sudo setfacl -m u:$USER:rx /.snapshots"
// What SNAPPER_FIX_COMMAND grants; the banner shows it under the engine's
// message, like `seldon doctor` (SNAPPER_FIX_GRANTS there).
var SNAPPER_FIX_GRANTS = "The command below grants your user read access to the snapshot directory listing and the snapshot info files (files inside a snapshot keep their own permissions), nothing else: no snapshot creation, change or deletion."
// The engineMissing banner's text. The AUR package does not exist yet
// (operator, 2026-10-02); flip this with INSTALL_ENGINE_COMMAND (WP-044).
var ENGINE_MISSING_DETAIL = "The plugin needs the seldon command. AUR package: coming soon; until then install from GitHub: the command below downloads install.sh from the release, which checks the engine against SHA256SUMS. Then check again."

function isObject(value) {
  return value !== null && typeof value === "object" && !Array.isArray(value)
}

function count(value) {
  var n = Number(value)
  return isFinite(n) && n > 0 ? Math.floor(n) : 0
}

// ---- Index ------------------------------------------------------------------

// Parse index.json text. Returns { ok, error, detail, contractVersion, index }.
// error: "" | "empty" | "parse" | "shape" | "contract". Only a matching
// contractVersion yields an index; a mismatch still reports the version found
// so the banner can show both numbers (CONTRACT.md rule 3).
function parseIndex(text) {
  var raw = text === undefined || text === null ? "" : String(text)
  var result = { ok: false, error: "", detail: "", contractVersion: 0, index: null }
  if (raw.trim() === "") {
    result.error = "empty"
    return result
  }
  var data
  try {
    data = JSON.parse(raw)
  } catch (e) {
    result.error = "parse"
    result.detail = String(e && e.message ? e.message : e)
    return result
  }
  if (!isObject(data)) {
    result.error = "shape"
    result.detail = "not a JSON object"
    return result
  }
  result.contractVersion = typeof data.contractVersion === "number" ? data.contractVersion : 0
  if (result.contractVersion !== CONTRACT_VERSION) {
    result.error = "contract"
    return result
  }
  if (!isObject(data.summary) || !isObject(data.state) || typeof data.generatedAt !== "string") {
    result.error = "shape"
    result.detail = "summary, state or generatedAt missing"
    return result
  }
  result.ok = true
  result.index = data
  return result
}

function timeMs(text) {
  if (typeof text !== "string" || text === "") return NaN
  return Date.parse(text)
}

// An index is stale when generatedAt is more than two hours behind `nowMs`.
// An unreadable timestamp counts as stale; a future one does not.
function isStale(generatedAt, nowMs) {
  var at = timeMs(generatedAt)
  if (!isFinite(at)) return true
  return nowMs - at > STALE_AFTER_MS
}

// The clock the status machine reads. Outside development it is the live
// clock. With SELDON_INDEX set (devMode), a fixture's fixed generatedAt would
// turn stale two hours after it was written, so the clock is pinned: to
// SELDON_NOW when that parses, otherwise to the index's own generatedAt.
function effectiveNowMs(liveMs, devMode, devNow, generatedAt) {
  if (!devMode) return liveMs
  var pinned = timeMs(devNow)
  if (isFinite(pinned)) return pinned
  var at = timeMs(generatedAt)
  return isFinite(at) ? at : liveMs
}

// s: { engine: "unknown"|"present"|"missing",
//      file: "loading"|"loaded"|"missing"|"invalid",
//      parse: parseIndex() result or null,
//      engineNotInitialised: bool, nowMs: number }
// Precedence: without an engine nothing can be fixed, so engineMissing wins;
// then the index file itself; then what the index says.
function deriveStatus(s) {
  if (!s) return "indexMissing"
  if (s.engine === "missing") return "engineMissing"
  var parsed = s.parse
  if (s.file !== "loaded" || !parsed) return s.engineNotInitialised ? "notInitialised" : "indexMissing"
  if (parsed.error === "contract") return "contractMismatch"
  if (!parsed.ok) return s.engineNotInitialised ? "notInitialised" : "indexMissing"
  var index = parsed.index
  if (s.engineNotInitialised || index.state.status === "notInitialised") return "notInitialised"
  if (index.state.status === "indexStale" || isStale(index.generatedAt, s.nowMs)) return "indexStale"
  return "ok"
}

// Counts the pill and tooltip need, or null without an index. ADR-0028 §7:
// attention (open drift that is no crisis) is openDrift − crisis; the
// contract has no field of its own for it.
function counts(index) {
  if (!index || !isObject(index.summary)) return null
  var summary = index.summary
  var drift = count(summary.openDrift)
  var crisis = count(summary.crisis)
  return {
    active: count(summary.activeCases),
    queued: count(summary.queuedCases),
    drift: drift,
    crisis: crisis,
    attention: Math.max(0, drift - crisis)
  }
}

function lastCapture(index) {
  return index && isObject(index.state) && typeof index.state.lastCapture === "string"
    ? index.state.lastCapture : ""
}

// ---- Pill -------------------------------------------------------------------

// The setting `driftInBar` as one of DRIFT_IN_BAR_MODES; anything else
// (a hand-edited shell.json) is the default.
function driftInBarMode(value) {
  return DRIFT_IN_BAR_MODES.indexOf(value) !== -1 ? value : DRIFT_IN_BAR_DEFAULT
}

// SPEC-PLUGIN §4: the counts after the bar glyph, `A · D`, parts hidden
// when 0: "", `2`, `· 3`, `2 · 3`. D by `mode` (driftInBarMode): the crisis
// count, all open drift, or never. The glyph itself is an image (barGlyph).
function pillText(c, mode) {
  var m = driftInBarMode(mode)
  var d = !c ? 0 : m === "all" ? c.drift : m === "crisis" ? c.crisis : 0
  var parts = []
  if (c && c.active > 0) parts.push(String(c.active))
  if (d > 0) parts.push("· " + d)
  return parts.join(" ")
}

// "urgent" when any crisis, "accent" when cases are active, else "default".
function pillTone(c) {
  if (c && c.crisis > 0) return "urgent"
  if (c && c.active > 0) return "accent"
  return "default"
}

// ---- Assets (plugin/assets/, copies from assets/; assets/README.md) ----------

// Every Prime Radiant mask is one colour: its shapes are `currentColor` and
// the fallback colour sits on the <svg> root only (design round 3, F1), so
// the root's `color` tints the whole file. tintedSvg() sets it to a theme
// colour ("#rrggbb", from Style/Color/the bar, never a constant) and returns
// the data URL components/MaskIcon.qml hands to Image; "" when either input
// is unusable.
function tintedSvg(svg, rgb) {
  var text = typeof svg === "string" ? svg : ""
  var open = text.match(/<svg\b[^>]*>/)
  if (!open || !/^#[0-9a-fA-F]{6}$/.test(String(rgb))) return ""
  var tag = open[0]
  var tinted = /\scolor="[^"]*"/.test(tag)
    ? tag.replace(/\scolor="[^"]*"/, " color=\"" + rgb + "\"")
    : tag.replace(/^<svg\b/, "<svg color=\"" + rgb + "\"")
  return "data:image/svg+xml;utf8," + encodeURIComponent(text.replace(tag, tinted))
}

// A4, the bar glyph (assets/DELIVERY.md §5): hand-hinted for the 16 and the
// 20 px box (bar scale 1.0 and 1.25), the vector for every other box. `box`
// is the glyph box in device pixels (Style.bar.iconCanvas times the output
// scale). `centre` is the glyph's vertical ink centre as a fraction of the
// box: the hinted grids have their centre pixel in row 7 of 16 and row 9 of
// 20, the vector is centred. BarWidget puts that centre on the digits'.
function barGlyph(box) {
  var px = Math.round(Number(box) || 0)
  if (px === 16) return { file: "a4-bar-glyph-16.svg", crisp: true, centre: 7.5 / 16 }
  if (px === 20) return { file: "a4-bar-glyph-20.svg", crisp: true, centre: 9.5 / 20 }
  return { file: "a4-bar-glyph.svg", crisp: false, centre: 0.5 }
}

// A5, the panel header mark (DELIVERY.md §5): the box is twice the heading's
// cap height, rounded to an even pixel count; the wordmark starts half a cap
// height after it; its baseline sits at box / 2 + cap / 2 below the box top
// (the mark's centre on the cap-height centre). For the 16 px heading that
// is 24 / 6 / 17.85, the delivered metrics. The hinted grids at 24 and 32
// device pixels, the A1 master for every other size.
function panelMark(capHeight, dpr) {
  var cap = Math.max(1, Number(capHeight) || 1)
  var ratio = Number(dpr) > 0 ? Number(dpr) : 1
  var box = 2 * Math.round(cap)
  var device = Math.round(box * ratio)
  var hinted = device === 24 || device === 32
  return {
    box: box,
    gap: Math.round(cap / 2),
    baseline: box / 2 + cap / 2,
    file: hinted ? "a5-panel-mark-" + device + ".svg" : "a1-icon-mask.svg",
    crisp: hinted
  }
}

// A11, the state pictograms (48 and 96 grids; used at 48 px and up only,
// design round 3, F2). The status banner shows its status's pictogram;
// contractMismatch has none.
var STATUS_PICTOGRAMS = {
  engineMissing: "engine-missing",
  notInitialised: "logbook-not-initialised",
  indexMissing: "index-missing",
  indexStale: "index-stale"
}

function statusPictogram(status) {
  return STATUS_PICTOGRAMS[status] !== undefined ? STATUS_PICTOGRAMS[status] : ""
}

// The Today tab's state pictogram (ADR-0028 §4b): crisis when any crisis,
// else case active when cases are active, else all clear; tone as the
// pill's (pillTone). Attention alone changes nothing, so the drift-open
// pictogram is not shown. null without counts.
function todayState(c) {
  if (!c) return null
  if (c.crisis > 0) return { id: "crisis", tone: "urgent" }
  if (c.active > 0) return { id: "case-active", tone: "accent" }
  return { id: "all-clear", tone: "default" }
}

// The pictogram file for a state id drawn at `size` logical pixels (the
// vector scales with the DPR): the 48 grid up to 72 px, the 96 grid above.
function pictogramFile(id, size) {
  return id ? "a11-state-" + id + "-" + (Number(size) > 72 ? 96 : 48) + ".svg" : ""
}

// A12, the timeline markers: shape alone tells them apart (release diamond,
// snapshot dot, case brackets, crisis spindle). The legend shows the files
// at `size` logical pixels (the vector scales with the DPR), the 12 grid up
// to 14 px, the 16 grid above; the canvas draws the same
// shapes from the 16 grid's path data (assets/a12-marker-*-16.svg, centre
// 8, 8; model.test.js keeps the two equal). Case brackets are rectangles
// (Timeline.qml), 2 of 16 units thick, as in the files.
var MARKER_KINDS = ["release", "snapshot", "case-span-start", "case-span-end", "crisis"]
var MARKER_PATHS = {
  release: "m8 2 6 6-6 6-6-6z",
  snapshot: "M12.64 8a4.64 4.64 0 1 1-9.29 0 4.64 4.64 0 0 1 9.29 0",
  crisis: "M8 .64a11 11 0 0 0 1.12 4.04q.47.95 1.12 1.78.64.84 1.44 1.54a11 11 0 0 0-2.56 3.32A11 11 0 0 0 8 15.36a11 11 0 0 0-1.12-4.04q-.47-.95-1.12-1.78Q5.12 8.69 4.32 8a11 11 0 0 0 2.56-3.32A11 11 0 0 0 8 .64"
}

function markerFile(kind, size) {
  if (MARKER_KINDS.indexOf(kind) === -1) return ""
  return "a12-marker-" + kind + "-" + (Number(size) > 14 ? 16 : 12) + ".svg"
}

// The timeline legend, in the order of the slot's subtitle; tone names the
// colour the canvas uses for the same marker (Timeline.markerColor).
var TIMELINE_LEGEND = [
  { markers: ["release"], label: "releases", tone: "accent" },
  { markers: ["snapshot"], label: "snapshots", tone: "snapshot" },
  { markers: ["case-span-start", "case-span-end"], label: "cases", tone: "accent" },
  { markers: ["crisis"], label: "crises", tone: "urgent" }
]

function plural(n, one, many) {
  return n + " " + (n === 1 ? one : many)
}

function relativeAge(fromMs, nowMs) {
  if (!isFinite(fromMs) || !isFinite(nowMs)) return "unknown"
  var minutes = Math.floor((nowMs - fromMs) / 60000)
  if (minutes < 1) return "just now"
  if (minutes < 60) return minutes + " min ago"
  var hours = Math.floor(minutes / 60)
  if (hours < 48) return hours + " h ago"
  return Math.floor(hours / 24) + " days ago"
}

var STATUS_PHRASES = {
  ok: "",
  engineMissing: "engine not installed",
  notInitialised: "logbook not initialised",
  indexMissing: "no index yet",
  indexStale: "index is stale",
  contractMismatch: "index format does not match the plugin"
}

function statusPhrase(status) {
  return STATUS_PHRASES[status] !== undefined ? STATUS_PHRASES[status] : "unknown state"
}

// Whether the index's counts mean anything in this status. An uninitialised
// logbook reports zeros, a mismatched or missing index has none.
function showsCounts(status) {
  return status === "ok" || status === "indexStale" || status === "engineMissing"
}

// SPEC-PLUGIN §4, ADR-0028 §4a, in the neutral tone: "Seldon — 2 active
// cases, 1 crisis, 7 changes without a case, last capture 4 min ago"; the
// crisis part only while there is one, the changes without a case are the
// attention count. The problem is appended when the status is not ok. The
// same text in every `driftInBar` mode.
function tooltipText(status, c, lastCaptureText, nowMs) {
  var problem = statusPhrase(status)
  if (!c || !showsCounts(status)) return "Seldon — " + (problem !== "" ? problem : "loading")
  var parts = [plural(c.active, "active case", "active cases")]
  if (c.crisis > 0) parts.push(plural(c.crisis, "crisis", "crises"))
  parts.push(plural(Math.max(0, c.drift - c.crisis), "change", "changes") + " without a case")
  parts.push(lastCaptureText ? "last capture " + relativeAge(timeMs(lastCaptureText), nowMs) : "never captured")
  var text = "Seldon — " + parts.join(", ")
  return problem !== "" ? text + " · " + problem : text
}

// ---- Banners ----------------------------------------------------------------

// One banner per non-ok status, each with its one-click fix (AGENTS.md §7).
// Action ids are dispatched by Service.fix(): copy, terminal, recheck,
// build, capture. `command` is always one of the constants above.
// ctx: { indexContractVersion, parseError, generatedAt, nowMs }
function bannerFor(status, ctx) {
  ctx = ctx || {}
  if (status === "engineMissing") {
    return {
      status: status,
      tone: "urgent",
      title: "Seldon engine not installed",
      detail: ENGINE_MISSING_DETAIL,
      command: INSTALL_ENGINE_COMMAND,
      actions: [
        { id: "terminal", label: "Install in terminal" },
        { id: "copy", label: "Copy" },
        { id: "recheck", label: "Check again" }
      ]
    }
  }
  if (status === "notInitialised") {
    return {
      status: status,
      tone: "accent",
      title: "Logbook not initialised",
      detail: "Create your logbook once with seldon init.",
      command: INIT_COMMAND,
      actions: [
        { id: "terminal", label: "Run in terminal" },
        { id: "copy", label: "Copy" },
        { id: "recheck", label: "Check again" }
      ]
    }
  }
  if (status === "indexMissing") {
    var unreadable = ctx.parseError && ctx.parseError !== "empty"
    return {
      status: status,
      tone: "accent",
      title: unreadable ? "Index unreadable" : "No index yet",
      detail: unreadable
        ? "index.json could not be read. Rebuilding it from the logbook fixes this."
        : "The engine has not written index.json yet.",
      command: "",
      actions: [{ id: "build", label: "Build index" }]
    }
  }
  if (status === "indexStale") {
    return {
      status: status,
      tone: "accent",
      title: "Index is stale",
      detail: "Last update " + relativeAge(timeMs(ctx.generatedAt), ctx.nowMs) + ". Capture to refresh it.",
      command: "",
      actions: [{ id: "capture", label: "Capture now" }]
    }
  }
  if (status === "contractMismatch") {
    var found = Number(ctx.indexContractVersion) || 0
    var pluginOlder = found > CONTRACT_VERSION
    return {
      status: status,
      tone: "urgent",
      title: "Index format mismatch",
      detail: "The index uses contract v" + found + ", this plugin reads v" + CONTRACT_VERSION
        + ". Update the " + (pluginOlder ? "plugin" : "engine") + ".",
      command: pluginOlder ? UPDATE_PLUGIN_COMMAND : UPDATE_ENGINE_COMMAND,
      actions: [
        { id: "terminal", label: "Update in terminal" },
        { id: "copy", label: "Copy" }
      ]
    }
  }
  return null
}

// ---- Engine calls -----------------------------------------------------------

function engineError(stdoutText, stderrText, exitCode) {
  try {
    var data = JSON.parse(String(stdoutText || ""))
    if (isObject(data) && isObject(data.error) && typeof data.error.message === "string")
      return data.error.message
  } catch (e) {
  }
  var err = String(stderrText || "").trim().split("\n")[0]
  return err !== "" ? err : "seldon exited with code " + exitCode
}

function engineVersion(stdoutText) {
  try {
    var data = JSON.parse(String(stdoutText || ""))
    if (isObject(data) && typeof data.version === "string") return data.version
  } catch (e) {
  }
  return ""
}

function matches(list, value) {
  return list.indexOf(value) !== -1
}

// Free text must say something: one argument, not empty, not only blanks.
function hasText(value) {
  return typeof value === "string" && value.trim() !== ""
}

// Validate an argument list against the commands of CONTRACT.md ("Commands
// the plugin may run"). Returns "" when allowed, else the reason.
//
// Free text (a note, a title, an explanation) is exactly one non-empty
// argument after a `--` separator, so the engine never reads it as an option.
// Before the separator every command may end in --json (`drift show`
// requires it); ids must match their schema pattern.
function validateArgs(args) {
  if (!Array.isArray(args) || args.length === 0) return "empty argument list"
  for (var i = 0; i < args.length; i++) {
    if (typeof args[i] !== "string") return "argument " + i + " is not a string"
    if (args[i].indexOf("\u0000") !== -1) return "argument " + i + " contains NUL"
  }
  var sep = args.indexOf("--")
  var a = sep === -1 ? args.slice() : args.slice(0, sep)
  var free = sep === -1 ? null : args.slice(sep + 1)
  if (free !== null && (free.length !== 1 || !hasText(free[0])))
    return "free text must be one non-empty argument after --"
  var json = a.length > 1 && a[a.length - 1] === "--json"
  if (json) a.pop()
  var n = a.length
  var withText = free !== null
  var only = function(i) { return n === i + 1 && a[i] === "--only" }
  switch (a[0]) {
  case "--version":
  case "status":
  case "rebuild":
  case "update-impact":
    return n === 1 && !withText ? "" : a[0] + " takes no arguments"
  case "capture":
    // CONTRACT.md writes it `capture --all --json --quiet`.
    if (withText) return "capture takes no free text"
    if (n === 4 && a[1] === "--all" && a[2] === "--json" && a[3] === "--quiet") return ""
    return n === 3 && a[1] === "--all" && a[2] === "--quiet" ? "" : "capture must be: capture --all --json --quiet"
  case "log":
    if (withText && n === 1) return ""
    if (withText && n === 3 && a[1] === "--case" && CASE_ID.test(a[2])) return ""
    return "log must be: log [--case <caseId>] -- <text>"
  case "plan":
    if (withText && n >= 6 && a[1] === "new" && a[2] === "--zone" && matches(ZONES, a[3])
        && a[4] === "--risk" && matches(RISKS, a[5])) {
      // Then optionally `--area <slug>`, then optionally `--priority <p>`.
      var k = 6
      if (k + 1 < n && a[k] === "--area" && AREA.test(a[k + 1])) k += 2
      if (k + 1 < n && a[k] === "--priority" && matches(PRIORITIES, a[k + 1])) k += 2
      if (k === n) return ""
    }
    if (!withText && n === 3 && matches(PLAN_STEPS, a[1]) && CASE_ID.test(a[2])) return ""
    // WP-101: reopen a completed case (a new case; nothing is destroyed)
    if (!withText && n === 3 && a[1] === "reopen" && CASE_ID.test(a[2]) && json) return ""
    return "plan must be: plan new --zone <z> --risk <r> [--area <a>] [--priority <p>] -- <title>"
      + " | plan start|verify|done|drop <caseId> | plan reopen <caseId> --json"
  case "drift":
    var id = n >= 3 && EVENT_ID.test(a[2])
    if (id && !withText && a[1] === "link" && n >= 4 && CASE_ID.test(a[3]) && (n === 4 || only(4))) return ""
    // dismiss takes its reason like explain its text, after `--` (WP-011
    // review: the contract form replacing `--reason <text>`).
    if (id && withText && a[1] === "dismiss" && (n === 3 || only(3))) return ""
    if (id && withText && a[1] === "explain") {
      // Then optionally --only, --zone <z>, --risk <r>, --area <slug>, in that
      // order (SPEC-ENGINE §3, WP-021).
      var j = 3
      if (j < n && a[j] === "--only") j += 1
      if (j + 1 < n && a[j] === "--zone" && matches(ZONES, a[j + 1])) j += 2
      if (j + 1 < n && a[j] === "--risk" && matches(RISKS, a[j + 1])) j += 2
      if (j + 1 < n && a[j] === "--area" && AREA.test(a[j + 1])) j += 2
      if (j === n) return ""
    }
    if (id && !withText && a[1] === "show" && n === 3 && json) return ""
    return "drift must be: drift link <eventId> <caseId> [--only] | explain <eventId> [--only] [--zone <z>]"
      + " [--risk <r>] [--area <a>] -- <text> | dismiss <eventId> [--only] -- <text> | show <eventId> --json"
  case "agent":
    // WP-022: the engine reads the launcher from its config; nothing else.
    if (!withText && n === 3 && a[1] === "start" && CASE_ID.test(a[2]) && json) return ""
    // WP-101: one sentence creates, starts and hands a case to the agent
    if (withText && n === 3 && a[1] === "start" && a[2] === "--new" && json) return ""
    return "agent must be: agent start <caseId> --json | agent start --new --json -- <intent>"
  case "doctor":
    // WP-101: read-only, the rules row only (Service.checkRules); no probe
    return !withText && n === 3 && a[1] === "--only" && a[2] === "rules" && json
      ? "" : "doctor must be: doctor --only rules --json"
  case "rules":
    return !withText && n === 2 && a[1] === "update" && json ? "" : "rules must be: rules update --json"
  case "decide":
    return withText && n === 2 && a[1] === "--no-edit" ? "" : "decide must be: decide --no-edit -- <title>"
  case "open":
    return !withText && n === 3 && (matches(OPEN_TARGETS, a[1]) || CASE_ID.test(a[1]) || DECISION_ID.test(a[1]))
      && a[2] === "--editor" ? "" : "open must be: open journal|ledger|status|logbook|<caseId>|<ADR id> --editor"
  default:
    return "command not allowed: " + a[0]
  }
}

function clampInterval(minutes) {
  var n = Math.round(Number(minutes))
  if (!isFinite(n)) return CAPTURE_INTERVAL_MIN_DEFAULT
  return Math.max(CAPTURE_INTERVAL_MIN_MIN, Math.min(CAPTURE_INTERVAL_MIN_MAX, n))
}

// SELDON_INDEX may be relative to the shell's working directory.
function resolvePath(path, workingDirectory) {
  var p = String(path || "")
  if (p === "" || p.charAt(0) === "/") return p
  var base = String(workingDirectory || "")
  if (base === "") return p
  return base.replace(/\/+$/, "") + "/" + p
}

// CONTRACT.md rule 1: ${XDG_STATE_HOME:-$HOME/.local/state}/seldon/index.json.
// The XDG spec says a relative XDG_STATE_HOME is invalid and is ignored.
function stateIndexPath(xdgStateHome, home) {
  var base = String(xdgStateHome || "")
  if (base.charAt(0) !== "/") base = String(home || "") + "/.local/state"
  return base.replace(/\/+$/, "") + "/seldon/index.json"
}

// ---- Panel actions: QuickEntry, Capture now, Open in editor ------------------

// Open cases for the QuickEntry case picker, active first, then verification,
// then queued. A case whose id does not match the schema pattern is left out:
// the id goes into an argument list.
function openCases(index) {
  var out = []
  var cases = index && isObject(index.cases) ? index.cases : {}
  var groups = ["active", "verification", "queued"]
  for (var g = 0; g < groups.length; g++) {
    var list = cases[groups[g]]
    if (!Array.isArray(list)) continue
    for (var i = 0; i < list.length; i++) {
      var c = list[i]
      if (!isObject(c) || typeof c.id !== "string" || !CASE_ID.test(c.id)) continue
      out.push({ id: c.id, title: str(c.title), status: groups[g] })
    }
  }
  return out
}

// The picker's options: "No case" first, then one per open case.
function caseOptions(index) {
  var cases = openCases(index)
  var out = [{ value: "", label: "No case" }]
  for (var i = 0; i < cases.length; i++)
    out.push({ value: cases[i].id, label: cases[i].id + (cases[i].title !== "" ? " · " + cases[i].title : "") })
  return out
}

// `seldon log [--case <id>] --json -- <text>` (CONTRACT.md): the text is one
// argument after `--`, exactly as typed. Returns { args } or { error }; blank
// text is refused here, before the engine is asked.
function logArgs(text, caseId) {
  if (!hasText(text)) return { error: "Write something first" }
  if (String(text).indexOf("\u0000") !== -1) return { error: "The note contains a NUL character" }
  var id = String(caseId || "")
  if (id !== "" && !CASE_ID.test(id)) return { error: "Not a case id: " + id }
  var args = id !== "" ? ["log", "--case", id, "--json", "--", text] : ["log", "--json", "--", text]
  return { args: args }
}

// `seldon open <what> --editor --json`: journal (today), ledger (this month),
// status (STATUS.md), logbook (the folder), a case id or a decision id.
// Anything else is refused (null).
function openArgs(what) {
  var w = String(what || "")
  if (!matches(OPEN_TARGETS, w) && !CASE_ID.test(w) && !DECISION_ID.test(w)) return null
  return ["open", w, "--editor", "--json"]
}

function parseJson(text) {
  try {
    var data = JSON.parse(String(text || ""))
    return isObject(data) ? data : null
  } catch (e) {
    return null
  }
}

// Result lines: { ok, text }. `text` is plain text (CONTRACT.md rule 6).

// `seldon log --json` → {"event": <ledger line>, "git": …} (SPEC-ENGINE §3).
function logResult(exitCode, stdoutText, stderrText) {
  if (exitCode !== 0) return { ok: false, text: engineError(stdoutText, stderrText, exitCode) }
  var data = parseJson(stdoutText)
  var e = data && isObject(data.event) ? data.event : null
  var where = e && typeof e.case === "string" && CASE_ID.test(e.case) ? e.case : "the journal"
  var id = e && typeof e.id === "string" && EVENT_ID.test(e.id) ? " · " + e.id : ""
  return { ok: true, text: "Saved to " + where + id }
}

// `seldon open --json` → {"what", "path", "editor": {"launched", "program"}}.
function openResult(exitCode, stdoutText, stderrText) {
  if (exitCode !== 0) return { ok: false, text: engineError(stdoutText, stderrText, exitCode) }
  var data = parseJson(stdoutText)
  var path = data && typeof data.path === "string" ? data.path : ""
  var editor = data && isObject(data.editor) ? data.editor : null
  if (editor && editor.launched === false)
    return { ok: false, text: typeof editor.error === "string" ? editor.error : "The editor did not start" }
  var program = editor && typeof editor.program === "string" ? editor.program : "the editor"
  return { ok: true, path: path, text: path !== "" ? "Opened " + path + " in " + program : "Opened in " + program }
}

// `seldon capture --json` → {"written": N, "collectors": [{"name", "ok", …}],
// "warnings": ["…"]}. `warnings` keeps the engine's warnings as it wrote
// them (WP-085; captureWarnings); [] for a failed capture.
function captureResult(exitCode, stdoutText, stderrText) {
  if (exitCode !== 0) return { ok: false, text: engineError(stdoutText, stderrText, exitCode), warnings: [] }
  var data = parseJson(stdoutText)
  var written = data ? count(data.written) : 0
  var failing = []
  var list = data && Array.isArray(data.collectors) ? data.collectors : []
  for (var i = 0; i < list.length; i++)
    if (isObject(list[i]) && list[i].ok === false && typeof list[i].name === "string") failing.push(list[i].name)
  var text = written === 0 ? "nothing new" : plural(written, "new event", "new events")
  if (failing.length > 0) text += " · failing: " + failing.join(", ")
  return { ok: true, text: text, warnings: captureWarnings(data) }
}

// ---- Panel: strips and banners under the status banner ----------------------

// SPEC-PLUGIN §5, ADR-0028 §4b: the red strip "N changes that can affect
// boot, login or the shell have no case". Empty without a crisis.
function crisisText(index) {
  var c = counts(index)
  if (!c || c.crisis === 0) return ""
  return plural(c.crisis, "change", "changes") + " that can affect boot, login or the shell "
    + (c.crisis === 1 ? "has" : "have") + " no case"
}

// ADR-0028 §4b: the Changelog header's quiet line "N changes without a
// case" (the attention count, as in the tooltip). Empty at 0. No badge,
// no colour.
function attentionText(index) {
  var c = counts(index)
  return c && c.attention > 0 ? plural(c.attention, "change", "changes") + " without a case" : ""
}

function collectors(index) {
  return index && isObject(index.state) && Array.isArray(index.state.collectors) ? index.state.collectors : []
}

// The snapper banner's hint after *Run in terminal* (WP-054, issue #2).
var SNAPPER_HINT = "When the command has finished, press Check again"

// ADR-0026: snapper runs degraded until the user grants read access. The
// banner shows the engine's message as plain text, then what the fix grants
// (SNAPPER_FIX_GRANTS), and offers the constant fix. Action ids are
// dispatched by Service.fix(actionId, "snapper"). *Check again* is a capture
// (the same call as *Capture now*): only a capture rewrites the collector
// state this banner reads; reloading the index would not (WP-054).
// `hinted`: Run in terminal was clicked and the index has not changed since;
// the banner then carries SNAPPER_HINT in `hint`.
function snapperBanner(index, hinted) {
  var list = collectors(index)
  for (var i = 0; i < list.length; i++) {
    var c = list[i]
    if (!isObject(c) || c.name !== "snapper" || c.enabled !== true || c.ok !== false) continue
    return {
      status: "snapperDegraded",
      tone: "accent",
      title: "Snapshots not readable",
      detail: (typeof c.message === "string" && c.message !== ""
        ? c.message
        : "The snapper collector has no permission to list snapshots.") + "\n" + SNAPPER_FIX_GRANTS,
      command: SNAPPER_FIX_COMMAND,
      actions: [
        { id: "terminal", label: "Run in terminal" },
        { id: "copy", label: "Copy" },
        { id: "capture", label: "Check again" }
      ],
      hint: hinted === true ? SNAPPER_HINT : ""
    }
  }
  return null
}

// ---- Panel: tab keys ----------------------------------------------------------

// SPEC-PLUGIN §5: digits 1–6 select tabs by a fixed number, so a key keeps
// its tab when later versions add tabs. A digit whose tab is absent does
// nothing.
var TAB_KEYS = { "1": "today", "2": "changelog", "3": "work", "4": "decisions", "5": "system", "6": "memory" }

function tabKeyFor(id) {
  for (var key in TAB_KEYS)
    if (TAB_KEYS[key] === id) return key
  return ""
}

// ---- Shared formatting ------------------------------------------------------

var WEEKDAYS = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"]
var MONTHS = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"]

function str(value) {
  return typeof value === "string" ? value : ""
}

// Index timestamps keep the offset of their source (event.schema.json), so
// the wall-clock time is read from the string itself, not converted.
function clockTime(ts) {
  var m = /^\d{4}-\d{2}-\d{2}T(\d{2}:\d{2})/.exec(str(ts))
  return m ? m[1] : ""
}

function dayOf(ts) {
  var m = /^(\d{4}-\d{2}-\d{2})/.exec(str(ts))
  return m ? m[1] : ""
}

function utcDate(date) {
  var d = new Date(Date.parse(date + "T00:00:00Z"))
  return isFinite(d.getTime()) ? d : null
}

// YYYY-MM-DD shifted by `delta` days, calendar arithmetic only.
function addDays(date, delta) {
  var d = utcDate(date)
  return d ? new Date(d.getTime() + delta * 86400000).toISOString().slice(0, 10) : ""
}

// "Today", "Yesterday", else "Tue 30 Sep" (with the year when it differs).
function dayLabel(date, today) {
  if (date === "") return "Undated"
  if (date === today) return "Today"
  if (today && date === addDays(today, -1)) return "Yesterday"
  var d = utcDate(date)
  if (!d) return date
  var label = WEEKDAYS[d.getUTCDay()].slice(0, 3) + " " + d.getUTCDate() + " " + MONTHS[d.getUTCMonth()]
  return String(today || "").slice(0, 4) === date.slice(0, 4) ? label : label + " " + date.slice(0, 4)
}

// `human`, `system`, or the agent's name for `agent:<name>`.
function actorLabel(actor) {
  var a = str(actor)
  return a.indexOf("agent:") === 0 ? a.slice(6) : a
}

// "Today" is the index's (ADR-0012 §10), not the clock's.
function todayDate(index) {
  if (index && isObject(index.today) && typeof index.today.date === "string") return index.today.date
  return index ? dayOf(index.generatedAt) : ""
}

// ---- Changelog --------------------------------------------------------------

// Every event source (event.schema.json) in filter order, with its glyph.
// Glyphs are Nerd Font codepoints, the icon set the bar's font ships.
var SOURCES = ["pacman", "snapper", "omarchy", "plugins", "theme", "config", "agent", "manual", "seldon"]
var SOURCE_GLYPHS = {
  pacman: "\u{F03D7}",   // package
  snapper: "\u{F0100}",  // camera
  omarchy: "\u{F06B0}",  // update
  plugins: "\u{F0431}",  // puzzle
  theme: "\u{F03D8}",    // palette
  config: "\u{F0493}",   // cog
  agent: "\u{F06A9}",    // robot
  manual: "\u{F03EB}",   // pencil
  seldon: "\u27E1"     // ⟡, the text glyph of the engine's own events
}

function sourceGlyph(source) {
  return SOURCE_GLYPHS[source] !== undefined ? SOURCE_GLYPHS[source] : "•"
}

// The tone of an open drift item (ADR-0028 §4b): the urgent colour is the
// crisis signal and nothing else, so a crisis is urgent whatever its zone
// (a hook in the yellow zone) and attention is the accent whatever its zone
// (a pacman install is red in the ledger, ADR-0014, and still quiet).
function driftTone(item) {
  return isObject(item) && item.crisis === true ? "urgent" : "accent"
}

// Zone colours come from theme tokens only (SPEC-PLUGIN §7): red is the
// theme's urgent colour, yellow its accent, green the muted colour.
function zoneTone(zone) {
  if (zone === "red") return "urgent"
  if (zone === "yellow") return "accent"
  if (zone === "green") return "muted"
  return ""
}

function events(index) {
  return index && Array.isArray(index.events) ? index.events : []
}

// Open drift by event: every item under its eventId, and a pacman group
// (ADR-0013; ADR-0015 §2: `members` present) also under its txId, so the
// group's other members find it.
function driftLookup(index) {
  var byId = {}
  var byTx = {}
  var list = index && Array.isArray(index.drift) ? index.drift : []
  for (var i = 0; i < list.length; i++) {
    var d = list[i]
    if (!isObject(d) || typeof d.eventId !== "string") continue
    byId[d.eventId] = d
    if (typeof d.members === "number" && typeof d.txId === "string") byTx[d.txId] = d
  }
  return { byId: byId, byTx: byTx }
}

// An open group member is caseless and unresolved (ADR-0013 §1).
function isOpenMember(e, txId) {
  return isObject(e) && e.txId === txId && !e.case && !e.resolution
}

// One row per index event, newest first as the index lists them (the index
// already folds resolutions onto their targets). filter: "" or "all" for
// every source, else one source name.
function changelogRows(index, filter) {
  var all = events(index)
  var drift = driftLookup(index)
  var today = todayDate(index)
  var only = filter && filter !== "all" ? filter : ""
  var rows = []
  for (var i = 0; i < all.length; i++) {
    var e = all[i]
    if (!isObject(e) || (only !== "" && e.source !== only)) continue
    var leader = drift.byId[e.id] || null
    var group = !leader && typeof e.txId === "string" && isOpenMember(e, e.txId) ? drift.byTx[e.txId] || null : null
    var item = leader || group
    var grouped = leader !== null && typeof leader.members === "number"
    // One colour source per row: open drift by its item's class (driftTone:
    // crisis urgent, attention accent). Every other row is an ordinary
    // Changelog row (ADR-0028 §4b), quiet whatever its zone: the muted
    // stripe when it has a zone, none when it has not.
    var tone = item !== null ? driftTone(item) : ZONES.indexOf(e.zone) !== -1 ? "muted" : ""
    var day = dayOf(e.ts)
    rows.push({
      id: str(e.id),
      day: day,
      dayLabel: dayLabel(day, today),
      time: clockTime(e.ts),
      source: str(e.source),
      glyph: sourceGlyph(e.source),
      kind: str(e.kind),
      subject: str(e.subject),
      detail: str(e.detail),
      actor: actorLabel(e.actor),
      caseId: str(e.case),
      zone: str(e.zone),
      tone: tone,
      resolution: str(e.resolution),
      resolutionDetail: str(e.resolutionDetail),
      snapshot: e.source === "snapper" && e.kind === "snapshot",
      drift: item !== null,
      crisis: item !== null && item.crisis === true,
      proposedCase: leader && typeof leader.proposedCase === "string" ? leader.proposedCase : "",
      // "+N" on the group's leader row: the leader plus N more. `members`
      // counts the leader too (ADR-0013 §2), so a group of 3 reads "+2".
      badge: grouped && leader.members > 1 ? "+" + (leader.members - 1) : "",
      txId: grouped ? str(leader.txId) : "",
      groupLeader: group ? str(group.eventId) : "",
      groupSubject: group ? str(group.subject) : ""
    })
  }
  return rows
}

// Event counts per source, for the filter chips.
function sourceCounts(index) {
  var result = { all: 0 }
  for (var s = 0; s < SOURCES.length; s++) result[SOURCES[s]] = 0
  var all = events(index)
  for (var i = 0; i < all.length; i++) {
    if (!isObject(all[i])) continue
    result.all++
    if (result[all[i].source] !== undefined) result[all[i].source]++
  }
  return result
}

// Chips: "all" first, then every source in SOURCES order.
function filterChips(index) {
  var c = sourceCounts(index)
  var chips = [{ id: "all", label: "all", count: c.all }]
  for (var i = 0; i < SOURCES.length; i++)
    chips.push({ id: SOURCES[i], label: SOURCES[i], count: c[SOURCES[i]] })
  return chips
}

// Next or previous filter id, wrapping (keys `f` / `F`).
function cycleFilter(current, direction) {
  var ids = ["all"].concat(SOURCES)
  var at = ids.indexOf(current || "all")
  if (at === -1) at = 0
  return ids[(at + (direction < 0 ? ids.length - 1 : 1)) % ids.length]
}

// The members of a pacman group as far as index.events still lists them
// (CONTRACT.md rule 4 caps it; `seldon drift show <id> --json` has the rest).
function groupMembers(index, txId) {
  var out = []
  if (!txId) return out
  var all = events(index)
  for (var i = 0; i < all.length; i++) {
    var e = all[i]
    if (isOpenMember(e, txId))
      out.push({ id: str(e.id), kind: str(e.kind), subject: str(e.subject), detail: str(e.detail) })
  }
  return out
}

function memberLine(member) {
  return member.kind + " " + member.subject + (member.detail !== "" ? "  " + member.detail : "")
}

// The second line of a row: what changed, who, for which case.
function rowMeta(r) {
  var parts = []
  if (r.detail !== "") parts.push(r.detail)
  if (r.actor !== "") parts.push(r.actor)
  if (r.caseId !== "") parts.push(r.caseId)
  return parts.join(" · ")
}

// "linked to C-…", "explained · C-…" (the retroactive case `drift explain`
// created, ADR-0021), or the resolution alone.
function resolutionHead(resolution, caseId) {
  if (resolution === "linked" && caseId !== "") return "linked to " + caseId
  if (resolution === "explained" && caseId !== "") return "explained · " + caseId
  return resolution
}

// The folded resolution (ADR-0012 §8, §11), or the open-drift note: it
// states, it never asks (ADR-0028 §3: a reason is never required).
function rowStatus(r) {
  if (r.resolution !== "") {
    var head = resolutionHead(r.resolution, r.caseId)
    return r.resolutionDetail !== "" ? head + ": " + r.resolutionDetail : head
  }
  if (!r.drift) return ""
  var note = r.groupLeader !== "" ? "In the open " + r.groupSubject + " group"
    : r.crisis ? "Crisis · no case" : "No case"
  if (r.proposedCase !== "") note += " · proposed for " + r.proposedCase
  return note
}

// ---- Today ------------------------------------------------------------------

function journalEntries(list) {
  var out = []
  if (!Array.isArray(list)) return out
  for (var i = 0; i < list.length; i++) {
    var e = list[i]
    if (!isObject(e)) continue
    out.push({ time: str(e.time), actor: actorLabel(e.actor), caseId: str(e.case), text: str(e.text) })
  }
  return out
}

// Today's journal, yesterday's (shown collapsed) and the summary counts.
// "without a case" is the attention count (openDrift − crisis), the number
// the tooltip and the Changelog line show; crises are in the strip and the
// pictogram (ADR-0028 §4b).
function todayView(index) {
  var today = index && isObject(index.today) ? index.today : {}
  var summary = index && isObject(index.summary) ? index.summary : {}
  var date = todayDate(index)
  var d = utcDate(date)
  return {
    date: date,
    title: d ? WEEKDAYS[d.getUTCDay()] + ", " + d.getUTCDate() + " " + MONTHS[d.getUTCMonth()] + " " + d.getUTCFullYear() : "Today",
    entries: journalEntries(today.entries),
    yesterday: journalEntries(today.yesterday),
    stats: [
      { label: "events today", value: count(summary.eventsToday) },
      { label: "in 7 days", value: count(summary.events7d) },
      { label: "active", value: count(summary.activeCases) },
      { label: "queued", value: count(summary.queuedCases) },
      { label: "without a case", value: Math.max(0, count(summary.openDrift) - count(summary.crisis)) }
    ]
  }
}

function entryMeta(entry) {
  var parts = []
  if (entry.time !== "") parts.push(entry.time)
  if (entry.actor !== "") parts.push(entry.actor)
  if (entry.caseId !== "") parts.push(entry.caseId)
  return parts.join(" · ")
}

// ---- System -----------------------------------------------------------------

function pair(label, value) {
  return { label: label, value: value }
}

function isInt(value) {
  return typeof value === "number" && isFinite(value)
}

function stamp(ts) {
  return (dayOf(ts) + " " + clockTime(ts)).trim()
}

// The System tab as sections of label/value rows. Every field of
// index.system is optional (the schema requires none there), so a section
// appears only when it has a row.
function systemSections(index, nowMs) {
  var sys = index && isObject(index.system) ? index.system : {}
  var sections = []
  var rows

  rows = []
  var om = isObject(sys.omarchy) ? sys.omarchy : {}
  if (str(om.version) !== "") rows.push(pair("Version", om.version))
  if (str(om.theme) !== "") rows.push(pair("Theme", om.theme))
  if (str(om.lastUpdate) !== "")
    rows.push(pair("Last update", stamp(om.lastUpdate) + (isFinite(nowMs) ? " · " + relativeAge(timeMs(om.lastUpdate), nowMs) : "")))
  if (str(om.repoHead) !== "") rows.push(pair("Checkout", om.repoHead))
  if (rows.length) sections.push({ title: "OMARCHY", rows: rows })

  rows = []
  var pk = isObject(sys.packages) ? sys.packages : {}
  if (isInt(pk.explicit)) rows.push(pair("Explicit", String(pk.explicit)))
  if (isInt(pk.total)) rows.push(pair("Installed", String(pk.total)))
  if (isInt(pk.aur)) rows.push(pair("AUR", String(pk.aur)))
  if (isInt(sys.deviations)) rows.push(pair("Deviations", String(sys.deviations)))
  if (rows.length) sections.push({ title: "PACKAGES", rows: rows })

  rows = []
  var pl = isObject(sys.plugins) ? sys.plugins : {}
  if (isInt(pl.enabled) && isInt(pl.installed)) rows.push(pair("Plugins", pl.enabled + " of " + pl.installed + " enabled"))
  else if (isInt(pl.installed)) rows.push(pair("Plugins", pl.installed + " installed"))
  else if (isInt(pl.enabled)) rows.push(pair("Plugins", pl.enabled + " enabled"))
  if (rows.length) sections.push({ title: "PLUGINS", rows: rows })

  rows = []
  var snaps = Array.isArray(sys.snapshots) ? sys.snapshots : []
  for (var i = 0; i < snaps.length; i++) {
    var s = snaps[i]
    if (!isObject(s) || !isInt(s.number)) continue
    // The time goes with the value: the label column is too narrow for it.
    var snapParts = [stamp(s.ts), str(s.description)]
    if (str(s.type) !== "" && s.type !== "single") snapParts.push(s.type)
    rows.push(pair("#" + s.number, snapParts.filter(function(p) { return p !== "" }).join(" · ")))
  }
  if (rows.length) sections.push({ title: "SNAPSHOTS", rows: rows })

  rows = []
  var areas = Array.isArray(sys.areas) ? sys.areas : []
  for (var j = 0; j < areas.length; j++) {
    var ar = areas[j]
    if (!isObject(ar) || str(ar.name) === "") continue
    var parts = []
    if (isInt(ar.cases)) parts.push(plural(ar.cases, "case", "cases"))
    if (ar.hasAgentsMd === true) parts.push("AGENTS.md")
    rows.push(pair(ar.name, parts.join(" · ")))
  }
  if (rows.length) sections.push({ title: "AREAS", rows: rows })

  rows = []
  var cs = collectors(index)
  for (var k = 0; k < cs.length; k++) {
    var c = cs[k]
    if (!isObject(c) || str(c.name) === "") continue
    var state = c.enabled === false ? "off" : c.ok === false ? "failing" : "ok"
    rows.push(pair(c.name, state + (c.ok === false && str(c.message) !== "" ? " · " + c.message : "")))
  }
  if (rows.length) sections.push({ title: "COLLECTORS", rows: rows })

  rows = []
  if (index && isObject(index.logbook) && str(index.logbook.machine) !== "") rows.push(pair("Machine", index.logbook.machine))
  if (index && str(index.engineVersion) !== "") rows.push(pair("Engine", index.engineVersion))
  if (index && str(index.generatedAt) !== "") rows.push(pair("Index written", stamp(index.generatedAt)))
  if (rows.length) sections.push({ title: "SELDON", rows: rows })

  return sections
}

// ---- Work ---------------------------------------------------------------------

// SPEC-PLUGIN §5, Work: three columns. Active holds the verification cases
// too, after the active ones; Completed holds completed and dropped cases,
// newest closed first, as the index lists them (at most 50, CONTRACT.md
// rule 4).
var WORK_COLUMNS = [
  { id: "queued", title: "Queued", groups: ["queued"] },
  { id: "active", title: "Active", groups: ["active", "verification"] },
  { id: "completed", title: "Completed", groups: ["completed"] }
]

var CASE_STATUSES = ["queued", "active", "verification", "completed", "dropped"]

// `seldon plan new` defaults (SPEC-ENGINE §3): the sheet starts with them.
var NEW_CASE_DEFAULTS = { zone: "yellow", risk: "R1", priority: "normal" }

// The WIP limit the Work tab measures active cases against. Neither the
// contract nor the logbook config has one yet; the bar widget setting
// `wipLimit` overrides this default.
var WIP_LIMIT_DEFAULT = 3
var WIP_LIMIT_MIN = 1
var WIP_LIMIT_MAX = 20

function clampWipLimit(value) {
  var n = Math.round(Number(value))
  if (!isFinite(n)) return WIP_LIMIT_DEFAULT
  return Math.max(WIP_LIMIT_MIN, Math.min(WIP_LIMIT_MAX, n))
}

// One case as the Work tab shows it. `actionable` is false for an id that
// does not match the schema pattern: such a case is shown, never passed on.
function workCase(c, group, column) {
  var steps = isObject(c.steps) ? c.steps : null
  var total = steps ? count(steps.total) : 0
  var id = str(c.id)
  return {
    id: id,
    title: str(c.title),
    status: matches(CASE_STATUSES, c.status) ? c.status : group,
    column: column,
    zone: str(c.zone),
    risk: str(c.risk),
    priority: str(c.priority),
    area: str(c.area),
    created: str(c.created),
    started: str(c.started),
    closed: str(c.closed),
    path: str(c.path),
    tone: zoneTone(c.zone),
    stepsText: steps ? Math.min(count(steps.done), total) + "/" + total : "",
    proposed: Array.isArray(c.proposedEvents) ? c.proposedEvents.length : 0,
    agents: Array.isArray(c.agents) ? c.agents.filter(function(a) { return typeof a === "string" && a !== "" }) : [],
    closedByAgent: caseTags(c).indexOf(TAG_CLOSED_BY_AGENT) !== -1,
    reopens: reopensOf(c),
    actionable: CASE_ID.test(id)
  }
}

// The reserved case tags (CONTRACT.md, ADR-0027 §5): an agent closed the
// case; the case reopens another (`reopens:<caseId>`).
var TAG_CLOSED_BY_AGENT = "closed-by-agent"
var TAG_REOPENS = "reopens:"

// A case's tags, strings only.
function caseTags(c) {
  return isObject(c) && Array.isArray(c.tags) ? c.tags.filter(function(t) { return typeof t === "string" }) : []
}

// The case id a `reopens:<caseId>` tag names, "" without one or with an id
// that is not a case id.
function reopensOf(c) {
  var tags = caseTags(c)
  for (var i = 0; i < tags.length; i++) {
    if (tags[i].indexOf(TAG_REOPENS) !== 0) continue
    var id = tags[i].slice(TAG_REOPENS.length)
    if (CASE_ID.test(id)) return id
  }
  return ""
}

// [{ id, title, cases, total }] for queued, active (+ verification),
// completed. `filter` "agent" keeps only the completed cases an agent closed
// (the spot check, ADR-0027 §5); `total` is the column's count without it.
function workColumns(index, filter) {
  var cases = index && isObject(index.cases) ? index.cases : {}
  var out = []
  for (var k = 0; k < WORK_COLUMNS.length; k++) {
    var col = WORK_COLUMNS[k]
    var list = []
    for (var g = 0; g < col.groups.length; g++) {
      var group = cases[col.groups[g]]
      if (!Array.isArray(group)) continue
      for (var i = 0; i < group.length; i++)
        if (isObject(group[i])) list.push(workCase(group[i], col.groups[g], col.id))
    }
    var shown = col.id === "completed" && filter === COMPLETED_FILTER_AGENT
      ? list.filter(function(c) { return c.closedByAgent }) : list
    out.push({ id: col.id, title: col.title, cases: shown, total: list.length })
  }
  return out
}

// The Completed column's filter that keeps the cases an agent closed.
var COMPLETED_FILTER_AGENT = "agent"

// "COMPLETED 2", or "COMPLETED 1 / 2" while the filter hides some.
function columnHeader(col) {
  var head = String(col.title).toUpperCase() + " " + col.cases.length
  return col.total !== undefined && col.total !== col.cases.length ? head + " / " + col.total : head
}

// Every case of the columns in reading order, the order the cursor walks.
function workCases(columns) {
  var out = []
  for (var k = 0; k < columns.length; k++) out = out.concat(columns[k].cases)
  return out
}

// "2 / 3 active": cases with status active (not verification) against the
// limit. tone: "" below it, "accent" at it, "urgent" over it.
function wipStatus(index, limit) {
  var cases = index && isObject(index.cases) ? index.cases : {}
  var active = Array.isArray(cases.active) ? cases.active.length : 0
  var max = clampWipLimit(limit)
  return {
    active: active,
    limit: max,
    text: active + " / " + max + " active",
    tone: active > max ? "urgent" : active === max ? "accent" : ""
  }
}

// The actions of a case card (SPEC-PLUGIN §5, WP-020): queued → Start;
// active → Verify, Drop; verification → Done, Drop; completed and dropped →
// Open. Every open case also gets Open, last. The first action is the one
// Enter runs. `write`: changes the logbook; `confirm`: a click only arms it,
// a second click runs it (Drop is final).
// `twice`: armed by the first press or click, run by the second (Start
// agent, WP-022); `confirm` also asks twice and warns that it is final.
var ACTIONS = {
  start: { id: "start", label: "Start", write: true, confirm: false, twice: false },
  verify: { id: "verify", label: "Verify", write: true, confirm: false, twice: false },
  agent: { id: "agent", label: "Start agent", write: true, confirm: false, twice: true },
  done: { id: "done", label: "Done", write: true, confirm: false, twice: false },
  drop: { id: "drop", label: "Drop", write: true, confirm: true, twice: false },
  open: { id: "open", label: "Open", write: false, confirm: false, twice: false },
  // WP-101: one click, no arming: it creates a case and destroys nothing
  reopen: { id: "reopen", label: "Reopen", write: true, confirm: false, twice: false }
}

var ACTIONS_BY_STATUS = {
  queued: ["start", "open"],
  active: ["verify", "agent", "drop", "open"],
  verification: ["done", "drop", "open"],
  completed: ["open", "reopen"],
  dropped: ["open"]
}

function caseActions(c) {
  if (!c || !c.actionable || ACTIONS_BY_STATUS[c.status] === undefined) return []
  var ids = ACTIONS_BY_STATUS[c.status]
  var out = []
  for (var i = 0; i < ids.length; i++) {
    var a = ACTIONS[ids[i]]
    out.push({ id: a.id, label: a.label, write: a.write, confirm: a.confirm, twice: a.twice, primary: i === 0 })
  }
  return out
}

function caseAction(c, actionId) {
  var list = caseActions(c)
  for (var i = 0; i < list.length; i++)
    if (list[i].id === actionId) return list[i]
  return null
}

// "dev-env · priority high · 2/4 steps"; a reopen leads with "reopens
// C-2026-002".
function caseMeta(c) {
  var parts = []
  if (c.reopens) parts.push("reopens " + c.reopens)
  if (c.area !== "") parts.push(c.area)
  if (c.priority !== "") parts.push("priority " + c.priority)
  if (c.stepsText !== "") parts.push(c.stepsText + " steps")
  return parts.join(" · ")
}

// "created 2026-09-28 · started 2026-10-01 · closed 2026-10-02"
function caseDates(c) {
  var parts = []
  if (c.created !== "") parts.push("created " + c.created)
  if (c.started !== "") parts.push("started " + c.started)
  if (c.closed !== "") parts.push("closed " + c.closed)
  return parts.join(" · ")
}

// "agent: claude-code" / "agents: claude-code, codex" from the case's
// `agents` (agent:NAME), "" without one.
function caseAgents(c) {
  var names = (c && Array.isArray(c.agents) ? c.agents : []).map(function(a) { return String(a).replace(/^agent:/, "") })
  if (names.length === 0) return ""
  return (names.length === 1 ? "agent: " : "agents: ") + names.join(", ")
}

// `seldon plan …` (CONTRACT.md, SPEC-ENGINE §3). Returns { args } or { error }.
//   planArgs("new", { title, zone, risk, area, priority })
//     → plan new --zone <z> --risk <r> [--area <a>] [--priority <p>] --json -- <title>
//     The title is one argument after `--`, exactly as typed; --area only
//     when given, --priority only when it is not the default (normal).
//   planArgs("start" | "verify" | "done" | "drop", caseId)
//     → plan <step> <caseId> --json
function planArgs(action, input) {
  if (action === "new") {
    var f = isObject(input) ? input : {}
    var title = f.title
    if (!hasText(title)) return { error: "Give the case a title" }
    if (String(title).indexOf("\u0000") !== -1) return { error: "The title contains a NUL character" }
    var zone = f.zone === undefined || f.zone === "" ? NEW_CASE_DEFAULTS.zone : f.zone
    var risk = f.risk === undefined || f.risk === "" ? NEW_CASE_DEFAULTS.risk : f.risk
    var priority = f.priority === undefined || f.priority === "" ? NEW_CASE_DEFAULTS.priority : f.priority
    var area = f.area === undefined || f.area === null ? "" : String(f.area)
    if (!matches(ZONES, zone)) return { error: "Not a zone: " + zone }
    if (!matches(RISKS, risk)) return { error: "Not a risk: " + risk }
    if (!matches(PRIORITIES, priority)) return { error: "Not a priority: " + priority }
    if (area !== "" && !AREA.test(area)) return { error: "Area must be a lowercase slug: letters, digits and -" }
    var args = ["plan", "new", "--zone", zone, "--risk", risk]
    if (area !== "") args.push("--area", area)
    if (priority !== NEW_CASE_DEFAULTS.priority) args.push("--priority", priority)
    return { args: args.concat(["--json", "--", title]) }
  }
  if (!matches(PLAN_STEPS, action) && action !== "reopen") return { error: "Not a plan step: " + action }
  var id = String(input || "")
  if (!CASE_ID.test(id)) return { error: "Not a case id: " + id }
  return { args: ["plan", action, id, "--json"] }
}

// `seldon plan … --json` (SPEC-ENGINE §3): new → {"case", "event",
// "areaCreated", "git"}; a step → {"case", "from", "to", "movedFrom",
// "activeCase", "journal", "event", "git"}. Returns { ok, text, caseId }.
function planResult(exitCode, stdoutText, stderrText) {
  if (exitCode !== 0) return { ok: false, text: engineError(stdoutText, stderrText, exitCode), caseId: "" }
  var data = parseJson(stdoutText)
  var c = data && isObject(data.case) ? data.case : null
  var id = c && typeof c.id === "string" && CASE_ID.test(c.id) ? c.id : ""
  var name = id !== "" ? id : "The case"
  // `plan reopen` (WP-101): {"case", "reopens", "earlier", …}
  if (data && typeof data.reopens === "string" && CASE_ID.test(data.reopens)) {
    var earlier = Array.isArray(data.earlier) ? data.earlier.filter(function(e) { return CASE_ID.test(String(e)) }) : []
    var again = earlier.length > 0 ? " · reopened before as " + earlier.join(", ") : ""
    // the engine leaves the active-case marker on an open case an agent
    // may be working (WP-101 round 2)
    var kept = isObject(data.activeCase) && typeof data.activeCase.kept === "string" && CASE_ID.test(data.activeCase.kept)
      ? " · the active case stays " + data.activeCase.kept : ""
    return { ok: true, text: "Reopened " + data.reopens + " as " + name + " (active)" + again + kept, caseId: id }
  }
  if (data && typeof data.from === "string" && typeof data.to === "string") {
    var text = name + ": " + data.from + " → " + data.to
    if (typeof data.journal === "string" && data.journal !== "") text += " · journal " + data.journal
    return { ok: true, text: text, caseId: id }
  }
  var created = (id !== "" ? "Created " + id : "Case created") + (c && typeof c.title === "string" && c.title !== "" ? " · " + c.title : "")
  if (data && typeof data.areaCreated === "string" && data.areaCreated !== "") created += " · new area " + data.areaCreated
  return { ok: true, text: created, caseId: id }
}

// `seldon agent start <caseId> --json` (WP-022): { args } or { error }; the
// answer {"launched", "launcher", "program", "case", …} → { ok, text, caseId }.
function agentArgs(caseId) {
  var id = String(caseId || "")
  if (!CASE_ID.test(id)) return { error: "Not a case id: " + id }
  return { args: ["agent", "start", id, "--json"] }
}

// `seldon agent start --new --json -- <intent>` (WP-101, ADR-0027 §6): the
// intent is one argument after `--`, exactly as typed. { args } or { error }.
function agentNewArgs(intent) {
  if (!hasText(intent)) return { error: "Say what the agent should do" }
  if (String(intent).indexOf("\u0000") !== -1) return { error: "The text contains a NUL character" }
  return { args: ["agent", "start", "--new", "--json", "--", String(intent)] }
}

function agentResult(exitCode, stdoutText, stderrText) {
  if (exitCode !== 0) return { ok: false, text: engineError(stdoutText, stderrText, exitCode), caseId: "" }
  var data = parseJson(stdoutText)
  var id = data && typeof data.case === "string" && CASE_ID.test(data.case) ? data.case : ""
  var launcher = data && typeof data.launcher === "string" ? data.launcher : ""
  var program = data && typeof data.program === "string" ? data.program : ""
  var via = launcher === "" ? program : program === "" || program === launcher ? launcher : launcher + " (" + program + ")"
  var created = data && isObject(data.created) && isObject(data.created.case) ? data.created.case : null
  var head = created && id !== ""
    ? "Created " + id + (typeof created.title === "string" && created.title !== "" ? " · " + created.title : "") + " · agent started"
    : "Agent started on " + (id !== "" ? id : "the case")
  return { ok: true, text: head + (via !== "" ? " · launcher " + via : ""), caseId: id }
}

// ---- The rules banner (WP-100, WP-101; ADR-0027 Migration) ------------------
//
// `seldon doctor --only rules --json` names the state of the logbook's agent
// rules in its `rules` row. When they are outdated and the engine's fix is a plain
// `seldon rules update` (it rewrites only the engine's block and archives an
// edited one: nothing is lost), the banner offers it as one click. A damaged
// or newer block (fix with --replace, or "update seldon") is shown without
// the click: that is the user's decision.
function rulesBanner(doctorText) {
  var data = parseJson(doctorText)
  var checks = data && Array.isArray(data.checks) ? data.checks : []
  for (var i = 0; i < checks.length; i++) {
    var c = checks[i]
    if (!isObject(c) || c.name !== "rules" || c.status === "ok") continue
    var fix = typeof c.fix === "string" ? c.fix : ""
    var oneClick = fix.indexOf("seldon rules update") === 0 && fix.indexOf("--replace") === -1
    var message = typeof c.message === "string" ? firstLine(c.message) : ""
    return {
      status: "rulesOutdated",
      tone: "accent",
      title: "The logbook's agent rules are " + (message !== "" ? message : "not current"),
      detail: oneClick
        ? "Agents read AGENTS.md. Update rewrites only Seldon's block; your own rules stay, an edited block is archived first."
        : "Fix in a terminal: " + (fix !== "" ? fix : "seldon doctor"),
      command: "",
      actions: oneClick ? [{ id: "update", label: "Update rules" }] : [],
      hint: ""
    }
  }
  return null
}

// How often the panel asks `seldon doctor --json` at most (on open).
var RULES_CHECK_MS = 10 * 60 * 1000

// The banner with the update's answer: pending or refused shows as its hint;
// a done update shows nothing (the next doctor answer decides).
function rulesBannerWith(banner, result) {
  if (!banner) return null
  if (!isObject(result) || (result.ok && !result.pending)) return banner
  var out = {}
  for (var k in banner) out[k] = banner[k]
  out.hint = String(result.text || "")
  if (result.pending) out.actions = []
  return out
}

// `seldon rules update --json` → { ok, text }: one line that says what the
// click did (WP-111), from the engine's `version` and `archived`; a refusal
// says what failed.
function rulesUpdateResult(exitCode, stdoutText, stderrText) {
  if (exitCode !== 0) return { ok: false, text: "Updating the agent rules failed: " + engineError(stdoutText, stderrText, exitCode) }
  var data = parseJson(stdoutText)
  var action = data && typeof data.action === "string" ? data.action : ""
  var version = data && typeof data.version === "number" && isFinite(data.version) ? " v" + data.version : ""
  var archived = data && typeof data.archived === "string" ? firstLine(data.archived) : ""
  if (action === "unchanged") return { ok: true, text: "The agent rules were already current" + (version !== "" ? " (" + version.trim() + ")" : "") }
  var text = "Agent rules updated" + (version !== "" ? " to" + version : "")
  if (archived !== "") text += "; your old copy is in " + archived
  return { ok: true, text: text }
}

// The neutral notice for a finished *Update rules* click (WP-111): its one
// line, until the panel opens again (Service.clearRulesResult). A pending or
// refused update shows in the banner instead; the capture's silent upgrade
// of an unedited block shows nothing.
function rulesNotice(result) {
  if (!isObject(result) || result.pending || !result.ok || !hasText(result.text)) return null
  return {
    status: "rulesUpdated",
    tone: "neutral",
    title: String(result.text),
    detail: "",
    command: "",
    actions: [],
    hint: ""
  }
}

// ---- Drift sheet (WP-021) ---------------------------------------------------
//
// One open drift item resolved from the Changelog: linked to a case,
// explained (a retroactive completed case, ADR-0021) or dismissed with a
// reason (SPEC-ENGINE §3, §5). A group (ADR-0013) resolves as one; `--only`
// resolves the named event alone.

var DRIFT_ACTIONS = ["link", "explain", "dismiss"]
var DRIFT_ACTION_LABELS = { link: "Link", explain: "Explain", dismiss: "Dismiss" }
// `drift explain` without --risk (SPEC-ENGINE §3); --zone defaults to the item's.
var EXPLAIN_RISK_DEFAULT = "R1"
// Member lines the sheet shows before "… and N more".
var DRIFT_MEMBERS_SHOWN = 8

function findEvent(index, id) {
  var all = events(index)
  for (var i = 0; i < all.length; i++)
    if (isObject(all[i]) && all[i].id === id) return all[i]
  return null
}

// The drift item event `eventId` belongs to, as the sheet shows it, or null
// when the event is not open drift in this index. The event may be the
// item's own (a single item or a group's leader) or an open member of a
// group; the sheet names that event in the engine call, so `--only`
// resolves exactly the row it was opened from.
function driftItemFor(index, eventId) {
  var id = String(eventId || "")
  if (!EVENT_ID.test(id)) return null
  var drift = driftLookup(index)
  var event = findEvent(index, id)
  var item = drift.byId[id] || null
  if (!item && event && typeof event.txId === "string" && isOpenMember(event, event.txId))
    item = drift.byTx[event.txId] || null
  if (!item || typeof item.eventId !== "string" || !EVENT_ID.test(item.eventId)) return null
  var grouped = typeof item.members === "number" && item.members > 1 && typeof item.txId === "string"
  // The ledger zone (ADR-0028 §7); `crisis` no longer implies red.
  var zone = typeof item.zone === "string" ? item.zone : str(event && event.zone)
  return {
    eventId: id,
    leaderId: item.eventId,
    day: dayOf(item.ts),
    time: clockTime(item.ts),
    source: str(item.source),
    glyph: sourceGlyph(item.source),
    kind: str(item.kind),
    subject: str(item.subject),
    detail: str(item.detail),
    actor: actorLabel(item.actor),
    zone: zone,
    tone: driftTone(item),
    crisis: item.crisis === true,
    proposedCase: typeof item.proposedCase === "string" && CASE_ID.test(item.proposedCase) ? item.proposedCase : "",
    grouped: grouped,
    txId: grouped ? item.txId : "",
    members: grouped ? item.members : 1,
    // What index.events still lists (CONTRACT.md rule 4 caps it); the sheet
    // asks `seldon drift show` for the rest.
    memberList: grouped ? groupMembers(index, item.txId).reverse() : [],
    badge: grouped ? "+" + (item.members - 1) : "",
    namedSubject: event ? str(event.subject) : str(item.subject)
  }
}

// The Link picker: the proposed case first, preselected, then every other
// open case (active, verification, queued). Without a proposal the first
// option is "Pick a case", which Link refuses.
function caseOptionsFor(index, item) {
  var proposed = item && item.proposedCase ? item.proposedCase : ""
  var cases = openCases(index)
  var out = []
  var label = function(c) { return c.id + (c.title !== "" ? " · " + c.title : "") }
  if (proposed !== "") {
    var found = null
    for (var i = 0; i < cases.length; i++) if (cases[i].id === proposed) found = cases[i]
    out.push({ value: proposed, label: (found ? label(found) : proposed) + " · proposed" })
  } else {
    out.push({ value: "", label: "Pick a case" })
  }
  for (var k = 0; k < cases.length; k++)
    if (cases[k].id !== proposed) out.push({ value: cases[k].id, label: label(cases[k]) })
  return out
}

// The sheet's starting action: Link when the engine proposes a case, else
// Explain. Neither is required (ADR-0028 §3); this is only where the form
// starts.
function driftDefaultAction(item) {
  return item && item.proposedCase !== "" ? "link" : "explain"
}

// `seldon drift link|explain|dismiss …` (CONTRACT.md, SPEC-ENGINE §3).
// Returns { args } or { error }; nothing reaches the engine unchecked.
//   driftArgs("link", { eventId, caseId, only })
//     → drift link <eventId> <caseId> [--only] --json
//   driftArgs("explain", { eventId, only, text, zone, risk, area, itemZone })
//     → drift explain <eventId> [--only] [--zone <z>] [--risk <r>] [--area <a>] --json -- <text>
//     --zone only when it differs from the item's zone (the engine's
//     default), --risk only when not R1, --area only when given
//   driftArgs("dismiss", { eventId, only, text })
//     → drift dismiss <eventId> [--only] --json -- <text>
// The text is one argument after `--`, exactly as typed, and one line (the
// engine refuses more).
function driftArgs(action, input) {
  var f = isObject(input) ? input : {}
  if (!matches(DRIFT_ACTIONS, action)) return { error: "Not a drift action: " + action }
  var id = String(f.eventId || "")
  if (!EVENT_ID.test(id)) return { error: "Not an event id: " + id }
  var only = f.only === true ? ["--only"] : []
  if (action === "link") {
    var caseId = String(f.caseId || "")
    if (caseId === "") return { error: "Pick a case first" }
    if (!CASE_ID.test(caseId)) return { error: "Not a case id: " + caseId }
    return { args: ["drift", "link", id, caseId].concat(only, ["--json"]) }
  }
  var text = f.text
  if (!hasText(text)) return { error: action === "explain" ? "Say why it changed first" : "Give a reason first" }
  if (/[\r\n\u0000]/.test(text)) return { error: "The text must be one line" }
  var args = ["drift", action, id].concat(only)
  if (action === "explain") {
    var zone = f.zone === undefined || f.zone === null ? "" : String(f.zone)
    var risk = f.risk === undefined || f.risk === null ? "" : String(f.risk)
    var area = f.area === undefined || f.area === null ? "" : String(f.area)
    if (zone !== "" && !matches(ZONES, zone)) return { error: "Not a zone: " + zone }
    if (risk !== "" && !matches(RISKS, risk)) return { error: "Not a risk: " + risk }
    if (area !== "" && !AREA.test(area)) return { error: "Area must be a lowercase slug: letters, digits and -" }
    if (zone !== "" && zone !== String(f.itemZone || "")) args.push("--zone", zone)
    if (risk !== "" && risk !== EXPLAIN_RISK_DEFAULT) args.push("--risk", risk)
    if (area !== "") args.push("--area", area)
  }
  return { args: args.concat(["--json", "--", text]) }
}

// What a call would resolve, for the button hint: "Link firefox and 2 more
// to C-2026-005", "Dismiss libinput only", "Explain tokyo-night as a new
// completed case".
function driftSummary(action, item, form) {
  if (!item || !matches(DRIFT_ACTIONS, action)) return ""
  var f = isObject(form) ? form : {}
  var what = !item.grouped ? item.namedSubject
    : f.only === true ? item.namedSubject + " only"
    : item.subject + " and " + (item.members - 1) + " more"
  if (action === "link") return "Link " + what + (f.caseId ? " to " + f.caseId : "")
  if (action === "explain") return "Explain " + what + " as a new completed case"
  return "Dismiss " + what
}

// `seldon drift link|explain|dismiss --json` (SPEC-ENGINE §3) → {eventId,
// resolution, only, txId, resolved, events, case, areaCreated, git}; a
// no-op (nothing open any more) → {resolved: 0, events: [], already:
// {resolution, case}}, exit 0. Returns { ok, already, text, caseId, resolved }.
function driftResult(action, exitCode, stdoutText, stderrText) {
  if (exitCode !== 0)
    return { ok: false, already: false, text: engineError(stdoutText, stderrText, exitCode), caseId: "", resolved: 0 }
  var data = parseJson(stdoutText)
  var resolved = data ? count(data.resolved) : 0
  if (data && resolved === 0) {
    var a = isObject(data.already) ? data.already : {}
    var how = typeof a.resolution === "string" ? a.resolution : ""
    var owner = typeof a.case === "string" && CASE_ID.test(a.case) ? a.case : ""
    var text = how !== "" ? "Already resolved: " + resolutionHead(how, owner)
      : owner !== "" ? "Nothing to resolve: it belongs to " + owner
      : "Nothing to resolve: not open drift"
    return { ok: true, already: true, text: text, caseId: owner, resolved: 0 }
  }
  var c = data && isObject(data.case) ? data.case : null
  var caseId = c && typeof c.id === "string" && CASE_ID.test(c.id) ? c.id : ""
  var n = plural(resolved, "event", "events")
  var out = action === "link" ? "Linked " + n + (caseId !== "" ? " to " + caseId : "")
    : action === "explain" ? "Explained " + n + (caseId !== "" ? " · created " + caseId : "")
    : "Dismissed " + n
  if (data && typeof data.areaCreated === "string" && data.areaCreated !== "") out += " · new area " + data.areaCreated
  return { ok: true, already: false, text: out, caseId: caseId, resolved: resolved }
}

// `seldon drift show <id> --json` → {event, open, item, txId, members}: every
// open member of the group, oldest first. Returns { ok, text, members }.
function driftShowResult(exitCode, stdoutText, stderrText) {
  if (exitCode !== 0) return { ok: false, text: engineError(stdoutText, stderrText, exitCode), members: [] }
  var data = parseJson(stdoutText)
  var list = data && Array.isArray(data.members) ? data.members : []
  var members = []
  for (var i = 0; i < list.length; i++) {
    var m = list[i]
    if (isObject(m) && typeof m.id === "string" && EVENT_ID.test(m.id))
      members.push({ id: m.id, kind: str(m.kind), subject: str(m.subject), detail: str(m.detail) })
  }
  return { ok: true, text: "", members: members }
}

// The member lines the sheet shows: at most DRIFT_MEMBERS_SHOWN, then
// "… and N more".
function memberLines(members, total) {
  var list = Array.isArray(members) ? members : []
  var out = []
  for (var i = 0; i < list.length && i < DRIFT_MEMBERS_SHOWN; i++) out.push("· " + memberLine(list[i]))
  var rest = Math.max(count(total), list.length) - out.length
  if (rest > 0) out.push("… and " + rest + " more")
  return out
}

// The folded resolution of event `id` in this index ("linked to C-…",
// "dismissed: <reason>"), "" when it has none.
function eventResolution(index, id) {
  var e = findEvent(index, id)
  if (!e || typeof e.resolution !== "string" || e.resolution === "") return ""
  return rowStatus({ resolution: e.resolution, resolutionDetail: str(e.resolutionDetail), caseId: str(e.case) })
}

// The crisis strip's target: the first crisis item (the index lists crises
// first, ADR-0020), else the first item, else "".
function firstCrisis(index) {
  var list = index && Array.isArray(index.drift) ? index.drift : []
  var first = ""
  for (var i = 0; i < list.length; i++) {
    var d = list[i]
    if (!isObject(d) || typeof d.eventId !== "string" || !EVENT_ID.test(d.eventId)) continue
    if (d.crisis === true) return d.eventId
    if (first === "") first = d.eventId
  }
  return first
}

// ADR-0020: the index lists the newest 200 open drift items; the summary
// counts all. "+N more changes without a case not listed here", or "".
function moreDriftText(index) {
  var c = counts(index)
  var listed = index && Array.isArray(index.drift) ? index.drift.length : 0
  var more = c ? c.drift - listed : 0
  return more > 0 ? "+" + plural(more, "more change", "more changes") + " without a case not listed here" : ""
}

// ---- Decisions (WP-023) -----------------------------------------------------

// schema/index.schema.json decisions[].status.
var DECISION_STATUSES = ["proposed", "accepted", "superseded"]

// One row per decision, newest first: by id, highest number first (the
// engine numbers decisions in the order they are made; the date is the
// user's to edit). A row whose id does not match the schema pattern is
// shown last and never opened (`actionable` false): the id goes into an
// argument list. tone: "accent" for proposed (it waits for a decision),
// "muted" for superseded.
function decisionRows(index) {
  var list = index && Array.isArray(index.decisions) ? index.decisions : []
  var out = []
  for (var i = 0; i < list.length; i++) {
    var d = list[i]
    if (!isObject(d)) continue
    var id = str(d.id)
    var status = matches(DECISION_STATUSES, d.status) ? d.status : ""
    out.push({
      id: id,
      title: str(d.title),
      status: status,
      date: str(d.date),
      path: str(d.path),
      tone: status === "proposed" ? "accent" : status === "superseded" ? "muted" : "",
      actionable: DECISION_ID.test(id)
    })
  }
  out.sort(function(a, b) {
    if (a.actionable !== b.actionable) return a.actionable ? -1 : 1
    return a.id < b.id ? 1 : a.id > b.id ? -1 : 0
  })
  return out
}

// "4 decisions · 1 proposed", or "No decisions yet".
function decisionSummary(rows) {
  var n = Array.isArray(rows) ? rows.length : 0
  if (n === 0) return "No decisions yet"
  var proposed = rows.filter(function(r) { return r.status === "proposed" }).length
  return plural(n, "decision", "decisions") + (proposed > 0 ? " · " + proposed + " proposed" : "")
}

// "2026-10-01 · decisions/ADR-0004-ollama-user-service.md"
function decisionMeta(row) {
  var parts = []
  if (row.date !== "") parts.push(row.date)
  if (row.path !== "") parts.push(row.path)
  return parts.join(" · ")
}

// `seldon decide --no-edit --json -- <title>` (CONTRACT.md, SPEC-ENGINE §3):
// the title is one argument after `--`, exactly as typed; the engine refuses
// a title of more than one line, so the plugin does too. Returns { args } or
// { error }.
function decideArgs(title) {
  if (!hasText(title)) return { error: "Give the decision a title" }
  if (title.indexOf("\u0000") !== -1) return { error: "The title contains a NUL character" }
  if (/[\r\n]/.test(title)) return { error: "The title must be one line" }
  return { args: ["decide", "--no-edit", "--json", "--", title] }
}

// `seldon decide --json` → {"decision": {"id", "title", "status", "date",
// "cases", "path"}, "editor", "git"}. Returns { ok, text, decisionId };
// decisionId is "" unless the id matches the schema pattern, so only a
// checked id is ever opened.
function decideResult(exitCode, stdoutText, stderrText) {
  if (exitCode !== 0) return { ok: false, text: engineError(stdoutText, stderrText, exitCode), decisionId: "" }
  var data = parseJson(stdoutText)
  var d = data && isObject(data.decision) ? data.decision : null
  var id = d && typeof d.id === "string" && DECISION_ID.test(d.id) ? d.id : ""
  var text = (id !== "" ? "Created " + id : "Decision created")
    + (d && typeof d.title === "string" && d.title !== "" ? " · " + d.title : "")
  return { ok: true, text: text, decisionId: id }
}

// ---- Memory (WP-023) --------------------------------------------------------

// What the Memory tab opens. The engine's `seldon open` has no memory target
// (SPEC-ENGINE §3), so every row opens the logbook folder for now; once the
// engine has one, this and memoryRows() change, nothing else.
var MEMORY_OPEN_TARGET = "logbook"

// SPEC-PLUGIN §5, Memory: the `## ` headings of memory/lessons.md, then the
// other memory files as the index lists them (most recently updated first).
// Rows: { section, kind: "lesson"|"topic", title, meta, target }; `section`
// is set on the first row of each section.
function memoryRows(index) {
  var m = index && isObject(index.memory) ? index.memory : {}
  var lessons = Array.isArray(m.lessons) ? m.lessons : []
  var topics = Array.isArray(m.topics) ? m.topics : []
  var out = []
  for (var i = 0; i < lessons.length; i++) {
    if (!hasText(lessons[i])) continue
    out.push({ section: "", kind: "lesson", title: lessons[i], meta: "", target: MEMORY_OPEN_TARGET })
  }
  if (out.length > 0) out[0].section = "LESSONS"
  var first = out.length
  for (var j = 0; j < topics.length; j++) {
    var t = topics[j]
    if (!isObject(t) || !hasText(t.topic)) continue
    var meta = []
    if (hasText(t.path)) meta.push(t.path)
    if (hasText(t.updated)) meta.push("updated " + t.updated)
    out.push({ section: "", kind: "topic", title: t.topic, meta: meta.join(" · "), target: MEMORY_OPEN_TARGET })
  }
  if (out.length > first) out[first].section = "TOPICS"
  return out
}

// "3 lessons · 2 topics", or "".
function memorySummary(rows) {
  var list = Array.isArray(rows) ? rows : []
  var lessons = list.filter(function(r) { return r.kind === "lesson" }).length
  var topics = list.length - lessons
  return list.length === 0 ? "" : plural(lessons, "lesson", "lessons") + " · " + plural(topics, "topic", "topics")
}

// ---- Prime Radiant (overlay) ------------------------------------------------

// The period selector (WP-030), in key order: `1`–`4` pick these, ←/→ walk
// them. "all" has no bounds: it covers every row the index carries.
var PERIODS = [
  { id: "30", days: 30, label: "30 d" },
  { id: "90", days: 90, label: "90 d" },
  { id: "365", days: 365, label: "365 d" },
  { id: "all", days: 0, label: "All" }
]
var PERIOD_DEFAULT = "90"

// The six slots (SPEC-PLUGIN §6), in grid order. `series` names the index
// field the slot renders: five `series.*` charts (WP-031) and The Plan, which
// reads `cases.active` and has no period.
var OVERLAY_SLOTS = [
  { id: "heatmap", title: "Heatmap", subtitle: "Events per day", series: "heatmap" },
  { id: "series", title: "Series", subtitle: "Explicit and total packages over time", series: "packages" },
  { id: "driftBars", title: "DriftBars", subtitle: "Drift opened vs resolved per week", series: "drift" },
  { id: "riskDonut", title: "RiskDonut", subtitle: "Cases by risk", series: "risk" },
  { id: "timeline", title: "Timeline", subtitle: "Releases, snapshots, cases, crises", series: "timeline" },
  { id: "plan", title: "The Plan", subtitle: "Active cases, steps and agent", series: "cases" }
]

var TIMELINE_KINDS = ["case", "release", "snapshot", "crisis"]
var TIMELINE_KIND_NAMES = {
  case: ["case", "cases"], release: ["release", "releases"],
  snapshot: ["snapshot", "snapshots"], crisis: ["crisis", "crises"]
}

function isPeriod(id) {
  for (var i = 0; i < PERIODS.length; i++) if (PERIODS[i].id === id) return true
  return false
}

function periodById(id) {
  for (var i = 0; i < PERIODS.length; i++) if (PERIODS[i].id === id) return PERIODS[i]
  return periodById(PERIOD_DEFAULT)
}

// "1"–"4" → the period id, anything else → "".
function periodForKey(text) {
  var n = Number(text)
  return String(text).length === 1 && n >= 1 && n <= PERIODS.length ? PERIODS[n - 1].id : ""
}

// The next period to the left (-1) or right (+1), wrapping like the tabs.
function cyclePeriod(current, direction) {
  var i = PERIODS.indexOf(periodById(current))
  var n = PERIODS.length
  return PERIODS[((i + (direction < 0 ? -1 : 1)) % n + n) % n].id
}

// `summon jax.seldon '{"period":"30"}'` opens on that period; anything else
// (no payload, broken JSON, an unknown id) opens on `fallback`.
function overlayPayloadPeriod(payloadJson, fallback) {
  var data = parseJson(payloadJson)
  return data && isPeriod(data.period) ? data.period : fallback
}

// A real calendar date "YYYY-MM-DD" (no 2026-02-30).
function isDate(value) {
  return isFinite(dayNumber(value))
}

// The days a period covers, as inclusive YYYY-MM-DD bounds ending on the
// index's today (ADR-0012 §10): "30" is today and the 29 days before it.
// "all", or no today, leaves both bounds "" (unbounded).
function periodWindow(period, today) {
  var p = periodById(period)
  if (p.days === 0 || !isDate(today)) return { period: p.id, from: "", to: "", days: p.days }
  return { period: p.id, from: addDays(today, 1 - p.days), to: today, days: p.days }
}

function dateInWindow(date, win) {
  if (!isDate(date)) return false
  return (win.from === "" || date >= win.from) && (win.to === "" || date <= win.to)
}

// A span from `start` to `end` (both YYYY-MM-DD; end "" while it is open)
// overlaps the window.
function spanInWindow(start, end, win) {
  if (!isDate(start)) return false
  if (end !== "" && (!isDate(end) || end < start)) return false
  return (win.to === "" || start <= win.to) && (end === "" || win.from === "" || end >= win.from)
}

// "2026-W40" → the Monday of that ISO week ("2026-09-28"), else "".
function isoWeekMonday(week) {
  var m = /^(\d{4})-W(\d{2})$/.exec(str(week))
  if (!m) return ""
  var n = Number(m[2])
  if (n < 1 || n > 53) return ""
  var jan4 = utcDate(m[1] + "-01-04")
  var week1 = addDays(m[1] + "-01-04", -((jan4.getUTCDay() + 6) % 7))
  var monday = addDays(week1, (n - 1) * 7)
  // Week 53 exists only in long years: its Monday must still be in the year
  // whose week 1 follows it by at least four days.
  return n === 53 && isoWeekMonday(String(Number(m[1]) + 1) + "-W01") <= monday ? "" : monday
}

// The rows of one `series.*` list that fall into the window, in index order.
// heatmap and packages go by date, drift by ISO week (a week counts when any
// of its days does), the timeline by day (case spans when they overlap the
// window; an open case runs on). Rows that break the schema are left out.
function seriesInPeriod(series, key, win) {
  var list = isObject(series) && Array.isArray(series[key]) ? series[key] : []
  var out = []
  for (var i = 0; i < list.length; i++) {
    var r = list[i]
    if (!isObject(r)) continue
    var keep = false
    if (key === "heatmap" || key === "packages") {
      keep = dateInWindow(r.date, win)
    } else if (key === "drift") {
      var monday = isoWeekMonday(r.week)
      keep = monday !== "" && spanInWindow(monday, addDays(monday, 6), win)
    } else if (key === "timeline") {
      if (r.kind === "case") keep = spanInWindow(str(r.ts), r.end === null || r.end === undefined ? "" : str(r.end), win)
      else if (TIMELINE_KINDS.indexOf(r.kind) !== -1) keep = dateInWindow(dayOf(r.ts), win)
    }
    if (keep) out.push(r)
  }
  return out
}

// series.risk as { R0: n, …, R3: n } (absent classes 0). It has no dates, so
// every period shows the same counts.
function riskCounts(series) {
  var risk = isObject(series) && isObject(series.risk) ? series.risk : {}
  var out = {}
  for (var i = 0; i < RISKS.length; i++) out[RISKS[i]] = count(risk[RISKS[i]])
  return out
}

function sumOf(rows, field) {
  var total = 0
  for (var i = 0; i < rows.length; i++) total += count(rows[i][field])
  return total
}

// What a slot's placeholder says for one period: { id, title, subtitle,
// rows, count, detail, windowed }. `rows` is the number of series rows the
// chart will draw; `count` says it in words.
function slotSummary(slot, data) {
  var out = { id: slot.id, title: slot.title, subtitle: slot.subtitle, rows: 0, count: "", detail: "", windowed: true }
  if (slot.id === "heatmap") {
    out.rows = data.heatmap.length
    out.count = plural(out.rows, "day", "days")
    out.detail = plural(sumOf(data.heatmap, "total"), "event", "events")
  } else if (slot.id === "series") {
    var p = data.packages
    out.rows = p.length
    out.count = plural(out.rows, "sample", "samples")
    out.detail = p.length === 0 ? "No package counts"
      : p.length === 1 ? count(p[0].explicit) + " explicit"
      : "Explicit " + count(p[0].explicit) + " → " + count(p[p.length - 1].explicit)
  } else if (slot.id === "driftBars") {
    out.rows = data.drift.length
    out.count = plural(out.rows, "week", "weeks")
    out.detail = sumOf(data.drift, "opened") + " opened · " + sumOf(data.drift, "resolved") + " resolved"
  } else if (slot.id === "riskDonut") {
    var parts = []
    var cases = 0
    for (var i = 0; i < RISKS.length; i++) {
      var n = data.risk[RISKS[i]]
      if (n > 0) out.rows++
      cases += n
      parts.push(RISKS[i] + " " + n)
    }
    out.count = plural(cases, "case", "cases")
    out.detail = parts.join(" · ") + " · all time"
    out.windowed = false
  } else if (slot.id === "timeline") {
    var t = data.timeline
    out.rows = t.length
    out.count = plural(out.rows, "entry", "entries")
    var byKind = []
    for (var k = 0; k < TIMELINE_KINDS.length; k++) {
      var kind = TIMELINE_KINDS[k]
      var c = t.filter(function(r) { return r.kind === kind }).length
      if (c > 0) byKind.push(plural(c, TIMELINE_KIND_NAMES[kind][0], TIMELINE_KIND_NAMES[kind][1]))
    }
    out.detail = byKind.length > 0 ? byKind.join(" · ") : "Nothing in this period"
  } else if (slot.id === "plan") {
    var plan = data.plan || planChart(null)
    out.rows = plan.rows.length
    out.count = plural(out.rows, "active case", "active cases")
    out.detail = plan.numbers.done + " of " + plan.numbers.steps + " steps done"
    out.windowed = false
  }
  return out
}

// Every period's window, series rows, slot summaries and chart data,
// computed once per index (Service.qml rebuilds it when the index changes),
// so opening the overlay or switching periods only looks things up:
// { today, plan, periods: { <id>: { window, series: { heatmap, packages,
// drift, timeline, risk }, charts: { heatmap, series, driftBars, riskDonut,
// timeline, plan }, slots: [slotSummary…] } } }.
// Each series is walked once for all four periods (splitSeries); the charts
// then work on the rows of their period only.
function periodTable(index) {
  aggregationRuns++
  var series = index && isObject(index.series) ? index.series : {}
  var today = index ? todayDate(index) : ""
  var risk = riskCounts(series)
  var wins = PERIODS.map(function(p) { return periodWindow(p.id, today) })
  // Timeline rows are parsed once, by splitSeries, for all four periods.
  var parsed = new Map()
  var cut = {
    heatmap: splitSeries(series, "heatmap", wins),
    packages: splitSeries(series, "packages", wins),
    drift: splitSeries(series, "drift", wins),
    timeline: splitSeries(series, "timeline", wins, parsed)
  }
  var plan = planChart(index)
  var donut = riskChart(risk)
  var table = { today: today, plan: plan, periods: {} }
  for (var i = 0; i < PERIODS.length; i++) {
    var win = wins[i]
    var data = {
      heatmap: cut.heatmap[i],
      packages: cut.packages[i],
      drift: cut.drift[i],
      timeline: cut.timeline[i],
      risk: risk,
      plan: plan
    }
    var charts = {
      heatmap: heatmapChart(data.heatmap, win, today),
      series: seriesChart(data.packages, win, today),
      driftBars: driftChart(data.drift),
      riskDonut: donut,
      timeline: timelineChart(data.timeline, win, today, parsed),
      plan: plan
    }
    var slots = []
    for (var s = 0; s < OVERLAY_SLOTS.length; s++) slots.push(slotSummary(OVERLAY_SLOTS[s], data))
    delete data.plan
    table.periods[PERIODS[i].id] = { window: win, series: data, charts: charts, slots: slots }
  }
  return table
}

// ---- Prime Radiant charts (WP-031) -----------------------------------------
//
// Pure chart data for the overlay's slots: bins, domains, colour steps,
// lanes, summaries and hover texts. The service builds it once per index
// (periodTable); the QML charts only turn it into pixels (the *Layout and
// *At helpers below are geometry, no aggregation).

// Counts the aggregation passes of this script instance (periodTable and
// the chart builders). The overlay reports it in `call view`, and the
// harness asserts that opening the overlay and switching periods add none.
var aggregationRuns = 0
function aggregationCount() {
  return aggregationRuns
}

// Colour steps 1–5 are the theme accent at these opacities (Util.alpha);
// step 0 (no events) is the foreground at CHART_ZERO_ALPHA.
var CHART_STEP_ALPHAS = [0.25, 0.42, 0.6, 0.8, 1.0]
var CHART_ZERO_ALPHA = 0.07
var CHART_EMPTY_TEXT = "no data in this period"
var DAY_MS = 86400000

// Each chart's data when it has nothing to draw (`kind` is its slot id).
// The builders start from it; emptyPeriodView uses it as it is.
function emptyChart(kind) {
  if (kind === "heatmap")
    return { empty: true, emptyText: CHART_EMPTY_TEXT, summary: "Heatmap: " + CHART_EMPTY_TEXT,
      numbers: { days: 0, events: 0, activeDays: 0, max: 0, busiest: "" },
      firstDay: 0, offset: 0, weeks: 0, cells: [], months: [] }
  if (kind === "series")
    return { empty: true, emptyText: CHART_EMPTY_TEXT, summary: "Series: " + CHART_EMPTY_TEXT,
      numbers: { samples: 0, explicitFirst: 0, explicitLast: 0, totalFirst: null, totalLast: null },
      x0: 0, x1: 0, points: [], lanes: [] }
  if (kind === "driftBars")
    return { empty: true, emptyText: CHART_EMPTY_TEXT, summary: "DriftBars: " + CHART_EMPTY_TEXT,
      numbers: { weeks: 0, opened: 0, resolved: 0, max: 0, peak: "" }, max: 0, weeks: [] }
  if (kind === "riskDonut") {
    var numbers = { total: 0 }
    for (var i = 0; i < RISKS.length; i++) numbers[RISKS[i]] = 0
    return { empty: true, emptyText: "no cases yet · all time", summary: "RiskDonut: no cases yet · all time",
      numbers: numbers, total: 0,
      parts: RISKS.map(function(r) { return { risk: r, count: 0, share: 0, from: 0, to: 0 } }) }
  }
  if (kind === "timeline")
    return { empty: true, emptyText: CHART_EMPTY_TEXT, summary: "Timeline: " + CHART_EMPTY_TEXT,
      numbers: { cases: 0, open: 0, releases: 0, snapshots: 0, crises: 0, lanes: 0 },
      x0: 0, x1: 0, lanes: 0, markers: [], spans: [], months: [] }
  return { empty: true, emptyText: "no active cases", summary: "The Plan: no active cases",
    numbers: { cases: 0, done: 0, steps: 0 }, rows: [] }
}

// YYYY-MM-DD → days since 1970-01-01 (proleptic Gregorian), NaN for
// anything that is not a calendar date. Plain arithmetic (H. Hinnant's
// days_from_civil): the period table parses thousands of dates per index.
function dayNumber(date) {
  if (typeof date !== "string" || date.length !== 10 || date.charCodeAt(4) !== 45 || date.charCodeAt(7) !== 45) return NaN
  var y = digitsAt(date, 0, 4)
  var mo = digitsAt(date, 5, 2)
  var d = digitsAt(date, 8, 2)
  if (y < 0 || mo < 0 || d < 0) return NaN
  var leap = y % 4 === 0 && (y % 100 !== 0 || y % 400 === 0)
  if (mo < 1 || mo > 12 || d < 1 || d > [31, leap ? 29 : 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31][mo - 1]) return NaN
  if (mo <= 2) y -= 1
  var era = Math.floor(y / 400)
  var yoe = y - era * 400
  var doy = Math.floor((153 * (mo > 2 ? mo - 3 : mo + 9) + 2) / 5) + d - 1
  return era * 146097 + yoe * 365 + Math.floor(yoe / 4) - Math.floor(yoe / 100) + doy - 719468
}

// The decimal number of `n` ASCII digits at `at`, -1 if one is not a digit.
function digitsAt(text, at, n) {
  var v = 0
  for (var i = at; i < at + n; i++) {
    var c = text.charCodeAt(i) - 48
    if (c < 0 || c > 9) return -1
    v = v * 10 + c
  }
  return v
}

// Days since 1970-01-01 → "YYYY-MM-DD" (civil_from_days).
function dateOfDay(n) {
  var z = Math.floor(n) + 719468
  var era = Math.floor(z / 146097)
  var doe = z - era * 146097
  var yoe = Math.floor((doe - Math.floor(doe / 1460) + Math.floor(doe / 36524) - Math.floor(doe / 146096)) / 365)
  var doy = doe - (365 * yoe + Math.floor(yoe / 4) - Math.floor(yoe / 100))
  var mp = Math.floor((5 * doy + 2) / 153)
  var d = doy - Math.floor((153 * mp + 2) / 5) + 1
  var mo = mp < 10 ? mp + 3 : mp - 9
  var y = yoe + era * 400 + (mo <= 2 ? 1 : 0)
  return (y < 1000 ? ("000" + y).slice(-4) : String(y)) + "-" + (mo < 10 ? "0" : "") + mo + "-" + (d < 10 ? "0" : "") + d
}

// 0 = Monday … 6 = Sunday (1970-01-01 was a Thursday).
function weekdayOfDay(n) {
  return ((n + 3) % 7 + 7) % 7
}

// "2026-09-28" → "2026-W40" (ISO 8601: the week of the Thursday).
function isoWeekOf(date) {
  var n = dayNumber(date)
  if (!isFinite(n)) return ""
  var thursday = n - weekdayOfDay(n) + 3
  var year = dateOfDay(thursday).slice(0, 4)
  var week = Math.floor((thursday - dayNumber(year + "-01-01")) / 7) + 1
  return year + "-W" + (week < 10 ? "0" : "") + week
}

// "28 Sep"
function shortDay(date) {
  var n = dayNumber(date)
  if (!isFinite(n)) return ""
  return Number(date.slice(8, 10)) + " " + MONTHS[Number(date.slice(5, 7)) - 1]
}

// The colour step (0–5) of `value` against the period's `max`: 0 for none,
// else 1–5 by the square root of value/max, so a single busy day does not
// flatten every other day into step 1.
function colourStep(value, max) {
  var v = count(value)
  var m = count(max)
  if (v === 0 || m === 0) return 0
  var n = CHART_STEP_ALPHAS.length
  return Math.max(1, Math.min(n, Math.ceil(n * Math.sqrt(Math.min(v, m) / m) - 1e-9)))
}

// Maps v from [d0, d1] onto [r0, r1]; a zero-width domain maps to the middle.
function scale(v, d0, d1, r0, r1) {
  return d1 === d0 ? (r0 + r1) / 2 : r0 + (v - d0) * (r1 - r0) / (d1 - d0)
}

function inWindowDate(date, win) {
  return (win.from === "" || date >= win.from) && (win.to === "" || date <= win.to)
}

function inWindowSpan(start, end, win) {
  return (win.to === "" || start <= win.to) && (end === "" || win.from === "" || end >= win.from)
}

// The rows of one `series.*` list for every window of `wins`, in index
// order: the same rows as seriesInPeriod(series, key, wins[i]), but each
// row is checked once and then only compared against the window bounds.
// `parsed` (optional, a Map) receives each kept timeline row's days for
// timelineChart.
function splitSeries(series, key, wins, parsed) {
  aggregationRuns++
  var list = isObject(series) && Array.isArray(series[key]) ? series[key] : []
  var out = wins.map(function() { return [] })
  for (var i = 0; i < list.length; i++) {
    var r = list[i]
    if (!isObject(r)) continue
    var start = ""
    var end = ""
    var span = false
    if (key === "heatmap" || key === "packages") {
      if (!isDate(r.date)) continue
      start = r.date
    } else if (key === "drift") {
      start = isoWeekMonday(r.week)
      if (start === "") continue
      end = addDays(start, 6)
      span = true
    } else if (key === "timeline") {
      if (r.kind === "case") {
        start = str(r.ts)
        end = r.end === null || r.end === undefined ? "" : str(r.end)
        var startDay = dayNumber(start)
        var endDay = end === "" ? NaN : dayNumber(end)
        if (!isFinite(startDay) || (end !== "" && (!isFinite(endDay) || end < start))) continue
        if (parsed) parsed.set(r, { start: startDay, end: endDay })
        span = true
      } else if (TIMELINE_KINDS.indexOf(r.kind) !== -1) {
        var x = timelinePos(r.ts)
        if (!isFinite(x)) continue
        start = dayOf(r.ts)
        if (parsed) parsed.set(r, { x: x, marker: null })
      } else {
        continue
      }
    } else {
      continue
    }
    for (var w = 0; w < wins.length; w++)
      if (span ? inWindowSpan(start, end, wins[w]) : inWindowDate(start, wins[w])) out[w].push(r)
  }
  return out
}

// The last day a chart's x axis covers: the window's end, else the later of
// today and the newest row.
function lastDay(win, today, newest) {
  if (win.to !== "") return win.to
  return isDate(today) && today >= newest ? today : newest
}

// Heatmap: one cell per day of the window (for All: from the oldest row),
// laid out in ISO weeks (columns) × weekdays (rows, Monday on top).
// { empty, emptyText, summary, numbers, firstDay, offset, weeks, cells:
// [{ date, col, row, total, step, sources }], months: [{ col, label }] }.
// `offset` is the weekday of the first day; a day not in the series counts 0.
function heatmapChart(rows, win, today) {
  aggregationRuns++
  var out = emptyChart("heatmap")
  if (rows.length === 0) return out
  var byDate = {}
  var oldest = rows[0].date
  var newest = rows[0].date
  for (var i = 0; i < rows.length; i++) {
    byDate[rows[i].date] = rows[i]
    if (rows[i].date < oldest) oldest = rows[i].date
    if (rows[i].date > newest) newest = rows[i].date
  }
  var first = dayNumber(win.from !== "" ? win.from : oldest)
  var last = dayNumber(lastDay(win, today, newest))
  out.empty = false
  out.firstDay = first
  out.offset = weekdayOfDay(first)
  out.weeks = Math.floor((last - first + out.offset) / 7) + 1
  var n = out.numbers
  for (var d = first; d <= last; d++) {
    var date = dateOfDay(d)
    var r = byDate[date]
    var total = r ? count(r.total) : 0
    var at = d - first + out.offset
    out.cells.push({ date: date, col: Math.floor(at / 7), row: at % 7, total: total, step: 0,
      sources: r && isObject(r.bySource) ? r.bySource : null })
    if (date.slice(8) === "01" || d === first) out.months.push({ col: Math.floor(at / 7), label: MONTHS[Number(date.slice(5, 7)) - 1] })
    n.events += total
    if (total > 0) n.activeDays++
    if (total > n.max || (total === n.max && total > 0)) {
      n.max = total
      n.busiest = date
    }
  }
  n.days = out.cells.length
  for (var c = 0; c < out.cells.length; c++) out.cells[c].step = colourStep(out.cells[c].total, n.max)
  out.summary = plural(n.events, "event", "events") + " on " + n.activeDays + " of " + plural(n.days, "day", "days")
    + (n.max > 0 ? " · busiest " + n.busiest + " (" + n.max + ")" : "")
  return out
}

// "Wed 2026-10-01 · 30 events · pacman 7 · agent 6 · …" (sources by count,
// then name).
function heatmapCellText(cell) {
  if (!cell) return ""
  var d = utcDate(cell.date)
  var parts = [(d ? WEEKDAYS[d.getUTCDay()].slice(0, 3) + " " : "") + cell.date, plural(cell.total, "event", "events")]
  var src = cell.sources
  if (src) {
    var names = Object.keys(src).filter(function(k) { return count(src[k]) > 0 })
    names.sort(function(a, b) { return count(src[b]) - count(src[a]) || (a < b ? -1 : a > b ? 1 : 0) })
    for (var i = 0; i < names.length; i++) parts.push(names[i] + " " + count(src[names[i]]))
  }
  return parts.join(" · ")
}

// Cell pitch and origin of the heatmap in a w × h canvas, leaving `labelW`
// on the left (weekday labels) and `labelH` on top (months).
function heatmapLayout(w, h, weeks, labelW, labelH) {
  var cols = Math.max(1, weeks)
  var pitch = Math.max(2, Math.floor(Math.min((w - labelW) / cols, (h - labelH) / 7)))
  var gap = Math.max(1, Math.round(pitch / 8))
  return { x0: labelW, y0: labelH, pitch: pitch, cell: pitch - gap, width: cols * pitch - gap, height: 7 * pitch - gap }
}

// The index of the heatmap cell under (x, y), -1 for none.
function heatmapCellAt(chart, layout, x, y) {
  if (!chart || chart.empty) return -1
  var col = Math.floor((x - layout.x0) / layout.pitch)
  var row = Math.floor((y - layout.y0) / layout.pitch)
  if (col < 0 || row < 0 || row > 6 || col >= chart.weeks) return -1
  var i = col * 7 + row - chart.offset
  return i >= 0 && i < chart.cells.length ? i : -1
}

// Series: package counts as step lines over the window (for All: from the
// first sample to today), one lane each for explicit and, when the series
// has it, total. { empty, emptyText, summary, numbers, x0, x1, points:
// [{ date, day, explicit, total }], lanes: [{ key, label, lo, hi, first,
// last }] }. Days are [x0, x1) in day numbers; a lane's [lo, hi] is its
// value range, padded when flat.
function seriesChart(rows, win, today) {
  aggregationRuns++
  var out = emptyChart("series")
  var points = rows.map(function(r) {
    return { date: r.date, day: dayNumber(r.date), explicit: count(r.explicit),
      total: typeof r.total === "number" && isFinite(r.total) ? count(r.total) : null }
  })
  points.sort(function(a, b) { return a.day - b.day })
  if (points.length === 0) return out
  out.empty = false
  out.points = points
  out.x0 = dayNumber(win.from !== "" ? win.from : points[0].date)
  out.x1 = dayNumber(lastDay(win, today, points[points.length - 1].date)) + 1
  var keys = ["explicit", "total"]
  for (var k = 0; k < keys.length; k++) {
    var key = keys[k]
    var have = points.filter(function(p) { return p[key] !== null })
    if (have.length === 0) continue
    var lo = have[0][key]
    var hi = lo
    for (var i = 0; i < have.length; i++) {
      lo = Math.min(lo, have[i][key])
      hi = Math.max(hi, have[i][key])
    }
    out.lanes.push({ key: key, label: key, lo: lo === hi ? lo - 1 : lo, hi: lo === hi ? hi + 1 : hi,
      min: lo, max: hi, first: have[0][key], last: have[have.length - 1][key] })
  }
  var n = out.numbers
  n.samples = points.length
  n.explicitFirst = out.lanes[0].first
  n.explicitLast = out.lanes[0].last
  var total = out.lanes.length > 1 ? out.lanes[1] : null
  if (total) {
    n.totalFirst = total.first
    n.totalLast = total.last
  }
  out.summary = "explicit " + n.explicitFirst + " → " + n.explicitLast
    + (total ? " · total " + n.totalFirst + " → " + n.totalLast : "") + " · " + plural(n.samples, "sample", "samples")
  return out
}

// The sample that holds on `day` (the last one at or before it; the first
// one before the first sample), -1 for an empty chart.
function seriesPointAt(chart, day) {
  if (!chart || chart.empty) return -1
  var at = 0
  for (var i = 0; i < chart.points.length; i++) if (chart.points[i].day <= day) at = i
  return at
}

function seriesPointText(p) {
  if (!p) return ""
  return p.date + " · explicit " + p.explicit + (p.total !== null ? " · total " + p.total : "")
}

// DriftBars: opened and resolved per ISO week, the weeks of the series in
// the window with any gap between two weeks filled with zeros.
// { empty, emptyText, summary, numbers, max, weeks: [{ week, monday,
// opened, resolved }] }.
function driftChart(rows) {
  aggregationRuns++
  var out = emptyChart("driftBars")
  var byMonday = {}
  var mondays = []
  for (var i = 0; i < rows.length; i++) {
    var monday = isoWeekMonday(rows[i].week)
    if (byMonday[monday] === undefined) mondays.push(monday)
    byMonday[monday] = rows[i]
  }
  if (mondays.length === 0) return out
  mondays.sort()
  var n = out.numbers
  var peakOpened = 0
  for (var d = dayNumber(mondays[0]); d <= dayNumber(mondays[mondays.length - 1]); d += 7) {
    var date = dateOfDay(d)
    var r = byMonday[date]
    var week = { week: r ? r.week : isoWeekOf(date), monday: date, opened: r ? count(r.opened) : 0, resolved: r ? count(r.resolved) : 0 }
    out.weeks.push(week)
    n.opened += week.opened
    n.resolved += week.resolved
    // The peak is the week with the most opened (the later one on a tie).
    if (week.opened > 0 && week.opened >= peakOpened) {
      peakOpened = week.opened
      n.peak = week.week
    }
    out.max = Math.max(out.max, week.opened, week.resolved)
  }
  out.empty = false
  n.weeks = out.weeks.length
  n.max = out.max
  out.summary = n.opened + " opened · " + n.resolved + " resolved in " + plural(n.weeks, "week", "weeks")
    + (n.peak !== "" ? " · peak " + n.peak : "")
  return out
}

// "2026-W40 · 28 Sep – 4 Oct · opened 6 · resolved 2"
function driftWeekText(w) {
  if (!w) return ""
  return w.week + " · " + shortDay(w.monday) + " – " + shortDay(addDays(w.monday, 6))
    + " · opened " + w.opened + " · resolved " + w.resolved
}

// RiskDonut: R0–R3 as shares of a full turn, all time (series.risk has no
// dates). { empty, emptyText, summary, numbers, total, parts: [{ risk,
// count, share, from, to }] } with from/to as fractions of the turn.
function riskChart(risk) {
  aggregationRuns++
  var counts = risk || {}
  var out = emptyChart("riskDonut")
  for (var i = 0; i < RISKS.length; i++) {
    out.total += count(counts[RISKS[i]])
    out.numbers[RISKS[i]] = count(counts[RISKS[i]])
  }
  out.numbers.total = out.total
  var at = 0
  for (var k = 0; k < RISKS.length; k++) {
    var part = out.parts[k]
    part.count = count(counts[RISKS[k]])
    part.share = out.total > 0 ? part.count / out.total : 0
    part.from = at
    part.to = at + part.share
    at = part.to
  }
  out.empty = out.total === 0
  out.summary = out.empty ? "RiskDonut: " + out.emptyText
    : plural(out.total, "case", "cases") + " · " + out.parts.map(function(p) { return p.risk + " " + p.count }).join(" · ") + " · all time"
  return out
}

// The part at `fraction` (0 = 12 o'clock, clockwise), -1 for none.
function riskPartAt(chart, fraction) {
  if (!chart || chart.empty) return -1
  var f = ((fraction % 1) + 1) % 1
  for (var i = 0; i < chart.parts.length; i++)
    if (chart.parts[i].count > 0 && f >= chart.parts[i].from && f < chart.parts[i].to) return i
  return -1
}

// "R2 · 4 cases · 50% · all time"
function riskPartText(p) {
  if (!p) return ""
  return p.risk + " · " + plural(p.count, "case", "cases") + " · " + Math.round(p.share * 100) + "% · all time"
}

// A timeline position: the day number plus the wall-clock time of the
// string as a fraction of the day (like clockTime, not converted).
function timelinePos(ts) {
  var day = dayNumber(dayOf(ts))
  if (!isFinite(day)) return NaN
  var m = /^\d{4}-\d{2}-\d{2}T(\d{2}):(\d{2})/.exec(str(ts))
  return m ? day + (Number(m[1]) * 60 + Number(m[2])) / 1440 : day
}

// Timeline: releases, snapshots and crises as markers on one band, case
// spans below it, packed into lanes (an open case runs to the end of today;
// spans are clipped to the window). Days are [x0, x1) in day numbers.
// { empty, emptyText, summary, numbers, x0, x1, lanes, markers: [{ kind,
// x, ts, label, ref }], spans: [{ x0, x1, lane, start, end, open, label,
// ref }], months: [{ day, label }] } (months: the month starts after x0).
// `parsed` (optional, a Map) keeps each row's parsed days across calls.
function timelineChart(rows, win, today, parsed) {
  aggregationRuns++
  var out = emptyChart("timeline")
  if (rows.length === 0) return out
  var todayN = dayNumber(today)
  var lo = Infinity
  var hi = -Infinity
  for (var i = 0; i < rows.length; i++) {
    var r = rows[i]
    var at = parsed ? parsed.get(r) : undefined
    if (at === undefined) {
      at = r.kind === "case" ? { start: dayNumber(r.ts), end: r.end ? dayNumber(r.end) : NaN } : { x: timelinePos(r.ts), marker: null }
      if (parsed) parsed.set(r, at)
    }
    if (r.kind === "case") {
      var start = at.start
      var open = r.end === null || r.end === undefined || r.end === ""
      var end = open ? Math.max(isFinite(todayN) ? todayN : start, start) : at.end
      out.spans.push({ x0: start, x1: end + 1, lane: 0, start: r.ts, end: open ? "" : r.end, open: open,
        label: str(r.label), ref: str(r.ref) })
      lo = Math.min(lo, start)
      hi = Math.max(hi, end + 1)
    } else {
      // Markers do not change with the period: one object for all four.
      var x = at.x
      if (!at.marker) at.marker = { kind: r.kind, x: x, ts: r.ts, label: str(r.label), ref: str(r.ref) }
      out.markers.push(at.marker)
      lo = Math.min(lo, Math.floor(x))
      hi = Math.max(hi, Math.floor(x) + 1)
    }
  }
  out.x0 = win.from !== "" ? dayNumber(win.from) : lo
  out.x1 = win.to !== "" ? dayNumber(win.to) + 1 : Math.max(hi, isFinite(todayN) ? todayN + 1 : hi)
  for (var d = out.x0 + 1; d < out.x1; d++) {
    var date = dateOfDay(d)
    if (date.slice(8) === "01") out.months.push({ day: d, label: MONTHS[Number(date.slice(5, 7)) - 1] })
  }
  // Clip to the window, then pack (interval partitioning, by start, then
  // index order): a span reuses the lane that frees up earliest when that
  // one is free at its start, else opens a new lane. The fewest lanes, in
  // O(n log n) for logbooks with many cases.
  var order = []
  for (var s = 0; s < out.spans.length; s++) {
    var sp = out.spans[s]
    sp.x0 = Math.max(sp.x0, out.x0)
    sp.x1 = Math.min(sp.x1, out.x1)
    order.push(s)
  }
  order.sort(function(a, b) { return out.spans[a].x0 - out.spans[b].x0 || a - b })
  var heap = []
  var lanes = 0
  for (var o = 0; o < order.length; o++) {
    var span = out.spans[order[o]]
    if (heap.length > 0 && heap[0].end <= span.x0) {
      span.lane = heap[0].lane
      heap[0].end = span.x1
      siftDown(heap, 0)
    } else {
      span.lane = lanes++
      heap.push({ end: span.x1, lane: span.lane })
      siftUp(heap, heap.length - 1)
    }
  }
  out.lanes = lanes
  out.empty = false
  var n = out.numbers
  n.cases = out.spans.length
  n.open = out.spans.filter(function(p) { return p.open }).length
  for (var m = 0; m < out.markers.length; m++) {
    var kind = out.markers[m].kind
    if (kind === "release") n.releases++
    else if (kind === "snapshot") n.snapshots++
    else if (kind === "crisis") n.crises++
  }
  n.lanes = out.lanes
  var parts = []
  if (n.cases > 0) parts.push(plural(n.cases, "case", "cases") + (n.open > 0 ? " (" + n.open + " open)" : ""))
  if (n.releases > 0) parts.push(plural(n.releases, "release", "releases"))
  if (n.snapshots > 0) parts.push(plural(n.snapshots, "snapshot", "snapshots"))
  if (n.crises > 0) parts.push(plural(n.crises, "crisis", "crises"))
  out.summary = parts.join(" · ")
  return out
}

// Min-heap of { end, lane } by end, then lane.
function heapLess(a, b) {
  return a.end < b.end || (a.end === b.end && a.lane < b.lane)
}

function siftUp(heap, i) {
  while (i > 0) {
    var parent = (i - 1) >> 1
    if (!heapLess(heap[i], heap[parent])) return
    var t = heap[i]
    heap[i] = heap[parent]
    heap[parent] = t
    i = parent
  }
}

function siftDown(heap, i) {
  for (;;) {
    var l = 2 * i + 1
    var m = i
    if (l < heap.length && heapLess(heap[l], heap[m])) m = l
    if (l + 1 < heap.length && heapLess(heap[l + 1], heap[m])) m = l + 1
    if (m === i) return
    var t = heap[i]
    heap[i] = heap[m]
    heap[m] = t
    i = m
  }
}

// Hover text of a timeline item: "release · Omarchy 4.0.6-1 · 2026-09-15
// 20:13" or "case · C-2026-003 … · 2026-09-26 – open".
function timelineItemText(item) {
  if (!item) return ""
  if (item.kind) return item.kind + " · " + item.label + " · " + dayOf(item.ts) + (clockTime(item.ts) !== "" ? " " + clockTime(item.ts) : "")
  return "case · " + item.label + " · " + item.start + " – " + (item.open ? "open" : item.end)
}

// Rows of the timeline in an h-high canvas: the marker band on top, then
// one row per lane (at most `maxLane` px each).
function timelineLayout(h, lanes, band, maxLane) {
  var laneY0 = band
  var laneH = lanes > 0 ? Math.max(1, Math.min(maxLane, (h - band) / lanes)) : 0
  return { bandY: band / 2, laneY0: laneY0, laneH: laneH }
}

// The timeline item under (px, py) in a w-wide canvas: { kind: "marker" |
// "span", index } or null. Markers are hit within `tolerance` px of their
// position in the band, spans within their lane row (and `tolerance` px
// beyond their ends).
function timelineItemAt(chart, layout, w, px, py, tolerance) {
  if (!chart || chart.empty || w <= 0) return null
  var best = null
  var bestDist = Infinity
  if (py < layout.laneY0) {
    for (var i = 0; i < chart.markers.length; i++) {
      var d = Math.abs(scale(chart.markers[i].x, chart.x0, chart.x1, 0, w) - px)
      if (d <= tolerance && d < bestDist) {
        best = { kind: "marker", index: i }
        bestDist = d
      }
    }
    return best
  }
  if (layout.laneH <= 0) return null
  var lane = Math.floor((py - layout.laneY0) / layout.laneH)
  for (var s = 0; s < chart.spans.length; s++) {
    var sp = chart.spans[s]
    if (sp.lane !== lane) continue
    var a = scale(sp.x0, chart.x0, chart.x1, 0, w)
    var b = scale(sp.x1, chart.x0, chart.x1, 0, w)
    var dist = px < a ? a - px : px > b ? px - b : 0
    if (dist <= tolerance && dist < bestDist) {
      best = { kind: "span", index: s }
      bestDist = dist
    }
  }
  return best
}

// The Plan: the active cases (`cases.active`, no period) with their steps
// and agent. { empty, emptyText, summary, numbers: { cases, done, steps },
// rows: [{ id, title, zone, risk, tone, done, total, progress, stepsText,
// agent }] }.
function planChart(index) {
  aggregationRuns++
  var cases = index && isObject(index.cases) && Array.isArray(index.cases.active) ? index.cases.active : []
  var out = emptyChart("plan")
  for (var i = 0; i < cases.length; i++) {
    var c = cases[i]
    if (!isObject(c)) continue
    var steps = isObject(c.steps) ? c.steps : {}
    var total = count(steps.total)
    var done = Math.min(count(steps.done), total)
    var agent = caseAgents(c)
    out.rows.push({ id: str(c.id), title: str(c.title), zone: str(c.zone), risk: str(c.risk), tone: zoneTone(c.zone),
      done: done, total: total, progress: total > 0 ? done / total : 0,
      stepsText: total > 0 ? done + "/" + total + " steps" : "no steps", agent: agent !== "" ? agent : "no agent" })
    out.numbers.done += done
    out.numbers.steps += total
  }
  out.numbers.cases = out.rows.length
  out.empty = out.rows.length === 0
  out.summary = out.empty ? "The Plan: " + out.emptyText
    : plural(out.rows.length, "active case", "active cases") + " · " + out.numbers.done + " of " + out.numbers.steps + " steps done"
  return out
}

// Columns of plan cards that fit `width` with cards of at least `minWidth`.
function planColumns(width, minWidth, gap, cards) {
  var fit = Math.max(1, Math.floor((Math.max(0, width) + gap) / (Math.max(1, minWidth) + gap)))
  return Math.max(1, Math.min(fit, Math.max(1, cards)))
}

// One period of a periodTable(), the default one for an unknown id. Without
// a table (the shell creates the overlay first and injects `service` after,
// SPEC-PLUGIN §6) it is emptyPeriodView's: no aggregation pass.
function periodView(table, period) {
  var periods = table && isObject(table.periods) ? table.periods : {}
  return periods[periodById(period).id] || emptyPeriodView(period)
}

// A period of periodTable(null), built from the empty shapes without any
// aggregation pass and kept per period, so an overlay without a service
// costs nothing and aggregationCount() stays where it was.
var emptyPeriodViews = {}
function emptyPeriodView(period) {
  var id = periodById(period).id
  if (emptyPeriodViews[id]) return emptyPeriodViews[id]
  var plan = emptyChart("plan")
  var data = { heatmap: [], packages: [], drift: [], timeline: [], risk: riskCounts(null), plan: plan }
  var charts = {}
  for (var i = 0; i < OVERLAY_SLOTS.length; i++) charts[OVERLAY_SLOTS[i].id] = emptyChart(OVERLAY_SLOTS[i].id)
  charts.plan = plan
  var slots = OVERLAY_SLOTS.map(function(slot) { return slotSummary(slot, data) })
  delete data.plan
  emptyPeriodViews[id] = { window: periodWindow(id, ""), series: data, charts: charts, slots: slots }
  return emptyPeriodViews[id]
}

// "30 d · 2026-09-02 – 2026-10-01", or "All · everything in the index".
function periodCaption(win) {
  var p = periodById(win ? win.period : "")
  if (!win || win.from === "") return p.label + " · everything in the index"
  return p.label + " · " + win.from + " – " + win.to
}

// The header's second line: machine · Omarchy version · generated time.
function overlayMeta(index) {
  if (!index) return ""
  var parts = []
  var logbook = isObject(index.logbook) ? index.logbook : {}
  if (hasText(logbook.machine)) parts.push(logbook.machine)
  var omarchy = isObject(index.system) && isObject(index.system.omarchy) ? index.system.omarchy : {}
  if (hasText(omarchy.version)) parts.push("Omarchy " + omarchy.version)
  if (dayOf(index.generatedAt) !== "") parts.push("generated " + dayOf(index.generatedAt) + " " + clockTime(index.generatedAt))
  return parts.join(" · ")
}

// The overlay's banner: the service's banner, with only the fix that runs
// no engine command (copying it); the overlay runs no engine command (WP-030).
function overlayBanner(banner) {
  if (!banner) return null
  var out = {}
  for (var k in banner) out[k] = banner[k]
  out.actions = (banner.actions || []).filter(function(a) { return a.id === "copy" })
  return out
}

// Slot geometry on a 12-column grid (SPEC-PLUGIN §6) inside `width` ×
// `height`, `gap` between cells. The column count follows the width:
//   wide   (≥ 3 slots of minWidth): Heatmap | Series DriftBars RiskDonut | Timeline | The Plan
//   medium (≥ 2):                  Heatmap | Series DriftBars | RiskDonut Timeline | The Plan
//   narrow:                        one slot per row
// Rows share the height by weight and never get less than minHeight: a row
// that would gets minHeight and the others share the rest by weight. Only
// when the minimum heights alone do not fit does contentHeight exceed height
// and the grid scroll.
// Returns { mode, contentHeight, slots: [{ id, x, y, w, h }] } in grid order.
var GRID_ROWS = {
  wide: { weights: [3, 4, 2, 2], rows: [[["heatmap", 12]], [["series", 4], ["driftBars", 4], ["riskDonut", 4]], [["timeline", 12]], [["plan", 12]]] },
  medium: { weights: [3, 4, 3, 2], rows: [[["heatmap", 12]], [["series", 6], ["driftBars", 6]], [["riskDonut", 4], ["timeline", 8]], [["plan", 12]]] },
  narrow: { weights: [3, 3, 3, 3, 2, 2], rows: [[["heatmap", 12]], [["series", 12]], [["driftBars", 12]], [["riskDonut", 12]], [["timeline", 12]], [["plan", 12]]] }
}

// Heights for rows of `weights` sharing `free`, each at least minH: rows
// whose share falls below minH are fixed at it, the rest share what is left
// by weight. When every row is fixed, the sum exceeds `free` (scrolling).
function rowHeights(weights, free, minH) {
  var fixed = weights.map(function() { return false })
  for (;;) {
    var left = free
    var weightLeft = 0
    for (var i = 0; i < weights.length; i++) {
      if (fixed[i]) left -= minH
      else weightLeft += weights[i]
    }
    var changed = false
    for (var k = 0; k < weights.length; k++) {
      if (!fixed[k] && (weightLeft <= 0 || Math.floor(left * weights[k] / weightLeft) < minH)) {
        fixed[k] = true
        changed = true
      }
    }
    if (!changed) {
      // Rounding leftovers go to the heaviest free row, so the rows fill
      // `free` exactly.
      var out = weights.map(function(wt, j) { return fixed[j] ? minH : Math.floor(left * wt / weightLeft) })
      var heaviest = -1
      for (var r = 0; r < weights.length; r++)
        if (!fixed[r] && (heaviest === -1 || weights[r] > weights[heaviest])) heaviest = r
      if (heaviest !== -1) out[heaviest] += free - out.reduce(function(a, b) { return a + b }, 0)
      return out
    }
  }
}

function overlayGrid(width, height, gap, minWidth, minHeight) {
  var w = Math.max(0, Math.floor(Number(width) || 0))
  var h = Math.max(0, Math.floor(Number(height) || 0))
  var g = Math.max(0, Math.floor(Number(gap) || 0))
  var minW = Math.max(1, Number(minWidth) || 1)
  var minH = Math.max(1, Math.floor(Number(minHeight) || 1))
  var mode = w >= 3 * minW + 2 * g ? "wide" : w >= 2 * minW + g ? "medium" : "narrow"
  var spec = GRID_ROWS[mode]
  var colW = (w - 11 * g) / 12
  var free = h - (spec.rows.length - 1) * g
  var heights = rowHeights(spec.weights, free, minH)
  var slots = []
  var y = 0
  for (var r = 0; r < spec.rows.length; r++) {
    var col = 0
    for (var c = 0; c < spec.rows[r].length; c++) {
      var span = spec.rows[r][c][1]
      var x = Math.floor(col * (colW + g))
      var last = col + span === 12
      var right = last ? w : Math.floor((col + span) * (colW + g) - g)
      slots.push({ id: spec.rows[r][c][0], x: x, y: y, w: Math.max(0, right - x), h: heights[r] })
      col += span
    }
    y += heights[r] + (r < spec.rows.length - 1 ? g : 0)
  }
  return { mode: mode, contentHeight: y, slots: slots }
}

// ---- Engine-call failures, busy guards, engine minimum (WP-068) -------------

// What a one-at-a-time guard (Service.plan, startAgent, drift, decide)
// answers the sheet whose call it refused.
var BUSY_TEXT = "Another action is running — try again in a moment"

// Exit 4 (lock held) of a capture or status: try again after LOCK_RETRY_MS,
// at most LOCK_RETRIES times, and say so in a neutral text meanwhile.
var LOCK_RETRY_MS = 30000
var LOCK_RETRIES = 3
var LOCK_WAIT_TEXT = "waiting for another seldon process; trying again shortly"

// The journal line for an engine call that exited above 0:
// "jax.seldon: seldon <command> exit <code>: <first stderr line>", or the
// engine's JSON error message when stderr is empty (`--json` errors go to
// stdout), cut to its first line too (WP-078). "" for exit 0. Plain text on
// one line.
function callWarning(args, exitCode, stdoutText, stderrText) {
  if (!(exitCode > 0)) return ""
  var head = Array.isArray(args) && args.length > 0 ? String(args[0]) : "?"
  var first = firstLine(stderrText)
  if (first === "") first = firstLine(engineError(stdoutText, "", exitCode))
  if (first === "") first = "seldon exited with code " + exitCode
  return "jax.seldon: seldon " + head + " exit " + exitCode + ": " + first
}

// The first line of `text` that is not blank, trimmed; "" when none is.
function firstLine(text) {
  var lines = String(text || "").split("\n")
  for (var i = 0; i < lines.length; i++) {
    var line = lines[i].trim()
    if (line !== "") return line
  }
  return ""
}

// "0.1.10" → [0, 1, 10]; a pre-release or build suffix is ignored, so a
// dev build of a version counts as that version. null when not readable.
function versionCore(text) {
  var m = /^v?([0-9]+)\.([0-9]+)\.([0-9]+)(?:[-+].*)?$/.exec(String(text || "").trim())
  return m ? [Number(m[1]), Number(m[2]), Number(m[3])] : null
}

// True only when both versions are readable and `version` < `min`.
function versionBelow(version, min) {
  var v = versionCore(version)
  var m = versionCore(min)
  if (!v || !m) return false
  for (var i = 0; i < 3; i++) if (v[i] !== m[i]) return v[i] < m[i]
  return false
}

// `seldon.engineMin` of the manifest the shell injects; "" without one.
function engineMinOf(manifest) {
  var s = isObject(manifest) && isObject(manifest.seldon) ? manifest.seldon : null
  return s && typeof s.engineMin === "string" ? s.engineMin : ""
}

// The "Index format mismatch" family's engine variant: the engine answers,
// but older than the manifest's engineMin (docs/VERSIONING.md). Shown in
// place of every status banner but engineMissing and contractMismatch (each
// names its own fix); null when the engine is new enough or either version
// is unknown.
function engineOutdatedBanner(status, engineVersion, engineMin) {
  if (status === "engineMissing" || status === "contractMismatch") return null
  if (!versionBelow(engineVersion, engineMin)) return null
  return {
    status: "engineOutdated",
    tone: "urgent",
    title: "Engine too old",
    detail: "This plugin needs engine " + engineMin + " or newer; seldon reports " + engineVersion
      + ". Update the engine to at least " + engineMin + ", then check again.",
    command: UPDATE_ENGINE_COMMAND,
    actions: [
      { id: "terminal", label: "Update in terminal" },
      { id: "copy", label: "Copy" },
      { id: "recheck", label: "Check again" }
    ]
  }
}

// ---- Plugin updated under a running shell (WP-090) --------------------------

// Quickshell 0.3.1 has no Qt.clearComponentCache, so the shell's plugin
// reload after `omarchy plugin update` re-creates the plugin from the code
// it compiled first; only a restart loads the new code
// (memory/omarchy-shell.md). The notice's one action runs this argument
// list, no arguments, no shell string.
var RESTART_SHELL_ARGV = ["omarchy-restart-shell"]

// `version` of the manifest the shell injects; "" without one.
function pluginVersionOf(manifest) {
  return isObject(manifest) && typeof manifest.version === "string" ? manifest.version : ""
}

// The neutral notice when the manifest on disk names another version than
// the running code (PLUGIN_VERSION): the plugin was updated, the shell
// still runs the old code. null when they agree or the manifest is not
// known yet. Its one action, "restart", is dispatched by
// Service.fix("restart", "restart") and runs RESTART_SHELL_ARGV.
function restartShellNotice(runningVersion, manifestVersion) {
  if (manifestVersion === "" || manifestVersion === runningVersion) return null
  return {
    status: "restartShell",
    tone: "neutral",
    title: "Restart the shell to finish the update",
    detail: "Seldon " + manifestVersion + " is installed, but the shell still runs " + runningVersion
      + ". The shell loads new plugin code only when it restarts.",
    command: RESTART_SHELL_ARGV.join(" "),
    actions: [{ id: "restart", label: "Restart shell" }],
    hint: ""
  }
}

// ---- Bar IPC owner (WP-078) -------------------------------------------------

// A bar widget the user can see: visible and not zero-size. In the bar's
// centre section (once a centre anchor is set, the default) every module is
// mounted twice, the drawn copy and a zero-size, hidden placeholder (the
// shell's BarModel.isDrawnSlot, on the widget itself).
function isDrawnWidget(item) {
  return !!item && item.visible === true && item.width > 0 && item.height > 0
}

// The instance that owns `jax.seldon.panel` among the bar's live widgets
// (`bar.moduleWidgets`), `leaving` left out: the first drawn one, else the
// first one at all (only placeholders on screen), else null. Mirrors the
// shell's BarModel.pickDrawnSlot, so IPC acts on the instance a panel
// hotkey would.
function pickDrawnWidget(items, leaving) {
  var placeholder = null
  var list = Array.isArray(items) ? items : []
  for (var i = 0; i < list.length; i++) {
    var item = list[i]
    if (!item || item === leaving) continue
    if (isDrawnWidget(item)) return item
    if (!placeholder) placeholder = item
  }
  return placeholder
}

// ---- Capture warnings (WP-085) ----------------------------------------------

// The `warnings` of a `capture --json` answer: its strings that are not
// blank, unchanged (the engine's text, its restore hint included). [] when
// there are none or the answer has no such list (an engine before 0.1.2).
function captureWarnings(data) {
  var list = isObject(data) && Array.isArray(data.warnings) ? data.warnings : []
  var out = []
  for (var i = 0; i < list.length; i++)
    if (typeof list[i] === "string" && list[i].trim() !== "") out.push(list[i])
  return out
}

// The neutral notice under the status banners for the warnings of the last
// capture the plugin ran that exited 0 (Service.captureWarnings), or null
// when it had none. `detail` is the first line of each warning, one per
// line; `full` is every warning in full, for the hover, and "" when the
// detail already shows all of it. No action: no fixed page of the user
// guide ships with the plugin, and the engine's text names the guide's
// section. Banner.qml renders it like a status banner, in the neutral tone.
var CAPTURE_WARNED_TITLE = "Capture warned"

function captureWarningNotice(warnings) {
  var list = Array.isArray(warnings) ? warnings : []
  if (list.length === 0) return null
  var firsts = list.map(firstLine)
  var detail = firsts.join("\n")
  var full = list.map(function(w) { return String(w).trim() }).join("\n\n")
  return {
    status: "captureWarned",
    tone: "neutral",
    title: CAPTURE_WARNED_TITLE,
    detail: detail,
    full: full === firsts.join("\n\n") ? "" : full,
    command: "",
    actions: [],
    hint: ""
  }
}

// ---- Desk (ADR-0034) ----------------------------------------------------------

// The desk's nine targets in sidebar order: eight sections with their fixed
// digit, then Settings on `,`. `wp` names the work package that fills a
// section the shell (WP-121) only stubs; `solo` sections have no list column
// (§2). `icon` is a 24-unit SVG path, drawn in the theme colour
// (components/desk/NavIcon.qml), from the approved prototype.
var DESK_SECTIONS = [
  { id: "today", label: "Today", key: "1", wp: "WP-122", solo: false,
    icon: "M4 5h16v15H4zM4 9h16M9 3v4M15 3v4" },
  { id: "changelog", label: "Changelog", key: "2", wp: "WP-122", solo: false,
    icon: "M5 6h14M5 12h14M5 18h9" },
  { id: "work", label: "Work", key: "3", wp: "WP-122", solo: false,
    icon: "M4 8h16v11H4zM9 8V5h6v3" },
  { id: "decisions", label: "Decisions", key: "4", wp: "WP-123", solo: false,
    icon: "M6 4h9l4 4v12H6zM14 4v5h5" },
  { id: "system", label: "System", key: "5", wp: "WP-123", solo: false,
    icon: "M4 5h16v11H4zM8 20h8M12 16v4" },
  { id: "memory", label: "Memory", key: "6", wp: "WP-123", solo: false,
    icon: "M7 4h10a2 2 0 0 1 2 2v14l-7-4-7 4V6a2 2 0 0 1 2-2z" },
  { id: "radiant", label: "Prime Radiant", key: "7", wp: "WP-123", solo: true,
    icon: "M12 2l2 7 7 3-7 3-2 7-2-7-7-3 7-3z" },
  { id: "graph", label: "Graph", key: "8", wp: "WP-125", solo: true,
    icon: "M4 6a2 2 0 1 0 4 0a2 2 0 1 0 -4 0M16 7a2 2 0 1 0 4 0a2 2 0 1 0 -4 0M10 17a2 2 0 1 0 4 0a2 2 0 1 0 -4 0M7 8l4 7M17 9l-4 6M8 6h8" },
  { id: "settings", label: "Settings", key: ",", wp: "", solo: false,
    icon: "M12 8a4 4 0 1 0 0 8a4 4 0 1 0 0-8zM4 12h2M18 12h2M12 4v2M12 18v2" }
]
var DESK_SECTION_DEFAULT = "today"
// The sidebar's fold button.
var DESK_COLLAPSE_ICON = "M15 6l-6 6 6 6"
var DESK_EXPAND_ICON = "M9 6l6 6-6 6"

// The bar-widget settings of the desk (manifest barWidget.defaults/schema).
var DESK_WIDTH_DEFAULT = 100
var DESK_WIDTH_MIN = 50
var DESK_WIDTH_MAX = 100
var DESK_WIDTH_PRESETS = [50, 67, 75, 100]
var DESK_SIDEBAR_MODES = ["open", "collapsed"]
var DESK_SIDEBAR_DEFAULT = "open"
// §1: never narrower than this (unless the screen is); below it the sidebar
// shows icons only, below DESK_STACK_WIDTH list and detail stack.
var DESK_MIN_WIDTH = 960
var DESK_STACK_WIDTH = 760
var DESK_REFUSED_TEXT = "Change it in Omarchy's bar settings (Seldon widget)."
// The plugin is enabled but not in the bar: the shell has no entry to keep
// the desk's settings in (Service.entryKnown).
var DESK_NO_ENTRY_TEXT = "Add Seldon to the bar to keep this setting; until then it holds until the shell restarts."

function deskSection(id) {
  for (var i = 0; i < DESK_SECTIONS.length; i++)
    if (DESK_SECTIONS[i].id === id) return DESK_SECTIONS[i]
  return null
}

function deskSectionIndex(id) {
  for (var i = 0; i < DESK_SECTIONS.length; i++)
    if (DESK_SECTIONS[i].id === id) return i
  return -1
}

// The section a typed character selects ("1"–"8", ","), or "".
function deskSectionForKey(text) {
  for (var i = 0; i < DESK_SECTIONS.length; i++)
    if (DESK_SECTIONS[i].key === text) return DESK_SECTIONS[i].id
  return ""
}

// Alt+↑/↓: the previous or next of the nine targets, wrapping.
function deskCycle(id, delta) {
  var n = DESK_SECTIONS.length
  var i = deskSectionIndex(id)
  if (i === -1) i = 0
  return DESK_SECTIONS[(((i + delta) % n) + n) % n].id
}

// The setting `deskWidth` as an integer per cent in 50–100; anything else
// (missing, a hand-edited shell.json) is the default.
function clampDeskWidth(value) {
  if (value === null || value === undefined || value === "") return DESK_WIDTH_DEFAULT
  var n = Math.round(Number(value))
  if (!isFinite(n)) return DESK_WIDTH_DEFAULT
  return Math.max(DESK_WIDTH_MIN, Math.min(DESK_WIDTH_MAX, n))
}

function deskSidebarMode(value) {
  return DESK_SIDEBAR_MODES.indexOf(value) !== -1 ? value : DESK_SIDEBAR_DEFAULT
}

// The desk inside its window (ADR-0034 §1). The window covers the screen's
// usable area; `gap` is Style.gapsOut on every side. The width is
// clamp(round(avail × pct / 100), min(960, avail), avail), centred; at
// 100 % it fills the row like a left/right-anchored surface.
function deskGeometry(windowWidth, windowHeight, pct, gap) {
  var g = Math.max(0, Math.round(Number(gap) || 0))
  var availW = Math.max(0, Math.floor(Number(windowWidth) || 0) - 2 * g)
  var availH = Math.max(0, Math.floor(Number(windowHeight) || 0) - 2 * g)
  var want = Math.round(availW * clampDeskWidth(pct) / 100)
  var w = Math.max(Math.min(DESK_MIN_WIDTH, availW), Math.min(availW, want))
  return { x: g + Math.floor((availW - w) / 2), y: g, w: w, h: availH, avail: availW }
}

// The columns at a desk width. `sizes`: { sidebar, icons, listMin, listMax }
// in pixels (Style.space of the prototype's 210 / 56 / 260 / 360).
// The thresholds are pixels of the desk itself.
function deskLayout(width, sidebarPref, solo, sizes) {
  var w = Math.max(0, Number(width) || 0)
  var forced = w < DESK_MIN_WIDTH
  var icons = forced || deskSidebarMode(sidebarPref) === "collapsed"
  var stacked = !solo && w < DESK_STACK_WIDTH
  var sidebarW = icons ? sizes.icons : sizes.sidebar
  var rest = Math.max(0, w - sidebarW)
  var listW = solo ? 0 : stacked ? rest : Math.max(sizes.listMin, Math.min(sizes.listMax, Math.round(rest * 0.3)))
  return {
    sidebar: icons ? "icons" : "open",
    forced: forced,
    stacked: stacked,
    solo: !!solo,
    sidebarW: sidebarW,
    listW: listW,
    detailW: solo ? rest : stacked ? rest : Math.max(0, rest - listW)
  }
}

// The desk's open(payloadJson) (§4, §7): {"section": id, "select": id,
// "filter": source, "period": p}. A `period` without a section implies the
// Prime Radiant (the old overlay payload). Unknown sections are dropped.
function deskPayload(payloadJson) {
  var data = parseJson(payloadJson) || {}
  var out = { section: "", select: "", filter: "", period: "" }
  if (deskSection(data.section)) out.section = data.section
  if (isPeriod(data.period)) out.period = data.period
  if (out.section === "" && out.period !== "") out.section = "radiant"
  if (typeof data.select === "string") out.select = data.select
  if (typeof data.filter === "string") out.filter = data.filter
  return out
}

// The header's KPI strip (§2): active · verification · queued · crises ·
// attention. `tone`: "accent" for active cases, "urgent" for crises, ""
// otherwise. Empty before an index is shown.
function deskKpis(index) {
  if (!index) return []
  var c = counts(index) || { active: 0, queued: 0, crisis: 0, attention: 0 }
  var cases = isObject(index.cases) ? index.cases : {}
  var verification = Array.isArray(cases.verification) ? cases.verification.length : 0
  return [
    { id: "active", label: "active", value: c.active, tone: c.active > 0 ? "accent" : "" },
    { id: "verification", label: "verification", value: verification, tone: "" },
    { id: "queued", label: "queued", value: c.queued, tone: "" },
    { id: "crises", label: "crises", value: c.crisis, tone: c.crisis > 0 ? "urgent" : "" },
    { id: "attention", label: "attention", value: c.attention, tone: "" }
  ]
}

// The counts at the right of the sidebar rows: { <section id>: { text,
// tone } }. Today: events today; Changelog: open changes (urgent with a
// crisis); Work: active · verification · queued; Decisions: proposed;
// Memory: lessons and topics. Empty text hides the count.
function deskCounts(index) {
  var out = {}
  for (var i = 0; i < DESK_SECTIONS.length; i++) out[DESK_SECTIONS[i].id] = { text: "", tone: "" }
  if (!index) return out
  var summary = isObject(index.summary) ? index.summary : {}
  var c = counts(index) || { active: 0, queued: 0, drift: 0, crisis: 0 }
  out.today = { text: String(count(summary.eventsToday)), tone: "" }
  if (c.drift > 0) out.changelog = { text: String(c.drift), tone: c.crisis > 0 ? "urgent" : "" }
  var cases = isObject(index.cases) ? index.cases : {}
  var verification = Array.isArray(cases.verification) ? cases.verification.length : 0
  if (c.active + verification + c.queued > 0)
    out.work = { text: c.active + " · " + verification + " · " + c.queued, tone: "" }
  var proposed = (Array.isArray(index.decisions) ? index.decisions : []).filter(function(d) {
    return isObject(d) && d.status === "proposed"
  }).length
  if (proposed > 0) out.decisions = { text: proposed + " new", tone: "" }
  var memory = isObject(index.memory) ? index.memory : {}
  var notes = (Array.isArray(memory.lessons) ? memory.lessons.length : 0) + (Array.isArray(memory.topics) ? memory.topics.length : 0)
  if (notes > 0) out.memory = { text: String(notes), tone: "" }
  return out
}

// The header's second line: machine · Omarchy version · captured N ago.
function deskSubline(index, lastCaptureText, nowMs) {
  var parts = []
  if (index) {
    var logbook = isObject(index.logbook) ? index.logbook : {}
    if (hasText(logbook.machine)) parts.push(logbook.machine)
    var omarchy = isObject(index.system) && isObject(index.system.omarchy) ? index.system.omarchy : {}
    if (hasText(omarchy.version)) parts.push("Omarchy " + omarchy.version)
  }
  if (hasText(lastCaptureText)) parts.push("captured " + relativeAge(timeMs(lastCaptureText), nowMs))
  return parts.join(" · ")
}

// The settings the desk writes through the shell facade's
// updateEntryInline, which replaces the plugin's shell.json entry with
// { id } + settings: every key of the current entry (unknown ones too, the
// id left out) and the one that changes. null when the value is already
// stored (the facade would report "nothing changed" as false, which the
// desk must not read as a refusal).
function deskSettingsWrite(entry, key, value) {
  var next = {}
  var current = isObject(entry) ? entry : {}
  for (var k in current) if (k !== "id") next[k] = current[k]
  if (next[key] === value) return null
  next[key] = value
  return next
}

// The screen the desk opens on: the index in `names` of Hyprland's focused
// monitor, else 0 (the first screen).
function pickScreen(names, focused) {
  var list = Array.isArray(names) ? names : []
  var i = hasText(focused) ? list.indexOf(focused) : -1
  return i === -1 ? 0 : i
}

// Settings › Appearance: "1728 px on this screen".
function deskWidthPreview(windowWidth, pct, gap) {
  return deskGeometry(windowWidth, 0, pct, gap).w + " px on this screen"
}

// The label of a width preset chip.
function deskPresetLabel(pct) {
  return pct === 100 ? "Full" : pct + " %"
}

// The old `jax.seldon.panel` tab names and their sections (the shim, §7).
var PANEL_TABS = ["today", "changelog", "work", "decisions", "system", "memory"]
