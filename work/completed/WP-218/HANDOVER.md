# WP-218 — Handover

Branch `wp/218-hook-fast-path` (from `next` at `bc1c1b77`), worktree
`wt/WP-218`. Not merged. Role: Engine Dev. Engine source (`main.rs`), one
test, SPEC-ENGINE §1 and CHANGELOG. No budget, contract, schema, fixture or
plugin change.

## What was done

- `engine/src/main.rs`: WP-092's fast path now covers three agent hooks.
  `plain_agent_hook(argv)` returns the `HookCommand` for exactly
  `[_, "hook", "claude-code" | "session-start" | "generic"]` (as UTF-8),
  else `None`; `plain_hook_cli` builds the `Cli` value the parser would
  read (every global flag off, `Generic { case_id: None }`), and `main`
  joins the same dispatch as before (`run_agent_hook`). Every other command
  line goes through clap unchanged, among them `hook generic --case ID`,
  `hook session-stop`, any global flag (`--quiet`, `--json`, …) and any
  extra argument. Replaces `is_plain_claude_code_hook` and
  `plain_claude_code_hook`.
- A debug build only (`#[cfg(debug_assertions)]`, as
  `SELDON_TEST_HOOK_PANIC`): `SELDON_TEST_HOOK_PARSE` set sends the three
  through clap, so a test can run both paths with the same argv. The
  release and bench builds do not contain the check.
- Unit test `a_plain_agent_hook_is_what_the_parser_reads` (was
  `plain_claude_code_hook_is_what_the_parser_reads`): for each of the three
  the fast path's `Cli` equals `Cli::try_parse_from` as a whole (`Debug`);
  16 other command lines (global flags, `--case ID`, `--case=ID`,
  `session-stop`, `install`, `uninstall`, `--help`, `Generic`, other
  commands, extra arguments) are not taken.
- Differential test `fast_path::both_paths_give_the_same`
  (`tests/hooks.rs`): two fresh logbooks with the same active case, 73
  calls in the same order, the first logbook through the fast path, the
  second with `SELDON_TEST_HOOK_PARSE`. After each call it compares exit
  code, stdout, stderr (temp-dir paths neutralised) and the whole ledger
  (event ids removed). Inputs: each of the 8 `fixtures/hooks/` payloads as
  PreToolUse and PostToolUse, inside and outside the logbook; a Bash call
  outside with `SELDON_CASE`, and with `CLAUDE_PROJECT_DIR` at the
  logbook; a `Notification`; session-start with `cwd` in the logbook, `/`,
  outside, relative, empty, missing, `SELDON_CASE` valid / empty /
  `not-a-case`, `CLAUDE_PROJECT_DIR`; generic with `startedAt`, a
  non-mutating command, `"case"`, outside, outside with `SELDON_CASE`, the
  four bad payloads and three escaped values of the `generic` tests, no
  actor with `SELDON_ACTOR` unset / empty / `Agent:X` / `agent:codex`; and
  for each hook stdin `""`, `not json`, `{}`, `[]`. Floors make sure the
  inputs reach each outcome (≥ 8 recorded commands, ≥ 4 context blocks,
  ≥ 10 reported errors).
- SPEC-ENGINE §1 principle 5 names the 1 ms for the three hooks in an
  unrelated session (ADR-0030) and the measured ≈ 0.77 ms; CHANGELOG
  [Unreleased] gets an Engine entry. ADR-0030's budget is unchanged.

Not done: `hook session-stop` has no fast path (not in the WP; it has no
1 ms row). It would be one more match arm with
`SessionStop { actor: CLAUDE_CODE }` and one more unit-test name if
wanted.

## How it was verified

