// Pure helpers for jax.seldon: index parsing, status derivation, pill and
// banner text, and validation of engine argument lists.
//
// Nothing here touches Qt, files or processes, so the same file runs under
// node (tests/plugin/model.test.js). Service.qml owns all I/O; BarWidget.qml,
// Panel.qml and components/ only render what these functions return,
// including the rows of the Today, Changelog and System tabs.

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
// ADR-0011: the one-time opt-in that lets the snapper collector read
// snapshots. `$USER` is expanded by the shell the user pastes it into (or by
// the terminal launcher's bash -c); nothing else in it varies.
var SNAPPER_FIX_COMMAND = "sudo snapper -c root set-config ALLOW_USERS=$USER SYNC_ACL=yes"

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
    if (withText && n === 6 && a[1] === "new" && a[2] === "--zone" && matches(ZONES, a[3])
        && a[4] === "--risk" && matches(RISKS, a[5])) return ""
    if (!withText && n === 3 && matches(["start", "verify", "done", "drop"], a[1]) && CASE_ID.test(a[2])) return ""
    return "plan must be: plan new --zone <z> --risk <r> -- <title> | plan start|verify|done|drop <caseId>"
  case "drift":
    var id = n >= 3 && EVENT_ID.test(a[2])
    if (id && !withText && a[1] === "link" && n >= 4 && CASE_ID.test(a[3]) && (n === 4 || only(4))) return ""
    // dismiss takes its reason like explain its text, after `--` (WP-011
    // review: the contract form replacing `--reason <text>`).
    if (id && withText && (a[1] === "explain" || a[1] === "dismiss") && (n === 3 || only(3))) return ""
    if (id && !withText && a[1] === "show" && n === 3 && json) return ""
    return "drift must be: drift link <eventId> <caseId> [--only] | explain|dismiss <eventId> [--only] -- <text>"
      + " | show <eventId> --json"
  case "decide":
    return withText && n === 2 && a[1] === "--no-edit" ? "" : "decide must be: decide --no-edit -- <title>"
  case "open":
    return !withText && n === 3 && (matches(OPEN_TARGETS, a[1]) || CASE_ID.test(a[1])) && a[2] === "--editor"
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

// CONTRACT.md rule 1: ${XDG_STATE_HOME:-$HOME/.local/state}/seldon/index.json.
// The XDG spec says a relative XDG_STATE_HOME is invalid and is ignored.
function stateIndexPath(xdgStateHome, home) {
  var base = String(xdgStateHome || "")
  if (base.charAt(0) !== "/") base = String(home || "") + "/.local/state"
  return base.replace(/\/+$/, "") + "/seldon/index.json"
}

// ---- Panel: strips and banners under the status banner ----------------------

// SPEC-PLUGIN §5: the red strip "N changes in the red zone need a reason".
// Empty when nothing is in the red zone.
function crisisText(index) {
  var c = counts(index)
  if (!c || c.crisis === 0) return ""
  return plural(c.crisis, "change", "changes") + " in the red zone " + (c.crisis === 1 ? "needs" : "need") + " a reason"
}

function collectors(index) {
  return index && isObject(index.state) && Array.isArray(index.state.collectors) ? index.state.collectors : []
}

// ADR-0011: snapper runs degraded until the user opts in. The banner shows the
// engine's message as plain text and offers the constant fix. Action ids are
// dispatched by Service.fix(actionId, "snapper").
function snapperBanner(index) {
  var list = collectors(index)
  for (var i = 0; i < list.length; i++) {
    var c = list[i]
    if (!isObject(c) || c.name !== "snapper" || c.enabled !== true || c.ok !== false) continue
    return {
      status: "snapperDegraded",
      tone: "accent",
      title: "Snapshots not readable",
      detail: typeof c.message === "string" && c.message !== ""
        ? c.message
        : "The snapper collector has no permission to list snapshots.",
      command: SNAPPER_FIX_COMMAND,
      actions: [
        { id: "terminal", label: "Run in terminal" },
        { id: "copy", label: "Copy" }
      ]
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
  seldon: GLYPH
}

function sourceGlyph(source) {
  return SOURCE_GLYPHS[source] !== undefined ? SOURCE_GLYPHS[source] : "•"
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
    // One colour source per row: open drift by its item's computed zone
    // (ADR-0013 §3: a routine group is yellow although its members are red in
    // the ledger), anything else by the event's own zone.
    var zone = item === null ? e.zone
      : typeof item.zone === "string" ? item.zone
      : item.crisis === true ? "red" : e.zone
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
      tone: zoneTone(zone),
      resolution: str(e.resolution),
      resolutionDetail: str(e.resolutionDetail),
      snapshot: e.source === "snapper" && e.kind === "snapshot",
      drift: item !== null,
      crisis: item !== null && item.crisis === true,
      proposedCase: leader && typeof leader.proposedCase === "string" ? leader.proposedCase : "",
      // "+N" on the group's leader row; N counts every member (ADR-0013 §2).
      badge: grouped ? "+" + leader.members : "",
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

// The folded resolution (ADR-0012 §8, §11), or the open-drift note.
function rowStatus(r) {
  if (r.resolution !== "") {
    var head = r.resolution === "linked" && r.caseId !== "" ? "linked to " + r.caseId : r.resolution
    return r.resolutionDetail !== "" ? head + ": " + r.resolutionDetail : head
  }
  if (!r.drift) return ""
  var note = r.groupLeader !== "" ? "In the open " + r.groupSubject + " group"
    : r.crisis ? "Needs a reason" : "Unexplained"
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
      { label: "open drift", value: count(summary.openDrift) }
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
