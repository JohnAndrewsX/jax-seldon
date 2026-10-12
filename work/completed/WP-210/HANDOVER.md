# WP-210 — Handover

Branch `wp/210-check-perf` (from `next` at `c4b5c13f`), worktree
`wt/WP-210`. Not merged. Role: Engine Dev. Tests, the `check-perf`
recipe and docs/TESTING.md only; no engine change, no budget changed.

## The three medium-bound rows, and why

From WP-157's paired base/head logs (`check-perf` with the gates' disk
`TMPDIR`, btrfs), the three non-index rows that failed on base and on
every branch:

| Row | Budget | Medians on the btrfs disk (base) | What it syncs |
|---|---|---|---|
| `hook, recorded (tmpfs), 10 000 lines` (`hooks.rs`, `fast_enough_at_10_000_ledger_lines`) | 5 ms | 13.0–19.7 ms | the ledger line (`ledger::append_file`, `sync_data`) and the case file (`sys::write_atomic`: `sync_all` of the file and of its directory) |
| `hook, recorded (tmpfs), 900 lines` (`fast_enough_just_below_the_rebuild_threshold`) | 5 ms | 26.8–32.2 ms | the same, plus `index.json` (the rebuild below the 1000-line threshold, also `write_atomic`) |
| `recent-config scan and save (the capture-cost delta)` (`capture_cost.rs`, `capture_cost_of_the_recent_config_scan`) | 10 ms | 11.0–14.7 ms | the saved scan (`recent::Saved::save` → `write_atomic`) |

Each of these writes, then waits for the disk to confirm it, before
returning. SPEC-ENGINE §1 asks for exactly that ("returns only after its
ledger line and the updated case file are synced to disk") and §8 states
the hook budget "with the state on tmpfs". On a disk the sync alone is
longer than the budget, so the row measured the disk, not the engine.
The two other recorded hook rows of the same tests (`curl line with a
marker`, `curl line with -e and -am`) sync the same way. They never ran
on the disk in those logs because the test stops at its first failed
row. They move to the tmpfs with their test.

## What was done

- `justfile`, `check-perf`: now a bash recipe. After the bench, it
  creates a directory of its own on the tmpfs `/dev/shm`
  (`mktemp -d /dev/shm/seldon-perf.XXXXXX`; never `/tmp`, never the
  runtime dir). It refuses to start when `/dev/shm` has less than
  256 MiB free, passes the directory to the tests as
  `SELDON_PERF_TMPFS`, and removes it on exit (`trap … EXIT`; INT/TERM
  exit through it).
- `engine/tests/common/mod.rs`:
  - `sync_timing_dir()` returns `SELDON_PERF_TMPFS`, or `TMPDIR` when
    unset, and panics unless that directory is on a tmpfs
    (`assert_on_tmpfs`). The panic message names the directory and
    says that `just check-perf` makes the tmpfs. A synced-write row is
    therefore never decided by a disk: it fails with that reason
    instead of a timing.
  - `file_system_of(path)` reads `/proc/self/mountinfo`: the deepest
    mount point above the canonical path wins (the last one mounted
    when two share it), and octal escapes are decoded.
  - `TempDir::new_in(base, tag)`.
  - `Env::new_in(snapper, base)`: home, logbook and state under
    `base`, stub programs in a second temp dir under `TMPDIR`, since a
    container's `/dev/shm` is usually `noexec`.
- `engine/tests/hooks.rs`: `Hooks::on_tmpfs()`. The 10 000-line and
  900-line timing tests use it. `fast_enough_for_an_unrelated_session`
  writes nothing and stays in `TMPDIR`.
- `engine/tests/capture_cost.rs`: in `capture_cost_of_the_recent_config_scan`
  only the state directory moves to the tmpfs, and the row's label
  gains `, tmpfs`. The synthetic home (~25 MB of files) stays in
  `TMPDIR`. The row `recent-config in the index build (80 files)`
  (2 ms) reads the saved file back from the same state directory. It
  is read-only, so on the page cache either way, and its budget is
  unchanged.
- New, not ignored (runs in `just check`):
  `a_synced_write_is_timed_on_a_tmpfs_only`. `/proc/self/fd` reads as
  `proc`, `/sys/kernel` as `sysfs`, `/proc` is refused with the
  message, and `/dev/shm` is accepted where it is a tmpfs.
- docs/TESTING.md, "Other recipes": names the rows, why they are
  medium-bound, the numbers, the tmpfs directory, its size and the
  refusal.
- No budget changed. Every other row runs in `TMPDIR` as before.

## Acceptance, item by item

- `just check-perf` green on the dev host with the gates' disk
  `TMPDIR`: **partly**. It was green once (load ~10). On the final
  head the three medium-bound rows passed, but the run failed on the
  non-medium session-start row (see "Open: the unrelated-session row").
- On CI: **not run** (open question 1).
- With a tmpfs `TMPDIR`: **green** (one run, final test code).
- A mutant that makes one row slow still fails it: **yes**, two
  mutants (slow save, slow ledger append), plus the mutant of the
  medium check.
- Evidence words: below.

## How much it writes to /dev/shm

- `check-perf`'s own directory: **about 2.1 MiB at its peak** (sampled
  every 0.2 s with `du` during the green disk run). That is one hook
  home with 10 000 ledger lines, the case file and the index, plus the
  saved recent-config scan. It is removed at the end, on success and
  on failure: after every run listed below, `ls /dev/shm` showed no
  `seldon-*` entry.
