# WP-102 — Plan (stage 102a: engine)

Branch `wp/102-import` from `next` (a1198b2); merges into `next`. Stage
102b (the desk's **Import tasks…**) follows after review.

## Which form

The WP holds two forms: the original (ADR-0027 §7: one case per *file*,
`--start [--agent]`, `--recursive`, paths outside home imported with the
path redacted) and the **0.2.0 form** (operator, 2026-10-06: one case per
open `- [ ]` item, `--include-done`, a path outside `$HOME` or inside the
logbook refused). The 0.2.0 form is the newer word and the one ADR-0034
builds the desk on; it is normative here. Where it is silent, ADR-0027 §7
fills the gap (see Decisions).

## Command

```
seldon import task <FILE>… [--area <slug>] [--zone Z] [--risk R]
                           [--include-done] [--dry-run] [--json]
```

## Decisions (what the WP leaves open)

1. **Items, else the whole file.** A file with at least one checklist item
   (`- [ ]`, `* [ ]`, `+ [ ]`, `- [x]`/`[X]`, outside code fences and
   frontmatter) imports one case per *top-level* item: an item nested
   inside another item's block belongs to that block. A file with no
   checklist item imports as one case (ADR-0027 §7: title from the first
   `# ` heading, else the file name without `.md`; Intent the text without
   that heading). This keeps the prose task file "handed to an agent"
   working and adds the 0.2.0 checklist form.
2. **Item → case.** Title: [`agent::title_of`] of the item's text (first
   sentence, ≤ 72 characters, cut at a word with `…`) — the same rule as
   `agent start --new`. Intent: the item's text, then its indented lines
   dedented (blank lines inside kept), then `Section: <heading>` when a
   heading stands above it. Escaped with `cases::escape_lines` (a heading
   or fence line in the task file can never end the Intent section).
   Queued, yellow/R1/normal unless `--zone/--risk/--area`, tag `imported`,
   Log `created (zone Z, risk R): imported from ~/x.md#12`.
3. **Done items.** `- [x]` items are skipped (`reason: done`) unless
   `--include-done`, which creates them **completed** (Log `created …:
   imported from …` then `completed: imported as done`; ledger
   `case-created` + `case-completed`). An agent actor may not use
   `--include-done` (exit 1): a completed case made by an agent without a
   *Result* would bypass ADR-0027 §5's close guard.
4. **Paths.** Files only (a directory is exit 1: name the files). `~/` is
   the home; the path is resolved with symlinks; refused (exit 1, nothing
   written, every path checked before anything is written): not under the
   home, inside the logbook, not a regular file, no `.md` extension
   (case-insensitive), larger than 1 MiB, not UTF-8, a control character
   in the path. Shown and recorded as `~/…` (always, since outside the
   home is refused).
5. **Idempotency marker** `.seldon/imports/tasks.json` (ADR-0027 §7):
   `{version: 1, items: [{file, line, hash, case, importedAt}]}`. `hash` is
   SHA-256 of the item's *redacted* text without its checkbox state (no
   secret-derived bytes in the committed logbook; ticking an item later
   does not re-import it). Same file and hash → skipped (`reason:
   already-imported`, `case`). A new hash where the marker has an entry
   for the same file and line whose hash is gone from the file → a new
   case whose Log names the earlier one (`…, changed since C-…`). The same
   text twice in one file → the second is skipped (`duplicate`). The
   marker is written after every created case, so a failure half way
   never re-creates the cases already made. An unreadable marker is exit 1,
   nothing written.
6. **Limits.** At most 200 new cases per run (exit 1 above, nothing
   written): one click in the desk must not flood the logbook.
7. **Redaction.** Title, Intent and the path go through the import
   `Scrubber` (SPEC-ENGINE §7 with config patterns, `/home/<user>` → `~`);
   `--json` reports `redactedLines`.
8. **Dry run** takes no lock and writes nothing: no ledger, no case, no
   marker, no commit, no index. Same JSON shape, `id: null`.
9. **Not in 0.2.0 form, deferred:** `--start`, `--start --agent`,
   `--recursive`, directories (the desk starts a case with one click; the
   CLI has `plan start` / `agent start <id>`).
10. **No index field in 102a.** The case's source lives in its Log line
    and the marker, not in the frontmatter (`case.schema.json` validates
    the frontmatter with `additionalProperties: false`; a `source:` key
    would be a schema change). The desk's "imported from <file>" (102b)
    needs an optional index field — a **contract question** for the
    orchestrator (ADR-0035 §6 rule: a later accepted ADR on `next`).

## JSON

```
{mode: "apply"|"dry-run",
 created: [{id|null, title, status, source, path|null, replaces|null}],
 skipped: [{source, reason: done|already-imported|duplicate|empty, case|null}],
 redactedLines, files, marker|null, git}
```

## Steps

1. `engine/src/import/task.rs`: parse (items, sections, whole-file), unit tests.
2. `plan::Spec` gains `done` (completed in one go); `commands/import.rs`
   gains `ImportSource::Task`; marker; apply/dry-run; autocommit
   `seldon: import task`; index rebuild.
3. `engine/tests/import_task.rs`: heading/no heading, items, nested,
   sections, title rule, idempotency twice, changed item, duplicate,
   `--include-done` (and refused for an agent), dry run writes nothing,
   refusals (outside home, inside logbook, non-Markdown, directory,
   not UTF-8), redaction of a token in the body, the index stays valid.
4. Manual mutants (redaction off, idempotency off, logbook check off) —
   each must fail a test; recorded in the handover.
5. Docs: SPEC-ENGINE §3, SPEC-LOGBOOK §3, guide 09 en/de (section "Task
   files"), guide 05 en/de, CHANGELOG, TESTING.md row.
6. `flock /tmp/seldon-check.lock just check`; HANDOVER.md.
