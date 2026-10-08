# WP-159 — Handover

Branch `wp/159-zero-width-secrets` from `next` (aa9c5902); merges into
`next`. Commits: 3bf3bce5 plan, 8fc6b956 engine, c37dc50b docs,
cc656095 de source line, a768139b mutants, ddb19900 mutant fix, then
this handover.

## Decision (for the reviewer): (a), engine-wide, inside `Redactor`

- **(a) holds engine-wide.** Every path through `Redactor` (`redact`,
  `redact_keeping_lines`, `matching_rules`, `matching_rules_by_line`)
  runs its rules, the built-in ones and `[redaction] patterns`, on a copy
  of the text without the invisible characters. Where nothing is masked,
  the text comes back byte for byte, invisible characters included, so
  WP-140's promise ("a note keeps them") holds. Where something is
  masked, the masked span is the original one: the invisible characters
  inside a masked match and at its edges go with it, and every other run
  goes back where it stood.
- **This is the one shared helper.** Every writer of free text already
  redacts through `Redactor` before it writes: `log`, `import task`, the
  closing summary, `plan`, `decide`, `drift`, `triage`, `agent`,
  `capture`, the hook and the collectors, plus the ledger's own pass over
  subject, detail and meta. So all of them get the fix, and none needed a
  call of its own.
- **(b) stays only where it already held.** The places that dropped the
  set before redaction keep dropping it: a hook's command line, the
  closing summary, plugin commit subjects, the index texts and `source`,
  and the text `import task` reads. They now drop it through one helper,
  `redact::without_invisible`. In those one-line or shown texts a hidden
  character has no use and can mislead. (b) was not added for notes,
  case files or the journal, where a ZWJ, a ZWNJ or an emoji's VS16
  belongs to the words.
- Consequence, documented in SPEC §7 and the guide: a user pattern that
  names an invisible character now matches nothing.

## What was done

1. **The set.** `redact::is_invisible` replaces
   `import::is_direction_or_format`, and every caller uses it. It
   contains WP-140's set plus everything this WP asked for: U+034F,
   U+115F, U+1160, U+3164, U+FFA0, U+FE00–U+FE0F, U+E0100–U+E01EF and
   U+180B–U+180D. I added three more: U+180F (the fourth Mongolian free
   variation selector, a variation selector the WP's list missed), U+17B4
   and U+17B5 (Khmer inherent vowels, invisible), and U+2065 (reserved,
   inside U+2060–U+206F). With these, the set holds every assigned
   Default_Ignorable_Code_Point. `import::bad_path_char` follows, so a
   task path may not hold these characters either.
2. **The helper.** `redact::without_invisible(text) -> Cow<str>`:
   borrowed when the text holds none, with an ASCII fast path.
3. **Redaction of the visible copy.** `Rule::replace_with` can carry an
   origin map: for each byte of the working text, the byte of the visible
   copy it came from, or `NO_ORIGIN` for a replacement's bytes (kept
   groups included). `restore` then puts each run of invisible characters
   back where both neighbours were copied unchanged. The map is built only
   when the text holds an invisible character.
4. **Contract side.** `fixtures/bad-path-chars.txt`, `plugin/Model.js`
   `BAD_PATH_CHARS` (one line), and `scripts/validate-fixtures.py`
   `FORMAT_SET` got the new ranges. There is no schema change: the
   schema's `source` pattern is looser than the engine's check, so every
   value the engine writes still matches. Contract 2 is unchanged.
5. **Docs.** ADR-0038 amendment note (WP-159). SPEC-ENGINE §3 (the task
   path), §4 (closing summary, plugin subjects), §6 (the set) and §7 (how
   redaction reads invisible characters). CONTRACT.md rule 9, guide 06
   en/de (one paragraph; de source line set), CHANGELOG and TESTING.

## Acceptance

- **The three examples are masked** (`to<X>ken=…`, `Authorization:
  Bearer<X> …`, `ghp_0123<X>4567…`):
  - in the event body: `tests/log.rs`
    `a_secret_split_by_an_invisible_character_is_masked` checks the
    ledger and the journal;
  - in the case file: `tests/import_task.rs`
    `a_secret_split_by_an_invisible_character_never_reaches_an_imported_case`
    checks the case file, the report, `plan show` and the ledger;
  - in the commit subject: `tests/plan.rs`
    `a_secret_split_by_an_invisible_character_is_masked_in_the_closing_commit`
    checks the closing commit, and
    `collectors::plugins::tests::a_subject_drops_every_format_character_before_the_redaction`
    checks plugin commit subjects for every new code point.

  Unit tests: `redact::tests::a_secret_split_by_an_invisible_character_is_masked`
  runs the three examples for 24 code points in all four entry points,
  plus a user pattern.
