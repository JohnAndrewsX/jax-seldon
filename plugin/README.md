![Seldon: the Prime Radiant mark and the wordmark on night blue](assets/a6-readme-hero-dark-1280x640.png)

# JAX Seldon

The Omarchy shell plugin for Seldon, the flight recorder and planning desk
for your Omarchy system.

[![CI](https://github.com/JohnAndrewsX/jax-seldon/actions/workflows/ci.yml/badge.svg)](https://github.com/JohnAndrewsX/jax-seldon/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/JohnAndrewsX/jax-seldon)](https://github.com/JohnAndrewsX/jax-seldon/releases/latest)
[![Licence: MIT](https://img.shields.io/github/license/JohnAndrewsX/jax-seldon)](LICENSE)

![Preview, 2480×1080: the Prime Radiant at 1920×1080 on the left, the panel's Today tab framed on the right, Tokyo Night](preview.png)

*Offscreen renders of the real QML on the sample index, Tokyo Night theme.*

The Seldon engine records what changes on your machine (packages, Omarchy
updates, themes, plugins, snapshots, config) in a plain-Markdown logbook
and matches it against the changes you planned as cases. This plugin
shows the result in the Omarchy shell. You write notes, plan cases and
explain unexpected changes from the panel; the engine does the writing.

- A pill in the bar counts active cases and unexplained changes
  ([The pill](#the-pill)).
- A panel with six tabs shows today's journal, the changelog, your work
  cases, decisions, the system and your agents' memory
  ([The panel](#the-panel)).
- The Prime Radiant is a fullscreen overlay with charts and the plan
  ([The Prime Radiant](#the-prime-radiant)).
- From the panel you write notes, create cases, move them through their
  steps and propose decisions; the engine writes each one.
- One sheet resolves drift: link a change to a case, explain it or
  dismiss it.
- Every degraded state, from a missing engine to a stale index, has a
  one-click fix ([States](#states)).

It follows your Omarchy theme, runs every program without a shell and
never writes a file itself ([Security](#security-privacy-privileges)).

[Install](#install) · [Usage](#usage) · [Keys](#keys) ·
[Configure](#configure) · [Troubleshooting](#troubleshooting) ·
[Remove](#remove) · [Documentation](#documentation)

## Requirements

- Omarchy 4 with the Omarchy shell.
- The `seldon` engine (see [Install](#install)). Without it the plugin
  shows "Seldon engine not installed" (see [States](#states)).
- A logbook, created once with `seldon init` (in a terminal; it asks where
  to put it, `~/Seldon` by default).

## Install

Three steps: the engine, your logbook, the plugin. None needs `sudo`.

**1. The engine.** AUR package: coming soon; until then install it from
GitHub. `install.sh` checks the release against its `SHA256SUMS`, refuses
on a mismatch, installs `~/.local/bin/seldon` and never asks for root.

Download, read, verify, run:

```sh
cd "$(mktemp -d)"
curl -fsSLO https://github.com/JohnAndrewsX/jax-seldon/releases/latest/download/install.sh
curl -fsSLO https://github.com/JohnAndrewsX/jax-seldon/releases/latest/download/SHA256SUMS
less install.sh                                    # read what it does
sha256sum -c --ignore-missing SHA256SUMS && bash install.sh
```

or in one line (the script still verifies the engine, not itself):

```sh
curl -fsSL https://github.com/JohnAndrewsX/jax-seldon/releases/latest/download/install.sh | bash
```

Run it again to update. Its options (`--version`, `--prefix`, `--unit`
for the optional watcher, `--force` over a self-built `seldon`) are in the
[project README](https://github.com/JohnAndrewsX/jax-seldon#install-options-update-and-removal).
Check the engine with `seldon --version`. If your shell says
`command not found`, open a new terminal.

Once the AUR package is live, you can install the engine from there
instead of GitHub. Install from one source only: both put a `seldon` on
your `PATH`.

```sh
omarchy pkg aur add jax-seldon   # install
yay -S jax-seldon                # update
```

**2. Your logbook**, once, in a terminal:

```sh
seldon init
```

It asks where to put the logbook (`~/Seldon` by default) and a few more
questions; Enter takes each default. `seldon doctor` then checks the
setup.

**3. The plugin:**

```sh
omarchy plugin add https://github.com/JohnAndrewsX/jax-seldon-plugin.git --enable
```

Omarchy asks before it clones the plugin. The pill lands in the bar's
right section. Without `--enable` the plugin is installed disabled; enable
it later with `omarchy plugin enable jax.seldon`.

**Development copy** (before a release, or to try local changes): copy the
`plugin/` folder of the [project repository](https://github.com/JohnAndrewsX/jax-seldon)
into the plugin directory (a copy, not a symlink: the shell refuses
symlinks in a plugin folder), then enable it and restart the shell:

```sh
mkdir -p ~/.config/omarchy/plugins/jax.seldon
cp -r plugin/. ~/.config/omarchy/plugins/jax.seldon/
omarchy plugin enable jax.seldon
omarchy-restart-shell
```

## Usage

### The pill

The Seldon mark, then `A · D` — A active cases, D unexplained changes
(open drift); zero parts are hidden (the mark alone, `2`, `· 3`). Accent
colour while cases are active, the theme's urgent colour when a change is
in the red zone, dimmed while something needs fixing; the mark takes the
same colour, so it follows the theme. The tooltip says what and when the
engine last captured.

| Click | Does |
|---|---|
| Left | toggle the panel |
| Middle | toggle the Prime Radiant |
| Right | capture now (`seldon capture`, then `seldon status`) |

### The panel

Six tabs, each with a fixed number key:

| # | Tab | Shows | Actions |
|---|---|---|---|
| 1 | Today | the date, today's counts (events today and in 7 days, active and queued cases, open drift), the QuickEntry, today's journal entries, yesterday's behind one row | write a note; *Open in editor* (today's journal) |
| 2 | Changelog | every event in the index, newest first, grouped by day; source filter chips with counts; snapshot rows highlighted; unexplained changes marked | *Resolve…* on every unexplained change; *Ledger* (this month's ledger); *Capture now* |
| 3 | Work | your cases in three columns, Queued · Active (verification included) · Completed (the last 50, dropped ones struck through); "2 / 3 active" against your limit; the card of the case under the cursor | *New case*; the card's actions (below) |
| 4 | Decisions | your decisions (ADRs), newest first: id, status (*proposed* marked, *superseded* struck through), title, date | *Open*; *New decision* |
| 5 | System | Omarchy version, theme and last update, package counts, deviations, plugins, snapshots, areas, collectors, machine and engine | *Open in editor* (the logbook's `STATUS.md`) |
| 6 | Memory | what your agents read at session start: the headings of `memory/lessons.md` and the other memory files with path and last update | *Open* (the logbook folder) |

Above every tab: the status banner (see [States](#states)), the snapper
banner when snapshots cannot be read, and a red strip "N changes in the red
zone need a reason" (a click opens the drift sheet for the first of them).

The panel is 460 spacing units wide (`Style.space(460)`), so it grows with
your theme's font size (`[font] base-size`) and stays within the screen.
Each tab button is at least as wide as its label; on a screen too narrow
for all six, the tabs wrap onto a second line rather than cut a label off.
The Changelog's filter chips wrap the same way.

**QuickEntry** (Today, or `n` from any tab): type a note and press Enter;
it goes to today's journal through `seldon log`, exactly as typed. Pick an
open case below the field to file the note under it. The line below shows
the saved event's id or the engine's error; the field empties only once the
note is saved, and a blank note is refused.

**Capture now** (Changelog, `c`, or a right click on the pill) runs
`seldon capture`, then `seldon status`; the button spins meanwhile and the
line below says what the capture wrote. New rows arrive with the rewritten
index, without a restart.

**Open in editor** runs
`seldon open <journal|ledger|status|logbook|case id|decision id> --editor`;
the engine starts your default editor (`omarchy-launch-editor`).

**Work.** One tile per case: id, steps done/total, title, the zone as the
stripe colour, *verification* or *dropped*, and "N proposed" when the engine
thinks open drift belongs to the case. The card of the case under the
cursor shows id, status, zone, risk, title, area, priority, steps, dates,
the proposed events and the actions its status allows:

| Status | Actions |
|---|---|
| queued | *Start*, *Open* |
| active | *Verify*, *Start agent*, *Drop*, *Open* |
| verification | *Done*, *Drop*, *Open* |
| completed, dropped | *Open* |

*Start*, *Verify*, *Done* and *Drop* run
`seldon plan start|verify|done|drop <id>`. A click runs the action, except
*Drop* and *Start agent*, which ask for a second click. *Start agent* runs
`seldon agent start <id>`: the engine makes the case the active case and
launches the agent configured in `~/.config/seldon/config.toml`
(`[agent] launcher`; by default `omarchy agent prompt`) with the case's
context as its first prompt. The line under
the columns shows the engine's answer (`C-2026-005: queued → active`) or
why it refused.

**New case** (the button, or `+` from any tab): a title, the zone (green,
yellow, red), the risk (R0–R3), the priority (high, normal, low) and an
optional area slug; they start at the engine's defaults (yellow, R1,
normal). Enter or *Create* runs `seldon plan new`. The fields keep their
text until the case exists, so a refused case is not lost.

**New decision** (the button, or `d` on Decisions): one field, the title.
The engine writes the decision as *proposed* (`seldon decide`) and the
plugin opens it in your editor.

**Resolve drift** (Enter on an unexplained Changelog row, its *Resolve…*
button, or a click on the red strip). The sheet shows the change (what,
who, when, its zone, the proposed case, every package of a transaction)
and three ways to resolve it:

| Action | Runs | Afterwards |
|---|---|---|
| *Link* | `seldon drift link <id> <case>` (open cases; the proposed case preselected) | the row says `linked to C-…` |
| *Explain* | `seldon drift explain <id> [--zone] [--risk] [--area] -- <why>` | the engine creates a completed case for it; the row says `explained · C-…` |
| *Dismiss* | `seldon drift dismiss <id> -- <reason>` | the row says `dismissed: <reason>` |

A package transaction resolves as one (*All N*); *Only <package>* resolves
just the row you opened (`--only`). Your text stays until the engine has
resolved the change; if the logbook resolved it already, the sheet says
"Already resolved: …" and nothing is written.

### Keys

**Panel** (the keys are the panel's while it is open):

| Key | Does |
|---|---|
| `1`–`6` | a tab by its fixed number: Today 1, Changelog 2, Work 3, Decisions 4, System 5, Memory 6 |
| ← / →, `h` / `l` | previous / next tab |
| ↑ / ↓, `k` / `j` | move in the list (the first press shows the cursor); on Work, through the cases column by column |
| Tab / Shift-Tab | the bar's next / previous panel, as in every Omarchy panel (never cycles tabs) |
| Enter, Space | open the row; on an unexplained Changelog row, the drift sheet; on Work, the card's first action (*Open* at once; *Start*, *Verify* or *Done* on the second press); on Decisions and Memory, the file in the editor |
| `x` | Work: drop the case under the cursor, on the second press |
| `a` | Work: start an agent on the active case under the cursor, on the second press |
| `f` / `F` | Changelog: next / previous source filter |
| `c` | capture now |
| `n` | write a note: focuses the QuickEntry (from any tab) |
| `+` | new case: opens the Work tab's sheet (from any tab) |
| `d` | Decisions: new decision |
| `e` | open this tab's file in the editor: journal, ledger, the case under the cursor, the decision under the cursor, `STATUS.md`, the logbook folder |
| Esc | close |

**Two-press arming.** On the keyboard, the Work card's *Start*, *Verify*,
*Done*, *Drop* (`x`) and *Start agent* (`a`), the drift sheet and the
new-decision sheet take two presses. The first Enter (or `x`, or `a`) arms
the action: its button is marked and the card says "Press Enter again:
Start C-2026-005". The second press sends it. A note (QuickEntry) and a new
case are sent with one Enter. In a list, any other key or a cursor move
disarms; in a sheet, only a change to the form disarms (Tab between fields
keeps the arm). A mouse click on a button sends at once, except *Drop* and
*Start agent*, which ask for a second click.

**Fields and sheets.** While the QuickEntry or a sheet has focus, every key
goes to it: Tab / Shift-Tab walk its fields and buttons, ↓ opens a picker,
←/→ (`h`/`l`) move in a picker and Enter or Space picks, Enter in a text
field or on the action button arms and the second Enter sends (the
QuickEntry and the new-case sheet send on the first Enter). Esc gives the keys back to the panel
and keeps what you typed.

**Prime Radiant:**

| Key | Does |
|---|---|
| `1` `2` `3` `4` | 30 d · 90 d · 365 d · All |
| ← / →, `h` / `l` | previous / next period (wraps) |
| Esc | close |

A click on the dimmed area or on *Close* closes it too.

### The Prime Radiant

A fullscreen overlay over a dimmed screen. Open it with a middle click on
the pill, `omarchy-shell shell toggle jax.seldon`, or the suggested
binding (see [Configure](#configure)).

- **Header:** "Prime Radiant", the machine, the Omarchy version and when
  the index was written; on the right the period selector (30 d · 90 d ·
  365 d · All, default 90 d on every open), *Close*, and the period's dates.
- **Periods** cover the days up to the index's today: 30 d is today and
  the 29 days before it; a drift week counts when any of its days does, a
  case when its span overlaps. *All* is everything the index carries.
- **Charts**, in the theme's colours (accent, foreground, urgent):
  - *Heatmap*: events per day as a calendar, weeks as columns, Monday on
    top, five shades of the accent.
  - *Series*: explicit and total package counts as step lines.
  - *DriftBars*: drift opened (accent) and resolved per ISO week.
  - *RiskDonut*: cases by risk R0–R3 (R3 urgent); always all time.
  - *Timeline*: releases, snapshots and crises on top, case spans below
    (open cases run to today).
  - *The Plan*: the active cases as cards with zone, risk, a steps bar and
    the agent; it has no period.
- **Hover:** each chart's title row carries its summary ("62 events on 13
  of 90 days · busiest 2026-10-01 (30)"), replaced by the hovered item's
  details (a day and its events by source, a package sample, a week's
  counts, a risk class and its share, a release, snapshot or case) while
  the pointer is on the chart. A chart without data in the period says
  "no data in this period".
- On a narrow screen the grid uses two columns, then one, and scrolls.
- When the panel shows a banner, the overlay shows it too, with only the
  *Copy* fix; the other fixes are in the panel. The overlay never runs
  the engine.

### States

When something is wrong the panel shows one banner with a one-click fix:

| State | Banner | One-click fix |
|---|---|---|
| Engine missing | Seldon engine not installed | *Install in terminal* runs the GitHub one-liner `curl -fsSL https://github.com/JohnAndrewsX/jax-seldon/releases/latest/download/install.sh \| bash` in a floating terminal (the script verifies the download against `SHA256SUMS`; see [Install](#install)); *Copy* puts it on the clipboard; *Check again* looks for the engine again |
| Logbook not initialised | Logbook not initialised | *Run in terminal* runs `seldon init` (it asks where to put the logbook); *Copy*; *Check again* |
| Index missing | No index yet / Index unreadable | *Build index* runs `seldon status`, which writes it |
| Index stale (older than 2 h) | Index is stale | *Capture now* |
| Index format mismatch | Index format mismatch, with both contract versions | *Update in terminal* / *Copy*: `omarchy plugin update jax.seldon` when the plugin is older, the GitHub one-liner from *Engine missing* again when the engine is older (until the AUR package is live, ADR-0024) |
| Snapshots not readable | Snapshots not readable, with the engine's message and what the fix grants: it adds your user to `ALLOW_USERS`, which also lets your user create, change and delete root snapshots without a password | *Run in terminal* / *Copy*: `sudo snapper -c root set-config ALLOW_USERS=$USER SYNC_ACL=yes` (once; Seldon never runs it on its own); then *Check again* runs a capture, like *Capture now*, which clears the banner once snapshots are readable. After *Run in terminal* the banner says "When the command has finished, press Check again" |

The engine is looked for when the shell starts and again on the status
banner's *Check again* (or `omarchy-shell jax.seldon.service refresh`),
not on every capture.

## Configure

**Settings** (Setup > Plugins > Seldon, or `omarchy bar set`):

| Key | Default | What it does |
|---|---|---|
| `captureIntervalMin` | `15` | How often (5–120 minutes) the engine captures system changes while the shell runs |
| `wipLimit` | `3` | The Work tab shows active cases against this limit ("2 / 3 active"); 1–20; it warns, it never blocks. Cases in verification do not count |

```sh
omarchy bar set jax.seldon captureIntervalMin 30 --json
omarchy bar move jax.seldon --section right
```

**Suggested binding** (not installed by the plugin). Add it to your
Hyprland bindings:

```
o.bind("SUPER + SHIFT + S", "Seldon", "omarchy-shell shell toggle jax.seldon")
```

**IPC targets:**

| Command | Reaches |
|---|---|
| `omarchy-shell shell toggle jax.seldon` | the Prime Radiant (also `summon`, `hide`) |
| `omarchy-shell shell summon jax.seldon '{"period":"30"}'` | the Prime Radiant on a period (`30`, `90`, `365`, `all`) |
| `omarchy-shell shell call jax.seldon view ""` | what the open Prime Radiant shows, as JSON; `unknown` while it is closed |
| `omarchy-shell shell call jax.seldon setPeriod 30` | pick a period in the open Prime Radiant |
| `omarchy-shell shell call jax.seldon hover "series 0.5,0.5"` | the read-out at that point of a chart (`heatmap`, `series`, `driftBars`, `riskDonut`, `timeline`, `plan`; x,y as fractions of its plot), as JSON; `hover ""` clears; a malformed argument returns `{"error": …}` and changes nothing. The heatmap's grid is square, height-bound and left-aligned (legend beside it), so at 30 d and 90 d only the left part of its plot holds cells: take the point from `Model.heatmapLayout` (docs/TESTING.md, "Runtime smoke test in the shell", step 4) |
| `omarchy-shell jax.seldon.panel open\|close\|toggle\|show\|hide` | the bar panel |
| `omarchy-shell jax.seldon.panel pill` | what the pill shows, as JSON |
| `omarchy-shell jax.seldon.panel view` | what the panel shows, as JSON |
| `omarchy-shell jax.seldon.panel tab today\|changelog\|work\|decisions\|system\|memory` | show a tab |
| `omarchy-shell jax.seldon.panel filter all\|<source>` | set the Changelog source filter |
| `omarchy-shell jax.seldon.panel resolve crisis\|<event id>` | open the drift sheet; sends nothing |
| `omarchy-shell jax.seldon.service status` | the service state, as JSON |
| `omarchy-shell jax.seldon.service refresh` | look for the engine again and re-read the index |
| `omarchy-shell jax.seldon.service capture` | capture now |

Because the plugin declares an overlay, the shell's `summon`/`toggle` route
opens the Prime Radiant, never the bar panel; the panel has its own target,
`jax.seldon.panel`.

## Security, privacy, privileges

The plugin runs **unsandboxed inside the Omarchy shell process**, like every
shell plugin. This is everything it does outside its own window:

- **Reads one file:** `${XDG_STATE_HOME:-~/.local/state}/seldon/index.json`,
  which the engine writes (or the file `SELDON_INDEX` names, see
  [Development](#development)). The contract would let it read the files
  the index points to; this version reads none of them and asks the engine
  to open them in your editor instead. Everything shown from the index is
  displayed as plain text, never evaluated.
- **Runs the `seldon` engine only with these fixed argument lists**
  (the project's `docs/CONTRACT.md`, "Commands the plugin may run"):

  ```
  seldon --version --json
  seldon status --json
  seldon capture --all --json --quiet
  seldon log [--case <id>] --json -- <text>
  seldon open <journal|ledger|status|logbook|caseId|ADR-NNNN> --editor --json
  seldon plan new --zone <z> --risk <r> [--area <slug>] [--priority <p>] --json -- <title>
  seldon plan start|verify|done|drop <id> --json
  seldon agent start <caseId> --json
  seldon drift link <eventId> <caseId> [--only] --json
  seldon drift explain <eventId> [--only] [--zone <z>] [--risk <r>] [--area <slug>] --json -- <text>
  seldon drift dismiss <eventId> [--only] --json -- <reason>
  seldon drift show <eventId> --json
  seldon decide --no-edit --json -- <title>
  seldon rebuild --json
  seldon update-impact --json
  ```

  `rebuild` and `update-impact` are allowed but not issued by this version.
  Every call goes through one check that refuses any argument list not on
  this list. Ids (case, event, decision, also the one the engine reports
  for a decision it just created) must match their schema pattern; zone,
  risk, priority and area must be known values or a slug; free text (a
  note, a title, an explanation, a reason) is exactly one non-empty
  argument after `--`, without NUL characters.
- **When it runs the engine:** `seldon --version --json` at shell start
  and on *Check again*; `seldon capture` then `seldon status` at start and
  every capture interval; everything else only on your click or key press.
  `seldon open` makes the engine start your editor and
  `seldon agent start` makes it start the agent launcher you configured in
  `~/.config/seldon/config.toml`; those are the engine's actions, under
  your configuration.
- **Two other programs, only when you click a banner button:**
  `wl-copy -- <command>` (*Copy*) and
  `omarchy-launch-floating-terminal-with-presentation <command>`
  (*Install in terminal*, *Run in terminal*, *Update in terminal*).
  `<command>` is always one of five constants in the plugin:
  `curl -fsSL https://github.com/JohnAndrewsX/jax-seldon/releases/latest/download/install.sh | bash`
  (the engine install while the AUR package does not exist; it fetches
  the script over TLS from this project's release and the script checks
  the engine against `SHA256SUMS`), `seldon init`,
  `omarchy plugin update jax.seldon`, `yay -S jax-seldon`,
  `sudo snapper -c root set-config ALLOW_USERS=$USER SYNC_ACL=yes`.
  Omarchy's launcher runs it in a terminal window you see
  (`sudo` asks for your password there).
- **Never shell strings:** every program is started with an argument list,
  without a shell. Nothing from the index or the logbook ever becomes part
  of a command line other than as one validated argument.
- **Writes nothing itself.** Notes, cases, drift resolutions and decisions
  are written by the engine, into your logbook, on your Enter or click.
- **No network.** No sockets, no downloads, no update checks. The one
  exception is yours to click: *Install in terminal* on the
  engine-missing banner runs the `curl … | bash` install in a terminal
  you see.
- **No units, binaries or installers.** The plugin folder holds QML,
  JavaScript, this README, the licence, the manifest and the preview
  image; no
  services, timers, scripts or symlinks. The engine is installed
  separately (`install.sh` from the GitHub release, or the AUR package);
  the plugin never installs it.
- **No privileges.** The plugin never runs `sudo`, `pacman` or `systemctl`.
- **Dev mode is read-only:** with `SELDON_INDEX` set, the plugin only
  probes `seldon --version --json`; it never runs capture, status or any
  writing command.

## Troubleshooting

| Symptom | Try |
|---|---|
| No pill in the bar | `omarchy plugin list` (is `jax.seldon` there and enabled?), `omarchy plugin enable jax.seldon`, `omarchy bar move jax.seldon --section right` |
| A banner instead of data | its button is the fix; see [States](#states) |
| A key or `shell toggle jax.seldon` opens the overlay, not the panel | by design; the panel is `omarchy-shell jax.seldon.panel toggle` |
| An action says the index is behind your logbook | you changed the logbook elsewhere (a terminal); *Capture now* or `seldon status`, then try again |
| Changes to plugin files do not show | `omarchy-restart-shell` (the shell caches plugin components) |
| What does the plugin see? | `omarchy-shell jax.seldon.service status` and `omarchy-shell jax.seldon.panel view` print it as JSON |
| The engine refuses something | run the same command with `seldon … --json` in a terminal for the full message; the engine's own docs are in the project repository |

Plugin warnings go to the shell's log: `journalctl --user -t omarchy-shell`.

## Remove

```sh
omarchy plugin remove jax.seldon
```

This removes the plugin only. The engine, your logbook (`~/Seldon` unless
you chose another place) and the index under `~/.local/state/seldon/`
stay. To remove the engine as well, installed from GitHub:

```sh
curl -fsSL https://github.com/JohnAndrewsX/jax-seldon/releases/latest/download/install.sh | bash -s -- --uninstall
```

It removes exactly the files `install.sh` installed (add the same
`--prefix` if you gave one; `bash install.sh --uninstall` does the same
with a downloaded copy). Installed from the AUR:

```sh
omarchy pkg drop jax-seldon
```

It runs `sudo pacman -Rns`, so it asks for your password (or use your AUR
helper: `yay -R jax-seldon`). Your logbook is yours: Seldon never deletes
it.

## Development

- `SELDON_INDEX=/path/to/index.json` makes the service read that file
  instead (relative paths resolve against the shell's working directory).
  Dev mode is read-only: the engine is probed, never run.
- `SELDON_NOW=<RFC 3339>` (with `SELDON_INDEX`) sets the clock for the
  staleness check; without it a fixture never turns stale.
- After changing plugin code, restart the shell (`omarchy-restart-shell`).
- Tests, the headless harnesses and how `preview.png` is made:
  [`docs/TESTING.md`](https://github.com/JohnAndrewsX/jax-seldon/blob/main/docs/TESTING.md).

## Documentation

| You want to | Read |
|---|---|
| Use Seldon, engine and plugin together | User guide: [English](https://github.com/JohnAndrewsX/jax-seldon/blob/main/docs/user/en/README.md) · [Deutsch](https://github.com/JohnAndrewsX/jax-seldon/blob/main/docs/user/de/README.md) |
| Get going in fifteen minutes | [Getting started](https://github.com/JohnAndrewsX/jax-seldon/blob/main/docs/user/en/01-getting-started.md) · [Erste Schritte](https://github.com/JohnAndrewsX/jax-seldon/blob/main/docs/user/de/01-getting-started.md) |
| Let an AI agent work in your logbook | [Agent guide](https://github.com/JohnAndrewsX/jax-seldon/blob/main/docs/AGENT-GUIDE.md) · [`llms.txt`](https://github.com/JohnAndrewsX/jax-seldon/blob/main/llms.txt) |
| Read what the plugin may do | [Plugin spec](https://github.com/JohnAndrewsX/jax-seldon/blob/main/docs/SPEC-PLUGIN.md) · [Contract](https://github.com/JohnAndrewsX/jax-seldon/blob/main/docs/CONTRACT.md) |
| See what changed | [Changelog](https://github.com/JohnAndrewsX/jax-seldon/blob/main/CHANGELOG.md) |

The plugin only reads Seldon's index; agents work through the engine. An
agent started inside a Seldon logbook follows that logbook's `AGENTS.md`;
the agent guide is its long form.

## Project home, contributing and licence

Project home, issues and the engine:
<https://github.com/JohnAndrewsX/jax-seldon>. This plugin repository is
generated from its `plugin/` folder; please file issues and pull requests
there, following its
[contributing guide](https://github.com/JohnAndrewsX/jax-seldon/blob/main/CONTRIBUTING.md).
Report a vulnerability privately, never in a public issue: see
[`SECURITY.md`](SECURITY.md).

MIT, see [`LICENSE`](LICENSE).
