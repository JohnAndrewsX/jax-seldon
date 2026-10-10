# WP-160 — Plan

Branch `wp/160-stale-dblck` from `next` (dda8b6aa). A `db.lck` left by a
pacman that died before the current boot no longer holds its open
transaction back: it is emitted `unfinished` (ADR-0043).

## The rule

- **Lock state** (new, `collectors::pacman::lock_state`): `Absent` (no
  file), `Held` (the file is there and not older than the current boot,
  or either time cannot be read), `Stale { modified, boot }` (its mtime
  lies before the boot time).
- **Boot time**: the `btime` line of `/proc/stat` (seconds since the
  epoch). A file that cannot be read, or has no such line, gives no boot
  time: the lock counts as `Held` (today's rule, never a false
  `unfinished`).
- `parse(…, lock_present, …)` is unchanged; the collector passes
  `state == Held`. A stale lock is treated like an absent one: the open
  transaction at the end of the log is emitted `unfinished` and the
  cursor moves past it, so a later capture (lock removed or not) adds
  nothing (idempotency).
- **Read only**: `metadata()` of the lock and a read of `/proc/stat`.
  The lock is never removed, touched or opened (AGENTS.md §6).

## Sources

- `Sources.proc_stat`: `SELDON_PROC_STAT`, default `/proc/stat`; under
  `SELDON_TEST_GUARD` without the variable `<guard>/proc-stat` (missing:
  no boot time, so `Held`), as `snapshots` and `omarchy_path` do.
- `Sources.pacman_db_lock` under `SELDON_TEST_GUARD` without the variable:
  `<guard>/db.lck` (absent unless a test makes it), so the new doctor row
  never reads the host's lock in a guarded run. Tests that set the
  variable are unchanged.
- The bench structs in the tests spell the new field out.

## Doctor

A `pacman` row after `snapper` (only when the pacman collector is on;
"collector disabled in config.toml" otherwise, as omarchy and snapper):

- absent → ok, "no db.lck: pacman is not running";
- held → ok, "db.lck from this boot: pacman is running (its transaction
  is recorded when it ends)"; no boot time → the same, "boot time not
  known";
- stale → degraded, names the path and its time, says pacman left it
  (killed or a power loss before the boot of …), that pacman refuses to
  run until it is gone and that Seldon records the open transaction as
  `unfinished`; fix (text, never run): "make sure no pacman, yay or
  update runs, then: sudo rm /var/lib/pacman/db.lck" (the configured
  path). Exit code unaffected (degraded).

## Spec

SPEC-ENGINE §4 pacman: "absent at capture time" becomes "absent, or
stale (older than the current boot, `/proc/stat` `btime`)", with the
fallback, the clock-jump caveat and the read-only rule; §3 doctor gains
the `pacman` row; the Sources env table if there is one gains
`SELDON_PROC_STAT`. The collector's module comment the same.
ADR-0013 §5 and ADR-0043 stay as they are: this is the clarification of
"pacman is gone" (ADR-0043's own wording) that ADR-0043 Consequences
names as WP-160. If the reviewer disagrees, a short ADR follows.

## Known limit (handover)

`btime` follows the wall clock: a pacman started before the clock was
set (RTC far behind, then NTP jumps forward) has a lock older than the
corrected boot time, and its running transaction would be emitted
`unfinished` while it runs. Rare (pacman before time sync); the status is
final (ADR-0043). A stronger test (a pacman process in `/proc/*/comm`)
reads beyond the zones; left to the reviewer.

## Tests

Unit (pacman.rs): `lock_state` absent, newer than boot, older than boot,
no `/proc/stat`, no `btime` line, garbage `btime`; `boot_time` parser.
Integration (collectors.rs, Bench): an open transaction with a lock older
than a fake boot → emitted `unfinished`, cursor at the end; a second
capture with the lock still there and one after it is removed add
nothing; with a lock newer than the boot → held back as today, then
emitted once the transaction completes. Doctor (doctor.rs): no lock → ok;
fresh lock → ok; stale lock → degraded with the path and the fix; lock
never removed.

Hand mutants: (1) `<` → `<=` in the stale compare … (2) treat
unknown boot as stale, (3) collector passes `exists()` again, (4) doctor
stale → ok; each must fail a test.

## Live measurement

Test host: a dev build with `SELDON_PACMAN_DB_LOCK` at a scratch file
(`touch -d` before and after the real `btime`) against a scratch pacman
log and a scratch HOME/runtime dir, real `/proc/stat`; plus doctor
against the host's real (absent) `db.lck`. Nothing under `/var` touched.
