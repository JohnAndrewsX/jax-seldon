# WP-200 — handover (the engine keeps index.json under its budget)

Branch `wp/200-index-budget` in `wt/WP-200`, from `next` 4a7a2376 (the
v0.2.0 release candidate). Nothing merged; the orchestrator merges after
the v0.2.0 tag. ADR-0053 is **proposed**: a Fable review first, then the
operator accepts.

## Done

1. **ADR-0053** (`decisions/ADR-0053-index-size-budget-cut.md`,
   DECISIONS.md row): over the budget the engine cuts the index it
   writes, rung by rung, each list from its oldest end, only as many rows
   as needed (24 rungs: a screenful of each list first, then history,
   then the present, crisis rows last, then the optional parts of
   `system`). `summary`, `series.risk` and `system.areas[].cases` keep the
   counts of the whole index. A cut case takes its timeline span, a cut
   crisis row its mark. `index::write` never writes 1 000 000 bytes or
   more (exit 2, the old file stays). No schema change, no new field,
   `contractVersion` and `contractReadableFrom` stay 2. §5 is the
   per-field table ADR-0051 §3 asks for: what a released 0.2.0 plugin
   reads and what changes for it.
2. **Engine.** `engine/src/index/budget.rs` (`fit`, the ladder,
   `size`); `index::fit(&mut Built)` runs right before every write of
   `index.json` (`index`/`status` after the git state and before
   `--check` validates; the rebuild after writing commands; `watch`; the
   session hook). The views, dossier, `drift`, `triage`, `rebuild` and
   every other reader of the build keep the whole index. `build` no
   longer warns about the size (the git and autocommit fields are added
   after it); the writers do. Under the budget `fit` changes nothing (one
   serialisation, as 0.2.0's size check).
3. **Warning** (`index`/`status` `warnings`, stderr elsewhere): `index.json
   would be N bytes, over its budget of 1000000 (CONTRACT.md rule 5); the
   largest section is `cases` (N bytes); written at N bytes without the
   oldest 40 completed cases, 216 events; the summary still counts every
   case, change and crisis, the logbook keeps every row`.
4. **Docs.** CONTRACT.md rules 4 and 5; SPEC-ENGINE §6 (the size
   budget); SPEC-PLUGIN §3 (the plugin's own refusal stays, for older
   engines); CHANGELOG `[Unreleased]`: one Highlight and an Engine entry.
   The plugin is unchanged; its "Index unreadable" detail and the user
   guides stay true (*Build index* now gets out of it).

## Measured (release profile, fixture logbook scaled by `tests/common/scale.rs`)

| logbook | whole index | written | cut | largest section |
|---|---|---|---|---|
| ×150 | 880 999 B | 880 999 B | nothing | `cases` 494 914 B (queued 205 285 B / 450, active 187 657 B / 300); `series.timeline` 184 151 B (1 460); `events` 115 679 B |
| ×200 | 1 078 006 B | 999 834 B | 40 completed, 216 events | `cases` 649 114 B |
| ×300 | 1 487 210 B | 999 612 B | 40 completed, 400 events, 651 queued | `cases` 957 514 B |
| synthetic: 3 000 active cases, long titles | 3 414 993 B | 999 186 B | every rung before the active cases, then the oldest active ones; every crisis row stays | `cases` |
| synthetic: 250 crises, subjects of 512 characters each escaped to 6 bytes | 2 100 147 B | 999 493 B | every rung before, then 43 crisis rows; `summary.crisis` counts all | `events` 818 511 B |

`fit` takes 0.4 ms at ×150 (no cut), 3.6 ms at ×200, 10 ms at ×300.

## How it was verified

- **fixture** — `engine/tests/index.rs` (in `just test`):
  `under_its_budget_the_index_is_not_cut`,
  `an_index_over_its_budget_is_cut_oldest_first_and_keeps_its_counts`,
  `crisis_rows_go_last_and_stay_counted`,
  `an_index_still_over_its_budget_is_not_written`,
  `status_writes_a_large_logbook_under_its_budget` (the acceptance test:
  `seldon status --json` on the fixture ×200, own HOME and temp dir via
  `Env`; the written file is < 1 MB and validates, `summary` equals the
  whole index's (400 crises), drift and every open case list equal the
  whole index's, 10 completed cases, the newest events, `index --check`
  exit 0); `long_texts_keep_the_index_under_its_size_budget` now asserts
  `fit` cuts nothing. The timeline check asserts a span for every listed
  case and a mark for every crisis row.
- **fixture** — the released 0.2.0 `plugin/Model.js` (`git show
  4a7a2376:plugin/Model.js`, node) on the written ×150, ×200 and ×300
  files: `parseIndex` ok (996 768 and 996 099 characters for ×200/×300),
  `deriveStatus` `ok`, `counts` equal to the whole index (×300: active
  600, queued 900, crisis 600, attention 1 200), `pillTone` `urgent`,
  `deskKpis` verification 300. Measurement scripts and files stayed in
  the private gates folder, not in the repository.
- **headless** — `SELDON_FULL_CHECK=1 just check` on the dev host at
  79ace07e: `check: ok`. Quickshell harnesses: service-states 366/0
  (`index-oversized`: "Index unreadable" with the 1.2 MB detail, as in
  0.2.0), desk-view 2091/0, bar-view 195/0, ipc-restart 43/0; qmllint
  ok (52 files), `omarchy plugin validate` ok, model.test.js 217,
  plaintext ok, docs-check ok (514 links), runtime dir 2 % full before,
  328 by-id entries before and after. TMPDIR, JUST_TEMPDIR and the cargo
  target under the private gates folder.
- **test host** — shellcheck through the orchestrator kit's wrapper
  (E80), inside the same `just check` run: ok.
- **fixture** (bench profile, dev host) — `just check-rss`: ok, peak
  11 720 kB (< 12 MB). Index budgets of `just check-perf`: build ×10
  13.4 ms, ×150 80.7 ms (< 100 ms, 880 997 bytes, nothing cut), `status`
  at the stated scale 58.5 ms (< 100 ms), redaction ok.
  **`just check-perf` as a whole fails** on three timings that do not
  touch the index: `hook, recorded (tmpfs)` 12.3 ms and 21.1 ms (budget
  5 ms) and the recent-config scan delta 10.9 ms (budget 10 ms). The
  base commit 4a7a2376, built and run the same way, fails the same three
  with the same numbers (12.3 / 21.1 / 10.9 ms): the tests expect the
  temp dir on tmpfs, and this run's TMPDIR is on disk (the rule for this
  WP). Not caused by WP-200; not rerun on tmpfs.
