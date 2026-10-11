# ADR-0054 — ADR-0053 amended: every listed drift item's event is in the index; a proposal's open changes go right before the crisis rows

**Status:** proposed
**Date:** 2026-10-12

> Amends [ADR-0053](ADR-0053-index-size-budget-cut.md) §2 (one rung
> added before the crisis rows), §5 (the rows below) and its consequences
> (a) and (b), and CONTRACT.md rule 4's event count (ADR-0020). ADR-0053
> stays accepted and unedited; everything else in it stands. No schema
> change, no new field: `contractVersion` and `contractReadableFrom` stay
> 2. Work package: WP-203, from the WP-200 stage-2 review (Fable,
> 2026-10-11, follow-ups a and b).

## Context

1. **A crisis without a Changelog row (ADR-0053 context 4).** Rule 4
   listed the newest 500 events. The 0.2.0 plugin finds a drift row only
   through its event (`changelogRows`, `driftItemFor`, the chips, Today's
   NEEDS YOU, the triage sheet's `open`). A listed drift item whose event
   is older than the 500 newest is counted by the bar and missing from
   all of them. On the fixture logbook scaled ×150 none of the 200 listed
   crisis rows had its event, before any cut.
2. **The schema caps `events` at 500** (`index.schema.json`,
   `maxItems: 500`). The follow-up as first worded ("the newest 500, plus
   the event of every listed drift item", up to 700) would change the
   schema after the 0.2.0 tag, which needs a bump (AGENTS.md §3, ADR-0053
   context 5). The 0.2.0 plugin would read 700 rows (it has no such cap),
   but the released engine's `index --check` and every schema validator
   would refuse them.
3. **A proposal item reads "No longer open" after a cut.** From rung 4
   on, `fit` may cut an open change that an unapplied triage proposal
   names; the sheet then labels it "No longer open" and leaves it out of
   the Apply count, though `drift apply` still decides it by the engine's
   classification (ADR-0035 §6). The proposal file names its items by
   `eventId` (`proposal.schema.json`), and the engine already reads that
   file for `index.triage`: the build can keep the ids beside the index
   without writing them.

## Decision

### 1. The events of the index

`index.events` holds at most 500 events, newest first, as the schema
says: the event every listed drift item names (`eventId`, a group's
leader), however old, and the newest of the rest (`build::listed_events`).
At most 200 items are listed (rule 4), so at least the newest 300 other
events stay. When every listed item's event is among the newest 500,
nothing changes. A group's other members are not added: a large pacman
transaction would take the list; `seldon drift show` has them, as before.

The cut of ADR-0053 is unchanged for events: the events rungs never take
an event a listed drift item names, and their floor counts the whole
list, so on a large logbook the pinned events leave less room for
history.

### 2. The rows of an unapplied proposal

The rungs of ADR-0053 §2 that cut drift items that are no crisis (4 and
17) leave those an unapplied proposal names (`index.triage` with
`applied` null; its items' `eventId`s, read by `triage::read` and never
written). A new rung after `system`'s optional parts takes them, oldest
first, each with its event; the crisis rows stay last:

| # | rows | oldest end | down to |
|---|---|---|---|
| 23 | the optional parts of `system` | — | gone |
| 24 | `drift` items that are no crisis and that an unapplied triage proposal names | oldest | 0 |
| 25 | `drift` crisis items | oldest | 0 |

A proposal holds at most 200 items (`proposal.schema.json`). An applied
proposal holds nothing: its run is over, and an item still open is an
open change like any other.

### 3. What a released 0.2.0 plugin reads

Read with the 0.2.0 `Model.js` (`git show v0.2.0:plugin/Model.js`, node)
on the fixture logbook scaled ×150, ×200 and ×300 (release profile,
`tests/common/scale.rs`), before (1e872c28) and after:

| logbook | index before → after | events written | crisis rows with a Changelog row | crisis chip, NEEDS YOU | pill crisis |
|---|---|---|---|---|---|
| ×150 | 880 974 → 896 715 B, no cut | 500 → 500 | 0 → 200 of 200 | 0 → 200 | 300 |
| ×200 | written 999 808 → 999 943 B | 284 → 214 | 0 → 200 of 200 | 0 → 200 | 400 |
| ×300 | written 999 586 → 999 537 B | 100 → 200 | 0 → 200 of 200 | 0 → 200 | 600 |

The pill counts every crisis (`summary.crisis`); rule 4's 200-row drift
cap lists 200 of them, and the desk now shows a row for each listed one.
×200 cuts 286 events instead of 216 (the 200 crisis events are pinned);
×300 cuts 708 queued cases instead of 651, for the same reason.

Per field, as ADR-0051 §3 asks:

| field | change | where 0.2.0 reads it | effect |
|---|---|---|---|
| `events[]` | the same ≤ 500 rows, newest first; up to 200 of them older than the newest of the rest | the Changelog, the graph | a row for every listed drift item; up to 200 fewer recent history rows, only while listed items are older than the 500 newest |
| the Changelog's chips | from the lists | the Changelog header | crisis, open and attention chips count every listed drift row; the source chips count the events written |
| `transactionDetail` (`partial`) | an old leader's transaction lists only its listed packages | an event's detail | 0.2.0 marks a transaction partial only when it holds the list's last event; an old leader's transaction reads "1 package" without the note, and so may the transaction cut at the boundary of the newest when an older event follows it. The drift form compares the packages with the item's `members` and asks `drift show` for the rest, so the group's own count stays whole. ADR-0053's cut had the same case (a pinned leader whose members went). Not a count a crisis or open drift is keyed on. |
| graph | none | Prime Radiant | it added every drift item as a node already (ADR-0053 §5, join 10); the same nodes |
| `triage` | items named by an unapplied proposal stay listed until rung 24 | the triage sheet | "No longer open" only after every rung before 24 ran dry; the Apply count stays whole until then |
| `summary.*` | none | the pill, the header, Today | the same counts |

No count the plugin keys a crisis or open drift on gets smaller; the
crisis chip and NEEDS YOU now equal the listed crisis rows. Nothing that
is not a crisis is written where crises are counted.

## Consequences

- `engine/src/index/build.rs` (`listed_events`), `engine/src/index/budget.rs`
  (`Rows::Triaged`, the ladder), `engine/src/index/triage.rs` and
  `model::Triage::event_ids` (`#[serde(skip)]`).
- CONTRACT.md rule 4 ("500 events: the event of every listed drift item,
  however old, and the newest of the rest") and rule 5 (the proposal's
  rung); SPEC-ENGINE §6; the reference derive
  (`scripts/validate-fixtures.py`). The golden fixtures do not change:
  the fixture logbook has 83 events.
- Tests (`engine/tests/index.rs`): a crisis row older than the 500
  newest has its event; the cut keeps the rows an unapplied proposal
  names (and an applied one holds nothing); those rows go after
  `system` and before the crisis rows; `triage::read` names the ids and
  the index does not write them. WP-200's ×200 test now expects 286
  events cut and every drift row's event kept.
- Follow-ups, plugin only: the transaction detail could mark a group's
  transaction partial when it lists fewer packages than the item's
  `members`; the text "The index lists the newest 500 events" reads "500
  events" (SPEC-PLUGIN §3). Neither is a contract change.

## Alternatives considered

- *The newest 500 plus up to 200 more* (the follow-up as worded):
  `maxItems` 700 is a schema change after the tag, so a bump
  (AGENTS.md §3); the released engine's `index --check` would refuse the
  index. Rejected for 0.2.x; contract 3 (WP-184) may raise the cap.
- *Every member of a listed group too:* a pacman transaction can hold
  hundreds of packages; unbounded within 500. Rejected.
- *Keep the proposal's rows with the crisis rows on one rung:* an open
  change would then outlive a crisis. Rejected: crisis rows last.
- *Hold the rows of an applied proposal too:* after the run its regular
  items are resolved; what is left is open drift like any other.
  Rejected.
