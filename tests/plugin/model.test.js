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

test("pillText hides zero parts (SPEC-PLUGIN §4), D by driftInBar (ADR-0028 §4a)", () => {
  for (const mode of [undefined, "crisis", "all", "none", "bogus"]) {
    assert.strictEqual(M.pillText(null, mode), "", String(mode))
    assert.strictEqual(M.pillText({ active: 0, drift: 0, crisis: 0 }, mode), "", String(mode))
    assert.strictEqual(M.pillText({ active: 2, drift: 0, crisis: 0 }, mode), "2", String(mode))
  }
  // all: every open drift item, the behaviour before 0.1.4
  assert.strictEqual(M.pillText({ active: 0, drift: 3, crisis: 0 }, "all"), "· 3")
  assert.strictEqual(M.pillText({ active: 2, drift: 3, crisis: 1 }, "all"), "2 · 3")
  // crisis, the default (also for a missing or unknown value): the crisis count
  for (const mode of [undefined, "", "crisis", "bogus", 7]) {
    assert.strictEqual(M.pillText({ active: 0, drift: 3, crisis: 0 }, mode), "", String(mode))
    assert.strictEqual(M.pillText({ active: 2, drift: 3, crisis: 0 }, mode), "2", String(mode))
    assert.strictEqual(M.pillText({ active: 2, drift: 3, crisis: 1 }, mode), "2 · 1", String(mode))
    assert.strictEqual(M.pillText({ active: 0, drift: 3, crisis: 2 }, mode), "· 2", String(mode))
  }
  // none: never a D
  assert.strictEqual(M.pillText({ active: 0, drift: 3, crisis: 2 }, "none"), "")
  assert.strictEqual(M.pillText({ active: 2, drift: 3, crisis: 2 }, "none"), "2")
})

test("driftInBarMode: the manifest's options, anything else the default", () => {
  same(M.DRIFT_IN_BAR_MODES, ["crisis", "all", "none"])
  assert.strictEqual(M.DRIFT_IN_BAR_DEFAULT, "crisis")
  for (const m of ["crisis", "all", "none"]) assert.strictEqual(M.driftInBarMode(m), m)
  for (const m of [undefined, null, "", "All", "crises", 1, true, {}]) assert.strictEqual(M.driftInBarMode(m), "crisis")
  // the manifest declares the same setting: an enum with these options and default
  const manifest = JSON.parse(fs.readFileSync(path.join(root, "plugin/manifest.json"), "utf8"))
  const entry = manifest.barWidget.schema.find((e) => e.key === "driftInBar")
  assert.strictEqual(entry.type, "enum")
  same(entry.options, M.DRIFT_IN_BAR_MODES)
  assert.strictEqual(entry.defaultValue, M.DRIFT_IN_BAR_DEFAULT)
  assert.strictEqual(manifest.barWidget.defaults.driftInBar, M.DRIFT_IN_BAR_DEFAULT)
})

test("pillTone: crisis beats active beats default", () => {
  assert.strictEqual(M.pillTone(null), "default")
  assert.strictEqual(M.pillTone({ active: 0, crisis: 0 }), "default")
  assert.strictEqual(M.pillTone({ active: 1, crisis: 0 }), "accent")
  // ADR-0028 §4a: attention alone never colours the bar
  assert.strictEqual(M.pillTone({ active: 0, drift: 4, crisis: 0 }), "default")
  assert.strictEqual(M.pillTone({ active: 0, drift: 4, crisis: 0, attention: 4 }), "default")
  assert.strictEqual(M.pillTone({ active: 0, crisis: 1 }), "urgent")
  assert.strictEqual(M.pillTone({ active: 2, crisis: 2 }), "urgent")
})

