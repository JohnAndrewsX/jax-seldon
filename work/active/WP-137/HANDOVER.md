# WP-137 — Handover: a transaction's packages in the Changelog detail

Branch `wp/137-transaction-detail` from `next` (aaf7a0a); merge into
`next`, never `main`. Plan: [PLAN.md](PLAN.md).

## Contract question (raised before any field was added)

The index had the packages (`txId`), their versions (`meta.from`/`to`,
`meta.version`) and the command (`meta.command`), but not whether a
transaction completed: the collector matched `transaction
completed|failed|interrupted` as one end marker, and a transaction
without an end line looked completed. I asked; the orchestrator answered
(2026-10-08): **`meta.txStatus`** under ADR-0035 §6, `contractVersion`
stays 2, **ADR-0043 as *proposed* for the operator**, engine + schema +
fixtures + plugin in this WP, no mark when the field is absent, separable
from WP-141.

## What was done

- **ADR-0043** (`decisions/ADR-0043-transaction-status.md`, *proposed*;
  DECISIONS.md row): `meta.txStatus` = `failed` | `interrupted`
  (pacman's own end line) | `unfinished` (no end line: the next
  `transaction started` came first, or the log ends with `db.lck`
  absent), on every package event of such a transaction; absent when it
  completed and on every line written before (append-only, no
  backfill). Only on `source: pacman` with a `txId`.
