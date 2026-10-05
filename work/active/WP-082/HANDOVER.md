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
