# WP-166 — Handover

Branch `wp/166-crash-inbox` from `next` (aa9c5902); merges into `next`.
E28 step 1: an agent's crash analysis goes into the logbook through the
engine.

## What was done

1. **`seldon inbox add --title T --file F|- [--tag T]… [--actor A]
   [--json]`** (`engine/src/commands/inbox.rs`, a new subcommand group
   `inbox` so a later `inbox list` fits).
   - Input: `--file -` reads stdin (a terminal is refused), else a regular
     file through `sys::read_small_file` (no symbolic link, FIFO, device
     or directory, checked before it is opened); at most 1 MiB; UTF-8.
     Any path (a report usually sits in a `mktemp` file); the path is
     never recorded.
   - Redaction as `import task` treats a task file (WP-102b, WP-140):
     CRLF as LF, `import::is_direction_or_format` characters dropped and
     counted, then `import::Scrubber::text` — the whole text through §7
     keeping its lines (a PEM key, a continued `mysql -p`, a JSON value on
     the next line are masked whole), then `/home/<user>` → `~`. Title:
     format characters dropped, `one_line`, scrubbed the same way, at most
     120 characters. Tags: `log`'s parser (now `pub(crate)`), redacted.
   - The file `inbox/<YYYY-MM-DD>-<slug>.md` (`cases::slug`, `note`
     without letters): frontmatter `type: inbox`, `created`, `actor`,
     `tags`, then `# <title>`, a blank line, the text. Created with
     `O_EXCL` (mode 0600; never overwrites, never writes through a link
     at the file's name); `inbox/` made when missing. *Corrected in round
     2:* round 1 did write through an `inbox/` that is itself a link to a
     folder outside the logbook (review N1); round 2 refuses it.
   - **Idempotent title clash:** an `inbox/*.md` whose body after its
     frontmatter equals this one is "already filed" — nothing written,
     exit 0, `filed: false`, its path named (also on another day, by
     another actor, with other tags). Another text under a taken name gets
     `-2` … `-99`, then exit 1.
   - Record: one commit of the new file alone (`autocommit_paths`,
     `seldon: inbox add`; the user's pending edits stay out), then an
     index rebuild (the index carries the head and the last autocommit).
     Under the state lock. Exit codes 0/1/3/4 as usual.
   - `--json`: `{filed, path, title, actor, tags, redactedLines,
     privatePaths, droppedCharacters, git}`.
2. **The skill** (`engine/assets/skills/seldon/SKILL.md`): a section
   *After a Crash Analysis* — after Omarchy's `diagnose-crash` and with a
   logbook, file the report with `seldon inbox add --title "<title>" --tag
   crash --actor agent:<name> --file - <<'SELDON_REPORT'` (quoted heredoc,
   nothing in it runs), then ask the user in one line whether it becomes a
   case; unattended: file it, ask nothing; never the core, memory contents
   or the environment; a "yes" opens a case naming the inbox file. The
   description gains the trigger ("after diagnosing a crash"), the
   Decision Framework a ninth line. Omarchy's notification and skill are
   untouched. A capture upgrades unedited installed copies (WP-111).
3. **Docs:** SPEC-ENGINE §3 (the command) and §7 (one more free-text
   writer); SPEC-LOGBOOK (the inbox line); guide en/de 04 (a skill bullet
   and *Crash analyses go into the inbox*), 05 (help blocks, regenerated),
   07 (the inbox row), German source lines moved to the English commits;
   AGENT-GUIDE §5 and `llms.txt` (a row each); CHANGELOG; TESTING.md row.

No schema, fixture, contract or plugin change.

## Decisions (what the WP left open)

1. **No ledger event.** The git commit is the record; a `crash` event kind
   is E28 step 2 with an ADR, and a `manual/note` now would put every
   filing on the plugin's timeline before that ADR decides how.
2. **"Idempotent" is by content, not by name.** The same title and text
   are found under any name in `inbox/` (a retry the next day files
   nothing); the same title with another text is a second crash and gets
   its own file. Actor, tags and date are left out of the comparison.
3. **Any path for `--file`** (unlike `import task`'s home-only rule): the
   text is redacted before it is written, and `diagnose-crash` works in
   `mktemp` files. Links are refused, so `--file <(…)` is too; the skill
   and the error message name `--file -`.
4. **The skill only, not the logbook's `AGENTS.md` rules block.** The WP
   says "the Seldon skill (rules text)"; the rules block is versioned
   (ADR-0027/0028) and a change there would need a rules version bump.
   Say if the block should carry the paragraph too.
5. **Format characters are dropped** (as `import task`), not kept:
   invisible text cannot reach a later agent reading the inbox.

## Not done / open

- WP-159 (secrets split by a zero-width character) introduces one shared
  helper and removes `import::is_direction_or_format`, which `inbox.rs`
  imports: **WP-166 merges after WP-159**, with `drop_format` switched to
  `redact::without_invisible` (round 3b). The helper does not take control
  characters; `inbox add` drops those itself since round 3.
- No live run on the test host: the command and the skill's recipe are
  covered end to end in temp homes (the recipe is run through bash
  verbatim from `SKILL.md`).
- `docs-check` keeps one warning that is not this WP's:
  `de/01-getting-started.md` is behind `en/` since fd7b741d (front pages
  on main).

## How it was verified

- **`flock /tmp/seldon-check.lock just check` on a45f8ad2: exit 0**
  (`check: ok`; log `engine/target/check-wp166-r2.log`), with a private
  `XDG_RUNTIME_DIR` (removed afterwards) and `SELDON_FULL_CHECK=1`: 96
  test binaries, 2424 tests passed, 0 failed; bar-view 196/0,
  ipc-restart 44/0, qmllint, plugin-validate, docs-check and plugin-test
  ok; `/run/user/<uid>` 2 % before and after. The commit after it adds
  only this handover.
- `engine/tests/inbox.rs`: 8 tests (stdin, file, redaction, idempotent
  title clash, limits, refusals, the skill's recipe run through bash);
  `engine/tests/skills.rs` checks the new skill lines name commands and
  options the engine has.
- `check-perf` not run: the WP touches neither the index build, `status`
  nor the hooks (only one more caller of the index rebuild).

- **Mutants:** `work/active/WP-166/mutants.py` (own target
  `engine/target/mutants-wp166`, run from a `git archive` copy of
  a45f8ad2 so the worktree stayed untouched): **34/35 killed** — format
  characters (text, title, count), scrubbing (text, title), blank lines,
  empty text, stdin size (none, off by one), stdin UTF-8, a link read
  through, one line, title length (none, off by one), the fallback slug,
  the counts (hits vs lines, title and text lines merged, private
  paths), tags, the frontmatter, the date, the numbering (first name,
  98, 100), a taken name (error, overwritten), `inbox/` creation, the
  already-filed scan (never, frontmatter compared, first name only),
  the commit (when nothing was filed, the user's edits), the index
  rebuild, the lock. The survivor, `CRLF kept` (no `\r\n` → `\n`
  before the scrubber), is equivalent: the rules read `\r\n` as `\n`
  since WP-128, the scrubber keeps line ends, and `trim_blank_lines`
  splits with `str::lines`, which drops a `\r` before each `\n`. The
  normalisation stays as `import task` has it. The first run lacked
  boundary tests, so tests for the limits, tag redaction, the index
  rebuild and the title/text line count were added before the run
  (a45f8ad2).

- `git diff aa9c5902..HEAD`: no home path but the `/home/alice` and
  `/home/<user>` placeholders, no user or host name.

## Round 2

Brief: the orchestrator's round-2 brief from the Opus stage-1 review
(APPROVE with notes N1–N5). `next` had not moved (aa9c5902), so nothing
was merged.

### Fixed

- **N1 — a linked `inbox/`.** `checked_inbox` (under the lock, before
  the already-filed scan and the write): `symlink_metadata` of
  `inbox`; a directory or missing is fine, a symbolic link or anything
  else is exit 1, "… is a symbolic link / is no directory, not the
  logbook's inbox folder", nothing written (as `triage::checked_dir`).
  Also covers N5: an `inbox` that is a regular file is now exit 1, not 2.
  The round-1 claim "never writes through a link" is corrected above (it
  held for the file's name only). `seldon decide` has the same hole for a
  linked `decisions/` (review N1); not touched here.
- **N3 — the three missing tests** (`engine/tests/inbox.rs`, 13 tests
  now): a title-only drop count (`droppedCharacters` 4 with a plain text);
  a link at an inbox name to a file holding the very filing is not
  "already filed"; the terminal refusal with a real pseudo-terminal
  (util-linux `script -qec … /dev/null`; skipped without `script`, ran
  here).
- **N4 — procfs views are refused** (my call): a file `stat` reports as
  size 0 that returns data (`/proc/self/status`, `/proc/self/environ`) is
  exit 1, "a file of size 0 that holds data (a /proc view); copy what the
  report needs into a file". *Corrected in round 3:* sysfs attributes
  report size 4096, so they are not caught; the check is a guard against
  a mistake, not against an adversary (an agent can pipe anything). Live process
  state is never a report, and the environment is exactly what the skill
  forbids filing. An empty regular file stays "the text must not be
  empty".
- **Title and human output:** after `one_line` (which still refuses line
  breaks), every control character (C0, DEL, C1 — `char::is_control`) is
  dropped from the title and counted in `droppedCharacters`; a title of
  controls only is "must not be empty". The title is the only free text
  in the human line, so no ESC reaches the terminal there.
- **N2** stays with WP-159: the text keeps its C0/C1 characters.
  *Corrected in round 3:* the premise was wrong — WP-159's helper takes
  no control characters; round 3 drops them here. The switch point for
  the format characters stays one function, `drop_format`.
- Docs: SPEC-ENGINE §3 (the link refusal, the size-0 views, the title's
  controls, the text's C0/C1 left to WP-159); TESTING.md row.
- No change to the logbook's `AGENTS.md` rules block (orchestrator).

### How it was verified

- **`flock /tmp/seldon-check.lock just check` on 315bf38c: exit 0**
  (`check: ok`; log `engine/target/check-wp166-r3.log`), private
  `XDG_RUNTIME_DIR` (removed afterwards), `SELDON_FULL_CHECK=1`: 96 test
  binaries, 2436 tests passed, 0 failed; desk-view 1780/0, bar-view
  196/0, ipc-restart 44/0; the real-home guards ok (desk-view notes the
  operator's live engine changed the real state dir during the run, "not
  a leak"); `/run/user/<uid>` 2 % before and after; docs-check only the
  known `de/01` warning from main. The commit after it adds only this
  section.
- **Mutants:** `mutants.py` with ten round-2 mutants (the inbox check
  removed, a link taken as the inbox, the filed scan through links,
  title controls kept or not counted, the title's drops not counted, a
  title of controls filed, a proc view filed, an empty file taken as a
  view, the terminal check off), run from a `git archive` copy of
  315bf38c with its own target: **44/45 killed**, every round-2 mutant
  among them; the survivor is round 1's equivalent `CRLF kept`.
