# Import from omarchy-agent

This page is for you if you kept a logbook with the omarchy-agent kit
before Seldon: an Obsidian vault with `pipeline/`, `journal/` and
`knowledge/`. Seldon can bring its cases, journal, knowledge and
deviations into your Seldon logbook. If you never used the kit, skip
to the last section, [Task files](#task-files): it is for everyone who
keeps tasks in Markdown files.

## What happens to your vault

Nothing. The import only reads the vault. It writes into your Seldon
logbook, and only after you looked at a report and said `--apply`.

## Step 1: Dry run

Create your Seldon logbook first ([Getting started](01-getting-started.md)).
Then point the import at the vault:

```sh
seldon import omarchy-agent ~/path/to/vault
```

The dry run is the default. It writes one file,
`outputs/IMPORT-omarchy-agent.md`, and commits it as
`seldon: import omarchy-agent (dry run)`. Nothing else in the logbook
changes.

## Step 2: Read the report

Open `~/Seldon/outputs/IMPORT-omarchy-agent.md` in your editor. It
lists:

- every case with its old and new id and status;
- ids that were taken in your logbook, and the new id each case gets;
- how many links and ids the import rewrites, per file;
- assumptions it made (for example a closed case without a close date);
- the journal days, memory files and deviation rows it will write;
- every file it does not import, with the reason;
- errors, which stop the import;
- lines where redaction removed something (file and line, never the
  text).

Fix errors in the vault, or move the file in question out of the vault,
and run the dry run again until the report says there are none.

## Step 3: Apply

```sh
seldon import omarchy-agent ~/path/to/vault --apply
```

The import writes everything in one go and makes one commit,
`seldon: import omarchy-agent`. It refuses (exit 1) while the report has
errors.

Before its first write, the import commits the changes the logbook
already has (your edits, ledger lines from hooks) as
`seldon: before import omarchy-agent`. If they cannot be committed (with
`--no-commit`, with `autocommit = false`, or when the commit fails), the
import refuses to start (exit 1) and writes nothing. Commit them
yourself, then run `--apply` again.

Running it again is safe. A second `--apply` says "Nothing changed" and
writes nothing. The marker `.seldon/imports/omarchy-agent.json` records
that the import is done.

## Where things go

| In the vault | In your logbook |
|---|---|
| cases in `pipeline/cases/` and `archive/cases/` | case files in `work/`, tagged `omarchy-agent` |
| status `new`, `planned`, `in-progress`, `verification` | `queued` (never active; the Log says what it was) |
| status `done` | `completed` |
| status `dropped` | `dropped` |
| section *Auftrag* | *Intent* |
| section *Plan* | *Plan* |
| section *Ergebnis* | *Result* |
| every other section | under `## History`, one heading level deeper |
| `journal/YYYY-MM.md` | the matching day files, under `## Imported from omarchy-agent` |
| `knowledge/<topic>/*.md` | one section each in `memory/<topic>.md` |
| knowledge lessons | sections in `memory/lessons.md` |
| open entries in `system/deviations.md` with a path | rows in the dossier's deviations table |
| inbox, Dashboard, templates, `STRUCTURE.md`, the rest of `system/`, `.obsidian/` | not imported; listed in the report |

Each imported case also gets one note in the ledger, dated on the case's
creation day. Zone, risk, priority and dates come along unchanged.

## Ids that are already taken

A case keeps its id when your logbook does not have it yet. If the id is
taken, the case gets the next free id of that year. It is tagged
`omarchy-agent/<old id>`, and a line under its title names the old id.
Every reference to the old id in the imported text (`[[C-2026-001]]` and
bare `C-2026-001`) becomes the new id.

## Privacy

Every imported line passes Seldon's redaction rules, your own
patterns included. Paths that start with your home folder become `~`.
The report names the lines it redacted by file and line number, never
their content.

## If something goes wrong

If `--apply` fails halfway, the engine says that nothing was committed
and prints the command that undoes the partial write. It names only the
files the import wrote. It looks like this:

```sh
cd ~/Seldon
git --literal-pathspecs checkout <commit> -- memory/lessons.md && rm -f -- work/queued/C-2026-002-zweiter-editor.md …
```

`git --literal-pathspecs checkout <commit> --` takes the files the import
changed back to the commit made just before it, and reads the file names
as they are (no wildcards). `rm -f` removes the files it created. Other
files in the logbook stay as they are. Run the command soon: a ledger
line that a hook adds to one of the import's ledger files after the
failure is taken back with it. Then fix the cause and run `--apply`
again.

Until you undo it, every new `--apply` refuses (exit 1) and prints the
same command (it is kept in `.seldon/imports/omarchy-agent.undo.json`).
If that file was changed so that it names files outside the import's
folders, or no commit, Seldon prints no command and points you to
`git status` instead.

## Task files

You may keep tasks in Markdown files in your projects — a `TODO.md`
with checkboxes, or a file that describes one job and that you handed to
an agent. One command turns them into Seldon cases, so the work shows up
on your desk and in the logbook:

```sh
seldon import task ~/projects/desk/TODO.md --dry-run
seldon import task ~/projects/desk/TODO.md
```

The dry run lists what it would create and writes nothing. Without
`--dry-run` the import applies at once and commits as
`seldon: import task`.

In the desk, **Import tasks…** in the Work list does the same: type the
path (and, if you like, an area), look at the dry run's list, then click
**Import N cases**. Imported cases show "imported" in the list. Their
detail shows the whole Intent as plain text, with the file it came from
and its number of lines; **Start** becomes clickable only once that text
is shown, and Enter never starts an imported case. If a case's Intent
holds invisible characters (someone edited the file by hand), the desk
marks each as `‹U+…›` and keeps Start off; so it does for an Intent
longer than it shows. Then read the case in the editor and start it from
the terminal (`seldon plan start <id>`).

- Every open item (`- [ ] …`) becomes one **queued** case. Its title is
  the item's first sentence (at most 72 characters); its *Intent* is the
  item with the lines indented below it (nested items too) and the
  heading it stands under.
- Done items (`- [x] …`) are skipped. With `--include-done` they become
  completed cases.
- A file without any checkbox is one case: the title is its first `# `
  heading, else the file name; the *Intent* is the rest of the file.
- New cases are yellow, R1, priority normal. `--zone`, `--risk` and
  `--area` set other values for all of them.
- Each case gets the tag `imported` and a Log line `imported from
  ~/projects/desk/TODO.md#12` (the file and the item's line).
- A task whose text is longer than the desk can show (64 KiB) is skipped
  ("too long"): split it, or make the case by hand.
- Invisible characters (zero-width spaces, direction marks, the Unicode
  "tag" characters that can spell hidden words) are removed from the
  text before it is written; the report counts them.

Your task file is only read: never changed, moved or run. Its text goes
through the same redaction as a note, so a token in it does not reach
the logbook — also one on a continued line.

Imported cases stay queued. Each *Intent* starts with `Imported from
<file> — read before you start this case.` Read it before you start the
case: once you start a case, an agent may act on its *Intent* without
asking, and the text came from a file, not from you. Until you start
it, an agent treats that text like a web page it fetched. Seldon refuses
an agent's start of an imported case: only you start it.

Running the import again is safe. Seldon remembers each item in
`.seldon/imports/tasks.json` and skips what it already imported (the
report says `already-imported` and names the case). Ticking an item
later does not import it again. If you reword an item, the import makes
a new case, and its Log line names the earlier one (`changed since
C-2026-004`).

The import refuses (exit 1, nothing written): a file outside your home
directory, a file inside the logbook, a folder (name the files in it),
anything that is not a `.md` file, a file over 1 MiB or not in UTF-8,
and more than 200 new cases at once. `--json` prints the report as
`{mode, created, skipped, redactedLines, …}` for scripts and agents.

---

Previous: [Rebuild, dossier and update impact](08-rebuild-dossier-update-impact.md) · [Index](README.md) · Next: [Troubleshooting](10-troubleshooting.md)
