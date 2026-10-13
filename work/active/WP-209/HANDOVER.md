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

## Round 2

Brief: the orchestrator's round 2 from the Opus stage-1 review (SEND
BACK, F1–F6) and the decisions on round 1's questions (Q1: the depth
limit stays here; Q2: the subject is the first privileged command in the
order the shell runs them). Branch merged with local `next` 63b06112
first (F6). Code commits 250a65c8, 2355caaa, 7e787fca.

### Findings

- **F1 — an opened script's depth limit.** `opened_commands` now carries
  a script's `too_deep` onto every command it opens (`Segment::too_deep`;
  a script with no command at all, only `<( )`, leaves one empty marker
  command), and `pkgcmd::too_deep(line)` reads the line's flag and those
  (it opens scripts only when the line has one); the hook ORs it into
  `skipped`. Found on the way: in `sudo sh -c '…$(×17…'` every opened
  command carries `sudo`, so the deepest one, whose word held the skipped
  text, would have become the record's subject with the unread text (a
  secret too) in it. Now a skipped substitution stands in every word as
  its brackets only (`$(…)`, `` `…` ``, `(…)`; `Reader::skips`), found
  with one non-recursive scan (`skip_end`: parentheses, quotes and
  backticks nested in `$( )`); and a privileged record names a program
  that a substitution computes (`sudo $(which x)`) only when no command
  names one. Tests: `hooks privileged::a_substitution_nested_too_deep_is_masked_whole`
  adds `sudo sh -c '…'` and `eval 'sudo …'` nested 17 deep with
  `alice:hunter2 | chpasswd` inside: both `lpadmin ‹redacted›`, no
  `hunter2` in the ledger; `pkgcmd::tests::substitutions_are_opened`:
  `sh -c`, `eval`, two scripts down, a script of `<( )` only, 16 deep
  still read, skipped text in no word (also a `<( )` inside `$( )` and
  `` `a $(b `qq`) zz` ``). Mutants: the carry dropped, the hook reading
  the line's flag only, no marker, skipped text in the word (two), the
  scan ending at a nested backtick, a computed program named first — all
  killed. Evidence: *fixture*.
- **F2 — directories per substitution.** Substitution numbers are unique
  on the line, those of opened scripts included (they continue after the
  highest so far; a script's own commands stay in the substitution its
  shell command stands in), and every command carries its parent
  (`Segment::parent`). `workdirs` keeps the directories by substitution
  number: a substitution reads those of the nearest one around it until
  it moves (`cd`, `pushd`, `popd`), and then copies them. That copy is
  what they were when it started: the commands of one substitution come
  in one block before any later command of the one around it, so the
  parent cannot move in between. Tests: `working_directories`: `cd a;
  $(cd /x; tee p $(tee q) r)` → `p`, `q`, `r` under `/x`, the line back in
  `/w/a`; `$(cd /x; $( $(tee q) )) && tee r` (a middle substitution with
  no command before its child); a `pushd` stack popped in a substitution
  and kept outside it; `substitutions_are_opened` checks the numbers and
  parents across `sh -c` and `eval`. Mutants: seeded from the line not the
  parent, numbers restarting in a script, a script's own commands with the
  wrong parent, a `cd` in a substitution moving the line — all killed.
  Evidence: *fixture*.
- **F3 — no stack copy per substitution.** The `pushd` stack shares its
  tails (`DirStack`: a push makes a new head, a copy is one reference
  count, freed node by node, no recursion), and directories are copied
  only on a move (F2). The reviewer's line (`pushd /; `×7000, `echo`,
  `$(:) `×13000, plus a `<(` after `--pass-file` so it is masked; 125 KB)
  is a row of `a_long_privileged_line_is_read_in_linear_time` with the
  60 ms budget. First measured at 43.4 ms; the rest was the hook reading
  the line four or five times, so `bash_records` now reads it once
  (`commands_and_vars`, `workdirs` once, `mutations_of`, `secret_in`; the
  public `mutations` and the test helper stay as wrappers): **29.6 ms**
  idle, 44.4 ms under load (below). Probe (not committed): parse plus
  `workdirs` at 31/62/125 KB 2.3+2.0 / 3.8+4.2 / 7.3+8.1 ms (before the
  vector rewrite of `workdirs`), linear. Evidence: *fixture*.
- **F4 — the unclosed triple quote with key near-misses** is a bench row of
  its own again (`unclosed triple quote and key near misses`, budget 20 ms
  at 128 KB): 0.48 / 1.87 / 3.74 ms at 16 / 64 / 128 KB idle; the row
  without them stays (0.15 / 0.59 ms at 16 / 64 KB, budget 1 / 2 ms).
  Evidence: *fixture*.
