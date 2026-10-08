# ADR-0047 — Before the logbook exists, the desk shows what the machine remembers on its own (`seldon preview --json`): one plugin-command row and its output schema

**Status:** proposed (WP-138; the orchestrator or the operator accepts)
**Date:** 2026-10-08

> Adds one row to CONTRACT.md's "Commands the plugin may run" and fixes
> the shape of that command's output in a new `schema/preview.schema.json`.
> ADR-0035 §6 covers neither: it lets a later ADR on `next` add optional
> fields to the v2 schemas, not commands or new schema files. ADR-0044
> set the precedent that a new command row is a contract change recorded
> by an ADR. The index does not change: `contractVersion` stays 2 and
> no index fixture changes (§7 says why AGENTS.md §3's bump rule does
> not apply). Operator decision 2026-10-07 (WP-138, after
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
   `pacman.log` (from its end, at most 8 MiB), the modification times
   of the files under `$XDG_CONFIG_HOME` (default `~/.config`), and
   `config.toml` when present: its `[redaction] skipPaths` and
   `patterns`. A config that cannot be read, or whose patterns do not
   compile, withholds what they filter: the file list and the pacman
   command lines.
2. **Bounds:** `--days` 1–7 (default 7); at most 200 rows, one per
   transaction and one per file; at most 80 files (the newest); at most
   10 packages listed per transaction, `count` holds all. Time: the walk
   of `~/.config` runs first and stops 0.25 s after it started (or after
   200 000 directory entries), then says `files.partial`; then at most
   the last 8 MiB of `pacman.log` are parsed (21 ms per MiB on the dev
   host's release build, so at most about 0.17 s), a log whose last 8 MiB
   are all inside the window says `pacman.partial`. Together the preview
   takes under 0.5 s on the dev host's release build with a warm cache:
   the worst case measured, a 100 MiB log dense to its end and 50 000
   files under `~/.config`, took 0.26–0.30 s (WP-138 round 2). A slower
   machine or a cold cache can take longer for the pacman part; the walk
   keeps its own 0.25 s. `truncated` says a bound cut rows.
3. **What the walk ignores:** symbolic links (never followed), caches,
   browser and Electron profiles (any folder holding `Cookies` or `Local
   State`), state, logs, locks, databases (SQLite, `*.db`, LevelDB,
   IndexedDB, Local and Session Storage, dconf), images, `.git`, Omarchy's
   plugin folder and `omarchy/shell.json`, editor swap files, `*~` and
   `*.bak.*`, and every `[redaction] skipPaths` match. Paths only (`~/…`,
   redacted, control, bidi and format characters — the set the index
   drops, ADR-0038 as amended by WP-140 — as U+FFFD), never content. The
   pacman side holds the same rule: package names, versions and the
   command line are one line each (the same characters as
   U+FFFD) and clipped to the schema's bounds (512, 256, 256 characters);
   pacman's grammar takes any non-blank word, so the log alone does not
   guarantee it.
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

7. **No `contractVersion` bump.** AGENTS.md §3 says a change to a
   schema needs an ADR, a `contractVersion` bump, fixtures and both
   sides, "no exceptions". The bump does not apply here, and this ADR is
   the record of why: `contractVersion` versions `index.json`, the one
   file both sides exchange without asking; the bump exists so that a
   plugin and an engine of different contracts notice the mismatch on
   that file (CONTRACT.md rule 3). `preview.schema.json` is a new,
   separate schema for the stdout of one command, and nothing in the
   index or in an existing schema changes. No mismatch can go unnoticed:
   a plugin older than this ADR never runs `seldon preview` (its
   `validateArgs` refuses the argv); a plugin with it, meeting a 0.1.x
   engine, gets clap's usage error (exit 1, JSON `error.message`), which
   the card shows while **Set up Seldon** still works; the output itself
   carries `contractVersion: 2`, and `Model.previewResult` refuses any
   other. The rest of §3's rule holds: this ADR, the fixtures
   (`preview.sample.json`, four invalid ones) and both sides land in one
   PR. A later change to `preview.schema.json` that a released plugin
   would misread needs a bump like any other.

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
