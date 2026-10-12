# WP-203 — handover (every listed drift item keeps its event in the index)

Branch `wp/203-drift-events` in `wt/WP-203`, from `next` 1e872c28, with
`next` 50bbd290 merged in (round 3). Nothing merged into `next`. The
operator accepted ADR-0054 (2026-10-12, E84); the file still says
**proposed**, because the orchestrator sets the status, DECISIONS.md and
STATUS.md at merge. No `contractVersion` bump, no schema change.

## Round 3 (merge of `next` 50bbd290)

- Merge commit 1d2c8ad2. Two conflicts:
  - `docs/user/de/03-daily-use.md`, the source line: both sides changed
    the English page (WP-201's dadf6b69, WP-203's f3a9ff8b), and the
    German page now holds both translations. Neither commit matches the
    merged English page, so cf3c5339 points the line at the merge
    1d2c8ad2, whose English page it matches (`git diff --quiet 1d2c8ad2
    -- docs/user/en/03-daily-use.md`); docs-check reports no warning.
  - `engine/tests/index.rs`, `status_writes_a_large_logbook_under_its_budget`:
    WP-207's approach (the whole warning computed from the two files, the
    boundary check) with WP-203's rule. The kept events are the newest of
    the non-pinned rest (`kept == all[..kept.len()]`, both without the
    drift rows' events); every drift row's event is kept; the boundary
    check puts back `all[kept.len()]`, the first non-pinned event the cut
    did not keep. The cut count is `whole − written`, no constant.
- **fixture** (dev host, private gates folder): the test passes with two
  TMPDIR lengths, `gates/tmp-dev-203` (whole index 1 097 997 B, written
  999 969 B) and a 70-character longer one (1 098 068 B, written
  999 803 B).
- ADR-0054 §3 and its consequences still name the 286 events cut on the
  ×200 logbook. That was the measurement on this host; since WP-207 the
  test counts it from the two files. Not edited (the text the operator
  accepted).
- **headless** and **fixture** (dev host, private gates folder, at
  cf3c5339, under `flock /tmp/seldon-check.lock`, twice: logs
  `gates/check-dev-203-r3.log` and `-r3b.log`): `SELDON_FULL_CHECK=1 just
  check` → **exit 1 both times**, on one check only. In both runs:
  - cargo 2 903 passed, 0 failed, 20 ignored over 112 test binaries;
    fmt and clippy (with and without `watch`) clean;
  - validate-fixtures ok (154 instances, 14 variants); docs-check ok
    (514 links, 14 translated pages); qmllint ok (53 files);
    model.test.js 221; real-home-guard.test 40/0; service-states 366/0;
  - desk-view 2336 passed, 1 failed. The one failure is the real-home
    guard: `real paths changed during the run
    (~/.local/state/seldon/recent-config.json changed …)`. The real
    `index.json`, `cursors.json` and `recent-config.json` got new
    modification times at the same sizes (09:47:29 → 10:02:29 in the
    first run, 10:32:30 → 10:47:29 in the second). That is the
    operator's live released 0.2.0 engine on its 15-minute timer (the
    host was upgraded to 0.2.0 at 08:41–08:50). The guard allows the
    live engine's `index.json`, `lock`, `cursors.json` and
    `manifest.json` (`tests/plugin/real-home-guard.sh:44`), but not
    `recent-config.json`, which 0.2.0 writes (`collectors/recent.rs`).
    desk-view runs about 15 minutes, so every run spans one tick. The
    tests' own HOME was private; the quickshell runtime held 333 by-id
    entries before and after. Not WP-203's code; the gate is not green.
    I did not change the guard (not this WP's file); see open question 5.
- Merge of `next` 71f070b8 (WP-212, the guard fix): b26b56bd, no
  conflicts; under `plugin/` and `tests/plugin/` it changes only
  `real-home-guard.sh` and its test.
- **headless** and **fixture** (dev host, private gates folder, at
  b26b56bd, under the lock; log `gates/check-dev-203-r3c.log`): exit 1.
  - cargo 2 903 / 0 / 20 over 112 binaries; validate-fixtures,
    docs-check, qmllint (53 files) ok; model.test.js 221;
    real-home-guard.test 71/0; service-states 366/0.
  - desk-view: the guard now passes (`changed by the operator's live
    engine (not a leak)`; 333 by-id entries before and after).
  - desk-view 2323 passed, 14 failed, all in the scenario `drift-only`:
    at step #1 no row was selected (`sectionView.selected` empty), and the
    rest followed from that (`HARNESS nothing to click: Link to case…`,
    `wait timed out: sectionView.detail.form.isOpen=false`). The host's
    load was 8.91. `drift-only` passed in the round-1, round-2, r3 and
    r3b runs on the same plugin and scenario code. Most likely a timing
    failure, but no re-run has confirmed it yet.
- **Paused (wind-down, 2026-10-12 12:46).** The re-run (`-r3d.log`)
  waited behind three other checks for the lock and had not started; I
  stopped it. The next step: at b26b56bd (or later), run
  `SELDON_FULL_CHECK=1 just check` under `flock /tmp/seldon-check.lock`.
  If it is green, WP-203 is ready. If `drift-only` fails again, look at
  that scenario under load before any other step. Nothing else is open.

## Round 2 (stage 1 and stage 2: APPROVE WITH FIXES)

Fable's instructions 1–6 (`WP-203-stage2-fable.md`), each with its
acceptance:

1. **The reference derive caps drift like `build::cap_drift`** (F1,
   blocking). `scripts/validate-fixtures.py`: the summary counts every
   open item first; then `drift` keeps every crisis in list order up to
   200 and the others in list order into `200 - crises`, newest first.
   The derive's `drift` output is the capped list, and so is `named`.
   `rest` takes `max(0, 500 - pinned)`. New test
   `the_reference_derive_lists_the_events_the_engine_does`: fixture + 600
   in-case notes (500 events, 6 open items) and fixture + 520 caseless
   `theme-set` events (526 open items, 200 drift rows, 500 events, every
   row's event listed); 0 diffs on `events`, `drift` and `summary`
   against `seldon index`. The in-case notes are also added to
   C-2026-003's frontmatter `events`, because the derive checks that list
   against the ledger (stage 1's probe ignored the derive's exit code).
   Mutants, each alone, source restored:
   - M6 (derive back to `folded[:500]`) fails the new test;
   - the derive without the drift cap fails the new test;
   - the derive without the clamp at 0 passes. It is an equivalent
     mutant: with the cap at most 200 events are pinned, so
     `500 - pinned` never goes negative. The clamp stays as a guard.

   `scripts/validate-fixtures.sh` ok, the `drift-capped` variant included
   (it only patches `summary`, which the cap leaves whole).
2. **The 0.2.0 limitation as ADR-0054's own fact** (F2). ADR-0054 §3, row
   `transactionDetail`, has Fable's wording. "ADR-0053's cut had the same
   case" is gone. The CHANGELOG sentence is Fable's, with WP-208.
   Consequences name WP-208 (the plugin follow-up the orchestrator
   queued). They also add the known limit: proposal rows past rule 4's
   200 drift cap are not listed; `fit` keeps only listed rows. The three
   texts (CHANGELOG, ADR-0054 §3, open question 4 below) say the same.
3. **The plugin follow-up** is drafted in open question 4 (WP-208 points
   there). Nothing under `plugin/` changed.
4. **Wording.** CONTRACT.md rule 4 and SPEC-ENGINE §6: "at most 500
   events". User docs en and de: "the change behind every open item the
   index lists". `en/05-cli-reference.md` is wrapped, and SPEC-ENGINE §6's
   long line too. The de files name the English commit f3a9ff8b.
5. **ADR-0054 form.** The status stays `proposed` and the title form "ADR-0053
   amended: …". The header now names ADR-0020's event count as amended,
   with a link. `git diff 1e872c28 --
   decisions/ADR-0053-index-size-budget-cut.md` is empty.
6. **This handover.** The derive's > 500 branch moved from *not run* to
   *fixture*. The deviation below is decided.

## Deviation from the WP's wording (decided)

Stage 1 and stage 2 accepted 500 with pins: no bump question for the
operator.

The WP asks for "the newest 500, plus the event of every listed drift
item" (up to 700 events). `schema/index.schema.json` caps `events` at
`maxItems: 500`. Lifting the cap is a schema change after the 0.2.0 tag:
AGENTS.md §3 then needs a bump, and the released engine's `index --check`
would refuse such an index. The 0.2.0 plugin itself would read 700 rows
(it has no such cap; `INDEX_EVENTS_MAX` only feeds a footer and the
transaction `partial` note).

So the build keeps the schema: **500 events, newest first, the event of
every listed drift item, however old, and the newest of the rest.** With
200 listed items at least the newest 300 others stay. This meets the WP's
goal and acceptance (every listed crisis row has its event and a 0.2.0
Changelog row) without a schema change. If you want the "+200" reading
instead, it is the schema's `maxItems` 700, a bump question for the
operator, and a two-line change in `build::listed_events` and the
reference derive. I did not take it.

## What was done

1. **(a) The events of the index** (`engine/src/index/build.rs`,
   `listed_events`). `index.events` holds every listed drift item's event
   (`eventId`, a group's leader) and the newest of the rest, at most 500.
   When every item's event is among the newest 500 nothing changes (the
   golden fixtures are unchanged: 83 events). A group's other members are
   not added; `drift show` has them.
2. **(b) The rows of an unapplied proposal** (`engine/src/index/budget.rs`).
   `triage::read` now reads the items' `eventId` into
   `model::Triage::event_ids`, which is `#[serde(skip)]`: no new index
   field. The drift rungs that cut open changes (4 and 17) leave those an
   unapplied proposal (`applied` null) names. A new rung `Rows::Triaged`
   takes them after `system`'s optional parts and before the crisis rows,
   each with its event. An applied proposal holds nothing. The proposal
   holds at most 200 items (schema), so the extra rung is bounded.
3. **Reference derive** (`scripts/validate-fixtures.py`): the same rule;
   since round 2 with `drift` capped as the engine caps it.
4. **Docs.** CONTRACT.md rules 4 and 5, SPEC-ENGINE §6, user docs en and de
   (the graph's 500 events; source lines updated), CHANGELOG
   `[Unreleased]` (also: ×150 now 897 KB, not 881 KB), DECISIONS.md row,
   `decisions/ADR-0054-adr-0053-amendment-drift-events-and-proposal-rows.md`
   (proposed; its §3 is the ADR-0051 §3 table for a 0.2.0 reader). ADR-0053
   is unedited; its DECISIONS row should say "amended by ADR-0054" once
   ADR-0054 is accepted (orchestrator's edit, as for ADR-0038/0048).
5. **Tests** (`engine/tests/index.rs`):
   - `a_drift_row_older_than_the_newest_500_events_keeps_its_event` (the
     WP's acceptance): 600 newer notes push every fixture drift event past
     the newest 500; the index lists 500 events, every drift row's event
     among them, the newest of the rest first, newest-first order, the
     0.2.0 crisis-row count equals `summary.crisis`, valid, not cut.
   - `the_cut_keeps_the_rows_an_unapplied_proposal_names`: 3 000 long
     active cases; the two named open changes stay, every other goes;
     applied, none stays.
   - `the_rows_a_proposal_names_go_right_before_the_crisis_rows`: 190
     long-subject crises; the warning orders system details < proposed
     open changes < crisis rows.
   - `the_triage_names_its_rows_for_the_cut_only`: `derive_at` with the
     fixture proposal fills `event_ids` from the file; the written
     `triage` keeps its six keys.
   - Changed: `long_texts_keep_the_index_under_its_size_budget` (the
     fixture crises' events are listed, so two fewer notes),
     `status_writes_a_large_logbook_under_its_budget` (×200 now cuts 286
     events, not 216; the cut list is the whole list without the oldest
     of the rest, and every drift row's event stays).

## Measured (release profile, `tests/common/scale.rs`, `derive_at` + `fit`)

Same probe on the base 1e872c28 and on this branch (throwaway test, not
committed; outputs in the private gates folder `wp-203/`):

| logbook | whole index before → after | written before → after | events written | cut before → after |
|---|---|---|---|---|
| ×150 | 880 974 → 896 715 B | the same, no cut | 500 → 500 | nothing |
| ×200 | 1 077 980 → 1 097 971 B | 999 808 → 999 943 B | 284 → 214 | 40 completed, 216 → 286 events |
| ×300 | 1 487 184 → 1 505 775 B | 999 586 → 999 537 B | 100 → 200 | 40 completed, 400 → 300 events, 651 → 708 queued |

×150 grows by 15 741 bytes (200 crisis events replace 200 notes), well
under the budget. `fit` at ×150 0.46 ms (no cut), ×200 3.9 ms, ×300
19.6 ms (base 0.38, 3.3 and 10.5 ms; one probe run each, cause of the
×300 difference not analysed; `fit` runs once per write).

**The released 0.2.0 plugin** (`git show v0.2.0:plugin/Model.js` under
node: `parseIndex`, `deskChangelog`, `changelogChips`, `deskToday`,
`changelogRow`, `firstCrisis`):

| index | crisis rows with a Changelog row | crisis chip | NEEDS YOU | pill crisis | first crisis in the Changelog |
|---|---|---|---|---|---|
| ×150 base | 0 / 200 | 0 | 0 | 300 | no |
| ×150 WP-203 | 200 / 200 | 200 | 200 | 300 | yes |
| ×200 cut, base | 0 / 200 | 0 | 0 | 400 | no |
| ×200 cut, WP-203 | 200 / 200 | 200 | 200 | 400 | yes |
| ×300 cut, base | 0 / 200 | 0 | 0 | 600 | no |
| ×300 cut, WP-203 | 200 / 200 | 200 | 200 | 600 | yes |

Every index parsed (`parseIndex` ok). The pill counts every crisis; rule
4's 200-row drift cap lists 200 of them. (b) on the 0.2.0 side follows
from `triageView`: an item is `open` when `changelogRow` finds a drift
row for its `eventId`, which the kept row and its pinned event give; not
run separately.

## How it was verified

- **fixture** (round 1): `cargo test --test index` 52 passed, 0 failed,
  2 ignored; clippy `-D warnings` and fmt clean;
  `scripts/validate-fixtures.sh` ok; `scripts/docs-check.sh` ok.
- **fixture** (round 2): `the_reference_derive_lists_the_events_the_engine_does`
  (the derive's > 500 and > 200 branches against the engine) and its
  three mutants (round 2, item 1); `scripts/validate-fixtures.sh` ok;
  `scripts/docs-check.sh` ok.
- **headless** and **fixture** (round 2; dev host, private gates folder,
  at f2732e0b): `SELDON_FULL_CHECK=1 just check` → `check: ok`, exit 0
  (log `gates/check-dev-203-r2.log`). In it:
  - cargo 2 903 passed, 0 failed, 20 ignored over 112 test binaries;
    fmt and clippy clean;
  - validate-fixtures ok (154 instances, 14 variants); docs-check ok;
    qmllint ok (52 files); model.test.js 218;
  - in a private headless Quickshell: service-states 366/0, desk-view
    2242/0 (round 1: 2243/0), bar-view 195/0, ipc-restart 43/0, and their mutant runs
    clean;
  - 328 by-id entries before and after each suite.
  - desk-view 2242 against 2243: `graph-big #3: .view.graph.ticksOver <= 5`
    went over its time budget twice on a loaded host (load 12.79, log line
    `WARN graph-big: #3 … counted only with SELDON_PERF_STRICT=1`), so
    `graph_time_ok` left it out of the count. It is a timing check, not a
    WP-203 change; nothing under `plugin/` changed.

  desk-view's guard printed `ok   desk-view: the real
  ~/.local/state/seldon changed by the operator's live engine (not a
  leak)`: the released engine of the live session wrote its own index
  while the suite ran, and the harness judged it not a leak. The other
  suites report the real state and config untouched.
- **fixture**: two mutants, each run alone, with the source restored
  afterwards:
  - holding no proposal rows fails
    `the_cut_keeps_the_rows_an_unapplied_proposal_names` and
    `the_rows_a_proposal_names_go_right_before_the_crisis_rows`;
  - listing only the newest 500 (the old `take(MAX_EVENTS)`) fails
    `a_drift_row_older_than_the_newest_500_events_keeps_its_event`,
    `long_texts_keep_the_index_under_its_size_budget` and
    `status_writes_a_large_logbook_under_its_budget`.
- **fixture** (release, dev host, private gates folder): the measurement
  and the 0.2.0 `Model.js` probe above.
- **headless** and **fixture** (round 1; dev host, private gates folder,
  at 93a9c8a9): `SELDON_FULL_CHECK=1 just check` → `check: ok`, exit 0
  (log `gates/check-dev-203.log`). In it: cargo 2 901 passed, 0
  failed, 20 ignored over 112 test binaries; fmt and clippy `-D
  warnings` (with and without `watch`) clean; validate-fixtures ok (154
  instances, 14 variants); docs-check ok (514 links); qmllint ok (52
  files); model.test.js 218; service-states 366/0, desk-view 2243/0,
  bar-view 195/0 and ipc-restart 43/0 in a private headless Quickshell;
  the runtime dir was 2 % full, with 328 by-id entries before and after;
  the real `~/.local/state/seldon` and `~/.config/seldon` were untouched.
  TMPDIR, JUST_TEMPDIR and the cargo target were all under the private
  gates folder.
- **test host**: shellcheck through the orchestrator kit's wrapper and
  the deploy's plugin validation, inside the same `just check` run, in
  both rounds.
- **not run**: CI; a live desktop.

## Open questions

1. Answered (stage 2): 500 with pinned events. No bump question.
2. Answered (stage 2): the proposal's rows stay on rung 24, after
   `system`'s optional parts.
3. Known limit (ADR-0054 consequences): proposal rows past rule 4's
   200-row drift cap are not listed by the build (the cap is crises
   first, then newest); `fit` keeps only listed rows.
4. **Plugin follow-up, WP-208 (draft; no contract change).** Base 0.2.0
   marked the boundary transaction partial because the newest 500 were
   contiguous. With pinned older leaders after the rest, it reads
   complete, and a pinned leader's transaction reads "1 package" without
   the note (the oldest pinned one excepted). This happens below 1 MB too.
   It is not an index field and not a crisis or open-drift count
   (ADR-0051 §1).

   Rule for the new plugin's `transactionIndex`: a transaction is
   `partial` when any of these holds (they replace the
   `all.length >= 500` test):
   - (a) it holds the oldest event that is not a listed drift item's
     `eventId` (the rest's boundary);
   - (b) its leader is older than that event (the pinned tail);
   - (c) it lists fewer package rows than its drift item's `members`.

   `transactionRows`' text "it keeps the newest 500 events" reads "it
   lists 500 events". EventDetail's "The index lists the newest 500
   events" and SPEC-PLUGIN §3 change the same way. Tests: the three cases
   and an untouched whole transaction; stage 1's counter-example
   (`gates/rv-203/probe-tx.js`, "24 packages: 24 upgraded" of 40) reads
   partial.
5. **The real-home guard and the 0.2.0 live engine (round 3).**
   `real_engine_files` in `tests/plugin/real-home-guard.sh` lacks
   `recent-config.json`. Since this host runs the released 0.2.0, every
   full check fails desk-view's guard as soon as the live timer fires
   during the suite. A fix on `next` (add the file to the list, with a
   case in `real-home-guard.test.sh`) is the orchestrator's call; with it,
   or with the live timer paused by the operator, the gate can be re-run.
