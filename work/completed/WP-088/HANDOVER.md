```
WP-088 HANDOVER
Done: (a) `CollectorState.pending_baseline` (`pendingBaseline`, serde default, skipped while false): a collector that degrades in a capture in which its state was lost is marked, keeps the mark while degraded or not run, and its first successful run records a `state-reset` note for it and clears the mark; (b) rule 8 reads the whole ledger after the append and explains every own change (its existing kinds) without a case and without any resolution line, so changes left open by an earlier capture are caught up; SPEC §2 §3 §5, CHANGELOG, TESTING.md
Not done: doctor wording for a marked collector (doctor.rs is not mine; WP-091)
Verified by: flock /tmp/seldon-check.lock just check → exit 0 (`check: ok`, run once); full cargo test 0 failures; fmt + clippy --all-targets -D warnings clean; docs-check ok; 18 mutants, all killed
Learned: memory/pitfalls.md, section "WP-088"
Decisions needed: 4, none blocking (below): interpretation of "degraded in the capture that records a reset"; doctor row wording for the mark; the boolean loses `logbook`; catch-up cost
Touched outside WP scope: engine/tests/{idempotency,own_changes}.rs (tests), docs/SPEC-ENGINE.md, docs/TESTING.md, CHANGELOG.md, memory/pitfalls.md
```

Branch `wp/088-reset-followups`, worktree `wt/WP-088`, from `d52e96d`. No
push, no PR. Commits (oldest first):

- `c04b7d2` engine: a collector degraded in a state reset records its gap later (WP-088)
- `4451736` engine: rule 8 catches up on own changes left open (WP-088)
- this commit: handover and pitfalls

Each commit carries its own code, tests, SPEC, TESTING and CHANGELOG
lines.

## (a) Pending baseline

- **State.** `collectors/mod.rs`: only the new field on `CollectorState`,
  `#[serde(default, skip_serializing_if = "std::ops::Not::not")]`. The
  typed cursor, `cursor_reads`, `Cursors` and the six collectors are
  unchanged.
- **Set.** `capture.rs` `waiting_baselines`: for every collector that ran
  and degraded, the baseline it *would* have taken goes through the
  capture's own pieces:
  - `baselines`: its cursor for this logbook is missing or does not read.
    This is the old body of `pending_reset`, now shared.
  - `losses`: the binding gate.
  - `held_losses`: the ledger rule.
  
  A collector that passes all three is marked. A collector that is already
  marked passes the gate again, so it stays marked while it degrades. A
  collector that is not run keeps its entry as it is.
- **Read.** `losses` treats `cursor.is_some() || pending_baseline` as
  "had state here" (`loss(…, had_state)`). The first successful run
  reports `Lost::Cursor` as before (no cursor). It now counts as a loss,
  so `state_reset` writes the note: sources `<it>`, files `cursors`, and
  the usual warning. That run saves a new state with
  `pending_baseline: false`.
- **doctor.** `pending_reset` keeps its signature. It calls the same
  `baselines` and `losses`, so the prediction also names a marked
  collector. See Decisions 2.
- **Idempotency.** After the run that records the gap, the collector has
  a readable cursor and no mark. The next capture writes 0 (tested).

## (b) Catch-up of own changes

- `reconcile::own_change_resolutions(events, ts)` now takes any event
  list. It skips every event that any `resolution` line in the list
  refers to (explained, dismissed or linked) and every event with a case.
  It explains only `attribution::own_change`, so the kinds are exactly
  rule 8's: plugin update, enable and disable of `jax.seldon`; upgrade and
  reinstall of `jax-seldon`. No case is set.
- `explain_own_changes` reads `ledger.read_all()` after the capture's
  append, so `written` is included and a single pass covers both new and
  old rows. If the read fails, it explains `written` only and warns. It
  now returns `Vec<String>` warnings, and `capture.rs` prints each one.
  `explainedSelf` counts both new and caught-up rows.
- "Has a resolution" does not depend on file order. The index folds a
  resolution only onto an earlier line, but if the catch-up followed that
  rule, an own change dated in a later month would get a new resolution
  on every capture (pitfalls).

## Tests

