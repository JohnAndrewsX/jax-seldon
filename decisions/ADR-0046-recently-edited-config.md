# ADR-0046 — The index lists recently edited files under ~/.config outside the watch paths; one click watches one

**Status:** proposed (WP-139)
**Date:** 2026-10-08

> Amends ADR-0038: adds one more **optional** field to contract 2 under
> ADR-0035 §6, before 0.2.0 is tagged — nothing required is added,
> nothing is removed or changes meaning, `contractVersion` stays 2. Adds
> one command form the plugin may run. Operator decision 2026-10-07
> (WP-139, after the comparison with the Omarchy plugin "What changed",
> nejcc.what-changed, MIT: the idea is taken, not its code).

## Context

The config collector hashes the files under `watchPaths` (SPEC-ENGINE
§4): exact (a hash, a ledger line, drift) but narrow — thirteen defaults,
Omarchy's own folders and the persistence paths. A user who edits a
terminal's config, `~/.config/git/config` or `~/.config/starship.toml`
leaves no trace, and does not know that Seldon is blind there. Widening
the defaults would turn every editor and app that writes into `~/.config`
into drift; the narrow list is deliberate (ADR-0028 §4d).

What the user lacks is the knowledge that those edits happened, and a
cheap way to make one of them watched.

## Decision

### 1. The scan

During `seldon capture`, when the config collector runs, the engine walks
`~/.config` once and keeps the **paths and modification times** — never
content — of the newest **80** regular files modified in the **7 days**
before the scan:

- **Outside the watch paths**: a file under a watch path is the config
  collector's.
- **`[redaction] skipPaths` honoured**: a skipped folder is not entered, a
  skipped file not listed (a name pattern matches a folder's name too).
- **Its own ignore list**: `.git`; folders whose name holds `cache`; the
  folders `state`, `log`, `logs`, `history`, `databases`, `IndexedDB`,
  `leveldb`, `Local Storage`, `Session Storage`, `blob_storage`; every
  folder that holds `Cookies` or `Local State` (a browser or Electron
  profile: nothing in it is listed); state, history, log, lock, pid,
  database, key-store and image files and editor temp files by name;
  `shell.json`; Omarchy's plugin folder; Seldon's own config folder (the
  click below edits it).
- **Bounded**: at most 20 000 directory entries read and 12 levels
  below `~/.config`; directory links are never followed, a link to a file
  counts by its target's time. A walk that reaches the entry budget stops
  and marks the state file (`cut`).
- **Never masked**: a `~`-path with a control, direction or format
  character (the set of `import::is_direction_or_format`), longer than 512
  characters, or one the logbook's redaction would change is left out.
  The list's only action needs the real path; a masked one would watch
  nothing.
- **No ledger line**, ever. The scan is not a collector: no cursor, no
  event, no drift.

The result is engine state, `$XDG_STATE_HOME/seldon/recent-config.json`
(`{scannedAt, files: [{path, mtime}], cut?}`), written atomically; a
failure to write it is a capture warning. The walk is a public function
of the engine, so `seldon preview` (WP-138) reuses it.

### 2. The index field

`system.recentConfig` (optional) = `{scannedAt, files: [{path, mtime}]}`,
newest first, at most 80; `path` starts with `~/.config/`, at most 512
characters; `mtime` and `scannedAt` RFC 3339. Every index build reads the
state file and drops again what is by now under a watch path, under a
skipPath, changed by the redaction, or older than 7 days at the build's
time — so the *Watch* click and a new skipPath take effect at the next
index build, without a capture. Absent before the first scan and when a
`[redaction] patterns` entry does not compile (as ADR-0038 §2 withholds
its texts). An unreadable state file is a build warning.

`path` is user content (CONTRACT.md rule 6): shown as plain text, never
evaluated, and passed to the engine only as the single argument after
`--` of §3.

### 3. The click: `seldon config watch --json -- <path>`

A new command, one fixed argument list the plugin may run (CONTRACT.md):
it appends the path, written `~/…`, to `config.toml watchPaths` by the
minimal edit of WP-109 (`with_added_watch_paths`: every other byte of the
file stays, the result must read back as the same file with exactly that
path added; an empty array now takes it too). Under the state lock (exit
4 while a capture holds it). Refused, exit 1, nothing written: a path
not below the home directory (or the home directory itself), one that
holds or lies in Seldon's own files (the logbook, the state directory,
Seldon's config), one under a skipPath, one with a control or format
character or over 512 characters, and a file the minimal edit cannot
extend (the message names the line to add by hand). A path already under
a watch path is exit 0, `added: false`, nothing written. Without a
`config.toml` the defaults plus the path are written. The index is
rebuilt at once.

The next capture takes the files the new path brings in as they are,
without events (WP-069's scope change). Their later edits are
`config-change` events and are classified by ADR-0028 as any other config
path: a path no row names is *attention* by the total row; a path under
`[drift] alwaysRedPaths` stays a crisis. ADR-0028 is unchanged.

### 4. Contract

`contractVersion` stays **2** (ADR-0035 §6). CONTRACT.md rule 9 lists the
field, its command form joins the plugin's fixed forms. An index without
the field (an earlier v2 build) is valid: the desk shows the System
section without the tile.

## Consequences

- **Cost** (bench profile, `just check-perf`, `capture_cost.rs`): the
  scan and the state file add about 2.3 ms to a capture on a synthetic
  lived-in `~/.config` (370 entries read: 40 programs' folders, a browser
  profile, an Electron app and a cache entered only to their first
  level); the index build adds about 0.5 ms for 80 files.
- **Size:** at most 80 × (512 + 25 + keys) bytes, about 46 KB at the
  worst, usually under 8 KB.
- **Privacy:** paths and times of files the user can read, in the user's
  own state directory, never content. The ignore list keeps browser
  profiles, key stores and databases out; skipPaths and the redaction
  keep what the user marked secret out.
- A config whose `watchPaths` equals the current default list stops being
  a default list after a click: a later engine's new defaults are no
  longer appended automatically (ADR-0028 §4d); `doctor`'s `watch` row
  names them, as for any list the user changed.
- Fixtures: `fixtures/state/recent-config.json` (the engine's state
  directory) feeds the sample's `system.recentConfig`; it lists one file
  under a watch path and one older than 7 days, which both drop out.
- SPEC-ENGINE §2 (the state file), §3 (`config watch`), §4 (the scan),
  §6 (the field); SPEC-PLUGIN (the System tile); CONTRACT.md rule 9 and
  the command forms.

## Alternatives considered

- *Widen the default `watchPaths` to `~/.config`:* every app that writes
  its state there would be drift; rejected by ADR-0028 §4d's reasoning.
- *Record the edits in the ledger:* an mtime is no evidence of what
  changed and from what; the ledger holds what Seldon can stand behind.
  The list is informational and forgets.
- *Scan at index time:* the index is rebuilt by every writing command
  and by the hooks (WP-009); a walk of `~/.config` there would multiply
  its cost. Once per capture is enough for a 7-day list.
- *Only into `seldon preview`'s shape* (WP-138): the preview serves the
  desk before `init`; after it the System section needs the list too, so
  the index carries it.
- *Show a redacted path:* the click would watch a path that does not
  exist; leaving it out is honest.
