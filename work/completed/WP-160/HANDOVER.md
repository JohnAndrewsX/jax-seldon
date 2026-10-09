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

## Round 2

Stage 1 (Opus) approved with notes N1–N4 and a wording nit; the
orchestrator's round-2 brief asked for all of them, the hardening of N2
included, and settled the ADR question: no ADR, a clarification of
ADR-0013 §5 and ADR-0043.

### What was done

- **N1.** `memory/rust-notes.md` is append-only: the bullet I had edited
  is back as it was (`git checkout dda8b6aa -- memory/rust-notes.md`), and
  a new section `2026-10-09 · WP-160` is appended (the `SELDON_PROC_STAT`
  variable and the guarded defaults; `File::set_modified`; temp dirs in
  unit tests behind a drop guard).
- **N2, hardening.** A stale lock lets the open transaction go only when
  its last line libalpm wrote (`[ALPM]`, `[ALPM-SCRIPTLET]`, a line the
  table matches or not, so hook and scriptlet output count) is older than
  `btime` too. A later line means pacman wrote since (a forward clock jump
  after it took the lock): held back as under a held lock, and recorded
  whole once it ends. `[PACMAN] Running` does not count (pacman logs it
  before it takes the lock: a retry after the boot that failed on the
  stale lock). With no open transaction, a Running line from this boot is
  read again next time, an older one is passed. `pacman.log`'s mtime is
  not used. `parse` now takes the `LockState` instead of a bool (the
  rotated file passes `Absent`, as before).
- **N2, spec.** SPEC-ENGINE §4 says what a forward clock jump still does:
  only a capture between the jump and pacman's next line emits the
  transaction `unfinished` while pacman runs, and the cursor moves past
  it, so the lines pacman writes after that, up to `transaction
  completed`, arrive as package lines outside any transaction (no
  `txId`, `meta.command`, status or group).
- **N3.** `lock_states` has a dangling symbolic link at the lock path:
  `Held`, the link not followed. The spec says so.
- **N4.** Under `SELDON_TEST_GUARD` without `SELDON_PACMAN_LOG` the log is
  `<guard>/pacman.log`; a new test (`tests/collectors.rs`,
  `a_guarded_capture_reads_the_guards_pacman_log`) runs a guarded
  `capture --source pacman`: without the file the collector fails naming
  the guard path, with it one event. No other test needed a change
  (the whole `cargo test` is green with the new default).
- **Doctor wording.** A lock from this boot: "db.lck from this boot: taken
  as a running pacman; …" (a pacman killed in this boot leaves one too).
  The stale row says "records a transaction it left open" (there may be
  none, or one still held by the rule above).
- **Test temp dir.** `lock_states` removes its folder through a drop
  guard, also on a failed assertion. The two leftovers from my round-1
  mutant runs, `/tmp/seldon-dblck-1907291` and `/tmp/seldon-dblck-1907613`
  (an empty `db.lck` and a `stat` file each), are removed by path.
- **DECISIONS.md.** ADR-0013: "§5 absent read as absent or stale
  (WP-160)"; ADR-0043: "its Consequence on a stale lock is resolved by
  WP-160".

### How it was verified

- New unit test `a_stale_lock_and_the_last_line`: an open transaction from
  before the boot → `unfinished`, cursor at the end; plus a failed retry's
  Running line after the boot → still `unfinished`; plus a package line,
  a hook line, a scriptlet line or a `.pacnew` line after the boot (and
  one in the boot second) → held, cursor at 0; a last line one second
  before the boot → `unfinished`; a lone Running line from this boot →
  read again, an older one passed.
- `pacman_emits_the_open_transaction_of_a_stale_lock` gains the
  reviewer's clock-jump case: stale lock, a scriptlet line after the boot
  → held; then `upgraded mesa` and `transaction completed`, lock removed →
  `gtk4`, `linux`, `mesa` with one `txId`, the command and no status (in
  the reviewer's live run, before this round, `linux` came out with none
  of them).
- **Hand mutants, round 2: 9 of 9 killed** (against `--lib
  collectors::pacman`, `--test collectors`, `--test doctor pacman`, the
  source restored and `git status` clean after each): unmatched ALPM lines
  not counted; package lines not counted; `.pacnew` lines not counted
  (killed only after I added that case); a stale lock always lets go;
  `before_boot` `<` → `<=`; Running lines counted for the open
  transaction; a lone Running line never held under a stale lock;
  `symlink_metadata` → `metadata` (the reviewer's survivor); no guard
  default for `pacman_log`. No `/tmp/seldon-dblck-*` was left, also after
  the mutants that made `lock_states` fail.
- `cargo fmt --check`, `clippy --all-targets --all-features -D warnings`
  clean; `cargo test` all green.
- **`just check`** on `459b072a` (the code of this round; this commit adds
  only the handover), `SELDON_FULL_CHECK=1`, private `XDG_RUNTIME_DIR`,
  `CARGO_TARGET_DIR` the worktree's `engine/target` on disk, scratch
  `JUST_TEMPDIR`, under `flock`: **ok**, exit 0. The one docs-check
  warning (the German getting-started page) is the one from round 1.
- Not repeated on the test host: the change is in the parser, covered by
  the tests above with fake boot times; round 1's live run showed the real
  `btime` path.

### Open for stage 2

- The clock-jump trade-off (review §6): the window left is a capture
  between the jump and pacman's next libalpm line. A pacman that logs
  nothing for a long time after the jump (a long scriptlet or hook
  without output, for example) could still be cut there.

## Stage 2 (Fable approved; notes folded in)

- **Merged `next`** (`ad891a6f`, WP-170 queued); no conflict.
- **N1.** A Running line without a transaction is read again while any
  lock is present, held or stale (`hold_running`, called from both tail
  branches of `parse`: after an emitted `unfinished` transaction, and
  with no open transaction). Under a stale-looking lock the download
  phase after a forward clock jump therefore keeps its `meta.command`;
  once the lock is gone the line is passed as before. The assertions of
  `a_stale_lock_and_the_last_line` were flipped as the packet says, with
  one correction: in the retry case the cursor stands at the Running
  line (`open.len()`), not at 0, because the dead transaction before it
  is emitted. Added: the same logs with the lock gone are passed.
- **N2.** SPEC-ENGINE §4 names the window that is left (a capture
  between the jump and pacman's next libalpm line, i.e. inside a long
  silent hook or scriptlet) and says the other shape, the download phase
  before `transaction started`, is closed by the Running rule. I did not
  list it as still open, because after N1 it is not.
- **N3.** Both DECISIONS.md rows point at "SPEC-ENGINE §4 pacman, 'Stale
  lock'" besides the WP.
- **Verified.** Hand mutants for N1, 2 of 2 killed (no hold after an
  emitted transaction; only a held lock holds a Running line), the source
  restored and `git status` clean. `cargo fmt --check`, clippy `-D
  warnings` clean. **`just check`** on `afc9f836` (this commit adds only
  this section), `SELDON_FULL_CHECK=1`, private `XDG_RUNTIME_DIR`,
  `CARGO_TARGET_DIR` the worktree's `engine/target` on disk, scratch
  `JUST_TEMPDIR`, under `flock`: **ok**, exit 0; the one docs-check
  warning is the known German getting-started page.
