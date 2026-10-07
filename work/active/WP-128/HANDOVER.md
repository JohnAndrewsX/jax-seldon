# WP-128 — Handover

Branch `wp/128-crlf-redaction` from `next` (109d02eb, WP-102a
included); merges into `next`.

## What was done

- **Every rule that continues over a line break reads `\r\n` as it
  reads `\n`** (`engine/src/redact.rs`):
  - `GAP` (white space or `\` line end between an option and its value):
    `\\\r?\n`. Covers `password-option`, `secret-option`, `openssl-pass`,
    `key-option`, every curl option rule, `sshpass`, `docker … login`.
  - `WORD` (an option's value): the backslash escape inside `"…"`, inside
    `$'…'` and bare reads `\` + CRLF as one escape (`\\(?:\r\n|(?s:.))`).
  - `PASS_ARG` (openssl's `pass:` value): the same in all five places.
  - `COMMAND_REST` (the command between its word and the option): the
    bare escape outside quotes. Its quoted forms already took any line
    end, so they are unchanged.
  - `HTTPIE_GAP`: `[ \t]|\r?\n|\\\r?\n`. The `httpie-auth` triggers gain
    `http\r`, `https\r`, `xh\r`, `xhs\r` (`+-a`), so a CRLF line still
    triggers the rule.
  - `db-client-password`: `(?:\\\r?\n|[^\n])` before `-p` and after its
    value.
- **CRLF text keeps its CRLF line ends** (`Rule::replace_with`): a `\r`
  that ends a replaced match (a rule that takes the rest of a line:
  `Authorization:`, `mysql … -p`, an unclosed quote) is put back after
  the marker. `redact_keeping_lines` puts back each line break a match
  held as it was (`\r\n` or `\n`), not always as `\n`.
- **Tests** (`engine/tests/redaction.rs`):
  - `CONTINUED`: 25 rows, one for each place where a rule reads a line
    end, each in LF form.
  - `redaction::crlf_line_ends_continue_as_lf_line_ends_do`: each row
    with `\r\n` loses its secrets, matches the same rules and gives the
    LF result with `\r\n`. This holds through `redact` and through
    `redact_keeping_lines`, which keeps the number of lines and of CRLF.
    A second pass changes nothing.
  - `commands::log_masks_continued_lines_with_lf_and_crlf_line_ends`:
    every row is run through `seldon log`, LF and CRLF. The event detail
    equals the redaction, and no secret is left in any file of the
    logbook or the state directory or in `git log -p`.
  - The HTTPie gap loop in `an_option_and_its_value_may_stand_on_two_lines`
    also runs `\r\n` and `\\\r\n` for all four command words.
- **Docs:** SPEC-ENGINE §7 now says that a line end is `\n` or `\r\n`
  for every rule, and describes the `\r` and line-break handling.
  SPEC-ENGINE §3 (`import task`) loses its "the engine-wide rule fix is
  its own WP" note. CHANGELOG has an Engine entry.

## Decisions (what the WP left open)

1. **The import keeps its CRLF→LF step.** WP-102 round 3 reads CRLF as
   LF before redaction. Removing that now would change the redacted
   item text, and with it the task hashes in `.seldon/imports/tasks.json`.
   A CRLF task file that was already imported would then come in again
   as "changed". The parser also expects LF. Only the code comment
   (`commands/import/task.rs`) and SPEC §3 now say why the step stays.
2. **Bare `\r\n` after HTTPie's word counts too.** `HTTPIE_GAP` already
   took a bare `\n` (`http⏎-a x`, WP-097). For parity it now takes
   `\r\n`, and the triggers gain the `\r` forms. A lone `\r` (old Mac
   line ends) is not a line end anywhere; this is unchanged.
3. **A `\r` at the end of a masked match is put back.** This is not a
   leak, but without it a CRLF note came out with mixed line ends and
   `redact(crlf) ≠ redact(lf)` with CRLF line ends. It applies to user
   patterns too (`token: .*` takes the `\r`, since `.` matches it). A
   `\r` holds no secret, and the second pass still gives the same text.
4. **Only the escapes that need it changed.** The quoted forms of
   `COMMAND_REST` already cross any line end, so changing them would add
   automaton size and leave mutants that no test can kill.

## How it was verified

- `cargo test --test redaction`: 29 tests, 28 run, 1 ignored (timing),
  all green. `cargo test --lib redact`: green. clippy `-D warnings` and
  fmt are clean. `scripts/docs-check.sh` reports ok.
- **Mutants:** `python3 work/active/WP-128/mutants.py` (own target dir
  `engine/target/mutants`) reverts each CRLF form on its own: GAP,
  HTTPIE_GAP bare and `\`, the HTTPie `\r` triggers, the db prefix and
  tail, the COMMAND_REST escape, the three WORD escapes, the five
  PASS_ARG escapes, the trailing `\r` and the CRLF put-back in
  `redact_keeping_lines`. **All 17 are killed.** The 15 regex mutants
  fail both the unit table and the `seldon log` test. The two line-end
  mutants fail the unit table, since `seldon log` checks for secrets,
  not line ends.
- **`flock /tmp/seldon-check.lock just check-perf`: exit 0.** Every
  budget passed on the first attempt. The load was 4–8, because another
  agent's `just check` was running. Redaction medians (bench profile,
  21 runs):

  | Row | 16 / 64 / 128 KB | budget |
  |---|---|---|
  | continued lines | 0.139 / 0.569 ms | 1 / 2 ms |
  | quoted line ends | 0.144 / 0.584 ms | 1 / 2 ms |
  | HTTPie line | 0.081 / 0.320 ms | 1 / 2 ms |
  | url line | 0.154 / 0.616 ms | 1 / 2 ms |
  | continued options | 1.24 / 5.04 / 10.08 ms | 20 ms at 128 KB |
  | two option kinds | 1.13 / 4.59 / 9.26 ms | 20 ms at 128 KB |
  | HTTPie options | 0.65 / 2.61 / 5.32 ms | 20 ms at 128 KB |
  | password and token | … / … / 5.05 ms | 10 ms at 128 KB |

  These are within noise of WP-097's recorded rows (continued options
  were 9.78 ms at 128 KB, continued lines 0.155 / 0.627 ms). I ran no
  A/B against the base.

  Hooks: 10 000 lines 0.78 / 1.80 / 3.07 / 2.56 ms; 900 lines 0.72 /
  3.40 / 4.84 / 4.55 ms (5 ms budget). The 900-line curl line with a
  marker at 4.84 ms was taken at load ~8 (WP-097 recorded 4.26 ms).
  `session-start` passed on its second attempt (0.97 ms of 1 ms).
- **`flock /tmp/seldon-check.lock just check` at `41dd267c`: exit 0**
  (`check: ok`; log `check-wp128-r1.log`). 84 test binaries `ok`;
  install 209/0, deploy-test-host 190/0, real-home-guard 11/0,
  service-states 328/0, desk-view 1346/0, bar-view 194/0, model.test.js
  124; qmllint ok (46 files), docs-check ok, plugin-test ok. The commits
  after it add only this handover.
- `git diff 109d02eb..HEAD` holds no home path, user name or host name (grepped).

## Not done / open

- No A/B bench against the base: the brief asked for the budget, and it
  holds.
- `docs/TESTING.md` has no row for `engine/tests/redaction.rs`, so
  none was added for these tests.
- No live check on the test host. Redaction is pure, and `seldon log`
  is covered end to end in a temp HOME.

## Round 2

The stage-1 review approved round 1 (4da0fbfc) with four non-blocking
findings. The orchestrator asked for N1–N4 before stage 2.

### What was done

- **N1, the `\r` comes back only after a built-in rule.** A user
  pattern's match is replaced whole, `\r` included, as on base. The new
  test `redaction::a_user_pattern_that_matches_a_cr_is_stable` uses the
  patterns `\r` and `[ \t\r]+` on the review's text
  `a token: abc\r\nb \r\n` and on `x\r\n\r\ny\r`. One pass leaves no
  `\r`, and a second pass gives the same text. This is the `seldon log`
  path: the command redacts the note, then the ledger redacts it again.
  Round 1's decision 3 said "applies to user patterns too … the second
  pass still gives the same text". That is withdrawn; it was false for
  these patterns.
- **N2, the dead guard is gone.** `&& !out.ends_with('\r')` could never
  trigger, because no built-in replacement ends in `\r`. The check is
  now `matched.ends_with('\r') && self.name != USER_PATTERN`, with a
  one-line comment. `USER_PATTERN` is a new constant that
  `with_patterns` uses as well.
- **N3, the docs.** SPEC-ENGINE §7 now says that CRLF line ends are kept
  "through `seldon log`, the other notes and the hook alike; the import
  reads CRLF as LF first (§3), so its cases hold LF". The CHANGELOG says
  "A CRLF note keeps its line ends; the task import reads CRLF as LF
  first, as before".
- **N4, lone CR and empty header values.**
  - `redaction::a_lone_cr_is_no_line_end` covers all four HTTPie words:
    `http\r-a a:b` stays as it is and matches no rule. This kills the
    review's mutant M8.
  - SPEC §7 now documents classic Mac line ends: a `\` before a lone
    `\r` continues nothing, and `curl -u \`, `\r`, then the credentials
    keeps them. This is the same as base and outside `\\\r?\n`.
  - `authorization-header` and `secret-header` now end their value with
    `[^'"\n]*[^'"\r\n]` instead of `[^'"\n]+`. For text without `\r`
    this is the same set of matches, so LF output does not change. An
    empty value before CRLF now gives what LF gives:
    `Authorization:‹redacted›\r\n` and `x-api-key: ‹redacted›\r\n\r\n`.
  - Three rows were added to `CONTINUED`: `X-Api-Key: fakeCr26`, an
    empty `Authorization:` at the end, and `x-api-key: \t` before a
    blank line.
  - `cookie-header` is unchanged. Its tail `[^'"\n]*` may take the
    `\r`, but the built-in put-back already gives the LF result, so a
    change there would be an equivalent mutant.
- The `seldon log` test now compares the event detail with the
  redaction of the trimmed note, because `log` trims the note first.
  The new empty-value rows showed this.

### Decisions

5. **Glued markers on a second pass are not changed in this round.**
   The case is a built-in marker followed directly by a `\r` that a
   user pattern masks. An example is `Authorization: Bearer x` + CRLF
   with the pattern `\r`. Pass 1 gives `Authorization:
   ‹redacted›‹redacted›\n`. Pass 2 merges the two markers into one,
   because the header value takes both.
   - On base this text was stable, because the header value swallowed
     the `\r`. With round 1 and with round 2 it is not.
   - The class itself is older than WP-128. On base,
     `tool --password a` + CRLF with the pattern `\r` merges the same
     way, because WORD stops before `\r`. So does `--password a;` with
     the pattern `;`.
   - SPEC §7 promises idempotence for the built-in rules only ("a user
     pattern … is applied as written"). Nothing leaks: pass 2 only
     merges markers.
   - A general fix would leave a built-in match alone when the part it
     masks consists only of markers. That would change
     `matching_rules`, which the import's per-line hits call on text
     that is already redacted, and so WP-102's report counts. This is
     beyond a round-2 nit. **Proposed follow-up:** "a built-in match
     whose masked part is only markers is left as it is", with the
     import's counts checked.
6. **The bench gets no CRLF row** (review question 4). The orchestrator
   did not ask for one. The review's 1 MiB and 4 MiB CRLF runs show
   linear time at about LF cost.

### How it was verified

- `cargo test --test redaction`: 31 tests, 30 run, 1 ignored (timing),
  all green. clippy `-D warnings` and fmt are clean.
  `scripts/docs-check.sh` reports ok.
- **Mutants:** `work/active/WP-128/mutants.py` now runs 21 mutants: the
  17 from round 1, with the trailing-`\r` mutant updated to the new
  condition, plus four new ones. The new ones put the `\r` back for user
  patterns too (N1), let either header value end in `\r` (N4, two
  mutants), and let `HTTPIE_GAP` take a lone `\r` (N4, the review's
  M8). **All 21 are killed.** N1 is killed by
  `a_user_pattern_that_matches_a_cr_is_stable`, the two header mutants
  by the `CONTINUED` parity test, and the lone-CR mutant by
  `a_lone_cr_is_no_line_end`.
- `just check-perf` was not run again. The brief asks for `just check`,
  and the regex change is limited to two header rules whose language is
  unchanged for text without `\r`.
- **`flock /tmp/seldon-check.lock just check` at `431f0287`: exit 0**
  (`check: ok`; log `check-wp128-r2.log`). 84 test binaries `ok`;
  install 209/0, deploy-test-host 190/0, real-home-guard 11/0,
  service-states 328/0, desk-view 1346/0, bar-view 194/0, model.test.js
  124; qmllint ok (46 files), docs-check ok. The commit after it adds
  only this section.
- `git diff 109d02eb..HEAD` holds no home path, user name or host name
  (grepped).
