# WP-168 — Handover

Branch `wp/168-linked-folders` from `next` (928e9430). Not pushed (the
orchestrator pushes).

## What was done

**The helper** (`engine/src/logbook/mod.rs`): `logbook::checked_dir(root,
relative)` and `Logbook::checked_dir(relative)`, as `triage::checked_dir`
(WP-124) for the state folder:

- every part of `relative` below the root is looked at with
  `symlink_metadata` (never followed): a directory → the next part; a
  symbolic link → exit 1 "`<part>` is a symbolic link, not a folder of
  the logbook; make it a folder and run the command again"; anything else
  (file, socket, FIFO, device) → the same with "no directory";
- the first part that does not exist ends the check (`Ok`): the writer
  creates it, 0700;
- another `stat` error (e.g. a name too long) → exit 2 with the path;
- `relative` must be plain names: `..`, `.`, an absolute path → exit 2
  (a caller's bug), checked before the walk (the first test run caught
  `a/../b` passing when `a` was missing);
- the part named is the first offending one, relative to the root
  (`work` when `work/` is the link and `work/active` was asked): no home
  path in the message;
- the root itself is not checked: a logbook behind a link (a synced
  folder) stays the user's choice.

`Logbook::checked_file(path)` checks the folder of a file (absolute below
the root, or relative) and returns the absolute path; a file in the root
(`AGENTS.md`, `STATUS.md`, `DECISIONS.md`) has nothing to check; a path
outside the root is exit 2. `cases::checked_folders(logbook)` checks the
three case folders.

**For WP-166:** `checked_inbox(ctx, &logbook)` can become
`logbook.checked_dir("inbox")?` — same check, same exit code, same "make
it a folder" fix. Differences: the message names `inbox` relative to the
logbook ("inbox is a symbolic link, not a folder of the logbook; …")
where WP-166 shows `~/…/inbox` and says "not the logbook's inbox folder";
WP-166's tests that match the message would change with it.
`inbox.rs` is not touched here; the unit tests of `checked_dir` include
`inbox` (a dangling link).

**`Logbook::open`:** a `.seldon` that is a file (ENOTDIR on
`.seldon/logbook.toml`) is now "not initialised" (exit 3) like a missing
one, where it was an engine error (exit 2); `is_initialised` already said
false for it.

## Every writer and its check

Found with `grep -rn 'create_dir\|OpenOptions::new\|write_new(\|write_atomic\|create_new_private\|write_generated\|fs::write(\|fs::rename(\|fs::copy('`
over `engine/src` (test modules left out). The check sits in the write
primitive that knows the logbook, and where a command writes the ledger
before a file, or reads a folder before it writes, also before that.

### Into the logbook

| Site | Folder | Check |
|---|---|---|
| `commands/mod.rs` `write_new` (decide new, plan new, drift explain, import cases) | the file's folder | `logbook.checked_file(path)` inside (signature now takes the logbook) |
| `logbook/cases.rs` `CaseFile::save` (`write_atomic`, `create_dir_private`, `rename`) | `work/<status>` from and to | `checked()` (shared with `prepare`, which runs before the ledger): `checked_file(&self.path)` and `checked_file(work/<to>/<name>)` |
| `cases::set_active_case` (`write_atomic`), `clear_active_case` (`remove_file`) | `.seldon` | `checked_file(ACTIVE_CASE_FILE)` inside |
| `cases::ensure_area` (`write_atomic`) | `areas/<area>` | `checked_file` inside, also when the area exists (one rule) |
| `logbook/journal.rs` `prepare` → `Pending::write`, `ensure_day` | `journal/YYYY` | `checked_file` inside; `prepare` runs before the ledger |
| `ledger.rs` `Ledger::append` (`create_dir_private`, `OpenOptions` append) | `ledger` | `checked_dir(root, "ledger")` before the month files; `Ledger::new` keeps the root (`Ledger::at`, tests only, has none) |
| `index/views.rs` `write_if_changed` (`ledger/*.md`; `STATUS.md`, `DECISIONS.md` in the root) | `ledger` | `checked_file(rel)` inside; `write_ledger_views` now returns `crate::error::Result` so the refusal stays exit 1 |
| `commands/decide.rs` new (`write_new`) and accept (`write_atomic`, after the ledger note) | `decisions` | `checked_dir("decisions")` right after the lock, before the next id is read / the note is written |
| `commands/plan.rs` `create` (new, reopen, import task) | `work/*`, `areas/<area>`, `.seldon` | `checked_folders` + area + `.seldon` (when it points) before the next id and the ledger |
| `plan` start/verify/done/drop | `work/*`, from and to, `.seldon` | `checked_folders` after the lock, before `find`; from/to and `.seldon` again before the ledger |
| `plan set`, `plan snapshot`, `plan reopen` | `work/*` | `checked_folders` after the lock; then `prepare`/`save` |
| `commands/log.rs` with `--case`, `commands/event.rs` with `--case` | `work/*` | `checked_folders` after the lock; `prepare` before the ledger |
| `commands/drift.rs` `run` (link, explain, dismiss) | `ledger`; `work/*` (link, explain); `areas/<area>` (explain) | after the lock, before `index::derive` reads them |
| `drift.rs` `write_resolution` (also `drift apply`, WP-124) | the new case's folder, `areas/<area>` | before the ledger (explain); link: `prepare` |
| `commands/dossier.rs` → `dossier::Files::write` | `system` | `checked_dir("system")` before `Files::read` |
| `commands/rebuild.rs` (`write_generated`) | `outputs` | `checked_file(REL_PATH)` before the read |
| `commands/import.rs` apply (`write_new`, days, memory, dossier, marker, undo, report) | `ledger work/* journal memory system outputs .seldon/imports` | `IMPORT_FOLDERS` before the plan reads them (after the half-done check, whose message stays first); every planned path (`check_folders`) before the ledger |
| `import.rs` `write_report` (dry run too) | `outputs` | `checked_file` inside |
| `commands/import/task.rs` `write_marker` | `.seldon/imports` | `checked_file(&marker_rel)` after the lock, before the first write (not in a dry run) |
| `commands/rules.rs` `archive` (`create_dir_private`, `create_new_private`) | `archive` | `checked_dir("archive")` inside; first write of `rules update` |
| `commands/skills.rs` `Archive::dir`/`copy` (`--replace`) | `archive`, `archive/skill-…/<label>` | `checked_dir` inside both |
| `commands/setup.rs` `copy_tree` (kit into `.claude/`) | `.claude/…` per file | `checked_dir(root, .claude/<sub>)` before each copy; the failure is the harness row's reason (setup reports, it does not exit) |
| `case_notes.rs:318`, `reconcile.rs:327/1056`, `hook.rs:1458` (`file.save`) | `work/<status>` | `CaseFile::checked`; these run after the ledger and report a refused save as a warning, as before |
| `hook.rs:1525` `journal::append`, `hook.rs:1599` views | `journal/YYYY`, `ledger` | inside `prepare`, `write_if_changed` |
| `agent.rs:734/1282` `set_active_case` | `.seldon` | inside |
| `logbook/layout.rs` `create` (`create_dir_private`, `write_new`) | all | none needed: `init` requires an empty or new root (`is_vacant`), where nothing can be a link |
| `capture.rs:464`, `rules.rs:113` (`AGENTS.md`) | the root | nothing to check |

