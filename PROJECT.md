# PROJECT.md — Seldon

## Goal

Build **Seldon**: an Omarchy Quattro plugin plus a Rust engine that together
keep a complete, agent-friendly, Markdown-based logbook of how an Omarchy
system evolves day by day, let the user plan changes, and make the execution
of those changes by AI agents traceable afterwards.

One sentence for the marketplace listing:

> Seldon records every change to your Omarchy system, matches it against
> what you planned, and shows the difference.

## Why

Omarchy is an agent-first OS. On a developer's machine, agents install
packages, edit configs, build themes and plugins, and tune the shell every
day. Within weeks nobody — human or agent — can say *why* the system looks
the way it does. Existing tools either log facts without intent
(`pacman.log`, `journalctl`) or record intent without facts (a hand-written
changelog that dies after two weeks). Seldon joins the two.

## Scope

In scope:

- The **logbook**: a Hermes-style project folder (`AGENTS.md`, `PROJECT.md`,
  `STATUS.md`, `DECISIONS.md`, `inbox/`, `journal/`, `ledger/`, `work/`,
  `areas/`, `system/`, `memory/`, `decisions/`, `resources/`, `outputs/`,
  `archive/`), readable without any tool, usable as an Obsidian vault.
- The **engine** `seldon` (Rust): setup wizard, collectors, journal and case
  commands, drift reconciliation, index generation, agent hooks, rebuild
  document, update-impact report.
- The **plugin** `jax.seldon` (QML): service, bar widget, panel with tabs,
  Prime Radiant overlay with visualisations.
- The **contract** (JSON Schema) between engine and plugin.
- Claude Code hook integration; generic hook interface for other agents.
- AUR packaging of the engine; marketplace submission of the plugin.

Out of scope (for v1):

- Executing system changes. Seldon observes, plans and records. It never
  runs `pacman`, never edits `/etc`, never touches `sudo`.
- A sandbox or permission system for agents (that is the agent harness's job;
  the Omarchy-Agent kit's zone model and `guard.py` stay where they are).
- Cloud sync, accounts, telemetry. Everything is local.
- Replacing Obsidian. Obsidian is an optional viewer, never a dependency.

## Users

1. **Eugen** — power user, developer, runs several agents (Claude Code,
   Codex, the Omarchy default agent) on one machine. Wants the full history
   and the planning desk.
2. **Any Omarchy user** who installs the plugin from the marketplace. Must
   get value within five minutes of `seldon init`, without ever opening a
   Markdown file.
3. **Agents** — every CLI command has `--json`, every write goes through the
   engine, every hook is one shell line.

## Definition of done (v1.0)

- `seldon init` creates a working logbook in under a minute, interactive or
  `--non-interactive`.
- Collectors for pacman, snapper, Omarchy (version, updates), plugins, theme
  and `~/.config` produce ledger events with zero duplicates across runs.
- `seldon status` regenerates `STATUS.md` and the JSON index in under 100 ms
  on a logbook with 10 000 events.
- The plugin shows Today, Changelog, Work, Decisions, System and Memory in
  the panel, and the Prime Radiant overlay renders the activity heatmap,
  package series, drift curve and case timeline from the index alone.
- Drift is computed, shown, and resolvable (link to case, explain, dismiss).
- A Claude Code session inside the logbook writes ledger events for every
  mutating command it runs, tagged with agent and case.
- `seldon rebuild` produces `outputs/REBUILD.md` that a fresh Omarchy install
  can follow to reach the documented state.
- Engine is on the AUR as `jax-seldon`; plugin passes `omarchy plugin
  validate` and `qmllint`; marketplace listing is live.
- Everything above is covered by tests that the orchestrator can run
  unattended (`cargo test`, schema validation, `omarchy plugin validate`,
  QML lint, fixture-driven UI smoke test).

## Non-goals we will be tempted by

- Building a general PKM tool. Seldon is about one machine.
- A QML setup wizard. The wizard is a terminal program.
- A daemon by default. Timers in the shell service are enough for v1.
- SIMD, async runtimes, or any performance work before a measured need.
