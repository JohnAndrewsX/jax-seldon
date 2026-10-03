```
WP-062 HANDOVER
```

Branch `wp/062-review`. Round 1: `13aa97b` (rules), `318321f` (ledger
fields and the command boundary), `2b33701` and `2cc9bef` (tests),
`1262516` (SPEC-ENGINE §7, CHANGELOG, user guide en), `4a0f1e5` (user
guide de), `fdc593f`, `17a76fc`, `5d347bb`. Round 2, after review:
`13d8d9d` (engine and tests), `abfa9b9` (SPEC-ENGINE §7 and §8 timing,
user guide en), `8fe678f` (user guide de), `93ebae2` (CHANGELOG,
pitfalls), `e097cd3`. Round 3, after review: `823e80c` (engine and
tests), `e58949f` (SPEC-ENGINE §7 and §8, user guide en), `69a941c`
(user guide de), `d61b932` (CHANGELOG), plus this update.

**Round 3 (review SEND BACK)**

1. **Credential names mask any value; only `key` names keep the shape
   check.** `secret-option` (`--token`, `--with-token`, `--secret`,
   `--client-secret`, `--passphrase`) and `secret-assignment`
   (`…SECRET=`, `…PASSWORD=`, `…PASSWD=`, `…PASSPHRASE=`, `…_PWD=`,
   `…_PASS=`, `SSHPASS=`) mask any non-empty value (`has_value`: an
   empty `""` names no secret); `--password` and `token=` as before.
   Two new rules keep `looks_like_credential` (unchanged thresholds:
   16 characters, or 8 that mix two of lower, upper, digit, other):
   `key-option` (`--api-key`, `--access-key`, `--secret-key`) and
   `key-assignment` (`…KEY=`). `BUILTIN` is now 18; the rules stay
   disjoint (`--secret-key` and `SECRET_KEY=` only match the `key`
   rules). New must-mask rows: `tool --token short`, `PASSWORD=x`
   (both moved from the clear list), `PASSWORD=hunter2`, `passwd=abc`,
   `secret=x1`, `DB_PASS=short`, `MYSQL_PWD=pw`, `PGPASSWORD=pw`,
   `SSHPASS=pw`, `PASSWORD="hunter2"`, `export SECRET='a b c'`,
   `tool --secret hunter2`, `--token abc`, `--passphrase abc`. The four
   earlier false positives stay clear, plus a new clear row
   `tool --api-key=auto`. Mutants: either credential rule with the shape
   check → `every_builtin_pattern` fails; either `key` rule without it →
   `harmless_text_stays` fails (`key-assignment` also
   `a_value_must_look_like_a_credential`).
2. **Folded trigger text.** `redact::trigger_text` lowers ASCII and maps
   the Kelvin sign (U+212A) to `k` and the long s (U+017F) to `s`, the
   two characters case-insensitive matching folds onto an ASCII letter;
   both the trigger check and `matching_rules` use it. Rows:
   `to\u{212A}en=…` (token-assignment), `API_\u{212A}EY=…`
   (key-assignment), `PA\u{17F}\u{17F}WORD=pw` (secret-assignment); the
   trigger test uses the same function. Mutants: either mapping removed
   → `every_builtin_pattern` and `every_row_holds_a_trigger_of_its_rule`
   fail.
3. **SPEC §8 timing** is now a dated comparison, no absolute figures:
   "not slower than before it: measured 2026-10-03 on a loaded dev host
   (load average 3 to 8) … a recorded command took 19 to 36 % less
   time". Re-measured on the round-3 code: release builds of `main` and
   `823e80c`, interleaved as in round 2, median of 21 (ms):

   | Case | `main` | `823e80c` |
   |---|---|---|
   | ledger 2 → 23 | 6.81 | 5.13 |
   | 975 → 996, with rebuild | 5.92 | 4.36 |
   | 1005 → 1026, no rebuild | 2.59 | 1.66 |
   | secret fixture, 2 → 23 | 4.29 | 3.47 |

   The host was busy (load average up to 8), so only the ratio is in
   the SPEC; the round-2 table below was taken at load 1 to 3.5.

