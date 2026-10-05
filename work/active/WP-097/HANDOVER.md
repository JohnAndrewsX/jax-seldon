```
WP-097 HANDOVER
```

Branch `wp/097-redact-forms`, base `1b23cf4`. Commits: `911d5d9` (rules
and rows), `25361f9` (timing rows), `2b878d6` (hook test), `27fb88e` and
`b8b3035` (continuation rows), `d749524` (SPEC-ENGINE §7), `71623d3`
(CHANGELOG), `5183b4b` (pitfalls), plus this file.

## Done

All five items of the WP, in its order. Everything is in
`engine/src/redact.rs`.

1. **Continued lines and quotes across lines.**
   - **Hook path, checked first.** `hook.rs` `bash_record` calls
     `pkgcmd::parse_shell(command)` and records `line.text`. That is the
     whole command line with only the heredoc bodies cut and trailing
     white space trimmed; `\⏎` and newlines inside quotes stay. `record`
     then runs `ledger.redactor().redact(&r.command)` before `clip`
     (4096), and the ledger redacts again on append. So the full
     multi-line text reaches redaction. The new end-to-end test
     `commands::the_hook_masks_a_command_continued_over_lines` proves it:
     `seldon hook generic` gets a curl command over five lines, with
     `-u` on line 3 and `-U` after a quoted JSON body over three lines.
     The test compares the recorded `meta.command` byte for byte, and
     `assert_nowhere` checks the ledger, the index, the logbook files
     and the git history.
   - **`COMMAND_REST`** (the quote-aware branch; the plain branch is
     unchanged):
     - `\` followed by any character, a line end included, continues
       the command;
     - single- and double-quoted strings may span lines;
     - a quote the text never closes still ends the command at the line
       end.
   - **`GAP`** (`\s` or `\⏎`) stands between every option and its value
     (`-u \⏎ admin:pw`), and between the HTTPie command word and the
     rest of the command.
   - **`db-client-password`**: its context and its masked tail now
     follow `\⏎` lines.
2. **`--oauth2-bearer`, `--pass`, `-E`/`--cert`.**
   - `secret-option` gets the names `pass` and `bearer`. This covers
     `--pass`, `--proxy-pass`, `--oauth2-bearer`, xh's `--bearer`, and
     also `--cert-key-pass`-style names.
   - New rule **`cert-password`** (an `option_rule` with curl scan-on).
     It matches `curl -E`, `--cert` and `--proxy-cert`; the two long
     forms also count without the command word. It masks the value
     only when the value holds `:`, and masks it whole (the file name
     included). Without a `:` it continues to a later `-E`, like
     `cookie-option` with a file.
3. **`http -a` / `xh -a`.** New rule **`httpie-auth`** for `http`,
   `https`, `xh` and `xhs` with `-a`/`--auth`. It masks any value (a
   bearer token is the `-a` value as well) and scans on. The command
   word must be followed by white space or `\⏎`, so `http://… -a log`
   (wget, yt-dlp) is no match. The lead's optional group is lazy (`??`);
   see the pitfall.
4. **`2>&1`.** The quote-aware branch reads `[<>]&` (`2>&1`, `>&2`,
   `<&3`), `&>` and `>|` as redirections. `&&`, `&` followed by a space,
   and `|` still end the command.
5. **Quote pairing.**
   - `$'…'` is an alternative in `COMMAND_REST`. It is tried before the
     ordinary `$`, so the review-2 line
     `curl -H $'a;b\'"' -u admin:pw -o "x"` is masked.
   - The escaped quote: option values are now **`WORD`**, one shell
     word. A word joins quoted and bare parts and may hold `$'…'`, `\"`
     inside double quotes, and `\x` escapes (`\;`, `\⏎`). A
     double-quoted part that never closes the escaped way falls back to
     the old `"[^"]*"` reading. `-u "a\"b;c" … -u x` masks both values.
     The scan-on resumes after the whole value.
   - **Same root cause, also fixed** (found while probing):
     - `-u admin:'p w'` and `-u "$U":pw` leaked the quoted or bare
       second part;
     - `--password $'…'` leaked everything after the `$`.
   - **`VALUE`** (the `key=` assignments) stays one part, because a
     shell word there over-masks quoted URLs (pitfall). It gains `\"`
     inside double quotes, with the old reading as fallback.

**Also found and done (cheap widening, same class):**
- `password-option` now covers wget's `--http-password` and
  `--ftp-password` before a space. The `=` forms were already
  `secret-assignment`.