- **fixture** (dev host; own `HOME`, private 0700 `XDG_RUNTIME_DIR` and
  `TMPDIR` under the private `gates/dev-218`, `CARGO_TARGET_DIR`
  `gates/target-dev-218`):
  - `cargo test --bin seldon` 3/0; `cargo test --test hooks` 84/0 (4
    ignored timing rows), the differential test included.
  - Mutants, each applied in the worktree and reverted with `git checkout`:
    - M1 fast path maps `session-start` to `ClaudeCode`: the differential
      test fails at call 35 (stdout `""` against the context block).
    - M2 = M1 and the `SELDON_TEST_HOOK_PARSE` check made `if false`: every
      per-call comparison passes (both paths equally wrong) and only the
      coverage floor `printed >= 4` fails. This shows that the variable is
      what sends the second logbook through clap.
    - M3 `plain_hook_cli` with `quiet: true`: the unit test fails.
    - M4 fast path also takes `Generic`: the unit test fails.
  - `cargo fmt --check`, `cargo clippy --all-targets -D warnings`: clean.
- **desktop** (dev host, `flock /tmp/seldon-check.lock`, bench profile):
  - `just check-perf` run 1, load 11 to 14 from gates outside the lock:
    exit 101. The three 1 ms rows passed (claude-code 914 µs,
    session-start 969 µs, generic 957 µs). The 5 ms row "recorded curl
    line with a marker, 900 lines" missed at 5.73 / 6.19 ms. That path is
    not touched (`hook claude-code` already had the fast path). Private
    `gates/dev-218/check-perf-1.log`.
  - `just check-perf` run 2 at load 2.5 (0.8 at the end): **exit 0**. 1 ms
    rows: **claude-code 817 µs, session-start 766 µs, generic 760 µs**
    (were 1.023 / 1.035 ms for session-start in the WP-216 review). The
    hook's 5 ms rows passed: 866 µs, 1.77, 3.05, 2.68 ms at 10 000 lines;
    779 µs, 3.20, 4.59, 4.49 ms at 900 lines. Private
    `gates/dev-218/check-perf-2.log`.
  - Interleaved profile: the WP-216 review's scratch test plus rows H and
    I, on a `git archive` of HEAD, not committed. 60 rounds, same home,
    10 000 lines, load < 1. Private `gates/dev-218/profile.log`.

    | Variant | Median | p25–p75 |
    |---|---|---|
    | A `hook claude-code` (fast path), unrelated | 778 µs | 766–797 µs |
    | B `--quiet hook claude-code` (clap) | 1014 µs | 994–1038 µs |
    | C `hook session-start`, `cwd` `/` (fast path) | 776 µs | 758–802 µs |
    | D `hook session-start`, `cwd` outside (fast path) | 768 µs | 757–790 µs |
    | E `hook generic`, unrelated (fast path) | 773 µs | 760–795 µs |
    | H `--quiet hook session-start` (clap) | 1007 µs | 988–1028 µs |
    | I `--quiet hook generic` (clap) | 1002 µs | 984–1026 µs |
    | F `hook claude-code`, `Notification` (fast path) | 689 µs | 676–710 µs |
    | G the same through clap | 938 µs | 920–961 µs |

    C, D and E are now at A's level. H − C = 231 µs and I − E = 228 µs,
    the clap cost the review measured.
- **headless** and the rest: see the full check below.
- **CI**, **test host**: not run.

- **Full check** (desktop, dev host; the Quickshell harnesses
  **headless**), `SELDON_FULL_CHECK=1 flock /tmp/seldon-check.lock just
  check` at `ed274d42`, shellcheck from the orchestrator kit: **exit 0,
  `check: ok`**. hooks 84/0 (4 ignored timing rows), service-states
  367/0, desk-view 2461/0, bar-view 196/0, ipc-restart 44/0,
  real-home-guard.test 79/0, check-perf.test 14/0, install.test 230/0,
  deploy-test-host.test 327/0, runtime-dir.test 45/0. The `FAIL` words in
  the log are the self-tests' expected mutant lines (`ok … FAIL`,
  `killed … FAIL`). All three harnesses report the real
  `~/.local/state/seldon` and `~/.config/seldon` untouched; runtime dir
  2 %, 333 `quickshell/by-id` entries. Private `gates/check-dev-218.log`.
  The only later commit adds this HANDOVER.

## Open questions

1. Should `hook session-stop` get the same fast path? It has no 1 ms row,
   and it is not in the WP.
2. WP-216 review Q3 still stands: check-perf is not a CI step. Run 1
   shows again that the 5 ms rows depend on the host's load.