- For the acceptance run with `TMPDIR` itself on tmpfs (every row then
  writes there), I used a second directory
  `/dev/shm/seldon-dev-210-tmp.*` and removed it afterwards. Peak
  growth of the whole of `/dev/shm` during that run: **78 MiB**
  (80 008 KiB over a 1 300 KiB baseline, `df` every 0.1 s, under a
  watchdog that would have stopped the run at 190 MiB). That figure is
  an upper bound for this check, since `df` counts everything on
  `/dev/shm`. The watchdog was attached about 20 s late: my first
  attempt used `setsid`, which forked, so the loop ended at once. I
  attached it to the run's process group while cargo was still
  compiling, before any test wrote.

## How it was verified

Every timing ran under `flock /tmp/seldon-check.lock`. Every test
process had its own `HOME` and a private `XDG_RUNTIME_DIR` (0700) under
the gates folder, and `CARGO_TARGET_DIR=gates/target-dev-210`. Logs
are in the private `gates/` folder (`perf-dev-210-*.log`,
`check-dev-210.log`).

- **desktop** (dev host), `just check-perf`, gates' btrfs `TMPDIR`,
  tests built from `d478bc0b`'s working tree (before the
  `assert_on_tmpfs` split and the refusal test; same check), load
  ~10: **green**, exit 0, every row on its first attempt. The three
  rows: 1.73 ms, 3.12 ms, 4.17 ms; the other recorded rows 2.67–4.64 ms.
  `perf-dev-210-disk.log`.
- **desktop**, `just check-perf`, `TMPDIR` on tmpfs
  (`/dev/shm/seldon-dev-210-tmp.*`), final test code: **green**, every
  row on its first attempt (index ×150 71.6 ms, `status` 53.3 ms, the
  three rows 1.85 / 3.52 / 3.25 ms, the unrelated-session rows
  0.72 / 0.94 / 0.99 ms). I did not get the exit code from this run
  (the `setsid` mistake above). The log has no `error:` line and every
  `test result:` is `ok`. `perf-dev-210-tmpfs.log`.
- **desktop**, `just check-perf`, gates' btrfs `TMPDIR`, final head
  `f44378f4`, load 0.5: **not green**. The three medium-bound rows
  passed (1.80 / 3.25 / 4.11 ms), but `hook session-start, unrelated
  session, 10 000 lines` missed 1 ms at 1.050 / 1.023 ms. See
  "Open: the unrelated-session row" below. `perf-dev-210-disk-final.log`.
- **desktop**, refusal: the four timing tests with `SELDON_PERF_TMPFS`
  pointed at the btrfs gates folder. All three synced-write tests
  failed with `… is not on a tmpfs: a synced write is timed on tmpfs
  only; just check-perf makes one (SELDON_PERF_TMPFS)` before timing
  anything. `perf-dev-210-disk-refused.log`.
