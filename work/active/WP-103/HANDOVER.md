```
WP-103 HANDOVER
```

Branch `wp/103-replay-twins`, base `1b23cf4`. Commits:

- `d00dbeb`: `replay` with twins, the ledger dedupe removed, tests;
- `e54e726`: SPEC-ENGINE §4 (config replay) and §7 (the `:` clause);
- `9ccd308`: CHANGELOG;
- `b5a5446`: pitfalls;
- `bae0067`: one more test (it kills mutant M11);
- this file.

## Done

1. **`replay` maps a subject to every file with that redaction**
   (`engine/src/collectors/config.rs`).
   - **Candidates.** The files of the cursor's generation (`base`) and
     of the generation the failed capture stored (`seen`, the manifest's
     current one), grouped by their redaction. A subject in which nothing
     is masked still names its own path, as before.
   - **Choice.** An event goes to a file whose state in `base` is its
     `hashFrom` (no file for an addition). If several files fit, it goes
     to the one `seen` has in the state the event leaves. An event that
     still fits several files alike (same content) waits until no other
     event can be placed. It then takes the first of them in path order:
     first among those that end right, otherwise among those that fit.
     Then the strict passes start again.
   - **Why `seen` and not the scan.** After one failed save, `seen` is
     exactly `base` plus the recorded events. So for different contents
     the choice is always unique, and for the same content the files are
     interchangeable in the ledger. The scan can have moved on since. The
     waiting step covers two failed saves in a row, where `seen` holds
     only the last capture's end states.
   - `replay` no longer takes the scan.
2. **The ledger dedupe after the diff (`unrecorded`, WP-069) is
   removed.** This goes beyond the WP text; see Decisions 1.
   - **After a replay it is dead code.** An event left unplaced fits no
     candidate. A diff event for file `P` starts from `P`'s final `base`
     state. If the two were equal, the leftover would fit `P`, which
     contradicts it being unplaced. `rescope` only drops files or adds
     them as scanned, so it creates no diff event either. Mutant M6b, the
     old dedupe only after a replay, is killed for the same reason: it
     compared with *all* ledger events, the applied ones included.
   - **Without a replay (cursor not behind) it only did harm.** The
     ledger range `[checked, now]` then holds only events the base
     already reflects: a removal is stamped with the capture time, which
     is the next `since`. Two losses, both now tested:
     - **a twin with the same content removed after the other** was
       dropped as "already recorded". This happens in the normal path,
       with no failure;
     - **`A→B` taken again** after `A→B` and `B→A` were recorded while
       the cursor stayed on `A`. The manifest's current hash then equals
       the cursor's, so that capture is not behind.
3. **SPEC-ENGINE §7:** the "character other than white space" clause of
   the `email` rule now says "(also a marker another rule left there)".
   - Checked against the code with a throwaway test (deleted):
     `mail me@example.com:glpat-EXAMPLEexample00000000 x` →
     `mail me@example.com:‹redacted› x`. The address stays, because
     `gitlab-token` runs first and `email` sees `:‹`.
   - With a space after the `:`, both are masked.
4. **SPEC-ENGINE §4**, the paragraph "Dedupe (WP-069)" is now "After a
   failed cursor save (WP-069, WP-073, WP-103)". It describes the choice
   among twins and says that a capture whose cursor is not behind drops
   nothing. The module doc of `config.rs` is updated as well.
5. **CHANGELOG** `[Unreleased] / Engine`: one entry.
6. **Tests** (`engine/tests/idempotency.rs`; made-up names only:
   `alice.webapp@` / `bob.webapp@` / `carol.webapp@example.com`). There
   are helpers for twin desktop entries, and steps are read back as
   (kind, from-name, to-name), every event under the one redacted
   subject.
   - `twins_after_a_failed_cursor_save_are_told_apart_by_their_hashes`
     is the WP's test. Each phase (add, change, remove) has both twins
     move, then a failed cursor save. Then one twin moves on (Alice
     changes again, goes back, comes back), and the recovery capture
     records exactly that step. A further capture writes nothing, and the
     ledger holds 9 events, each once. In the add phase Bob's file is
     older (`set_modified`), so his event comes first.
   - `twins_with_the_same_content_after_a_failed_cursor_save` uses
     three files, two of which change alike. Then one changes, the save
     fails, and another changes alike.
   - `twins_with_the_same_content_removed_one_after_the_other`: the
     normal path, no failure.
   - `a_twin_added_and_removed_around_a_failed_cursor_save_is_recorded`:
     the twin exists only in `seen`.
   - `twins_after_two_failed_cursor_saves`: the waiting step.
   - `a_twin_removed_and_added_again_across_two_failed_cursor_saves`
     kills M11.
   - `a_step_taken_again_after_two_failed_cursor_saves_is_recorded`:
     no twins, cursor not behind.
   - `a_file_whose_removal_the_ledger_lost_after_a_failed_cursor_save_is_recorded`
     covers an unredacted subject naming its own path (a WP-073 path
     that had no test).

