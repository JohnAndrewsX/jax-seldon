WP-034 HANDOVER

Branch `wp/034-watch`, worktree `wt/WP-034`. Base: `main` at `071f6b9`.
`main` is now at `338ff3c`, with WP-032 merged. Not pushed, no PR.

Commits `071f6b9..HEAD`:

| Commit | What |
|---|---|
| `bcda0c9` | `seldon watch` behind the `watch` feature, `notify` dependency, `main.rs` variant and arm, `commands/mod.rs` one line, unit tests, `tests/watch.rs` |
| `03059ad` | `engine/systemd/seldon-watch.service` and `README.md`; wording of the error without the feature |
| `1b4cb89` | `just check-watch` (part of `check`), docs/TESTING.md |
| `4d6f82d` | memory/rust-notes.md, memory/pitfalls.md |

**Merge with `main`.** I checked a trial merge of `main` (`338ff3c`) into
a throw-away worktree; the worktree is removed.
- Three conflicts, all "both sides appended at the same place":
  - `engine/src/main.rs`: `Rebuild` (WP-032) and `Watch` both come after
    `Agent`. Keep both, `Rebuild` first, in the enum and in the `match`.
  - `memory/pitfalls.md` and `memory/rust-notes.md`: entries appended at
    the end on both sides. Keep both, WP-032's first.
- `commands/mod.rs`, `justfile` and `docs/TESTING.md` merge cleanly.
- After that resolution, `just fmt-check clippy test check-watch
  schema-validate` exits 0: 673 tests passed, 0 failed, with and without
  the feature.
- git's rerere recorded the three resolutions.

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
  - **Triggers a rebuild:** an event without paths (an inotify overflow
    rescan) and a watcher error.
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
  - **Other rebuild errors:** one `error` line, then the watcher goes on.
  - **Log, one line per rebuild:**
    - Text on stderr: `HH:MM:SS seldon watch: index rebuilt (N event(s),
      D open drift) in X ms after K change(s): <up to 10 paths>`, plus
      the load warnings.
    - `--json`: one object per line on stdout with
      `status: watching | rebuilt | error | stopped`. `rebuilt` carries
      `generatedAt`, `events`, `summary`, `changes`, `paths`,
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
- **Tests.** `just check` gains `check-watch`:
  - clippy `--features watch`;
  - `cargo test --features watch`;
  - the RSS test again under `--profile bench`.
  - `engine/tests/watch.rs`, with the feature:
    - one change → exactly one rebuild, ≥ 2 s after it, then quiet. The
      new memory topic is in the index and the index validates.
    - a burst of 24 writes plus a new folder → one rebuild. A later write
      in the new folder is seen.
    - none of these trigger a rebuild: generated views, `STATUS.md`,
      temp and backup files, `PROJECT.md`, reads of every watched file,
      `seldon index` and `seldon status`. `.seldon/logbook.toml` does
      trigger one.
    - held lock → no rebuild and still running; the rebuild comes < 2 s
      after the release.
    - SIGTERM and SIGINT → exit 0 and a `stopped` line.
    - not initialised → exit 3; `--interval 1` → exit 1.
    - RSS on ×10 (below).
  - Without the feature: the exit-1 test.
  - Unit tests: path and event filter.
- **docs/TESTING.md:** a `check-watch` row, a `tests/watch.rs` row, and a
  section "The `watch` feature" (memory bound, `SELDON_WATCH_BIN`, the
  measured numbers).

## RSS (PLAN.md: < 10 MB)

×10 fixture (`tests/common/scale.rs`, 500 events in the index), read from
`/proc/<pid>/status` after one rebuild:

| Binary | Idle | After rebuild | Peak (VmHWM) |
|---|---|---|---|
| debug (test profile) | 12.5 MB | 16.7 MB | 16.7 MB |
| bench profile (in `just check-watch`) | 6.1 MB | 8.9 MB | 8.9 MB |
| musl release `--features watch` (`SELDON_WATCH_BIN`) | 5.0 MB | 7.6 MB | 7.9 MB |

