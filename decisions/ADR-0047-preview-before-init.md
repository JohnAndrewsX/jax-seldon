# ADR-0047 — Before the logbook exists, the desk shows what the machine remembers on its own (`seldon preview --json`): one plugin-command row and its output schema

**Status:** proposed (WP-138; the orchestrator or the operator accepts)
**Date:** 2026-10-08

> Adds one row to CONTRACT.md's "Commands the plugin may run" and fixes
> the shape of that command's output in a new `schema/preview.schema.json`.
> ADR-0035 §6 covers neither: it lets a later ADR on `next` add optional
> fields to the v2 schemas, not commands or new schema files. ADR-0044
> set the precedent that a new command row is a contract change recorded
> by an ADR. The index does not change: `contractVersion` stays 2 and
> no index fixture changes. Operator decision 2026-10-07 (WP-138, after
> the comparison with the Omarchy plugin "What changed": the idea is
> taken, not its code).

## Context

Until `seldon init` has run, the desk is empty: every section says "No
index to show" and the status notice offers the setup. Yet the machine
already remembers its last days without Seldon — pacman's log and the
modification times under `~/.config` — only without who and why, and
only until the logs rotate. Showing that before the setup makes the
difference Seldon makes concrete.

The plugin reads only `index.json` and the files it points to (AGENTS.md
§3, CONTRACT.md rule 1), and there is no index before `init`; it never
reads `/var/log` or `~/.config` itself. So the engine must produce the
preview, and without a logbook.

## Decision

1. **`seldon preview [--days N] [--json]`** (SPEC-ENGINE §3): read-only,
   no logbook needed (one that exists is not read), no lock, nothing
   written — no logbook, no state directory, no config. It reads
   `pacman.log` (from its end, at most 64 MiB), the modification times
   of the files under `$XDG_CONFIG_HOME` (default `~/.config`), and
   `config.toml` when present: its `[redaction] skipPaths` and
   `patterns`. A config that cannot be read, or whose patterns do not
   compile, withholds what they filter: the file list and the pacman
   command lines.
2. **Bounds:** `--days` 1–7 (default 7); at most 200 rows, one per
   transaction and one per file; at most 80 files (the newest); at most
   10 packages listed per transaction, `count` holds all; the walk of
   `~/.config` stops 0.4 s after the command started (the whole preview
   stays under 0.5 s) and after 200 000 directory entries, then says
   `files.partial`. `truncated` says a bound cut rows.
3. **What the walk ignores:** symbolic links (never followed), caches,
   browser and Electron profiles (any folder holding `Cookies` or `Local
   State`), state, logs, locks, databases (SQLite, `*.db`, LevelDB,
   IndexedDB, Local and Session Storage, dconf), images, `.git`, Omarchy's
   plugin folder and `omarchy/shell.json`, editor swap files, `*~` and
   `*.bak.*`, and every `[redaction] skipPaths` match. Paths only (`~/…`,
   redacted, control and bidi characters as U+FFFD), never content.
4. **The output** is `schema/preview.schema.json` (closed objects):
   `{contractVersion: 2, generatedAt, since, days, pacman: {ok, message?,
   partial, transactions: [{at, command?, status?, count, kinds,
   packages: [{kind, name, version | from + to}]}]}, files: {ok,
   message?, partial, root, items: [{path, modified}]}, truncated,
   elapsedMs}`. Every name, path and command line is user content
   (CONTRACT.md rule 6): shown as plain text, never an argument of a
   command. `fixtures/preview.sample.json` is the engine's own output for
   the fixture home of `engine/tests/preview.rs`, held equal by that test.
5. **One row in CONTRACT.md:** `seldon preview --json` — read-only, its
   own process beside the queue (as `doctor --only rules` and `agent
   sessions`), never in dev mode, only while the status is
   `notInitialised`. The plugin re-checks the shape and the bounds
   (`Model.previewResult`) and drops what it cannot read.
6. **The desk** (SPEC-PLUGIN §5.4): Today lists the transactions and
   files while the logbook is not initialised, and the overview's setup
   slot holds the card "This is without memory: no who, no why, gone when
   the logs rotate. Set up Seldon?" with **Set up Seldon**, the
   notInitialised notice's own terminal fix. WP-119's setup card takes
   the slot over.

## Consequences

- A new user sees their machine's last week before the first question.
- The `~/.config` walk is a neutral module (`engine/src/config_scan.rs`)
  that WP-139 can run during capture with its own bounds.
- `seldon preview` is a reader of `/var/log/pacman.log` and of
  modification times under `~/.config` (AGENTS.md §6: collectors read
  pacman.log; mtimes are metadata, not content). It never runs a program.
- The contract's command list grows by one read-only row; an engine
  without the command (0.1.x) answers it with a clap error, which the
  plugin shows on the card; its Set up Seldon still works.

## Alternatives considered

- *A preview section in `index.json`*: there is no index before `init`,
  and writing one would write the state directory before the user chose
  Seldon. Rejected.
- *The plugin reads pacman.log itself*: breaks AGENTS.md §3 (the plugin
  reads only the index). Rejected.
- *Hashes or content of the config files*: the preview needs only "what
  changed when"; content would widen what Seldon reads before consent.
  Rejected.