- `tests/idempotency.rs` `state_reset::`:
  - `a_collector_degraded_in_the_reset_records_its_gap_when_it_runs`: in
    the reset capture, snapper degrades and the note names only pacman.
    Snapper is marked and pacman is not. While degraded, and while not
    run (`--source pacman`), snapper stays marked. Its first successful
    run writes 1 line, the note `snapper`/`cursors`, plus the warning, and
    clears the mark. The next capture writes 0. Before every capture,
    `predicted()` asserts what doctor's row names.
  - `a_collector_that_alone_lost_its_state_while_degraded_records_it_later`:
    `--source snapper` only. The degraded capture writes no note at all,
    and the later successful run writes one.
  - `a_degraded_collector_that_lost_nothing_waits_for_nothing`: degraded
    from the first capture on (ledger rule), and bound here with the
    entry removed by hand (binding gate, WP-081 F1). Neither case is
    marked, and neither writes a note.
  - `a_cursors_file_without_the_mark_reads_unchanged`: an old entry
    without the field reads as `false` and writes back byte-identical;
    `true` is written as `"pendingBaseline":true`.
- `tests/own_changes.rs`
  `own_changes_left_open_are_explained_by_the_next_capture`: after a
  baseline, `seldon event` writes these rows, as a version before rule 8
  would have:
  - `plugin-update` and `plugin-enable` of `jax.seldon`, then
    `drift dismiss` on the enable;
  - `upgrade jax-seldon`;
  - `plugin-add jax.seldon`;
  - `downgrade jax-seldon`;
  - an update of another plugin.

  A plugins capture that collects nothing gives `explainedSelf` 2: the
  update and the upgrade, each with exactly one rule-8 line. The
  dismissed row keeps only its dismissal. Drift is the other plugin, the
  downgrade and the add, with no new case file and a valid index. A
  second capture explains 0, and the ledger length stays the same.
- `reconcile::tests::own_changes_with_a_resolution_keep_it` (unit): an
  open row is explained; dismissed, explained and linked rows are not; the
  result of the catch-up yields nothing.

## Mutants

Method: a script in my scratchpad applies each mutant to the committed
tree, runs the tests below with `--no-fail-fast`, then restores the file
with `git checkout HEAD --` and `touch`. Every mutant failed on test
assertions, not on a build error. The `just check` above ran after the
last mutant.

Part (a), `cargo test --test idempotency --test doctor`:

| # | Mutant | Killed by |
|---|---|---|
| A1 | mark never set | `…degraded_in_the_reset…`, `…alone_lost_its_state…` |
| A2 | gate ignores the mark (`had_state` = cursor only) | same two |
| A3 | mark kept after a successful run | same two |
| A4 | mark without the ledger rule | `…lost_nothing_waits_for_nothing`, `a_first_successful_run_after_a_degraded_init_is_no_reset` |
| A5 | mark without the binding gate | `…lost_nothing_waits_for_nothing` |
| A6 | successful collectors marked too | `…degraded_in_the_reset…`, `…alone_lost_its_state…` |
| A7 | `false` written too (no `skip_serializing_if`) | 4 tests incl. `a_cursors_file_without_the_mark_reads_unchanged` |
| A8 | no `serde(default)`: old files fail to load | 21 tests (doctor and idempotency) |
| A9 | mark never read | `…degraded_in_the_reset…`, `…alone_lost_its_state…` |

Part (b), `cargo test --lib --test own_changes --test own_writes`:

| # | Mutant | Killed by |
|---|---|---|
| B1 | no catch-up (only `written`) | `own_changes_left_open_…` |
| B2 | rows with a resolution not skipped | 5 tests incl. the unit test |
| B3 | a dismissal does not count | `own_changes_left_open_…`, unit |
| B4 | a link does not count | unit |
| B5 | rows with a case caught up too | `seldons_own_changes_without_a_case_are_explained` |
| B6 | catch-up explains `plugin-add` of Seldon | `own_changes_left_open_…`, `enabling_disabling_…` |
| B7 | catch-up explains a `downgrade` of Seldon | `own_changes_left_open_…` |
| B8 | the resolution carries a case | 6 tests |
| B9 | explained twice per capture | 5 tests |
| M9 | read-failure fallback explains nothing (`Vec::new()`) | `an_unreadable_ledger_explains_the_new_changes_and_catches_up_later` (round 2) |
| M10 | "has a resolution" by the index's order rule | `an_own_change_dated_after_the_capture_clock_is_explained_once` (round 2) |

## Verified by

- `flock /tmp/seldon-check.lock just check`: exit 0 on the first and
  only run, ending with `check: ok`. Plugin harness results:

  | Suite | Passed | Failed |
  |---|---|---|
  | bar-view | 143 | 0 |
  | panel-view | 771 | 0 |
  | overlay-view | 319 | 0 |
  | service-states | 297 | 0 |
  | install.test | 209 | 0 |
  | model.test.js | 88 | — |

