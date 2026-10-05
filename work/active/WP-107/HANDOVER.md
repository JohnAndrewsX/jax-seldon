```
WP-107 HANDOVER
```

Branch `wp/107-replay-marker`, base `71c7d9a`. Commits:

- `b99296a`: the marker in the config cursor, and the replay reads past
  it (`config.rs`);
- `c7a463b`: tests for r13, c03 and the two other known limits, plus the
  marker in the CLI cursor test (`collectors.rs`);
- `c6a0a2a`: SPEC-ENGINE §3/§4;
- `fd4d12f`: CHANGELOG;
- `0524514`: tests for the old-cursor window and an unreadable ledger;
- `9785b3c`, `1528e3d`: a CLI test showing the marker counts config
  events only (it kills K14);
- `1743a69`: pitfalls;
- this file.

## Done

1. **The cursor carries an exact marker** (`engine/src/collectors/config.rs`).
   - New optional field `atCheck` in the config cursor. It holds how many
     config events stamped with the cursor's `checked` the ledger holds,
     in ledger order, once this capture's own events are written. It is
     the ledger's config events at `now` before the append plus the
     capture's own events at `now`.
   - **This is a count, not an event id.** See Decisions 1.
   - It is exact because `capture` holds the state lock from the
     collector's ledger read until its append, so no other config event
     can come between them.
   - Absent when the ledger could not be read (a baseline does not need
     the ledger; a capture that has a cursor degrades, as before).
2. **The replay reads strictly after the marker.**
   - The ledger is read once per capture, from the cursor's check (or
     from `now` for a baseline) to `now`, config events only. The replay
     and the marker count both use that read.
   - The replay reads every event stamped after `since`. Of the events
     stamped with `since`, it reads those after the first `atCheck`.
     That is the same rule whether or not the cursor is behind;
     `behind` still chooses `seen`.
   - **A cursor without the field** (saved before WP-107) reads the
     WP-103 window unchanged: when behind, from `since` on without the
     removals at `since`; when not, strictly after `since`. The next
     capture saves a marker.
   - Old cursors load: `typed_cursor` reads them (`serde(default)`), so
     `cursor_reads` and doctor see no change. A downgraded engine
     ignores the unknown field (no `deny_unknown_fields`).
3. **Tests** (`engine/tests/idempotency.rs`; made-up names only):
   - `a_change_at_the_cursors_check_is_not_replayed_onto_a_twin`: **r13**.
   - `a_removal_at_the_cursors_check_is_not_replayed_onto_a_twin`: **r10**
     (WP-103; it now passes through the marker, not the removal skip).
   - `a_change_in_the_second_of_the_restored_check_is_not_recorded_twice`:
     **c03** through `seldon capture --all`. It runs twice: once pinned
     (`SELDON_NOW`, and the file's mtime in the second of the backed-up
     check), and once on the real clock without `SELDON_NOW`, as the
     reviewer ran it. The old WP-103 CLI test is now the same helper
     with the mtime between the captures.
   - `a_removal_by_a_capture_in_the_same_second_as_the_check_is_replayed`:
     WP-103 limit 1. It checks both outcomes: no duplicate removal, and
     no missed re-addition when the file is back.
   - `a_restored_state_directory_does_not_repeat_a_change_at_its_check`:
     WP-103 limit 2, with an older mtime.
   - `a_marker_counts_the_events_of_earlier_captures_in_the_same_second`:
     two captures at 10:10; the second one's marker is 2.
   - `crash::the_marker_counts_config_events_only`: CLI. A theme event at
     the check comes before the config removal in the ledger. The cursor
     save is really skipped (`SELDON_TEST_CAPTURE_CRASH=after-append`).
   - `a_cursor_without_the_marker_reads_the_window_it_was_saved_for`:
     four parts (not behind: a removal and an addition at the check;
     behind: a removal and a clamped change at the check).
   - `a_ledger_that_cannot_be_read_gives_no_marker`: `chmod 000` on the
     bench's ledger folder. It skips itself when the mode has no effect
     (root).
   - `collectors::capture_runs_on_seldon_now` also checks `atCheck: 1`
     in the `cursors.json` that the real `capture` writes.
   - Every WP-103 replay and restore test is unchanged and green.
