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

## Round 2

Brief: the orchestrator's round-2 brief from the Opus stage-1 review
(SEND BACK for B1, B1b and B2; the C0/C1 controls required here; N1, N5,
the check-perf count, the ADR). `next` (a67bd480: ADR-0045, WP-168/169
queued) merged as 209fffcd without a conflict. Commits: d800bf34 engine,
c1de423b the scale count, e2ad33d7 docs and ADR-0048, 873b2d3c the de
source line (it also carries the updated `mutants.py`, swept in by a
`commit -a`).

### Fixed

- **B1 — two readings.** `Redactor::redact` and `redact_keeping_lines`
  read the text twice. First they read the copy without the characters
  `hides_from_rules` names, and mask the original span (`restore`). Then
  they read the result as given (`self.passes(&restored, …)`), where an
  invisible character is a boundary. `matching_rules` and
  `matching_rules_by_line` report the union of both readings, by rule
  index and in rule order.
  - The second reading runs over **all** rules, not only the
    boundary-anchored ones. It stays correct when rules change, and the
    bench below shows it fits the budgets.
  - `restore` now walks the runs once instead of a binary search per
    character. A text in which the first reading masks nothing is copied
    whole without `restore`.
  - Test: `redact::tests::an_invisible_character_before_a_secret_is_a_boundary`
    has 17 rows: `sk-key`, `db-client-password` (mysql, psql,
    smbclient), `curl-user`, `proxy-option` (`curl -U`), `cookie-option`
    (`curl -b`), `cert-password` (`curl -E`), `sshpass-password`,
    `cookie-header`, `secret-header` (`x-auth-token:`; `x-api-key:` is
    masked in either reading), `registry-login-password` (docker,
    podman), `nmcli-secret`, `httpie-auth`, plus `url-userinfo` and
    `email`. Each row runs with U+200B, U+FE0F, U+3164 and U+E0041,
    through all four entry points, and checks four things. The copy alone
    leaks, except for `url-userinfo` and `email`, which match at any word.
    The result does not leak. The character before the secret stays.
    Redacting twice gives the same text.
  - End to end: `log.rs` has `x<U+200B>sk-…` and `a<U+200B>mysql …
    -p…` rows.
- **B1b — redact, then drop.** `Redactor::redact_dropping_invisible`
  (new) is used by:
  - the index's `plain_text`;
  - `closing_summary`: its bidi controls are no longer turned into
    spaces first; they are invisible and go after the redaction;
  - plugin `commit_subject`;
  - `bash_records`: a line with invisible characters is redacted with
    the logbook's patterns (the built-in rules if they do not compile),
    then the characters are dropped, then the line is parsed. A line
    without them is unchanged;
  - `import task`: new `Scrubber::text_dropping_invisible` redacts the
    whole text keeping lines, then drops the characters, then rewrites
    home paths. So `/ho<U+200B>me/alice` becomes `~` again; with
    drop-after-redact alone it would have stayed. `droppedCharacters`
    still counts the file's invisible characters, those inside a masked
    secret included.

  Tests: `index::build::tests::shown_texts_are_redacted_before_invisible_characters_go`,
  the closing-summary and plugin-subject unit tests, a `sk-` row in
  `hooks.rs` `format_characters_hide_no_secret_on_a_command_line`, and
  an `x<U+200B>sk-…` plus `/ho<U+200B>me/alice/notes` line in
  `import_task.rs`.
- **B2 — `marked_text`.** Control characters become spaces, then the
  text is redacted, then the invisible characters in the result are
  marked. `hidden` counts every invisible character in the text, those
  inside a masked secret included. The file still holds them and an agent
  reads the file, so the desk's Start gate keys off what is there, not
  off what is shown. Test: `import_task.rs` `plan_show_redacts_before_it_marks`
  with a hand-edited case holding `to<U+200B>ken=…`,
  `ghp_0123<U+FE0F>…`, a plain `token=` and `x<U+200B>sk-…`; the
  result is `… x‹U+200B›‹redacted›`, `hidden` 3. `plan show`'s human
  output and `body` are the case file as it is (never redacted, the
  plain form included); the test checks only `intent`.
- **C0/C1 controls.** `hides_from_rules(c) = is_invisible(c) ||
  (c.is_control() && !c.is_whitespace())`, for the redaction's first
  reading only. It keeps `\t`, `\n`, `\r`, VT, FF and NEL. `is_invisible`
  (paths, index) is unchanged. Test: `a_control_character_splits_no_secret`
  covers BS, NUL, BEL, CSI (U+009B), ESC, DEL and US with `to<C>ken=` and
  `ghp_0123<C>4567…`; a text with only controls is unchanged; the six
  white-space controls are not in the predicate; a CRLF continuation
  works with a BEL in front. End to end, `log.rs` has BS, BEL, CSI and ESC
  rows (NUL cannot be in argv).
- **N1:** `a_run_at_the_start_before_a_match_goes_with_it`
  (`"\u{200B}token=abc"` → `"token=‹redacted›"`). The reviewer's M10
  (`resize(…, 0)`) is now killed by `lib`.
- **N5:** a CHANGELOG entry: a hand-edited emoji with U+FE0F keeps the
  desk's Start off (fails safe; start the case with `seldon plan start`).
