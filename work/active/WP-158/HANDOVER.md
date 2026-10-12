# WP-158 — Handover

Branch `wp/158-lock-test` (from `next` at `3ba46b8f`), worktree
`wt/WP-158`. Not merged. Role: Engine Dev. A test fix plus module docs;
no behaviour change in the engine.

## The cause (confirmed)

The flock of `logbook::lock` belongs to the open file description, and
the kernel releases it only when the last descriptor of that description
is closed. The lock file is opened with `O_CLOEXEC` (Rust's default), but
`CLOEXEC` acts at `exec`, not at `fork`: a child another thread forks (or
`posix_spawn`s, which is a `clone` with `CLONE_VFORK`; only the spawning
thread waits, the others run on) gets a copy of every descriptor of the
process and holds it until its `exec`. A `drop` of the lock in that
window does not release it, and an immediate `acquire` gets `LockHeld`.
The lib tests run in parallel threads of one process, and many of them
spawn programs (`sys::run_command` with `process_group(0)`, the git
runner, the collectors' stubs). `sys::run_with` already retries
`ETXTBSY` for the same reason (a written stub still open in a forked
child).

Proof (a throw-away test in `lock.rs`, not committed; dev host): for
5 s, `acquire` / `drop` / `acquire` in a loop on one thread.

| other threads | attempts | re-acquire refused |
|---|---|---|
| none | 914 541 | 0 |
| 4 threads spawning `true` with `process_group(0)` in a loop | 1 052 087 | 365 360 |

So the old test fails whenever another test is between `fork` and `exec`
at its `drop`; on a quiet host that window is microseconds, which is why
it failed only once (gate-124b, loaded host).

## Can the engine itself hit it? No

- The engine starts its programs from the thread that holds the lock, and
  `Command::spawn` returns only after the child's `exec` (glibc's and
  musl's `posix_spawn` wait for it with `CLONE_VFORK`; Rust's fork path
  waits on its `CLOEXEC` error pipe). When the engine drops the lock,
  none of its children still has the descriptor.
- The threads the engine starts never fork: the output pipe readers of
  `sys::run_with` (`Drain`) and the `notify` watcher thread of
  `seldon watch` (feature `watch`). Every other `thread::spawn` under
  `engine/src` is inside `#[cfg(test)]` (`sys.rs`, `logbook/git.rs`,
  `index/mod.rs`, …).
- Another `seldon` process has its own file description; its children
  never hold this process's lock.

So no production change. Switching to POSIX `fcntl` locks (per process,
not inherited by a child) was considered and rejected: two locks in one
process do not exclude each other, closing any descriptor of the file
drops the lock, it needs `libc` FFI, and it does not exclude an older
`seldon` that still takes the flock. The invariant ("spawn from one
thread; threads the engine starts never fork") is written down in the
module docs of `engine/src/logbook/lock.rs`, so a later change that
forks from a second thread sees it.

## What was done

- `engine/src/logbook/lock.rs`, test
  `second_lock_is_refused_until_release`: the re-acquire after `drop`
  goes through `acquire_within`, which retries `LockHeld` every 2 ms for
  up to 10 s and returns any other error at once. The refusal while the
  first lock is held stays a single `acquire` (that one is
  deterministic). Normal case: the first retry is not needed.
- Module docs of `lock.rs`: why the lock can outlive a drop in a
  multi-threaded process and why the engine never sees it.
- Other tests that drop the lock and expect a writer to proceed
  (`tests/hooks.rs`, `tests/watch.rs`, …) run the engine as a separate
  process, which waits for the lock itself (the hook's and the watcher's
  retries), so they already tolerate the window. No change.

## How it was verified

- **fixture** (dev host, the lib test binary of `8c6da365` and of
  `3ba46b8f` built with `CARGO_TARGET_DIR` under the private gates folder
  and run directly; each run with its own scratch `HOME`, XDG dirs and
  `TMPDIR` and its own `XDG_RUNTIME_DIR` (0700), all removed afterwards):
  - **Acceptance, 200 runs: 200 passed, 0 failed.** Load: no busy loops;
    two streams of the lib tests run side by side (100 runs each, each
    run 417 tests on the harness's 16 threads, so the other stream's
    spawning tests add fork/exec pressure), on a host already at load
    36 / 35 / 26 (1 / 5 / 15 min, 16 cores) from other agents' gates;
    29 / 28 / 25 at the end. 6 min 38 s; one run took 2.3 to 8.0 s.
  - First attempt, stopped on the orchestrator's request: 16 `yes` busy
    loops (one per core). The old binary passed 30 of 30 runs there (the
    window is too short to hit the test itself often); the fixed binary
    passed 185 runs before the loop was stopped (counted as an aborted
    extra run, not as the acceptance). The busy loops were killed with
    the loop; none was left running.
  - The cause itself: the table above (throw-away test, not committed).

- **CI**: not run (I did not push).
- **test host**: not run (an engine test fix; nothing host-specific).

## Open points

- None for the engine. If a later WP adds a thread that spawns programs
  while another thread may drop the lock, the module docs say what to
  do (spawn from the lock's thread, or wait for the re-acquire).

## just check

JUST_CHECK
