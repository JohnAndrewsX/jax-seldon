// Unit tests for plugin/Model.js under node (no Qt). Run: node tests/plugin/model.test.js
"use strict"

const fs = require("fs")
const path = require("path")
const vm = require("vm")
const assert = require("assert")

const root = path.resolve(__dirname, "../..")
const M = {}
vm.createContext(M)
vm.runInContext(fs.readFileSync(path.join(root, "plugin/Model.js"), "utf8"), M, { filename: "Model.js" })

const sample = fs.readFileSync(path.join(root, "fixtures/index.sample.json"), "utf8")
const notInit = fs.readFileSync(path.join(root, "fixtures/index-variants/not-initialised.json"), "utf8")
const v2 = fs.readFileSync(path.join(root, "fixtures/invalid/index.contract-v2.json"), "utf8")

let passed = 0
function test(name, fn) {
  try {
    fn()
    passed++
  } catch (e) {
    console.error("FAIL " + name + "\n  " + e.message)
    process.exitCode = 1
  }
}
// vm objects come from another realm, so compare through JSON.
const same = (a, b) => assert.strictEqual(JSON.stringify(a), JSON.stringify(b))

const gen = Date.parse("2026-10-01T17:05:12+02:00")
const H = 3600 * 1000

test("pillText hides zero parts (SPEC-PLUGIN §4)", () => {
  assert.strictEqual(M.pillText(null), "⟡")
  assert.strictEqual(M.pillText({ active: 0, drift: 0 }), "⟡")
  assert.strictEqual(M.pillText({ active: 2, drift: 0 }), "⟡ 2")
  assert.strictEqual(M.pillText({ active: 0, drift: 3 }), "⟡ · 3")
  assert.strictEqual(M.pillText({ active: 2, drift: 3 }), "⟡ 2 · 3")
})

test("pillTone: crisis beats active beats default", () => {
  assert.strictEqual(M.pillTone(null), "default")
  assert.strictEqual(M.pillTone({ active: 0, crisis: 0 }), "default")
  assert.strictEqual(M.pillTone({ active: 1, crisis: 0 }), "accent")
  assert.strictEqual(M.pillTone({ active: 0, crisis: 1 }), "urgent")
  assert.strictEqual(M.pillTone({ active: 2, crisis: 2 }), "urgent")
})

test("parseIndex accepts the sample and reads its counts", () => {
  const r = M.parseIndex(sample)
  assert.strictEqual(r.ok, true)
  same(M.counts(r.index), { active: 2, queued: 3, drift: 4, crisis: 2 })
  assert.strictEqual(M.pillText(M.counts(r.index)), "⟡ 2 · 4")
})

test("parseIndex reports a contract mismatch with the version found", () => {
  const r = M.parseIndex(v2)
  assert.strictEqual(r.ok, false)
  assert.strictEqual(r.error, "contract")
  assert.strictEqual(r.contractVersion, 2)
  assert.strictEqual(r.index, null)
})

test("parseIndex rejects empty, broken and non-object input", () => {
  assert.strictEqual(M.parseIndex("").error, "empty")
  assert.strictEqual(M.parseIndex(null).error, "empty")
  assert.strictEqual(M.parseIndex("{").error, "parse")
  assert.strictEqual(M.parseIndex("[1]").error, "shape")
  assert.strictEqual(M.parseIndex('{"contractVersion":1}').error, "shape")
})

const ok = M.parseIndex(sample)
const status = (over) => M.deriveStatus(Object.assign({
  engine: "present", file: "loaded", parse: ok, engineNotInitialised: false, nowMs: gen
}, over))

test("deriveStatus: every state, in precedence order", () => {
  assert.strictEqual(status({}), "ok")
  assert.strictEqual(status({ engine: "unknown" }), "ok")
  assert.strictEqual(status({ engine: "missing" }), "engineMissing")
  assert.strictEqual(status({ engine: "missing", file: "missing", parse: null }), "engineMissing")
  assert.strictEqual(status({ file: "missing", parse: null }), "indexMissing")
  assert.strictEqual(status({ file: "loading", parse: null }), "indexMissing")
  assert.strictEqual(status({ file: "invalid", parse: M.parseIndex("{") }), "indexMissing")
  assert.strictEqual(status({ file: "missing", parse: null, engineNotInitialised: true }), "notInitialised")
  assert.strictEqual(status({ parse: M.parseIndex(v2) }), "contractMismatch")
  assert.strictEqual(status({ parse: M.parseIndex(notInit) }), "notInitialised")
  assert.strictEqual(status({ engineNotInitialised: true }), "notInitialised")
  assert.strictEqual(status({ nowMs: gen + 2 * H + 1000 }), "indexStale")
  assert.strictEqual(status({ nowMs: gen + 2 * H - 1000 }), "ok")
  assert.strictEqual(status({ nowMs: gen - 5 * H }), "ok")
})

test("deriveStatus: index.state.status indexStale is honoured", () => {
  const stale = JSON.parse(sample)
  stale.state.status = "indexStale"
  assert.strictEqual(status({ parse: M.parseIndex(JSON.stringify(stale)) }), "indexStale")
})

test("effectiveNowMs pins the clock only in dev mode", () => {
  assert.strictEqual(M.effectiveNowMs(42, false, "2026-10-01T20:00:00+02:00", "2026-10-01T17:05:12+02:00"), 42)
  assert.strictEqual(M.effectiveNowMs(42, true, "", "2026-10-01T17:05:12+02:00"), gen)
  assert.strictEqual(M.effectiveNowMs(42, true, "2026-10-01T20:05:12+02:00", "2026-10-01T17:05:12+02:00"), gen + 3 * H)
  assert.strictEqual(M.effectiveNowMs(42, true, "garbage", "garbage"), 42)
})

test("tooltipText matches the SPEC-PLUGIN §4 example", () => {
  const c = { active: 2, drift: 3, crisis: 0, queued: 0 }
  const now = Date.parse("2026-10-01T17:09:00+02:00")
  assert.strictEqual(M.tooltipText("ok", c, "2026-10-01T17:05:00+02:00", now),
    "Seldon — 2 active cases, 3 unexplained changes, last capture 4 min ago")
  assert.strictEqual(M.tooltipText("ok", { active: 1, drift: 1, crisis: 1 }, "", now),
    "Seldon — 1 active case, 1 unexplained change (1 in the red zone), never captured")
  assert.strictEqual(M.tooltipText("indexStale", c, "2026-10-01T17:05:00+02:00", now + 3 * H),
    "Seldon — 2 active cases, 3 unexplained changes, last capture 3 h ago · index is stale")
  assert.strictEqual(M.tooltipText("engineMissing", null, "", now), "Seldon — engine not installed")
  assert.strictEqual(M.tooltipText("notInitialised", { active: 0, drift: 0, crisis: 0 }, "", now),
    "Seldon — logbook not initialised")
})

test("relativeAge", () => {
  assert.strictEqual(M.relativeAge(0, 30 * 1000), "just now")
  assert.strictEqual(M.relativeAge(0, 4 * 60 * 1000), "4 min ago")
  assert.strictEqual(M.relativeAge(0, 3 * H), "3 h ago")
  assert.strictEqual(M.relativeAge(0, 72 * H), "3 days ago")
  assert.strictEqual(M.relativeAge(NaN, 0), "unknown")
})

test("bannerFor: one banner per non-ok status, each with a fix", () => {
  assert.strictEqual(M.bannerFor("ok", {}), null)
  const constants = ["", M.INSTALL_ENGINE_COMMAND, M.INIT_COMMAND, M.UPDATE_ENGINE_COMMAND, M.UPDATE_PLUGIN_COMMAND]
  for (const s of M.STATUSES.filter((x) => x !== "ok")) {
    const b = M.bannerFor(s, { indexContractVersion: 2, generatedAt: "2026-10-01T17:05:12+02:00", nowMs: gen + 3 * H })
    assert.ok(b, s)
    assert.strictEqual(b.status, s)
    assert.ok(b.actions.length >= 1, s + " has a fix")
    assert.ok(constants.indexOf(b.command) !== -1, s + " command is a constant")
    for (const a of b.actions) assert.ok(["copy", "terminal", "recheck", "build", "capture"].indexOf(a.id) !== -1, a.id)
    if (b.actions.some((a) => a.id === "copy" || a.id === "terminal")) assert.notStrictEqual(b.command, "", s)
  }
})

