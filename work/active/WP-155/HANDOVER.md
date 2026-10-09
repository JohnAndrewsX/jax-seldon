# WP-155 — handover

Branch `wp/155-deploy-next` from `main` (614de740). Not pushed (the
orchestrator pushes and opens the draft PR for CI). `git merge-tree`
against `origin/next` (928e9430): clean, no conflicts.

## Done

- `scripts/deploy-test-host.sh [--dry-run] [--branch main|next] CHECK_LOG`.
  `--branch` defaults to `main`, so every existing call is unchanged.
  `--branch next`: the same refusals with `next` for `main` (on `next`,
  clean, HEAD equals `origin/next`, next's check log with `head <sha>` HEAD
  or an ancestor, no change under `engine/`, `plugin/`, `schema/` or the
  script since, `exit 0` last, no skipped harnesses; the host list, pin,
  symlink and missing-tool refusals unchanged). Refused as well: a
  `--branch` value other than `main`/`next`, `--branch` without a value,
  `--branch` with `--release`.
- Build `SELDON_BUILD=next.<short>` → `X.Y.Z+next.<short>`; marker
  `build=next.<short>`; log line `mode: "next"`.
- Backup `~/.local/state/seldon-dev/backup-before-next-<UTC stamp>/` with
  whichever exist: `shell.json`, `state-seldon/` (`~/.local/state/seldon`),
  `seldon.engine` (`~/.local/bin/seldon`), `plugin-jax.seldon/` (the
  plugin dir, `.git` included). The names follow the orchestrator's backup
  by hand on the test host (`backup-before-next-20261007-173246`), which I
  looked at read-only; the stamp follows the script's own UTC stamps
  (`20261009T120000Z`), as the `plugin-<kind>-<stamp>` dirs do.
  - Taken on the host as the first step of the install, before the engine
    swap, and only when the host does not run a next build yet (its
    plugin marker is not `build=next.*`). Next onto next takes none.
  - A failed backup stops the deploy before any other change: exit 2, the
    log line names the failure and the partial backup dir.
  - The summary and the log line (new field `backup`, empty otherwise)
    name it; the dry run says whether one would be taken.
- A main deploy onto a host that runs next warns that next's state may
  not load in main and names `backup-before-next-*/state-seldon` (it does
  not restore anything).
- Docs: script header, justfile comment, docs/TESTING.md ("Test host
  follows main", the check-deploy row, the log fields), ORCHESTRATION.md
  §11, DEVELOPMENT.md.

## Verification

- `bash tests/deploy/deploy-test-host.test.sh`: 243 passed, 0 failed (191
  before; 52 new rows for `--branch next`: the refusals above, the dry
  run, next over a main build with every backup entry checked against
  the state before the swap, next onto next, next onto a release, a
  failed backup and its retry, main onto next).
- Hand mutants (a sed per mutant on a scratch copy of the script, the
  test run against it): 26 distinct valid mutants, 26 killed, 0
  survived (the four backup-entry mutants run again after the rename,
  killed again). Among
  them: branch/upstream/value/--release refusals dropped, backup never
  or always or on main, each backup entry dropped, a failed backup
  ignored, `main.` in build, version or marker, the warning dropped or
  shown on main onto main, the skip message, the log/summary/abort
  backup field, the dry-run lines. Three first attempts that deleted a
  line were syntax errors (invalid kills) and were redone as no-op
  replacements.
- `tests/plugin/runtime-dir.test.sh`: 42 passed (the script's two
  `# live runtime dir` lines unchanged).
- Full gate: `XDG_RUNTIME_DIR=/tmp/r155 SELDON_FULL_CHECK=1
  JUST_TEMPDIR=<scratch> flock /tmp/seldon-check.lock just check`,
  private runtime dir created 0700 and removed afterwards, cargo target
  the worktree's `engine/target`:
  - `gates/check-wp155-r1.log` at 8b51f404: `check: ok`, exit 0.
  - `gates/check-wp155-r2.log` at 33d5569f (HEAD before this handover):
    `check: ok`, exit 0 (deploy-test-host.test 243/0, no harness skipped).
- **shellcheck is not installed here**: the test fell back to `bash -n`.
  CI runs shellcheck; the WP needs a green CI run before the merge.

## Not done

- **Live deploy of `next`** to the test host: not possible before this
  WP is in `next`. The script refuses unless the checkout is on `next`
  with HEAD equal to `origin/next` and the check log's commit has the
  same script; a checkout that fakes `origin/next` would route around
  exactly that refusal, so I did not build one. After the merge into
  `next` and the push, the orchestrator (or I, on request) runs:
  ```
  SELDON_TEST_HOST=<alias> just deploy-test-host --dry-run --branch next <next check log>
  SELDON_TEST_HOST=<alias> just deploy-test-host --branch next <next check log>
  ```
  The test host runs `0.1.4+main.19c2f5e0` now (marker `build=main.*`),
  so the first next deploy takes a fresh backup beside the one by hand.
- CHANGELOG: not touched. `main`'s `[Unreleased]` is empty and `next`'s
  starts at the same line, so an entry here would conflict on the merge
  into `next`. Suggested line for `next`'s "Packaging and docs":
  "`just deploy-test-host --branch next <log>` deploys the next build to
  the test host (`+next.<sha>`), with a backup of shell.json, the state
  dir, the engine and the plugin before the host first runs next
  (WP-155)."

## Open questions

1. **The logbook is not in the backup.** The WP lists shell.json, the
   state dir, the engine and the plugin. Next writes contract-2 ledger
   kinds into the test host's logbook that a 0.1.x engine skips
   ("going back is not supported", CHANGELOG/WP-120), and
   `~/.config/seldon/` is not in it either. Add both to the backup, or is
   the test logbook expendable?
2. **Backup once per switch** (only when the host does not run next yet),
   not on every next deploy. I read "backup-before-next" as the way back
   to main; a backup of next's own state would not help that. Say if
   every next deploy should back up.
3. The state dir is copied while `seldon-watch.service` may run (the test
   host's unit is inactive per its deploy log). Stop the watcher around
   the copy?
