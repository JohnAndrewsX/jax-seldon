# ADR-0049 — A file of the logbook is never written through a symbolic link

**Status:** proposed
**Date:** 2026-10-09

> One rule for the folders (WP-168) and the files of the logbook, before
> 0.2.0. Replaces the sentence of SPEC-ENGINE §2 "A symbolic link at the
> path is followed: the link stays and its target is replaced" for files
> of the logbook; outside the logbook it stays. No contract change.
> Work package: WP-171 (from the WP-168 review, N1 and N3; Fable stage 2,
> F1 and F4).

## Context

WP-168 made every folder the engine writes in a real directory inside the
logbook: a folder that is a symbolic link or a file is refused with exit
1, and nothing is written. Files stayed outside that rule. The write
primitive `sys::write_atomic` resolves a link at the path on purpose
(`MAX_LINKS`) and replaces the link's target, as SPEC-ENGINE §2 says; the
ledger's append opens the month file and follows a link there.

So a link at a file escapes the logbook, and worse than a linked folder:
the WP-168 review replaced `journal/2026/2026-10-09.md` with a link to a
file outside, and `seldon log` rewrote that file (frontmatter, its old
text, the entry), exit 0. The same holds for `STATUS.md`, `DECISIONS.md`,
`AGENTS.md`, the case files, `ledger/*.jsonl`, the ledger views,
`outputs/REBUILD.md`, `system/*.md` and the files under `.seldon/`. A
dangling link created its target, wherever it pointed. The engine cannot
tell a link the user made from one another process planted.

Following links is still right outside the logbook: `config.toml` and
Claude Code's `settings.json` are linked by dotfile managers, and the user
names those paths.

## Decision

### 1. One rule for folders and files

A file of the logbook the engine writes — replaced (`write_atomic`) or
appended (the ledger) — must be a regular file inside the logbook, or not
exist yet. When it is a symbolic link, the command stops with exit 1:

> `<file>` is a symbolic link, not a file of the logbook; make it a file
> and run the command again

and anything else that is no regular file (a directory, FIFO, socket or
device in a file's place) with "is no regular file". Nothing is written:
not the file, not through the link, not the ledger. The file is named
relative to the logbook root, as WP-168 names folders. The folder check
runs first, so a linked folder is named before a file in it.

Refuse, not replace: replacing the link with a real file (the rename
already would, without the link resolution) is silent — the user's link
is gone without a word. A refusal names the file and the fix, and the
`doctor` row (§3) names every one at once.

### 2. Where the check runs

The same places as WP-168's folder check: before a command's first write
and before the ledger (a command that writes the ledger and then a file
checks the file first), and again in the write primitive. The logbook's
primitives never resolve a link: `sys::write_atomic_nofollow` and
`sys::write_generated_nofollow` refuse a link or non-regular file at the
path, and should one appear after that check, the rename replaces the
link itself, not its target. The ledger's append checks the month file
before it opens it, and opens it with `O_NOFOLLOW` (§4). `write_atomic`
and `write_generated` keep following
links and are used only for files outside the logbook (config, state,
Claude Code settings, agent skills).

A generated view — `STATUS.md`, the fence of `DECISIONS.md`, a month
view `ledger/<month>.md` — is skipped instead of refused: `status` and
`index` name it in a warning ("`<file>` not updated: `<file>` is a
symbolic link, …"), exit 0, and still write `index.json` (CONTRACT rule
2: the plugin's file stays fresh). The view is rebuilt from the ledger
once it is a real file again; nothing of the user's is lost by skipping
it. A linked `ledger/` folder stays refused (WP-168): the ledger itself
is written there. `decide`'s fill of `DECISIONS.md` was a warning
already.

A capture's silent upgrade of the rules block in `AGENTS.md` turns the
refusal into a warning and goes on, as for every other reason that
upgrade cannot write. The setup kit's copy refuses a link where it
would put a file (before: an existing link counted as "kept", a dangling
one made the copy create its target).

Writers that create a file with `O_CREAT|O_EXCL` (a new decision, case or
area README through `write_new`, the archives, `init`) never follow a
final link already; their path is checked too, so a link gives the reason
rather than "exists".

Reading through a linked file is unchanged: the index still shows a
linked journal day. Only writes are refused.

### 3. `doctor` names them: the `layout` row

`seldon doctor` gets a `layout` row (read-only, no lock), found without
following a link:

- every part of a folder the engine writes in (SPEC-ENGINE §2), and the
  year folders `journal/<YYYY>/` and area folders `areas/<area>/`, that
  is a link or no directory;
- every file the engine writes (one list, derived from the writers and
  tied to their tests) that is a link or no regular file, a directory in
  its place included;
- in `decisions/`, the three status folders, `ledger/`, `system/` and
  `memory/`, also any other entry that is a link or neither a folder nor
  a regular file. In `areas/<area>/`, `outputs/`, `journal/<YYYY>/` and
  `.seldon/` only the engine's own files are looked at: a case template
  kept in a dotfiles repository, a note in an area or a report of the
  user's in `outputs/` is not named.

`error` when one of them is refused by a command (exit 1), naming those
(the first five, then a count) with the fix to replace each by a real
folder or file. What no command refuses — a skipped view (§2), another
link beside Seldon's files — is `degraded` on its own, and named after
the refused ones when both occur. `ok` when there is nothing to name.
This makes WP-168's refusal discoverable before a write fails (WP-168
review N3).

### 4. Accepted residual (WP-168 and WP-171)

The check is `lstat`-then-create without `O_NOFOLLOW`; a process with
write access to the logbook can swap a folder for a link between the two.
Accepted: such a process can edit the logbook directly. Files are closed
against that race: a replaced file's rename never follows a link, and
the ledger's append opens the month file with `O_NOFOLLOW` (std's
`OpenOptionsExt::custom_flags`, the constant written out as the crate
does for `SIGKILL`) and takes only a regular file, checked on the open
file. An `openat` walk for every folder part would close the folder race
too; it is left out for proportion, not because it cannot be done.

## Consequences

- A logbook with a linked file stops the commands that write it, exit 1,
  until the user replaces the link with the file (a generated view is
  skipped with a warning instead, §2). CHANGELOG, Unreleased ›
  Breaking, beside WP-168's entry.
- The logbook folder itself may still be a link (WP-168): a logbook kept
  on another disk goes there whole, or behind a bind mount.
- SPEC-ENGINE §2: the "followed" sentence is limited to files outside the
  logbook, and "Linked folders" becomes "Linked folders and files"; §3
  lists the `layout` row. SPEC-LOGBOOK §2 says files too.
- The doctor JSON shape is not part of `schema/`: `contractVersion`
  stays 2, no fixture changes.

## What would change this

Evidence of a real user layout that needs a linked file inside the
logbook and cannot be served by linking the whole logbook → an explicit
allow-list in `logbook.toml`, never "follow silently".