test("bannerFor contractMismatch names the side to update", () => {
  assert.strictEqual(M.bannerFor("contractMismatch", { indexContractVersion: 2 }).command, M.UPDATE_PLUGIN_COMMAND)
  assert.strictEqual(M.bannerFor("contractMismatch", { indexContractVersion: 0 }).command, M.UPDATE_ENGINE_COMMAND)
  assert.ok(M.bannerFor("contractMismatch", { indexContractVersion: 2 }).detail.indexOf("v2") !== -1)
})

test("bannerFor indexStale shows the age", () => {
  const b = M.bannerFor("indexStale", { generatedAt: "2026-10-01T17:05:12+02:00", nowMs: gen + 3 * H })
  assert.strictEqual(b.detail, "Last update 3 h ago. Capture to refresh it.")
})

const EID = "01M3VTGNY0NZG4AY80814WSKGR"

test("validateArgs accepts every CONTRACT.md command form", () => {
  const good = [
    ["--version"], ["--version", "--json"], ["status", "--json"],
    ["capture", "--all", "--json", "--quiet"], ["capture", "--all", "--quiet", "--json"],
    ["log", "--", "Zed läuft"], ["log", "--case", "C-2026-004", "--", "text"],
    ["log", "--", "--json"], ["log", "--", "-rf --case C-2026-001"], ["log", "--", "--"],
    ["plan", "new", "--zone", "red", "--risk", "R2", "--", "A title; rm -rf ~"],
    ["plan", "start", "C-2026-005"], ["plan", "drop", "C-2026-1234"],
    ["drift", "link", EID, "C-2026-005"], ["drift", "link", EID, "C-2026-005", "--only"],
    ["drift", "explain", EID, "--", "theme test"], ["drift", "explain", EID, "--only", "--", "only this one"],
    ["drift", "dismiss", EID, "--", "tried it"], ["drift", "dismiss", EID, "--only", "--", "--tried it"],
    ["drift", "show", EID, "--json"],
    ["decide", "--no-edit", "--", "Use zed"], ["rebuild", "--json"], ["update-impact", "--json"],
    ["open", "journal", "--editor"], ["open", "C-2026-003", "--editor"],
    ["decide", "--no-edit", "--json", "--", "--help"], ["open", "ADR-0004", "--editor", "--json"],
    ["open", "logbook", "--editor", "--json"]
  ]
  for (const a of good) assert.strictEqual(M.validateArgs(a), "", JSON.stringify(a))
})

test("validateArgs refuses everything else", () => {
  const bad = [
    [], "status", ["seldon", "status"], ["init"], ["hook", "install", "claude-code"],
    ["status", "--logbook", "/tmp"], ["status", "--", "x"], ["capture"], ["capture", "--all"],
    ["capture", "--all", "--json", "--quiet", "--", "x"],
    // free text without `--` (the WP-010 forms)
    ["log", "Zed läuft"], ["log", "text", "--case", "C-2026-004"],
    ["plan", "new", "t", "--zone", "red", "--risk", "R2"], ["decide", "Use zed", "--no-edit"],
    ["drift", "explain", EID, "theme test"],
    // empty, blank or split free text
    ["log"], ["log", "--"], ["log", "--", ""], ["log", "--", "   "], ["log", "--", "a", "b"],
    ["decide", "--no-edit", "--", "\t"], ["drift", "explain", EID, "--", ""],
    ["drift", "dismiss", EID, "--", ""], ["drift", "dismiss", EID, "--", "  "],
    ["log", "--case", "C-26-1", "--", "x"], ["log", "--case", "C-2026-001; reboot", "--", "x"],
    ["log", "--case", "--", "x"],
    ["plan", "new", "--zone", "purple", "--risk", "R1", "--", "t"],
    ["plan", "new", "--zone", "red", "--risk", "R9", "--", "t"],
    ["plan", "start", "../C-2026-001"], ["plan", "finish", "C-2026-001"], ["plan", "start", "C-2026-001", "--", "x"],
    ["drift", "link", "01m3vtgny0nzg4ay80814wskgr", "C-2026-005"], ["drift", "link", "81M3VTGNY0NZG4AY80814WSKGR", "C-2026-005"],
    ["drift", "link", EID, "C-2026-005", "--all"], ["drift", "link", EID, "--only", "C-2026-005"],
    ["drift", "explain", EID], ["drift", "explain", EID, "--only"], ["drift", "explain", EID, "--force", "--", "x"],
    // the superseded `--reason <text>` form, and other shapes
    ["drift", "dismiss", EID, "--reason", "tried it"], ["drift", "dismiss", EID, "--only", "--reason", "tried it"],
    ["drift", "dismiss", EID, "reason"], ["drift", "dismiss", EID, "--", "x", "--only"],
    ["drift", "dismiss", EID], ["drift", "dismiss", EID, "--only"],
    ["drift", "show", EID], ["drift", "show", "C-2026-005", "--json"], ["drift", "show", EID, "--only", "--json"],
    ["decide", "--", "t"], ["open", "/etc/passwd", "--editor"], ["open", "journal"],
    ["decide", "--no-edit", "--json"], ["decide", "--no-edit", "--case", "C-2026-001", "--", "t"],
    ["decide", "--json", "--no-edit", "--", "t"], ["decide", "--no-edit", "--json", "--", "a", "b"],
    ["open", "ADR-4", "--editor"], ["open", "ADR-00041", "--editor"], ["open", "adr-0004", "--editor"],
    ["open", "ADR-0004; reboot", "--editor"], ["open", "ADR-0004", "--editor", "--", "x"],
    ["open", "memory", "--editor"], ["open", "memory/lessons.md", "--editor"],
    ["log", "--", 42], ["log", "--", "a\u0000b"]
  ]
  for (const a of bad) assert.notStrictEqual(M.validateArgs(a), "", JSON.stringify(a))
})

test("stateIndexPath honours XDG_STATE_HOME (CONTRACT.md rule 1)", () => {
  assert.strictEqual(M.stateIndexPath("", "/home/u"), "/home/u/.local/state/seldon/index.json")
  assert.strictEqual(M.stateIndexPath(undefined, "/home/u"), "/home/u/.local/state/seldon/index.json")
  assert.strictEqual(M.stateIndexPath("/srv/state/", "/home/u"), "/srv/state/seldon/index.json")
  assert.strictEqual(M.stateIndexPath("relative/state", "/home/u"), "/home/u/.local/state/seldon/index.json")
})

const sampleIndex = JSON.parse(sample)
const degraded = JSON.parse(fs.readFileSync(path.join(root, "fixtures/index-variants/snapper-degraded.json"), "utf8"))

test("crisisText: the red strip of SPEC-PLUGIN §5", () => {
  assert.strictEqual(M.crisisText(sampleIndex), "2 changes in the red zone need a reason")
  const one = JSON.parse(sample)
  one.summary.crisis = 1
  assert.strictEqual(M.crisisText(one), "1 change in the red zone needs a reason")
  one.summary.crisis = 0
  assert.strictEqual(M.crisisText(one), "")
  assert.strictEqual(M.crisisText(null), "")
})

test("snapperBanner: only for an enabled snapper collector that fails (ADR-0011)", () => {
  assert.strictEqual(M.snapperBanner(sampleIndex), null)
  assert.strictEqual(M.snapperBanner(null), null)
  const b = M.snapperBanner(degraded)
  assert.strictEqual(b.title, "Snapshots not readable")
  assert.strictEqual(b.command, M.SNAPPER_FIX_COMMAND)
  assert.ok(/^\S+ snapper -c root set-config ALLOW_USERS=\$USER SYNC_ACL=yes$/.test(b.command), b.command)
  assert.ok(b.detail.indexOf("ALLOW_USERS") !== -1, "shows the engine message")
  same(b.actions.map((a) => a.id), ["terminal", "copy"])
  const off = JSON.parse(JSON.stringify(degraded))
  off.state.collectors.forEach((c) => { if (c.name === "snapper") c.enabled = false })
  assert.strictEqual(M.snapperBanner(off), null)
  const bare = JSON.parse(JSON.stringify(degraded))
  bare.state.collectors.forEach((c) => { delete c.message })
  assert.ok(M.snapperBanner(bare).detail !== "")
})

