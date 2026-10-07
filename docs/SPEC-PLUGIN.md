# SPEC-PLUGIN.md — `jax.seldon`, the Quattro plugin

Normative. Lives in `plugin/`, installed to `~/.config/omarchy/plugins/jax.seldon`.

## 1. Manifest

```json
{
  "schemaVersion": 1,
  "id": "jax.seldon",
  "name": "JAX Seldon",
  "version": "0.1.3",
  "author": "JohnAndrewsX",
  "license": "MIT",
  "description": "The memory of your Omarchy machine, for you and your agents: ledger, journal, cases, drift and the Prime Radiant in one desk.",
  "kinds": ["service", "bar-widget", "overlay"],
  "entryPoints": { "service": "Service.qml", "barWidget": "BarWidget.qml", "overlay": "Desk.qml" },
  "barWidget": {
    "displayName": "Seldon", "category": "System", "allowMultiple": false, "defaultSection": "right",
    "defaults": { "captureIntervalMin": 15, "wipLimit": 3, "driftInBar": "crisis", "deskWidth": 100, "deskSidebar": "open" },
    "schema": [ "… one entry per key of defaults (plugin/manifest.json)" ]
  },
  "seldon": { "contractVersion": 2, "engineMin": "0.1.0" }
}
```

Verify the exact manifest keys against `$OMARCHY_PATH/shell/README.md`
(`/usr/share/omarchy` on a package install) before committing (WP-010 does
this); the shell is the source of truth. Do **not** add `panel` to `kinds`:
the shell's panel loader picks one UI kind per plugin id (`panel` before
`overlay` before `menu`), and `overlay` is what makes `omarchy-shell shell
toggle jax.seldon` open the desk (§5, §8; ADR-0034 §1).

The settings live inline on the plugin's `shell.json` entry (the shell's
storage rule 3) and appear in Omarchy's bar settings from `barWidget.schema`:
`captureIntervalMin` (§3), `wipLimit` (§5.7), `driftInBar` (§4),
`deskWidth` (integer 50–100, step 1, default 100: the desk's width in per
cent of the screen, §5.1) and `deskSidebar` (`open` | `collapsed`, default
`open`, §5.2). The pill pushes the whole entry to the service (§4); a key a
user never set takes its default.

## 2. Files

```
plugin/
├── manifest.json
├── Service.qml         data: watches index.json, runs capture timer, exposes model
├── BarWidget.qml       the pill; the jax.seldon.panel shim (§8)
├── Desk.qml            the desk (§5): window, header, notices, sidebar, sections, keys
├── Model.js            pure functions: formatting, colour mapping, aggregation, desk geometry
├── components/
│   ├── desk/           DeskWindow (layer shell), Header, KpiStrip, Notices, Sidebar,
│   │                   NavIcon, Search, Section (the section base), ListColumn, ListRow,
│   │                   GroupedRow, DetailPane, ActionBar, KeyValues, Arm (arm twice),
│   │                   Progress, CaseTile; the sections' parts: EventDetail, DriftForm,
│   │                   JournalField, NewCaseSheet
│   ├── overlay/        the Prime Radiant's charts (§6): Heatmap, Series, DriftBars,
│   │                   RiskDonut, Timeline, ThePlan, OverlaySlot, ChartCanvas
│   ├── Banner.qml  MaskIcon.qml
│   └── the 0.1 panel's tab components still to port (DecisionsTab, SystemTab,
│       MemoryTab, NewDecisionSheet): unreferenced since WP-121; WP-123 ports and deletes
│       them (WP-122 did Today, Changelog and Work)
├── sections/           Today, Changelog, Work, Decisions, System, Memory, Radiant, Graph,
│                       Settings (+ SectionStub for the sections not built yet)
├── README.md  LICENSE  SECURITY.md  preview.png  assets/
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
  While the AUR package does not exist (ADR-0024), the engineMissing
  banner's fix is the constant GitHub one-liner `curl -fsSL
  https://github.com/JohnAndrewsX/jax-seldon/releases/latest/download/install.sh | bash`
  (run only on the user's click, in the floating terminal; the script
  checks the engine against `SHA256SUMS`), and its text says "AUR
  package: coming soon; until then install from GitHub". Both are
  constants in `Model.js`, `INSTALL_ENGINE_COMMAND` and
  `ENGINE_MISSING_DETAIL`; they flip back to `omarchy pkg aur add
  jax-seldon` and an AUR text together when the package is live
  (WP-044).
- Exposes `function run(args)` for other files; **only fixed argument
  arrays**, never strings assembled from index content except as single
  arguments (case ids, event ids validated by regex before use).
- Every engine call that exits above 0 (the probe included) logs one
  `console.warn` line, `jax.seldon: seldon <command> exit <code>: <first
  stderr line>` (the first line of the engine's JSON error message when
  stderr is empty, WP-078), so `journalctl --user -t omarchy-shell` shows
  each failing timed capture (WP-068).
- Exit 4 (lock held) of a `capture` or `status` is not an error: the call
  runs again after 30 s, at most 3 times (a locked capture takes its queued
  `status` along); meanwhile the capture result reads "waiting for another
  seldon process; trying again shortly" in the neutral tone and
  `capturing` stays true. After the third retry the exit is an error as
  before. An explicit *Capture now* replaces a pending retry (WP-068):
  the Changelog button (a click while it reads *Capturing*, WP-078), the
  bar's right click, the `c` key and IPC `capture` alike.
- Capture warnings (WP-085): the `warnings` of a capture the plugin ran
  that exited 0 (`capture --json`, SPEC-ENGINE §3; today the state reset
  of WP-081 with its restore hint) are kept as the engine wrote them in
  `captureWarnings` and become a neutral notice under the status and
  snapper banners on every tab: "Capture warned", the first line of each
  warning, all of each on hover. It has no action (no page of the user
  guide ships with the plugin; the warning names its section) and is set
  as plain text, never part of a command. It stays until a capture the
  plugin runs exits 0 without warnings; a failed or locked capture leaves
  it. Captures run outside the plugin (the CLI, hooks) are not seen.
