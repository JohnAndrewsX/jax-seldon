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
  same(M.counts(r.index), { active: 2, queued: 3, drift: 3, crisis: 2 })
  assert.strictEqual(M.pillText(M.counts(r.index)), "⟡ 2 · 3")
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

test("validateArgs accepts every CONTRACT.md command form", () => {
  const good = [
    ["--version"], ["--version", "--json"], ["status", "--json"],
    ["capture", "--all", "--json", "--quiet"], ["capture", "--all", "--quiet", "--json"],
    ["log", "Zed läuft"], ["log", "text", "--case", "C-2026-004"],
    ["plan", "new", "A title; rm -rf ~", "--zone", "red", "--risk", "R2"],
    ["plan", "start", "C-2026-005"], ["plan", "drop", "C-2026-1234"],
    ["drift", "link", "01M3VTGNY0NZG4AY80814WSKGR", "C-2026-005"],
    ["drift", "explain", "01M3VTGNY0NZG4AY80814WSKGR", "theme test"],
    ["drift", "dismiss", "01M3VTGNY0NZG4AY80814WSKGR", "--reason", "tried it"],
    ["decide", "Use zed", "--no-edit"], ["rebuild", "--json"], ["update-impact", "--json"],
    ["open", "journal", "--editor"], ["open", "C-2026-003", "--editor"]
  ]
  for (const a of good) assert.strictEqual(M.validateArgs(a), "", JSON.stringify(a))
})

test("validateArgs refuses everything else", () => {
  const bad = [
    [], "status", ["seldon", "status"], ["init"], ["hook", "install", "claude-code"],
    ["status", "--logbook", "/tmp"], ["capture"], ["capture", "--all"],
    ["log"], ["log", ""], ["log", "x", "--case", "C-26-1"], ["log", "x", "--case", "C-2026-001; reboot"],
    ["plan", "new", "t", "--zone", "purple", "--risk", "R1"], ["plan", "new", "t", "--zone", "red", "--risk", "R9"],
    ["plan", "start", "../C-2026-001"], ["plan", "finish", "C-2026-001"],
    ["drift", "link", "01m3vtgny0nzg4ay80814wskgr", "C-2026-005"], ["drift", "link", "81M3VTGNY0NZG4AY80814WSKGR", "C-2026-005"],
    ["drift", "explain", "01M3VTGNY0NZG4AY80814WSKGR"], ["drift", "dismiss", "01M3VTGNY0NZG4AY80814WSKGR", "reason"],
    ["decide", "t"], ["open", "/etc/passwd", "--editor"], ["open", "journal"],
    ["log", 42], ["log", "a\u0000b"]
  ]
  for (const a of bad) assert.notStrictEqual(M.validateArgs(a), "", JSON.stringify(a))
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