- **F5 — SPEC.** §8 lists as limits `${…}`, `$((…))` and `$[…]` (a `$(…)`
  or backticks inside them not read), `$((cmd) )`, which the parser reads
  as arithmetic, and a backtick substitution nested in backticks by
  escaping; the claim that `$((…))` runs no command is gone. §7 "masked too
  much" names a triple-quoted value that never closes, masked up to the
  next unrelated triple quote. §8 also says how a substitution's
  directories work (F2) and that the depth limit holds in opened scripts
  (F1). Evidence: `docs-check` in `just check`.
- **Q2 — the subject.** SPEC §8 ("One per line, for its first privileged
  command in the order the shell runs them … `sudo foo $(sudo bar)` is
  recorded as `bar`; a program named by a substitution only when no other
  has a name") and the ADR-0039 WP-209 note say so. The note now names
  §1's **Limits** sentence it narrows ("A privileged command inside
  `$(…)`, backticks or `env -S` … is not seen") and says which parts of it
  stay (`env -S` and the others).
- **F6 — merge.** `git merge 63b06112`: only `CHANGELOG.md` conflicted;
  both bullets kept (WP-219's first, then WP-209's, then WP-218's).

### How it was verified (round 2)

All in the worktree with its own HOME, a 0700 runtime dir, TMPDIR and
JUST_TEMPDIR under `gates/dev-209/`,
`CARGO_TARGET_DIR=gates/target-dev-209`, each command written literally.

- `cargo fmt --check`: clean. `cargo clippy --all-targets -- -D
  warnings`: clean. `cargo test` (all binaries, lib included): 56
  binaries, 1465 passed, 0 failed, 11 ignored (timing) (*fixture*).
- **Mutants** (`work/active/WP-209/mutants.py`, 45 now, ten new), on
  a `git archive` copy under `flock`: 7e787fca **45/45 killed**, none only
  by a failing compile (log `gates/mutants-dev-209-r2c.log`). On the way:
  250a65c8 44/45 — `a backtick inside $( ) ends the skip` survived, the
  test lacked the case; 2355caaa added it (`` `a $(b `qq`) zz` ``) and
  45/45 (log `mutants-dev-209-r2b.log`).
- **Timing, idle** (bench profile, under the lock, 7e787fca; log
  `gates/perf-dev-209-r2-idle.log`): hook lines 16/64/128 KB
  12.4/15.2/19.6 ms, with substitutions 13.5/19.2/27.2 ms, the `pushd`
  line 29.6 ms (budget 60 ms); every redaction row green.
- **Load run** (8 `yes` loops, each `timeout 480`, all and the tests
  under `flock -s` on the check lock, so no exclusive check ran beside
  them; load average 2.7 → 7.6 on 16 cores; ended by their timeout):
  - hook timing: 16/64/128 KB 12.8/18.6/18.6 ms, with substitutions
    15.2/18.8/39.2 ms, `pushd` line **44.4 ms**: green (log
    `gates/perf-dev-209-load-r2.log`). An earlier load run on 2355caaa,
    before the vector `workdirs` and the single reading, had the `pushd`
    line at 71.4 ms (fail, log `perf-dev-209-load.log`); that is why both
    changes are in this round.
  - redaction timing: **failed** at `single-quoted keys, 64 KB`
    (2.79 ms against 2 ms), a WP-157 row this WP does not change; the test
    stops at its first failing row, so the rows after it were not timed
    under load. Paired idle: base bde78fdc 1.47 ms, head 1.45 ms (logs
    `perf-dev-209-base-redact.log`, `perf-dev-209-r2-idle.log`). No budget
    moved.
  - functional, under the same load: lib 421/0, attribution 20/0, hooks
    92/0 (4 ignored), redaction 37/0 (1 ignored), shell_parser 5/0
    (*fixture*).
- **`flock /tmp/seldon-check.lock env SELDON_FULL_CHECK=1 just check`**
  on 7e787fca: **exit 0, `check: ok`** (log `gates/check-dev-209-r2.log`):
  112 test binaries, 2941 passed, 0 failed, 22 ignored (*fixture*);
  install 230/0, deploy-test-host 327/0 (with shellcheck), qmllint ok (53
  files), docs-check ok, real-home-guard 79/0; plugin-test ok, desk-view
  2723/0, bar-view 196/0, ipc-restart 44/0 (*headless*); the real
  `~/.local/state/seldon` and `~/.config/seldon` untouched, no leftover in
  either runtime dir.
- shellcheck: only inside `just check` (no shell script changed). *test
  host*, *desktop*, *CI*: *not run*.
- `git diff bde78fdc..HEAD`: no home path, user or host name added.

### Open points

- The `single-quoted keys` 64 KB budget (2 ms) fails under 8 loops on base
  and head alike (1.47 ms idle); the orchestrator's call whether it gets a
  budget per medium, as Fable suggested for check-perf in WP-157.
- `Reader::substitution` still checks each character of a substitution
  against every heredoc cut (pre-existing, O(text × heredocs); bounded by
  the depth limit), as noted in round 1.
