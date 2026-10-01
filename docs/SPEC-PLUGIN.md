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

Verify the exact manifest keys against `~/.local/share/omarchy/shell/README.md`
before committing (WP-010 does this); the shell is the source of truth.

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
  to `state: engineMissing`. States: `ok | engineMissing | notInitialised |
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

`KeyboardPanel` anchored to the pill. Tabs `1`–`6`, `←/→`, `Esc`,
`Tab` switches Omarchy panels. Width `Style.space(360)`.

| Tab | Content | Actions |
|---|---|---|
| Today | today's journal entries, yesterday collapsed | QuickEntry (`seldon log`), "Open in editor" |
| Changelog | ledger rows newest first, source filter chips, snapshot rows highlighted, drift rows marked | row → link/explain/dismiss sheet; "Capture now" |
| Work | three columns queued/active/completed (last 5) | "New case" (title + zone + risk), start/verify/done, "Start agent" (runs `omarchy agent prompt` or the configured launcher with the case context) |
| Decisions | ADR list with status | "New decision" |
| System | omarchy version, package counts, deviations, snapshots, plugins, theme | "Rebuild doc", "Update impact" |
| Memory | lessons headings, memory topics | "Open" |

Banner states (top of every tab): engine missing → "Install the engine:
`omarchy pkg add jax-seldon`" with *Copy* and *Open terminal*; not
initialised → "Run `seldon init`" with *Open terminal*; index stale →
*Capture now*; crisis → red strip "N changes in the red zone need a reason".

## 6. Overlay.qml — Prime Radiant

Fullscreen `Overlay`, opened by `omarchy-shell shell toggle jax.seldon`
(overlay route; check README for the exact route) or middle click.
Layout: 12-column grid, `Style.space` gutters.

- Row 1: title "Prime Radiant", machine name, Omarchy version, period
  selector (30 / 90 / 365 days), close hint.
- Row 2 (full width): **Heatmap** — events per day, 53 × 7, theme accent
  ramp; hover shows date, counts by source.
- Row 3: **Series** packages explicit over time (step line) · **DriftBars**
  drift opened vs resolved per week · **RiskDonut** cases by risk.
- Row 4: **Timeline** — Omarchy releases, snapshots, cases as spans
  (queued→completed), crisis markers.
- Row 5: **The Plan** — active cases with step progress and agent.

All charts are drawn with `Canvas` or `Shape` from arrays prepared by
`Model.js`; no external QML modules.

## 7. Theming

Every colour from `Style` / the bar's palette; charts use `accent`,
`foreground` at opacities, `error` for crisis. Font from the bar. Test with
at least three Omarchy themes incl. a light one.

## 8. Keybinding and IPC

Suggested user binding (documented, not installed): `o.bind("SUPER + SHIFT
+ S", "Seldon", "omarchy-shell shell toggle jax.seldon")`. IPC routes the
plugin must honour: `open`, `close`, `toggle` on the bar widget; the overlay
exposes the same through its entry point.

## 9. Validation

`omarchy plugin validate plugin/` and `qmllint -I "$OMARCHY_PATH/shell"
plugin/*.qml plugin/components/*.qml` on every commit. A smoke test
launches the shell with the fixture index (`SELDON_INDEX` env override is
honoured by Service.qml for development only) and checks that each tab and
the overlay render without QML errors (`qs log --tail`).

## 10. Security posture (for the marketplace listing)

No network. No bundled binaries, units or installers. Reads one JSON file.
Runs the `seldon` engine with fixed arguments. Opens a terminal only on
explicit click. Documented in README under "Security, privacy, privileges".