- The rules check (WP-101): `["seldon", "doctor", "--only", "rules",
  "--json"]` in its own `Process`, read-only and beside the queue (it
  takes no lock and starts no omarchy, snapper or git probe), when the
  desk opens or the engine turns up while it is open, at most every
  10 minutes; never in dev mode or without an engine. Only its `rules` row
  is read (`rulesBanner`, §5.6). Doctor's exit 1 (an error row) still
  carries the JSON; only exits above 1 log a warning line. The banner's
  click queues `["seldon", "rules", "update", "--json"]` like any write,
  then forces a new check.
- One call at a time per family (plan and agent, drift, decide): a call
  refused because one of its family is pending returns false and sets
  `busyRefusal` to `{ family, action, caseId, eventId, text }` with the
  text "Another action is running — try again in a moment"; the new-case,
  drift and new-decision sheets show it in the neutral tone (WP-068). The
  new-decision sheet refuses a second Create of its own pending decision
  without asking; one another panel sent (the bar builds a panel per
  monitor, all on one service) gets the busy text (WP-078).
- Engine minimum: the manifest's `seldon.engineMin` (injected by the
  shell) is compared with the probed version (`major.minor.patch`; a
  pre-release suffix counts as its version). An older engine gets the
  "Engine too old" banner (§5) and one warning line (WP-068).
- Running code vs installed plugin (WP-090): `Model.PLUGIN_VERSION`, set
  by hand with the manifest's `version` (docs/VERSIONING.md), is compared
  with the `version` of the manifest the shell injects; the shell re-reads
  it from disk on every rescan but keeps the plugin code it compiled first
  (Quickshell 0.3.1 has no `Qt.clearComponentCache`). When they differ the
  service publishes `restartNotice` (§5). Reading the plugin's own
  manifest is not Seldon data: CONTRACT.md and AGENTS.md §3 cover the
  logbook and the index.

## 4. BarWidget.qml

`WidgetButton` showing the bar glyph (A4, the Prime Radiant mark) and the
counts `A · D`, where A = active cases and D is set by the bar-widget
setting `driftInBar` (ADR-0028 §4a; manifest `enum`, options `crisis`,
`all`, `none`, default `crisis`, read with `setting()` from the widget's
`shell.json` entry, which hot-reloads; an unknown value is the default):
`crisis` → `summary.crisis`, `all` → `summary.openDrift`, `none` → never.
Hidden parts when 0: the glyph alone, `2`, `· 1`, `2 · 1`; a vertical bar
shows the glyph only. Colour: default foreground; accent when A > 0;
theme error colour when any crisis, in every mode (attention never
colours the bar); the glyph always takes the counts' colour. Tooltip, the
same in every mode and in the neutral tone: "Seldon — 2 active cases, 1
crisis, 7 changes without a case, last capture 4 min ago" (the crisis
part only while there is one; "changes without a case" counts attention,
`openDrift − crisis`). The widget pushes the mode to the service
(`setDriftInBar`), whose IPC read-out reports the same pill.
Left click toggles the desk (§5; `toggle jax.seldon` through the plugin's
scoped facade, `bar.shell`); middle click opens the desk at the Prime
Radiant (`summon` with `{"section":"radiant"}`); right click runs capture.
The widget has no popup of its own (ADR-0034 §7): it offers the bar no
`open`/`close`, so Tab between bar panels passes it by. It pushes its
whole `shell.json` entry to the service with the other settings
(`setDeskSettings`), which the desk reads and writes back (§5.5).

Glyph (WP-051, `assets/DELIVERY.md` §5): the glyph box is the shell's icon
canvas, `Style.bar.iconCanvas` (16 px at scale 1.0, 20 px at 1.25), placed
`Style.space(2)` before the counts. File by the box in device pixels: 16 →
`a4-bar-glyph-16.svg`, 20 → `a4-bar-glyph-20.svg` (hand-hinted, drawn
unsmoothed), anything else → `a4-bar-glyph.svg` (vector). The counts sit
where the shell's own label would (vertically centred), so they share the
neighbouring widgets' baseline; the glyph's ink centre (row 7.5 of 16, 9.5
of 20, the middle of the vector box) is put on the digits' centre — the
baseline minus half the height of the ten digits in the bar font — and
snapped to device pixels. Brief check 4: the two centres within 1 px at
scale 1.0 and 1.25; `tests/plugin/bar-view.sh` measures it in three themes.

Every Prime Radiant image in the plugin (§4–§6) is a copy under
`plugin/assets/` of a file in `assets/` (ADR-0009; no symlinks), drawn by
`components/MaskIcon.qml`: the masks are `currentColor` with the fallback
colour on the `<svg>` root, so the plugin sets the root's `color` to a
theme colour and hands the file to `Image` as a data URL (read once through
`FileView`). No colour is written into QML; the tint follows the theme.
The shell's own monochrome icons (tray) use `MultiEffect` colorization,
which the headless harness's software renderer does not paint; the root
colour gives the same result in both.

## 5. Desk.qml — the desk

ADR-0034. One surface for everything the plugin shows: the manifest's
`overlay` entry point, opened by the pill, by `omarchy-shell shell
toggle|summon jax.seldon [payload]` and by the `jax.seldon.panel` shim
(§8). It replaced the 0.1 bar popup (`Panel.qml`) and the fullscreen
Prime Radiant overlay (`Overlay.qml`) in WP-121.

### 5.1 Window and width

The shell's overlay loader creates `Desk.qml` without properties, then
injects `shell` (the plugin's scoped facade, `PluginShellApi`), `manifest`
and `service`; every binding tolerates `service === null`. `summon` calls
`open(payload)`, `hide` calls `close()` and unloads the item, `toggle`
reads `opened`. What the desk keeps between opens (the section, each
section's selection) lives in the service (`deskMemory`).

The window, `components/desk/DeskWindow.qml`, is one layer-shell
`PanelWindow` on the overlay layer with exclusive keyboard focus while
shown, anchored on all four edges and transparent, the pattern of the
shell's own menu (`plugins/menu/Menu.qml`): the desk is a card inside it,
and the rest of the surface is a transparent click-catcher that closes
the desk (no dimming: the desk reads as an application). It keeps out of
other surfaces' exclusive zones (`ExclusionMode.Normal`, no zone of its
own), so the bar stays visible above it and the pill stays clickable. On
every open — from closed, and again on a summon of the open desk (the
keybinding, the pill of another monitor) — it moves to the monitor
Hyprland has focused (`Hyprland.focusedMonitor.name` against
`Quickshell.screens`, as `Bar.qml focusedScreenName`; the first screen
when none matches); one window, never one per screen. It does not follow
the focus while it stays open: with Hyprland's focus following the mouse,
a pointer crossing to the other monitor would move the desk away under
the user; the next open takes it there.

Width (`Model.deskGeometry`): with the window's width `W` (the screen's
usable width) and `gap = Style.gapsOut`, `avail = W − 2·gap` and

