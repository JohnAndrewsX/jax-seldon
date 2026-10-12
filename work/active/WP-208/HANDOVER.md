# WP-208 — handover (the plugin marks a transaction partial by its own rows)

Branch `wp/208-tx-partial` in `wt/WP-208`, from `next` 9a9f5f7b (v0.3.0).
Plugin only: no engine, schema or fixture change; no `contractVersion`
bump. Nothing merged.

## What was done

1. **The rule** (`plugin/Model.js`). New `eventsBoundary(index)`: with
   the list at its 500-event cap (`INDEX_EVENTS_MAX`), the position of
   the oldest event that is no listed drift item's `eventId`; -1 under
   the cap. `transactionIndex` now gives each transaction `partial`
   (the field `oldest` is gone) and an `open` count. A transaction is
   partial when
   - (a)+(b) one of its events (a package line, or a file it left) sits
     at or past the boundary: it holds the boundary event (a), or a
     pinned event older than it, which is a listed item's leader (b).
     Every event past the boundary is pinned (ADR-0054 §1), so this is
     the drafted (a) or (b) in one test;
   - (c) its drift group (`drift[].txId`, `members`) lists fewer open
     members (`isOpenMember`: caseless, unresolved) than `members`. Open
     members, not all package rows: a line linked to a case is in the
     transaction but not in `members`, so counting every row could hide
     a missing open one (mutant M6 below). (c) holds at any list size.
   `transactionDetail` shows `partial` only when a package is listed: a
   file whose transaction the index lists no package of keeps its own
   line "not in the index any more (…)", without the note under it.
2. **Texts.** `transactionRows`: "not in the index any more (it lists at
   most 500 events)". EventDetail's note: "The index lists at most 500
   events; the rest of this transaction is in the ledger." SPEC-PLUGIN
   (the transaction detail paragraph, §5) states the rule. CHANGELOG
   `[Unreleased]`: a Highlights bullet and a Plugin entry.
3. **Tests.**
   - `tests/plugin/model.test.js`, new test `transactionDetail: a
     transaction is partial by its own rows (WP-208)`: a synthetic full
     list (500 events) built in the test, no fixture change:
     - the 0.2.0 counter-example of the WP-203 review: the rest ends in a
       transaction of 40 whose newest 24 lines are listed, followed by
       pinned events → "24 packages: 24 upgraded", partial (a);
     - a pinned group leader of 3 and a pinned single item whose other
       lines (in a case) were cut, neither the list's last event →
       partial (b); the single item only by (b);
     - a group of 5 with 3 listed, far from the boundary → partial (c);
     - an untouched whole group of 4 → not partial; one line linked to a
       case: 3 open of `members` 4 → partial, of `members` 3 → whole;
     - under the cap (boundary lines dropped): the boundary transaction
       and the pinned single item read whole, the groups by (c) only;
     - a file left by a transaction with no listed package: no note.
   - The WP-137 tests are unchanged except the "not in the index any
     more" text.
   - `tests/plugin/desk-view.sh`: `drift-show` (the fixture without
     lib32-mesa: the group lists 2 of 3) shows the note; `transactions`
     step 4 (the whole mesa group) has none.

## Deviation from the WP's wording (please decide)

The WP and the draft say the texts read "500 events". I wrote **"at
most 500 events"**: (c) also fires when the size budget cut the list
(CONTRACT.md rule 5, ADR-0053), where the index lists fewer than 500,
and CONTRACT.md rule 4 says "at most 500 events" since WP-203. One word
in two strings and SPEC-PLUGIN; easy to drop if you want the WP's form.

## How it was verified

- **fixture** (dev host, node): `node tests/plugin/model.test.js` → 222
  passed (base 221). Mutants, each alone, source restored with `git
  checkout` afterwards:
  - M1 no boundary rule → 2 tests fail (WP-137's cut index, WP-208);
  - M2 back to 0.2.0's "the list's last event" → WP-208 fails;
  - M3 only the boundary event itself (`i === boundary`) → WP-208 fails;
  - M4 no `members` rule → WP-208 fails;
  - M5 `partial` without a listed package → WP-208 fails;
  - M6 count every package instead of open members → WP-208 fails;
  - M7 no 500 gate (boundary on any list) → 2 tests fail;
  - M8 pinned events ignored (boundary = last event) → WP-208 fails.
- **fixture** (dev host): the WP-203 stage-1 probes
  (`gates/rv-203/probe-tx.js`, `probe-txall.js`, `boundary.json`, read
  only) against this branch's `Model.js`: T-REVIEW "24 packages: 24
  upgraded" of 40 reads **partial true** (0.2.0: false); the pinned
  mesa leader reads partial true (as in 0.2.0, where it was the oldest
  pinned one).
- **fixture** (dev host): `transactionIndex` of v0.3.0 and of this
  branch over `fixtures/index.sample.json` and every
  `fixtures/index-variants/*.json`: `partial` equals the old `oldest`
  for every transaction of every file except one:
  `drift-members-capped.json`, whose mesa group lists 2 of its 3
  members, now reads partial (c). That is the case the WP fixes.
- **headless** and **fixture** (dev host, private gates folder: TMPDIR,
  JUST_TEMPDIR and the cargo target under it; at 7f08594f, under `flock
  /tmp/seldon-check.lock`; log `gates/check-dev-208.log`):
  `SELDON_FULL_CHECK=1 just check` → `check: ok`, exit 0. In it:
  - cargo 2 915 passed, 0 failed, 22 ignored over 112 test binaries;
    fmt and clippy clean;
  - validate-fixtures ok (154 instances, 14 variants); docs-check ok
    (523 links, 14 translated pages); qmllint ok (53 files);
    model.test.js 222; plaintext.test.js all PlainText (188 blocks);
    check-tokens.test 14/0; real-home-guard.test 71/0;
    runtime-dir.test 45/0;
  - in a private headless Quickshell: service-states 366/0, desk-view
    2454/0 (the new `drift-show #2` and `transactions #4` checks
    included; mutant run clean), bar-view 195/0, ipc-restart 43/0;
  - desk-view's guard: `the real ~/.local/state/seldon changed by the
    operator's live engine (not a leak)`; the other suites report the
    real state and config untouched; 333 by-id entries before and
    after each suite.
- **test host**: shellcheck through the orchestrator kit's wrapper and
  the deploy's plugin validation (`deploy: plugin validated on the
  host`), inside the same `just check` run.
- **not run**: CI; a live desktop; the test host's live desk. Only
  this handover commit came after the run.

## Open questions

1. "at most 500 events" or the WP's "500 events" (above).
2. Not in the WP's text list, so unchanged: the graph footer "Newest N
   events · …" (`Model.js` `graphBuild`, SPEC-PLUGIN §5, desk-view
   `graph-big`). Since ADR-0054 the N events are not all the newest when
   pinned events are older than the rest. Changing it changes the
   headless desk-view's expected text; a follow-up if you want it.
3. Known limit, as before: under the size budget (rule 5) the index has
   no field that says events were cut, so (a)/(b) apply only at the
   500-event cap; (c) covers a group under the cut, a transaction
   without an open group is not marked. A pacman transaction that
   other sources' events interleave (same seconds) can lose its oldest
   lines without holding the boundary event; not seen in practice.
