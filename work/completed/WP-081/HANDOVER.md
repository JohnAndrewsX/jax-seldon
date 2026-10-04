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

---

# Round 2 (review SEND BACK: F1–F5, proposal 2)

Commits (oldest first): `f51b98b` hook stderr warning, `230f7b4` engine
F1–F4 + tests, `e8bf173` docs (en, SPEC, CHANGELOG, TESTING), `298e160`
docs(de) stamped at `e8bf173`. No rebase (merge order 079 → 080 → 081).

## What changed

- **F1 (blocking).** `capture` reads what `cursors.json` was bound to
  *before* `bind` (`Binding::{None, This, Other}`) and filters the
  collectors' `Lost::Cursor` through `loss()`: bound to this logbook, only
  a cursor that is there and does not read is a loss; a collector without
  one (every run degraded so far, disabled until now, never selected)
  takes its first baseline without a note. This covers both the review's
  repro and the "disabled since init, enabled later" case (that one has no
  entry at all, not an entry with `cursor: None`, so the gate is "no cursor
  for this logbook", which includes the review's case). Side effect: a
  hand edit that removes one collector's entry from an otherwise intact
  `cursors.json` is no longer a loss (two round-1 tests used that to
  simulate a loss; they now write an unreadable cursor value instead).
  Q1 limitation (`init --no-capture`, then a hook/agent event, then the
  first capture) is one sentence in SPEC-ENGINE §3.
- **F2.** New files kind `logbook` (`Lost::Logbook`, set by `capture` only,
  ordered first: logbook, cursors, manifest, owned). Detail and warning say
  "missing, unreadable or bound to another logbook"; with `logbook` the
  warning's tail is "Nothing can be restored: the state belonged to another
  logbook path, and the new baseline is this logbook's (user guide: Moving
  or copying the logbook)", and doctor's row says "(the state in
  ~/.local/state/seldon belonged to another logbook)" with the fix
  "nothing to restore: … run seldon capture to clear this row". One
  sentence in 07 "Moving or copying the logbook" (en, de).
- **F3.** Tests for R1 (corrupt `owned.json` + `capture --source pacman`:
  no rename, no note, no warning), R2 (snapper info-file path: `baseline`
  is `Some(Cursor)` on a fresh run and `None` with a cursor, in
  `collectors.rs`), R3 (doctor: another logbook's cursors with an earlier
  `lastRun` do not make this logbook's reset "the last capture").
- **F4.** Detail: `… new baseline for <source> (<files>), … at <baseline>,
  recorded <capture time>; changes made in between may not be recorded`
  (`<baseline>` = `Ctx::baseline`: logbook `created` or `--since`).
- **F5.** 10-troubleshooting (en, de): what `owned.json.bad` is, that it
  can be deleted, that the next one replaces it.
- **Proposal 2.** `hook session-stop` prints capture's `warnings` on
  stderr as `seldon: warning: …` (stdout stays empty); test
  `hooks::sessions::session_stop_prints_a_state_reset_on_stderr`. SPEC §8.

## How verified

- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`: clean.
- Suites idempotency (20), doctor (29), collectors (19), hooks (54, 2
  ignored perf), index (25, 1 ignored): all ok.
- `bash scripts/docs-check.sh`: ok. No `just check` this round (per brief).
- Pre-fix run (round-1 `engine/src` checked out under the new tests):
  `a_first_successful_run_after_a_degraded_init_is_no_reset`,
  `a_collector_enabled_later_is_no_reset`,
  `state_of_another_logbook_is_a_reset_without_a_restore`,
  `session_stop_prints_a_state_reset_on_stderr` and the F4 assertion in
  `a_lost_state_directory_is_recorded_once` fail there; the round-1 code
  wrote the false note of the review's repro.
- Mutants round 2 (same method; suites idempotency, doctor, collectors,
  hooks), all killed:

  | Mutant | Killed by |
  |---|---|
  | N1 F1 gate removed (no cursor here counts) | `a_first_successful_run_after_a_degraded_init_is_no_reset`, `a_collector_enabled_later_is_no_reset` |
  | N2 F1 gate too strong (unreadable cursor ignored) | `an_unreadable_cursor_is_a_reset`, `plugins_and_theme_with_events_have_a_reset` |
  | N3 another logbook reported as `cursors` | `state_of_another_logbook_is_a_reset_without_a_restore` |
  | N4 no `cursors.json` is no loss | `a_lost_state_directory_is_recorded_once`, `capture_writes_the_ledger_once`, `session_stop_prints_a_state_reset_on_stderr` |
  | N5 warning always offers a backup | `state_of_another_logbook_is_a_reset_without_a_restore` |
  | N6 doctor fix always offers a backup | `state_of_another_logbook_is_a_reset_without_a_restore` |
  | N7 detail names the capture time as baseline | `a_lost_state_directory_is_recorded_once` |
  | N8 hook drops the warning | `session_stop_prints_a_state_reset_on_stderr` |
  | R1 "config ran ok" gate of owned.json removed | `a_corrupt_owned_file_waits_for_the_config_collector` |
  | R2 snapper info-file flag removed | `collectors::snapper_reads_the_info_files_when_listing_is_not_permitted` |
  | R3 check_reset logbook guard removed | `doctor::a_state_reset_is_shown_until_the_next_capture` |

  The round-1 mutants M1–M15 were re-run against the round-2 code: all
  still killed. The suites ran once more after the last mutant (baseline).

## Open

- None from this round. Proposal 1 (doctor warns before the capture) is
  the orchestrator's follow-up WP; F6 left as decided.

## Touched outside WP scope (round 2)

- `engine/src/commands/hook.rs` and `engine/tests/hooks.rs` (proposal 2,
  accepted). `engine/tests/collectors.rs` (R2). Docs as listed above.
