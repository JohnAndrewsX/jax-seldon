```
WP-104 HANDOVER
Done: a crash after a capture appended the first events of a source it baselined without a note no longer makes the next capture record a false state reset (probes P1, P2); new field `silentBaselines` in cursors.json, saved with the WP-099 mark, keyed by logbook; doctor after a crashed reset says the next capture warns, not "will record"; 7 new tests, 3 changed; SPEC §2 §3, CHANGELOG, TESTING, user guide (en + de), pitfalls
Not done: nothing of the WP; the field name differs from the WP's example (Decision 1)
Verified by: flock /tmp/seldon-check.lock just check → exit 0 (`check: ok`), run after the last mutant and again on the final tree; cargo test (engine) 0 failures; fmt + clippy --all-targets -D warnings clean; cargo check --release --test idempotency ok; docs-check ok; 19 mutants: 17 killed, 2 equivalent
Learned: memory/pitfalls.md, section "WP-104"
Decisions needed: 3, none blocking (below)
Touched outside WP scope: docs/user/{en,de}/10-troubleshooting.md (one bullet each, the new doctor row), docs/TESTING.md, memory/pitfalls.md
```

Branch `wp/104-no-false-reset`, worktree `wt/WP-104`, from `71c7d9a`.
No push, no PR. Commits (oldest first):

- `52e0470` engine: a crash after a silent baseline gives no false state reset; doctor warns of a crashed reset (WP-104)
- `ea87fb5` docs: SPEC-ENGINE silentBaselines and doctor's crashed-reset row, changelog, testing, user guide (WP-104)
- `2317a6c` engine: test that a second crash keeps the first silent baselines (WP-104)
- `b65d3d1` memory: pitfalls from WP-104
- `1d0d179` docs: German troubleshooting source line after WP-104
- this commit: handover

## Design

### Capture (`capture.rs`)

The ledger rule (`held_losses`: a loss counts only when the ledger holds
an event of its source) is the one input a crash after the append
changes. The WP-099 mark keeps everything else the same. So the fix
marks which sources the rule said "not held" for:

1. `silent` = the sources whose baseline the capture takes (`lost` after
   the binding gate) **or leaves waiting** (degraded or not-run
   candidates, WP-088/091) and that the ledger holds no event of.
2. The save before the append (WP-099) now also runs when there is no
   note. It adds `silent` to `silentBaselines[<canonical logbook>]`,
   after the names already there (union, run order). It saves only when
   the marked file differs from the loaded one, so a normal capture
   writes nothing extra.
3. The save after the append clears `silentBaselines`, as it clears
   `pendingNotes`.
4. The next capture leaves the marked sources of *its* logbook out of the
   ledger rule (`held_sources`, used by the reset note, the waiting marks
   and doctor). The events the crashed append wrote are then no loss.

Why include the waiting candidates: a collector the crashed capture did
not run appends nothing. But the theme hook or `seldon event` can give
its source an event before the next capture, which would otherwise be a
false reset. Without the crash, that collector has no loss either.
Test: `a_collector_not_run_by_the_crashed_capture_is_marked_too`.

Why keyed by logbook: the marked file is saved "as loaded". With a lost
state directory it has no `logbook`, and with another logbook's state it
names that other one. A flat list would hide a genuine loss in the next
capture of another logbook. Test: `a_silent_mark_counts_for_its_own_logbook_only`.

Refactor in passing: the ledger is now read once per capture for the
reset and the waiting marks together (`held_sources` over both, then
`only_held`), not once for each. `state_reset` no longer reads the
ledger. `waiting_baselines` is now `waiting_candidates` (the gate only),
and the ledger rule is applied at the call site.

### Doctor (`doctor.rs`, `capture::pending_reset`)

`pending_reset` returns a struct `PendingReset { binding, lost, recorded }`.
It applies the same `silentBaselines` exclusion. It then splits off the
losses whose `state-reset` note the ledger already holds at a
`pendingNotes` time (the WP-099 lookup, `pending_notes` + `noted_sources`).
`check_pending_reset` gives:

- `lost` → the WP-083 row, unchanged: "the next capture will record a
  state reset for …".
- `recorded` → a new degraded row, with the same reason and fix: "the
  next capture will warn of the state reset for <sources> that a capture
  recorded before it stopped without saving its state: cursors missing
  in <state dir>, so changes made since the last completed capture may
  not be recorded".
- If both apply (e.g. the crashed capture ran `--source pacman`), both
  rows show. The "record" row names exactly what the next note will name.

A collector with a `pendingBaseline` whose note was already recorded
goes to the new row, not to the waiting row: its gap is recorded.

### Backward compatibility

`silentBaselines` has a serde default and is written only while set.
Older files load unchanged (`a_cursors_file_without_silent_baselines_reads_unchanged`,
byte-exact round trip). An older engine ignores the field and drops it
on save (no `deny_unknown_fields`), which only brings back the false
note after a downgrade-across-a-crash. No schema, index or contract
change.

## Tests (`engine/tests/idempotency.rs`, `crash::`)

New:

- `a_crashed_first_capture_gives_no_note` (P1): `init --no-capture`, crash
  after the append of the first capture. The marked file is exactly
  `{"collectors": {}, "silentBaselines": {<root>: [all six]}}` with no
  `pendingNotes`, and doctor shows neither row. The next capture writes
  0, gives no warning, and saves bound without the field. A later lost
  state directory still writes `snapper,pacman`.
- `a_source_first_recorded_by_the_crashed_capture_gets_no_note` (P2,
  replaces the WP-099 test of the limitation): the reset notes are
  `["pacman"]` only, the warning names pacman only, and doctor shows the
  "warn" row and no "record" row. A later genuine loss writes
  `snapper,pacman`.
