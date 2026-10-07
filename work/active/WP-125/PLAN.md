# WP-125 — Plan: the graph (desk section 8)

Branch `wp/125-graph` from `next` (7ac3bda: contract v2, the desk shell,
sections 1–7); merge into `next`. Inputs read: AGENTS.md (§7, §8, Omarchy
first), ADR-0034 (§3, §5, §8), WP-125, the handovers and plans of WP-121,
WP-122 and WP-123, `tests/plugin/COVERAGE.md`, SPEC-PLUGIN §5–§8, the
prototype's `graph()` and NOTES.md (private; read, not copied),
`Model.js` (periods, charts, `MARKER_PATHS`, day helpers),
`components/overlay/ChartCanvas.qml` and `Timeline.qml` (Canvas and A12
patterns), `sections/Radiant.qml` (a solo section), `Service.qml`,
`Desk.qml`, the harness (`harness/desk.qml`, `desk-view.sh`),
`model.bench.js`, the fixture (contract 2, `decisions[].cases`).

## Steps

1. `Model.js` (a new block at the end): `GRAPH_CAP`, `graphBuild(index,
   cap)` (nodes, edges, day index, folding beyond the cap, footer
   numbers), `graphState(build, prev)` (typed arrays, positions kept by id
   across index updates, deterministic start), `graphSetCut`,
   `graphStep(state, budgetMs)` (one force iteration, alpha decay, sleep
   after 200 ticks, the pinned drag node), `graphWake`, `graphPick`,
   `graphInfo`, `graphBounds`/`graphFit`, `graphShape` (the node shapes
   on any 2d context, shared by the canvas and the legend). Unit tests
   for build, fold, edges, day index, step, pick, info; `model.bench.js`
   gains the graph step at 400 nodes (≤ 8 ms, gate).
2. `Service.qml`: `graph` built when the index changes (ADR-0034 §3).
3. `components/graph/GraphCanvas.qml`: the Canvas, the Timer (≤ 30 Hz,
   only while the section is shown and the layout awake), drag, pan,
   zoom, hover, the hover card with *Open case*, per-tick timing.
4. `sections/Graph.qml`: row 1 (title, caption, Play, the date slider,
   the legend), the canvas, the footer; keys; `select`; `view()`.
   `SectionStub.qml` goes (no user left).
5. `Desk.qml`: `view().graph` (also while another section is shown, so
   the harness can see that nothing ticks there).
6. Harness: steps `graphPlay`, `graphCut:<day>`, `graphHover:<id>`,
   `graphDrag:<id|empty>:<dx>,<dy>`; `desk-view.sh` section 11 (settle and
   sleep, budget, replay monotonic, hover card and Open case, drag wakes,
   pan/zoom without ticks, hidden section without ticks, folding on a
   synthetic big index, no index, theme shots).
7. Early risk check: the step and paint times inside Quickshell (QV4) on
   the dev host before the QML is finished; then on the test host with a
   backfilled 90-day logbook.
8. Docs: SPEC-PLUGIN §2, §5.3, §5.4 (section 8), §8; KEYBINDINGS.md;
   TESTING.md; guide paragraph en/de; handover.

## Decisions (what the WP leaves open)

1. **Built in the service, lazily.** ADR-0034 §3: the service prepares,
   the section only lays out and draws. `graphBuild` costs about 5 ms of
   QV4 on 500 events (measured, like `periodTable`), so the service builds
   only once section 8 was opened in the shell session, then on every
   index change.
