# ADR-0034 — The plugin is a desk: one wide panel with sidebar, list and detail; Prime Radiant and the graph live inside it

**Status:** accepted (operator decision 2026-10-06, after three rounds on the
clickable prototype: "Let the Seldon prototype become reality; you have
absolutely free rein from now on, plus the test host"; later the same
evening, "all as recommended": no separate 0.1.5 — the guided setup
(WP-119), Ask agent and bulk triage (WP-095) and the import of Markdown
task files as cases (WP-102) are built inside the desk; the contract v2
bundle, WP-113 and WP-114 ship in the same release). Ships in **0.2.0**
on the integration branch `next` (merged into `main` after 0.1.4 is tagged).
**Date:** 2026-10-06

> Supersedes SPEC-PLUGIN §5 (the tabbed bar panel) and §6 (the fullscreen
> Prime Radiant overlay) as surfaces; amends §1 (manifest), §7 (theming)
> and §8 (IPC). Builds on ADR-0027 (act, then account), ADR-0028 §3
> (crises stay with the user), ADR-0029 (the planned link) and ADR-0030
> (the agent starts like Omarchy's). The two index fields the desk needs
> ride in the contract v2 bundle (ADR-0035, drafted with this ADR). The
> engine's logbook layout does not change. The prototype
> `refs/ui-2026-10-06/prototype-desk-v6.html` (private) is the spec of look
> and behaviour; this ADR says how it becomes Quickshell.

Tagline (approved): **"Seldon is the memory of your Omarchy machine — for
you and your agents."**

## Context

Today the plugin has two surfaces: a ~450 px `KeyboardPanel` popup under
the bar pill with six tabs (Today, Changelog, Work, Decisions, System,
Memory) and a drift sheet, and a separate fullscreen overlay, the Prime
Radiant (six charts). The operator, after the first productive setup
(2026-10-06): "far too compact for the amount of information. Seldon may
take half the screen width, at least"; likes LAGEBILD's sidebar
navigation ("Outlook and an admin dashboard"); Prime Radiant "could live
inside the new UI and grow there". Three prototype rounds added: width as
a user setting from 50 % up, default full; sticky action bars at the top
of every detail; a Graph section (Obsidian-like, grows over time, replay);
"Agent sorts N open changes" in the Changelog.

Facts checked (read-only, `$OMARCHY_PATH/shell`, Omarchy 4.0.x on the dev
host; LAGEBILD `ui/Panel.qml`; our `plugin/`):

- The shell mounts one `Loader` per plugin that declares `panel`,
  `overlay` or `menu` (`shell.qml`, "on-demand panels"); it picks one UI
  kind per id (`panel` before `overlay` before `menu`), creates the entry
  item **without properties**, then injects `shell`, `manifest`,
  `pluginRegistry`, `barWidgetRegistry` and `service` (`onLoaded`).
  `summon <id> <payload>` calls `item.open(payload)`, `hide` calls
  `close()`; `shell call <id> <method> <arg>` reaches the loaded item.
  Neither kind sizes anything: **the plugin owns its window.** Overlays use
  a layer-shell `PanelWindow` (menu, emojis, image-picker, our
  `OverlayWindow.qml`); LAGEBILD (kind `panel`) uses a `FloatingWindow`
  980 × 820 that Hyprland tiles unless a window rule floats it.
- Settings live inline on the plugin's `shell.json` entry (README,
  "Storage rules" 3). The shell hands a bar widget its entry as
  `settings`; Omarchy's bar-settings panel renders `barWidget.schema`
  (`integer` with `min`/`max`/`step`, `enum`, `string`, `path`). Our pill
  already pushes `captureIntervalMin` and `driftInBar` into the service
  (`BarWidget.pushSettings`). The third-party facade offers a settings
  write (`_updateSettings` → `shell.updateEntryInline`), which **replaces
  the whole entry** with `{id} + settings`. `~/.config/omarchy/shell.json`
  is a watched path: every write is a `config-change` event (routine since
  ADR-0028, but an event).
- Multi-monitor: the bar runs one window per screen (`Variants` over
  `Quickshell.screens`) and resolves the active one with
  `Hyprland.focusedMonitor` (`Bar.qml focusedScreenName`). Overlays are
  one window.
