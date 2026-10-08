# WP-143 — Plan

Branch `wp/143-tightenings` from `next`; merges into `next`.

## Parts

1. **Case template** (`engine/templates/{en,de}/.seldon/templates/case.md`):
   the Plan gains `- Persists:` (after *Affected paths*) and `- Stop if:`
   (last), both empty. `plan new`, `agent start --new`, `plan reopen`,
   `import task` all render through `cases::new_body`, so they all get
   them.
2. **Rules and skill** (rules block v4 en/de, `assets/skills/seldon/`
   `SKILL.md` and `case.md`, `docs/AGENT-GUIDE.md`):
   - fill *Stop if* and stop when it holds;
   - fill *Persists* with one of three values;
   - verify the effect, not the setting (example: press the key binding,
     not only write it);
   - every claim in *Result* and `memory/` carries `measured`,
     `documented` or `inferred`.
3. **Closing commit**: `plan done` commits
   `seldon: <ID> completed — <title>: <first line of Result>`, redacted,
   clipped; `plan drop` `seldon: <ID> dropped — <title>[: <reason>]`.
4. **doctor row `workpieces`**: `work/<case-id>…/` folders that no case
   owns (orphaned) or whose closed case left a large folder (oversized):
   count, size, the oldest; always `ok` (information only).
5. Docs: SPEC-LOGBOOK (template, §2), SPEC-ENGINE (autocommit, doctor,
   plan new), user guide 06/07 (en, de), CHANGELOG.

## Decisions

- **D1 — rules block stays v4.** The text changes inside v4, as WP-111's
  did inside v3; the engine already treats "a block of this version with
  other text, exactly as an earlier engine shipped it" as unedited. The
  block of 0.1.4 (WP-116 as merged, 868c5da; unchanged on `next`) goes
  into `templates/rules-v4/AGENTS-wp116-{en,de}.md` and its two hashes
  into `RELEASED_BLOCKS`, so a 0.1.4 logbook is upgraded silently by the
  next capture (doctor: "v4 as Seldon wrote it; the next capture updates
  it to v4"; commit `rules update (unedited, v4 → v4)`). A bump to v5
  would touch the marker, the tests and the docs for no behaviour the
  hash list does not already give.
- **D2 — existing logbooks get the new template lines.** `init` copied
  `case.md` into `.seldon/templates/`; that copy was never changed since
  WP-006. A logbook copy that is byte for byte a template an earlier
  engine shipped (sha256 list, en and de) counts as the built-in: `plan
  new` uses this engine's template. Nothing is written into
  `.seldon/templates/`; a copy the user edited is used as it is.
- **D3 — the closing summary.** Tail after `— ` is the title, then `: `
  and the first line of the first paragraph of *Result* (a leading list
  marker `- `/`* `/`+ ` dropped) when there is one. Control characters
  become spaces; the tail is redacted (logbook redactor) and then clipped
  to 100 characters with `…` (redact before clip, so a cut cannot hide a
  secret from the patterns). A human close without *Result* gets the
  title alone.
- **D4 — drop.** `plan drop` is a closing step too: title, then the
  redacted `--reason` when given (the Result of a dropped case is usually
  empty; the reason is what a drop says). Same clip.
- **D5 — other completions keep their commit.** `drift explain`'s
  completed case and `import task --include-done` commit under their
  command's summary; they are not a `plan done`.
- **D6 — workpieces.** A workpiece folder is a directory directly under
  `work/` whose name is a case id (`C-YYYY-NNN`) followed by nothing or
  `-…` (`queued`, `active`, `completed` are not). Orphaned: no case file
  with that id (a case file that does not parse counts as present).
  Oversized: its case is completed or dropped and the folder holds more
  than 10 MiB. Size: regular files, symbolic links not followed. Oldest:
  the smallest case id (ids are chronological; no mtimes). Row placed
  after `rollbacks`/`planned`; `ok` always, no fix. Message:
  `none` or `N folder(s) left by closed or missing cases (O orphaned,
  B oversized), X in all; the oldest: work/<name>/`.
- **D7 — no schema change**, no contract bump.

## Tests

- engine unit: closing summary (title, result line, list marker, clip,
  redaction, control characters).
- integration (`tests/plan.rs`): the `plan done` commit subject with a
  Result; the drop subject.
- integration (`tests/doctor.rs`): workpieces row — none; orphaned;
  oversized of a completed case; a small one of a completed case and one
  of an open case not counted; oldest.
- template: built-in renders `Stop if:` and `Persists:`; a shipped copy
  in `.seldon/templates/` yields the new lines, an edited copy is kept.
- rules: `the_released_blocks_are_exactly_the_shipped_blocks` covers v4.
