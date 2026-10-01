// Pure helpers for jax.seldon: index parsing, status derivation, pill and
// banner text, and validation of engine argument lists.
//
// Nothing here touches Qt, files or processes, so the same file runs under
// node (tests/plugin/model.test.js). Service.qml owns all I/O; BarWidget.qml,
// Panel.qml and components/Banner.qml only render what these functions return.

var CONTRACT_VERSION = 1

// SPEC-PLUGIN §3: an index older than two hours is stale.
var STALE_AFTER_MS = 2 * 60 * 60 * 1000

// ADR-0005: capture at shell start and every 15 minutes.
var CAPTURE_INTERVAL_MIN_DEFAULT = 15
var CAPTURE_INTERVAL_MIN_MIN = 5
var CAPTURE_INTERVAL_MIN_MAX = 120

// The service's `status` (named so because `state` clashes with Item.state).
var STATUSES = ["ok", "engineMissing", "notInitialised", "indexMissing", "indexStale", "contractMismatch"]

// Patterns from schema/event.schema.json $defs.
var CASE_ID = /^C-[0-9]{4}-[0-9]{3,}$/
var EVENT_ID = /^[0-7][0-9A-HJKMNP-TV-Z]{25}$/
var ZONES = ["green", "yellow", "red"]
var RISKS = ["R0", "R1", "R2", "R3"]
var OPEN_TARGETS = ["journal", "ledger", "status"]

// Fix commands shown in banners. Constants only: nothing from the index is
// ever spliced into a command (AGENTS.md §8). The engine is an AUR package
// (ADR-0004); `omarchy pkg add` only reaches the official repositories, so
// the AUR variant is the one that works.
var INSTALL_ENGINE_COMMAND = "omarchy pkg aur add jax-seldon"
var UPDATE_ENGINE_COMMAND = "yay -S jax-seldon"
var UPDATE_PLUGIN_COMMAND = "omarchy plugin update jax.seldon"
var INIT_COMMAND = "seldon init"

var GLYPH = "⟡"

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

// Counts the pill and tooltip need, or null without an index.
function counts(index) {
  if (!index || !isObject(index.summary)) return null
  var summary = index.summary
  return {
    active: count(summary.activeCases),
    queued: count(summary.queuedCases),
    drift: count(summary.openDrift),
    crisis: count(summary.crisis)
  }
}

function lastCapture(index) {
  return index && isObject(index.state) && typeof index.state.lastCapture === "string"
    ? index.state.lastCapture : ""
}

// ---- Pill -------------------------------------------------------------------

// SPEC-PLUGIN §4: `⟡ A · D`, parts hidden when 0: `⟡`, `⟡ 2`, `⟡ · 3`.
function pillText(c) {
  var parts = [GLYPH]
  if (c && c.active > 0) parts.push(String(c.active))
  if (c && c.drift > 0) parts.push("· " + c.drift)
  return parts.join(" ")
}

// "urgent" when any crisis, "accent" when cases are active, else "default".
function pillTone(c) {
  if (c && c.crisis > 0) return "urgent"
  if (c && c.active > 0) return "accent"
  return "default"
}

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

// SPEC-PLUGIN §4: "Seldon — 2 active cases, 3 unexplained changes, last
// capture 4 min ago", with the problem appended when the status is not ok.
function tooltipText(status, c, lastCaptureText, nowMs) {
  var problem = statusPhrase(status)
  if (!c || !showsCounts(status)) return "Seldon — " + (problem !== "" ? problem : "loading")
  var drift = plural(c.drift, "unexplained change", "unexplained changes")
  if (c.crisis > 0) drift += " (" + c.crisis + " in the red zone)"
  var captured = lastCaptureText
    ? "last capture " + relativeAge(timeMs(lastCaptureText), nowMs)
    : "never captured"
  var text = "Seldon — " + plural(c.active, "active case", "active cases") + ", " + drift + ", " + captured
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
      detail: "The plugin needs the seldon command. Install it, then check again.",
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
        { id: "copy", label: "Copy" }
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

// Validate an argument list against the commands of CONTRACT.md ("Commands
// the plugin may run"). Returns "" when allowed, else the reason. Every
// command may end in --json; ids must match their schema pattern; free text
// is one argument.
function validateArgs(args) {
  if (!Array.isArray(args) || args.length === 0) return "empty argument list"
  for (var i = 0; i < args.length; i++) {
    if (typeof args[i] !== "string") return "argument " + i + " is not a string"
    if (args[i].indexOf("\u0000") !== -1) return "argument " + i + " contains NUL"
  }
  var a = args.slice()
  if (a.length > 1 && a[a.length - 1] === "--json") a.pop()
  var n = a.length
  var text = function(i) { return typeof a[i] === "string" && a[i] !== "" }
  switch (a[0]) {
  case "--version":
  case "status":
  case "rebuild":
  case "update-impact":
    return n === 1 ? "" : a[0] + " takes no arguments"
  case "capture":
    // CONTRACT.md writes it `capture --all --json --quiet`.
    if (args.length === 4 && args[1] === "--all" && args[2] === "--json" && args[3] === "--quiet") return ""
    return n === 3 && a[1] === "--all" && a[2] === "--quiet" ? "" : "capture must be: capture --all --json --quiet"
  case "log":
    if (n === 2 && text(1)) return ""
    if (n === 4 && text(1) && a[2] === "--case" && CASE_ID.test(a[3])) return ""
    return "log must be: log <text> [--case <caseId>]"
  case "plan":
    if (n === 7 && a[1] === "new" && text(2) && a[3] === "--zone" && matches(ZONES, a[4])
        && a[5] === "--risk" && matches(RISKS, a[6])) return ""
    if (n === 3 && matches(["start", "verify", "done", "drop"], a[1]) && CASE_ID.test(a[2])) return ""
    return "plan must be: plan new <title> --zone <z> --risk <r> | plan start|verify|done|drop <caseId>"
  case "drift":
    if (n === 4 && a[1] === "link" && EVENT_ID.test(a[2]) && CASE_ID.test(a[3])) return ""
    if (n === 4 && a[1] === "explain" && EVENT_ID.test(a[2]) && text(3)) return ""
    if (n === 5 && a[1] === "dismiss" && EVENT_ID.test(a[2]) && a[3] === "--reason" && text(4)) return ""
    return "drift must be: drift link <eventId> <caseId> | explain <eventId> <text> | dismiss <eventId> --reason <text>"
  case "decide":
    return n === 3 && text(1) && a[2] === "--no-edit" ? "" : "decide must be: decide <title> --no-edit"
  case "open":
    return n === 3 && (matches(OPEN_TARGETS, a[1]) || CASE_ID.test(a[1])) && a[2] === "--editor"
      ? "" : "open must be: open journal|ledger|status|<caseId> --editor"
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