```
width = clamp(round(avail × deskWidth / 100), min(960, avail), avail)
```

centred (`x = gap + ⌊(avail − width) / 2⌋`), from `gap` below the bar to
`gap` above the screen's bottom. At 100 % it fills the row. ADR-0034 §1's
`screen.width` is read as this `avail`: the screen's usable width minus
Omarchy's outer gaps on both sides (90 % of a 1920 px screen is 1719 px,
not 1728). ADR-0034 §1
assumed an unanchored layer-shell axis for the centring; the desk
computes it instead (WP-121 verified the menu pattern on the dev and the
test host: the surface sits below the bar, the card centred to the pixel).

### 5.2 Layout

Top to bottom (prototype `prototype-desk-v6`):

- **Header** (`Header.qml`): the A5 mark in the accent and "SELDON"
  (heading font, bold, letter-spaced) over "machine · Omarchy version ·
  captured N ago" (`Model.deskSubline`); the **status chip** when a notice
  is up — the first notice's title and "+N" for more, in the notice's
  tone, "N notices" where the title does not fit; a click folds the
  notices; the **KPI strip** (`Model.deskKpis`): active (accent while > 0)
  · verification · queued · crises (urgent while > 0) · attention
  (`openDrift − crisis`), each a click to its section (Work; Changelog
  with the crisis or open filter); Settings (`,`) and *Esc*.
- **Notices** (§5.6) under the header, full width, foldable by the chip.
- **Sidebar** (`Sidebar.qml`): "SECTIONS", the nine targets with icon,
  label and count (`Model.deskCounts`: Today the events today, Changelog
  the open changes (urgent with a crisis), Work active · verification ·
  queued, Decisions "N new" (proposed), Memory lessons + topics), the
  search field and the fold button at the bottom. Icons are the
  prototype's 24-unit paths drawn with `QtQuick.Shapes` in the theme
  colour (`Model.DESK_SECTIONS[].icon`, `NavIcon.qml`).
- **The section**: a list column (`ListColumn.qml`; title, the section's
  head — chips, a field —, a `ListView` of `ListRow`s: stripe crisis /
  attention, title, meta, an age or date at the right) and a detail pane
  (`DetailPane.qml`: the sticky `ActionBar` — actions, id and risk at the
  right, the arm hint — outside the scrolling content, so it stays while
  the detail scrolls). Solo sections (7 Prime Radiant, 8 Graph) have no
  list column.
- **Footer**: the service's last error (urgent), else "Dev mode,
  read-only: <index path>" in dev mode; at the right the key hint.

Columns (`Model.deskLayout`): sidebar `Style.space(210)` open,
`Style.space(56)` icons only; list `clamp(round(0.3 × rest),
Style.space(260), Style.space(360))`; the detail takes the rest. A desk
narrower than 960 px (only on a screen that narrow) shows the sidebar's
icons whatever `deskSidebar` says; narrower than 760 px the list and the
detail **stack**: the list, Enter or a click shows the detail with a "‹
Back to the list" row. Otherwise `deskSidebar` decides (`collapsed`:
icons only; the search is then reached by widening it).

### 5.3 Keys

