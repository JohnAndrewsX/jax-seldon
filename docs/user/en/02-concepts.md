# Concepts

This page explains the ideas behind Seldon: the logbook, cases, zones,
risk, drift, the baseline, crises, decisions and memory. Read it once.
The other pages use these words without explaining them again.

## The idea

Your machine changes every day. You install packages, Omarchy updates
itself, an agent edits a config file. A few weeks later nobody can say
why something is there. The system log has the facts but not the reasons.
A hand-written changelog has the reasons until you stop writing it.

Seldon keeps both and compares them:

- It records what changed. Collectors read the package log, snapper,
  the Omarchy version, the shell plugins, the theme and your config files.
  Agent hooks record the commands an agent runs.
- It holds what you plan to change. You write cases: one Markdown file per
  planned change.
- It shows the difference. A change that no case covers and that is
  worth a look is drift. You may say what it was; you never have to.

Seldon is a recorder. It never changes your system, never runs a package
manager or `sudo`, never blocks a command and never sends anything over
the network. Its name comes from Hari Seldon in Asimov's *Foundation*:
the Plan predicts, and a crisis is where reality leaves the Plan.

## The three parts

| Part | What it is | What it does |
|---|---|---|
| Logbook | a folder, `~/Seldon` by default | holds the record as Markdown and JSON lines; a git repository |
| Engine | the program `seldon` | the only writer of the logbook; you and your agents call it |
| Plugin | `jax.seldon` in the Omarchy bar | shows the logbook; every action calls the engine |

The plugin reads one file, the index
(`~/.local/state/seldon/index.json`). The engine rebuilds it after every
command. You never edit it; deleting it loses nothing, because
`seldon status` writes it again.

## The logbook

The logbook is a normal folder of Markdown files. You can read it in any
editor, in Obsidian or on GitHub. It has two kinds of record:

- The ledger (`ledger/YYYY-MM.jsonl`) is written by the machine. Each
  line is one event: a package installed, a theme switched, a config
  file changed, a note written, a case started. The ledger is
  append-only. Nothing in it is ever edited or deleted; a correction is a
  new event.
- The journal (`journal/YYYY/YYYY-MM-DD.md`) is written by people and
  agents. It holds the why: what you tried, what you learned.

Seldon links the two by case ids: a journal entry and the events it
talks about carry the same case.

Each event has a source. There are nine:

| Source | What it records |
|---|---|
| `pacman` | packages installed, removed, upgraded, downgraded |
| `snapper` | snapshots created and deleted |
| `omarchy` | Omarchy version changes |
| `plugins` | shell plugins added, removed, enabled, disabled, updated |
| `theme` | theme switches |
| `config` | files added, changed or removed under the watched paths |
| `agent` | commands an agent ran, recorded by a hook |
| `manual` | notes (`seldon log`) and events you record by hand |
| `seldon` | the engine's own steps: case created, started, closed, drift resolved |

[The logbook](07-the-logbook.md) describes every folder.

## Cases

A case is one planned change, in one Markdown file under `work/`. It
has an id like `C-2026-004`, a title, a zone, a risk, an area and four
sections. An *area* is a long-lived topic of the machine, with a folder
under `areas/`. A new logbook has six: `hyprland`, `themes`, `packages`,
`dev-env`, `plugins` and `shell`. A new area name creates its folder on
first use. The four sections are:

- *Intent*: why, and what is different afterwards.
- *Plan*: goal, steps, affected paths, rollback, verification.
- *Log*: dated lines, append-only. The engine adds one for every step.
- *Result*: what came out.

You write *Intent*, *Plan* and *Result*. The engine keeps the rest: the
status, the list of linked events, the agents that worked on it.

A case moves through these statuses. The engine moves the file between
folders; you never move it yourself.

| Status | Folder | Command that gets it there |
|---|---|---|
| queued | `work/queued/` | `seldon plan new` |
| active | `work/active/` | `seldon plan start` |
| verification | `work/active/` | `seldon plan verify` |
| completed | `work/completed/` | `seldon plan done` |
| dropped | `work/completed/` | `seldon plan drop` (from queued, active or verification) |

The order is fixed: queued, active, verification, completed. A completed
or dropped case stays closed. Every step is a ledger event and a git
commit.

The case started last is the *active case*. Its id is in
`.seldon/active-case`. Agent hooks stamp every command they record with
it, so the case collects its *trace*: the events recorded for it, in
order.

## Zones

Every event and every case has a zone. The zone says how much the
change touches the system.

| Zone | What falls in it |
|---|---|
| red | packages, Omarchy itself, systemd units (also unit files under `~/.config/systemd/`), `/etc`, the boot loader |
| yellow | config files under the watched paths, themes, shell plugins |
| green | everything the collectors do not track: project files, language package managers, `git` |

Snapshots, notes and the engine's own steps have no zone. Green
changes are recorded only by agent hooks, and only while a case is active.

## Risk

Every case also has a risk, from `R0` to `R3`. The risk is your
estimate of what it takes to undo the change.

| Risk | Meaning |
|---|---|
| R0 | reversible in seconds, nothing depends on it; you undo it by hand, no snapshot |
| R1 | reversible by hand in minutes with a known command; the Plan names the rollback step |
| R2 | rollback needs the plan and a snapshot or backup; start with `--snapshot`, verify before closing |
| R3 | can break boot, login or the shell; snapshot mandatory, your explicit go for each step, never unattended |

Seldon stores the risk and shows it. It does not enforce it. A new case
starts as yellow, `R1`, unless you say otherwise.

## Drift

