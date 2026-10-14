# APP-PARITY.md — From the desk to the app: what 0.5.0 must show

Proposed with [ADR-0056](../decisions/ADR-0056-seldon-is-an-omarchy-app.md)
§5 (E96 delegation; the operator accepts). 0.5.0 removes the desk only
when every row below is **app** (an acceptance test of the app, offscreen
against `fixtures/index.sample.json` or the fake engine) or **drop** (a
recorded reason). A row that is neither blocks the tag. The source is
`tests/plugin/desk-view.sh` as of 0.4.0, scenario by scenario; the pill's
harnesses (`bar-view.sh`, `service-states.sh`, `ipc-restart.sh`) stay
with the plugin and are not listed.

The views (ADR-0056 §3): **Changes** (default; absorbs Today and the
Changelog), **Plan** (Work), **Record** (Decisions, System, Memory, the
journal), **Radiant** (Prime Radiant and the graph). "States" means the
app's required states (engine missing, logbook not initialised, index
missing, index stale) and its notices.

## The daily acts (the gate)

Each needs an app test that runs the act against the fake engine and
checks the exact argv sent:

1. Resolve a change: link, explain, dismiss (Changes).
2. Start, verify and complete a case; a new case from one sentence (Plan).
3. Write a note into today's journal (Changes and Record).
4. Accept a decision (Record).
5. The Prime Radiant with its periods (Radiant).
6. The graph (Radiant).

## Scenario by scenario