- `builtin_rules` now `debug_assert!`s that its names equal `BUILTIN` in
  order.

**`option_rule`** now takes the command lead from `command(word)`, so
`httpie-auth` can bring its own lead. The other five rules build exactly
the pattern they built before. `BUILTIN` has 26 entries.

**Tests** (`engine/tests/redaction.rs`):
- TABLE: 41 new rows. They cover each item, each new rule form, the
  escaped-quote and joined-word forms, and two wget rows.
- CLEAR: 15 new rows:
  - an unquoted line end ends the command;
  - `\ ⏎` is no continuation;
  - an apostrophe on one line;
  - `&&`, `&` and `;` after a redirection;
  - a cert without a password;
  - `--cert-type`;
  - curl's `-e`;
  - `| grep -E 'a:b'`;
  - `http -A bearer`;
  - `-a` after an `http://` URL (wget, yt-dlp);
  - `--pass-through`, `--bypass` and `--password-stdin`.
- **Removed CLEAR row**:
  `git commit -m "Fix curl 'quote⏎handling' -U flag"`. It pinned
  "a quoted string ends at the line end", which this WP reverses on
  purpose. Its value is now masked, and SPEC §7 lists this as accepted
  over-masking.
- New tests:
  - `an_option_and_its_value_may_stand_on_two_lines`: 20 forms, every
    option rule, each with exactly one rule matching, and idempotent;
  - `cert_and_httpie_rules_are_disjoint_and_stable`: the new rules
    match only their own rows; the `--pass`/`bearer` rows match only
    `secret-option`; every row is stable on a second pass;
  - the hook test above.
- Timing (`long_lines_with_a_marker_stay_fast`):
  - three no-option rows (continued lines, quotes around every line end,
    an HTTPie line), budgets 1 ms at 16 KB and 2 ms at 64 KB;
  - two many-match rows (continued options, HTTPie options), budget
    20 ms at 128 KB.

**Docs:**
- SPEC-ENGINE §7: the rule list, the context and value paragraph, and a
  limits paragraph (see below).
- Module doc of `redact.rs`.
- CHANGELOG `[Unreleased] / Engine`, in neutral wording.

## Not done (documented as limits in SPEC §7)

- As the WP says:
  - the last `sshpass -p` of several;
  - `.netrc` contents echoed (also `curl -n`/`-K` config files);
  - abbreviated long options (`--us`).
- **Combined short options:** `curl -su admin:pw` and `-sE c.pem:pw`
  are not masked (probe confirmed). curl reads `-su x` as `-s -u x`.
  The triggers are the option as written (`curl+-u`), and a trigger
  that every curl line holds would compile all curl rules for every
  curl line, which costs about 0.4–0.8 ms per hook call. See Decisions.
- `openssl … -pass pass:…` (also `-passin`/`-passout`) is not masked.
  It is outside the WP list. A follow-up rule is cheap; see Decisions.
- An assignment value that joins parts: `PASSWORD=a'b'` keeps `'b'`.
  Kept on purpose (pitfall: a shell word after `token=` breaks quoted
  URLs).
- **Over-masking accepted and documented:**
  - an option word inside a quoted argument that spans lines;
  - a stray apostrophe that pairs with one on a later line;
  - `x264 --pass 1`;
  - a certificate's file name next to its password;
  - an option value at the end of a quoted string
    (`bash -c "curl -u a:b" && echo "x"` → `bash -c "curl -u ‹redacted›"`,
    probe confirmed).
- No user guide change. Guide 06's rule list names categories, and the
  WP lists only SPEC and CHANGELOG. Say if guide 06 en/de should list
  the new forms.

## Verified by

- `cargo fmt --check` and `cargo clippy --all-targets --locked -- -D
  warnings`: clean.
- `redaction`: 24 passed, 1 ignored (debug).
- `scripts/docs-check.sh`: ok.
- **`flock /tmp/seldon-check.lock just check` at `5183b4b`: exit 0**
  (21:23–21:41). Results:
  - 70 test binaries `ok`;
  - service-states 314/0, panel-view 782/0, overlay-view 319/0,
    bar-view 143/0;
  - install 209/0, deploy-test-host 190/0, real-home-guard 11/0;
  - qmllint 29 files, docs-check ok, `check: ok`.
- **`flock /tmp/seldon-check.lock just check-perf` at `5183b4b`: exit
  0** (21:41–21:50). Every budget passed: index ×150, hooks and
  redaction.

