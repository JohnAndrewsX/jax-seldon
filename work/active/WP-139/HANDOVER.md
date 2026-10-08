# WP-139 — handover

Branch `wp/139-recent-config` from `next` (aa9c5902), merges into `next`.
Plan: `PLAN.md`. Not pushed (the orchestrator pushes).

## What was done

- **ADR-0046 (proposed)**, an ADR-0038 amendment: one optional index field
  `system.recentConfig = {scannedAt, files: [{path, mtime}]}` within
  contract 2 (ADR-0035 §6), and the plugin command form `seldon config
  watch --json -- <path>`. `contractVersion` stays 2. ADR-0028 unchanged
  (a newly watched path outside its rows is *attention* by the total row).
  DECISIONS.md lists it as proposed: **it needs the operator's acceptance**.
- **Engine, the scan** (`engine/src/collectors/recent.rs`, no collector,
  no event): during `capture`, when the config collector runs, an mtime
  walk of `~/.config`: newest 80, 7 days, ≤ 20 000 entries, depth 12;
  watch paths, `[redaction] skipPaths` (folders not entered; a name
  pattern matches a folder name too), Omarchy's plugin folder, Seldon's
  config folder and config file, and the WP's ignore list (`.git`, caches,
  state, log, lock, databases, images, browser/Electron profiles by
  `Cookies`/`Local State`, `shell.json`, plus editor temp files) left out;
  directory links never followed; a path with a control/format character,
  over 512 characters, with `.`/`..` folders, or one the logbook's
  redaction would change is left out (never shown masked). Result:
  `$XDG_STATE_HOME/seldon/recent-config.json` (paths and times only). The
  walk is `pub fn scan` for WP-138's `seldon preview`.
- **Engine, the index:** every build reads the state file and drops what
  is by now watched, skipped, redacted or older than 7 days, so the click
  and a new skipPath act at the next index build. Absent before the first
  scan and with an invalid `[redaction] patterns` entry; an unreadable
  file is a build warning.
- **Engine, `seldon config watch <PATH>`** (`commands/config_cmd.rs`):
  under the state lock; refuses (exit 1, nothing written) outside home,
  home itself, Seldon's own files or a folder holding them, skipPaths,
  control/format characters, > 512 characters, and a `config.toml` the
  minimal edit cannot extend (a separate message when the file has no
  `watchPaths` at all, since one path alone would replace the defaults);
  already covered → exit 0, `added: false`; no `config.toml` → defaults
  plus the path. Appends by WP-109's `with_added_watch_paths`, which now
  also takes an empty array. Rebuilds the index. The next capture takes
  the file in without an event (verified by test).
- **Schema, fixtures, derive:** `system.recentConfig` in
  `index.schema.json`; `fixtures/state/recent-config.json` (engine state)
  feeds the sample — it holds one file under a watch path and one older
  than 7 days, which drop out; `validate-fixtures.py`
  `derive_recent_config` derives the field; the golden test copies the
  file into the state dir. Sample and variants regenerated.
