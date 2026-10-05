```
WP-084 HANDOVER
```

Branch `wp/084-review`. Commits: `04c4d88` (rules and tests), `d11c66d`
(disjointness against older rows), `18f13db` (JSON row for the key
anchor), `d5ddb85` (SPEC-ENGINE §7, user guide en), `8eea200`
(CHANGELOG), `c102b2b` (user guide de, re-stamped at `d5ddb85`),
`da0bffb` (pitfalls), plus this file.

## Done

Five built-in rules in three groups (`redact::BUILTIN` is now 23), all
with `KEEP_PREFIX`: the option, key or header name stays, the credential
becomes `‹redacted›`.

1. **Proxy credentials**
   - `proxy-option`: the value after `curl -U` / `-Uuser:pw` (curl
     context as for `curl-user`, so `useradd -U` stays), after
     `--proxy-user[= ]` (curl, wget) and after wget's
     `--proxy-password <space>`. Its `=` form `--proxy-password=` is
     already `secret-assignment` and stays there (disjoint).
   - `proxy-userinfo`: `user:pass` without a scheme before the last `@`
     after `curl -x`, `--proxy[= ]`, `…proxy=` (`HTTPS_PROXY=`,
     `http.proxy=`), optionally quoted. `--proxy http://user:pass@host`
     was already covered by `url-userinfo`; the new rule refuses a value
     with `//` after the `:` so the two stay disjoint (row in the table).
2. **Inline JSON** — `json-secret`: the non-empty string value of a key
   that *ends* in `password|passwd|passphrase|secret|token`
   (`"password"`, `"client_secret"`, `"access_token"`, `"Token"`),
   plain JSON (with `\"` escapes inside the value) or shell-escaped
   (`\"password\":\"…\"`). Not `"password_hint"`, `"token_type"`,
   `"secrets"`; not an empty `""` / `\"\"`.
3. **Cookies**
   - `cookie-header`: `Cookie:` / `Set-Cookie:` value up to a closing
     quote or the line end; the value must start with a non-space
     character, so `'Cookie: '` and `'Cookie:'` stay. `[ \t]*` instead of
     `\s*`, so a value never continues onto the next line.
   - `cookie-option`: the value after `curl -b` / `-bX` / `--cookie[= ]`
     when it holds a `=` (without one, curl reads cookies from that file,
     so `-b cookies.txt` stays); `--cookie-jar` stays.

Tests (`engine/tests/redaction.rs`): 22 new must-mask rows (4
proxy-option, 4 proxy-userinfo, 1 url-userinfo `--proxy http://…`, 1
secret-assignment `--proxy-password=`, 6 json-secret, 3 cookie-header,
3 cookie-option), 13 new must-stay-clear rows (`useradd -U -m bob`,
`sudo useradd -U bob && curl …`, `curl -x proxy:3128`, `curl -x
me@proxy:3128`, `http.proxy=http://proxy:3128`, `"password_hint"`,
`"token_type"`/`"secrets"`, empty `"password": ""` plain and escaped,
`Cookie: ` and `Cookie:`, `-b cookies.txt -c cookies.txt`,
`--cookie-jar`). New test
`proxy_json_and_cookie_rules_are_disjoint_and_stable`: every row of a
new rule matches exactly that rule, no row of an older rule matches a
new one, redacting twice changes nothing, plus one combined line with
exact output. Idempotency is also covered by the existing
`masking_twice_changes_nothing` (all rows, single and joined).

Docs: SPEC-ENGINE §7 rule list (in run order), the placeholder style,
the disjointness note; user guide en/de; CHANGELOG `[Unreleased]`.

## Not done

- No `"authorization"` / `"cookie"` JSON keys and no JSON number values
  (`"password": 1234`); not asked for, easy to add.
- The WP-062 residuals stay out of scope and are not covered by the new
  rules: short `--api-key=abc`, bare `pass=x`, `PWD=`.
- No `just check-perf` run (see Open questions).

## How verified

- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` clean;
  `cargo test --locked`: all 31 test binaries green; `redaction` 19
  tests; `scripts/docs-check.sh -q` exit 0.
- `just check` (after waiting for an idle plugin harness, at `da0bffb`):
  run 1 exit 1, one harness case only: `service-states: snapper-degraded:
  fix commands were:` (both fix actions recorded, but the copy and
  terminal records interleaved: `wl-copy`, terminal, `--`, fix, fix, …)
  — an ordering race between two async processes; this WP touches nothing
  in `plugin/` or `tests/plugin/`. Treated as transient and re-run once:
  **run 2 exit 0** (`check: ok`; service-states 275/0, panel-view 743/0,
  overlay-view 319/0, bar-view 143/0, install 209/0, real-home-guard
  11/0, docs-check ok).
- Mutants (scratch script, `engine/src/redact.rs` restored and
  `git diff --exit-code` clean afterwards), each run of `--test redaction`:

  | Mutant | Caught by |
  |---|---|
  | `proxy-option` removed | `every_builtin_pattern`, `…_disjoint_and_stable` |
  | `proxy-userinfo` removed | same |
  | `json-secret` removed | same |
  | `cookie-header` removed | same |
  | `cookie-option` removed | same |
  | `-U` without the curl context | `harmless_text_stays` (`useradd -U`) |
  | `-U` case-insensitive (overlaps `curl -u`) | `…_disjoint_and_stable` |
  | `--proxy-password` also with `=` (overlaps `secret-assignment`) | `…_disjoint_and_stable` |
  | `proxy-userinfo` accepts `scheme://` (overlaps `url-userinfo`) | `every_builtin_pattern`, `…_disjoint_and_stable` |
  | JSON key not anchored at its end | `every_builtin_pattern` (after `18f13db`; see below) |
  | `json-secret` with any value (no empty check) | `harmless_text_stays` |
  | `has_json_value` without the `\"…\"` form | `harmless_text_stays` |
  | no escaped JSON alternative | `every_builtin_pattern`, `…_disjoint_and_stable` |
  | escaped trigger `token\"` removed | `every_row_holds_a_trigger_of_its_rule` + 2 |
  | `Cookie:` empty value allowed | `harmless_text_stays` |
  | `cookie-option` without the `=` check | `harmless_text_stays` |
  | `cookie-option` also `--cookie-jar` | `every_builtin_pattern`, `…_disjoint_and_stable` |

  Two survived in the first round. *JSON key not anchored* survived
  because the trigger `password"` kept the rule from running on
  `"password_hint"` alone; the row `{"password_hint": …, "password": …}`
  (`18f13db`) catches it now (pitfall recorded). *`-x`
  case-insensitive* still survives and is treated as equivalent: curl's
  `-X` takes an HTTP method, which never has the form `user:pass@`; the
  meaningless clear row `curl -X POST …` was dropped.
- Cost (release, first redaction in a process, i.e. regex compile): a
  `curl -u … https://…` line 0.52 ms before, 1.05 ms after (curl lines
  now also compile `proxy-option`, `proxy-userinfo`, `cookie-option`,
  ~100–180 µs each); a line with a JSON key and a `Cookie:` header
  +0.37 ms; a `pacman -S` line unchanged (no trigger). Measured with a
  throwaway test, not committed.

## Open questions

1. **Hook budget for curl lines.** A recorded `curl` command now pays
   ~0.5 ms more once per hook process. The `check-perf` hook benches use
   the pacman fixture, which triggers none of the new rules, so they
   would not see it. WP-062 measured the worst case (below the rebuild
   threshold) at 3.7–4.4 ms; +0.5 ms stays under 5 ms but closer. If that
   matters: AND-triggers (`curl` *and* `-u`) would cut it; that changes
   `redact::triggers`' shape and is a follow-up, not done here.
2. `cookie-header` is unanchored like `authorization-header`, so prose
   such as `log -- "cookie: banner fixed"` becomes `cookie: ‹redacted›`.
   Over-redaction by design (SPEC §7); say if notes should be spared.

## Touched outside scope

