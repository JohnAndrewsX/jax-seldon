```
WP-108 HANDOVER
```

Branch `wp/108-hook-headroom`, base `e6c6f16`. Commits:
- `ef5aab8`: a trigger with a capital is checked as written;
  `cert-password` needs `curl+-E` (the WP's lever (a) as worded);
- `ad6796a`: `cert-password` triggers in order, `curl>-E>:`;
- `9ce6be3`: `url-userinfo` needs an `@` after `://`;
- `36ea003`: `proxy-option` needs `curl+-U` as written;
- `0c3539b`: an option rule compiles its scan-on only when the rest
  holds the option;
- `0e6f05d`: one more trigger row;
- `873afcd`, `dd3d37e`: SPEC-ENGINE §7 and §8, CHANGELOG;
- `e7d1cd3`: module doc, hook test comment;
- `4e3403f`: pitfalls;
- plus this file.

## Result in one line

The row (`set -e; curl -fsSL -u … | sudo -E bash && git commit -am
zed`, 900 lines) went from **4.76 ms to 3.87 ms** median on a quiet
host (load 0.7–0.9, 8 interleaved rounds), about **−0.9 to −1.0 ms** in
every set. The 5 ms bound is unchanged. Lever (b) was not needed and
not coded.

**The WP's lever (a), as worded, does not reach the target. Please
confirm the wider version (see Decisions).** The row also holds `sudo
-E`, so a trigger on `-E` as written (with or without the space) still
compiles `cert-password` on that line. Even with that rule gone, the
row stays above 4 ms: on the hook line, `cert-password` was the
smallest of the four rules that compiled (profile below). I therefore
applied the same idea, "a trigger is literal text that every match
contains", to the other three. No change to what is masked.

## Done

### Profile first (base `e6c6f16`, before any change)

Throwaway probe, never committed: an `eprintln!` of each rule's
compile and search time behind an env var, in a bench build. Each line
went through `seldon event manual note --subject …` in a scratch logbook
(HOME and XDG in the scratchpad). Median of 21 cold processes, under
the lock, load about 8.

| Rule on the row's line | compile | compile + search |
|---|---|---|
| `url-userinfo` (`://`, no `@` in the line) | 245 µs | 266 µs |
| `curl-user` (matches) | 238 µs + 243 µs for its scan-on (`next`) | 565 µs |
| `proxy-option` (`curl+-u` fires on `-u`; needs `-U`) | 304 µs | 333 µs |
| `cert-password` (`curl+-e` fires on `set -e`, `sudo -E`) | 237 µs | 265 µs |
| **redaction total** | | **1470 µs** |

None of these except `curl-user` can match on that line. After the
change, the same probe on head compiles only `curl-user`, without its
scan-on: **476 µs against 1510 µs** in the same run. On the
curl-with-marker line it is 1198 µs against 1209 µs (no change; see
Not done).

### Changes (`engine/src/redact.rs`)

1. **`holds_trigger(text, lower, trigger)`.**
   - A trigger with an ASCII capital is checked on the text as written,
     any other one on the lower-case trigger text as before. This is
     for a rule whose literals are case-sensitive.
   - Besides `+` (all parts, anywhere), a trigger may join parts with
     `>`: all present, in this order. It takes the leftmost occurrence
     of each part, and the next part is looked for after the end of the
     previous one.
   - Soundness: the replacement `‹redacted›` holds no part, as before,
     and the text a rule leaves keeps the order of what it keeps. So a
     trigger checked on the input still covers the text after earlier
     rules. The `triggers` doc says so.
2. **`cert-password`**: `curl>-E>:`, `--cert>:`, `--proxy-cert>:` (was
   `curl+-e`, `--cert`, `--proxy-cert`).
   - The rule's `curl` and `-E` are case-sensitive (mutant R4 of WP-097
     pins `-E`). A match needs a value with `:` after the option, so
     every masked match holds `curl`, then `-E`, then `:`. The same
     holds for `--cert`/`--proxy-cert` and `:`.
   - A repeated `-E` that scans on is covered as well: the masked
     value's `:` comes after the later `-E`.
   - The glued `-Ec.pem:pw` still triggers.
