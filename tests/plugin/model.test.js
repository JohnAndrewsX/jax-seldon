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
const v3 = fs.readFileSync(path.join(root, "fixtures/invalid/index.contract-v3.json"), "utf8")
// a contract-1 index (a 0.1.x engine): the sample as v1 wrote it
const v1 = JSON.stringify(Object.assign(JSON.parse(sample), { contractVersion: 1 }))

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
  same(M.counts(r.index), { active: 2, queued: 3, drift: 6, crisis: 2, attention: 4 })
  assert.strictEqual(M.pillText(M.counts(r.index)), "2 · 2")
  assert.strictEqual(M.pillText(M.counts(r.index), "all"), "2 · 6")
})

test("parseIndex reports a contract mismatch with the version found", () => {
  const r = M.parseIndex(v3)
  assert.strictEqual(r.ok, false)
  assert.strictEqual(r.error, "contract")
  assert.strictEqual(r.contractVersion, 3)
  assert.strictEqual(r.index, null)
  const old = M.parseIndex(v1)
  assert.strictEqual(old.error, "contract")
  assert.strictEqual(old.contractVersion, 1)
})

// ADR-0035: the plugin reads contract 2 and accepts its new fields; the
// surfaces that show them come with the desk (WP-122–125)
test("parseIndex accepts the contract-2 fields of the sample", () => {
  assert.strictEqual(M.CONTRACT_VERSION, 2)
  const r = M.parseIndex(sample)
  assert.strictEqual(r.ok, true)
  const ix = r.index
  same(ix.logbook.git.autocommit, { ok: true, at: "2026-10-01T17:00:01+02:00", message: "seldon: note C-2026-004" })
  same(ix.triage.counts, { items: 3, crises: 1 })
  assert.strictEqual(ix.triage.path.indexOf("proposals/"), 0)
  same(ix.decisions.filter((d) => d.cases.length === 2).map((d) => d.id), ["ADR-0003"])
  assert.strictEqual(ix.events.filter((e) => e.meta && e.meta.truncated === true).length, 1)
  same(ix.events.filter((e) => e.kind === "state-loss" || e.kind === "case-updated").map((e) => e.kind),
    ["case-updated", "state-loss"])
  // a minimal v2 index without the optional fields is fine too
  const bare = JSON.parse(sample)
  delete bare.triage
  delete bare.logbook.git
  assert.strictEqual(M.parseIndex(JSON.stringify(bare)).ok, true)
})