`memory/pitfalls.md` (append), `CHANGELOG.md` (append),
`docs/user/{en,de}/06-configuration.md` (the user-facing rule list, as
WP-062 did). Nothing outside the repository; tests use scratch dirs only.

## Round 2 (review: APPROVE with A, B, C, D and the perf gate)

Commits: `f227f27` (A), `dacf792` (B, C, D), `93087d1` + `d44b2a3`
(hook perf case), `bb2b4e7` (joined curl triggers, see below),
`ca0c0ae` (timing lines), `767865b` (SPEC-ENGINE §7, user guide en),
`2437a3f` (CHANGELOG), `a6b068d` (user guide de, re-stamped at
`767865b`), plus this section.

### What changed

- **A — ASCII word boundaries.** Every `\b` in the rule table is
  `(?-u:\b)` now (url-userinfo, secret-header, cookie-header, sk-key,
  db-client-password, the four curl rules, sshpass, registry login).
  `secret-assignment`, `key-assignment` and the `…proxy=` part of
  `proxy-userinfo` drop the boundary instead: a match starts at the
  first character of the name anyway (leftmost match over `[a-z0-9_]*`),
  and an ASCII boundary would no longer see one before `ſ`/`K` at the
  start of a name. Two rows pin that (`ſECRET=pw`, `KEY=fakeKey57`
  with the Kelvin sign). Module docs and SPEC §7 say why.
- **D — cookie pairs.** `cookie-header` masks a value only when it starts
  with `name=` (`[^'"\s=;]+=`), so `cookie: banner fixed`,
  `make cookie: all` and `curl -H "Cookie: $COOKIE"` stay (clear rows).
- **B — same line.** Clear row `curl -H 'Cookie:\nsid=fakeCookie7' …`.
- **C — JSON.** `\s*` on both sides of the `:` (newline included); keys
  ending in `api_key` / `apiKey` (`api_?key` under the rule's `(?i)`, so
  `API_KEY` and `ApiKey` too). Rows: `{"password"\n:\n  "…"}`,
  `{"api_key": …}`, `{"openaiApiKey" :…}`, and
  `{"apiKeyHint": "x", "monkey": "y", "apiKey": …}` (the hint stays; a
  clear row alone would be hidden by the trigger, see the round-1
  pitfall). Triggers `api_key"`, `apikey"` and their escaped forms.
- **Perf case in `just check-perf`.** `hooks::hook_budget` measures a
  third case: a recorded `curl -fsSL https://bob:…@h.example/… -o … &&
  sudo pacman -U …` line (the URL rule leaves the marker first), 21
  calls, budget 5 ms, and asserts that the recorded command holds the
  marker and not the password. The 900-line test leaves room for
  2 × 2 × 22 commands (was 2 × 22).
- **Joined curl triggers (not asked for; separate commit `bb2b4e7`,
  drop it if unwanted).** A trigger may join literals with `+`
  (`redact::holds_trigger`): `curl-user` and `proxy-option` need
  `curl+-u` (`-U` and `--user` read `-u`), `proxy-userinfo` `curl+-x`,
  `cookie-option` `curl+-b` or `curl+--cookie`; `--proxy-` and `proxy`
  stay single. Why: with the plain `curl` trigger every curl line
  compiled all four curl rules; measured before this commit, the curl
  case at 10 000 lines took 3.30 ms (pacman line 1.93 ms; on `main`
  2.45 ms), and at 900 lines 4.89 ms (`main` 4.32 ms) under load 6–8.
  Test: `every_row_holds_a_trigger_of_its_rule` asserts that
  `curl -fsSL https://h.example/f -o /tmp/f` triggers none of the four.

### Timings

`long_lines_with_a_marker_stay_fast` (ignored; `cargo test --profile
bench --test redaction -- --ignored`), median of 21, warm rules. The
lines start with `curl` and the URL (url line) or plain German text
(note) and are filled with `a-u-x-b` / `Schlüssel-u-x-b geändert`, so
every curl rule is triggered, finds no option and scans the whole line
(the worst case; with an early match the literal prefilter skips the
rest and hides the slow path). Budgets 1 ms (16 KB) and 2 ms (64 KB).

