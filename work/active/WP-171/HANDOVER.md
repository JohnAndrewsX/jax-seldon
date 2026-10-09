# WP-171 — Handover

Branch `wp/171-file-links` from `next` (48924e49, WP-168 merged). Not
pushed (the orchestrator pushes).

## What was done

**Decision:** `decisions/ADR-0049-logbook-files-are-not-links.md`
(proposed; next free number after ADR-0048 on `next` and every open
branch, checked with `git ls-tree` over all local branches and the
worktrees). Refuse, as WP-168 does for folders: one rule for folders and
files. It records the accepted residual for WP-168 and WP-171 (Fable F4)
in §4, word for word as the WP asks, plus the file case (the ledger's
append opens the month file after the check). No earlier ADR said links
are followed (SPEC-ENGINE §2 did), so nothing is superseded; the SPEC
sentence is changed. `DECISIONS.md` has the row.

**The helper** (`engine/src/logbook/mod.rs`): `logbook::checked_file(root,
relative)` (new free function) and `Logbook::checked_file(path)` (now
calls it): the folder with WP-168's `checked_dir` first, then the file
itself with `symlink_metadata` (never followed). A regular file or none →
`Ok(root/relative)`; a symbolic link (dangling, to a file, to a folder) →
exit 1 "`<file>` is a symbolic link, not a file of the logbook; make it a
file and run the command again"; anything else (directory, FIFO, socket,
device) → "is no regular file"; another `stat` error → exit 2 with the
path; a path without a file name or with `..`/absolute parts → exit 2.
Every existing caller of `Logbook::checked_file` names a file the engine
writes, so they all got the stricter check.

**The write primitives** (`engine/src/sys.rs`): `write_atomic_nofollow`
and `write_generated_nofollow` never resolve a link (`write_atomic_at` on
the path itself) and refuse a link or a non-regular file at the path via
`sys::regular_or_missing` (exit 2: the command's `checked_file` names it
first; should one appear after that, the rename replaces the link, never
its target). A regular file keeps its mode, a new one is 0600.
`ledger::append_file` calls `regular_or_missing` before it opens the
month file. `write_atomic`/`write_generated` keep following links and
are now used only outside the logbook (config, state, Claude Code
settings, agent skills; `tests/file_writes.rs` still proves they
follow).

## Every writer of a logbook file

Found with `grep -rn 'write_atomic\|write_generated\|append_file\|OpenOptions\|fs::write(\|create_new_private\|write_new\|fs::copy\|fs::rename'`
over `engine/src` (test modules left out).

| Writer | File(s) | Check (before the ledger / first write) | Primitive |
|---|---|---|---|
| `Ledger::append` | `ledger/<month>.jsonl` | **new**: every month file of the batch, before the first line | `append_file` refuses a link |
| `journal::prepare` → `Pending::write`, `ensure_day` | `journal/YYYY/<day>.md` | `checked_file` inside (now the file too) | nofollow |
| `CaseFile::save` / `prepare` | the case file, from and to | `CaseFile::checked` (now the files too) | nofollow; the move is a `rename`, which never follows |
| `plan start|verify|done|drop` | the case file | **new**: `file.prepare` before the ledger (it had only the folder check; a linked case file got the ledger line, then a refused save) | — |
| `cases::set_active_case`, `clear_active_case` | `.seldon/active-case` | inside (now the file too) | nofollow |
| `cases::ensure_area` | `areas/<area>/README.md` | inside; **new**: `plan new --area` and `drift explain --area` check the README (not only its folder) before the ledger | nofollow |
| `decide accept` | `decisions/ADR-*.md` | **new**: `checked_file(&path)` before the ledger note | nofollow |
| `decide` (new) → `fill_index` | `DECISIONS.md` | after the decision is written: a warning, as before for every fill failure | nofollow |
| `views::write_ledger_views`, `write_status`, `write_decisions_index` | `ledger/*.md`, `STATUS.md`, `DECISIONS.md` | `write_if_changed` inside; **new**: `write_status`/`write_decisions_index` check before they read (a link not read, a FIFO not opened) and return `crate::error::Result` so the refusal is exit 1, not 2 | nofollow |
| `rebuild` | `outputs/REBUILD.md` | before the read (now the file too) | nofollow |
| `dossier` | `system/*.md` | **new**: every changed file before the write | nofollow |
| `import omarchy-agent --apply` | days, memory, report, marker, undo file, dossier files | `check_folders` (now the files too), before the ledger; **new**: `already_imported` checks the marker before it reads it (a linked marker was read as "already imported", exit 0) | nofollow |
| `import task` | `.seldon/imports/tasks.json` | before anything (now the file too) | nofollow |
| `rules update` | `AGENTS.md` | **new**: before the read and the archive copy | nofollow |
| capture's silent rules upgrade | `AGENTS.md` | **new**: `checked_file`; a refusal is the existing warning "AGENTS.md: Seldon's agent rules were not updated: …", the capture goes on | nofollow |
| `setup` kit copy | `.claude/…` files | **new**: `checked_file(root, to/rel)`; an existing link counted as "kept" and a dangling one made `fs::copy` create its target outside | `fs::copy` after the check |
| `write_new`, `layout::create`, rules and skills archives | new files | already `O_CREAT|O_EXCL` (never follows a final link); `write_new` checks with `checked_file`, so a link gives the reason, not "exists" | unchanged |