The debug binary maps about 6 MB more code, so the test profile bounds
only the growth over idle (< 6 MB; optimised ≈ 3 MB). The optimised runs
assert peak < 10 MB. The bench-profile margin is only about 1.1 MB. The
shipped musl build has about 2 MB.

## Not done

- No initial rebuild at start. The watcher reacts to changes only; the
  shell timer and every writing command keep the index fresh. Say if you
  want one.
- The watcher writes neither the `ledger/*.md` views nor `STATUS.md`.
  That is deliberate:
  - it keeps the watcher a reader of the logbook, and never leaves it
    dirty between commits;
  - `seldon index` and `seldon status` still own the views.
- Packaging: the PKGBUILD (Phase 4) must decide whether the shipped
  binary gets `--features watch`. Until then the unit needs a
  self-built binary (README step 1).

## Verified by

- `just check` → `check: ok`, exit 0 (5 min on the dev host). This
  includes `check-watch`, `plugin-validate`, `qmllint` (18 files) and
  `plugin-test`.
- `cargo clippy --features watch --all-targets -- -D warnings`: clean.
  Also clean without the feature.
- `cargo test --features watch --test watch`: 7 passed. Run 4× in a row
  and once alongside the full suite, with no flake. Without the feature:
  1 passed.
- `just build-release` (musl, no feature): ok, static. `seldon watch` →
  exit 1 with the message.
- `SELDON_WATCH_BIN=<musl release --features watch> cargo test --features
  watch --test watch rss -- --nocapture`: peak 7852 kB.
- Manual smoke run in a scratch home (`HOME` and all `XDG_*` redirected,
  `SELDON_TEST_GUARD`):
  - one edit → one `rebuilt` line;
  - a `seldon status` run while watching → nothing;
  - SIGTERM → exit 0 and a `stopped` line.
- `systemd-analyze --user verify engine/systemd/seldon-watch.service`: the
  only complaint is "~/.local/bin/seldon is not executable", because the
  binary is not installed on this host. A scratch copy with
  `ExecStart=/usr/bin/true` verifies with no output and exit 0.
- The unit was **not** enabled, started or installed. No `systemctl`
  command ran, and nothing was written under `~/.config` or
  `~/.local/state`.

## SPEC-ENGINE §3 wording (for the orchestrator)

Replace the `seldon watch` line in the command block with:

```
seldon watch [--interval SECS] [--json]        # feature "watch" (off by default, ADR-0005; without it: exit 1
                                               # "built without the watch feature"). Watches ledger/ work/ journal/
                                               # decisions/ system/ memory/ (recursive) and .seldon/logbook.toml;
                                               # after SECS quiet (default and minimum 2; at most 5×SECS into a
                                               # burst) rebuilds index.json under the lock (a held lock delays,
                                               # retried every 250 ms). Writes index.json only: no capture, no
                                               # views, no STATUS.md, no commit. Ignores reads, ledger/*.md,
                                               # STATUS.md, hidden/temp/backup files. One line per rebuild on stderr;
                                               # --json: JSON lines on stdout {status: watching|rebuilt|error|stopped}.
                                               # SIGTERM/SIGINT → exit 0 after the rebuild in progress; exit 3 when
                                               # the logbook is not initialised. User unit: engine/systemd/ (WP-034)
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

- Should `just check` keep the bench-profile RSS step? It adds one
  optimised compile: about 10 s incremental, longer from a cold cache,
  and CI runs it too. The alternative is a separate recipe outside
  `check`, like `bench`.
- Should the PKGBUILD (Phase 4) ship the binary with `--features watch`?
  It adds 144 KB to the static binary (4.79 → 4.93 MB) and costs nothing
  at runtime unless the command
  is used.

## Touched outside WP scope

- `engine/src/commands/mod.rs`: one line, `pub mod watch;`, needed to
  register the module.
- `engine/Cargo.lock`: the `notify` tree.
- Nothing else. `index/`, `commands/{status,capture,rebuild}.rs`,
  `plugin/`, `schema/`, `fixtures/` and `tests/common/` are untouched.
