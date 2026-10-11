# WP-200 — handover (the engine keeps index.json under its budget)

Branch `wp/200-index-budget` in `wt/WP-200`, from `next` 4a7a2376 (the
v0.2.0 release candidate). Nothing merged; the orchestrator merges after
the v0.2.0 tag. ADR-0053 stays **proposed**: the operator accepts it
after this round.

## Round 2 (review 1 SEND BACK, Fable stage 2)

| item | what changed | commit |
|---|---|---|
| F1, Fable 1 | `Rows::Events` never takes the event a listed drift row names (`eventId`), on rung 2 and on rung 15. The set is rebuilt on every `trim` call, and the floor counts the whole list, pinned rows included (rustdoc says so). `Rows::Attention` and `Rows::Crises` take the row's event with it (its bytes are in `also`), and a crisis row also takes its mark. Group members fall under the events rungs, as under rule 4. | 4dead2f6 |
| Fable 2 | `Rows::System` (rung 23) now comes before `Rows::Crises` (rung 24). The warning lists "system details" before "crisis rows". | 4dead2f6 |
| Fable 6 | The warning says "without N events" instead of "without the oldest …". | 4dead2f6 |
| F3, Fable 4 | Tests for `plan new` (the rebuild after a writing command), `hook session-stop` and `watch` on the fixture ×200. | 605e5cd5, 8d471f65 |
| F4, Fable 5 | `status` prints "N in verification" from the build before the cut (`Rebuilt::verification`). | 03a9c224 |
| F2, F5, Fable 7, 8 | ADR-0053 §2, §3, §4 and §5, context 4 and the consequences; CONTRACT.md rule 5; SPEC-ENGINE §6; CHANGELOG. | 25343eab |
| Fable 9 | This handover. | (this commit) |

**F5 premise corrected.** `watch` does not cut silently. `index::fit`
adds its warning to `built.warnings`, and `watch` prints those in the
`warnings` of its `rebuilt` line and in its text log, as it does with
every build warning. That was already true in round 1. ADR-0053 §3 said
"`watch` logs no build warning, as before", which was my mistake, and
stage 1 and Fable both relied on it. The new watch test asserts the
warning in the `rebuilt` line. SPEC-ENGINE §6 and ADR-0053 §3 now name
that channel instead of "silently".

**The backstop's output (Fable, open question 4).** `index` and
`status` exit 2 and print only the write's error, on one line. `fit`'s
warning is in the build, but the error ends the command before it is
printed. The rebuild after a writing command prints `index.json not
rebuilt: <the write's error>`. The session hook prints both lines: the
warning ("… still N bytes with every list cut; not written") and
`seldon hook: index: …`. I accept both lines for the hook. ADR-0053 §4
says so.

## Done (whole WP)

1. **ADR-0053** (`decisions/ADR-0053-index-size-budget-cut.md`, a
   DECISIONS.md row, status proposed). Over the budget, the engine cuts
   the index it writes, rung by rung. Each list is cut from its oldest
   end, and only as many rows go as needed. There are 24 rungs: a
   screenful of each list first, then history, then the present, then
   `system`'s optional parts, and crisis rows last.
   - An event that a listed drift row names is cut only with its row.
   - `summary`, `series.risk` and `system.areas[].cases` keep the counts
     of the whole index.
   - `index::write` never writes 1 000 000 bytes or more (exit 2; the old
     file stays).
   - No schema change and no new field. `contractVersion` and
     `contractReadableFrom` stay 2.
   - §5 is the per-field table that ADR-0051 §3 asks for, now with every
     count the 0.2.0 plugin takes from a list's length, and the triage
     caveat.
2. **Engine.** `engine/src/index/budget.rs` (`fit`, the ladder, `size`).
   `index::fit(&mut Built)` runs right before every write of
   `index.json`:
   - `index` and `status`: after the git state, before `--check`
     validates;
   - the rebuild after writing commands;
   - `watch`;
   - the session hook.

   The views, the dossier, `drift`, `triage`, `rebuild` and `status`'s
   terminal summary keep the whole index. `events` in the `--json` of
   `index` and `status` counts the rows written. Under the budget `fit`
   changes nothing (one serialisation, the same size check 0.2.0 made).