## Not done / known limits

- **Same-content twins and the previous capture's events.** The replay
  reads from the cursor's `checked`, and a removal of the capture that
  saved that cursor carries exactly that time. That event is already in
  `base`, but it can fit a twin with the same content.
  - Example: Alice removed in a good capture, any failed save in the next
    one, then a recovery. Alice's removal goes to Bob in the replay, and
    the diff writes a false `config-add` for Bob.
  - It needs the same content *and* a failed save. An addition or change
    of that capture does the same only if the file's mtime was in the
    capture's second.
  - Not fixed. Skipping removals at `ts == since` would fix it, but it
    loses a re-addition when a failed capture ran in the same second as
    the previous one. See Decisions 2.
- **Redacted intermediate files across two failed saves.** A file added
  in the first failed capture and removed in the second one, whose
  ledger write also failed, is in no generation the next capture has.
  An unredacted subject still names its own path (tested). A redacted
  one names nothing, so the removal is not recorded (unchanged since
  WP-073).
- **Not looked at:** `plugins.rs` has its own `unrecorded` (keyed by
  plugin id, so no redaction issue); `redact.rs` (WP-097) and
  `capture.rs` (WP-099) are untouched.
- **No CHANGELOG line for §7.** The SPEC clause documents existing
  behaviour, so no behaviour change.

## Verified by

- **Pre-fix run.** The first eight tests against the old `config.rs`
  (`d00dbeb` without the `src` change): 6 failed:
  - `twins_after_a_failed_cursor_save_…`: `[ConfigAdd None→Alice 2]`
    instead of `Alice 1→Alice 2`;
  - `twins_with_the_same_content_after_…`: `[]` instead of one change;
  - `…removed_one_after_the_other`: `[]` instead of one removal;
  - `a_twin_added_and_removed_…`: `[]`;
  - `a_step_taken_again_…`: 0 instead of 1;
  - `a_file_whose_removal_the_ledger_lost_…`: failed in its first form.
    That form missed the case (the manifest was back on the cursor's
    hash, so there was no replay). It now changes a second file, and it
    passes on the old and the new code: it guards the kept fallback.
  - `twins_after_two_failed_cursor_saves` passed on the old code; it is
    there for M4. The ninth test (`bae0067`, for M11) was not run
    against the old code.
- `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings`
  are clean. `cargo test --no-fail-fast` is green (`idempotency`: 42
  passed).
- `scripts/docs-check.sh`: ok.
- **`flock /tmp/seldon-check.lock just check` at `bae0067`: exit 0**
  (21:21–21:32). Results: service-states 314/0, panel-view 782/0,
  overlay-view 319/0, bar-view 143/0, install 209/0, real-home-guard
  11/0, qmllint 29 files, docs-check ok, `check: ok`.
  The commit after `bae0067` adds only this file.

### Mutants (`config.rs`; each one runs the whole engine suite with `--no-fail-fast`, script in the session scratchpad; restored with `git checkout HEAD --` + `touch`; `git status` clean afterwards)

| # | Mutant | Caught by |
|---|---|---|
| M1 | one path per subject (the last wins, the old map) | `twins_after_a_failed_…`, `twins_after_two_…`, `twins_with_the_same_content_after_…` |
| M2 | no preference by `seen` | `twins_after_a_failed_…`, `twins_after_two_…`, `twins_with_the_same_content_after_…` |
| M3 | an event that fits several alike is never placed | `twins_with_the_same_content_after_…` |
| M4 | such an event is placed at once (no waiting) | `twins_after_two_…` |
| M5 | `seen`'s files are no candidates | `a_twin_added_and_removed_…`, `twins_after_a_failed_…`, `twins_after_two_…` |
| M6 | the old ledger dedupe on every capture | `a_step_taken_again_…`, `twins_with_the_same_content_after_…`, `…removed_one_after_the_other` |
| M6b | the old ledger dedupe only after a replay | `twins_with_the_same_content_after_…` |
| M7 | no own path for an unredacted subject | `a_file_whose_removal_the_ledger_lost_…` |
| M8 | the guess takes the first fitting file, not the first that ends right | `twins_with_the_same_content_after_…` (three files) |
| M10 | an addition fits a file that exists | `twins_after_two_…` |
| M11 | a removal fits any existing file | `a_twin_removed_and_added_again_…` (`bae0067`; it survived the first run) |
| M9 | a single fitting file is not sure by itself | **survives** |
| M12 | no early return on an empty ledger range | **survives** (performance only) |
| M13 | events in ledger order, not by time (WP-073 code) | **survives** |

