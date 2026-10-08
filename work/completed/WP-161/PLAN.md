# WP-161 — Plan

Branch `wp/161-runtime-dir` from `main`. Input: `work/queued/WP-161.md`,
the operator's incident report of 2026-10-08 (private folder).

## Findings on `main`

Quickshell starts under `tests/` and `scripts/` and the runtime dir each
one gets:

| Where | Runtime dir today | Meant to reach a live shell? |
|---|---|---|
| `tests/plugin/panel-view.sh` | `${XDG_RUNTIME_DIR:-$work}`, i.e. the session's | no |
| `tests/plugin/overlay-view.sh` | same | no |
| `tests/plugin/service-states.sh` (both `run` and `sheets_run`) | inherited, i.e. the session's (no `env -i`) | no |
| `tests/integration/e2e.sh --engine-only` (headless `Service.qml`) | inherited, the session's | no |
| `tests/plugin/bar-view.sh` | `$work`; IPC cases a short `/tmp/seldon-ipc.*` removed by hand, not by the trap | no |
| `tests/integration/e2e.sh` remote prelude | `/run/user/<uid>` on the test host | yes (test host's live shell) |
| `scripts/deploy-test-host.sh:183` remote prelude | same | yes |

`desk-view.sh` exists only on `next`; it has the same bug (line 85 there).

## Steps

1. **Private runtime dir in every harness.** Each harness that starts
   Quickshell makes one short private dir next to `$work`:
   `rt=$(mktemp -d /tmp/seldon-rt.XXXXXX)`, `chmod 700 "$rt"`, removed by
   the existing EXIT trap, and passes `XDG_RUNTIME_DIR="$rt"` to every
   Quickshell. Under `/tmp` and not under `$work` because `$work` follows
   `TMPDIR`, which may be long; the IPC socket path
   (`<rt>/quickshell/by-id/<id>/ipc.sock`) must stay under 108 bytes
   (the reason `bar-view.sh` already uses `/tmp/seldon-ipc.*`).
   `bar-view.sh`: its per-case IPC dirs are also removed by the trap.
2. **Live-shell exceptions stay, with a comment.** `scripts/deploy-test-host.sh`
   and the remote prelude of `tests/integration/e2e.sh` keep the test
   host's `/run/user/<uid>`; each such line carries the marker comment
   `# live runtime dir:` with the reason.
3. **Static guard** `tests/plugin/runtime-dir.test.sh`: fails on any
   `${XDG_RUNTIME_DIR:-…}` (also `-`, `=`, `:=`), any `$XDG_RUNTIME_DIR`
   expansion and any `/run/user` under `tests/` and `scripts/` unless the
   line carries the marker. Self-test: mutants of the four harnesses and
   `e2e.sh` with `${XDG_RUNTIME_DIR:-$rt}` brought back, a dropped marker,
   and an `env` without `XDG_RUNTIME_DIR` must each fail; the clean copies
   pass. A new recipe `check-runtime-dir` runs it with `shellcheck` (CI
   too; no host tools needed).
4. **Dynamic guard** in `tests/plugin/real-home-guard.sh`: snapshot the
   entries of `quickshell/by-id` in `/run/user/$UID` and in the inherited
   `$XDG_RUNTIME_DIR` (when different) before the run; `real_home_check`
   adds one line per dir with the counts before and after and fails when
   a new entry appeared. `real-home-guard.test.sh` gets cases for it in a
   scratch runtime dir.
5. **E29, less load.** `plugin-test` runs `service-states.sh`,
   `panel-view.sh`, `overlay-view.sh` and `bar-view.sh` only when
   `plugin/`, `tests/plugin/`, `schema/` or `fixtures/` changed against the
   merge base with `main` (committed, staged, unstaged or untracked), or
   when `SELDON_FULL_CHECK=1`; otherwise it prints why it skipped them. No
   git or no merge base: they run.
6. **Runtime space preamble.** A first `check` step `check-runtime-space`:
   `df` of `/run/user/$UID`, warning above 50 %, refusing above 80 %;
   skipped with a notice when the dir does not exist (CI).
7. **Docs.** `docs/TESTING.md`: the table rows, the skip rule and
   `SELDON_FULL_CHECK`, the runtime-dir rule for harnesses, both guards.

## Verification

- `df -h /run/user/$UID` before each check, stop if over 50 %.
- Count `/run/user/$UID/quickshell/by-id` before and after `just check`
  (with `SELDON_FULL_CHECK=1`, so the harnesses run), run under
  `flock /tmp/seldon-check.lock` with `XDG_RUNTIME_DIR` set to a private
  `/tmp/rNNN` and `JUST_TEMPDIR` in the scratch dir.
- Mutant run of the static guard (step 3) in the check output.
- Push, CI green (shellcheck runs there).

## Not in this WP

- `desk-view.sh` on `next`: after this lands on `main` and `main` is merged
  into `next`, the static guard fails on it until it gets the same fix
  (the handover gives the exact change).
