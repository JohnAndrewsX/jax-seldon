```
WP-091 HANDOVER
Done: (a) snapper changing between degraded and ok since its last run for this logbook writes one `seldon` note, subject `snapper`, actor `system`, no case, with the collector's message, in both directions; (b) N2: a collector not run (`--source`, or disabled) in the capture that drops its state is marked through the same baselines → losses → held_losses chain, on an entry with only the mark; (c) doctor gives a marked collector its own `state` row (degraded or not run since a state reset, `cursors` or `logbook` wording, a `--source` capture as the fix); SPEC §2 §3 §4, CHANGELOG, TESTING.md
Not done: user guide 10-troubleshooting (en/de) does not describe the new doctor row (not in scope; see Decisions 3)
Verified by: flock /tmp/seldon-check.lock just check → exit 0 (`check: ok`, run once, after the last mutant); full cargo test 0 failures; fmt + clippy --all-targets -D warnings clean; docs-check ok; 24 mutants, all killed
Learned: memory/pitfalls.md, section "WP-091"
Decisions needed: 4, none blocking (below): any degraded state counts for the note; downgrade reads of a bare entry; user guide row; a disabled collector's mark is not in doctor
Touched outside WP scope: engine/src/index/mod.rs and the one doctor.rs line for the optional `lastRun` (2 lines), engine/tests/idempotency.rs, docs/TESTING.md, CHANGELOG.md, memory/pitfalls.md
```

Branch `wp/091-reset-notes`, worktree `wt/WP-091`, from `886d876`. No
push, no PR. Commits (oldest first):

- `06d39f3` engine: a collector not run in a state reset records its gap later (WP-091)
- `9abf2b9` engine: doctor names a waiting baseline by its reason (WP-091)
- `fbc6caf` engine: snapper access changes are recorded as notes (WP-091)
- `8f44f21` engine: test a mark of another logbook and the snapper-only note (WP-091)
- this commit: handover and pitfalls

Each code commit carries its own tests, SPEC, TESTING and CHANGELOG lines.

## Design decision for N2 (taken before coding)

**What `index.state.collectors` derives from a bare marked entry:
exactly what it derives from no entry** — `ok: true`, no message,
`lastRun: null`. The collector never ran under this state, so the index
must not claim a run or a failure. No schema change.

To get there:

- `CollectorState.last_run` became `Option<String>`, `serde(default,
  skip_serializing_if = Option::is_none)`. Every existing file has
  `lastRun`, so it reads as `Some` and writes back byte-identical (the
  WP-088 round-trip test still passes unchanged).
- `CollectorState::waiting(mark)` is the bare entry:
  `{"ok":true,"events":0,"pendingBaseline":"cursors"|"logbook"}` — no
  `cursor`, no `lastRun`, no `message`, no `fix`.
- Readers of `lastRun` skip `None`: the index (`and_then`), doctor's
  "last capture" comparison (`filter_map`). doctor's collectors row skips
  it already (`ok: true`).

## (b) N2: a collector not run in the capture that loses the state

- `capture::waiting_baselines` now takes the capture's not-run collectors
  besides the degraded ones. A not-run collector enters the chain only
  when it has **no entry** in `cursors` as bound for this capture:
  - binding none (no file / no logbook) or another logbook (`bind` drops
    every entry): it would lose its state, so it goes through
    `baselines` → `losses` (`cursors` resp. `logbook`) → `held_losses`
    (at least one event of its source).
  - bound here with an entry: kept exactly as it is (WP-088 behaviour). An
    unreadable cursor there is recorded by its own next run, and doctor
    keeps calling it "cursors unreadable", which is true.
  - bound here without an entry: the gate says "never ran here", no mark.
- `run` writes a `CollectorState::waiting` entry for each marked not-run
  collector at the same moment it saves the others (after the append).
- The read side is WP-088's, unchanged: `loss` sees the mark, the first
  successful run records `state-reset` with the mark's `files` word, and
  clears it. That also holds for a collector enabled again later.
- doctor's `pending_reset` only looks at enabled collectors, so a
  disabled collector's mark is silent until it is enabled (Decisions 4).