- **not run** — a live desktop with a real logbook over 1 MB, and CI.

## Not done

- No field that tells the desk "N more rows in the logbook": that is a
  contract-3 field (WP-184); ADR-0053 §3 says why not now.
- The reference derive (`scripts/validate-fixtures.py`) does not cut
  (the fixtures are far below the budget).
- No plugin change.

## Open questions (for the reviewer and Fable)

1. **The order of rungs 2 and 3**: events (to 100) go before queued cases
   (to 50). ×300 shows the effect: 400 events and 651 queued cases. The
   argument: the Changelog is history the logbook keeps, a queued case is
   work still to do. The other order is one line in `LADDER`.
2. **Counts the 0.2.0 plugin takes from list lengths**: the
   verification count (header and sidebar), the proposed-decision count
   and Memory's counts drop with their rows. Verification and proposed
   decisions go only on rungs 20 and 22; memory topics and lessons on
   rungs 6, 11 and 12. None is a crisis or open-drift count.
3. **Crisis rows before refusing** (rung 23): a cut index still counts
   every crisis; a refused write would leave an old index that may count
   fewer. ADR-0053 chooses the cut; ADR-0051 §1's wording ("listed in
   `drift[]`") already had the 200-row cap of rule 4.
4. The backstop error makes `seldon status` exit 2 when what no rung
   cuts is itself 1 MB (a hand-edited dossier fence or logbook metadata
   of that size). Acceptable, or should the engine also cut those texts?