- **Plugin:** System's sixth tile *Recently edited* (`Model.systemTiles`,
  `Model.recentFiles`): count, Scanned row, each file with its age, "not
  watched" and *Watch* → `Service.watchPath` → `config watch --json --
  <path>` (`Model.watchArgs`, `validateArgs`, `watchPathError`:
  `~/.config/…`, no bad characters, no `.`/`..`, ≤ 512); the answer above
  the list; a held lock is the engine's message in place, no retry.
- **Docs:** SPEC-ENGINE §2 (state file), §3 (`config watch`), §4 (the
  scan), §6 (the field); CONTRACT.md rule 9 and the command forms;
  SPEC-PLUGIN System; CLI reference en/de (help blocks regenerated, the
  German page's source line moved); CHANGELOG; fixtures/README.

## How it was verified

- `just check` with `SELDON_FULL_CHECK=1`, private `XDG_RUNTIME_DIR`:
  **green** on 7d3868d9 (`check: ok`; cargo 2450 passed, 0 failed incl.
  the `watch` feature; desk-view 1804/0, service-states 344/0,
  model.test.js 187; validate-fixtures, docs-check, qmllint, plugin
  validate ok). The first run (on 0396062e) failed one harness line that
  still counted five System tiles (`search-sections` #6), fixed in
  d0baefbd. `/run/user/1000` at 2 % throughout; private runtime dir.
- Engine tests: `collectors::recent` (10 unit tests: ignore list, bounds
  80 / 7 days / entry budget / depth, skipPaths incl. folder names, watch
  paths, links, redaction and bad characters, the index-time filter, a
  hand-written state file, a future mtime) and `tests/recent_config.rs`
  (12: capture lists and writes no event, only with the config
  collector, skipPaths also added after the scan, a config file elsewhere
  under `~/.config`, the click's minimal edit / idempotence / no event at
  the next capture / later edit is `config-change`, no config file, an
  empty list, every refusal writes nothing, no `watchPaths` key, held
  lock exit 4). Golden index test with the state file.
- Plugin: `model.test.js` (tile, `recentFiles`, `watchArgs`,
  `watchPathError`, `validateArgs`, `watchResult`); desk harness `system`
  (sixth tile, list, texts, dev mode), `system-watch` (two live clicks,
  exact argv, rows go), `system-watch-locked`.
- **Capture-cost delta** (`capture_cost.rs`, bench profile, dev host,
  2026-10-08): scan + state file **≈ 2.3 ms per capture** on a synthetic
  lived-in `~/.config` (370 entries read, 80 listed, a browser profile,
  an Electron app and a cache entered only to their first level); the
  index build adds **≈ 0.5 ms** for 80 files. `just check-perf`
  (on d0baefbd): every timing budget passed (index build ×10 5.8 ms,
  ×150 73 ms; hooks; capture cost incl. the two new benches), but the
  recipe **failed on a precondition, not a timing**:
  `status_at_10_000_ledger_lines_is_under_100_ms` asserts the scaled
  fixture has 10 788 ledger lines and finds 11 656. This branch touches
  neither `fixtures/logbook/` nor `tests/common/scale.rs`; the fixture
  ledger grew with WP-137/WP-141 on `next` (aa9c5902 is the base). Not
  fixed here (out of scope): the number in that test, SPEC-ENGINE §6 and
  the justfile comment need the new scale, and `status` needs re-timing.
- **Hand mutants** (`mutants.py`): 58 of 58 killed (48 engine: the walk, the path rules, the index-time filter, capture, `config watch`, the empty-array edit; 10 plugin: `validateArgs`, `watchPathError`, `recentFiles`, the tile, `watchResult`). The one first survivor (sub-second mtimes not cut) got a test. Service.qml/System.qml are covered by the desk harness only (too slow per mutant).
- No path under a skipPath appears: tested at scan and at index time,
  for path and folder-name patterns, and for a pattern added after the
  scan.

## Not done / open

- ADR-0046 is **proposed**; the operator accepts it (or not).
- Not run on the test host (no live desktop check of the tile); the
  dev-host rule E38 keeps dev builds off the dev host.
- **ADR number:** renumbered to **ADR-0046** on the orchestrator's note
  (next has ADR-0045 EASY | PRO; WP-138's is ADR-0047). DECISIONS.md will
  conflict on merge only by position: keep both rows, 0045 before 0046.
- **One walker (orchestrator note 2):** WP-138 adds
  `engine/src/config_scan.rs` (`scan(config_dir, &SkipPaths, &Limits)`,
  newest first, `partial`). This branch still has its own walk in
  `collectors/recent.rs`. The plan, once WP-138 is on `next` and `next`
  is merged here:
  1. Delete `Walk`, `ignored_dir`, `ignored_file`, `DIR_NAMES`,
     `FILE_ENDINGS`, `PROFILE_MARKS` from `recent.rs`; `recent::scan`
     calls `config_scan::scan`.
  2. Move what only WP-139 needs into `config_scan`, not around it:
     an `exclude: Vec<PathBuf>` in `Limits` (folders and files never
     entered: the watch paths, Seldon's config folder and file, the
     plugin folder as `Sources` finds it). Filtering after the walk
     would let watched files fill `max_files` and would walk the watched
     trees for nothing.
  3. Keep in `recent.rs` only the per-path rules of the index field
     (`shown_path`: `~/.config/`, no `.`/`..`, no control or format
     character, ≤ 512, unchanged by the redaction), applied to the
     walker's output before the cut to 80 (so `max_files` = 80 plus a
     margin, or the cut moves after the filter), the state file and
     `shown`.
  4. Reconcile the two ignore lists into the one in `config_scan`
     (WP-138's adds crashpad, dconf, sentry, webstorage, `*.bak.*`; mine
     adds history, `shell.json` anywhere, `*state.json`, `history.json`,
     `Singleton*`, rotated `*.log.N`, `.#*`, `#*#`, `.goutputstream-*`,
     `*.tmp-*`, `.kdbx`); one link rule — WP-138 lists no link to a file,
     mine lists it by its target (stow-style dotfiles); I would keep
     WP-138's (no link followed at all) and change my test, unless the
     reviewer wants dotfile links. Depth (16 vs 12) and WP-138's deadline
     follow `config_scan`; the capture passes no deadline or a generous
     one.
  5. Re-run `collectors::recent` tests (moved to `config_scan` where they
     test the walk), `recent_config`, golden index, the cost bench, the
     hand mutants on the merged walker; SPEC-ENGINE §4 and ADR-0046 §1
     then name `config_scan` and the merged ignore list (a wording
     change, no contract change).
- A click on a config whose `watchPaths` still equals the current default
  list makes it a user list: a later engine's new defaults are then not
  appended automatically (doctor names them). Stated in ADR-0046.
- The scan's `cut` flag (entry budget reached) is only in the state
  file, not in the index; the desk does not say "the list may be
  incomplete". Not seen at 370 entries; a question for the reviewer
  whether 20 000 needs a visible note.
- The ignore list adds editor temp files, `.pid` and `.kdbx` beyond the
  WP's list (noise and a key store); named in ADR-0046 and SPEC-ENGINE §4.