Not touched (outside the logbook): `index.json`, cursors, manifests,
`autocommit.json`, launches, the migration marker, `config.toml`,
`hook install --settings` (WP-168 N4: the user names the path), agent
skill folders.

**Reading** through a linked file is unchanged, as WP-168 for folders:
the index still shows a linked journal day; `case_files`,
`decision_files` and `Files::read` still list a linked file (a dangling
one is no file to them, so `decide accept` of a dangling decision says
"unknown decision").

**`doctor`'s `layout` row** (`layout::misplaced`,
`commands/doctor.rs::check_layout`), appended after the reset rows (the
earlier rows keep their places): every part of the folders the engine
writes in that is a link or no directory; every entry directly in
`decisions/`, the three status folders, `ledger/`, `system/`, `outputs/`,
`memory/`, and one level deeper in `journal/`, `areas/`, `.seldon/`,
that is a link or neither a folder nor a regular file; `AGENTS.md`,
`STATUS.md`, `DECISIONS.md`. `work/C-…/` workpieces, the contents of
`archive/` and `.claude/` and other root entries are not looked into.
`ok` "no linked folders or files where Seldon writes", else `error` with
the first five and "and K more", names cleaned as in the `workpieces`
row (the helper is now `shown_name`, shared), fix "replace each with a
real folder or file (move what the link points to into its place), then
run the command again". Error, not degraded (ADR-0049 §3). The doctor
JSON shape is not part of `schema/`: no contract change, no fixture
change.