- **desktop**, mutants (each applied, run, reverted with
  `git checkout`, tree clean after):
  1. `assert_on_tmpfs` returns at once → killed by
     `a_synced_write_is_timed_on_a_tmpfs_only` ("/proc taken for a
     tmpfs").
  2. 8 ms sleep in `recent::Saved::save`, full `just check-perf` →
     killed: `recent-config scan and save … tmpfs` medians
     [12.30 ms, 12.21 ms], budget 10 ms; recipe exit 101.
     `perf-dev-210-mutant-save.log`.
  3. 6 ms sleep in `ledger::append_file`, full `just check-perf` →
     killed: `hook, recorded (tmpfs)` medians [7.88, 8.06] ms at
     10 000 lines and [9.47, 9.66] ms at 900 lines, budget 5 ms.
     `perf-dev-210-mutant-append.log`.
- **desktop**, `flock /tmp/seldon-check.lock` with
  `SELDON_FULL_CHECK=1 just check` on `f44378f4`: **green** (`check:
  ok`, exit 0). It ran qmllint (52 files), plugin-validate,
  service-states 367/0, desk-view 2314/0, bar-view 196/0, ipc-restart
  44/0. shellcheck ran on the test host through the orchestrator
  kit's wrapper (8 calls). No real-home guard tripped.
  `check-dev-210.log`.
- `cargo clippy --all-targets -- -D warnings` and `cargo fmt --check`:
  clean (desktop).
- **CI: not run.** `check-perf` is not a CI step (docs/TESTING.md:
  opt-in, quiet host), and I did not push. What the recipe needs
  there: `/dev/shm` as a tmpfs with 256 MiB free. Docker's default
  `/dev/shm` is 64 MiB, so on CI's container the recipe would refuse
  to start with a clear message unless the job sets `--shm-size`, or
  the free-space floor is lowered to what the check really uses
  (~2 MiB, see above). The stubs stay off `/dev/shm`, so `noexec`
  does not matter. Open question 1.

## Open: the unrelated-session row

`hook session-start, unrelated session, 10 000 lines` (budget 1 ms,
ADR-0030 §1, process start included) is not medium-bound. It writes
nothing, and paired runs in one session at load ~12 (`TMPDIR` btrfs,
then tmpfs, three times) gave session-start 1.19/1.22, 1.18/1.18 ms on
btrfs and 1.13/1.37, 1.03/1.09, 1.07/1.30 ms on tmpfs. Both miss alike
(`perf-dev-210-unrelated-pairs.log`). On the dev host its medians
were 0.94–0.97 ms in the two green runs and 1.02–1.05 ms on the quiet
final run: it sits at its budget, so `check-perf` is decidable but
flips on this row. Per the WP I did not touch its budget. WP-159's
review run measured 0.89 ms. Whether `next` got slower here since then
I did not bisect.

Three more runs of that test alone on the final head (gates' btrfs
`TMPDIR`, load 3.5, 6.8 and 6.3): session-start missed every time
(1.019/1.047, 1.204/1.214, 1.296/1.221 ms). The claude-code row passed
on its first or second attempt (`perf-dev-210-unrelated-quiet.log`).
It also failed on base `3ba46b8f` in WP-157's paired log (1.288/1.315 ms).
This WP does not change that test's setup (`Hooks::new()`, `TMPDIR`).
So `check-perf` is **not reliably green on the dev host because of
this one non-medium row**. The rows this WP is about are green on
every run.

## The guard block (as asked)

My first build command used a wrapper script in my scratchpad
(`run.sh bash -c 'cargo fmt … && cargo clippy …'`, setting HOME, the
runtime dir and the temp dirs). The guard blocked it fail-closed:
"run.sh is not known to the guard and its arguments name bash". It
also broke the brief's rule of literal commands. Nothing in it ran. I
stopped and reported. The orchestrator approved the inline form
(`env HOME=<literal> XDG_RUNTIME_DIR=<literal> RUSTUP_HOME=… CARGO_HOME=…
CARGO_TARGET_DIR=<literal> TMPDIR=<literal> cargo …`, separate
`mkdir -m 700` and `rm -rf` of the runtime dir). The guard accepted
every command in that form, and the wrapper file was deleted. A first
`check-perf` attempt in that form exited 127 before running anything:
`just` is not in `/usr/bin` on this host. I used its literal path.

The orchestrator's note about the 0.2.0 upgrade and the shell
restarts (08:41–08:50): my full check ran later and did not fail on
the real-home guards, so no re-run was needed for that.

## Open questions

1. CI: should `check-perf` run on CI at all (today it does not)? If
   yes, either the job sets `--shm-size` of at least 256 MiB, or the
   floor becomes, say, 32 MiB. Today's floor is a margin I chose for
   a desktop whose compositor shares `/dev/shm`, not a measured
   need.
2. The session-start unrelated-session row (above): a follow-up to
   find out why it costs ~1 ms (profile the hook's scope check), or a
   spec decision on its budget. That is not this WP's to make.
