# WP-081 HANDOVER — A lost or corrupt state directory leaves a trace

Branch `wp/081-review`, worktree `wt/WP-081`. Commits (oldest first):

- `f90557b` engine: capture records a lost or corrupt state directory as a state-reset note
- `79758ed` engine: doctor shows a state reset the last capture recorded; tests for the state reset
- `78c1b2e` engine: test the state reset of plugins and theme
- `32dd831` docs: state reset in SPEC-ENGINE, the user guide and the changelog
- `3c3239a` docs(de): state reset in pages 07, 10 and 11, stamped at 32dd831

## Done

- **Detection** (`engine/src/collectors/mod.rs`): `Outcome.baseline:
  Option<Lost>` with `Lost::{Cursor, Manifest, Owned}` (`cursors`,
  `manifest`, `owned`). Every collector sets it in its baseline branch,
  that is whenever its typed cursor is `None` (no entry, another logbook,
  or a value that does not read as its cursor): pacman, snapper (list and
  info-file paths), omarchy, plugins, theme, config. The config collector
  sets `Manifest` for its `(cursor, no matching generation)` branch
  (manifest missing, corrupt or out of date). Degraded runs never set it.
- **Ledger** (`engine/src/commands/capture.rs`): after attribution, the
  capture keeps the losses whose source the ledger already holds ≥ 1
  event of (months read newest first, stops when all are found) and
  appends one event: `source: seldon`, `kind: note`, `actor: system`,
  subject `state-reset`, detail `state directory missing or unreadable:
  new baseline for <source> (<files>), … at <capture time>; changes made
  while it was missing may not be recorded`, `meta.sources` and
  `meta.files` as comma lists. No schema or fixture change (the existing
  `note` kind, `seldon` source, free scalar `meta` keys).
  A corrupt `owned.json` counts as `config (owned)` when the config
  collector ran ok; that capture moves it to `owned.json.bad` (always,
  not only when a note is written), so the next capture does not see it
  again. First capture of a fresh logbook: no events of the source → no note.
- **Capture output**: human `warning: state reset recorded: <sources>
  took a new baseline because ~/.local/state/seldon was missing or
  unreadable, so changes made meanwhile may be missing. If you have a
  backup of it, restore it and run `seldon capture` again (user guide:
  Back up and restore the state directory)` on stdout; `--json` has a new
  key `warnings` (list of strings, empty without a reset).
- **doctor** (`engine/src/commands/doctor.rs`, new `check_reset`, called
  in the logbook block after `check_collectors`; nothing in the snapper
  check or the constants was touched): a second `state` row, `degraded`,
  while the ledger's newest `state-reset` note is as new as the newest
  `lastRun` in `cursors.json` for this logbook; message names sources and
  files from `meta`, fix = restore a backup and run `seldon capture`
  (without a backup the next capture clears the row). No row otherwise.
  The existing WP-070 `state` error rows for a corrupt `cursors.json`,
  `manifest.json`, `owned.json` keep their wording and add what the next
  capture now does (state reset if the ledger holds those events; owned
  moved to `.bad`). The test's `contains` strings of WP-070 still match.
- **Docs**: SPEC-ENGINE §3 (capture: "State reset (WP-081)" paragraph,
  `capture --json` gains `warnings`; doctor `state`: the "open decision"
  sentence replaced) and §4 (collector intro sentence); TESTING.md rows;
  CHANGELOG; user guide en/de 07, 10, 11 (see "Touched outside scope").

## Not done

- The `hook session-stop` capture (`hook.rs`) discards capture's output,
  so its warning is not printed there; the note and the doctor row still
  show the reset. Printing it on stderr there would be a one-liner if wanted.
- The plugin does not show capture's new `warnings` (Plugin Dev work,
  out of scope for an Engine Dev WP); the index already carries the note.
- doctor does not *predict* a reset (no cursors for this logbook while the
  ledger holds collector events, i.e. the moment a restore still prevents
  the gap). Proposal below.

## Verified by

- `cargo fmt`, `cargo clippy --all-targets -- -D warnings`: clean.
- `cargo test --no-fail-fast` (engine): all green. One existing test
  changed on purpose: `idempotency::capture_writes_the_ledger_once`
  removed `cursors.json` after captures with events and expected 0 written;
  it now expects exactly the note (sources `snapper,pacman,omarchy`,
  files `cursors`) and 0 on the next capture.