Verified (round 3): `cargo fmt --check`, `cargo clippy --all-targets --
-D warnings`, `redaction` 17, `log` 12, `hooks` 39, `import` 7, lib 153,
`scripts/docs-check.sh` ok; 6 round-3 mutants caught, sources restored
(`git diff --exit-code`), debug binary rebuilt; scratch release builds
and bench homes deleted.

**Round 2 (review SEND BACK)**

1. **Tags (B1).** `seldon log` redacts each `--tag` at the command
   boundary; the journal's `#tag` line and the ledger's `meta.tags` hold
   the same masked text. Test
   `commands::log_masks_a_tag_in_the_journal_and_the_ledger` (a made-up
   `ghp_` tag next to a plain one: `#‹redacted› #keys` in the journal,
   `‹redacted›,keys` in the ledger, nowhere else in the logbook, state or
   git history). Mutant: tags taken unredacted → that test fails.
2. **Hook cost (B2).** The built-in rules live in one process-wide
   `LazyLock`; each rule keeps its pattern text and compiles into a
   `OnceLock` the first time a text holds one of its literal triggers
   (`redact::triggers`, necessary substrings in ASCII lower case: `://`,
   `token=`, `key=`, `ghp_`, `curl`, …). A command line without any
   trigger compiles nothing. Test `every_row_holds_a_trigger_of_its_rule`
   (every rule has triggers, every must-mask row holds one of its rule's
   triggers, the marker `‹redacted›` holds none, so a replacement can
   never make a later rule miss). Mutant: a wrong trigger for the URL
   rule → 7 tests fail.

   Timing: three release builds (`main`, the reviewed `5d347bb`, the
   fix), one scratch HOME per build with all `XDG_*` and
   `SELDON_TEST_GUARD`, an active case, the repository's mutating hook
   fixture with a fresh `tool_use_id` per run, 21 rounds with the build
   order rotated every round, median (min) in ms; load about 1 to 3.5:

   | Ledger lines | `main` | `5d347bb` | fix |
   |---|---|---|---|
   | 2 → 23 | 3.12 (3.02) | 5.82 (5.70) | 1.83 (1.75) |
   | 975 → 996 (with rebuild) | 5.02 (4.86) | 7.81 (7.51) | 3.72 (3.60) |
   | 1005 → 1026 (no rebuild) | 2.19 (2.12) | 3.65 (3.47) | 1.38 (1.30) |
   | 2 → 23, secret fixture | 3.28 (3.18) | 6.21 (6.02) | 2.49 (2.39) |

   A first fix with `LazyLock` only (all 16 compiled once) measured 5.39 /
   6.22 / 3.74 ms in the same setup: still about 1.5 ms over `main`,
   hence the triggers. SPEC-ENGINE §8 carries the new numbers; §1's
   "`hook` < 5 ms" holds again (it did not hold for `5d347bb` near 1000
   lines), so line 12 is unchanged.
3. **Clear text (F).** (Superseded by round 3 for the credential names:
   now only the `key` rules check the shape.) `secret-option` and
   `secret-assignment` mask a value only when it looks like a credential
   (`redact::looks_like_credential`): without its quotes, at least 16
   characters (`CREDENTIAL_LONG`), or at least 8 (`CREDENTIAL_MIN`) that
   mix two of lower case, upper case, digits and other characters.
   `--password` and `token=` keep masking any value, as before this WP.
   The header rule takes only names that end in a credential word:
   `X-…-Key`, `X-…-Token`, `X-…-Secret`, `X-Auth`, `X-…-Auth`, `Api-Key`,
   `Private-Token` (`Authorization` is its own rule). New
   must-stay-clear rows: `the key=value pairs`, `sort --key=2 names.txt`,
   `hotkey=Super`, `curl -H 'X-Author: me' …`, plus `tool --token short`
   and `PASSWORD=x ./run.sh` (26 clear rows). All 46 must-mask rows stay
   green (three made-up values were lengthened to 9 characters to pass
   the threshold). Test `a_value_must_look_like_a_credential` pins the
   boundaries (7 mixed: no, 8 mixed: yes, 13 one class: no, 16: yes,
   quotes not counted). Mutants: either rule without the check →
   `harmless_text_stays` fails; the round-1 header pattern →
   `harmless_text_stays` fails. `matching_rules` applies the same check,
   so the import report counts only lines that are actually masked.
