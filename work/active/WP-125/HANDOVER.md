# WP-125 — Handover: the graph (desk section 8)

Branch `wp/125-graph` from `next` (7ac3bda: contract v2, the desk shell,
sections 1–7); merge into `next`. Plan and the decisions the WP left
open: [PLAN.md](PLAN.md); the same decisions are listed below.

## What was done

- **`Model.js`** (a block at the end): `graphBuild(index, cap)` — nodes
  (areas, the cases of all four lists, decisions, change events; crises
  from `drift[]`), edges (event → case, case → area, `decisions[].cases`
  → case, `drift.proposedCase` → case dashed), the day index, folding
  beyond 400 (`graphFold`), the footer; `graphState` (positions kept by
  id), `graphSetCut`, `graphStep(state, budgetMs)` (one force iteration:
  exact pairs up to 160 nodes, Barnes–Hut above; alpha decay to sleep
  after 200 ticks; the pinned drag node), `graphWake`, `graphPin`,
  `graphPick`, `graphNeighbours`, `graphInfo`, `graphBounds`,
  `graphFit`, `graphShape`, `graphWarm`. 13 new unit tests
  (`model.test.js`, 137 in all); `model.bench.js` gates one step at 400
  nodes (≤ 8 ms; 0.25 ms under node) and reports `graphBuild`.
  `tests/plugin/graph-index.js` makes the busy index (500 events, 66
  cases, 20 decisions, 10 areas → 400 nodes, 295 changes in 99 groups)
  for the bench, the tests and the harness.
- **`components/graph/GraphCanvas.qml`**: the canvas, the Timer (34 ms,
  only while section 8 is shown in the open desk and the layout awake),
  per-tick timing, drag / pan / wheel zoom / hover, the card with *Open
  case*, auto-fit until the view is moved, label budget, the replay.
- **`sections/Graph.qml`**: row 1 (title, caption, *Play growth*, the
  date slider, "date · N nodes [of M]"), row 2 (legend, keys), the
  canvas, the footer; keys; `select`; `view()`, `graphView()`.
- **`Service.qml`**: `graph` (built per index once section 8 was opened:
  `graphWanted`), `graphLayout` (the layout kept across desk opens).
  **`Desk.qml`**: `view().graph` (also while another section is shown).
  `SectionStub.qml` deleted (no stub left).
- **Harness**: steps `graphPlay`, `graphCut:<day>`, `graphHover:<id>`,
  `graphDrag:<id|empty>:<dx>,<dy>`; the report's `graphWanted`,
  `graphNodes`. `desk-view.sh` section 11 (11a–11h, 83 checks) and graph
  shots in the three themes. `tests/plugin/graph-live.sh` +
  `harness/graph-live.qml`: the real desk in its layer-shell window on a
  session, read only, with an event-loop probe (not in `just check`).
