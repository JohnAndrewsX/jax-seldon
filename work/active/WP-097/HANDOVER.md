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

## Round 2 (review: stage 1 SEND BACK, stage 2 fix round)

Commits:
- `e8414b3`: items 1–3 and the item 4 rows, plus the `VALUE` fix below;
- `98d22ae`: item 5;
- `b67744c`, `0a7828b`, `72ed1de`: rows that killed surviving mutants;
- `cf2065a`: item 6;
- `4d51f6b` and `0139497`: item 7 (en, then de with its source line);
- then pitfalls and this section.

### Items

1. **`httpie-auth` triggers.** There are now 16 literals: `http`,
   `https`, `xh` and `xhs`, each followed by a space, `\t`, `\n` or `\`,
   each `+-a`. The lead gap is `HTTPIE_GAP` = `(?:[ \t\n]|\\\n)`.
   - **Also the option's leading white space** (`\s` before `-a`/`--auth`)
     is `HTTPIE_GAP`. The brief named only the lead. But when the lazy
     lead group is empty, the white space right after the word is the
     option's. With `\s` there, `http<VT>-a x` would match while it holds
     no trigger. The gap after `-a` stays `{GAP}`, as the brief says.
   - **Trigger-negative assertion: deviation from the brief, please
     check.** The brief's line `git commit -am "fix https redirect"`
     holds `https ` and `-a`, so with the 16 literals it does compile
     `httpie-auth` by construction. That is the case SPEC item 6(b)
     documents. `every_row_holds_a_trigger_of_its_rule` therefore asserts:
     - no `httpie-auth` trigger for the B1 shape:
       `git commit -am "fix https://h.example redirect"` and
       `curl -fsSL https://h.example/f -o f && git commit -a -m x && ls -a`;
     - a trigger **does** hold for the brief's line.
   - A loop pins every word × gap pair, so dropping any single literal
     is killed.
2. **`WORD`.** A quoted part no longer crosses a line end that no `\`
   escapes:
   - `"(?:[^"\\\n]|\\(?s:.))*"`, `'[^'\n]*'`, `\$'(?:[^'\\\n]|\\(?s:.))*'`,
     fallback `"[^"\n]*"`;
   - an unclosed-quote tail `(?:['"][^\n'"]*)?` after the parts, or that
     tail alone, so `-u 'admin:x` is masked to the line end.

   TABLE rows:
   - the two N2 texts (with their third line, which is what makes them
     bite);
   - `curl -u 'admin:fakeUnclosed3 …⏎next line`;
   - the same for an unclosed `"` and an unclosed `$'` (they killed N2,
     N3 and N4).

   The O1/O2 rows are kept (item 4).
3. **`secret-option`** no longer reads a first name word `no`
   (`NOT_NO`). The CLEAR row is `smbclient //srv/share --no-pass -c 'ls'`.
   A TABLE row `--node-token` keeps a word that only starts with `no`.
4. **N5:** two TABLE rows, `curl -d "a\⏎b" -u admin:fakeO2 …` and
   `curl -u "ad\"m\⏎in;x" … -u bob:fakeO1`. Mutants N8 (O1) and N9 (O2)
   are killed.
5. **`hooks.rs`.** `hook_budget` gets a third timed line:
   `set -e; curl -fsSL -u bob:fakePw2 https://h.example/install.sh |
   sudo -E bash && git commit -am zed`.
   - It runs in both families. That is the shared function; the 900-line
     test is the binding one.
   - The 900-line test now leaves room for 150 recorded commands instead
     of 100 (3 kinds × 2 attempts × 22).
6. **SPEC-ENGINE §7.**
   - (a) to (d) as worded in the brief, plus `--no-pass` in the
     not-masked list.
   - The `httpie-auth` and value paragraphs say that the gap is ASCII and
     that values are read per line.
7. **Guide 06 en/de.** One bullet each. The de source line now points at
   `4d51f6b`. Correction to round 1: guide 06 did list individual
   options (N6).