4. **Pitfall (N1)** reworded: a payload whose paths lie outside
   `~/.config`, or a fixture; a guard block is reported.
5. **SPEC §7 (N3):** "Redacting twice gives the same text for the
   built-in rules"; a user pattern that matches across the marker's edge
   is applied as written.

Not claimed, a later item: `curl -U`/`--proxy-user`, JSON
`"password": "…"` and `Cookie:` headers are not covered by a built-in
rule (a user pattern can cover them).

Verified (round 2): `cargo fmt --check`, `cargo clippy --all-targets --
-D warnings`, `cargo test` (all binaries, 0 failed; `redaction` 17,
`log` 12, `hooks` 39, `import` 7), `scripts/docs-check.sh` ok; the
mutant runs and the scratch builds deleted afterwards.

**Done (round 1)**

- **Rules** (`engine/src/redact.rs`, `BUILTIN` now 16, in run order):
  - The URL rule masks userinfo that holds a `:` from `://` up to the
    last `@` before white space or a quote, so a password may contain
    `/ ? # : @`. Userinfo without a `:` (a bare token) is masked up to the
    last `@` before the path, as before.
  - New: `secret-option` (`--token`, `--api-key`, `--with-token`,
    `--secret`, `--client-secret`, … with `=` or a space; not
    `--token-file`), `secret-assignment` (`…KEY=`, `…SECRET=`,
    `…PASSWORD=`, `…PASSWD=`, `…PASSPHRASE=`, `…_PWD=`, `…_PASS=`,
    `SSHPASS=`, case-insensitive; the shell's `PWD=`/`OLDPWD=` stay
    clear; `…TOKEN=` stays with `token-assignment`), `secret-header`
    (`X-…-Key:`, `Api-Key:`, `Private-Token:`), `gitlab-token`,
    `slack-token`, `curl-user` (`-u`, `--user`, within one command of
    the line), `sshpass-password`, `registry-login-password` (`docker`,
    `podman`, `buildah`, `nerdctl`, `helm registry` `login -p`).
  - Widened: `aws-access-key` also `ASIA…`; `github-token` `ghp_`,
    `gho_`, `ghu_`, `ghs_`, `ghr_` at any length from 36, and
    `github_pat_…`; `openai-key` is now
    `sk-key`, `\bsk[-_]…{20,}`: it matches `sk_…` as SPEC §7 says, and
    starts at a word, so a name such as `python-task-manager-application-git`
    is no longer cut.
  - Masking twice gives the same text: a match that lies inside an
    existing `‹redacted›` is left alone (a user pattern such as `red`
    used to rewrite the marker itself).
  - `Redactor::for_config(&Config)` is the one helper commands use.
- **Ledger** (`engine/src/ledger.rs`): `append` redacts `subject` (then
  cuts it at 512 characters, like `detail` at 4096) and every string
  value of `meta`, not only `detail` and `meta.command`.
- **Commands** redact their free text right after opening the logbook,
  before the lock and the first write: `log` (the note), `plan new` (the
  title: frontmatter, heading, file name, ledger, STATUS.md), the `plan`
  step `--reason`, the `plan done` journal stub, `decide` (the title:
  file, file name, DECISIONS.md), `drift explain` (the intent: the new
  case and the ledger) and `drift dismiss` (the reason). `event` writes
  only to the ledger, which now redacts every field.
