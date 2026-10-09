# WP-166 — Plan

Branch `wp/166-crash-inbox` from `next` (aa9c5902). E28 step 1: an agent's
crash analysis goes into the logbook's inbox through the engine.

## The command

`seldon inbox add --title T --file F|- [--tag T]… [--actor A] [--json]`
(new subcommand group `inbox`, room for `inbox list` later).

- **Input.** `--file -` reads stdin (a terminal is refused: "pipe the
  text"), else a regular file read with `sys::read_small_file` (no
  symbolic link, FIFO or device; at most 1 MiB; UTF-8). Any path: a
  report usually sits in a `mktemp` file; the path is never recorded.
- **Text, as `import task` treats a task file** (WP-102b, WP-140):
  CRLF as LF, `import::is_direction_or_format` characters dropped
  (counted), then `import::Scrubber::text`: the whole text through §7
  keeping its lines (multi-line rules, PEM keys), then `/home/<user>` →
  `~`. Blank after that → exit 1.
- **Title.** Format characters dropped, then one line (`one_line`), at
  most 120 characters, redacted and home paths rewritten the same way.
- **Tags.** `log`'s `--tag` parser (shared), redacted as `log` does.
- **Actor.** `--actor`, else `$SELDON_ACTOR`, else `human`.
- **The file.** `inbox/<YYYY-MM-DD>-<slug>.md` (local date of `now`,
  `cases::slug(title, "note")`):

  ```
  ---
  type: inbox
  created: <RFC 3339>
  actor: <actor>
  tags: [crash]
  ---
  # <title>

  <text>
  ```

  Created with `O_EXCL` (never overwrites, never follows a link), mode
  0600 as every new engine file; the `inbox/` folder is created when
  missing.
- **Idempotent title clash.** Before writing, every `inbox/*.md` whose
  body (after the frontmatter) equals this one (`# title`, blank line,
  text) is "already filed": nothing written, exit 0, `filed: false`, its
  path named. Otherwise a taken name gets `-2`, `-3`, … (up to `-99`, then
  exit 1). So a retry never files twice, and a second crash with the same
  title gets a file of its own.
- **Record.** One commit of the new file alone (`autocommit_paths`,
  `seldon: inbox add`): the user's other edits stay out of it. Index
  rebuilt (it carries the last autocommit and the head). No ledger event:
  the `crash` kind is E28 step 2 with an ADR; the git commit is the record.
- **Lock.** Written under the state lock (exit 4 when held); no logbook →
  exit 3.
- **`--json`.** `{filed, path, title, actor, tags, redactedLines,
  privatePaths, droppedCharacters, git}`.

No schema, fixture, contract or plugin change.

## The skill

`engine/assets/skills/seldon/SKILL.md`: a section "After a Crash
Analysis": after Omarchy's `diagnose-crash` and with a logbook, file the
report with `seldon inbox add … --tag crash --file -` (quoted heredoc),
then ask the user in one line whether it becomes a case (unattended: file
it, ask nothing). Never the core or memory contents. The description gains
the trigger. Omarchy's notification and skill untouched.

## Docs

SPEC-ENGINE §3 (command, `--json`), §7 (one more writer), SPEC-LOGBOOK
(inbox line), guide en/de (04 working with agents, 05 CLI reference help
block, 07 the logbook), CHANGELOG, TESTING.md row.

## Tests (`engine/tests/inbox.rs`)

stdin; file; redaction (token, PEM key over lines, home path, format
characters, title); idempotent title clash (same text → nothing, other
text → `-2`); the commit holds only the new file; refusals (terminal-less
empty stdin, blank text, multi-line title, symbolic link, missing file,
no logbook → 3); the skill's recipe run verbatim through bash (as the
report recipe test does).

## Gate

`just check` with a private runtime dir (prompt's command line), then hand
mutants on the new code (`work/active/WP-166/mutants.py`, run from a copy
of the tree, own target dir).
