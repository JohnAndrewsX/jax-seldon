```
WP-106 HANDOVER
```

Branch `wp/106-openssl-pass`, base `71c7d9a`. Commits: `307777a` (rule
and rows), `c31ae40` (unclosed `$'`/`$"` fallback, more rows),
`feacead` (SPEC-ENGINE §7), `3ec8616` (CHANGELOG), `51c7ea7`
(pitfalls), plus this file.

## Done

**New rule `openssl-pass`** in `engine/src/redact.rs`, after
`secret-option` in `BUILTIN` (now 27 entries).

- **Pattern:** `(?i)(-[a-z0-9_-]*(?:pass|secret)[a-z0-9_-]*(?:=|GAP+))`
  then `PASS_ARG`. Replacement `KEEP_PREFIX`: the option stays, the
  whole value is masked, `pass:` included
  (`-passin pass:x` → `-passin ‹redacted›`).
- **Trigger:** `pass:`, one unique literal. Every match holds it,
  because `PASS_ARG` requires it (see below). The marker holds no part
  of it. No other rule shares it.
- **`PASS_ARG`** is a `WORD` whose first part must start with `pass:`:
  - bare, or inside `'…'`, `"…"` (with `\"`), `$'…'`, or the
    unclosed-as-escapes `"…"` fallback;
  - then any further `WORD` parts (joined quoted and bare parts, `\x`,
    `\⏎`), then `WORD`'s unclosed-quote tail;
  - or a lone unclosed `'`, `"`, `$'` or `$"` before `pass:`, which
    takes the rest of its line only.
  - **Why in the regex and not a check on `WORD`.** With a check, the
    flag in `-twopass -passin pass:x` took `-passin` as its value,
    failed the check, and the search resumed after it: `pass:x` leaked.
    With the shape in the regex, a failed value lets the search restart
    at the next option. A TABLE row pins this (`-twopass …`, and
    `-passin -passout pass:…`).
- **Stays clear:** the sources `env:VAR`, `file:path`, `fd:N` and
  `stdin`; a value without `pass:` (`-passin passfile`); `pass:` after an
  option whose name holds neither `pass` nor `secret`
  (`git commit -m pass:fixed`); `pass:` after no option.

**Wider than the WP list (please confirm, see Decisions).** The WP
names `-pass`, `-passin` and `-passout`. `openssl <cmd> -help` on the
dev host (OpenSSL 3.6.4) lists more pass phrase source options of the
same form: `-password` and `-passcerts` (pkcs12); `-keypass`,
`-newkeypass`, `-otherpass`, `-tls_keypass`, `-srv_keypass`,
`-rsp_keypass`, `-secret` and `-srv_secret` (cmp); `-dpass` (s_server);
`-proxy_pass` (s_client); `-pkeyopt_passin` (pkeyutl).
- The rule therefore takes any option whose name holds `pass` or
  `secret`.
- openssl also accepts `--opt` and `-opt=value` (probe:
  `genpkey -pass=pass:…` and `--pass=pass:…` both work). easyrsa writes
  `--passin=pass:…`, so `=` and two dashes are covered too.
- The `(?i)` follows the module rule "err towards redacting too much".
  openssl itself rejects `PASS:`.

**Tests** (`engine/tests/redaction.rs`):
- **TABLE: 22 rows**, covering:
  - each first-part form;
  - joined parts, `\;` and `\⏎` inside the value;
  - each unclosed form, with a quote on the next line that must stay;
  - `=` with two dashes (easyrsa);
  - upper case;
  - the cmp and s_client names;
  - `|` after the value;
  - a flag before the option.
- **CLEAR: 5 rows**: sources (`env:`, `file:`, `fd:`, `stdin`,
  `"file:$HOME/…"`), `-passin passfile`, `git commit -m pass:fixed`,
  and `the pass: column stays`.