- **Docs**: SPEC-PLUGIN §2, §5.3, §5.4 (section 8: data, folding, layout,
  the shell thread's budget, screen, pointer and card, replay), §7, §8;
  KEYBINDINGS.md; TESTING.md (model tests, bench, section 11, §3d the
  live run with the test-host numbers); guide 03 "The graph" and 05 (the
  index bounds the graph), en and de.

## Decisions (what the WP left open)

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

## How it was verified

- `omarchy plugin validate plugin/` and `just qmllint` (46 files, token
  check) before every plugin commit.
- `node tests/plugin/model.test.js`: 137 passed. `model.bench.js`:
  graphStep 0.25 ms at 400 nodes (gate 8), graphBuild 0.67 ms.
- `desk-view.sh` section 11: 83 passed on its own (sample: 200 ticks,
  `tickMsMax` 2–3 ms; busy index: 4–5 ms). DESK_SHOTS renders of the
  graph in Tokyo Night, Kanagawa and Catppuccin Latte (settled, hover,
  50 %, replay) checked by eye.
- **QV4 numbers on the dev host** (a scratch Quickshell instance timing
  Model.js): exact repulsion at 400 nodes 9.8 ms typed / 5.6 ms plain
  arrays; Barnes–Hut 2.2 ms; a whole step 2.1 ms; first ticks after
  `graphWarm` 3 ms like the rest; graphBuild on 500 events 5–7 ms.
- **Test host** (authorised; Omarchy 4.0.4, Quickshell 0.3.1, 6 cores,
  1920×1080 at 1.25; the "dirty" host before its reinstall): `next`'s
  engine built here (musl, disk target), copied with the plugin to
  `~/.cache/seldon-smoke-wp125` (removed afterwards); a scratch HOME,
  `seldon init --non-interactive --since 2026-07-09 --baseline --no-git`
  — the ADR-0033 90 days: 2267 events (the host's install burst of
  2026-08-14 and seven weeks), the index's newest 500. Then
  `tests/plugin/graph-live.sh` on that index and on the busy index, the
  real layer-shell desk (1528×830 on HDMI-A-2):
  - real backfill, 152 nodes (one "+355" upgrade day): `tickMsMax`
    **3 ms** over 659 ticks, step ≤ 2;
  - busy index, 400 nodes / 229 edges: `tickMsMax` **7 ms** over 691
    ticks, step ≤ 6, drawing ≤ 2, **none over 8**;
  - both replays grew monotonically to every node; switching to Today
    stopped the ticks; the layout slept after 200 ticks each time.
  The first live run found two things, fixed before the numbers above:
  the first tick ran interpreted (10 ms; `graphWarm`, one function per
  body) and the slider animation throttled the replay (above).
  The event-loop probe's remaining gaps (≤ 20 ms, a few per phase, also
  with the graph hidden) are frames, not work: Qt's threaded render
  loop holds the GUI thread to vsync while any QML animation runs (a bare
  layer-shell window with one `Behavior` shows 170 such gaps in 3 s; a
  Timer-moved rectangle none); `QSG_RENDER_TIMING` showed the GUI
  thread's share of each frame at 0–1 ms. One screenshot of the live
  graph was checked by eye (the host's theme, mid-replay) and deleted.
- `flock /tmp/seldon-check.lock just check`: see "Final check".

## What was not done / open

- The plugin README, `preview.png` and the CHANGELOG: WP-126.
- A `seldon graph --json` export over the whole logbook: deferred by
  ADR-0034 §5; the footer says what the index holds.
- For WP-126 / the desk in general: any running QML animation holds the
  shell thread to the display's frames (decision 14). The graph avoids
  it during the replay; other desk animations (the Settings slider, a
  `Behavior` on hover) do the same briefly, as the bar's own widgets do.
- Commit 6afbb3b (the model) already deleted `SectionStub.qml`, which
  `Graph.qml` used until 7d342e1: the tree between them does not load
  section 8 (for a bisect).
- `~/Work/johnandrewsx/jax-seldon-private/gates/target-wp125` holds the
  engine build and the check's target (disk); remove it after the merge.

## Security (for stage 2)

- No new process, command or file write: the graph reads only the index
  through the service; `graph-live.sh` runs its own read-only instance
  (dev mode) and is not part of the plugin.
- *Open case* only switches sections (`Desk.section("work")`,
  `Desk.select(id)`), with ids that passed `CASE_ID` in `graphBuild`.
- Every text from the index (labels, card) is drawn as plain text
  (`fillText`, `Text.PlainText`).
- The live runs took the test host's keyboard for about 30 s each (five
  runs) and wrote only under `~/.cache/seldon-smoke-wp125`, removed.

## Final check

`flock /tmp/seldon-check.lock just check` on 74b4e00 (log
`check-wp125-r1.log`, `CARGO_TARGET_DIR` on disk): **exit 0** (`check:
ok`; engine tests ok, install.test 209, deploy-test-host 190, docs-check
ok, qmllint ok 46 files, model.test.js 137, model.bench ok (graphStep
0.35 ms at 400 nodes), real-home-guard 11, service-states 328,
desk-view 1431 (the busy index's slowest tick 4 ms), bar-view 194).
Only this handover and PLAN.md's decisions were added after that commit.
