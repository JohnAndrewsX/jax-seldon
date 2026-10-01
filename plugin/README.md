# JAX Seldon — Omarchy plugin

Flight recorder and planning desk for your Omarchy system. The plugin shows
what the Seldon engine has recorded: active cases, unexplained changes
(drift) and the Prime Radiant overlay. It reads one file,
`${XDG_STATE_HOME:-~/.local/state}/seldon/index.json`, which the engine
writes.

> Phase 3. The panel has all six tabs: Today, Changelog, Work, Decisions,
> System and Memory. The Prime Radiant shows its five charts and The Plan
> for 30 d, 90 d, 365 d or everything.
> Project home: https://github.com/JohnAndrewsX/jax-seldon

## Requirements

- Omarchy 4 with the Omarchy shell.
- The `seldon` engine from the AUR: `omarchy pkg aur add jax-seldon`.
  Without it the plugin shows an "engine not installed" banner with that
  command.

## The pill

`⟡ A · D` — A active cases, D unexplained changes; zero parts are hidden
(`⟡`, `⟡ 2`, `⟡ · 3`). Accent colour when cases are active, the theme's
urgent colour when a change is in the red zone, dimmed while something needs
fixing. The tooltip says what and when the engine last captured.

| Click | Does |
|---|---|
| Left | toggle the panel |
| Middle | toggle the Prime Radiant |
| Right | capture now (`seldon capture`, then `seldon status`) |

## The panel

