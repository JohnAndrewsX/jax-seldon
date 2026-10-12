# WP-207 — Handover

Branch `wp/207-budget-test` (from `next` at `00ac1633`), worktree
`wt/WP-207`. Not merged. Role: Engine Dev. Tests only; no engine change.

## What varies by host

`logbook.path` in the index. It is the logbook's absolute path, and in
`status_writes_a_large_logbook_under_its_budget` that is
`$TMPDIR/seldon-test-env-<pid>-<n>/logbook`: the temp dir of the host
and the test process's pid (4 to 7 digits). Every other string of the
×200 index is the fixture's (`machine` comes from the fixture's
`.seldon/logbook.toml`; the other paths are relative to the logbook; the
scaled logbook has no git). Measured: the whole index grows byte for
byte with the length of `TMPDIR` (68 → 1 077 997 B, 58 → 1 077 987 B,
128 → 1 078 057 B, 268 → 1 078 197 B), and CI's `/tmp` gave
1 077 932 B. The cut takes events until the file is under the budget, so
the boundary moves with the path: 216 events on the dev host's gate
path, 215 on CI's `/tmp`, 217 with a temp path 200 characters longer.

Not a defect of the engine: the index says where the logbook is, and
the budget counts the bytes it really writes. In-process tests build
with a fixed `logbook_path` (`/home/user/Seldon`) and do not vary.

## What was done

- `engine/tests/index.rs`, `status_writes_a_large_logbook_under_its_budget`:
  the warning is no longer a prefix and a suffix with the constant
  `216 events`. The test builds the **whole** expected warning from the
  two files and asserts the engine printed exactly that string:
  the whole size, `` the largest section is `cases` (<bytes of whole.cases>) ``,
  `written at <size of the written file>`, `<whole completed − 10>
  completed cases`, `<whole events − written events> events`, and the
  closing sentence. Only those two lists can appear in it, so the
  warning still says that only history went.
- New: the cut took no event more than the file needs — the written
  size plus the newest cut event (and its comma) is at or over the
  budget again (the same check `a_rung_takes_only_the_rows_the_file_needs`
  makes for drift rows). This keeps what the constant proved (the cut
  stops at the boundary) without tying it to one host.
- Unchanged: written size under the budget, the file validates,
  `summary` equals the whole index's (400 crises), `drift` and the
  queued, active and verification cases equal the whole index's, 10
  completed cases left, 100 ≤ events < 500 and the kept events are the
  newest of the whole list, `index --check` passes.
- The other WP-200 tests: `a_writing_command_…`, `the_session_hook_…`
  and `status_counts_from_the_whole_index` use the same temp logbook but
  assert no host-dependent count (size under budget, summary, "warning"
  wording, fewer verification cases). The in-process tests
  (`an_index_over_its_budget_…`, `crisis_rows_go_last_…`,
  `the_cut_keeps_the_events_…` with its exact `100`,
  `a_rung_takes_only_…`, `an_index_still_over_…`) build with
  `input()`'s fixed path. No change needed.

## How it was verified

- **fixture** (dev host, `cargo test --test index status_writes_a_large`,
  `CARGO_TARGET_DIR` and `TMPDIR` under the private gates folder):
  passed with `TMPDIR` of 68 characters (the gate path; 216 events
  cut), 58 (−10, the shortest path the gates folder allows), 128 (+60)
  and 268 (+200; 217 events cut). The old test, run once at +200,
  fails exactly as CI did (`… 40 completed cases, 217 events; …`),
  so the length reproduces the defect; the new test passes there.
- **not run**: a temp path 30 characters shorter than the gate path
  (38 characters). The brief keeps `TMPDIR` under the gates folder
  (56 characters before any subfolder) and off `/tmp`, so −10 is the
  shortest local run. The short side is CI's `/tmp` (64 shorter, the
  215-event case of run 38185406818); the new test computes that count
  from the files the same way.
- **CI**: not run on the branch (CI runs on pushes to `main`/`next` and
  on pull requests; I did not push). Acceptance "CI green on the
  branch's merge into `next`" is for the orchestrator.
- `SELDON_FULL_CHECK=1 just check` on the dev host: see the last line
  of this file.

## Open points

- ADR-0053's measurement table (×200: 999 825 B, 216 events) is the dev
  host's measurement; the ADR is accepted and was not edited.

## just check

`SELDON_FULL_CHECK=1 just check` on the dev host at `9c5de136` (`TMPDIR` and
`CARGO_TARGET_DIR` under the private gates folder): exit 0, `check: ok`;
the engine tests ran as **fixture** tests, the Quickshell harnesses
(service-states, desk-view, bar-view, ipc-restart) as **headless**,
all passed.