| Line | `main` 616be66 | round 1 4d02fc3 | round 2 |
|---|---|---|---|
| url line, 16 KB | 1.19 ms | 5.36 ms | 0.13 ms |
| url line, 64 KB | 4.66 ms | 21.4 ms | 0.55 ms |
| German note, 16 KB | 0.99 ms | 4.52 ms | 0.13 ms |
| German note, 64 KB | 4.19 ms | 17.6 ms | 0.54 ms |

(`main` and round 1 measured with a throwaway copy of the test against
their `redact.rs`, not committed.)

Hook budget (`just check-perf`, once, after the plugin harness was
idle, at `a6b068d`, 10:07, load average 4.5): **exit 0**.

| Case | 10 000 lines | 900 lines (rebuild) |
|---|---|---|
| hook, not recorded | 1.36 ms | 1.35 ms |
| hook, recorded (pacman fixture) | 1.86 ms | 3.72 ms |
| hook, recorded curl line with a marker | 2.84 ms | 4.44 ms |

Index build ×10 4.37 ms, ×150 78.2 ms; `status` at 10 011 lines 46.0 ms.
The curl line still pays about 1 ms over the pacman line: its
`pacman -U` reads `-u`, so `curl-user` and `proxy-option` compile with
`url-userinfo` (three regexes, ~0.1–0.2 ms each, plus the second URL
work). For comparison, before the joined triggers and under load 6–8:
3.30 / 4.89 ms, and with `main`'s `redact.rs` 2.45 / 4.32 ms. The
4.44 ms leaves 0.56 ms of headroom at the rebuild threshold.

### Mutants (round 2)

| Mutant | Caught by |
|---|---|
| both assignment rules with `(?-u:\b)` instead of none; `key-assignment` alone too | `every_builtin_pattern` (`ſECRET=`, Kelvin `KEY=` rows) |
| `cookie-header` without the cookie pair (round-1 form) | `harmless_text_stays` |
| `cookie-header` `\s*` after the colon | `harmless_text_stays` (newline row) |
| `json-secret` `[ \t]*` after the colon | `every_builtin_pattern`, `…_disjoint_and_stable` |
| `json-secret` `[ \t]*` before the colon | `every_builtin_pattern`, `…_disjoint_and_stable` |
| `json-secret` without `api_?key` | `every_builtin_pattern`, `…_disjoint_and_stable` |
| `json-secret` `api_key` only (no `apiKey`) | `every_builtin_pattern`, `…_disjoint_and_stable` |
| `json-secret` key not anchored (with `api_?key`) | `every_builtin_pattern` |
| curl rules back on the plain `curl` trigger | `every_row_holds_a_trigger_of_its_rule` |
| `cookie-option` without the `curl+--cookie` trigger | `every_row_holds_a_trigger_of_its_rule` + 2 |
| `holds_trigger` without the `+` split | `every_row_holds_a_trigger_of_its_rule` + 3 |
| `cookie-option` back on a Unicode `\b` | `long_lines_with_a_marker_stay_fast` (bench profile) |

The last one survived with the first timing lines (an early match let
the prefilter skip the slow path; then the joined triggers kept
`cookie-option` from running at all); the worst-case filler fixed both.
Source restored after each run (`git diff --exit-code engine/src`).

### Verified (round 2)

`cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` clean;
suites `redaction` 19 (+1 ignored), `hooks` 54 (+2 ignored),
`idempotency` 20; the full `cargo test` green; `scripts/docs-check.sh`
exit 0. No second `just check` (the orchestrator's gate runs it).

### Open (round 2)

- Noted by the reviewer for a follow-up WP, not done here: the curl
  context stops at `&`/`;` inside quotes (a query string before the
  option, also `curl-user`), and repeated options.
- SPEC §7 still spells the sk rule `\bsk[-_]…` in the rule list; the
  new sentence says all boundaries are ASCII.