### Timings

**Redaction, bench profile, median of 21** (`check-perf` run, load
about 4). The WP-087/WP-093 rows stay within their budgets. For
example, the url line is 0.63 ms at 64 KB against a 2 ms budget, and
two option kinds at 128 KB take 9.2 ms against 20 ms. The url line is
about +0.1 ms over WP-087's recorded 0.53 ms. That figure was measured
at another load, and I did not run an A/B for it. The new rows:

| Row | 16 / 64 / 128 KB | budget |
|---|---|---|
| continued lines | 0.155 / 0.627 ms | 1 / 2 ms |
| quoted line ends | 0.159 / 0.643 ms | 1 / 2 ms |
| HTTPie line | 0.075 / 0.307 ms | 1 / 2 ms |
| continued options | 1.19 / 4.81 / 9.78 ms | 20 ms at 128 KB |
| HTTPie options | 0.57 / 2.23 / 4.46 ms | 20 ms at 128 KB |

Growth is ×2.0 per doubling (16 → 64 → 128 KB) in every row.

**Hook A/B, curl line with a marker** (method of WP-087: bench builds
of `seldon` from base `1b23cf4` and from head, swapped into
`target/release/seldon`, then the hooks test binary run directly, 3
interleaved rounds under the lock, load 7.9 → 4.0):

| Case | base | head | diff |
|---|---|---|---|
| 10 000 lines | 2.40 / 2.40 / 2.40 ms | 2.54 / 2.49 / 2.50 ms | +0.14 / +0.10 / +0.11 |
| 900 lines | 4.04 / 4.01 / 3.94 ms | 4.19 / 4.09 / 4.19 ms | +0.15 / +0.08 / +0.25 |

- **Median cost: about +0.1 ms at 10 000 lines and +0.15 ms at 900
  lines.** The cause is the larger `COMMAND_REST`/`WORD` automata that
  `curl-user` and `proxy-option` compile for this line. The new rules
  are not triggered by it.
- **Headroom at 900 lines:** 0.8 ms under 5 ms at load 4–8. The
  `check-perf` run gave 4.26 ms.
- The recorded and not-recorded hook cases are unchanged within noise.
- `target/release/seldon` is a fresh bench build of head again.

### Mutants

Each mutant ran `--test redaction --no-fail-fast`. The source was
restored with `git checkout HEAD` + `touch`, and the baseline was green
afterwards. The runner is a throwaway script in the session scratchpad
and is not committed. The table names the killing tests (`…` =
further tests).