- `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` are
  clean. `bash scripts/docs-check.sh` reports ok.
- **Cost of (b).** I compared release builds of `c04b7d2` (old) and
  `4451736` (new) on a ledger of 10 012 lines (the fixture ×141 with
  fresh ids), using `capture --source theme` with the index rebuild.
  Runs were interleaved, 15 each, in a scratch HOME. Median: old 22.8 ms,
  new 31.5 ms. Host load was 3–6.

## Decisions needed (none blocking)

1. **Interpretation of (a).** The WP says "degraded in the capture that
   records a state reset". I also mark a degraded collector that was the
   *only* one to lose its state. That capture writes no note at all, and
   without the mark the gap would never be recorded. The rule is the
   capture's gate plus the ledger rule, applied to the baseline the
   degraded run did not take. Reversible: in `run`, mark only when
   `reset.is_some()`. Mutant A1 and the test `…alone_lost_its_state…`
   pin the current behaviour.
2. **doctor row for a marked collector.** The gate is shared, so doctor
   predicts the marked collector. While the collector stays degraded (for
   example snapper without the grant), doctor keeps showing that row,
   with wording that is wrong for this case:
   - The binding is "this logbook", so the message says `cursors
     unreadable in <state dir>`, though the cursor is missing.
   - The fix offers "restore … or run seldon capture to accept the new
     baseline", but a capture clears the row only once the collector runs
     successfully.

   SPEC §3 states this limitation. Proposal: a doctor wording follow-up
   (doctor.rs belongs to WP-089 this wave), e.g. "<c> waits for its
   baseline since it degraded in a state reset; its next successful
   capture records the gap". The alternative is to keep the mark out of
   doctor's gate. That would mean a second gate, which WP-083 avoided.
3. **The boolean loses `logbook`.** If the state was another logbook's,
   the later note still says files `cursors`, and the warning gives the
   restore hint instead of "nothing can be restored". The WP asks for a
   boolean, so I accepted this. Fixing it needs a small enum
   (`pendingBaseline: "cursors" | "logbook"`).
4. **Catch-up cost.** Reading the whole ledger costs about 9 ms per
   capture at 10k lines, on top of the index rebuild's own read. A cheap
   reduction would be a raw-line pre-filter in `ledger.rs`: parse only
   lines that hold `refersTo` or an own id. ledger.rs is not in this WP,
   so I did not do it. Accept, or queue it.

## Notes

- No guard block happened.
- The real host was not touched. All runs used scratch HOME and XDG
  directories, and the perf logbook was a copy of `fixtures/logbook` in
  my scratchpad.
- `pending_reset`'s signature is unchanged. doctor.rs, sys.rs, redact.rs
  and the six collector modules are untouched.

---

# Round 2 (stage 1 SEND BACK, stage 2 APPROVE with this exact round)

Commits (oldest first), no rebase:

- `063a966` engine: the pending baseline records what was lost (WP-088)
- `4d691bf` engine: rule 8 dates its resolution after the event; fallback tested (WP-088)
- this commit: handover round 2 and the pitfalls line

## What changed

- **B1, the mark records what was lost.**
  - `CollectorState.pending_baseline` is now an `Option<PendingBaseline>`.
    `PendingBaseline` is `Cursors | Logbook`, serde `lowercase`, and the
    field is `default` with `skip_serializing_if = Option::is_none`.
    `cursors.json` therefore stores `"pendingBaseline": "cursors" |
    "logbook"`, the same word as the note's `files`.
  - `waiting_baselines` returns the `Lost` that the gate produced. `run`
    stores it through `PendingBaseline::of`: `Cursor` becomes `cursors`,
    `Logbook` becomes `logbook`.
  - `loss` takes `had_cursor` and the mark. Under `Binding::This`, a mark
    yields `p.lost()`, so a `logbook` mark records `Lost::Logbook`. The
    later note then says files `logbook`, and the warning says "Nothing
    can be restored".
  - A collector that is already marked and degrades again keeps its kind.
  - There is no shim for a dev-build `true`, as decided.
- **Tests for B1.**
  - New: `state_reset::a_collector_degraded_when_the_state_was_another_logbooks_says_so_later`.
    It captures A, then B, then A with snapper degraded (mark `logbook`,
    kept on a second degraded capture), then snapper succeeds. The notes
    are `[(pacman, logbook), (snapper, logbook)]`, the warning starts
    with "state reset recorded: snapper" and contains "Nothing can be
    restored", the mark is cleared, and the next capture writes 0.
  - Changed: `a_cursors_file_without_the_mark_reads_unchanged` now
    round-trips `"pendingBaseline":"cursors"` and `"logbook"`
    byte-exactly. The helper `pending()` returns the word, and the round-1
    tests assert `Some("cursors")` / `None`.
