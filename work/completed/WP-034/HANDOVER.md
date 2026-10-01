WP-034 HANDOVER

Branch `wp/034-watch`, worktree `wt/WP-034`. Rebased onto `main` at
`338ff3c` (WP-032 merged) on the orchestrator's instruction. Not pushed,
no PR.

Commits `338ff3c..HEAD`:

| Commit | What |
|---|---|
| `50c5b25` | `seldon watch` behind the `watch` feature, `notify` dependency, `main.rs` variant and arm, `commands/mod.rs` one line, unit tests, `tests/watch.rs` |
| `2a53888` | `engine/systemd/seldon-watch.service` and `README.md`; wording of the error without the feature |
| `f818df8` | `just check-watch` (part of `check`), docs/TESTING.md |
| `77e88c4` | memory/rust-notes.md, memory/pitfalls.md |
| `a4409f0` | first handover |
| HEAD | review follow-ups (decisions A–C, items 1–4), this handover |

**Rebase.** rerere applied the resolutions recorded in the trial merge;
no conflict markers are left:
- `engine/src/main.rs`: `Rebuild` (WP-032) first, then `Watch`, in the
  enum and in the `match`.
- `memory/pitfalls.md` and `memory/rust-notes.md`: WP-032's section
  first, then WP-034's.

## Review follow-ups (one commit)

Decisions from the review:
- **(A)** The bench-profile RSS run has left `just check`.
  - `check-watch` (still part of `check`) runs clippy and
    `cargo test --features watch`.
  - The new recipe `just check-rss` runs `rss_stays_under_10_mb` under
    `--profile bench`. It is not in `check` and CI does not run it.
  - docs/TESTING.md makes `check-rss` required before the handover of a
    WP that touches `engine/src/index/` or
    `engine/src/commands/watch.rs`.
- **(B)** The Phase 4 PKGBUILD ships the binary with the feature. The
  README's step 1 now reads "Until the package ships it …".
- **(C)** One unconditional rebuild right after the `watching` line:
  - The index then reflects edits made while the watcher was down.
  - The `rebuilt` line carries `"trigger": "start"`; the text says
    `index rebuilt (…) at start`. Later rebuilds carry
    `"trigger": "changes"`.
  - The rebuild at start respects the lock like any other.
  - Events that arrive before it runs join it.

Small items:
1. A re-watch that fails (a folder that vanished or was replaced) logs an
   `error` line and the watcher goes on.
   - `watch_folders` now returns the failed watches instead of failing
     the command.
   - A failed folder stays out of the watched set, so the next root
     event for it tries again.
   - At start a failed watch is still fatal: exit 2.
2. `TICK` 200 ms → 500 ms.
3. On a rescan (`event.need_rescan()`, inotify overflow) all six folders
   are unwatched and watched afresh, so a subfolder whose create event
   was lost is not left unwatched.
   - `relevant()` counts a rescan even when the event carries a path.
4. This handover: hashes, base, decisions, SPEC wording.

Test changes:
- `Watch::start` now consumes the `watching` line and the rebuild at
  start (`trigger: "start"`).
- The first test writes a memory file before the start and finds it in
  the index after the rebuild at start.
- SIGTERM/SIGINT: `"rebuilds": 1`.
- The RSS test holds the lock while the watcher starts. Idle is read
  first, then the lock is released, then come the rebuild at start
  (500 events) and one change-triggered rebuild.
- New `a_replaced_folder_is_watched_again`: `memory/` is renamed away and
  recreated. That gives one rebuild for `memory`. A write in
  `memory.old/` stays quiet, so the old watch is gone. A write in the new
  `memory/` gives one rebuild, and the index's topics are `["fresh"]`.
- New unit assertion: a rescan event with an unrelated path counts.

## Done

