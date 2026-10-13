# WP-209 — Handover

Branch `wp/209-redaction-after-157` from local `next` (bde78fdc); merges
into `next`. Security class: reviewer chain Opus, then Fable. Every secret
in the tests is made up (`fake…`, `hunter2`, `pw`).

## What was done

The five items of the WP, the private reviews' N2 (N1 and N3 were fixed in
WP-157 round 3), and one robustness fix that item 5 made necessary.

SPEC-ENGINE §7 (`engine/src/redact.rs`):

1. **Python triple-quoted values** (`json-secret`): `"""…"""` and
   `'''…'''` after a secret key, over lines too, with a string prefix
   (`rf"""…"""`), tried before the empty `""` at their start; also
   `\"\"\"…\"\"\"` inside a double-quoted shell string (`python3 -c
   "…"`, the common agent form). `has_json_value` strips triple quotes
   before single ones, so `""""""` is no value.
2. **Key words with `key` after them** (`json-secret`): `SECRET_KEY` takes
   an optional `[_-]?(access[_-]?)?key` after the key word: `"secret_key"`,
   `"secretKey"`, `"token-key"`, `'aws_secret_access_key'`, escaped
   `\"SECRET_KEY\"`. Still end-anchored: `secret_key_base`,
   `password_key_hint`, `token_type`, `secret_name` stay. Triggers: each
   key word, then `key` and a quote, in that order (`secret>key"` …).
3. **nmcli's field option before the object word only** (`nmcli-secret`):
   the `field` group no longer keeps a match by itself; `Rule::keep`
   (new, a function on the captures) calls `nmcli_field`, which keeps it
   only when nothing but nmcli's global options (and the values of
   `--colors`, `--escape`, `--fields`, `--get-values`, `--mode`, `--wait`,
   any prefix) stand between `nmcli` and the field option, and only in a
   match that starts at the command word. `nmcli con modify Home -f
   wifi-sec.psk PW` (which nmcli rejects) is masked; `nmcli -m tabular -g
   wifi-sec.psk con show Home` stays whole.

Shared parser and hook (`engine/src/pkgcmd.rs`,
`engine/src/commands/hook.rs`, `engine/src/commands/hook/secret_args.rs`):

4. **`stdbuf` and `setsid` are wrappers** (`WRAPPERS`, 14 now): stdbuf's
   `-i/-o/-e` and `--input/--output/--error` take values, setsid's `-h`,
   `-V`, `--help`, `--version` are probes. `echo PW | sudo stdbuf -i0
   cryptsetup open …` is `cryptsetup ‹redacted›`.