- **N1, the fallback is tested.**
  `own_changes::an_unreadable_ledger_explains_the_new_changes_and_catches_up_later`:
  - Setup: a `seldon event` own change (`plugin-enable jax.seldon`) in
    `2026-07.jsonl`, that file set to mode 000, then a plugin update.
  - With the file unreadable, the capture writes 1 event and
    `explainedSelf` is 1. The update gets its rule-8 line, and stderr
    holds `seldon: warning: seldon's earlier own changes not checked: …
    Permission denied`.
  - After restoring the mode, the next capture's `explainedSelf` is 1:
    the July row is caught up.
  - The test skips when mode 000 does not keep the user out (root). It
    checks that by trying to read the file, not the euid, because the
    crate has no libc.
- **I1, the resolution date.**
  - `own_change_resolutions` dates each line `ts.max(e.ts)`. The append
    already picks the month file by the line's own time, so a line for an
    event after the clock lands after the event.
  - Test: `own_changes::an_own_change_dated_after_the_capture_clock_is_explained_once`.
    A `plugin-update jax.seldon` is dated 2026-11-05, and the capture
    clock is 2026-10-01.
    - Results: `explainedSelf` 1, the resolution's `ts` equals the
      event's, the event is not in `drift`, and the index row is
      `explained`. A second capture gives 0 with the same ledger length.
  - The same test kills M10. A second November row is dismissed at the
    October clock, so its dismissal comes *before* it in the ledger. The
    catch-up keeps that dismissal. The index-order rule would add a
    rule-8 line.
- **Docs.**
  - SPEC §2: the cursors row.
  - SPEC §3 doctor paragraph: the `logbook` mark limitation, fixed by
    WP-091.
  - SPEC §3 state-reset paragraph: "files as recorded in the mark,
    `cursors` or `logbook`".
  - SPEC §5 rule 8: the resolution date clause and the fallback sentence.
  - CHANGELOG (both lines), TESTING.md rows, the amendment line under
    Outputs (a) in `WP-088.md`, and the pitfalls line on the clock-back
    fold gap of `drift dismiss` and rule 7 (not fixed here).

## Mutants round 2

Same script and method as round 1. Tests: `--lib`, `--test idempotency`,
`doctor`, `own_changes`, `own_writes`, all with `--no-fail-fast`. All 10
were killed by test assertions, none by a build error.

| # | Mutant | Killed by |
|---|---|---|
| C1 | mark always `cursors` | `…another_logbooks_says_so_later` |
| C2 | gate reads every mark as `Lost::Cursor` | `…another_logbooks_says_so_later` |
| C3 | gate ignores the mark | 3 tests incl. `…degraded_in_the_reset…` |
| C4 | `PendingBaseline::of(Logbook)` gives `Cursors` | `…another_logbooks_says_so_later` |
| C5 | serde words not lowercase | 4 tests incl. `a_cursors_file_without_the_mark_reads_unchanged` |
| C6 | mark never set | 3 tests |
| M9 | read-failure fallback explains nothing | `an_unreadable_ledger_…` |
| M9b | fallback without its warning | `an_unreadable_ledger_…` |
| M10 | "has a resolution" by the index's order rule | `an_own_change_dated_after_the_capture_clock_…` |
| M11 | resolution at the capture time only (no `max`) | `an_own_change_dated_after_the_capture_clock_…` |

## Verified by

- `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings`
  are clean.
- Full `cargo test --no-fail-fast` has 0 failures. Touched suites:
  idempotency 27, doctor 31, own_changes 9, plus lib and own_writes.
- `bash scripts/docs-check.sh` reports ok.
- `flock /tmp/seldon-check.lock just check`: exit 0, run once after the
  last mutant, ending with `check: ok`. Plugin harness results:

  | Suite | Passed | Failed |
  |---|---|---|
  | bar-view | 143 | 0 |
  | panel-view | 771 | 0 |
  | overlay-view | 319 | 0 |
  | service-states | 297 | 0 |
  | install.test | 209 | 0 |
  | real-home-guard | 11 | 0 |
  | model.test.js | 88 | — |

- No guard block. The real host was not touched.

## Not in this round (stage 2)

N2 (a collector not run or disabled at the reset) and the doctor wording
go to WP-091. D4 (cost) is accepted.
