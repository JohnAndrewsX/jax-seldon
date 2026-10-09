# WP-159 — Plan

Branch `wp/159-zero-width-secrets` from `next` (aa9c5902); merges into
`next`.

## Decision proposed for the reviewer: (a), engine-wide, inside `Redactor`

- **(a) holds engine-wide.** Every path through `Redactor` (`redact`,
  `redact_keeping_lines`, `matching_rules`, `matching_rules_by_line`)
  runs its rules on a copy of the text without the invisible characters.
  When that copy holds no secret, the text comes back exactly as given,
  invisible characters included: WP-140's promise ("a note keeps them")
  holds for a note without a secret. When it holds one, the result is the
  redacted copy with every invisible character put back where both of
  its neighbours were copied unchanged; the invisible characters inside a
  masked match, and at its edges, are gone with the match.
- So every writer of free text gets it, because each already redacts
  through `Redactor` before it writes (`log`, `import task`, the closing
  summary, `plan`, `decide`, `drift`, `triage`, `agent`, `capture`, the
  hook, the collectors, and the ledger's own pass over subject, detail
  and meta). There is one helper, not one per caller.
- **(b) stays where it already holds** and is not extended: the places
  that drop the set before redaction today (a hook's command line, the
  closing summary, plugin commit subjects, the index texts and `source`,
  the text `import task` reads) keep dropping it, through one shared
  `redact::without_invisible`, because there the characters have no use
  and a one-line text should not hide anything.
- A user pattern (`[redaction] patterns`) sees the copy too: a pattern
  that names an invisible character matches nothing. Documented.

## Items

1. `redact::is_invisible(c)` (the set) and
   `redact::without_invisible(text) -> Cow<str>` (borrowed when the text
   holds none; ASCII fast path). The set is `is_direction_or_format`'s
   plus U+034F, U+115F, U+1160, U+180B–U+180D, U+3164, U+FE00–U+FE0F,
   U+FFA0, U+E0100–U+E01EF. `import::is_direction_or_format` goes;
   every caller uses `redact::is_invisible` / `without_invisible`
   (`bad_path_char`, hook, plan, plugins, index build, import task).
2. `Redactor`: the four entry points work on the visible copy. For the
   masking of the original text, `Rule::replace_with` can carry an
   origin map (for each byte of the working text, the byte of the visible
   copy it came from, or none for a replacement), and a `restore` step
   puts the invisible runs back between two copied neighbours. The map is
   built only when the text holds an invisible character and its copy
   holds a secret.
3. Contract side: `fixtures/bad-path-chars.txt` gains the ranges (the
   engine's `bad_path_char` test reads it), `plugin/Model.js
   BAD_PATH_CHARS` the same (one line), `scripts/validate-fixtures.py
   FORMAT_SET` the same (the reference-set test compares it). No schema
   change: the schema's `source` pattern is looser than the engine's
   check, and every value the engine writes still matches it. Contract 2
   unchanged.
4. Tests:
   - unit (`redact`): each of `to<X>ken=…`, `Authorization: Bearer<X> …`,
     `ghp_0123<X>4567…` masked for every new code point and a sample of
     the old ones, in all four entry points; a text with invisible
     characters and no secret comes back byte for byte; invisible
     characters outside the match survive; idempotence (twice = once);
     CRLF and `keep_lines` line counts; a user pattern.
   - `is_invisible` per code point (each new range's ends and one inside;
     neighbours outside).
   - integration: `seldon log` (ledger, journal; a case note), `import
     task` (the case file), `plan done` (the closing commit subject),
     plugin commit subject (unit), each with the three examples.
   - existing: the hook test of WP-140, the `bad-path-chars` fixture
     tests (engine and plugin), the Python reference set.
5. Docs: ADR-0038 amendment note (WP-159), SPEC-ENGINE §6/§7 (the set,
   where it is dropped, where it is kept and how redaction sees it),
   CONTRACT.md rule 9, CHANGELOG, TESTING.md.
6. Mutants by hand on the new code (`work/active/WP-159/mutants.py`,
   from a copy of the tree, own target dir).
7. `just check` with a private runtime dir; `just check-perf` for the
   redaction bench (the visible-copy scan sits on the hot path).
