# SPEC-PLUGIN.md — `jax.seldon`, the Quattro plugin

Normative. Lives in `plugin/`, installed to `~/.config/omarchy/plugins/jax.seldon`.

## 1. Manifest

```json
{
  "schemaVersion": 1,
  "id": "jax.seldon",
  "name": "JAX Seldon",
  "version": "0.1.0",
  "author": "JohnAndrewsX",
  "license": "MIT",
  "description": "Flight recorder and planning desk for your Omarchy system: ledger, journal, cases, drift, and the Prime Radiant overlay.",
  "kinds": ["service", "bar-widget", "overlay"],
  "entryPoints": { "service": "Service.qml", "barWidget": "BarWidget.qml", "overlay": "Overlay.qml" },
  "barWidget": { "displayName": "Seldon", "category": "System", "allowMultiple": false, "defaultSection": "right" },
  "seldon": { "contractVersion": 1, "engineMin": "0.1.0" }
}
```

Verify the exact manifest keys against `$OMARCHY_PATH/shell/README.md`
(`/usr/share/omarchy` on a package install) before committing (WP-010 does
this); the shell is the source of truth. Do **not** add `panel` to `kinds`:
the shell's panel loader picks one UI kind per plugin id (`panel` before
`overlay` before `menu`), so `panel` would take `summon`/`toggle` away from
the Prime Radiant (see memory/omarchy-shell.md, WP-001 findings).

## 2. Files

```
plugin/
├── manifest.json
├── Service.qml         data: watches index.json, runs capture timer, exposes model
├── BarWidget.qml       pill; loads Panel.qml
├── Panel.qml           tabbed panel
├── Overlay.qml         Prime Radiant
├── Model.js            pure functions: formatting, colour mapping, aggregation for charts
├── components/
│   ├── Tabs.qml  EventRow.qml  CaseCard.qml  Kanban.qml  Banner.qml
│   ├── Heatmap.qml  Series.qml  DriftBars.qml  RiskDonut.qml  Timeline.qml
│   └── QuickEntry.qml
├── README.md  LICENSE  preview.png
└── fixtures -> ../fixtures (NOT a symlink in the plugin folder; copied in CI for dev builds)
```

## 3. Service.qml

- `FileView` on `~/.local/state/seldon/index.json` with `watchChanges: true`;
  on change parse JSON (try/catch), validate `contractVersion`, publish
  `index` property. Parsing happens on the shell thread — the index is
  small (< 1 MB by contract); if it grows, move parsing to a `Process`
  that emits a trimmed view.
- Timer: every 15 min and at start, `Process { command: ["seldon",
  "capture", "--all", "--json", "--quiet"] }`; then `["seldon", "status",
  "--json"]`. Never both at once; a `busy` flag serialises calls.
- Engine detection: at start run `["seldon", "--version"]`; map failures
  to `status: engineMissing` (the property is named `status`, because
  `state` clashes with `Item.state`). States: `ok | engineMissing | notInitialised |
  indexMissing | indexStale (> 2 h) | contractMismatch`.
- Exposes `function run(args)` for other files; **only fixed argument
  arrays**, never strings assembled from index content except as single
  arguments (case ids, event ids validated by regex before use).

## 4. BarWidget.qml

`WidgetButton` with text `⟡ A · D` where A = active cases, D = open drift
(hidden parts when 0: `⟡`, `⟡ 2`, `⟡ · 3`). Colour: default foreground;
accent when A > 0; theme error colour when any crisis. Tooltip: "Seldon —
2 active cases, 3 unexplained changes, last capture 4 min ago". Left click
toggles Panel; middle click opens Prime Radiant; right click runs capture.

## 5. Panel.qml

