# Daily use

This page covers the parts of Seldon you see every day: the pill in the
bar, the panel with its six tabs, the keys, and the Prime Radiant
overlay with its periods. Everything here works with the mouse and with
the keyboard.

## A day with Seldon

- In the morning, glance at the pill. `⟡ 2 · 3` means two active cases
  and three unexplained changes.
- Before a change, create a case (`+` in the panel) and start it.
- While you work, write a note when you learn something (`n` in the
  panel).
- When the pill shows drift, open the Changelog and resolve it: link,
  explain or dismiss.
- At the end of a case, verify it and close it on the Work tab.
- Once a week, open the Prime Radiant and look at the picture.

## The pill

The pill sits on the right of the bar.

- `⟡ A · D`: A is the number of active cases, D the number of open drift
  items. Parts that are zero are hidden: `⟡`, `⟡ 2`, `⟡ · 3`.
- It uses your theme's accent colour while cases are active, the theme's
  urgent colour when there is a crisis, and dims while something needs
  fixing.
- The tooltip says what the numbers mean and when the engine last
  captured.

| Click | Does |
|---|---|
| Left | open or close the panel |
| Middle | open or close the Prime Radiant |
| Right | capture now (`seldon capture`, then `seldon status`) |

## The panel

The panel opens under the pill. It has six tabs, each with a fixed
number key. Above every tab you may see a banner (something needs fixing,
see [Troubleshooting](10-troubleshooting.md#banners-in-the-panel)) and a
red line "N changes in the red zone need a reason". Click the red line to
resolve the first crisis.

The pictures on this page are renders of the sample logbook in the
Tokyo Night theme. Your panel uses your theme and shows your data.

### Today (1)

![The Today tab: the date, counts for today, the note field with a case picker, and today's journal entries](../../images/panel-tokyo-night-today.png)

*Sample data.*

Today shows the date and today's counts: events today and in seven days,
active and queued cases, open drift. Below is the note field: type a
note, press Enter, and it goes into today's journal through `seldon log`.
Pick an open case under the field to file the note under it. The field
empties only once the note is saved. Today's journal entries follow;
yesterday's are behind one row. *Open in editor* opens today's journal
file.

### Changelog (2)

![The Changelog tab: source filter chips with counts, Ledger and Capture now buttons, and events grouped by day, newest first](../../images/panel-tokyo-night-changelog.png)

*Sample data, filtered to the source `seldon`.*

The Changelog lists every event, newest first, grouped by day. The chips
at the top filter by source; each shows its count. Snapshot rows are
highlighted. Drift rows are marked in their zone's colour and carry a
*Resolve…* button.

*Capture now* runs a capture; the line below says what it found. *Ledger*
opens this month's ledger view in your editor.

To resolve drift, press Enter on a drift row, click *Resolve…*, or click
the red line. The drift sheet shows what changed, who did it, when, its
zone, the proposed case and every package of a transaction. Pick one
action:

| Action | Runs | The row then says |
|---|---|---|
| *Link* | `seldon drift link <EVENT> <CASE>` (open cases; the proposed one preselected) | `linked to C-…` |
| *Explain* | `seldon drift explain <EVENT> -- <why>`, optionally with zone, risk and area | `explained · C-…` |
| *Dismiss* | `seldon drift dismiss <EVENT> -- <reason>` | `dismissed: <reason>` |

A package transaction resolves as one (*All N*). *Only <package>*
resolves the row you opened. Your text stays in the sheet until the engine
has written it. If someone resolved the change in the meantime, the sheet
says "Already resolved: …" and writes nothing.

### Work (3)

![The Work tab: three columns Queued, Active and Completed with case tiles, and the card of the selected case with its Start and Open buttons](../../images/panel-tokyo-night-work.png)

*Sample data.*

Work shows your cases in three columns: Queued, Active (cases in
verification included) and Completed (the last 50; dropped cases struck
through). "2 / 3 active" compares your active cases with your limit. The
limit warns; it never blocks. A tile shows the id, steps done of total,
the title and the zone as its stripe colour. "N proposed" means Seldon
thinks some open drift belongs to this case.

The card below shows the case under the cursor and the actions its
status allows:

| Status | Actions |
|---|---|
| queued | *Start*, *Open* |
| active | *Verify*, *Start agent*, *Drop*, *Open* |
| verification | *Done*, *Drop*, *Open* |
| completed, dropped | *Open* |

*New case* (or `+` from any tab) asks for a title, zone, risk, priority
and an optional area. It starts at yellow, R1, normal. *Start agent*
sends an agent to work the case; see
[Working with agents](04-working-with-agents.md#start-an-agent-from-the-panel).

### Decisions (4)

![The Decisions tab: four decisions newest first with id, status, title, date and file path, and a New decision button](../../images/panel-tokyo-night-decisions.png)

*Sample data.*

Decisions lists your ADRs, newest first: id, status (*proposed* is
marked, *superseded* struck through), title and date. *Open* opens one in
your editor. *New decision* (or `d`) asks for a title, creates the
decision as *proposed* and opens it.

### System (5)

![The System tab: Omarchy version, theme and last update, package counts, plugins and the latest snapshots](../../images/panel-tokyo-night-system.png)

*Sample data.*

System shows the machine: Omarchy version, theme and last update,
package counts, deviations, plugins, snapshots, areas, the state of each
collector, the machine name and the engine version. *Open in editor*
opens `STATUS.md`.

### Memory (6)

![The Memory tab: three lesson headings and two memory topics with path and last update](../../images/panel-tokyo-night-memory.png)

*Sample data.*

Memory shows what your agents read at the start of a session: the
headings of `memory/lessons.md` and the other memory files with their
last update. *Open* opens the logbook folder.

## Keys

The panel's keys work while the panel is open.

| Key | Does |
|---|---|
| `1` to `6` | a tab by its number: Today 1, Changelog 2, Work 3, Decisions 4, System 5, Memory 6 |
| ← / →, `h` / `l` | previous / next tab |
| ↑ / ↓, `k` / `j` | move in the list; on Work, through the cases column by column |
| Tab / Shift-Tab | the bar's next / previous panel, as in every Omarchy panel |
| Enter, Space | open the row; on a drift row, the drift sheet; on Work, the card's first action |
| `x` | Work: drop the case under the cursor (press twice) |
| `a` | Work: start an agent on the active case under the cursor (press twice) |
| `f` / `F` | Changelog: next / previous source filter |
| `c` | capture now |
| `n` | write a note (from any tab) |
| `+` | new case (from any tab) |
| `d` | Decisions: new decision |
| `e` | open this tab's file in your editor |
| Esc | close |

Actions that write take two presses on the keyboard: *Start*, *Verify*, *Done*, *Drop* (`x`), *Start agent* (`a`),
the drift sheet and a new decision. The first Enter arms the action and
the card says "Press Enter again: Start C-2026-005". The second press
sends it. Any other key in a list disarms it. A note and a new case are
sent with one Enter. With the mouse, one click sends, except *Drop* and
*Start agent*, which ask for a second click.

While a text field or a sheet has focus, every key goes to it. Tab and Shift-Tab walk its fields. Esc gives the keys back to
the panel and keeps what you typed.

## The Prime Radiant

![The Prime Radiant: a heatmap of events per day, package series, drift bars, a risk donut, a timeline and the active cases, next to the panel's Today tab](../../../plugin/preview.png)

*Renders of the sample logbook, Tokyo Night. Left the Prime Radiant, right the panel.*

The Prime Radiant is a fullscreen overlay with the picture of your
machine. Open it with a middle click on the pill, or with
`omarchy-shell shell toggle jax.seldon`. To put it on a key, add this
line to your Hyprland bindings (Seldon never installs it for you):

```text
o.bind("SUPER + SHIFT + S", "Seldon", "omarchy-shell shell toggle jax.seldon")
```

It shows six charts:

| Chart | Shows |
|---|---|
| Heatmap | events per day as a calendar, weeks as columns, Monday on top |
| Series | explicit and total package counts over time |
| DriftBars | drift opened and resolved per week |
| RiskDonut | cases by risk, R0 to R3, always all time |
| Timeline | Omarchy releases, snapshots and crises on top, cases as spans below |
| The Plan | the active cases with their zone, risk, steps and agent |

Each chart's title row carries a summary. Point at a chart and the title
row shows the item under the pointer instead: a day and its events by
source, a week's counts, a case.

### Periods

The period selector at the top picks the window: 30 days, 90 days, 365
days or All. It covers the days up to today; 30 d is today and the 29
days before. The Prime Radiant opens on 90 d every time. The RiskDonut
and The Plan ignore the period.

| Key | Does |
|---|---|
| `1` `2` `3` `4` | 30 d, 90 d, 365 d, All |
| ← / →, `h` / `l` | previous / next period |
| Esc | close |

A click on the dimmed area or on *Close* closes it too. The Prime Radiant
only shows; it never runs the engine.

## From the terminal

Everything in the panel has a command. The panel runs the same commands,
so the result is the same:

| In the panel | In a terminal |
|---|---|
| note field | `seldon log -- "Text"` |
| *Capture now* | `seldon capture` |
| *New case* | `seldon plan new -- "Title"` |
| *Start*, *Verify*, *Done*, *Drop* | `seldon plan start <ID>` and so on |
| drift sheet | `seldon drift link`, `explain`, `dismiss` |
| *New decision* | `seldon decide -- "Title"` |
| *Open in editor* | `seldon open journal --editor` |

The [CLI reference](05-cli-reference.md) lists every command.

---

Previous: [Concepts](02-concepts.md) · [Index](README.md) · Next: [Working with agents](04-working-with-agents.md)
