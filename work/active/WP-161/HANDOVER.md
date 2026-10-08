# WP-161 — Handover

Branch `wp/161-runtime-dir` from `main`. Not pushed by the dev (the guard
hook blocked `git push -u`; the orchestrator pushes and watches CI).

## Done

- **Private runtime dir for every test Quickshell.** `panel-view.sh`,
  `overlay-view.sh`, `service-states.sh` (both `run` and `sheets_run`; it
  had no `env -i` and inherited the session's dir) and the headless run
  of `tests/integration/e2e.sh --engine-only` each make
  `rt=$(mktemp -d /tmp/seldon-rt.XXXXXX)`, `chmod 700`, remove it in the
  EXIT trap and pass `XDG_RUNTIME_DIR="$rt"`. `bar-view.sh` uses the same
  `$rt` (it used `$work`); its two IPC cases get `$rt/ipc*` instead of
  separate `/tmp/seldon-ipc.*` dirs, so the trap removes them too.
  Under `/tmp`, not under `$work`, because `$work` follows `TMPDIR` and
  the IPC socket path must stay under 108 bytes (deviation from the WP's
  "`$work` or a subdir of it"; reason in PLAN.md and TESTING).
- **Live-shell exceptions kept and marked.** `scripts/deploy-test-host.sh`
  (prelude, line 185) and the remote prelude of `tests/integration/e2e.sh`
  keep the test host's `/run/user/<uid>`, with a comment and the marker
  `# live runtime dir: …`.
- **Static guard** `tests/plugin/runtime-dir.test.sh`, new recipe
  `check-runtime-dir` in `check` (runs in CI; shellcheck when installed).
  Under `tests/` and `scripts/`, outside comments: any `XDG_RUNTIME_DIR`
  expansion (`:-`, `-`, `=`, plain) and any `/run/user` fails unless the
  line has the marker; every Quickshell started with `-p`/`--path` must
  set `XDG_RUNTIME_DIR` in the same command. Mutants (24 cases): the old
  `${XDG_RUNTIME_DIR:-$rt}` back in each of the five files, the session's
  dir passed on, the setting dropped (service-states, e2e, panel-view), a
  literal `/run/user/1000`, a marker dropped (deploy, guard); each must be
  caught for the right reason, the unchanged copies must pass.
- **Run guard** in `tests/plugin/real-home-guard.sh`: lists
  `quickshell/by-id` in `/run/user/$UID` and in the inherited
  `XDG_RUNTIME_DIR` (when different) before a harness; `real_home_check`
  adds one line per dir with the counts before and after and fails on a
  new entry no running process holds open (`/proc/*/fd`). New entries a
  live process holds (another Quickshell app, a restarted shell) are only
  noted: the first full check would otherwise have failed on two
  `qs -p /usr/share/flea/…` instances that started mid-run.
  `real-home-guard.test.sh`: 9 new cases (new, first, gone, swapped,
  live, live plus leftover, inherited dir untouched and grown).
- **E29:** `plugin-test` runs the four Quickshell harnesses only when
  `plugin/`, `tests/plugin/`, `schema/` or `fixtures/` changed against the
  merge base with `main` (committed, staged, unstaged, untracked), or with
  `SELDON_FULL_CHECK=1`; otherwise a skip notice. No git or no merge base:
  they run. `docs/ORCHESTRATION.md` G2 now says gates and the main check
  use `SELDON_FULL_CHECK=1`.
- **Space preamble** `check-runtime-space`, first in `check` and a
  dependency of `plugin-test`: `df -h /run/user/<uid>` and the by-id count,
  warns above 50 %, refuses above 80 %, skipped without the dir (CI).
- **Docs:** `docs/TESTING.md` new section "The session's runtime dir", the
  table rows, the isolation paragraph of layer 2.

## Verification

Full check, `SELDON_FULL_CHECK=1`, under `flock /tmp/seldon-check.lock`,
`XDG_RUNTIME_DIR=/tmp/r161` (mode 700), `JUST_TEMPDIR` in the scratch
dir, by-id counts taken inside the lock. Ran at `29f78e52`; `6d09bed6` only
adds a shellcheck directive comment to `runtime-dir.test.sh`, and that
file's on-disk content during the run already had it.

```
by-id before: 324
check-runtime-space: /run/user/<uid> is 2 % full, 324 entries in quickshell/by-id
install.test: 229 passed, 0 failed
deploy-test-host.test: 190 passed, 0 failed
runtime-dir.test: 24 passed, 0 failed
real-home-guard.test: 40 passed, 0 failed
ok   service-states: no leftover in /run/user/<uid>/quickshell/by-id (324 before, 324 after)
ok   service-states: no leftover in /tmp/r161/quickshell/by-id (0 before, 0 after)
service-states: 332 passed, 0 failed
panel-view: 925 passed, 0 failed     (same two runtime lines, 324/324 and 0/0)
overlay-view: 328 passed, 0 failed   (same)
bar-view: 196 passed, 0 failed       (same)
plugin-test: ok
check: ok
by-id after: 324
exit 0
```

No `/tmp/seldon-rt.*` left after the run; `/tmp/r161` empty. `df` of the
session runtime dir before every check: 1–2 %.

The first full check (at `d19947e1`) failed in `check-deploy`: the marker
comment "the test host's shell" inside the single-quoted remote preludes
ended the quote (`shell: command not found`). Fixed in `d6d0b076` (also in
`e2e.sh`, which `check` does not run).

## Not done / open

- **CI not run** (no push; the orchestrator pushes). shellcheck is not
  installed on the dev host, so `runtime-dir.test.sh` has never been
  through shellcheck; CI is the first time.
- **E29 skip path not exercised**: this branch changes `tests/plugin/`, so
  only the "run" branch ran. The skip condition was reviewed, not run.
- **`next`**: after main is merged into next, `check-runtime-dir` fails
  there on two files until they get the same fix:
  - `tests/plugin/desk-view.sh` line 85 `XDG_RUNTIME_DIR="${XDG_RUNTIME_DIR:-$work}"`
    → the `rt` block as in `panel-view.sh` and `XDG_RUNTIME_DIR="$rt"`;
  - `tests/plugin/graph-live.sh` line 37 starts `"$qs_bin" -p` with the
    inherited (session) runtime dir; same fix. Not in the WP's list.
- **`just` itself** writes its shebang recipe scripts under
  `$XDG_RUNTIME_DIR/just/` unless `JUST_TEMPDIR` is set (removed after each
  recipe; documented in TESTING). My first `just` call ran without it (the
  dir is empty).
- **Incident during this WP**: between 10:11 and 10:28 another run added
  ~317 leftover entries to the real by-id (7 → 324, 48 MB, 2 %); not from
  this branch. The orchestrator traced it to a WP-156 desk-view run. The
  entries are still there; I did not remove anything.
- Guard hook blocks reported, not routed around: `git push -u` and a
  `source <(…)` ad-hoc check (replaced by reason checks in the mutant
  test itself).

## Round 2

Review 1 (stage 1, SEND BACK). `main` (1afb1aef) merged first, no conflict.

- **B1** (CI red, shellcheck): `sed 's/^/     /' <<<"$out"` instead of
  `echo | sed` (SC2001, both places), `got="unset"` / `got="session"` /
  `got="trap"` (SC2209). shellcheck is still not installed on the dev
  host; one more spot that could have tripped it (`"\$""{qs_bin}"`) is now
  a single-quoted variable. CI is the first shellcheck run.
- **B2** (no silent skip on main): the harnesses also run when the merge
  base is `HEAD` (on `main`, a detached `main`, a branch without its own
  commit). `deploy-test-host.sh` refuses a check log containing
  `Quickshell harnesses skipped` (new case in `deploy-test-host.test.sh`,
  191/0); the skip notice says so. The condition, copied out of the
  justfile into a scratch clone whose `main` is this branch (it only
  prints RUN or SKIP, starts nothing):

  | Case | Result |
  |---|---|
  | main clean | RUN |
  | detached main | RUN |
  | fresh branch, no commit | RUN |
  | branch with a docs-only commit | SKIP |
  | + unstaged `justfile` change | RUN |
  | + untracked file under `fixtures/` | RUN |
  | docs-only with `SELDON_FULL_CHECK=1` | RUN |

- **N1** (wider static rules): a start is `"$qs_bin"`, `${qs_bin}`,
  `$qs_bin`, `quickshell` or `qs` with `-p`, `--path` or `--path=`. Any
  mention of the name `XDG_RUNTIME_DIR` that is not an assignment
  `NAME=…` needs the marker: catches `"$(printenv XDG_RUNTIME_DIR)"`,
  `v=XDG_RUNTIME_DIR` for `${!v}`, `${XDG_RUNTIME_DIR:=…}`. (A `${!v}`
  whose name is assembled from pieces stays out of reach; the per-run
  guard covers it.) `real-home-guard.test.sh` names it in `env -u`; that
  line now carries the marker (its inner script moved into a variable so
  the marker sits on a shell line, not inside the quoted script).
- **N2**: a file that makes `rt=$(mktemp …)` must remove `"$rt"` in an
  `EXIT` trap; mutants: bar-view, panel-view, e2e trap without `"$rt"`.
- **N3**: `justfile` is a skip path.
- **N5**: TESTING says exactly when `check-runtime-space` runs (once per
  `just` invocation: at the start of `check`; first in `just plugin-test`
  alone).
- **AGENTS.md §6**: wording kept. TESTING: engine and cargo tests inherit
  `XDG_RUNTIME_DIR` but the engine never reads it; `runtime-dir.test.sh`
  proves it (no `*.rs` under `engine/` names the variable, outside
  `target/`) and fails if one ever does.
- `runtime-dir.test.sh` now 36 cases (24 → 35 mutants, plus the engine
  check).

### Verification

Full check at `81ccc950`, `SELDON_FULL_CHECK=1`, `flock
/tmp/seldon-check.lock`, `XDG_RUNTIME_DIR=/tmp/r161` (0700),
`JUST_TEMPDIR` in the scratch dir; by-id listed inside the lock.

```
lock taken 13:54:04
tmpfs 3.2G 50M 3.1G 2% /run/user/<uid>
by-id before: 327
install.test: 229 passed, 0 failed
deploy-test-host.test: 191 passed, 0 failed
runtime-dir.test: 36 passed, 0 failed
terminal-scripts: 65 passed, 0 failed
real-home-guard.test: 40 passed, 0 failed
ok   service-states: no leftover in /run/user/<uid>/quickshell/by-id (327 before, 327 after)
service-states: 332 passed, 0 failed
panel-view: 925 passed, 0 failed     (327 before, 327 after)
overlay-view: 328 passed, 0 failed   (327 before, 327 after)
bar-view: 196 passed, 0 failed       (327 before, 327 after)
plugin-test: ok
check: ok
by-id after: 327
by-id lists identical
exit 0
```

No `/tmp/seldon-rt.*` left; `/tmp/r161` empty. The session's by-id went
from 324 (round 1) to 327 between the two checks, outside any run of this
branch. Not pushed (the orchestrator pushes and watches CI).

Still open from round 1: the `next` follow-up (`desk-view.sh`,
`graph-live.sh`), and `deploy-test-host` still accepts a log with
`plugin-test: skipped (SELDON_SKIP_HOST_CHECKS set …)` (not asked; one
line if wanted).

## Round 3

CI on PR #9 still red on shellcheck SC2001 only (`sed 's/^/     /' <<<"$out"`
at `runtime-dir.test.sh` 97, 105, 139). The three indents are now one
helper `indent`, a plain `while IFS= read -r` loop with `printf`; no sed
on a variable is left in the file (the one `sed` left reads the mutant's
file). Verified locally: `bash -n` ok, `runtime-dir.test: 36 passed,
0 failed`, the helper's output checked by hand. shellcheck still only in
CI. No full check rerun (one test file, failure-path output only).
