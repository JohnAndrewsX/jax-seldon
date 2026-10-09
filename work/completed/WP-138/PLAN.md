# WP-138 plan — before init: a preview without memory

Role: Plugin Dev with an engine part. Branch `wp/138-preview-before-init`
from `next`, worktree `wt/WP-138`.

## Inputs read

- `work/queued/WP-138.md`, AGENTS.md (§3 contract, §6 zones, §7 rules).
- WP-119 (queued, not built: the setup card does not exist yet; today the
  `notInitialised` banner's *Create* opens the init terminal script) and
  WP-139 (queued, running in parallel on `wp/139-recent-config`, nothing
  committed yet: it wants the same `~/.config` scan during capture).
- The pacman collector (`collectors/pacman.rs`: `parse`, `Tx`, the line
  table), the config collector's `SkipPaths`, `config.rs` (`Dirs`,
  `DEFAULT_SKIP_PATHS`, `DEFAULT_ROUTINE_PATHS`), `redact.rs`.
- ADR-0035 (§6: within contract 2 only optional fields of the v2 schemas),
  ADR-0044 (a new plugin-command row is a contract change recorded by an
  ADR; `contractVersion` stays 2 when the index does not change).
- docs/CONTRACT.md, SPEC-ENGINE §3, SPEC-PLUGIN §5.4 (Today, the
  `todaySetupSlot`), `plugin/Service.qml` (the read-only `EngineCall`s
  `doctorCall`, `sessionsCall`), `Model.validateArgs`,
  `sections/Today.qml`, `tests/plugin/desk-view.sh`, `fake-seldon`,
  `scripts/validate-fixtures.py`, `scripts/docs-check.py` (the CLI
  reference carries every command's `--help`).

## Contract

The index does not change. New: one command the plugin may run
(`seldon preview --json`) and the shape of its output,
`schema/preview.schema.json`. ADR-0035 §6 does not cover a new schema
file or a command row, so a short **ADR-0047** (proposed; the orchestrator
or operator accepts) records both, following ADR-0044's precedent:
`contractVersion` stays 2, the plugin reads the command's stdout like
`plan show --json`, never a file.

## Engine: `seldon preview [--days N] [--json]`

- Read-only: no logbook needed (works with or without one), no lock, no
  state, no config written; `config.toml` is read when present (its
  `skipPaths` and redaction patterns). A config that cannot be read or
  whose patterns do not compile withholds what it would filter (the file
  list, the pacman command lines), as `ShownMessages` does.
- `--days` 1–7, default 7.
- **pacman**: the tail of `pacman.log` (`SELDON_PACMAN_LOG`), read
  backwards in growing slices until the slice starts before the window
  (cap 64 MiB), parsed with the collector's own `parse`; package lines in
  the window, grouped by transaction, newest first: `{at, command?,
  status?, count, packages: [{kind, name, version | from+to}]}`. The
  command line redacted and clipped to 256 bytes. A missing or unreadable
  log is `pacman.ok: false` with a message, never an error exit.
- **files**: a bounded mtime scan of `$XDG_CONFIG_HOME` (default
  `~/.config`) in a neutral module `engine/src/config_scan.rs`, so
  WP-139 can call it from capture: regular files only, symlinks never
  followed, modified within the window, newest 80. Ignored: cache
  folders, browser and Electron profiles (any folder holding `Cookies` or
  `Local State`), `state`, logs, locks, databases (sqlite, `*.db`, LevelDB,
  IndexedDB, Local/Session Storage), images, `.git`, Omarchy's plugin
  folder, `omarchy/shell.json`, editor swap and backup files, and every
  `[redaction] skipPaths` match (the defaults without a config). Paths
  only, shown `~/…`, through the redaction; never content.
- **Bounds**: at most 200 rows (files first, at most 80; package lines
  fill the rest, newest transaction first, a cut transaction keeps its
  `count`), `truncated` when anything was cut; the scan stops at a deadline
  (0.5 s total from the start of the command) and an entry budget, then
  says `files.partial: true`. `elapsedMs` reported.
- Exit 0 always for a completed preview; 1 for a bad `--days`.

## Plugin

- `Model.validateArgs`: `preview --json`, nothing else.
- `Service.qml`: `previewCall`, its own read-only process beside the
  queue (as `doctor` and `agent sessions`); run when the status becomes
  `notInitialised` with an engine present, again when the desk opens and
  the last answer is older than `Model.PREVIEW_REFRESH_MS`; never in dev
  mode. `Model.previewResult` checks the shape (rows, bounds) and keeps
  only what the schema allows.
- Today, while `notInitialised`: the list shows the preview rows
  (PACKAGES · LAST 7 DAYS: one row per transaction; EDITED UNDER
  ~/.config: one row per file) instead of "No index to show"; the
  overview's `todaySetupSlot` holds the preview card: "This is without
  memory: no who, no why, gone when the logs rotate. Set up Seldon?" and
  **Set up Seldon**, which is the `notInitialised` banner's terminal fix
  (`service.fix("terminal", "status")`, the init script; WP-119 replaces
  it with its card). Empty and failed previews say so in one line.

## Tests

- Engine unit tests: the tail reader (window edges, a transaction across
  the window start, a log with no line in the window, a 64 MiB cap), the
  scan's ignore rules, skipPaths, bounds (80 files, 200 rows), symlinks,
  the deadline.
- Engine integration (`tests/preview.rs`): output validates against
  `preview.schema.json`; no logbook, no state dir, no config written
  (tree compared before and after); `--days 8` exit 1; missing pacman.log;
  a broken config withholds; budget on a large synthetic log (< 0.5 s,
  release-independent bound documented).
- Fixtures: `fixtures/preview.sample.json` (valid) and
  `fixtures/invalid/preview.*.json`, mapped in `validate-fixtures.py`.
- Plugin: `model.test.js` (validateArgs, previewResult, previewRows);
  `fake-seldon` answers `preview --json` from the fixture; desk-view
  case `preview-uninit` (live, `FAKE_SELDON_MODE=uninit`): rows, the
  card's sentence, the click runs the init terminal script; the dev-mode
  `uninit` case runs no preview.
- Hand mutants on the new code; `just check` with the private runtime dir.

## Docs

SPEC-ENGINE §3 (the command), CONTRACT.md (the row and the output),
SPEC-PLUGIN §5.4 (Today before init), docs/user CLI reference (en, de via
`docs-check.sh --write`), CHANGELOG, DECISIONS.md (ADR-0047).

## Not in scope

WP-119's setup card and `init --defaults`; WP-139's capture-time list and
"Watch this path". Live check on the fresh test host: the orchestrator's
deploy (E38: dev builds go to the test host only).