- Theme: `Color.popups.{background,text,border}`, `Color.accent`,
  `Color.urgent`, `Color.foreground`, `Style.font.*`, `Style.space()`,
  `Style.spacing.*`, the state tokens; the prototype's theme switch
  (Tokyo Night, Kanagawa, Latte) is exactly this binding.
- The index (contract 1) has what the graph needs except one link:
  `events[].case`, `drift[].proposedCase` and `drift[].crisis`,
  `cases.*[].area|created|closed|events`, `system.areas[]`,
  `decisions[]` with `id, title, status, date, path` — but **not** the
  decision's `cases`, which the logbook frontmatter carries
  (`engine/src/model/decision.rs: cases`). The index is a view: newest
  500 events, last 50 completed cases (CONTRACT rule 4).
- The harness: `panel-view.sh` (1493 lines, ~30 scenarios),
  `overlay-view.sh` (432), `bar-view.sh`, `service-states.sh`,
  `model.test.js` (1792). The panel and overlay harnesses mount the real
  shell `Commons/` and `Ui/`, replace only the layer-shell window with a
  stand-in Item, and drive real key events.

## Decision

### 1. One surface, the desk; Omarchy's overlay mechanism

The desk replaces both `Panel.qml` (the popup) and `Overlay.qml`. The
manifest keeps `kinds: ["service", "bar-widget", "overlay"]`; the
`overlay` entry point becomes `Desk.qml`. Not `panel`: the loader would
treat it the same, SPEC-PLUGIN §1's warning stands, and the operator's
and the docs' binding `omarchy-shell shell toggle jax.seldon` keeps
opening the one Seldon surface. Overlay semantics are the right ones: it
covers a large part of the screen, takes exclusive keyboard focus while
open, Esc closes.

Window: a layer-shell `PanelWindow` (`components/desk/DeskWindow.qml`,
replaceable by the harness as today), `WlrLayer.Overlay`,
`WlrKeyboardFocus.Exclusive` while open, anchored **top and bottom**,
`ExclusionMode.Normal` so the bar stays visible above it (the prototype
shows the bar), margins `Style.gapsOut`. Horizontal size:

```
width = clamp(round(screen.width × deskWidth / 100), min(960, screen.width), screen.width)
```

At 100 % the window is anchored left and right; below that it is centred
(no left/right anchor; Quickshell centres an unanchored axis). Outside
the desk a transparent click-catcher closes it; no dimming — the desk
reads as an application, not a modal. **Screen:** the focused monitor at
`open()` (`Hyprland.focusedMonitor.name` matched against
`Quickshell.screens`; fallback: the first screen), one window, re-targeted
on every open, never one per screen.

Settings, stored like `driftInBar` in `barWidget.defaults` and `schema`:

| key | type | default | meaning |
|---|---|---|---|
| `deskWidth` | integer 50–100, step 1 | 100 | per cent of the screen's width |
| `deskSidebar` | enum `open` / `collapsed` | `open` | the sidebar's state |