3. **`url-userinfo`**: `://>@` (was `://`). The pattern requires `@`
   after `://`.
4. **`proxy-option`**: `curl+-U`, as written, and `--proxy-` (was
   `curl+-u`). The rule's `curl` and `-U` are case-sensitive; the long
   forms are `(?i)` and stay under `--proxy-`.
5. **Scan-on on demand.** An option rule's `next` pattern (the same
   option again in the same command) is now compiled only when the
   text after a match holds one of the rule's `again` literals.
   - `again` lists the literals, as written: `curl-user` `-u`;
     `proxy-option` `-U`, `--`; `proxy-userinfo` `-x`, `--`;
     `cookie-option` `-b`, `--cookie`; `cert-password` `-E`, `-cert`;
     `httpie-auth` `-a`; `registry-login-password` `-p`.
   - Each option part of those patterns is case-sensitive. A
     case-insensitive long form holds `--`.
   - Once compiled, `next` runs as before. The check only decides
     whether to compile it, so the output cannot change. A rest without
     a literal cannot match `next`.
   - A `debug_assert!` checks that every option rule has `again`
     literals and every other rule has none.

### Tests

- `every_row_holds_a_trigger_of_its_rule` (`tests/redaction.rs`), new
  negatives:
  - the row's line and `curl -e https://ref…` compile no
    `cert-password`;
  - `curl -u` and `curl --user` compile no `proxy-option`;
  - a URL without `@` after its scheme compiles no `url-userinfo`
    (`git@h:o/r` before the URL included);
  - the marker check now splits on `+` and `>` and also checks the
    marker as written.
- New `triggers_in_order_and_as_written`: 20 rows covering:
  - case;
  - order;
  - overlap (`cur>rl`);
  - the leftmost occurrence (`curl -E a:b curl`);
  - `curl+-U`;
  - `://>@`.
- New unit test `redact::tests::option_rules_scan_on_only_with_their_literals`.
  It uses fresh `builtin_rules()`, because the shared static keeps
  whatever an earlier test compiled.
  - For each option rule, a repeated option is masked twice: 10 rows,
    with the short and long forms that only `next` finds.
  - A curl line that gives `-u` once leaves `next` uncompiled.

### Docs

- **SPEC-ENGINE §7:**
  - the trigger paragraph covers `>` and capitals;
  - the scan-on sentence says when `next` is compiled;
  - the cost sentence: `cert-password` "compiles only on a line holding
    `curl`, then `-E` as written, then a `:` … (WP-108; before, any curl
    line holding `-e` did, about 0.3 ms per hook call)".
- **SPEC-ENGINE §8:** the 2026-10-06 number.
- Module doc of `redact.rs`, CHANGELOG `[Unreleased] / Engine` (neutral
  wording).

## A/B (interleaved, under `flock /tmp/seldon-check.lock`)

Method: bench builds of `seldon`, one per step, kept in
`engine/target/ab/` (on disk). Each was swapped into
`target/release/seldon`, then base's `hooks` test binary ran
`fast_enough_just_below_the_rebuild_threshold` directly. The order
rotates every round. The figures are first-attempt medians of 21 calls.
Afterwards `target/release/seldon` is head's bench build again (and
`check-perf` rebuilt it).

Builds: base `e6c6f16`; s1 `ef5aab8` (lever (a) as worded); s2
`ad6796a`; s3 `9ce6be3`; s4 `36ea003`; s5 `0c3539b`; head (the same
code as s5, plus comments).

**The row (`-e`/`-am` curl line, 900 lines):**

