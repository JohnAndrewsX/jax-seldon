# WP-219 — Handover

Branch `wp/219-hook-fast-path-2` (from `next` at `bde78fdc`), worktree
`wt/WP-219`. Not merged. Role: Engine Dev. Engine source (`main.rs`), two
tests, SPEC-ENGINE §1 and CHANGELOG. No budget, contract, schema, fixture or
plugin change; no new crate (`regex` and `toml` are existing dependencies).

## What was done

- **`hook session-stop` fast path** (`engine/src/main.rs`):
  `plain_agent_hook` also takes exactly `[_, "hook", "session-stop"]` and
  returns `SessionStop { actor: CLAUDE_CODE }`, what clap reads from the
  default value. `hook install` writes this line (`hook.rs`, `SessionEnd`).
  `hook session-stop --actor …`, `--actor=…` and every global flag still
  go through clap. All four plain hooks now skip the parser.
- **Unit test, the switch** (`the_test_switch_sends_every_hook_to_the_parser`,
  debug builds only, like the switch): without the variable all four
  plain lines take the fast path. The test then runs its own test binary
  again for this test alone, with `env_clear()` and only
  `SELDON_TEST_HOOK_PARSE=1`, so there is no `HOME` and no runtime dir.
  In that run all four must return `None`; the parent requires
  `1 passed`. The variable is never set in the test process itself:
  `set_var` would race with the threads of other tests (as `sys.rs`
  already notes).
- **Unit test, the mapping** (`a_plain_agent_hook_is_what_the_parser_reads`):
  `session-stop` joins the three names compared with `Cli::try_parse_from`
  by `Debug`. The plain `hook session-stop` left the "not taken" list;
  `--actor agent:codex`, `--actor=agent:claude-code` and
  `--json hook session-stop` joined it.
- **Differential test** (`tests/hooks.rs`, `mod fast_path`): rewritten so
  that each call runs at the same time in two homes made alike, one on the
  fast path and one through the parser. It compares **exit code, stdout,
  stderr, every file under the temp dir** (logbook, state dir, config;
  `.git/` excluded, git is covered by the count) **and the commit count**.
  Before the comparison it takes out only what differs between two
  homes: the temp-dir path, the logbook's machine id and `created` time
  (read from `.seldon/logbook.toml`), the logbook's own commit ids (full
  and 7-character, read with `git rev-list --all`), the inode of the
  pacman log (its cursor), and ULIDs. Files that are not UTF-8 (the
  stubs' `git`, a link to the host's binary) are compared byte for byte.
  Neutralising that binary with the regex took 7 s per call. Tests:
  - `both_paths_give_the_same`: WP-218's 73 calls plus 16 `session-stop`
    calls: outside, relative, empty `cwd`; `SELDON_CASE` empty and
    `not-a-case`; stdin `/dev/null` for each of the four hooks; `""`,
    `not json`, `{}`, `[]`; then three served ends: in the logbook,
    launched (`SELDON_CASE`) outside, `CLAUDE_PROJECT_DIR`. 89 calls.
    Floors: ≥ 8 commands, ≥ 4 context blocks, ≥ 10 reported errors, and
    with git ≥ 3 new commits.
  - The seven probe cases from the WP-218 review, written anew. Each runs
    all four hooks with a payload (`claude-code` as PreToolUse and
    PostToolUse), the four with stdin `/dev/null`, and `session-stop`
    last: `in_a_session_in_the_logbook`, `in_a_session_seldon_launched`
    (payload `cwd` outside, `SELDON_CASE` set), `without_a_logbook` (not
    initialised; `SELDON_LOGBOOK` naming a missing dir; the same dir
    empty), `with_a_broken_config`, `with_a_bad_clock`
    (`SELDON_NOW=yesterday`), `while_the_lock_is_held` (PreToolUse only:
    each served call waits the 8 s `LOCK_PATIENCE`, `session-stop` twice;
    no command recorded).
  - Note: a hook with no `cwd` (stdin `/dev/null`) is in scope by design
    (`in_scope_by_dir`, `(_, None) => true`), so the `/dev/null`
    `session-stop` journals, captures and commits. The comparison covers
    that path too.
- **check-perf row**: `fast_enough_for_an_unrelated_session` gains
  `("session-stop", SessionEnd payload with cwd outside)` under the same
  1 ms budget (median of 20, 10 000 ledger lines). It also checks that no
  journal day was written.
- SPEC-ENGINE §1 principle 5 now names the four hooks; CHANGELOG
  [Unreleased] Engine gets an entry.

## How it was verified

- **fixture** (dev host). Own `HOME`, private 0700 `XDG_RUNTIME_DIR` and
  `TMPDIR` under the private `gates/run-dev-219`, `CARGO_TARGET_DIR`
  `gates/target-dev-219`, `CARGO_HOME`/`RUSTUP_HOME` passed through:
  - `cargo test --bin seldon` 4/0. `cargo test --test hooks fast_path`
    7/0 in 49 s; `while_the_lock_is_held` sets the pace with the 8 s
    lock waits, both paths running at once.
  - Mutants, each applied in the worktree after the commit and reverted
    with `git checkout`:
    - M1, the WP's parse-switch mutant: `SELDON_TEST_HOOK_PARSE` check
      made `if false`. **The new unit test fails**: the inner run panics
      at `claude-code`, and the parent reports the failed child. The
      other three unit tests pass.
    - M2: the fast path maps `session-stop` to `--actor agent:codex`. The
      mapping unit test fails (`session-stop`). The differential tests
      `both_paths_give_the_same` (call 76), `in_a_session_in_the_logbook`
      and `in_a_session_seldon_launched` fail on
      `state/seldon/autocommit.json`.
  - `cargo fmt`, `cargo clippy --all-targets -D warnings`: clean.
- **desktop** (dev host, `flock /tmp/seldon-check.lock`, bench profile):
  `just check-perf` **exit 0** at load 6 to 7 (other agents' work outside
  the lock). The 1 ms rows, unrelated session, 10 000 lines, median of
  20: **claude-code 823 µs, session-start 768 µs, generic 776 µs,
  session-stop 767 µs** (new). All other check-perf rows passed. Log in
  the private `gates/run-dev-219/check-perf-1.log`.
- **Full check** (desktop, dev host; the Quickshell harnesses
  **headless**). Command: `SELDON_FULL_CHECK=1 flock
  /tmp/seldon-check.lock just check` at `e8630705`, shellcheck from the
  orchestrator kit. Result: **exit 0, `check: ok`**. hooks 90/0 (4
  ignored timing rows), main unit tests 4/0, service-states 367/0,
  desk-view 2723/0, bar-view 196/0, ipc-restart 44/0,
  real-home-guard.test 79/0, check-perf.test 14/0, install.test 230/0,
  deploy-test-host.test 327/0, runtime-dir.test 45/0, docs-check ok. The
  `FAIL` words in the log are the self-tests' expected mutant lines. All
  harnesses report the real `~/.local/state/seldon` and
  `~/.config/seldon` untouched. Runtime dir 2 %, 333 `quickshell/by-id`
  entries before and after. Log in the private
  `gates/run-dev-219/check-full-1.log`. The only later commit adds this
  HANDOVER.
- **CI**, **test host**: not run.

## Open questions

1. `hook session-stop` is now inside 1 ms for an unrelated session. Its
   served path (journal, `capture --all`, views, commit) still has no
   budget row. Not in this WP.
2. WP-216 review Q3 still stands: check-perf is not a CI step.
