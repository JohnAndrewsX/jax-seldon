# WP-208 — handover (the plugin marks a transaction partial by its own rows)

Branch `wp/208-tx-partial` in `wt/WP-208`, from `next` 9a9f5f7b (v0.3.0).
Plugin only: no engine, schema or fixture change; no `contractVersion`
bump. Nothing merged.

## Round 2 (review-1: SEND BACK; orchestrator decisions F1–F3, Q1)

Commit 1a6378dc. Round 1 below stays as written; where round 2 changed
it, this section rules.

- **F1, the note for one listed package.** `transactionDetail.list` now
  also holds when the transaction is partial, so the package list, and
  the note in it, appears for a partial transaction of one package (the
  pinned leader whose other lines were cut). The Transaction row is
  unchanged. A whole completed package still shows no list.
  - Model: the WP-208 test asserts what the detail shows (`summary`,
    `partial`, `list`, `partialText`) for every case, and the
    Transaction row of the one-package leader (T-PIN).
  - desk-view: new case `tx-leader-only` (the sample without lib32-mesa
    and vulkan-radeon, the review's `leader-only` shape, built with jq in
    the script; `drift show` gives the three members): the list shows
    "1 package: 1 downgraded in this transaction" and "↓ mesa …", the
    note is drawn, the form holds 3 members, nothing overflows.
- **F2, rule (b) hedged.** `partial` is now `"cut"`, `"maybe"` or `""`;
  `transactionDetail` gives `partialText` (constants
  `TX_PARTIAL_TEXTS`), EventDetail draws it:
  - `cut` (rules a and c: the index shows lines are missing): "The index
    lists at most 500 events; the rest of this transaction is in the
    ledger."
  - `maybe` (rule b alone: at the cap, a line past the boundary, i.e. a
    pinned drift event, and neither a nor c): "The index lists at most
    500 events; the ledger may hold more of this transaction."
  - `cut` wins over `maybe`. A pinned group leader whose other members
    are cut is `cut` by (c); a pinned single item, or a pinned leader
    whose group is all listed, is `maybe`. In the review's
    `boundary.json` the whole ollama install now reads `maybe`, mesa
    `cut`, T-REVIEW `cut`.
  - SPEC-PLUGIN (the transaction detail paragraph) and the CHANGELOG say
    exactly that.
- **F3, the graph footer.** "N events · M completed cases in the index"
  (no "Newest"), and " · the others are only in the logbook" instead of
  " · older ones are only in the logbook" (older events can be listed
  now; the clause also covers completed cases). SPEC-PLUGIN §5, the
  model test and desk-view's `graph-settle` and `graph-big` follow.
- **Q1** accepted: "at most 500 events" stays.

### Round 2 verification

- **fixture** (dev host, node): `model.test.js` 222 passed. Mutants on
  the round-2 model, each alone, source restored and compared with
  `diff -q` against a saved copy:
  - M1 no boundary rule → WP-137 and WP-208 tests fail;
  - M2 back to 0.2.0's "list's last event" → WP-208 fails;
  - M3 everything past the boundary `cut` (no hedge) → WP-208 fails;
  - M4 no `members` rule → WP-208 fails;
  - M5 a note without a listed package → WP-208 fails;
  - M6 every package counted, not open members → WP-208 fails;
  - M7 no 500 gate → WP-137 and WP-208 fail;
  - M8 pinned events ignored → WP-208 fails;
  - M9 `list` without the partial term (round 1's F1) → WP-208 fails;
  - M10 the `maybe` text equal to the `cut` text → WP-208 fails;
  - M11 `maybe` overwrites `cut` → WP-208 fails (it survived until the
    test got the case "the boundary and a pinned older line in one
    transaction").
- **fixture** (dev host): the review's `probe/probe-208.js` (read only)
  with 1a6378dc's `Model.js` against v0.3.0: `boundary.json` T-REVIEW
  `cut`, list true; ollama `maybe`, list true; mesa (1 package) `cut`,
  list true, note visible. `leader-only.json`: mesa "1 package: 1
  downgraded" `cut`, list true, note visible; every other transaction
  `""`. `index.sample.json`: none partial. (The probe counts every
  transaction as "changed" because `partial` is now a string; old
  `false` equals new `""` in each line.)
- **headless** (dev host, private gates folder, under the lock): 
  `tests/plugin/desk-view.sh` alone with `plugin/` at 2c5332d9 and the
  round-2 script (log `gates/desk-view-dev-208-old-plugin.log`): 2455
  passed, 5 failed, exactly the three `tx-leader-only #2` checks and the
  two footer texts (`graph-settle #3`, `graph-big #3`). `plugin/` was
  restored to HEAD right after (`git diff --quiet HEAD`).
- **headless** and **fixture** (dev host, private gates folder: TMPDIR,
  JUST_TEMPDIR and the cargo target under it; at 1a6378dc, under `flock
  /tmp/seldon-check.lock`; log `gates/check-dev-208-r2.log`):
  `SELDON_FULL_CHECK=1 just check` → `check: ok`, exit 0. In it:
  - cargo 2 915 passed, 0 failed, 22 ignored over 112 test binaries;
    fmt and clippy `-D warnings` (with and without `watch`) clean;
  - validate-fixtures ok (154 instances, 14 variants); docs-check ok
    (523 links, 14 translated pages); qmllint ok (53 files);
    model.test.js 222; plaintext.test.js all PlainText (188 blocks);
    check-tokens.test 14/0; real-home-guard.test 71/0;
    runtime-dir.test 45/0; live-index-wait 72 wait, 12 opt out, 0
    problems;
  - in a private headless Quickshell: service-states 366/0, desk-view
    2460/0 (`tx-leader-only #2` ×5, `graph-settle #3` and `graph-big #3`
    with the new footer, `drift-show #2`, `transactions #4`; mutant run
    clean), bar-view 195/0, ipc-restart 43/0;
  - desk-view's guard: `the real ~/.local/state/seldon changed by the
    operator's live engine (not a leak)`; the other suites report the
    real state and config untouched; 333 by-id entries before and after
    each suite.
- **test host**: shellcheck through the orchestrator kit's wrapper and
  the deploy's plugin validation (`deploy: plugin validated on the
  host`), inside the same `just check` run.
- **not run**: CI; a live desktop; the test host's live desk. Only the
  handover commit came after the run.

## What was done (round 1)

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

## Deviation from the WP's wording (decided: accepted in round 2, Q1)

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

1. Answered (round 2, Q1): "at most 500 events".
2. Answered (round 2, F3): the graph footer changed in this WP.
3. Known limit, as before: under the size budget (rule 5) the index has
   no field that says events were cut, so (a)/(b) apply only at the
   500-event cap; (c) covers a group under the cut, a transaction
   without an open group is not marked. A pacman transaction that
   other sources' events interleave (same seconds) can lose its oldest
   lines without holding the boundary event; not seen in practice.
4. Review Q4 (round 2 headless): the one-package case is in the dev's
   gate (`tx-leader-only` in desk-view), so the reviewer can read it from
   the full check's log without setting up a harness.