4. **SPEC-ENGINE.**
   - §4, the replay paragraph (now "WP-069, WP-073, WP-103, WP-107"):
     the window sentences and the three known limits are replaced by the
     marker rule, why a count is exact, and the old-cursor fallback.
   - §3: the capture sentence says "replaying the config events after
     its cursor's marker" and is rewrapped.
   - Module doc of `config.rs` and the doc of `replay` follow.
5. **CHANGELOG** `[Unreleased] / Engine`: one entry naming the three
   fixed cases.

## Not done / known limits

- **Not looked at:** `capture.rs` (WP-104) and `redact.rs` (WP-106) are
  untouched. `plugins.rs` and `theme.rs` keep their own windows (exact
  match against the ledger, as WP-103 checked).
- **What the marker cannot fix:**
  - **Clock set back.** A capture whose `now` lies before the cursor's
    check reads nothing. That is the `.min(ctx.now)` guard, as before.
  - **Lines that read differently later.** A line at the check that a
    later engine version could not parse any more would shift the count.
  - Both are outside the scenarios the WP names; SPEC does not list
    them as limits.
- The WP-103 limit "redacted intermediate files across two failed saves,
  with a twin known" is unchanged; it is not a window question.

## Verified by

- **Pre-fix run.** The new tests ran against the code at base `71c7d9a`;
  the tests came first in this session.
  - r13: `[(ConfigChange, "…Mail (‹redacted›@example.com).desktop")]`
    after recovery.
  - Limit 1: `[(ConfigRemove, "~/.config/hypr/x.conf")]` written again.
    The re-addition part was checked separately under K1, which is the
    old window: `[]` instead of `[(ConfigAdd, x.conf)]`.
  - Limit 2: `[(ConfigChange, "~/.config/hypr/a.conf")]` written again.
  - c03 pinned: `written: 1` instead of 0. c03 on the real clock, run
    alone under K1: failed 3 of 3 (`written: 1`).
  - The marker and old-cursor tests fail on the old code at their marker
    asserts (no `atCheck` field).
- `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings`
  are clean.
- `cargo test --no-fail-fast` (whole engine suite) at `b99296a` +
  `c7a463b`: 784 passed, 0 failed. At `1743a69` it ran inside
  `just check` (below); `idempotency` has 66 tests.
- **Stability:**
  - `a_change_in_the_second_of_the_restored_check_…` (it includes the
    real-clock run) passed 10 of 10.
  - `own_writes::removing_the_theme_hook_leaves_no_drift` (the WP-103
    flake shape) passed 10 of 10.
- `scripts/docs-check.sh`: ok.
- **`flock /tmp/seldon-check.lock just check` at `1743a69`: exit 0**
  (01:19–01:38).
  - Cargo test results over all its runs: 1582 passed, 0 failed.
  - service-states 314/0, panel-view 782/0, overlay-view 319/0,
    bar-view 143/0, install 209/0, deploy-test-host 190/0,
    real-home-guard 11/0, model.test.js 89; qmllint 29 files;
    docs-check ok; `check: ok`.
  - The commit after `1743a69` adds only this file.

### Mutants (`config.rs`)

How they ran:

- each one against the whole engine suite with `--no-fail-fast`;
- script and patterns in a private subdirectory of the session
  scratchpad;
- `config.rs` restored with `git checkout HEAD --` + `touch`, and
  `git status` clean afterwards.

K1–K16 ran at `0524514`. K14 ran again at `9785b3c` and at `1528e3d`.