| # | Mutant | Killed by |
|---|---|---|
| C1 | `\\(?s:.)` → `\\.` outside quotes (no `\⏎`) | `every_builtin_pattern`, `an_option_…_two_lines`, hook test |
| C2 | single quotes stop at the line end | `every_builtin_pattern`, `proxy_json_…`, hook test |
| C3 | double quotes stop at the line end | `every_builtin_pattern`, `proxy_json_…` |
| C4 | no `$'…'` in the context | `every_builtin_pattern` |
| C5 | no `[<>]&` | `every_builtin_pattern`, `proxy_json_…` |
| C6 | `[<>]&` → `>&` | same |
| C7 | no `&>` | same |
| C8 | no `>\|` | `every_builtin_pattern` |
| C9 | unclosed-quote tail crosses line ends | `harmless_text_stays` |
| C10 | ordinary character may be a line end | `harmless_text_stays`, `masking_twice…` |
| C11 | `&>` → any `&` | `harmless_text_stays` |
| C12 | `$'…'` without escapes | `every_builtin_pattern` |
| G1 | `GAP` = `\s` only | `every_builtin_pattern`, `an_option_…_two_lines` |
| G2–G4, G6, G7 | per-rule gap `\s` only (`-u`, `--password`, `sshpass -p`, `-E`, `login -p`) | `an_option_…_two_lines` (+ `every_builtin_pattern` for G2–G4) |
| G5 | HTTPie lead `\s` only | `an_option_…_two_lines`, **after `b8b3035`** (survived first: the row had a space before `\`) |
| W1 | `WORD` = old `VALUE` | `every_builtin_pattern`, `masking_twice…`, `cert_and_httpie_…`, `email_…` |
| W2 | `WORD` without `$'…'` | `every_builtin_pattern` |
| W3 | `WORD` without `\"` | `every_builtin_pattern`, `masking_twice…` |
| W4 | `WORD` without `\x` | `every_builtin_pattern` |
| W5 | `WORD` without the unclosed fallback | `every_builtin_pattern` |
| W6 | `WORD` escape not across a line end | `every_builtin_pattern` |
| W7 | `WORD` parts not joined | `every_builtin_pattern`, `cert_and_httpie_…`, `masking_twice…`, `email_…` |
| V1 | `VALUE` without `\"` | `every_builtin_pattern`, `masking_twice…`, … |
| V2 | `VALUE` without the unclosed fallback | `every_builtin_pattern` |
| R1 / R2 | `secret-option` without `pass` / `bearer` | `every_builtin_pattern`, `cert_and_httpie_…` |
| R3 | cert without the `:` check | `harmless_text_stays`, `every_builtin_pattern`, `cert_and_httpie_…` |
| R4 | cert `-E` case-insensitive | `harmless_text_stays` (curl `-e` row) |
| R5 | cert without the plain long forms | `every_builtin_pattern`, `an_option_…_two_lines`, `cert_and_httpie_…` |
| R6 | cert without `--proxy-cert` | `every_builtin_pattern`, `cert_and_httpie_…` |
| R7 | cert without scan-on | `every_builtin_pattern`, `cert_and_httpie_…` |
| R8 | HTTPie lead without its gap (`http://` counts) | `harmless_text_stays` |
| R9 | HTTPie lead greedy `?` | `every_builtin_pattern` (repeated `-a` row) |
| R10 | HTTPie `-a` case-insensitive | `harmless_text_stays`, `every_builtin_pattern` |
| R11 / R12 | HTTPie without `xh` / without `https` | `every_builtin_pattern`, `cert_and_httpie_…` (+ `an_option_…` for R11) |
| R13 | `password-option` without wget's forms | `every_builtin_pattern` |
| R14 / R15 | `db-client` context / tail single-line | `every_builtin_pattern` |
| R16 | cert trigger without `curl+-e` | `every_row_holds_a_trigger…`, `every_builtin_pattern`, … |
| R17 | HTTPie trigger without `xh` | `an_option_…_two_lines` |
| R18 | `secret-option` trigger without `-pass`/`bearer` | `every_row_holds_a_trigger…`, `every_builtin_pattern`, … |
| R19 | `BUILTIN` order swapped | the `debug_assert!` (every test that redacts) |
| R20 | `password-option` trigger without wget's forms | `every_row_holds_a_trigger…`, `every_builtin_pattern` |
| R21 | `command()` without group `cmd` (no scan-on) | `every_builtin_pattern`, `proxy_json_…`, `masking_twice…`, … |

Result: 51 mutants, 50 killed in the first run. G5 survived and was
killed after `b8b3035`.

Not mutated (perf-only, or no behaviour of its own): the lazy `??` is
covered by R9, and the order of the `$'` alternative before the
ordinary `$`.

### Probes (throwaway, not committed)

- About 30 lines, each checked for a second-pass change. None changed
  on a second pass.
- The probes confirmed every "not masked" and "over-masked" claim in
  SPEC §7.
- `x=$(curl -u …)`, `… 2>&1 | curl -u …` and `… & curl -u …` are
  masked, and a heredoc's curl is masked by redaction as well (the hook
  cuts it anyway).

## Learned

Appended to `memory/pitfalls.md`:
- leftmost-first priority with a greedy optional lead;
- a gap row needs no white space at the gap;
- why assignments keep `VALUE`;
- combined short options against triggers.

## Decisions needed

None blocking. Three for the orchestrator:

1. **Combined short options** (`curl -su a:b`). Fix them with a
   cluster prefix of curl's boolean letters
   (`-[0-46#:aBfgGIijJkLlMnNOpqRsSvVZ]*u`) and accept the trigger cost
   (every curl line compiles the curl rules, about +0.4–0.8 ms per hook
   call with a curl line, close to the 5 ms budget at 900 lines)? Or
   leave them as a documented limit (current state)?
2. **`openssl -pass pass:…`** (`-passin`, `-passout`): add a follow-up
   WP? It needs one unique-literal rule (`pass:` after
   `-pass(?:in|out)?`).
3. Guide 06 en/de: should it name the new forms? It is not in this WP's
   outputs.

## Touched outside WP scope

- `memory/pitfalls.md` (append).
- No guard-hook blocks. Nothing outside the repository and the session
  scratchpad. `capture.rs` and `collectors/config.rs` are untouched.
- `target/release/seldon` was swapped temporarily during the A/B. A
  fresh head bench build is in place.
