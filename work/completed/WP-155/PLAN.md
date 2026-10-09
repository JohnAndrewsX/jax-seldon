# WP-155 — plan

Branch `wp/155-deploy-next` from `main`; merges into `main` first, then
into `next`. `scripts/deploy-test-host.sh` and its test are identical on
both branches today, so one change fits both.

## Interface

```
scripts/deploy-test-host.sh [--dry-run] [--branch main|next] CHECK_LOG
scripts/deploy-test-host.sh [--dry-run] --release vX.Y.Z
```

`--branch` defaults to `main`; anything else than `main` or `next` is
refused, and so is `--branch` with `--release`. `just deploy-test-host
--branch next <log>` works through the existing `*args` recipe.

## The next mode

- Same refusals, with the branch as a variable: on `next`, clean, HEAD
  equals `origin/next`, the check log's `head <sha>` is HEAD or an
  ancestor, no change under `engine/`, `plugin/`, `schema/` or the script
  since, `exit 0` last, no "Quickshell harnesses skipped". The host list,
  the machine-id pin, the symlink and missing-tool refusals are unchanged.
- Build `SELDON_BUILD=next.<short>` → version `X.Y.Z+next.<short>` (the
  engine accepts any semver build metadata, `engine/src/lib.rs`). The
  marker says `build=next.<short>`; the log line says `mode: "next"`.
- **Backup** `~/.local/state/seldon-dev/backup-before-next-<UTC stamp>/`
  with whichever of these exist: `shell.json` (`~/.config/omarchy/`),
  `state/` (`~/.local/state/seldon`), `seldon` (`~/.local/bin/seldon`),
  `plugin/` (the plugin dir, `.git` included). Taken on the host before
  the first change (engine swap), and only when the host does not run a
  next build yet (its marker is not `build=next.*`): the backup is the
  way back from next, and a second one from next to next would hold next
  state. A failed backup aborts the deploy before anything changes
  (exit 2, logged). The log line and the summary name the backup.
- Dry run: the build line names `next.<short>`, and a `backup` line says
  whether a backup would be taken.
- `main` onto a host that runs next: a warning that next's state may not
  load in main, naming the backup dir to restore from. No refusal.

## Tests (tests/deploy/deploy-test-host.test.sh)

A `next` branch in the scratch repo, pushed to the bare origin.
- Refusals for `--branch next`: not on next (on main), HEAD not
  `origin/next`, no `origin/next`, a dirty tree, a log of main's HEAD
  that is not an ancestor of next, a plugin change since the checked
  commit, `--branch` with a bad value, `--branch` without a value,
  `--branch` with `--release`, `--branch main` while on next.
- Dry run: version `+next.<short>`, the backup line, nothing changed.
- First next deploy over a main build: version, marker, log line
  (`mode` next, `backup`), backup dir with shell.json, state dir, engine
  (the main build), plugin (the main dev copy); summary names it.
- Second next deploy: no new backup.
- A host without shell.json or state dir: backup of what exists.
- Backup fails (unwritable seldon-dev): exit 2, engine unchanged.
- Back to main from next: the warning, the deploy works.
- The existing rows stay green (main is the default).

## Docs

Script header, justfile comment, docs/TESTING.md ("Test host follows
main" → main or next, the check-deploy row), docs/ORCHESTRATION.md and
docs/DEVELOPMENT.md if they say "main only", CHANGELOG.

## Checks

- `bash tests/deploy/deploy-test-host.test.sh`, shellcheck locally if
  installed (CI otherwise), hand mutants against the new refusals and
  the backup.
- Full gate: `SELDON_FULL_CHECK=1 just check` with a private runtime dir.
- Live: `--dry-run --branch next`, then a real deploy of `next` to the
  test host with a next gate log, and back to main if the orchestrator
  wants the test host on main (open question in the handover).
