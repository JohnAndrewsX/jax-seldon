WP-098 HANDOVER

Branch `wp/098-test-host-main`, worktree `wt/WP-098`.

## Done

- **Dev version marker** (`engine/src/lib.rs`): `VERSION` is Cargo.toml's
  version plus `+<SELDON_BUILD>` when the build sets `SELDON_BUILD`
  (`0.1.3+main.1a2b3c4`); unset or empty stays plain (release builds).
  No build script: `option_env!` in a const, concatenated by two
  `const fn`s; a value that is not semver build metadata
  (dot-separated `[0-9A-Za-z-]` identifiers) fails the build (E0080).
  Every consumer already reads `VERSION`: `--version`, `--version
  --json`, the index's `engineVersion`, doctor's engine row, the man
  page footer; `main.rs` needed no change. Cargo rebuilds when the
  variable changes (checked: plain → marked → plain).
- **Tests for both forms:** unit tests through the same const fns
  (plain, marked, the metadata rule, a bad marker panics);
  `engine/tests/cli.rs`, `index.rs`, `manual.rs` compare with
  `seldon::VERSION` and check it is the crate version plus
  `+SELDON_BUILD` when set, so `SELDON_BUILD=main.1a2b3c4 cargo test`
  checks the marked form end to end; `tests/plugin/model.test.js`:
  `versionCore`/`versionBelow`/`engineOutdatedBanner` read
  `0.1.3+main.1a2b3c4` as 0.1.3.
