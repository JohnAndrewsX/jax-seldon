# WP-160 handover — a stale pacman db.lck hides an unfinished transaction

Branch `wp/160-stale-dblck` (from `next` `dda8b6aa`). Plan: `PLAN.md`.

## What was done

- **Lock state** (`engine/src/collectors/pacman.rs`): `lock_state(lock,
  proc_stat)` gives `Absent`, `Held { boot }` or `Stale { modified, boot }`.
  Stale iff the lock's mtime lies strictly before the `btime` of
  `/proc/stat` (whole seconds, rounded down by the kernel, so a lock made
  after the boot is never older). No `/proc/stat`, no or a bad `btime`
  (also `0`), or no mtime → `Held` (the rule before WP-160; never a false
  `unfinished`). The lock is looked at with `symlink_metadata` only: never
  opened, touched or removed (AGENTS.md §6). A symbolic link at the lock
  path counts as present, as for pacman (its `O_EXCL` create fails on
  one); before, a dangling link counted as absent.
- **Collector**: passes `is_held()` instead of `exists()` to `parse`. A
  stale lock is therefore treated like an absent one: the open
  transaction at the end of the log is emitted with `meta.txStatus:
  unfinished` (ADR-0043) and the cursor moves past it, so later captures
  add nothing. `parse` and the rotation rule are unchanged.
- **Sources** (`collectors/mod.rs`): `proc_stat` (`SELDON_PROC_STAT`,
  default `/proc/stat`). Under `SELDON_TEST_GUARD` without the variables,
  `proc_stat` is `<guard>/proc-stat` and `pacman_db_lock` is
  `<guard>/db.lck` (so the new doctor row never reads the host's lock in
  a guarded run; tests that set `SELDON_PACMAN_DB_LOCK` are unchanged).
  The three test bench structs spell the new field out.
- **Doctor** (`commands/doctor.rs`): a `pacman` row after `snapper`.
  Absent → ok "no db.lck: pacman is not running"; this boot → ok "pacman
  is running; its transaction is recorded when it ends"; no boot time →
  ok, says so; stale → **degraded**, "stale <lock> from <local time>,
  before this boot (<time>): a pacman was killed or lost its power.
  pacman refuses to run until the lock is gone; Seldon records the
  transaction it left open as unfinished", fix "make sure no pacman, yay
  or omarchy update is running, then: sudo rm <lock>" (text only, never
  run); collector off → ok "collector disabled in config.toml" without
  looking at the lock. Degraded does not change doctor's exit code.
- **Docs**: SPEC-ENGINE §4 pacman (the stale rule, the fallback, read
  only, the clock caveat, the env variables, "absent or stale" in the
  status sentence) and §3 doctor (the row); the collector's module
  comment; `memory/rust-notes.md` (the env list); CHANGELOG Unreleased ›
  Engine.
- **No ADR.** ADR-0013 §5 says "`db.lck` is absent"; ADR-0043 says
  "`db.lck` is absent (pacman is gone)" and its Consequences name WP-160
  for exactly this case. I read the change as a clarification of "pacman
  is gone" and only updated the spec. If the reviewer reads ADR-0013 §5
  literally, a short ADR superseding that one sentence is needed.

## How it was verified

- **Unit** (`pacman.rs`): `boot_time_from_proc_stat` (the `btime` line,
  no newline, and nine lines that are not one: empty, missing, blank,
  negative, `0`, `12x`, `xbtime`, `btimes`); `lock_states` (absent; one
  minute after the boot and in the boot second → held; one second before
  → stale; no stat file and no `btime` → held; mtime and content of the
  lock unchanged afterwards).
- **Collector** (`tests/collectors.rs`,
  `pacman_emits_the_open_transaction_of_a_stale_lock`): fake boot
  2026-10-01 10:30 +02:00, lock from 10:00 → `gtk4`, `linux` emitted
  `unfinished` with the command line, cursor at the end, lock untouched;
  a capture with the lock still there and one after it is removed add
  nothing (idempotency); the next pacman transaction afterwards is
  recorded normally without status. Boot 09:00, same lock → held back,
  cursor at 0, then emitted without status once `transaction completed`
  is logged. No `/proc/stat` → held.
- **Doctor** (`tests/doctor.rs`, `the_pacman_row_names_a_stale_lock`):
  guarded defaults, the four states, the human line and the fix text,
  the lock untouched, the collector switched off.
- **Hand mutants**, each run against the three test groups, **7 of 7
  killed**: `<` → `<=` in the stale compare; no boot time → absent; the
  collector back to `exists()`; doctor's stale row ok; `btime 0`
  accepted; no guard default for `proc_stat`; doctor looking at the lock
  with the collector off. The source was restored after each one.
- **`just check`**, `SELDON_FULL_CHECK=1`, private `XDG_RUNTIME_DIR`,
  scratch `JUST_TEMPDIR`, under `flock /tmp/seldon-check.lock`: **ok**
  (exit 0). Its one docs-check warning (the German getting-started page
  behind the English one) was there before and is not from this WP.
  `cargo fmt --check` and `clippy --all-targets --all-features -D
  warnings` clean.
- **Live, test host** (static musl release build of this branch, scratch
  HOME, private runtime dir, scratch pacman log and lock, the host's real
  `/proc/stat` with `btime` 2026-10-01 06:45 +02:00):
  - lock `touch -d` one minute after the boot → capture emits nothing;
    doctor ok "db.lck from this boot";
  - lock one hour before the boot → capture emits `gtk4` with
    `txStatus: unfinished`; doctor degraded with the stale text and the
    `sudo rm` fix;
  - capture again with the lock, and after removing it: 0 events each;
    1 pacman event in the ledger;
  - doctor with the default lock path (the host's real
    `/var/lib/pacman/db.lck`, absent): ok "no db.lck".
  Nothing under `/var` was touched; the scratch folder and runtime dirs
  were removed by path afterwards. Case C-2026-005 and its windows were
  not touched.

## What was not done

- No plugin change: a stale lock reaches the plugin as the
  `unfinished` mark of ADR-0043. The plugin calls only `seldon doctor
  --only rules`, so the new `pacman` row is seen in a terminal, not in
  the panel (a banner would be a plugin WP).
- No `/proc/*/comm` check for a running pacman (beyond the zones).

## Open questions

1. **Clarification or ADR?** See above (ADR-0013 §5 wording).
2. **Clock jumps.** `btime` follows the wall clock. A pacman started
   before the clock is set forward (an RTC far behind, then time sync)
   has a lock older than the corrected boot time, and its running
   transaction would be emitted `unfinished` while it runs; the status is
   final (ADR-0043). Rare (pacman before time sync) and in the spec. A
   stronger test would read `/proc/*/comm` for a pacman process, which
   the zones do not allow today; the operator may decide.
3. **Doctor status.** A stale lock is `degraded` (Seldon works fully;
   pacman itself refuses to run). If the reviewer wants it to fail doctor
   (`error`, exit 1), it is one word.
4. **Host /tmp quota.** During the mutant runs the user quota on the dev
   host's `/tmp` (tmpfs) was full (about 12 of 16 GB used, mostly not
   this WP's). I removed only this WP's own build folders and ran the
   mutants against the worktree's `engine/target` instead.
