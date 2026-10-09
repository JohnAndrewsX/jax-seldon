# WP-171 — Plan

Branch `wp/171-file-links` from `next` (48924e49, WP-168 merged). A file
of the logbook the engine writes (replaced or appended) must be a regular
file inside the logbook; a symbolic link there (the write would land
wherever it points) or anything else that is no regular file is refused
with exit 1 and a reason, before anything is written. One rule with
WP-168's folders. `doctor` gets a `layout` row naming linked folders and
files, so the user learns before a write is refused.

## The decision (ADR-0049, proposed)

Refuse, as WP-168 does for folders (the WP's recommendation, Fable's
WP-168 stage 2 §4). Not chosen: replacing the link with a real file
(`write_atomic_replace`). It is silent: the user's link (a deliberate
`STATUS.md -> ~/Dashboard/…`) disappears without a word, and the engine
cannot tell a deliberate link from a planted one. A refusal names the file
and the fix; the `doctor` row finds every one at once.

ADR-0049 also records the residual for WP-168 and WP-171 (Fable F4):
the check is `lstat`-then-create without `O_NOFOLLOW`.

No earlier ADR says links are followed (SPEC-ENGINE §2 does; specs are
normative and edited here), so ADR-0049 supersedes nothing.

## The helper

`logbook::checked_file(root, relative)` (free function, as
`checked_dir`) and `Logbook::checked_file(path)` (existing; now calls it):

- the folder of the file: `checked_dir` (WP-168, unchanged);
- the file itself, with `symlink_metadata` (never followed): a regular
  file or missing → `Ok(root/relative)`; a symbolic link → user error
  "`<file>` is a symbolic link, not a file of the logbook; make it a file
  and run the command again"; anything else (a directory, FIFO, socket,
  device) → the same with "no regular file"; another error → exit 2 with
  the path.
- `relative` plain names only (as `checked_dir`), at least one.

Every current caller of `Logbook::checked_file` names a file the engine
writes, so the stricter check applies wherever the folder check applies
today (journal day, case files from and to, `.seldon/active-case`,
`areas/<area>/README.md`, the views, `outputs/REBUILD.md`, the import's
planned files, its report, marker and undo file, the task marker,
`write_new`).

## The write primitives (never follow a link)

`sys::write_atomic` resolves the final link on purpose (`MAX_LINKS`,
SPEC §2 "the link stays and its target is replaced"): right for
`config.toml` and Claude Code's `settings.json`, which dotfile managers
link, wrong for the logbook. New in `sys`:

- `write_atomic_nofollow(path, bytes)` and
  `write_generated_nofollow(path, bytes)`: no link resolution; the mode
  comes from `symlink_metadata` of a regular file (else 0600); a link or a
  non-regular file at `path` is an error (exit 2: the caller's
  `checked_file` should have named it first). Should a link appear after
  the check, the rename replaces the link itself, never its target.
- `ledger::append_file`: `symlink_metadata` first, a link or non-regular
  file is an error (the open follows a link; std has no `O_NOFOLLOW`
  without `libc`, which is not an allowed crate). `Ledger::append` checks
  every month file with `checked_file` before the first append.

`write_atomic` and `write_generated` keep following links and are used
only outside the logbook (config, state, settings, agent skills).

## Writers (each gets the file check and the non-following primitive)

| Writer | File(s) | Check before the ledger / first write |
|---|---|---|
| `Ledger::append` | `ledger/<month>.jsonl` | inside, all months first |
| `journal::prepare` → `Pending::write`, `journal::ensure_day` | `journal/YYYY/<day>.md` | inside (prepare runs before the ledger) |
| `CaseFile::save` (write, move) | the case file, from and to | `CaseFile::checked` (prepare runs before the ledger) |
| `cases::set_active_case`, `clear_active_case` | `.seldon/active-case` | inside |
| `cases::ensure_area` | `areas/<area>/README.md` | inside |
| `decide accept` | `decisions/ADR-*.md` | **new**: before the ledger note |
| `index::views::write_if_changed` | `ledger/*.md`, `STATUS.md`, `DECISIONS.md` | inside |
| `rebuild` | `outputs/REBUILD.md` | before the write |
| `dossier` (`seldon dossier`, import) | `system/*.md` | **new**: every changed file before the write (import: its `check_folders`) |
| `import --apply` | days, memory, report, marker, undo | `check_folders` (before the ledger) |
| `import task` marker | `.seldon/imports/…` | before anything |
| `rules update` | `AGENTS.md` | **new**: before the archive copy |
| capture's silent rules upgrade | `AGENTS.md` | **new**: a warning, the capture goes on (as for every other reason the upgrade cannot write) |
| `setup` kit copy | `.claude/…` files | **new**: each target file; a link there is refused, not "kept" (`exists()` followed it; a dangling link made `fs::copy` create the file outside) |

