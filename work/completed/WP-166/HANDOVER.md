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

## Round 3a

Brief: Fable stage 2, SEND BACK (small). Done on f53cf048:

- **Finding 2 — the text's controls.** In `add()`, after the CRLF step
  and the format characters, every control character but tab and newline
  is dropped (`is_text_control`: `c.is_control() && c != '\n' && c !=
  '\t'`, the predicate `hook` and `plan` use — *corrected in round 3c:*
  they turn such characters into a space or U+FFFD before the redaction,
  they do not drop them) and counted in `droppedCharacters`.
  So `to\x08ken=hunter2abc` is masked, an ESC colour sequence loses its
  ESC. Test `the_text_s_controls_are_dropped_but_tab_and_newline`. The
  claims that WP-159 takes C0/C1 are gone from the comment, SPEC §3 and
  this handover (marked *Corrected in round 3*).
- **Finding 3 — /proc only.** Message, comment, SPEC §3 and this handover
  say "a /proc view"; sysfs attributes report size 4096 and are not
  caught. A guard against a mistake, not an adversary.
- **Finding 5 (optional) taken:** the skill adds "nor a backtrace with
  variable values (`bt`, not `bt full`)".
- Verified: `--test inbox` 14/0, `--test skills` 27/0, fmt and clippy
  `-D warnings` clean; three round-3 mutants (controls kept, not counted,
  a tab dropped) killed from a `git archive` copy, `CRLF kept` still
  equivalent (a lone `\r` is a dropped control now too). The full check
  runs in round 3b, after WP-159 lands in `next` and `drop_format`
  switches to `redact::without_invisible` (finding 1).

## Round 3b

Brief: WP-159 and WP-168 landed in `next` (44febeae), then WP-138
(b4a11dba) and the WP-164 completion (59ea37d4). Merged `next` three
times (7e5a5010, daad4c88, 87a9e8d1); conflicts only in CHANGELOG.md and
docs/TESTING.md (both sides kept; next's redaction row, then the inbox
row).

### Done

- **Finding 1 — WP-159's helpers, redaction before stripping** (as
  WP-159's Fable packet, note 6). `drop_format` and
  `import::is_direction_or_format` are gone from `inbox.rs`:
  - title: `Scrubber::text_dropping_invisible` (redact, drop
    `redact::is_invisible`, home paths), then `one_line`, then every
    control character dropped;
  - text: CRLF as LF, `Scrubber::text_dropping_invisible`, then every
    control character but tab and newline dropped (round 3a), then the
    outer blank lines;
  - `droppedCharacters` counts the invisible and control characters of
    the title and the text as given (as `import task` counts).
  So `x<U+200B>sk-…` (the boundary an invisible character makes) is
  masked in title and text, and `/ho<U+200B>me/alice` is still a home
  path (test `redaction_reads_the_text_before_its_invisible_characters_go`).
  The redactor reads past lone control characters too (WP-159), so
  `to\x08ken=` is masked before the control is dropped. *Corrected in
  round 3c:* only past the controls that are no white space; a CR, VT,
  FF or NEL it reads as a space, and dropping it after the redaction
  glued the secret (review 3b, B1).
