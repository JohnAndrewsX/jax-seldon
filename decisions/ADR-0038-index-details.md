# ADR-0038 — The index carries what the desk's details show: the drift rule, a case's intent, result and source, a decision's lead

**Status:** proposed
**Date:** 2026-10-07

> Adds four **optional** fields to contract 2 under ADR-0035 §6, before
> 0.2.0 is tagged: nothing required is added, nothing is removed or
> changes meaning, `contractVersion` stays 2. Extends ADR-0025 (two more
> clipped texts, with their own marker) and CONTRACT.md rule 9. Answers
> the contract question of WP-102's handover (an imported case's source).
> Work package: WP-127.

## Context

The desk (ADR-0034 §2) shows details the index cannot fill today, so the
plugin either guesses or sends the user to the editor:

1. **Why a crisis is loud.** The Changelog's "why loud" callout names the
   ADR-0028 rule that classified the item. The engine computes that rule
   on every index build (`class.rs`) and reports it in `seldon drift show`,
   but `index.drift` does not carry it. WP-122 therefore runs one `drift
   show` per selected crisis and shows a guess until it answers.
2. **What a case is for and what came of it.** Work's case detail shows
   key/values, the plan's progress and the log; Intent and Result are in
   the case file, which the plugin never reads (AGENTS.md §3). For a
   one-line intent the user has to open the editor.
3. **What a decision decided.** The Decisions detail shows the title and
   "the text is in the file".
4. **Where an imported case came from.** `seldon import task` (WP-102a)
   writes the source (`~/…/file.md#line`) into the case's Log line and
   into its marker `.seldon/imports/tasks.json`. The desk's "imported
   from" line (WP-102b) may parse neither. `case.schema.json` validates
   frontmatter and index cases alike with `additionalProperties: false`,
   so a frontmatter key is a schema change too.

## Decision

Each field below is optional in the schema, written by the engine on
every index build, checked by `seldon index --check`, shown in
`fixtures/index.sample.json` and derived by `scripts/validate-fixtures.py`.

### 1. `drift[].rule`

The id of the ADR-0028 §2 row that classified the item — exactly the
`rule` that `seldon drift show <id>` and `drift list --json` report for
it (for a pacman group, the group's rule; `attention-all` under `[drift]
attention = "all"`). A lowercase slug, at most 64 characters. The set of
ids is not closed in the schema: a later rule is no contract change, and
the plugin shows an id it does not know by name.

The plugin reads `rule` when the item has it and runs `drift show` only
when it does not (an index from an earlier v2 build). With the field, a
click in the Changelog starts no process.

### 2. `cases[].intent`, `cases[].result`, `decisions[].lead`

- `intent`: the first paragraph of the case's `## Intent` section;
  `result`: the first paragraph of its `## Result`; `lead`: the first
  paragraph of the decision's `## Decision`. These headings are the
  built-in templates' in every logbook language.
- **First paragraph:** the section's text without HTML comments (a
  template placeholder is no text, as for the plan, WP-115), leading
  blank lines and heading lines skipped, then every line up to the next
  blank line, each trimmed at the end, joined by line breaks. A section
  that is missing or has no such text gives no field.
- **An imported case** (tag `imported`, CONTRACT.md rule 8) begins its
  Intent with the engine's provenance line (`Imported from <source> —
  read before you start this case.`, WP-102). When that line is the whole
  first paragraph, `intent` is the paragraph after it: the source is in
  `source` (§3), and the provenance line repeated as the intent would
  tell the user nothing.
- **Redacted** by the logbook's redaction (SPEC-ENGINE §7, `[redaction]
  patterns` included) on every build: these texts are written by hand,
  and nothing redacted them before. A `[redaction] patterns` entry that
  does not compile withholds all three fields (and `source`): the engine
  does not know what to hide.
- **Control characters** other than line breaks and tabs become spaces;
  **direction and format characters** (U+200E, U+200F, U+202A–U+202E,
  U+2066–U+2069; U+200B–U+200D, U+2060, U+FEFF) are dropped
  (orchestrator's decision, WP-127 round 2): the texts are shown in the
  shell process, a reordered line can mislead, and a zero-width space
  inside a token would hide it from its redaction rule. Both happen
  before the redaction.
- **Redaction before the clip:** a secret at the cut is masked whole,
  never cut into a prefix its rule no longer knows.
- **Only what is shown is read:** the build stops reading a section after
  the paragraphs it needs (two for an imported Intent), so a whole task
  file imported as an Intent costs no more than its first paragraphs.
- **Clipped** as ADR-0025 clips an event's detail — at most 256 bytes of
  JSON, on a character boundary, marker included — with the marker `…
  (N more characters in the file)`: the case or decision file has the
  rest, and *Open in editor* stays next to the text. No `truncated` flag:
  the plugin offers the file either way and keys nothing off the cut.
- The plugin shows each as plain text (rule 6), never as Markdown.

### 3. `cases[].source` and the frontmatter key `source`

- `seldon import task` writes **`source`** into the new case's
  frontmatter: the task's place as the import names it, `~/…/file.md#N`
  for an item (N its line) or `~/…/file.md` for a file imported whole —
  the same redacted text as the Log line and the report. The key is
  written only on import; any other case has none.