Already safe, unchanged: `commands::write_new`, `layout::create`, the
rules and skills archives (`create_new` = `O_CREAT|O_EXCL`, which never
follows a final link); `git` refuses paths beyond a link itself.

Out of scope (outside the logbook): state files, `config.toml`,
`hook install --settings` (WP-168 N4: the user names the path), agent
skill folders. WP-166's inbox writer (not on `next` yet) uses
`write_new` (`O_EXCL`); whichever merges second checks it.

Reading through a linked file is unchanged (as WP-168 for folders): the
index still shows a linked journal day; only writes are refused.

## `doctor`: the `layout` row

After the `logbook` row's siblings (appended after `pending reset`, so
the earlier rows keep their places). Read-only, no lock:

- the folders the engine writes in (SPEC §2 list): each part checked as
  `checked_dir` does (a link or no directory is named);
- the files the engine writes there: every entry directly in
  `decisions/`, `work/{queued,active,completed}/`, each `journal/<year>/`,
  `ledger/`, each `areas/<area>/`, `system/`, `outputs/`, `memory/`,
  `.seldon/`, `.seldon/imports/`, and the root files `AGENTS.md`,
  `STATUS.md`, `DECISIONS.md`: a link or a non-regular file is named (a
  sub-folder is not looked into; workpiece folders `work/C-…/`,
  `archive/` contents and `.claude/` contents are the user's or are
  written with `O_EXCL`);
- `ok` "no linked folders or files where Seldon writes"; otherwise
  `error` "N where Seldon writes: <rel> (symbolic link), <rel> (no
  regular file), …" (the first 5, "and K more"), fix "replace each with a
  real folder or file (move the link's target into its place), then run
  the command again".
- error, not degraded: every command that writes there exits 1 (a linked
  `ledger/` stops `status` and `capture`).

The doctor JSON shape is not part of `schema/` (SPEC-ENGINE §3): no
contract change.

## Docs

- `decisions/ADR-0049-logbook-files-are-not-links.md` (proposed).
- SPEC-ENGINE §2: the "followed" sentence replaced (logbook files are
  never written through a link; outside the logbook links are followed as
  before); the "Linked folders" paragraph becomes "Linked folders and
  files"; §3 doctor list gains `layout`.
- SPEC-LOGBOOK where it lists links (if it does), TESTING, CHANGELOG
  (Unreleased › Breaking, next to WP-168's entry), user docs
  (troubleshooting, CLI reference doctor rows) en and de.

## Tests

- Unit `logbook::`: `checked_file` free function: regular, missing,
  link (to a file, dangling, to a directory), directory, FIFO, folder
  link still named first, plain names only.
- Unit `sys::`: `write_atomic_nofollow` / `write_generated_nofollow`
  refuse a link (target untouched, link kept) and a FIFO, keep the mode
  of a regular file, create 0600; `write_atomic` still follows (the
  existing `file_writes` tests).
- Unit `ledger::append_file` refuses a link.
- Integration `tests/linked_files.rs`, one per writer: the file replaced
  by a link to a copy outside the logbook (and a dangling link where the
  writer would create it): exit 1, the reason, the outside file and the
  logbook (tree) unchanged, the ledger unchanged; a FIFO for the journal
  day (no hang). `primitives` module: the library writers on their own.
  Capture with a linked `AGENTS.md`: exit 0, the warning, the target
  unchanged. `doctor`: ok row, a linked folder, a linked file, a FIFO,
  the cap of five.
- Hand mutants (`work/active/WP-171/mutants.py`, WP-168's runner), each
  against a copy of the tree and scratch roots only.

## Safety

All test temp dirs under an on-disk `TMPDIR`
(`engine/target/tmp-wp171`), `CARGO_TARGET_DIR` in the worktree's
`engine/target`; the check with `XDG_RUNTIME_DIR=/tmp/r171` (0700,
removed afterwards); no run against a real logbook.