test("changelogRows: 62 events newest first, one +2 group (3 members), folded resolutions, snapshots", () => {
  const rows = M.changelogRows(sampleIndex, "all")
  assert.strictEqual(rows.length, 62)
  same(rows.map((r) => r.id), sampleIndex.events.map((e) => e.id))
  const badged = rows.filter((r) => r.badge !== "")
  assert.strictEqual(badged.length, 1)
  assert.strictEqual(badged[0].badge, "+2")
  assert.strictEqual(badged[0].subject, "firefox")
  assert.strictEqual(badged[0].txId, "tx-20260930T214115")
  same(rows.filter((r) => r.groupLeader !== "").map((r) => r.subject).sort(), ["libinput", "noto-fonts"])
  assert.strictEqual(rows.filter((r) => r.resolutionDetail !== "").length, 7)
  assert.strictEqual(rows.filter((r) => r.snapshot).length, 8)
  assert.strictEqual(rows.filter((r) => r.drift).length, 6)
  same(rows.filter((r) => r.crisis).map((r) => r.kind), ["config-add", "install"])
  const theme = rows.find((r) => r.id === EID)
  assert.strictEqual(theme.proposedCase, "C-2026-005")
  assert.strictEqual(M.rowStatus(theme), "Unexplained · proposed for C-2026-005")
  const tyme = rows.find((r) => r.subject === "io.github.example.tyme")
  assert.strictEqual(M.rowStatus(tyme), "explained: Zeiterfassung nur zum Testen, noch nicht in der Bar.")
  assert.strictEqual(M.rowStatus(rows.find((r) => r.subject === "tailscale")), "linked to C-2026-008")
  assert.strictEqual(M.rowStatus(rows.find((r) => r.subject === "libinput")), "In the open firefox group")
  assert.strictEqual(M.rowStatus(rows.find((r) => r.subject === "ollama")), "Needs a reason")
  assert.strictEqual(M.rowStatus(rows[0]), "")
  assert.strictEqual(rows[0].dayLabel, "Today")
  assert.strictEqual(rows[0].time, "17:00")
  assert.strictEqual(rows.find((r) => r.subject === "firefox").dayLabel, "Yesterday")
  assert.strictEqual(rows[rows.length - 1].dayLabel, "Tue 1 Sep")
  assert.strictEqual(rows.find((r) => r.subject === "ollama").tone, "urgent")
  assert.strictEqual(theme.tone, "accent")
  assert.strictEqual(rows[0].tone, "")
  // One colour source per row: open drift by its item's zone, so the routine
  // group (members red in the ledger) is accent throughout; resolved or
  // cased events by their own zone.
  for (const s of ["firefox", "libinput", "noto-fonts"]) {
    const r = rows.find((x) => x.subject === s)
    assert.strictEqual(r.zone, "red", s + " ledger zone")
    assert.strictEqual(r.tone, "accent", s)
  }
  assert.strictEqual(rows.find((r) => r.subject === "hyprland").tone, "urgent")
  assert.strictEqual(rows.find((r) => r.subject === "btop").tone, "urgent")
  assert.strictEqual(tyme.tone, "accent")
  const noZone = JSON.parse(sample)
  noZone.drift.forEach((d) => { delete d.zone })
  const nz = M.changelogRows(noZone, "all")
  assert.strictEqual(nz.find((r) => r.subject === "ollama").tone, "urgent", "crisis without zone")
  assert.strictEqual(nz.find((r) => r.subject === "firefox").tone, "urgent", "falls back to the event zone")
  assert.strictEqual(M.rowMeta(rows.find((r) => r.subject === "zed")), "0.198.4-1 · claude-code · C-2026-004")
})

test("changelogRows: the source filter narrows the list", () => {
  const counts = M.sourceCounts(sampleIndex)
  assert.strictEqual(counts.all, 62)
  let total = 0
  for (const s of M.SOURCES) {
    const rows = M.changelogRows(sampleIndex, s)
    assert.strictEqual(rows.length, counts[s], s)
    assert.ok(rows.every((r) => r.source === s), s)
    total += rows.length
  }
  assert.strictEqual(total, 62)
  assert.strictEqual(M.changelogRows(sampleIndex, "pacman").length, 12)
  assert.strictEqual(M.changelogRows(sampleIndex, "snapper").length, 10)
  assert.strictEqual(M.changelogRows(sampleIndex, "").length, 62)
  same(M.filterChips(sampleIndex).map((c) => c.id), ["all"].concat(Array.from(M.SOURCES)))
  assert.strictEqual(M.cycleFilter("all", 1), "pacman")
  assert.strictEqual(M.cycleFilter("seldon", 1), "all")
  assert.strictEqual(M.cycleFilter("all", -1), "seldon")
  assert.strictEqual(M.cycleFilter("nonsense", 1), "pacman")
  same(M.changelogRows(null, "all"), [])
})

test("groupMembers expands a drift group from index.events by txId", () => {
  const members = M.groupMembers(sampleIndex, "tx-20260930T214115")
  same(members.map((m) => m.subject), ["libinput", "noto-fonts", "firefox"])
  assert.strictEqual(M.memberLine(members[2]), "upgrade firefox  143.0.1-1 → 143.0.2-1")
  same(M.groupMembers(sampleIndex, ""), [])
  // A transaction with a case is no open group.
  same(M.groupMembers(sampleIndex, "tx-20261001T101204"), [])
})

test("tab keys are fixed per tab id (SPEC-PLUGIN §5)", () => {
  same(M.TAB_KEYS, { 1: "today", 2: "changelog", 3: "work", 4: "decisions", 5: "system", 6: "memory" })
  same(["today", "changelog", "system", "nope"].map((id) => M.tabKeyFor(id)), ["1", "2", "5", ""])
})

test("every source has a glyph; zones map to theme tones only", () => {
  for (const s of M.SOURCES) assert.ok(M.sourceGlyph(s).length >= 1, s)
  assert.strictEqual(M.sourceGlyph("nope"), "•")
  same(["red", "yellow", "green", undefined].map(M.zoneTone), ["urgent", "accent", "muted", ""])
})

test("todayView: today's and yesterday's journal and the summary counts", () => {
  const t = M.todayView(sampleIndex)
  assert.strictEqual(t.date, "2026-10-01")
  assert.strictEqual(t.title, "Thursday, 1 Oct 2026")
  assert.strictEqual(t.entries.length, 4)
  assert.strictEqual(t.yesterday.length, 1)
  assert.strictEqual(M.entryMeta(t.entries[0]), "09:25 · claude-code · C-2026-003")
  assert.strictEqual(M.entryMeta(t.entries[2]), "14:40 · human")
  same(t.stats.map((s) => s.value), [30, 41, 2, 3, 4])
  const empty = M.todayView(null)
  same([empty.entries.length, empty.yesterday.length, empty.title], [0, 0, "Today"])
})

