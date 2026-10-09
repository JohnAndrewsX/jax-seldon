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

## Round 2a (stage-1 review `WP-139-review-1.md`, brief part A)

- **B1:** a name that is not UTF-8 is left out in the walk (a folder so
  named is not entered) and by `shown_path` (`to_str()` first); unit test
  with two Latin-1 names and a Latin-1 folder. `config watch` refuses a
  path that does not exist (ADR-0046 §1 and §3, SPEC-ENGINE §3, CLI
  reference en/de).
- **B2:** `recent-config.json` is read by `sys::read_small_file(path,
  STATE_FILE_MAX)`; FIFO and `/dev/zero`-link tests (build warning, no
  field); the sentence is in SPEC-ENGINE §2's row.
- **B3 part 1:** optional `system.recentConfig.partial: true` (schema
  `const: true`, only written when true) from the entry budget, the new
  deadline or folders left below the depth; the state file's `cut` became
  `partial`. Variant `fixtures/index-variants/recent-partial.json`
  (overlay, engine golden test `recent_partial_equals_the_variant`); the
  desk reads "The scan stopped early; the list may be incomplete." for an
  empty partial list and appends it to a non-empty one's lead.
- **N1:** deadline 500 ms (`recent::DEADLINE`); 20 000-entry bench
  `capture_cost_of_the_recent_config_scan_at_its_entry_budget`: **118 ms**
  median on the dev host (load 0.5), budget deadline + 100 ms; the
  reviewer's ~400 ms on a loaded host is quoted in ADR-0046 Consequences
  and SPEC-ENGINE §4.
- **N2:** `config watch` checks path shape, home and existence, then
  takes the lock and loads the config under it (own files, skipPaths,
  covered).
- **N3:** the Watch answer shows whenever the Recently edited tile is
  current; harness `system-watch-last` (last row watched: answer and
  "Nothing … was edited" both shown).
- **N4:** the logbook is excluded from the walk (test with a logbook under
  `~/.config`).
- **N5:** `just check-rss`: `rss_stays_under_11_mb_on_the_x10_fixture …
  ok`.

Verified: `just check` (SELDON_FULL_CHECK=1, private runtime dir,
`/run/user/1000` 2 %) **green** — cargo 2456 passed, 0 failed; desk-view
1817/0 (new: `system-watch-last`, `system-partial`); service-states 344/0;
model.test.js 187. Hand mutants: **69 of 69 killed** (11 new for round 2;
two survivors on the first pass — the walk's own UTF-8 skip and `partial`
into the state file — got assertions). Three old patterns were adapted to
the changed code.

Not in part A: the `check-perf` precondition (10 788 vs 11 656 ledger
lines) is unchanged and not this WP's; part B (one walker in WP-138's
`config_scan.rs`, breadth-first or fair budget, file links, union ignore
list with `node_modules`, `Limits.exclude`, root `~/.config`) waits for
WP-138 on `next`. AGENTS.md §6 needs the operator's line for the
`~/.config` metadata walk (review §3, open question 4).

## Round 2b (brief part B, after WP-138 on next)

- **Merges of `next`:** 21ad24b7 (954855dd), then b4a11dba (cbb15779;
  its `preview.rs` fix equals mine, only the formatting conflicted, taken
  from `next`). First merge: conflicts in CHANGELOG,
  DECISIONS (rows 0045, 0046, 0047, 0048 kept in order), fixtures/README
  and validate-fixtures.py, all unions. **`next` at 21ad24b7 does not
  build:** WP-159 replaced `import::is_direction_or_format` with
  `redact::is_invisible`, which WP-138's `commands/preview.rs` still
  called. The merge commit fixes it there and in recent-config (all three
  call sites now `redact::is_invisible`); `next` needs the same one-line
  fix in `preview.rs` if it is gated before this branch merges.
