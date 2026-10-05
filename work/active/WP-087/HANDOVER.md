```
WP-087 HANDOVER
```

Branch `wp/087-redact-curl`, base `d52e96d`. Commits: `8143fb0`
(quote-aware context), `078705c` (repeated options), `ca79115` (timing
lines), `6f8df75` + `e0224f2` (rows), `bdbb0fa` (SPEC-ENGINE §7 and
module doc), `6bde984` (CHANGELOG), `a979cbc` (pitfalls), plus this file.

## Done

1. **One quote-aware context fragment**, `redact::COMMAND_REST`, defined
   once and used by all six rules that had `[^\n;&|]*?`: `curl-user`,
   `proxy-option`, `proxy-userinfo`, `cookie-option`, `sshpass-password`,
   `registry-login-password`. It is a lazy repetition of
   - any character except a line end, `;`, `&`, `|`, a quote or a backslash,
   - a backslash escape outside quotes (`\;`, `\&`),
   - a single-quoted string `'…'` (on the same line),
   - a double-quoted string with `\"` escapes inside it,

   followed by an optional **unclosed-quote tail**: one quote, then no
   quote and no separator. The tail keeps the old coverage for a note such
   as `curl's -u admin:pw …`. Because no quoted string may follow the
   tail, a stray quote cannot pair with a later one across an unquoted
   `;` (clear row `curl -o 'out' https://h.example; useradd -m 'bob' -U bob`).
   The line end still ends the command, as before.