- **Engine**
  - `collectors/pacman.rs`: `Line::TxEnd(Option<TxStatus>)`, `Tx.status`,
    set in `parse` at the end line, at the next start and at the end of
    the log without the lock; a transaction pacman still runs is held
    back as before and gets no status; `Tx::events` writes it.
  - `model/event.rs`: `TxStatus` enum; `Meta.tx_status`, read leniently
    like `risk` (another word reads as none, the line still loads);
    `validate` refuses it off a pacman line with a `txId`.
  - `index/build.rs` (`clipped`): the index keeps it only on a pacman
    event with a `txId`.
  - `commands/event.rs`: `--meta txStatus=…` refused ("written by the
    pacman collector only").
  - `index/views.rs`: the month view adds `· transaction interrupted`
    (or `failed`, `unfinished`) after the version.
- **Schema / contract**: `event.schema.json` meta `txStatus` (enum,
  description, an `if` requiring `txId` and `source: pacman`), the
  conventional-keys text; CONTRACT.md rule 9; SPEC-ENGINE §4 and the
  `seldon event` refusal list; SPEC-LOGBOOK §5 (the view line); two
  invalid fixtures (`event.tx-status-without-tx`,
  `event.tx-status-other-word`).
- **Fixtures**: `logs/pacman.log` and `logs/pacman-rotation/pacman.log`
  gain **09-18** a plain `pacman -Syu` with a `:: Replace` (− pulseaudio,
  ↑ pipewire, + pipewire-pulse, ↑ wireplumber) and **09-19** a `pacman
  -Syu` that ends with `transaction interrupted` after gtk4 and
  libadwaita; both routine `sysupgrade`. The 09-27 downgrade group is
  the third case. Ledger (93 lines), the September view, the sample
  index (82 events) and every variant (`drift-explained-case`'s path
  index 70 → 76), the attention-all index, the engine goldens, the
  README story, counts and the byte offset (11832 → 13254) follow.
  `validate-fixtures.py` drops a stray `txStatus` like the engine and
  checks that a transaction's lines agree; two self-checks, each seen
  failing against a broken rule.
- **Plugin**
  - `Model.js`: `eventTx`, `txStatusOf`, `transactionIndex` (once per
    index, `deskChangelog().tx`), `transactionDetail`, `transactionRows`,
    `txSummary`, `txPackageLine`; rows carry `tx`, `txStatus`, `alert`
    (also Today's NEEDS YOU rows); `eventDetail` adds *Command* and
    *Transaction* after *What* and returns `transaction`.
  - `ListRow.qml` / `GroupedRow.qml`: `alert`, one word in
    `Color.urgent` before the meta line; `Changelog.qml` and `Today.qml`
    bind it; the Changelog's `view()` lists `alerts`.
  - `EventDetail.qml`: the urgent callout (*Transaction interrupted /
    failed / did not finish*) above the key/values, the package list
    below them, the files pacman left counted (WP-141), the cut note;
    `pragma ComponentBehavior: Bound` for the delegate; `view()` reports
    `transaction` and the form's `membersShown`.
  - `DriftForm.qml`: `membersShownAbove` hides the member lines when the
    list above holds every open member.
- **Tests**: engine unit (`line_table`, `transaction_status`,
  `transaction_buffering`, `a_tx_status_reads_leniently`,
  `validation_rules`, `meta_pairs`), integration
  (`contract_v2::a_tx_status_is_kept_only_on_a_transaction_line`, the
  refusal in `event_refuses_the_v2_kinds_and_keys`), and every fixture
  test with its counts; `model.test.js` 163 (5 new: mixed, interrupted
  with failed/unfinished/odd words, downgrade + cut index + clipped
  command, the WP-141 hook, the QML plain-text/no-colour check);
  `desk-view.sh` case `transactions` (8b′) and, with `DESK_SHOTS`,
  `shot-tx-<theme>` (interrupted at 100 % and 50 %, mixed).
- **Docs**: SPEC-PLUGIN (the detail, the row), TESTING.md, CHANGELOG
  (Engine, Plugin).

## Decisions (left open by the WP)

1. **`unfinished` is a status.** A transaction without an end line is the
   worst case (pacman killed mid-commit). Checked on the dev host's own
   log (counts only, read-only): 36 starts, 36 ends, all `completed`, no
   start without an end, so real logs give no false `unfinished`.
2. **Absent never reads as "completed".** Without `txStatus` nothing is
   marked and no text says the transaction completed (earlier engine
   builds, lines from before ADR-0043).
3. **The class does not change.** An interrupted `-Syu` stays routine
   (ADR-0043 §2): the mark is the urgent word and the callout, not a
   stripe, so the chips, the counts and "one count everywhere" stay the
   engine's. Whether "did not complete" should earn attention is an
   ADR-0028 question (below).
4. **Row mark:** one urgent word ("interrupted", "failed", "unfinished")
   before the meta line, on every package row of the transaction; the
   sidebar search finds it. A file pacman left (a note) is no package
   row and gets no word; its detail has the *Transaction* row.
5. **Detail layout:** the status callout above the key/values (the
   urgent thing first, like *Why loud?*); *Command* and *Transaction*
   after *What*; the package list after the key/values, the unusual
   first (↓ − + ↑ ↻), by name within a kind, the selected event's line
   bold.
6. **When the list shows:** two or more packages, a status, or files
   left. One completed package says no more than the rows (*Command*,
   *Transaction: 1 package: 1 installed*).
7. **The DriftForm's member lines** hide only when the list holds every
   open member of the group (the full 09-27 group); after an `--only`
   (two open of three) or when the index lacks a member, they stay, since
   then they say something the list does not.
8. **A cut transaction** (CONTRACT.md rule 4): when the index holds 500
   events and its oldest one belongs to the transaction, the detail says
   older lines are in the ledger. The plugin cannot know the true count.
9. **The ledger view** names the status too (engine, small): the
   logbook a human reads should say what the index says.
10. **Fixture:** two new transactions instead of editing existing ones
    (the 09-30 `-Syu` carries 38 self-checks); both routine, so no drift
    count moves.
11. **WP-141 hook, separable:** `eventTx` reads `meta.transaction` on a
    pacman `note`; such a note gets the *Transaction* row and is counted
    as "left N files" in its transaction's detail, never listed as a
    package. Unit-tested on a synthetic note only; no fixture row, no
    dependency on WP-141 or ADR-0042.
12. **Callout texts** say that pacman's post-transaction hooks run only
    after a completed transaction (libalpm logs `transaction completed`
    and then runs them; a failed or interrupted commit skips both). That
    is from my knowledge of `lib/libalpm/trans.c`, not checked against
    the source in this session (no network): **the reviewer should
    confirm it**, or the sentence should be softened to "may not have
    run".

## How it was verified

- `cargo test` (all), `clippy -D warnings`, `fmt` on every engine commit;
  `TZ=UTC` runs of the engine's lib, collectors, contract_v2, index,
  drift and idempotency tests and of `model.test.js`: green.
- Engine hand mutants (own target dir, deleted after): 8 of 8 killed
  (the next-start and end-of-log `unfinished`, the word mapping, the
  write in `Tx::events`, the index filter, the lenient read, the
  validate rule, the view suffix). Plugin mutants under node: 5 of 5
  killed (the cap rule survived first; a test was added).
- `omarchy plugin validate plugin/` and `just qmllint` (47 files, 0
  warnings) before each plugin commit.
- `desk-view.sh` with `DESK_SHOTS`: the three themes (tokyo-night,
  kanagawa, catppuccin-latte) at 100 % and 50 % looked at by eye: the
  urgent word in both rows, the callout, the list with ↑ − + glyphs in
  the theme font, nothing outside its box.
- `flock /tmp/seldon-check.lock just check`: an environment failure
  only (ENOSPC on `/run/user/1000`); see "Gate" below.
- Not run: tests as root (no privilege on the dev host; CI does);
  `check-perf`/`check-rss` (the index build gains one `Option` per event
  and a filter; no new pass).

## Gate

`flock /tmp/seldon-check.lock just check` on `22da65d` (the code head;
this handover commit adds only this file): **exit 1, an environment
failure only.** `/run/user/1000` (tmpfs, `XDG_RUNTIME_DIR`) is 100 % full
of old quickshell instance folders; per the orchestrator nothing there
was deleted or cleaned.

- Run 1 (`check-wp137-1.log`): `fmt-check`, `clippy`, `test` and
  `check-watch` green; then `just` could not write its recipe script for
  `check-packaging` ("No space left on device", os error 28: `just`
  keeps its temp files under `XDG_RUNTIME_DIR`).
- Run 2 (`check-wp137-2.log`, the same gate with `JUST_TEMPDIR` in my
  scratchpad; `/run/user/1000` untouched): everything green —
  fmt-check, clippy, every engine test (0 failed), check-watch,
  check-packaging, check-install, check-deploy, check-guard,
  schema-validate (`validate-fixtures: ok`), docs-check,
  plugin-validate, qmllint (47 files), `model.test.js` 163,
  `model.bench.js`, terminal-scripts 65/0, real-home-guard 11/0 — until
  **`service-states.sh`: 312 passed, 30 failed**, and `plugin-test`
  stopped there.
- So `desk-view.sh` and `bar-view.sh` ran on their own under the same
  lock: **desk-view 1467 passed, 119 failed**; **bar-view 194 passed, 0
  failed**.
- **Every one of the 149 failures (30 + 119) is "log has errors"**, and
  each such log holds only quickshell's two `Failed to copy (detailed)
  log from memfd … error code 28 "No space left on device"` lines (238
  in desk-view, 60 in service-states); no assertion failed. The
  `transactions` case: 28 assertions ok, its one failure the ENOSPC log
  check. graph-drag and graph-big (the timing cases that failed once
  under load earlier) passed.
- Before the host filled up, `desk-view.sh` with `DESK_SHOTS` ran on
  this branch with 1623 passed and 2 failed (a `shows` on a row
  scrolled out of view, fixed in the test, and the graph-big timing case
  under load). `service-states.sh` I did not run before the host filled
  up; of what it drives, this WP changes only `DriftForm.qml` (scenario
  35), by a property that defaults to the old behaviour, and its 312
  assertions all passed in run 2.

To repeat once `/run/user/1000` has room: `flock /tmp/seldon-check.lock
just check`.

## Open questions

- **Operator: accept ADR-0043** (`meta.txStatus`, contract 2).
- **Operator (follow-up, ADR-0028):** should a transaction that did not
  complete raise attention (or a crisis for an interrupted kernel or
  bootloader upgrade) instead of only being marked? Today it stays in
  its class.
- **ADR number:** 0043 was free on every branch I could see; a parallel
  WP may take it first; renumber on merge if so.
- **Merge with WP-141:** both touch `fixtures/logs/pacman.log`, the
  rotation log, the October/September ledger, the sample index and the
  variants, the goldens, the README and the counts in `model.test.js`,
  `desk-view.sh` and the engine tests. Resolve by taking both log lines,
  then `python3 scripts/validate-fixtures.py --write-index`,
  `SELDON_BLESS=1 cargo test --test collectors`, the views from the
  engine (as `index.rs::ledger_views_equal_the_fixture_views` does), and
  the counts from the failing assertions; the byte offset 13254 grows by
  WP-141's lines. Once WP-141 lands, its notes light up the hook (the
  *Transaction* row and "left N files") with no further change.
