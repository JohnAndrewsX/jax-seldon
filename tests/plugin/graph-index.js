// A big index for the graph (WP-125): the sample grown to what a busy
// machine's index holds — 500 changes over the 90 days before the sample's
// today (most on few busy days), 50 completed cases, more active and queued
// ones, 10 areas, 20 decisions naming cases, a few crises. Deterministic.
// model.bench.js times the layout on it; desk-view.sh draws it.
// Run: node tests/plugin/graph-index.js > big.json
"use strict"

const fs = require("fs")
const path = require("path")
const vm = require("vm")

const root = path.resolve(__dirname, "../..")

function bigIndex() {
  const M = {}
  vm.createContext(M)
  vm.runInContext(fs.readFileSync(path.join(root, "plugin/Model.js"), "utf8"), M, { filename: "Model.js" })
  const index = JSON.parse(fs.readFileSync(path.join(root, "fixtures/index.sample.json"), "utf8"))
  const today = M.dayNumber(index.today.date)
  let seed = 5
  const rnd = () => (seed = (seed * 9301 + 49297) % 233280) / 233280
  const areas = ["dev-env", "hyprland", "packages", "plugins", "shell", "themes", "audio", "network", "editor", "backup"]
  index.system.areas = areas.map((name) => ({ name, hasAgentsMd: false, cases: 0 }))
  const kase = (i, status) => ({
    id: "C-2026-" + String(100 + i).padStart(3, "0"), title: "Case " + i, status, zone: "green", risk: "R1",
    priority: "normal", area: areas[i % areas.length], created: M.dateOfDay(today - Math.floor(rnd() * 90)),
    started: null, closed: null, snapshotBefore: null, agents: [], events: [], tags: [], path: "work/x.md",
    steps: { total: 1, done: 0 }
  })
  index.cases.completed = Array.from({ length: 50 }, (_, i) => kase(i, "completed"))
  index.cases.active = index.cases.active.concat(Array.from({ length: 4 }, (_, i) => kase(50 + i, "active")))
  index.cases.queued = index.cases.queued.concat(Array.from({ length: 6 }, (_, i) => kase(60 + i, "queued")))
  index.decisions = index.decisions.concat(Array.from({ length: 16 }, (_, i) => ({
    id: "ADR-" + String(i + 10).padStart(4, "0"), title: "Decision " + i, status: "accepted",
    date: M.dateOfDay(today - Math.floor(rnd() * 90)), path: "decisions/x.md", cases: [index.cases.completed[i].id]
  })))
  const kinds = [["pacman", "upgrade"], ["pacman", "upgrade"], ["pacman", "install"], ["config", "config-change"],
    ["snapper", "snapshot"], ["agent", "command"], ["theme", "theme-set"], ["plugins", "plugin-update"]]
  const linked = index.cases.completed.concat(index.cases.active)
  const crockford = "0123456789ABCDEFGHJKMNPQRSTVWXYZ"
  index.events = Array.from({ length: 500 }, (_, i) => {
    const day = today - Math.floor(Math.pow(rnd(), 1.6) * 90)
    const [source, kind] = kinds[Math.floor(rnd() * kinds.length)]
    const id = "01M3VZNFC0M2FQVGJBZGX9" + [3, 2, 1, 0].map((p) => crockford[Math.floor(i / Math.pow(32, p)) % 32]).join("")
    const e = { id, ts: M.dateOfDay(day) + "T1" + (i % 10) + ":00:00+02:00", source, kind, subject: "package-" + i, actor: "human" }
    if (rnd() < 0.3) e.case = linked[Math.floor(rnd() * linked.length)].id
    return e
  })
  index.events.sort((a, b) => (a.ts < b.ts ? 1 : a.ts > b.ts ? -1 : 0))
  index.drift = index.events.filter((e) => !e.case).slice(0, 4).map((e, i) => ({
    eventId: e.id, ts: e.ts, source: e.source, kind: e.kind, subject: e.subject, actor: e.actor, zone: "red",
    crisis: i < 2, proposedCase: i === 3 ? linked[0].id : null
  }))
  return index
}

module.exports = { bigIndex }

if (require.main === module) process.stdout.write(JSON.stringify(bigIndex(), null, 1) + "\n")
