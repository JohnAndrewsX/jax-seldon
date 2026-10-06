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
Left click toggles Panel; middle click opens Prime Radiant; right click
runs capture.

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
list "+N more open drift items not listed here" when `summary.openDrift`
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
and badge from one source; once resolved, by the event's own zone. The
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
| Work | three columns queued/active/completed (last 50, scrollable) | "New case" (title + zone + risk + optional area/priority), start/verify/done/drop with two-press arming, Open in editor on every card; "Start agent" (runs `omarchy agent prompt` or the configured launcher with a prompt that names the case and the logbook) is WP-022 |
| Decisions | ADR list with status | "New decision" |
| System | omarchy version, package counts, deviations, snapshots, plugins, theme | "Open in editor" (rebuild/update-impact actions are Phase 3 engine commands, allowed by CONTRACT.md, not wired in v1) |
| Memory | lessons headings, memory topics | "Open" |

Header (WP-051): the A5 lockup — the mark, then "Seldon" in the heading
font (`Style.font.heading`, bold), baseline-aligned. Metrics from
`assets/DELIVERY.md` §5, derived from the heading's cap height (the tight
height of "H"): box = 2 × cap rounded to an even pixel count, the
wordmark `round(cap / 2)` after the box, its baseline `box / 2 + cap / 2`
below the box top (the mark's centre on the cap-height centre). At the
default font that is box 24, gap 6, baseline 18, the delivered numbers.
File by the box in device pixels: 24 → `a5-panel-mark-24.svg`, 32 →
`a5-panel-mark-32.svg`, anything else → `a1-icon-mask.svg` (e.g. 30 at
font scale 1.25). Colour: the panel foreground.

State pictograms (A11, 48 and 96 grids, never drawn below 48 px; by the
size in logical pixels — the vector scales with the DPR — the 48 grid up
to 72 px, the 96 grid above): the status banner shows its status's
pictogram, `Style.space(48)` square, left of its text, in the banner's
tone — engine missing → `engine-missing`, not initialised →
`logbook-not-initialised`, index missing (or unreadable) →
`index-missing`, index stale → `index-stale`; the contract mismatch and
the snapper banner have none. The Today tab shows the day's state left of
the date and the counts, `Style.space(48)`: crisis (urgent) when any
crisis, else case active (accent) when active cases, else all clear
(foreground); none without an index. Attention alone changes nothing
(ADR-0028 §4b), so the drift-open pictogram is not shown.

Banner states (top of every tab): engine missing → "Install the engine:"
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
*Capture now*; capture warnings → the neutral "Capture warned" notice of
§3 under the banners, without an action; plugin updated under a running
shell (§3) → the neutral "Restart the shell to finish the update" above
the banners, with both versions and one action, *Restart shell*, which
runs the argv `["omarchy-restart-shell"]` once per service instance
(a second click could kill the new shell; WP-090); crisis → red strip "N changes
that can affect boot, login or the shell have no case" ("1 change … has
no case"), only while `summary.crisis` > 0 (ADR-0028 §4b); a click opens
the first crisis.

## 6. Overlay.qml — Prime Radiant

Fullscreen `Overlay`, opened by `omarchy-shell shell toggle jax.seldon`
(overlay route; check README for the exact route) or middle click.
Layout: 12-column grid, `Style.space` gutters.

- Row 1: title "Prime Radiant", machine name, Omarchy version, period
  selector (30 / 90 / 365 days / All; default 90 d, resets on every open;
  WP-030), close hint. While the service is not ok, the status banner
  sits under it with its *Copy* fix only and the hint "Fix it from the
  Seldon panel (click the Seldon mark in the bar)."; its state pictogram
  (§5) is `Style.space(96)` here, the 96 grid.
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
  and toggle the panel independently of the overlay (WP-010). The bar
  builds the widget once per monitor (plus a zero-size, hidden placeholder
  in the bar's centre section: once a centre anchor is set, the default,
  the shell mounts the whole centre list a second time, hidden), and a
  target takes one handler: only the first drawn instance the bar lists
  (`bar.moduleWidgets`; visible
  and not zero-size, as the shell's `pickDrawnSlot` routes a panel
  hotkey) enables its handler, a placeholder only when no instance is
  drawn (WP-078). When an instance comes, goes, or is drawn or hidden,
  every instance looks again, the owner first, so the next one takes the
  target over with one handler at a time (WP-067); IPC calls act on that
  instance's panel.
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

No network. No bundled binaries, units or installers. Reads one JSON file,
and its own images under `plugin/assets/` (SVG and PNG artwork, no
scripts; WP-051).
Runs the `seldon` engine with fixed arguments (the forms in CONTRACT.md).
Besides the engine it starts only `wl-copy` and Omarchy's floating-terminal
launcher, each with one constant command, only on a banner click, and
`omarchy-restart-shell` without arguments on the restart notice's click
(WP-090). Never a
shell string built from logbook content. Documented in README under
"Security, privacy, privileges" (WP-041).