- **`scripts/deploy-test-host.sh` + `just deploy-test-host`** as the WP
  lists, details in docs/TESTING.md "Test host follows main":
  - host only from `SELDON_TEST_HOST`, a plain ssh alias, listed exactly
    in `scripts/guard-hosts.local` (`GUARD_HOSTS_FILE` overrides, as in
    guard.sh); also refused when the host's `/etc/machine-id` is this
    machine's; all exit 1 before any build or change;
  - refuses unless on `main`, clean (`git status --porcelain`, untracked
    included), HEAD = `origin/main`, the check log's last non-blank line
    is `exit 0` and the log is newer than the last commit touching
    `engine/` or `plugin/` (see Decisions 1);
  - builds with `SELDON_BUILD=main.<short sha>` and refuses to ship a
    binary that does not report exactly that version;
  - one ssh call installs: `seldon.new` → `seldon` by `mv`, the previous
    one as `seldon.prev`; HEAD's `plugin/` (`git archive`, so ignored
    files like `.qmlls.ini` never go out) by `rsync --checksum --delete`;
    a plugin dir without the dev marker (the release clone, or a plain
    copy) is moved once to
    `~/.local/state/seldon-dev/plugin-<git|copy|dev>-<UTC stamp>` —
    outside the plugins dir, because the shell loads every dir there and
    a second `jax.seldon` would clash; `.seldon-dev-build` holds
    `build=`, `commit=`, `deployed=`; `omarchy plugin validate` on the
    host copy;
  - restart only when the plugin files changed (hash of names + contents,
    `.git` and the marker left out) or a restart is still pending
    (`~/.local/state/seldon-dev/restart-pending`), after `ping` + 5 s,
    and only when `omarchy-shell lock status` says neither `locked`,
    `sessionLocked` nor `secure` (unreadable = locked); otherwise
    "restart pending", caught up by the next deploy;
  - smoke: `--version --json`, `doctor --json` exit 0 (degraded rows
    listed as notes), `capture --json` exit 0, `jax.seldon.service
    refresh` then `status` settles on `ok` + the new `engineVersion`,
    and no `restartNotice` (WP-090's field; absent counts as none);
  - one JSON line per deploy to `~/.local/state/seldon-dev/deploy.jsonl`
    (also when a step after the build fails), summary, exit 0/1/2;
  - `--dry-run`: refusals, one read-only probe, the plan (incl. whether
    the plugin files would change and whether a restart would run);
  - `--release vX.Y.Z`: warns about newer state (move
    `~/.local/state/seldon` aside first; it does not move it), downloads
    the release's `install.sh` + `SHA256SUMS` on the host, refuses a
    mismatch, clones `jax-seldon-plugin` at the tag and validates it,
    runs `install.sh --version vX.Y.Z --force` (`--force` because the
    dev binary is not in install.sh's manifest), moves the dev copy
    aside, puts the clone in place, then restart, smoke, log.
- **Hermetic test** `tests/deploy/deploy-test-host.test.sh` (`just
  check-deploy`, in `just check`, ~5 s): scratch repo + bare origin, a
  `cargo` stub, an `ssh` stub that runs the remote scripts under `env -i`
  with a scratch HOME and a whitelisted PATH (the dev host's real
  `omarchy-*` in `/usr/bin` are out of reach; `quickshell`, `hyprctl`,
  `systemctl`, `wtype` are trap stubs). 133 checks; the WP's refusals
  (unknown host, dirty tree, failed check log, locked shell) and every
  other branch listed in the TESTING.md row.
- Docs: TESTING.md (row + section), ORCHESTRATION.md §11 (one item),
  DEVELOPMENT.md (one paragraph), CHANGELOG (Engine line, new
  "Packaging and docs" section under Unreleased); memory: pitfalls,
  rust-notes, omarchy-shell.

## Not done

- **No real deploy** (brief: the first real deploy is the orchestrator's
  after the merge). The script never ran against the test host or any
  real ssh host; the remote side ran only under the stub.
- **shellcheck did not run**: it is not installed on the dev host (CI
  installs it). Both new scripts carry one file-wide
  `# shellcheck disable=SC2016` (the remote scripts are single-quoted on
  purpose); I removed the `-a`/`-o` tests (SC2166) and the `A && B || C`
  (SC2015) by hand. A CI branch dry run would show anything left.

## Verified by

- `flock /tmp/seldon-check.lock just check` on b1616e9 (working tree
  plus the memory notes, which no step reads): `check: ok`, `exit 0` —
  deploy-test-host.test 133/0, install.test 209/0, model.test.js 88,
  service-states 297/0, panel-view 771/0, overlay-view 319/0, bar-view
  143/0, docs-check ok (one existing warning: de/06 translation behind
  en/06, not touched here).
- `SELDON_BUILD=main.1a2b3c4 cargo test --lib --test cli --test manual
  --test index`: all green (201 / 17 / 25 / 7); plain: green.
- Binary: plain `seldon 0.1.3`; `SELDON_BUILD=main.1a2b3c4` →
  `{"name":"seldon","version":"0.1.3+main.1a2b3c4"}`; plain again →
  `seldon 0.1.3` (rebuild tracked); `SELDON_BUILD='main x'` → E0080
  "SELDON_BUILD must be semver build metadata".
- `bash tests/deploy/deploy-test-host.test.sh` → 133 passed, 0 failed;
  `VERBOSE=1` prints the dry run, a first deploy and a locked deploy.
- **Mutants, script** (each applied alone to `scripts/deploy-test-host.sh`,
  test run, original restored) — all 39 killed:
  host list ignored · prefix match on the host list · comments not
  stripped · host format check dropped · branch check dropped · clean
  check dropped · pushed check dropped · `exit 0` check dropped ·
  `exit 0` anywhere in the log · log age check dropped · machine-id
  check dropped · symlink check dropped · build without `SELDON_BUILD` ·
  binary version check dropped · dry run builds · no `seldon.prev` ·
  clone not moved aside · backup inside the plugins dir · rsync without
  `--delete` · marker not written · always restart · lock not checked ·
  `secure` ignored · `sessionLocked` ignored · unreadable lock = free ·
  flag cleared although the restart failed · pending flag set after the
  validation · host-side validation ignored · doctor exit ignored ·
  service `engineVersion` ignored · restart notice ignored · smoke
  failure exits 0 · no log line · release checksum not checked ·
  `install.sh` without `--force` · plugin cloned at the default branch ·
  release keeps the dev copy · no newer-state warning · plugin from the
  working tree instead of HEAD. (Three survived the first round —
  `--delete`, working tree vs HEAD, and a mis-anchored build mutant —
  and got cases: a removed/added plugin file, an ignored
  `plugin/.qmlls.ini`.)
- **Mutants, engine** (`--lib --test cli`, plain and with
  `SELDON_BUILD=main.1a2b3c4`): `.` instead of `+` · `..` allowed ·
  leading/trailing `.` allowed · `_` allowed · no validation in
  `version_bytes` · empty build gets `+` — all killed in both runs.
  `VERSION` ignoring `SELDON_BUILD` survives plain `cargo test` (the
  marked form is not built there) and is killed by the marked run and,
  at every deploy, by the script's binary version check (Decisions 2).

## Learned (in memory/)

- pitfalls: a stub first on PATH does not make a fake host safe (real
  `omarchy-shell`/`omarchy-restart-shell` also in `/usr/bin`; whitelist
  PATH under `env -i`); `just` runs recipes in the justfile dir (pass
  the check log absolute); keep a restart pending across a failed
  validation; the guard's package-command false positive on a read-only
  grep.
- rust-notes: `option_env!` in a const is rebuild-tracked; const string
  concatenation via `[u8; N]` + `from_utf8`.
- omarchy-shell: every dir under `~/.config/omarchy/plugins` with a
  manifest is a plugin (backups outside); `omarchy plugin update` needs a
  git checkout, a clone at a tag still fast-forwards.

## Decisions needed

1. **Which commit did the check log check?** The main check logs
   (`gates/main-check-NNN.log`) end in `exit N` but name no commit. The
   script takes the log's mtime as evidence and refuses when any commit
   touching `engine/` or `plugin/` is newer, so docs/bookkeeping commits
   after the check do not force a re-run. Stricter alternative: the
   orchestrator's check command writes a `head <sha>` line and the
   script requires it to equal HEAD (small follow-up in the script and
   in ORCHESTRATION). Recommendation: keep the mtime rule unless the
   orchestrator wants the exact match.
2. **Marked form in `just check`?** `just check` builds only the plain
   form. A second test pass with `SELDON_BUILD` would cost another full
   test build per check. Recommendation: no; the deploy refuses a binary
   without the exact marker, and the marked run is in this handover.
3. **Smoke when the test host has no graphical session** (operator
   logged out): `jax.seldon.service status` cannot answer, so the smoke
   fails and the deploy exits 2 (engine and plugin are installed, the
   restart stays pending). Recommendation: keep it strict; say so if a
   logged-out host should only warn.
4. **State on the way back:** `--release` warns but does not move
   `~/.local/state/seldon` (the WP says warn). A `--move-state` flag
   would be a few lines if the orchestrator wants it.

## Notes for the first real deploy (orchestrator)

- Run from the main checkout (it has `scripts/guard-hosts.local`; the
  worktrees do not): `SELDON_TEST_HOST=<alias> just deploy-test-host
  --dry-run /abs/path/main-check-NNN.log` first, then without
  `--dry-run`. Check the session is unlocked if you want the restart now.
- The first deploy moves the release clone to
  `~/.local/state/seldon-dev/plugin-git-<stamp>` and replaces
  `~/.local/bin/seldon`; install.sh's manifest on the host then no longer
  matches, which is why `--release` passes `--force`.
- The smoke's `seldon capture --json` writes to the logbook configured on
  the test host (`~/.config/seldon/config.toml` there).
- `just e2e` on the test host restores what it found, the deployed build
  included.

## Guard

- One block: a read-only `grep -n -i '…\|<package manager> -S\|run:'
  .github/workflows/ci.yml` was blocked as "privileged or package
  command" (the pattern text). Not worked around: I searched the same
  file for other words (`shellcheck`, `just check`). No other block.

## Touched outside WP scope

- `engine/tests/cli.rs`, `engine/tests/index.rs`,
  `engine/tests/manual.rs` (read `seldon::VERSION`, so both forms
  pass), `tests/plugin/model.test.js` (four asserts in the engineMin
  test; WP-090 edits this file too, in other places),
  `memory/{pitfalls,rust-notes,omarchy-shell}.md`. `main.rs`, redact.rs,
  capture.rs, doctor.rs, plugin/ and scripts/guard.sh unchanged.

## Round 2 (stage 1 SEND BACK, brief WP-098-round-2-brief.md)

### Done

- **B1** `cargo build … --release --features watch` (release.yml:121,
  PKGBUILD); the test asserts `--release --features watch --target
  x86_64-unknown-linux-musl` in the cargo stub's log. The dry run's plan
  line names it too.
- **S1** `rsync -rlp --checksum --delete --exclude=/.seldon-dev-build`.
  Test: an engine-only commit with a later commit time (git archive
  stamps every file with it) leaves an unchanged plugin file's mtime on
  the host as it was.
- **S2** `--target-dir "$root/engine/target"`. The cargo stub now
  honours `--target-dir`, else `CARGO_TARGET_DIR`, like cargo; a deploy
  with `CARGO_TARGET_DIR` exported ships the new marked build, and the
  log shows the target dir.
- **S3** The check log must start with `head <full sha>` (first line,
  40 hex), end in `exit 0`, the sha must be HEAD or an ancestor (`git
  merge-base --is-ancestor`), and `git diff --quiet <sha> HEAD --
  engine plugin scripts/deploy-test-host.sh` must hold. The mtime rule
  is gone. Tests: no head line, head line not first, a short sha, an
  unknown sha, a sha on another branch, a change after the sha to
  `engine/`, `plugin/` and the script (each refused), a docs-only commit
  after the sha (passes). Checked against the real
  `gates/main-check-118.log` format (first line `head f07d5d1a…`).
- **N1** While the restart step reports `pending (…)`, a `restartNotice`
  in the smoke prints `note restart notice while the restart is pending:
  …`; the summary still says "restart pending", exit 0. Without a
  pending restart the notice still fails the smoke (existing case).
- **N2** The read-only probe runs `command -v` on the tools of the mode
  (main: rsync jq tar mktemp install sha256sum find xargs omarchy
  omarchy-shell omarchy-restart-shell; release: curl git jq sha256sum
  find xargs omarchy omarchy-shell omarchy-restart-shell) and refuses
  with exit 1 naming the missing ones, before any build or change.
  Tests: no rsync, no omarchy (main), neither curl nor omarchy
  (release); host fingerprint unchanged, no build.
- **N3** o4: the first `shell ping` is before the restart in the call
  log, and a deploy with `SELDON_DEPLOY_SETTLE=2` takes ≥ 2 s; o5: a
  failing `capture --json` fails the smoke (exit 2, named); o7: covered
  by S3's `plugin/` case; o9: `--release` with a clone that fails
  validation exits 2, install.sh not run, the dev copy stays.
- **D3** The smoke first waits up to 10 s (or `SELDON_DEPLOY_WAIT`) for
  `omarchy-shell shell ping`; without an answer it reports `FAIL service:
  no graphical session (the shell does not answer ping)` and skips the
  60 s status poll; the summary says "smoke failed: no graphical session
  on <host> (nobody logged in?), so the plugin was not checked; engine
  and plugin are installed". Other smoke failures are now listed in the
  summary line too.
- **Q3** `--dry-run` prints `(ssh resolves it to <hostname>)` from `ssh
  -G -- <alias>` (no connection; checked locally with an `.invalid`
  alias). The ssh stub answers `-G` with `hostname 192.0.2.7`.
- Docs: TESTING.md section (refusals, build, rsync flags, smoke notes,
  dry run, absolute log path) and table row; CHANGELOG line.

### Verified by

- `bash tests/deploy/deploy-test-host.test.sh` → 164 passed, 0 failed.
- Mutants, round 2 — 38 of round 1 (anchors updated where the code
  moved; the mtime-rule mutant dropped with the rule) plus 20 new, 58 in
  all, each applied alone, the test run, the original restored — all
  killed: head line not required · ancestor check dropped · diff
  ignores `plugin` (o7) · diff ignores `engine` · diff ignores the
  script · diff over the whole tree (kills the docs-only pass) · no
  `--features watch` · no `--target-dir` · `rsync -a` again · notice
  fails although pending · notice never fails · remote tools not
  checked · rsync not in the tool list · smoke ping check dropped ·
  summary does not name the session · restart without the ping wait
  (o4) · restart without the settle sleep (o4) · capture exit not
  checked (o5) · release skips validating the clone (o9) · dry run
  without `ssh -G`.
- `flock /tmp/seldon-check.lock just check` on 63206cf, run 1: `exit 1`
  — one failure in `service-states` (296/1), "snapper-degraded: fix
  commands were" with the records `--`, `sudo setfacl …` out of order.
  That is the record-order flake WP-090(b) fixes on main; this branch
  does not touch `plugin/`, the harness or `fake-recorder`. Run 2,
  same commit, nothing changed: `check: ok`, `exit 0` —
  deploy-test-host.test 164/0, install.test 209/0, model.test.js 88,
  service-states 297/0, panel-view 771/0, overlay-view 319/0, bar-view
  143/0, docs-check ok (the existing de/06 warning).

### Decisions

- D2, D4 accepted as written; Q2 (machine-id pinning) not now — nothing
  changed for them.
- None new.

### Touched outside WP scope

- None beyond round 1.