- **Cargo feature `watch`**, off by default, adds `notify = 8.2.0`
  (`default-features = false`, optional).
  - Justification (one line): `notify` is the only watcher crate
    AGENTS.md §7 allows. Its inotify backend runs on its own thread
    (mio poll); there is no async runtime.
  - On Linux it brings in inotify, inotify-sys, mio, walkdir, same-file,
    log, notify-types, bitflags, and libc (libc was already in the lock
    file).
  - The release build without the feature has no `notify` in its tree
    (`cargo tree -e normal | grep -c notify` → 0).
- **Without the feature:**
  - `seldon watch` → exit 1, `seldon: built without the watch feature;
    rebuild seldon with `cargo build --release --features watch``.
  - With `--json` → `{"error":{"code":1,…}}`.
  - Checked on the musl release binary from `just build-release`.
- **`seldon watch [--interval SECS] [--json]`, with the feature**
  (`engine/src/commands/watch.rs`):
  - **What it watches:** `ledger/`, `work/`, `journal/`, `decisions/`,
    `system/` and `memory/` recursively, and `.seldon/logbook.toml`. The
    file is watched through `.seldon/`, not recursively, because editors
    replace files.
    - The root is also watched, not recursively, but only so that one of
      the six folders is picked up when it is created later. If a folder
      is deleted or replaced, its watch is renewed.
    - The logbook is resolved once at start and pinned, so a later
      `config.toml` change does not move the watcher.
  - **Never triggers a rebuild:**
    - `Access` events. notify reports every open and every
      close-without-write; without this filter the rebuild's own reads
      re-trigger it forever.
    - `ledger/*.md` and `STATUS.md`.
    - Anything with a hidden path component: `write_atomic` temp files
      `.<name>.tmp-<pid>`, swap files, `.obsidian`.
    - `*~`, `*.swp` and `*.swx`.
    - Root files other than the six folders.
  - **Triggers a rebuild:** the start (once), an event without paths, a
    rescan (which also re-watches the six folders, see item 3), and a
    watcher error.
  - **Debounce:**
    - The rebuild runs once the logbook has been quiet for `--interval`
      seconds, at the latest 5 intervals after the first change.
    - `--interval` takes 2…3600; the default is 2. Below 2 is a clap
      error, exit 1.
  - **Rebuild:**
    - Takes the state lock, then `index::derive_at` with a fresh clock,
      then `git_info`, then `index::write`.
    - Writes `index.json` only. It writes no logbook file, so it cannot
      trigger itself.
    - Uses only public `index::` functions. `index/`, `status.rs`,
      `capture.rs` and `rebuild.rs` are untouched.
  - **Lock held** (`Error::LockHeld`): the rebuild is retried every
    250 ms. It does not fail and does not print anything.
  - **Logbook missing or deleted:** exit 3, at start and during a rebuild.
  - **Watches that cannot be set up at start** (the inotify watch limit):
    exit 2. A failing re-watch later is an `error` line (item 1).
  - **Other rebuild errors:** one `error` line, then the watcher goes on.
  - **Log, one line per rebuild:**
    - Text on stderr: `HH:MM:SS seldon watch: index rebuilt (N event(s),
      D open drift) in X ms after K change(s): <up to 10 paths>`, plus
      the load warnings.
    - `--json`: one object per line on stdout with
      `status: watching | rebuilt | error | stopped`. `rebuilt` carries
      `trigger` (`start` | `changes`), `generatedAt`, `events`,
      `summary`, `changes`, `paths`,
      `durationMs` and `warnings`.
    - There is one `watching` line at start (tests wait for it) and a
      final `stopped` line.
  - **Signals:** on SIGTERM or SIGINT the rebuild in progress finishes,
    then the watcher exits 0 with
    `{"status":"stopped","signal":"SIGTERM","rebuilds":N}`.
    - Handled by the C library's `signal(2)` through a 3-line
      `extern "C"` block, with no extra crate.
    - The handler stores an atomic and resets to `SIG_DFL`, so a second
      Ctrl-C kills at once.
  - **Clock:** real time for each rebuild. Under `SELDON_NOW`, that time
    plus the time elapsed since start (`Context::now` is fixed at process
    start).