- **M9.** Such an event waits, and the guess then places it on the same
  file, in time order: the order the strict pass would have used. I
  found no input where the end state differs. The short cut keeps
  `hashFrom` as the primary key and skips the `seen` lookup.
- **M13.** Within one capture each file has at most one event, and the
  captures append in time order. Also, `seen` decides between twins, so
  ledger order and time order give the same result here.

## Learned

Appended to `memory/pitfalls.md`:

- a key built from a redacted subject is not a key;
- a removal's time is the next capture's `since`;
- "cursor behind" is a hash comparison and can miss a failed save;
- `File::set_modified` orders config events in a test.

## Decisions needed

1. **Removing the dedupe on the normal path** goes beyond the WP's
   wording ("replay … after a failed cursor save"). It is the same
   collision, though (a key from the redacted subject), and it lost an
   event without any failure. The commit can be reverted alone in
   `config.rs`; the three tests that kill M6 would then fail.
2. **Same-content twins and the previous capture's removal** (above):
   leave as documented, or skip removals at `ts == since` in the replay
   (a follow-up of about 3 lines plus a test)?
3. **Privacy note for the `redact.rs` owner (WP-097).** An address
   directly followed by `:` and another rule's marker stays unmasked:
   `me@example.com:‹redacted›`. SPEC now says so, as the WP asks. Whether
   that is wanted is a redaction decision, not mine.

## Touched outside WP scope

- **SPEC-ENGINE §4**, the config replay paragraph. AGENTS.md §7 requires
  it: the code changed what that paragraph described.
- `memory/pitfalls.md` (append).
- No guard-hook blocks. Nothing outside the repository. Only scratch
  logbooks were used (test benches); the operator's logbook was not.
- The mutant script and the old `unrecorded` copy are in a private
  subdirectory of the session scratchpad. The throwaway redaction
  probe test was created and deleted in one command and never
  committed.

## Round 2 (stage 1 SEND BACK B1; stage 2: design (a) with one correction)

Commits:

- `c766e3e`: replay on every capture, tests;
- `8e85a19`: the CLI test changes its file between the captures;
- `7d5b0fe`: SPEC §3/§4, rewrap;
- `9522e87`: the window as the brief says (see "Correction" below);
- `77a2dc3`: SPEC §4;
- `79159d2`: pitfalls;
- this section.

### Done

1. **B1: the replay runs on every capture with a cursor and its
   generation** (`config.rs` ≈ 1000).
   - `seen = stored_ahead.unwrap_or(&current)`. When behind, `seen` is
     the manifest's current generation, as built in round 1. Otherwise
     it is this capture's scan, so a twin added after the backup is a
     candidate.
   - **Window** (`replay(…, behind)`): when behind, from `since` on,
     skipping `ConfigRemove` at exactly `since`; when not, strictly after
     `since`. `.min(ctx.now)` stays as the read guard. `read_range` with
     `from > to` returns an empty list: its filter is
     `ts >= from && ts <= to`, with no error path. The "strictly after"
     is a filter on `ts`, not `+ 1 s`, so it does not depend on whole
     seconds.
   - **Correction: I first deviated from the brief and then went back
     to it.** In `c766e3e` I read additions and changes at `since` also
     when the cursor was not behind. The reason: a file with an older
     mtime (`cp -p`) is stamped with `since`, and the brief's window
     records such a change again after a restore.
     - The full suite then showed the cost:
       `own_writes::removing_the_theme_hook_leaves_no_drift` failed 9 of
       10 runs on `8e85a19`. Its real-time captures fall into one
       second, so the previous capture's own addition at `since` was
       replayed and the removal written twice. That happens in normal
       operation, with no failure and no restore.
     - The brief's window risks a duplicate only after a restore of a
       file with an older mtime. So `9522e87` follows the brief, and
       SPEC §4 names both limits.
     - After `c766e3e` I ran only `idempotency`, not the whole suite.
       That is noted in pitfalls.
