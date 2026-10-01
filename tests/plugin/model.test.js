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
    ["drift", "dismiss", EID, "--reason", "tried it"], ["drift", "dismiss", EID, "--only", "--reason", "tried it"],
    ["drift", "show", EID, "--json"],
    ["decide", "--no-edit", "--", "Use zed"], ["rebuild", "--json"], ["update-impact", "--json"],
    ["open", "journal", "--editor"], ["open", "C-2026-003", "--editor"]
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
    ["drift", "dismiss", EID, "--reason", ""], ["drift", "dismiss", EID, "--reason", "  "],
    ["log", "--case", "C-26-1", "--", "x"], ["log", "--case", "C-2026-001; reboot", "--", "x"],
    ["log", "--case", "--", "x"],
    ["plan", "new", "--zone", "purple", "--risk", "R1", "--", "t"],
    ["plan", "new", "--zone", "red", "--risk", "R9", "--", "t"],
    ["plan", "start", "../C-2026-001"], ["plan", "finish", "C-2026-001"], ["plan", "start", "C-2026-001", "--", "x"],
    ["drift", "link", "01m3vtgny0nzg4ay80814wskgr", "C-2026-005"], ["drift", "link", "81M3VTGNY0NZG4AY80814WSKGR", "C-2026-005"],
    ["drift", "link", EID, "C-2026-005", "--all"], ["drift", "link", EID, "--only", "C-2026-005"],
    ["drift", "explain", EID], ["drift", "explain", EID, "--only"], ["drift", "explain", EID, "--force", "--", "x"],
    ["drift", "dismiss", EID, "reason"], ["drift", "dismiss", EID, "--reason", "x", "--only"],
    ["drift", "dismiss", EID, "--", "x"],
    ["drift", "show", EID], ["drift", "show", "C-2026-005", "--json"], ["drift", "show", EID, "--only", "--json"],
    ["decide", "--", "t"], ["open", "/etc/passwd", "--editor"], ["open", "journal"],
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

test("changelogRows: 58 events newest first, one +3 group, folded resolutions, snapshots", () => {
  const rows = M.changelogRows(sampleIndex, "all")
  assert.strictEqual(rows.length, 58)
  same(rows.map((r) => r.id), sampleIndex.events.map((e) => e.id))
  const badged = rows.filter((r) => r.badge !== "")
  assert.strictEqual(badged.length, 1)
  assert.strictEqual(badged[0].badge, "+3")
  assert.strictEqual(badged[0].subject, "firefox")
  assert.strictEqual(badged[0].txId, "tx-20260930T214115")
  same(rows.filter((r) => r.groupLeader !== "").map((r) => r.subject).sort(), ["libinput", "noto-fonts"])
  assert.strictEqual(rows.filter((r) => r.resolutionDetail !== "").length, 7)
  assert.strictEqual(rows.filter((r) => r.snapshot).length, 6)
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
  assert.strictEqual(M.rowMeta(rows.find((r) => r.subject === "zed")), "0.198.4-1 · claude-code · C-2026-004")
})

test("changelogRows: the source filter narrows the list", () => {
  const counts = M.sourceCounts(sampleIndex)
  assert.strictEqual(counts.all, 58)
  let total = 0
  for (const s of M.SOURCES) {
    const rows = M.changelogRows(sampleIndex, s)
    assert.strictEqual(rows.length, counts[s], s)
    assert.ok(rows.every((r) => r.source === s), s)
    total += rows.length
  }
  assert.strictEqual(total, 58)
  assert.strictEqual(M.changelogRows(sampleIndex, "pacman").length, 12)
  assert.strictEqual(M.changelogRows(sampleIndex, "snapper").length, 8)
  assert.strictEqual(M.changelogRows(sampleIndex, "").length, 58)
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
  same(t.stats.map((s) => s.value), [27, 38, 2, 3, 4])
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
  assert.strictEqual(s[3].rows.length, 4)
  same(s[3].rows[0], { label: "#113  2026-10-01 14:30", value: "pre: ollama" })
  same(s[4].rows[1], { label: "hyprland", value: "1 case · AGENTS.md" })
  const bare = JSON.parse(sample)
  bare.system = {}
  delete bare.state.collectors
  same(M.systemSections(bare, now).map((x) => x.title), ["SELDON"])
  bare.system = { packages: { aur: 3 }, plugins: { installed: 4 }, snapshots: [{ number: 7, ts: "2026-01-01T00:00:00Z", type: "pre" }] }
  same(M.systemSections(bare, now).slice(0, 3).map((x) => x.rows), [[{ label: "AUR", value: "3" }],
    [{ label: "Plugins", value: "4 installed" }], [{ label: "#7  2026-01-01 00:00", value: "pre" }]])
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

console.log("model.test.js: " + passed + " passed" + (process.exitCode ? ", some FAILED" : ""))
