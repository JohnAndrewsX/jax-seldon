# WP-212 — Handover

Branch `wp/212-guard-engine-files` (from `next` at `537817fb`), worktree
`wt/WP-212`. Not merged. Role: Plugin Dev. Tests and docs only; no
engine or plugin change.

## The engine's state files (from `engine/src`)

Every `state_dir.join(…)` in `engine/src`, by name:

| File | Where | Written by | Routine |
|---|---|---|---|
| `index.json` | `config.rs` | every index build | yes |
| `lock` | `config.rs` | every writing command | yes |
| `cursors.json` | `collectors/mod.rs` | the collectors of a capture | yes |
| `manifest.json` | `collectors/config.rs`, `MANIFEST_FILE` | the config collector of a capture | yes |
| `owned.json` | `collectors/config.rs`, `OWNED_FILE` | a capture (it uses up the engine's own writes) | yes |
| `recent-config.json` | `collectors/recent.rs`, `FILE` | the recent collector of a capture, about every 15 minutes | yes |
| `autocommit.json` | `index/autocommit.rs` | an index build with autocommit on | yes |
| `launches.json` | `commands/agent.rs`, `LAUNCHES` | `seldon agent` launches (operator's action) | no |
| `agent-launch.log` | `commands/agent.rs`, `LAUNCH_LOG` | the same | no |
| `proposals/` | `index/triage.rs`, `commands/triage.rs`, `DIR` | `seldon triage` (operator's or agent's action) | no |
| `hooks/` | `commands/setup.rs` | `init --theme-hook` | no |
| `hooks-user-wide` | `commands/hook.rs`, `MIGRATED_MARKER` | the hooks migration, once | no |

The engine's atomic writes also leave short-lived `.<name>.tmp-<pid>`
files, and a corrupt `owned.json` is renamed to `owned.json.bad`. Both
are new paths, and a new path still fails the guard (rule unchanged).

## What was done

- `tests/plugin/real-home-guard.sh`: `real_engine_files` now holds the
  seven routine files (`owned.json`, `recent-config.json` and
  `autocommit.json` are new), each with the source it comes from. A new
  list, `real_engine_not_routine`, holds the five others. A change to
  one of those during a run still fails. The guard's other rules
  (config.toml, no path appearing or disappearing, the logbook and
  machine in the index, the runtime dir) are unchanged.
- `tests/plugin/real-home-guard.test.sh`:
  - New rows: for each of `manifest.json`, `owned.json`,
    `recent-config.json` and `autocommit.json`, the file rewritten alone
    (the dev-194 case) and together with the index → live. All four
    rewritten together → live. FAIL rows for a changed non-engine file
    (`calls.log`), a look-alike (`recent-config.json.bak`), a
    not-routine engine file (`launches.json`) and `recent-config.json`
    appearing. `guard_case` takes an optional 6th argument: the state
    files that exist before the run.
  - The drift test: `engine_state_names` reads every
    `state_dir.join(…)` in the Rust sources. For a literal it takes the
    value. For a constant it takes the `const` of that name in the same
    file, or else the single value of that name anywhere in the tree.
    Anything it cannot resolve is reported as unresolved.
    `drift_case` compares that set with the guard's two lists (read by
    sourcing the guard in a scratch HOME and a scratch session dir) in
    both directions. It fails on a name the engine writes that the guard
    knows in neither list, on an unresolved argument, and on a name the
    guard lists that the engine no longer uses.
  - Proven on copies of `engine/src`: a new literal state file, a new
    constant, a `join(name)` with a variable, and `autocommit.json`
    renamed. Each one fails and names the file. The real tree passes
    with 12 names.
- `docs/TESTING.md`: the guard paragraph names the seven routine files
  and describes the drift test.

## How it was verified

- **fixture** (dev host, scratch HOMEs and runtime dirs under the
  private gates folder): `bash tests/plugin/real-home-guard.test.sh`
  71 passed, 0 failed.
- **fixture**: shellcheck on both scripts. 4 findings in the test and 1
  in the guard, all already on `537817fb` (SC2016 and SC2001 on the old
  case script, SC2088 on the tilde rows). None new.
- **desktop** (dev host running the released 0.2.0, private 0700
  `XDG_RUNTIME_DIR` removed afterwards, `TMPDIR`, `JUST_TEMPDIR` and
  `CARGO_TARGET_DIR` under the private gates folder):
  `flock /tmp/seldon-gate.lock just check` with `SELDON_FULL_CHECK=1`
  at `61e75c3a`. It ran from 11:04 to 11:31 with `check: ok`, exit 0, no
  `FAIL` line. A live capture landed during service-states:
  `ok   service-states: the real ~/.local/state/seldon changed by the
  operator's live engine (not a leak)` (367 passed). desk-view (2338),
  bar-view (196) and ipc-restart (44) passed with the real paths
  untouched. The guard does not print which files a passing change
  touched, and the real directory was not inspected, so it is not
  recorded which state files that capture rewrote.
- The first full check, at `d7f684f8`, failed in `check-runtime-dir`:
  the new guard-list line named `XDG_RUNTIME_DIR` without the
  `# live runtime dir:` marker on the same line. It was fixed in
  `61e75c3a`. `just check-runtime-dir` alone: 45 passed.

## What was not done / open questions

- The failure in `gate-n020r.log` is a different case.
  `config.toml` changed, and `autocommit.json` and `recent-config.json`
  appeared during the run (an install or `init` on the dev host while
  the gate ran). The guard still fails that by design, since a config
  change or a new path is not a routine capture. A first capture after
  installing 0.2.0, which creates `recent-config.json`, also fails it
  once. If that should pass, it needs an orchestrator decision on the
  "no path appears" rule, which this WP was told not to change.
- The drift test reads only `state_dir.join(…)` on one line. A state
  path built some other way, or a `join` split across lines, is not
  seen. Today `engine/src` has neither.
