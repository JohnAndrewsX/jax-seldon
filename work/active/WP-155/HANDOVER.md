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

## Round 2

The orchestrator's answers to the open questions: (1) back up the
logbook and `~/.config/seldon` too, and say how to restore; (2) a backup
only on the switch to next; (3) stop an active `seldon-watch.service`
for the copy and start it again. Then, mid-round, the incident below and
four requirements. All done; commits ed114508 (script, test) and
f8c3906c (docs, pitfalls).

### Incident 2026-10-09: a hand mutant copied `/` into `/tmp`

- **What happened.** My round-2 mutant runner (a script in my Claude
  scratchpad under `/tmp`, making copies of the script and the test in
  the scratchpad) ran mutant `no-root-check`. It dropped the `$lb == /`
  part of the logbook guard. The test's row "logbook `/`: refused" then
  ran the script with a logbook of `/`, the probe no longer flagged it,
  and the install step ran `cp -a -- / <fake home>/.local/state/
  seldon-dev/backup-…/logbook` (the fake host is the dev host itself
  under `env -i` with a scratch HOME, so `/` was the dev host's real
  root). The test's temp dir was under `/tmp` (`mktemp -d`), so it
  filled the RAM-backed `/tmp` with 13 GB and the user's quota. The next
  mutant in the list (`unsafe-not-refused`) removed the refusal too and
  would have done the same.
- **Who stopped it.** The orchestrator killed the processes by PID
  (360469, 360470, 360576, 360578, 360585, 360598) and deleted
  `/tmp/tmp.akpwyMyeKH` by explicit path; `/tmp` back at 2 %. When I read
  the STOP, two runner processes of mine were still listed; by the time I
  looked again (a few seconds later) they were gone. My kill command for
  them (a compound command with a process-tree function) was blocked by
  the guard ("cannot check, nested too deep"); I did not redo it another
  way, and it was no longer needed. The test temp dirs of the killed
  runs were already gone; I removed my mutant leftover in the scratchpad
  by explicit path (80 KB).
- **Why.** The test of a guard around a copy executed the copy as soon
  as the guard was gone, against a real path, in RAM. I did not ask what
  each mutant makes the test run next. No data on the dev host was
  changed (a copy only reads its source), but the full `/tmp` could have
  broken the session (cf. the runtime-dir incident of 2026-10-08).
- **Fixes (the orchestrator's four requirements).**
  1. Tests: the default logbook is the fake home's `~/Seldon`; on the
     fake host `cp` and `du` are wrappers that refuse every path argument
     outside the test dir (logged to `trap.log`, which the final row
     requires empty). The rows for `/`, a parent of the home, the home,
     above `seldon-dev`, outside the home, a link out of it and a sibling
     that has the home as prefix assert the refusal (exit 1, no build,
     the host unchanged, nothing measured or copied). Verified: with the
     whole guard removed (mutant `safe-always`) the wrapper logged
     `du outside the test dir: /` and nothing was copied.
  2. Script: `logbook_safe` — the logbook's real path must be strictly
     below the home's real path and must not hold `seldon-dev`. Checked
     in the probe (refusal, exit 1, before any build or change; an unsafe
     logbook is not even measured) and again in the install step right
     before the copy (a test turns the logbook into a link out of the
     home between probe and install: exit 2, nothing copied).
  3. The test's temp dir is `<checkout>/target/deploy-test.XXXXXX` (disk,
     ignored), and the script under test gets `TMPDIR` inside it. Mutant
     copies live in `<worktree>/target/mutants`, one runner at a time
     (`flock`), `timeout 300` per mutant.
  4. Size cap: the probe measures what the backup would copy (`du -sck`
     of shell.json, `~/.config/seldon`, the state dir, the engine, the
     plugin dir and a safe logbook); above `SELDON_DEPLOY_BACKUP_MAX_MB`
     (default 1024) the deploy is refused with the size and the cap.
- Recorded in `memory/pitfalls.md` (2026-10-09 · WP-155) and in
  docs/TESTING.md.

### Changes

