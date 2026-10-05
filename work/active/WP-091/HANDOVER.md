```
WP-091 HANDOVER
Done: (a) snapper changing between degraded and ok since its last run for this logbook writes one `seldon` note, subject `snapper`, actor `system`, no case, with the collector's message, in both directions; (b) N2: a collector not run (`--source`, or disabled) in the capture that drops its state is marked through the same baselines → losses → held_losses chain, on an entry with only the mark; (c) doctor gives a marked collector its own `state` row (degraded or not run since a state reset, `cursors` or `logbook` wording, a `--source` capture as the fix); SPEC §2 §3 §4, CHANGELOG, TESTING.md
Not done: user guide 10-troubleshooting (en/de) does not describe the new doctor row (not in scope; see Decisions 3)
Verified by: flock /tmp/seldon-check.lock just check → exit 0 (`check: ok`, run once, after the last mutant); full cargo test 0 failures; fmt + clippy --all-targets -D warnings clean; docs-check ok; 24 mutants, all killed
Learned: memory/pitfalls.md, section "WP-091"
Decisions needed: 4, none blocking (below): any degraded state counts for the note; downgrade reads of a bare entry; user guide row; a disabled collector's mark is not in doctor
Touched outside WP scope: engine/src/index/mod.rs and the one doctor.rs line for the optional `lastRun` (2 lines), engine/tests/idempotency.rs, docs/TESTING.md, CHANGELOG.md, memory/pitfalls.md
```

Branch `wp/091-reset-notes`, worktree `wt/WP-091`, from `886d876`. No
push, no PR. Commits (oldest first):

- `06d39f3` engine: a collector not run in a state reset records its gap later (WP-091)
- `9abf2b9` engine: doctor names a waiting baseline by its reason (WP-091)
- `fbc6caf` engine: snapper access changes are recorded as notes (WP-091)
- `8f44f21` engine: test a mark of another logbook and the snapper-only note (WP-091)
- this commit: handover and pitfalls

Each code commit carries its own tests, SPEC, TESTING and CHANGELOG lines.

## Design decision for N2 (taken before coding)

**What `index.state.collectors` derives from a bare marked entry:
exactly what it derives from no entry** — `ok: true`, no message,
`lastRun: null`. The collector never ran under this state, so the index
must not claim a run or a failure. No schema change.

To get there:

- `CollectorState.last_run` became `Option<String>`, `serde(default,
  skip_serializing_if = Option::is_none)`. Every existing file has
  `lastRun`, so it reads as `Some` and writes back byte-identical (the
  WP-088 round-trip test still passes unchanged).
- `CollectorState::waiting(mark)` is the bare entry:
  `{"ok":true,"events":0,"pendingBaseline":"cursors"|"logbook"}` — no
  `cursor`, no `lastRun`, no `message`, no `fix`.
- Readers of `lastRun` skip `None`: the index (`and_then`), doctor's
  "last capture" comparison (`filter_map`). doctor's collectors row skips
  it already (`ok: true`).

## (b) N2: a collector not run in the capture that loses the state

- `capture::waiting_baselines` now takes the capture's not-run collectors
  besides the degraded ones. A not-run collector enters the chain only
  when it has **no entry** in `cursors` as bound for this capture:
  - binding none (no file / no logbook) or another logbook (`bind` drops
    every entry): it would lose its state, so it goes through
    `baselines` → `losses` (`cursors` resp. `logbook`) → `held_losses`
    (at least one event of its source).
  - bound here with an entry: kept exactly as it is (WP-088 behaviour). An
    unreadable cursor there is recorded by its own next run, and doctor
    keeps calling it "cursors unreadable", which is true.
  - bound here without an entry: the gate says "never ran here", no mark.
- `run` writes a `CollectorState::waiting` entry for each marked not-run
  collector at the same moment it saves the others (after the append).
- The read side is WP-088's, unchanged: `loss` sees the mark, the first
  successful run records `state-reset` with the mark's `files` word, and
  clears it. That also holds for a collector enabled again later.
- doctor's `pending_reset` only looks at enabled collectors, so a
  disabled collector's mark is silent until it is enabled (Decisions 4).

## (c) doctor wording for a marked collector

`check_pending_reset` now returns `Vec<Check>`. Under binding "this
logbook" it splits `pending_reset`'s losses by the mark:

- unmarked (an unreadable cursor): the WP-083 row, unchanged.
- marked: a new row, `waiting_row`:
  - message: `snapper degraded or not run since a state reset (cursors
    missing or unreadable in ~/.local/state/seldon); its next successful
    capture records the gap, so changes made in between may not be
    recorded`;
  - with a `logbook` mark: `(the state in ~/.local/state/seldon belonged
    to another logbook)`; with mixed marks each collector gets its own
    reason; plural: "each one's next successful capture";
  - fix: `run seldon capture --source snapper once it can run (a
    degraded collector: the collectors row's fix first)`; with
    `--logbook` the capture command names that logbook. No restore: the
    other collectors took their new baseline already, and restoring their
    old cursors could replay config diffs.
- A mark counts only in cursors bound here: another logbook's marks are
  that logbook's, so doctor on a second logbook still shows the "bound to
  another logbook" row (test `a_mark_of_another_logbook_is_no_waiting_row_here`).

## (a) The snapper note

- `capture::access_change(cursors, states, now)`: snapper ran in this
  capture, its entry in `cursors` (as bound for this capture) has a
  `lastRun`, and `ok` differs → one event `Event::new(now, Seldon, Note,
  "snapper")` (actor `system` by default, no case, no meta). It is
  appended with the capture's events, after the state-reset note.
- Detail:
  - `snapper collector degraded: <message>; at the last capture it was ok`
  - `snapper collector ok again (<message>); at the last capture it was
    degraded: <earlier message>` — the `(<message>)` part only when the
    run has one, which the info-file path (ADR-0026 grant) does:
    `snapper list is not permitted; N snapshots read from …`.
- No note: the first run for a logbook; after a lost state directory or
  another logbook's state (`bind` emptied the entries, nothing to compare
  with); for an entry with only the mark (`lastRun` none); in a capture
  that does not run snapper. The saved state then carries the new `ok`,
  so the next capture writes nothing.
- Not an own change (rule 8 explains only plugin/package kinds) and no
  drift (zone none for `seldon` notes); both are asserted.
- `snapper.rs` and `collectors/mod.rs`'s collector interface are
  untouched; the note lives in `capture.rs`.

## Tests

`engine/tests/idempotency.rs`:

- `state_reset::` (N2, new):
  - `a_collector_not_run_in_the_reset_records_its_gap_when_it_runs`:
    `--source pacman` before snapper has events → no entry (ledger rule);
    after a removed state directory, `--source pacman` writes only
    pacman's note, snapper's entry is exactly the bare JSON, omarchy,
    plugins, theme and config get none; snapper's index row equals
    theme's (no entry) apart from the name; doctor: no "next capture" row,
    the waiting row names snapper; not run again → unchanged; with an
    unreadable pacman cursor besides, both rows show (pacman, snapper)
    and the next capture writes one note `snapper,pacman`/`cursors`;
    mark cleared, `lastRun` set, then 0.
  - `a_disabled_collector_when_the_state_was_another_logbooks_says_so_later`:
    snapper disabled while the state was another logbook's → mark
    `logbook`, index `enabled: false`; enabled again → note
    `snapper`/`logbook`, "Nothing can be restored", then 0.
  - `a_collector_not_run_keeps_its_entry`: bound here, an unreadable
    snapper cursor, `--source pacman` → entry byte-equal, "next capture"
    row names snapper, no waiting row; the next capture records it.
  - `a_mark_of_another_logbook_is_no_waiting_row_here` (doctor).
- `state_reset::` (WP-088 tests, adjusted): the waiting row's exact
  message and fix (`cursors`), the `logbook` wording, `predicted()` no
  longer names marked collectors; the success capture after a degraded
  snapper now writes 2 (reset note + access note), asserted.
  `a_first_successful_run_after_a_degraded_init_is_no_reset` now asserts
  no `seldon` note at all (kills "a note for every collector").
- `snapper_access::` (new module):
  - `a_grant_and_its_removal_are_recorded_once_each`: degraded first run
    → no note; repeat → 0; info files appear (the grant) → one note with
    both messages, actor `system`, no case, `explainedSelf` 0; repeat → 0;
    info files gone (a `SYNC_ACL` rewrite) → one `degraded` note; repeat
    → 0; no state reset; ledger schema-valid; neither note is in `drift`.
  - `a_failure_and_recovery_of_the_list_are_recorded`: ok by `snapper
    list`, `--source pacman` while snapper fails (nothing), then the
    failure and the recovery, each once.
  - `no_change_without_an_earlier_run_here`: lost state directory; bare
    entry then a degraded run.
- `engine/src/commands/doctor.rs` `tests::a_waiting_row_names_each_reason`
  (unit): plural, mixed reasons, the `--logbook` capture in the fix.

