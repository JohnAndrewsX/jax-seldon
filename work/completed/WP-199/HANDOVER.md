# WP-199 — Handover

Branch `wp/199-init-git-wait` (from `next` at `d5ad7759`), worktree
`wt/WP-199`. From the WP-193 handover's open question (the doctor flake).

## What was found

`init` waits for every git it starts: each call goes through
`sys::run_command_in_engine_group`, which `try_wait`s the child until it
exits and then drains both pipes. The child that outlives `init` is not
the engine's: **git 2.55's `commit` runs `git maintenance run --auto
--quiet --detach`**, and that process detaches (daemonizes) even in a
fresh repository with nothing to do. Seen on this machine:

- `GIT_TRACE2_PERF` of one `git commit` in a fresh repository: the
  maintenance child enters the `maintenance/detach` region, its parent
  exits 0 after ~1 ms (the commit returns), and a second `exit` from the
  detached process follows.
- `inotifywait -r .git` around the same commit: after the commit's own
  writes, `objects/maintenance.lock` is created and deleted by the
  detached process.

So when `init` returns, a git may still be creating and removing
`.git/objects/maintenance.lock`; `remove_dir_all(.git)` in the doctor
test then meets a directory that is not empty, or a file that is gone
("something still wrote into `.git` while it was being removed"). Rare,
since the detached process lives about a millisecond, so it showed up
only under load. The engine cannot wait for that process: it is no child
of the engine any more.

## What was done

- **`engine/src/logbook/git.rs`**: `NO_BACKGROUND` =
  `-c gc.auto=0 -c maintenance.auto=false`, first in the argv of every
  call that writes (`init`, `add`, `commit`; new `write_command`, used by
  `run`). `maintenance.auto=false` stops the commit from starting
  maintenance at all (git checks it before it spawns the child);
  `gc.auto=0` covers git before 2.29 (`gc --auto` after a commit) and
  maintenance's gc task. Options on the command line beat the user's and
  the logbook's config. Read-only queries are unchanged: no query starts
  maintenance, and their argv keeps `--no-lazy-fetch` first (WP-154's
  tests and spec pin that). The module doc gets the rule.
- **Tests**
  - New `tests/git.rs` `init_leaves_no_git_running_in_the_logbook`: three
    `init`s with `GIT_TRACE2` (git's own trace of every git `seldon`
    starts and their children). It asserts that the trace saw the commits
    (`cmd_name commit`), that no `child_start` names `maintenance` or
    `gc`, and that right after each `init` no process has its cwd in the
    logbook (`/proc/<pid>/cwd`). **It fails without the fix** (fixture
    run with `run()` switched back to the plain command): the trace shows
    three `child_start[0] git maintenance run --auto --quiet --detach`.
    The `/proc` scan alone did not catch the ~1 ms process in that run.
    The trace check is the deterministic part.
  - Unit `a_query_never_reaches_the_network_and_a_commit_is_the_users`
    pins the commit's argv with the two options first.
  - `git_runs_in_the_engines_process_group`: its wrapper took `$1` as the
    verb and would now log `-c`. It now takes the first argument that is
    neither an option nor contains `=`, as the WP-154 test already does.
- **Docs**: SPEC-ENGINE (the git calls paragraph, WP-199 sentence),
  TESTING (`tests/git.rs` row), CHANGELOG (Engine).

## What was not done

- The doctor test itself is unchanged (no retry of the removal): the
  cause is fixed in the engine, so the test needs no workaround.
- The plugins collector's git calls (read-only queries in a plugin's
  clone) get no options: queries start no maintenance.
- The WP-154 test's `~/.gitconfig` with `maintenance.auto = false` /
  `gc.auto = 0` stays; it is harmless and also covers that test's own
  `git commit`.

## How it was verified

All runs on this desktop (dev host) with `CARGO_TARGET_DIR` on disk in
the private gates folder (`target-wp199`), `TMPDIR` and a private 0700
`XDG_RUNTIME_DIR` on disk under the private gates folder. No network. The
real `~/Seldon`, `~/.local/state/seldon` and `~/.config` were not
touched. git 2.55.0.

- *fixture*: the cause, by hand in a scratch repository on disk with its
  own `HOME`: `GIT_TRACE2_PERF` and `inotifywait` as above, before and
  after `-c gc.auto=0 -c maintenance.auto=false` (after: no
  `maintenance.lock`, no maintenance child).