- `a_crash_before_the_append_marks_the_silent_baselines`.
- `a_collector_not_run_by_the_crashed_capture_is_marked_too` (theme-hook
  event in between).
- `a_second_crash_keeps_the_first_silent_baselines` (`--source pacman`,
  then `--source omarchy`, both cursors unreadable with no events of
  their source; the third capture records nothing).
- `a_silent_mark_counts_for_its_own_logbook_only` (crash on another
  logbook; an unreadable pacman cursor here is still recorded).
- `a_cursors_file_without_silent_baselines_reads_unchanged`.

Changed:

- `the_state_reset_note_is_written_once`: the exact marked file now
  includes `silentBaselines`. Doctor gives the exact "warn" row and no
  "record" row. The completed save has no `silentBaselines`.
- `a_source_the_note_did_not_name_is_recorded`: doctor gives "record"
  for snapper and "warn" for pacman.
- The P2 test above.

## Mutants

The script is in my scratchpad (`mutants/run.py`, private). It applies
each mutant to the committed tree, runs `cargo test --no-fail-fast
--test idempotency --test doctor --test index --lib`, and restores with
`git checkout HEAD --` plus `touch`. There were no build errors.
`just check` ran after the last mutant.

| # | Mutant | Result | Killed by |
|---|---|---|---|
| W1 | capture ignores `silentBaselines` | killed | P1, P2, not-run |
| W2 | `held_sources` ignores `unheld` (capture + doctor) | killed | P1, P2, not-run |
| W3 | silent marks saved only together with a note | killed | 4, incl. P1 |
| W4 | completed save keeps `silentBaselines` | killed | 4 |
| W5 | silent leaves out the waiting candidates | killed | `a_collector_not_run…` |
| W6 | marks of any logbook count | killed | `a_silent_mark_counts_for_its_own_logbook_only` |
| W7 | marks saved under an empty path | killed | 6 |
| W8 | new silent names replace the carried ones | killed (after the new test) | `a_second_crash_keeps_the_first_silent_baselines` |
| W9 | marked file never saved | killed | 10 |
| W10 | marked file always saved | **equivalent** | — |
| W11 | silent includes held sources | killed | 4 |
| W12 | `unheld_sources` keeps duplicates | **equivalent** | — |
| W13 | doctor ignores `silentBaselines` | killed | P1, P2, not-run |
| W14 | doctor: no recorded split | killed | 3 |
| W15 | doctor: any pending note marks every loss recorded | killed | `a_source_the_note_did_not_name_is_recorded` |
| W16 | doctor: recorded row dropped | killed | 3 |
| W17 | doctor: recorded row says "will record" | killed | 3 |
| W18 | waiting marks skip the ledger rule | killed | 4 `state_reset::` |
| W19 | doctor recorded row: wrong reason | killed | `the_state_reset_note_is_written_once` |

Why W10 and W12 are equivalent:

- **W10**: when nothing is added, the marked file equals the loaded one.
  Saving it rewrites the same state. With no file it writes
  `{"collectors": {}}`, which loads as no file does (`Binding::None`, no
  entries). The only difference is one extra write per capture. This is
  why the guard exists, but no test can observe it.
- **W12**: the merge loop into `silentBaselines` drops a name already
  there, so a duplicate from `unheld_sources` never reaches the file.

## Decisions needed (none blocking)

1. **Field name `silentBaselines`, not the WP's example
   `pendingBaselines`.** `pendingBaseline` already exists per collector
   (WP-088, "a baseline waits") with a different meaning. A sibling field
   `pendingBaselines` would read as its plural. The shape is a map
   `{<canonical logbook>: [sources]}`, not a list, for the reason in
   "Why keyed by logbook". Please confirm, or name the field you want.
2. **Two doctor rows after a crash while bound here.** With cursors
   bound to this logbook (e.g. an unreadable pacman cursor), the marked
   file keeps the old `lastRun`s. The ledger's crashed note is newer, so
   the WP-081 row "the last capture recorded a state reset …" shows as
   well as the new "will warn" row. Both are true and have the same fix.
   After a lost state directory (the common case) only the new row
   shows. I left this as is. Merging the rows would touch `check_reset`,
   which is not this WP.
3. **Residual edge, accepted in my view.** The exclusion is per source,
   not per loss kind. If a source was baselined silently by the crashed
   capture and *another* loss of the same source arises before the next
   capture (e.g. `owned.json` gets corrupted in between), that one
   capture does not record it. Telling the two apart would need kinds in
   the mark. It needs a crash plus a second, independent loss in the
   window before the next capture.

## Notes

- No guard-hook block happened.
- The host was not touched; all runs used the tests' scratch HOME and
  stubs. `redact.rs` (WP-106) and `collectors/config.rs` (WP-107) were
  not touched. `collectors/mod.rs`: one field on `Cursors` (+ doc), as
  briefed.
- SPEC §3: the WP-099 limitation sentence is removed. The half on the
  crashed note's `recorded` time stays as a plain statement, because it
  is still true. The doctor paragraph has the exclusion and the new row.
  §2 has the field.
- `just check` ran twice: after the last mutant (exit 0), and on the
  final tree after the German source-line commit (result below).

## `just check` (final tree, `1d0d179`)

`flock /tmp/seldon-check.lock just check`: exit 0, ending with
`check: ok`, no docs-check warning. All cargo `test result` lines
0 failed (fmt, clippy with and without `watch` clean).

| Suite | Passed | Failed |
|---|---|---|
| bar-view | 143 | 0 |
| panel-view | 782 | 0 |
| overlay-view | 319 | 0 |
| service-states | 314 | 0 |
| install.test | 209 | 0 |
| deploy-test-host.test | 190 | 0 |
| real-home-guard | 11 | 0 |
| model.test.js | 89 | — |