## Mutants

Method as WP-088: a script in my scratchpad applies each mutant to the
committed tree, runs `cargo test --no-fail-fast --test idempotency
--test doctor --lib`, then restores the file with `git checkout HEAD --`
and `touch`. All 24 failed on test assertions, none on a build error (S7
and S8 first did not compile and were rewritten).

| # | Mutant | Killed by |
|---|---|---|
| N1 | not-run collectors never marked | `…not_run_in_the_reset…`, `…disabled_collector…` |
| N2 | not-run with an entry marked too | `…not_run_keeps_its_entry`, `…degraded_in_the_reset…` |
| N3 | marks skip the ledger rule | 4 incl. `…not_run_in_the_reset…`, `…enabled_later_is_no_reset` |
| N4 | bare entry gets a `lastRun` | `…not_run_in_the_reset…`, `no_change_without_an_earlier_run_here` |
| N5 | bare entry `ok: false` | `…not_run_in_the_reset…` |
| N6 | bare mark always `cursors` | `…disabled_collector…` |
| N7 | `lastRun: null` written | `…not_run_in_the_reset…` |
| N8 | `lastRun` required on read | 4 incl. `…not_run_in_the_reset…` |
| N9 | bare entries not saved | `…not_run_in_the_reset…`, `…disabled_collector…` |
| N10 | index invents a `lastRun` for a bare entry | `…not_run_in_the_reset…` |
| D1 | no split (marks in the "next capture" row) | 4 incl. `…mark_of_another_logbook…` |
| D2 | marks counted under any binding | `…mark_of_another_logbook_is_no_waiting_row_here` |
| D3 | `logbook` mark worded as `cursors` | unit, `…degraded_when_the_state_was_another_logbooks…` |
| D4 | fix without `--source` | unit, `…degraded_in_the_reset…` |
| D5 | mixed reasons collapsed | unit |
| D6 | always singular | unit |
| S1 | no access note | 6 incl. both `snapper_access` direction tests |
| S2 | no `lastRun` guard (bare entry compared) | `no_change_without_an_earlier_run_here` |
| S3 | only degraded → ok | `a_grant…`, `a_failure…` |
| S4 | only ok → degraded | 6 incl. `a_grant…`, `a_failure…` |
| S5 | a note for every collector | `a_first_successful_run_after_a_degraded_init_is_no_reset` |
| S6 | old and new message swapped | `a_grant…` |
| S7 | the run's own message dropped | `a_grant…` |
| S8 | the note carries a case | `a_grant…` |

## Verified by

- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`: clean.
- Full `cargo test --no-fail-fast`: 0 failures (idempotency 34, doctor
  31, lib incl. the new unit test).
- `bash scripts/docs-check.sh`: ok (its existing warning on
  `de/06-configuration.md` is not from this WP).
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

## Decisions needed (none blocking)

1. **Which degraded states count for the note.** I followed the WP
   literally: any change of `ok` (permission, `snapper` missing, a
   timeout, unexpected output). The detail always carries the message,
   so the history says which. A snapper that times out once writes two
   notes. Alternative: count only `NO_PERMISSIONS` against ok, a
   two-line change in `access_change` (then a timeout between two ok runs
   is silent, and ok → timeout → no permission is never recorded).
2. **Downgrade.** An engine before WP-091 requires `lastRun`, so it
   cannot load a `cursors.json` that holds a bare entry: every capture
   fails (exit 2) and doctor offers `mv cursors.json …bad`, which then
   records a state reset. Only after a downgrade and only while a mark
   waits. Alternatives: give the bare entry a `lastRun` (the index would
   then claim a run that did not happen) or keep marks of not-run
   collectors in a separate top-level map in `cursors.json` (old engines
   ignore it and drop it on save, losing the mark). I chose the bare
   entry, as the WP's design note says.
3. **User guide.** `10-troubleshooting.md` (en and de) explains the
   "next capture will record a state reset" row but not the new waiting
   row. A small docs follow-up (en + de translation).
4. **A disabled collector's mark is not in doctor.** `pending_reset`
   covers enabled collectors only (WP-083), so doctor shows the row once
   the collector is enabled again. Accept, or have doctor mention
   disabled marks too.

## Notes

- No guard block happened.
- The real host was not touched; all runs used the tests' scratch HOME
  and stub programs. `redact.rs`, `hook.rs`, `snapper.rs` and the plugin
  are untouched.
- A crash between the ledger append and the cursor save repeats the
  access note on the next capture, as it repeats a state-reset note;
  same accepted limitation.