## (c) doctor wording for a marked collector

`check_pending_reset` now returns `Vec<Check>`. Under binding "this
logbook" it splits `pending_reset`'s losses by the mark:

- unmarked (an unreadable cursor): the WP-083 row, unchanged.
- marked: a new row, `waiting_row`:
  - message: `snapper degraded or not run since a state reset (cursors
    missing or unreadable in ~/.local/state/seldon); its next successful
    capture records the gap, so changes made in between may not be
    recorded`;
  - with a `logbook` mark: `(the state in ~/.local/state/seldon belonged
    to another logbook)`; with mixed marks each collector gets its own
    reason; plural: "each one's next successful capture";
  - fix: `run seldon capture --source snapper once it can run (a
    degraded collector: the collectors row's fix first)`; with
    `--logbook` the capture command names that logbook. No restore: the
    other collectors took their new baseline already, and restoring their
    old cursors could replay config diffs.
- A mark counts only in cursors bound here: another logbook's marks are
  that logbook's, so doctor on a second logbook still shows the "bound to
  another logbook" row (test `a_mark_of_another_logbook_is_no_waiting_row_here`).

## (a) The snapper note

- `capture::access_change(cursors, states, now)`: snapper ran in this
  capture, its entry in `cursors` (as bound for this capture) has a
  `lastRun`, and `ok` differs → one event `Event::new(now, Seldon, Note,
  "snapper")` (actor `system` by default, no case, no meta). It is
  appended with the capture's events, after the state-reset note.
- Detail:
  - `snapper collector degraded: <message>; at the last capture it was ok`
  - `snapper collector ok again (<message>); at the last capture it was
    degraded: <earlier message>` — the `(<message>)` part only when the
    run has one, which the info-file path (ADR-0026 grant) does:
    `snapper list is not permitted; N snapshots read from …`.
- No note: the first run for a logbook; after a lost state directory or
  another logbook's state (`bind` emptied the entries, nothing to compare
  with); for an entry with only the mark (`lastRun` none); in a capture
  that does not run snapper. The saved state then carries the new `ok`,
  so the next capture writes nothing.
- Not an own change (rule 8 explains only plugin/package kinds) and no
  drift (zone none for `seldon` notes); both are asserted.
- `snapper.rs` and `collectors/mod.rs`'s collector interface are
  untouched; the note lives in `capture.rs`.

## Tests

`engine/tests/idempotency.rs`:

- `state_reset::` (N2, new):
  - `a_collector_not_run_in_the_reset_records_its_gap_when_it_runs`:
    `--source pacman` before snapper has events → no entry (ledger rule);
    after a removed state directory, `--source pacman` writes only
    pacman's note, snapper's entry is exactly the bare JSON, omarchy,
    plugins, theme and config get none; snapper's index row equals
    theme's (no entry) apart from the name; doctor: no "next capture" row,
    the waiting row names snapper; not run again → unchanged; with an
    unreadable pacman cursor besides, both rows show (pacman, snapper)
    and the next capture writes one note `snapper,pacman`/`cursors`;
    mark cleared, `lastRun` set, then 0.
  - `a_disabled_collector_when_the_state_was_another_logbooks_says_so_later`:
    snapper disabled while the state was another logbook's → mark
    `logbook`, index `enabled: false`; enabled again → note
    `snapper`/`logbook`, "Nothing can be restored", then 0.
  - `a_collector_not_run_keeps_its_entry`: bound here, an unreadable
    snapper cursor, `--source pacman` → entry byte-equal, "next capture"
    row names snapper, no waiting row; the next capture records it.
  - `a_mark_of_another_logbook_is_no_waiting_row_here` (doctor).
- `state_reset::` (WP-088 tests, adjusted): the waiting row's exact
  message and fix (`cursors`), the `logbook` wording, `predicted()` no
  longer names marked collectors; the success capture after a degraded
  snapper now writes 2 (reset note + access note), asserted.
  `a_first_successful_run_after_a_degraded_init_is_no_reset` now asserts
  no `seldon` note at all (kills "a note for every collector").
