# WP-205 — Handover

Branch `wp/205-today-live-wait` (from `next` at `3ba46b8f`), worktree
`wt/WP-205`. Not merged. Role: Plugin Dev. Tests only; no plugin change.

## The race

A live case (`run <case> ""`) starts with no index file; the fake
engine's start-up `--version`, `capture`, `status` write it, and the
service's `FileView` loads it after that. The harness starts step 1 once
the service is `ready` — but `ready` is also true while the file is
*missing*, so step 1 can run on no index. On an idle host the index
lands about when step 1 runs; under load it lands later. `today-live #1`
then saw `Model.caseOptions(null)` (the "No case" option alone, so the
journal field hides its picker), as in gate n202m; `drift-only #1` lost
its `summon:<selection>` (the row does not exist yet), as in gate n194b.

## What was done

- `tests/plugin/harness/desk.qml`: `HARNESS_WAIT_INDEX` (opt-in). When
  set, the first step also waits until the service holds an index, in
  the same up-to-15 s start wait as before. If none arrives, it logs
  `HARNESS wait timed out: index`, which `clean_log` fails. The header
  lists it.
- `tests/plugin/desk-view.sh`: `HARNESS_WAIT_INDEX=1` on 53 `run`
  lines, 54 live cases (only the `run` lines change; no step numbers move):
  today-resolve, today-live, today-refuse, drift-live, drift-only,
  drift-already, drift-locked, drift-show, drift-show-member, work-live,
  work-agent, work-reopen, held-key, import-live, import-refused,
  import-reask, import-hidden, tab-focus, cursor-follow, why-loud,
  why-loud-bare, why-loud-planned, why-loud-all, why-loud-group,
  why-loud-truncated, case-gone, aside, aside-stale, aside-gone,
  aside-double, aside-nowindow, aside-index, aside-openfail,
  aside-focused, decisions-accept, decisions-accept-refused,
  decisions-accept-index, decisions-accept-locked, system-watch,
  system-watch-locked, system-watch-last, triage, triage-refused,
  triage-ask, triage-none, triage-keys, triage-keys-stacked,
  triage-detour, triage-detour-stacked, triage-swap, triage-swap-apply,
  triage-swap-discard, keys-ask, keys-ask-stacked.
- `tests/plugin/fake-seldon`: `FAKE_SELDON_FIRST_WRITE_DELAY` (off by
  default): the first `status` in mode ok waits that many seconds before
  it writes the index (`$HOME/first-write` marks it done).
- `tests/plugin/desk-view.sh`, `run`: under `SELDON_FULL_CHECK=1` every
  live run gets `FAKE_SELDON_FIRST_WRITE_DELAY=2` (each of the 76 live
  `run` lines has its own `HOME`, so each case is delayed once). A
  future live case that reads the index before it arrives fails on every
  full check, not only on a loaded host (orchestrator decision on this
  WP's evidence). Cost: `today-live` 8.5 s instead of 7.0 s (two runs
  each, idle), at most 2 s per live case, about 2.5 min for 76; all live
  cases, no subset. A check without `SELDON_FULL_CHECK` runs them with
  no delay.
- `docs/TESTING.md` §3: the setting, the rule that every live case
  reading the index from its first step sets it, and the full check's
  delayed first write.

### Why a harness setting, not a `wait:` step (deviation from *Do*)

The WP asks for a `wait:` step on a field the loaded index sets. That
works for `today-live`, but most of the racing cases start with
`summon:<selection>`: the selection is made at step 1 and is lost when
the row is not there yet, and a `wait:` can only follow the summon (its
path reads the open desk). A `wait:` step also reports, so every later
step number of the case would move (a large diff against WP-201 and
WP-206, which edit the same file). The setting waits before step 1,
for every kind of first step, and keeps each case's step numbers.

### How the cases were found

The fake engine's first `status` held back 3 s (a local, uncommitted
change to `tests/plugin/fake-seldon`, reverted), the whole `desk-view.sh`
run: 305 checks in 55 cases failed. 54 were live cases racing the index
(the 54 above; `triage-swap-apply`/`-discard` share one `run` line).
`triage-big` (a dev-mode index, not live) failed a 15 s `wait:` once
on the loaded host and passed in the next run; not this race.

The 23 live cases left without the setting: they wait first
(rules-outdated, rules-fail, setup-flow, setup-blocked, preview-uninit,
preview-failed), settle first (capture-warned, setup-partial,
graph-dirty, aside-tile), have no index by design (uninit-sections;
capture-click, whose start-up capture is held locked), or read no
index-derived field before their first `settle` (today-new, held-forms,
held-fields, held-import, import-area, work-locked, work-run,
work-run-refused, decisions-live, decisions-locked, tones-default). All
passed with the index 3 s late.

## How it was verified

- **headless** (dev host, `desk-view.sh`, `TMPDIR` under the private
  gates folder; each Quickshell with its own `HOME` and a private
  0700 `XDG_RUNTIME_DIR` removed afterwards, as the script does):
  - first index write 3 s late, without the setting: 1936 passed,
    305 failed (above);
  - first index write 3 s late, with the setting: 2241 passed,
    0 failed;
  - `today-live` alone, idle: passed with and without the setting.
  - **acceptance** (orchestrator decision: the delayed first write is
    the proof, not load): `today-live` alone with `SELDON_FULL_CHECK=1`
    (first write 2 s late) fails 3 of 3 without the setting, exactly
    as in gate n202m (`journal.cases` 1, want 7; "No case" not shown),
    and passes 3 of 3 with it.
- **headless** under load: 8 busy loops (the orchestrator's slot, under
  `flock` on the shared check lock, killed at the end), `today-live`
  20 times with the setting and 20 times without, alternating: 40 of 40
  passed, load average about 11 on 16 cores. No failure in 20 without
  the wait under 8 loops; the race did not open (gate n202m failed at
  load 15–18). The acceptance's "fails at least once without the wait
  under load" was therefore not shown under load; the delayed write
  above shows it deterministically.
- **test host**: `shellcheck tests/plugin/desk-view.sh
  tests/plugin/fake-seldon` (through the wrapper that runs it on the
  test host): the same 70 findings (51 and 19; all existing) on `next`
  and on this branch.
- **CI**: not run (no push).
- `SELDON_FULL_CHECK=1 just check`: see the last section.

## Open points

- The setting is opt-in: a new live case that reads the index from its
  first step must set it (TESTING.md §3 says so); the full check's
  delayed first write fails such a case every time. Making it the
  default for live runs would need opt-outs for the uninit, setup,
  preview and capture-locked cases; not done.
- The full check now runs live cases only with the index 2 s late; the
  early-index timing (the index there by step 1) runs in a check
  without `SELDON_FULL_CHECK`. Cases with the setting behave the same
  either way.
- `service-states.sh` and `bar-view.sh` were not searched for the same
  race (not in this WP's inputs).

## just check

`SELDON_FULL_CHECK=1 flock /tmp/seldon-check.lock just check` on the dev
host at `74d89ca4` (`TMPDIR`, `JUST_TEMPDIR` and `CARGO_TARGET_DIR`
under the private gates folder): exit 0, `check: ok`; the engine tests
ran as **fixture** tests, the Quickshell harnesses as **headless**, the
live desk cases with their first index write 2 s late (service-states
366, desk-view 2243, bar-view 195, ipc-restart 43 passed, 0 failed; no
`wait timed out` in the log). An earlier full check at `22ecce5c`
(before the delay option, not under the lock) was green too (desk-view
2242).
