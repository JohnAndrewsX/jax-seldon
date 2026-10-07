// Timing of Model.periodTable under node (WP-031): the work the service does
// on every index write for the Prime Radiant (one pass per series, then the
// charts of four periods). Budget: 10 ms on the sample index scaled ×10 in
// a plain function scope (fastest of 31 runs, which a busy machine can only
// slow down; idle about 1.9 ms, about 4 ms with the host fully loaded).
// Also reports 7000 timeline rows (the WP-030 review's case) and, for
// comparison, the cut through seriesInPeriod (four passes per series).
//
// Two loads of Model.js: "plain" (one function scope, closer to how a JS
// engine runs a QML script import; the gate) and "sandbox" as
// model.test.js loads it (a vm context, where every top-level name is a slow
// contextified global lookup; reported only: about 8× slower and swings
// from 14 to 30 ms with the host's load).
// Run: node tests/plugin/model.bench.js
"use strict"

const fs = require("fs")
const path = require("path")
const vm = require("vm")

const root = path.resolve(__dirname, "../..")
const source = fs.readFileSync(path.join(root, "plugin/Model.js"), "utf8")
const M = {}
vm.createContext(M)
vm.runInContext(source, M, { filename: "Model.js" })
const P = new Function(source + "\nreturn { periodTable, seriesInPeriod, periodWindow, todayDate, PERIODS }")()

const sample = M.parseIndex(fs.readFileSync(path.join(root, "fixtures/index.sample.json"), "utf8")).index
const BUDGET_MS = 10

// Every series list and case group repeated `n` times (rows unchanged, so
// every copy lands in the same windows: the worst case for the charts).
function scaled(index, n) {
  const out = JSON.parse(JSON.stringify(index))
  const rep = (list) => [].concat(...Array.from({ length: n }, () => list))
  for (const key of ["heatmap", "packages", "drift", "timeline"]) out.series[key] = rep(index.series[key])
  for (const group of Object.keys(index.cases)) out.cases[group] = rep(index.cases[group])
  return out
}

// `rows` timeline rows over the year before today: every tenth a case span
// (a third of them open), the rest releases, snapshots and crises.
function timelineRows(index, rows) {
  const out = JSON.parse(JSON.stringify(index))
  const today = M.dayNumber(index.today.date)
  const kinds = ["snapshot", "release", "crisis"]
  out.series.timeline = Array.from({ length: rows }, (_, i) => {
    const day = today - (i * 7919) % 366
    if (i % 10 === 0) {
      const end = i % 30 === 0 ? null : M.dateOfDay(Math.min(today, day + (i % 17)))
      return { kind: "case", ts: M.dateOfDay(day), end, label: "C-2026-" + i, ref: "C-2026-" + i }
    }
    const hh = String(i % 24).padStart(2, "0")
    return { kind: kinds[i % 3], ts: M.dateOfDay(day) + "T" + hh + ":15:00+02:00", label: "item " + i, ref: String(i) }
  })
  return out
}

// The WP-030 cut: seriesInPeriod for every period and series.
function oldCut(m, index) {
  const today = m.todayDate(index)
  for (const p of m.PERIODS) {
    const win = m.periodWindow(p.id, today)
    for (const key of ["heatmap", "packages", "drift", "timeline"]) m.seriesInPeriod(index.series, key, win)
  }
}

// { median, best } of `runs` timed calls after five warm-up calls, in ms.
function timed(fn, runs) {
  for (let i = 0; i < 5; i++) fn()
  const times = []
  for (let i = 0; i < runs; i++) {
    const t = process.hrtime.bigint()
    fn()
    times.push(Number(process.hrtime.bigint() - t) / 1e6)
  }
  times.sort((a, b) => a - b)
  return { median: times[Math.floor(times.length / 2)], best: times[0] }
}

const cases = [
  ["sample", sample],
  ["sample ×10", scaled(sample, 10)],
  ["7000 timeline rows", timelineRows(sample, 7000)]
]
let failed = false
for (const [name, index] of cases) {
  const rows = ["heatmap", "packages", "drift", "timeline"].map((k) => index.series[k].length).join("/")
  const ms = timed(() => M.periodTable(index), 31)
  const cut = timed(() => oldCut(M, index), 31)
  const plain = timed(() => P.periodTable(index), 31)
  const plainCut = timed(() => oldCut(P, index), 31)
  console.log(`model.bench: ${name} (rows ${rows}): periodTable ${ms.median.toFixed(2)} ms sandbox / ` +
    `${plain.median.toFixed(2)} ms plain (median); WP-030 seriesInPeriod cut alone ${cut.median.toFixed(2)} / ` +
    `${plainCut.median.toFixed(2)} ms`)
  if (name === "sample ×10" && plain.best > BUDGET_MS) {
    console.error(`model.bench: ${name} over the ${BUDGET_MS} ms budget (plain, fastest run ${plain.best.toFixed(2)} ms)`)
    failed = true
  }
}
// The graph (WP-125, ADR-0034 §5): one layout tick (Model.graphStep) at
// 400 nodes, the cap, on tests/plugin/graph-index.js's busy index; gate 8 ms
// (fastest of 31 plain runs; the shell's own engine, QV4, is about 10× slower
// than node here: desk-view.sh reports its tickMs at 400 nodes). Also
// reports graphBuild, the service's work per index change.
const G = new Function(source + "\nreturn { graphBuild, graphState, graphStep, graphWake, parseIndex }")()
const { bigIndex } = require("./graph-index.js")
const big = G.parseIndex(JSON.stringify(bigIndex())).index
const GRAPH_BUDGET_MS = 8
const build = G.graphBuild(big, 400)
const state = G.graphState(build, null)
for (let i = 0; i < 40; i++) G.graphStep(state, 1000)
const tick = timed(() => {
  G.graphWake(state, 0.5)
  G.graphStep(state, 1000)
}, 31)
const built = timed(() => G.graphBuild(big, 400), 31)
console.log(`model.bench: graph at ${build.nodes.length} nodes, ${build.edges.length} edges: graphStep ` +
  `${tick.median.toFixed(2)} ms (median, plain), graphBuild ${built.median.toFixed(2)} ms`)
if (build.nodes.length !== 400) {
  console.error(`model.bench: the graph index gives ${build.nodes.length} nodes, not the cap of 400`)
  failed = true
}
if (tick.best > GRAPH_BUDGET_MS) {
  console.error(`model.bench: graphStep over the ${GRAPH_BUDGET_MS} ms budget (fastest run ${tick.best.toFixed(2)} ms)`)
  failed = true
}
process.exitCode = failed ? 1 : 0
