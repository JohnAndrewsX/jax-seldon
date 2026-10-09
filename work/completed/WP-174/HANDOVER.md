# WP-174 — Handover

Branch `wp/174-bounded-readers` from `next` (0f8d77db, WP-171 merged).
Not pushed (the orchestrator pushes).

## What was done

**The helper** (`engine/src/sys.rs`):

- `sys::open_regular(path)`: `std::fs::metadata` (follows a link) must
  say regular file, then the open with `O_NONBLOCK` (`custom_flags`; the
  constant written out per arch as `O_NOFOLLOW` is), then `fstat` on the
  open fd must say regular file again. A FIFO, device, socket or
  directory is an `InvalidInput` error "a FIFO | a device | a socket |
  a directory, not a regular file; not read". Because of the first check
  such a file is not opened at all. If one is swapped in after that
  check, the non-blocking open plus the fd check still refuse it without
  blocking. `NotFound` passes through unchanged, so every caller's
  `NotFound` arm still works.
- `sys::read_regular(path, max)` / `read_regular_string(path, max)`:
  `open_regular`, then the size from the fd, then `take(max + 1)`. A
  larger file is `FileTooLarge`, "more than <cap>; not read". Non-UTF-8
  input is `InvalidData`, as with `fs::read_to_string`.
- Caps: `LEDGER_MONTH_MAX` = 256 MiB (ledger months, about half a million
  events). `LOGBOOK_FILE_MAX` = 16 MiB for everything else.

**Links are followed on purpose.** The WP left two options: refuse a
final link, or check the type on the open fd. I took the fd check,
because ADR-0049 §2 says "Reading through a linked file is unchanged:
the index still shows a linked journal day. Only writes are refused."
`O_NOFOLLOW` on reads would have changed that accepted decision. A link
to `/dev/zero` is still refused, because the check looks at what the
link leads to.

**Readers routed** (every `fs::read` / `read_to_string` / `File::open`
of a logbook path in `engine/src`):

