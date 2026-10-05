# WP-083 HANDOVER — doctor warns before the capture that would record a state reset

Branch `wp/083-review`, worktree `wt/WP-083`. Commits (oldest first):

- `ba4c982` engine: doctor warns before the capture that would record a state reset
- `b685610` docs: doctor's state-reset prediction in SPEC-ENGINE, the user guide and the changelog
- `35b4084` docs(de): doctor's state-reset prediction in pages 10 and 11, stamped at b685610
- (this commit) orchestration: WP-083 handover and pitfalls

## Done

- **Shared detection** (`engine/src/commands/capture.rs`). Capture's
  pieces became shared functions, and capture itself now calls them:
  `Binding::of` (what `cursors.json` was bound to), `losses` (the WP-081
  F1 gate `loss()` over a list of baselines) and `held_losses` (the
  ledger rule: only sources the ledger holds ≥ 1 event of; `state_reset`
  calls it). New `pending_reset(ledger, config, cursors, logbook)`:
  every collector a plain `seldon capture` runs (`select` with default
  args, i.e. enabled ones; `hook session-stop` uses `--all`, the same
  set) whose cursor for this logbook is missing or does not read takes a
  baseline (`Lost::Cursor`), then goes through `losses` and
  `held_losses`. It returns the binding too (it names the cause). No
  second implementation of the gate or the ledger rule.
- **Whether a cursor reads** (`engine/src/collectors/mod.rs`): new
  required trait method `Collector::cursor_reads(&Value) -> bool`, in
  every collector `typed_cursor::<ItsCursor>(Some(cursor)).is_some()`,
  the same predicate each collector's baseline branch uses. That
  touches all six collector files (4 lines each, right after `fn name`),
  `snapper.rs` included; see "Touched outside scope".
- **doctor** (`engine/src/commands/doctor.rs`, new
  `check_pending_reset`, called in the logbook block right after
  `check_reset`): a `degraded` `state` row
  `the next capture will record a state reset for <sources>: cursors missing in ~/.local/state/seldon, so changes made since the last capture may not be recorded`
  (`cursors unreadable in …` when bound here and a cursor does not read;
  `cursors in … bound to another logbook`). Fix: `restore
  ~/.local/state/seldon from a backup now (user guide: Back up and
  restore the state directory), or run seldon capture to accept the new
  baseline`; for another logbook: `nothing to restore: the state belongs
  to another logbook path; run seldon capture to accept the new baseline
  (user guide: Moving or copying the logbook)`. No row when nothing is
  lost or `cursors.json` cannot be read (its WP-070 error row stands).
  After the capture the row is gone and the WP-081 row takes over.
  Exit code unchanged (degraded is not an error).
- **Docs**: SPEC-ENGINE §3 doctor `state` paragraph; user guide en 10
  (table row, new section "doctor says the next capture will record a
  state reset" before "A state reset was recorded") and 11 (step 6:
  doctor tells you before the first capture); de 10 and 11 translated
  and stamped at `b685610`; CHANGELOG `[Unreleased]` Engine; TESTING.md
  rows for doctor and idempotency.

## Not done / limits (documented in SPEC §3 and page 10)

- doctor cannot know whether a collector will degrade in that capture
  (snapper without the read grant): such a collector takes no baseline,
  so the row can name more collectors than the note.
- `manifest` and `owned` losses are not predicted: their corrupt-file
  error rows (WP-070/081) already say the next capture records a reset.
  A *missing* `manifest.json` next to a readable config cursor is not
  predicted (would need the config collector's generation lookup in
  doctor); it is rare (a partial restore) and still recorded by capture.
- A corrupt or unreadable `cursors.json` gives no prediction row: the
  next capture fails rather than resets, and the existing error row
  covers it.

## How verified

- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`: clean.
- `cargo test` (engine, all suites): green; doctor 31, idempotency 22.
- New / changed tests:
  - `tests/doctor.rs` `a_state_reset_is_predicted_before_the_capture`:
    fresh logbook (with and without `cursors.json`) → no row; captures
    with config events → no row; config entry removed while bound here
    (never ran here) → no row; unreadable config cursor → row
    "cursors unreadable", exit 0; `cursors.json` missing → row "cursors
    missing" with the exact restore fix and the human line; config
    disabled in `config.toml` (edited as a TOML table, config row
    asserted ok) → no row; restored `cursors.json` → no row; capture →
    the WP-081 "the last capture recorded" row instead; next capture →
    quiet.
  - `a_state_reset_is_shown_until_the_next_capture` (WP-081 R3 tail):
    now expects the prediction row for another logbook's cursors with
    the exact no-restore message and fix, and still no "last capture
    recorded" row. Intended behaviour change.
  - `tests/idempotency.rs` `state_reset::`: helper `predicted()` reads
    doctor's row; before every capture of the WP-081 scenarios it must
    name exactly the sources of the note that capture writes (lost state
    dir `snapper,pacman`; unreadable pacman `pacman`; plugins and theme
    `plugins,theme`; another logbook `snapper,pacman` with the
    no-restore text) and nothing before captures that write none (fresh
    logbook, no events of the source, degraded init, enabled later,
    after a reset). New `a_restore_before_the_capture_prevents_the_reset`
    and `every_collector_reads_its_own_cursor` (all six accept their own
    saved cursor and reject `"not a cursor"` and `7`).
- Pre-fix run (base `616be66` `engine/src` under the new tests):
  `a_state_reset_is_predicted_before_the_capture` and the changed
  `a_state_reset_is_shown_until_the_next_capture` fail there. (The
  idempotency suite does not compile on the base: `cursor_reads` is new.)
- `bash scripts/docs-check.sh`: ok (414 links, 14 translated pages).
- `just check`: exit 0 on the first run (`check: ok`; plugin harness
  bar-view 143, panel-view 743, overlay-view 319, service-states 275
  passed, 0 failed; no transient, no re-run). Started after a bounded
  wait (47 × 10 s) until WP-085's `just check` and every plugin harness
  process had ended. The final run is also the baseline after the last
  mutant.
- Mutants (script in my scratchpad; each applied, `cargo test --test
  doctor --test idempotency --no-fail-fast`, restored with
  `git checkout HEAD --` plus `touch`). All 17 killed, none a build error:

  | Mutant (claim) | Killed by |
  |---|---|
  | P1 doctor never shows the row (missing cursors with events → row) | `a_state_reset_is_predicted_before_the_capture`, `a_state_reset_is_shown_until_the_next_capture`, 5 `state_reset::` tests |
  | P2 ledger rule dropped in the prediction (fresh logbook → no row) | `a_state_reset_is_predicted_before_the_capture`, `a_collector_without_events_in_the_ledger_has_no_reset`, 4 more |
  | P3 another logbook offers a restore (no-restore fix) | `a_state_reset_is_shown_until_the_next_capture`, `state_of_another_logbook_is_a_reset_without_a_restore` |
  | P4 a readable cursor counts as lost (row gone after the capture) | 10 tests incl. `a_restore_before_the_capture_prevents_the_reset` |
  | P5 binding ignored in the prediction (never ran here → no row) | `a_state_reset_is_predicted_before_the_capture`, `a_first_successful_run_after_a_degraded_init_is_no_reset`, `a_collector_enabled_later_is_no_reset` |
  | P6 disabled collectors predicted | `a_state_reset_is_predicted_before_the_capture` |
  | P7 an unreadable cursor counts as read (unreadable → row) | `a_state_reset_is_predicted_before_the_capture`, `an_unreadable_cursor_is_a_reset`, `plugins_and_theme_with_events_have_a_reset` |
  | P8 unreadable worded as missing | `a_state_reset_is_predicted_before_the_capture` |
  | P9 pacman `cursor_reads` always true | `every_collector_reads_its_own_cursor`, `an_unreadable_cursor_is_a_reset` |
  | P10 omarchy `cursor_reads` always false | `every_collector_reads_its_own_cursor` |
  | P11 config `cursor_reads` always false | `every_collector_reads_its_own_cursor`, 2 doctor tests |
  | P12 theme `cursor_reads` always true | `every_collector_reads_its_own_cursor`, `plugins_and_theme_with_events_have_a_reset` |
  | P13 plugins `cursor_reads` always true | `every_collector_reads_its_own_cursor`, `plugins_and_theme_with_events_have_a_reset` |
  | P14 snapper `cursor_reads` always false | `every_collector_reads_its_own_cursor` and 6 more |
  | P15 shared ledger rule broken (`held_losses` keeps all) | 16 tests (capture and doctor both: shared code) |
  | P16 shared gate broken (`loss`: no cursor here is a loss) | `a_state_reset_is_predicted_before_the_capture` plus the two WP-081 F1 capture tests (shared code) |
  | P17 prediction row not degraded | `a_state_reset_is_predicted_before_the_capture`, `fixture_logbook_is_valid_and_untouched`, `state_of_another_logbook_is_a_reset_without_a_restore` |

  P15 and P16 show the sharing: one mutant in the shared function fails
  both the capture tests and the doctor prediction tests.

## Open questions

- None blocking. Side effect worth knowing: `doctor` on the fixture
  logbook in a scratch home (no state) now shows the prediction row
  (degraded; P17 shows `fixture_logbook_is_valid_and_untouched` runs into
  it). That is accurate: a first capture there would record a reset for
  the fixture's collector sources (the WP-081 "Known limitation" of a
  ledger with events before the first capture).
- Plugin: the prediction is a doctor row only; the index carries
  nothing new (no contract change). If the panel should warn before the
  capture too, that is a Plugin Dev / contract WP.

## Touched outside WP scope

- `engine/src/collectors/{pacman,snapper,omarchy,plugins,theme,config}.rs`:
  one 4-line `cursor_reads` impl each, placed right after `fn name` in
  `impl Collector for …`. **`snapper.rs` is WP-082's file this wave**;
  the hunk sits apart from the collect/diff code, but if WP-082 renames
  `SnapperCursor` or changes how its cursor is read (migration), the
  merge must keep `cursor_reads` equal to snapper's baseline predicate.
  `every_collector_reads_its_own_cursor` will catch a mismatch for the
  saved-cursor direction.
- `engine/tests/idempotency.rs` (assertions added to WP-081 tests, two
  new tests), `docs/TESTING.md`, `memory/pitfalls.md`.

---

# Round 2 (review APPROVE with F1–F3 and a SPEC nuance)

Commits (oldest first): `45578ee` engine F1 test + F2, `a8da0e4` SPEC
nuance and the F2 fix wording, then this commit (handover round 2 and
the F3 pitfalls line). No rebase.

## What changed

- **F1.** `a_state_reset_is_predicted_before_the_capture` now points
  `config.toml`'s `logbook` at a symlink to the logbook (the cursors are
  bound to the canonical path), checks that doctor resolves the link
  (`"logbook"` in the JSON is the link) and expects no prediction row.
- **F2.** `check_pending_reset` gets the logbook's `LogbookSource`. For
  `Flag` (`--path` or `--logbook`) both fixes name `seldon --logbook
  <path> capture` (path shown as the doctor header shows it, like the
  existing `seldon init --path <path>` fix); otherwise `seldon capture`
  (the environment's `SELDON_LOGBOOK` is the one a plain capture uses).
  Tests: the missing-cursors case run with `--path` (restore fix), and
  the WP-081 R3 tail, which already runs doctor with `--path` while
  `config.toml` points at the other logbook (no-restore fix).
- **F3.** `memory/pitfalls.md`: `collect`'s `typed_cursor::<T>` and
  `cursor_reads` must use the same `T`; `every_collector_reads_its_own_cursor`
  does not catch a `collect` that accepts an older shape `cursor_reads`
  rejects.
- **SPEC nuance.** SPEC-ENGINE §3: no row also when the ledger cannot be
  read (the `ledger` row covers it); the fix names `seldon --logbook
  <path> capture` for a `--path`/`--logbook` logbook. The doc comment of
  `check_pending_reset` says the same. User guide pages unchanged (the
  row's fix text itself names the command).

## How verified

- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`: clean.
- Suites doctor (31), idempotency (22), collectors (19), hooks (54, 2
  ignored perf): all ok. `bash scripts/docs-check.sh`: ok. No `just
  check` this round (per brief).
- Mutants (same script and method):

  | Mutant | Killed by |
  |---|---|
  | Q1 `canonicalize` in `check_pending_reset` replaced by `logbook.root.clone()` (the review's surviving mutant) | `a_state_reset_is_predicted_before_the_capture` |
  | Q2 the fix names the `--path` logbook for `Env` instead of `Flag` | `a_state_reset_is_predicted_before_the_capture`, `a_state_reset_is_shown_until_the_next_capture` |
  | Q3 the no-restore fix always says `seldon capture` | `a_state_reset_is_shown_until_the_next_capture` |

## Open

- Not in this round (the orchestrator's follow-up): a collector that
  degrades in the capture that records a reset keeps no cursor, so its
  later first success is gated as "never ran here" and its gap is never
  recorded.
