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
- **After the merge of `next` (b4a11dba: WP-159, WP-138)**, merge commit
  ee341011: one conflict, `DECISIONS.md` (next's rows for ADR-0045
  accepted, ADR-0047, ADR-0048, then ADR-0049). This branch never used
  `import::is_direction_or_format` (only `bad_path_char`, still on
  `next`); WP-159 and WP-138 bring no new logbook writer (grep over the
  diff: test writes only). The German CLI reference's source line moves
  to the merge (its WP-138 text came with `next`). **`next` itself is
  not `cargo fmt` clean** (`commands/preview.rs:331`, the `shown()`
  condition of b4a11dba): `just check` stopped at `fmt-check` (log
  `check-wp171-dev-r3.log`); 71b3f5df is that `cargo fmt` alone, the
  same change a fix on `next` would make. **`just check` at 71b3f5df:
  exit 0, `check: ok`**, 102 test binaries, 2646 passed, 0 failed;
  real-home-guard 40/0, service-states 344/0, desk-view 1808/0,
  bar-view 196/0, ipc-restart 44/0, docs-check ok. Log:
  `check-wp171-dev-r4.log`. The mutants ran before the merge (the merge
  touches none of the mutated lines).
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

## Round 2 (review 1: SEND BACK, small)

Packet: `review-0.1.1/handovers/WP-171-review-1.md`; the orchestrator's
calls on F1–F5 and Q2–Q4.

- **F1 (the row names only what the engine writes).** `layout::written(rel)`
  is now the one list of every writer's files (`AGENTS.md`; the views
  `STATUS.md`, `DECISIONS.md`, `ledger/<YYYY-MM>.md`;
  `ledger/<YYYY-MM>.jsonl`; `journal/<YYYY>/<YYYY-MM-DD>.md`;
  `work/<status>/C-*.md`; `decisions/ADR-*.md`; `areas/<area>/README.md`;
  the dossier's fence files in `system/`; `memory/*.md`;
  `outputs/REBUILD.md`, `outputs/IMPORT-*.md`; `.seldon/active-case`;
  `.seldon/imports/{omarchy-agent.json, omarchy-agent.undo.json,
  tasks.json}`). In `areas/<area>/`, `outputs/`, `journal/<YYYY>/` and
  `.seldon/` only those are named: the reviewer's linked case template,
  area note, `outputs` report and journal attachments now give `ok`
  (`tests/doctor.rs::the_layout_row_names_only_what_seldon_writes`).
  Every entry is still named in `decisions/`, the status folders,
  `ledger/`, `system/`, `memory/`. **The tie:** every refused or skipped
  file in `tests/linked_files.rs` (all writers, every swap) must be
  named by `layout::misplaced` with the matching kind and refusal
  (`named_by_doctor`); a writer whose file the list lacks fails there.
  SPEC-ENGINE §3 and ADR-0049 §3 now say exactly that.
- **F2.** A directory where the engine writes a file is named "no
  regular file" (`ledger/2026-10.jsonl/`, a day, an ADR, an import
  report).