- Backup entries now: `shell.json`, `config-seldon/`, `state-seldon/`,
  `logbook/`, `seldon.engine`, `plugin-jax.seldon/`, `RESTORE.txt`. The
  logbook is the path the host's installed engine resolves
  (`seldon doctor --json` → `.logbook`, its real path); none found (no
  engine, no logbook dir) → no logbook entry, the dry run says
  "none found".
- `RESTORE.txt` (also printed in the summary, indented): deploy main (or
  `--release`) first, stop the watcher, then per entry that existed
  `mv <path> <path>.next && cp -a <backup>/<entry> <path>`, `cp -p` for
  shell.json, start the watcher, `omarchy-restart-shell`. The main
  deploy onto a next host points to the newest `RESTORE.txt`.
- An active `seldon-watch.service` is stopped before the copy and started
  again after it (before the engine swap); the existing watch step then
  restarts it on the new binary. A failed stop: exit 2, nothing copied.
  A failed start: exit 2 before the swap. A failed copy still starts it
  again.
- The probe of a next deploy reports what exists, the logbook and the
  size; `realpath` and `du` join the host's required tools for next.

### Verification

- `bash tests/deploy/deploy-test-host.test.sh`: 296 passed, 0 failed
  (243 in round 1).
- Hand mutants (`gates/mutants-wp155-r2.sh`, log
  `gates/mutants-wp155-r2.log`), on disk as above: 29 mutants, 29
  killed. `safe-home-prefix` (accept `/home/userX` for `/home/user`)
  survived the first pass; the sibling row was added and it is killed.
  The round-1 mutants were not run again.
- Full gate `gates/check-wp155-r3.log` at f8c3906c (private runtime dir
  `/tmp/r155`, 0700, removed afterwards; `SELDON_FULL_CHECK=1`): `check:
  ok`, exit 0 (deploy-test-host.test 296/0, docs-check ok, no harness
  skipped).
- shellcheck: still not installed here; CI runs it.

### Live on the test host

**Not run.** The deploy script did not run against the test host in
either round, and nothing on the test host was changed. A live
`--branch next` deploy (dry run included) needs this branch merged into
`next` and pushed: the script refuses anywhere else than a `next`
checkout whose HEAD is `origin/next` with the same script, and faking
that would route around the refusal.

What I did run there, read-only over ssh (no script, no writes):
- round 1: the plugin marker (`build=main.19c2f5e0`), `seldon-dev`
  (one hand backup `backup-before-next-20261007-173246` with
  `plugin-jax.seldon/`, `seldon.engine`, `shell.json`, `state-seldon/`),
  the last deploy log lines (main, smoke ok);
- round 2: `seldon doctor --json` → logbook `~/Seldon`; the size of what
  the backup would copy: shell.json 4 KiB, `~/.config/seldon` 4 KiB, the
  state dir 792 KiB, the engine 6.6 MiB, the plugin 824 KiB, the logbook
  1.9 MiB, 10 MiB in all (cap 1024 MiB); `seldon-watch.service`
  inactive.

After the merge into `next` and the push:
```
SELDON_TEST_HOST=<alias> just deploy-test-host --dry-run --branch next <next check log>
SELDON_TEST_HOST=<alias> just deploy-test-host --branch next <next check log>
```
The first deploy takes a backup (the host runs main) and prints
`RESTORE.txt`.

### Still open

- No CHANGELOG entry (see round 1; the suggested line should now also
  name the logbook and `~/.config/seldon`).
- A host without an engine (or whose engine cannot tell the logbook) is
  backed up without its logbook, said in the dry run but not refused.
  Refuse instead?

## Round 3

Stage-1 review (`review-0.1.1/handovers/WP-155-review-1.md`): SEND BACK
on B1 (CI red: the backup-failure rows used `chmod 000`, and CI runs the
test as root). The orchestrator's round-3 list: B1, N1, Q3, Q4, N4, N5,
N6, N3, Q5. Commits 231d0031 (script, test) and 3ab7f3f2 (docs).