2. **Tests** (`engine/tests/idempotency.rs`):
   - `a_restored_older_state_directory_records_only_what_changed_since`:
     r07. It changes `a.conf` and removes `gone.conf`, then restores.
     It asserts 0 events, then B→C and not A→C.
   - `a_restored_state_directory_knows_a_twin_added_after_the_backup`:
     a redacted twin added after the backup.
   - `a_backup_restored_after_a_state_reset_records_nothing_twice`: r08.
     It clears the cursors and `manifest.json`, takes a baseline, then
     restores.
   - `a_backup_restored_after_two_steps_writes_no_shortcut`: A→B→C.
   - `a_state_directory_restored_through_the_cli_records_nothing_twice`:
     c03, the guide-07 steps through `seldon capture --all --json` with
     `SELDON_NOW`. The folder is copied with a small `copy_dir` (the test
     PATH has only stubs), and `Cli::command` was split out of `Cli::run`.
     The file's mtime is set between the captures. Without that it lies
     before `SELDON_NOW` and is clamped to the check, the limit above.
   - `a_removal_at_the_cursors_check_is_not_replayed_onto_a_twin`: r10.
     Same-content twins: Alice is removed in a good capture, the next
     save fails, then the recovery.
   - `a_change_clamped_to_the_cursors_check_is_replayed_after_a_failed_save`:
     the behind window reads from `since` on.
   - `a_file_added_and_removed_within_one_second_is_removed_once`: the
     not-behind window reads strictly after `since`. It is the Bench
     shape of the theme-hook flake.
   - `a_redacted_file_whose_removal_the_ledger_lost_is_recorded`: the
     probe, below.
3. **N1 (X4): a comment, not a test.** At the `continue`: "a placement
   late in a pass can make a step the pass already went by strict, while
   the first step that fits may still be ambiguous". X4 survives (see
   below).
4. **SPEC.**
   - §3: the brief's sentence. Before writing it I checked the other
     diff collectors: plugins (`unrecorded`, kind, subject, enabled,
     version) and theme (a `theme-set` to the current slug since the
     check) still dedupe by exact match. The line after it is rewrapped.
   - §4: the paragraph is now "Replay (WP-069, WP-073, WP-103)":
     - the candidates (the cursor's generation and the current one:
       the manifest's when behind, else the scan);
     - a masked subject names its own path;
     - the brief's sentences on the window;
     - two known limits: a same-second capture with a failed save, and a
       change at the restored check after a restore. The history
       sentence is gone.
   - §7: the long line is rewrapped.
5. **Module doc** of `config.rs`, the doc of `replay` (both cases), and
   the **CHANGELOG** entry now cover restores and the A→B→C case.
6. **Probe.** `redact("~/.local/share/applications/Mail (‹redacted›@example.com).desktop")`
   returns the same text (throwaway test, deleted): the local part takes
   no marker quote, and a match inside a marker is left alone. So the
   fallback (`None if redactor.redact(&r.subject) == r.subject`) also
   takes a masked subject as a path.
   - **When no file of that subject is known**, the fallback puts a key
     with the masked name into the local `base`. Only this capture's diff
     sees it; the manifest is written from the scan. A removal the ledger
     lost is then recorded under the subject the ledger holds, which is
     what the real removal would write.
     `a_redacted_file_whose_removal_the_ledger_lost_is_recorded` shows it,
     and M7 now also fails that test.
   - **When a twin of the subject is known**, the subject names only the
     twin. The fallback is not reached, and the removal is lost.
   - **Correction of round 1's "Not done".** "A redacted one names
     nothing, so the removal is not recorded" was wrong for the first
     case. It holds only when a twin with that subject is known.

### Pre-fix evidence

- **Against round 1's `config.rs` (`0920e76`).** These six fail:
  `a_restored_older_…`, `a_restored_state_directory_knows_…`,
  `a_backup_restored_after_a_state_reset_…`,
  `a_backup_restored_after_two_steps_…`, `a_removal_at_the_cursors_check_…`
  and `a_state_directory_restored_through_the_cli_…`. The CLI test fails
  on `written`: 1, config 1 event, against 0.
  - These pass there: the clamped behind test (round 1 already read from
    `since`), the same-second test (round 1 ran no replay without a
    failure) and the probe test (the fallback was there). `own_writes`
    and `redaction` are green.
- **Against stage 1's variant** (`seen` = the manifest's current
  generation also when not behind; mutant R2):
  `a_restored_state_directory_knows_a_twin_added_after_the_backup` fails
  with `[(ConfigAdd, None, Some("Bob 1"))]`, as the brief predicted.
  - The CLI test failed there as well in its first form, because the
    file's mtime was clamped to the check. With the mtime set between
    the captures it passes there and fails on round 1.
