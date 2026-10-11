# ADR-0053 — The engine keeps index.json under its budget: the oldest rows go, in a fixed order; the counts stay whole

**Status:** proposed
**Date:** 2026-10-11

> Amends CONTRACT.md rule 5 ("an index of 1 000 000 bytes or more makes
> the engine warn") and extends ADR-0020 and ADR-0025 (the caps and the
> text clip that keep an ordinary index small). No schema change, no new
> field, `contractVersion` and `contractReadableFrom` stay 2: a released
> 0.2.0 plugin reads the cut index as it reads any other. From the WP-126a
> review (F4, Q-A). Work package: WP-200.

## Context

1. **The plugin refuses an oversized index.** Since 0.2.0 the plugin
   refuses an `index.json` of 1 000 000 characters or more before
   `JSON.parse` (`Model.INDEX_MAX_CHARS`, E72): it parses on the shell
   thread. It shows "Index unreadable" with *Build index*.
2. **The engine only warns.** Over its budget (`SIZE_BUDGET`, 1 000 000
   bytes) the engine named the largest section in a build warning and
   wrote the file anyway. On a logbook that large, *Build index* writes
   the same file again and the desk stays on the banner: a dead end
   (review F4). Every crisis count of the bar is then gone.
3. **What grows.** Events and drift are capped (rule 4) and their texts
   clipped (ADR-0025). Open cases, decisions, memory topics, journal
   entries, the package history, the drift weeks and the timeline (a
   span per listed case, a mark per release) are not. Measured with the
   0.2.0 build (release profile, fixture logbook scaled by
   `tests/common/scale.rs`):

   | logbook | index | largest sections |
   |---|---|---|
   | ×150 (12 750 ledger lines, 1 200 cases) | 880 999 bytes | `cases` 494 914 (queued 205 285 in 450 rows, active 187 657 in 300); `series.timeline` 184 151 (1 460 rows); `events` 115 679 |
   | ×200 | 1 078 006 bytes | `cases` 649 114; `series.timeline` 232 001 |
   | ×300 | 1 487 210 bytes | `cases` 957 514; `series.timeline` 329 601 |
   | 3 000 active cases with 220-character titles (synthetic) | 3 414 993 bytes | `cases` |
   | 250 crises whose subjects take the most JSON the schema allows (512 characters, each escaped to 6 bytes; synthetic) | 2 100 147 bytes | `events` 818 511, then `drift` |

   A real machine passes the budget through its open cases: about 220
   times the fixture's, for example a large task list imported as
   queued cases (`seldon import task`).
4. **A gap 0.2.0 already has.** A crisis row whose event is older than
   the 500 newest has no Changelog row in 0.2.0 before any cut (rule 4's
   events cap); a follow-up WP lists those events in the build. In the
   fixture scaled ×150, none of the 200 listed crisis rows has its event
   in `events[]`; the bar counts them, the Changelog's chips do not.
5. **The window for a new field has closed.** ADR-0035 §6 allowed
   optional contract-2 fields only until 0.2.0 was tagged; a new field
   now is a schema change, which needs a bump (AGENTS.md §3). The
   released plugin would not read it anyway.

## Decision

### 1. Where

The engine builds the whole index as before. Right before it writes
`index.json`, it cuts the copy it writes to under the budget
(`index::fit`, `engine/src/index/budget.rs`): in `index`/`status`
(after the git state, before `--check` validates), in the rebuild after
every writing command, in `watch` and in the session hook. Under the
budget nothing changes: the file is byte for byte what 0.2.0 writes.
The views (`STATUS.md`, `ledger/*.md`, `DECISIONS.md`), the dossier,
`drift`, `triage` and every other reader of the build keep the whole
index; only the plugin's file is cut.

### 2. What goes, in which order