`KeyboardPanel` anchored to the pill. Digits select tabs by fixed id
(Today 1, Changelog 2, Work 3, Decisions 4, System 5, Memory 6; a digit
for an absent tab is ignored), `←/→` and `h/l` move between tabs, `↑/↓`
and `j/k` move in lists, `Esc` closes, `Tab`/`Shift-Tab` hand over to the
neighbouring Omarchy panel (never cycle tabs, like every first-party
panel). `n` focuses the QuickEntry from any tab, `e` opens the current
tab's file in the editor, `c` captures (WP-012). Work tab (WP-020): `+`
opens the new-case sheet from any tab; Enter runs a card's first action,
but writing actions (Start, Verify, Done) need Enter twice — the first
press arms and shows a hint, any other key disarms (in a list every key
is navigation; in a form — the sheets — only a change to the form
disarms, so Tab between fields keeps the arm); `x x` drops; a mouse
click on Drop turns into "Confirm drop". The WIP text counts `active`
cases against the bar-widget setting `wipLimit` (default 3; warns, never
blocks). Completed shows the index's last 50 (scrollable). *Start agent*
(WP-022) on an active case's card: key `a` twice or click + Confirm →
`seldon agent start <id> --json`; the card shows "agent: <name>" from the
case's `agents`; refused on queued/verification/closed cases. While a text
field or the sheet has focus the panel blocks the key catcher; `Esc`
hands the keys back and keeps the draft. Drift sheet (WP-021): Enter or
a click on an open drift row, the row's *Resolve…* button, or a click on
the red strip (first crisis) opens the sheet in the Changelog's place;
actions Link (open cases, the proposed case preselected), Explain (intent
+ optional zone/risk/area; result shows the created case with *Open*),
Dismiss (reason); groups offer *All N* / *Only <package>* (`--only`);
writes use two-press arming where any change to the form disarms; the
draft is kept per event; a no-op shows "Already resolved: …"; above the
list "+N more open drift items not listed here" when `summary.openDrift`
exceeds `drift.length` (ADR-0020). Folded rows read `linked to C-…`,
`explained · C-…: <intent>` (ADR-0021), `dismissed: <reason>`. Decisions tab (WP-023, digit 4): newest first by
id; Enter, `e`, double click or *Open* run `open ADR-NNNN --editor --json`
(id validated); `d` opens the new-decision sheet (title → `decide
--no-edit --json -- <title>`, then `open <newId> --editor --json` from the
result; two-press arming; title kept on refusal). Memory tab (digit 6):
lessons headings and memory topics with `updated`; Enter, `e` or *Open*
run `open logbook --editor --json` until the engine gains a memory target
(`open memory[/<topic>]`, queued). Linked cases per decision need a
contract field (`decisions[].cases`) and are deferred to the next contract
bump. Width `Style.space(380)` (WP-011). Files: one component per tab,
`components/TodayTab.qml`, `ChangelogTab.qml`, `WorkTab.qml`,
`DecisionsTab.qml`, `SystemTab.qml`, `MemoryTab.qml`, plus `EventRow.qml`,
`Tabs.qml`, `Banner.qml`, `QuickEntry.qml`, `CaseCard.qml`,
`NewCaseSheet.qml`, `DriftSheet.qml`, `NewDecisionSheet.qml`. The Changelog source filter has
one chip per schema source (all nine, including `manual`, `agent`,
`seldon`). While an event is open drift, its row is coloured by the drift
item's computed zone (ADR-0013 §3) — stripe, glyph, status and badge from
one source; once resolved, by the event's own zone.

| Tab | Content | Actions |
|---|---|---|
| Today | today's journal entries, yesterday collapsed | QuickEntry (`seldon log`), "Open in editor" |
| Changelog | ledger rows newest first, source filter chips, snapshot rows highlighted, drift rows marked | row → link/explain/dismiss sheet; "Capture now" |
| Work | three columns queued/active/completed (last 50, scrollable) | "New case" (title + zone + risk + optional area/priority), start/verify/done/drop with two-press arming, Open in editor on every card; "Start agent" (runs `omarchy agent prompt` or the configured launcher with the case context) is WP-022 |
| Decisions | ADR list with status | "New decision" |
| System | omarchy version, package counts, deviations, snapshots, plugins, theme | "Open in editor" (rebuild/update-impact actions are Phase 3 engine commands, allowed by CONTRACT.md, not wired in v1) |
| Memory | lessons headings, memory topics | "Open" |

Banner states (top of every tab): engine missing → "Install the engine:
`omarchy pkg aur add jax-seldon`" (ADR-0016; `omarchy pkg add` reaches the
official repositories only) with *Copy* and *Open terminal*; contract
mismatch → `omarchy plugin update jax.seldon` when the plugin is older
than the index, `yay -S jax-seldon` when the engine is older; snapshots
not readable (ADR-0011) → the one-line snapper fix with *Copy* and *Open
terminal*; not
initialised → "Run `seldon init`" with *Open terminal*; index stale →
*Capture now*; crisis → red strip "N changes in the red zone need a reason".

## 6. Overlay.qml — Prime Radiant

Fullscreen `Overlay`, opened by `omarchy-shell shell toggle jax.seldon`
(overlay route; check README for the exact route) or middle click.
Layout: 12-column grid, `Style.space` gutters.

- Row 1: title "Prime Radiant", machine name, Omarchy version, period
  selector (30 / 90 / 365 days / All; default 90 d, resets on every open;
  WP-030), close hint.
- Row 2 (full width): **Heatmap** — events per day of the period as ISO
  weeks × 7 days (53 × 7 at 365 d and All), five steps of the theme
  accent; hover shows the date and counts by source.
- Row 3: **Series** explicit and total packages over time (step lines,
  one lane each; a count holds until the next sample; left of the first
  sample in the period no line is drawn but the hover reads out that
  first sample) · **DriftBars** drift opened vs resolved per ISO week
  (the peak is the week with the most opened) · **RiskDonut** cases by
  risk, all time.
- Row 4 (full width): **Timeline** — Omarchy releases, snapshots and
  crisis markers on one band; cases as spans from created to closed
  (open cases run to today), packed in lanes.
- Row 5 (full width): **The Plan** — active cases (`cases.active`) as
  cards with step progress and agent (no period; the sixth slot, WP-031).

