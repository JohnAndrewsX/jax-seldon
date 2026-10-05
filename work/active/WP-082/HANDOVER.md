WP-082 HANDOVER — snapper collector: one date per snapshot across list and info files
Branch: wp/082-review (worktree wt/WP-082), on 616be66, not pushed.

Commits:
- `0dcdb9c` engine: snapper keeps one date per snapshot in the repeated hour
- `7bddc20` docs: SPEC-ENGINE §4 snapper date rule replaces the known limit
- `881e64d` docs: snapper list shape checked on the dev host
- `19d0028` docs: CHANGELOG line for the snapper date rule
- `85ef55a` memory: WP-082 pitfalls
- this handover

**Done**

- **The rule** (`collectors/snapper.rs`, `snapshot_dates`; module doc;
  SPEC-ENGINE §4). A snapshot has one date, an instant. The info files
  give it in UTC (exact). A list time names one instant, except in the
  repeated hour when summer time ends. There the first of these decides:
  1. the snapshot's info file (`<snapshots>/<n>/info.xml`), when it can be
     read: the canonical date. The list path reads it only for such a
     time, read-only;
  2. the date the cursor knows for the number, when it is one of the two;
  3. the number order: when the earlier instant lies before the date of
     the snapshot numbered before it, the later one;
  4. else the earlier instant.
  In `diff`, a known number whose date is the *other* instant of the same
  local time (`fold_twins`) is the same snapshot. It takes the new date
  without an event. This rule makes a switch of read path in either
  direction event-free. It also migrates old cursors, without a separate
  migration step: an entry written before WP-082 moves to the info file's
  instant the first time the info files are read.
- **Finding: earlier versions did not take the earlier instant.** The SPEC
  and the WP-073 handover said the list was read "as the earlier of its two
  instants". In fact chrono 0.4.45 orders `LocalResult::Ambiguous` by
  offset, the smaller first, and `earliest()` returns that one. So
  `Tz::localize` gave 02:30+01:00, the *later* instant (checked in the
  chrono source, `tz_info/rule.rs` and `timezone.rs`, and by the pre-WP
  code under the new tests, see below). The new code sorts the two
  instants. The SPEC now says that an old cursor held the later instant.
- **DST gap** (the comment at the old line 396): unchanged behaviour,
  newly covered. A list time in the skipped hour names no instant. The
  snapshot is known without a date and without an event, and gets its
  date from a later read without an event.
- **SPEC-ENGINE §4**: the "Known limit … (to be fixed later)" sentences are
  replaced by the rule.
- **History sentences, shape check on this host.** I ran
  `LC_ALL=C snapper --jsonout list` read-only (exit 0, snapper 0.13.1;
  this user may list). Output went to the session scratchpad only,
  nothing went into the repo. Result: one config `root`, 5 entries. Fields
  `subvolume` str, `number` int, `default` bool, `active` bool, `type`
  (`single` only), `pre-number` null, `date` (pattern ok; `""` for 0),
  `user` str, `used-space` null, `cleanup` str, `description` str,
  `userdata` null. No unknown keys, all required keys present.
  `scripts/validate-fixtures.py`'s builtin validator reports 0 errors (and
  3 errors for 3 planted faults). **It matches.** I therefore replaced the
  "not verified on the dev host" sentences in
  `schema/external/snapper-list.schema.json` (description) and
  `fixtures/README.md:205` with "checked against snapper 0.13.1 on the dev
  host (WP-082)". This is not a contract schema: no ADR, no
  `contractVersion` bump. Not seen on the host: a `pre`/`post` pair (a
  numeric `pre-number`) and a non-null `userdata`. Those parts of the
  shape still rest on upstream and the fixtures. The `date` = local time
  claim was not checked: that would need reading `/.snapshots`, which the
  brief does not allow.