- **B1.** The fake host's `cp` wrapper takes a switch: `$R/cp_fail`
  names a substring, and a `cp` whose *source* argument contains it fails
  (the destination is not matched, so `seldon-dev/…` does not trip it).
  The backup-failure scenario uses `.local/state/seldon` instead of
  `chmod 000` and now also checks that only the state dir failed (the
  config copied before it is in the partial backup). No row of the test
  depends on file permissions any more (grep for `chmod`, `-r`, `-w`,
  `id -u`: none left but `chmod 755` of stubs). I could not run the test
  as root here (no sudo); the switch does not depend on the user.
  Mutants `copy-fail-ignored` and `backup-fail-continues` (the review's
  M12, M25) are killed by these rows.
- **N1.** `$R/du_empty` makes the fake `du` print nothing: refused with
  "cannot tell how large the backup on <host> would be", no build, the
  host unchanged. Mutant `size-unknown` (M17) killed.
- **Q3.** A host whose engine does not name a logbook — no engine, or
  `seldon doctor --json` without `.logbook` — is refused before anything
  changes: "<host>'s engine does not name a logbook (seldon doctor
  --json, .logbook), so the backup would miss it; install or fix the
  engine there (a main deploy, or install.sh) until `seldon doctor
  --json` names the logbook, then deploy next again". A logbook the
  engine names that does not exist yet has nothing to copy; the dry run
  says "<path>, absent" (the release-host row).
- **Q4.** `RESTORE.txt` and the summary: first stop the watcher and put
  back the logbook, `~/.config/seldon`, the state dir and `shell.json`
  (next's copies move aside as `<path>.next`), then right away deploy
  main (or `--release`), whose smoke then writes into the restored
  logbook; then start the watcher again. A test row checks the order
  (stop < logbook < state < deploy < start). Note: between the restore
  and the main deploy the next engine and plugin still run on the
  restored state; "right away" says so, the watcher is stopped, but the
  shell's plugin can still call the next engine in that window.
- **N4.** A `--release` deploy onto a host that runs next warns like the
  main deploy and points to `RESTORE.txt` (before the deploy); it
  replaces the generic "move ~/.local/state/seldon aside" warning there.
  Row: a release dry run on the next host.
- **N5.** The unused `home=` probe output is gone.
- **N6.** The header's over-long line is wrapped; the versions read
  `X.Y.Z+main.<sha>` and `X.Y.Z+next.<sha>` (X.Y.Z from
  engine/Cargo.toml).
- **N3.** If a first next deploy fails after the engine swap but before
  the marker is written, the retry takes a second backup whose
  `seldon.engine` is already next's, and "the newest RESTORE.txt" then
  means that one; its logbook, config and state copies are still from
  before next unless the failed attempt wrote to them, so for a restore
  after such a failure take the oldest `backup-before-next-*` of that
  switch.
- **Q5.** Noted: the CHANGELOG line for `next` comes with the merge into
  `next` (suggested text in round 1; add the logbook and
  `~/.config/seldon`).
- **N2** (`mkdir` vs `mkdir -p` of the stamp dir, equivalent mutant): no
  change.

### Verification

- `bash tests/deploy/deploy-test-host.test.sh`: 310 passed, 0 failed.
  Temp dir under `target/` (disk), fake `cp`/`du` refuse paths outside
  it; `/tmp` at 2 % before and after.
- Hand mutants for the new and changed code
  (`gates/mutants-wp155-r3.sh`, log `gates/mutants-wp155-r3.log`; copies
  under the worktree's `target/mutants`, one runner under `flock`,
  `timeout 300` each): 12 mutants, 12 killed. No mutant of
  `logbook_safe` or the install-step recheck this round (unchanged code;
  killed in round 2 under the same wrappers).
- Full gate `gates/check-wp155-r4.log` at 3ab7f3f2 (private runtime dir
  `/tmp/r155`, 0700, removed afterwards; `SELDON_FULL_CHECK=1`): `check:
  ok`, exit 0 (deploy-test-host.test 310/0, docs-check ok, no harness
  skipped).
- shellcheck: not installed here; CI (the orchestrator pushes).

### Live on the test host

Not run, nothing changed there; same reason as in rounds 1 and 2 (the
script refuses anything but a `next` checkout at `origin/next`). No ssh
to the test host this round.