2. **Repeated options.** `option_rule` builds the five command-option
   rules (all the above except `sshpass-password`). Each gets a second
   pattern, `next = \A(COMMAND_REST option) value`. `Rule::matches` runs
   it on the text after each match that starts at the command word (named
   group `cmd`). This emulates `\G`, which the `regex` crate lacks: the
   pattern is anchored where the previous match ended, and the code
   comment there names the mechanism. The scan repeats until the command
   holds no further option, so the command word is not needed again and
   the anchor is not loosened. `redact` and `matching_rules` both use
   `Rule::matches`; the hand-written replace loop gives the same result
   as `replace_all` for every other rule.
   - A match without the command word does not scan on. A wget line with
     `--proxy-user=bob -U Wget/1.25` keeps its user agent (wget's `-U`).
   - The curl option lists now also hold the options that count without
     the command word (`--proxy-user`/`--proxy-password` for
     `proxy-option`, `--proxy` for `proxy-userinfo`). Before,
     `curl --proxy-user a:b … -U c:d` and `curl --proxy a:b@p -x c:d@q`
     leaked the first credential, because the lazy context ran past it.
   - The `proxy-userinfo` match now runs on to the end of the value
     (named group `host`, with an optional closing quote), so the next
     scan starts outside the quotes. The replacement is
     `${1}‹redacted›@${host}`, and the output is the same as before.
   - `cookie-option`: if the first `-b` names a file and fails the `=`
     check, the scan still goes on and the later `--cookie 'sid=…'` is
     masked (it leaked before).
3. **Tests** (`engine/tests/redaction.rs`)
   - 10 TABLE rows for quoted or escaped separators: `'a=1&b=2'`,
     `"a;b|c"`, `{\"q\":\"x;y\"}`, `\&` outside quotes, `sshpass -P
     'pass;word:'`, `--authfile 'a&b.json'`, and the apostrophe note.
   - 12 TABLE rows for repeated options: `-u`/`--user`/`-uX`, `-U` twice,
     `--proxy-user` then `-U`, `--proxy-user` twice, `--proxy` then `-x`,
     quoted `-x` twice with a quoted `;` between, `-b` twice, a file `-b`
     then `--cookie`, `docker login -p` twice, the wget `-U` row, and
     `sshpass -p … ssh -p 2222` (the port stays).
   - 7 CLEAR rows: unquoted `;`, `&`, `|` after quoted strings, and prose
     (`Merged the curl changes; useradd -U …`, `Fixed curl's output;
     useradd -U is next`).
   - Every row also runs through `masking_twice_changes_nothing` and the
     disjointness test.
   - The timing test gains a "quoted line" and an "apostrophes" filler.
4. **SPEC-ENGINE §7**: one paragraph (the context, the scan-on, and why
   `sshpass` keeps a single `-p`). Module doc in `redact.rs`. CHANGELOG
   `[Unreleased]` gets an `### Engine` line in neutral wording.

## Not done

- **Line continuations and quotes across lines.** `curl -sS \⏎ -u a:b`
  and a quoted string that spans lines still end the command at the line
  end. This was so before, and the WP did not ask for it. It is a
  possible follow-up, since multi-line `curl` commands are common in
  scripts.
- `2>&1` ends the command (an unquoted `&`), as before.
- An option inside a quoted argument (`curl -d 'x -u y'`) is still
  masked through the unclosed-quote tail (over-masking, as before). A
  plain `--proxy-user` *inside quotes* in the context of an anchored
  match is skipped, also as before.
- `proxy-userinfo`'s optional closing quote can swallow the opening
  quote of a token glued to the value (`a:b@p'x'`). That shifts the quote
  pairing for the rest of the command. It is contrived and has no row.
- No user guide change. The WP lists only SPEC and CHANGELOG, and the
  en/de rule lists stay accurate.
- No budget for long lines full of repeated options; see the timings.

## Verified by

- `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` are
  clean.
- `cargo test --locked --no-fail-fast`: 32 binaries, 0 failed.
  `redaction`: 19 passed, 1 ignored.
- **`flock /tmp/seldon-check.lock just check` at `6bde984`: exit 0**
  (12:40–12:58, including the wait for the lock). Results: service-states
  297/0, panel-view 771/0, overlay-view 319/0, bar-view 143/0, install
  209/0, real-home-guard 11/0, qmllint 29 files, docs-check ok,
  `check: ok`.
- **Pre-fix run.** The new tests against `main`'s `redact.rs` make
  `every_builtin_pattern` and `…_disjoint_and_stable` fail. Source
  restored with `git checkout HEAD` + `touch`.

### Timings (same machine, bench profile, median of 21, under the check lock)

`long_lines_with_a_marker_stay_fast` (worst case: every curl rule
triggered, no option found, whole line scanned):

| Line | before (`d52e96d`, load 7.1) | after (`6bde984`, load 2.8) |
|---|---|---|
| url line, 16 KB | 0.121 ms | 0.126 ms |
| url line, 64 KB | 0.532 ms | 0.559 ms |
| German note, 16 KB | 0.126 ms | 0.129 ms |
| German note, 64 KB | 0.529 ms | 0.557 ms |
| quoted line, 16 / 64 KB (new) | 0.076 / 0.304 ms ¹ | 0.128 / 0.558 ms |
| apostrophes, 16 / 64 KB (new) | 0.134 / 0.671 ms ¹ | 0.126 / 0.528 ms |

¹ Measured with a throwaway copy of the test against `main`'s
`redact.rs`. The old context stopped at the first quoted `;`, which is
the bug, so it scanned less of the line.

Hook budget (`hooks` ignored tests, `just check-perf`'s second half):

| Case | before | after (single run) | interleaved A/B, rounds 2–3: old / new |
|---|---|---|---|
| curl line with a marker, 10 000 lines | 2.53 ms | 2.90 ms | 2.45, 2.50 / 2.68, 2.76 ms |
| curl line with a marker, 900 lines | 4.51 ms | 4.94 ms | 4.53, 4.42 / 4.50, 4.59 ms |
| recorded pacman fixture, 900 lines | 3.79 ms | 3.87 ms | – |

- **A/B method.** Bench builds of `seldon` from `main`'s and this
  `redact.rs` were swapped into `target/release/seldon`, and the hooks
  test binary ran 3 × 2 alternately under the lock. Round 1 ran under
  load 7, and *both* variants exceeded 5 ms there (old 5.65/6.08,
  new 5.26/5.49 ms), so it is not counted.
- **What it costs.** The 10 000-line case is consistently about 0.2 ms
  slower. The 900-line case is within noise, with about 0.4–0.5 ms of
  headroom to 5 ms (WP-084 had 0.56 ms). The cause is regex compile cost,
  because the context is a larger automaton. Compile plus first match:
  `curl-user` 100 → 140 µs, `proxy-option` 219 → 256 µs (throwaway
  test).
- **Repeated options on long lines** (throwaway test, not committed).
  About 285 ns per masked option, linear: a 16 KB line of `-u a:b`
  takes 0.67 ms, 64 KB takes 2.7 ms. That is the same order as
  `replace_all` for `token=` (0.47 / 1.9 ms on both versions) and
  `--password` (0.36 / 1.46 ms). `main` was faster here only because it
  masked just the first option.

### Mutants (each run of `--test redaction --no-fail-fast`, source restored with `git checkout HEAD` + `touch`, baseline green afterwards, `git diff` clean)

| # | Mutant | Caught by |
|---|---|---|
| M1 | context back to `[^\n;&|]*?` | `every_builtin_pattern`, `…_disjoint_and_stable` |
| M2 | no single-quoted string | same |
| M3 | no double-quoted string | same |
| M4 | double quotes without `\"` escapes | `every_builtin_pattern` |
| M5 | no backslash escape outside quotes | `every_builtin_pattern` |
| M6 | no unclosed-quote tail | `every_builtin_pattern` |
| M7 | unclosed quote as a loop alternative (lone quote anywhere) | `harmless_text_stays` |
| M8 | separators allowed after an unclosed quote | `harmless_text_stays` |
| M9 | unquoted `;` no longer ends the command | `harmless_text_stays` |
| M10 | unquoted `&`/`|` no longer end it | `harmless_text_stays` |
| M11 | no scan-on (`next: None`) | `every_builtin_pattern`, `masking_twice…`, `…_disjoint_and_stable` |
| M12 | scan-on also without the command word | `every_builtin_pattern` (wget `-U` row) |
| M13 | `proxy-option` curl list without `--proxy-user` | `every_builtin_pattern` |
| M14 | `proxy-userinfo` curl list without `--proxy` | `every_builtin_pattern`, `masking_twice…`, `…_disjoint_and_stable` |
| M15 | `proxy-userinfo` match ends at `@` (no host) | same three |
| M16 | host without the closing quote | same three |
| M17 | `sshpass` scans on too | `every_builtin_pattern` (`ssh -p 2222` row) |
| M18 | `matching_rules` with `captures_iter` (no scan-on) | `every_builtin_pattern`, `…_disjoint_and_stable` |

M15 and M16 survived the first round. The unclosed-quote tail re-paired
the stray closing quote, so the second `-x` was still found (see
`e0224f2` and the pitfall). The row now puts a quoted `;` between the
two options, and both are killed.

## Learned

Appended to `memory/pitfalls.md`:

- The `regex` crate has no `\G`, and `\A` in `captures_at` means the
  text start; slice the text instead.
- A fallback alternative can hide a mutant of the main path.
- How to run hook A/B timings without a second worktree.

## Decisions needed

None blocking. One judgement for the reviewer, taken from the WP's own
test list (only curl options are named as repeated):

- `sshpass-password` uses the new context but does **not** scan on. A
  `-p` after the command that sshpass runs belongs to that command
  (`sshpass -p pw ssh -p 2222 host`), and scanning on would mask the
  port.
- `registry-login-password` does scan on (`docker login -p a -p b`).

Say if `sshpass` should scan on anyway.

Hook headroom: the 900-line curl case has about 0.4–0.5 ms left under
5 ms, and `main` itself exceeds the budget under load 7. If the
headroom matters, WP-084's open question 1 (narrower triggers, e.g.
`pacman -U` no longer triggering `curl+-u`) is the lever.

## Touched outside WP scope

- `memory/pitfalls.md` (append).
- `CHANGELOG.md` and `docs/SPEC-ENGINE.md` (WP outputs).
- No guard-hook blocks. Nothing outside the repository. Scratch scripts
  and binaries stayed in the session scratchpad.
- During the A/B run, `engine/target/release/seldon` was temporarily
  swapped. The final bench build of this branch is back in place.