test("parseIndex accepts the sample and reads its counts", () => {
  const r = M.parseIndex(sample)
  assert.strictEqual(r.ok, true)
  same(M.counts(r.index), { active: 2, queued: 3, drift: 4, crisis: 2, attention: 2 })
  assert.strictEqual(M.pillText(M.counts(r.index)), "2 · 2")
  assert.strictEqual(M.pillText(M.counts(r.index), "all"), "2 · 4")
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

test("tooltipText matches the SPEC-PLUGIN §4 / ADR-0028 §4a example", () => {
  const c = { active: 2, drift: 3, crisis: 0, queued: 0 }
  const now = Date.parse("2026-10-01T17:09:00+02:00")
  assert.strictEqual(M.tooltipText("ok", c, "2026-10-01T17:05:00+02:00", now),
    "Seldon — 2 active cases, 3 changes without a case, last capture 4 min ago")
  assert.strictEqual(M.tooltipText("ok", { active: 2, drift: 8, crisis: 1 }, "2026-10-01T17:05:00+02:00", now),
    "Seldon — 2 active cases, 1 crisis, 7 changes without a case, last capture 4 min ago")
  assert.strictEqual(M.tooltipText("ok", { active: 1, drift: 1, crisis: 1 }, "", now),
    "Seldon — 1 active case, 1 crisis, 0 changes without a case, never captured")
  assert.strictEqual(M.tooltipText("ok", { active: 0, drift: 3, crisis: 2 }, "", now),
    "Seldon — 0 active cases, 2 crises, 1 change without a case, never captured")
  // a summary that counts more crises than drift never goes negative
  assert.strictEqual(M.tooltipText("ok", { active: 0, drift: 1, crisis: 2 }, "", now),
    "Seldon — 0 active cases, 2 crises, 0 changes without a case, never captured")
  assert.strictEqual(M.tooltipText("indexStale", c, "2026-10-01T17:05:00+02:00", now + 3 * H),
    "Seldon — 2 active cases, 3 changes without a case, last capture 3 h ago · index is stale")
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

test("bannerFor engineMissing: the GitHub one-liner while the AUR package does not exist (ADR-0024)", () => {
  const b = M.bannerFor("engineMissing", {})
  assert.strictEqual(b.command, M.INSTALL_ENGINE_COMMAND)
  assert.strictEqual(M.INSTALL_ENGINE_COMMAND,
    "curl -fsSL https://github.com/JohnAndrewsX/jax-seldon/releases/latest/download/install.sh | bash")
  assert.strictEqual(b.detail, M.ENGINE_MISSING_DETAIL)
  assert.ok(b.detail.indexOf("AUR package: coming soon") !== -1)
  assert.ok(b.detail.indexOf("SHA256SUMS") !== -1)
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

test("crisisText: the red strip of SPEC-PLUGIN §5 / ADR-0028 §4b, only with a crisis", () => {
  assert.strictEqual(M.crisisText(sampleIndex), "2 changes that can affect boot, login or the shell have no case")
  const one = JSON.parse(sample)
  one.summary.crisis = 1
  assert.strictEqual(M.crisisText(one), "1 change that can affect boot, login or the shell has no case")
  // attention alone: no strip
  one.summary.crisis = 0
  assert.strictEqual(M.crisisText(one), "")
  assert.strictEqual(M.crisisText(null), "")
})

test("attentionText: the Changelog header's quiet line (ADR-0028 §4b)", () => {
  // the sample: 4 open drift, 2 crises → 2 without a case besides the crises
  assert.strictEqual(M.attentionText(sampleIndex), "2 changes without a case")
  const x = JSON.parse(sample)
  x.summary.openDrift = 3
  x.summary.crisis = 2
  assert.strictEqual(M.attentionText(x), "1 change without a case")
  x.summary.crisis = 3
  assert.strictEqual(M.attentionText(x), "")
  x.summary.crisis = 5
  assert.strictEqual(M.attentionText(x), "", "never negative")
  assert.strictEqual(M.counts(x).attention, 0, "never negative")
  x.summary.openDrift = 0
  x.summary.crisis = 0
  assert.strictEqual(M.attentionText(x), "")
  assert.strictEqual(M.attentionText(null), "")
  same(M.counts(x), { active: 2, queued: 3, drift: 0, crisis: 0, attention: 0 })
})

test("snapperBanner: only for an enabled snapper collector that fails (ADR-0026)", () => {
  assert.strictEqual(M.snapperBanner(sampleIndex), null)
  assert.strictEqual(M.snapperBanner(null), null)
  const b = M.snapperBanner(degraded)
  assert.strictEqual(b.title, "Snapshots not readable")
  assert.strictEqual(b.command, M.SNAPPER_FIX_COMMAND)
  assert.strictEqual(b.command, "sudo setfacl -m u:$USER:rx /.snapshots")
  const message = degraded.state.collectors.find((c) => c.name === "snapper").message
  // the engine's message, then what the fix grants
  assert.strictEqual(b.detail, message + "\n" + M.SNAPPER_FIX_GRANTS)
  assert.strictEqual(M.SNAPPER_FIX_GRANTS, "The command below grants your user read access to the snapshot directory " +
    "listing and the snapshot info files (files inside a snapshot keep their own permissions), nothing else: " +
    "no snapshot creation, change or deletion.")
  same(b.actions.map((a) => a.id), ["terminal", "copy", "capture"])
  assert.strictEqual(b.hint, "")
  const off = JSON.parse(JSON.stringify(degraded))
  off.state.collectors.forEach((c) => { if (c.name === "snapper") c.enabled = false })
  assert.strictEqual(M.snapperBanner(off), null)
  const bare = JSON.parse(JSON.stringify(degraded))
  bare.state.collectors.forEach((c) => { delete c.message })
  assert.strictEqual(M.snapperBanner(bare).detail,
    "The snapper collector has no permission to list snapshots.\n" + M.SNAPPER_FIX_GRANTS)
})

test("snapperBanner: Check again is a capture, the hint follows Run in terminal (WP-054, #2)", () => {
  const b = M.snapperBanner(degraded)
  same(b.actions.map((a) => a.label), ["Run in terminal", "Copy", "Check again"])
  // The same action id as the stale banner's Capture now: Service.fix
  // dispatches both to captureNow(), not to the index-only recheck.
  const capture = M.bannerFor("indexStale", { generatedAt: sampleIndex.generatedAt, nowMs: Date.now() }).actions[0]
  same(capture, { id: "capture", label: "Capture now" })
  assert.strictEqual(b.actions[2].id, capture.id)
  assert.ok(b.actions.every((a) => a.id !== "recheck"))
  assert.strictEqual(M.SNAPPER_HINT, "When the command has finished, press Check again")
  assert.strictEqual(M.snapperBanner(degraded, true).hint, M.SNAPPER_HINT)
  assert.strictEqual(M.snapperBanner(degraded, false).hint, "")
  assert.strictEqual(M.snapperBanner(degraded, "yes").hint, "")
  same(M.snapperBanner(degraded, true).actions, b.actions)
  assert.strictEqual(M.snapperBanner(sampleIndex, true), null)
  assert.strictEqual(M.snapperBanner(null, true), null)
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
  assert.strictEqual(M.rowStatus(theme), "No case · proposed for C-2026-005")
  const tyme = rows.find((r) => r.subject === "io.github.example.tyme")
  assert.strictEqual(M.rowStatus(tyme), "explained: Zeiterfassung nur zum Testen, noch nicht in der Bar.")
  assert.strictEqual(M.rowStatus(rows.find((r) => r.subject === "tailscale")), "linked to C-2026-008")
  assert.strictEqual(M.rowStatus(rows.find((r) => r.subject === "libinput")), "In the open firefox group")
  assert.strictEqual(M.rowStatus(rows.find((r) => r.subject === "ollama")), "Crisis · no case")
  assert.strictEqual(M.rowStatus(rows[0]), "")
  assert.strictEqual(rows[0].dayLabel, "Today")
  assert.strictEqual(rows[0].time, "17:00")
  assert.strictEqual(rows.find((r) => r.subject === "firefox").dayLabel, "Yesterday")
  assert.strictEqual(rows[rows.length - 1].dayLabel, "Tue 1 Sep")
  assert.strictEqual(rows.find((r) => r.subject === "ollama").tone, "urgent")
  assert.strictEqual(theme.tone, "accent")
  assert.strictEqual(rows[0].tone, "")
  // One colour source per row: open drift by its item's class (ADR-0028
  // §4b), so the attention group (members red in the ledger) is accent
  // throughout; every other row is an ordinary, quiet row: muted whatever
  // its zone, no stripe without one.
  for (const s of ["firefox", "libinput", "noto-fonts"]) {
    const r = rows.find((x) => x.subject === s)
    assert.strictEqual(r.zone, "red", s + " ledger zone")
    assert.strictEqual(r.tone, "accent", s)
  }
  // red zone with a case, not drift; red zone resolved (explained, linked);
  // yellow zone resolved: all muted
  for (const s of ["hyprland", "zed", "btop", "tailscale"]) {
    const r = rows.find((x) => x.subject === s)
    assert.strictEqual(r.zone, "red", s + " ledger zone")
    assert.strictEqual(r.drift, false, s)
    assert.strictEqual(r.tone, "muted", s)
  }
  assert.strictEqual(tyme.zone, "yellow")
  assert.strictEqual(tyme.tone, "muted")
  same([...new Set(rows.filter((r) => !r.drift).map((r) => r.tone))].sort(), ["", "muted"])
  assert.strictEqual(rows.filter((r) => !r.drift && r.tone === "").every((r) => r.zone === ""), true)
  // The tone follows `crisis`, never the zone: without a zone, and with
  // zones swapped (a yellow crisis, a red attention item).
  const noZone = JSON.parse(sample)
  noZone.drift.forEach((d) => { delete d.zone })
  const nz = M.changelogRows(noZone, "all")
  assert.strictEqual(nz.find((r) => r.subject === "ollama").tone, "urgent", "crisis without zone")
  assert.strictEqual(nz.find((r) => r.subject === "firefox").tone, "accent", "attention without zone")
  const swapped = JSON.parse(sample)
  swapped.drift.forEach((d) => { d.zone = d.crisis ? "yellow" : "red" })
  const sw = M.changelogRows(swapped, "all")
  assert.strictEqual(sw.find((r) => r.subject === "ollama").tone, "urgent", "yellow crisis")
  assert.strictEqual(sw.find((r) => r.id === EID).tone, "accent", "red attention")
  assert.strictEqual(sw.find((r) => r.subject === "libinput").tone, "accent", "red attention group member")
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
  // "without a case" is the attention count: 4 open drift − 2 crises
  same(t.stats.map((s) => s.label), ["events today", "in 7 days", "active", "queued", "without a case"])
  same(t.stats.map((s) => s.value), [30, 41, 2, 3, 2])
  const over = JSON.parse(sample)
  over.summary.crisis = 9
  assert.strictEqual(M.todayView(over).stats[4].value, 0, "never negative")
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
  same(M.captureResult(0, cap({ written: 0, collectors: [] }), ""), { ok: true, text: "nothing new", warnings: [] })
  same(M.captureResult(0, cap({ written: 1 }), ""), { ok: true, text: "1 new event", warnings: [] })
  same(M.captureResult(0, cap({ written: 12, collectors: [{ name: "pacman", ok: true }, { name: "snapper", ok: false, fix: "x" }] }), ""),
    { ok: true, text: "12 new events · failing: snapper", warnings: [] })
  same(M.captureResult(4, '{"error":{"code":4,"message":"lock held"}}', ""), { ok: false, text: "lock held", warnings: [] })
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
  same(by("C-2026-002"), ["open*", "reopen"])
  // Reopen (WP-101): one click, no arming
  assert.strictEqual(M.caseAction(cases.find(c => c.id === "C-2026-002"), "reopen").label, "Reopen")
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

test("driftItemFor: zone is the ledger zone, tone and labels follow crisis (ADR-0028 §7, §4b)", () => {
  // A crisis on a hook path in the yellow zone (the row WP-109's fixture is
  // to carry), and an attention install that is red in the ledger.
  const x = JSON.parse(sample)
  const unit = x.drift.find((d) => d.eventId === UNIT)
  unit.zone = "yellow"
  unit.subject = "~/.config/omarchy/hooks/post-update.d/10-sync"
  const ollama = x.drift.find((d) => d.eventId === OLLAMA)
  ollama.crisis = false
  const yc = M.driftItemFor(x, UNIT)
  assert.strictEqual(yc.crisis, true)
  assert.strictEqual(yc.zone, "yellow")
  assert.strictEqual(yc.tone, "urgent")
  const ra = M.driftItemFor(x, OLLAMA)
  assert.strictEqual(ra.crisis, false)
  assert.strictEqual(ra.zone, "red")
  assert.strictEqual(ra.tone, "accent")
  // Explain pre-fills the ledger zone: a yellow crisis sends no --zone
  same(M.driftArgs("explain", { eventId: UNIT, text: "x", zone: yc.zone, itemZone: yc.zone }).args,
    ["drift", "explain", UNIT, "--json", "--", "x"])
  // without a zone on the item: the event's zone, never "red" because of crisis
  delete unit.zone
  const ev = x.events.find((e) => e.id === UNIT)
  ev.zone = "yellow"
  assert.strictEqual(M.driftItemFor(x, UNIT).zone, "yellow")
  assert.strictEqual(M.driftItemFor(x, UNIT).tone, "urgent")
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
  assert.strictEqual(M.moreDriftText(capped), "+246 more changes without a case not listed here")
  capped.summary.openDrift = 5
  assert.strictEqual(M.moreDriftText(capped), "+1 more change without a case not listed here")
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

// ---- Prime Radiant (WP-030) --------------------------------------------------

test("periods: ids, keys 1–4, ←/→ wrap, payload", () => {
  same(M.PERIODS.map((p) => p.id), ["30", "90", "365", "all"])
  assert.strictEqual(M.PERIOD_DEFAULT, "90")
  same(["1", "2", "3", "4", "0", "5", "", "12", "a"].map(M.periodForKey), ["30", "90", "365", "all", "", "", "", "", ""])
  assert.strictEqual(M.cyclePeriod("30", -1), "all")
  assert.strictEqual(M.cyclePeriod("all", 1), "30")
  assert.strictEqual(M.cyclePeriod("90", 1), "365")
  assert.strictEqual(M.cyclePeriod("bogus", 1), "365")
  assert.strictEqual(M.overlayPayloadPeriod('{"period":"30"}', "90"), "30")
  assert.strictEqual(M.overlayPayloadPeriod('{"period":"7"}', "90"), "90")
  assert.strictEqual(M.overlayPayloadPeriod("", "365"), "365")
  assert.strictEqual(M.overlayPayloadPeriod("{broken", "all"), "all")
  assert.strictEqual(M.isPeriod("all"), true)
  assert.strictEqual(M.isPeriod(30), false)
})

test("periodWindow: inclusive days ending on the index's today", () => {
  same(M.periodWindow("30", "2026-10-01"), { period: "30", from: "2026-09-02", to: "2026-10-01", days: 30 })
  same(M.periodWindow("90", "2026-10-01"), { period: "90", from: "2026-07-04", to: "2026-10-01", days: 90 })
  same(M.periodWindow("365", "2026-10-01"), { period: "365", from: "2025-10-02", to: "2026-10-01", days: 365 })
  same(M.periodWindow("all", "2026-10-01"), { period: "all", from: "", to: "", days: 0 })
  // Across a leap day, and an unknown period counts as the default.
  same(M.periodWindow("30", "2024-03-01"), { period: "30", from: "2024-02-01", to: "2024-03-01", days: 30 })
  assert.strictEqual(M.periodWindow("x", "2026-10-01").period, "90")
  // No today: unbounded.
  same(M.periodWindow("30", ""), { period: "30", from: "", to: "", days: 30 })
  assert.strictEqual(M.periodCaption(M.periodWindow("30", "2026-10-01")), "30 d · 2026-09-02 – 2026-10-01")
  assert.strictEqual(M.periodCaption(M.periodWindow("all", "2026-10-01")), "All · everything in the index")
})

test("isoWeekMonday: ISO 8601 weeks, week 53 only in long years", () => {
  assert.strictEqual(M.isoWeekMonday("2026-W40"), "2026-09-28")
  assert.strictEqual(M.isoWeekMonday("2026-W01"), "2025-12-29")
  assert.strictEqual(M.isoWeekMonday("2025-W01"), "2024-12-30")
  assert.strictEqual(M.isoWeekMonday("2026-W53"), "2026-12-28")
  assert.strictEqual(M.isoWeekMonday("2020-W53"), "2020-12-28")
  assert.strictEqual(M.isoWeekMonday("2025-W53"), "")
  same(["2026-W00", "2026-W54", "2026-40", "", null].map(M.isoWeekMonday), ["", "", "", "", ""])
})

test("seriesInPeriod: dates, weeks that touch the window, case spans", () => {
  const win = M.periodWindow("30", "2026-10-01")
  const series = {
    heatmap: [{ date: "2026-09-01", total: 1 }, { date: "2026-09-02", total: 2 }, { date: "2026-10-01", total: 3 },
      { date: "2026-10-02", total: 4 }, { date: "bad", total: 5 }, null],
    drift: [{ week: "2026-W35", opened: 1, resolved: 0 }, { week: "2026-W36", opened: 1, resolved: 0 },
      { week: "2026-W41", opened: 1, resolved: 0 }, { week: "x", opened: 1, resolved: 0 }],
    timeline: [
      { kind: "case", ts: "2026-08-01", end: "2026-09-01", label: "ended before" },
      { kind: "case", ts: "2026-08-01", end: "2026-09-02", label: "ends on the first day" },
      { kind: "case", ts: "2026-08-01", end: null, label: "open" },
      { kind: "case", ts: "2026-10-02", end: null, label: "starts after" },
      { kind: "case", ts: "2026-09-10", end: "2026-09-01", label: "ends before it starts" },
      { kind: "snapshot", ts: "2026-09-02T00:10:00+02:00", label: "in" },
      { kind: "release", ts: "2026-09-01T23:59:00+02:00", label: "out" },
      { kind: "crisis", ts: "2026-10-01T17:00:00+02:00", label: "in" },
      { kind: "other", ts: "2026-09-20T00:00:00Z", label: "unknown kind" }
    ]
  }
  same(M.seriesInPeriod(series, "heatmap", win).map((r) => r.total), [2, 3])
  // W36 is Mon 31 Aug – Sun 6 Sep: it touches the window.
  same(M.seriesInPeriod(series, "drift", win).map((r) => r.week), ["2026-W36"])
  same(M.seriesInPeriod(series, "timeline", win).map((r) => r.label), ["ends on the first day", "open", "in", "in"])
  const all = M.periodWindow("all", "2026-10-01")
  assert.strictEqual(M.seriesInPeriod(series, "heatmap", all).length, 4)
  assert.strictEqual(M.seriesInPeriod(series, "drift", all).length, 3)
  assert.strictEqual(M.seriesInPeriod(series, "timeline", all).length, 7)
  same(M.seriesInPeriod(null, "heatmap", win), [])
  same(M.seriesInPeriod({ heatmap: "x" }, "heatmap", win), [])
})

test("periodTable: the sample's counts per period", () => {
  const table = M.periodTable(ok.index)
  assert.strictEqual(table.today, "2026-10-01")
  const rows = (p) => table.periods[p].slots.map((s) => s.id + "=" + s.rows).join(",")
  assert.strictEqual(rows("30"), "heatmap=30,series=2,driftBars=5,riskDonut=4,timeline=17,plan=2")
  assert.strictEqual(rows("90"), "heatmap=90,series=3,driftBars=5,riskDonut=4,timeline=18,plan=2")
  assert.strictEqual(rows("365"), "heatmap=365,series=3,driftBars=5,riskDonut=4,timeline=18,plan=2")
  assert.strictEqual(rows("all"), "heatmap=366,series=3,driftBars=5,riskDonut=4,timeline=18,plan=2")
  const s30 = table.periods["30"].slots
  same(s30.map((s) => s.count), ["30 days", "2 samples", "5 weeks", "8 cases", "17 entries", "2 active cases"])
  same(s30.map((s) => s.detail), ["57 events", "Explicit 324 → 327", "13 opened · 9 resolved",
    "R0 1 · R1 3 · R2 3 · R3 1 · all time", "7 cases · 2 releases · 6 snapshots · 2 crises", "6 of 9 steps done"])
  same(s30.map((s) => s.windowed), [true, true, true, false, true, false])
  assert.strictEqual(table.periods["90"].slots[0].detail, "62 events")
  same(table.periods["30"].series.risk, { R0: 1, R1: 3, R2: 3, R3: 1 })
  assert.strictEqual(table.periods["30"].series.packages[0].date, "2026-09-03")
  // periodView picks a period, the default one for an unknown id.
  assert.strictEqual(M.periodView(table, "365").window.period, "365")
  assert.strictEqual(M.periodView(table, "nope").window.period, "90")
})

test("periodTable: no index, empty series", () => {
  const table = M.periodTable(null)
  assert.strictEqual(table.today, "")
  same(table.periods["30"].slots.map((s) => s.rows), [0, 0, 0, 0, 0, 0])
  same(table.periods["30"].slots.map((s) => s.detail), ["0 events", "No package counts", "0 opened · 0 resolved",
    "R0 0 · R1 0 · R2 0 · R3 0 · all time", "Nothing in this period", "0 of 0 steps done"])
  const charts = table.periods["all"].charts
  same(Object.keys(charts).map((k) => k + "=" + charts[k].empty), ["heatmap=true", "series=true", "driftBars=true",
    "riskDonut=true", "timeline=true", "plan=true"])
  same([charts.heatmap.emptyText, charts.riskDonut.emptyText, charts.plan.emptyText],
    ["no data in this period", "no cases yet · all time", "no active cases"])
  assert.strictEqual(M.periodView(null, "30").window.period, "30")
  const one = M.periodTable({ generatedAt: "2026-10-01T10:00:00Z", series: { packages: [{ date: "2026-09-30", explicit: 7 }] } })
  assert.strictEqual(one.periods["30"].slots[1].detail, "7 explicit")
})

test("periodView without a table: periodTable(null)'s period, no aggregation", () => {
  // The shell injects `service` after creating the overlay, so Overlay.qml's
  // periodData first asks for a view of no table (SPEC-PLUGIN §6).
  const empty = M.periodTable(null)
  const before = M.aggregationCount()
  for (const p of M.PERIODS) {
    same(M.periodView(null, p.id), empty.periods[p.id])
    same(M.emptyPeriodView(p.id), empty.periods[p.id])
  }
  same(M.periodView({ periods: {} }, "30"), empty.periods["30"])
  assert.strictEqual(M.periodView(null, "nope").window.period, "90")
  // Kept per period: the same object on every call.
  assert.strictEqual(M.periodView(null, "30"), M.periodView(undefined, "30"))
  assert.strictEqual(M.aggregationCount() - before, 0)
})

test("overlayMeta and overlayBanner", () => {
  assert.strictEqual(M.overlayMeta(ok.index), "workstation-7f3a · Omarchy 4.0.7-1 · generated 2026-10-01 17:05")
  assert.strictEqual(M.overlayMeta(null), "")
  assert.strictEqual(M.overlayMeta({ generatedAt: "x" }), "")
  assert.strictEqual(M.overlayBanner(null), null)
  const b = M.bannerFor("notInitialised", {})
  const o = M.overlayBanner(b)
  same(o.actions.map((a) => a.id), ["copy"])
  assert.strictEqual(o.title, b.title)
  assert.strictEqual(b.actions.length, 3)
  same(M.overlayBanner(M.bannerFor("indexStale", { generatedAt: "2026-10-01T10:00:00Z", nowMs: gen })).actions, [])
})

test("overlayGrid: 12 columns, three modes, six slots, minimum heights first, then scroll", () => {
  const ids = (g) => g.slots.map((s) => s.id).join(",")
  const wide = M.overlayGrid(1500, 700, 8, 300, 120)
  assert.strictEqual(wide.mode, "wide")
  assert.strictEqual(ids(wide), "heatmap,series,driftBars,riskDonut,timeline,plan")
  assert.strictEqual(wide.contentHeight, 700)
  for (const s of wide.slots) assert.ok(s.x >= 0 && s.y >= 0 && s.x + s.w <= 1500 && s.y + s.h <= 700, JSON.stringify(s))
  same(wide.slots.map((s) => s.w), [1500, 494, 495, 495, 1500, 1500])
  // Weights 3:4:2:2 of 676; rows that fall under 120 get 120 and the
  // rest share what is left; the rounding remainder goes to the heaviest.
  same(wide.slots.map((s) => s.h), [184, 248, 248, 248, 122, 122])
  same(wide.slots.map((s) => s.y), [0, 192, 192, 192, 448, 578])
  // Gaps between neighbours are exactly `gap`.
  assert.strictEqual(wide.slots[2].x - (wide.slots[1].x + wide.slots[1].w), 8)
  assert.strictEqual(wide.slots[3].x - (wide.slots[2].x + wide.slots[2].w), 8)
  // Short: the two small rows hold their minimum, the others shrink, no scroll.
  const short = M.overlayGrid(1500, 560, 8, 300, 120)
  same(short.slots.map((s) => s.h), [126, 170, 170, 170, 120, 120])
  assert.strictEqual(short.contentHeight, 560)
  const medium = M.overlayGrid(700, 700, 8, 300, 120)
  assert.strictEqual(medium.mode, "medium")
  same(medium.slots.map((s) => s.y), [0, 174, 174, 406, 406, 580])
  same(medium.slots.map((s) => s.x + s.w <= 700), [true, true, true, true, true, true])
  const narrow = M.overlayGrid(500, 400, 8, 300, 120)
  assert.strictEqual(narrow.mode, "narrow")
  same(narrow.slots.map((s) => s.h), [120, 120, 120, 120, 120, 120])
  assert.ok(narrow.contentHeight > 400)
  const zero = M.overlayGrid(0, 0, 8, 300, 120)
  assert.strictEqual(zero.slots.length, 6)
  assert.ok(zero.slots.every((s) => s.w >= 0))
})

test("rowHeights: weights, minimums, exact fill", () => {
  // 838 by 3:4:2:2 is 228.5 / 304.7 / 152.4 / 152.4; the two leftover
  // pixels of rounding go to the heaviest row.
  same(M.rowHeights([3, 4, 2, 2], 838, 120), [228, 306, 152, 152])
  same(M.rowHeights([3, 4, 2, 2], 624, 120), [164, 220, 120, 120])
  same(M.rowHeights([3, 4, 2, 2], 400, 120), [120, 120, 120, 120])
  same(M.rowHeights([1], 50, 120), [120])
  for (const free of [480, 500, 640, 700, 1000, 1333]) {
    const h = M.rowHeights([3, 4, 2, 2], free, 120)
    assert.strictEqual(h.reduce((a, b) => a + b, 0), free, String(free))
    assert.ok(h.every((x) => x >= 120))
  }
})

// ---- Charts (WP-031) --------------------------------------------------------

test("day numbers, ISO weeks, colour steps, scale", () => {
  assert.strictEqual(M.dayNumber("1970-01-01"), 0)
  assert.strictEqual(M.dateOfDay(M.dayNumber("2026-10-01")), "2026-10-01")
  assert.ok(Number.isNaN(M.dayNumber("2026-1-01")))
  assert.ok(Number.isNaN(M.dayNumber(null)))
  // Plain arithmetic, checked against Date for every day of 1899–2101, both
  // ways; impossible dates are not dates.
  for (let n = M.dayNumber("1899-01-01"); n <= M.dayNumber("2101-12-31"); n++) {
    const date = new Date(n * 86400000).toISOString().slice(0, 10)
    if (M.dateOfDay(n) !== date || M.dayNumber(date) !== n) assert.fail(n + " " + date)
  }
  same(["2026-02-29", "2026-02-30", "2024-02-29", "2100-02-29", "2000-02-29", "2026-04-31", "2026-13-01", "2026-00-10",
    "2026-01-00", "2026-01-32", "2026-1a-01", "2026/01/01"].map((d) => Number.isFinite(M.dayNumber(d))),
  [false, false, true, false, true, false, false, false, false, false, false, false])
  same(["2026-02-30", "2026-09-30", "x"].map(M.isDate), [false, true, false])
  // Monday = 0: 2026-09-28 is a Monday, 2026-10-04 a Sunday.
  same(["2026-09-28", "2026-10-01", "2026-10-04", "1969-12-29"].map((d) => M.weekdayOfDay(M.dayNumber(d))), [0, 3, 6, 0])
  same(["2026-09-28", "2026-10-04", "2024-12-30", "2021-01-03", "2020-12-31", "2027-01-01"].map(M.isoWeekOf),
    ["2026-W40", "2026-W40", "2025-W01", "2020-W53", "2020-W53", "2026-W53"])
  for (const w of ["2026-W01", "2026-W40", "2020-W53", "2025-W01"]) assert.strictEqual(M.isoWeekOf(M.isoWeekMonday(w)), w)
  // 0 for none; sqrt steps 1–5 against the max.
  same([0, 1, 3, 7, 8, 20, 30, 31].map((v) => M.colourStep(v, 30)), [0, 1, 2, 3, 3, 5, 5, 5])
  same([M.colourStep(5, 0), M.colourStep(-1, 5), M.colourStep(1, 1)], [0, 0, 5])
  assert.strictEqual(M.CHART_STEP_ALPHAS.length, 5)
  assert.strictEqual(M.scale(5, 0, 10, 0, 100), 50)
  assert.strictEqual(M.scale(5, 5, 5, 0, 100), 50)
  assert.strictEqual(M.scale(0, 0, 10, 100, 0), 100)
})

test("splitSeries equals seriesInPeriod for every period and series", () => {
  const today = "2026-10-01"
  const wins = M.PERIODS.map((p) => M.periodWindow(p.id, today))
  const odd = {
    heatmap: [{ date: "2026-09-01", total: 1 }, { date: "bad" }, null, { date: "2026-10-02", total: 4 }, { date: "2025-01-01", total: 2 }],
    packages: [{ date: "2026-08-01", explicit: 1 }, { date: "2026-09-30", explicit: 2 }],
    drift: [{ week: "2026-W27", opened: 1, resolved: 0 }, { week: "2026-W36", opened: 1, resolved: 0 }, { week: "x" }, { week: "2026-W41" }],
    timeline: [
      { kind: "case", ts: "2026-06-01", end: "2026-07-04", label: "ends on the 90-day edge" },
      { kind: "case", ts: "2026-06-01", end: "2026-07-03", label: "ends before" },
      { kind: "case", ts: "2026-06-01", end: null, label: "open" },
      { kind: "case", ts: "2026-09-10", end: "2026-09-01", label: "backwards" },
      { kind: "snapshot", ts: "2026-07-04T00:00:00+02:00", label: "edge" },
      { kind: "other", ts: "2026-09-20T00:00:00Z", label: "unknown" }
    ]
  }
  for (const series of [ok.index.series, odd]) {
    for (const key of ["heatmap", "packages", "drift", "timeline"]) {
      const cut = M.splitSeries(series, key, wins)
      wins.forEach((w, i) => same(cut[i], M.seriesInPeriod(series, key, w)))
    }
  }
  same(M.splitSeries(null, "heatmap", wins), [[], [], [], []])
})

test("heatmapChart: weeks × weekdays, steps, months, hover text, layout and hit test", () => {
  const table = M.periodTable(ok.index)
  const h30 = table.periods["30"].charts.heatmap
  assert.strictEqual(h30.empty, false)
  same(h30.numbers, { days: 30, events: 57, activeDays: 12, max: 30, busiest: "2026-10-01" })
  assert.strictEqual(h30.summary, "57 events on 12 of 30 days · busiest 2026-10-01 (30)")
  // 2026-09-02 is a Wednesday: the first column starts at row 2.
  same([h30.offset, h30.weeks, h30.cells.length], [2, 5, 30])
  same([h30.cells[0].date, h30.cells[0].col, h30.cells[0].row], ["2026-09-02", 0, 2])
  const last = h30.cells[29]
  same([last.date, last.col, last.row, last.total, last.step], ["2026-10-01", 4, 3, 30, 5])
  same(h30.months.map((m) => m.col + m.label), ["0Sep", "4Oct"])
  assert.strictEqual(M.heatmapCellText(last),
    "Thu 2026-10-01 · 30 events · pacman 7 · agent 6 · seldon 6 · snapper 4 · config 2 · manual 2 · omarchy 1 · plugins 1 · theme 1")
  assert.strictEqual(M.heatmapCellText(h30.cells[1]), "Thu 2026-09-03 · 1 event · pacman 1")
  assert.strictEqual(M.heatmapCellText(null), "")
  same([365, 366].map((n) => table.periods[n === 365 ? "365" : "all"].charts.heatmap.weeks), [53, 53])
  assert.strictEqual(table.periods["all"].charts.heatmap.numbers.days, 366)
  // Layout: the smaller of width per week and height per weekday.
  const L = M.heatmapLayout(530, 220, 5, 30, 10)
  same([L.pitch, L.cell, L.x0, L.y0, L.width, L.height], [30, 26, 30, 10, 146, 206])
  assert.strictEqual(M.heatmapLayout(1000, 220, 53, 30, 10).pitch, 18)
  // Hit test: the last cell, the first (row 2 of column 0), the empty
  // cells before it, outside.
  assert.strictEqual(M.heatmapCellAt(h30, L, 30 + 4 * 30 + 5, 10 + 3 * 30 + 5), 29)
  assert.strictEqual(M.heatmapCellAt(h30, L, 30 + 5, 10 + 2 * 30 + 5), 0)
  assert.strictEqual(M.heatmapCellAt(h30, L, 30 + 5, 10 + 5), -1)
  assert.strictEqual(M.heatmapCellAt(h30, L, 5, 15), -1)
  assert.strictEqual(M.heatmapCellAt(h30, L, 30 + 5 * 30 + 5, 15), -1)
  // Days missing from the series count 0 (All runs to today).
  const gap = M.heatmapChart([{ date: "2026-09-28", total: 2 }], M.periodWindow("all", "2026-10-01"), "2026-10-01")
  same(gap.cells.map((c) => c.total), [2, 0, 0, 0])
  same(M.heatmapChart([], M.periodWindow("30", "2026-10-01"), "2026-10-01").empty, true)
})

test("seriesChart: step lines per lane, padded flat lanes, the sample at a day", () => {
  const table = M.periodTable(ok.index)
  const s30 = table.periods["30"].charts.series
  same(s30.numbers, { samples: 2, explicitFirst: 324, explicitLast: 327, totalFirst: 2005, totalLast: 2009 })
  same(s30.lanes.map((l) => [l.key, l.lo, l.hi, l.first, l.last]), [["explicit", 324, 327, 324, 327], ["total", 2005, 2009, 2005, 2009]])
  same([M.dateOfDay(s30.x0), M.dateOfDay(s30.x1 - 1)], ["2026-09-02", "2026-10-01"])
  assert.strictEqual(s30.summary, "explicit 324 → 327 · total 2005 → 2009 · 2 samples")
  // The sample that holds: the last at or before the day, else the first.
  same([M.dayNumber("2026-09-02"), M.dayNumber("2026-09-03"), M.dayNumber("2026-09-30"), M.dayNumber("2026-10-01") + 0.5]
    .map((d) => M.seriesPointAt(s30, d)), [0, 0, 0, 1])
  assert.strictEqual(M.seriesPointText(s30.points[1]), "2026-10-01 · explicit 327 · total 2009")
  // No totals: one lane; a flat lane gets a padded range.
  const flat = M.seriesChart([{ date: "2026-09-30", explicit: 7 }, { date: "2026-09-01", explicit: 7 }], M.periodWindow("all", "2026-10-01"), "2026-10-01")
  same(flat.lanes.map((l) => [l.key, l.lo, l.hi]), [["explicit", 6, 8]])
  same(flat.points.map((p) => p.date), ["2026-09-01", "2026-09-30"])
  assert.strictEqual(flat.summary, "explicit 7 → 7 · 2 samples")
  assert.strictEqual(M.seriesPointText({ date: "2026-09-01", explicit: 7, total: null }), "2026-09-01 · explicit 7")
  assert.strictEqual(M.seriesPointAt(M.seriesChart([], M.periodWindow("30", ""), ""), 5), -1)
})

test("driftChart: weeks with gaps filled, peak, hover text", () => {
  const d = M.periodTable(ok.index).periods["90"].charts.driftBars
  same(d.numbers, { weeks: 5, opened: 13, resolved: 9, max: 6, peak: "2026-W40" })
  assert.strictEqual(d.summary, "13 opened · 9 resolved in 5 weeks · peak 2026-W40")
  assert.strictEqual(M.driftWeekText(d.weeks[4]), "2026-W40 · 28 Sep – 4 Oct · opened 6 · resolved 2")
  const gaps = M.driftChart([{ week: "2026-W40", opened: 1, resolved: 0 }, { week: "2026-W37", opened: 0, resolved: 2 }])
  same(gaps.weeks.map((w) => w.week + ":" + w.opened + "/" + w.resolved), ["2026-W37:0/2", "2026-W38:0/0", "2026-W39:0/0", "2026-W40:1/0"])
  same([gaps.max, gaps.numbers.peak], [2, "2026-W40"])
  // Across a year end with a week 53.
  same(M.driftChart([{ week: "2020-W52", opened: 1, resolved: 0 }, { week: "2021-W02", opened: 1, resolved: 0 }]).weeks.map((w) => w.week),
    ["2020-W52", "2020-W53", "2021-W01", "2021-W02"])
  // The peak is the week with the most opened, not the last one with any.
  const bumpy = M.driftChart([{ week: "2026-W36", opened: 5 }, { week: "2026-W37", opened: 1 }, { week: "2026-W38", opened: 0, resolved: 9 }])
  same(bumpy.numbers, { weeks: 3, opened: 6, resolved: 9, max: 9, peak: "2026-W36" })
  assert.strictEqual(bumpy.summary, "6 opened · 9 resolved in 3 weeks · peak 2026-W36")
  // A tie goes to the later week.
  same(M.driftChart([{ week: "2026-W36", opened: 2 }, { week: "2026-W37", opened: 3 }, { week: "2026-W38", opened: 3 },
    { week: "2026-W39", opened: 1 }]).numbers.peak, "2026-W38")
  same(M.driftChart([]).empty, true)
  assert.strictEqual(M.driftChart([{ week: "2026-W40", opened: 0, resolved: 3 }]).summary, "0 opened · 3 resolved in 1 week")
})

test("riskChart: shares, part at an angle, all time", () => {
  const r = M.periodTable(ok.index).periods["30"].charts.riskDonut
  same(r.numbers, { total: 8, R0: 1, R1: 3, R2: 3, R3: 1 })
  same(r.parts.map((p) => [p.risk, p.count, p.from, p.to]), [["R0", 1, 0, 0.125], ["R1", 3, 0.125, 0.5], ["R2", 3, 0.5, 0.875], ["R3", 1, 0.875, 1]])
  assert.strictEqual(r.summary, "8 cases · R0 1 · R1 3 · R2 3 · R3 1 · all time")
  same([0, 0.1, 0.125, 0.49, 0.5, 0.99, 1.0, -0.25].map((f) => M.riskPartAt(r, f)), [0, 0, 1, 1, 2, 3, 0, 2])
  same([M.riskPartText(r.parts[0]), M.riskPartText(r.parts[2])], ["R0 · 1 case · 13% · all time", "R2 · 3 cases · 38% · all time"])
  // The same object for every period (no dates).
  const t = M.periodTable(ok.index)
  assert.ok(t.periods["30"].charts.riskDonut === t.periods["all"].charts.riskDonut)
  const none = M.riskChart({})
  same([none.empty, none.summary, M.riskPartAt(none, 0.5)], [true, "RiskDonut: no cases yet · all time", -1])
})

test("timelineChart: markers, clipped spans in lanes, months, hit test", () => {
  const t30 = M.periodTable(ok.index).periods["30"].charts.timeline
  same(t30.numbers, { cases: 7, open: 6, releases: 2, snapshots: 6, crises: 2, lanes: 6 })
  assert.strictEqual(t30.summary, "7 cases (6 open) · 2 releases · 6 snapshots · 2 crises")
  same([M.dateOfDay(t30.x0), M.dateOfDay(t30.x1 - 1)], ["2026-09-02", "2026-10-01"])
  same(t30.months.map((m) => M.dateOfDay(m.day) + m.label), ["2026-10-01Oct"])
  // C-2026-002 (closed, 12–13 Sep) and C-2026-003 (open since 26 Sep) share
  // lane 0; open cases run to the end of today.
  const span = (ref) => t30.spans.find((s) => s.ref === ref)
  same([span("C-2026-002").lane, span("C-2026-003").lane, span("C-2026-004").lane], [0, 0, 1])
  same([span("C-2026-002").x1 - span("C-2026-002").x0, span("C-2026-003").x1, span("C-2026-003").open], [2, t30.x1, true])
  const rel = t30.markers.find((m) => m.ref === "4.0.6-1")
  assert.strictEqual(rel.x, M.dayNumber("2026-09-15") + (20 * 60 + 13) / 1440)
  same([M.timelineItemText(rel), M.timelineItemText(span("C-2026-002")), M.timelineItemText(span("C-2026-003"))],
    ["release · Omarchy 4.0.6-1 · 2026-09-15 20:13", "case · C-2026-002 Hyprland-Monitorlayout für Dual-WQHD · 2026-09-12 – 2026-09-13",
      "case · C-2026-003 Omarchy auf 4.0.7 aktualisieren · 2026-09-26 – open"])
  // A span that started before the window is clipped to it.
  const t90 = M.periodTable(ok.index).periods["90"].charts.timeline
  assert.strictEqual(t90.spans.find((s) => s.ref === "C-2026-001").x0, M.dayNumber("2026-09-01"))
  const clipped = M.timelineChart([{ kind: "case", ts: "2026-08-01", end: null, label: "x" }], M.periodWindow("30", "2026-10-01"), "2026-10-01")
  same([clipped.spans[0].x0, clipped.spans[0].x1], [M.dayNumber("2026-09-02"), M.dayNumber("2026-10-02")])
  // Layout and hit test: 300 px for 30 days = 10 px a day.
  const L = M.timelineLayout(100, t30.lanes, 20, 14)
  same([L.bandY, L.laneY0, L.laneH], [10, 20, 80 / 6])
  const relX = M.scale(rel.x, t30.x0, t30.x1, 0, 300)
  same(M.timelineItemAt(t30, L, 300, relX + 3, 10, 5), { kind: "marker", index: t30.markers.indexOf(rel) })
  assert.strictEqual(M.timelineItemAt(t30, L, 300, relX + 30, 10, 5), null)
  const c2 = span("C-2026-002")
  same(M.timelineItemAt(t30, L, 300, M.scale(c2.x0, t30.x0, t30.x1, 0, 300) + 5, 25, 5), { kind: "span", index: t30.spans.indexOf(c2) })
  assert.strictEqual(M.timelineItemAt(t30, L, 300, 2, 25, 5), null)
  same(M.timelineChart([], M.periodWindow("30", "2026-10-01"), "2026-10-01").empty, true)
})

test("planChart: active cases with steps and agent, columns", () => {
  const p = M.periodTable(ok.index).plan
  same(p.numbers, { cases: 2, done: 6, steps: 9 })
  assert.strictEqual(p.summary, "2 active cases · 6 of 9 steps done")
  same(p.rows.map((r) => [r.id, r.tone, r.stepsText, r.progress, r.agent]),
    [["C-2026-003", "urgent", "4/5 steps", 0.8, "agent: claude-code"], ["C-2026-004", "urgent", "2/4 steps", 0.5, "agent: claude-code"]])
  // Every period carries the same plan.
  const t = M.periodTable(ok.index)
  assert.ok(t.periods["30"].charts.plan === t.plan && t.periods["all"].charts.plan === t.plan)
  const odd = M.planChart({ cases: { active: [{ id: "C-2026-009", title: "t", steps: { total: 2, done: 5 } }, { id: "C-2026-010", title: "u" }, null] } })
  same(odd.rows.map((r) => [r.stepsText, r.progress, r.agent]), [["2/2 steps", 1, "no agent"], ["no steps", 0, "no agent"]])
  same([M.planChart(null).empty, M.planChart(null).emptyText], [true, "no active cases"])
  same([M.planColumns(1800, 240, 6, 2), M.planColumns(1800, 240, 6, 9), M.planColumns(400, 240, 6, 3), M.planColumns(0, 240, 6, 3)], [2, 7, 1, 1])
})

test("aggregationCount counts periodTable and chart passes", () => {
  const before = M.aggregationCount()
  M.periodTable(ok.index)
  // periodTable, 4 splits, plan, risk, then 4 periods × 4 windowed charts.
  assert.strictEqual(M.aggregationCount() - before, 1 + 4 + 2 + 16)
  M.heatmapCellText(M.periodTable(ok.index).periods["30"].charts.heatmap.cells[0])
  M.heatmapCellAt(null, {}, 0, 0)
  assert.strictEqual(M.aggregationCount() - before, 2 * 23)
})

// ---- Assets (WP-051) ----------------------------------------------------------

const pluginAssets = path.join(root, "plugin/assets")
const asset = (name) => fs.readFileSync(path.join(pluginAssets, name), "utf8")

test("every file the plugin names exists in plugin/assets/ and is a copy of assets/", () => {
  const named = [M.barGlyph(16).file, M.barGlyph(20).file, M.barGlyph(24).file,
    M.panelMark(11.7, 1).file, M.panelMark(16.1, 1).file, M.panelMark(14.6, 1).file]
  for (const id of Object.keys(M.STATUS_PICTOGRAMS).map((s) => M.statusPictogram(s))
    .concat(["crisis", "drift-open", "case-active", "all-clear"])) {
    named.push(M.pictogramFile(id, 48), M.pictogramFile(id, 96))
  }
  for (const kind of M.MARKER_KINDS) named.push(M.markerFile(kind, 12), M.markerFile(kind, 16))
  assert.strictEqual(new Set(named).size, 3 + 3 + 16 + 10) // A4, A5 and the A1 master, A11, A12
  for (const name of named) {
    const p = path.join(pluginAssets, name)
    assert.ok(!fs.lstatSync(p).isSymbolicLink(), name + " is a symlink")
    assert.ok(fs.readFileSync(p).equals(fs.readFileSync(path.join(root, "assets", name))), name + " differs from assets/")
  }
  for (const name of fs.readdirSync(pluginAssets)) {
    assert.ok(fs.readFileSync(path.join(pluginAssets, name)).equals(fs.readFileSync(path.join(root, "assets", name))),
      name + " differs from assets/")
  }
})

test("tintedSvg sets the root colour of a mask and nothing else", () => {
  for (const name of fs.readdirSync(pluginAssets).filter((n) => n.endsWith(".svg"))) {
    const svg = asset(name)
    // round 3 (F1): the fallback colour on the root only
    assert.strictEqual((svg.match(/\scolor="/g) || []).length, 1, name)
    assert.ok(/<svg\b[^>]*\scolor="#000"/.test(svg), name)
    const url = M.tintedSvg(svg, "#a9b1d6")
    assert.ok(url.startsWith("data:image/svg+xml;utf8,"), name)
    const out = decodeURIComponent(url.slice("data:image/svg+xml;utf8,".length))
    assert.strictEqual(out, svg.replace('color="#000"', 'color="#a9b1d6"'), name)
  }
  assert.strictEqual(M.tintedSvg('<svg viewBox="0 0 1 1"/>', "#123456"),
    "data:image/svg+xml;utf8," + encodeURIComponent('<svg color="#123456" viewBox="0 0 1 1"/>'))
  assert.strictEqual(M.tintedSvg("", "#123456"), "")
  assert.strictEqual(M.tintedSvg(asset("a4-bar-glyph.svg"), "red"), "")
  assert.strictEqual(M.tintedSvg(asset("a4-bar-glyph.svg"), "#80a9b1d6"), "")
})

test("barGlyph: hinted 16 and 20 px boxes with their centre rows, the vector otherwise", () => {
  same(M.barGlyph(16), { file: "a4-bar-glyph-16.svg", crisp: true, centre: 7.5 / 16 })
  same(M.barGlyph(20), { file: "a4-bar-glyph-20.svg", crisp: true, centre: 9.5 / 20 })
  same(M.barGlyph(24), { file: "a4-bar-glyph.svg", crisp: false, centre: 0.5 })
  same(M.barGlyph(19.6), M.barGlyph(20))
  // The hinted grids' ink: rows 3–11 of 16 and 4–14 of 20, centre pixel row 7 and 9.
  for (const [name, top, bottom] of [["a4-bar-glyph-16.svg", 3, 11], ["a4-bar-glyph-20.svg", 4, 14]]) {
    const rows = [...asset(name).matchAll(/<rect [^>]*y="(\d+)"/g)].map((m) => Number(m[1]))
    assert.strictEqual(Math.min(...rows), top, name)
    assert.strictEqual(Math.max(...rows), bottom, name)
  }
})

test("panelMark: the delivered A5 metrics at 16 and 22 px headings", () => {
  // DELIVERY.md §5: cap 11.7 → box 24, gap 6, baseline 17.84; cap 16.1 → 32, 8, 24.03
  const a = M.panelMark(11.7, 1)
  same([a.box, a.gap, a.file, a.crisp], [24, 6, "a5-panel-mark-24.svg", true])
  assert.ok(Math.abs(a.baseline - 17.85) < 0.01)
  const b = M.panelMark(16.1, 1)
  same([b.box, b.gap, b.file, b.crisp], [32, 8, "a5-panel-mark-32.svg", true])
  assert.ok(Math.abs(b.baseline - 24.05) < 0.01)
  // 24 logical px on a 1.25 output are 30 device px: the vector master
  same([M.panelMark(11.7, 1.25).file, M.panelMark(11.7, 1.25).crisp], ["a1-icon-mask.svg", false])
  same([M.panelMark(14.6, 1).box, M.panelMark(14.6, 1).file], [30, "a1-icon-mask.svg"])
})

test("state pictograms: one per non-ok status but the contract mismatch, the day's state by urgency", () => {
  same(["engineMissing", "notInitialised", "indexMissing", "indexStale", "contractMismatch", "ok"].map(M.statusPictogram),
    ["engine-missing", "logbook-not-initialised", "index-missing", "index-stale", "", ""])
  assert.strictEqual(M.todayState(null), null)
  same(M.todayState({ active: 2, drift: 4, crisis: 2 }), { id: "crisis", tone: "urgent" })
  // ADR-0028 §4b: attention alone changes nothing
  same(M.todayState({ active: 2, drift: 4, crisis: 0 }), { id: "case-active", tone: "accent" })
  same(M.todayState({ active: 0, drift: 4, crisis: 0 }), { id: "all-clear", tone: "default" })
  same(M.todayState({ active: 0, drift: 4, crisis: 1 }), { id: "crisis", tone: "urgent" })
  same(M.todayState({ active: 2, drift: 0, crisis: 0 }), { id: "case-active", tone: "accent" })
  same(M.todayState({ active: 0, drift: 0, crisis: 0 }), { id: "all-clear", tone: "default" })
  assert.strictEqual(M.pictogramFile("crisis", 48), "a11-state-crisis-48.svg")
  assert.strictEqual(M.pictogramFile("crisis", 60), "a11-state-crisis-48.svg")
  assert.strictEqual(M.pictogramFile("crisis", 96), "a11-state-crisis-96.svg")
  assert.strictEqual(M.pictogramFile("", 96), "")
})

test("timeline markers: the canvas paths are the A12 16-grid files' paths", () => {
  for (const kind of Object.keys(M.MARKER_PATHS)) {
    const d = asset("a12-marker-" + kind + "-16.svg").match(/\sd="([^"]+)"/)[1]
    assert.strictEqual(M.MARKER_PATHS[kind], d, kind)
  }
  assert.strictEqual(M.markerFile("release", 12), "a12-marker-release-12.svg")
  assert.strictEqual(M.markerFile("crisis", 15), "a12-marker-crisis-16.svg")
  assert.strictEqual(M.markerFile("diamond", 12), "")
  same(M.TIMELINE_LEGEND.map((e) => e.label), ["releases", "snapshots", "cases", "crises"])
})

test("engineMin (WP-068): a version below the manifest's engineMin gets the update banner, equal or above none", () => {
  const manifest = JSON.parse(fs.readFileSync(path.join(root, "plugin/manifest.json"), "utf8"))
  assert.strictEqual(M.engineMinOf(manifest), manifest.seldon.engineMin)
  assert.strictEqual(M.engineMinOf(null), "")
  assert.strictEqual(M.engineMinOf({ seldon: {} }), "")
  const b = M.engineOutdatedBanner("ok", "0.1.9", "0.2.0")
  assert.ok(b)
  assert.strictEqual(b.title, "Engine too old")
  assert.strictEqual(b.tone, "urgent")
  assert.strictEqual(b.command, M.UPDATE_ENGINE_COMMAND)
  assert.ok(b.detail.indexOf("Update the engine to at least 0.2.0") !== -1, b.detail)
  assert.ok(b.detail.indexOf("0.1.9") !== -1, b.detail)
  same(b.actions.map((a) => a.id), ["terminal", "copy", "recheck"])
  assert.strictEqual(M.engineOutdatedBanner("ok", "0.2.0", "0.2.0"), null)
  assert.strictEqual(M.engineOutdatedBanner("ok", "0.2.1", "0.2.0"), null)
  assert.strictEqual(M.engineOutdatedBanner("ok", "1.0.0", "0.2.0"), null)
  // Numbers, not text: 0.10 is newer than 0.9.
  assert.strictEqual(M.engineOutdatedBanner("ok", "0.10.0", "0.9.0"), null)
  assert.ok(M.engineOutdatedBanner("ok", "0.9.0", "0.10.0"))
  // A dev or pre-release build counts as its version.
  assert.strictEqual(M.engineOutdatedBanner("ok", "0.2.0-dev", "0.2.0"), null)
  assert.ok(M.engineOutdatedBanner("ok", "0.1.0-fake", "0.2.0"))
  // The test host's builds from main (WP-098): `+main.<sha>` build metadata.
  same(M.versionCore("0.1.3+main.1a2b3c4"), [0, 1, 3])
  assert.strictEqual(M.versionBelow("0.1.3+main.1a2b3c4", "0.1.3"), false)
  assert.strictEqual(M.versionBelow("0.1.3+main.1a2b3c4", "0.1.4"), true)
  assert.strictEqual(M.engineOutdatedBanner("ok", "0.1.3+main.1a2b3c4", "0.1.3"), null)
  // Unknown on either side: no claim.
  assert.strictEqual(M.engineOutdatedBanner("ok", "", "0.2.0"), null)
  assert.strictEqual(M.engineOutdatedBanner("ok", "0.1.0", ""), null)
  assert.strictEqual(M.engineOutdatedBanner("ok", "garbage", "0.2.0"), null)
  // It replaces the other status banners but the two that name their own fix.
  for (const s of ["notInitialised", "indexMissing", "indexStale"])
    assert.strictEqual(M.engineOutdatedBanner(s, "0.1.0", "0.2.0").title, "Engine too old", s)
  assert.strictEqual(M.engineOutdatedBanner("engineMissing", "0.1.0", "0.2.0"), null)
  assert.strictEqual(M.engineOutdatedBanner("contractMismatch", "0.1.0", "0.2.0"), null)
  // The engine of this tree meets the manifest of this tree.
  const engine = fs.readFileSync(path.join(root, "engine/Cargo.toml"), "utf8").match(/^version = "([^"]+)"/m)[1]
  assert.strictEqual(M.engineOutdatedBanner("ok", engine, manifest.seldon.engineMin), null)
})

test("restart notice (WP-090): the running code's version is the manifest's; another manifest version gets the notice", () => {
  const manifest = JSON.parse(fs.readFileSync(path.join(root, "plugin/manifest.json"), "utf8"))
  // Set by hand with the manifest (docs/VERSIONING.md): they must agree in the repository.
  assert.strictEqual(M.PLUGIN_VERSION, manifest.version)
  assert.strictEqual(M.pluginVersionOf(manifest), manifest.version)
  assert.strictEqual(M.pluginVersionOf(null), "")
  assert.strictEqual(M.pluginVersionOf({ version: 3 }), "")
  assert.strictEqual(M.restartShellNotice(M.PLUGIN_VERSION, M.pluginVersionOf(manifest)), null)
  // Not injected yet: no notice.
  assert.strictEqual(M.restartShellNotice("0.1.4", ""), null)
  const n = M.restartShellNotice("0.1.4", "0.1.5")
  assert.ok(n)
  assert.strictEqual(n.tone, "neutral")
  assert.strictEqual(n.title, "Restart the shell to finish the update")
  assert.ok(n.detail.indexOf("Seldon 0.1.5 is installed") !== -1, n.detail)
  assert.ok(n.detail.indexOf("still runs 0.1.4") !== -1, n.detail)
  assert.strictEqual(n.command, "omarchy-restart-shell")
  same(n.actions, [{ id: "restart", label: "Restart shell" }])
  // One fixed program, no arguments.
  same(M.RESTART_SHELL_ARGV, ["omarchy-restart-shell"])
  // Any difference counts, a downgrade too: the code on disk is not the code running.
  assert.ok(M.restartShellNotice("0.1.5", "0.1.4"))
  // No pictogram: the panel shows it without one.
  assert.strictEqual(M.statusPictogram(n.status), "")
})

test("callWarning (WP-068): one line with the exit code and the first stderr line", () => {
  assert.strictEqual(M.callWarning(["capture", "--all"], 0, "{}", "noise"), "")
  assert.strictEqual(M.callWarning(["capture", "--all"], 2, "", "seldon: boom\nsecond line"),
    "jax.seldon: seldon capture exit 2: seldon: boom")
  assert.strictEqual(M.callWarning(["status", "--json"], 2, "", "\n  \nlate line\n"), "jax.seldon: seldon status exit 2: late line")
  // `--json` errors go to stdout: their message stands in for an empty stderr.
  assert.strictEqual(M.callWarning(["capture"], 4, '{"error":{"code":4,"message":"another seldon process holds the lock"}}', ""),
    "jax.seldon: seldon capture exit 4: another seldon process holds the lock")
  assert.strictEqual(M.callWarning(["log"], 1, "", ""), "jax.seldon: seldon log exit 1: seldon exited with code 1")
})

test("callWarning (WP-078): the JSON fallback is cut to its first line", () => {
  const out = JSON.stringify({ error: { code: 2, message: "\n  index unreadable: line 3\n  caused by: bad utf-8\n" } })
  assert.strictEqual(M.callWarning(["status", "--json"], 2, out, ""), "jax.seldon: seldon status exit 2: index unreadable: line 3")
  assert.strictEqual(M.callWarning(["status", "--json"], 2, out, " \n"), "jax.seldon: seldon status exit 2: index unreadable: line 3")
  const blank = JSON.stringify({ error: { code: 2, message: " \n " } })
  assert.strictEqual(M.callWarning(["capture"], 2, blank, ""), "jax.seldon: seldon capture exit 2: seldon exited with code 2")
})

test("plugin/README.md States lists every banner with its fixes (WP-078)", () => {
  const readme = fs.readFileSync(path.join(root, "plugin/README.md"), "utf8")
  const table = readme.slice(readme.indexOf("### States"), readme.indexOf("\n## ", readme.indexOf("### States")))
  const rows = table.split("\n").filter((l) => l.startsWith("| ") && !l.startsWith("| State "))
  const banners = [
    M.bannerFor("engineMissing"), M.bannerFor("notInitialised"),
    M.bannerFor("indexMissing", { parseError: "empty" }), M.bannerFor("indexMissing", { parseError: "bad json" }),
    M.bannerFor("indexStale", { generatedAt: "2026-10-01T10:00:00+02:00", nowMs: Date.parse("2026-10-01T14:00:00+02:00") }),
    M.bannerFor("contractMismatch", { indexContractVersion: 2 }), M.bannerFor("contractMismatch", { indexContractVersion: 0 }),
    M.engineOutdatedBanner("ok", "0.0.1", "9.0.0"), M.snapperBanner(degraded, false),
    M.captureWarningNotice(["state reset recorded: pacman took a new baseline"])
  ]
  for (const b of banners) {
    assert.ok(b, "a banner")
    const row = rows.find((r) => r.split(" | ")[1].includes(b.title))
    assert.ok(row, "States has a row whose banner is " + b.title)
    for (const a of b.actions) assert.ok(row.includes("*" + a.label + "*"), b.title + ": the row names *" + a.label + "*")
    // The command the fix runs or copies: the install line by name (its
    // `|` is escaped in the table), every other one verbatim.
    if (b.command === M.INSTALL_ENGINE_COMMAND)
      assert.ok(row.includes("GitHub one-liner"), b.title + ": the row names the GitHub one-liner")
    else if (b.command !== "")
      assert.ok(row.includes("`" + b.command + "`"), b.title + ": the row has `" + b.command + "`")
  }
})

test("busy and lock texts (WP-068)", () => {
  assert.strictEqual(M.BUSY_TEXT, "Another action is running — try again in a moment")
  assert.strictEqual(M.LOCK_RETRY_MS, 30000)
  assert.strictEqual(M.LOCK_RETRIES, 3)
  assert.ok(M.LOCK_WAIT_TEXT.indexOf("waiting for another seldon process") === 0)
})

test("pickDrawnWidget (WP-078): the first drawn widget owns IPC, a placeholder only alone", () => {
  const w = (name, visible, width, height) => ({ name, visible, width, height })
  const placeholder = w("placeholder", false, 0, 0)
  const hiddenSized = w("hidden", false, 40, 26)
  const zeroWide = w("zero", true, 0, 26)
  const a = w("a", true, 40, 26)
  const b = w("b", true, 40, 26)
  assert.strictEqual(M.pickDrawnWidget([placeholder, a, b]), a)
  assert.strictEqual(M.pickDrawnWidget([hiddenSized, zeroWide, b]), b)
  assert.strictEqual(M.pickDrawnWidget([a, b], a), b)
  assert.strictEqual(M.pickDrawnWidget([placeholder, a], a), placeholder)
  assert.strictEqual(M.pickDrawnWidget([null, zeroWide, placeholder]), zeroWide)
  assert.strictEqual(M.pickDrawnWidget([], null), null)
  assert.strictEqual(M.pickDrawnWidget(undefined, null), null)
  assert.strictEqual(M.isDrawnWidget(w("x", true, 1, 1)), true)
  assert.strictEqual(M.isDrawnWidget(w("x", true, 1, 0)), false)
})

test("captureResult keeps the capture's warnings; captureWarningNotice (WP-085)", () => {
  const reset = "state reset recorded: pacman, config took a new baseline because ~/.local/state/seldon was missing, " +
    "unreadable or bound to another logbook, so changes made in between may be missing. If you have a backup of it, " +
    "restore it and run `seldon capture` again (user guide: Back up and restore the state directory)"
  const cap = (o) => JSON.stringify(Object.assign({ ok: true, written: 1, collectors: [] }, o))
  // kept as the engine wrote them; blank and non-string entries dropped
  same(M.captureResult(0, cap({ warnings: [reset, "", "  ", 7, null] }), "").warnings, [reset])
  same(M.captureResult(0, cap({ warnings: "not a list" }), "").warnings, [])
  same(M.captureResult(0, cap({}), "").warnings, [])
  // a failed capture carries none, even if its output had a list
  same(M.captureResult(2, cap({ warnings: [reset] }), "boom").warnings, [])

  assert.strictEqual(M.captureWarningNotice([]), null)
  assert.strictEqual(M.captureWarningNotice(undefined), null)
  const one = M.captureWarningNotice([reset])
  assert.strictEqual(one.title, "Capture warned")
  assert.strictEqual(one.tone, "neutral")
  assert.strictEqual(one.detail, reset)
  assert.strictEqual(one.full, "")
  assert.strictEqual(one.command, "")
  same(one.actions, [])
  assert.strictEqual(M.statusPictogram(one.status), "")
  // first line of each warning; the full text for the hover
  const two = M.captureWarningNotice([reset, "\ncannot move ~/.local/state/seldon/owned.json aside: denied\n  caused by: EACCES\n"])
  assert.strictEqual(two.detail, reset + "\ncannot move ~/.local/state/seldon/owned.json aside: denied")
  assert.strictEqual(two.full, reset + "\n\ncannot move ~/.local/state/seldon/owned.json aside: denied\n  caused by: EACCES")
})


// ---- WP-101: one-sentence start, closed-by-agent, reopen, the rules banner

test("WP-101: agent start --new, plan reopen, doctor and rules update are the only new forms", () => {
  for (const intent of ["--help", "Install zed; rm -rf ~", "$(reboot)", "-- x", "a\nb"]) {
    const built = M.agentNewArgs(intent)
    same(built.args, ["agent", "start", "--new", "--json", "--", intent])
    assert.strictEqual(M.validateArgs(built.args), "", intent)
  }
  assert.ok(M.agentNewArgs("  ").error)
  assert.ok(M.agentNewArgs("a\u0000b").error)
  same(M.planArgs("reopen", "C-2026-002").args, ["plan", "reopen", "C-2026-002", "--json"])
  assert.ok(M.planArgs("reopen", "C-26-2; rm").error)
  for (const ok of [["plan", "reopen", "C-2026-002", "--json"], ["doctor", "--json"], ["rules", "update", "--json"]])
    assert.strictEqual(M.validateArgs(ok), "", ok.join(" "))
  for (const bad of [
    ["agent", "start", "--new", "--", "x"],            // --json missing
    ["agent", "start", "--new", "--json"],            // no text
    ["agent", "start", "--new", "--zone", "red", "--json", "--", "x"],
    ["agent", "start", "C-2026-001", "--json", "--", "x"],
    ["plan", "reopen", "C-2026-002"],
    ["plan", "reopen", "C-2026-002", "--json", "--", "x"],
    ["doctor"], ["doctor", "--fix", "--json"],
    ["rules", "update"], ["rules", "update", "--replace", "--json"], ["rules", "--json"]
  ]) assert.notStrictEqual(M.validateArgs(bad), "", bad.join(" "))
})

test("WP-101: the answers of agent start --new and plan reopen", () => {
  const created = JSON.stringify({ launched: true, launcher: "default", program: "omarchy", case: "C-2026-009",
    created: { case: { id: "C-2026-009", title: "Install zed" } } })
  same(M.agentResult(0, created, ""), { ok: true, text: "Created C-2026-009 · Install zed · agent started · launcher default (omarchy)", caseId: "C-2026-009" })
  const refused = JSON.stringify({ error: { code: 1, message: "no default agent: Omarchy has none set; nothing was created. Fix: `omarchy default agent <name>`" } })
  same(M.agentResult(1, refused, ""), { ok: false, text: "no default agent: Omarchy has none set; nothing was created. Fix: `omarchy default agent <name>`", caseId: "" })
  const reopened = JSON.stringify({ case: { id: "C-2026-010" }, reopens: "C-2026-002", earlier: ["C-2026-009", "x; y"] })
  same(M.planResult(0, reopened, ""), { ok: true, text: "Reopened C-2026-002 as C-2026-010 (active) · reopened before as C-2026-009", caseId: "C-2026-010" })
  same(M.planResult(0, JSON.stringify({ case: { id: "C-2026-009" }, reopens: "C-2026-002", earlier: [] }), "").text,
    "Reopened C-2026-002 as C-2026-009 (active)")
  // round 2: the active case an agent works stays
  same(M.planResult(0, JSON.stringify({ case: { id: "C-2026-009" }, reopens: "C-2026-002", earlier: [], activeCase: { kept: "C-2026-004" } }), "").text,
    "Reopened C-2026-002 as C-2026-009 (active) · the active case stays C-2026-004")
  same(M.planResult(0, JSON.stringify({ case: { id: "C-2026-009" }, reopens: "C-2026-002", earlier: [], activeCase: { kept: "x; rm" } }), "").text,
    "Reopened C-2026-002 as C-2026-009 (active)")
})

test("WP-101: closed-by-agent marker, the Completed filter, reopens in the meta line", () => {
  const index = JSON.parse(sample)
  const all = M.workColumns(index)
  const c2 = all[2].cases.find(c => c.id === "C-2026-002")
  same([c2.closedByAgent, c2.reopens], [true, ""])
  same([all[2].cases.find(c => c.id === "C-2026-001").closedByAgent], [false])
  const agent = M.workColumns(index, M.COMPLETED_FILTER_AGENT)
  same(agent[2].cases.map(c => c.id), ["C-2026-002"])
  same(agent.map(c => M.columnHeader(c)), ["QUEUED 3", "ACTIVE 3", "COMPLETED 1 / 2"])
  same(all.map(c => M.columnHeader(c)), ["QUEUED 3", "ACTIVE 3", "COMPLETED 2"])
  // the filter touches the Completed column only
  same(agent[0].cases.length + agent[1].cases.length, all[0].cases.length + all[1].cases.length)
  const reopened = JSON.parse(fs.readFileSync(path.join(root, "fixtures/index-variants/case-reopened.json"), "utf8"))
  const c9 = M.workColumns(reopened)[1].cases.find(c => c.id === "C-2026-009")
  same([c9.reopens, c9.closedByAgent, M.caseMeta(c9)], ["C-2026-002", false, "reopens C-2026-002 · hyprland · priority normal · 0/0 steps"])
  // a tag that names no case id is no reopen; tags that are not strings are ignored
  same(M.workCase({ id: "C-2026-011", tags: ["reopens:x; rm", 7, null, "closed-by-agent"] }, "completed", "completed").reopens, "")
  same(M.workCase({ id: "C-2026-011", tags: "closed-by-agent" }, "completed", "completed").closedByAgent, false)
})

test("WP-101: the rules banner from doctor's rules row", () => {
  const doctor = (row) => JSON.stringify({ checks: [{ name: "engine", status: "ok", message: "x" }, row] })
  const outdated = M.rulesBanner(doctor({ name: "rules", status: "degraded", message: "outdated (v1)", fix: "seldon rules update" }))
  same([outdated.title, outdated.actions.map(a => a.id), outdated.command],
    ["The logbook's agent rules are outdated (v1)", ["update"], ""])
  // an edited block is archived by the same command: still one click
  same(M.rulesBanner(doctor({ name: "rules", status: "degraded", message: "outdated (v2, its text differs)", fix: "seldon rules update (archives your copy)" })).actions.length, 1)
  // damaged or newer: shown, no click
  const damaged = M.rulesBanner(doctor({ name: "rules", status: "error", message: "damaged", fix: "seldon rules update --replace (archives the file)" }))
  same([damaged.actions, damaged.detail], [[], "Fix in a terminal: seldon rules update --replace (archives the file)"])
  same(M.rulesBanner(doctor({ name: "rules", status: "degraded", message: "newer (v3)", fix: "update seldon" })).actions, [])
  for (const quiet of [doctor({ name: "rules", status: "ok", message: "current (v2)" }), "", "not json", JSON.stringify({ checks: "x" })])
    assert.strictEqual(M.rulesBanner(quiet), null)
  // the update's answer: pending hides the click, a refusal is the hint, done changes nothing
  same(M.rulesBannerWith(outdated, { ok: true, pending: true, text: "Updating the rules…" }).actions, [])
  same(M.rulesBannerWith(outdated, { ok: false, pending: false, text: "lock held" }).hint, "lock held")
  assert.strictEqual(M.rulesBannerWith(outdated, { ok: true, pending: false, text: "Rules updated" }), outdated)
  assert.strictEqual(M.rulesBannerWith(null, { ok: false, text: "x" }), null)
  same(M.rulesUpdateResult(0, JSON.stringify({ action: "rewritten" }), ""), { ok: true, text: "Rules updated" })
  same(M.rulesUpdateResult(0, JSON.stringify({ action: "unchanged" }), ""), { ok: true, text: "The rules were current" })
  same(M.rulesUpdateResult(4, JSON.stringify({ error: { code: 4, message: "locked" } }), ""), { ok: false, text: "locked" })
})

console.log("model.test.js: " + passed + " passed" + (process.exitCode ? ", some FAILED" : ""))