- **A text with a zero-width character and no secret is written
  unchanged** (decision above). Tests: the log test, and
  `redact::tests::invisible_characters_away_from_a_secret_stay`: byte for
  byte without a secret; runs away from a match, at the start and at the
  end are kept; runs at a match's edges are dropped; redacting twice gives
  the same text. Also `keeping_lines_over_an_invisible_character` and
  `without_invisible_drops_the_set`. WP-140's hook test (a note keeps its
  U+200D) still passes.
- **`just check` green:** `XDG_RUNTIME_DIR=<private> SELDON_FULL_CHECK=1
  JUST_TEMPDIR=<scratch> flock /tmp/seldon-check.lock just check` on
  a768139b exited 0 (`check: ok`, log `engine/target/check-wp159-r1.log`).
  2420 tests passed, 0 failed; ipc-restart 44/0; plugin-test, docs-check
  and qmllint ok. The private runtime dir was created 0700 and removed by
  its path afterwards; `/run/user/$UID` stayed at 2 % before and after.
  The commits after a768139b change only `work/`.
- **Mutants on the helper are killed.** `work/active/WP-159/mutants.py`,
  run from a copy of the tree with its own target
  (`engine/target/mutants-wp159`), killed 33/33. They cover 12 for the
  set's new ranges (lib and the Python reference test), 2 for the helper,
  3 for the visible copy in each entry point, 4 for the origin map, 6 for
  `restore` (edges, start, end, off by one, byte offsets), and 6 for the
  places that drop the set (path, hook, closing summary, plugin subject,
  index text, import task). On the first run one mutant was "killed" only
  by a compile error; I rewrote it so it compiles, and the lib tests kill
  it (ddb19900).
- **`check-perf` redaction rows** (`--profile bench --test redaction
  long_lines`; only this test, not the whole `check-perf`): every budget
  met on the first attempt. The new row `invisible characters` (no secret)
  took 0.31 ms at 16 KB and 1.35 ms at 64 KB (budget 1/2 ms); `split
  secrets` took 3.41 ms at 128 KB (budget 20 ms). Old rows are unchanged:
  `german note` 64 KB 0.77 ms, `password and token` 128 KB 5.05 ms, `two
  option kinds` 128 KB 10.6 ms. This ran while the mutant builds were
  using the CPU, so the numbers are an upper bound. The text with
  invisible characters costs about 1.7× a plain non-ASCII text, because
  of the copy and the origin map.

## Reachable from the plugin (for the 0.2.0 question)

Yes, on the base. The desk's note field runs `seldon log`, which kept
every invisible character, so a pasted `to<U+200B>ken=…` reached the
ledger and the journal unmasked. The desk's import runs `import task`;
before this WP it dropped only the old set, so a task split by U+FE0F,
U+3164 or U+E0100 leaked. Both are fixed here.

## Not done / open

- The vault import's home-path rewrite (`Scrubber::home_paths`) reads
  the text as given, so `/ho<U+200B>me/alice` is not rewritten to `~`. It
  is a private path, not a secret rule, and not in this WP. A follow-up if
  wanted.
- `docs-check` warns that `docs/user/de/01-getting-started.md` lags its
  English page. That comes from the base (fd7b741d on `next`), not this
  WP.
- `marked_text` (`plan show --json`) now also marks variation selectors
  and fillers as `‹U+XXXX›`, so a hand-written Intent with an emoji's VS16
  shows the marker and keeps Start off (WP-102b's rule). That is intended:
  variation selectors can carry hidden bytes. The reviewer may want to
  confirm it.
- The index's texts drop VS16, so an emoji in a desk detail may draw in
  text style.
- No live check on the test host. Everything is covered end to end in
  temp homes.

## Guard

No guard-hook block in this WP.

## Hygiene

`git diff aa9c5902..HEAD`: no added home path, user name or host name.
Nothing from the private review folder was copied.