- **Against `8e85a19`** (additions at `since` read when not behind):
  `a_file_added_and_removed_within_one_second_is_removed_once` fails
  with a second `(ConfigRemove, "~/.config/hypr/x.conf")`.
  `removing_the_theme_hook_leaves_no_drift` failed 9 of 10 runs there and
  passes 10 of 10 on `9522e87`.

### Mutants (round 2; final code `9522e87`; each runs the whole engine suite with `--no-fail-fast`; restored with `git checkout HEAD --` + `touch`; `git status` clean afterwards)

A first run on `8e85a19` was stopped after M5. There, M1–M4 also "failed"
the theme-hook test, which was the flake above, so those results are
discarded.

| # | Mutant | Caught by |
|---|---|---|
| R1 | replay only when the cursor is behind (round 1) | `a_backup_restored_after_a_state_reset_…`, `a_backup_restored_after_two_steps_…`, `a_restored_older_…`, `a_restored_state_directory_knows_…`, `a_state_directory_restored_through_the_cli_…` |
| R2 | `seen` = the manifest's current generation when not behind (stage 1's variant) | `a_restored_state_directory_knows_a_twin_added_after_the_backup` |
| R3 | behind: a removal at `since` is replayed | `a_removal_at_the_cursors_check_is_not_replayed_onto_a_twin` |
| R4 | not behind: additions and changes at `since` are read too (`c766e3e`) | `a_file_added_and_removed_within_one_second_…`, `removing_the_theme_hook_leaves_no_drift`, `commands::a_desktop_entry_named_after_an_address_is_masked` |
| R5 | behind: strictly after `since` | `a_change_clamped_to_the_cursors_check_is_replayed_after_a_failed_save` |
| M1 | one path per subject | `a_restored_state_directory_knows_…`, `twins_after_a_failed_…`, `twins_after_two_…`, `twins_with_the_same_content_after_…` |
| M2 | no preference by `seen` | `twins_after_a_failed_…`, `twins_after_two_…`, `twins_with_the_same_content_after_…` |
| M3 | an event that fits several alike is never placed | `twins_with_the_same_content_after_…` |
| M4 | such an event is placed at once | `twins_after_two_…` |
| M5 | `seen`'s files are no candidates | `a_restored_state_directory_knows_…`, `a_twin_added_and_removed_…`, `twins_after_a_failed_…`, `twins_after_two_…` |
| M6 | the old ledger dedupe on every capture | `a_step_taken_again_…`, `twins_with_the_same_content_after_…`, `…removed_one_after_the_other` |
| M6b | the old dedupe only after a replay (pattern adapted to `behind: bool`; the first try did not compile) | `twins_with_the_same_content_after_…` |
| M7 | no own path for an unredacted subject | `a_file_whose_removal_the_ledger_lost_…`, `a_redacted_file_whose_removal_the_ledger_lost_is_recorded` |
| M8 | the guess takes the first fitting file | `twins_with_the_same_content_after_…` |
| M10 | an addition fits a file that exists | `twins_after_two_…` |
| M11 | a removal fits any existing file | `a_twin_removed_and_added_again_…` |
| M9 | a single fitting file is not sure by itself | **survives**, as in round 1 |
| M12 | no early return on an empty ledger range | **survives** (performance only) |
| M13 | events in ledger order, not by time | **survives**, as in round 1 |
| X4 | no restart after a pass that placed something | **survives** (N1: commented, not tested) |

### Verified (round 2)

- `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings`
  are clean. `cargo test --no-fail-fast` is green on `9522e87`
  (`idempotency`: 51 passed).
- `scripts/docs-check.sh`: ok.
- **`flock /tmp/seldon-check.lock just check` at `79159d2`: exit 0**
  (22:19–22:28). Results: service-states 314/0, panel-view 782/0,
  overlay-view 319/0, bar-view 143/0, install 209/0, real-home-guard
  11/0, qmllint 29 files, docs-check ok, `check: ok`. The commit after
  `79159d2` adds only this section.

### Decisions (round 2)

- **Restore after an older-mtime change** (SPEC §4 known limit): left as
  the brief decided. The exact marker in the cursor (queued by the
  orchestrator) fixes this limit and the same-second one.
- Decision 3 of round 1: no action, per the brief.

### Touched outside scope (round 2)

- **`engine/tests/idempotency.rs`, `Cli`:** `command()` split out of
  `run()`; `run` is unchanged for its callers.
- `memory/pitfalls.md` (append).
- No guard-hook blocks. Scratch benches only. The mutant script and the
  probe stayed in the session scratchpad; the probe test was deleted
  unseen by git.