| Set | Time, load | Rounds | base | s1 | s2 | s3 | s4 | s5 / head |
|---|---|---|---|---|---|---|---|---|
| 1 | 09:08, 3.7–4.2 | 5 | 5.36 / 5.77 / 7.76 / 4.99 / 4.74 | 5.43 / 4.88 / 4.80 / 4.79 ¹ | 5.00 / 5.35 / 4.57 / 4.60 / 4.56 | | | |
| 2 | 09:10, 6.9–10.2 | 5 | 4.83 / 4.85 / 5.06 / 4.78 / 5.28 | | 4.55 / 4.82 / 4.48 / 4.57 / 4.58 | 4.40 / 4.45 / 4.42 / 4.47 / 5.00 | 4.08 / 4.13 / 4.09 / 4.14 / 4.10 | |
| 3 | 09:14, 4.4–6.0 | 6 | 4.85 / 4.85 / 4.73 / 4.84 / 5.04 / 4.78 | | | | 4.09 / 4.07 / 4.05 / 4.02 / 4.02 / 4.02 | s5: 3.89 / 3.85 / 3.85 / 3.94 / 3.88 / 3.91 |
| 4 | 09:25, 4.9–5.7 | 6 | 5.08 / 5.12 / 5.10 / 5.04 / 5.15 / 4.87 | | | | | head: 4.30 / 4.11 / 4.10 / 3.97 / 4.02 / 4.03 |
| 5 | 09:35, 0.7–0.9 | 8 | 4.77 / 4.70 / 4.86 / 4.75 / 4.76 / 4.74 / 4.84 / 4.96 | | | | | head: 3.84 / 3.94 / 3.88 / 3.77 / 3.86 / 3.93 / 4.19 / 3.81 |

¹ One s1 run in round 1 failed earlier, on the curl-with-marker row
(5.64 ms on attempt 2, load 4); rounds 1–2 of set 1 were noisy for
every build.

Medians per set:
- set 2: base 4.85, s2 4.57, s3 4.45, s4 4.10;
- set 3: base 4.84, s4 4.04, s5 3.88;
- set 4: base 5.09, head 4.07;
- **set 5 (quiet): base 4.76, head 3.87 ms.**

What each step gives (sets 2 and 3):
- s1 (lever (a) as worded): no gain. `cert-password` still compiles,
  because of `sudo -E`.
- s2 (`curl>-E>:`): −0.3 ms.
- s3 (`://>@`): −0.1 ms.
- s4 (`curl+-U`): −0.35 ms.
- s5 (scan-on on demand): −0.16 to −0.2 ms.

**The other rows, set 5 (quiet), base / head:**
- not recorded: 0.71 / 0.68 ms;
- recorded: 2.86 / 2.90 ms;
- curl line with a marker: 4.23 / 4.29 ms.

These are equal within noise. On the marker row head was +0.04 to
+0.06 ms in sets 4 and 5 and −0.01 in set 3, so no change shows beyond
noise. That line compiles `curl-user` and `proxy-option` because of
`pacman -U` (see Not done).

## Mutants

Runner: a throwaway script in the session scratchpad. It applied one
exact replacement to `src/redact.rs`, ran `cargo test --lib --test
redaction --no-fail-fast` (dev profile), then restored with `git
checkout HEAD` + `touch`. It ran on the committed `0e6f05d`. Before it,
the baseline was green: lib 221, redaction 26 passed. After it, the
source diff was empty.