test("systemSections: every field optional", () => {
  const now = Date.parse("2026-10-01T17:05:12+02:00")
  const s = M.systemSections(sampleIndex, now)
  same(s.map((x) => x.title), ["OMARCHY", "PACKAGES", "PLUGINS", "SNAPSHOTS", "AREAS", "COLLECTORS", "SELDON"])
  same(s[0].rows, [{ label: "Version", value: "4.0.7-1" }, { label: "Theme", value: "tokyo-night" },
    { label: "Last update", value: "2026-10-01 09:21 · 7 h ago" }])
  same(s[2].rows, [{ label: "Plugins", value: "33 of 40 enabled" }])
  assert.strictEqual(s[3].rows.length, 6)
  same(s[3].rows.slice(0, 3), [{ label: "#115", value: "2026-10-01 16:30 · tailscale: MagicDNS · post" },
    { label: "#114", value: "2026-10-01 16:30 · tailscale: MagicDNS · pre" },
    { label: "#113", value: "2026-10-01 14:30 · pre: ollama" }])
  same(s[4].rows[1], { label: "hyprland", value: "1 case · AGENTS.md" })
  const bare = JSON.parse(sample)
  bare.system = {}
  delete bare.state.collectors
  same(M.systemSections(bare, now).map((x) => x.title), ["SELDON"])
  bare.system = { packages: { aur: 3 }, plugins: { installed: 4 }, snapshots: [{ number: 7, ts: "2026-01-01T00:00:00Z", type: "pre" }] }
  same(M.systemSections(bare, now).slice(0, 3).map((x) => x.rows), [[{ label: "AUR", value: "3" }],
    [{ label: "Plugins", value: "4 installed" }], [{ label: "#7", value: "2026-01-01 00:00 · pre" }]])
  const failing = M.systemSections(degraded, now).find((x) => x.title === "COLLECTORS")
  assert.ok(failing.rows.find((r) => r.label === "snapper").value.indexOf("failing · snapper: No permissions") === 0)
  same(M.systemSections(null, now), [])
})

test("dayLabel and clockTime read the timestamp as written", () => {
  assert.strictEqual(M.clockTime("2026-09-03T21:14:06+00:00"), "21:14")
  assert.strictEqual(M.clockTime("garbage"), "")
  assert.strictEqual(M.dayLabel("2026-10-01", "2026-10-01"), "Today")
  assert.strictEqual(M.dayLabel("2026-09-30", "2026-10-01"), "Yesterday")
  assert.strictEqual(M.dayLabel("2026-03-01", "2026-03-02"), "Yesterday")
  assert.strictEqual(M.dayLabel("2025-12-31", "2026-10-01"), "Wed 31 Dec 2025")
  assert.strictEqual(M.dayLabel("", "2026-10-01"), "Undated")
  assert.strictEqual(M.actorLabel("agent:claude-code"), "claude-code")
  assert.strictEqual(M.actorLabel("human"), "human")
})

test("clampInterval and resolvePath", () => {
  assert.strictEqual(M.clampInterval(15), 15)
  assert.strictEqual(M.clampInterval(1), 5)
  assert.strictEqual(M.clampInterval(999), 120)
  assert.strictEqual(M.clampInterval("x"), 15)
  assert.strictEqual(M.resolvePath("/a/b.json", "/w"), "/a/b.json")
  assert.strictEqual(M.resolvePath("fixtures/i.json", "/w/"), "/w/fixtures/i.json")
  assert.strictEqual(M.resolvePath("i.json", ""), "i.json")
})

test("engineVersion and engineError read the SPEC-ENGINE §3 JSON shapes", () => {
  assert.strictEqual(M.engineVersion('{"name":"seldon","version":"0.1.0"}'), "0.1.0")
  assert.strictEqual(M.engineVersion("seldon 0.1.0"), "")
  assert.strictEqual(M.engineError('{"error":{"code":1,"message":"unrecognized subcommand"}}', "", 1), "unrecognized subcommand")
  assert.strictEqual(M.engineError("", "boom\nmore", 2), "boom")
  assert.strictEqual(M.engineError("", "", 2), "seldon exited with code 2")
})

// ---- WP-012: panel actions

test("openCases and caseOptions: open cases only, active first, ids checked", () => {
  same(M.openCases(sampleIndex).map((c) => c.id + " " + c.status), [
    "C-2026-003 active", "C-2026-004 active", "C-2026-008 verification",
    "C-2026-005 queued", "C-2026-006 queued", "C-2026-007 queued"
  ])
  const opts = M.caseOptions(sampleIndex)
  same(opts[0], { value: "", label: "No case" })
  same(opts[2], { value: "C-2026-004", label: "C-2026-004 · Zed als zweiten Editor installieren" })
  assert.strictEqual(opts.length, 7)
  // no index, no cases, a malformed id: only "No case" survives
  same(M.caseOptions(null), [{ value: "", label: "No case" }])
  const odd = { cases: { active: [{ id: "C-2026-001; x", title: "x" }, { id: "C-2026-002" }, null], queued: "x" } }
  same(M.openCases(odd).map((c) => c.id + "|" + c.title), ["C-2026-002|"])
})

test("logArgs: the text is one argument after `--`, exactly as typed", () => {
  for (const text of ["--help", 'a "b" c', "line one\nline two", "-rf --case C-2026-001", "--", "  padded  ", "$(true)"]) {
    same(M.logArgs(text, "").args, ["log", "--json", "--", text])
    same(M.logArgs(text, "C-2026-004").args, ["log", "--case", "C-2026-004", "--json", "--", text])
    assert.strictEqual(M.validateArgs(M.logArgs(text, "C-2026-004").args), "", text)
  }
  assert.strictEqual(M.logArgs("", "").error, "Write something first")
  assert.strictEqual(M.logArgs("  \n\t ", "").error, "Write something first")
  assert.strictEqual(M.logArgs(null, "").error, "Write something first")
  assert.strictEqual(M.logArgs("x", "C-26-1").error, "Not a case id: C-26-1")
  assert.strictEqual(M.logArgs("x", "C-2026-001 --tag y").error, "Not a case id: C-2026-001 --tag y")
  assert.ok(M.logArgs("a\u0000b", "").error)
})

test("openArgs: journal, ledger, status, logbook, a case or decision id, nothing else", () => {
  for (const what of ["journal", "ledger", "status", "logbook", "C-2026-004", "ADR-0001"]) {
    same(M.openArgs(what), ["open", what, "--editor", "--json"])
    assert.strictEqual(M.validateArgs(M.openArgs(what)), "", what)
  }
  for (const what of ["", "case", "memory", "memory/lessons.md", "ADR-1", "ADR-0001 ", "ADR-0001-language",
    "decisions/ADR-0001-language.md", "../../etc/passwd", "/etc/passwd", "C-2026-1", "journal --x", null])
    assert.strictEqual(M.openArgs(what), null, String(what))
})

test("logResult, openResult, captureResult read the SPEC-ENGINE §3 shapes", () => {
  const ev = (o) => JSON.stringify({ event: Object.assign({ id: EID, source: "manual", kind: "note" }, o), git: null })
  same(M.logResult(0, ev({ subject: "journal", case: null }), ""), { ok: true, text: "Saved to the journal · " + EID })
  same(M.logResult(0, ev({ subject: "C-2026-004", case: "C-2026-004" }), ""), { ok: true, text: "Saved to C-2026-004 · " + EID })
  same(M.logResult(0, "not json", ""), { ok: true, text: "Saved to the journal" })
  same(M.logResult(1, '{"error":{"code":1,"message":"unknown case C-2026-999"}}', ""), { ok: false, text: "unknown case C-2026-999" })
  same(M.logResult(3, "", "logbook not initialised"), { ok: false, text: "logbook not initialised" })

  const opened = M.openResult(0, JSON.stringify({ what: "journal", path: "/l/journal/2026/2026-10-01.md",
    editor: { launched: true, program: "omarchy-launch-editor" } }), "")
  same(opened, { ok: true, path: "/l/journal/2026/2026-10-01.md", text: "Opened /l/journal/2026/2026-10-01.md in omarchy-launch-editor" })
  assert.strictEqual(M.openResult(0, "{}", "").text, "Opened in the editor")
  same(M.openResult(0, JSON.stringify({ path: "/p", editor: { launched: false, error: "no editor" } }), ""), { ok: false, text: "no editor" })
  same(M.openResult(1, '{"error":{"code":1,"message":"cannot open /p: no terminal"}}', ""), { ok: false, text: "cannot open /p: no terminal" })

  const cap = (o) => JSON.stringify(Object.assign({ ok: true, logbook: "/l", files: [] }, o))
  same(M.captureResult(0, cap({ written: 0, collectors: [] }), ""), { ok: true, text: "nothing new" })
  same(M.captureResult(0, cap({ written: 1 }), ""), { ok: true, text: "1 new event" })
  same(M.captureResult(0, cap({ written: 12, collectors: [{ name: "pacman", ok: true }, { name: "snapper", ok: false, fix: "x" }] }), ""),
    { ok: true, text: "12 new events · failing: snapper" })
  same(M.captureResult(4, '{"error":{"code":4,"message":"lock held"}}', ""), { ok: false, text: "lock held" })
})

