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