The desk's **Settings › Appearance** shows the slider in 10 % steps with
the presets 50 / 67 / 75 / Full and a preview line ("1728 px on this
screen"); it writes through the facade's settings update, **all** of the
entry's keys (the write replaces the entry), only on slider release or
preset click — every write is a config event. Omarchy's bar settings show
the same keys; both paths are valid. Responsive rules: desk width < 960 px
(only on screens narrower than 960) — the sidebar is icons-only and the
list and the detail stack like LAGEBILD's single column, with a "back"
row; otherwise the user's `deskSidebar` decides.

### 2. Information architecture

Header (one row): brand mark and "SELDON", machine · Omarchy version ·
"captured N min ago" (today's banner states move here as a status chip
and keep their one-click fixes), the **KPI strip** (active · verification
· queued · crises in urgent · attention), gear (`,`), Esc.

Sidebar sections, fixed digits, counts at the right:

| # | Section | List | Detail | From today's |
|---|---|---|---|---|
| 1 | Today | tiles (events today, 7 days); "Needs you" = crises; journal note field + entries | overview with active-case tiles and **New case** (one sentence → `agent start --new`); an event when selected; while Seldon is not recording, the **setup card** of WP-119 (engine → logbook via `seldon init --defaults`, ADR-0033 → snapshots; zero questions) takes the detail pane, and the first-run card ("Seldon is recording … nothing to do") follows it | TodayTab, QuickEntry, the crisis strip, WP-101's intent field, the banner fixes (WP-119's card replaces the three setup banners) |
| 2 | Changelog | **Agent sorts N open changes** (§6), chips open · crisis · attention · routine · in case · all, rows with stripe/age | event detail: sticky bar (Ask agent / Link to case… / Explain… / Dismiss / Open case), "why loud" callout, when/who/what/case/rule/source | ChangelogTab, EventRow, DriftSheet (its link/explain/dismiss forms open inline in the detail) |
| 3 | Work | intent field + Run; **Import tasks…** (WP-102: `seldon import task` reads the user's Markdown task files, dry run first, then one click; imported cases carry the `imported` tag of CONTRACT rule 8); groups Active · Verification · Queued · Completed with progress | case detail: sticky bar by status (Hand to agent / To verification / Complete / Back to active / Start / Reopen / Open in editor; id · risk right), Intent, Plan, Log, linked changes, Result; an imported case names its source file | WorkTab, CaseCard, NewCaseSheet (kept for the manual path) |
| 4 | Decisions | rows (proposed striped) + New decision | body, Accept when proposed, Open in editor; linked cases (v2) | DecisionsTab, NewDecisionSheet |
| 5 | System | Omarchy · Packages · Snapshots · Deviations · Collectors | big value, lead, key/value rows, "from the dossier; rebuilt on every capture" | SystemTab |
| 6 | Memory | lessons and topics | text; "every agent reads this at the start of a session" | MemoryTab |
| 7 | Prime Radiant | solo (full width) — §4 | | Overlay.qml |
| 8 | Graph | solo — §5 | | new |
| , | Settings | Appearance · Capture · Agents · Quiet | Appearance is live; the others show the values and name where they are set (bar settings, `config.toml`) | new |

Keyboard (SPEC-PLUGIN §5 is rewritten to this): `1`–`8` sections, `,`
settings, `Alt+↑/↓` previous/next section, `/` the sidebar search (filters
the current list; Esc leaves it), `↑/↓` `j/k` move in the list, `Enter`
selects, `Esc` leaves search → in the stacked layout back to the list →
closes the desk. Letters stay as today where they still mean something:
`c` capture, `n` note (Today), `+` new case, `i` intent field, `e` open in
editor, `a` hand to agent, `r` reopen, `f`/`F` filter, `x` drop, `d` new
decision. Writing actions keep the arm-twice rule; the sticky bar shows
the arm hint. Tab/Shift-Tab no longer hand over to the bar (the desk is
not a bar popup). UI labels are English (AGENTS.md §2); the prototype's
German is the logbook's language, not the UI's.

Lists are `ListView`s with delegates from `Model.js` row functions (as
today's `changelogRows`, `workColumns`, `decisionRows`), the detail a
`Flickable` with the action bar outside it (sticky by construction). The
cursor stays on its item across index updates, as today.

### 3. Data and threading

Unchanged: `Service.qml` owns the index (FileView), the engine queue
(fixed argv from `Model.js`, `validateArgs`), the capture timer, the
settings push. Everything a section renders is prepared in the service
when the index changes (the `Model.periodTable` pattern) and looked up by
the sections; no section aggregates on paint. The loader creates the item
before injecting `service`: every binding tolerates `service === null`
(as the overlay does today, asserted by the harness).

### 4. Prime Radiant inside the desk

Section 7, solo. Reused as they are: the six charts
(`components/overlay/Heatmap|Series|DriftBars|RiskDonut|Timeline|ThePlan`),
`OverlaySlot`, `ChartCanvas`, `Model.periodTable|periodView|overlayGrid`,
the period chips 30 / 90 / 365 d / All (default 90 d on every entry of
the section). Dropped: `OverlayWindow`, `OverlayHeader`, the scrim, the
overlay's own banner (the header chip covers it). IPC: `shell call
jax.seldon view ""` reports the whole desk (section, selection, counts,
and the slots when section 7 is shown); `setPeriod` and `hover` keep
their names and act on section 7. Payload: `{"section":"radiant",
"period":"30"}`; the old `{"period":"30"}` implies `radiant`, so an
existing binding still lands on the charts. Middle click on the pill
opens the desk at section 7.

### 5. The graph

Data: `index.json` only. Nodes: areas (`system.areas`), cases (all four
lists), decisions, and events whose kind is a change (`install`,
`remove`, `upgrade`, `downgrade`, `reinstall`, `snapshot`,
`snapshot-delete`, `update`, `plugin-*`, `theme-set`, `config-*`,
`command`); not case lifecycle, notes or resolutions. A node is a crisis
when its event id is in `drift[]` with `crisis: true`. Edges:
`event.case → case`; `case.area → area`; `drift.proposedCase` dashed;
`decision.cases → case` (**new in contract v2**: `decisions[].cases`,
the frontmatter list the engine already parses). Day index: event `ts`,
case `created`, decision `date`, area = the earliest node attached to
it. Replay: a date slider sets the cut-off day; Play advances it. Hover
card: label, kind, since day N, M links, "Open case" (goes to Work with
the case selected). Known limit, stated in the graph's footer: the index
holds the newest 500 events and 50 completed cases; a `seldon graph
--json` export over the whole logbook is **deferred** — not 0.2.0.

Performance budget (asserted by the harness on the fixture and measured
once on the test host): `Canvas` drawing; the force layout is pure JS in
`Model.js` (`graphBuild`, `graphStep`) driven by a `Timer` at ≤ 30 Hz on
the shell thread with **≤ 8 ms per tick** (`view` reports `tickMs`); the
simulation runs at most 200 ticks (alpha decay) and then sleeps; drag,
pan, zoom, hover and replay wake it; nothing runs while the section is
hidden or the desk closed. **Node cap 400**: beyond it, events of one day
and source fold into one cluster node ("+N", hover lists them), areas,
cases and decisions never fold. Escalation if the budget fails on the
test host: a `WorkerScript` for the layout, or a static layout by day and
area — decided then, not now.

### 6. Bulk triage: "Agent sorts N open changes"

In the Changelog's list head when N > 0 (N = open attention + crises)
and a default agent exists. One click → `seldon agent ask triage --json`
(fixed argv, no text): the engine launches the default agent on the
WP-101/ADR-0030 path (`SELDON_ACTOR`, `SELDON_ATTENDED=1`, id-only
prompt that names the skill's `triage.md`). The agent reads the ledger,
cases and journal through `seldon … --json` and writes a **proposal**:
`seldon drift propose --json` with JSON on stdin — items `{eventId,
action: "link"|"explain", caseId | title+intent, evidence: [refs]}` where
every ref is a journal time, event id, snapshot number or case id the
engine can resolve; an item without a resolvable ref is refused, so
unprovable changes stay quiet. The engine validates ids, stores the
proposal under `~/.local/state/seldon/proposals/<ulid>.json`, and the
index points at it (**contract v2** object `triage: {id, at, actor,
counts, path}`; the plugin reads only what the index points to, AGENTS.md
§3). The desk shows it as a detail: sticky bar **Apply proposals /
Discard** and the line "agent · proposal, nothing written yet"; items
with their evidence; crises listed in their own block with one click
each, never inside Apply (ADR-0028 §3). Apply → `seldon drift apply
<proposalId> [--item <eventId>]… --json`: the user's click is the actor
(`human`), each resolution carries `resolutionDetail` "proposed by
agent:<name> — <evidence>", crises only when named by `--item`,
idempotent (an item already resolved is skipped and reported), the
proposal file is marked applied. Relation to WP-095: the `agent ask`
family is one design — `ask triage`, `ask drift <eventId>`, `ask case
<caseId>` — and the ADR WP-095 asked for ("agent prompts carry
identifiers, never logbook text") is written as ADR-0036 in the triage
WP; WP-095 is superseded by it, its single-row "Ask agent" being the
primary action of a crisis or attention detail in §2.

### 7. The bar pill, the old surfaces, migration

The pill is unchanged in look and in `driftInBar`. Left click toggles the
desk, middle click opens it at Prime Radiant, right click as today. The
`jax.seldon.panel` IPC target stays for one minor release as a shim that
forwards `open|close|show|hide|toggle|pill|view` to the desk and maps
`tab <name>` to `section <name>`; `resolve <eventId>` opens the event's
detail; removal in 0.3.0 is announced in the CHANGELOG. `jax.seldon
.service` is unchanged. The popup and the fullscreen overlay are removed,
not kept behind a setting. A 0.1.x user needs no action: the new keys
take their defaults (`deskWidth` 100, `deskSidebar` open), existing keys
keep their values, `omarchy plugin update` shows the diff, the restart
notice of WP-090 appears once.

### 8. Risks and how this ADR handles them

- *QML performance on the shell thread*: data prepared in the service;
  `ListView` delegates, not `Repeater`s, for lists; graph budget and cap
  (§5); the harness measures `tickMs` and first-paint; theme sweep on the
  test host with the real fixture-sized logbook.
- *Harness rewrite*: `panel-view.sh` and `overlay-view.sh` become one
  `desk-view.sh` with `harness/desk.qml`; the step vocabulary stays
  (`key`, `text`, `type`, `click`, `hover`, `shot`, `wait`, `settle`,
  `view`) plus `section:<id>`, `select:<id>`, `width:<pct>`,
  `resize:<W>x<H>`. Coverage gate: every scenario of the two old scripts
  has a named successor in `desk-view.sh` or a written reason in the
  handover; `model.test.js` keeps its row-function tests.
- *Exclusive keyboard focus*: a focused text field must release Esc and
  `/`; the harness types into fields with real keys as today.
- *Settings writes*: only on explicit user action; the write carries all
  keys; a refused facade call falls back to "Change it in Omarchy's bar
  settings" without an error state.
- *Width edge cases*: fractional scaling and screens narrower than 960 px
  go through the clamp and the stacked layout; tested at 1366, 1920, 2560
  and 3840 px in the harness.

## Consequences

- 0.2.0 is a breaking minor (VERSIONING.md: before 1.0 the minor is the
  breaking slot): the popup, its keyboard map and the overlay route change;
  CHANGELOG **Breaking**; `engineMin` 0.2.0 (triage commands, v2 fields).
- Contract v2 (ADR-0035) gains `decisions[].cases` and `triage` beside
  the bundle already decided (case risk in the ledger, autocommit result,
  `meta.truncated`, state-loss event kind).
- SPEC-PLUGIN §1, §2, §5, §6, §7, §8 and KEYBINDINGS.md are rewritten;
  guides 02–05 en/de and the plugin README follow; `preview.png` is the
  desk.
- `plugin/`: `Desk.qml`; `components/desk/` (DeskWindow, Header,
  KpiStrip, Sidebar, ListColumn, DetailPane, ActionBar, Search);
  `sections/` (Today, Changelog, Work, Decisions, System, Memory,
  Radiant, Graph, Settings); `components/graph/GraphCanvas.qml`;
  `Model.js` grows (section rows, graph build and step, triage view).
- The plugin writes `shell.json` for the first time (its own entry only).
- No 0.1.5: WP-119 (setup card, `init --defaults`, 90-day backfill),
  WP-095 (Ask agent, triage) and WP-102 (import) are desk work in 0.2.0;
  WP-095 is superseded by WP-124; WP-119 and WP-102 keep their numbers,
  retargeted to `next` and to the desk (PLAN-0.2.0). WP-113 (plugin tree and toggles hashes, `authorized_keys`
  opt-in) and WP-114 (`/etc/pacman.conf`, `/etc/pacman.d/*.conf`, hashes
  only; AGENTS.md §6 amended by the operator) are engine work in the same
  release and show up in the desk only as more routine history and, when
  they hit, as attention or crisis rows under the existing rules.

## What would change the how (not the decision)

- Quickshell cannot centre an unanchored layer-shell axis on Hyprland →
  anchor left with a computed margin.
- The facade refuses the settings write for an overlay-kind caller → the
  desk's slider becomes a preview plus the sentence "Set the width in
  Omarchy's bar settings"; the key stays in the manifest schema.
- The 400-node layout misses 8 ms per tick on the test host → §5's
  escalation.

## Assumptions

Omarchy 4.0.x shell as installed on the dev host and the fresh test host
of 2026-10-07; Quickshell 0.3.x (`Canvas`, `Timer`, layer shell,
`Quickshell.Hyprland`); the facade's public settings-update method exists
for third-party callers (the worker verifies the exact name in
`shell.qml` before relying on it); the contract v2 bundle lands first in
0.2.0 (ADR-0035); the test host may be moved to `next` builds for live
checks (test-host authorisation).
