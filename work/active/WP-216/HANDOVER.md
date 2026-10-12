# WP-216 — Handover

Branch `wp/216-test-leftovers` (from `next` at `9a9f5f7b`), worktree
`wt/WP-216`. Not merged. Role: Engine Dev. Tests, the `check-perf`
recipe and docs only: no engine source, plugin, schema or budget change.

## What was done, per item

### 1. check-perf: stale directories, SIGTERM, an interrupted run

- `justfile` `check-perf`: at the start, `find` removes this user's
  `seldon-perf.*` directories older than a day (`-maxdepth 1 -type d
  -user $(id -u) -mmin +1440`) in the parent; no run takes a day, so a
  run in progress is never swept. `trap 'exit 130' INT` and
  `trap 'exit 143' TERM` (was `exit 130` for both). The parent is
  `SELDON_PERF_SHM`, default `/dev/shm` (only the test sets it).
- New `tests/perf/check-perf.test.sh`, run by the new recipe
  `check-perf-dirs` (in `check`): `just check-perf` with a stub `cargo`
  and a scratch parent, in its own process group (`setsid`), own `HOME`
  and a private 0700 `XDG_RUNTIME_DIR` (removed with the scratch dir).
  14 rows: a passing run removes its directory; SIGINT to the group exits
  130, SIGTERM 143, the directory removed; SIGKILL exits 137 and leaves
  it; the next run keeps it while young, and sweeps it once older than a
  day, with another stale one, and keeps a 23-hour one, another name and
  a file of the same pattern.
- A signal to the recipe's bash alone (not its group) runs the trap only
  after the foreground `cargo` ends: bash semantics, unchanged, and what
  a terminal's Ctrl-C or a `kill -- -<pgid>` does not hit.

### 2. check-perf: the free-space floor

- Measured: the directory peaked at **2104 KiB** (`du -sk` every 0.2 s,
  194 samples, a whole `just check-perf` run, dev host, 2026-10-12).
- Floor set to **16 MiB** (was 256): about 8× the peak, for a peak
  between two samples and for larger scale fixtures; it fits a
  container's default 64 MiB `/dev/shm` (review Q1). Reason in the
  recipe comment and in TESTING.md.

### 3. The mount-table lookup

- `common::file_system_of` now reads the table and calls the new
  `common::file_system_in(table, path)`; behaviour unchanged.
- New test `the_last_mount_on_a_mount_point_wins` (`tests/capture_cost.rs`)
  on a synthetic mount table: two mounts on one point, both orders; the
  deepest point over a shallower one, both orders; a deeper mount below
  the overmount; a sibling with a common string prefix
  (`/srv/stateful`); an escaped space (`\040`); no match.

### 4. The 1 ms row `hook session-start, unrelated session, 10 000 lines`

No budget moved. A profile and a question.

Profile (scratch test, not committed; private
`gates/dev-216/wp216-profile-test.patch`): the same test home as the row
(10 000 ledger lines, an active case), 60 interleaved calls per variant,
process start included, bench profile, dev host, load ≈ 0.7.

| Variant | Median | p25–p75 |
|---|---|---|
| A `hook claude-code` (WP-092 fast path, no clap), unrelated | 756 µs | 746–776 µs |
| B `--quiet hook claude-code` (the same work through clap) | 974 µs | 963–993 µs |
| C `hook session-start`, `cwd` `/` (the row's payload) | 964 µs | 949–975 µs |
| D `hook session-start`, `cwd` `/srv/made-up-project` | 964 µs | 954–979 µs |
| E `hook generic`, unrelated | 963 µs | 950–974 µs |
| F `hook claude-code` fast path, a `Notification` event (returns before the logbook is opened) | 672 µs | 661–689 µs |
| G the same through clap | 902 µs | 892–918 µs |

Reading:
- **The cost is clap.** `Cli::try_parse_from` builds the parser of every
  command: B − A = 218 µs, G − F = 230 µs. `hook claude-code` skips it
  (`main.rs`, `is_plain_claude_code_hook`, WP-092); `hook session-start`
  and `hook generic` do not, so they sit at B's level (C, D, E ≈ B).
- The scope check itself (open the logbook, compare the directory) is
  A − F ≈ 84 µs; `cwd` `/` costs nothing extra (C = D).
- Process start + context is ≈ 0.67 ms of the 1 ms.
- In the same `just check-perf` run the rows passed narrowly: claude-code
  779 µs, session-start **979 µs**, generic 965 µs (budget 1 ms). On
  WP-210's runs session-start missed at 1.050/1.023 ms (generic was not
  reached: the test stops at the first failing row). So 2–5 % either
  side of the budget, decided by host noise.

**Question for the orchestrator:** extend the WP-092 fast path to
`seldon hook session-start` and `seldon hook generic` (argv exactly
`[_, "hook", <name>]`, plus `generic` without `--case`) in an engine WP:
about 0.22 ms saved, which puts both near claude-code's ≈ 0.76–0.78 ms.
Or a spec decision on the 1 ms budget of those two rows. This WP does
neither.

### 5. The real-home guard (WP-212 review)