3. **Warning.** It appears in the `warnings` of `index` and `status`, in
   watch's `rebuilt` line, and on stderr elsewhere: `index.json would be N
   bytes, over its budget of 1000000 (CONTRACT.md rule 5); the largest
   section is `cases` (N bytes); written at N bytes without 40 completed
   cases, 216 events; the summary still counts every case, change and
   crisis, the logbook keeps every row`.
4. **Docs.** CONTRACT.md rules 4 and 5; SPEC-ENGINE §6; SPEC-PLUGIN §3
   (unchanged in round 2); CHANGELOG `[Unreleased]`.

## Measured (release profile, fixture logbook scaled by `tests/common/scale.rs`, round 2 code)

| logbook | whole index | written | cut |
|---|---|---|---|
| ×150 | 880 991 B | 880 991 B | nothing |
| ×200 | 1 077 997 B | 999 825 B | 40 completed cases, 216 events |
| ×300 | 1 487 201 B | 999 603 B | 40 completed cases, 400 events, 651 queued cases |
| fixture, its mesa group made a crisis, 20 crises of 2026-09-30, 300 newer notes in a case, 2 000 queued cases | 1 337 639 B | 999 866 B | 303 events, 460 queued cases; all 26 drift rows and their events stay |
| synthetic: 3 000 active cases, long titles | 3 414 993 B | 999 826 B | every rung before 21, then 2 125 active cases; every crisis row and its event stay |
| synthetic: 250 crises, subjects of 512 characters each escaped to 6 bytes | 2 100 147 B | 996 643 B | every rung before 24 (3 system details included), then 95 crisis rows with their events and marks; `summary.crisis` counts all |

`fit` takes 0.9 ms at ×150 (no cut), 3.6 ms at ×200 and 10.1 ms at
×300. Pinning costs no extra serialisation.

## How it was verified

- **fixture**: `engine/tests/index.rs`, in `just test`.
  - `under_its_budget_the_index_is_not_cut`.
  - `an_index_over_its_budget_is_cut_oldest_first_and_keeps_its_counts`
    (3 000 active). Asserts `cut.events.len()` equals the crisis rows that
    have an event, the drift↔event pairing in both directions, and
    `fit(cut) == None`.
  - `crisis_rows_go_last_and_stay_counted`. Asserts every optional part
    of `system` is gone before any crisis row, "system details" before
    "crisis rows" in the warning, and the pairing.
  - `the_cut_keeps_the_events_of_listed_drift_rows` (new; stage 1's
    scenario). Asserts every drift row stays, the pairing in both
    directions, and that the 0.2.0 crisis rows (events that are a crisis
    row's own event) equal `summary.crisis`.
  - `an_index_still_over_its_budget_is_not_written`.
  - `status_writes_a_large_logbook_under_its_budget` (×200).
  - `a_writing_command_writes_a_large_logbook_under_its_budget` (new,
    `plan new` on ×200).
  - `the_session_hook_writes_a_large_logbook_under_its_budget` (new,
    `hook session-stop` on ×200).
  - `status_counts_from_the_whole_index` (new: ×200 plus 2 500 long active
    cases; the written verification list is cut, and the terminal prints
    the whole count).
  - `long_texts_keep_the_index_under_its_size_budget`.
- **fixture**: `engine/tests/watch.rs`
  `watch_writes_a_large_logbook_under_its_budget` (new; `--features
  watch`, in `just check-watch`). Asserts the file is under the budget,
  it validates, the `rebuilt` line has the same summary and events as
  the file, and it carries the warning.
- **fixture, mutations** (each run by hand, then the source restored):
  - Events rung not pinned: 3 tests fail (3 000 active, 250 crises,
    queued-heavy).
  - `fit` removed from `try_rebuild`:
    `a_writing_command_…` fails.
  - `fit` removed from `session_stop`:
    `the_session_hook_…` fails. A first version survived, because the
    write was refused and the file of the capture's own rebuild stayed;
    the test now also asserts that no write was refused.
  - `fit` removed from watch's `rebuild`: the watch test fails.
  - `status` counting verification from the written list:
    `status_counts_from_the_whole_index` fails.
- **fixture**: the released 0.2.0 `plugin/Model.js` (`git show
  4a7a2376:plugin/Model.js`, node, stage 1's `probe.js`) on the written
  files (Fable instruction 3):
  - Queued-heavy cut index (999 866 B, 998 263 characters, `parseIndex`
    ok): `counts().crisis` 23 = `summary.crisis` 23, `changelogChips`
    crisis 23, open 26, attention 3, NEEDS YOU 23. It is the same on the
    whole index (crisis 23, open 26, NEEDS YOU 23); round 1's cut showed
    crisis 1 and NEEDS YOU 1.
  - The ×200 and ×300 cut indexes: `parseIndex` ok, the pill counts all
    (crisis 400 and 600). The chips are 0 in the whole index too: the
    pre-existing gap of follow-up (a).

  Scripts and files stayed in the private gates folder.
- **headless**: `SELDON_FULL_CHECK=1 just check` on the dev host at
  25343eab (the code of this round; afterwards only this handover and one
  sentence of ADR-0053 changed): `check: ok`. Quickshell harnesses:
  service-states 366/0, desk-view 2091/0, bar-view 195/0, ipc-restart
  43/0; terminal-scripts 73/0; qmllint ok (52 files); model.test.js 217;
  plaintext ok; docs-check ok (514 links); the runtime dir was 2 % full
  before, with 328 by-id entries before and after. TMPDIR, JUST_TEMPDIR
  and the cargo target were all under the private gates folder.
- **test host**: shellcheck through the orchestrator kit's wrapper
  (E80), inside the same `just check` run.
- **fixture** (round 1, bench profile, dev host): `just check-rss` ok,
  peak 11 720 kB. Index budgets of `just check-perf`: ×10 13.4 ms, ×150
  80.7 ms, `status` 58.5 ms. **`just check-perf` as a whole fails** on
  three timings that do not touch the index:
  - `hook, recorded (tmpfs)` 12.3 ms and 21.1 ms (budget 5 ms);
  - the recent-config scan delta 10.9 ms (budget 10 ms).

  It fails on the base 4a7a2376 too, with the same numbers. TMPDIR is on
  disk for this WP. Not rerun on tmpfs, and not rerun in round 2.
- **not run**: a live desktop with a real logbook over 1 MB; CI.

## Not done (follow-up WPs, Fable stage 2)

- **(a)** The build lists the event of every listed drift row, past the
  500 newest (≤ 200 extra events). Today a crisis row whose event is
  older than the 500 newest has no Changelog row in 0.2.0, before any
  cut (×150: 0 of 200). ADR-0053 context 4.
- **(b)** `fit` keeps the drift rows that an unapplied triage proposal
  names. From rung 4 on, such an item shows "No longer open"; Apply
  still runs, and the engine decides. ADR-0053 §5.
- **(c)** Plugin only: Work's column headers read "50 / 450" from
  `summary.queuedCases` and `summary.activeCases`.
- No field telling the desk "N more rows in the logbook": that would be
  a contract-3 field (WP-184).
- The reference derive (`scripts/validate-fixtures.py`) does not cut.
- No plugin change and no text cutting.

## Open questions

1. **Order of rungs 2 and 3** (events before queued cases): stage 1
   and Fable say keep it, now that the events of drift rows are pinned.
   Kept.
2. **Counts the 0.2.0 plugin takes from list lengths.** None is a crisis
   or open-drift count; each is a row in ADR-0053 §5:
   - the source chips;
   - Work's column headers;
   - `wipStatus` (rung 21);
   - the verification count (rung 20);
   - the proposed-decision count (rung 22) and `decisionSummary`'s total
     (from rung 5);
   - Memory's counts (rungs 6, 11, 12);
   - the Yesterday toggle (rung 7);
   - `anyCase`/`firstRun` (only once rung 21 ran dry).

   The crisis, open and attention chips count listed drift rows, and they
   keep their whole-index values.
3. **Crisis rows before refusing** (rung 24): agreed by stage 1 and
   Fable. Each crisis row takes its event and mark with it, and `system`
   goes first.
4. **Backstop exit 2**: accepted by both. The output is described above.