test("parseIndex rejects empty, broken and non-object input", () => {
  assert.strictEqual(M.parseIndex("").error, "empty")
  assert.strictEqual(M.parseIndex(null).error, "empty")
  assert.strictEqual(M.parseIndex("{").error, "parse")
  assert.strictEqual(M.parseIndex("[1]").error, "shape")
  assert.strictEqual(M.parseIndex('{"contractVersion":2}').error, "shape")
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
  assert.strictEqual(status({ parse: M.parseIndex(v3) }), "contractMismatch")
  assert.strictEqual(status({ parse: M.parseIndex(v1) }), "contractMismatch")
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

// Which script goes with which command; UPDATE_ENGINE_COMMAND is the
// install line while the AUR package does not exist, so the update banners
// are checked by name below.
const TERMINAL_SCRIPT_OF = {}
TERMINAL_SCRIPT_OF[M.INIT_COMMAND] = M.INIT_SCRIPT
TERMINAL_SCRIPT_OF[M.SNAPPER_FIX_COMMAND] = M.SNAPPER_FIX_SCRIPT
TERMINAL_SCRIPT_OF[M.UPDATE_PLUGIN_COMMAND] = M.UPDATE_PLUGIN_SCRIPT
TERMINAL_SCRIPT_OF[M.INSTALL_ENGINE_COMMAND] = M.INSTALL_ENGINE_SCRIPT

test("bannerFor: one banner per non-ok status, each with a fix", () => {
  assert.strictEqual(M.bannerFor("ok", {}), null)
  const constants = ["", M.INSTALL_ENGINE_COMMAND, M.INIT_COMMAND, M.UPDATE_ENGINE_COMMAND, M.UPDATE_PLUGIN_COMMAND]
  for (const s of M.STATUSES.filter((x) => x !== "ok")) {
    const b = M.bannerFor(s, { indexContractVersion: 3, generatedAt: "2026-10-01T17:05:12+02:00", nowMs: gen + 3 * H })
    assert.ok(b, s)
    assert.strictEqual(b.status, s)
    assert.ok(b.actions.length >= 1, s + " has a fix")
    assert.ok(constants.indexOf(b.command) !== -1, s + " command is a constant")
    for (const a of b.actions) assert.ok(["copy", "terminal", "recheck", "build", "capture"].indexOf(a.id) !== -1, a.id)
    if (b.actions.some((a) => a.id === "copy" || a.id === "terminal")) assert.notStrictEqual(b.command, "", s)
    // The terminal runs the script that goes with the shown command (WP-117).
    if (b.actions.some((a) => a.id === "terminal")) {
      assert.ok(M.isTerminalScript(b.script), s + " script is a terminal script")
      assert.strictEqual(b.script, TERMINAL_SCRIPT_OF[b.command], s + " script matches its command")
    } else {
      assert.ok(!b.script, s + " has no script")
    }
  }
})

test("bannerFor engineMissing: a setup step without an index, urgent when the engine is gone (WP-117)", () => {
  const b = M.bannerFor("engineMissing", {})
  assert.strictEqual(b.command, M.INSTALL_ENGINE_COMMAND)
  assert.strictEqual(M.INSTALL_ENGINE_COMMAND,
    "curl -fsSL https://github.com/JohnAndrewsX/jax-seldon/releases/latest/download/install.sh | bash")
  assert.strictEqual(b.script, M.INSTALL_ENGINE_SCRIPT)
  assert.strictEqual(b.tone, "accent")
  assert.strictEqual(b.title, "Install the engine")
  assert.strictEqual(b.detail, M.ENGINE_MISSING_DETAIL)
  assert.strictEqual(M.ENGINE_MISSING_DETAIL,
    "Downloads seldon from the Seldon release on GitHub into ~/.local/bin and checks it; runs as your user, no password.")
  same(b.actions, [{ id: "terminal", label: "Install" }, { id: "copy", label: "Copy" }, { id: "recheck", label: "Check again" }])
  assert.strictEqual(M.bannerFor("engineMissing", { indexExists: false }).tone, "accent")
  // only `true` counts
  assert.strictEqual(M.bannerFor("engineMissing", { indexExists: "yes" }).tone, "accent")
  const gone = M.bannerFor("engineMissing", { indexExists: true })
  assert.strictEqual(gone.tone, "urgent")
  assert.strictEqual(gone.title, "Seldon engine missing")
  assert.strictEqual(gone.detail, b.detail)
  same(gone.actions, b.actions)
  assert.strictEqual(gone.script, b.script)
})

test("bannerFor notInitialised: Create runs seldon init in the terminal (WP-117)", () => {
  const b = M.bannerFor("notInitialised", { indexExists: true })
  assert.strictEqual(b.tone, "accent")
  assert.strictEqual(b.title, "Create your logbook")
  assert.strictEqual(b.detail, "Sets up your logbook and starts recording; the terminal asks a few questions, no password.")
  assert.strictEqual(b.command, "seldon init")
  assert.strictEqual(b.script, M.INIT_SCRIPT)
  same(b.actions, [{ id: "terminal", label: "Create" }, { id: "copy", label: "Copy" }, { id: "recheck", label: "Check again" }])
})

test("bannerFor contractMismatch names the side to update", () => {
  const plugin = M.bannerFor("contractMismatch", { indexContractVersion: 3 })
  const engine = M.bannerFor("contractMismatch", { indexContractVersion: 1 })
  assert.strictEqual(M.bannerFor("contractMismatch", { indexContractVersion: 0 }).command, M.UPDATE_ENGINE_COMMAND)
  assert.strictEqual(plugin.command, M.UPDATE_PLUGIN_COMMAND)
  assert.strictEqual(plugin.script, M.UPDATE_PLUGIN_SCRIPT)
  assert.strictEqual(engine.command, M.UPDATE_ENGINE_COMMAND)
  assert.strictEqual(engine.script, M.UPDATE_ENGINE_SCRIPT)
  assert.strictEqual(plugin.detail, "The index uses contract v3 and this plugin reads v2: update the plugin.")
  assert.strictEqual(engine.detail, "The index uses contract v1 and this plugin reads v2: update the engine.")
  same(plugin.actions, [{ id: "terminal", label: "Update" }, { id: "copy", label: "Copy" }])
})

// The terminal scripts (WP-117), verbatim: what the user reads in the
// floating terminal and what bash runs. A text change here is a review
// item (AGENTS.md §8).
const SCRIPTS = {
  INSTALL_ENGINE_SCRIPT: "seldon_cancelled=; trap 'seldon_cancelled=1' INT TERM; " +
    "gum style --bold 'Seldon: install the engine'; " +
    "gum style --width 72 'Downloads seldon from the Seldon release on GitHub into ~/.local/bin and checks it against the release checksums. Runs as your user, no password.'; " +
    "gum style --padding '1 0 1 2' 'curl -fsSL https://github.com/JohnAndrewsX/jax-seldon/releases/latest/download/install.sh | bash'; " +
    "if [ -z \"$seldon_cancelled\" ] && (set -o pipefail; curl -fsSL https://github.com/JohnAndrewsX/jax-seldon/releases/latest/download/install.sh | bash); then gum style --padding '1 0 0 0' --foreground 2 'The engine is installed. In the Seldon panel, press Check again.'; trap - INT TERM; " +
    "elif [ -n \"$seldon_cancelled\" ]; then gum style --padding '1 0 0 0' --foreground 3 'Cancelled. The install did not finish. Run it again; your logbook is untouched.'; trap - INT TERM; (exit 130); " +
    "else gum style --padding '1 0 0 0' --foreground 1 'The install did not finish. Run it again; your logbook is untouched.'; " +
    "trap - INT TERM; fi",
  UPDATE_ENGINE_SCRIPT: "seldon_cancelled=; trap 'seldon_cancelled=1' INT TERM; " +
    "gum style --bold 'Seldon: update the engine'; " +
    "gum style --width 72 'Downloads the latest seldon from the Seldon release on GitHub into ~/.local/bin and checks it against the release checksums. Runs as your user, no password; your logbook stays as it is.'; " +
    "gum style --padding '1 0 1 2' 'curl -fsSL https://github.com/JohnAndrewsX/jax-seldon/releases/latest/download/install.sh | bash'; " +
    "if [ -z \"$seldon_cancelled\" ] && (set -o pipefail; curl -fsSL https://github.com/JohnAndrewsX/jax-seldon/releases/latest/download/install.sh | bash); then seldon status >/dev/null 2>&1 || true; " +
    "gum style --padding '1 0 0 0' --foreground 2 'The engine is updated. In the Seldon panel, press Check again.'; trap - INT TERM; " +
    "elif [ -n \"$seldon_cancelled\" ]; then gum style --padding '1 0 0 0' --foreground 3 'Cancelled. The update did not finish. Run it again; your logbook is untouched.'; trap - INT TERM; (exit 130); " +
    "else gum style --padding '1 0 0 0' --foreground 1 'The update did not finish. Run it again; your logbook is untouched.'; " +
    "trap - INT TERM; fi",
  UPDATE_PLUGIN_SCRIPT: "seldon_cancelled=; trap 'seldon_cancelled=1' INT TERM; " +
    "gum style --bold 'Seldon: update the plugin'; " +
    "gum style --width 72 'Omarchy fetches the new jax.seldon, shows what changes and asks before it updates. No password.'; " +
    "gum style --padding '1 0 1 2' 'omarchy plugin update jax.seldon'; " +
    "if [ -z \"$seldon_cancelled\" ] && (set -o pipefail; omarchy plugin update jax.seldon); then gum style --padding '1 0 0 0' --foreground 2 'If the plugin was updated, the Seldon panel offers Restart shell to load it.'; trap - INT TERM; " +
    "elif [ -n \"$seldon_cancelled\" ]; then gum style --padding '1 0 0 0' --foreground 3 'Cancelled. The plugin update did not finish.'; trap - INT TERM; (exit 130); " +
    "else gum style --padding '1 0 0 0' --foreground 1 'Nothing changed. The plugin stays at its version.'; " +
    "trap - INT TERM; fi",
  INIT_SCRIPT: "seldon_cancelled=; trap 'seldon_cancelled=1' INT TERM; " +
    "gum style --bold 'Seldon: create your logbook'; " +
    "gum style --width 72 'Sets up the logbook folder and starts recording. Asks a few questions; Enter takes the suggested answer. No password.'; " +
    "gum style --padding '1 0 1 2' 'seldon init'; " +
    "if [ -z \"$seldon_cancelled\" ] && (set -o pipefail; seldon init); then gum style --padding '1 0 0 0' --foreground 2 'Your logbook is ready. The panel updates by itself.'; trap - INT TERM; " +
    "elif [ -n \"$seldon_cancelled\" ]; then gum style --padding '1 0 0 0' --foreground 3 'Cancelled. Press Create in the panel to start again.'; trap - INT TERM; (exit 130); " +
    "else gum style --padding '1 0 0 0' --foreground 1 'No logbook was created; the message above says why. Press Create in the panel to try again.'; " +
    "trap - INT TERM; fi",
  SNAPPER_FIX_SCRIPT: "seldon_cancelled=; trap 'seldon_cancelled=1' INT TERM; " +
    "gum style --bold 'Seldon: let your user read the snapshot list'; " +
    "gum style --width 72 'Grants read access to /.snapshots: the listing and the snapshot info files, nothing else. No snapshot is created, changed or deleted. Asks for your password once.'; " +
    "gum style --padding '1 0 1 2' 'sudo setfacl -m u:$USER:rx /.snapshots'; " +
    "if [ -z \"$seldon_cancelled\" ] && (set -o pipefail; sudo setfacl -m u:${USER:?}:rx /.snapshots); then if seldon capture >/dev/null 2>&1 || { sleep 3; seldon capture >/dev/null 2>&1; }; then gum style --padding '1 0 0 0' --foreground 2 'Snapshots are now recorded. The panel updates by itself.'; " +
    "else gum style --padding '1 0 0 0' --foreground 2 'Read access granted. Seldon records snapshots at its next capture.'; fi; trap - INT TERM; " +
    "elif [ -n \"$seldon_cancelled\" ]; then gum style --padding '1 0 0 0' --foreground 3 'Cancelled. Nothing changed.'; trap - INT TERM; (exit 130); " +
    "else gum style --padding '1 0 0 0' --foreground 1 'Nothing changed. Snapshots stay off; Seldon works without them.'; " +
    "trap - INT TERM; fi"
}

test("terminal scripts: verbatim, fixed, each shows and runs its command (WP-117, AGENTS.md §8)", () => {
  for (const name of Object.keys(SCRIPTS)) assert.strictEqual(M[name], SCRIPTS[name], name)
  same(M.TERMINAL_SCRIPTS, Object.keys(SCRIPTS).map((n) => SCRIPTS[n]))
  const commandOf = {
    INSTALL_ENGINE_SCRIPT: M.INSTALL_ENGINE_COMMAND, UPDATE_ENGINE_SCRIPT: M.UPDATE_ENGINE_COMMAND,
    UPDATE_PLUGIN_SCRIPT: M.UPDATE_PLUGIN_COMMAND, INIT_SCRIPT: M.INIT_COMMAND, SNAPPER_FIX_SCRIPT: M.SNAPPER_FIX_COMMAND
  }
  // The grant's run line stops on an empty USER instead of granting
  // `u::rx` (round 2, N5); the shown command is the one Copy copies.
  const runOf = { SNAPPER_FIX_SCRIPT: "sudo setfacl -m u:${USER:?}:rx /.snapshots" }
  assert.strictEqual(runOf.SNAPPER_FIX_SCRIPT.replace("${USER:?}", "$USER"), M.SNAPPER_FIX_COMMAND)
  for (const name of Object.keys(commandOf)) {
    const script = M[name]
    const command = commandOf[name]
    const run = runOf[name] || command
    // shown, single-quoted, as Copy puts it on the clipboard; then run,
    // unless ^C came first
    assert.ok(script.indexOf("gum style --padding '1 0 1 2' '" + command + "'; ") !== -1, name + " shows " + command)
    assert.ok(script.indexOf("if [ -z \"$seldon_cancelled\" ] && (set -o pipefail; " + run + "); then ") !== -1, name + " runs " + run)
    assert.strictEqual(script.split(run).length - 1, run === command ? 2 : 1, name + ": the run line once, shown once")
    // ^C and TERM are noted, never fatal, and reset before the end
    assert.strictEqual(script.indexOf("seldon_cancelled=; trap 'seldon_cancelled=1' INT TERM; gum style --bold 'Seldon: "), 0, name)
    // green, red, and the cancelled line (palette 3) ending in 130, the
    // wrapper's "cancelled": no Done, the window closes
    assert.ok(script.indexOf("--foreground 2 '") !== -1 && script.indexOf("--foreground 1 '") !== -1, name)
    assert.ok(/--foreground 3 'Cancelled\. [^']*'; trap - INT TERM; \(exit 130\); else /.test(script), name)
    assert.ok(/'; trap - INT TERM; fi$/.test(script), name)
    assert.ok(M.isTerminalScript(script), name)
  }
  // no result line claims that nothing changed after install.sh may have
  // replaced the binary (round 2, N4)
  for (const name of ["INSTALL_ENGINE_SCRIPT", "UPDATE_ENGINE_SCRIPT"])
    assert.strictEqual(M[name].indexOf("Nothing changed"), -1, name)
  // the grant says "recorded" only after a capture that succeeded (N3)
  assert.ok(M.SNAPPER_FIX_SCRIPT.indexOf("then if seldon capture >/dev/null 2>&1 || { sleep 3; seldon capture >/dev/null 2>&1; }; " +
    "then gum style --padding '1 0 0 0' --foreground 2 'Snapshots are now recorded.") !== -1)
  assert.strictEqual(M.SNAPPER_FIX_SCRIPT.indexOf("|| true"), -1)
  // the commands contain no quote that could end the shown text early
  for (const c of Object.values(commandOf)) assert.strictEqual(c.indexOf("'"), -1, c)
  assert.strictEqual(M.isTerminalScript(M.SNAPPER_FIX_COMMAND), false)
  assert.strictEqual(M.isTerminalScript(M.SNAPPER_FIX_SCRIPT + " "), false)
  assert.strictEqual(M.isTerminalScript(""), false)
  assert.strictEqual(M.isTerminalScript(undefined), false)
  assert.strictEqual(M.isTerminalScript([M.INIT_SCRIPT]), false)
  // terminalArgv decides what Service.fix launches (round 2, N1): the
  // launcher with a known script, nothing for a forged banner.
  const launcher = "omarchy-launch-floating-terminal-with-presentation"
  for (const script of M.TERMINAL_SCRIPTS) same(M.terminalArgv({ script: script }), [launcher, script])
  same(M.terminalArgv(M.bannerFor("notInitialised", {})), [launcher, M.INIT_SCRIPT])
  for (const forged of [{ script: "id" }, { script: M.INIT_COMMAND, command: M.INIT_COMMAND }, { script: M.INIT_SCRIPT + "; id" },
    { script: "" }, { script: [M.INIT_SCRIPT] }, { command: M.INIT_COMMAND }, {}, null, undefined, M.INIT_SCRIPT,
    M.bannerFor("indexStale", { generatedAt: "2026-10-01T10:00:00+02:00", nowMs: 0 })])
    assert.strictEqual(M.terminalArgv(forged), null, JSON.stringify(forged))
  assert.strictEqual(M.shellQuoted("it's"), "'it'\\''s'")
})

test("terminal scripts: nothing from the index reaches them (AGENTS.md §8)", () => {
  const evil = "'; rm -rf ~; echo '$(reboot)`id`"
  const x = JSON.parse(fs.readFileSync(path.join(root, "fixtures/index-variants/snapper-degraded.json"), "utf8"))
  x.state.collectors.forEach((c) => { if (c.name === "snapper") c.message = evil })
  x.generatedAt = evil
  const banners = [
    M.snapperBanner(x), M.bannerFor("engineMissing", { indexExists: true, generatedAt: evil }),
    M.bannerFor("notInitialised", { generatedAt: evil }),
    M.bannerFor("contractMismatch", { indexContractVersion: evil }), M.bannerFor("contractMismatch", { indexContractVersion: 2 }),
    M.engineOutdatedBanner("ok", "0.0.1-" + evil, "9.0.0")
  ]
  for (const b of banners) {
    assert.ok(b && M.isTerminalScript(b.script), b && b.title)
    assert.strictEqual(b.script.indexOf("rm -rf"), -1, b.title)
    assert.strictEqual(b.command.indexOf("rm -rf"), -1, b.title)
  }
  // the engine's message is the hover text, as plain text, never the script
  assert.ok(banners[0].full.indexOf(evil) === 0)
})

test("terminal scripts: bash parses each one", () => {
  const { execFileSync } = require("child_process")
  for (const script of M.TERMINAL_SCRIPTS) execFileSync("bash", ["-n", "-c", script])
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
  // the sample: 6 open drift, 2 crises → 4 without a case besides the crises
  assert.strictEqual(M.attentionText(sampleIndex), "4 changes without a case")
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
  assert.strictEqual(b.title, "Read snapshots (optional)")
  assert.strictEqual(b.tone, "accent")
  assert.strictEqual(b.command, M.SNAPPER_FIX_COMMAND)
  assert.strictEqual(b.command, "sudo setfacl -m u:$USER:rx /.snapshots")
  assert.strictEqual(b.script, M.SNAPPER_FIX_SCRIPT)
  // one sentence; the engine's message, then what the fix grants, on hover (WP-117)
  assert.strictEqual(b.detail, "A one-time read grant on /.snapshots; it asks for your password once, and Seldon works without it.")
  const message = degraded.state.collectors.find((c) => c.name === "snapper").message
  assert.strictEqual(b.full, message + "\n" + M.SNAPPER_FIX_GRANTS)
  assert.strictEqual(M.SNAPPER_FIX_GRANTS, "The command below grants your user read access to the snapshot directory " +
    "listing and the snapshot info files (files inside a snapshot keep their own permissions), nothing else: " +
    "no snapshot creation, change or deletion.")
  same(b.actions.map((a) => a.id), ["terminal", "copy", "capture"])
  assert.strictEqual(b.hint, undefined)
  const off = JSON.parse(JSON.stringify(degraded))
  off.state.collectors.forEach((c) => { if (c.name === "snapper") c.enabled = false })
  assert.strictEqual(M.snapperBanner(off), null)
  const bare = JSON.parse(JSON.stringify(degraded))
  bare.state.collectors.forEach((c) => { delete c.message })
  assert.strictEqual(M.snapperBanner(bare).full,
    "The snapper collector has no permission to list snapshots.\n" + M.SNAPPER_FIX_GRANTS)
})

test("snapperBanner: Grant runs the script, Check again is a capture, no hint (WP-054, WP-117)", () => {
  const b = M.snapperBanner(degraded)
  same(b.actions.map((a) => a.label), ["Grant", "Copy", "Check again"])
  // The same action id as the stale banner's Capture now: Service.fix
  // dispatches both to captureNow(), not to the index-only recheck.
  const capture = M.bannerFor("indexStale", { generatedAt: sampleIndex.generatedAt, nowMs: Date.now() }).actions[0]
  same(capture, { id: "capture", label: "Capture now" })
  assert.strictEqual(b.actions[2].id, capture.id)
  assert.ok(b.actions.every((a) => a.id !== "recheck"))
  // The script captures after the grant, so the banner goes by itself:
  // no "press Check again" hint (WP-117 removed SNAPPER_HINT).
  assert.strictEqual(M.SNAPPER_HINT, undefined)
  assert.strictEqual(b.hint, undefined)
  assert.ok(M.SNAPPER_FIX_SCRIPT.indexOf("then if seldon capture ") !== -1)
})

test("changelogRows: 76 events newest first, one +2 group (3 members), folded resolutions, snapshots", () => {
  const rows = M.changelogRows(sampleIndex, "all")
  assert.strictEqual(rows.length, 76)
  same(rows.map((r) => r.id), sampleIndex.events.map((e) => e.id))
  const badged = rows.filter((r) => r.badge !== "")
  assert.strictEqual(badged.length, 1)
  assert.strictEqual(badged[0].badge, "+2")
  assert.strictEqual(badged[0].subject, "mesa")
  assert.strictEqual(badged[0].txId, "tx-20260927T123000")
  same(rows.filter((r) => r.groupLeader !== "").map((r) => r.subject).sort(), ["lib32-mesa", "vulkan-radeon"])
  assert.strictEqual(rows.filter((r) => r.resolutionDetail !== "").length, 9)
  assert.strictEqual(rows.filter((r) => r.snapshot).length, 8)
  assert.strictEqual(rows.filter((r) => r.drift).length, 8)
  same(rows.filter((r) => r.crisis).map((r) => r.kind), ["config-add", "config-add"])
  const theme = rows.find((r) => r.id === EID)
  assert.strictEqual(theme.proposedCase, "C-2026-005")
  assert.strictEqual(M.rowStatus(theme), "No case · proposed for C-2026-005")
  const tyme = rows.find((r) => r.subject === "io.github.example.tyme" && r.kind === "plugin-add")
  assert.strictEqual(M.rowStatus(tyme), "explained: Zeiterfassung nur zum Testen, noch nicht in der Bar.")
  // WP-113: an in-place edit of a third-party plugin, explained: a quiet row
  const edited = rows.find((r) => r.kind === "plugin-update" && r.detail.startsWith("files changed"))
  assert.strictEqual(edited.subject, "io.github.example.weather-plus")
  assert.strictEqual(M.rowStatus(edited), "explained: Vorhersage-Panel selbst angepasst (größere Schrift).")
  assert.strictEqual(edited.tone, "muted")
  assert.strictEqual(M.rowStatus(rows.find((r) => r.subject === "tailscale")), "linked to C-2026-008")
  // ADR-0029 rule 9: the engine's link reads like any other
  assert.strictEqual(M.rowStatus(rows.find((r) => r.subject === "io.github.example.display-profiles")),
    "linked to C-2026-002: planned by C-2026-002; active at the time")
  assert.strictEqual(M.rowStatus(rows.find((r) => r.subject === "vulkan-radeon")), "In the open mesa group")
  // ADR-0028: the 09-30 -Syu group is routine history, not drift
  assert.strictEqual(M.rowStatus(rows.find((r) => r.subject === "libinput")), "")
  assert.strictEqual(M.rowStatus(rows.find((r) => r.subject === "ollama")), "No case")
  assert.strictEqual(M.rowStatus(rows.find((r) => r.kind === "config-add" && r.crisis)), "Crisis · no case")
  assert.strictEqual(M.rowStatus(rows[0]), "")
  assert.strictEqual(rows[0].dayLabel, "Today")
  assert.strictEqual(rows[0].time, "17:00")
  assert.strictEqual(rows.find((r) => r.subject === "firefox").dayLabel, "Yesterday")
  assert.strictEqual(rows[rows.length - 1].dayLabel, "Tue 1 Sep")
  // ADR-0028: a package installed without a case is attention, the unit a crisis
  assert.strictEqual(rows.find((r) => r.subject === "ollama").tone, "accent")
  assert.strictEqual(rows.find((r) => r.subject === "~/.config/systemd/user/ollama.service").tone, "urgent")
  assert.strictEqual(theme.tone, "accent")
  assert.strictEqual(rows[0].tone, "")
  // One colour source per row: open drift by its item's class (ADR-0028
  // §4b), so the attention group (members red in the ledger) is accent
  // throughout; every other row is an ordinary, quiet row: muted whatever
  // its zone, no stripe without one.
  for (const s of ["mesa", "lib32-mesa", "vulkan-radeon"]) {
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
  assert.strictEqual(nz.find((r) => r.subject === "~/.config/systemd/user/ollama.service").tone, "urgent", "crisis without zone")
  assert.strictEqual(nz.find((r) => r.subject === "mesa").tone, "accent", "attention without zone")
  const swapped = JSON.parse(sample)
  swapped.drift.forEach((d) => { d.zone = d.crisis ? "yellow" : "red" })
  const sw = M.changelogRows(swapped, "all")
  assert.strictEqual(sw.find((r) => r.subject === "~/.config/systemd/user/ollama.service").tone, "urgent", "yellow crisis")
  assert.strictEqual(sw.find((r) => r.id === EID).tone, "accent", "red attention")
  assert.strictEqual(sw.find((r) => r.subject === "vulkan-radeon").tone, "accent", "red attention group member")
  assert.strictEqual(M.rowMeta(rows.find((r) => r.subject === "zed")), "0.198.4-1 · claude-code · C-2026-004")
})

test("changelogRows: the source filter narrows the list", () => {
  const counts = M.sourceCounts(sampleIndex)
  assert.strictEqual(counts.all, 76)
  let total = 0
  for (const s of M.SOURCES) {
    const rows = M.changelogRows(sampleIndex, s)
    assert.strictEqual(rows.length, counts[s], s)
    assert.ok(rows.every((r) => r.source === s), s)
    total += rows.length
  }
  assert.strictEqual(total, 76)
  assert.strictEqual(M.changelogRows(sampleIndex, "pacman").length, 15)
  assert.strictEqual(M.changelogRows(sampleIndex, "snapper").length, 10)
  assert.strictEqual(M.changelogRows(sampleIndex, "").length, 76)
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
  // "without a case" is the attention count: 6 open drift − 2 crises
  same(t.stats.map((s) => s.label), ["events today", "in 7 days", "active", "queued", "without a case"])
  same(t.stats.map((s) => s.value), [32, 53, 2, 3, 4])
  // "1 event today", not "1 events today" (WP-117)
  for (const [n, label] of [[0, "events today"], [1, "event today"], [2, "events today"]]) {
    const one = JSON.parse(sample)
    one.summary.eventsToday = n
    same(M.todayView(one).stats[0], { label: label, value: n })
  }
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
// ADR-0028: the open group of the sample (a named downgrade) and its crisis on a hook path
const MESA = "01M3H6M720FC6BAG7ETNQTXW9K"
const LIB32 = "01M3H6M8184NVTFDTEGPD71P5H"
const HOOK = "01M3Q7R0Z08ZD5R76DQA3PHQ1G"

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

test("driftItemFor: the six sample items, a group member, and events that are not open drift", () => {
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
  const hook = M.driftItemFor(sampleIndex, HOOK)
  assert.strictEqual(hook.crisis, true)
  assert.strictEqual(hook.zone, "yellow", "the ledger zone (ADR-0028 §7)")
  const group = M.driftItemFor(sampleIndex, MESA)
  assert.strictEqual(group.grouped, true)
  assert.strictEqual(group.members, 3)
  assert.strictEqual(group.badge, "+2")
  assert.strictEqual(group.zone, "red")
  same(group.memberList.map((m) => m.subject), ["mesa", "lib32-mesa", "vulkan-radeon"])
  // Opened from a member row: the group's item, named by that member.
  const member = M.driftItemFor(sampleIndex, LIB32)
  assert.strictEqual(member.eventId, LIB32)
  assert.strictEqual(member.leaderId, MESA)
  assert.strictEqual(member.namedSubject, "lib32-mesa")
  assert.strictEqual(member.subject, "mesa")
  // routine (ADR-0028): history, no drift item
  assert.strictEqual(M.driftItemFor(sampleIndex, FIREFOX), null)
  assert.strictEqual(M.driftItemFor(sampleIndex, LIBINPUT), null)
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
  const group = M.driftItemFor(sampleIndex, LIB32)
  assert.strictEqual(M.driftSummary("link", M.driftItemFor(sampleIndex, THEME), { caseId: "C-2026-005" }),
    "Link tokyo-night to C-2026-005")
  assert.strictEqual(M.driftSummary("dismiss", group, {}), "Dismiss mesa and 2 more")
  assert.strictEqual(M.driftSummary("dismiss", group, { only: true }), "Dismiss lib32-mesa only")
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
  assert.strictEqual(M.moreDriftText(capped), "+244 more changes without a case not listed here")
  capped.summary.openDrift = 7
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
  same(rows.map((r) => r.path + "|" + r.updated), ["memory/lessons.md|", "memory/lessons.md|", "memory/lessons.md|",
    "memory/omarchy.md|2026-10-01", "memory/hyprland.md|2026-09-13"])
  same(M.memoryDetail(rows[0]), { heading: "Lesson", rows: [["File", "memory/lessons.md"]] })
  same(M.memoryDetail(rows[3]), { heading: "Topic", rows: [["File", "memory/omarchy.md"], ["Updated", "2026-10-01"]] })
  assert.strictEqual(M.memoryDetail(null), null)
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

// ---- Desk sections 4–6 (WP-123) ---------------------------------------------

test("deskFilter: every word, any field, case-insensitive", () => {
  const rows = M.decisionRows(sampleIndex)
  const ids = (list) => list.map((r) => r.id).join(",")
  assert.strictEqual(ids(M.deskFilter(rows, "", ["id", "title"])), "ADR-0004,ADR-0003,ADR-0002,ADR-0001")
  assert.strictEqual(ids(M.deskFilter(rows, "  zed ", ["id", "title"])), "ADR-0003")
  assert.strictEqual(ids(M.deskFilter(rows, "PROPOSED", ["status"])), "ADR-0004")
  assert.strictEqual(ids(M.deskFilter(rows, "adr-000 snap", ["id", "title"])), "ADR-0002")
  assert.strictEqual(ids(M.deskFilter(rows, "nothing", ["id", "title"])), "")
  same(M.deskFilter(null, "x", ["id"]), [])
})

test("decisionDetail: Accept only while proposed; nothing writes", () => {
  const rows = M.decisionRows(sampleIndex)
  const proposed = M.decisionDetail(rows[0])
  assert.strictEqual(proposed.heading, "ADR-0004 · proposed · 2026-10-01")
  same(proposed.rows, [["Status", "proposed"], ["Date", "2026-10-01"], ["File", "decisions/ADR-0004-ollama-user-service.md"]])
  same(proposed.actions.map((a) => a.id + ":" + a.primary + ":" + a.enabled), ["accept:true:true", "open:false:true"])
  assert.ok(proposed.note.indexOf("set status: accepted in its frontmatter") !== -1)
  const accepted = M.decisionDetail(rows[1])
  same(accepted.actions.map((a) => a.id), ["open"])
  assert.strictEqual(accepted.note, "")
  const odd = M.decisionDetail(M.decisionRows({ decisions: [{ id: "ADR-1", title: "x", status: "superseded" }] })[0])
  same(odd.actions.map((a) => a.id + ":" + a.enabled), ["open:false"])
  assert.ok(odd.note.indexOf("Superseded") === 0)
  assert.ok(odd.lead.indexOf("does not match") !== -1)
  assert.strictEqual(M.decisionDetail(null), null)
})

test("decisionCases: the v2 field, titles from the case lists", () => {
  same(M.decisionCases(sampleIndex, "ADR-0003").map((c) => [c.id, c.title, c.status].join("|")), [
    "C-2026-004|Zed als zweiten Editor installieren|active", "C-2026-005|Theme-Wechsel auf Tokyo Night durchziehen (Zed, Neovim)|queued"])
  same(M.decisionCases(sampleIndex, "ADR-0004"), [])
  const v1 = JSON.parse(sample)
  for (const d of v1.decisions) delete d.cases
  assert.strictEqual(M.decisionCases(v1, "ADR-0003"), null, "an index without the field hides the block")
  const idx = JSON.parse(sample)
  idx.decisions[0].cases = ["C-2026-003", "C-2026-999", "", 7, "bad"]
  idx.decisions[1].cases = []
  same(M.decisionCases(idx, "ADR-0004").map((c) => [c.id, c.title, c.status, c.actionable].join("|")), [
    "C-2026-003|Omarchy auf 4.0.7 aktualisieren|active|true", "C-2026-999|||true", "bad|||false"])
  same(M.decisionCases(idx, "ADR-0003"), [])
  assert.strictEqual(M.decisionCases(idx, "ADR-0099"), null)
  assert.strictEqual(M.decisionCases(null, "ADR-0004"), null)
})

test("systemTiles: five tiles, big values, every field optional", () => {
  const now = Date.parse("2026-10-01T17:05:12+02:00")
  const t = M.systemTiles(sampleIndex, now)
  same(t.map((x) => x.id), ["omarchy", "packages", "snapshots", "deviations", "collectors"])
  same(t.map((x) => x.meta), ["4.0.7-1", "2009 installed", "115 newest", "5 files", "6/6 ok"])
  same(t.map((x) => x.lead), ["theme tokyo-night · updated 7 h ago", "327 explicit · 41 from the AUR",
    "6 snapshots in the index (the newest 10)", "Config files that differ from Omarchy's defaults; the list is in STATUS.md",
    "last capture just now"])
  same(t[0].rows, [["Version", "4.0.7-1"], ["Theme", "tokyo-night"], ["Last update", "2026-10-01 09:21 · 7 h ago"],
    ["Plugins", "33 of 40 enabled"]])
  same(t[1].rows, [["Explicit", "327"], ["Installed", "2009"], ["AUR", "41"]])
  same(t[2].rows[0], ["#115", "2026-10-01 16:30 · tailscale: MagicDNS · post"])
  same(t[3].rows, [])
  same(t[4].rows.slice(0, 1).concat(t[4].rows.slice(6)), [["pacman", "ok"], ["Machine", "workstation-7f3a"],
    ["Engine", "0.1.0"], ["Index written", "2026-10-01 17:05"], ["Area dev-env", "4 cases"], ["Area hyprland", "1 case · AGENTS.md"],
    ["Area packages", "1 case"], ["Area plugins", "0 cases"], ["Area shell", "1 case"], ["Area themes", "1 case · AGENTS.md"]])
  assert.ok(t.every((x) => x.stripe === "" && x.empty === false))
  const fail = M.systemTiles(degraded, now)[4]
  assert.strictEqual(fail.meta, "5/6 ok")
  assert.strictEqual(fail.stripe, "attention")
  assert.ok(fail.lead.indexOf("1 collector failing") === 0)
  const bare = JSON.parse(sample)
  bare.system = {}
  delete bare.state.collectors
  const b = M.systemTiles(bare, now)
  same(b.map((x) => x.meta), ["—", "—", "—", "—", "—"])
  same(b.map((x) => x.lead), ["Not in the index", "Not in the index", "Not in the index", "Not in the index", "last capture just now"])
  same(b[4].rows, [["Machine", "workstation-7f3a"], ["Engine", "0.1.0"], ["Index written", "2026-10-01 17:05"]])
  bare.system = { packages: { explicit: 3 }, deviations: 1 }
  same(M.systemTiles(bare, now).map((x) => x.meta).slice(1, 4), ["3 explicit", "—", "1 file"])
  same(M.systemTiles(null, now).map((x) => x.meta), ["—", "—", "—", "—", "—"])
})

// ---- Prime Radiant (WP-030) --------------------------------------------------

test("periods: ids, ←/→ wrap", () => {
  same(M.PERIODS.map((p) => p.id), ["30", "90", "365", "all"])
  assert.strictEqual(M.PERIOD_DEFAULT, "90")
  assert.strictEqual(M.cyclePeriod("30", -1), "all")
  assert.strictEqual(M.cyclePeriod("all", 1), "30")
  assert.strictEqual(M.cyclePeriod("90", 1), "365")
  assert.strictEqual(M.cyclePeriod("bogus", 1), "365")
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
  same(s30.map((s) => s.detail), ["71 events", "Explicit 324 → 327", "13 opened · 8 resolved",
    "R0 1 · R1 3 · R2 3 · R3 1 · all time", "7 cases · 2 releases · 6 snapshots · 2 crises", "6 of 9 steps done"])
  same(s30.map((s) => s.windowed), [true, true, true, false, true, false])
  assert.strictEqual(table.periods["90"].slots[0].detail, "76 events")
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
  same(h30.numbers, { days: 30, events: 71, activeDays: 15, max: 32, busiest: "2026-10-01" })
  assert.strictEqual(h30.summary, "71 events on 15 of 30 days · busiest 2026-10-01 (32)")
  // 2026-09-02 is a Wednesday: the first column starts at row 2.
  same([h30.offset, h30.weeks, h30.cells.length], [2, 5, 30])
  same([h30.cells[0].date, h30.cells[0].col, h30.cells[0].row], ["2026-09-02", 0, 2])
  const last = h30.cells[29]
  same([last.date, last.col, last.row, last.total, last.step], ["2026-10-01", 4, 3, 32, 5])
  same(h30.months.map((m) => m.col + m.label), ["0Sep", "4Oct"])
  assert.strictEqual(M.heatmapCellText(last),
    "Thu 2026-10-01 · 32 events · seldon 8 · pacman 7 · agent 6 · snapper 4 · config 2 · manual 2 · omarchy 1 · plugins 1 · theme 1")
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
  // ADR-0028 §5: routine rows open nothing, their old resolutions count nothing
  same(d.numbers, { weeks: 5, opened: 13, resolved: 8, max: 6, peak: "2026-W40" })
  assert.strictEqual(d.summary, "13 opened · 8 resolved in 5 weeks · peak 2026-W40")
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
  assert.strictEqual(b.detail, "This plugin needs engine 0.2.0 or newer and seldon reports 0.1.9.")
  assert.strictEqual(b.script, M.UPDATE_ENGINE_SCRIPT)
  same(b.actions, [{ id: "terminal", label: "Update" }, { id: "copy", label: "Copy" }, { id: "recheck", label: "Check again" }])
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
    M.bannerFor("engineMissing"), M.bannerFor("engineMissing", { indexExists: true }), M.bannerFor("notInitialised"),
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
  for (const ok of [["plan", "reopen", "C-2026-002", "--json"], ["doctor", "--only", "rules", "--json"], ["rules", "update", "--json"]])
    assert.strictEqual(M.validateArgs(ok), "", ok.join(" "))
  for (const bad of [
    ["agent", "start", "--new", "--", "x"],            // --json missing
    ["agent", "start", "--new", "--json"],            // no text
    ["agent", "start", "--new", "--zone", "red", "--json", "--", "x"],
    ["agent", "start", "C-2026-001", "--json", "--", "x"],
    ["plan", "reopen", "C-2026-002"],
    ["plan", "reopen", "C-2026-002", "--json", "--", "x"],
    ["doctor"], ["doctor", "--json"], ["doctor", "--fix", "--json"], ["doctor", "--only", "rules"],
    ["doctor", "--only", "probes", "--json"],
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
  same(M.rulesUpdateResult(0, JSON.stringify({ action: "rewritten" }), ""), { ok: true, text: "Agent rules updated" })
  same(M.rulesUpdateResult(0, JSON.stringify({ action: "unchanged" }), ""), { ok: true, text: "The agent rules were already current" })
  same(M.rulesUpdateResult(4, JSON.stringify({ error: { code: 4, message: "locked" } }), ""), { ok: false, text: "Updating the agent rules failed: locked" })
})

test("WP-111: one line for the Update rules click", () => {
  const r = (v) => M.rulesUpdateResult(0, JSON.stringify(v), "")
  same(r({ action: "rewritten", from: "v2", version: 3, archived: null }), { ok: true, text: "Agent rules updated to v3" })
  same(r({ action: "rewritten", from: "v2", version: 3, archived: "archive/AGENTS-2026-10-06.md" }),
    { ok: true, text: "Agent rules updated to v3; your old copy is in archive/AGENTS-2026-10-06.md" })
  same(r({ action: "kept", version: 3, archived: "archive/AGENTS-2026-10-06-2.md" }).text,
    "Agent rules updated to v3; your old copy is in archive/AGENTS-2026-10-06-2.md")
  same(r({ action: "unchanged", version: 3 }), { ok: true, text: "The agent rules were already current (v3)" })
  // only the first line of an archive path; a version that is no number is left out
  same(r({ action: "created", version: "3; rm", archived: "a\nb" }).text, "Agent rules updated; your old copy is in a")
  const notice = M.rulesNotice({ ok: true, pending: false, text: "Agent rules updated to v3" })
  same([notice.status, notice.tone, notice.title, notice.actions], ["rulesUpdated", "neutral", "Agent rules updated to v3", []])
  for (const quiet of [null, undefined, "x", { ok: true, pending: true, text: "Updating the rules…" },
    { ok: false, pending: false, text: "Updating the agent rules failed: x" }, { ok: true, pending: false, text: "" }])
    assert.strictEqual(M.rulesNotice(quiet), null)
})

// ---- The desk (ADR-0034, WP-121)

test("DESK_SECTIONS: nine targets, digits 1–8 and `,`, wrap with deskCycle", () => {
  same(M.DESK_SECTIONS.map(s => s.id), ["today", "changelog", "work", "decisions", "system", "memory", "radiant", "graph", "settings"])
  same(M.DESK_SECTIONS.map(s => s.key).join(""), "12345678,")
  same(M.DESK_SECTIONS.filter(s => s.solo).map(s => s.id), ["radiant", "graph"])
  for (let i = 1; i <= 8; i++) assert.strictEqual(M.deskSectionForKey(String(i)), M.DESK_SECTIONS[i - 1].id)
  assert.strictEqual(M.deskSectionForKey(","), "settings")
  for (const t of ["0", "9", "a", "", ".", "/"]) assert.strictEqual(M.deskSectionForKey(t), "")
  assert.strictEqual(M.deskCycle("today", -1), "settings")
  assert.strictEqual(M.deskCycle("settings", 1), "today")
  assert.strictEqual(M.deskCycle("graph", 1), "settings")
  assert.strictEqual(M.deskCycle("bogus", 1), "changelog")
  let id = "today"
  for (let i = 0; i < 9; i++) id = M.deskCycle(id, 1)
  assert.strictEqual(id, "today")
  for (const s of M.DESK_SECTIONS) assert.ok(/^[MmLlHhVvCcSsQqTtAaZz0-9 .,-]+$/.test(s.icon), s.id)
})

test("clampDeskWidth and deskSidebarMode take bad shell.json values to the defaults", () => {
  for (const [v, want] of [[undefined, 100], [null, 100], ["", 100], ["x", 100], [NaN, 100], [49, 50], [10, 50], [50, 50],
    [67, 67], [74.6, 75], ["80", 80], [100, 100], [150, 100]])
    assert.strictEqual(M.clampDeskWidth(v), want, String(v))
  for (const [v, want] of [["open", "open"], ["collapsed", "collapsed"], ["Collapsed", "open"], [undefined, "open"], [1, "open"]])
    assert.strictEqual(M.deskSidebarMode(v), want, String(v))
})

test("deskGeometry: the clamp of ADR-0034 §1, centred, inside the gaps", () => {
  const g = (W, pct) => M.deskGeometry(W, 1000, pct, 5)
  // avail = W - 10
  same(g(1920, 100), { x: 5, y: 5, w: 1910, h: 990, avail: 1910 })
  same(g(1920, 50), { x: 5 + Math.floor((1910 - 960) / 2), y: 5, w: 960, h: 990, avail: 1910 })
  assert.strictEqual(g(1920, 67).w, 1280)
  assert.strictEqual(g(1920, 75).w, 1433)
  assert.strictEqual(g(2560, 50).w, 1275)
  assert.strictEqual(g(3840, 67).w, 2566)
  // narrower than 960 + gaps: the whole width at any setting
  assert.strictEqual(g(1366, 50).w, 960)
  assert.strictEqual(g(900, 50).w, 890)
  assert.strictEqual(g(900, 100).w, 890)
  for (const W of [800, 1366, 1920, 2560, 3840]) for (const p of [50, 67, 75, 100]) {
    const r = g(W, p)
    assert.ok(r.x >= 5 && r.x + r.w <= W - 5, W + " " + p)
    assert.ok(Math.abs((r.x - 5) - (W - 5 - r.x - r.w)) <= 1, "centred " + W + " " + p)
  }
  same(M.deskGeometry(0, 0, 100, 5), { x: 5, y: 5, w: 0, h: 0, avail: 0 })
})

test("deskLayout: icons under 960 or collapsed, stacked under 760, solo has no list", () => {
  const sizes = { sidebar: 210, icons: 56, listMin: 260, listMax: 360 }
  const l = (w, pref, solo) => M.deskLayout(w, pref, solo, sizes)
  same(l(1910, "open", false), { sidebar: "open", forced: false, stacked: false, solo: false, sidebarW: 210, listW: 360, detailW: 1340 })
  same(l(960, "open", false), { sidebar: "open", forced: false, stacked: false, solo: false, sidebarW: 210, listW: 260, detailW: 490 })
  assert.strictEqual(l(959, "open", false).sidebar, "icons")
  assert.strictEqual(l(959, "open", false).forced, true)
  assert.strictEqual(l(959, "open", false).stacked, false)
  assert.strictEqual(l(1910, "collapsed", false).sidebar, "icons")
  assert.strictEqual(l(1910, "collapsed", false).forced, false)
  assert.strictEqual(l(760, "open", false).stacked, false)
  same(l(759, "open", false), { sidebar: "icons", forced: true, stacked: true, solo: false, sidebarW: 56, listW: 703, detailW: 703 })
  same(l(1910, "open", true), { sidebar: "open", forced: false, stacked: false, solo: true, sidebarW: 210, listW: 0, detailW: 1700 })
  assert.strictEqual(l(600, "open", true).stacked, false)
})

test("deskPayload: section, select, filter; a period alone means the Prime Radiant", () => {
  same(M.deskPayload(""), { section: "", select: "", filter: "", period: "" })
  same(M.deskPayload("{}"), { section: "", select: "", filter: "", period: "" })
  same(M.deskPayload("not json"), { section: "", select: "", filter: "", period: "" })
  same(M.deskPayload('{"section":"work"}').section, "work")
  same(M.deskPayload('{"section":"nope"}').section, "")
  same(M.deskPayload('{"period":"30"}'), { section: "radiant", select: "", filter: "", period: "30" })
  same(M.deskPayload('{"section":"radiant","period":"30"}'), { section: "radiant", select: "", filter: "", period: "30" })
  same(M.deskPayload('{"section":"today","period":"30"}').section, "today")
  same(M.deskPayload('{"period":"7"}').section, "")
  same(M.deskPayload('{"section":"changelog","select":"01J","filter":"pacman"}'), { section: "changelog", select: "01J", filter: "pacman", period: "" })
  same(M.deskPayload('{"select":5}').select, "")
})

test("deskKpis and deskCounts on the sample; nothing without an index", () => {
  const idx = M.parseIndex(sample).index
  same(M.deskKpis(idx).map(k => k.id + " " + k.value + " " + k.tone),
    ["active 2 accent", "verification 1 ", "queued 3 ", "crises 2 urgent", "attention 4 "])
  same(M.deskKpis(null), [])
  const c = M.deskCounts(idx)
  same(Object.keys(c), M.DESK_SECTIONS.map(s => s.id))
  same(c.today, { text: "32", tone: "" })
  same(c.changelog, { text: "6", tone: "urgent" })
  same(c.work, { text: "2 · 1 · 3", tone: "" })
  same(c.decisions, { text: "1 new", tone: "" })
  same(c.memory, { text: "5", tone: "" })
  same(c.system, { text: "", tone: "" })
  same(c.radiant, { text: "", tone: "" })
  for (const v of Object.values(M.deskCounts(null))) same(v, { text: "", tone: "" })
})

test("deskSubline: machine · Omarchy · captured", () => {
  const idx = M.parseIndex(sample).index
  assert.strictEqual(M.deskSubline(idx, "2026-10-01T17:05:00+02:00", gen),
    "workstation-7f3a · Omarchy 4.0.7-1 · captured just now")
  assert.strictEqual(M.deskSubline(idx, "", gen), "workstation-7f3a · Omarchy 4.0.7-1")
  assert.strictEqual(M.deskSubline(null, "", gen), "")
})

test("deskSettingsWrite carries every key of the entry and the change, null when stored", () => {
  const entry = { id: "jax.seldon", captureIntervalMin: 30, driftInBar: "all", future: [1] }
  same(M.deskSettingsWrite(entry, "deskWidth", 67), { captureIntervalMin: 30, driftInBar: "all", future: [1], deskWidth: 67 })
  same(M.deskSettingsWrite({ deskWidth: 67 }, "deskWidth", 67), null)
  same(M.deskSettingsWrite({ deskWidth: 67, deskSidebar: "open" }, "deskSidebar", "collapsed"), { deskWidth: 67, deskSidebar: "collapsed" })
  same(M.deskSettingsWrite(null, "deskWidth", 50), { deskWidth: 50 })
  // the input is not changed
  same(entry, { id: "jax.seldon", captureIntervalMin: 30, driftInBar: "all", future: [1] })
})

test("pickScreen: the focused monitor, else the first", () => {
  assert.strictEqual(M.pickScreen(["DP-1", "DP-2"], "DP-2"), 1)
  assert.strictEqual(M.pickScreen(["DP-1", "DP-2"], "HDMI-A-1"), 0)
  assert.strictEqual(M.pickScreen(["DP-1"], ""), 0)
  assert.strictEqual(M.pickScreen([], "DP-1"), 0)
  assert.strictEqual(M.pickScreen(null, null), 0)
})

test("deskWidthPreview and preset labels", () => {
  assert.strictEqual(M.deskWidthPreview(1920, 67, 5), "1280 px on this screen")
  assert.strictEqual(M.deskWidthPreview(1366, 50, 5), "960 px on this screen")
  same(M.DESK_WIDTH_PRESETS.map(M.deskPresetLabel), ["50 %", "67 %", "75 %", "Full"])
})

// ---- Desk sections Today, Changelog, Work (WP-122)


test("deskChangelog: every event once, by class, with title, meta, age and stripe", () => {
  const idx = M.parseIndex(sample).index
  const p = M.deskChangelog(idx)
  assert.strictEqual(p.rows.length, 76)
  const byCls = {}
  for (const r of p.rows) byCls[r.cls] = (byCls[r.cls] || 0) + 1
  same(Object.keys(byCls).sort().map(k => k + " " + byCls[k]), ["attention 6", "case 37", "crisis 2", "routine 31"])
  const unit = M.changelogRow(p, UNIT)
  same([unit.title, unit.listMeta, unit.age, unit.stripe, unit.cls], ["ollama.service", "config · config-add", "14:03", "crisis", "crisis"])
  const mesa = M.changelogRow(p, MESA)
  same([mesa.title, mesa.age, mesa.stripe, mesa.hideKey], ["mesa +2", "27 Sep 12:30", "attention", MESA])
  // a group member hides with its leader
  assert.strictEqual(M.changelogRow(p, LIB32).hideKey, MESA)
  assert.strictEqual(M.changelogRow(p, "nope"), null)
  same(M.deskChangelog(null).rows, [])
})

test("rowAge: the time today, else day and month (the year when it differs)", () => {
  assert.strictEqual(M.rowAge("2026-10-01", "17:00", "2026-10-01"), "17:00")
  assert.strictEqual(M.rowAge("2026-09-30", "08:15", "2026-10-01"), "30 Sep 08:15")
  assert.strictEqual(M.rowAge("2025-12-31", "23:59", "2026-01-01"), "31 Dec 2025 23:59")
  assert.strictEqual(M.rowAge("", "", "2026-10-01"), "")
})

test("changelogView and changelogChips: chips, search, Hide (attention only), a group once", () => {
  const p = M.deskChangelog(M.parseIndex(sample).index)
  same(M.changelogChips(p, {}).map(c => c.id + " " + c.count),
    ["open 6", "crisis 2", "attention 4", "routine 31", "case 37", "all 76"])
  // the drift chips list a group as its leader; "all" lists every event
  same(M.changelogView(p, "open", {}, "").map(r => r.title).slice(-1), ["mesa +2"])
  assert.strictEqual(M.changelogView(p, "open", {}, "").length, 6)
  assert.strictEqual(M.changelogView(p, "bogus", {}, "").length, 6)
  assert.strictEqual(M.changelogView(p, "all", {}, "").length, 76)
  assert.ok(M.changelogView(p, "all", {}, "").some(r => r.id === LIB32))
  // the search matches subject, meta, detail and actor, case-insensitive
  same(M.changelogView(p, "open", {}, "OLLAMA").map(r => r.title), ["ollama.service", "ollama"])
  same(M.changelogView(p, "crisis", {}, "codex").map(r => r.id), [UNIT])
  // Hide keeps attention out of open and attention, never a crisis
  const hidden = { [MESA]: true, [UNIT]: true }
  assert.strictEqual(M.changelogView(p, "open", hidden, "").length, 5)
  assert.strictEqual(M.changelogView(p, "crisis", hidden, "").length, 2)
  assert.strictEqual(M.changelogView(p, "all", hidden, "").length, 76)
  assert.strictEqual(M.hiddenCount(p, hidden), 1)
  same(M.changelogChips(p, hidden).slice(0, 3).map(c => c.count), [5, 2, 3])
})

test("one count everywhere: chips, sidebar, header, the quiet line, hidden (B2)", () => {
  const idx = M.parseIndex(sample).index
  const p = M.deskChangelog(idx)
  const chips = {}
  for (const c of M.changelogChips(p, {})) chips[c.id] = c.count
  const kpis = {}
  for (const k of M.deskKpis(idx)) kpis[k.id] = k.value
  assert.strictEqual(chips.open, Number(M.deskCounts(idx).changelog.text))
  assert.strictEqual(chips.open, M.counts(idx).drift)
  assert.strictEqual(chips.crisis, kpis.crises)
  assert.strictEqual(chips.attention, kpis.attention)
  assert.strictEqual(M.attentionText(idx), chips.attention + " changes without a case")
  same([chips.open, chips.crisis, chips.attention], [6, 2, 4])
  // Hide the mesa group (from a member): one change hidden, the chips one
  // less; the sidebar and the header count the index, which Hide leaves
  const hidden = { [M.changelogRow(p, LIB32).hideKey]: true }
  assert.strictEqual(M.hiddenCount(p, hidden), 1)
  same(M.changelogChips(p, hidden).slice(0, 3).map(c => c.count), [5, 2, 3])
  assert.strictEqual(M.deskCounts(idx).changelog.text, "6")
})

test("cycleChip wraps both ways", () => {
  assert.strictEqual(M.cycleChip("open", 1), "crisis")
  assert.strictEqual(M.cycleChip("all", 1), "open")
  assert.strictEqual(M.cycleChip("open", -1), "all")
  assert.strictEqual(M.cycleChip("bogus", 1), "crisis")
})

test("eventDetail: heading, class, the key/values; why loud from the engine's rule only (B1)", () => {
  const idx = M.parseIndex(sample).index
  const p = M.deskChangelog(idx)
  const theme = M.eventDetail(idx, p, THEME)
  same([theme.heading, theme.title, theme.cls, theme.open, theme.proposedCase, theme.whyLoud],
    ["theme · theme-set", "tokyo-night", "attention", true, "C-2026-005", ""])
  same(theme.kv.map(r => r[0]), ["When", "Who", "What", "Case", "Rule", "Source", "Zone", "Event"])
  same(theme.kv[3], ["Case", "proposed: C-2026-005"])
  same(theme.kv[4], ["Rule", "attention · planned by C-2026-005, not linked; quiet until you say something"])
  // no rule yet: the class and the source, never a cause
  const unit = M.eventDetail(idx, p, UNIT)
  assert.strictEqual(unit.whyLoud, "The engine classed this config change as a crisis; `seldon drift show " + UNIT
    + "` names the rule. No open case plans it, and no case is linked.")
  same(unit.kv[4], ["Rule", "crisis · no case"])
  same(unit.kv[0], ["When", "2026-10-01 14:03"])
  const pending = M.eventDetail(idx, p, UNIT, { state: "pending" })
  assert.ok(pending.whyLoud.startsWith("The engine classed this config change as a crisis; asking it for the rule."))
  same(pending.kv[4], ["Rule", "crisis · rule: asking the engine · no case"])
  // the engine's rule, per rule
  const known = rule => M.eventDetail(idx, p, UNIT, { state: "known", rule: rule, cls: "crisis" })
  assert.strictEqual(known("always-red-paths").whyLoud,
    "The path matches your crisis list ([drift] alwaysRedPaths in ~/.config/seldon/config.toml). No open case plans it, and no case is linked.")
  assert.ok(known("always-red").whyLoud.startsWith("A package on your crisis list ([drift] alwaysRed in ~/.config/seldon/config.toml)"))
  assert.ok(known("attention-all").whyLoud.startsWith("[drift] attention = \"all\" is set: every change without a case is open drift, and a crisis is a change in the red zone."))
  assert.ok(known("future-rule").whyLoud.startsWith("The engine's rule: future-rule."))
  same(known("always-red-paths").kv[4], ["Rule", "crisis · rule always-red-paths · no case"])
  // a crisis an open case plans: the callout and the Case and Rule rows agree
  const planned = JSON.parse(sample)
  planned.drift.find(d => d.eventId === UNIT).proposedCase = "C-2026-003"
  const pp = M.deskChangelog(planned)
  const d = M.eventDetail(planned, pp, UNIT, { state: "known", rule: "always-red-paths", cls: "crisis" })
  assert.ok(d.whyLoud.endsWith("C-2026-003 plans it (its plan names this change); nothing has linked it yet."))
  assert.strictEqual(d.whyLoud.indexOf("No open case"), -1)
  same([d.kv[3], d.kv[4]], [["Case", "proposed: C-2026-003"], ["Rule", "crisis · rule always-red-paths · planned by C-2026-003, not linked"]])
  same(M.eventActions(d, {}).map(a => a.label)[0], "Link to C-2026-003…")
  // a member shows the group's proposal and rule
  assert.ok(M.eventDetail(idx, p, LIB32).kv[4][1].indexOf("one pacman transaction (ADR-0013)") !== -1)
  assert.strictEqual(M.eventDetail(idx, p, "nope"), null)
  const folded = p.rows.find(r => r.resolution !== "")
  assert.ok(M.eventDetail(idx, p, folded.id).kv.some(r => r[0] === "Resolved"))
  assert.strictEqual(M.whyLoud({ cls: "attention", source: "config" }, "", { state: "known", rule: "always-red-paths" }), "")
  // contract 2: a detail the index clipped (event meta.truncated, a drift item's truncated) says so
  const clippedEvent = p.rows.find(r => (M.findEvent(idx, r.id).meta || {}).truncated === true)
  assert.ok(M.eventDetail(idx, p, clippedEvent.id).kv[2][1].endsWith(" (clipped in the index; the ledger has it in full)"))
  assert.ok(!M.eventDetail(idx, p, THEME).kv[2][1].includes("clipped"))
  const cut = JSON.parse(sample)
  cut.drift.find(d => d.eventId === UNIT).truncated = true
  assert.ok(M.eventDetail(cut, M.deskChangelog(cut), UNIT).kv[2][1].endsWith("(clipped in the index; the ledger has it in full)"))
})

test("eventDetail: a plugin update names its commits as plain text after What (WP-136)", () => {
  const WEATHER_UPDATE = "01M3A5RK9GGCM0KRRCQ47NGCN0"
  const idx = M.parseIndex(sample).index
  const d = M.eventDetail(idx, M.deskChangelog(idx), WEATHER_UPDATE)
  same(d.kv.map(r => r[0]).slice(0, 4), ["When", "Who", "What", "Commits"])
  same(d.kv[2][1], "1.2.0 → 1.3.0, pulled 3 commits: Release 1.3.0 …")
  same(d.kv[3][1], "Release 1.3.0\nAdd a wind gust row\nFix the unit toggle in the panel")
  const variant = edit => {
    const v = JSON.parse(sample)
    edit(v.events.find(e => e.id === WEATHER_UPDATE).meta)
    return M.eventDetail(v, M.deskChangelog(v), WEATHER_UPDATE).kv.map(r => r[0])
  }
  same(variant(m => { m.git = "rollback" })[3], "Rolled back")
  // no list, or one that is not text: no row
  assert.ok(!variant(m => { delete m.commits }).includes("Commits"))
  assert.ok(!variant(m => { m.commits = 3 }).includes("Commits"))
  // an event of another kind is unchanged
  assert.ok(!M.eventDetail(idx, M.deskChangelog(idx), THEME).kv.some(r => r[0] === "Commits"))
})

test("driftRuleInfo and driftShowResult: the rule from `drift show`", () => {
  same(M.driftRuleInfo({ [UNIT]: { rule: "always-red-paths", cls: "crisis" } }, null, UNIT),
    { state: "known", rule: "always-red-paths", cls: "crisis" })
  same(M.driftRuleInfo({}, { eventId: UNIT, pending: true }, UNIT).state, "pending")
  same(M.driftRuleInfo({}, { eventId: MESA, pending: true }, UNIT).state, "unknown")
  same(M.driftRuleInfo(null, null, UNIT).state, "unknown")
  const r = M.driftShowResult(0, JSON.stringify({ open: true, class: "crisis", rule: "always-red", members: [] }), "")
  same([r.ok, r.rule, r.cls], [true, "always-red", "crisis"])
  same([M.driftShowResult(0, "{}", "").rule, M.driftShowResult(1, "", "boom").ok], ["", false])
  // kept while the item is an open crisis
  const idx = M.parseIndex(sample).index
  const rules = { [UNIT]: { rule: "always-red-paths", cls: "crisis" }, [THEME]: { rule: "other", cls: "attention" } }
  same(Object.keys(M.keptDriftRules(rules, idx)), [UNIT])
  const kept = { [UNIT]: rules[UNIT] }
  assert.strictEqual(M.keptDriftRules(kept, idx), kept)
  same(M.keptDriftRules(kept, null), {})
})

test("eventActions: open drift, Hide only for attention, Open case, routine none; Ask agent first when there", () => {
  const idx = M.parseIndex(sample).index
  const p = M.deskChangelog(idx)
  const labels = (id, opts) => M.eventActions(M.eventDetail(idx, p, id), opts).map(a => a.label + (a.primary ? "*" : ""))
  same(labels(THEME, {}), ["Link to C-2026-005…*", "Explain…", "Dismiss…", "Hide"])
  same(labels(THEME, { hidden: true }).slice(-1), ["Show"])
  same(labels(UNIT, {}), ["Link to case…*", "Explain…", "Dismiss…"])
  same(labels(UNIT, { askAgent: true }), ["Ask agent*", "Link to case…", "Explain…", "Dismiss…"])
  const inCase = p.rows.find(r => r.cls === "case")
  same(labels(inCase.id, {}), ["Open case*"])
  const routine = p.rows.find(r => r.cls === "routine")
  same(labels(routine.id, {}), [])
  same(M.eventActions(null, {}), [])
})

test("deskToday and todayRows: needs you, journal, yesterday, the overview", () => {
  const idx = M.parseIndex(sample).index
  const t = M.deskToday(idx, M.deskChangelog(idx))
  same([t.title, t.state.id, t.headline], ["Thursday, 1 Oct 2026", "crisis", "Seldon is recording. 2 changes need you."])
  same(t.tiles.map(x => x.label + " " + x.value), ["events today 32", "7 days 53"])
  same(t.needs.map(r => r.id), [UNIT, HOOK])
  same(t.cases.map(c => c.id + " " + c.text), ["C-2026-003 4/5 steps · claude-code", "C-2026-004 2/4 steps · claude-code"])
  same(M.todayRows(t, false).map(r => r.type), ["crisis", "crisis", "entry", "entry", "entry", "entry", "toggle"])
  same(M.todayRows(t, true).map(r => r.id).slice(-2), ["toggle", "yesterday:0"])
  assert.strictEqual(M.todayRows(t, false)[6].title, "▸ Yesterday · 1 entry")
  const none = M.deskToday(null, null)
  same([none.headline, none.tiles, none.needs, none.state], ["No index to show", [], [], null])
  same(M.todayRows(none, false).map(r => r.id), ["empty"])
  // the sidebar search: crises and entries, yesterday's too
  same(M.todayRows(t, false, "snapshot").map(r => r.id), ["entry:0", "entry:2", "yesterday:0"])
  same(M.todayRows(t, false, "OLLAMA").map(r => r.id), [UNIT, "entry:2"])
  same(M.todayRows(t, false, "no such words").length, 0)
})

test("deskWork and workView: groups in order, labels, the By agent filter, search", () => {
  const idx = M.parseIndex(sample).index
  const p = M.deskWork(idx)
  const v = M.workView(p, "", "")
  same(v.rows.map(r => r.id), ["C-2026-003", "C-2026-004", "C-2026-008", "C-2026-005", "C-2026-006", "C-2026-007", "C-2026-002", "C-2026-001"])
  same(v.labels, { active: "ACTIVE · 2", verification: "VERIFICATION · 1", queued: "QUEUED · 3", completed: "COMPLETED · 2" })
  same(v.rows.find(r => r.id === "C-2026-005").listMeta, "C-2026-005 · R1 · themes · 1 proposed")
  same(v.rows.find(r => r.id === "C-2026-002").listMeta, "C-2026-002 · R1 · hyprland · closed by agent")
  const agent = M.workView(p, "agent", "")
  assert.strictEqual(agent.labels.completed, "COMPLETED · 1 / 2")
  same(M.workView(p, "", "ZWEITEN").rows.map(r => r.id), ["C-2026-004"])
  same(M.workView(M.deskWork(null), "", "").rows, [])
})

test("caseDeskActions by status; Enter never launches; hints and verbs", () => {
  const ids = st => M.caseDeskActions({ status: st, actionable: true }).map(a => a.id + (a.arm ? "!" : "") + (a.primary ? "*" : "") + (a.enter ? "^" : ""))
  same(ids("queued"), ["start!*^", "drop!", "open"])
  same(ids("active"), ["agent!*", "verify!^", "drop!", "open"])
  same(ids("verification"), ["done!*^", "drop!", "open"])
  same(ids("completed"), ["reopen*^", "open"])
  same(ids("dropped"), ["open*^"])
  same(M.caseDeskActions({ status: "active", actionable: false }), [])
  // Enter on any status never takes an action that launches something
  for (const st of ["queued", "active", "verification", "completed", "dropped"]) {
    const enter = M.caseEnterAction({ status: st, actionable: true })
    assert.ok(enter && !enter.launches, st)
  }
  assert.strictEqual(M.caseEnterAction({ status: "active", actionable: true }).id, "verify")
  const active = { status: "active", actionable: true }
  assert.strictEqual(M.caseArmHint(M.caseDeskAction(active, "agent"), "C-2026-003"), "Hand to agent C-2026-003? Press a again or click Confirm.")
  assert.strictEqual(M.caseArmHint(M.caseDeskAction(active, "verify"), "C-2026-003"), "To verification C-2026-003? Press Enter again or click Confirm.")
  assert.strictEqual(M.caseArmHint(M.caseDeskAction(active, "drop"), "C-2026-003"), "Drop C-2026-003? Press x again or click Confirm. This is final.")
  same(["start", "verify", "done", "drop", "reopen", "agent", "open"].map(M.caseActionVerb), ["start", "verify", "done", "drop", "reopen", "", ""])
  for (const verb of ["start", "verify", "done", "drop", "reopen"])
    assert.strictEqual(M.validateArgs(M.planArgs(verb, "C-2026-003").args), "", verb)
})

test("caseDetail: key/values, plan, log and linked changes from the index", () => {
  const idx = M.parseIndex(sample).index
  const p = M.deskWork(idx)
  const d = M.caseDetail(idx, p, "C-2026-004")
  same([d.heading, d.meta, d.plan.text], ["C-2026-004 · active", "C-2026-004 · R2", "2 of 4 steps done"])
  same(d.kv.map(r => r[0]), ["Status", "Risk", "Zone", "Area", "Priority", "Agent", "Rollback", "Dates", "File"])
  same(d.log.map(r => r[1]), [
    "note · human · Zed fühlt sich gut an. Theme-Sync fehlt noch, siehe Inbox.",
    "case-started · human · R2", "case-created · human"].slice(0, d.log.length))
  assert.ok(d.linked.length >= 1 && d.linkedMore === "")
  // ids the index no longer lists are counted
  const copy = JSON.parse(sample)
  copy.cases.active[1].events.push("01M3ZZZZZZZZZZZZZZZZZZZZZZ")
  const d2 = M.caseDetail(copy, M.deskWork(copy), "C-2026-004")
  assert.strictEqual(d2.linkedMore, "+1 older change the index no longer lists")
  assert.strictEqual(M.caseDetail(idx, p, "C-2026-999"), null)
  // contract 2: the case-updated event carries the risk into the log
  same(M.caseDetail(idx, p, "C-2026-003").log.map(r => r[1]).filter(t => t.startsWith("case-updated")).length, 1)
})

test("free text goes exactly as typed, surrounding blanks included (N3)", () => {
  const text = "  two  spaces around  "
  same(M.logArgs(text, "").args.slice(-1), [text])
  same(M.logArgs(text, "C-2026-004").args.slice(-1), [text])
  same(M.agentNewArgs(text).args.slice(-1), [text])
  same(M.planArgs("new", { title: text }).args.slice(-1), [text])
  same(M.driftArgs("explain", { eventId: UNIT, text: text }).args.slice(-1), [text])
  same(M.driftArgs("dismiss", { eventId: UNIT, text: text }).args.slice(-1), [text])
  // blank alone is refused, never trimmed into something
  for (const blank of ["", "   "]) {
    assert.ok(M.logArgs(blank, "").error)
    assert.ok(M.agentNewArgs(blank).error)
    assert.ok(M.planArgs("new", { title: blank }).error)
    assert.ok(M.driftArgs("dismiss", { eventId: UNIT, text: blank }).error)
  }
})

// ADR-0038: what the desk's details show — the index's own rule, a case's
// intent, result and source, a decision's lead; each optional.
const bare = (() => {
  const x = JSON.parse(sample)
  for (const d of x.drift) delete d.rule
  for (const g of Object.keys(x.cases)) for (const c of x.cases[g]) { delete c.intent; delete c.result; delete c.source }
  for (const d of x.decisions) delete d.lead
  return x
})()

test("driftRuleInfo: the index's rule first, no engine call; drift show only without it (ADR-0038 §1)", () => {
  const idx = M.parseIndex(sample).index
  same(M.driftRuleInfo(null, null, UNIT, idx), { state: "known", rule: "always-red-paths", cls: "crisis" })
  same(M.driftRuleInfo(null, null, MESA, idx), { state: "known", rule: "package", cls: "attention" })
  // the index wins over an answer of drift show for the same item
  same(M.driftRuleInfo({ [UNIT]: { rule: "other", cls: "crisis" } }, null, UNIT, idx).rule, "always-red-paths")
  // without the field: the drift show path, unchanged
  same(M.driftRuleInfo(null, null, UNIT, bare), { state: "unknown", rule: "", cls: "" })
  same(M.driftRuleInfo(null, { eventId: UNIT, pending: true }, UNIT, bare).state, "pending")
  same(M.driftRuleInfo({ [UNIT]: { rule: "always-red-paths", cls: "crisis" } }, null, UNIT, bare).rule, "always-red-paths")
  same(M.driftRuleInfo(null, null, UNIT, undefined).state, "unknown")
  // a rule out of the schema's shape is no rule
  for (const bad of ["", "Always-Red", "a b", "x".repeat(65), 7, null]) {
    const x = JSON.parse(sample)
    x.drift.find(d => d.eventId === UNIT).rule = bad
    same(M.driftRuleInfo(null, null, UNIT, x).state, "unknown")
  }
  // the callout from the index's rule
  const d = M.eventDetail(idx, M.deskChangelog(idx), UNIT, M.driftRuleInfo(null, null, UNIT, idx))
  assert.ok(d.whyLoud.startsWith("The path matches your crisis list"), d.whyLoud)
})

test("caseDetail: intent, result and an imported case's source when the index has them (ADR-0038)", () => {
  const idx = M.parseIndex(sample).index
  const p = M.deskWork(idx)
  const c7 = M.caseDetail(idx, p, "C-2026-007")
  assert.ok(c7.intent.startsWith("Herdr-Orchestrator als Default-Agent registrieren — "), c7.intent)
  same(c7.result, "")
  same(c7.kv.filter(r => r[0] === "Imported from"), [["Imported from", "~/Notizen/aufgaben.md#4"]])
  const c1 = M.caseDetail(idx, p, "C-2026-001")
  same(c1.result, "Logbuch läuft, Baseline erfasst, `seldon doctor` ohne Befund.")
  same(c1.kv.filter(r => r[0] === "Imported from"), [])
  // an index without the fields: nothing shown, nothing broken
  const b = M.caseDetail(bare, M.deskWork(bare), "C-2026-007")
  same([b.intent, b.result, b.kv.some(r => r[0] === "Imported from")], ["", "", false])
  // a non-string is no text
  const odd = JSON.parse(sample)
  Object.assign(odd.cases.queued.find(c => c.id === "C-2026-007"), { intent: 7, result: ["x"], source: {} })
  const o = M.caseDetail(odd, M.deskWork(odd), "C-2026-007")
  same([o.intent, o.result, o.kv.some(r => r[0] === "Imported from")], ["", "", false])
})

test("decisionDetail: the lead as text when the index has it (ADR-0038 §2)", () => {
  const rows = M.decisionRows(sampleIndex)
  const d3 = M.decisionDetail(rows.find(r => r.id === "ADR-0003"))
  same(d3.text, "Zed wird Zweiteditor, Neovim bleibt Standard.")
  same(d3.lead, "The whole text is in the file; Open in editor shows it.")
  const b = M.decisionDetail(M.decisionRows(bare).find(r => r.id === "ADR-0003"))
  same([b.text, b.lead], ["", "The text is in the file; Open in editor shows it."])
})

// ---- The graph (WP-125, ADR-0034 §5) --------------------------------------

const graphSample = M.parseIndex(sample).index
const { bigIndex } = require("./graph-index.js")
const graphBig = M.parseIndex(JSON.stringify(bigIndex())).index
const nodeOf = (b, id) => b.nodes.find((n) => n.id === id)
const edgeIds = (b) => b.edges.map((e) => b.nodes[e.a].id + (e.dashed ? " ~ " : " - ") + b.nodes[e.b].id)

test("graphBuild: nodes from the index, changes only, crises from drift", () => {
  const b = M.graphBuild(graphSample, 400)
  same(b.numbers, { nodes: 68, edges: 26, areas: 6, cases: 8, decisions: 4, changes: 48, crises: 2, clusters: 0,
    folded: 0, events: 76, completed: 2 })
  // order: areas, cases, decisions, changes by day
  const kinds = b.nodes.map((n) => (n.kind === "crisis" ? "change" : n.kind))
  same([...new Set(kinds)], ["area", "case", "decision", "change"])
  // no case lifecycle, notes, corrections, state loss
  const changeIds = new Set(b.nodes.filter((n) => n.kind === "change" || n.kind === "crisis").map((n) => n.id))
  for (const e of graphSample.events) assert.strictEqual(changeIds.has(e.id), M.graphIsChange(e.kind), e.kind)
  for (const k of ["case-created", "note", "correction", "state-loss", "resolution"]) assert.ok(!M.graphIsChange(k), k)
  for (const k of ["install", "plugin-disable", "theme-set", "config-remove", "command", "snapshot-delete"]) assert.ok(M.graphIsChange(k), k)
  // crisis = event id in drift with crisis: true
  const crises = b.nodes.filter((n) => n.kind === "crisis").map((n) => n.id).sort()
  same(crises, graphSample.drift.filter((d) => d.crisis).map((d) => d.eventId).sort())
  // every case of the four lists
  same(b.nodes.filter((n) => n.kind === "case").map((n) => n.id).sort(),
    ["C-2026-001", "C-2026-002", "C-2026-003", "C-2026-004", "C-2026-005", "C-2026-006", "C-2026-007", "C-2026-008"])
  assert.strictEqual(nodeOf(b, "C-2026-001").done, true)
  assert.strictEqual(nodeOf(b, "C-2026-003").sub, "active · R3 · shell")
  assert.strictEqual(nodeOf(b, "C-2026-003").caseId, "C-2026-003")
  assert.strictEqual(b.footer, "Newest 76 events · 2 completed cases in the index")
})

test("graphBuild: edges event→case, case→area, decision→case, proposedCase dashed", () => {
  const b = M.graphBuild(graphSample, 400)
  const edges = edgeIds(b)
  assert.ok(edges.includes("C-2026-003 - area:shell"))
  assert.ok(edges.includes("ADR-0003 - C-2026-004") && edges.includes("ADR-0003 - C-2026-005"))
  assert.ok(edges.includes("ADR-0001 - C-2026-001"))
  // event.case → case, solid; the change node names its case
  const linked = graphSample.events.filter((e) => e.case === "C-2026-003" && M.graphIsChange(e.kind))
  assert.ok(linked.length > 0)
  for (const e of linked) {
    assert.ok(edges.includes(e.id + " - C-2026-003"), e.id)
    assert.strictEqual(nodeOf(b, e.id).caseId, "C-2026-003")
  }
  // drift.proposedCase → case, dashed
  const proposed = graphSample.drift.filter((d) => d.proposedCase)
  for (const d of proposed) assert.ok(edges.includes(d.eventId + " ~ " + d.proposedCase), d.eventId)
  assert.strictEqual(b.edges.filter((e) => e.dashed).length, proposed.length)
  // degrees follow the edges
  assert.strictEqual(b.deg.reduce((x, y) => x + y, 0), 2 * b.edges.length)
  // contract 1 (no decisions[].cases): no decision edges, nothing else lost
  const v1idx = JSON.parse(JSON.stringify(graphSample))
  for (const d of v1idx.decisions) delete d.cases
  assert.strictEqual(M.graphBuild(v1idx, 400).edges.length, b.edges.length - 3)
})

test("graphBuild: day index (event ts, case created, decision date, area = earliest neighbour)", () => {
  const b = M.graphBuild(graphSample, 400)
  assert.strictEqual(M.dateOfDay(b.first), "2026-09-01")
  assert.strictEqual(b.span, 30)
  assert.strictEqual(nodeOf(b, "C-2026-003").date, "2026-09-26")
  assert.strictEqual(nodeOf(b, "C-2026-003").day, 25)
  assert.strictEqual(nodeOf(b, "ADR-0002").date, "2026-09-02")
  const e = graphSample.events.find((x) => x.kind === "install")
  assert.strictEqual(nodeOf(b, e.id).date, e.ts.slice(0, 10))
  // areas: themes ← C-2026-005 (09-29), dev-env ← C-2026-001 (09-01), plugins has no neighbour → first day
  assert.strictEqual(nodeOf(b, "area:themes").date, "2026-09-29")
  assert.strictEqual(nodeOf(b, "area:dev-env").day, 0)
  assert.strictEqual(nodeOf(b, "area:plugins").day, 0)
  // an area only a case names still gets its node
  const extra = JSON.parse(JSON.stringify(graphSample))
  extra.cases.queued[0].area = "audio"
  assert.strictEqual(nodeOf(M.graphBuild(extra, 400), "area:audio").date, extra.cases.queued[0].created)
  // a drift item older than the index's events is still a node
  const old = JSON.parse(JSON.stringify(graphSample))
  old.events = old.events.filter((x) => x.id !== old.drift[1].eventId)
  assert.strictEqual(nodeOf(M.graphBuild(old, 400), old.drift[1].eventId).kind, "crisis")
})

test("graphBuild: nothing to draw without an index", () => {
  for (const idx of [null, undefined, "x", {}]) {
    const b = M.graphBuild(idx, 400)
    assert.strictEqual(b.empty, true)
    assert.strictEqual(b.nodes.length, 0)
  }
  assert.strictEqual(M.graphBuild(graphSample).nodes.length, 68)
})

test("graphBuild: beyond the cap, changes fold by day and source; areas, cases, decisions, crises never", () => {
  const b = M.graphBuild(graphBig, 400)
  assert.strictEqual(b.nodes.length, 400)
  assert.strictEqual(b.foldLevel, "day-source")
  assert.strictEqual(b.numbers.areas, 10)
  assert.strictEqual(b.numbers.cases, graphBig.cases.queued.length + graphBig.cases.active.length +
    graphBig.cases.verification.length + graphBig.cases.completed.length)
  assert.strictEqual(b.numbers.decisions, graphBig.decisions.length)
  assert.strictEqual(b.numbers.crises, 2)
  // every change is a node or in exactly one cluster
  assert.strictEqual(b.numbers.changes + b.numbers.crises + b.numbers.folded, 500)
  const clusters = b.nodes.filter((n) => n.kind === "cluster")
  assert.strictEqual(clusters.length, b.numbers.clusters)
  for (const c of clusters) {
    assert.strictEqual(c.label, "+" + c.count)
    assert.ok(c.count >= 2 && c.members.length <= 12 && c.members.length + c.more === c.count, c.id)
    assert.match(c.title, /^\d+ changes · \d{4}-\d{2}-\d{2} · [a-z]+$/)
  }
  assert.strictEqual(b.numbers.folded, clusters.reduce((n, c) => n + c.count, 0))
  // the biggest groups fold first: no unfolded day/source group is bigger than a folded one
  const crisisIds = new Set(graphBig.drift.filter((d) => d.crisis).map((d) => d.eventId))
  const sizes = {}
  for (const e of graphBig.events) if (M.graphIsChange(e.kind) && !crisisIds.has(e.id)) {
    const key = e.ts.slice(0, 10) + " · " + e.source
    sizes[key] = (sizes[key] || 0) + 1
  }
  const foldedKeys = new Set(clusters.map((c) => c.title.replace(/^\d+ changes · /, "")))
  const smallestFolded = Math.min(...clusters.map((c) => c.count))
  for (const key of Object.keys(sizes)) if (!foldedKeys.has(key)) assert.ok(sizes[key] <= smallestFolded, key)
  assert.match(b.footer, / · older ones are only in the logbook · 295 changes folded into 99$/)
  // a cluster carries its members' case links
  const linkedCluster = b.edges.find((e) => b.nodes[e.a].kind === "cluster" || b.nodes[e.b].kind === "cluster")
  assert.ok(linkedCluster)
  // a tighter cap folds coarser
  const tight = M.graphBuild(graphBig, 150)
  assert.ok(tight.nodes.length <= 150 && tight.nodes.length > 120, String(tight.nodes.length))
  assert.ok(["day", "week", "month"].includes(tight.foldLevel), tight.foldLevel)
  // fixed nodes alone over the cap: everything foldable folds, nothing else goes
  const tiny = M.graphBuild(graphBig, 10)
  assert.strictEqual(tiny.foldLevel, "month")
  assert.strictEqual(tiny.numbers.cases, b.numbers.cases)
})

test("graphFold: finest level that fits, biggest groups first, crises stay", () => {
  const evs = [
    { day: 10, source: "pacman", crisis: false }, { day: 10, source: "pacman", crisis: false },
    { day: 10, source: "pacman", crisis: false }, { day: 10, source: "config", crisis: false },
    { day: 11, source: "pacman", crisis: false }, { day: 11, source: "pacman", crisis: false },
    { day: 12, source: "pacman", crisis: true }, { day: 12, source: "pacman", crisis: true }
  ]
  same(M.graphFold(evs, 8), { level: "", groups: [], count: 8 })
  same(M.graphFold(evs, 6), { level: "day-source", groups: [[0, 1, 2]], count: 6 })
  same(M.graphFold(evs, 5), { level: "day-source", groups: [[0, 1, 2], [4, 5]], count: 5 })
  // day level: the config change joins day 10
  same(M.graphFold(evs, 4), { level: "day", groups: [[0, 1, 2, 3], [4, 5]], count: 4 })
  // the two crises never fold, so 3 is the floor (week of day 10 and 11 = one group)
  const r = M.graphFold(evs, 1)
  assert.strictEqual(r.count, 3)
})

test("graphState: start layout deterministic, positions kept by id, cut kept mid-replay", () => {
  const b = M.graphBuild(graphSample, 400)
  const s1 = M.graphState(b, null)
  const s2 = M.graphState(b, null)
  same(Array.from(s1.x), Array.from(s2.x))
  assert.strictEqual(s1.visCount, 68)
  assert.strictEqual(s1.cut, b.span)
  assert.strictEqual(s1.alpha, 1)
  for (let i = 0; i < 30; i++) M.graphStep(s1, 8)
  // the same build again: positions and sleep kept, nothing added
  const s3 = M.graphState(b, s1)
  same([s3.added, s3.removed], [0, 0])
  same(Array.from(s3.x), Array.from(s1.x))
  assert.strictEqual(s3.alpha, s1.alpha)
  // one more change: kept positions, the new node beside its case, a reheat
  const grown = JSON.parse(JSON.stringify(graphSample))
  grown.events.unshift({ id: "01M3ZZZZZZZZZZZZZZZZZZZZZZ", ts: "2026-10-01T18:00:00+02:00", source: "pacman",
    kind: "install", subject: "htop", actor: "human", case: "C-2026-003" })
  const b2 = M.graphBuild(grown, 400)
  const s4 = M.graphState(b2, s1)
  same([s4.added, s4.removed], [1, 0])
  const i = s4.at["C-2026-003"]
  assert.strictEqual(s4.x[i], s1.x[s1.at["C-2026-003"]])
  const n = s4.at["01M3ZZZZZZZZZZZZZZZZZZZZZZ"]
  assert.ok(Math.hypot(s4.x[n] - s4.x[i], s4.y[n] - s4.y[i]) < 80)
  assert.ok(s4.alpha >= 0.3 && !s4.sleeping)
  // a replay in progress keeps its day
  M.graphSetCut(s1, 10, false)
  assert.strictEqual(M.graphState(b2, s1).cut, 10)
})

test("graphSetCut: visible = day ≤ cut, monotonic over the days, growth beside a neighbour", () => {
  const b = M.graphBuild(graphSample, 400)
  const s = M.graphState(b, null)
  let last = -1
  for (let day = 0; day <= b.span; day++) {
    M.graphSetCut(s, day, true)
    assert.strictEqual(s.visCount, b.nodes.filter((n) => n.day <= day).length, "day " + day)
    assert.ok(s.visCount >= last)
    last = s.visCount
  }
  assert.strictEqual(last, b.nodes.length)
  // clamped
  M.graphSetCut(s, 999, false)
  assert.strictEqual(s.cut, b.span)
  M.graphSetCut(s, -5, false)
  assert.strictEqual(s.cut, 0)
  // a node that appears starts next to a visible neighbour
  M.graphSetCut(s, 24, false)
  const c = s.at["C-2026-003"] // day 25, area shell visible from day 25 too; its events later
  M.graphSetCut(s, 25, true)
  const shell = s.at["area:shell"]
  assert.ok(s.vis[c] && s.vis[shell])
  // the pinned node is let go when it disappears
  M.graphPin(s, c, 0, 0)
  M.graphSetCut(s, 0, false)
  assert.strictEqual(s.pinned, -1)
})

test("graphStep: alpha decays, sleeps after 200 ticks, wakes, holds the pinned node", () => {
  const b = M.graphBuild(graphSample, 400)
  const s = M.graphState(b, null)
  // Bounded: a layout that never sleeps fails here instead of hanging.
  let ticks = 0
  while (ticks < M.GRAPH_TICKS_MAX + 50 && M.graphStep(s, 8)) ticks++
  assert.strictEqual(ticks, M.GRAPH_TICKS_MAX)
  assert.ok(s.sleeping && s.alpha <= M.GRAPH_ALPHA_MIN * 1.0001, String(s.alpha))
  assert.strictEqual(M.graphStep(s, 8), false)
  // settled: nothing overlaps
  for (let p = 0; p < s.n; p++) for (let q = p + 1; q < s.n; q++)
    assert.ok(Math.hypot(s.x[p] - s.x[q], s.y[p] - s.y[q]) > 2, s.ids[p] + " / " + s.ids[q])
  // linked nodes end nearer than the average pair
  const linkLen = b.edges.map((e) => Math.hypot(s.x[e.a] - s.x[e.b], s.y[e.a] - s.y[e.b]))
  const mean = linkLen.reduce((x, y) => x + y, 0) / linkLen.length
  assert.ok(mean < 120, String(mean))
  // a drag wakes it and holds the node where the pointer is
  const i = s.at["C-2026-004"]
  M.graphPin(s, i, 500, -300)
  M.graphWake(s, 0.3)
  assert.ok(!s.sleeping && s.ticks === 0 && s.alpha >= 0.3)
  for (let t = 0; t < 20; t++) M.graphStep(s, 8)
  same([s.x[i], s.y[i]], [500, -300])
  // its neighbours follow
  const nb = Object.keys(M.graphNeighbours(s, i)).map(Number)
  assert.ok(nb.length > 0)
  assert.ok(nb.some((j) => s.x[j] > 100), "a neighbour moved towards the drag")
  M.graphPin(s, -1, 0, 0)
  assert.strictEqual(s.pinned, -1)
  // the budget counter
  assert.strictEqual(s.over, 0)
  const slow = M.graphState(b, null)
  M.graphStep(slow, -1)
  assert.ok(slow.lastMs >= 0 && slow.maxMs >= slow.lastMs)
})

test("graphStep: exact pairs up to 160 visible nodes, the quadtree above", () => {
  const small = M.graphState(M.graphBuild(graphSample, 400), null)
  M.graphStep(small, 1000)
  same([small.exactSteps, small.treeSteps], [1, 0])
  const big = M.graphState(M.graphBuild(graphBig, 400), null)
  for (let t = 0; t < 3; t++) M.graphStep(big, 1000)
  same([big.visCount, big.exactSteps, big.treeSteps], [400, 0, 3])
  // the replay's early days are small again: exact
  M.graphSetCut(big, 0, false)
  M.graphWake(big, 0.5)
  M.graphStep(big, 1000)
  assert.ok(big.visCount <= M.GRAPH_EXACT_MAX && big.exactSteps === 1, String(big.visCount))
})

test("graphBuild: case references that are prototype keys link nothing and throw nothing", () => {
  const odd = JSON.parse(JSON.stringify(graphSample))
  const keys = ["constructor", "__proto__", "toString", "hasOwnProperty", "valueOf"]
  odd.events.filter((e) => M.graphIsChange(e.kind)).slice(0, keys.length).forEach((e, i) => { e.case = keys[i] })
  odd.drift.forEach((d, i) => { d.proposedCase = keys[i % keys.length] })
  odd.decisions.forEach((d, i) => { d.cases = [keys[i % keys.length], "C-2026-001"] })
  odd.cases.queued[0].area = "constructor"
  const b = M.graphBuild(odd, 400)
  assert.strictEqual(b.empty, false)
  for (const e of b.edges) assert.ok(b.nodes[e.a] && b.nodes[e.b])
  assert.ok(b.nodes.every((n) => typeof n.caseId === "string" && (n.caseId === "" || /^C-[0-9]{4}-[0-9]{3,}$/.test(n.caseId))))
  // the area named "constructor" is a real area of its case
  assert.ok(edgeIds(b).includes(odd.cases.queued[0].id + " - area:constructor"))
  // a case reference that names another node (a decision, an area) links
  // nothing and offers no "Open case"
  const other = JSON.parse(JSON.stringify(graphSample))
  const changes = other.events.filter((e) => M.graphIsChange(e.kind))
  changes[0].case = "ADR-0003"
  changes[1].case = "area:themes"
  other.drift[0].proposedCase = "ADR-0004"
  other.decisions[0].cases = ["area:shell"]
  const bo = M.graphBuild(other, 400)
  const kinds = (e) => [bo.nodes[e.a].kind, bo.nodes[e.b].kind].sort().join("-")
  assert.ok(!bo.edges.some((e) => ["change-decision", "area-change", "crisis-decision", "area-decision"].includes(kinds(e))),
    bo.edges.map(kinds).join(" "))
  for (const id of [changes[0].id, changes[1].id]) assert.strictEqual(nodeOf(bo, id).caseId, "")
  // an event id that is a prototype key is a node like any other
  const odd2 = JSON.parse(JSON.stringify(graphSample))
  odd2.events.find((e) => M.graphIsChange(e.kind)).id = "__proto__"
  const b2 = M.graphBuild(odd2, 400)
  assert.strictEqual(b2.nodes.filter((n) => n.id === "__proto__").length, 1)
  const s = M.graphState(b2, null)
  assert.strictEqual(s.at["constructor"], undefined)
  assert.strictEqual(typeof s.at["__proto__"], "number")
})

test("graphBuild: more fixed nodes than the cap → a still picture that never ticks", () => {
  const many = JSON.parse(JSON.stringify(graphSample))
  for (let i = 0; i < 2000; i++) many.system.areas.push({ name: "area-" + i, hasAgentsMd: false, cases: 0 })
  const b = M.graphBuild(many, 400)
  assert.strictEqual(b.still, true)
  assert.strictEqual(b.numbers.areas, 2006)
  // every change folds as far as it goes (month level), crises stay
  assert.strictEqual(b.foldLevel, "month")
  assert.strictEqual(b.numbers.crises, 2)
  const s = M.graphState(b, null)
  assert.ok(s.still && s.sleeping && s.alpha === 0)
  assert.strictEqual(M.graphStep(s, 8), false)
  M.graphWake(s, 1)
  assert.strictEqual(s.sleeping, true)
  M.graphSetCut(s, 3, true)
  M.graphWake(s, 0.5)
  assert.strictEqual(M.graphStep(s, 8), false)
  // a drag still moves the node itself, at once
  const i = s.visList[0]
  M.graphPin(s, i, 77, -5)
  same([s.x[i], s.y[i]], [77, -5])
  // nothing overlaps on the spiral
  let close = 0
  for (let p = 0; p < 300; p++) for (let q = p + 1; q < 300; q++) if (Math.hypot(s.x[p] - s.x[q], s.y[p] - s.y[q]) < 4) close++
  assert.strictEqual(close, 0)
  // the cap holds again: live
  assert.strictEqual(M.graphBuild(graphSample, 400).still, false)
  assert.strictEqual(M.graphBuild(graphBig, 400).still, false)
})

test("graphRepelTree approximates the exact repulsion (Barnes–Hut, θ 0.9)", () => {
  const b = M.graphBuild(graphBig, 400)
  const s = M.graphState(b, null)
  for (let t = 0; t < 60; t++) M.graphStep(s, 1000)
  const zero = () => { for (let i = 0; i < s.n; i++) s.vx[i] = s.vy[i] = 0 }
  zero()
  M.graphRepelExact(s, 150)
  const exact = Array.from(s.vx).map((v, i) => [v, s.vy[i]])
  zero()
  M.graphRepelTree(s, 150)
  let err = 0
  let norm = 0
  for (let i = 0; i < s.n; i++) {
    err += Math.hypot(s.vx[i] - exact[i][0], s.vy[i] - exact[i][1])
    norm += Math.hypot(exact[i][0], exact[i][1])
  }
  assert.ok(err / norm < 0.05, "relative error " + (err / norm).toFixed(3))
  // above GRAPH_EXACT_MAX visible nodes the step uses the tree
  assert.ok(s.tree && s.tree.count > s.visCount)
})

test("graphPick, graphNeighbours, graphInfo", () => {
  const b = M.graphBuild(graphSample, 400)
  const s = M.graphState(b, null)
  const i = s.at["C-2026-003"]
  assert.strictEqual(M.graphPick(s, s.x[i] + 3, s.y[i] - 3, 0), i)
  assert.strictEqual(M.graphPick(s, s.x[i] + 20, s.y[i], 0) === i, false)
  assert.strictEqual(M.graphPick(s, s.x[i] + 9, s.y[i], 4), i)
  M.graphSetCut(s, 0, false)
  assert.strictEqual(M.graphPick(s, s.x[i], s.y[i], 0), -1)
  const info = M.graphInfo(b, i)
  assert.strictEqual(info.line, "Case · since 2026-09-26 · day 25 · " + b.deg[i] + " links")
  assert.strictEqual(info.title, "C-2026-003 Omarchy auf 4.0.7 aktualisieren")
  assert.strictEqual(info.caseId, "C-2026-003")
  const crisis = b.nodes.findIndex((n) => n.kind === "crisis")
  assert.match(M.graphInfo(b, crisis).line, /^Crisis · since \d{4}-\d{2}-\d{2} · day \d+ · \d+ links?$/)
  assert.strictEqual(M.graphInfo(b, -1), null)
  assert.strictEqual(M.graphInfo(null, 0), null)
  same(Object.keys(M.graphNeighbours(s, s.at["ADR-0003"])).map((k) => s.ids[k]).sort(), ["C-2026-004", "C-2026-005"])
  const big = M.graphBuild(graphBig, 400)
  const ci = big.nodes.findIndex((n) => n.kind === "cluster")
  const cinfo = M.graphInfo(big, ci)
  assert.strictEqual(cinfo.kindLabel, "Folded changes")
  assert.ok(cinfo.members.length > 0)
})

test("graphBounds, graphFit", () => {
  const s = M.graphState(M.graphBuild(graphSample, 400), null)
  const bounds = M.graphBounds(s)
  assert.ok(bounds.x1 > bounds.x0 && bounds.y1 > bounds.y0)
  const fit = M.graphFit({ x0: -100, y0: -50, x1: 100, y1: 50 }, 440, 240, 20, 5)
  same(fit, { x: -0, y: -0, k: 2 })
  same(M.graphFit({ x0: 0, y0: 0, x1: 10, y1: 10 }, 400, 400, 0, 2.5), { x: -12.5, y: -12.5, k: 2.5 })
  same(M.graphFit(null, 100, 100, 0, 2), { x: 0, y: 0, k: 1 })
  M.graphSetCut(s, 0, false)
  M.graphSetCut(s, -1, false)
  const none = M.graphState(M.graphBuild(null, 400), null)
  assert.strictEqual(M.graphBounds(none), null)
})

test("graphShape: disc, square, spindle; legend kinds", () => {
  const calls = []
  const ctx = new Proxy({}, { get: (_, name) => (...args) => calls.push(name + ":" + args.length) })
  for (const kind of ["case", "area", "change", "cluster", "decision", "crisis"]) {
    calls.length = 0
    M.graphShape(ctx, kind, 10, 10, 5)
    const want = kind === "decision" ? ["moveTo:2", "lineTo:2", "lineTo:2", "lineTo:2", "closePath:0"]
      : kind === "crisis" ? ["moveTo:2", "quadraticCurveTo:4", "quadraticCurveTo:4", "quadraticCurveTo:4", "quadraticCurveTo:4", "closePath:0"]
      : ["moveTo:2", "arc:6"]
    same(calls, want)
  }
  same(M.GRAPH_LEGEND.map((e) => e.kind), ["case", "area", "decision", "change", "crisis", "cluster"])
  assert.ok(M.graphMinRadius("area") > M.graphMinRadius("change"))
})

console.log("model.test.js: " + passed + " passed" + (process.exitCode ? ", some FAILED" : ""))
