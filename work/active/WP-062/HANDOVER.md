```
WP-062 HANDOVER
```

Branch `wp/062-review`. Commits: `13aa97b` (rules), `318321f` (ledger
fields and the command boundary), `2b33701` and `2cc9bef` (tests),
`1262516` (SPEC-ENGINE §7, CHANGELOG, user guide en), `4a0f1e5` (user
guide de), plus this handover.

**Done**

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
  `cargo fmt --check`, `scripts/docs-check.sh` ok; `just check` at the
  end (result below).

**Learned** (memory/pitfalls.md)

- A made-up token of the wrong length is no test of a rule (`ghp_` +
  34 characters is not the documented form): count with `${#T}` first.
- A new rule that matches the same text as an old one doubles the
  import report's per-rule counts and moves its golden; keep built-in
  rules disjoint.
- A clear row proves a narrowed rule only if the old rule matched it;
  run the mutant (the first `task-…` row was one character too short).

**Decisions needed**

- None. Accepted over-matches, for the reviewer: `hotkey=Super` and
  `monkey=1` are masked by `secret-assignment`; `https://u:p@host/a/@b`
  is masked up to the last `@`, host included.

**Touched outside WP scope:** none (all files are on the WP's list;
`memory/pitfalls.md` appended).