Each chart's summary is its caption in the slot's title row (also its
accessible description and in `call view`); while the pointer is on the
chart the caption shows the hovered item. A chart without data in the
period says "no data in this period". Chart data is prepared by the
service (`Model.periodTable`) when the index changes; the overlay only
draws (one paint per chart per data or size change; no aggregation on
the first frame after open — the harness asserts it). The shell creates
the overlay item first and injects `service` afterwards (its Loader's
`onLoaded`: `if ("service" in item) item.service = …`), so every binding
first runs with `service === null` and must tolerate that without work
(`Model.periodView` then returns a kept empty view, no aggregation pass);
the harness creates the overlay in the same order.

The Heatmap is a square grid bound by the slot's height, left-aligned
with its legend beside it; at 30 d and 90 d it fills only the left part
of a full-width slot (WP-037).

Period windows follow ADR-0012 §10: inclusive day windows ending on
`today`; drift weeks and case spans count when they overlap the window;
`series.risk` is all-time and the donut says so.

All charts are drawn with `Canvas` or `Shape` from arrays prepared by
`Model.js`; no external QML modules.

## 7. Theming

Every colour from `Style` / the bar's palette; charts use `accent`,
`foreground` at opacities, `Color.urgent` for crises and R3. Font from the bar. Test with
at least three Omarchy themes incl. a light one.

## 8. Keybinding and IPC

Suggested user binding (documented, not installed): `o.bind("SUPER + SHIFT
+ S", "Seldon", "omarchy-shell shell toggle jax.seldon")`.

How the shell routes (verified against `shell.qml`, Omarchy 4.0.4; see
memory/omarchy-shell.md): because `kinds` includes `overlay`, the plugin is
*not* a bar-widget-panel plugin. `shell summon|hide|toggle jax.seldon`
therefore reaches **Overlay.qml** (the Prime Radiant), never the bar
widget; `shell call jax.seldon <method>` reaches only the loaded overlay
item and only while it is loaded. Routes the plugin must honour:

- Overlay entry point: `open(payloadJson)`, `close()`, `opened` — this is
  what the keybinding above hits. While loaded, `shell call jax.seldon
  view ""` reads the slots (aggregation counters, each chart's summary and
  hover), `setPeriod <30|90|365|all>` switches the period, and
  `hover "<slot> <fx>,<fy>"` (fractions in [0, 1]; `""` clears; anything
  else returns `{ error }` and changes nothing) drives the hover read-out
  for tests (WP-030/031).
- Bar panel: `IpcHandler` target **`jax.seldon.panel`** owned by the bar
  widget (`open`, `close`, `show`, `hide`, `toggle`, `pill`, and the
  read-out/navigation methods `view`, `tab <today|changelog|work|decisions|system|memory>`, `filter <source>`,
  `resolve crisis|<eventId>` (opens the drift sheet, WP-021) — none runs
  the engine; WP-011), following the
  first-party `Panel { ipcTarget }` pattern, so `qs ipc` can open, close
  and toggle the panel independently of the overlay (WP-010).
- Service: `IpcHandler` target **`jax.seldon.service`** (`status`,
  `refresh`, `capture`) — read-only state and the two actions any local
  process could trigger anyway; it is how the test host reads plugin state
  headlessly.
- Quirk: `shell togglePanelAt <section> <n>` on the Seldon pill opens the
  overlay, not the panel, because the shell routes by plugin id. Use the
  panel target above.

## 9. Validation

`omarchy plugin validate plugin/` and `just qmllint` on every commit.
`just qmllint` builds a temporary `qs/` import root from the installed shell
(a plain `-I "$OMARCHY_PATH/shell"` cannot resolve `qs.*` imports) and
requires **zero warnings** with `--max-warnings 0`, with exactly two
categories demoted to info because first-party plugins cannot avoid them
either: `missing-property` (nested `Style`/`Color` tokens) and
`uncreatable-type` (`PanelWindow`). Consequence: the lint is blind to typos
in token names, so the runtime smoke test below is a **hard acceptance
test** of every plugin WP, not an option. The smoke test launches the shell
with the fixture index (`SELDON_INDEX` env override is honoured by
Service.qml for development only) and checks that each tab and the overlay
render without QML errors (`qs log --tail`). With `SELDON_INDEX` set the
plugin is in read-only dev mode (CONTRACT.md rule 1): it never runs a
writing engine command, and `SELDON_NOW` pins its clock. The headless
harness under `tests/plugin/` (`just plugin-test`) runs Service.qml in an
offscreen Quickshell against fake engines and checks the exact argv of
every command the plugin issues; it is part of `just check`. After copying
changed plugin code into a live shell, run `omarchy-restart-shell` —
Quickshell 0.3.1 does not reload plugin code on file change.

## 10. Security posture (for the marketplace listing)

No network. No bundled binaries, units or installers. Reads one JSON file.
Runs the `seldon` engine with fixed arguments (the forms in CONTRACT.md).
Besides the engine it starts only `wl-copy` and Omarchy's floating-terminal
launcher, each with one constant command, only on a banner click. Never a
shell string built from logbook content. Documented in README under
"Security, privacy, privileges" (WP-041).
