# ADR-0044 — The desk reads a case's whole Intent (`plan show --json` `intent`) and imports task files (`import task`): two plugin-command rows

**Status:** proposed (WP-102b round 2; accepting it is the operator's decision)
**Date:** 2026-10-08

> Adds two rows to CONTRACT.md's "Commands the plugin may run" and fixes the
> shape of one field of an existing command's `--json`. ADR-0035 §6 does not
> cover either: it lets a later ADR on `next` add optional fields to the v2
> schemas, not commands, and no `schema/*.json` describes a command's
> output. ADR-0040 (proposed, WP-135) asks for an ADR whenever a row is
> added to that list; this is that ADR for WP-102b. The index's shape does
> not change: `contractVersion` stays 2, no fixture changes. Builds on
> ADR-0027 §2(a) (instructions in fetched text are outside the Intent),
> §7 (the import), ADR-0034 §2 (*Import tasks…* in the Work section) and
> ADR-0038 §2–§3 (`cases[].intent`, `cases[].source`, the direction and
> format set).

## Context

`seldon import task` (WP-102a) turns a user's Markdown task file into
queued cases. The file is untrusted text: once the user starts such a
case, an agent acts on its *Intent* without asking (ADR-0027). The engine
therefore refuses an agent's start of an imported case, and the desk
(WP-102b) shows the whole Intent before its *Start* — the "For 102b"
rules of WP-102 round 3: the Intent as plain monospace text, the
provenance line, the source, the line count; Start never from the list or
a key.

The index carries only the first paragraph of an Intent, clipped to 256
bytes (ADR-0038 §2), and the plugin never reads Markdown (AGENTS.md §3).
So the desk needs the whole Intent from the engine. The stage-1 review of
WP-102b (round 1) found two ways around the review: an Intent longer than
the 64 KiB the desk shows (the part that matters below the cut), and
direction and format characters — the tags U+E0000–U+E007F can spell a
whole instruction — that the review dropped while the case file, its
title and `plan show` for an agent kept them.

## Decision

1. **Two rows in CONTRACT.md:**
   - `seldon plan show <caseId> --json` — read-only; the plugin reads its
     `intent` (2.) and nothing else from it;
   - `seldon import task --json [--dry-run] [--area <slug>] -- <path>` —
     the path one argument after `--`, never interpolated; the dry run
     first, then the import.

   Both run through the plugin's one engine queue with a fixed argument
   list (`Model.validateArgs`); the case id matches the schema's pattern,
   the area is a slug, the path passes the plugin's character check (5.)
   and ends in `.md`.
2. **`plan show --json` `intent`** = `{text, lines, truncated, hidden}`:
   - `text` is the whole `## Intent` section as display text: control
     characters other than line breaks and tabs as spaces; every
     direction or format character (ADR-0038's set as WP-140 amended it,
     the tags included) **marked** as `‹U+XXXX›`, not dropped; then the
     logbook's redaction;
   - `hidden` counts the marked characters;
   - `text` holds at most 64 KiB (65 536 bytes), cut at a character
     boundary; `truncated` says it was cut; `lines` counts the lines
     before the cut;
   - `intent` is `null` while `[redaction] patterns` do not compile
     (withheld, as the index withholds its texts).
3. **An imported Intent always fits what the desk shows (B1).**
   `import task` skips a task whose *Intent* as it would be written (the
   provenance line, then the escaped text) is longer than 64 KiB: reason
   `too-long`, nothing written for it.
4. **Nothing invisible reaches an imported case (B2).** `import task` drops
   direction and format characters from the task text — and so from the
   title — before it redacts and writes (`droppedCharacters` in its
   `--json`). A case made or edited by hand can still hold them; 2. marks
   them.
5. **The desk's Start of an imported case** is enabled only while the
   detail shows this case's finished, successful `plan show` with
   `truncated` false and `hidden` 0. Otherwise it stays off, and the bar
   says why and where to go: "Read the whole Intent in the editor; start
   this case from the terminal." Asked again on every new index, the last
   text stays on screen and Start is off until the answer. The path
   characters a task file may not hold are one set on both sides — the
   engine's `bad_path_char` (control, direction and format characters,
   U+2028, U+2029) and the plugin's `BAD_PATH_CHARS` — both tested
   against `fixtures/bad-path-chars.txt`.

**Open (N6).** `plan start` is not bound to the text the user was shown:
if the case file changes between the last `plan show` and the click, the
user starts text they did not see (the re-ask on every new index narrows
the window; an edit made without any engine command does not move the
index). A later `plan start --intent-sha <sha256 of intent.text>`,
refused when the Intent changed, would close it; not in 0.2.0.

## Consequences

- CONTRACT.md gains the two rows; SPEC-ENGINE §3 (`plan show`, `import
  task`) and SPEC-PLUGIN §5.4 (Work) describe 2.–5.
- A long task (over 64 KiB as one Intent) is never imported; the user
  splits it or makes the case by hand, where the desk sends them to the
  editor and the terminal.
- A hidden instruction in a task file is gone before the case exists; one
  added by hand later is visible in the desk's review and keeps Start off.
- Zero-width characters that split a secret are dropped before the
  import's redaction, so for `import task` the secret is masked as a whole;
  the same gap elsewhere (`seldon log` keeps them, by WP-140's rule for
  notes) is outside this ADR.
- If ADR-0040 is not accepted as written, this ADR still records the
  decisions 2.–5.; the two CONTRACT.md rows then need no ADR of their
  own.