- The ones the WP named: `ledger::read_month` (`status`, `doctor`, every
  ledger reader), `commands/rules::read` (doctor's `rules` row and the
  capture's silent upgrade), doctor's `fences` row.
- Also routed:
  - `Logbook::open` (`.seldon/logbook.toml`, read by every command)
  - `model::load` and `index/load.rs` (every record the index and
    doctor read)
  - the journal day, the case files, the template and `active-case`
    (`cases::active_case`, as Fable asked for symmetry)
  - the import marker, undo and report files, and the omarchy-agent
    import's journal and memory targets
  - the views' `STATUS.md`, `DECISIONS.md` and `ledger/*.md`
  - `outputs/REBUILD.md`, the dossier's `system/*.md`, and
    `rebuild::read_dossier`
  - the session-start context (`STATUS.md`, the journal day,
    `memory/lessons.md`)
  - `hook::read_end`: the hook's toolUseId lookup in the ledger month
    (`open_regular`, plus the cap)
  - `index::ledger_lines_exceed` (`open_regular`; a `/dev/zero` here
    would have looped forever with no newline to count)
  - the logbook's `.git` files that `git_head_fast` and
    `git::is_linked_work_tree_of` read
  - Claude Code's settings file (`merge`/`unmerge`/`claude_hooks_in`).
    This includes the user-wide one; following the link stays as before,
    only a FIFO or device is refused.

**doctor** (`commands/doctor.rs`): `fences` now reports a STATUS.md or
DECISIONS.md that is no regular file as `degraded`, with "not checked
(…); `seldon status` leaves it as it is" and a fix. The old text was an
`error` saying "status stops on it", which is wrong for such a file:
`status` skips it as a view (WP-171). A file over the cap is still
`error`, because `status` does stop on that.

**Outcomes** (all measured, `tests/bounded_reads.rs`):

| File (FIFO or link to `/dev/zero`) | Command | Result |
|---|---|---|
| `ledger/2026-10.jsonl` | `status` | exit 2: "cannot read …/ledger/2026-10.jsonl: a FIFO, not a regular file; not read" |
| `ledger/2026-10.jsonl` | `doctor` | `ledger` error with the reason; `layout` reached and names it; exit 1 |
| `AGENTS.md` | `doctor` | `rules` degraded (any unreadable rules file already was); `layout` error names it; exit 1 |
| `AGENTS.md` | `capture` | exit 0; the silent upgrade leaves the file alone (pre-existing: an unreadable `AGENTS.md` is not upgraded and gets no warning) |
| `STATUS.md`, `DECISIONS.md` | `doctor` | `fences` degraded "not checked"; `layout` degraded names it; exit 0 |

**Tests**:

- Unit test `sys::a_bounded_read_takes_only_a_regular_file`:
  - regular file, linked file, cap
  - FIFO, `/dev/zero` link and directory, through `read_regular`,
    `read_regular_string` and the fd-level `open_checked` alone (the
    swapped-in case)
  - missing and dangling, which stay `NotFound`
  - non-UTF-8
- `tests/bounded_reads.rs`:
  - the three tests in the table above
  - `a_link_to_a_regular_file_is_read_as_before`: a linked ledger month
    and a linked `AGENTS.md` are read through (`status` events 1,
    `rules` ok, `ledger` ok) and still named by `layout`
  - each command runs with a 30 s kill and `RLIMIT_AS` 2 GiB, so a
    regression fails fast instead of filling the host's memory
- Grep test `only_files_outside_the_logbook_are_read_without_the_bound`:
  every `fs::read(`, `fs::read_to_string(` and `File::open(` in
  `engine/src`, counted per file. It skips the unit tests (the file
  after its first `#[cfg(test)]` + `mod`) and comment lines. Every file
  that still has such calls must be on an allow-list with what it reads,
  all outside the logbook: config, state, `/proc`, collectors, the skill
  folders, the setup kit, the import sources, and `sys.rs` itself.
  Modelled on WP-171's writer test. `OpenOptions` is not counted: those
  are writers, and WP-171's test covers them.
- Mutation check by hand: `open_regular` reduced to a plain
  `File::open`. Three tests hit the 30 s kill (FIFO) and the grep test
  failed. Restored afterwards.

**Docs**:

- SPEC-ENGINE §3: a new "Bounded reads (WP-174)" paragraph right after
  the `layout` row, since that row is what the guarantee protects.
- The `fences` row text in §3 is updated.
- §2's "Reading through a linked folder or file is unchanged" sentence
  now points to the new paragraph.
- CHANGELOG, Unreleased › Engine.
- No ADR: ADR-0049 §2 still holds as written. No contract change.

## Verification

- `SELDON_FULL_CHECK=1 just check`: `check: ok`, exit 0. 2670 engine
  tests passed, 0 failed. Plugin harnesses all green (desk-view 1808,
  bar-view 196, ipc-restart 44, service-states 344). Log:
  `jax-seldon-private/gates/engine174-check.log`.
- The first full run (`engine174-check-run1.log`) had 2 failures in
  `desk-view` `drift-already #5`. That harness uses the fake engine
  (`FAKE_SELDON_FIXTURE`) and does not exercise this change. Run alone
  it passed 1808/0 (`engine174-deskview.log`), and the second full run
  passed too: a timing flake.
- `just check-perf`:
  - index ×10 13.7 ms and ×150 80.0 ms (budget 100)
  - capture costs all well within budget
  - hook "not recorded" 0.86 ms, unrelated session < 1 ms
  - **2 failures**: hook "recorded (tmpfs)" 12.3 ms (10 000 lines) and
    20.7 ms (900 lines), budget 5 ms.
  - Cause: the test assumes its temp dir is on tmpfs, and my `TMPDIR` is
    on disk by the rules (fsync cost). On a throwaway worktree of the
    base `next` commit 0f8d77db, same settings, the same two fail with
    12.5/12.2 ms and 20.0/20.2 ms. So this branch adds nothing measurable
    there. Log: `engine174-perf.log`. These two need a tmpfs run (CI or
    the test host) for a real number.
- Environment: `CARGO_TARGET_DIR`
  `jax-seldon-private/gates/target-engine174`, `TMPDIR` `gates/tmp-engine174`,
  private 0700 `XDG_RUNTIME_DIR` `gates/run-engine174`, all on disk.
  FIFO and `/dev/zero` only inside temp logbooks, with a time limit. The
  real `~/Seldon`, `~/.local/state/seldon` and `~/.config` were not
  touched.

## Open questions and observations

1. Exit code: a ledger month that is a FIFO, a device or over the cap
   makes `status` exit **2** (engine error), the same path as any
   "cannot read" before. Exit 1 ("the fix is in your files") would match
   WP-171's refusals better, but needs a typed error through `Ledger`.
   I left it at 2. The orchestrator decides.
2. A capture with an unreadable `AGENTS.md` skips the silent upgrade
   without a warning. That is pre-existing; doctor's `rules` and
   `layout` rows name the file. A warning would be a small follow-up if
   wanted.
3. Pre-existing, not changed: for a FIFO `AGENTS.md`, doctor's `logbook`
   row says "missing: AGENTS.md" (`layout::missing` uses `is_file`).
   This is misleading next to the `layout` row; candidate for a follow-up.
4. `O_NONBLOCK` stays set on the open regular file. On Linux it has no
   effect on reading a regular file. A FUSE filesystem could in theory
   behave otherwise; I see no case in a logbook.
5. The 16 MiB cap also applies to the user-wide Claude Code settings
   file and to the logbook's `.git` `packed-refs`. Both are far below it
   in practice. A `packed-refs` over the cap only makes `git.head`
   unknown in the index.

## Round 2 (review 1: APPROVE with items; orchestrator decisions)

- **F1, a next step on every refusal.**
  - The reader's `InvalidInput` text ends "…; not read: make it a regular
    file and run the command again".
  - Doctor's `ledger` row has a fix for a refused month (`error`), and
    for a large one (below).
- **Q4, exit 1.** A ledger month the reader refuses (no regular file, or
  over the cap) now exits **1**, like WP-171's refusals.
  - How: `ledger::month_read_error` wraps it in the new
    `error::Refused`.
  - `impl From<anyhow::Error> for Error` replaces the old `#[from]`: an
    error chain that contains `Refused` becomes `Error::User`, anything
    else stays `Engine`. Only `read_month` and the hook's `read_end`
    produce `Refused`, so no other exit code changes.
  - Over the cap, the message adds "keep a copy of it, remove lines you
    can do without by hand (… plain JSON Lines …) and run the command
    again".
  - Other logbook files keep their earlier behaviour (the rules row
    degraded, views skipped).
- **F2.**
  - The 256 MiB read cap is kept. No append refusal.
  - Doctor's `ledger` row is `degraded` for a month of 128 MiB or more
    (`sys::LEDGER_MONTH_WARN`): "ledger/<m>.jsonl is N MiB: Seldon reads a
    ledger month of at most 256 MiB and refuses a larger one (status,
    doctor and the index stop on it)". The fix: keep a copy and trim by
    hand before it reaches the limit.
  - It combines with the bad-lines message.
  - Documented in SPEC-ENGINE §3 (bounded reads and the `ledger` row)
    and in the CHANGELOG.
- **F3.** `open_checked` clears `O_NONBLOCK` with `fcntl(F_GETFL/F_SETFL)`
  (an extern, like `mkfifo`; constants 3/4) once the fd is known to be
  regular. The unit test asserts the flag is off.
- **F4.** `read_small_file` opens with `open_checked` after its `lstat`.
  That closes the swap race for every caller: triage's journal day, the
  proposals, `autocommit.json`, the git config files of the plugins
  collector. The SPEC sentence about the grep test is now true: `sys.rs`
  is listed as "the checked opens".
- **F5.** The unit test's FIFO, `/dev/zero` and directory assertions run
  in a thread with a 10 s `recv_timeout`. `read_small_file` is checked
  there too.
- **F6.** The grep test now:
  - also counts `File::options(`, `.read(true)` and an import
    `use std::fs::{…read…}` (for a bare `read_to_string(`)
  - skips each `#[cfg(test)]` module from its attribute to its closing
    `}` at column 0, so production code after a test module counts
  - still counts per file, not per call site; changing that would need
    line anchors that move with every edit.
- **Tests added:**
  - `a_ledger_month_past_the_cap_is_refused_and_one_near_it_is_named`,
    with sparse files:
    - 129 MiB: doctor `degraded` with the size, the cap and the fix,
      exit 0
    - 256 MiB + 1: `status` exit 1 with the remedy; doctor `ledger`
      error with a fix, exit 1
  - The FIFO and `/dev/zero` test now expects exit 1, the remedy, and
    the doctor fix.

**Round 2 verification.**
- `SELDON_FULL_CHECK=1 just check`: `check: ok`, exit 0. 2674 engine
  tests passed, 0 failed. desk-view 1808/0, bar-view 196/0,
  ipc-restart 44/0, service-states 344/0. Log:
  `gates/engine174-check-r2.log`.
- The first round-2 run (`engine174-check-r2-run1.log`) failed 4 desk-view
  checks (`aside-openfail`, `aside-focused`). Both run on the fake engine,
  as in round 1. Run alone they passed 1808/0
  (`engine174-deskview-r2.log`), and the second full run is green: a
  timing flake under load. Worth a look by whoever owns desk-view.
- Same environment as round 1: target, `TMPDIR` and a private 0700
  `XDG_RUNTIME_DIR`, all on disk under `gates/`. The real `~/Seldon`,
  `~/.local/state/seldon` and `~/.config` were not touched.
- `check-perf` was not re-run. Round 2 adds one `fcntl` per open and
  one `metadata` per month in doctor only.

## Follow-up (not this WP)

- **F7: a FIFO at the logbook's `.git/HEAD`.** The engine's own
  `git_head_fast` no longer blocks. The `git` children (`rev-parse`,
  `status`, the autocommit) still block on it until the engine's 10 s
  git timeouts: the reviewer measured `status` 40 s, `doctor` 30 s,
  `log` > 20 s, `capture`/`index` 10 s. The `layout` row does not name
  `.git/HEAD`. Candidates:
  - check `.git/HEAD` (and `.git` itself) for a regular file before any
    git call, and skip git with a warning otherwise;
  - name `.git/HEAD` in the `layout` row.
- Handover round 1, items 2 and 3 are still open: a capture with an
  unreadable `AGENTS.md` gets no warning, and the `logbook` row says
  "missing: AGENTS.md" for a FIFO there.