**Docs:** SPEC-ENGINE §2 (the "followed" sentence limited to files
outside the logbook; "Linked folders and files"), §3 (`layout` in the
doctor list and its paragraph), SPEC-LOGBOOK §2, TESTING (the new test
file), CHANGELOG (Unreleased › Breaking, beside WP-168's entry), user
guide en and de (troubleshooting table: thirteen checks, CLI reference
doctor text; the German pages' source lines follow the English commit,
`docs-check` clean).

## Tests

- Unit: `logbook::tests` (`checked_file`: regular, missing, link to a
  file, dangling, to a folder, a folder in a file's place, a linked year
  named first, plain names; the method form), `sys::tests`
  (`write_atomic_nofollow`/`write_generated_nofollow` refuse a link, a
  dangling link and a folder, the target and the link unchanged, no temp
  file left, the mode kept, a new file 0600; `write_atomic` still follows;
  `regular_or_missing`), `ledger::tests` (a month file that is a link,
  dangling or a folder, with `Ledger::at`, i.e. without a root),
  `layout::tests` (`misplaced`), `setup::tests` (the kit copy with a link
  and a dangling link where a file goes).
- `engine/tests/linked_files.rs` (new, 15 tests): one per writer —
  journal day (existing and new), ledger month (`log`, `plan new`), case
  file (in place, moved from, moved to; `plan start|set|done`, `log
  --case`, `event … --case`), active case (`plan start`, `plan done`),
  area README (`plan new`, `plan set`), `decide accept`, the views
  through `status` and `index`, `rebuild`, `dossier`, `rules update`, the
  omarchy-agent import (report, marker, undo, deviations, a day; the dry
  run), the task import marker, `drift explain --area` on the fixture —
  each with a link to a copy outside, a dangling link, a directory and a
  FIFO where they apply: exit 1, the exact reason, the logbook and
  everything outside unchanged (a snapshot that follows no link and opens
  no FIFO), a dangling link's target not made. A command still running
  after a minute is killed by its own handle and fails the test (a FIFO
  opened). `primitives`: journal, active case, area README through the
  library.
- `tests/rules.rs`: a capture with a linked `AGENTS.md` warns, the
  target and the link unchanged. `tests/doctor.rs`: the `layout` row ok,
  two names, seven names (five and "and 2 more"), the human line, a name
  with control and direction characters.

Where a swap does not apply, the test says why: a dangling link or a
folder where a case or decision is found is no case/decision ("unknown
case"); a link to a file where a case moves to is read as a case of its
own first (exit 1, "invalid case"); a link to a file at a new import day
is read as the day (the plan reports its error). Nothing is written in
either case.

## Verification

- **`just check`** at b8f75f81 (the code as handed over; this
  handover commit adds only this file):
  `XDG_RUNTIME_DIR=/tmp/r171 SELDON_FULL_CHECK=1 JUST_TEMPDIR=<gates>/just-tmp-wp171
  TMPDIR=<gates>/tmp-wp171 flock /tmp/seldon-check.lock just check`, the
  runtime dir made 0700 and removed afterwards, every temp dir on disk.
  **exit 0, `check: ok`**: 100 test binaries, 2564 passed, 0 failed;
  real-home-guard 40/0, service-states 344/0, desk-view 1785/0, bar-view
  196/0, ipc-restart 44/0, docs-check ok. Log:
  `jax-seldon-private/gates/check-wp171-dev-r2.log` (r1, at an earlier
  commit of this branch, also ok: `check-wp171-dev-r1.log`).
  `/tmp` and `/run/user/1000` at 2–3 % throughout.
- **Hand mutants** (`work/active/WP-171/mutants.py`, 55), each against a
  `git archive` copy under `gates/`, target dir `gates/target-wp171`,
  the tests' TMPDIR in the copy (scratch roots only; no test touches a
  real path). **50/55 killed.** The 5 survivors are the second layer at
  single call sites (`Pending::write`, `CaseFile::save`, the views,
  `rebuild`, `Files::write` back on `write_atomic`/`write_generated`):
  `checked_file` refuses first; only a race reaches that layer, and the
  primitives themselves are killed by their unit tests (`sys: …`
  mutants). Logs: `gates/mutants-wp171-dev.log` (45/55: 3 doctor
  mutants survived because the runner's filter missed the doctor test by
  name, and 2 layout mutants showed real test gaps: no FIFO in a folder,
  nothing inside a linked folder), `gates/mutants-wp171-dev-r2.log`
  (after the fix of both, the 15 doctor and layout mutants: 15/15).
- **One incident, own test, no harm:** the first mutant run hung in
  `primitives::the_journal_is_not_written_through_a_link`: under the
  mutant that accepts a FIFO, `journal::append` opened the FIFO inside
  the test process (no child to time out). I stopped my own run by PID
  (the mutant runner, its cargo and the test binary, checked by their
  command lines first). The SIGTERM left that mutant in the copy's
  source, so the copy was deleted and made again. Fixed: the in-process
  call has a one-minute limit (`in_time`), the runner kills a run's
  process group after 15 minutes and restores the source on SIGTERM.
  Log of the stopped run: `gates/mutants-wp171-dev-aborted.log`.
- **Cleanup:** `/tmp/r171`, the gate copy, `gates/target-wp171` and the
  temp dirs are deleted (explicit paths, looked at first: empty). No
  guard-hook block. Never run against a real logbook.

## Not done / open

- **WP-166 merge** (merges before this one, per the WP): its inbox
  writer uses `create_new_private` (`O_EXCL`), which never follows a link,
  so nothing changes for it; `doctor`'s `layout` row does not list
  `inbox/` on this branch (not on `next` yet). Whichever merges second:
  add `("inbox", None)` to `layout::WRITTEN_FOLDERS` and `inbox/` to the
  SPEC-ENGINE §2 lists; expect textual conflicts in `doctor.rs`,
  SPEC-ENGINE §2/§3 and CHANGELOG.
- **Residual, recorded in ADR-0049 §4:** `lstat`-then-create/open; the
  ledger append follows a link swapped in after the check (std has
  `O_NOFOLLOW` only through `libc`, not an allowed crate). A replaced file
  is safe against that race (the rename never follows).
- **Two layers:** the second layer at single call sites (a nofollow
  primitive replaced by `write_atomic`) survives the mutants, as
  expected: the `checked_file` before it refuses first, only a race
  reaches it; the primitives themselves are killed by their unit tests.
- `doctor`'s `layout` row is an `error`, so a linked journal day from
  last year makes doctor exit 1 until it is replaced. ADR-0049 §3 says
  why; the orchestrator may prefer `degraded`.
- ADR-0049 is **proposed**; the operator accepts it.
