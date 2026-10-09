# WP-168 — Plan

Branch `wp/168-linked-folders` from `next` (928e9430). Every folder of the
logbook the engine creates or writes a file in must be a real directory
inside the logbook; a symbolic link or a non-directory there is refused
with exit 1 and a reason, before anything is written.

## The helper

`logbook::checked_dir(root, relative)` and its method form
`Logbook::checked_dir(relative)`, in `engine/src/logbook/mod.rs`, as
`triage::checked_dir` (WP-124) does for the state folder `proposals/`:

- walks `relative` part by part below the root: every part that exists
  is checked with `symlink_metadata` (never followed);
- a directory → next part; a symbolic link → user error "`<part>` is a
  symbolic link, not a folder of the logbook; make it a folder and run
  the command again"; anything else (file, FIFO, socket, device) → the
  same with "no directory";
- the first part that does not exist ends the walk: `Ok` (the writer
  creates it and its children with 0700, and creates no links);
- another `stat` error → engine error (exit 2) with the path;
- `relative` must be plain names (`Normal` components); `..`, an absolute
  path or a root/prefix component is an engine error (a caller bug);
- the root itself is not checked: a logbook kept behind a link (a synced
  folder) is the user's choice, and `Logbook::open` already went through
  it.
- The part named is the first offending one, relative to the root
  (`work` when `work/` is a link, `journal/2026` when the year is a file):
  no home path in the message.

`Logbook::checked_file(path)` checks the folder of a file `path` (absolute
below the root, or relative to it) and returns the absolute path; a file
directly in the root has nothing to check.

WP-166 can replace its `checked_inbox` by `logbook.checked_dir("inbox")`
(same check, same exit code, same "make it a folder" fix); `inbox.rs` is
not touched here.

## Where it is called (the writers)

The check sits in the write primitives that know the logbook, so a new
caller cannot forget it, and where a command writes the ledger before a
file, also before the ledger (a refused folder then writes nothing):

| Writer | Folder(s) | Where the check runs |
|---|---|---|
| `commands::write_new` (decide, plan new, drift explain, import cases) | the file's folder | inside (signature takes the logbook) |
| `CaseFile::checked` (`save`, `prepare`: every case write, move) | `work/<status>` from and to | inside; `prepare` runs before the ledger |
| `journal::prepare`, `journal::ensure_day` | `journal/YYYY` | inside; `prepare` runs before the ledger |
| `Ledger::append` | `ledger` | inside, before the month files (built with `Ledger::new`) |
| `cases::ensure_area` | `areas/<area>` | inside |
| `cases::set_active_case` | `.seldon` | inside |
| `decide accept` | `decisions` | before the ledger note |
| `plan new` | `work/<status>`, `areas/<area>`, `.seldon` | before the ledger |
| `dossier` (`seldon dossier`, import) | `system` | before `Files::read` / before the ledger |
| `index::views` (`ledger/*.md`) | `ledger` | inside `write_if_changed` |
| `rebuild` | `outputs` | before the write |
| `import --apply` | every folder of the plan, `.seldon/imports`, `outputs` | before the ledger |
| `import` task marker | `.seldon/imports` | inside `write_marker` |
| `rules` archive, `hook install skills --replace` archive | `archive`, `archive/skill-…/<label>` | inside |
| `setup` kit copy | `.claude/…` | each target folder |

Writes outside the logbook (state, config, Claude Code settings, agent
skill folders) are not this WP's: the state folder `proposals/` already
has its check (WP-124). Files in the root (`AGENTS.md`, `STATUS.md`,
`DECISIONS.md`) have no folder to check. `layout::create` writes into an
empty or new root, where nothing can be a link.

`anyhow`-typed functions on these paths (dossier `Files::write`, the
views) become `crate::error::Result` where needed so the refusal stays
exit 1 (a user error turned into `anyhow` would come back as exit 2).

## Tests

- Unit (`logbook/mod.rs`): a real folder, a missing folder, a missing
  part below a real one, a link (to a dir, dangling), a file, a FIFO, a
  link in the middle (`work` linked, `work/active` asked), a file in the
  middle, `..` and absolute refused, the root behind a link accepted,
  `checked_file` for a root file and a nested one, `inbox` (for WP-166).
- Integration (`engine/tests/linked_folders.rs`), one test per folder,
  each with a link (to a folder outside the logbook) and with a file in
  its place: exit 1, the reason names the folder, nothing written through
  the link, the ledger unchanged where the command writes one:
  `decisions`, `work`, `work/queued`, `work/active`, `work/completed`,
  `journal`, `journal/<year>`, `ledger`, `areas`, `areas/<area>`,
  `system`, `outputs`, `archive`, `.seldon`, `memory` (import),
  `.claude` (setup).
- Mutants by hand on the helper (`work/active/WP-168/mutants.py`, as
  WP-166): every arm of the match, the NotFound early return, the
  component filter, `checked_file`'s parent.

## Docs

SPEC-ENGINE §2 (file modes paragraph): the folder rule and the exit code;
SPEC-LOGBOOK §2 a line that the logbook's folders are real directories.
TESTING.md if it lists the test files.

## Check

`XDG_RUNTIME_DIR=<private> SELDON_FULL_CHECK=1 JUST_TEMPDIR=<scratch>
flock /tmp/seldon-check.lock just check`, CARGO_TARGET_DIR on disk.