- New tests: `tests/idempotency.rs` `state_reset::` (6 tests: lost state
  dir incl. human warning, JSON warnings and detail; no reset without
  events of the source; unreadable pacman cursor; corrupt manifest;
  corrupt owned.json moved aside; plugins and theme with events) and
  `tests/doctor.rs` `a_state_reset_is_shown_until_the_next_capture`.
- `bash scripts/docs-check.sh`: ok (de pages stamped at `32dd831`, the
  commit that holds the current English pages).
- `just check`: exit 0 on the first run (`check: ok`, plugin harness
  bar-view 143 passed, no transient failures, no re-run needed).
- Mutants (script in my scratchpad; each applied, `cargo test --test
  idempotency --test doctor --no-fail-fast`, restored with `git checkout
  HEAD --` plus `touch`; the final `just check` is the baseline after the
  last mutant). All 15 killed:

  | Mutant | Killed by |
  |---|---|
  | M1 detection removed (`lost.push` dropped in `collect_all`) | 5 tests, e.g. `a_lost_state_directory_is_recorded_once`, doctor test |
  | M2 note written on every capture (pacman always `Lost::Cursor`) | `a_lost_state_directory_is_recorded_once`, `capture_writes_the_ledger_once`, `an_unreadable_cursor_is_a_reset`, `a_collector_without_events…` |
  | M3 corrupt owned.json not moved aside (note again next time) | `a_corrupt_owned_file_is_a_reset_and_moved_aside` |
  | M4 doctor silent on a corrupt manifest (parser always ok) | `a_corrupt_state_file_is_an_error_with_its_fix`, `a_state_reset_is_shown_until_the_next_capture` |
  | M5 ledger filter removed (first capture is a reset) | 8 tests |
  | M6 manifest loss not flagged by config | `a_corrupt_manifest_is_a_reset`, doctor test |
  | M7 doctor never shows the reset row | doctor test |
  | M8 doctor never clears the reset row | doctor test |
  | M9 capture prints no warning | `a_lost_state_directory_is_recorded_once`, `a_corrupt_manifest_is_a_reset` |
  | M10 corrupt owned.json not detected | `a_corrupt_owned_file_is_a_reset_and_moved_aside` |
  | M11 pacman never flags an unreadable/missing cursor | 3 tests incl. `an_unreadable_cursor_is_a_reset` |
  | M12 plugins flag removed | `plugins_and_theme_with_events_have_a_reset` |
  | M13 theme flag removed | `plugins_and_theme_with_events_have_a_reset` |
  | M14 omarchy flag removed | `capture_writes_the_ledger_once` |
  | M15 snapper flag removed | `capture_writes_the_ledger_once`, `a_lost_state_directory_is_recorded_once` |

  The WP's three named mutants are M1 (detection removed), M2/M3 (note
  written twice) and M4 (doctor silent on a corrupt manifest).

## Learned (also in memory/pitfalls.md)

- `Cli::capture` in `tests/idempotency.rs` always adds `--all`, which
  `--source` refuses (exit 1); run `--source` captures through `cli.run`.
- doctor's reset row compares the note's `ts` with `lastRun` at whole
  seconds: two captures with the same `SELDON_NOW` keep the row; tests
  give each capture its own time.

## Decisions needed

- None blocking. Proposals for the orchestrator:
  1. doctor could warn *before* the capture (`cursors.json` missing or
     bound to another logbook while the ledger holds collector events):
     that is the moment a restore prevents the gap altogether. Not in the
     WP outputs, so not done.
  2. Hook session-stop: print capture warnings on stderr (`seldon:
     warning: …`).
  3. Plugin: surface `capture --json` `warnings` (Plugin Dev WP).

## Touched outside WP scope

- `docs/user/{en,de}/07-the-logbook.md`: the restore steps (WP-056) live
  there, not in 11, and its sentence "nothing warns you" became false; it
  now describes the note and links to the new troubleshooting section.
  Page 11 got a backup hint before step 6 of the uninstall.
- `docs/TESTING.md` rows (doctor, new idempotency row).
- The coordination with WP-079 held: in `doctor.rs` only the import
  lines, one call line in the logbook block, the new `check_reset` and the
  three effect strings of `check_state` changed; `check_snapper`,
  `SNAPPER_FIX*` and the `check_snapper` call are untouched.
