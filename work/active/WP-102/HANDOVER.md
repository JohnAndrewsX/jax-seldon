# WP-102 — Handover, stage 102a (engine)

Branch `wp/102-import` from `next` (a1198b2); merges into `next`. Stage
102b (the desk's **Import tasks…**) is not started; it follows after
review. Plan and decisions: [PLAN.md](PLAN.md).

## What was done

- **`seldon import task <FILE>… [--area A] [--zone Z] [--risk R]
  [--include-done] [--dry-run] [--actor A] [--json]`** — a new variant of
  `ImportSource` (`engine/src/commands/import/task.rs`), the parser in
  `engine/src/import/task.rs` (pure, unit-tested).
  - A file with checklist items: one case per top-level open item; the
    item's indented block (nested items, a fence opened there) belongs to
    it; the nearest heading above is its section. Title by
    `agent::title_of` (first sentence, ≤ 72 chars, the `agent start
    --new` rule); Intent = item text + dedented block + `Section: …`.
  - A file without items: one case (ADR-0027 §7) — title from the first
    `# ` heading, else the file name; Intent the rest. An empty file is
    skipped (`empty`).
  - Queued, yellow/R1/normal unless flagged; tag `imported`; Log
    `created (zone Z, risk R): imported from ~/x.md#12`; ledger
    `case-created` with `meta.risk` through `plan::create` (contract 2).
  - `--include-done`: `[x]` items as completed cases (`case-created` +
    `case-completed` in one write, Log `completed: imported as done`).
    `plan::Spec` gained `done: Option<String>` for this; the three other
    callers pass `None`.
  - Idempotent: marker `.seldon/imports/tasks.json` `{version: 1, items:
    [{file, line, hash, case, importedAt}]}`; the hash is SHA-256 of the
    **redacted** text without the checkbox state (no secret-derived bytes,
    no task text in the marker; ticking an item does not re-import it). A
    reworded item at the same line → a new case whose Log says `changed
    since C-…`. Written after every case, so a failure half way never
    duplicates.
  - Untrusted input: every path is checked before the first write — `~/`
    expanded, symlinks resolved, regular `.md` file under the home (not
    the home), not inside the logbook, ≤ 1 MiB, UTF-8, no control
    character in the path; a directory is refused. Text is redacted line
    by line (SPEC-ENGINE §7 + config patterns, `/home/<user>` → `~`) with
    the import `Scrubber` before parsing; the Intent is escaped with
    `cases::escape_lines` (a task file's `## Log` or fence can never end
    or hide the case's sections). The file is only read; nothing from it
    becomes an argv or a command. At most 200 new cases per run.
  - `--dry-run`: no lock, no write. `--json`: `{mode, created: [{id,
    title, status, source, path, replaces}], skipped: [{source, reason,
    case}], redactedLines, areaCreated, files, marker, git}`. Autocommit
    `seldon: import task` and an index rebuild only when a case was made.
- **Docs:** SPEC-ENGINE §3 (full rule set), SPEC-LOGBOOK §3 (Log words,
  "Imported task files"), guide 09 en/de section *Task files* /
  *Aufgabendateien*, guide 05 en/de entry with the generated help block,
  CHANGELOG (Engine), TESTING.md row. CONTRACT.md rule 8 already named the
  `imported` tag; unchanged.

## Decisions (what the WP left open)

The WP has two forms. The **0.2.0 form** (operator, 2026-10-06) is the
newer word and the one ADR-0034 builds the desk on, so it is normative;
ADR-0027 §7 fills what it does not say. In short (PLAN.md has each):

1. Items, else the whole file (keeps ADR-0027 §7's prose task file).
2. Item → case as above; `Section:` line instead of a new field.
3. `--include-done` refused for an agent actor (it would make completed
   cases without the *Result* ADR-0027 §5 requires of an agent's close).
4. Files only; paths outside the home **refused** (0.2.0 form), not
   imported with a redacted path (original form).
5. Idempotency by the marker ADR-0027 §7 names, not by a frontmatter
   `source:` key (see the contract question).
6. 200-case cap, 1 MiB file cap.
7. **Deferred** (original form only, not in the 0.2.0 form): `--start`,
   `--start --agent`, `--recursive`, directories. The desk starts a case
   with one click; the CLI has `plan start` and `agent start <id>`.

## Contract question (for the orchestrator, before 102b)

The desk's "imported from <file>" (0.2.0 form) needs the case's source in
the index. Today it is only in the case's Log line and the marker; the
plugin may not parse either (AGENTS.md §3). `case.schema.json` validates
both the frontmatter and the index case with `additionalProperties:
false`, so a `source:` frontmatter key, as the 0.2.0 form words it, is a
schema change too. Proposal for an ADR under ADR-0035 §6 (optional field,
contract 2, before 0.2.0 is tagged): **optional `source`** on index case
objects — `"~/proj/TODO.md#12"` (string, `~/`-relative, redacted), present
only on imported cases — derived by the engine from the marker (or written
to the frontmatter as an optional key, if the ADR prefers the file to be
self-describing). 102a adds nothing to the contract.

A second, smaller note for 102b: the desk should pass the path as one
argument after `--` (`seldon import task --json [--dry-run] -- <path>`);
a path starting with `-` otherwise reads as an option. Tested
(`a_path_after_double_dash_may_start_with_a_dash`). CONTRACT.md's argv
line comes with 102b.

## How it was verified

- `engine/tests/import_task.rs`, 12 tests (heading / no heading, items,
  sections, nested, title rule, idempotency twice with same ledger and
  HEAD, ticked item, changed item, duplicate, empty, `--include-done` and
  its agent refusal, dry run leaves the whole temp tree byte-identical,
  ten refusals each after a good file with the tree unchanged — outside
  home, `..`, symlink out, inside the logbook, not `.md`, directory, not
  UTF-8, > 1 MiB, missing, control character — the 200 cap, redaction of
  a token / bearer header / `/home/alice` across the logbook and index,
  invalid marker, path after `--`). Also run with `TZ=UTC` and
  `TZ=America/New_York`: green.
- Unit tests in `import/task.rs` (5).
- **Mutants:** `python3 work/active/WP-102/mutants.py` applies twelve
  mutants to `commands/import/task.rs` one by one (redaction off,
  idempotency off, logbook refusal off, home refusal off, escaping off,
  done skip off, dry run applies, limit off, changed-item link off,
  duplicate check off, extension check off, size cap off) and restores
  the file: **all twelve killed**.
- `flock /tmp/seldon-check.lock just check` → see the last section.

## Not done / open

- 102b (desk) — after review, and after the contract question above.
- The live check on the test host with a ten-item file belongs to 102b's
  acceptance (desk + engine); not run in 102a.
- A Log line embeds the redacted path; a file name holding ` · ` would
  look like an extra Log field — the same as any `--reason` text today,
  not a new class; the v2 harm guard reads the risk from the ledger, not
  the Log, for these cases.
- A symlink swapped between the check and the read by the user's own
  processes is not defended against (the path is opened by its resolved
  name; the user owns the home).

## Check

`flock /tmp/seldon-check.lock just check` on e7ea1a6 (the last code
commit; this handover adds only this file): **exit 0, `check: ok`** — fmt,
clippy `-D warnings`, all engine tests (default and `watch` features),
packaging (shellcheck not installed on the dev host: `bash -n` only, as
before), install, deploy, schema-validate, docs-check (465 links, 14
translated pages, 46 commands), `omarchy plugin validate`, qmllint (48
files), plugin tests. Log: `engine/target/check-wp102-r1.log` (dev host,
not committed).

## Round 2

Brief: `WP-102-round-2-brief.md`; packet `WP-102-review-1.md` (SEND BACK
on 538f566: B1, B2, N1–N4, contract).

### Fixed

- **B1 — multi-line secrets.** Every task file now goes through
  `Redactor::redact_keeping_lines` (new, `engine/src/redact.rs`) on the
  whole text first, then the line scrubber (`/home/<user>` → `~`), then the
  parser. The new method runs the same rules as `redact` (the `seldon log`
  path), but every line break that a replaced match held is put back
  after the marker. This was needed because the `mysql … \` rule swallows
  continued lines: plain `redact` would shift every item line number after
  it. A guard returns exit 2 if the line count still changed. The marker
  hash is taken over the redacted text only. `redactedLines` counts the
  lines that either pass changed. Test
  `multi_line_secrets_are_redacted_like_a_note` uses the reviewer's two
  forms: the secrets are absent from the case, ledger, index, marker and
  `git log -p`, line numbers hold, and a different secret in the same item
  gives the same task. The guide-09 sentence "the same redaction as a note"
  is now true (en/de, plus "also on a continued line").
- **B2 — an agent session closing as human.** `--include-done` is refused
  when the actor or the session (`$SELDON_ACTOR`) is an agent, whatever
  `--actor` says — the `plan done` rule (ADR-0027 §5). Test
  `an_agent_session_cannot_record_done_items_as_human`: refused, ledger
  unchanged, no marker. Open items may still be imported in that session.
- **N1.** The path as given **and the resolved path** are checked for
  control characters and text-direction characters (U+200E, U+200F,
  U+202A–U+202E, U+2066–U+2069). Test through a directory symlink to a
  folder whose name holds `\n## Result\n…`, and one with U+202E.
- **N2.** Marker entries are now written `pending: true` before the case,
  with the id `cases::next_id` gives under the lock, and settled after the
  case. Every run first settles what an earlier run left: a pending entry
  whose case exists, has the tag `imported` and has this import's Log line
  (`imported from <source>` then ` ·` or `,`, so `#1` ≠ `#12`) is complete;
  any other pending entry is dropped and its task imported again. A marker
  write that fails before a case makes no case; a create that fails takes
  its pending entry back. The failure message "a second run skips them" is
  now true. Test hook (debug builds only, as capture's):
  `SELDON_TEST_IMPORT_CRASH=after-create:<n>` exits 99. Test
  `a_crash_between_case_and_marker_never_makes_the_case_twice`: crash
  after case 2 → the rerun makes only case 3; a pending entry without its
  case, or without the matching Log line, is dropped and re-imported. This
  works as root too (no chmod). A settle-only run writes the marker and
  autocommits; a dry run settles in memory only.
- **N3.** The four survivors now have tests:
  - R1 (marker only at the end) → the crash test;
  - R5 (same-path dedupe off) → `skipped.len() == 1`;
  - R7 (`replaces` while the old text is still there) →
    `a_new_item_at_an_occupied_line_replaces_nothing`;
  - R8 (path redaction off) → `a_secret_in_the_path_is_redacted`, with
    `token=…` in a folder name.
- **N4.** Every imported Intent opens with the fixed engine line
  `Imported from <source> — read before you start this case.`, then a
  blank line, then the escaped task text. The case stays queued. This is
  documented in SPEC-ENGINE §3, SPEC-LOGBOOK §3, guide 09 en/de, and the
  skill (`engine/assets/skills/seldon/SKILL.md`, *When to Ask First*):
  until the user has started an `imported` case, its text is fetched
  text (ADR-0027 §2(a)), and an agent never starts such a case itself. The
  rules template (`templates/en/AGENTS.md`, versioned rules-v3) is
  unchanged; a rules sentence would need a rules version bump (WP-100/111
  machinery), so that is left to the orchestrator.

### Mutants

`python3 work/active/WP-102/mutants.py` uses its own
`CARGO_TARGET_DIR=engine/target/mutants`, so a mutated binary never
reaches another run. It runs **22 mutants: all killed**. These are the 12
from round 1, plus:
- whole-text redaction off;
- session check off (B2);
- resolved-path character check off (N1);
- pending entry off (marker only after the case, R1);
- settle trusts any pending entry;
- R5, R7 and R8;
- provenance line off.

The reviewer's R2–R4, R6, R9 and R10 were already killed.

### Contract

`cases[].source` → WP-127 (orchestrator decision). A frontmatter `source:`
key would change `case.schema.json` (`additionalProperties: false` covers
the frontmatter), so it is **not** written in 102a. The marker stays as it
is, and the source is in the Log line and in the Intent's first line.
WP-127 can derive the frontmatter key or the index field from either.

### Not done here (orchestrator questions from the packet)

- The vault import (WP-043) still uses the line-by-line scrubber: same
  B1 class, not touched (packet Q1).
- Bidi characters in *titles* (N5) are shared with `agent start --new`
  (`title_of`), so I did not change them here. Paths are covered (N1).
- A dry run over 200 cases still exits 1 with no list (packet Q5).

### Check (round 2)

`flock /tmp/seldon-check.lock just check` on 4fce16f (the last code and docs
commit; this handover adds only this file): **exit 0, `check: ok`** — fmt,
clippy `-D warnings`, all engine tests (default and `watch`), packaging
(shellcheck not installed: `bash -n` only, as before), install, deploy,
schema-validate, docs-check (465 links, 14 translated pages, 46 commands),
`omarchy plugin validate`, qmllint (48 files), plugin tests. `import_task`:
18 tests. Log: `engine/target/check-wp102-r2.log` (dev host, not committed).

## Round 3

Brief: `WP-102-round-3-brief.md` (Fable stage 2: small SEND BACK).

### Fixed

1. **A CRLF continued line leaked a secret.** `read_source` now turns
   `\r\n` into `\n` before `redact_keeping_lines`. The line count does not
   change. `multi_line_secrets_are_redacted_like_a_note` now also imports
   both multi-line forms (`mysql … \` / `-p …`, and JSON `"password":` with
   the value on the next line) with CRLF endings. The secrets are absent
   from the case, ledger, index, marker and `git log -p`, and the line
   numbers hold. Mutant "CRLF normalisation off" is killed. The engine-wide
   rule fix (`\\\r?\n` in the db rule and in WP-097's option-rule
   continuation) is the orchestrator's separate WP and is not touched here.
2. **The engine refuses an agent's start of an imported case.**
   `plan::refuse_agent_start_of_imported`: a case tagged `imported`, with
   the actor **or** the session (`$SELDON_ACTOR`) an agent, whatever
   `--actor` says (the B2 pattern) → exit 1 "C-… is not started: an
   imported case is started by the user (ADR-0027 §2a); ask them to start
   it (agent:…'s session)". It is called in `plan.rs` `step()` for
   `Transition::Start` (under the lock, after the transition check) and in
   `agent.rs` `launch_on` for a queued case (before the plain "start it
   first" hint). `TAG_IMPORTED` now lives in `plan.rs`, and
   `import/task.rs` re-exports it. Test `an_agent_cannot_start_an_imported_case`:
   - `--actor agent:…`, `$SELDON_ACTOR=agent:…` and `--actor human` inside
     an agent session are all refused, with the ledger unchanged;
   - `agent start` gives the same answer and launches nothing (stub
     `omarchy` never runs);
   - the user's own start goes through, and an agent may start a case that
     is not imported.

   Three mutants are killed: the refusal in `step()`, its session part, and
   the refusal in `agent start`. One sentence each was added to SPEC-ENGINE
   §3 (`plan start` and `import task`). The skill now says "The engine
   refuses an agent's start of an imported case: ask the user to start
   it." Guide 09 en/de says it too.
3. **Optional item, done.** `bad_path_char` also refuses U+200B–U+200D,
   U+2060 and U+FEFF. The linked-folder test was extended to five names,
   and a mutant is killed.

A note for the orchestrator: `plan reopen` of a completed imported case
makes a new active case with the same Intent, tagged `reopens:<ID>` and
not `imported`. That case was started by the user once before (or
imported as done), so I left reopen as it is.

### Mutants

`python3 work/active/WP-102/mutants.py` takes a file per mutant and uses
its own target `engine/target/mutants`. **27 mutants: all killed.**

### For 102b (no code in 102a)

- **Start** on an `imported` card never fires from the card. It opens the
  detail first, which shows:
  - the **whole Intent as plain monospace text** (never rendered
    Markdown);
  - the provenance line;
  - `source` (WP-127);
  - the line count.
  Cards carry an "Imported" marker from the tag.
- **Fixed argv:** `seldon import task --json [--dry-run] -- <path>`, with
  the path as one argument (a path starting with `-` is tested). The
  dry-run list comes first, then one click imports.
- **Live check** on the test host with a ten-item file that holds one
  `## Result` line and one CRLF secret.
- **`cases[].source` (WP-127)** is an engine-written optional frontmatter
  key that the index copies:
  - `~/`-relative and redacted;
  - no control, bidi or format characters;
  - at most 512 characters;
  - display only, never an argv.
  The marker stays the only idempotency key.

### Check (round 3)

`flock /tmp/seldon-check.lock just check` on 1cf45a0 (the last code and
docs commit; this handover adds only this file): **exit 0, `check: ok`** —
fmt, clippy `-D warnings`, all engine tests (default and `watch`),
packaging, install, deploy, schema-validate, docs-check (465 links, 14
translated pages, 46 commands), `omarchy plugin validate`, qmllint (48
files), plugin tests. `import_task`: 19 tests. The branch diff holds no
private path (`/home/` only as `/home/alice` and `/home/<user>`). Log:
`engine/target/check-wp102-r3.log` (dev host, not committed).

## Merge of next

`git fetch`, then `git merge --no-ff origin/next` (5fb3911: WP-123 desk
sections, WP-127/128 queued) into `wp/102-import`, as the merge alone:
merge commit 5ddf80b.

- **Status:** no conflicts. The only file both sides changed is
  `docs/TESTING.md`. It auto-merged; my `import_task.rs` row is there once
  and no conflict markers are left.
- **What next brought:** plugin, `docs/SPEC-PLUGIN.md`,
  `docs/KEYBINDINGS.md`, plugin tests and work files. None of it touches
  the engine, the specs or the guides this WP changed.
- **Check:** `flock /tmp/seldon-check.lock just check` on 5ddf80b →
  **exit 0, `check: ok`**:
  - fmt, clippy, all engine tests (default and `watch`; 84 test binaries
    ok, none failed);
  - packaging, install, deploy, schema-validate;
  - docs-check (465 links, 14 translated pages, 46 commands);
  - `omarchy plugin validate`, qmllint (46 files after WP-123's removals),
    plugin tests.

  Log: `engine/target/check-wp102-merge1.log` (dev host, not committed).

102b is not started; it waits for WP-127 (`cases[].source`).

## 102b

**Merge:** `git fetch`, then a merge of the local `next` (aaf7a0a: main,
WP-124, WP-127, WP-136, WP-140) as its own commit, 7ce404d. Local `next`
was ahead of `origin/next` (8671446); the brief said `git merge next`, so
I aborted a started merge of `origin/next` and merged the local one. There
were no conflicts. Nothing I had built was disturbed, which I checked:
WP-127 already writes the frontmatter `source` from `import task`, and
WP-140 changed `redact.rs` beside `redact_keeping_lines`.

### What was built

The desk import follows the WP's 0.2.0 form and the "For 102b" rules of
round 3.

- **One engine addition: `plan show <id> --json` gives `intent: {text,
  lines, truncated}`.** The desk must show the *whole* Intent, but the
  index carries only its first paragraph (clipped to 256 bytes), and the
  plugin never reads Markdown. So:
  - the engine extracts the section and cleans it as the index cleans text
    (`index::build::plain_text`: control characters as spaces, direction and
    format characters dropped, redacted), without the clip;
  - the text is capped at 64 KiB, cut at a character boundary;
  - the line count is taken before the cut;
  - `intent` is `null` while the redaction patterns do not compile.

  Test `plan_show_gives_the_whole_intent_for_the_desk` (also: a long file
  gives `truncated`). Documented in SPEC-ENGINE §3 and in CONTRACT.md's
  argv list.
- **Import tasks…** is a button in the Work list head, beside *By agent*
  and *New case*. It opens `components/desk/ImportForm.qml` in the detail:
  - an explanation line, the path field and an optional area;
  - **Dry run** (Enter in a field) sends `seldon import task --json
    --dry-run [--area <a>] -- <path>` and shows the list: title, status ·
    source · changed since; the skips with their reason in words; and
    "Would create N cases · M tasks skipped";
  - **Import N cases** is enabled only for the path and area that the
    shown dry run was for. One click sends the same command without
    `--dry-run`;
  - the engine's refusal shows in the form. Esc closes the form and keeps
    the fields;
  - after an import the form closes and empties, the first new case is
    selected, and the list's result line says "Imported N cases: …".

  `Model.importPathError` refuses, before any call: an empty path, a
  control, bidi or format character, a relative path, a path over 4096
  characters, and anything that is not `.md`. `validateArgs` accepts
  exactly `import task --json [--dry-run] [--area <slug>] -- <path>` (with
  the path check) and `plan show <id> --json`. The path is always one
  argument after `--`; a path that looks like shell text stays one
  argument (unit test).
- **Imported cases:**
  - The list meta reads "id · imported · risk · …".
  - The detail asks `plan show` (read-only), again on every new index,
    and shows **IMPORTED TASK · N lines**. Under it is the accent line
    "From <source>. Read the whole Intent before you start the case: once
    started, an agent acts on it without asking. Only you start it." Then
    the engine's text in a bordered box, `Text.PlainText`,
    `Style.font.family` (Omarchy's monospace alias), `WrapAnywhere`, never
    Markdown. Its first line is the provenance line. A note follows when
    the text is truncated.
  - The key/values show "Imported from: <source>" (WP-127).
  - While the Intent is loading, withheld or unavailable (dev mode), the
    block says so and the index's first paragraph stays.
- **Start of an imported case:**
  - It never fires from the list or a key: such a case has **no Enter
    action**. Without this, Enter would have fallen to *Drop*; I found
    that and closed it.
  - The bar's Start is enabled only while `Model.intentReviewed` is true
    (this case's finished, successful `plan show`).
  - It arms by click only: "Start C-…? Click Confirm."
  - Until then the hint reads "Start waits until the whole Intent below is
    shown; only you start an imported case".
  - `press()` refuses as well, as a second guard.
- **Service:** `importTasks(path, area, dryRun)` (one at a time, through
  the queue) and `showCase(id, again)`. The new state is in `importResult`
  and `caseShown`; both are in the IPC `status` (without the Intent text).
- **Rules followed:** Style tokens only, `Text.PlainText` everywhere (a
  unit test reads both QML files), fixed argv, everything through the
  async queue, `qmllint` and `omarchy plugin validate` before each plugin
  commit, and no absolute home path.

### Tests

- **Model** (`model.test.js`, +7 tests, 165 pass): `importPathError`,
  `importArgs`, `validateArgs` (5 accepted and 13 refused forms),
  `importResult`, `caseShowArgs`/`caseShowResult`, the imported case's
  actions (no Enter, no key, Start armed by click), `intentReviewed`, and
  plain text only.
- **Harness** (`desk-view.sh` against `fake-seldon`, which now speaks
  `import task` and `plan show`):
  - **import-live:** the form; the dry run's list; one click imports; the
    selected case shows its whole Intent (5 lines, the provenance line, the
    source); no first-paragraph INTENT beside it; Enter twice does nothing;
    Start is armed by click and runs; a second dry run is all "already
    imported". The argv has the path as one argument after `--`, and
    `plan show` only for the imported case and before its Start (the
    number of `plan show` calls depends on index timing, so that part is
    checked separately).
  - **import-refused:** the form's own path check; the engine's refusal in
    the form; the fields kept after Esc; a withheld Intent leaves Start off;
    two clicks on the disabled Start run nothing.
  - **import-dev:** the index's first paragraph; "needs the engine"; no
    Start; the form stays shut; nothing overflows at 50 %.
- **Plugin mutants** (`work/active/WP-102/plugin-mutants.py` against the
  trimmed harness from `mk-desk-import.py`): four of five are killed —
  `intentReviewed` always true, the bar enabling Start without the review,
  Enter falling to Drop, and `validateArgs` skipping the path check.
  **"press() ignores the review" survives**: no path reaches `press("start")`
  except the disabled bar button (checked: two clicks do nothing) and
  Enter (which has no action), so that guard is defence in depth only.
- **Engine:** `import_task` now has 23 tests.

### Not done here / for the orchestrator

- **The live check on the test host** with a ten-item file that holds one
  `## Result` line and one CRLF secret is not run yet. It needs the
  installed build on the test host (packaging), after merge. The engine
  side of it is covered by the tests (escaping, CRLF, redaction); the desk
  side by the harness.
- **The desk has no key for Import tasks…**, so `docs/KEYBINDINGS.md` is
  unchanged.
- **`plan show` runs through the one engine queue**, as `drift show`
  does. On a busy queue the review waits its turn, and Start stays off
  until then.

### Check (102b)

`flock /tmp/seldon-check.lock just check` on 42ace2d (the last code and
docs commit; this handover adds only this file): **exit 0, `check: ok`**.
It covers:
- fmt, clippy `-D warnings`, and all engine tests (default and `watch`);
- packaging, install, deploy and schema-validate;
- docs-check (467 links, 14 translated pages, 53 commands);
- `omarchy plugin validate` and qmllint (48 files);
- the plugin tests: model.test.js 165, service-states 342/0, desk-view
  1609/0, bar-view 194/0.

The branch diff since the merge holds no private path. Log:
`engine/target/check-wp102b-r1.log` (dev host, not committed).
