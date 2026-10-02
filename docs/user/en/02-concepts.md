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
- It shows the difference. A change that no case covers is drift.
  You decide what it was.

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

Drift is an event that changes the system, has no case and has no
resolution yet. Only changes can be drift: packages, Omarchy, plugins,
theme and config. Snapshots, notes and case steps never are.

Most changes are linked to a case on their own:

- An agent's command carries the active case, because the hook writes it
  into the event.
- A package that came in as a dependency of a package in a case belongs
  to that case.

Everything else is drift. That includes all your own changes, in the
terminal or anywhere else: a collector sees that the theme changed, but
not that you meant it for a case. Only an agent's command, recorded by a
hook, carries the active case. If an open case names the changed
package, path or theme in its *Plan*, Seldon proposes that case (the
*proposed case*), and the panel shows it preselected.

You resolve drift in one of three ways:

| Action | Command | Use it when |
|---|---|---|
| link | `seldon drift link <EVENT> <CASE>` | the change belongs to a case (open or closed) |
| explain | `seldon drift explain <EVENT> -- "<why>"` | it had a reason but no case; Seldon creates a completed case with your text as its title |
| dismiss | `seldon drift dismiss <EVENT> -- "<reason>"` | it does not matter (a dependency, noise) |

The event stays in the ledger either way. The resolution is a new event
that points to it. Packages from one transaction form one drift item, a
*transaction group*: a routine upgrade of forty packages is one item, and one command resolves
all of them. Add `--only` to resolve that one event alone.

## Crisis

A crisis is drift in the red zone. It has the same three actions. The
pill and the panel show crises first, in your theme's error colour, with
the line "N changes in the red zone need a reason".

Routine upgrades are not crises. A transaction that only upgrades
packages, from a full system upgrade, is yellow drift. It turns red when
it installs or removes a package, or when it touches a package on the
always-red list (`linux*`, `systemd`, `glibc`, `hyprland`, `omarchy`,
`quickshell` by default; see [Configuration](06-configuration.md#drift)).

## Baseline

A new logbook records from the moment you create it. The wizard can also
*backfill*: record changes since a past date, from the package log and
snapper. None of those older changes belongs to a case, so each one opens
as drift, and most of them as crises.

The baseline settles that. After a backfill the wizard asks whether
to mark everything it found as the pre-Seldon baseline. Say yes, and the
engine dismisses every open item with the reason "pre-Seldon baseline".
The events stay in the ledger and on the charts; they no longer ask for
a reason. Without a backfill there is nothing to baseline.

## Decisions

A decision is a short record of a choice that shapes the machine,
in the ADR format (architecture decision record): context, decision,
consequences. `seldon decide -- "<title>"` creates
`decisions/ADR-NNNN-<slug>.md` with the status *proposed* and opens it in
your editor. You change the status to *accepted* or *superseded* in the
file. The Decisions tab lists them.

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