- **systemd user unit** `engine/systemd/seldon-watch.service`:
  - `ExecStart=%h/.local/bin/seldon watch`, `Restart=on-failure`,
    `RestartSec=10`, `StartLimitBurst=5` in 300 s.
  - `RestartPreventExitStatus=1 3`: no feature, or no logbook.
  - Hardening that works without namespaces: `NoNewPrivileges`,
    `LockPersonality`, `RestrictRealtime`, `RestrictSUIDSGID`,
    `SystemCallArchitectures=native`, `Nice=10`, idle I/O.
  - `WantedBy=default.target`.
- **`engine/systemd/README.md`:**
  - What the watcher does and does not do: index only, no capture, no
    views, no commit.
  - Build with the feature; try it in a terminal; install and `enable
    --now`, by the user only (the wizard never installs it); journal;
    configure through a drop-in; exit-code and restart table; removal.
- **Tests.** `just check` gains `check-watch`: clippy
  `--features watch` and `cargo test --features watch`. `just check-rss`
  (outside `check`, decision A) runs the RSS test under `--profile
  bench`.
  - `engine/tests/watch.rs`, with the feature:
    - the rebuild at start includes an edit made before the start.
    - one change → exactly one rebuild, ≥ 2 s after it, then quiet. The
      new memory topic is in the index and the index validates.
    - a replaced folder is watched again; the old one goes quiet.
    - a burst of 24 writes plus a new folder → one rebuild. A later write
      in the new folder is seen.
    - none of these trigger a rebuild: generated views, `STATUS.md`,
      temp and backup files, `PROJECT.md`, reads of every watched file,
      `seldon index` and `seldon status`. `.seldon/logbook.toml` does
      trigger one.
    - held lock → no rebuild and still running; the rebuild comes < 2 s
      after the release.
    - SIGTERM and SIGINT → exit 0 and a `stopped` line (`rebuilds: 1`,
      the one at start).
    - not initialised → exit 3; `--interval 1` → exit 1.
    - RSS on ×10 (below).
  - Without the feature: the exit-1 test.
  - Unit tests: path and event filter.
- **docs/TESTING.md:**
  - a `check-watch` row and a `tests/watch.rs` row;
  - `check-rss` under "Other recipes";
  - a section "The `watch` feature": the memory bound, when `check-rss`
    is required, `SELDON_WATCH_BIN`, the measured numbers.

## RSS (PLAN.md: < 10 MB)

×10 fixture (`tests/common/scale.rs`, 500 events in the index), read from
`/proc/<pid>/status`. Idle is read while the lock holds the start rebuild
back; the other values come after the rebuild at start and one
change-triggered rebuild:

| Binary | Idle | After rebuilds | Peak (VmHWM) |
|---|---|---|---|
| debug (test profile) | 13.0 MB | 17.4 MB | 17.4 MB |
| bench profile (`just check-rss`) | 6.0 MB | 9.1 MB | 9.1 MB |
| musl release `--features watch` (`SELDON_WATCH_BIN`) | 4.8 MB | 7.5 MB | 7.9 MB |

The debug binary maps about 6 MB more code, so the test profile bounds
only the growth over idle (< 6 MB). The optimised runs assert peak
< 10 MB.
- Bench-profile margin: about 0.9 MB, down from 1.1 MB with one rebuild,
  because a second rebuild leaves a little more heap behind.
- Shipped musl build: about 2 MB.

## Not done

- The watcher writes neither the `ledger/*.md` views nor `STATUS.md`.
  That is deliberate:
  - it keeps the watcher a reader of the logbook, and never leaves it
    dirty between commits;
  - `seldon index` and `seldon status` still own the views.
- Packaging (decision B): the Phase 4 PKGBUILD builds with
  `--features watch`. Until then the unit needs a self-built binary
  (README step 1).
- The rescan re-watch (item 3) has no integration test, because an
  inotify queue overflow cannot be forced reliably in a test. The unit
  test covers its detection. The re-watch itself is the same `rewatch()`
  that `a_replaced_folder_is_watched_again` exercises.

