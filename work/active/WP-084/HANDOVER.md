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