- **One walker:** `engine/src/config_scan.rs` is the only `~/.config`
  walk; `collectors/recent.rs` keeps the per-path rules (`shown_path`),
  the exclusions, the times, the state file and the index filter, and
  calls `config_scan::scan_keeping`. In `config_scan`:
  - **breadth-first** (a queue; each folder's entries sorted by name), so
    a heavy folder cannot spend the 20 000 entries before shallow config
    files elsewhere are read (test `breadth_first_reads_shallow_files_…`:
    depth-first would fail it);
  - **links:** a link to a folder is never entered; a link to a file is
    listed under its own path with the target's mtime (stat only, never
    opened); a dangling link is nothing. WP-138's test became
    `folder_links_are_never_followed_file_links_count_by_their_target`;
  - **ignore list:** the union of both plus `node_modules` (folders
    `history`, `node_modules`; files `state`, `shell.json` anywhere,
    `history.json`, `*state.json`, `Singleton*`, `*.log.*`, `*.tmp-*`,
    `.#*`, `#*#`, `.goutputstream-*`; extensions `kdbx`, `jxl`, `xpm`,
    `swx`); a folder below the root holding `Cookies` or `Local State` is
    skipped whole. WP-138's tests adjusted where the union changed them
    (`omarchy/themed/shell.json` now ignored; the exclude test uses
    `settings.json`; the preview test no longer shows Omarchy's
    `history.json` under a user's own skipPaths);
  - **`Limits.exclude` before a folder is entered:** watch paths, Seldon's
    config folder and file, the plugin folder, the logbook;
  - **`keep`** (`scan_keeping`): the caller's path rules before the cut
    to `max_files`, also for folders (not entered when refused): recent's
    `shown_path` (UTF-8, no control or invisible character, ≤ 512, no
    `.`/`..`/empty folder, unchanged by the redaction); test
    `refused_paths_take_no_place_in_the_eighty`;
  - **partial** from the entry budget, the deadline or the depth (16,
    WP-138's) reaches the state file and the index;
  - `Scan.entries` (entries read), `config_scan::ROOT = ".config"`.
- **Root:** `~/.config` for both callers; `seldon preview` no longer
  follows `$XDG_CONFIG_HOME` (identical on Omarchy, where it is unset).
  **ADR-0047 is accepted and says `$XDG_CONFIG_HOME` and "symbolic links
  never followed"**; ADR-0046 §5 (proposed) records both changes to the
  preview, and SPEC-ENGINE (preview and §4) and `preview.schema.json`'s
  description of `root` follow. If the operator wants ADR-0047 left as
  written, the preview needs its own root again (one line).
- **Cost** (bench profile, dev host, load ≈ 2–4.9 from other agents'
  gates): 2.7 ms per capture on the lived-in tree (2.3 ms before, at
  rest), 128 ms at the 20 000-entry budget, 0.49 ms per index build.
- **Verified** on 37c75f08: `just check` (SELDON_FULL_CHECK=1, private
  runtime dir, JUST_TEMPDIR on disk in the private gates folder,
  `/run/user/1000` 2 %) **green** — cargo 2650 passed, 0 failed (with the
  `watch` feature); desk-view 1845/0; service-states 344/0; model.test.js
  194; validate-fixtures (11 variants), docs-check, qmllint, plugin
  validate ok. `just check-rss`: `rss_stays_under_11_mb_on_the_x10_fixture
  … ok`. Logs: `jax-seldon-private/gates/check-wp139-r2b.log`,
  `rss-wp139-r2b.log`.
- **Hand mutants** now run in a scratch copy (`git archive HEAD` into
  `jax-seldon-private/gates/wp139-mutant-root`, target dir
  `gates/target-wp139`, both on disk; the root is removed after the run):
  **74 of 74 killed** (29 on the one walker: breadth-first, links, the
  exclusions, skipPaths, `keep` for folders and before the cut, the union
  names, budget, deadline, depth, `partial`; the rest as before). The one
  survivor of the first run (preview walking `$XDG_CONFIG_HOME`) got the
  test `the_root_is_dot_config_whatever_xdg_config_home_says`.
- **Not mutated:** the sort of each folder's entries (determinism only;
  no output differs on the test trees).
- **Open:** ADR-0047's text (root, file links) vs. ADR-0046 §5 — see
  above; `check-perf`'s ledger-line precondition (10 788 vs 11 656) is
  unchanged and not this WP's; not run on the test host.

## Round 3 (review 2, orchestrator decisions)

- **Merge of `next`** dcc19901 (main's AGENTS.md §3/§6, E41): clean,
  7507d8fe.
- **B1/B2/N1, the walker:** a link to a file is listed only when its
  target is a regular file **inside `~/.config`** that the walk itself
  would list: not skipped (the target or a folder above it), not excluded
  (watch paths, Seldon's own files, the plugin folder), not ignored by
  name. The target is resolved by `config_scan::resolve_within`, which
  never looks at anything outside `~/.config` (E41): an absolute target
  elsewhere or `..` above the root ends it before anything there is
  touched; a loop ends after 40 hops; an absolute target may spell the
  root as written or canonical (`~/.config` itself a link). So
  `~/.config/app/token.conf → ~/secrets/token`, `ownlink.json →
  ~/.local/state/seldon/index.json` and `procfile.conf →
  /proc/self/status` are all left out; so is a stow link to `~/dotfiles`
  (outside `~/.config`; `config watch` still takes it, below).
- **B1/B2, `config watch`:** the same checks on the canonical path
  (`collectors::config::link_refusal`, `outside_home` on): a path that
  leads through a link out of the home, into Seldon's own files or under
  skipPaths is refused with that reason and the target.
- **The config collector's link following** (`collectors/config.rs`, the
  root loop and the walk): a watch path that is itself a link or lies
  behind one is checked on its canonical target before it is opened:
  into Seldon's own files → counted as before (`link(s) into Seldon's own
  files not followed`); under skipPaths or an excluded folder, or (the
  watch path itself a link) out of the home → not followed, counted
  (`N link(s) not followed: …`). A link to a file inside a watched folder
  is left out when its target is skipped or excluded; a target outside
  the home stays allowed there on purpose (`systemctl --user enable`
  links to `/usr`, the `system-link` evidence of ADR-0028 §5 / WP-109).
  Boot-configuration roots (WP-164) are not touched. Tests:
  `a_link_to_a_skipped_secret_is_never_listed_watched_or_hashed` (also
  hand-written into watchPaths and as a link inside `~/.config/hypr`: no
  event, nothing in the manifest's files/skipped) and
  `a_link_into_seldons_state_keeps_captures_idempotent` (three captures,
  `written: 0` after the first, no event for the link);
  `a_link_out_of_the_home_is_not_listed_and_not_watchable`; unit tests
  `a_file_link_counts_only_with_a_listable_target_inside_the_root`
  (`config_scan`) and
  `a_link_counts_only_with_a_target_the_list_would_show_inside_dot_config`
  (`recent`).
- **B3:** `recent::shown_path` and `config watch` use
  `import::bad_path_char` (control, invisible, U+2028, U+2029: the
  plugin's `BAD_PATH_CHARS`); tests with U+2028 and U+2029.
- **N2:** ADR-0046's header says "amends ADR-0047 §3"; its DECISIONS.md
  row too.
- Docs: ADR-0046 §1, §3, §5 (and the collector paragraph), SPEC-ENGINE
  §3/§4 (preview and recent walk, collector links), CLI reference en/de,
  CHANGELOG.
- **A decision to confirm:** a config whose `~/.config` is itself a link
  to a folder outside the home (e.g. `/mnt/dotfiles`) loses nothing in
  the collector (the home rule applies only when the watch path itself
  is the link), but `config watch` refuses its paths (strict: canonical
  outside the home). Rare on Omarchy; say if the watch rule should match
  the collector's.
- **Mutants (round 3):** 86 of 87 killed in the scratch copy; the
  survivor (`..` above the root ignored instead of ending the
  resolution) was equivalent on the test tree and got an assertion
  (`escape.ini → ../dots/foot.ini` must resolve to nothing).
- **Gate:** the full check of 130f9da4 was **stopped by Claude Code's
  low-memory reaper** while it ran (not a failure of the check; log
  `gates/check-wp139-r3.log`, unfinished). Not restarted on my own; it
  needs the orchestrator's go.

## Round 3b (orchestrator decision on the open question)

- A `~/.config` that is itself a link to a folder outside the home (a
  dotfile setup such as `/mnt/dotfiles`) counts as `~/.config`:
  `collectors::config::link_refusal` maps a target below the canonical
  `~/.config` to `~/.config/…` before the skipPaths check, so `config
  watch` accepts paths there, as the collector does; skipPaths, Seldon's
  own files and links that leave that root are still refused. Test
  `a_dot_config_that_links_out_of_the_home_counts_as_dot_config`
  (accepted path; refused skipPath, own config, link out); checked to fail
  without the change (mutant `r3b …` added to `mutants.py`, run once by
  hand, killed). `recent_config` 17/17 and the lib tests 379/379 pass,
  clippy clean. The full check is the orchestrator's (gate w139r3).

## Round 4 (Fable stage 2: B1, N1–N3)

- **B1:** the config collector checks folder links too. A folder link in
  a watched folder whose canonical target is skipped or excluded is not
  entered (`scan.refused`); below a followed folder link every file and
  folder is checked where it really lies (`Walker::refused_below_link`:
  Seldon's own files as before — a folder that only holds them is still
  walked beside them, WP-113 — then `link_refusal` for skipPaths and the
  excluded folders). Test
  `a_folder_link_in_a_watched_folder_never_reads_a_skipped_file`: Fable's
  scenarios A (`~/.config/systemd/user/foo.d → ~/secrets`) and B (`bar.d →
  ~/units`, `~/units/private/` and `~/units/top.secret` skipped): no event
  for any of them, nothing in the manifest's files/skipped,
  `bar.d/ok.service` watched, and exactly `3 link(s) not followed` per
  capture — one per layer, so each of the three arms is caught alone
  (checked by hand: each one-off mutant fails the test; the three are in
  `mutants.py` as `s2 …`).
- **N1:** `recent::exclusions` holds `dirs.state_dir` (unit test with a
  state directory under `~/.config`).
- **N2:** the ADR-0047 row in DECISIONS.md says "§3 amended by ADR-0046,
  proposed".
- **N3:** CONTRACT.md's `config watch --json` row and ADR-0046 §3 name
  `{added, path, coveredBy, config}`.
- ADR-0046 §5: Fable's two edits (the collector paragraph; the sentence
  that a `~/.config` which is itself a link is read where it leads, also
  by `seldon preview` before `init`). SPEC-ENGINE §4 follows.
- A folder link to a folder that only *holds* Seldon's files (e.g. a
  link to `~/.local`) is still entered and walked beside them, as WP-113
  decided: the new folder arm refuses only skipped or excluded targets.
  The first engine run caught this (`collector_hashes`
  `a_link_to_an_ancestor_of_seldons_dirs_stays_out_of_them`); fixed.
- Verified: `cargo test -j 4` (engine, default features): 1326 passed, 0
  failed, 10 ignored; clippy and fmt clean. No full check (the
  orchestrator gates).