- **check-perf count:** `status_at_10_000_ledger_lines_is_under_100_ms`
  asserted 10 788 ledger lines; the fixture logbook on `next` gives
  11 656. This is a count, not a budget. Updated in `tests/index.rs` and
  `tests/common/scale.rs`, and the stated number in SPEC-ENGINE §6 and
  TESTING.md. The 100 ms budget is unchanged (median 51.1 ms).
- **ADR:** the appended WP-159 note is gone from ADR-0038, which is back
  to `next`'s text. The new **ADR-0048** (proposed) amends ADR-0038 §2
  and §3, following ADR-0037 and ADR-0042. It covers the set (every
  assigned default-ignorable code point plus U+0600–U+0605 and
  U+FFF9–U+FFFB), the two readings, the controls in the first reading
  only, redact-then-drop, `plan show`'s marking and count, and what is
  left to WP-169. DECISIONS.md: ADR-0038 "accepted; §2/§3 amended by
  ADR-0048 (proposed)", and a row for ADR-0048.
- **Docs:** SPEC-ENGINE §4 (closing summary, plugin subjects), §6 (the
  order of the index texts) and §7 (the two readings, the controls,
  where the set is dropped, WP-169's leftovers); CONTRACT rule 9;
  guide 06 en/de ("inside or before", controls; de source line set);
  CHANGELOG; TESTING.

### Not here (WP-169)

ANSI CSI and OSC sequences as whole units (dropping ESC alone leaves
`pass[0mword=`); raw ESC kept in a note's journal entry and Markdown; tag
characters spelling a hidden text in a note (N3); the Cf characters
outside the set (N2: U+06DD, U+070F, U+0890–U+0891, U+08E2, U+110BD,
U+110CD, U+13430–U+1343F). ADR-0048 names them.

### How it was verified

- **`XDG_RUNTIME_DIR=<private> SELDON_FULL_CHECK=1 JUST_TEMPDIR=<scratch>
  flock /tmp/seldon-check.lock just check` on 873b2d3c: exit 0**
  (`check: ok`; log `engine/target/check-wp159-r2.log`). 2432 tests
  passed, 0 failed; ipc-restart 44/0; plugin-test, docs-check, qmllint
  ok. The docs-check warning about `de/01-getting-started.md` comes from
  `next`, as in round 1. The runtime dir was 0700 and removed by its path;
  `/run/user/$UID` was at 2 % before and after.
- **`flock /tmp/seldon-check.lock just check-perf` on 873b2d3c: exit 0**
  (log `engine/target/perf-wp159-r2.log`). All 62 budget rows passed on
  the first attempt.
  - Redaction: `invisible characters` 16 KB 0.31 ms and 64 KB 1.25 ms
    (budgets 1 and 2 ms; round 1 was 1.35 ms with one reading);
    `split secrets` 128 KB 4.32 ms (20 ms); `german note` 64 KB 0.82 ms;
    `two option kinds` 128 KB 10.5 ms (20 ms); `password and token`
    128 KB 5.42 ms (10 ms).
  - Hook: 10 000 lines 0.73 / 1.61 / 2.99 / 2.56 ms (5 ms); 900 lines
    0.70 / 3.03 / 4.42 ms (5 ms); unrelated session 0.69–0.85 ms (1 ms).
  - Index ×10 5.7 ms, ×150 68.8 ms; status at the stated scale 51.1 ms
    (100 ms).
- **Mutants:** `work/active/WP-159/mutants.py` now has 47, run from a
  `git archive` copy of 873b2d3c with its own target
  (`engine/target/mutants-wp159`): **47/47 killed**, none by a compile
  error. Round 2 adds:
  - 6 for the first reading: controls kept, white-space controls
    dropped, the ASCII path, the copy in each entry point;
  - 3 for the second reading: none, and no union in each
    `matching_rules*`;
  - M10 and the run walk;
  - 12 for redact-then-drop and mark: the helper, the hook (twice), the
    closing summary (twice), the plugin subject, the index text, `plan
    show` (marks before, counts after), and `import task` (keeps the
    characters, drops them after the home paths).
- This round's diff: no added home path, user or host name. Nothing from
  the private folder was copied.

### Guard

No guard-hook block.

## Ready (2026-10-09)

- ADR-0048 accepted by the operator (E43): status line and DECISIONS.md
  rows updated (690a3b80); ADR-0038's row reads "accepted; §2/§3 amended
  by ADR-0048".
- `next` at 48924e49 merged as b82d3523. CHANGELOG conflict: both
  entries kept (WP-159's and next's WP-164 entries). de guide 06 source
  line conflict: set to the merge, e0644048.
- **`XDG_RUNTIME_DIR=<private /tmp/r159h, 0700, removed after>
  SELDON_FULL_CHECK=1 JUST_TEMPDIR=<disk> TMPDIR=<disk> flock
  /tmp/seldon-check.lock just check` on e0644048: exit 0** (`check: ok`,
  log `engine/target/check-wp159-r3.log`). 2538 tests passed, 0 failed;
  ipc-restart 44/0; docs-check ok (no warnings); plugin-test ok. Cargo
  target and temp dirs were on disk; `/run/user/$UID` was at 2 % and
  `/tmp` at 2 %.