- **Q3.** `error` only for what a command refuses; a skipped view or
  another link beside Seldon's files is `degraded` on its own (doctor
  exit 0), and listed after the refused ones ("; also M where Seldon
  writes but refuses nothing …") when both occur. `layout::Found` carries
  `refused`.
- **F3/Q2.** A linked or non-regular `STATUS.md`, `DECISIONS.md` or
  `ledger/<month>.md` is skipped: `status` and `index` warn
  "`<file>` not updated: `<file>` is a symbolic link, …" (human and
  `--json` `warnings`), exit 0, and write `index.json`; a linked
  `ledger/` folder is still refused (WP-168). `views::checked_view`;
  `write_status`/`write_decisions_index`/`write_ledger_views` return the
  skip as `Fill::Skipped`. Test: `linked_files::the_views_are_skipped`
  (each view × link, dangling, folder, FIFO; `status` and `index`;
  `generatedAt` is the new time). The `warnings` key of `status --json`
  and `index --json` is not new, only the text (AGENTS.md §3 E42: no
  contract change).
- **F4.** `sys::open_append_nofollow`: `custom_flags(O_NOFOLLOW)` (the
  constant written out per architecture, as `SIGKILL`; generic
  `0o400000`, arm/aarch64/powerpc `0o100000`), `ELOOP` named as a link,
  and the open file checked to be regular (a FIFO opens `O_RDWR` without
  blocking and is refused). The ledger's append uses it after the check;
  the race is closed for files. Unit test calls it directly, with no
  check in front (link, dangling link, FIFO, regular, new). ADR-0049 §4
  reworded: proportion, not impossibility, for the folder walk.
- **F5.** `tests/rules.rs`: `Some(0)`.
- **Q4.** No writer on this branch writes `inbox/` (only `init`'s
  `.gitkeep` into an empty logbook), so `inbox` is not in the row; left
  to WP-166.
- **Merge of `next`** (dcc19901, 49dc118f): no conflict; it brings no
  logbook writer.

### Verification (round 2)

- **`just check` at 49dc118f**: same command and dirs as round 1 (runtime
  dir 0700, made and removed; temp dirs on disk). **exit 0, `check:
  ok`**, 102 test binaries, 2654 passed, 0 failed; real-home-guard 40/0,
  service-states 344/0, desk-view 1808/0, bar-view 196/0, ipc-restart
  44/0, docs-check ok. Log: `gates/check-wp171-dev-r5.log`.
- **Mutants** (76: the round-1 set, new ones for `written`, the walk, the
  row's classes, the skipped views, the append open), against a copy
  at 49dc118f, target `gates/target-wp171`: **71/76 killed**, no run hit
  the time limit; the 5 survivors are the second layer at single call
  sites, as in round 1. Log: `gates/mutants-wp171-dev-r3.log`.
- Copy, target dir, temp dirs and `/tmp/r171` deleted (explicit paths,
  looked at first). No guard-hook block; never run against a real
  logbook.

## Round 3 (Fable stage 2: APPROVE WITH NITS)

Packet: `review-0.1.1/handovers/WP-171-stage2-fable.md`.

- **N2a.** The setup kit's copy no longer uses `std::fs::copy` after the
  check: `setup::copy_new` makes each file with `O_CREAT|O_EXCL` and the
  kit file's mode (exact, umask ignored), removes a half-written copy, and
  a file made since the check (a link included) counts as kept. Unit test
  `copy_new_never_follows_a_link` calls it with no check in front (link,
  dangling link: `AlreadyExists`, nothing written where they point; a new
  file 0750 like its source). So ADR-0049 §4's "Files are closed" holds;
  §4 now names the three mechanisms (rename, `O_NOFOLLOW` append,
  `O_EXCL` for new files).
- **N2b.** ADR-0049 §3: the kit's files under `.claude/` are refused by
  `setup` itself and not listed in the row (it checks the folder).
- **N2c.** §4 starts "For folders, the check is `lstat`-then-create …".
- **N3.** `linked_files::only_files_outside_the_logbook_are_written_through_a_link`
  counts every call of `write_atomic`/`write_generated` (and
  `_mode`/`_replace`) per file of `engine/src` against a list of files
  outside the logbook, each with what it writes. The five layer-2
  mutants are now killed by it. SPEC-ENGINE §2 and TESTING say so.
- **N1** (readers that open a FIFO or read `/dev/zero` at a ledger month,
  `AGENTS.md`, `STATUS.md`, `DECISIONS.md`) is not this WP; the
  orchestrator queues it.
- **Merge of `next`** (8e78f356, 80e29750): no conflict, new WP files
  only.

### Verification (round 3)

- **`just check` at 80e29750** (runtime dir 0700, made and removed; temp
  and target on disk; `SELDON_FULL_CHECK=1`): **exit 0, `check: ok`**,
  102 test binaries, 2658 passed, 0 failed; real-home-guard 40/0,
  service-states 344/0, desk-view 1808/0, bar-view 196/0, ipc-restart
  44/0, docs-check ok. Log: `gates/check-wp171-dev-r6.log`.
- **Mutants** (77, round 2's plus the copy), copy at 80e29750, target
  `gates/target-wp171`: **77/77 killed**, the five layer-2 mutants by the
  call-site list; no time limit hit. Log: `gates/mutants-wp171-dev-r4.log`.
- Copy, target, temp dirs and `/tmp/r171` deleted (explicit paths,
  looked at first: empty). No guard-hook block; never run against a real
  logbook.