test("group badge: +(members - 1), the leader not counted twice", () => {
  const badge = (members) => M.changelogRows({
    events: [{ id: "01M3W0000000000000000000AA", source: "pacman", kind: "upgrade", subject: "a", txId: "t" }],
    drift: [{ eventId: "01M3W0000000000000000000AA", txId: "t", members: members }]
  }, "all")[0].badge
  assert.strictEqual(badge(3), "+2")
  assert.strictEqual(badge(2), "+1")
  assert.strictEqual(badge(1), "")
})

test("validateArgs: plan new takes --area and --priority, in that order, each optional", () => {
  const base = ["plan", "new", "--zone", "yellow", "--risk", "R1"]
  const good = [
    base.concat(["--json", "--", "t"]),
    base.concat(["--area", "dev-env", "--json", "--", "t"]),
    base.concat(["--priority", "high", "--", "t"]),
    base.concat(["--area", "a1", "--priority", "low", "--json", "--", "--help"]),
    ["plan", "verify", "C-2026-004", "--json"]
  ]
  for (const a of good) assert.strictEqual(M.validateArgs(a), "", JSON.stringify(a))
  const bad = [
    base.concat(["--priority", "high", "--area", "dev-env", "--", "t"]),
    base.concat(["--area", "Dev_Env", "--", "t"]),
    base.concat(["--area", "-x", "--", "t"]),
    base.concat(["--area", "--", "t"]),
    base.concat(["--priority", "urgent", "--", "t"]),
    base.concat(["--priority", "--", "t"]),
    base.concat(["--area", "a", "--area", "b", "--", "t"]),
    base.concat(["--actor", "agent:x", "--", "t"]),
    base.concat(["--json"]),
    ["plan", "start", "C-2026-004", "--reason", "x"],
    ["plan", "list"], ["plan", "show", "C-2026-004"]
  ]
  for (const a of bad) assert.notStrictEqual(M.validateArgs(a), "", JSON.stringify(a))
})

test("workColumns: Queued 3, Active 3 (2 active + 1 verification), Completed 2 on the sample", () => {
  const index = JSON.parse(sample)
  const cols = M.workColumns(index)
  same(cols.map(c => c.id + " " + c.cases.length), ["queued 3", "active 3", "completed 2"])
  same(cols[1].cases.map(c => c.id + " " + c.status), ["C-2026-003 active", "C-2026-004 active", "C-2026-008 verification"])
  same(M.workCases(cols).map(c => c.id), ["C-2026-005", "C-2026-006", "C-2026-007", "C-2026-003", "C-2026-004",
    "C-2026-008", "C-2026-002", "C-2026-001"])
  const c5 = cols[0].cases[0]
  same([c5.proposed, c5.stepsText, c5.tone, c5.area, c5.priority, c5.started, c5.actionable], [1, "0/4", "accent", "themes", "normal", "", true])
  same(M.workCases(cols).filter(c => c.proposed > 0).map(c => c.id), ["C-2026-005"])
  assert.strictEqual(cols[1].cases[0].tone, "urgent")
  assert.strictEqual(cols[2].cases[1].tone, "muted")
  assert.strictEqual(M.caseMeta(cols[1].cases[1]), "dev-env · priority normal · 2/4 steps")
  assert.strictEqual(M.caseDates(cols[2].cases[0]), "created 2026-09-12 · started 2026-09-12 · closed 2026-09-13")
  assert.strictEqual(M.caseDates(c5), "created 2026-09-29")
  // No index, no cases, broken entries: empty columns, never a throw.
  same(M.workColumns(null).map(c => c.cases.length), [0, 0, 0])
  same(M.workColumns({ cases: { queued: [null, 7, { id: "x" }], active: "no" } }).map(c => c.cases.length), [1, 0, 0])
  const odd = M.workColumns({ cases: { queued: [{ id: "C-26-1; rm", title: "t", status: "bogus", steps: { total: 2, done: 5 } }] } })[0].cases[0]
  same([odd.status, odd.actionable, odd.stepsText], ["queued", false, "2/2"])
  same(M.caseActions(odd), [])
})

test("wipStatus: active cases (not verification) against the limit", () => {
  const index = JSON.parse(sample)
  same(M.wipStatus(index, 3), { active: 2, limit: 3, text: "2 / 3 active", tone: "" })
  same(M.wipStatus(index, 2).tone, "accent")
  same(M.wipStatus(index, 1), { active: 2, limit: 1, text: "2 / 1 active", tone: "urgent" })
  assert.strictEqual(M.wipStatus(null, 3).text, "0 / 3 active")
  assert.strictEqual(M.wipStatus(index, "x").limit, 3)
  assert.strictEqual(M.clampWipLimit(0), 1)
  assert.strictEqual(M.clampWipLimit(99), 20)
  assert.strictEqual(M.clampWipLimit("4"), 4)
})

test("caseActions by status (WP-020); Enter runs the first, Drop asks twice", () => {
  const cases = M.workCases(M.workColumns(JSON.parse(sample)))
  const by = id => M.caseActions(cases.find(c => c.id === id))
    .map(a => a.id + (a.primary ? "*" : "") + (a.confirm ? "?" : "") + (a.twice ? "!" : ""))
  same(by("C-2026-005"), ["start*", "open"])
  // Start agent (WP-022) only on an active case, never the first action
  same(by("C-2026-003"), ["verify*", "agent!", "drop?", "open"])
  assert.strictEqual(M.caseAction(cases.find(c => c.id === "C-2026-003"), "agent").label, "Start agent")
  same(by("C-2026-008"), ["done*", "drop?", "open"])
  same(by("C-2026-002"), ["open*"])
  same(M.caseActions({ id: "C-2026-010", status: "dropped", actionable: true }).map(a => a.id), ["open"])
  same(M.caseActions(null), [])
  assert.strictEqual(M.caseAction(cases[0], "drop"), null)
  assert.strictEqual(M.caseAction(cases[3], "drop").label, "Drop")
})

test("planArgs: fixed argv, title one argument after `--`, ids and slugs checked", () => {
  const titles = ["--help", 'a "b" c', "-rf --zone red", "--", "$(reboot)", "Zed; rm -rf ~"]
  for (const title of titles) {
    const built = M.planArgs("new", { title: title, zone: "yellow", risk: "R1", priority: "normal", area: "" })
    same(built.args, ["plan", "new", "--zone", "yellow", "--risk", "R1", "--json", "--", title])
    assert.strictEqual(M.validateArgs(built.args), "", title)
  }
  same(M.planArgs("new", { title: "t" }).args, ["plan", "new", "--zone", "yellow", "--risk", "R1", "--json", "--", "t"])
  const full = M.planArgs("new", { title: "Zed", zone: "red", risk: "R2", area: "dev-env", priority: "high" }).args
  same(full, ["plan", "new", "--zone", "red", "--risk", "R2", "--area", "dev-env", "--priority", "high", "--json", "--", "Zed"])
  assert.strictEqual(M.validateArgs(full), "")
  assert.strictEqual(M.planArgs("new", { title: "  " }).error, "Give the case a title")
  assert.strictEqual(M.planArgs("new", { title: "t", area: "Dev Env" }).error, "Area must be a lowercase slug: letters, digits and -")
  assert.strictEqual(M.planArgs("new", { title: "t", area: "-x" }).error, "Area must be a lowercase slug: letters, digits and -")
  assert.strictEqual(M.planArgs("new", { title: "t", zone: "purple" }).error, "Not a zone: purple")
  assert.strictEqual(M.planArgs("new", { title: "t", risk: "R9" }).error, "Not a risk: R9")
  assert.strictEqual(M.planArgs("new", { title: "t", priority: "urgent" }).error, "Not a priority: urgent")
  assert.strictEqual(M.planArgs("new", { title: "a\u0000b" }).error, "The title contains a NUL character")
  for (const step of ["start", "verify", "done", "drop"]) {
    same(M.planArgs(step, "C-2026-005").args, ["plan", step, "C-2026-005", "--json"])
    assert.strictEqual(M.validateArgs(M.planArgs(step, "C-2026-005").args), "")
  }
  assert.strictEqual(M.planArgs("start", "C-26-1; rm -rf ~").error, "Not a case id: C-26-1; rm -rf ~")
  assert.strictEqual(M.planArgs("start", "--help").error, "Not a case id: --help")
  assert.strictEqual(M.planArgs("finish", "C-2026-005").error, "Not a plan step: finish")
  assert.strictEqual(M.planArgs("list", "").error, "Not a plan step: list")
})