- `snapper_access::` (new module):
  - `a_grant_and_its_removal_are_recorded_once_each`: degraded first run
    → no note; repeat → 0; info files appear (the grant) → one note with
    both messages, actor `system`, no case, `explainedSelf` 0; repeat → 0;
    info files gone (a `SYNC_ACL` rewrite) → one `degraded` note; repeat
    → 0; no state reset; ledger schema-valid; neither note is in `drift`.
  - `a_failure_and_recovery_of_the_list_are_recorded`: ok by `snapper
    list`, `--source pacman` while snapper fails (nothing), then the
    failure and the recovery, each once.
  - `no_change_without_an_earlier_run_here`: lost state directory; bare
    entry then a degraded run.
- `engine/src/commands/doctor.rs` `tests::a_waiting_row_names_each_reason`
  (unit): plural, mixed reasons, the `--logbook` capture in the fix.

## Mutants

Method as WP-088: a script in my scratchpad applies each mutant to the
committed tree, runs `cargo test --no-fail-fast --test idempotency
--test doctor --lib`, then restores the file with `git checkout HEAD --`
and `touch`. All 24 failed on test assertions, none on a build error (S7
and S8 first did not compile and were rewritten).

| # | Mutant | Killed by |
|---|---|---|
| N1 | not-run collectors never marked | `…not_run_in_the_reset…`, `…disabled_collector…` |
| N2 | not-run with an entry marked too | `…not_run_keeps_its_entry`, `…degraded_in_the_reset…` |
| N3 | marks skip the ledger rule | 4 incl. `…not_run_in_the_reset…`, `…enabled_later_is_no_reset` |
| N4 | bare entry gets a `lastRun` | `…not_run_in_the_reset…`, `no_change_without_an_earlier_run_here` |
| N5 | bare entry `ok: false` | `…not_run_in_the_reset…` |
| N6 | bare mark always `cursors` | `…disabled_collector…` |
| N7 | `lastRun: null` written | `…not_run_in_the_reset…` |
| N8 | `lastRun` required on read | 4 incl. `…not_run_in_the_reset…` |
| N9 | bare entries not saved | `…not_run_in_the_reset…`, `…disabled_collector…` |
| N10 | index invents a `lastRun` for a bare entry | `…not_run_in_the_reset…` |
| D1 | no split (marks in the "next capture" row) | 4 incl. `…mark_of_another_logbook…` |
| D2 | marks counted under any binding | `…mark_of_another_logbook_is_no_waiting_row_here` |
| D3 | `logbook` mark worded as `cursors` | unit, `…degraded_when_the_state_was_another_logbooks…` |
| D4 | fix without `--source` | unit, `…degraded_in_the_reset…` |
| D5 | mixed reasons collapsed | unit |
| D6 | always singular | unit |
| S1 | no access note | 6 incl. both `snapper_access` direction tests |
| S2 | no `lastRun` guard (bare entry compared) | `no_change_without_an_earlier_run_here` |
| S3 | only degraded → ok | `a_grant…`, `a_failure…` |
| S4 | only ok → degraded | 6 incl. `a_grant…`, `a_failure…` |
| S5 | a note for every collector | `a_first_successful_run_after_a_degraded_init_is_no_reset` |
| S6 | old and new message swapped | `a_grant…` |
| S7 | the run's own message dropped | `a_grant…` |
| S8 | the note carries a case | `a_grant…` |

## Verified by

- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`: clean.
- Full `cargo test --no-fail-fast`: 0 failures (idempotency 34, doctor
  31, lib incl. the new unit test).
- `bash scripts/docs-check.sh`: ok (its existing warning on
  `de/06-configuration.md` is not from this WP).
- `flock /tmp/seldon-check.lock just check`: exit 0, run once after the
  last mutant, ending with `check: ok`. Plugin harness results:

  | Suite | Passed | Failed |
  |---|---|---|
  | bar-view | 143 | 0 |
  | panel-view | 771 | 0 |
  | overlay-view | 319 | 0 |
  | service-states | 297 | 0 |
  | install.test | 209 | 0 |
  | real-home-guard | 11 | 0 |
  | model.test.js | 88 | — |

## Decisions needed (none blocking)

1. **Which degraded states count for the note.** I followed the WP
   literally: any change of `ok` (permission, `snapper` missing, a
   timeout, unexpected output). The detail always carries the message,
   so the history says which. A snapper that times out once writes two
   notes. Alternative: count only `NO_PERMISSIONS` against ok, a
   two-line change in `access_change` (then a timeout between two ok runs
   is silent, and ok → timeout → no permission is never recorded).
2. **Downgrade.** An engine before WP-091 requires `lastRun`, so it
   cannot load a `cursors.json` that holds a bare entry: every capture
   fails (exit 2) and doctor offers `mv cursors.json …bad`, which then
   records a state reset. Only after a downgrade and only while a mark
   waits. Alternatives: give the bare entry a `lastRun` (the index would
   then claim a run that did not happen) or keep marks of not-run
   collectors in a separate top-level map in `cursors.json` (old engines
   ignore it and drop it on save, losing the mark). I chose the bare
   entry, as the WP's design note says.
3. **User guide.** `10-troubleshooting.md` (en and de) explains the
   "next capture will record a state reset" row but not the new waiting
   row. A small docs follow-up (en + de translation).
4. **A disabled collector's mark is not in doctor.** `pending_reset`
   covers enabled collectors only (WP-083), so doctor shows the row once
   the collector is enabled again. Accept, or have doctor mention
   disabled marks too.

## Notes

- No guard block happened.
- The real host was not touched; all runs used the tests' scratch HOME
  and stub programs. `redact.rs`, `hook.rs`, `snapper.rs` and the plugin
  are untouched.
- A crash between the ledger append and the cursor save repeats the
  access note on the next capture, as it repeats a state-reset note;
  same accepted limitation.

---

# Round 2 (stage 1 APPROVE with fixes, stage 2 APPROVE after this round)

Commits (oldest first), no rebase:

- `8fcc111` engine: round 2 wording, crash limitation, reset-row test (WP-091)
- `592f833` docs: guide 10 (de) names the waiting row (WP-091)
- this commit: handover round 2

## What changed

- **F1 (SPEC only).** SPEC §3, after "a loss is recorded once": the
  known limitation, in the brief's words. A crash between the ledger
  append and the `cursors.json` save repeats the `state-reset` note and
  the snapper access note. Collector events are not repeated. §4's
  snapper paragraph now says "each change is recorded once (except after
  a crash …, §3 state reset, known limitation)". There is no ledger
  dedup (WP-099).
- **F2.** `a_collector_not_run_in_the_reset_records_its_gap_when_it_runs`
  now asserts, right after the bare entry is written, that doctor has
  exactly one "the last capture recorded a state reset: " row and that
  it starts with `pacman took a new baseline (`. Mutant **R2** (a missing
  `lastRun` read as `2999-01-01T00:00:00+00:00` in `check_reset`) is
  **killed**. It fails at that assertion (`idempotency.rs:1225`, `left:
  0`): the row is gone, and only the waiting row is left. lib, doctor and
  the other idempotency tests stay green under R2.
- **F3.** The note now says "at its last run" instead of "at the last
  capture". The change is in `capture.rs` (3 strings), SPEC §4 and the 4
  quoting assertions in `idempotency.rs`. TESTING.md does not quote the
  detail.
- **Decision 2.** There is a CHANGELOG line under Unreleased → Engine,
  and `docs/VERSIONING.md` has a "Downgrading the engine is not
  supported" paragraph at the end of "What the numbers promise". Both use
  the brief's text.
- **Decision 3.** In guide 10, the `state` row's degraded clause names
  the waiting row: en with the brief's text, de with the brief's
  translation. The de source line is now `en/10-troubleshooting.md @
  8fcc111`, the commit that holds the en change. docs-check reports no
  translation warning for page 10.
- D1 and D4 are as decided; there is no code change.

## Verified by

- `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings`
  are clean. Full `cargo test --no-fail-fast` has 0 failures
  (idempotency 34). `bash scripts/docs-check.sh` reports ok. Its only
  warning is the existing `de/06-configuration.md` one.
- `flock /tmp/seldon-check.lock just check`: **exit 0 on the 4th run**,
  ending with `check: ok`. Results: bar-view 143/0, panel-view 771/0,
  overlay-view 319/0, service-states 297/0, install.test 209/0,
  real-home-guard 11/0, model.test.js 88.
- The first three runs failed for reasons outside this WP. Nothing was
  changed between the runs.
  - **Runs 1 and 2:** `service-states` FAILED in the real-home guard with
    "`~/.local/state/seldon/agent-launch.log` changed".
    - Cause: the operator's installed plugin launched real agents during
      the runs. At 15:10:21, `ps` shows `foot
      --app-id=org.omarchy.agent -e claude … case C-2026-011 … at
      ~/Seldon`. The run-1 launch at 14:47:47 lies in the same kind of
      window. `seldon agent` writes the launcher's stderr there (foot's
      `xdg-toplevel-icon` warnings).
    - The guard allows `index.json`, `lock`, `cursors.json` and
      `manifest.json` from the operator's live engine, but not
      `agent-launch.log`. So a real agent launch during a check fails
      it.
    - `plugin-test` stops there, so bar, panel and overlay did not run
      in those two.
    - I did not touch the operator's agent.
  - **Run 3:** `check-watch` FAILED in
    `watch::with_feature::rss_stays_under_10_mb_on_the_x10_fixture`
    (debug build): growth 6296 kB against the 6144 kB bound, at load
    average 6.
    - The same test passed in runs 1 and 2 and in round 1, and 3 times
      alone afterwards.
    - This WP's only index change is round 1's one-line `and_then`,
      which was also in all those green runs.

## Notes for the orchestrator

- **Real-home guard gap (not fixed, not in scope):**
  `tests/plugin/real-home-guard.sh` `real_engine_files` lacks
  `agent-launch.log`. On a dev host whose operator launches agents from
  the plugin, the check fails whenever a launch falls into a
  `service-states` run.
  - Proposal for a small follow-up: allow `agent-launch.log` to grow
    (size only up, same path), like the other engine files.
- **The watch RSS bound in debug builds** has about 150 kB of headroom
  under load. It is worth watching if it fails again.
- No guard-hook block happened. The real host was not touched.

---

# Round 3 (gate: `watch::with_feature::rss_stays_under_10_mb_on_the_x10_fixture`)

No code change in this round. The test, its bound and the engine are
unchanged. This section is the measurement the orchestrator asked for,
and a proposal.

## Method

- **Builds.** One debug `seldon` binary per commit, `--features watch`,
  the same profile `cargo test` uses:
  - `886d876` (base; its `engine/` is identical to main's, and `git diff
    886d876 main -- engine` is empty);
  - `06d39f3`, `9abf2b9`, `fbc6caf`, `8f44f21`, `bda0177`.
  
  Each was built from a `git archive` copy with its sources touched,
  because `git archive` mtimes otherwise make cargo reuse the previous
  build. I checked by hash that the six binaries differ.
- **Harness.** One copy of the test binary measured each `seldon` through
  `SELDON_WATCH_BIN`. I also added a scratch-only `eprintln!` of
  `RssAnon`, `RssFile` and `RssShmem` from `/proc/<pid>/status` at idle
  and after the rebuild. The test's own line still gives idle, rss and
  peak. `VmRSS = RssAnon + RssFile + RssShmem`.
- **Runs.** Every run held `flock /tmp/seldon-check.lock`. The binaries
  were measured interleaved, round by round, so that load drift hits all
  of them alike. Load was about 3.5.
- **Growth** is `peak − idle`, as in the test (the bound is 6144 kB).

## Numbers

**Bisect, binaries on disk** (`engine/target`, as in `cargo test`).
6 interleaved rounds per binary:

| commit | engine change | growth mean (sd) | heap `RssAnon` growth | file pages `RssFile` growth | idle mean |
|---|---|---|---|---|---|
| `886d876` (= main) | — | 4320 (180) | 1836 | 2484 | 16391 |
| `06d39f3` | N2 | 3307 (144) | 1836–1840 | 1471 | 17753 |
| `9abf2b9` | doctor row | 3310 (220) | 1836–1840 | 1473 | 17915 |
| `fbc6caf` | snapper note | 3641 (184) | 1836 | 1805 | 17457 |
| `8f44f21` | **none** (tests only) | 2882 (94) | 1836–1840 | 1045 | 18334 |
| `bda0177` | 3 strings | 4472 (125) | 1836 (one run 664) | 2668 | 16239 |

**Base against branch, on disk.** 10 more interleaved rounds:

| commit | growth mean | max |
|---|---|---|
| `886d876` | 4247 | 4732 |
| `bda0177` | 4563 | 4884 |

In both, heap growth is 1836 kB in every run but one (`bda0177`, 804 kB),
and the whole difference is `RssFile`.

**Base against branch, binaries on tmpfs** (there the binary counts as
shmem). 10 rounds:

| commit | growth | mean | over the bound |
|---|---|---|---|
| `886d876` | 5808–6192 | 6000 | 1 run of 10 (6192 kB) |
| `bda0177` | 5616–6064 | 5853 | none |

Heap growth was 1840 kB for both. An earlier 5-round tmpfs pass over all
six commits gave means between 5859 (base) and 6026, with the base itself
at 6128.

## Where the ~0.5 MB comes from

- **Not the heap.** The rebuild allocates the same 1836 kB on main and on
  every commit of this branch, to the kB. No code in the rebuild path
  changed. `CollectorState`'s `last_run: Option<String>` has the size of
  a `String` (niche), the test has no `cursors.json`, and the index's
  one-line `and_then` allocates nothing new.
- **It is the mapped binary.** The debug `seldon` is 110 MB, and its file
  pages are mapped by page faults with fault-around. How many are mapped
  before the test reads "idle" varies with the file's layout and its
  page-cache state. The peak is almost constant (idle + growth ≈
  20.7–21.2 MB in every row above). The variable is the *idle* baseline,
  and growth = peak − idle inherits it.
- **The proof is the tests-only commit.** `fbc6caf` and `8f44f21` have
  no `engine/src` difference, and equal `.text` (14 161 053 B) and
  `.rodata` (758 632 B). Their binaries differ only in the embedded build
  path and the build ID. Yet their mean growth differs by 760 kB (3641
  against 2882), with the idle moving the other way (17457 against
  18334).
  
  That spread is larger than the main-to-branch gap the orchestrator
  measured. Builds from different worktree paths (main vs `wt/WP-091`)
  differ the same way. On tmpfs, main itself went over the bound once
  (6192 kB).
- **So the growth is bounded and not caused by WP-091.** In the test
  profile the metric has about ±0.5–0.8 MB of build-dependent noise
  against about 0.2–0.5 MB of headroom on main. Any engine change can
  tip it. That fits round 2's run 3 and the gate.

## Proposal (orchestrator's go needed; not done)

The bound stays at 6144 kB. In the debug branch of the test, apply it to
the growth of `RssAnon` (the heap; deterministic, 1836 kB today) instead
of `VmHWM − VmRSS(idle)`, which is mostly mapped code. The absolute
`VmHWM < 10 MB` bound for the optimised binary (`check-watch`'s bench
run, `SELDON_WATCH_BIN`) is unchanged. That is about 5 lines in
`tests/watch.rs`, a file outside this WP. It measures what the PLAN bound
is about (memory the rebuild allocates) and does not loosen it. The
alternative, run the debug check only as advisory, is weaker.

## Verified by

- The measurements above. Raw data: m1–m4 in my scratchpad `r3/`.
- No engine or test file changed in this round, so round 2's green
  `just check` and mutants stand. The gate failure is this test only.
- No guard block. The host was not touched. The scratch binaries in the
  gitignored `engine/target/r3` were removed.