| # | Mutant | Result |
|---|---|---|
| A1 | capitals ignored (always lower case) | killed: `triggers_in_order…`, `every_row_holds…`, `every_builtin_pattern`, `proxy_json…`, `cert_and_httpie…`, `two_lines`, the hook test |
| A2 | always as written | killed: `every_row_holds…`, `every_builtin_pattern`, `openssl_pass…`, `ledger_redacts…` and 7 more |
| A3 | `>` read as `+` (no order) | killed: `every_row_holds…`, `triggers_in_order…` |
| A4 | the next part may overlap the previous | killed: `triggers_in_order…` |
| A5 | ordered parts never found | killed (did not compile; equivalent to "never") |
| A6 | `rfind` instead of `find` | killed: `triggers_in_order…`, `every_row_holds…`, `every_builtin_pattern`, `proxy_json…` |
| T1 | cert trigger back to `curl+-e` | killed: `every_row_holds…` (negatives) |
| T2 | cert without the `:` (`curl>-E`) | killed: `every_row_holds…` |
| T3 / T4 | cert without `--cert>:` / `--proxy-cert>:` | killed: `every_row_holds…`, `every_builtin_pattern`, `cert_and_httpie…` (+ `two_lines` for T3) |
| T5 | cert `curl>-e>:` (lower case) | killed: `every_row_holds…` |
| T6 / T7 | `url-userinfo` back to `://` / reversed `@>://` | killed: `every_row_holds…` (T7 also `every_builtin_pattern`, `email…`, `ledger_redacts…`, …) |
| T8 / T9 | `proxy-option` back to `curl+-u` / without `--proxy-` | killed: `every_row_holds…` (T9 also `every_builtin_pattern`, `two_lines`, `proxy_json…`) |
| G1 | scan-on always compiled (no gate) | killed: `option_rules_scan_on…` |
| G2 | scan-on never compiled | killed: `option_rules_scan_on…`, `every_builtin_pattern`, `masking_twice…`, … |
| G3 | `any` literal → `all` literals | killed: same as G2 |
| L1 / L2 | `curl-user` again `-x` / `--user` | killed: `option_rules_scan_on…`, `every_builtin_pattern` |
| L3 | `proxy-option` again without `-U` | killed: `option_rules_scan_on…` |
| L4 | `proxy-option` again without `--` | **survived** (equivalent) |
| L5 | `proxy-userinfo` again without `-x` | killed: `option_rules_scan_on…`, `every_builtin_pattern`, … |
| L6 | `proxy-userinfo` again without `--` | **survived** (equivalent) |
| L7 / L8 | cookie again without `-b` / `--cookie` | killed: `option_rules_scan_on…` |
| L9 | cert again without `-E` | killed: `option_rules_scan_on…`, `every_builtin_pattern`, `cert_and_httpie…` |
| L10 | cert again without `-cert` | **survived** (equivalent) |
| L11 | httpie again `--auth` only | killed: `option_rules_scan_on…`, `every_builtin_pattern` |
| L12 | registry again `-u` | killed: `option_rules_scan_on…` |
| D1 | `option_rule` without `again` | killed: the `debug_assert!` (every test that redacts) |

**Result: 31 mutants, 28 killed, 3 survived.**
- The survivors are the long-form literals of the three rules that also
  have a plain form: `--proxy-user`/`--proxy-password`, `--proxy`/`…proxy=`,
  `--cert`/`--proxy-cert`.
- When `next` is skipped there, the rule's own pattern finds the
  same long option through its plain alternative. That alternative
  has the same text with no `\s` needed before it, and its group 1
  is kept the same way, so the output is identical.
- I kept the literals because they are obviously sound. Dropping them
  rests on that equivalence argument. Reviewer's call (Decisions).

## Checks

- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`:
  clean, both inside `just check`.
- **`flock /tmp/seldon-check.lock just check` at `dd3d37e`: exit 0**
  (09:37–09:45). Results:
  - 70 test binaries ok;
  - install 209/0, deploy-test-host 190/0, real-home-guard 11/0;
  - service-states 314/0, panel-view 782/0, overlay-view 319/0,
    bar-view 143/0;
  - qmllint 29 files, docs-check ok, `check: ok`.
  - After it, only `memory/pitfalls.md` and this file changed.
- **`flock /tmp/seldon-check.lock just check-perf` at `e7d1cd3` (the
  same code as `dd3d37e`): exit 0** (09:35–09:37). Load was 0.9 at the
  start and 7.6 at the end, from other worktrees. Every budget passed on
  attempt 1:
  - index ×10 4.20 ms, ×150 80.3 ms;
  - hooks at 10 000 lines: 0.76 / 1.59 / 2.82 / **2.23 ms** (not
    recorded / recorded / marker line / the row);
  - hooks at 900 lines: 0.72 / 2.93 / 4.27 / **3.91 ms**;
  - `status` 46.2 ms;
  - redaction rows unchanged within noise, for example: url line 64 KB
    0.60 ms; two kinds 128 KB 9.2 ms; continued options 128 KB
    10.1 ms; password/token 128 KB 4.9 ms.
- In the bench profile, `hooks::robustness::a_panic_exits_zero` fails
  on base and on head alike. Its panic switch is
  `#[cfg(debug_assertions)]`, so this is expected. `just check` runs the
  dev profile, where it passes. Pitfall added.

## Not done

- **Lever (b) (detached rebuild)** was neither measured nor coded. Lever
  (a)'s family reaches the target. Measuring (b) would mean building
  it, and (b) needs an ADR first. WP-092 estimated it at about −1.6 ms.
- **The curl line with a marker** (`… && sudo pacman -U …`, 4.2–4.3 ms)
  is unchanged:
  - `pacman -U` holds `-U`, so `proxy-option` (`curl+-U`) compiles,
    and so does `curl-user` (`curl+-u` on lower case);
  - neither can match, but a trigger cannot say "within the curl
    command";
  - `curl-user`'s `-u` is case-sensitive too, so a written-case
    `curl+-u` would keep it off that line (about 0.25 ms). It needs a
    way to mark a lower-case trigger as "as written". I left it as it
    is, because it is not this WP's row.
- **Margin:** the row's median is 3.87 ms on a quiet host, and 4.07 ms
  at load 5 (set 4). That meets "≤ 4 ms median" with about 0.1 ms to
  spare. The 5 ms bound now has about 1 ms headroom.

## Guard-hook block (reported, not routed around)

- **What was blocked:** one Bash call, as a "privileged or package
  command". It ran my probe script with the row's line as a quoted
  argument (`… | sudo -E bash && git commit -am zed`); the text was
  data for `seldon event --subject`.
- **What I did:** as `memory/pitfalls.md` prescribes for such text, I
  wrote the probe lines to a scratchpad file with the Write tool, and
  the script read them from there. The test rows that hold `sudo -E` in
  `tests/redaction.rs` and this file were written with the Edit/Write
  tools. No command was reworded to get past the guard.

## Decisions needed

1. **Scope of lever (a).** The WP named a bounded `cert-password`
   trigger (` -E`/`--cert`/`--proxy-cert` literals). That literal alone
   gives nothing on the row (`sudo -E`). Even the ordered `curl>-E>:`
   alone gives only −0.3 ms (4.57 ms).
   - Reaching ≤ 4 ms took three more changes of the same kind:
     `url-userinfo`, `proxy-option`, and the scan-on.
   - Each is its own commit and can be dropped alone. s4 without s5
     lands at 4.04–4.10 ms, at load 5–10.
   - Please confirm the wider scope, and the new trigger grammar (`>`
     and capitals) in SPEC §7.
2. **The equivalent survivors L4/L6/L10:** keep the `--`/`-cert`
   literals (now), or drop them on the plain-form argument?
3. **Follow-up for the `pacman -U` line** (Not done): worth a small WP?

## Learned

Appended to `memory/pitfalls.md` (WP-108):
- an option literal is held by other commands' options too (`sudo -E`);
  use what a match needs after the option, in order;
- an option rule pays two compiles; test the on-demand scan-on with
  fresh rules;
- profile per rule before choosing a lever;
- `a_panic_exits_zero` in the bench profile.