test("planResult reads the SPEC-ENGINE §3 plan shapes", () => {
  const step = (o) => JSON.stringify(Object.assign({ case: { id: "C-2026-005" }, movedFrom: null, activeCase: null,
    journal: null, event: {}, git: null }, o))
  same(M.planResult(0, step({ from: "queued", to: "active" }), ""), { ok: true, text: "C-2026-005: queued → active", caseId: "C-2026-005" })
  same(M.planResult(0, step({ from: "verification", to: "completed", journal: "journal/2026/2026-10-01.md" }), ""),
    { ok: true, text: "C-2026-005: verification → completed · journal journal/2026/2026-10-01.md", caseId: "C-2026-005" })
  same(M.planResult(0, JSON.stringify({ case: { id: "C-2026-009", title: "Zed" }, event: {}, areaCreated: null, git: null }), ""),
    { ok: true, text: "Created C-2026-009 · Zed", caseId: "C-2026-009" })
  same(M.planResult(0, JSON.stringify({ case: { id: "C-2026-009", title: "Zed" }, areaCreated: "dev-env" }), ""),
    { ok: true, text: "Created C-2026-009 · Zed · new area dev-env", caseId: "C-2026-009" })
  same(M.planResult(0, "not json", ""), { ok: true, text: "Case created", caseId: "" })
  const refused = "C-2026-003 is active; `seldon plan done` needs a case that is verification; run `seldon plan verify` first"
  same(M.planResult(1, JSON.stringify({ error: { code: 1, message: refused } }), ""), { ok: false, text: refused, caseId: "" })
  same(M.planResult(4, "", "lock held"), { ok: false, text: "lock held", caseId: "" })
})

test("Start agent (WP-022): agentArgs, agentResult, the allow-list, the agents line", () => {
  same(M.agentArgs("C-2026-003").args, ["agent", "start", "C-2026-003", "--json"])
  assert.strictEqual(M.validateArgs(M.agentArgs("C-2026-003").args), "")
  for (const bad of ["C-26-1; rm -rf ~", "--help", "", null, "C-2026-003 --launcher x"])
    assert.strictEqual(M.agentArgs(bad).error, "Not a case id: " + (bad === null ? "" : bad), String(bad))
  // only `agent start <caseId> --json`: no launcher, no free text, no other verb
  for (const args of [["agent", "start", "C-2026-003"], ["agent", "start", "C-2026-003", "--launcher", "x", "--json"],
    ["agent", "stop", "C-2026-003", "--json"], ["agent", "start", "C-26-1", "--json"],
    ["agent", "start", "C-2026-003", "--json", "--", "text"], ["agent", "--json"]])
    assert.notStrictEqual(M.validateArgs(args), "", JSON.stringify(args))
  const answer = { launched: true, launcher: "default", program: "omarchy", argv: ["omarchy", "agent", "prompt", "{prompt}"],
    case: "C-2026-003", cwd: "/home/u/Seldon", previousActiveCase: null }
  same(M.agentResult(0, JSON.stringify(answer), ""),
    { ok: true, text: "Agent started on C-2026-003 · launcher default (omarchy)", caseId: "C-2026-003" })
  same(M.agentResult(0, JSON.stringify(Object.assign({}, answer, { launcher: "omarchy" })), "").text,
    "Agent started on C-2026-003 · launcher omarchy")
  same(M.agentResult(0, "not json", ""), { ok: true, text: "Agent started on the case", caseId: "" })
  const queued = "C-2026-005 is queued; start it first: `seldon plan start C-2026-005`"
  same(M.agentResult(1, JSON.stringify({ error: { code: 1, message: queued } }), ""), { ok: false, text: queued, caseId: "" })
  // the agents line from the schema's `agents`
  const cases = M.workCases(M.workColumns(JSON.parse(sample)))
  for (const c of cases) assert.ok(Array.isArray(c.agents), c.id)
  assert.strictEqual(M.caseAgents({ agents: [] }), "")
  assert.strictEqual(M.caseAgents({ agents: ["agent:claude-code"] }), "agent: claude-code")
  assert.strictEqual(M.caseAgents({ agents: ["agent:claude-code", "agent:codex"] }), "agents: claude-code, codex")
  assert.strictEqual(M.caseAgents(null), "")
  same(M.workColumns({ cases: { active: [{ id: "C-2026-001", agents: ["agent:x", 7, null, ""] }] } })[1].cases[0].agents, ["agent:x"])
})

// ---- Drift sheet (WP-021) ----
const THEME = "01M3VTGNY0NZG4AY80814WSKGR"
const UNIT = "01M3VNJ9JGZ9169T01XCW16FT0"
const OLLAMA = "01M3VNFTF8EVHWFFZ687N14Q0C"
const FIREFOX = "01M3SXBQVR7AW8PJQC1YXDCQ14"
const LIBINPUT = "01M3SXBRV0WPNQ721VWGG2WXZ1"
const NOTO = "01M3SXBRV0E702XKBM22HEV1B8"

test("validateArgs: drift explain takes --only, --zone, --risk, --area, in that order, each optional", () => {
  const ok = [
    ["drift", "explain", EID, "--json", "--", "x"],
    ["drift", "explain", EID, "--only", "--zone", "red", "--risk", "R2", "--area", "dev-env", "--json", "--", "x"],
    ["drift", "explain", EID, "--zone", "green", "--json", "--", "x"],
    ["drift", "explain", EID, "--risk", "R0", "--json", "--", "x"],
    ["drift", "explain", EID, "--area", "a1", "--", "x"],
    ["drift", "link", EID, "C-2026-005", "--only", "--json"], ["drift", "dismiss", EID, "--only", "--json", "--", "x"]
  ]
  for (const a of ok) assert.strictEqual(M.validateArgs(a), "", JSON.stringify(a))
  const bad = [
    ["drift", "explain", EID, "--risk", "R2", "--zone", "red", "--", "x"],
    ["drift", "explain", EID, "--zone", "red", "--only", "--", "x"],
    ["drift", "explain", EID, "--zone", "purple", "--", "x"], ["drift", "explain", EID, "--risk", "R7", "--", "x"],
    ["drift", "explain", EID, "--area", "Dev Env", "--", "x"], ["drift", "explain", EID, "--area", "-x", "--", "x"],
    ["drift", "explain", EID, "--zone", "--", "x"], ["drift", "explain", EID, "--actor", "human", "--", "x"],
    ["drift", "dismiss", EID, "--zone", "red", "--", "x"], ["drift", "link", EID, "C-2026-005", "--zone", "red"]
  ]
  for (const a of bad) assert.notStrictEqual(M.validateArgs(a), "", JSON.stringify(a))
})