2. **Folding.** Beyond 400 nodes changes fold at the finest level that
   fits — day and source (the ADR's), else day, ISO week, month — the
   biggest groups first and only as many as needed. Areas, cases,
   decisions **and crises** never fold (a crisis must stay visible). A
   cluster carries its members' links; its card lists up to 12 changes.
   On the test host's real backfill this folded one upgrade day of 355
   changes into one "+355" node.
3. **What counts as a node.** Open drift items the index no longer lists
   among its events are still change nodes (a crisis older than the
   newest 500 events stays); an area a case names but `system.areas`
   lacks gets a node; an area without a neighbour sits at day 0; a case
   without `created` falls back to `started`, `closed`, today.
4. **Layout.** The prototype's forces with retuned constants (repulsion
   150/d, gravity 0.02, springs 0.1 to 30 + both radii, damping 0.6,
   speed 12), scaled by alpha, which decays from 1 to 0.001 in exactly
   200 ticks; deterministic start (an FNV hash and a sunflower spiral, no
   `Math.random`), so the same index gives the same picture and the tests
   are stable. Exact pairs took 10 ms per tick in QV4 at 400 nodes
   (0.3 ms under node), so above 160 visible nodes the repulsion is a
   Barnes–Hut quadtree in flat arrays (θ 0.9, d3's; within 5 % of the
   exact force, tested); plain arrays (QV4 is 1.7× faster on them than on
   typed arrays); `graphWarm` compiles the functions before the first
   real tick.
5. **`graphStep(state, budgetMs)`**: one iteration per tick; `budgetMs`
   is the tick budget it counts overruns against (`over`), not a
   time-slicing knob — a slower host settles in the same 200 ticks.
6. **What `tickMs` measures**: the step plus the drawing calls of its
   paint, the shell thread's share. The canvas rasterises on its own
   thread (`Canvas.Threaded`: immediate rasterising blocked the event
   loop 7 ms per frame, threaded at most 4 ms in total, measured);
   `paintMs` (until the picture is there) is reported beside it. One path
   per node (one path with 400 antialiased discs: 20 ms; 400 paths:
   2 ms), nothing allocated per paint but the focus's neighbour set.
7. **What wakes the layout**: dragging a node and the replay (they change
   positions). Pan, zoom and hover only repaint — they change no
   position, so they need no tick.
8. **The layout survives a close.** The loader drops the desk on hide;
   the service keeps the layout, so a reopened desk shows it settled
   without a tick.
9. **Shapes and colours** (SPEC-PLUGIN §7): case = accent disc (closed
   cases at 50 %), area = foreground ring, decision = foreground square
   at 72 %, change = foreground dot at 42 % (larger with more links),
   crisis = urgent concave spindle (A12's crisis shape), folded = a
   ringed foreground dot. Nodes keep a minimum size on screen.
10. **Labels**: at most 40 at rest (areas, crises, open cases, decisions,
    closed cases, groups); while the layout moves only areas and crises
    (16); cases, decisions and groups from zoom 0.5; the focus and its
    neighbours always.
11. **The card** sits at the top right (the prototype's place), shows the
    hovered node, else the kept one, else the last hovered (so the
    pointer can reach *Open case*); a click on a node keeps it, a click
    on the background or Esc lets it go. *Open case* for a case and for a
    change linked to one.
12. **The view fits** the visible nodes until the user pans, zooms or
    drags; `0` fits again.
13. **Keys**: ←/→ the day, Space / Enter / `p` play-pause, `0` fit, `-` /
    `=` zoom, Esc pause → let the card go → the desk. `+` stays the
    desk's new case.
14. **Replay**: from day 0 when the cut is on the last day, else on from
    the cut; about 50 steps 120 ms apart; a new node starts beside a
    visible neighbour. The slider's knob holds still while playing: its
    `Behavior` animation throttled the shell thread to the display's
    frames for the whole replay (266 gaps of ~17 ms on the test host).
15. **The harness's timing gate.** On the sample every reported `tickMs`
    ≤ 8 and at most 2 of the 200 ticks over it; on the busy index at most
    5 of 200. Reason: the dev host compiles other WPs at the same time;
    under that load single ticks of the 67-node sample measured 9–16 ms
    (their normal is 1–3 ms), so a strict every-tick gate would fail the
    shared `just check` at random. `slowTicks` prints them. The strict
    every-tick statement is the test host's (below: none over).
16. **`view().graph`** at the top level of the desk's view, not in
    `sectionView`, so a check sees the graph while another section is
    shown.
17. **"guide 05 en/de paragraph"**: guide 05 is the CLI reference and the
    graph has no command; the paragraph went to guide 03 (daily use, the
    Prime Radiant's neighbour) and 05 got two sentences under `seldon
    index` (the index bounds the graph; no whole-logbook export yet).
    WP-126 rewrites guide 03 for the desk.