- **Docs**: SPEC-ENGINE §7 first paragraph (every rule, the fields, the
  commands, idempotency, the cut lengths); user guide 06 redaction
  section en/de (the list of forms and fields; example user patterns
  that the built-in rules do not already cover); CHANGELOG
  `[Unreleased]` › Engine.

**Not done**

- Existing ledger lines, journal days, case files and git history are
  not rewritten (append-only). The docs say so.
- `hook.rs` and the `skipPaths` paragraph of SPEC §7 are WP-063's; the
  hook gets the new ledger behaviour without a change there.
- `import` (WP-061's file): not touched. Its scrubber already passes
  every imported file through the same `Redactor`, so the new rules
  apply to imports too; the import tests and golden are unchanged.

**Verified by**

- `engine/tests/redaction.rs` (14 tests, all green):
  - `every_builtin_pattern`: one row per rule and form; URL rows with
    `/`, `?`, `#`, `:` in the password and a bare token; every value
    made up, the prefixed token forms spelled in parts with `concat!`.
  - `harmless_text_stays`: 20 clear rows (`docker run -p 8080:80`,
    `ssh -p 2222`, `sort -u`, `curl --user-agent`, `--token-file …`,
    `docker login --password-stdin`, `PWD=/tmp OLDPWD=/var`, URLs with a
    port or `/@user` in the path, …).
  - `masking_twice_changes_nothing`: every row, the rows joined, a
    prose line, with the user pattern `red|act`.
  - `ledger_redacts_the_subject_and_every_meta_value`: subject, typed
    meta keys, `extra` strings, a number kept, a subject that grows past
    512 is cut.
  - `commands::*`, one per command (`log`, `plan new` + step reasons,
    `plan done` stub with a title edited by hand, `decide`, `drift
    explain` + `dismiss` on a fixture copy, `event --subject/--detail/
    --meta`): the masked text is in the journal, case, decision file,
    DECISIONS.md, STATUS.md and ledger, and no file of the logbook or
    state directory and no commit (`git log -p --all`) holds the value.
- Every item has a mutant that a test catches (12 targeted mutants plus
  a run with all engine sources from `main`: 9 of the 14 tests fail
  there); the runs are with the orchestrator.
- Manual run in a scratch HOME with all `XDG_*` and `SELDON_TEST_GUARD`
  (TESTING.md "Manual runs"), made-up values: `log`, `plan new`,
  `decide`, `event`, an agent-hook payload with `curl -u` and a URL
  password with `/`, then `status`: 0 hits of any value in the logbook,
  the state dir and `git log -p`; the hook line reads
  `curl -u ‹redacted› https://‹redacted›@example.invalid/r > ~/.config/hypr/r`.
- `cargo test` (all), `cargo clippy --all-targets -- -D warnings`,
  `cargo fmt --check`, `scripts/docs-check.sh` ok; `just check` at
  `fdc593f` (the tree of this handover minus this file): exit 0,
  `check: ok`, host steps included (plugin validate, qmllint,
  plugin-test with the real-home guard).

**Learned** (memory/pitfalls.md)

- A made-up token of the wrong length is no test of a rule (`ghp_` +
  34 characters is not the documented form): count with `${#T}` first.
- A new rule that matches the same text as an old one doubles the
  import report's per-rule counts and moves its golden; keep built-in
  rules disjoint.
- A clear row proves a narrowed rule only if the old rule matched it;
  run the mutant (the first `task-…` row was one character too short).

**Decisions needed**

- None. Accepted over-match, for the reviewer: `https://u:p@host/a/@b`
  is masked up to the last `@`, host included. (Round 1 also masked
  `hotkey=Super`; round 2 no longer does.)

**Touched outside WP scope:** none (all files are on the WP's list;
`memory/pitfalls.md` appended).