| # | Desk scenario | Fate | App acceptance or reason |
|---|---|---|---|
| 1 | Width 50–100 % of the screen | drop | The window's size is the compositor's; replaced by "the layout holds at 1366, 1920, 2560 and 3840 px wide windows" |
| 2 | Thresholds: sidebar icons under 960 px, stacking under 760 px | app | A narrow window reflows: views as icons, list and detail stacked below the app's threshold |
| 3 | Keys 1–8 and `,` reach nine targets; Alt+↓/↑; `/` search | app | `1`–`4` reach the four views; `/` searches; `?` lists every shortcut; `Q` quits; Space runs the selected row's main action (omarchy-app keyboard rules) |
| 3b | Data states: a search without a match | app | Same text and *Clear search* in Changes, Plan, Record |
| 3c | Empty lists offer their first step | app | Plan's empty list offers *New case*; Record's offers *New decision* |
| 4 | Settings › Appearance (width slider, sidebar) | drop | No window geometry to set; what is in force stays in `seldon doctor` and `config.toml`. Proposed drop — the app WP may add a settings view if a setting survives |
| 5 | Open and close as the shell and the pill do; the panel shim | app / drop | App: `seldon open` starts the window, a second one raises it, the pill's left click runs it, `Q` quits. Drop: `shell toggle|summon`, middle click to the Radiant, the `jax.seldon.panel` shim (removed in 0.5.0, ADR-0056 §5) |
| 6 | Stacked layout: Enter shows the detail, Esc goes back | app | Same in a narrow window; Esc goes back, never quits |
| 7 | Notices under the header; the header's chip | app | States |
| 7a | Snapshots not readable: the setup card's optional step, *Grant* | app | States: the setup card stands in Changes until Seldon is set up |
| 7b | Not initialised: the setup card, *Create logbook* | app | States: the setup card |
| 7c | Plugin updated under a running shell: restart notice | drop (app) | The app needs no shell restart. The pill keeps its own notice in `service-states.sh` |
| 7d | Rules notice: *Update rules* runs `rules update --json` once | app | States |
| 7e | Capture warnings; `c` captures again | app | States |
| 7f | Setup card end to end (no engine, no logbook) | app | States: the app is started by the engine, so "engine missing" means the app was started directly; the fix names the package |
| 7g | The grant's own capture did not run | app | States |
| 7h | Engine older than `engineMin` before init | app | States, with the app's own `engineMin` |
| 7i | No snapper on this machine: no snapshot step, no card | app | States |
| 7j | First-run card from its fixture | app | States |
| 7k | Logbook folder in use (exit 3's reason) | app | States |
| 8a | Today: date, tiles, NEEDS YOU, journal, active cases, *New case* | app | Changes opens on NEEDS YOU and the note field; active cases and *New case* in Plan |
| 8b | Changelog: chips and counts, rows by class, event detail | app | Changes |
| 8c | Quiet surfaces; inline link, explain, dismiss forms (arm, run) | app | Changes, daily act 1 |
| 8d | Work: groups, WIP line, case detail, By agent, reopen, new case | app | Plan, daily act 2 |
| 8e | A view change gives the keys back; drafts survive; selection follows across an index update | app | All views |
| 8f | The "why loud" callout from `seldon drift show` | app | Changes |
| 8g | The desk steps aside for what it opens; one agent per case | app / drop | App: one agent per case (`agent start`, `agent focus`, `--again`) in Plan. Drop: stepping aside — a window does not cover what it opens |
| 9 | Prime Radiant | app | Radiant, daily act 5 |
| 9a | Periods 30 d, 90 d, 365 d, all; the `view` IPC | app / drop | App: periods. Drop: the shell IPC (no shell surface) |
| 9b | First frame runs no aggregation | app | Radiant: the window's first frame shows before the charts aggregate |
| 9c | Hover read-outs on each chart | app | Radiant |
| 9d | Layout at 2560×1440 | app | Radiant |
| 9e | Narrow reflow of the grid | app | Radiant |
| 9f | Not initialised | app | States |
| 9g | Every index variant renders every chart | app | Radiant, against every fixture variant |
| 10a | Decisions: list, proposed one striped, Accept, Open in editor | app | Record, daily act 4; *Open in editor* runs `seldon edit … --editor --json` (ADR-0058) |
| 10b | A decision that names cases | app | Record |
| 10c | New decision from the form | app | Record |
| 10d | Refusals keep the title (blank title; lock held, exit 4) | app | Record and Plan forms |
| 10e | System: the dossier's tiles and detail | app | Record |
| 10f | Memory: lessons and topics | app | Record |
| 10g | Not initialised | app | States |
| 10h | Stacked layout for sections 4–6 | app | Record in a narrow window |
| 10i | Label fit at 960 px and 3840 px | app | Every view in `en` and `de` (ADR-0057): no text leaves its box |
| 11 | The graph: settles and sleeps | app | Radiant, daily act 6 |
| 11a | Settle and sleep: at most 200 ticks | app | Radiant |
| 11b | Nothing while hidden | app | Radiant: no tick while another view shows or the window is hidden |
| 11c | Replay from day 0 | app | Radiant |
| 11d | Hover lights a case and shows its card | app | Radiant |
| 11e | Drag a node | app | Radiant |
| 11f | A busy index (500 events, 66 cases) | app | Radiant |
| 11g | No index; nothing to draw | app | States and Radiant |
| 11h | Narrow windows | app | Radiant |
| 11i | More fixed nodes than the cap | app | Radiant |

## New in the app (no desk scenario)

From the omarchy-app skill and the conformance audit; each is an app
acceptance in 0.5.0:

- `?` overlay with every shortcut; `Q` quits; Space is the main action.
- Theme live (accent and Hype's fuller palette, re-armed watcher, `#FFD60A`
  fallback); `theme.radius` on every panel, dialog and button.
- Language live (`en`, `de`), right-to-left mirroring checked with a
  temp language file set to `ar`.
- *Import tasks* picks its file through the xdg-desktop-portal
  FileChooser (Omacut's pattern), never a typed path.
- `.desktop` with `StartupWMClass=seldon` and `Name[de]`; the hicolor
  icon; the window's app id is `seldon`.
- No QML warning with `QT_FORCE_STDERR_LOGGING=1`; the README screenshot
  from the offscreen harness with the default accent and fixture data.