| Key | Does |
|---|---|
| `1`–`8` | Today, Changelog, Work, Decisions, System, Memory, Prime Radiant, Graph |
| `,` | Settings |
| `Alt+↓` / `Alt+↑` | the next / previous of the nine, wrapping |
| `/` | the sidebar search (filters the current section's list); Enter leaves it and keeps the filter, Esc clears it and leaves |
| `↑`/`↓`, `k`/`j` | move in the list |
| `Enter`, `Space` | in the stacked layout: show the selected row's detail; otherwise the detail's first action that launches nothing — Work arms, then runs it (on an active case *To verification*, as in 0.1: **Enter never starts an agent**, only `a` or a click on *Hand to agent* does); on open drift (Today, Changelog) the default form opens; Today's yesterday row opens or closes. ADR-0034 §2's "Enter selects" is moot where the selection is the cursor (WP-122) |
| `Esc` | in this order: the section's own state (an inline form), the search filter, the stacked detail, then close |
| `c` | capture now |
| `n`, `+` | Today's note field, Work's new case (sections 1 and 3 take them) |
| other letters | the current section's (`i`, `e`, `a`, `r`, `f`/`F`, `x`, `d` as ADR-0034 §2 lists them; sections 1–3 in §5.4, `d` in WP-123) |

Every character goes to the current section first (`Section.textKey`);
the desk takes `c`, `n`, `+` only when the section did not. A focused
field keeps every key; Esc in it is the field's. Tab and Shift-Tab do
nothing (the desk is not a bar popup). A section change closes the search
filter and gives the keys back to the desk. Writing actions arm on the
first press (`components/desk/Arm.qml`: `press(id)` arms, the same id
again returns true and disarms); every key that did not press disarms,
and the sticky action bar shows the hint while armed (the two-press rule
of §5.7, shared by the sections).

### 5.4 Sections

`sections/*.qml` extend `components/desk/Section.qml`. A section is made
on its first visit and kept while the desk is loaded; only the current
one is visible. It reads from the desk: `service`, `index` (null while
its contents mean nothing in the status, as before), `layout`,
`searchText`, `arm`, `detailShown`; it sets `editing` while a field of
its own has the keys and `selectedId` (kept between opens). The desk
calls `move(dy)`, `activate()`, `textKey(t)`, `select(id)`, `back()` and
`applyPayload({ select, filter, period })`, each returning true when used,
and `view()` for the read-out. Sections never aggregate on paint: what
they render is prepared by the service when the index changes and looked
up (ADR-0034 §3); lists are `ListView`s.

| # | Section | Built in |
|---|---|---|
| 1–3 | Today, Changelog, Work | WP-122 (below) |
| 4–6 | Decisions, System, Memory | WP-123 (§5.7) |
| 7 | Prime Radiant | WP-123 (§6) |
| 8 | Graph | WP-125 (ADR-0034 §5) |
| `,` | Settings | WP-121 (§5.5) |

Until a section is built it is a stub (`SectionStub.qml`): its title and
"Coming in WP-12x." in the list and the detail.

In every section the **selection is the cursor**: ↑/↓ (`k`/`j`) move it
and the detail follows; it is an id, so it stays on its item when a new
index adds rows above it. Lists are `ListView`s of `GroupedRow`s (a
`ListRow` with an optional group header: the day, NEEDS YOU, ACTIVE · 2).
The rows of sections 1–3 come from `Service.deskChangelog`, `deskToday`
and `deskWork`, built once per index (`Model.deskChangelog`, `deskToday`,
`deskWork`); a section only filters them (chip, sidebar search) and looks
details up by id. A field of a section that is hidden gives the keys back
to the desk (`Desk.takeKeys`) and keeps its draft. No section runs an
engine command the 0.1 panel did not run; every argv is a CONTRACT.md form
built in `Model.js`.

#### Today (1)

The list: the date beside the day's state pictogram (A11, §5.6) and *Open
in editor* (`e`, today's journal), the tiles events today and 7 days, the
journal field (`JournalField.qml`, the 0.1 QuickEntry: `n` focuses it;
`seldon log [--case <id>] --json -- <text>`, the text one argument after
`--`, a picker of the open cases, Enter saves, the field empties only once
the engine has saved the note, its refusal is the line under the field and
the text stays, Esc gives the keys back), then **NEEDS YOU** — the
crises, newest first, a group by its leader (the 0.1 red strip's
successor) — and **JOURNAL**: today's entries, "Nothing in today's journal
yet.", the yesterday row (Enter or a click opens it in place). The sidebar
search filters the crises and entries, yesterday's included. Nothing is
selected on entry, so the detail is the **overview**: an empty slot for
WP-119's setup card (`todaySetupSlot`), one sentence ("Seldon is
recording. 2 changes need you." / "… Nothing needs you."), a lead on what
stays quiet, the active cases as tiles (id · risk, title, the plan's
progress, "2/4 steps · claude-code"; a click opens the case in Work) and
**New case**: one sentence → `seldon agent start --new --json --
<intent>` (`i` focuses it; the call and the result line Work's *Run*
shares; the field keeps its text until the engine has made the case). A
selected crisis shows the event (`EventDetail.qml`, as in the Changelog);
resolved here, it leaves NEEDS YOU and stays shown with the engine's answer.

#### Changelog (2)

The list: the slot of "Agent sorts N open changes" (`triageSlot`, empty
until WP-124b), the chips **open · crisis · attention · routine · in
case · all** with their counts (`f` / `F` the next / previous; default
open). A row's class: open drift with `crisis` → crisis; other open drift,
group members included → attention; else with a case → in case; else
routine. **One count everywhere**: the drift chips (open, crisis,
attention) list and count changes, a pacman group once as its leader
("mesa +2"; its members are in the leader's detail), so open equals the
sidebar's Changelog count, crisis and attention the header's figures and
the quiet line, and Hide counts a group once; routine, in case and all
list and count ledger events, a group's members as their own rows. The
count line says "N changes" on a drift chip, "N events" on the others. Then "N events · newest first" with *Ledger* (`e`, this month's
ledger) and *Capture now* (`c`; it reads "Capturing" with a spinner while
the capture, its status or a lock retry is pending, and a click then
captures at once), the quiet "N changes without a case" (dim, no colour,
ADR-0028 §4b), "+N more changes without a case not listed here" (ADR-0020),
"N changes hidden this session" with *Show*, "Last capture: …". Rows,
newest first under their day: the subject's last path segment (a group's
leader with "+N"), "source · kind · case", the time today or "30 Sep
17:00", a stripe on open drift (crisis urgent, attention accent; nothing
else is coloured, whatever its zone). A new chip starts at its first row;
when the selected event leaves the chip (it was just linked) it stays shown
in the detail and the list has no highlighted row; ↑/↓ continue from where
it was. The shim's `filter <source>` and a payload `filter` that is a
source name show "all" with the source in the sidebar search; a chip id
selects that chip.

The detail (`EventDetail.qml`; prototype `eventDetail`): the sticky bar
(`Model.eventActions`) — open drift: *Ask agent* first once the engine has
`agent ask` (WP-124b; `Service.askAgentAvailable`, false until then),
*Link to case…* (*Link to C-… …* when the engine proposes one), *Explain…*,
*Dismiss…*, and for attention *Hide* / *Show*; an event with a case: *Open
case* (Work with the case selected; for a case the index no longer lists
— it keeps the last 50 completed — one line says so and offers *Open in
editor*, whose engine answer replaces the line: only the engine can tell
whether the file is still there); routine: none; the class at the right.
Then "source · kind", the full subject, its class, for a crisis the **Why
loud?** callout from the engine's rule: the index has none, so the detail
asks `seldon drift show <id> --json` (in CONTRACT.md's table, read-only;
for a group its leader) once for a selected crisis and keeps the answer
while the item stays a crisis (`Service.driftRules`). `always-red` → "A
package on your crisis list ([drift] alwaysRed in
~/.config/seldon/config.toml) was installed, removed or downgraded by name
in this transaction."; `always-red-paths` → "The path matches your crisis
list ([drift] alwaysRedPaths …)."; `attention-all` → "[drift] attention =
"all" is set: every change without a case is open drift, and a crisis is
a change in the red zone."; another rule is named as it is. Until the
answer (and in dev mode, without an engine) it says only what the index
proves: "The engine classed this <source> change as a crisis" and how to
ask for the rule. Then, from `proposedCase`, "C-… plans it (its plan names
this change); nothing has linked it yet." or "No open case plans it, and no
case is linked." — the Case row ("proposed: C-…") and the Rule row
("crisis · rule … · planned by C-…, not linked" or "· no case") say the
same. The key/values When · Who · What · Case · Rule · Source · Zone ·
Resolved · Event (values wrap at word boundaries; a longer token breaks
anywhere), a
group's members (`seldon drift show` for those the index no longer lists),
"proposed for C-…", and "None of this is required. An agent explains only
what it can prove." The bar's Link, Explain and Dismiss only open the
inline form (`DriftForm.qml`, the 0.1 drift sheet's logic and API): Link
(the open cases, the proposed one first and preselected; a group offers
*All N* / *Only <package>*, `--only` naming exactly the selected row),
Explain (why; zone pre-filled with the ledger zone, risk R1, an optional
area slug), Dismiss (the reason). Enter on an open drift row opens the
default form (Link when proposed, else Explain; none is required, ADR-0028
§3). The form's own button writes: Enter in a field or on the button arms
it ("Press Enter again: Link firefox and 2 more to C-2026-005", in the form
and the sticky bar), the second Enter runs it, a click runs at once; any
change to the form disarms, a new index does not. The text stays until the
engine has resolved the item (its refusal and the busy text are the line
under the form); Esc or *Cancel* hides the form and keeps the draft, per
event. A no-op says "Already resolved: …". Resolved, the form goes, the
keys return to the desk, and the detail shows "Resolved: …" and the
engine's answer. *Hide* writes nothing: the item leaves the open and
attention chips for this shell session (`Service.deskHidden`; a group as
one); a crisis has no Hide (ADR-0028 §3); *Dismiss* is the recorded way.

#### Work (3)

The list: the one-sentence start — an intent field and *Run* (`i`;
`seldon agent start --new --json -- <intent>`, "Running" with a spinner
while the engine works, its refusal (no default agent, with its fix) the
result line and the text kept, then the field empties and the new case is
selected); the WIP line "2 / 3 active" against the bar setting `wipLimit`
(default 3; "· at the limit" in the accent, "· over the limit" urgent;
warns, never blocks); *By agent* (the Completed group narrowed to the cases
an agent closed, a spot check, ADR-0027 §5; its header then reads
"COMPLETED · 1 / 2"); *New case* (`+`); the engine's answer to the last
case action. Then the cases by group — ACTIVE · VERIFICATION · QUEUED ·
COMPLETED (completed and dropped, the index's last 50) — with "id · risk ·
area · closed by agent · reopens … · N proposed" and the plan's steps.

The detail: the sticky bar by status (`Model.caseDeskActions`) — queued:
*Start*, *Drop*; active: *Hand to agent* (`agent start <id> --json`), *To
verification*, *Drop*; verification: *Complete*, *Drop*; completed:
*Reopen* (`plan reopen <id> --json`); every case *Open in editor* (`e`,
`open <id> --editor --json`), last; id · risk at the right. Every writing
action but Reopen arms on the first press or click and runs on the second
(`Arm.qml`; the button reads "Confirm …", the bar's hint names the key:
"Hand to agent C-2026-003? Press a again or click Confirm.", "To
verification C-2026-003? Press Enter again or click Confirm.", "Drop
C-2026-004? Press x again or click Confirm. This is final."); any other
key, another selection or a new index disarms. Enter arms and runs the
first action that launches nothing (`Model.caseEnterAction`: Start, To
verification, Complete, Reopen, Open in editor) — **Enter never starts an
agent** (operator, WP-122 round 2): on an active case Enter twice is To
verification, as in 0.1, while *Hand to agent* stays the first button and
runs only from `a` twice or a click and Confirm. `x` Drop, `r` Reopen (one
press: it creates a case and destroys nothing). A step's case moves to its new group
with the next index and stays selected; a Run, a reopen or the sheet's new
case is selected. Without write access (dev mode, no engine, not
initialised) nothing arms and the bar says why. Under the bar what the
index carries (`Model.caseDetail`; the plugin never reads the case file,
AGENTS.md §3): the title, "completed by agent" / "reopens C-…", the
key/values Status · Risk (R3: "every step that can break boot needs your
go") · Zone · Area · Priority · Agent · Rollback (the snapshot) · Dates ·
Reopens · Proposed · File, PLAN (the steps' progress; "The steps, the
Intent and the Result are in the case file." with *Open in editor*), LOG
(this case's lifecycle events and notes in the index, newest first) and
LINKED CHANGES · N (the case's `events` the index still lists, "+N older
changes the index no longer lists"). *New case* puts the 0.1 sheet in the
detail (`NewCaseSheet.qml`: title, zone, risk, priority, an optional area
slug refused in the plugin when malformed → `plan new --zone <z> --risk
<r> [--area <a>] [--priority <p>] --json -- <title>`; Enter creates; the
fields keep their text until the engine has made the case; Esc closes the
sheet with its draft, `+` brings it back). "Back to active" (prototype) is
not built: no engine verb moves a case from verification to active.

### 5.5 Settings

Four groups in the list; Appearance is selected first.

- **Appearance** (live): "Desk width" — a slider from 50 to 100 % in
  steps of 10 (`PanelSlider`, six notches), the value as "N %", the
  presets 50 % / 67 % / 75 % / Full, the line "N px on this screen" and a
  small picture of the screen with the desk on it; "Sidebar" — Open /
  Collapsed. Dragging previews the width on the desk itself and writes
  nothing; so does the mouse wheel or a touchpad over the slider, which
  `PanelSlider` turns into a move and a release per notch: the desk takes
  those as a preview and writes once, 600 ms after the last notch (or
  the next release or preset click takes its place, or closing the desk
  writes it). The release, a preset or a sidebar click (and the sidebar's
  fold button) **write once** through the facade's
  `updateEntryInline("jax.seldon", settings)` — the only place the plugin
  writes `shell.json`, its own entry only. `settings` is every key of the
  current entry (unknown ones too) plus the changed one, because the
  shell replaces the entry with `{ id } + settings`
  (`Model.deskSettingsWrite`); defaults the user never set are not
  written. A value that is already stored is not written (the facade
  would answer "nothing changed" as `false`, and every write is a
  `config-change` event, ADR-0028). Until the shell's reload brings the
  entry back through the pill, the desk shows the written value (5 s at
  most). A refused write (`false`, or no such method) leaves the stored
  value and the page says "The shell did not take the change. Change it
  in Omarchy's bar settings (Seldon widget)." — no error state. When the
  plugin is enabled but not in the bar (no pill has pushed an entry,
  `Service.entryKnown`), the shell has no entry to keep the setting in:
  the desk writes nothing, keeps the change in the service until the
  shell restarts, and says "Add Seldon to the bar to keep this setting;
  until then it holds until the shell restarts." Omarchy's bar settings
  show the same keys; both paths are valid.
- **Capture**, **Agents**, **Quiet** (read-only): the values in force and
  where each is set — the capture interval, the active cases limit and
  "changes counted in the bar" in Omarchy's bar settings; the collectors,
  the launcher, the start folder and what may be loud in
  `~/.config/seldon/config.toml` (SPEC-ENGINE §2). The desk never edits
  `config.toml`.

### 5.6 Notices, header mark, pictograms

The notices under the header are the 0.1 panel's banners with their
one-click fixes, in this order: the restart notice after a plugin update,
the status banner, snapshots not readable, the outdated agent rules,
what their update did, the capture warnings. Each is `Banner.qml` on the
service's object; a fix goes to `Service.fix(action, banner)`. Their
texts and fixes:

Banner states (under the header): engine missing → "Install the engine:"
the GitHub one-liner while the AUR package does not exist (§3,
ADR-0024), afterwards `omarchy pkg aur add jax-seldon` (ADR-0016;
`omarchy pkg add` reaches the official repositories only), with *Install in
terminal*, *Copy* and *Check again*; contract
mismatch → `omarchy plugin update jax.seldon` when the plugin is older
than the index, the GitHub installer one-liner when the engine is older (until the
AUR package is live, ADR-0024); engine older than the manifest's
`engineMin` (§3; in place of every status banner but engine missing and
contract mismatch) → "Engine too old", "Update the engine to at least
X", the same installer one-liner with *Update in terminal*, *Copy* and
*Check again* (WP-068); snapshots
not readable (ADR-0026) → the one-line read grant
`sudo setfacl -m u:$USER:rx /.snapshots` with *Run in terminal*, *Copy*
and *Check again*; the detail is the engine's message, then on its own
line what the fix grants (read access to the snapshot directory listing
and the snapshot info files, no snapshot creation, change or deletion)
(WP-054, issue #2); *Check again*
runs a capture, the same call as *Capture now* (`capture --all --json
--quiet`, then `status --json`), because only a capture rewrites the
collector state this banner reads (reloading the index would not); after
*Run in terminal* the banner shows "When the command has finished, press
Check again" under its buttons until the index next changes; not
initialised → "Run `seldon init`" with *Run in terminal*, *Copy* and
*Check again*; index stale →
*Capture now*; outdated agent rules (WP-101, ADR-0027 migration) → "The
logbook's agent rules are outdated (v1)" from the `rules` row of `seldon
doctor --only rules --json`, which the service runs when the desk opens (and when the
engine turns up while it is open), at most every 10 minutes, in its own
read-only process beside the queue, never in dev mode; *Update rules* runs
`seldon rules update --json` (it rewrites only the engine's block and
archives an edited one, so nothing is lost), then doctor again; a damaged
or newer block shows the engine's fix as text, without a click (the
`--replace` archive is the user's decision); capture warnings → the neutral "Capture warned" notice of
§3 under the banners, without an action; plugin updated under a running
shell (§3) → the neutral "Restart the shell to finish the update" above
the banners, with both versions and one action, *Restart shell*, which
runs the argv `["omarchy-restart-shell"]` once per service instance
(a second click could kill the new shell; WP-090). The 0.1 panel's red
crisis strip ("N changes that can affect boot, login or the shell have no
case", ADR-0028 §4b) has no successor in the desk: the header's crises
figure (urgent while `summary.crisis` > 0) and Today's "Needs you"
(WP-122) carry it.

Header mark (WP-051): the A5 lockup — the mark, then "SELDON" in the heading
font (`Style.font.heading`, bold), baseline-aligned. Metrics from
`assets/DELIVERY.md` §5, derived from the heading's cap height (the tight
height of "H"): box = 2 × cap rounded to an even pixel count, the
wordmark `round(cap / 2)` after the box, its baseline `box / 2 + cap / 2`
below the box top (the mark's centre on the cap-height centre). At the
default font that is box 24, gap 6, baseline 18, the delivered numbers.
File by the box in device pixels: 24 → `a5-panel-mark-24.svg`, 32 →
`a5-panel-mark-32.svg`, anything else → `a1-icon-mask.svg` (e.g. 30 at
font scale 1.25). Colour: the accent (ADR-0034, prototype).

State pictograms (A11, 48 and 96 grids, never drawn below 48 px; by the
size in logical pixels — the vector scales with the DPR — the 48 grid up
to 72 px, the 96 grid above): the status banner shows its status's
pictogram, `Style.space(48)` square, left of its text, in the banner's
tone — engine missing → `engine-missing`, not initialised →
`logbook-not-initialised`, index missing (or unreadable) →
`index-missing`, index stale → `index-stale`; the contract mismatch and
the snapper banner have none. The Today tab shows the day's state left of
the date and the counts (events today, in 7 days, active, queued, and
"without a case", the attention count `openDrift − crisis`, the number the
tooltip and the Changelog line show), `Style.space(48)`: crisis (urgent) when any
crisis, else case active (accent) when active cases, else all clear
(foreground); none without an index. Attention alone changes nothing
(ADR-0028 §4b), so the drift-open pictogram is not shown.

### 5.7 Behaviour carried over from the 0.1 panel

Normative for sections 4–6 until WP-123 rewrites it into the section's own
paragraph; sections 1–3 are §5.4 (WP-122), which supersedes what this
section says about the Today, Changelog and Work tabs. Superseded by §5.1–§5.4 already: the
`KeyboardPanel` popup and its width, the tab strip and its cells, Tab /
Shift-Tab handing over to the bar, ←/→ between tabs, the red strip. The
text as the 0.1 panel had it:

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
case's `agents`; refused on queued/verification/closed cases. One-sentence
start (WP-101, ADR-0027 §6): above the WIP line an intent field and *Run*
send `seldon agent start --new --json -- <intent>` (the text one argument
after `--`, exactly as typed; Enter in the field or the button; key `i`
takes the field from the Work tab); *Run* reads "Running" with a spinner
while the engine works, the engine's refusal (no default agent, with its
fix) is the result line, the field keeps its text until the engine has
made the case, then empties, and the cursor goes to the new case. The
manual path stays: *New case* opens the sheet. A completed case whose
`tags` hold `closed-by-agent` (CONTRACT.md rule 8) reads "by agent" on
its tile and "completed by agent" on its card; the *By agent* toggle in
the header narrows the Completed column to those cases (its header then
reads "COMPLETED n / total"), a spot check that is never due. Every
completed card offers *Reopen* beside *Open*: one click or key `r`, no
arming (it creates a case and destroys nothing) → `seldon plan reopen
<id> --json`; the result line names the new case, earlier reopens and,
when the engine left `.seldon/active-case` on an open case an agent may be
working, "the active case stays <id>"; the cursor goes to the new case,
whose meta line reads "reopens <id>". While a text
field or the sheet has focus the panel blocks the key catcher; `Esc`
hands the keys back and keeps the draft. So does every tab change (keys,
a click on the tab strip, IPC `tab`), because a hidden field would keep
the focus: an open sheet stays open with its draft, an open picker closes
(WP-067). The Changelog cursor stays on its event when a new index adds
rows above it (a new filter starts at the top), as the Work and Decisions
cursors stay on their case and decision. Drift sheet (WP-021): Enter or
a click on an open drift row, the row's *Resolve…* button, or a click on
the red strip (first crisis) opens the sheet in the Changelog's place;
actions Link (open cases, the proposed case preselected), Explain (intent
+ optional zone/risk/area; result shows the created case with *Open*),
Dismiss (reason); groups offer *All N* / *Only <package>* (`--only`);
writes use two-press arming where any change to the form disarms; the
draft is kept per event; a no-op shows "Already resolved: …"; above the
list "+N more changes without a case not listed here" when `summary.openDrift`
exceeds `drift.length` (ADR-0020). Folded rows read `linked to C-…`,
`explained · C-…: <intent>` (ADR-0021), `dismissed: <reason>`. Decisions tab (WP-023, digit 4): newest first by
id; Enter, `e`, double click or *Open* run `open ADR-NNNN --editor --json`
(id validated); `d` opens the new-decision sheet (title → `decide
--no-edit --json -- <title>`, then `open <newId> --editor --json` from the
result; two-press arming; title kept on refusal; the busy text of §3
while a decision sent from another panel is pending). Memory tab (digit 6):
lessons headings and memory topics with `updated`; Enter, `e` or *Open*
run `open logbook --editor --json` until the engine gains a memory target
(`open memory[/<topic>]`, queued). Linked cases per decision need a
contract field (`decisions[].cases`) and are deferred to the next contract
bump. Width `Style.space(460)` (WP-039; was 380 from WP-011, the
first-party list panels' width, too narrow for six tabs): the shell's
`fittedContentWidth` caps it at the screen. Tab cells are at least as wide
as their label in bold plus the Button padding (equal shares when every
label fits one; otherwise each its own width plus an equal part of the
rest; selection never changes the widths); when the six do not fit one
line the strip wraps, never clips. Filter chips wrap (`Flow`); the
Changelog header ("N events from <source> · newest first") wraps instead
of eliding, so the sort order is never cut off. Only user content may
elide (Changelog row text, Work mini-card titles; Enter or the card shows
it in full); labels never do. Files: one component per tab,
`components/TodayTab.qml`, `ChangelogTab.qml`, `WorkTab.qml`,
`DecisionsTab.qml`, `SystemTab.qml`, `MemoryTab.qml`, plus `EventRow.qml`,
`Tabs.qml`, `Banner.qml`, `QuickEntry.qml`, `CaseCard.qml`,
`NewCaseSheet.qml`, `DriftSheet.qml`, `NewDecisionSheet.qml`. The Changelog source filter has
one chip per schema source (all nine, including `manual`, `agent`,
`seldon`). While an event is open drift, its row is coloured by the drift
item's class (ADR-0028 §4b): a crisis (`crisis: true`) in the urgent
colour whatever its zone, attention in the accent whatever its zone (a
pacman item is red in the ledger and still quiet) — stripe, glyph, status
and badge from one source. Every other row (resolved, linked, with a case,
routine, never drift) is an ordinary Changelog row and quiet whatever its
zone: the muted stripe when the event has a zone, none when it has not
(ADR-0028 §4b; the zone stays text where it is shown, e.g. the sheet). The
open row's note states, never asks: "Crisis · no case" or "No case"
(" · proposed for C-…" when the engine proposes a case). Under the
Changelog header a quiet line "N changes without a case" (attention,
`openDrift − crisis`; hidden at 0): dim, no badge, no tab-strip colour,
no action. Drift-sheet labels follow `crisis`, never `zone`: heading
"RESOLVE A CRISIS" (else "RESOLVE DRIFT"), the card's zone label
"<zone> · crisis" (the ledger zone, e.g. "yellow · crisis") in the urgent
colour; Explain pre-fills the ledger zone. Above Link / Explain / Dismiss
the sheet has an *Ask agent* slot, first because the agent explains with
evidence and the human never has to (ADR-0028 §3); it stays empty and
takes no space until WP-095 puts its button there.

| Tab | Content | Actions |
|---|---|---|
| Today | today's journal entries, yesterday collapsed | QuickEntry (`seldon log`), "Open in editor" |
| Changelog | ledger rows newest first, source filter chips, snapshot rows highlighted, drift rows marked | row → link/explain/dismiss sheet; "Capture now" |
| Work | intent field + *Run* (WP-101); three columns queued/active/completed (last 50, scrollable), "by agent" marker and filter | *Run* (one sentence → a started case with an agent, WP-101), "New case" (title + zone + risk + optional area/priority), start/verify/done/drop with two-press arming, Open in editor on every card, *Reopen* on completed cards (WP-101); "Start agent" (runs `omarchy agent prompt` or the configured launcher with a prompt that names the case and the logbook) is WP-022 |
| Decisions | ADR list with status | "New decision" |
| System | omarchy version, package counts, deviations, snapshots, plugins, theme | "Open in editor" (rebuild/update-impact actions are Phase 3 engine commands, allowed by CONTRACT.md, not wired in v1) |
| Memory | lessons headings, memory topics | "Open" |

## 6. Prime Radiant — desk section 7

Section 7 of the desk (§5; ADR-0034 §4), solo (no list column): `7`, the
pill's middle click, or a payload `{"section":"radiant"}` — and the 0.1
overlay's `{"period":"30"}`, which implies the section, so an existing
binding still lands on the charts. The fullscreen overlay window, its
header and its own banner are gone (WP-121): the desk's header and
notices cover them. WP-123 builds the section from the charts below,
reused as they are; until then it is a stub. The chart semantics below
stay normative. Layout: 12-column grid, `Style.space` gutters.

- Row 1: the period selector (30 / 90 / 365 days / All; default 90 d,
  reset on every entry of the section; WP-030).
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
  (open cases run to today), packed in lanes. Marker shapes (A12, WP-051;
  shape alone tells them apart): release = diamond (accent), snapshot =
  dot (foreground at 70 %), crisis = the narrow concave spindle (urgent),
  drawn from the 16-grid path data of `assets/a12-marker-*-16.svg`
  (`Model.MARKER_PATHS`, kept equal to the files by the unit tests); a
  case span = a bar a third of the lane tall between brackets the lane
  tall, `[` at its start, `]` at its close (an open case has no closing
  bracket; a span too short for both stays a plain bar). The slot's title
  row shows the legend in place of the subtitle: each marker file
  (`Style.space(12)`; by the size in logical pixels — the vector scales
  with the DPR — the 12 grid up to 14 px, the 16 grid above) in the
  canvas colour, then "releases", "snapshots", "cases" (`[` `]`),
  "crises"; it hides when it does not fit beside the caption.
- Row 5 (full width): **The Plan** — active cases (`cases.active`) as
  cards with step progress and agent (no period; the sixth slot, WP-031).

Each chart's summary is its caption in the slot's title row (also its
accessible description and in `call view`); while the pointer is on the
chart the caption shows the hovered item. A chart without data in the
period says "no data in this period". Chart data is prepared by the
service (`Model.periodTable`) when the index changes; the overlay only
draws (one paint per chart per data or size change; no aggregation on
the first frame after entering the section — the harness asserts it).
The shell creates the desk first and injects `service` afterwards (its
Loader's `onLoaded`: `if ("service" in item) item.service = …`), so every
binding first runs with `service === null` and must tolerate that
without work (`Model.periodView` then returns a kept empty view, no
aggregation pass); the harness creates the desk in the same order.

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

How the shell routes (verified against `shell.qml`, Omarchy 4.0.x; see
memory/omarchy-shell.md): because `kinds` includes `overlay`, the plugin is
*not* a bar-widget-panel plugin. `shell summon|hide|toggle jax.seldon`
therefore reaches **Desk.qml**, never the bar widget; `shell call
jax.seldon <method>` reaches only the loaded desk and only while it is
loaded. Routes the plugin honours:

- The desk (overlay entry point): `open(payloadJson)`, `close()`,
  `opened`. Payload (`Model.deskPayload`): `{"section": <id>, "select":
  <id>, "filter": <chip or source>, "period": <30|90|365|all>}`, every key
  optional (`filter`: a Changelog chip — `open`, `crisis`, `attention`,
  `routine`, `case`, `all` — or a source name, which shows `all` with the
  source in the sidebar search); section ids `today`, `changelog`, `work`, `decisions`,
  `system`, `memory`, `radiant`, `graph`, `settings`; a `period` alone
  means `radiant`; unknown values are ignored. While loaded, `shell call
  jax.seldon view ""` returns the desk as JSON (opened, section,
  selection, screen, window and desk geometry, width per cent, layout,
  keys, search, status, KPIs, counts, notices, chip, settings writes, arm,
  the section's own `view()`), `section <id>` shows a section ("ok" /
  "unknown section"), `select <id>` selects in the current section ("ok"
  / "not found"). WP-123 adds `setPeriod` and `hover` (the 0.1 overlay's
  names, acting on section 7); WP-125 the graph's read-outs.
- The shim: `IpcHandler` target **`jax.seldon.panel`**, owned by the bar
  widget, kept for one minor release (removed in 0.3.0, announced in the
  CHANGELOG; ADR-0034 §7). It forwards through the plugin's facade:
  `open`, `show` → `summon jax.seldon {}`; `close`, `hide` → `hide`;
  `toggle` → `toggle`; `tab <today|changelog|work|decisions|system|memory>`
  → summon at that section ("ok" / "unknown tab"); `resolve crisis` → the
  Changelog with the crisis filter, `resolve <eventId>` → the Changelog
  with that event selected ("unknown target" for anything else); `filter
  <source>` → the Changelog's `all` chip with the source in the sidebar
  search ("unknown source"); `view`
  → the desk's `view` while it is loaded, else `{"opened":false}`; `pill`
  → what the pill shows. None runs the engine. The bar builds the widget
  once per monitor (plus a zero-size, hidden placeholder in the bar's
  centre section: once a centre anchor is set, the default, the shell
  mounts the whole centre list a second time, hidden), and a target takes
  one handler: only the first drawn instance the bar lists
  (`bar.moduleWidgets`; visible and not zero-size, as the shell's
  `pickDrawnSlot` routes a panel hotkey) enables its handler, a
  placeholder only when no instance is drawn (WP-078). When an instance
  comes, goes, or is drawn or hidden, every instance looks again, the
  owner first, so the next one takes the target over with one handler at
  a time (WP-067).
- Service: `IpcHandler` target **`jax.seldon.service`** (`status`,
  `refresh`, `capture`) — read-only state and the two actions any local
  process could trigger anyway; it is how the test host reads plugin state
  headlessly.
- `shell togglePanelAt <section> <n>` on the Seldon pill toggles the
  desk, as the pill's left click does.

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
Service.qml for development only) and checks that each desk section
renders without QML errors (`qs log --tail`). With `SELDON_INDEX` set the
plugin is in read-only dev mode (CONTRACT.md rule 1): it never runs a
writing engine command, and `SELDON_NOW` pins its clock. The headless
harness under `tests/plugin/` (`just plugin-test`) runs Service.qml in an
offscreen Quickshell against fake engines and checks the exact argv of
every command the plugin issues; it is part of `just check`. After copying
changed plugin code into a live shell, run `omarchy-restart-shell` —
Quickshell 0.3.1 does not reload plugin code on file change.

## 10. Security posture (for the marketplace listing)

No network. No bundled binaries, units or installers. Reads one JSON file,
and its own images under `plugin/assets/` (SVG and PNG artwork, no
scripts; WP-051).
Runs the `seldon` engine with fixed arguments (the forms in CONTRACT.md).
Besides the engine it starts only `wl-copy` and Omarchy's floating-terminal
launcher, each with one constant command, only on a banner click, and
`omarchy-restart-shell` without arguments on the restart notice's click
(WP-090). Never a
shell string built from logbook content. Writes no file itself; the one
change it asks for is its own settings entry in `shell.json`, through
the shell's facade (`updateEntryInline` with the plugin's own id, which
the facade enforces), on an explicit click or slider release (§5.5). Documented in README under
"Security, privacy, privileges" (WP-041).