5. **The commands inside `$( )`, backticks and `<( )` are commands of the
   line.** `Reader::inside`/`Reader::close`: the substitution's lexer
   pushes onto the line's own segment list (one list for every depth, so
   nesting costs no copy), so its commands come before the command they
   stand in, as the shell runs them. `Segment::substitution` numbers them
   (0 = the line itself); `workdirs` gives a substitution's commands their
   own `cd` and restores the line's directories after them (`D=$(cd /x &&
   pwd); tee y` writes `y` where the line is). A substitution's lexer
   starts with the line's feed (`echo PW | x $(passwd …)`: fed). `${…}`,
   `$((…))`, `$[…]` are not opened, as before. Effects: classification,
   privileged records and the secret check all read them
   (`x=$(sudo mkpasswd -m sha-512 PW)` is recorded as `mkpasswd
   ‹redacted›`; base recorded nothing). WP-157 round 3's
   `lexer.fed |= inner.fed` is kept, in `Reader::close`.
6. **Depth limit (`SUBSTITUTION_DEPTH_MAX` = 16).** Base reads `$(` by
   recursion and copies each substitution's text into every enclosing
   word: a 40 KB line of nested `$(` overflowed the stack (abort, the hook
   records nothing and exits non-zero) and depth 5000 took 49 ms (probe
   below). Opening the commands would have made it worse (O(depth²)
   memory). A substitution deeper than 16 is now skipped to its end
   without recursion (`closing`, `backtick_end`), `ShellLine::too_deep`
   is set, and the hook records the line as `<program> ‹redacted›` (its
   commands, feeds and heredocs are unknown). Fail-safe: more masked,
   never less.
7. **N2 — a descriptor the line feeds** (`secret_substitution`): the
   descriptors given text on the line (`N< <(…)`, `N<<< …`; `<`/`<<<`
   alone: 0) are collected first; then each `/dev/fd/N`,
   `/proc/self/fd/N` or `/dev/stdin` with such an N counts when the word
   before it names a secret, as for `<(` (`--password-file /dev/fd/3 3<
   <(echo PW)`, `exec 4< <(…); tool --passphrase-file=/dev/fd/4`, `-passin
   file:/dev/fd/3`). A descriptor from a file (`3< pw.txt`), another
   descriptor, `/dev/fd/33` for 3 and an option without a secret word stay
   whole. Two passes over the text, no regex.

Docs: SPEC-ENGINE §7 (the two `json-secret` forms, nmcli's field rule
with `Rule::keep`; the WP-157 "not masked" sentence on triple quotes
removed) and §8 (the two wrappers; the substitution paragraph replaces
"not classified" with what is read and its limits, the depth limit; the
descriptor sentence; the feed of a substitution); the ADR-0039 amendment
note (WP-209); TESTING (hooks and redaction rows); CHANGELOG (Engine).
`work/active/WP-209/mutants.py` (WP-157's runner, 35 WP-209 mutants).

## What was not done

- `${x:-$(cmd)}` and `$(( $(cmd) ))`: the `$(…)` inside is not read
  (SPEC §8 says so). Opening `${…}` needs a parameter-expansion reader.
- A here-string or `<(…)` given to a descriptor whose file the program
  reads without a secret-named option (`tool -p /dev/fd/3 3< <(…)`): not
  masked, the same class as `restic -p <(…)` (SPEC §8 already says short
  options are not read).
- Not changed (pre-existing, noted): `Reader::substitution` checks every
  character of a substitution against all heredoc cuts (`cuts.iter()`),
  O(text × heredocs) on a line with many heredocs inside substitutions.
  With the depth limit it is bounded by 16 × that; not measured as a
  problem. A possible follow-up.
- No CHANGELOG Highlights bullet; the orchestrator's call.

## How it was verified

- **Unit and integration tests (*fixture*)**, worktree, own HOME,
  0700 runtime dir and TMPDIR under `gates/dev-209/`,
  `CARGO_TARGET_DIR=gates/target-dev-209`: lib 421/0; `--test hooks`
  86/0 (4 ignored, timing); `--test redaction` 37/0 (1 ignored, timing);
  attribution 20/0, shell_parser 5/0. New and changed tests:
  `pkgcmd::tests::substitutions_are_opened`,
  `stdbuf_and_setsid_are_wrappers`, `working_directories` (substitution
  rows), `fed_commands` (expectations updated: the commands inside now
  come first; `exec 3< <(a); b` → `[false, true, true]`, `a` inside is not
  fed), `quotes_operators_and_comments` and
  `heredocs_in_substitutions_and_shifts` (the inner commands now listed);
  `secret_args` positive and negative rows for every item;
  `hooks privileged::substitutions_wrappers_and_descriptors_on_a_privileged_line`
  and `privileged::a_substitution_nested_too_deep_is_masked_whole`;
  `redaction` TABLE rows (`fakePy07`–`fakePy17`, `fakeNm09`–`fakeNm12`)
  and CLEAR rows.
- **Mutants (*fixture*)**: `work/active/WP-209/mutants.py` on a
  `git archive 8acebfef` copy (`gates/dev-209/mut`), under `flock
  /tmp/seldon-check.lock`: **35/35 killed**, none only by a failing
  compile (log `gates/mutants-dev-209.log`). One per rule and branch: the
  three triple-quote forms, one-line content, the two empty-value checks,
  the key suffix, `access`, any word after the key word, the key
  triggers; nmcli field anywhere / never / without option values / after
  a scan-on; each wrapper, stdbuf's value options, setsid's probes;
  substitutions not opened, `<( )` not opened, not fed, feeding nothing,
  numbered 0, a `cd` moving the line, no depth limit, too-deep not masked;
  six descriptor mutants.
- **Linear time (*fixture*, release `bench` profile, under the lock)**:
  - `redaction long_lines_with_a_marker_stay_fast` on head: green. WP-209
    rows, 16/64/128 KB: triple-quoted values 0.63/2.52/5.01 ms, escaped
    triple quotes 0.40/1.57/3.20, triple quote openers 0.51/2.05/4.02,
    secret key words 0.76/3.01/5.96, nmcli fields after the object
    0.72/2.87/5.91, before it 0.61/2.44/4.87 (budget 20 ms at 128 KB);
    near-miss rows at 16/64 KB: unclosed triple quote 0.16/0.62 ms, key
    word near misses 0.62 ms at 64, nmcli global options 0.67 ms at 64
    (budget 1/2 ms). Logs `gates/perf-dev-209-head.log`,
    `perf-dev-209-head-r2.log` (the unclosed-triple row first held
    `'token': ""` near-misses, 1.91 ms at 64 KB against 2 ms; the filler
    now holds no key, as the row's name says).
  - `hooks a_long_privileged_line_is_read_in_linear_time`: head 16/64/128
    KB 13.1/16.6/21.4 ms, the new line of substitutions, wrappers,
    descriptors and a 20-deep nest 13.7/19.5/27.0 ms (budget 60 ms at
    128 KB); base bde78fdc, same session, 12.1/15.7/19.9 ms (log
    `gates/perf-dev-209-base.log`).
  - Probe, not committed (`gates/dev-209/probe_nesting.rs`, logs
    `probe-head.log`, `probe-base.log`): nested `$(` at depth
    100/1000/5000/20000/60000: head 0.05/0.24/1.18/4.83/13.9 ms; base
    0.07/3.16/49.0 ms, then a stack overflow (abort) at 20000. Lines of
    sibling substitutions 64/128/256 KB: head parse 6.0/10.6/22.5 ms
    (it keeps 52 k commands at 256 KB), base 2.8/4.8/10.2 ms; descriptor
    lines and triple-quote openers: parse and redaction alike on both,
    each doubling about 2×.
- **`SELDON_FULL_CHECK=1 flock /tmp/seldon-check.lock just check`**:
  on 3b6d5fdf (the code as handed over), own HOME, 0700 runtime dir,
  TMPDIR and JUST_TEMPDIR under `gates/dev-209/`, shellcheck from the
  orchestrator kit: **exit 0, `check: ok`** (log
  `gates/check-dev-209.log`): 112 test binaries, 2927 passed, 0 failed,
  22 ignored (*fixture*); install 230/0, deploy-test-host 327/0 (both
  with shellcheck), qmllint ok (53 files), docs-check ok, real-home-guard
  79/0; plugin-test ok with the Quickshell harnesses, desk-view 2723/0,
  bar-view 196/0, ipc-restart 44/0 (*headless*); the real
  `~/.local/state/seldon` and `~/.config/seldon` untouched, no leftover
  in either runtime dir.
- `just check-perf` as a whole: *not run* (the rows this WP touches ran
  as above; the three disk-TMPDIR rows WP-157 reported as failing on base
  are not this WP's).
- shellcheck: only through `just check` (no shell script changed). *test host*,
  *desktop*, *CI*: *not run*.
- `git diff bde78fdc..HEAD`: no home path, user or host name added; test
  secrets are `fake…`, `hunter2`, `pw`, `tok`.

## Open questions

1. The depth limit (16) and its fallback (`<program> ‹redacted›`) are new
   behaviour beyond the WP's list, added because opening substitutions
   made the pre-existing deep-nesting cost worse. Accept, or move to its
   own WP?
2. A privileged record's subject is the first privileged command in
   shell order, so `sudo foo $(sudo bar)` is now recorded with subject
   `bar` (it runs first). Fine for the reviewers?