- *fixture*: the new test fails without the fix and passes with it;
  `cargo test -j 4 --test git` 14/14; `--lib logbook::git` 6/6.
- *fixture*: **the doctor tests 50/50 under parallel load**: the
  `tests/doctor.rs` binary (all 39 tests, default test threads) run 50
  times in a row while a full `cargo test -j 4 --locked` of the engine
  ran alongside the whole time as load. 50/50 passed, and the load suite
  passed too (exit 0).
- *fixture* and *headless*: `SELDON_FULL_CHECK=1 just check` (build
  jobs 4): `check: ok`, exit 0. fmt, clippy, the engine tests with and
  without `watch`, packaging, docs-check, plugin-validate, qmllint; the
  offscreen Quickshell harnesses service-states 359, desk-view 2009,
  bar-view 196, ipc-restart 44 passed, 0 failed.
- shellcheck: *not run* (not installed); no shell script changed, only
  the inline wrapper string in `tests/git.rs`, which runs in the test.
  CI.

## Open questions

- None blocking. Side effect to know: with `gc.auto=0` on the engine's
  commits, Seldon never packs the logbook's loose objects on its own. A
  logbook gets one or a few commits per command; git commands the user
  runs there still auto-pack, and `git -C <logbook> gc` packs by hand
  (CHANGELOG says so). If that should be Seldon's job, a later WP could
  run a foreground `git gc --auto` (waited for, no detach) in `doctor
  --fix` or similar.

## Round 2 (review 1: APPROVE, two items folded in)

- **N3, no fsmonitor daemon.** `NO_FSMONITOR` = `-c core.fsmonitor=false`
  on every engine git call (`logbook/git.rs`). In a write it comes after
  `NO_BACKGROUND`: `-c gc.auto=0 -c maintenance.auto=false -c
  core.fsmonitor=false <verb> …`. In a query it comes after
  `--no-lazy-fetch`, which stays first as WP-154 and SPEC require:
  `--no-lazy-fetch -c core.fsmonitor=false <verb> …`. Without
  `--no-lazy-fetch` (an old git) the query starts with the option.
  `git --version` carries it too. Checked by hand on git 2.55: a
  `status` with `-c core.fsmonitor=true` starts `git fsmonitor--daemon
  start` and `run --detach`; with `-c core.fsmonitor=false` after it,
  0 children. I stopped that hand-started daemon (`fsmonitor--daemon
  stop`), and `pgrep` shows none. The plugins collector already had the
  option (`GIT_OPTIONS`).
- **Pins.** Unit `a_query_never_reaches_the_network_and_a_commit_is_the_users`:
  the query argv with and without `--no-lazy-fetch`, `--version` and the
  commit argv, literally. `every_git_call_but_add_and_commit_is_a_query_without_network`
  checks every logged call: writes start with all three options, and
  queries start with `--no-lazy-fetch -c core.fsmonitor=false`.
  `a_git_without_no_lazy_fetch_still_answers` expects the retried
  `status` with the option.
- **Integration.** `init_leaves_no_git_running_in_the_logbook` now has
  `core.fsmonitor = true` in the test home's `.gitconfig` and also
  rejects any `fsmonitor` child in the trace. Mutation (both
  `extend_from_slice(&NO_FSMONITOR)` removed): FAILED with
  `child_start[0] git fsmonitor--daemon start` / `run --detach` per
  logbook. The test runs `git fsmonitor--daemon stop` in each logbook
  before its asserts, and after the mutation run `pgrep` showed no daemon.
- **SPEC-ENGINE**: the fsmonitor rule, and one sentence saying the
  options are per call, never written to a config file, and reach the
  user's own hooks (and any git they start) through
  `GIT_CONFIG_PARAMETERS`. The query paragraph now says
  `--no-lazy-fetch` first, then the option. TESTING row and CHANGELOG
  updated.
- **Verified** (desktop, same on-disk target, TMPDIR and private 0700
  XDG_RUNTIME_DIR; no network; real `~/Seldon`, `~/.local/state/seldon`,
  `~/.config` untouched):
  - *fixture*: `cargo test -j 4 --locked --test git --test doctor --test
    init`: 14, 39 and 47 passed.
  - *fixture*: `--lib logbook::git`: 6 passed.
  - *fixture*: full `cargo test -j 4 --locked`: exit 0, 55 binaries ok.
  - *fixture*: clippy `-D warnings` clean, fmt clean.
  - *not run* in round 2: `just check` and the 50/50 doctor load run.
    The change after round 1 is an argv option and its tests.
