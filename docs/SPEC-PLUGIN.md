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
user never set takes its default. One state key rides in the same entry
and is not in `barWidget.schema` (Omarchy's settings panel does not show
it): `setupSnapshots: "not-now"`, written once by the setup card's *Not
now* and taken out by Settings › Capture's *Offer again* (§5.4, §5.5;
WP-119).

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
│   │                   KeyButton (a form's button, once per key press),
│   │                   Progress, CaseTile; the sections' parts: EventDetail, DriftForm,
│   │                   JournalField, NewCaseSheet, NewDecisionForm
│   ├── overlay/        the Prime Radiant's charts (§6): Heatmap, Series, DriftBars,
│   │                   RiskDonut, Timeline, ThePlan, OverlaySlot, ChartCanvas
│   ├── graph/          GraphCanvas: the graph's canvas, layout ticks, pointer, card (§5.4)
│   └── Banner.qml  MaskIcon.qml (the 0.1 tab components are ported and deleted:
│                       WP-122 Today, Changelog, Work; WP-123 Decisions, System, Memory)
├── sections/           Today, Changelog, Work, Decisions, System, Memory, Radiant, Graph,
│                       Settings; ReadingSection (the frame of Decisions, System, Memory)
├── README.md  LICENSE  SECURITY.md  preview.png  assets/
└── fixtures -> ../fixtures (NOT a symlink in the plugin folder; copied in CI for dev builds)
```

## 3. Service.qml

- `FileView` on `~/.local/state/seldon/index.json` with `watchChanges: true`;
  on change parse JSON (try/catch), validate `contractVersion` (with
  `contractReadableFrom`, CONTRACT.md rule 3, ADR-0051), publish
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
  checks the engine against `SHA256SUMS`), and its text says what the
  button does: download from the Seldon release on GitHub into
  `~/.local/bin`, checked, as the user, no password (WP-117). Both are
  constants in `Model.js`, `INSTALL_ENGINE_COMMAND` and
  `ENGINE_MISSING_DETAIL`; they flip back to `omarchy pkg aur add
  jax-seldon` and an AUR text together when the package is live
  (WP-044), with the texts of `INSTALL_ENGINE_SCRIPT` (§5).
  The engine is probed again on the status banner's *Check again*, when
  the desk opens while it is missing, and every 5 s for at most ten
  minutes after a setup terminal opened (the setup card's watch, §5.4
  "Setup card"); never by the capture timer.
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
- One call at a time per family (plan and agent, drift, decide, decide
  accept): a call
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

**The desk steps aside for what it opens (WP-156).** A window the desk
starts would appear under the overlay. After the engine answers a call
that opened or focused a window — `agent start` (also `--new`), `agent
focus` (also "starting"), `open … --editor` (launched or focused; also the
editor a new decision opens), and `agent ask` (*Ask agent*, "Agent sorts
N open changes") — the service closes the desk through the facade, as Esc
does (`Service.stepAside`, `Model.opensWindow`); a notice's terminal fix
(*Grant*, a detached launch with no answer) closes it at once, as
Omarchy's menus close before what they launch. A refusal keeps the desk
open with the engine's text. The desk passes no window: the new window
takes Hyprland's focus when it maps (no launcher reports an address).
The selection is remembered, so the next open shows the case.

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
| `Enter`, `Space` | in the stacked layout: show the selected row's detail; otherwise the detail's first action that launches nothing — Work arms, then runs it (on an active case *To verification*, as in 0.1: **Enter never starts an agent**, only `a` or a click on *Hand to agent* does); on open drift (Today, Changelog) the default form opens; Today's yesterday row opens or closes; Decisions, System and Memory have no such action (theirs open the editor: `e`). ADR-0034 §2's "Enter selects" is moot where the selection is the cursor (WP-122) |
| `Esc` | in this order: the section's own state (an inline form), the search filter, the stacked detail, then close |
| `c` | capture now |
| `n`, `+` | Today's note field, Work's new case (sections 1 and 3 take them) |
| `←`/`→` | the current section's (the Prime Radiant's periods, the graph's day) |
| other letters | the current section's (`i`, `e`, `a`, `r`, `f`/`F`, `x`, `d` as ADR-0034 §2 lists them; sections 1–3 and 4–6 in §5.4; `h`/`l` the Prime Radiant's periods, §6; the graph's `p`, `0`, `-`, `=`, §5.4) |

Every character goes to the current section first (`Section.textKey`);
the desk takes `c`, `n`, `+` only when the section did not. A focused
field keeps every key; Esc in it is the field's. Tab and Shift-Tab do
nothing (the desk is not a bar popup). A section change closes the search
filter and gives the keys back to the desk. Writing actions arm on the
first press (`components/desk/Arm.qml`: `press(id)` arms, the same id
again returns true and disarms); every key that did not press disarms,
and the sticky action bar shows the hint while armed (the two-press rule
of §5.7, shared by the sections). A held key does not confirm: Omarchy's Hyprland repeats a key held
for 250 ms, and `Desk.keyPressed` drops every auto-repeat except of the
keys that move (`Model.deskKeyRepeats`: the arrows with or without Alt,
the page keys, Home, End, `j`/`k`, `h`/`l`, `-`/`=`). A dropped repeat
changes nothing: it neither arms, confirms, writes, launches nor
disarms, so an armed action stays armed with its hint until the key is
released and pressed again; a click is unaffected. In a form, Return and
Enter in a field and Return, Enter and Space on a writing button act once
per press (the forms' and fields' `keyPressed`, `components/desk/
KeyButton.qml`); the arm-twice forms (Link, Explain, Dismiss, New
decision; §5.4) and the one-Enter fields (the note, the intents, the
new-case sheet, Import tasks…) alike. No field acts on Qt's `accepted`,
which a repeat emits too (WP-173).

House rules for every key, now and later (WP-183; the checklist
`docs/skills/omarchy-ux/SKILL.md` points here):

- **The selection is the cursor.** One cursor highlight at a time; the
  selection has its own look. Rows follow Omarchy's
  `Ui/CursorSurface.qml` and read no hover of their own: the row under
  the pointer gets Omarchy's hover look only after the pointer really
  moved (`Ui/PointerMoveGate.qml`; rows that move under a still pointer
  do not count), and any key, a scroll or the pointer leaving clears it,
  so a keyboard move under a still pointer leaves one highlight. A click
  selects. The selection is Omarchy's selected fill plus a second cue
  that is not a fill: an accent bar of at least 3:1 and a bold title
  (`components/desk/ListRow.qml`, `ListColumn.hoverIndex`); the sidebar
  does the same with its current section. A focus ring is drawn only on
  real `activeFocus` (a field, a control the keys reached) or on an armed
  writing button, never as a second marker in a list. Seldon's own
  controls (`KeyButton.qml`, the forms' submit keys, an armed action-bar
  button) wear `components/desk/FocusRing.qml`: the theme's focus border
  where it reaches 3:1 on the popup surface, else the `ui` tone (§7) at
  2 px, drawn over the control's own border (one frame); the chosen
  Changelog chip has an accent bar. `qs.Ui` fields keep Omarchy's focus
  look and get no second frame.
- **Digits are the desk's.** `Desk.qml` dispatches the section digits
  before `Section.textKey`, so a section key on a digit is shadowed as
  soon as a section takes that digit. No new section key uses a digit;
  the Graph's `0` predates this rule, and WP-181's key registry decides
  it with a uniqueness test.
- **A letter means one thing** in every section that has the action
  (ADR-0034 §2: `e` editor, `a` agent, `r` reopen, `x` drop). Super is
  Hyprland's; no key inside the desk uses it.
- **No undo key without a true inverse.** A key that takes something
  back is bound only where the contract has the opposite verb. `plan
  reopen` is not one: it makes a new active case "Reopen: <title>"
  (`engine/src/commands/plan.rs`, `reopen`). Where there is no inverse,
  the action arms (above) and its result line names the real way back.
- **Copying copies the id.** A key or button that copies a record copies
  its id, never logbook text, as ADR-0036 §1 does for agent prompts.
- **Every hinted key is a click too**, and a primary action is always a
  visible button, never only a key.

### 5.4 Sections

`sections/*.qml` extend `components/desk/Section.qml`. A section is made
on its first visit and kept while the desk is loaded; only the current
one is visible. It reads from the desk: `service`, `index` (null while
its contents mean nothing in the status, as before), `layout`,
`searchText`, `arm`, `detailShown`; it sets `editing` while a field of
its own has the keys and `selectedId` (kept between opens). The desk
calls `move(dy)` (↑/↓), `moveAcross(dx)` (←/→), `activate()`,
`textKey(t)`, `select(id)`, `back()` and `applyPayload({ select, filter,
period })`, each returning true when used, and `view()` for the
read-out. Sections never aggregate on paint: what
they render is prepared by the service when the index changes and looked
up (ADR-0034 §3); lists are `ListView`s.

| # | Section | Built in |
|---|---|---|
| 1–3 | Today, Changelog, Work | WP-122 (below) |
| 4–6 | Decisions, System, Memory | WP-123 (below) |
| 7 | Prime Radiant | WP-123 (§6) |
| 8 | Graph | WP-125 (ADR-0034 §5) |
| `,` | Settings | WP-121 (§5.5) |

Every section is built (the stubs of WP-121, `SectionStub.qml`, went
with WP-125).

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
selected on entry, so the detail is the **overview**: the setup card
while a setup step is left (`todaySetupSlot`, below), one sentence ("Seldon is
recording. 2 changes need you." / "… Nothing needs you."), or, while
the logbook has no case and nothing is open, the **first-run card**
"Seldon is recording. Nothing to do." with one line on what Seldon does
from here (WP-119; a tile whose figure is 0 shows it in the muted tone),
a lead on what stays quiet, the active cases as tiles (id · risk, title, the plan's
progress, "2/4 steps · claude-code"; a click opens the case in Work) and
**New case**: one sentence → `seldon agent start --new --json --
<intent>` (`i` focuses it; the call and the result line Work's *Run*
shares; the field keeps its text until the engine has made the case). A
selected crisis shows the event (`EventDetail.qml`, as in the Changelog);
resolved here, it leaves NEEDS YOU and stays shown with the engine's answer.

Before the logbook exists (status `notInitialised`, WP-138, ADR-0047) the
list shows what the machine remembers on its own, from `seldon preview
--json` (read-only, its own process beside the queue; asked when the
status becomes `notInitialised`, when the engine probe answers, and when
the desk opens if the last answer is older than 5 minutes; never in dev
mode): **PACKAGES · LAST 7 DAYS**, one row per pacman transaction
("Upgraded linux, linux-headers, mesa and 11 more", the day and time and
the command line, `failed`/`interrupted`/`unfinished` aside), and
**EDITED CONFIG FILES**, one row per file under `~/.config` (path, day and
time); a group
without rows says so, a source not read says why. The search finds any
listed package. The setup card's step 2 says what was found ("The last 7
days: 6 pacman transactions and 4 files edited under ~/.config.") or why
nothing is, then "This is without memory: no who, no why, gone when the
logs rotate."

**Setup card** (WP-119; ADR-0033, ADR-0045 §6, the prototype debate's
A8; `components/desk/SetupCard.qml`, `Model.setupCard`). Until Seldon is
set up, the three setup states are one card in Today's overview instead
of three notices: **engine → logbook → snapshots**. The banners stay the
model behind it (`bannerFor`, `snapperBanner`): a step's buttons go to
`Service.fix` with the banner's id, as the notices' did, and a step is
`ready` only while its banner is up. The card:

- Headline "Set up Seldon · N of T steps to go", N the open steps, T the
  steps this machine has; " (optional)" when only the snapshot step is
  left. T is 3 until the index says otherwise: the snapshot step is
  missing (T = 2) when the index's snapper collector is off or its
  message is the engine's `snapper is not installed`
  (`Model.SNAPPER_NOT_INSTALLED`, `NOT_INSTALLED` in the engine; a value
  the plugin keys on, CONTRACT.md rule 10), and done when it reads
  snapshots. One lead line under it.
- The card's surface is the normal fill with a border in the accent's UI
  tone, as a notice's frame; never the selected fill, which is the
  cursor's (§5.3 "one cursor highlight"). Its text takes the theme's
  tones (§7): the current step in `accentText`, done steps in the
  foreground, waiting steps and captions in `dim`.
- Steps, numbered, done ones ticked (✓, "· done"), the waiting ones
  quiet; only the current step has buttons, its command (small, as
  *Copy* copies it) and its line: (1) **Install the engine** — the
  engine-missing banner's text, *Install* (the install terminal) and
  *Copy*; (2) **Create the logbook** — "Creates <folder> and starts
  recording; the last 90 days become history “before Seldon”. No
  questions, no password." (the folder the engine names: exit 3's
  `path`, else the notInitialised index's `logbook.path`, "~/…"), the
  preview's line (above), *Create logbook* (`seldon init --defaults` in
  the init terminal) and *Copy*. When exit 3's `reason` says the folder
  cannot be used (CONTRACT.md rule 10), step 2 says so instead ("~/Seldon
  holds other files, so Seldon does not create its logbook there. Choose
  another folder: seldon init asks where.") and offers **Choose a
  folder**: the sixth terminal script, `INIT_ASK_SCRIPT`, plain `seldon
  init`, which asks only where (`Model.INIT_ASK_FIX`, `Service.fix`'s
  `initAsk`), and *Copy*; (3) **Read
  snapshots (optional)** — the snapper banner's sentence, *Grant* (the
  grant terminal; its tooltip says what the grant gives), *Copy* and
  **Not now**. No *Check again*: after a step's terminal opened (from the
  card or from a notice) the service looks again by itself every 5 s for
  at most ten minutes (`Service.setupWatch`): the engine probe for step 1
  (every 5 s for two minutes, then every 30 s: each failed probe is a line
  in the shell's log, about 40 at most), the index for step 2 (the
  FileView watches it too) and `status --json` every 30 s (its exit 3
  tells why an init failed), a capture every 30 s for step 3 (only a
  capture rewrites the collector row; the grant script's own capture
  comes first). Meanwhile the step's line says "A
  terminal opened. This card moves on by itself when the step is done."
  A desk opened while the engine is missing probes it once.
- An engine older than `engineMin` takes the status banner's place
  (§5.6): the logbook step is not ready and says "First: Engine too old
  (the notice above)." — an old engine would refuse `init --defaults`.
- While only the optional step is left (Seldon records), Today's
  sentence ("Seldon is recording. 2 changes need you.") and its lead stay
  under the card.
- **Not now** is final: stored once as `setupSnapshots: "not-now"` in the
  plugin's `shell.json` entry through the desk's one settings path
  (§5.5); the step, the card (when nothing else is left), the chip and
  the snapshot notice go, and nothing asks again. Only Settings › Capture
  offers it again. Without a bar entry, or when the shell refuses the
  write, it holds until the shell restarts, and Settings › Capture says
  so (§5.5's no-entry sentence, or "The shell did not take the change; it
  holds until the shell restarts."): the key is not in Omarchy's bar
  settings. This exception is accepted and documented (orchestrator,
  2026-10-10): the desk without the pill is rare.
- It is null — and the notices show the banners as before — when the
  engine that was there is gone (an index exists: the urgent "Seldon
  engine missing"), for an index missing or unreadable, and for a
  contract mismatch. Dev mode shows it from the fixture; its buttons open
  the terminals, its own captures are refused as every engine call is.
- No mode logic: EASY (ADR-0045 §6) shows this same card. Its open steps
  are what WP-179's `Model.needs` counts.

When the card is done the first-run card (above) follows. The header's
chip (`Model.deskChip`) shows the card's headline (accent) while it is
up, "+N" for the notices beside it, and a click leads to Today's overview
with the card (`Desk.showSetup`) instead of folding the notices — unless
a notice is urgent ("Engine too old", "Seldon engine missing", a contract
mismatch): the urgent one takes the chip, in its tone, and the click
folds the notices as before.

#### Changelog (2)

The list: the triage slot (`triageSlot`, WP-124b; ADR-0034 §6,
ADR-0036) — **Agent sorts N open changes** (N = the open changes, a group
once) while something is open and the engine can write
(`Service.triageButton`; whether a default agent exists only the engine
knows, so a refusal names the fix under the button): `agent ask triage
--json`, "Starting an agent…" with a spinner while it runs, then the
engine's answer or refusal; and, while the index names a proposal
(`index.triage`), its row "Proposal · N items proposed by <actor> at <at>,
C crises held back — apply each below", which shows the proposal in the
detail (below). Then the chips **open · crisis · attention · routine · in
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

The proposal (`TriageDetail.qml`, WP-124b): the file `index.triage.path`
names, next to `index.json` (CONTRACT.md rule 1; exactly
`proposals/<id>.json`, `Model.triagePath`), read by a FileView and checked
against `proposal.schema.json` (`Model.parseProposal`: only its
properties, `logbook` present, `at` and `applied` date-times, the length
limits, a link without title or intent and an explanation without a case,
at most 4 MiB of text; a file off in any part is not shown: "could not be
read as the engine writes it"). **Bound to the proposal the user opened**
(round 2): its row (or *Review the new proposal*) stores that id
(`Changelog.seenProposalId`), and Apply and Discard name it. When the
index names another proposal by then, the bar says "Replaced by a newer
proposal by <actor> at <at> — review it", offers *Review the new
proposal* and has Apply and Discard off, the items are not shown, and the
service refuses the old id even when asked directly
(`Model.triageSeen`, `Service.triageCall`); a proposal that is gone
("Proposal <id> is not there any more …") keeps the last answer about it
shown. Only the answer about the opened id is shown. The sticky bar:
*Apply proposals (N)* (N the items Apply takes that are still open; one
click, `drift apply <id> --json`; enabled also with nothing open, so a
second run says what it skipped) and *Discard* (it removes the agent's
unapplied work, so it arms: "Confirm discard", hint "Discard proposal
<id>? Click Confirm discard. The logbook does not change.", then `drift
discard <id> --json`); **under its buttons, as the bar's hint, the line
"N items proposed by <actor> at <at>, C crises held back — apply each
below"** (plain text, wraps). Below the bar: the state ("<actor> ·
proposal, nothing written yet", or "Applied <at>. That marks the run, not
every item: what is still open shows below."), the last run's answer
("Applied N · skipped S · refused R"; a gone proposal: "… The proposal is
gone; the list shows what is open now" — refresh, never retry), the line
"Apply re-reads the file and every reference. If the file was changed
since you opened it, what Apply writes can differ from what is shown
here."; then **CRISES — EACH ON ITS OWN**: every
item that is a crisis by the file's flag or the index's class, with
*Apply this crisis* (`drift apply <id> --item <eventId> --json`, one per
run); then **WHAT APPLY TAKES**. The items are built only while the pane
is shown, by an asynchronous Loader (a 200 × 10 proposal: the click
returns at once, the list builds in slices). Each item: the change's
subject (from `index.events`), "Link to C-…" or "Explain: <title>" with
the intent, the outcome of the last run (Done, Skipped: <reason>,
Refused: <reason>, the engine's words) or "No longer open: nothing to
apply.", and every evidence ref — its kind and ref, then the engine's text
with "by <authors> ·" first (every author, ADR-0036 §2), wrapped (also
inside a long path), never clipped; an item with evidence that names an
agent or `unknown` among its authors (a Plan "worked by agent:…" too) is
marked "Read twice: some evidence names an agent or an unknown author."
and that text drawn in the accent colour. Every text is plain text
(CONTRACT.md rule 6; `model.test.js` checks every `Text` of the file).

The detail (`EventDetail.qml`; prototype `eventDetail`): the sticky bar
(`Model.eventActions`) — open drift: *Ask agent* first while the engine can
write (WP-124b; `agent ask drift <id> --json`, one click; the engine's
answer or refusal under the title),
*Link to case…* (*Link to C-… …* when the engine proposes one), *Explain…*,
*Dismiss…*, and for attention *Hide* / *Show*; an event with a case: *Open
case* (Work with the case selected; for a case the index no longer lists
— it keeps the last 50 completed — one line says so and offers *Open in
editor*, whose engine answer replaces the line: only the engine can tell
whether the file is still there); routine: none; the class at the right.
Then "source · kind", the full subject, its class, for a crisis the **Why
loud?** callout from the engine's rule: the drift item's `rule` (for a
group its leader's; ADR-0038 §1), so a click starts no process; an index
without it (an earlier contract-2 engine) makes the detail ask `seldon
drift show <id> --json` (in CONTRACT.md's table, read-only; for a group
its leader) once for a selected crisis and keep the answer while the item
stays a crisis (`Service.driftRules`). `always-red` → "A
package on your crisis list ([drift] alwaysRed in
~/.config/seldon/config.toml) was installed, removed or downgraded by name
in this transaction."; `always-red-paths` → "The path matches your crisis
list ([drift] alwaysRedPaths …)."; `pacnew-red` → "pacman left a .pacnew,
.pacsave or .pacorig beside a file that boot or login depend on
(mkinitcpio, Limine, PAM); check it with pacdiff before the next reboot."
(WP-141, ADR-0042); `attention-all` → "[drift] attention =
"all" is set: every change without a case is open drift, and a crisis is
a change in the red zone."; another rule is named as it is. Until the
answer (and in dev mode, without an engine, when the index has no rule)
it says only what the index proves: "The engine classed this <source> change as a crisis" and how to
ask for the rule. Then, from `proposedCase`, "C-… plans it (its plan names
this change); nothing has linked it yet." or "No open case plans it, and no
case is linked." — the Case row ("proposed: C-…") and the Rule row
("crisis · rule … · planned by C-…, not linked" or "· no case") say the
same. The key/values When · Who · What (· Command · Transaction) (· Commits) · Case · Rule · Source · Zone ·
Resolved · Event (Command and Transaction, WP-137: a pacman event's
`meta.command` and its transaction — "4 packages: 1 removed, 1 installed,
2 upgraded", then the status when it did not complete and "left N files"
for the files pacman left in it, WP-141's notes, which show the same row
by their `meta.transaction`; Commits, WP-136: a plugin update's `meta.commits` as
plain text, one subject per line, keyed "Rolled back" when `meta.git` is
`rollback`; absent when the event has no such string;
values wrap at word boundaries; a longer token breaks
anywhere; a detail the index clipped — the event's `meta.truncated` or the
drift item's `truncated`, contract 2 — reads "(clipped in the index; the
ledger has it in full)"; for a file pacman left — a pacman `note` whose
subject ends in `.pacnew`, `.pacsave` or `.pacorig`, SPEC-ENGINE §4,
WP-141 — a row **Hint** after What: "Merge with pacdiff (from
pacman-contrib) in a terminal. Seldon does not read that file, so it cannot
tell whether that happened since." — text only, never a button or a
command the plugin runs, AGENTS.md §8; for the change of pacman's ignore
list — a pacman `note` with `meta.ignorePkg`, ADR-0052 — the Hint
"pacman's full upgrade skips the packages in IgnorePkg and IgnoreGroup;
`pacman -S` still updates them. The list is in System."), **the transaction** (WP-137): for a
pacman event whose transaction did not complete (`meta.txStatus`,
ADR-0043) an urgent callout above the key/values — *Transaction failed*,
*Transaction interrupted* or *Transaction did not finish*, with what
that means (the packages listed may be all it changed; a failed one could
not be installed, upgraded or removed), then the steps — for `failed`
and `unfinished` "pacman's after-update steps (boot image, boot menu,
Omarchy's resume hooks) did not run for this transaction; if
omarchy-settings was in it, Hyprland's auto-reload may stay paused for
this session.", for `interrupted` the same with "may not have run for
every package of this transaction" — and one shared tail: "Before you
reboot, reinstall the packages marked ↑ or ↻ below (`pacman -S` with
their names): that runs those steps for them; a plain rerun does not. A
package marked − stays removed; for one marked ↓, or when unsure, ask
your agent in a case." (alpm-hooks(5) CAVEATS: post-transaction hooks do
not run after a failed transaction or a killed pacman, and run only for
a transaction's targets; for `interrupted` not verified against
libalpm's source; WP-137 rounds 2 and 3.) Text only: the plugin runs
nothing — whatever the event's class; then, below the key/values, "N packages: … in this
transaction" and every package of the transaction the index lists, the
unusual first (↓ downgraded, − removed, + installed, ↑ upgraded, ↻
reinstalled; by name within a kind), "name  old → new" or "name
version", the selected event's line in bold; shown for two or more
packages, a status or files left (one completed package says no more
than the rows); a transaction whose oldest line is the index's oldest
event at its 500-event cap (CONTRACT.md rule 4) says older lines are in
the ledger. Without `meta.txStatus` nothing is marked and nothing claims
the transaction completed (an index of an earlier build, or a line
written before ADR-0043). The Changelog row (and Today's NEEDS YOU row)
of such an event shows the status as one word in the urgent colour
before its meta line (`ListRow.alert`), and the sidebar search finds the
word; the class, the stripe and the counts are unchanged. Then a
group's members (`seldon drift show` for those the index no longer lists;
hidden when the transaction above lists every open member),
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
"COMPLETED · 1 / 2"); *Import tasks…* (WP-102b, below); *New case* (`+`);
the engine's answer to the last case action (after an import: "Imported N
cases: C-… · M tasks skipped"). Then the cases by group — ACTIVE ·
VERIFICATION · QUEUED · COMPLETED (completed and dropped, the index's last
50) — with "id · imported · risk · area · closed by agent · reopens … · N
proposed" (`imported` from the tag, CONTRACT.md rule 8) and the plan's
steps.

The detail: the sticky bar by status (`Model.caseDeskActions`) — queued:
*Start*, *Drop*; active: *Hand to agent* (`agent start <id> --json`), *To
verification*, *Drop*; verification: *Complete*, *Drop*; completed:
*Reopen* (`plan reopen <id> --json`); every case *Open in editor* (`e`,
`open <id> --editor --json`), then *Ask agent* while the engine can write
(WP-124b; `agent ask case <id> --json`, one click, any status; the agent
gets no case to work; the answer or refusal under the title); id · risk at
the right. Every writing
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
Reopens · Proposed · File · Imported from (an imported case's `source`,
text only, ADR-0038 §3), INTENT and RESULT (the index's `intent` and
`result`, the sections' first paragraphs, plain text; each hidden without
it), PLAN (the steps' progress; "The steps, the Intent and the Result are
in the case file.", or "The steps and the full Intent and Result are in
the case file." when either text is shown, with *Open in editor*), LOG
(this case's lifecycle events and notes in the index, newest first, with
the risk a `case-created`/`case-started`/`case-updated` line carries,
contract 2) and
LINKED CHANGES · N (the case's `events` the index still lists, "+N older
changes the index no longer lists").

One agent per case (WP-156, ADR-0041): the service knows which cases have
an agent window open, or an agent starting (`agent sessions --json`, its
own process beside the queue, like `doctor`; asked when the desk opens,
after every `agent` or `open` answer, when the index changes and every 15 s
while the desk is open, `Model.SESSIONS_POLL_MS`). On such an active case
the bar shows **Focus** (`agent focus <id> --json`; key `a`; runs at once,
writes nothing; Enter still never takes it) in place of *Hand to agent*,
"agent working" before id · risk, the Agent row reads "working now ·
<actor> · workspace <n>" (or "starting · its window is not open yet"), and
the case's row says "agent working". Without Hyprland the engine tracks
nothing: the desk keeps *Hand to agent*, and the busy state and the 2 s
floor are what stop a double launch. A stale answer heals itself: the
engine's "already working" refusal and its "no agent is working" both ask
again. While a call is in flight its button is busy and disabled
(*Starting…*, *Focusing…*, *Opening…*), as Run and Capture are; an open
is refused while one is pending and the same target is not sent again
within 2 s of a successful open (`Model.OPEN_REPEAT_MS`), for every
*Open in editor* on the desk. *New case* puts the 0.1 sheet in the
detail (`NewCaseSheet.qml`: title, zone, risk, priority, an optional area
slug refused in the plugin when malformed → `plan new --zone <z> --risk
<r> [--area <a>] [--priority <p>] --json -- <title>`; Enter creates; the
fields keep their text until the engine has made the case; Esc closes the
sheet with its draft, `+` brings it back). "Back to active" (prototype) is
not built: no engine verb moves a case from verification to active.

*Import tasks…* (WP-102b, ADR-0034 §2; `components/desk/ImportForm.qml`)
puts a form in the detail: a line on what happens ("Each open - [ ] item
becomes a queued case; … The file is only read. An imported case is started
by you, after you have read its whole Intent."), the path field (from the
home `~/…` or absolute; `Model.importPathError` refuses, before any call,
an empty path, one with a character of `BAD_PATH_CHARS` — the engine's
`bad_path_char` set: control, direction and format characters (WP-140's
set, the tags included), U+2028, U+2029; both sides are tested against
`fixtures/bad-path-chars.txt` —, a relative one, one longer than 4096 characters and one that does
not end in `.md` — the engine checks the rest: under the home, outside the
logbook, a regular file, its size and encoding) and an optional area slug.
*Dry run* (Enter in a field) sends `seldon import task --json --dry-run
[--area <a>] -- <path>`, the path one argument after `--`; the form lists
what would be created (title; status · source · "changed since C-…") and
what is skipped ("Skipped ~/…#5: done (- [x])", "already imported
(C-…)", "no text", "the same text again", "too long to review in the
desk (over 64 KiB)"), and the line "Would create N cases · M tasks skipped
· K invisible characters dropped". Only then, and only for the path and area that
dry run was for, *Import N cases* is enabled: one click sends the same
without `--dry-run`. The engine's refusal shows in the form, urgent; the
fields keep their text (Esc closes the form with them). After the import
the form closes, its fields empty, and the first new case is selected. One
import at a time (Service.importTasks); every text from the engine is plain
text (rule 6).

An imported case (tag `imported`): its detail asks the engine for the
whole Intent — `seldon plan show <id> --json`, read-only, again with every
new index, and once more for an index that arrives while an answer is in
flight (that answer enables nothing); while it asks again the last text
stays on screen, unchanged text is not laid out again, and Start is off
until the answer — and shows, under IMPORTED TASK · N lines, the accent line
"From ~/…#N. Read the whole Intent before you start the case: once
started, an agent acts on it without asking. Only you start it." and the
engine's `intent.text` as plain text in the system's monospace font
(`Style.font.family`) in a bordered box, never rendered as Markdown; its
first line is the engine's provenance line; a direction or format
character the case file holds shows as `‹U+XXXX›` (the engine marks it,
`hidden`), with "N hidden characters are marked ‹U+…› above: text you
cannot see in the file. Read the case in the editor; start it from the
terminal."; "The first 64 KiB are shown; the rest is in the case file.
Read the whole Intent in the editor; start this case from the terminal."
when `truncated` (an imported Intent normally is not: `import task`
skips a longer one, ADR-0044; a later pattern change can cut it, and Start
then stays off). While it loads, or when
the engine withholds it (`intent: null`) or cannot be asked (dev mode),
the block says so and the index's first paragraph (INTENT) stays. **Its
Start never fires from the list or a key** (ADR-0027 §2(a); Fable,
WP-102 round 3): Enter has no action on an imported case (neither Start
nor, in its place, Drop); the bar's *Start* is enabled only while the
detail shows the whole Intent the engine gave for this case, not cut and
with nothing hidden (`Model.intentReviewed`: `truncated` false, `hidden`
0; ADR-0044), and it arms by click only ("Start C-…? Click Confirm.");
until then the bar's hint (`Model.reviewHint`) reads "Start waits until
the whole Intent below is shown; only you start an imported case", or, for
a cut or marked Intent, "N hidden characters are marked and the Intent is
longer than the desk shows: read the whole Intent in the editor; start
this case from the terminal." `press()` refuses such a Start too, behind
the bar. The engine
refuses an agent's start of an imported case anyway.

#### Decisions, System, Memory (4–6; WP-123)

The three reading sections share `sections/ReadingSection.qml`: a list of
rows from a `Model.js` row function, narrowed by the sidebar search
(`Model.deskFilter`: every word, case-insensitive, over the section's
fields), and the detail of the selected row under its sticky action bar.
The selection is the cursor: `↑`/`↓` `j`/`k` move it and the detail
follows; a click selects; `Enter` shows the detail in the stacked layout.
It stays on its row by id across index updates; a row that is gone (or
filtered out) leaves the first row shown. Row and detail models are
bindings on the index (one evaluation per index change, as the 0.1 tabs);
the index carries no decision body and no memory text, so the details
show what it has and the editor shows the rest (AGENTS.md §3: the plugin
reads only the index). Without an index the lists say "No index to show"
and no action runs.

- **Decisions (4).** Rows from `Model.decisionRows` (newest first by id;
  title, "id · status", the date at the right; a proposed one with the
  accent stripe); above them the count ("4 decisions · 1 proposed") and
  *New decision*. The detail: "ADR-NNNN · status · date", the title (a
  superseded one struck through), for a proposed one what Accept means,
  the first paragraph of its *Decision* as plain text (`decisions[].lead`,
  ADR-0038; hidden without it, and the closing line then reads "The text
  is in the file; …" instead of "The whole text is in the file; …"),
  Status / Date / File, and a CASES · N block from `decisions[].cases`
  (contract 2, ADR-0034 §5) with each case's title and status from the
  case lists (a case the index no longer lists by its id; a click goes to
  it in Work; "This decision names no case." for an empty list); an index
  without the field hides the block.
  Sticky bar: *Accept* (only while proposed, primary) and *Open in
  editor*, the id at the right. **Accept writes (WP-135, ADR-0040)**, so
  it arms: the first click arms it — the bar reads *Confirm accept* and
  shows "Accept ADR-NNNN? Click Confirm: it becomes accepted with today's
  date." — the second runs `seldon decide accept ADR-NNNN --json`
  (`Service.acceptDecision`, id validated; one accept at a time, the busy
  text of §3 otherwise). Any key, another selection, another section or
  a new index disarms. Accept has no key of its own; it is disabled while
  nothing can write (dev mode, no engine, not initialised) and while an
  accept is pending. The engine's answer ("Accepted ADR-NNNN · title",
  "ADR-NNNN is accepted already" or its refusal) shows at the top of that
  decision's detail; the accepted decision arrives with the index (no
  Accept any more). Open in editor writes nothing and does not arm. `e`
  opens; `d` or *New
  decision* shows the form (`components/desk/NewDecisionForm.qml`) in the
  detail pane: title → Enter arms ("Press Enter again: create the
  decision “…”"), Enter again (or a click on *Create*) runs `seldon decide
  --no-edit --json -- <title>`, then the service opens `<newId>` from the
  answer; any change to the title disarms; the title is kept on refusal;
  the busy text of §3 while another decision is pending. Esc in the form,
  or a click on a decision, leaves it with the title kept; `d` brings it
  back; once the index lists the new decision it is selected and the list
  head says "Created ADR-NNNN · title".
- **System (5).** Seven tiles from `Model.systemTiles`, each with a big
  value: Omarchy (version; theme, last update, checkout, plugins),
  Packages (installed; explicit, AUR), Snapshots (the newest number; the
  index's list, at most 10), Deviations (the count; the list is in
  STATUS.md), Collectors ("ok/enabled"; each collector, then machine,
  engine, index time and the logbook's areas), Recently edited (WP-139,
  ADR-0046: the count of `system.recentConfig.files`; Scanned), Ignored
  by pacman (WP-165, ADR-0052: the count of the names in
  `system.pacmanIgnore`, "ignored"; IgnorePkg and IgnoreGroup). Every
  field of `index.system` is optional: a tile without its data shows "—"
  and "Not in the index"; a failing collector stripes the Collectors tile
  and its lead says so. The detail: the big value and unit, the lead, the
  key/value rows, where the data comes from (the tile's `source`; "From
  the dossier; rebuilt on every capture." for the first five). Sticky
  bar: *Open in editor* (`seldon open status --editor --json`, the full
  report); `e` the same.
  Recently edited's detail lists the files as the engine wrote them,
  newest first (`Model.recentFiles`; a row whose path is not a plain
  `~/.config/…` path without control, direction or invisible characters
  and `.`/`..` folders is left out): the path as plain text (elided in
  the middle), "<age> · not watched", and *Watch*, which runs `seldon
  config watch --json -- <path>` (`Service.watchPath`; the path one
  argument after `--`, `Model.watchArgs`; one at a time, the busy text of
  §3 otherwise; disabled while nothing can write). The engine's answer
  ("Watching <path> from the next capture on; it is taken as it is,
  without an event", "<path> is watched already" or its refusal, a held
  lock included: no retry) shows above the list while the tile is
  current, also after the last row went; the row goes with the index the
  engine rebuilds. Its footer: "From the last capture's scan of
  ~/.config: paths and times only, never content. Seldon keeps no record
  of these edits until a path is watched." With no file in 7 days the
  lead says so — unless the scan stopped early (`partial`, ADR-0046 §2):
  then it reads "The scan stopped early; the list may be incomplete.",
  which a non-empty list's lead also ends with. An index without the
  field shows "—".
  Ignored by pacman (`Model.pacmanIgnore`): the lead "pacman's
  full upgrade skips them; `pacman -S` still updates them.", with no
  name "pacman ignores nothing: no IgnorePkg or IgnoreGroup in
  pacman.conf.", and for a partial list " Part of pacman's configuration
  could not be read; the list may be incomplete." after it; the rows
  IgnorePkg and IgnoreGroup with the names comma-separated as plain text
  ("—" for none), and with hidden names (`hidden`, plus any name not of
  the engine's shape) "Not shown" · "N names (not a plain package or
  group name, or masked by your redaction)", which the big value counts
  too; no stripe and no action. Its footer: "From pacman.conf
  and the files it includes, read on every capture: the IgnorePkg and
  IgnoreGroup names only, nothing else of the files." An index without
  the field shows "—".
- **Memory (6).** Rows from `Model.memoryRows`: the `## ` headings of
  memory/lessons.md, then the memory topics with path and `updated`
  (`summary` "3 lessons · 2 topics" above them). The detail: "Lesson" or
  "Topic", the text, File (and Updated), "Every agent reads this at the
  start of a session." Sticky bar: *Open in editor*, which opens the
  logbook folder (`seldon open logbook --editor --json`) until the engine
  gains a memory target; `e` the same; nothing from the index reaches the
  argument list.

**Graph (8; WP-125, ADR-0034 §5).** Solo. The machine's memory as a
network, from the index alone; `sections/Graph.qml` with
`components/graph/GraphCanvas.qml`.

- **Data.** `Service.graph` = `Model.graphBuild(index, GRAPH_CAP)`
  (empty while the index means nothing in the status), built only for a
  shown section 8: an index change marks it dirty (`graphDirty`), and the
  section calls `graphRefresh()` when it is shown and when the graph gets
  dirty while it is shown (`graphBuilds` counts the builds). A build
  costs about 5 ms of QV4 on 500 events, which no capture pays while the
  graph is not on screen. Ids are looked up in maps without a prototype,
  and a case reference (`event.case`, `drift.proposedCase`,
  `decisions[].cases`) must match `CASE_ID` before it links: a foreign
  index's `constructor` or `ADR-0003` as a case links nothing. Nodes: the logbook's areas (`system.areas`, and any area a case
  names that the list lacks), the cases of all four lists, the decisions,
  and the events whose kind is a change (`Model.GRAPH_CHANGE_KINDS` and
  `plugin-*`, and a pacman `note` — a file pacman left, WP-141; not case
  lifecycle, other notes, corrections, resolutions, state loss): the index's events, then open drift items it no longer lists
  among them. A change is a crisis when its id is in `drift[]` with
  `crisis: true`. Edges, once each (a solid one wins over a dashed one):
  `event.case` → case, `case.area` → area, `decisions[].cases` → case
  (contract 2; an index without the field has none), `drift.proposedCase`
  → case dashed. Day index: an event's date (`ts`), a case's `created`
  (else `started`, `closed`, today), a decision's `date` (else today); an
  area takes its earliest neighbour's day, one without a neighbour the
  first day; days count from the earliest. **Cap 400** (`GRAPH_CAP`):
  beyond it changes fold into cluster nodes ("+N") — by day and source,
  else by day, ISO week, month: the finest level that fits, the biggest
  groups first and only as many as the cap needs. Areas, cases, decisions
  and crises never fold. A cluster carries its members' links; its card
  lists up to 12 of its changes, newest first. **More fixed nodes than
  the cap** (areas, cases, decisions and crises together over 400):
  `build.still` — a still picture in node order (the start layout: a
  node beside a placed neighbour, else on the spiral), no force step and
  no tick ever (ADR-0034 §5's static escalation), the caption says "A
  still picture: N areas, cases, decisions and crises are more than the
  400 nodes the layout moves"; hover, drag (the node moves at once),
  pan, zoom and the replay's cut still work.
- **Layout.** `Model.graphState(build, prev)` (plain arrays: QV4 reads
  them faster than typed ones; positions kept by id across index
  updates; a new node starts beside a placed neighbour, else on a
  sunflower spiral; deterministic) and `Model.graphStep(state,
  budgetMs)`: one force iteration — repulsion (each pair exactly up to
  160 visible nodes, a Barnes–Hut quadtree with θ 0.9 above), a pull to
  the centre, springs along the edges, all scaled by alpha, which decays
  from 1 to 0.001 over 200 ticks; then the layout sleeps (the state
  counts which repulsion ran, `exactSteps` and `treeSteps`: the tests
  hold 400 nodes to the tree). `graphWarm`
  runs the functions on a six-node graph once, so the first real tick
  is not interpreted. The service keeps the layout (`graphLayout`): a
  reopened desk shows it settled, without a tick.
- **The shell thread** (ADR-0034 §5's budget). A Timer of 34 ms (≤ 30
  Hz) steps the layout only while section 8 is shown in the open desk
  and the layout is awake. A tick is one `graphStep` plus the drawing
  calls of its paint: `tickMs` = `stepMs` + `drawMs`, at most 8 ms;
  every tick is timed (`tickMsMax`, `ticksOver`, the first five slow ones
  in `slowTicks`). The Canvas rasterises on its own thread
  (`Canvas.Threaded`; `paintMs` until the picture is there), one path per
  node (one path with 400 antialiased discs took 20 ms to fill, 400
  paths 2 ms), and a paint allocates nothing on the JS heap but the
  focus's neighbour set. Dragging a node and the replay wake the layout
  (alpha at least 0.3, the tick count from zero). ADR-0034 §5's "drag,
  pan, zoom, hover and replay wake it" is read as: pan, zoom and hover
  repaint (the layout stays asleep) — they move no node, so a tick would
  change nothing. TESTING.md has the measurements.
- **Screen.** Row 1: "Graph", the caption, *Play growth* (*Pause* while
  playing), the date slider (the cut-off day: nodes of later days are
  hidden and take no part in the layout) and "YYYY-MM-DD · N nodes [of
  M]". Row 2: the legend — Case (accent disc; closed cases at 50 %), Area
  (a foreground ring), Decision (a square, foreground at 72 %), Change (a
  dot, foreground at 42 %, larger with more links), Crisis (the urgent
  spindle), Folded (a ringed dot, only when something folded) — and the
  keys while there is room. Then the canvas, and the footer: "Newest N
  events · M completed cases in the index", with " · older ones are only
  in the logbook" once the index is at its limits (500 events, 50
  completed cases), and " · K changes folded into G". Until the view is
  panned or zoomed (or a node dragged) it fits the visible nodes; `0` or
  a fit returns to that. A node keeps a few pixels on screen however far
  out the zoom is. Labels: areas and crises always; cases, decisions and
  folded groups from zoom 0.5 once the layout rests; at most 40, by that
  priority; a change only with the focus. A label that would leave the
  canvas at the right goes to the left of its node; labels stay inside
  it vertically.
- **Pointer and card.** Hover lights a node and its links (the rest at
  25 %) and shows its card at the top right: the title, "Kind · since
  YYYY-MM-DD · day N · M links", status · risk · area (a case) or source
  · kind (a change), a folded group's changes, and *Open case* for a case
  or a change linked to one (Work with the case selected). The card stays
  on the last hovered node while the pointer travels to it; a click on
  the background or Esc lets it go; a click on a node keeps it, as
  `select <id>` does. Dragging a node holds it at the pointer and the
  rest follows; dragging the background pans; the wheel zooms at the
  pointer (0.15–4).
- **Replay.** *Play growth*, Space, Enter or `p` play from the first day
  (or on from the cut when it is before the last day) in about 50 steps
  120 ms apart; a node that appears starts beside a visible neighbour;
  again pauses. `←`/`→` move the cut one day, the slider sets it. The
  slider's knob does not animate while playing (a running QML animation
  throttles the shell thread to the display's frames). `-` and `=` zoom
  out and in. Esc: pause, then let a kept card go, then the desk's order.
- Without an index: "No index to show"; with nothing to draw: "Nothing to
  draw yet: no areas, cases, decisions or changes in the index". The
  plugin reads only the index (§3, AGENTS.md §3); a `seldon graph --json`
  export over the whole logbook is deferred (ADR-0034 §5), so the footer
  says what the index holds.

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
  `config.toml`. Capture also names the snapshots (`Model.snapshotSetting`:
  recorded, not readable yet, put off with *Not now*, off, or no snapper
  on this machine) and, while the setup card's snapshot step is put off
  and still open, its one button **Offer again**, which takes
  `setupSnapshots` out of the entry through the same write (§1) and
  brings the step back to the card (WP-119).

### 5.6 Notices, header mark, pictograms

The notices under the header are the 0.1 panel's banners with their
one-click fixes — the setup states excepted, which the setup card in
Today shows while it is up (§5.4 "Setup card", `Model.isSetupNotice`:
the engine not installed yet, the logbook not created, snapshots not
readable, the last also after its *Not now*) — in this order: the
restart notice after a plugin update,
the status banner, the engine newer than the plugin, snapshots not
readable, the outdated agent rules,
what their update did, the capture warnings. Each is `Banner.qml` on the
service's object; a fix goes to `Service.fix(action, banner)`. Their
texts and fixes:

Banner states (under the header), each detail one sentence (WP-117):
engine missing → the GitHub one-liner while the AUR package does not
exist (§3, ADR-0024), afterwards `omarchy pkg aur add jax-seldon`
(ADR-0016; `omarchy pkg add` reaches the official repositories only),
with *Install*, *Copy* and *Check again*; without an index it is the
first setup step, "Install the engine" in the accent tone, with an index
(the engine was there and is gone) "Seldon engine missing" in the urgent
tone; contract
mismatch → `omarchy plugin update jax.seldon` when the plugin is older
than the index, the GitHub installer one-liner when the engine is older (until the
AUR package is live, ADR-0024), with *Update* and *Copy*; an index
of a newer contract that says this plugin can read it (`contractVersion`
above the plugin's, `contractReadableFrom` at most the plugin's,
ADR-0051) → no status banner (the status is what the index says; the
pill keeps its counts and colour) but the neutral notice "The engine is
newer than the plugin", "The engine writes index vN; this plugin reads
v2 — update the plugin.", `omarchy plugin update jax.seldon` with
*Update* (the plugin update's terminal script, as the mismatch banner's)
and *Copy*, not while the engine is missing; engine older than the manifest's
`engineMin` (§3; in place of every status banner but engine missing and
contract mismatch) → "Engine too old", "This plugin needs engine X or
newer and seldon reports Y.", the same installer one-liner with *Update*, *Copy* and
*Check again* (WP-068); snapshots
not readable (ADR-0026) → "Read snapshots (optional)", "A one-time read
grant on /.snapshots; it asks for your password once, and Seldon works
without it.", the one-line read grant
`sudo setfacl -m u:$USER:rx /.snapshots` with *Grant*, *Copy*
and *Check again*; the engine's message and, on its own line, what the
grant gives (read access to the snapshot directory listing and the
snapshot info files, no snapshot creation, change or deletion) are the
banner's hover text; *Check again*
runs a capture, the same call as *Capture now* (`capture --all --json
--quiet`, then `status --json`), because only a capture rewrites the
collector state this banner reads (reloading the index would not; WP-054);
not initialised → "Create your logbook", `seldon init --defaults`, with *Create logbook*,
*Copy* and *Check again*; index stale →
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

Terminal scripts (WP-117). *Copy* puts the banner's plain command on the
clipboard; *Install*, *Create*, *Grant* and *Update* open Omarchy's
presentation terminal (`omarchy-launch-floating-terminal-with-presentation`:
logo, the script, "Done!", the theme's gum colours) with the banner's
script, one of six constants in `Model.js` (`INSTALL_ENGINE_SCRIPT`,
`UPDATE_ENGINE_SCRIPT`, `UPDATE_PLUGIN_SCRIPT`, `INIT_SCRIPT`,
`SNAPPER_FIX_SCRIPT`, and the setup card's `INIT_ASK_SCRIPT`); the service launches nothing else
(`Model.terminalArgv`). Each follows Omarchy's own scripts: a bold `gum
style` line "Seldon: <what>", one paragraph (why; whether it asks for a
password), the command indented as *Copy* copies it, the command run in
`(set -o pipefail; …)`, then one line of what changed, green (palette 2)
on success, red (palette 1) on failure. A result line never claims more
than happened: after a failed install or engine update it says "The
install (update) did not finish. Run it again; your logbook is
untouched." (install.sh can stop after it replaced the binary). The
script then ends with status 0, so the wrapper's "Done!" follows. Ctrl+C
(or TERM) is trapped: the script skips a command that has not started,
prints a "Cancelled. …" line (palette 3) and ends with 130, Omarchy's
"cancelled" status, on which the wrapper prints no "Done!" and the window
closes, as with Omarchy's own scripts. After a successful snapshot grant
the script runs `seldon capture` (once more if the lock is held), which
rewrites the index, so the banner goes without a click; only when a
capture succeeded does it say "Snapshots are now recorded. The panel
updates by itself.", else "Read access granted. The snapshots were not
recorded yet; Seldon tries again at its next capture." (it reports, never
forecasts; the setup card's own capture then shows the truth, WP-117
stage 2). After an engine update it runs `seldon status`, so the new
engine rewrites the index. `seldon init --defaults` writes the index
itself. After an install the service probes the engine by itself (the
setup card's watch, §5.4), so the result line says "The engine is
installed. The Seldon panel finds it by itself."; after an engine update
it is probed on *Check again*, and the result line says so. The scripts are built once from string
literals: nothing from the index, the logbook or the environment is in
them (AGENTS.md §8); `$USER` stays literal in the shown command and is
expanded only where it runs, there as `${USER:?}` in the grant, which
stops before `sudo` when USER is empty (a grant `u::rx` would change the
owner bits). ADR-0026 holds: the engine never runs the grant, the user's
click runs it in the user's terminal.

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
the date and the counts (events today — "1 event today" — in 7 days, active, queued, and
"without a case", the attention count `openDrift − crisis`, the number the
tooltip and the Changelog line show), `Style.space(48)`: crisis (urgent) when any
crisis, else case active (accent) when active cases, else all clear
(foreground); none without an index. Attention alone changes nothing
(ADR-0028 §4b), so the drift-open pictogram is not shown.

### 5.7 Behaviour carried over from the 0.1 panel

Superseded for every section: sections 1–3 are §5.4 (WP-122), sections
4–6 §5.4 "Decisions, System, Memory" (WP-123); kept as the record of the
0.1 panel. Superseded by §5.1–§5.4 already: the
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
`explained · C-…: <intent>` (ADR-0021), `dismissed: <reason>`. Width `Style.space(460)` (WP-039; was 380 from WP-011, the
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

## 6. Prime Radiant — desk section 7

Section 7 of the desk (§5; ADR-0034 §4), solo (no list column): `7`, the
pill's middle click, or a payload `{"section":"radiant"}` — and the 0.1
overlay's `{"period":"30"}`, which implies the section, so an existing
binding still lands on the charts. The fullscreen overlay window, its
header and its own banner are gone (WP-121): the desk's header and
notices cover them. `sections/Radiant.qml` (WP-123) holds the charts
below, reused as they are (`components/overlay/`). The chart semantics
below stay normative. Layout: `Model.overlayGrid` of the section's own
size (its width and height minus `Style.spacing.huge` padding and the
first row), `Style.spacing.panelGap` gutters, the overlay's minimum slot
of `Style.space(240)` × `Style.space(120)`: wide, medium or narrow by the
width (the 960 px desk at 50 % gives medium), scrolling only when the
minimum heights do not fit (`↑`/`↓` scroll it then).

- Row 1: "Prime Radiant", the period's window ("90 d · 2026-07-04 –
  2026-10-01", "All · everything in the index") and the period selector
  (30 / 90 / 365 days / All; WP-030) with the hint "←/→ period" beside it
  while the row has room. 90 d on every entry of the section,
  unless the payload names a period (a summon of the open desk at section
  7 sets it at once). `←`/`→` and `h`/`l` walk the periods and wrap; the
  digits are the desk's sections (the overlay's `1`–`4` are gone); a chip
  click picks one and leaves the keys with the desk.
- Row 2 (full width): **Heatmap** — events per day of the period as ISO
  weeks × 7 days (53 × 7 at 365 d and All), five steps of the theme
  accent; hover shows the date and counts by source.
- Row 3: **Series** explicit and total packages over time (step lines,
  one lane each; a count holds until the next sample; left of the first
  sample in the period no line is drawn but the hover reads out that
  first sample) · **DriftBars** drift opened vs resolved per ISO week
  (the peak is the week with the most opened) · **RiskDonut** cases by
  risk, all time (the count in the centre, "all time" under it only where
  the hole holds it; the caption always says it).
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
service (`Model.periodTable`) when the index changes; the section only
draws (one paint per chart per data or size change; no aggregation on
the first frame after entering the section — the harness asserts it).
Nothing above the grid may change its height after the first frame: the
desk header takes its height from its fonts, not from its Rows' polish,
and row 1 from its texts and the selector. A notice that settles under
the header in the first frame (index stale, snapper not readable) still
moves the grid once; each chart then paints twice.
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
`foreground` at opacities, `Color.urgent` for crises and R3. The graph
(§5.4) has no token for the prototype's area and decision colours: an
area is a foreground ring, a decision a foreground square, a change a
foreground dot (shape and opacity tell them apart, as the Timeline's A12
markers do); a case is accent, a crisis urgent. Font from the bar. Test with
at least three Omarchy themes incl. a light one.

The house rules below (WP-183) hold for every surface; the checklist
`docs/skills/omarchy-ux/SKILL.md` points here.

**Colour.** Only the theme's five roles in `Color` (`foreground`,
`background`, `accent`, `urgent`, `muted`;
`$OMARCHY_PATH/shell/Commons/Color.qml`) and its surface groups; the
desk draws on `Color.popups.*`, so contrast is judged against that
surface. No hex literal, no colour read from a theme file. There is no
warn or success role: a surface that needs a fourth hue adds
`Color.pick("seldon.<key>", <role>)` in its own WP, with one line here.
A state never reads by colour alone; a glyph or a word goes with it
(WP-178), and a selection fill gets a second cue: an accent bar of at
least 3:1 and, on a row, a bold title (WP-177).

**Text tones** (WP-177). Text never takes `accent`, `urgent` or `muted`
as they are (primary text is the raw foreground, `Color.popups.text`), and
`Color.muted` is never a text colour. Secondary text, text in the accent
and text in the urgent colour take the tones `Model.deskTones` derives
from the active theme (`components/Tone.qml`: `dim`, `accentText`,
`urgentText`); a UI part that must be seen takes `accentUi` (the
selection bar) or `ui` (a ring, a line, the graph's cluster ring). The rule: the role is mixed
towards the theme's own foreground, in steps of 2 %, until it reaches
**4.7:1** for text and **3.2:1** for a UI part on every surface it sits
on. Mixing never adds a colour the theme lacks: no black, no white, no
fallback. The surfaces are the desk's, `Color.popups.background` (not
`Color.background`), with Style's normal, hover and selected fills over
it and the accent and urgent tints at the selected alpha (a notice, a
chip, the sidebar's current section). A `popups.background-alpha` below
1 is taken as opaque: the wallpaper behind it cannot be measured. Where
the theme's own foreground does not reach a target on a surface, the
tone is that foreground and the tests report the theme instead of
failing it. Only text and UI parts are mixed: stripes, pictograms, the
bars of state, charts and the pill keep the raw roles. The hairlines
between header, notices, list and detail are `Tone.divider` (the
foreground at 12 %). The tones are derived once per theme change
(`Tones.js`, a shared library) and checked in `tests/plugin/model.test.js`
on three committed themes (`fixtures/themes/roles.json`) and on every
theme under `$OMARCHY_PATH/themes/`; `tests/plugin/check-tokens.py`
fails on `Color.muted` as a Text colour and on a literal
`Util.alpha(…, <number>)` outside its allow-list: named colour properties
of the charts and the graph (data colours), each with its alpha.

**Geometry.** Rows and `qs.Ui` controls take `Style.cornerRadius`
(Hyprland's `decoration:rounding`, 0 by default), as Omarchy's own
`Ui/CursorSurface.qml` does. Only stripes, accent bars and heatmap cells
are square (radius 0); a progress bar counts as a bar. Three shapes
differ until WP-126's sweep: the list row's stripe and the progress bars
are capsules (`components/desk/ListRow.qml`, `components/desk/Progress.qml`,
`radius: height / 2`), and the banner card takes `Style.spacing.labelGap`
(`components/Banner.qml`) instead of `Style.cornerRadius`. Spacing and sizes come from
`Style.spacing.*` where a name exists (`controlHeight`, `rowPaddingX`,
`panelPadding`, …) and from `Style.space(N)` otherwise, so they scale
with the font; type sizes from `Style.font.*`, the family
`Style.font.family`. A control keeps its size in every state: the
widest border is reserved up front.

**States.** Interaction fills and borders come from Omarchy's state
tokens (`Style.*Fill`, `Style.*FillFor`, `Style.*BorderFor`,
`Border.controlSpec`), never a literal alpha. The defaults (`Commons/Style.qml`; a theme's `shell.toml`
may change them): fill normal 0.04, hover/cursor 0.08, selected 0.18,
pressed 0.22, selection 0.35; border normal 0.4, hover/cursor 0.25,
selected 1.0; focus follows hover (fill 0.08, border 0.25). With that
default, Omarchy's focus border (the foreground at 0.25, 1 px) reaches
1.4–2.0:1 on the 22 shipped themes (the lower of its contrast against the
popup background and against the normal fill), below WCAG 1.4.11's 3:1, so
Seldon's own controls draw their own ring (§5.3) and yield to a theme
whose focus border reaches 3:1.

**Motion.** Omarchy's values where the plugin animates at all: 140 ms
`Easing.OutCubic` for moves and fades (`Ui/PopupCard.qml`,
`Ui/KeyboardPanel.qml`), 60–120 ms for colour (`Ui/CursorSurface.qml`,
`Ui/Button.qml`). The desk has no animation today.

**Glyphs.** Only glyphs that Omarchy's `monospace` font (JetBrainsMono
Nerd Font) covers, or the plugin's `components/MaskIcon.qml`. `⏸ ↺ ↻ ⏰`
are not in that font (`fc-list ':charset=23F8'`, and likewise 21BA, 21BB
and 23F0, lists no JetBrainsMono face) and fall back to another font;
`↻` in `Model.js` (`TX_GLYPHS`) is replaced in WP-178.

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
  / "not found"). The Prime Radiant keeps the 0.1 overlay's names:
  `setPeriod <30|90|365|all>` shows section 7 with that period and
  returns the period now selected; an unknown id changes nothing — not
  the section either — and returns section 7's period ("" before its
  first visit);
  `hover "<slot> <x>,<y>"` (x and y fractions of the chart's plot) returns
  `{ slot, hover }`, the read-out at that point, and `hover ""` clears
  every chart's hover — while section 7 is shown, else `{ error }`; a
  malformed argument returns `{ error }` and changes nothing. With section
  7 shown, `view`'s `sectionView` holds period, window, caption, grid mode
  and area, `scrolls`, the aggregation passes (`service`, `section`) and
  the six slots (counts, window geometry, chart summary, numbers, empty,
  hover, paints, paintMs, plot size). Once section 8 has been visited,
`view`'s `graph` holds the graph's read-out — also while another
section is shown, so a check can see that nothing ticks there (null
before): nodes, edges, folded, clusters, numbers, visible, cut, span,
date, ticks (all), run (since the last wake), alpha, sleeping, timer,
stepMs, drawMs, paintMs, tickMs, tickMsMax, tickSamples, slowTicks,
ticksOver, flipped (labels the last paint drew left of their node), over
(steps over the budget), stepMsMax, paints, wakes,
playing, replay (the visible count after each step of the last replay),
hovered, pinned, cardNode, card, view `{ x, y, k, fit }`. `select <id>`
with section 8 shown keeps that node's card ("not found" for an unknown
id).
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
  a time (WP-067). The hand-over is deferred (Qt.callLater); nothing is
  enabled during teardown (WP-162).
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
Besides the engine it starts only `wl-copy` with one constant command and
Omarchy's floating-terminal launcher with one constant script (§5), only
on a banner click, and
`omarchy-restart-shell` without arguments on the restart notice's click
(WP-090). Never a
shell string built from logbook content. Writes no file itself; the one
change it asks for is its own settings entry in `shell.json`, through
the shell's facade (`updateEntryInline` with the plugin's own id, which
the facade enforces), on an explicit click or slider release (§5.5). Documented in README under
"Security, privacy, privileges" (WP-041).