- **WP-168:** `checked_inbox` replaced by `logbook.checked_dir(INBOX)`;
  the message tests assert its wording ("inbox is a symbolic link / is no
  directory, not a folder of the logbook").
- SPEC-ENGINE §3 (the order: scrubber with the invisible set, then the
  controls; `Logbook::checked_dir`), CHANGELOG ("invisible and control
  characters"), TESTING.md (round 3b sentence).
- `next` at b4a11dba failed `fmt-check` in `engine/src/commands/preview.rs`;
  fixed here in its own commit (ab572658). `next` has the same fix since
  66257638, so the later merge took it without a conflict.
- `de/05-cli-reference.md`'s source line now points at daad4c88. WP-138
  had already translated `seldon preview`; only its source line stayed
  at 3721c641. docs-check now passes with no warning.
- WP-171 (the doctor `layout` row) is not on `next` yet. Whichever lands
  second adds `inbox/` to that row's folder list, with a test.
- Residual (accepted; *closed in round 3c*): a home path split by a
  control character (`/ho\x01me/alice`) kept its user name; the control drop follows the
  scrubber, whose home-path rewrite is private to it. Not a secret
  (§7 covers home paths as privacy only).

### How it was verified

- **`flock /tmp/seldon-check.lock just check` on ab572658: exit 0**
  (`check: ok`; log `engine/target/check-wp166-r7.log`). Run with a private
  `XDG_RUNTIME_DIR` (removed afterwards), `SELDON_FULL_CHECK=1`, and
  `JUST_TEMPDIR` and every target on disk (`engine/target/`):
  - 102 test binaries, 2626 tests passed, 0 failed;
  - desk-view 1808/0, bar-view 196/0, ipc-restart 44/0;
  - the real-home guards ok (service-states notes the operator's live
    engine, "not a leak");
  - `/run/user/<uid>` 2 % before and after.
  From ab572658 to the head only `AGENTS.md`, `STATUS.md`, work files and
  the `de/05` source line changed (the last merge of `next`); docs-check
  was re-run on the head: no warning.
- Two earlier runs, for the record:
  - r4 on 7e5a5010 failed one desk-view timing budget (`graph-big
    ticksOver <= 5`) while my mutant run loaded the host. r5 on the same
    commit, without that load: exit 0, desk-view 1785/0.
  - r6 on daad4c88 stopped at `fmt-check` (`preview.rs`, above).
- **Mutants:** `mutants.py` was rewritten for the new code: the stale
  patterns replaced, three round-3b mutants added (the text stripped
  before its redaction, the same for the title, the text's invisible
  characters not counted), and the WP-168 code dropped as not ours. The
  target dir can now be set (`MUTANTS_TARGET`), so it stays on disk.
  Run from a `git archive` copy of 7e5a5010 under `engine/target/`:
  **49/50 killed**; the survivor is the equivalent `CRLF kept`. The copy
  has been removed.

## Round 3c

Brief: review 3b, SEND BACK on B1. Round 3b dropped the control
characters *after* the redaction. The rules read a CR, VT, FF or NEL as
white space, so a secret those split (`to<VT>ken=`, `ghp_0123<NEL>…`, a
VT or NEL in the title) was read as two words and then glued back, and
filed in clear in the file, its name and the git history. The
orchestrator's rule (Q1): nothing is removed after the redaction that
the redactor's reading copy kept. The brief's order for that: drop the
controls, then `Scrubber::text_dropping_invisible`. Code ece63f11, head
d85d26e4 (`next` merged; only work files came in).

### Why the brief's order was not taken

A probe of drop-first (debug build, scratch home on disk under
`SELDON_TEST_GUARD`) showed the mirror hole. `done<CR>sk-ABCDEFGHIJKLMNOPQRSTUVWX`
becomes `donesk-ABCDEFGHIJKLMNOPQRSTUVWX` once the CR is gone. The key
is now glued to the word before it, no boundary-anchored rule matches,
and it was filed in clear. Kept, a control splits a secret; dropped, it
glues one. One reading cannot cover both.

### The order now (`engine/src/commands/inbox.rs:124-155`)

- **Text.** CRLF as LF, then:
  1. the scrubber over the text as given (`Scrubber::text`; a control is
     a boundary here, so `done<CR>sk-…` is found);
  2. every control character but tab and newline dropped;
  3. the scrubber again, with the invisible characters dropped after its
     redaction and home paths rewritten (`Scrubber::text_dropping_invisible`;
     it finds what the drop joined, `to<VT>ken=`).

  Then the outer blank lines.
- **Title.** The same two passes. The drop between them takes every
  control but the line ends, which `one_line` refuses afterwards. No ESC
  reaches the human line.
- **The rule holds literally.** After the last redaction only
  `is_invisible` characters are removed, and that pass's reading copy
  left exactly those out. Pass 2 never un-masks: the marker is neither
  invisible nor a control, and a match of markers alone is left as it is
  (WP-140).
- `droppedCharacters` counts the invisible and control characters of the
  title and the text as given.
- This also closes round 3b's accepted residual: `/ho\x01me/alice` is
  now rewritten to `~`.
- Docs: the module doc, SPEC-ENGINE §3 (the rule, the two passes, "the
  controls that are no white space"), TESTING.md, and the corrections
  marked in rounds 3a and 3b above (the `hook`/`plan` sentence, the
  "reads past lone controls" claim).

### The all-controls test

`no_control_character_glues_a_secret_after_the_redaction`
(`engine/tests/inbox.rs`) runs over all 65 control code points: C0, DEL
and C1, NEL included.

- **Text:** 63 of them (tab and newline aside), one line each with
  `to<c>ken=`, a `ghp_` token split by `<c>`, `/ho<c>me/alice` and
  `done<c>sk-…`. It asserts:
  - the exact filed line (`token=‹redacted› ‹redacted› ~/x done‹redacted›`);
  - no control left;
  - `Redactor::builtin().redact(body) == body`;
  - `redactedLines` 63 and `droppedCharacters` 4·63.
- **Title:** 64 of them (NUL cannot travel in an argv), each splitting
  `to<c>ken=` and a `ghp_` token and gluing `x<c>sk-…`. It asserts the
  `--json` title, the file name and the file content. `\n` and `\r`
  exit 1 with "must be one line".
- **Afterwards:** no secret in any inbox file name or in `git log -p`.
- It fails on round 3b's order (CR splitter in clear), on round 3a's
  (`done<CR>sk-` glued) and on controls-as-spaces (`to ken=`).

### How it was verified

- `cargo test --test inbox`: 16 passed, 0 failed. `cargo fmt --check`
  and `cargo clippy --all-targets -D warnings` are clean.
- **Mutants** (`mutants.py`): the round-3c ones are no first pass
  (text, title), the controls dropped after the scrubber (text, title),
  and the title keeping its white-space controls. Run from a
  `git archive` copy of d85d26e4 with the target on disk: **51/54
  killed**. The three survivors are equivalent:
  - `CRLF kept`, as in every round;
  - the two round-3b "stripped before its redaction" mutants. Pass 1 now
    reads the text as given, including its copy without the invisible
    characters, so `x<U+200B>sk-…` is masked before pass 2 sees it.
- The full check I had queued after the mutants was stopped by Claude
  Code for low memory before it started (empty log, nothing left
  behind). The orchestrator gated ece63f11 and d85d26e4 (gate logs
  w166c, w166c-head). Fable stage 2: APPROVE, code unchanged. This
  commit is docs only.

### Residual (accepted, WP-169)

Two lone controls in mixed roles in one secret (`done<CR>sk<CR>-…`) are
filed glued, as two invisible characters are everywhere (ADR-0048 §2's
limit). Pass 1 cannot match (the token is split), and pass 2 cannot
match (the boundary is gone); no finite number of extra passes closes
it. Mixed control and invisible (`done<CR>sk<ZW>-`, `done<ZW>sk<CR>-`)
is masked.

An ESC CSI colour sequence inside a key (`sk-ABCDEFGHIJ<ESC>[0mKLMN…`)
leaves the fragments interleaved with `[0m`. That is also WP-169, which
ADR-0048 already names.