- **Not derived from the marker.** The marker is engine state for
  idempotency; the index would depend on a second file and on its
  settle rules, and a case whose marker entry is gone (a reset logbook
  copy) would lose its origin. The frontmatter makes the case file
  describe itself.
- **The marker stays the only idempotency key.** Editing or removing
  `source` changes nothing in a later import.
- **Shape:** starts with `~/`; no control, bidi or format characters (the
  set the import refuses in a path); at most 512 bytes in UTF-8, hence at
  most 512 characters. A longer path is written as `~/…` followed by as
  many of its last characters as fit (507 bytes), so the file name and
  line survive.
- **The index copies it** after the logbook's redaction once more (a
  hand edit is possible). A string that then breaks the shape is dropped
  with a build warning; a value that is not a string counts as no
  source. The case itself always loads: `source` is display only, so it
  is never a reason to skip a case.
- **Display only.** Neither engine nor plugin ever passes `source` to a
  command or opens it; the desk shows it as text.
- `case.schema.json` gains `source` (frontmatter and index) and `intent`
  and `result` (index only; the engine ignores frontmatter keys of those
  names, as it ignores any key it does not know).

### 4. Contract

`contractVersion` stays **2** (ADR-0035 §6). CONTRACT.md rule 9 lists the
four fields; rule 5's "cases, decisions and memory topics are not cut"
gains "except `intent`, `result` and `lead`". An index without the fields
(an earlier v2 build) is valid and renders: the plugin falls back to
`drift show` for the rule and hides the three texts and the source.

## Consequences

- Size, per object at most: a drift item about 74 bytes (`rule` and its
  key), a case about 1.06 KB (`intent` and `result` 256 bytes of JSON each,
  `source` 512 bytes, the keys), a decision about 0.27 KB. At the caps of
  CONTRACT.md rule 4 (200 drift items, 50 completed cases) that is about
  68 KB, plus about 1.06 KB per open case and 0.27 KB per decision, which
  are not capped: 300 open cases with every field full add about 320 KB,
  which the over-budget warning of ADR-0025 names. (A `source` of
  characters JSON escapes, `"` or `\`, can take up to twice its bytes;
  real paths hold few.)
- The build reads each case's and decision's body once more for its
  sections (it reads the files already) and runs three short texts per
  case through the redaction; `check-perf` covers the build.
- A 0.2.0 plugin against an earlier v2 engine build: the fallbacks above.
  A 0.1.x plugin meets the contract-2 mismatch banner, unchanged.
- Fixtures: C-2026-007 becomes an imported case (tag `imported`,
  `source`, the provenance line); every case with text in Intent or
  Result and every decision with a Decision section shows the new fields.
- SPEC-ENGINE §6, SPEC-LOGBOOK §3 (`source`), SPEC-PLUGIN (the details),
  CONTRACT.md rules 5 and 9.

## Alternatives considered

- *Derive `source` from the import marker* (WP-102 handover, option 1):
  rejected in §3.
- *The whole Intent section in the index:* unbounded per open case; the
  first paragraph is the one-liner the detail needs, the editor has the
  rest. WP-102b's "whole Intent as plain text" for an imported case reads
  the file through *Open in editor* or needs its own field and ADR.
- *A `truncated` flag beside each text* (as ADR-0035 §3): the plugin
  would key nothing off it.
- *`lead` as the first paragraph of the body:* that is the Context, not
  the decision.