- `tests/plugin/real-home-guard.test.sh`:
  - FAIL rows for `agent-launch.log` and `hooks-user-wide` changed during
    a run, and for a triage proposal rewritten in place
    (`proposals/p1.json`, temp file + rename). With the existing
    `launches-changed` and `hooks-changed` every not-routine name has one.
    `guard_case`'s seed list takes `dir/file` now.
  - `routine-list`: `real_engine_files` must be exactly the seven names
    (sorted, literal in the test), so a name moved into or out of it
    needs a test edit.
  - The drift scan reads with `grep -rHzoP 'state_dir\s*\.join\([^)]*\)'`,
    so a rustfmt-split chain (`dirs\n.state_dir\n.join(\n"x",\n)`) is
    seen, trailing comma included. New row `drift-new-split` (a copy of
    `engine/src` with such a join) fails as wanted. On the real tree both
    scanners find the same 15 joins and 12 names.
- 79 passed, 0 failed (was 71).

### 6. service-states.sh index wait (WP-205 review, Q3)

- `tests/plugin/harness/shell.qml`: `HARNESS_WAIT_INDEX`. The actions and
  the final snapshot also wait until the service shows an index
  (`indexShown`, as desk.qml), within the existing grace period; a
  timeout logs `HARNESS wait timed out: index`, and `run()` in
  service-states.sh fails the run on that line.
- `service-states.sh`: 23 live runs (no `SELDON_INDEX`) without
  `HARNESS_UNTIL=status=ok` got `HARNESS_WAIT_INDEX=1` (22 kept it); 5 opt out with
  `# no index wait: <reason>` (`engine-fresh`, `actions-noengine`: no
  engine; `live-uninit`, `xdg-relative`: no logbook, so no index;
  `capture-locked`: its `["snapshot"]` action runs during the locked
  start-up capture, before any index, on purpose; the first full check
  failed it with the wait, see below). The
  5 `status=ok` runs count as waiting (status ok needs the index). The
  sheets harness already waits for an index in its `idle()`.
- `tests/plugin/live-index-wait.py` checks both harnesses (no argument)
  or the named files (`service-states.sh` by name gets its rules): a
  live service run is a `run <case> <ms>` without `SELDON_INDEX=`; it
  must hold `HARNESS_WAIT_INDEX=1` or `HARNESS_UNTIL=status=ok`, or the
  marker. Self-test mutants per file: each wait kind removed, a marker
  removed, a new live run with neither; all caught. desk-view: 71 wait,
  12 opt out (unchanged); service-states: 27 wait, 5 opt out.

## How it was verified

- **fixture** (dev host, scratch dirs under the private `gates/dev-216`):
  - `real-home-guard.test.sh` 79/0. Mutants in scratch copies: the
    review's M4 (`agent-launch.log`, `hooks-user-wide` moved to routine)
    now fails 3 rows (`agent-launch.log-changed`,
    `hooks-user-wide-changed`, `routine-list`); the drift scan without
    `\s` across lines fails `drift-new-split`.
  - `check-perf.test.sh` 14/0. Mutants of the recipe in scratch copies:
    the old `trap 'exit 130' INT TERM` fails `sig-TERM-exit` (130, want
    143); no sweep fails `sweep-left`; a sweep without the age limit
    fails `kill-young-kept` and `sweep-left`.
  - `the_last_mount_on_a_mount_point_wins` passes; the mutant "first
    mount wins" (`depth > *d`) fails it (`ext4`, want `tmpfs`); source
    restored.
  - `live-index-wait.py`: 0 problems, all 7 self-mutants caught.
  - shellcheck (orchestrator kit): `check-perf.test.sh` clean; the
    changed `real-home-guard.test.sh` and `service-states.sh` have the
    same 18 findings as on the base, none new.
- **desktop** (dev host, under `flock /tmp/seldon-check.lock`):
  `just check-perf` on the branch, exit 0, load ≈ 0.7 at the end
  (private `gates/perf-dev-216-head.log`); the profile above ran in the
  same run.
- **headless** and the rest: the full check, see below.
- **CI**, **test host**: not run.

- **Full check** (desktop, dev host; the Quickshell harnesses
  **headless**), `SELDON_FULL_CHECK=1 flock /tmp/seldon-check.lock just
  check`:
  - Round 1 at `793d2f2c`: exit 1. One row failed:
    `capture-locked: while waiting` (service-states, 365/1). Its
    `["snapshot"]` action is meant to run during the locked start-up
    capture, but the new wait moved it past the index. Fixed in
    `0b3707b2` with an opt-out marker that gives the reason.
  - Round 2 at `0b3707b2`: **exit 0, `check: ok`**, 0 `FAIL` lines, no
    `wait timed out`. service-states 366/0, desk-view 2451/0, bar-view
    195/0, ipc-restart 43/0, real-home-guard.test 79/0,
    check-perf.test 14/0, live-index-wait 0 problems. desk-view reports
    "the real ~/.local/state/seldon changed by the operator's live
    engine (not a leak)". That is the guard's allowed case. The log is in
    the private `gates/check-dev-216-r2.log`.
  - Later commits change only this HANDOVER.

## Not done / open

1. The 1 ms rows: the question in item 4.
2. `check-perf` is still not a CI step (review Q1); with the 16 MiB
   floor it would no longer refuse Docker's default `/dev/shm`.
3. The service-states wait has no deterministic delay like desk-view's
   `FAKE_SELDON_FIRST_WRITE_DELAY`: there the gap is after the
   `status` call returns (the file watch), and a delay inside `status`
   only lengthens the call the harness already waits for. The guard and
   the timeout failure are the proof; a fake engine that writes the index
   after `status` returns would be a later option.