Over the budget, the lists below lose rows, one rung after the other,
each from its oldest end, and only as many as the file needs: after
each cut the engine measures again, and it stops the moment the file is
under the budget. A rung cuts its rows down to the number given (rows
it does not take do not count; the events' floor counts the whole list).
The first seven rungs keep a screenful of each list; the rest cut history
before the present, then the optional parts of `system`, and crisis rows
last.

| # | rows | oldest end | down to |
|---|---|---|---|
| 1 | `cases.completed` | earliest `closed` | 10 |
| 2 | `events` | oldest | 100 |
| 3 | `cases.queued` | earliest `created`, then id | 50 |
| 4 | `drift` items that are no crisis | oldest | 50 |
| 5 | `decisions` not `proposed` | lowest id | 20 |
| 6 | `memory.topics` | least recently updated | 20 |
| 7 | `today.yesterday` | earliest | 0 |
| 8 | `series.timeline` releases and snapshots | earliest | 0 |
| 9 | `series.packages` | earliest day | 0 |
| 10 | `series.drift` | earliest week | 0 |
| 11 | `memory.lessons` | first written | 0 |
| 12 | `memory.topics` | least recently updated | 0 |
| 13 | `system.areas` | last by name | 0 |
| 14–18 | `cases.completed`, `events`, `cases.queued`, `drift` no crisis, `decisions` not proposed | as above | 0 |
| 19 | `today.entries` | earliest | 0 |
| 20 | `cases.verification` | earliest `created` | 0 |
| 21 | `cases.active` | earliest `created` | 0 |
| 22 | `decisions` `proposed` | lowest id | 0 |
| 23 | the optional parts of `system` (snapshots, `recentConfig`, `pacmanIgnore`, `omarchy`, `packages`) | — | gone |
| 24 | `drift` crisis items | oldest | 0 |

An event a listed drift row names (`eventId`) is never cut while its row
is listed; a cut drift row takes its event and, for a crisis, its
timeline mark. Group members fall under the `events` rungs as before
(rule 4). A cut case takes its timeline span with it: the timeline names
listed rows only, as before.

What is never cut: `summary`, `state`, `logbook`, `contractVersion`,
`contractReadableFrom`, `triage`, `series.heatmap` (366 days) and
`series.risk`. The counts stay those of the whole index: `summary`
(`activeCases`, `queuedCases`, `openDrift`, `crisis`, `eventsToday`,
`events7d`), `series.risk` and `system.areas[].cases` are computed before
the cut and not touched by it. Texts are not cut further than ADR-0025
and ADR-0038 already do.

Measured with this order (release profile; the tests of WP-200 hold
the ×200 row and the three synthetic ones):

| logbook | written | cut |
|---|---|---|
| ×150 | 880 991 bytes | nothing |
| ×200 | 999 825 bytes | 40 completed cases, 216 events |
| ×300 | 999 603 bytes | 40 completed cases, 400 events, 651 queued cases |
| the fixture, 20 recent crises, 300 newer notes, 2 000 queued cases (1 337 639 bytes) | 999 866 bytes | 303 events, 460 queued cases; every drift row and its event stay |
| 3 000 long active cases | 999 826 bytes | every rung before 21, then 2 125 active cases; every crisis row and its event stay |
| 250 crises, longest subjects | 996 643 bytes | every rung before 24, then 95 crisis rows with their events; `summary.crisis` counts all |

The cut costs one serialisation under the budget (the size check 0.2.0
already made) and about 10 ms at ×300.

### 3. How the index says so

With the fields a 0.2.0 plugin already reads, and no new one:

- **The counts.** `summary` keeps the counts of the whole index, as rule
  4 already promises for the capped lists ("`summary.openDrift` and
  `summary.crisis` always count all"). A list shorter than its count is
  a cut list; rule 4 already says the index is a view and the logbook
  holds the rest.
- **The warning.** The build warning that named the largest section now
  names what went: `index.json would be N bytes, over its budget of
  1000000 (CONTRACT.md rule 5); the largest section is `cases` (N
  bytes); written at N bytes without 40 completed cases, 216
  events; the summary still counts every case, change and crisis, the
  logbook keeps every row`. `index` and `status` show it in their
  `warnings` (`--json` and the human output); `watch` in the `warnings`
  of its `rebuilt` line and its text log, as every build warning; the
  rebuild after a writing command and the session hook print it on
  stderr (`seldon: warning: …`). `status`'s terminal summary counts from
  the whole index (also "N in verification"); `events` in the `--json` of
  `index` and `status` is the rows written.

A field that marks the cut per list (for the desk to say "N more in the
logbook") is a contract-3 field (WP-184): it would need a bump now, and
a released plugin would not read it.

### 4. The backstop: an oversized index is never written

`index::write` refuses an index of 1 000 000 bytes or more (an engine
error, exit 2): the old file stays. With §2 this happens only when what
no rung cuts is that large by itself, for example a hand-edited dossier
fence or logbook metadata of a megabyte; the warning then ends `still N
bytes with every list cut; not written`. An old index the plugin reads
(it turns stale after two hours, with *Build index*) is better than one
it refuses. `index` and `status` then exit 2 with the error of the write
alone (one line); the rebuild after a writing command prints it as
`index.json not rebuilt: …`; the session hook prints both, the warning
and the write's error.

### 5. What a released 0.2.0 plugin reads

The plugin reads the cut index as its own contract (`contractVersion`
2, `contractReadableFrom` 2, the schema holds: every list only gets
shorter). Per field, as ADR-0051 §3 asks of a change a released plugin
reads:

| field | change | where 0.2.0 reads it | effect |
|---|---|---|---|
| `summary.*` | none | the pill, its tone, the header's KPIs, Today's counts, the sidebar counts | the same numbers and the same crisis colour as the whole index |
| `drift[]` crisis items | cut only on rung 24, each with its event and mark | the Changelog's crisis rows and chips, Today's NEEDS YOU, the crisis strip, the graph's crisis nodes | every crisis row stays, with its event, unless crisis rows alone are over the budget; the bar still counts and colours every crisis |
| `drift[]` other items | cut from rung 4, each with its event | the Changelog | fewer open-change rows; the count stays (`summary.openDrift`) |
| `events[]` | cut from rung 2 | the Changelog, the graph | fewer old rows, as past rule 4's 500; never one a listed drift row names |
| the Changelog's chips (open, crisis, attention: rows of listed drift; one per source) | from the lists | the Changelog header | crisis and open chips count every listed drift row whose event the whole index had (no change); the source chips count the events written |
| `cases.completed` | cut from rung 1 | Work, the timeline, the graph | fewer closed cases, as past rule 4's 50 |
| `cases.queued`, `cases.active` | cut from rungs 3 and 21 | Work, Today's plan, the agent picker | fewer rows; the counts stay (`summary.queuedCases`, `activeCases`) |
| Work's column headers | the lists' lengths | Work | "QUEUED 50" while the header's KPI says 450: the column counts its rows (a plugin-only follow-up can show "50 / 450" from `summary`) |
| `wipStatus` | `cases.active` length against the WIP limit | the Work header | lower, and its `urgent` tone may go, only on rung 21 |
| `anyCase` / `firstRun` | every case list empty | Today's first-run card | only once every case list is cut (rung 21 ran dry): the card can show while `summary.activeCases` > 0 |
| `cases.verification` | cut on rung 20 | Work; the header's and the sidebar's verification count is the list's length | the count drops with the rows: only after every rung before it ran dry |
| `decisions[]` | cut from rung 5, proposed ones on rung 22 | Decisions; the sidebar's proposed count and `decisionSummary`'s total are the list's | proposed decisions stay until rung 22; the total drops from rung 5 |
| `memory.topics`, `memory.lessons` | cut from rungs 6 and 11 | Memory; its sidebar count is the lists' | fewer rows and a smaller count |
| `today.entries`, `today.yesterday` | cut on rungs 19 and 7 | Today's journal; the Yesterday toggle's count is the list's | fewer entries and a smaller count |
| `series.timeline`, `series.packages`, `series.drift` | cut on rungs 8–10, spans and marks with their rows | Prime Radiant | shorter charts; `series.heatmap` and `series.risk` whole |
| `system.areas` | cut on rung 13 | System, the graph | fewer areas |
| `drift[].proposedCase`, `decisions[].cases`, `events[].case` | may name a case that is not listed | the link proposal, the graph | as today for a case past the 50 completed: the graph draws no edge, *Link to …* still runs `seldon drift link` with the id |
| `triage` | none (never cut) | the triage sheet | items stay; an item whose drift row went (from rung 4) shows "No longer open" and is not in the Apply count; *Apply proposals* still runs and the engine decides (ADR-0035 §6) |

No count the plugin keys a crisis or open drift on gets smaller, and
nothing that is not a crisis is written where it counts crises
(ADR-0051 §1's misreads). The counts that do get smaller (Work's
headers, `wipStatus`, verification, proposed decisions,
`decisionSummary`, Memory, Yesterday, the source chips, `firstRun`) are
counts of the rows the file lists, as rule 4 already has them.

## Consequences

- `engine/src/index/budget.rs` (`fit`, the ladder); `index::write`
  refuses an oversized index; `build` no longer warns (the writers do,
  after the git state, which is part of the file).
- CONTRACT.md rule 5: the engine keeps the file under the budget by §2
  and never writes one over it. SPEC-ENGINE §6 and SPEC-PLUGIN §3 say
  the same; the plugin's own refusal and its banner stay (an older
  engine can still write an oversized file).
- Tests (`engine/tests/index.rs`, `engine/tests/watch.rs`): the index
  under its budget is not cut; 3 000 long active cases; 250 crises with
  the longest subjects; recent crises with 2 000 queued cases (every
  drift row keeps its event); the backstop; `status`, a writing command,
  the session hook and `watch` on the fixture ×200 write under the
  budget, say what they cut and keep every count; `status`'s terminal
  counts.
- Follow-up WPs (pre-existing gaps, not this ADR): (a) the build lists
  the event of every listed drift row, past the 500 newest (context 4);
  (b) `fit` keeps the drift rows an unapplied triage proposal names;
  (c) plugin only: Work's column headers read "N / total" from
  `summary`. Numbers once the orchestrator assigns them.
- The reference derive (`scripts/validate-fixtures.py`) does not cut:
  the fixtures are far below the budget, and the golden test holds the
  engine to them.
- The plugin is unchanged. Its "Index unreadable" detail ("if it stays,
  seldon status in a terminal names the largest section") stays true
  for an older engine.

## Alternatives considered

- *Lower fixed caps for every list* (e.g. 100 open cases): changes the
  index of every machine, also far below the budget, and still needs a
  bound for texts. Rejected: a cut only where it is needed.
- *Clip case titles and subjects:* bounds the bytes per row, not the
  rows; rule 5 keeps titles whole. Not needed with §2.
- *A `trimmed` field per list now:* a schema change after the tag needs
  a bump (AGENTS.md §3), and 0.2.0 plugins would not read it. Queued
  for contract 3 (WP-184).
- *Several files (a lean index plus detail files):* the plugin reads
  one file; a bump and a plugin change. Rejected for 0.2.x.
- *Refuse to write instead of cutting crisis rows (rung 24):* the old
  index stays, and it may count fewer crises than the logbook holds; a
  cut index still counts all of them. Rejected.
- *Cases before events* (rung 3 before rung 2): the Changelog is
  history the logbook also keeps; a queued case is work still to do.
  Rejected, but this is the order a reviewer may weigh differently.
