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