## Verified by

At HEAD (after the follow-ups):
- `cargo clippy --features watch --all-targets -- -D warnings`: clean.
- `cargo test --features watch`, run twice: 341 passed, 0 failed both
  times. `tests/watch.rs` alone: 8 passed.
- `just check` → `check: ok`, exit 0. Across its runs with and without
  the feature, 673 tests passed and 0 failed. It also ran
  `plugin-validate`, `qmllint` (21 files) and `plugin-test`. The
  bench-profile step no longer runs there.
- `just check-rss`: 1 passed. Peak 9068 kB.
- `SELDON_WATCH_BIN=<musl release --features watch> cargo test --features
  watch --test watch rss -- --nocapture`: peak 7936 kB.

From the first handover, still valid:
- `just build-release` (musl, no feature): ok, static. `seldon watch` →
  exit 1 with the message.
- `systemd-analyze --user verify engine/systemd/seldon-watch.service`: the
  only complaint is "~/.local/bin/seldon is not executable", because the
  binary is not installed on this host. A scratch copy with
  `ExecStart=/usr/bin/true` verifies with no output and exit 0.
- Manual smoke run in a scratch home (`HOME` and all `XDG_*` redirected,
  `SELDON_TEST_GUARD`): one edit → one rebuild; `seldon status` while
  watching → nothing; SIGTERM → exit 0.
- The unit was **not** installed, enabled, started or reloaded. No
  `systemctl` command ran, and nothing was written under `~/.config` or
  `~/.local/state`.

## SPEC-ENGINE §3 wording (for the orchestrator)

Replace the `seldon watch` line in the command block with:

```
seldon watch [--interval SECS] [--json]        # feature "watch" (off by default, ADR-0005; without it: exit 1
                                               # "built without the watch feature"). Watches ledger/ work/ journal/
                                               # decisions/ system/ memory/ (recursive) and .seldon/logbook.toml;
                                               # one rebuild at start, then reacts to changes: after SECS quiet (default and minimum 2; at most 5×SECS into a
                                               # burst) rebuilds index.json under the lock (a held lock delays,
                                               # retried every 250 ms). Writes index.json only: no capture, no
                                               # views, no STATUS.md, no commit. Ignores reads, ledger/*.md,
                                               # STATUS.md, hidden/temp/backup files. One line per rebuild on stderr;
                                               # --json: JSON lines on stdout {status: watching|rebuilt|error|stopped}.
                                               # SIGTERM/SIGINT → exit 0 after the rebuild in progress; exit 3 when
                                               # the logbook is not initialised; watcher error (inotify limit: the
                                               # watches cannot be set up at start) → exit 2; a failing re-watch
                                               # later is an error line. User unit: engine/systemd/ (WP-034)
```

## Learned (in memory/)

- rust-notes:
  - an optional feature with an optional dependency, and the subcommand
    kept in every build;
  - notify 8.2: Linux deps, an mpsc handler, Access events from reads,
    the race with files written into a new folder;
  - signals through `extern "C" signal`;
  - a long-running command must not reuse `ctx.now`;
  - `Command::get_envs` to reuse the test environment;
  - debug against optimised RSS, and `--profile bench` for an absolute
    bound.
- pitfalls:
  - a watcher sees its own reads;
  - the unit is never enabled or started on the dev host; use
    `systemd-analyze verify` with the scratch-copy trick;
  - the default release build has no watcher, hence
    `RestartPreventExitStatus=1 3`;
  - wait for the `watching` line; a sleep is not enough.

## Decisions needed

None. The review settled the two open questions (decisions A and B).

## Touched outside WP scope

- `engine/src/commands/mod.rs`: one line, `pub mod watch;`, needed to
  register the module.
- `engine/Cargo.lock`: the `notify` tree.
- Nothing else. `index/`, `commands/{status,capture,rebuild}.rs`,
  `plugin/`, `schema/`, `fixtures/` and `tests/common/` are untouched.
