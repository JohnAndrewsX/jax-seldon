# WP-203 — handover (every listed drift item keeps its event in the index)

Branch `wp/203-drift-events` in `wt/WP-203`, from `next` 1e872c28. Nothing
merged. ADR-0054 (an amendment of the accepted ADR-0053) is **proposed**;
the reviewer and the operator decide it. No `contractVersion` bump, no
schema change.

## Deviation from the WP's wording (please decide)

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
3. **Reference derive** (`scripts/validate-fixtures.py`): the same rule.
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

- **fixture**: `cargo test --test index` 52 passed, 0 failed, 2 ignored; clippy `-D warnings` and fmt clean;
  `scripts/validate-fixtures.sh` ok; `scripts/docs-check.sh` ok.
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
- **headless** and **fixture** (dev host, private gates folder, at
  93a9c8a9): `SELDON_FULL_CHECK=1 just check` → `check: ok`, exit 0
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
  the deploy's plugin validation, inside the same `just check` run.
  Only the handover commit came after this run.
- **not run**: CI; a live desktop; the reference derive's > 500 branch
  (the fixture logbook has 83 events, so the golden test exercises only
  the unchanged case; the Rust tests cover the rule).

## Open questions

1. The deviation above: 500 with pinned events (done) or 700 with a
   schema change and a bump question (not done).
2. ADR-0054 §2 puts the proposal's rows after `system`'s optional parts.
   A reviewer may weigh them before (rung 23): both keep "crisis rows
   last".
3. Plugin-only follow-ups (no contract change, ADR-0054 consequences):
   the transaction detail of an old group leader reads "1 package"
   without the `partial` note (0.2.0 marks only the list's last event; the
   drift form's member list stays whole via `members` and `drift show`);
   the texts "The index lists the newest 500 events" / "it keeps the
   newest 500 events" could read "500 events".
4. Proposal rows past rule 4's 200-row drift cap are not listed by the
   build (the cap is crises first, then newest); WP-203 only keeps listed
   rows through the cut. Not asked; noted.