- **CHANGELOG** `[Unreleased]` → Engine: one line.
- **Tests** (`engine/tests/collectors.rs`). These are CLI tests: the
  binary runs through `common::Env` with `TZ=CET-1CEST,M3.5.0,M10.5.0/3`.
  chrono reads that POSIX rule without tzdata. `Tz` has no zone variant,
  and `set_var` in the test process would race. Helper `Zoned`: a snapper
  stub that lists `list.json`, or answers `No permissions.` while
  `denied` exists; info files in `<guard>/.snapshots`.
  - `snapper_keeps_one_date_from_the_list_to_the_info_files`: 11 at 02:30
    CET. Listed without an info file it gets +02:00. The info files then
    give 0 events and the cursor takes +01:00. Listing again without the
    info files gives 0 events and the cursor keeps +01:00; listing with
    them, the same. Then a **real deletion (10) and reuse (11) across the
    switch** still give `snapshot-delete 11`, `snapshot 11` (both at the new
    date) and `snapshot-delete 10` at capture time.
  - `snapper_keeps_one_date_from_the_info_files_to_the_list`: 11 at 02:30
    CEST and 12 at 02:15 CET read from the info files, then listed with and
    without the info files: 0 events, the dates unchanged.
  - `snapper_dates_a_listed_snapshot_in_the_repeated_hour`: a new listed
    snapshot gets its info file's instant (+01:00). Without info files,
    11 at 02:40 gets the earlier and 12 at 02:20 the later instant (the
    number order).
  - `snapper_moves_an_older_cursor_to_the_info_file_date`: the ledger line
    and the cursor are set as a pre-WP-082 capture wrote them (+01:00); the
    info file says +02:00. Result: 0 events, and the cursor is moved.
  - `snapper_remembers_a_listed_time_in_the_dst_gap`: 2027-03-28 02:30 is
    listed: no event and no date. The info files then give 03:30+02:00
    without an event.

**Not done**

- **Crash plus path switch plus fold.** The ledger dedupe still compares
  `(number, ts)` exactly. Take a snapshot in the repeated hour, captured
  from the list. The cursor save then fails. The next capture reads the
  info files, which give the other instant. That snapshot would be
  recorded twice. Three rare conditions must meet, so I left it out. The
  fix would be to treat fold twins as equal in `dedupe`'s `seen` set.
  Follow-up candidate.
- No manual scratch-HOME run beyond the CLI tests, which run the binary
  in a temp HOME under `SELDON_TEST_GUARD`.

**Verified by**

- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`: clean.
- `cargo test --no-fail-fast`: every test binary, 0 failures.
- `scripts/validate-fixtures.py`: ok. `scripts/docs-check.sh`: ok (no
  user page changed, so there is no `docs(de)` re-stamp).
- `just check`: see the last section.
- **Mutants** (`snapper.rs`, each restored and touched afterwards; run on
  the snapper tests of `tests/collectors.rs`):
  - M0, the whole pre-WP `snapper.rs`: 4 of the new tests fail. The
    info→list switch records the false pair
    `["snapshot-delete 11 2026-10-25T02:30:00+01:00", "snapshot 11 …+01:00"]`
    (the old known limit, reproduced). It also lists 11 at +01:00, which
    confirms the chrono finding.
  - M1, the rule removed (`&& !fold_twins(…)` dropped): **the switch test
    records a false pair** `["snapshot-delete 11 2026-10-25T02:30:00+01:00",
    "snapshot 11 2026-10-25T02:30:00+01:00"]`. It is killed by
    `…from_the_list_to_the_info_files` and `…older_cursor…`. The older-cursor
    test first let M1 survive: it faked only the cursor, and the ledger
    dedupe dropped the pair. It now fakes the ledger line too (pitfall
    noted).
  - M2, no info file in the list path: killed by
    `…dates_a_listed_snapshot…` (11 at +02:00).
  - M3, no cursor date in the list path: killed by
    `…from_the_list_to_the_info_files` (the cursor flips back to +02:00).
  - M4, no number order: killed by `…dates_a_listed_snapshot…` (12 at
    +02:00).
  - M5, chrono's order kept (no sort): killed by three tests.
  - M6, a gap time read as UTC instead of no instant: killed by
    `…dst_gap`.

**Open questions**

- `collectors::Tz::localize` (`collectors/mod.rs`, not in this WP) says
  "the earlier instant of an ambiguous time", but it returns the later
  one (see the finding above). pacman still uses it for log lines without
  an offset. Either the doc comment or the code should change; I left
  both alone, because the file is outside this WP.

**Touched outside the WP's files**

- `memory/pitfalls.md` (append, expected), `CHANGELOG.md` (expected).
  Nothing else.

**just check**

Run once, after waiting until no other worktree ran the plugin harness
(a bounded wait on the harness scripts, harness quickshell and `just
check`; WP-084's check finished first): **exit 0**, `check: ok`, first
try, no re-run. Plugin harness: service-states 275, panel-view 743,
overlay-view 319, bar-view 143 passed, 0 failed; qmllint ok (29 files);
plugin-validate ok; validate-fixtures ok; docs-check ok; packaging,
install.test (209) and real-home-guard (11) ok.

---

## Round 2 (review: SEND BACK, F1 + F2 + the `Tz::localize` comment)

Commits:
- `3e6cc48` engine: snapper keeps only the readings that show the listed time (F1)
- `1eb359a` engine: snapper dedupe knows both instants of a time in the repeated hour (F2)
- `a177530` engine: Tz::localize says which instant of an ambiguous time it takes
- `998e386` docs: SPEC-ENGINE §4 rule holds at the ends of the hour, dedupe
- `16f4f0e` memory: chrono's closed fold and gap ends
- `cdadb59` docs: CHANGELOG snapper line names the dedupe
- this section

**What changed**

- **F1 (blocking, fixed).** `readings` now goes through a new `instants(tz,
  naive)`. That function keeps only the instants whose wall-clock time in
  `tz` is the listed time (`wall`), then sorts them. A probe confirmed
  the review: 03:00:00 on 2026-10-25 came back from chrono as
  `Ambiguous(03:00+01:00, 03:00+02:00)`, the same with the POSIX rule
  and with `Europe/Berlin`. It is now one instant, 02:00 UTC.
  **Widened by one case:** the same probe showed the start of the skipped
  hour is closed too. chrono gives 2027-03-28 02:00:00 as
  `Single(02:00+01:00)`, which is 01:00 UTC, i.e. 03:00 CEST on the wall.
  The wall-time check applies to every reading, so that time now names
  no instant, like the rest of the skipped hour (as the SPEC already
  said). snapper never lists such a time; without the check, the SPEC
  sentence would have been false at 02:00:00. `fold_twins(a, b)` became
  `fold_twin(tz, t)` (the other instant of `t`'s wall time, through
  `instants`), so the twin rule uses the same filter.
- **F2 (fixed).** `dedupe` reads the ledger from `first − 1 h` to
  `last + 1 h`. A `snapshot`, and the `snapshot-delete` of a reused
  number (the `replaced` branch), counts as recorded when the ledger holds
  its time or its fold twin (`recorded`). Both paths are covered:
  - **failed cursor save**: the list records 11 at 02:30+02:00, the
    cursor stays as before, then the info files give 02:30+01:00;
  - **lost state directory**: the baseline falls back to the logbook's
    `created`, which lies before the snapshot. The test removes
    `~/.local/state/seldon`, once before a list capture and once before
    an info-file capture.
  The round-1 "Not done" item (crash plus path switch plus fold) is
  therefore done.
- **`Tz::localize` comment** (`collectors/mod.rs`, comment only, no code
  change). It now says that an ambiguous time gives chrono's `earliest()`,
  the reading with the smaller offset, which is the later instant. pacman
  keeps that behaviour; snapper resolves such times itself. The round-1
  open question is closed with this.
- **SPEC-ENGINE §4**: the rule names the instants that show the listed
  time on the wall clock. That is two from 02:00:00 to 02:59:59 on the
  night summer time ends (03:00:00 is one), none from 02:00:00 to
  02:59:59 when it begins, else one. A new sentence on the dedupe names
  both paths. CHANGELOG line extended by the dedupe; pitfalls: one bullet
  on the closed ends.

**Tests** (`tests/collectors.rs`, all through the binary with
`TZ=CET-1CEST,M3.5.0,M10.5.0/3`):
- new `snapper_keeps_one_date_at_the_ends_of_the_repeated_hour`: 10
  at 01:30, 11 at 02:00:00 (CET, listed as the earlier instant, 02:00+02:00)
  and 12 at 03:00:00 (one instant, 03:00+01:00). Then the info files with
  11 → 01:00:00 and 12 → 02:00:00 UTC: 0 events, and the cursor holds
  +01:00 for both. This is the reviewer's probe plus the 02:00:00 boundary.
- `snapper_remembers_a_listed_time_in_the_dst_gap` extended: 22 at
  2027-03-28 02:00:00 gives no event and no date. The info files then
  give 03:45+02:00 without an event.
- new `snapper_dedupes_the_other_instant_after_a_lost_cursor`: (a) a
  failed cursor save, with 11 alone new to the next capture, so the
  ledger's 11 lies an hour outside the event range and only the widening
  finds it; (b) the state directory lost before a list capture and again
  before an info-file capture: 0 events each time, 2 snapper lines in the
  ledger; (c) a reused number: the deletion and the new snapshot came from
  the list, the cursor save failed, then the info files: 0 events.
- new `snapper_keeps_one_date_for_a_snapshot_known_without_an_event`: the
  state is lost and `created` lies after the snapshots, so they become
  history without events. The switch to the info files then gives 0
  events, and the cursor moves to +01:00. This test was needed because
  M1 (the twin rule in `diff` removed) **survived** once the dedupe knew
  the twins: the dedupe drops the false pair whenever the ledger holds
  the snapshot. Only a snapshot the ledger does not know shows that the
  `diff` rule is still needed.
- `Zoned` sets the logbook's `created` to 2026-10-20 after `init`, so
  that the lost-state baseline does not depend on today's date. `capture`
  returns only snapper events, because a lost state also writes a
  `seldon` `state-reset` note (WP-081).

**Mutants** (round 2; each restored and touched; run on the snapper tests
of `tests/collectors.rs`):
- R1, the wall-time check removed (F1): killed by `…ends_of_the_repeated_hour`
  and `…dst_gap`.
- R2, the wall-time check only for two readings (the gap start): killed by
  `…dst_gap`.
- R3, the dedupe range not widened: killed by `…after_a_lost_cursor`.
- R4, the dedupe without the fold twin: killed by `…after_a_lost_cursor`.
- R5, the `replaced` deletion checked exactly: killed by
  `…after_a_lost_cursor` (part c).
- Round 1's M1 to M6, re-run on the new code:
  - M1 (rule removed): killed by `…known_without_an_event`. Before that
    test existed it SURVIVED; see above.
  - M2, M3 and M4: killed as in round 1.
  - M5 (no sort): killed by five tests.
  - M6 (a gap time read as UTC): **survives, as an equivalent mutant.**
    The wall-time check drops any instant whose wall time is not the
    listed one, and a gap time read as UTC is such an instant. R1 and R2
    cover the gap behaviour instead.
- The round-2 split was checked on its own: the F1-only state (`3e6cc48`)
  passed clippy and the snapper tests (10) before F2 went on top.

**Verified by (round 2)**

- `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings`:
  clean.
- Suites: collectors 27, idempotency 20, doctor 30, index 25 (1
  ignored, as before), lib `collectors::` 17; 0 failures.
- `bash scripts/validate-fixtures.sh`: ok, exit 0.
- No second `just check`, as instructed (the orchestrator's gate runs
  it).

**Touched outside the WP's files (round 2)**

- `engine/src/collectors/mod.rs`: the `Tz::localize` doc comment only,
  as decided. `memory/pitfalls.md` and `CHANGELOG.md` (expected).

**Note for the record:** pacman still goes through `Tz::localize`
unchanged, which includes chrono's closed ends: a pacman line without an
offset at 03:00:00 on the night summer time ends takes the right instant.
At 02:00:00 when it begins, it takes 02:00+01:00, but pacman cannot write
that time. No change, as decided.