| # | Mutant | Caught by |
|---|---|---|
| K1 | marker ignored (every cursor reads the WP-103 window) | r13, c03, limit 1, limit 2 (4 tests) |
| K2 | a cursor without the marker reads every event at the check | `a_cursor_without_the_marker_…` |
| K3 | `>= n`: the cursor's own last event at the check is read again | r10, r13, `a_marker_counts_…`, `…removed_one_after_the_other`, `commands::a_desktop_entry_named_after_an_address_is_masked` |
| K4 | `> n + 1`: one later event at the check is skipped | c03, limit 1, limit 2, `a_change_clamped_…` |
| K5 | the marker leaves out the capture's own events | r10, r13, limit 1, `a_marker_counts_…`, `…removed_one_after_the_other`, `collectors::capture_runs_on_seldon_now`, `commands::a_desktop_entry_…` |
| K6 | the marker leaves out the ledger's earlier events at `now` | `a_marker_counts_…`, `commands::a_desktop_entry_…` |
| K7 | the marker counts every event read, not only those at `now` | limit 1 (the `y.conf` removal at 10:00 is there for it) |
| K8 | the ledger is read from `now` only | 23 tests |
| K9 | an event before `since` in the read counts as after it | **survives**: equivalent. The read starts at `since.min(now)`, so only a clock set back gives such an event. |
| K10 | old cursor: nothing at the check when behind | `a_cursor_without_the_marker_…` |
| K11 | old cursor: removals at the check read when behind | `a_cursor_without_the_marker_…` |
| K12 | old cursor: additions at the check read when not behind | `a_cursor_without_the_marker_…` |
| K13 | `atCheck: null` written instead of no field | `a_ledger_that_cannot_be_read_…` |
| K14 | the marker and the replay count events of every source | `crash::the_marker_counts_config_events_only`. It survived the first two runs; see below. |
| K15 | a read error gives a marker of 0 | `a_ledger_that_cannot_be_read_…` |
| K16 | a read error with a cursor replays nothing instead of degrading | `a_ledger_that_cannot_be_read_…` |

- **K14.** The bench runs one collector at a time, so no other source's
  event ever stood at the check.
  - The first CLI test checked only the last capture's `written`, so
    K14 still survived it. Under K14 the false `config-add` for the twin
    is written by the crashed capture already: it appends before it
    stops, and it reads the ledger with the same cursor.
  - `1528e3d` checks every config event in the ledger. That kills K14.
- **Not run as a mutant:** counting every event as the ordinal at the
  check (not only those stamped with it). It is equivalent: an event
  stamped later than the check comes before the cursor's own events in
  the ledger only when the clock was set back.

## Learned

Appended to `memory/pitfalls.md`:

- a marker saved before the append can be a count, not an id;
- ledger order is not time order across captures;
- a real-time test kills a mutant only by timing, so pair it with a
  pinned twin;
- `chmod 000` makes a ledger read error testable.

## Decisions needed

1. **A count instead of an event id.** The WP says "the id of the last
   config event the capture wrote".
   - **Why not the id.** The collector returns its cursor before
     `Ledger::append` assigns the ids, and `capture` saves the cursor as
     returned. An id therefore needs `capture.rs` to patch the config
     cursor after the append, and `capture.rs` is WP-104's in this
     round. `memory/pitfalls.md` (WP-099) already notes: "a key that
     must be saved before the append cannot be the id".
   - **Why the count is enough.** It marks the same event, the last
     config event at the check up to and including the capture's own,
     and it is exact under the state lock.
   - **It is also more robust.** "Strictly after an id" compares ULIDs
     across processes. Each `append` uses a fresh generator on the system
     clock, so a clock set back between captures would misorder them.
     The count depends only on the order of lines at one instant within
     one month file.
   - **If an id is still wanted,** it is a follow-up: `capture.rs`
     patches the cursor after the append, plus a fallback for a capture
     without config events. The count would then go.
2. None other.

## Touched outside WP scope

- `engine/tests/collectors.rs`: one assertion (`atCheck` in the CLI
  cursor test).
- `memory/pitfalls.md` (append).
- No guard-hook blocks. Nothing outside the repository. Only scratch
  benches and `common::Env` test homes were used; the operator's logbook
  was not.
- The mutant script and its patterns are in a private subdirectory of
  the session scratchpad.
- The two pre-fix probes (the re-add part and the real-clock c03 under
  K1) were temporary edits of the test file, restored with
  `git checkout HEAD --` in the same command and never committed.