test("driftItemFor: the four sample items, a group member, and events that are not open drift", () => {
  const theme = M.driftItemFor(sampleIndex, THEME)
  assert.strictEqual(theme.subject, "tokyo-night")
  assert.strictEqual(theme.proposedCase, "C-2026-005")
  assert.strictEqual(theme.grouped, false)
  assert.strictEqual(theme.tone, "accent")
  assert.strictEqual(M.driftDefaultAction(theme), "link")
  const unit = M.driftItemFor(sampleIndex, UNIT)
  assert.strictEqual(unit.crisis, true)
  assert.strictEqual(unit.zone, "red")
  assert.strictEqual(M.driftDefaultAction(unit), "explain")
  assert.strictEqual(M.driftItemFor(sampleIndex, OLLAMA).subject, "ollama")
  const group = M.driftItemFor(sampleIndex, FIREFOX)
  assert.strictEqual(group.grouped, true)
  assert.strictEqual(group.members, 3)
  assert.strictEqual(group.badge, "+2")
  assert.strictEqual(group.zone, "yellow")
  same(group.memberList.map((m) => m.subject), ["firefox", "noto-fonts", "libinput"])
  // Opened from a member row: the group's item, named by that member.
  const member = M.driftItemFor(sampleIndex, LIBINPUT)
  assert.strictEqual(member.eventId, LIBINPUT)
  assert.strictEqual(member.leaderId, FIREFOX)
  assert.strictEqual(member.namedSubject, "libinput")
  assert.strictEqual(member.subject, "firefox")
  assert.strictEqual(M.driftItemFor(sampleIndex, "01M3VDBX30F5DH0JNY7S0K95GC"), null) // tailscale, linked
  assert.strictEqual(M.driftItemFor(sampleIndex, "01M1MB2M1GWZYF485HTGVZ1KS3"), null) // btop, explained
  assert.strictEqual(M.driftItemFor(sampleIndex, "not an id"), null)
  assert.strictEqual(M.driftItemFor(null, THEME), null)
})

test("caseOptionsFor: the proposed case first, else 'Pick a case'; open cases only", () => {
  const theme = M.caseOptionsFor(sampleIndex, M.driftItemFor(sampleIndex, THEME))
  same(theme.map((o) => o.value), ["C-2026-005", "C-2026-003", "C-2026-004", "C-2026-008", "C-2026-006", "C-2026-007"])
  assert.strictEqual(theme[0].label, "C-2026-005 · Theme-Wechsel auf Tokyo Night durchziehen (Zed, Neovim) · proposed")
  const unit = M.caseOptionsFor(sampleIndex, M.driftItemFor(sampleIndex, UNIT))
  same(unit.map((o) => o.value), ["", "C-2026-003", "C-2026-004", "C-2026-008", "C-2026-005", "C-2026-006", "C-2026-007"])
  assert.strictEqual(unit[0].label, "Pick a case")
})

test("driftArgs: fixed argv, ids checked, the text one argument after `--`", () => {
  same(M.driftArgs("link", { eventId: THEME, caseId: "C-2026-005" }).args, ["drift", "link", THEME, "C-2026-005", "--json"])
  same(M.driftArgs("link", { eventId: FIREFOX, caseId: "C-2026-004", only: true }).args,
    ["drift", "link", FIREFOX, "C-2026-004", "--only", "--json"])
  same(M.driftArgs("dismiss", { eventId: LIBINPUT, only: true, text: "--help" }).args,
    ["drift", "dismiss", LIBINPUT, "--only", "--json", "--", "--help"])
  same(M.driftArgs("explain", { eventId: UNIT, text: 'say "hi"; $(reboot)', zone: "red", risk: "R1", area: "", itemZone: "red" }).args,
    ["drift", "explain", UNIT, "--json", "--", 'say "hi"; $(reboot)'])
  same(M.driftArgs("explain", { eventId: FIREFOX, only: true, text: "x", zone: "red", risk: "R3", area: "browser", itemZone: "yellow" }).args,
    ["drift", "explain", FIREFOX, "--only", "--zone", "red", "--risk", "R3", "--area", "browser", "--json", "--", "x"])
  for (const [action, f] of [["link", { eventId: THEME, caseId: "C-2026-005", only: true }],
    ["explain", { eventId: THEME, text: "-rf --zone red", zone: "green", risk: "R2", area: "a", itemZone: "yellow" }],
    ["dismiss", { eventId: THEME, text: "--" }]])
    assert.strictEqual(M.validateArgs(M.driftArgs(action, f).args), "", action)
  assert.strictEqual(M.driftArgs("link", { eventId: THEME, caseId: "" }).error, "Pick a case first")
  assert.strictEqual(M.driftArgs("link", { eventId: THEME, caseId: "C-26-1; rm -rf ~" }).error, "Not a case id: C-26-1; rm -rf ~")
  assert.strictEqual(M.driftArgs("link", { eventId: THEME.toLowerCase(), caseId: "C-2026-005" }).error,
    "Not an event id: " + THEME.toLowerCase())
  assert.strictEqual(M.driftArgs("explain", { eventId: THEME, text: " \t " }).error, "Say why it changed first")
  assert.strictEqual(M.driftArgs("dismiss", { eventId: THEME, text: "" }).error, "Give a reason first")
  assert.strictEqual(M.driftArgs("dismiss", { eventId: THEME, text: "two\nlines" }).error, "The text must be one line")
  assert.strictEqual(M.driftArgs("explain", { eventId: THEME, text: "x", area: "Dev Env" }).error,
    "Area must be a lowercase slug: letters, digits and -")
  assert.strictEqual(M.driftArgs("explain", { eventId: THEME, text: "x", zone: "purple" }).error, "Not a zone: purple")
  assert.strictEqual(M.driftArgs("purge", { eventId: THEME }).error, "Not a drift action: purge")
})

test("driftSummary names what a call resolves", () => {
  const group = M.driftItemFor(sampleIndex, LIBINPUT)
  assert.strictEqual(M.driftSummary("link", M.driftItemFor(sampleIndex, THEME), { caseId: "C-2026-005" }),
    "Link tokyo-night to C-2026-005")
  assert.strictEqual(M.driftSummary("dismiss", group, {}), "Dismiss firefox and 2 more")
  assert.strictEqual(M.driftSummary("dismiss", group, { only: true }), "Dismiss libinput only")
  assert.strictEqual(M.driftSummary("explain", M.driftItemFor(sampleIndex, OLLAMA), {}), "Explain ollama as a new completed case")
})

test("driftResult reads the SPEC-ENGINE §3 drift shapes, the no-op and refusals", () => {
  const out = (o) => JSON.stringify(Object.assign({ eventId: THEME, resolution: "linked", only: false, txId: null,
    events: [], case: null, areaCreated: null, git: null }, o))
  same(M.driftResult("link", 0, out({ resolved: 1, case: { id: "C-2026-005" } }), ""),
    { ok: true, already: false, text: "Linked 1 event to C-2026-005", caseId: "C-2026-005", resolved: 1 })
  same(M.driftResult("explain", 0, out({ resolution: "explained", resolved: 3, case: { id: "C-2026-009" }, areaCreated: "dev-env" }), ""),
    { ok: true, already: false, text: "Explained 3 events · created C-2026-009 · new area dev-env", caseId: "C-2026-009", resolved: 3 })
  same(M.driftResult("dismiss", 0, out({ resolution: "dismissed", resolved: 3 }), ""),
    { ok: true, already: false, text: "Dismissed 3 events", caseId: "", resolved: 3 })
  const noop = (already) => JSON.stringify({ eventId: THEME, resolution: "linked", resolved: 0, events: [], already: already })
  same(M.driftResult("link", 0, noop({ resolution: "linked", case: "C-2026-005" }), ""),
    { ok: true, already: true, text: "Already resolved: linked to C-2026-005", caseId: "C-2026-005", resolved: 0 })
  assert.strictEqual(M.driftResult("explain", 0, noop({ resolution: "explained", case: "C-2026-009" }), "").text,
    "Already resolved: explained · C-2026-009")
  assert.strictEqual(M.driftResult("dismiss", 0, noop({ resolution: "dismissed", case: null }), "").text, "Already resolved: dismissed")
  assert.strictEqual(M.driftResult("link", 0, noop({ resolution: null, case: "C-2026-004" }), "").text,
    "Nothing to resolve: it belongs to C-2026-004")
  assert.strictEqual(M.driftResult("link", 0, noop({ resolution: null, case: null }), "").text, "Nothing to resolve: not open drift")
  same(M.driftResult("link", 1, JSON.stringify({ error: { code: 1, message: "unknown case C-2026-999" } }), ""),
    { ok: false, already: false, text: "unknown case C-2026-999", caseId: "", resolved: 0 })
})