- **`an_option_and_its_value_may_stand_on_two_lines`:** two rows,
  `-passin \⏎  pass:…` and `-passin\⏎  pass:…`. The second has no white
  space before the `\` (pitfall WP-097).
- **New test `openssl_pass_is_disjoint_and_stable`:**
  - the rule matches only its own rows, and no other row matches it;
  - every row is stable on a second pass;
  - the overlap with `secret-option` (`--pass pass:…`) and
    `password-option` (`--password pass:…`) is pinned: both rules
    match, and the output holds one marker.
- **Timing (`long_lines_with_a_marker_stay_fast`):**
  - "openssl sources": options with sources, and `pass:` after `-k`;
    budgets 1 ms at 16 KB and 2 ms at 64 KB;
  - "openssl options": many matches, quoted and bare, with `-twopass`;
    budget 20 ms at 128 KB.

**Docs:**
- SPEC-ENGINE §7:
  - the rule in the list, with its trigger;
  - removed from the WP-097 "not masked" list;
  - a "Not masked (WP-106)" sentence;
  - over-masking and the import-report overlap under "Masked too much".
- Module doc of `redact.rs`.
- CHANGELOG `[Unreleased] / Engine`, in neutral wording.

## Not done (documented as limits in SPEC §7)

- **A password given directly** rather than as `pass:…` is not masked:
  `openssl enc -k pw`, `-srppass`, keytool's `-storepass`.
- **`pass:` split by quotes or an escape** is not masked: `pa'ss':x`,
  `\pass:x`. Neither is an option name in quotes. The trigger must be
  literal text, and the shell would join these forms.
- **Over-masking, accepted and documented:**
  - `pass:…` after any option whose name holds `pass` or `secret`
    (`--bypass pass:x`);
  - the empty `pass:`;
  - the rest of the line after a closed `$"pass:…"`, which reads as an
    unclosed quote.
- Guide 06 is unchanged; the WP lists only SPEC and CHANGELOG. See
  Decisions.

## Verified by

- `cargo fmt --check` and
  `cargo clippy --all-targets --locked -- -D warnings`: clean.
- `redaction`: 25 passed, 1 ignored (timing).
- **`flock /tmp/seldon-check.lock just check` at `51c7ea7`: exit 0**
  (01:11–01:20, load 13 → 19 from other worktrees). Results:
  - 70 test binaries ok;
  - service-states 314/0, panel-view 782/0, overlay-view 319/0,
    bar-view 143/0;
  - install 209/0, deploy-test-host 190/0, real-home-guard 11/0;
  - qmllint 29 files, docs-check ok, `check: ok`.
- **Timing rows** (the `check-perf` test line for `redaction` and
  `hooks`, bench profile, under the lock, load about 7):
  - exit 0, every budget passed on the first attempt;
  - `index` bench ×150 not run (redaction does not touch it).

| Row | 16 / 64 / 128 KB | budget |
|---|---|---|
| openssl sources | 0.081 / 0.341 ms | 1 / 2 ms |
| openssl options | 0.447 / 1.78 / 3.57 ms | 20 ms at 128 KB |

  - Both rows grow ×4.2 from 16 to 64 KB and ×2.0 from 64 to 128 KB:
    linear.
  - The WP-087/093/097 rows stay within their budgets. For example: url
    line at 64 KB 0.67 ms (budget 2 ms); two option kinds at 128 KB
    9.5 ms (20 ms); continued options at 128 KB 10.2 ms (20 ms).
  - **Hook:** no hook line holds `pass:`, so `openssl-pass` is never
    compiled there; the trigger keeps the cost off every line without
    `pass:`. All hook budgets passed on attempt 1.
  - **The `-e`/`-am` line at 900 lines took 4.95 ms** (budget 5 ms).
    That is WP-097's known tight headroom (base there measured 4.2 to
    5.1 ms) at load 7. It is not this change, which that line does not
    trigger. No A/B was run for it.
- **Probes** (a throwaway binary in the scratchpad, not committed): 29
  lines, all masked or clear as SPEC says, and all stable on a second
  pass. They also cover `url-userinfo` and `email` inside a `pass:`
  value; both are then masked whole.

### Mutants

Each mutant ran `cargo test --test redaction --no-fail-fast` against the
committed `c31ae40` source. The source was restored with
`git checkout HEAD` + `touch`. Afterwards the tree was clean and the
baseline green. `PASS_ARG` mutants were applied to that line only,
because `WORD` holds the same parts. The runner is in the session
scratchpad (`mut/run.py`) and is not committed.

Test names: `every_builtin…` = `every_builtin_pattern`,
`two_lines` = `an_option_and_its_value_may_stand_on_two_lines`,
`disjoint` = `openssl_pass_is_disjoint_and_stable`.

| # | Mutant | Killed by |
|---|---|---|
| F1 | first part without escaped `"pass:…"` | `every_builtin…` |
| F2 | without `'pass:…'` | `every_builtin…` |
| F3 | without `$'pass:…'` | `every_builtin…` |
| F4 | without bare `pass:` | `every_builtin…`, `two_lines`, `disjoint` |
| F5 | without the `"pass:…"` fallback | `every_builtin…` |
| F6–F9 | each first-part quote crosses a line end | `every_builtin…` |
| P1–P6 | parts without escaped `"…"` / `'…'` / `$'…'` / `\x` / bare char / `"…"` fallback | `every_builtin…` (P5 also `two_lines`) |
| P7 | no parts after the first | `every_builtin…`, `two_lines` |
| P8–P11 | each part quote crosses a line end | `every_builtin…` |
| T1 | no tail after the parts | `every_builtin…` |
| T2 | tail stops at a quote (`[^\n'"]*`) | `every_builtin…` |
| T3 | tail crosses lines | `every_builtin…` |
| L1 | no lone unclosed quote | `every_builtin…`, `disjoint` |
| L2 | lone quote without `\$?` | `every_builtin…`, `disjoint` |
| L3 / L4 | lone quote stops at a quote / crosses lines | `every_builtin…` |
| R1 / R2 | name without `secret` / without `pass` | `every_builtin…` (R2 also `two_lines`, `disjoint`) |
| R3 / R4 | nothing before / after `pass\|secret` in the name | `every_builtin…`, `disjoint` (R4 also `two_lines`) |
| R5 | any option name | `harmless_text_stays` |
| R6 | no `=` form | `every_builtin…`, `disjoint` |
| R7 | gap `\s` only | `two_lines` |
| R8 | one gap character | `two_lines` |
| R9 | case-sensitive | `every_builtin…`, `disjoint` |
| R10 | replacement `WHOLE` (option masked too) | `every_builtin…` |
| R11 | trigger `-pass+pass:` (unsound for `-keypass`, `-secret`) | `every_row_holds_a_trigger…`, `every_builtin…`, `disjoint` |
| R12 | no trigger | `every_row_holds_a_trigger…` |
| R13 | rule order before `secret-option` | `disjoint` |

**Result: 40 mutants, 40 killed in the first run.** The first-part
forms `$'`/`$"` unclosed, the glued `"…\"` part and the crossing rows
were added in `c31ae40`, before the run, while I planned the mutants.
That is also when the unclosed-`$'` leak was found and fixed.

Not mutated: a broader trigger (`pass`). It is perf-only and changes no
output.

## Learned

Appended to `memory/pitfalls.md`:
- a value check after the match is too late when the option name is
  broad: put the value's shape into the regex;
- a forced first part needs its own unclosed-quote fallback, `$`
  included.

## Decisions needed

None blocking. Three for the orchestrator:

1. **Scope.** The rule covers every option whose name holds `pass` or
   `secret`, not only `-pass`/`-passin`/`-passout`. This includes
   openssl's other pass phrase sources and easyrsa's `--passin=`.
   Confirm, or narrow it to `-pass(?:in|out)?` (then `-password`,
   `-passcerts`, `-keypass` and `-secret` stay unmasked).
2. **Import report overlap.** `--pass pass:…` and `--password pass:…`
   are matched by `secret-option`/`password-option` and by
   `openssl-pass`. The output has one marker, but the import report
   counts the line under both rules. This is documented in SPEC and
   pinned by a test. Accept, or exclude those names from `openssl-pass`?
3. **Guide 06 en/de:** add one bullet ("the `pass:…` value of openssl's
   `-pass`/`-passin`/`-passout`")? It is not in this WP's outputs.

## Touched outside WP scope

- `memory/pitfalls.md` (append).
- No guard-hook blocks. Nothing outside the repository and the session
  scratchpad. The made-up `openssl genpkey` probes on the dev host wrote
  to stdout only.