Every change is recorded. Drift is a recorded change that has no case,
has no resolution yet, and is worth a look. Only changes can be drift:
packages, Omarchy, plugins, theme and config. Snapshots, notes and case
steps never are.

Most changes are linked to a case on their own:

- An agent's command carries the active case, because the hook writes it
  into the event.
- A package that came in as a dependency of a package in a case belongs
  to that case.

A change without a case is sorted by what a wrong one would cost, not by
who made it:

| Class | Examples | What happens |
|---|---|---|
| routine | a theme switch, a plugin toggle, adding Seldon's own plugin, a plain full upgrade (`pacman -Syu`, `omarchy update`, kernels included), Omarchy's own copy of a file, `shell.json` | history in the Changelog, not drift; nobody is asked |
| attention | a package installed or removed by name, a third-party plugin added or updated, an override under a watched path, a removed file | open drift, quietly: the panel lists it, the bar does not count it |
| crisis | an `alwaysRed` package installed or removed by name, a new file in a persistence path such as `~/.config/systemd/user` or Omarchy's hooks | see [Crisis](#crisis) |

Your own changes in the terminal are recorded like any other: a
collector sees that a package came in, not that you meant it for a case.
If an open case names the changed package, path or theme in its *Plan*,
Seldon proposes that case (the *proposed case*), and the panel shows it
preselected, also for a routine change.

You may resolve drift, you never have to. There are three ways:

| Action | Command | Use it when |
|---|---|---|
| link | `seldon drift link <EVENT> <CASE>` | the change belongs to a case (open or closed) |
| explain | `seldon drift explain <EVENT> -- "<why>"` | it had a reason but no case; Seldon creates a completed case with your text as its title |
| dismiss | `seldon drift dismiss <EVENT> -- "<reason>"` | it does not matter (a dependency, noise) |

The event stays in the ledger either way. The resolution is a new event
that points to it. Packages from one transaction form one drift item, a
*transaction group*: an install of a package with ten dependencies is one
item, and one command resolves all of them. Add `--only` to resolve that
one event alone.

Agents see the open items too. The context every agent session starts
with lists the crises and attention items of the last seven days. An
agent explains or links an item only when its own *Log*, a hook event or
your words prove why it happened; otherwise it leaves the item open.

## Crisis

A crisis is a change that can break boot, login, the shell or security,
and that nobody asked for in a case. It is the one change Seldon makes
loud: the pill counts it in your theme's error colour, and the panel
shows the line "N changes that can affect boot, login or the shell have
no case". You are told once. Nothing else is required of you.

It has the same three actions. An agent may not explain or dismiss a
crisis; it may link one only to its own active case, and otherwise tells
you about it in one line.

Routine upgrades are not crises, even when they bring a new kernel. A
package from the always-red list (`linux*`, `systemd`, `glibc`,
`hyprland`, `omarchy`, `quickshell` by default; see
[Configuration](06-configuration.md#drift)) is a crisis only when it is
installed or removed by name outside a case. A new file in a persistence
path is a crisis whoever wrote it: units in `~/.config/systemd/user`,
Omarchy's hooks in `~/.config/omarchy/hooks`, `~/.config/autostart`,
`~/.config/environment.d`, `~/.config/uwsm`, `~/.profile`,
`~/.bash_profile`, and the `authorized_keys` files once you watch them. Those
files run at login or on events without being your ordinary
configuration. A third-party plugin edited in place counts as a plugin
update: listed, never counted in the bar.

## Baseline

A new logbook also *backfills*: its first capture records the last 90
days from the package log and snapper. None of those older changes
belongs to a case, so many of them would open as drift.

The baseline settles that. The setup card and `seldon init` dismiss every
item the backfill opens with the reason "before Seldon", without a
question. The events stay in the ledger and on the charts; they no
longer ask for a reason. `seldon init --ask` lets you choose the date and
asks before it dismisses; `seldon init --non-interactive` records from
now on, so there is nothing to baseline.

## Decisions

A decision is a short record of a choice that shapes the machine,
in the ADR format (architecture decision record): context, decision,
consequences. `seldon decide -- "<title>"` creates
`decisions/ADR-NNNN-<slug>.md` with the status *proposed* and opens it in
your editor. *Accept* in the desk's Decisions section (twice: the first
click arms it) or `seldon decide accept ADR-NNNN` makes it *accepted*
with today's date and notes it in the ledger. Accepting is yours: an
agent may propose a decision, never accept one. *Superseded* you set in
the file. The Decisions section lists them.

## Memory

Memory is what agents learned about this machine, in `memory/`.
`memory/lessons.md` has one `##` heading per lesson: what happened and
what to do next time. Every agent reads it at the start of a session.
Other files, such as `memory/hyprland.md`, hold notes on one topic. You
and your agents write these files directly; the engine only reads them.

## The dossier

The dossier (`system/`) describes the machine as it is now: packages,
services, Omarchy version and theme, hardware, plugins and the files you
changed against Omarchy's defaults. The engine fills the parts between
`<!-- seldon:begin … -->` and `<!-- seldon:end -->` markers; you write
around them. See [Rebuild, dossier and update impact](08-rebuild-dossier-update-impact.md).

## Capture

A capture runs the collectors and appends new events to the ledger.
It happens:

- when you run `seldon capture`;
- every 15 minutes while the plugin runs, and when the shell starts;
- when you press *Capture now* in the panel or right-click the pill;
- at the end of an agent session that has Seldon's hooks.

Running it twice in a row writes nothing the second time. Each collector
remembers how far it read.

---

Previous: [Getting started](01-getting-started.md) · [Index](README.md) · Next: [Daily use](03-daily-use.md)