## Touched outside WP scope

- `engine/tests/hooks.rs`: a comment only.
- `memory/pitfalls.md`: an append.
- `docs/SPEC-ENGINE.md` §8: one number.
- Nothing outside the repository except the session scratchpad: probe
  patches, A/B and mutant scripts, logs, and a scratch logbook with a
  scratch HOME.
  - That logbook's `init` ran its read-only dossier queries on the dev
    host into the scratchpad, not into the repo.
- The A/B binaries are in `engine/target/ab/` (git-ignored target dir).

## Round 2 (review: stage 1 SEND BACK, tests only)

Commits: `55d908d` (B1 rows, N1), `2455b6e` (N2), plus this section. No
merge of `main` (N3 is the orchestrator's).

### Items

1. **B1.** `redact::tests::option_rules_scan_on_only_with_their_literals`
   has seven new rows, one per option rule. Each repeats the option
   glued to its value, so an `again` literal that holds the option plus
   a space no longer passes:
   - `curl -u a:fakeA1 h -ub:fakeA2`;
   - `curl -U a:fakeB1 h -Ub:fakeB2`;
   - `curl -x a:fakeC1@p h -xb:fakeC2@q`;
   - `curl -b s=fakeD1 h -bt=fakeD2`;
   - `curl -E c.pem:fakeE1 h -Ed.pem:fakeE2`;
   - `http -a a:fakeF1 h -ab:fakeF2`;
   - `docker login -p fakeG1 r -pfakeG2`.

   Each row runs on fresh `builtin_rules()`, as the hook process does.
   The test asserts that no `fake` is left and that the output holds
   two markers.
2. **N1.** The `Rule` doc now says "a text that holds none of
   `triggers` as [`holds_trigger`] reads them (in lower case, or as
   written for one with a capital; `+` and the `>` order) cannot match".
   The paragraph is re-wrapped, and no comment line is longer than 74
   columns.
3. **N2.** SPEC-ENGINE §7: the 107-column line is re-wrapped, and so are
   the next two lines. No word changed.

### Mutants (round 2)

The runner is the same as in round 1, run on the committed `55d908d`
(dev profile, `--lib --test redaction --no-fail-fast`). The baseline
was green before it: lib 221, redaction 26. The source diff was empty
after it.

| # | Mutant | Result |
|---|---|---|
| M4 | cert `again` `-E ` | killed: `option_rules_scan_on…` |
| M15 | registry `again` `-p ` | killed: `option_rules_scan_on…` |
| M22 | cookie `again` `-b ` | killed: `option_rules_scan_on…` |
| M23 | proxy-userinfo `again` `-x ` | killed: `option_rules_scan_on…` |
| M24 | proxy-option `again` `-U ` | killed: `option_rules_scan_on…` |
| M25 | curl-user `again` `-u ` (with `--user` kept) | killed: `option_rules_scan_on…` |
| M14 | httpie `again` `-a ` (re-run; killed in review 1) | killed: `option_rules_scan_on…` |

Result: 7 of 7 killed. With round 1, 38 mutants: 35 killed, and 3
equivalent survivors (L4, L6, L10), which stay as decided.

### Verified

- `cargo fmt` and `cargo clippy --all-targets -- -D warnings` are clean,
  inside `just check`.
- **`flock /tmp/seldon-check.lock just check` at `2455b6e`: exit 0**
  (10:09–10:18). Results:
  - 70 test binaries ok;
  - install 209/0, deploy-test-host 190/0, real-home-guard 11/0;
  - service-states 314/0, panel-view 782/0, overlay-view 319/0,
    bar-view 143/0;
  - qmllint 29 files, docs-check ok, `check: ok`.
- `check-perf` was not re-run. This round changed a test, a doc comment
  and a SPEC line; no code on the hook path changed.

No guard-hook blocks this round. Touched outside WP scope: none.
