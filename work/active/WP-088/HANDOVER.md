```
WP-088 HANDOVER
Done: (a) `CollectorState.pending_baseline` (`pendingBaseline`, serde default, skipped while false): a collector that degrades in a capture in which its state was lost is marked, keeps the mark while degraded or not run, and its first successful run records a `state-reset` note for it and clears the mark; (b) rule 8 reads the whole ledger after the append and explains every own change (its existing kinds) without a case and without any resolution line, so changes left open by an earlier capture are caught up; SPEC §2 §3 §5, CHANGELOG, TESTING.md
Not done: no test for the fallback when the ledger cannot be read after the append (explains `written` only, warns); doctor wording for a marked collector (doctor.rs is not mine, see Decisions)
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

No mutant for the read-failure fallback; there is no test for it (Not
done).

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