| Tab | Shows |
|---|---|
| Today | the date, today's counts (events today and in 7 days, active and queued cases, open drift), the QuickEntry (below), today's journal entries, yesterday's behind one row; *Open in editor* (today's journal) |
| Changelog | every event in the index, newest first, grouped by day; source filter chips with counts; *Ledger* (this month's ledger in the editor); *Capture now*; *Resolve…* on every unexplained change (the drift sheet, below) |
| Work | your cases in three columns, Queued · Active (verification included) · Completed (the last 50, dropped ones struck through); the active cases against your limit ("2 / 3 active"); the card of the case under the cursor with its actions; *New case* |
| Decisions | your decisions (ADRs), newest first: id, status (*proposed* marked with the accent stripe, *superseded* struck through), title, date and file; *Open* on the decision under the cursor; *New decision* |
| System | Omarchy version, theme and last update, package counts, deviations, plugins, snapshots, areas, collectors, machine and engine; a section appears only when the index has it; *Open in editor* (the logbook's `STATUS.md`) |
| Memory | what your agents read at session start: the headings of `memory/lessons.md` (LESSONS) and the other memory files with their path and last update (TOPICS); *Open* (the logbook folder, see below) |

**QuickEntry** (Today): type a note and press Enter; it goes to today's
journal through `seldon log`, exactly as typed. Pick an open case below the
field to file the note under it (*Open case* opens that case in the
editor). The line below shows the saved event's id or the engine's error;
the field empties only once the note is saved. A blank note is refused.
The note appears in the journal list when the engine rewrites the index.

**Capture now** (Changelog, `c`, right click on the pill) runs `seldon
capture`, then `seldon status`; the button spins meanwhile and the line
below says what the capture wrote. New rows arrive with the rewritten
index, without a restart.

**Open in editor** runs `seldon open <journal|ledger|status|logbook|case
id|decision id> --editor`; the engine starts your default editor
(`omarchy-launch-editor`).

**Decisions**: Enter, `e`, a double click or *Open* opens the decision under
the cursor (`seldon open ADR-NNNN --editor`). *New decision* (or `d`) shows
a sheet with one field, the title. Enter arms it ("Press Enter again:
create the decision “…”"), the second Enter sends `seldon decide --no-edit
-- <title>`; a click on *Create* sends at once, and changing the title
takes the first press back. The engine writes the decision as *proposed*
and the plugin then opens it in your editor; the list shows it, with the
cursor on it, once the engine rewrites the index. The title stays in the
field until the engine has created the decision, so a refusal (shown in
the sheet) loses nothing; Esc closes the sheet and keeps the title.

**Memory**: the engine cannot open a single memory file yet, so Enter, `e`,
a double click and *Open* all open the logbook folder (`seldon open
logbook --editor`); the files are in its `memory/` folder.

**Work** shows one tile per case: id, steps done/total, title, the zone as
the stripe colour, *verification* or *dropped*, and "N proposed" when the
engine thinks open drift belongs to the case. Under the columns, the card of
the case under the cursor: id and status, zone and risk, title, area,
priority, steps, created/started/closed, the proposed events, and the
actions its status allows:

| Status | Actions |
|---|---|
| queued | *Start*, *Open* |
| active | *Verify*, *Start agent*, *Drop*, *Open* |
| verification | *Done*, *Drop*, *Open* |
| completed, dropped | *Open* |

*Start*, *Verify*, *Done* and *Drop* run `seldon plan start|verify|done|drop
<id>`; *Open* opens the case file in the editor. A click runs the action,
except *Drop*, which asks for a second click (a dropped case stays dropped),
and *Start agent*, which also asks twice.

*Start agent* runs `seldon agent start <id> --json`: the engine makes the
case the active case and launches the agent configured in
`~/.config/seldon/config.toml` (`[agent] launcher`; by default `omarchy
agent prompt`, Omarchy's default coding agent in its own window) with the
case's context as its first prompt. The result line says which launcher
started, or why not (a queued case: start it first; no default agent; no
launcher). The plugin passes nothing but the case id; the launcher and its
rules are the engine's (`engine/hooks/README.md`). The card shows the
agents a case lists, e.g. "agent: claude-code".
The line under the WIP text shows the engine's answer, e.g. `C-2026-005:
queued → active`, or why it refused; the case moves to its new column when
the engine rewrites the index, and the cursor goes with it. If the index is
behind your logbook (you changed a case in a terminal), the engine refuses
and says why; nothing else changes.

**New case** (the button or `+` from any tab) opens a sheet: a title, the
zone (green, yellow, red), the risk (R0–R3), the priority (high, normal,
low) and an optional area slug (`dev-env`: lowercase letters, digits, `-`).
They start at the engine's defaults, yellow, R1, normal. Enter or *Create*
runs `seldon plan new`; the title goes to the engine exactly as typed. The
fields keep their text until the case exists, so a refused case is not
lost; then the sheet closes and the cursor sits on the new case. Esc
closes the sheet and keeps what you typed.

The active-cases limit is a setting (below); the Work tab warns at and over
it, the engine does not enforce it.

The actions need the engine and an initialised logbook; otherwise the
field says why. In dev mode (`SELDON_INDEX`) they are disabled.

A Changelog row shows the source glyph, kind, subject and time, then what
changed, who, and the case. Each row has one colour, in theme colours (red =
urgent, yellow = accent, green = muted): while the event is open drift, the
zone of its drift item (a routine upgrade group is yellow), otherwise the
event's own zone. It paints the stripe on the left and, for open drift, the
glyph, the status line and the "+N" badge. Snapshot rows
are highlighted. A resolved event shows how and why: `linked to C-…`,
`explained · C-…: <why>` (the case `explain` created), `dismissed:
<reason>`. Open drift says *Unexplained* or, in the red zone, *Needs a
reason*, with the proposed case when there is one, and has a *Resolve…*
button. A package transaction that is open drift as one group shows "+N"
(N more packages besides the one shown) on its leader. When the index
lists only the newest 200 unexplained changes and you have more, a line
above the list says "+N more open drift items not listed here".

**Resolve drift** (Enter on an unexplained row, its *Resolve…* button, or
a click on the red strip, which picks the first red-zone change). The
sheet shows the change: what, who, when, its zone, the proposed case, and
for a package transaction every package in it. Three ways to resolve it:

| Action | Runs | Afterwards |
|---|---|---|
| *Link* | `seldon drift link <id> <case>`; the case picker starts on the proposed case, otherwise on "Pick a case"; open cases only | the row says `linked to C-…` |
| *Explain* | `seldon drift explain <id> [--zone] [--risk] [--area] -- <why>`; the zone starts at the change's zone, the risk at R1, the area is optional | the engine creates a completed case for it; the row says `explained · C-…`, the case shows in Work, *Open C-…* opens it |
| *Dismiss* | `seldon drift dismiss <id> -- <reason>` | the row says `dismissed: <reason>` |

A package transaction resolves as one (*All N*); *Only <package>* resolves
just the row you opened the sheet from (`--only`) and leaves the others as
a smaller group. Press Enter twice to send (the first press shows "Press
Enter again: Link firefox and 2 more to C-2026-004"), or click the button
once; changing anything in the form takes the first press back. Your text
stays until the engine has resolved the change, so a refusal loses
nothing, and Esc closes the sheet and keeps what you typed for that
change. The resolved rows, the pill and the red strip update when the
engine rewrites the index. If your logbook resolved the change already
(say, in a terminal), the sheet says "Already resolved: …" and nothing is
written.

Above every tab: the status banner (below), the snapper banner when
snapshots cannot be read, and a red strip "N changes in the red zone need a
reason" (a click opens the drift sheet for the first of them).

| Key | Does |
|---|---|
| Tab / Shift-Tab | the bar's next / previous panel, as in every Omarchy panel |
| ← / →, h / l | previous / next tab |
| 1–6 | a tab by its fixed number: Today 1, Changelog 2, Work 3, Decisions 4, System 5, Memory 6 |
| ↑ / ↓, k / j | move in the list; on Work, through the cases column by column |
| Enter, Space | open the row (the full text, yesterday's entries); on an unexplained Changelog row, the drift sheet; on Work, the card's first action: *Open* at once, *Start*, *Verify* or *Done* on the second press; on Decisions and Memory, the file in the editor |
| x | Work: drop the case under the cursor, on the second press |
| a | Work: start an agent on the active case under the cursor, on the second press |
| f / F | Changelog: next / previous source filter |
| c | capture now |
| n | write a note: focuses the QuickEntry (from any tab) |
| + | new case: opens the Work tab's sheet (from any tab) |
| d | Decisions: new decision (the sheet) |
| e | open this tab's file in the editor: journal, ledger, the case under the Work cursor, the decision under the Decisions cursor, `STATUS.md`, the logbook folder (Memory) |
| Esc | close |

On Work, the first Enter (or x) arms the action: its button is marked and
the card says "Press Enter again: Start C-2026-005". Any other key or a
cursor move disarms it.

While the QuickEntry field or its case picker has focus, every key goes to
it: Enter saves, Tab moves between field and picker (↓ opens the picker),
Esc gives the keys back to the panel. The same holds for the new-case
sheet: Tab walks title, zone, risk, priority, area, *Create*, *Cancel*; in
a picker ←/→ (h/l) move and Enter or Space picks; Enter in a text field
creates the case. And for the drift sheet: Tab walks the action (Link,
Explain, Dismiss), then the case picker and *All / Only* (Link), the text,
zone, risk and area (Explain) or the reason (Dismiss), then the action
button, *Cancel* and *Open C-…*; Enter in a text field or on the action
button arms, the second Enter sends. And for the new-decision sheet: Tab
walks title, *Create*, *Cancel*; Enter in the title or on *Create* arms,
the second Enter sends.

## The Prime Radiant

A fullscreen overlay over a dimmed screen: `omarchy-shell shell toggle
jax.seldon` (or the suggested binding below) or a middle click on the pill.

- **Header:** "Prime Radiant", the machine, the Omarchy version and when the
  index was written; on the right the period selector (30 d · 90 d · 365 d ·
  All, default 90 d), *Close*, and the period's dates.
- **Grid** of six slots on 12 columns: Heatmap (full width); Series,
  DriftBars and RiskDonut side by side; Timeline (full width); The Plan
  (full width). On a narrow screen the grid uses two columns, then one, and
  scrolls once the slots are at their minimum height.
- **Charts**, all in the theme's colours (accent, foreground, urgent):
  - *Heatmap*: events per day as a calendar, weeks as columns and Monday on
    top; five shades of the accent from few to many events (a faint cell for
    none). Hover: the day and its events by source.
  - *Series*: explicit and total package counts as step lines, one lane
    each, with their range. Hover: the sample at that day.
  - *DriftBars*: drift opened (accent) and resolved per ISO week. Hover:
    the week, its dates and both counts.
  - *RiskDonut*: cases by risk R0–R3 (R3 in the urgent colour), with the
    count in the middle and a legend; always all time. Hover: the class, its
    count and share.
  - *Timeline*: releases (diamonds), snapshots (dots) and crises (urgent
    diamonds) on top, case spans below (open cases bright, running to
    today). Hover: the item's kind, label and date.
  - *The Plan*: the active cases as cards: zone stripe, id and risk, title,
    steps done of total as a bar, the agent. It has no period.

  Each slot's title row carries the chart's summary ("62 events on 13 of 90
  days · busiest 2026-10-01 (30)"), replaced by the hovered item's details
  while the pointer is on the chart. A chart without data in the period
  says "no data in this period" (the donut "no cases yet · all time", The
  Plan "no active cases").
- A period covers the days up to the index's today: 30 d is today and the
  29 days before it; a drift week counts when any of its days does, a case
  when its span overlaps. *All* is everything the index carries. Cases by
  risk have no dates and are the same for every period ("all time").
- When the panel shows a banner, the overlay shows it too, with only the
  fix that runs no command (*Copy*); the other fixes are in the panel.

| Key | Action |
|---|---|
| `1` `2` `3` `4` | 30 d · 90 d · 365 d · All |
| ← → (`h` `l`) | previous / next period (wraps) |
| Esc | close |

A click on the dimmed area or on *Close* closes it too. The overlay reads
the service's index only and never runs the engine. The service prepares
every period's chart data when the index changes, so opening the overlay
and switching periods only draw.

## States

When something is wrong the panel shows one banner with a one-click fix.

| Status | Banner | Fix |
|---|---|---|
| `engineMissing` | Seldon engine not installed | *Install in terminal*, *Copy* `omarchy pkg aur add jax-seldon`, *Check again* |
| `notInitialised` | Logbook not initialised | *Run in terminal* / *Copy* `seldon init`, *Check again* |
| `indexMissing` | No index yet / Index unreadable | *Build index* (`seldon status`) |
| `indexStale` | Index is stale (older than 2 h) | *Capture now* |
| `contractMismatch` | Index format mismatch | *Update in terminal* / *Copy* the plugin or engine update command |
| snapper collector failing | Snapshots not readable (with the engine's message) | *Run in terminal* / *Copy* `sudo snapper -c root set-config ALLOW_USERS=$USER SYNC_ACL=yes` (once; Seldon never runs it) |

The engine is looked for once when the shell starts and again on *Check
again* (or `jax.seldon.service refresh`), not on every capture interval.

## IPC

| Command | Reaches |
|---|---|
| `omarchy-shell shell toggle jax.seldon` | the Prime Radiant overlay (also `summon`, `hide`) |
| `omarchy-shell shell summon jax.seldon '{"period":"30"}'` | open the Prime Radiant on a period (`30`, `90`, `365`, `all`) |
| `omarchy-shell shell call jax.seldon view ""` | what the open Prime Radiant shows (period, window, banner, grid mode, aggregation counts, each slot's counts and geometry, each chart's summary, numbers, hover read-out and paint count), as JSON; `unknown` while it is closed |
| `omarchy-shell shell call jax.seldon setPeriod 30` | pick a period in the open Prime Radiant |
| `omarchy-shell shell call jax.seldon hover "heatmap 0.9,0.5"` | the read-out at that point of a chart (`heatmap`, `series`, `driftBars`, `riskDonut`, `timeline`, `plan`; x,y as fractions of its plot), as JSON; `hover ""` clears it |
| `omarchy-shell jax.seldon.panel open\|close\|toggle\|show\|hide` | the bar panel |
| `omarchy-shell jax.seldon.panel pill` | what the pill shows, as JSON |
| `omarchy-shell jax.seldon.panel view` | what the panel shows (tab, rows, banners, strip, pill, QuickEntry, the Work columns, card and sheet, the drift sheet, the decisions and their sheet, the memory rows, last action results), as JSON |
| `omarchy-shell jax.seldon.panel tab today\|changelog\|work\|decisions\|system\|memory` | show a tab |
| `omarchy-shell jax.seldon.panel filter all\|<source>` | set the Changelog source filter |
| `omarchy-shell jax.seldon.panel resolve crisis\|<event id>` | open the drift sheet (the red strip's first crisis, or that event); sends nothing |
| `omarchy-shell jax.seldon.service status` | the service state, as JSON |
| `omarchy-shell jax.seldon.service refresh` | look for the engine again and re-read the index |
| `omarchy-shell jax.seldon.service capture` | capture now |

Because the plugin declares an overlay, the shell's own `summon`/`toggle`
route opens the Prime Radiant, never the bar panel; the panel has its own
target, `jax.seldon.panel`.

Suggested binding (not installed by the plugin), in your Hyprland bindings:

```
o.bind("SUPER + SHIFT + S", "Seldon", "omarchy-shell shell toggle jax.seldon")
```

## Settings

Setup > Plugins > Seldon:

- **Capture interval (minutes)**, 5–120, default 15.
- **Active cases limit**, 1–20, default 3: what the Work tab measures active
  cases against ("2 / 3 active"). Cases in verification do not count.

## Security, privacy, privileges

- No network access. No bundled binaries, units or installers.
- Reads one JSON file. Everything shown from it is displayed as plain text,
  never evaluated.
- Runs the `seldon` engine only with fixed argument lists from the
  contract; ids are checked against their patterns first (a case id from
  the index included, event ids, and decision ids, also the one the engine
  reports for a decision it just created), free text (a QuickEntry note, a
  case title, a drift explanation or reason, a decision title) is one
  non-empty argument after `--`. Never builds a shell command from logbook
  content. Writing to the logbook happens only
  through the engine, on your Enter or click.
- Opens a terminal or touches the clipboard only when you click a banner
  button, and then only with a constant command.

## Development

- `SELDON_INDEX=/path/to/index.json` makes the service read that file
  instead (relative paths resolve against the shell's working directory).
  Dev mode is read-only: the engine is probed, never run.
- `SELDON_NOW=<RFC 3339>` (with `SELDON_INDEX`) sets the clock for the
  staleness check; without it a fixture never turns stale.
- After changing plugin code, restart the shell (`omarchy-restart-shell`):
  the shell's hot reload re-creates the plugin from cached components.

See `docs/TESTING.md` in the project repository for the smoke test.

## License

MIT