**Also fixed (found by this round's tests): `VALUE` idempotency.** The
WORD change shifted the quote parity of the joined-rows text in
`masking_twice_changes_nothing`, and it failed. The cause is pre-existing
since WP-093; it also happens on base `1b23cf4`.
- An address glued to a quoted `key=` value
  (`TOKEN="a"bob@example.com`) became two glued markers. The bare
  `VALUE` read them as a new value on the second pass.
- Fix: the bare `VALUE` does not start at `‹`.
- The three probe lines are now in `masking_twice_changes_nothing`, and
  mutant N19 is killed.

### Verified

- `cargo fmt --check` and `cargo clippy --all-targets --locked -- -D
  warnings`: clean. `redaction` 24 passed (1 ignored), `hooks` 55 passed
  (2 ignored). `docs-check`: ok.
- **`flock /tmp/seldon-check.lock just check` at `72ed1de`: exit 0**
  (22:32–22:41). Results:
  - 70 test binaries ok;
  - service-states 314/0, panel-view 782/0, overlay-view 319/0,
    bar-view 143/0;
  - install 209/0, deploy-test-host 190/0, real-home-guard 11/0;
  - qmllint 29 files, docs-check ok, `check: ok`.
- `just check-perf` was not run again: the brief asks for `just check`
  once. The hook timings below come from the A/B.
- Probes confirm the new SPEC sentences:
  - `Fixed http redirect, checked with ls -a home` → `-a ‹redacted›`;
  - a comment `\` continues the command;
  - `--no-pass` stays;
  - the N2 note keeps lines two and three.

### Hook timings (A/B/C)

Method: bench builds of `seldon` from base `1b23cf4`, round 1
`847b808` and head (`redact.rs` swapped), placed in
`target/release/seldon`. The hooks test binary is head's and runs
directly. Four interleaved rounds under the lock, load 7.5 → 1.9.

| Line, ledger size | base | round 1 | head |
|---|---|---|---|
| new `-e`/`-am` line, 900 | 4.19 / 5.07¹ / 4.15 / 4.23 ms | 4.89 / 4.85 / 4.93 / 4.99 ms | 4.68 / 4.68 / 4.71 / 4.67 ms |
| new `-e`/`-am` line, 10 000 | 2.58 / 2.72 / 2.59 / 2.55 ms | 3.32 / 3.26 / 3.28 / 3.31 ms | 3.05 / 3.09 / 3.06 / 3.13 ms |
| curl line with a marker, 900 | 3.92 / 4.21 / 3.98 / 3.99 ms | 4.03 / 4.06 / 4.04 / 4.09 ms | 4.14 / 4.06 / 4.13 / 4.31 ms |
| curl line with a marker, 10 000 | 2.38 / 2.72 / 2.43 / 2.40 ms | 2.54 / 2.65 / 2.57 / 2.54 ms | 2.54 / 2.58 / 2.57 / 2.61 ms |

¹ In round 2 (load 4.4), base itself went over the budget on the new
line (5.07, then 5.34 ms on the retry).

- **Head against round 1, new line: −0.2 ms.** This is the narrower
  HTTPie trigger.
- **Head against base, new line: about +0.5 ms.** This is `cert-password`
  (SPEC 6(d), accepted) plus the larger curl-user context.
- **Headroom on the new line at 900 lines: about 0.3 ms.** That is
  under the 5 ms bound, but tight; base itself shows 4.2–5.1 ms there.

### Mutants (round 2)

Same runner and restore as round 1. 23 mutants: N1–N19 new, R8, R9, C1
and C9 re-run because the lead and the context changed.

| # | Mutant | Killed by |
|---|---|---|
| N1 / N2 / N3 / N4 | WORD's `'…'` / `"…"` / `$'…'` / fallback cross lines | `every_builtin_pattern` (N2–N4 after `0a7828b`; N3 also `masking_twice…`) |
| N5 | no tail after the parts | `every_builtin_pattern` |
| N6 | no lone unclosed quote | `every_builtin_pattern` |
| N7 | tails cross lines | `every_builtin_pattern`, `masking_twice…`, `email_…`, `cert_and_httpie_…` |
| N8 (O1) | WORD `"…"` without `\⏎` | `every_builtin_pattern` |
| N9 (O2) | context `"…"` without `\⏎` | `every_builtin_pattern` |
| N10 | `secret-option` without `NOT_NO` | `harmless_text_stays` |
| N11 | `NOT_NO` admits `no` | `harmless_text_stays` |
| N12 | `NOT_NO` without `no…` words | `every_builtin_pattern` (after `72ed1de`) |
| N13 | `NOT_NO` without a–m/o–z words | `every_builtin_pattern`, `cert_and_httpie_…` |
| N14 | `HTTPIE_GAP` = `\s` | `every_builtin_pattern` (after `72ed1de`, NBSP row) |
| N15 | `HTTPIE_GAP` without `\⏎` | `an_option_…_two_lines` |
| N16 / N17 / N18 | one trigger literal dropped (`xh\`, `https `, `xhs\t`) | `an_option_…_two_lines` (N17 also 3 more) |
| N19 | bare `VALUE` may start at `‹` | `masking_twice_changes_nothing` |
| R8 | lead without its gap | `harmless_text_stays` (after `72ed1de`; the old `wget http://…` row no longer holds a trigger) |
| R9 | lead greedy | `every_builtin_pattern` |
| C1 / C9 | as round 1 | as round 1 |

Result: 23 of 23 killed. Six survived first (N2, N3, N4, N12, N14,
R8); each was killed by the row named in the table.

### Guard-hook block (reported, not routed around)

One Bash call was blocked as a "privileged or package command": a
`printf` that wrote probe text (holding `sudo -E bash` and a
`useradd` line) to a scratchpad file. The command itself did nothing
privileged. As `memory/pitfalls.md` prescribes for file content, I wrote
the same probe text with the Write tool and re-ran only the commit that
had been part of the blocked call. No command was reworded to get past
the guard.

### Decisions needed

- The trigger-negative assertion deviates from the brief (item 1 above).
  Please confirm.
- Headroom of the new hook line at 900 lines is about 0.3 ms (base: 4.2
  to 5.1 ms). Accepted per SPEC 6(d), or narrow `cert-password`'s
  `curl+-e`?

### Touched outside WP scope

- `engine/tests/hooks.rs` (item 5) and guide 06 en/de (item 7), both as
  the brief asks.
- `memory/pitfalls.md` (append).

## Round 3 (verification: SEND BACK small)

Commits: `d9242d0` (B1, rows), `6c844c4` (N2, SPEC), then this section.

- **B1.** Both unclosed-quote tails of `WORD` are now `['"][^\n]*`
  (was `[^\n'"]*`), so an unclosed quote takes the rest of its line,
  the other quote character included. Only `WORD` changed;
  `COMMAND_REST` is untouched.
  - TABLE rows: the reviewer's Q1 (`curl -u 'admin:fa"keQ1 rest⏎next`),
    Q2 (`curl -u "admin:it's fakeQ2⏎next`) and Q3
    (`curl -u admin:'fake"Q3`). Each is masked to the line end, and the
    next line stays.
  - The Q3 text is in `masking_twice_changes_nothing`.
- **N2.** SPEC §7, 6(b) now reads "on a line that holds, or follows a
  line ending in, `http`, `https`, `xh` or `xhs` as a word followed by
  white space (…; the gap after the word may be a line end)".
- **N1** is accepted by stage 2. No change.

### Verified

- `cargo fmt --check` and `cargo clippy --all-targets --locked -- -D
  warnings`: clean. `redaction` 24 passed (1 ignored), `hooks` 55 passed
  (2 ignored). `docs-check`: ok.
- **`flock /tmp/seldon-check.lock just check` at `6c844c4`: exit 0**
  (23:17–23:26). Results:
  - 70 test binaries ok;
  - service-states 314/0, panel-view 782/0, overlay-view 319/0,
    bar-view 143/0;
  - install 209/0, deploy-test-host 190/0, real-home-guard 11/0;
  - qmllint 29 files, docs-check ok, `check: ok`.

### Mutants (round 3)

Same runner, run on the committed `d9242d0`, baseline green afterwards.

| # | Mutant | Killed by |
|---|---|---|
| T1 | both tails back to `[^\n'"]*` (round-2 formula) | `every_builtin_pattern`, `masking_twice…`, `email_…`, `cert_and_httpie_…` |
| T2 | only the tail after the parts | same four |
| T3 | only the lone tail | same four |
| N7 | tails cross a line end | `every_builtin_pattern` |

**Process note.** The first run of these mutants started before the
fix was committed. The runner restores with `git checkout HEAD`, so it
reverted the uncommitted fix. Its final source check caught this. I
re-applied the fix, committed it, and re-ran all four mutants on the
committed state; the table shows that run. Nothing else was affected.

No guard-hook blocks this round. Touched outside WP scope: none.
