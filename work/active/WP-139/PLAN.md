# WP-139 — plan

Branch `wp/139-recent-config` from `next` (aa9c5902), merges into `next`.
Basis: ADR-0035 §6 (optional fields within contract 2 before 0.2.0 is
tagged), CONTRACT.md rule 9, ADR-0038 (the last such amendment).

## Steps

1. **ADR-0046 (proposed), an ADR-0038 amendment:** one optional index
   field, `system.recentConfig = {scannedAt, files: [{path, mtime}]}`,
   and the fixed-argv command `seldon config watch --json -- <path>`.
   `contractVersion` stays 2. ADR-0028 is unchanged: a newly watched
   path outside the listed rows is *attention* by its total row.
2. **Scan (`engine/src/collectors/recent.rs`, no collector):** an mtime
   walk of `~/.config` during `capture` when the config collector runs.
   - Bounds: 7 days, the newest 80, at most 20 000 entries read, depth
     12; regular files only; no directory link followed (a link to a
     file is stat'ed, never read).
   - Own ignore list: `.git`; any folder whose name holds `cache`; any
     folder holding `Cookies` or `Local State` (browser and Electron
     profiles); state, log, lock, database, image and editor temp files
     by name; the plugin folder `~/.config/omarchy/plugins/`,
     `~/.config/seldon/`, `shell.json`.
   - `[redaction] skipPaths` honoured (a skipped folder is not entered);
     every path under a watch path left out.
   - A `~`-path with a control character or over 512 characters, or one
     the logbook's redaction would change, is left out (never shown
     masked: the click needs the real path).
   - Result in `$XDG_STATE_HOME/seldon/recent-config.json`; paths and
     mtimes only, never content. A failure is a capture warning.
   - Public `scan` for WP-138's `seldon preview`.
3. **Index:** `system.recentConfig` from that file, filtered again at
   build time against the current `watchPaths`, `skipPaths`, the
   redaction and the 7 days, so a click or a new skipPath takes effect
   at the next index build, without a capture.
4. **`seldon config watch <PATH>`:** under the state lock; the path
   resolves as a watch path does (§2), must lie under the home
   directory, outside Seldon's own files and outside every skipPath;
   already covered → exit 0, `added: false`, nothing written. Appended
   to `watchPaths` as `~/…` by the minimal edit of WP-109
   (`with_added_watch_paths`, extended to an empty array); a file that
   cannot be edited so → exit 1, nothing written. Rebuilds the index.
   The next capture takes the file in without an event (WP-069 scope
   change).
5. **Schema, fixtures, derive:** `system.recentConfig` in
   `index.schema.json`; the sample gets a list; the golden test writes
   the state file; `validate-fixtures.py` copies it as it copies `state`
   (state-derived) and checks its invariants.
6. **Plugin:** a sixth System tile, *Recently edited*: the count, and in
   the detail one row per file — path, time, "not watched", *Watch* —
   which runs `config watch --json -- <path>` (Model.validateArgs form,
   path checked). Result line under the list.
7. **Docs:** SPEC-ENGINE §2 (state file, config), §3 (`config watch`),
   §4 (the scan), §6 (index field); CONTRACT.md rules 9 and the command
   forms; SPEC-PLUGIN System; CHANGELOG.
8. **Tests:** ignore list, bounds (80, 7 days, entry budget), skipPaths
   (folder and file, also added after the scan), watched paths left
   out, redaction, the click writing `watchPaths` (minimal edit, empty
   array, idempotent, refusals), the index field and its absence; plugin
   model tests; capture-cost delta measured (`capture_cost.rs`).
9. Hand mutants on the new code, `just check` green, HANDOVER.md.