test("driftShowResult, memberLines: the full member list of a group", () => {
  const shown = M.driftShowResult(0, JSON.stringify({ event: {}, open: true, item: null, txId: "t",
    members: [{ id: FIREFOX, kind: "upgrade", subject: "firefox", detail: "a → b" }, { id: "bad" }, { id: NOTO, kind: "upgrade", subject: "noto-fonts" }] }), "")
  same(shown.members.map((m) => m.subject), ["firefox", "noto-fonts"])
  assert.strictEqual(M.driftShowResult(1, "", "boom").ok, false)
  same(M.memberLines(shown.members, 2), ["· upgrade firefox  a → b", "· upgrade noto-fonts"])
  same(M.memberLines(shown.members, 3), ["· upgrade firefox  a → b", "· upgrade noto-fonts", "… and 1 more"])
  const many = Array.from({ length: 12 }, (_, i) => ({ kind: "upgrade", subject: "p" + i, detail: "" }))
  const lines = M.memberLines(many, 12)
  assert.strictEqual(lines.length, 9)
  assert.strictEqual(lines[8], "… and 4 more")
})

test("folded resolutions: explained · C-… (ADR-0021), the crisis target, +N more (ADR-0020)", () => {
  assert.strictEqual(M.rowStatus({ resolution: "explained", caseId: "C-2026-009", resolutionDetail: "why" }), "explained · C-2026-009: why")
  assert.strictEqual(M.rowStatus({ resolution: "dismissed", caseId: "", resolutionDetail: "tried it" }), "dismissed: tried it")
  assert.strictEqual(M.eventResolution(sampleIndex, "01M3VDBX30F5DH0JNY7S0K95GC"), "linked to C-2026-008")
  assert.strictEqual(M.eventResolution(sampleIndex, THEME), "")
  assert.strictEqual(M.firstCrisis(sampleIndex), UNIT)
  assert.strictEqual(M.firstCrisis({ drift: [{ eventId: THEME, crisis: false }] }), THEME)
  assert.strictEqual(M.firstCrisis({ drift: [] }), "")
  assert.strictEqual(M.moreDriftText(sampleIndex), "")
  const capped = JSON.parse(sample)
  capped.summary.openDrift = 250
  assert.strictEqual(M.moreDriftText(capped), "+246 more open drift items not listed here")
  capped.summary.openDrift = 5
  assert.strictEqual(M.moreDriftText(capped), "+1 more open drift item not listed here")
})

// ---- Decisions and Memory (WP-023) ----

test("decisionRows: the sample's four decisions, newest first", () => {
  const rows = M.decisionRows(sampleIndex)
  same(rows.map((r) => r.id + " " + r.status + " " + r.date), [
    "ADR-0004 proposed 2026-10-01", "ADR-0003 accepted 2026-10-01",
    "ADR-0002 accepted 2026-09-02", "ADR-0001 accepted 2026-09-01"])
  same(rows.map((r) => r.tone), ["accent", "", "", ""])
  assert.ok(rows.every((r) => r.actionable))
  assert.strictEqual(rows[0].title, "Ollama nur als User-Service mit Case")
  assert.strictEqual(M.decisionMeta(rows[0]), "2026-10-01 · decisions/ADR-0004-ollama-user-service.md")
  assert.strictEqual(M.decisionSummary(rows), "4 decisions · 1 proposed")
})

test("decisionRows: index order does not matter, broken entries survive", () => {
  const idx = { decisions: [
    { id: "ADR-0002", title: "b", status: "superseded", date: "2026-09-02" },
    null, "x", { id: "ADR-2", title: "bad id", status: "accepted", date: "2026-10-09" },
    { id: "ADR-0010", title: "c", status: "weird", date: "2026-08-01" },
    { id: "ADR-0001", title: 7, status: "accepted" }] }
  const rows = M.decisionRows(idx)
  same(rows.map((r) => r.id), ["ADR-0010", "ADR-0002", "ADR-0001", "ADR-2"])
  same(rows.map((r) => r.actionable), [true, true, true, false])
  same(rows.map((r) => r.status), ["", "superseded", "accepted", "accepted"])
  same(rows.map((r) => r.tone), ["", "muted", "", ""])
  assert.strictEqual(rows[2].title, "")
  assert.strictEqual(M.decisionMeta(rows[2]), "")
  same(M.decisionRows(null), [])
  same(M.decisionRows({ decisions: "no" }), [])
  assert.strictEqual(M.decisionSummary([]), "No decisions yet")
  assert.strictEqual(M.decisionSummary(M.decisionRows({ decisions: [{ id: "ADR-0001", status: "accepted" }] })), "1 decision")
})

test("decideArgs: the title is one argument after --, exactly as typed", () => {
  for (const t of ["Use zed", "--help", 'a "quoted" title', "-rf --case C-2026-001", "$(reboot)", "--", "  padded  "]) {
    same(M.decideArgs(t), { args: ["decide", "--no-edit", "--json", "--", t] })
    assert.strictEqual(M.validateArgs(M.decideArgs(t).args), "", t)
  }
  assert.strictEqual(M.decideArgs("").error, "Give the decision a title")
  assert.strictEqual(M.decideArgs(" \t ").error, "Give the decision a title")
  assert.strictEqual(M.decideArgs(null).error, "Give the decision a title")
  assert.strictEqual(M.decideArgs(42).error, "Give the decision a title")
  assert.strictEqual(M.decideArgs("two\nlines").error, "The title must be one line")
  assert.strictEqual(M.decideArgs("a\rb").error, "The title must be one line")
  assert.ok(M.decideArgs("a\u0000b").error)
})

test("decideResult reads the SPEC-ENGINE §3 decide shape and refusals", () => {
  const out = JSON.stringify({ decision: { id: "ADR-0005", title: "--help", status: "proposed", date: "2026-10-01",
    cases: [], path: "decisions/ADR-0005-help.md" }, editor: null, git: { committed: true } })
  same(M.decideResult(0, out, ""), { ok: true, text: "Created ADR-0005 · --help", decisionId: "ADR-0005" })
  same(M.decideResult(0, JSON.stringify({ decision: { id: "ADR-5; reboot", title: "x" } }), ""),
    { ok: true, text: "Decision created · x", decisionId: "" })
  same(M.decideResult(0, "not json", ""), { ok: true, text: "Decision created", decisionId: "" })
  same(M.decideResult(1, '{"error":{"code":1,"message":"the title must be one line"}}', ""),
    { ok: false, text: "the title must be one line", decisionId: "" })
  same(M.decideResult(4, "", "lock held"), { ok: false, text: "lock held", decisionId: "" })
})

test("memoryRows: the sample's three lessons and two topics", () => {
  const rows = M.memoryRows(sampleIndex)
  same(rows.map((r) => r.kind + " " + r.title), [
    "lesson `omarchy pkg add` statt yay direkt", "lesson Theme-Overrides nie im Omarchy-Repo",
    "lesson Hyprland reload nach bindings.conf", "topic omarchy", "topic hyprland"])
  same(rows.map((r) => r.section), ["LESSONS", "", "", "TOPICS", ""])
  same(rows.map((r) => r.meta), ["", "", "", "memory/omarchy.md · updated 2026-10-01", "memory/hyprland.md · updated 2026-09-13"])
  assert.ok(rows.every((r) => r.target === "logbook"))
  assert.strictEqual(M.validateArgs(M.openArgs(rows[0].target)), "")
  assert.strictEqual(M.memorySummary(rows), "3 lessons · 2 topics")
})

test("memoryRows: every part optional, broken entries left out", () => {
  same(M.memoryRows(null), [])
  same(M.memoryRows({ memory: {} }), [])
  assert.strictEqual(M.memorySummary([]), "")
  const rows = M.memoryRows({ memory: { lessons: ["", 3, "one"], topics: [null, { topic: "" }, { topic: "x", path: "memory/x.md" }, { topic: "y" }] } })
  same(rows.map((r) => r.section + "|" + r.title + "|" + r.meta), ["LESSONS|one|", "TOPICS|x|memory/x.md", "|y|"])
  const topicsOnly = M.memoryRows({ memory: { topics: [{ topic: "t", updated: "2026-09-01" }] } })
  same(topicsOnly.map((r) => r.section + "|" + r.meta), ["TOPICS|updated 2026-09-01"])
  assert.strictEqual(M.memorySummary(topicsOnly), "0 lessons · 1 topic")
})

console.log("model.test.js: " + passed + " passed" + (process.exitCode ? ", some FAILED" : ""))
