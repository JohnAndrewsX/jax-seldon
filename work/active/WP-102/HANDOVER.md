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