### Not the logbook (out of scope)

State and config: `config.rs:904`, `init.rs:1040`, `capture.rs:153`
(config file); `collectors/mod.rs:480`, `collectors/config.rs:330/419`,
`capture.rs:316` (cursors, baselines); `index/mod.rs:79` (`index.json`);
`index/autocommit.rs:58`; `agent.rs:962/1327` (launches); `hook.rs:2180`
(migration marker); `logbook/lock.rs` (lock); `triage.rs` (proposals,
WP-124's own check). Outside the home's logbook: `hook.rs:1713/2012`
(Claude Code settings), `skills.rs:400` (agent skill folders; foreign
links never touched, WP-094), `setup.rs:669` (theme hook script).

## Tests

- Unit (`logbook::tests`, 6): real/missing folders and parts, the root;
  link (to a folder, dangling), file, socket, a link and a file in the
  middle, the part named; `..`, `a/../b`, `/etc`, `./a` refused; the root
  behind a link accepted; ENOTDIR below a file, a name too long (exit 2);
  `checked_file` relative, absolute, root file, outside the logbook.
- Unit (`setup::tests::copy_tree_keeps_existing_files_and_modes`): a
  linked `.claude/skills` stops the copy, nothing written through it.
- Integration `engine/tests/linked_folders.rs` (19 tests), each folder
  with a link to a copy outside the logbook and with a file in its place:
  exit 1, the reason names the folder, the logbook tree (through the link
  included) and the ledger unchanged: `decisions` (new, accept), `work`
  (new, start), `work/queued`, `work/active` (start, `log --case`,
  `plan set`), `work/completed` (done), `journal`, `journal/2026`,
  `ledger` (log, plan new), `ledger` views (`index`; link only: with a
  file there the index cannot read the ledger), `areas`, `areas/editors`
  (new, set), `.seldon` (link: exit 1; file: exit 3), `.seldon/imports`
  (`import task`), `system` (dossier), `outputs` (rebuild), `archive`
  (`rules update`), `memory`, `work/completed`, `system`, `outputs`
  (`import omarchy-agent --apply`), `drift explain` (`work`,
  `work/completed`, `areas`, `areas/dev-env`, `ledger`), `drift link`
  (`work/queued`, `work/active`, `ledger`), `event --case` (`work/queued`,
  `ledger`). `inbox` has no writer on `next` (WP-166's).

## Mutants

MUTANTS_RESULT

## Check

CHECK_RESULT

## Not done / open questions

- **Reads are unchanged.** A command that only reads goes through a
  linked folder as before (the index, `plan show`, `open`); only writes
  are refused. A *file* in the place of a folder still fails a command
  that reads it before it writes anything with exit 2 ("cannot list …:
  Not a directory"), e.g. `seldon index` with a file at `ledger`; `drift
  apply` was not tried that way. Where a writing command reads first, the
  check was moved in front of the read (case commands, `drift
  link|explain|dismiss`, `import --apply`, `decide`).
- **doctor** has no row for a linked folder yet; a user finds out at the
  first refused write. A `layout` row naming linked folders would be a
  small follow-up.
- **Stricter than strictly needed, on purpose (one rule):** a command
  that writes a case refuses when any of the three case folders is a link
  (a case moves between them, the next id reads all three); `ensure_area`
  refuses a linked area even when its README exists and nothing would be
  written; `import --apply` refuses all its folders before planning.
- The pacman/Claude Code hooks with a linked `ledger/`: the capture exits
  1 with the reason (hooks report on stderr and record nothing), as for
  any other refused write.
