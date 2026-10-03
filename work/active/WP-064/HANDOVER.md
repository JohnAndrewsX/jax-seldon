```
WP-064 HANDOVER
```

Branch `wp/064-review`. Commits: `6b95373` (engine), `cce00da` (tests,
SPEC-ENGINE §2, CHANGELOG), plus this handover.

**Done**

- `sys::write_atomic` (`engine/src/sys.rs`):
  - follows a symbolic link at the path (up to 40 links, relative links
    against the link's directory): the link stays, the target is
    replaced; a link to a missing file creates the target; a link loop
    is an error;
  - the temp file `.<name>.tmp-<pid>` is created next to the target with
    `create_new` and then gets exactly the target's permission bits
    (`fchmod`, so the umask cannot change them); a new file gets 0600
    (`sys::NEW_FILE_MODE`), new parent directories 0700
    (`sys::NEW_DIR_MODE`, `sys::create_dir_private`);
  - the file is synced before and the directory after the rename; the
    temp file is removed when any step fails; a leftover temp file with
    the same name is replaced.
  - `sys::write_atomic_mode` sets an explicit mode; the theme hook script
    (`commands/setup.rs` `write_script`) uses it with 0755 instead of a
    `chmod` after the rename.
  - `sys::create_new_private` creates a new file with exactly 0600.
- `logbook/layout.rs`: `init` creates the logbook folders 0700 and its
  files 0600 under any umask; `GITIGNORE` adds `.*.tmp-*`. An existing
  directory given to `init --path` keeps its mode.
- `sys::run_command` (and `sys::run`): the program runs in its own
  process group. At the deadline the group is killed (`kill(-pgid,
  SIGKILL)` through a local `extern "C"` declaration, as in
  `commands/watch.rs`; no new crate). After the program exits, the
  output pipes are awaited only until the same deadline; if something
  the program started still holds a pipe then, the group is killed and
  the pipes get 200 ms more. Complete output → `Run::Exited` with the
  program's status; pipes still open after that → `Run::TimedOut`.
- Tests: new `engine/tests/file_writes.rs` (13 tests); helpers
  `common::with_umask` and `common::mode` appended to
  `engine/tests/common/mod.rs`; `tests/init.rs` expects the new
  `.gitignore` line.
- Docs: SPEC-ENGINE §2 paragraph "File modes" under the table;
  CHANGELOG `[Unreleased]` Engine line.
- Advisor (Fable) agreed: existing files keep their mode everywhere
  (logbook, state, Claude settings), new files 0600 everywhere;
  `Exited` only with complete output, else `TimedOut`.

**Not done**

- Files created outside `write_atomic` and the layout keep the umask
  mode. A manual `init` + `log` + `plan new` under umask 022 shows
  exactly these: the state directory and the lock file
  (`logbook/lock.rs`, which creates the state directory first), ledger
  month files (`ledger.rs` `append_file`), new case and ADR files
  (`commands/mod.rs` `write_new`), and `agent-launch.log`
  (`commands/agent.rs`). Each is a one-line switch to the new sys
  helpers. They are outside this WP's file list; I asked the orchestrator
  and had no answer by handover. See Decisions needed.
- `.git/` follows git's own rules (0755/0644 under umask 022);
  `git init --shared=0600` would make it private (`logbook/git.rs`, not
  in scope).
- Existing logbooks keep their `.gitignore` without `.*.tmp-*`, and old
  leftover temp files are not cleaned up (the WP asks for neither).
- User-guide line on tightening an existing logbook (page 06 is WP-062's
  in this wave). Suggested text: "Seldon creates new logbook, config and
  state files readable only by you. A logbook made by an earlier version keeps
  its modes; to tighten it run `chmod -R go= ~/Seldon
  ~/.local/state/seldon ~/.config/seldon`."
- `hook uninstall` deletes a settings file that holds only Seldon's hooks
  with `remove_file` (`collectors/config.rs` `delete_own_file`). On a
  symlinked settings file this removes the link, and the target keeps
  Seldon's hooks. Not in scope; a follow-up should remove the hooks from
  the target (or write an empty object) when the path is a link.

**Verified by**

- `just check` on `cce00da` → exit 0, `check: ok`; 929 cargo tests
  passed, 0 failed (58 suites, with and without `watch`); fmt and clippy
  `-D warnings` clean; `docs-check: ok (382 links, 14 translated pages,
  40 commands, 437 command lines)`; plugin harnesses passed. shellcheck
  is not installed here (`bash -n` only).
- `cargo test --test file_writes`: 13 passed:
  - `atomic::a_symlinked_file_is_written_through`,
    `a_link_to_a_missing_file_creates_the_target`,
    `a_link_loop_is_an_error`, `the_mode_of_an_existing_file_is_kept`
    (0600, 0640, 0664, 0755), `a_new_file_and_its_new_directories_are_private`,
    `an_explicit_mode_is_set_exactly`,
    `a_failed_replace_leaves_no_temp_file` (a non-empty directory at the
    path makes the rename fail);
  - `run::a_helper_that_keeps_the_pipe_open_is_stopped_at_the_deadline`
    (`sh -c 'sleep 5 & …; echo done'`, timeout 1 s: returns `Exited`
    with `done` in < 1.8 s, the background `sleep` has ended),
    `a_timeout_stops_the_whole_process_group` (`TimedOut` in < 1.5 s,
    background `sleep` ended), `a_quick_program_keeps_its_output`;
  - `cli::init_creates_private_directories_and_files_under_any_umask`
    (umask 022 and 000: every logbook, config and state directory 0700
    and file 0600, except `.git` and the out-of-scope paths above, which
    the test names), `a_new_logbook_ignores_temp_files` (`git status`
    shows no `.STATUS.md.tmp-4242`), `hook_install_writes_through_a_symlinked_settings_file`
    (umask 022, target 0600 before and after, the link stays, the
    target's other keys are kept).
- Pre-fix run (current tests against `690dd71`'s `sys.rs`, with a
  temporary `write_atomic_mode` stub): 11 of 13 failed. Only
  `a_quick_program_keeps_its_output` and the stubbed
  `an_explicit_mode_is_set_exactly` passed. The pipe test took 5.0 s.
- Mutants (each applied alone; `cargo test --test file_writes`):

  | Mutant | Killed by |
  |---|---|
  | no link resolution | `a_symlinked_file_is_written_through`, `a_link_to_a_missing_file_creates_the_target`, `a_link_loop_is_an_error`, `hook_install_writes_through_a_symlinked_settings_file` |
  | existing mode ignored (always 0600) | `the_mode_of_an_existing_file_is_kept` |
  | no exact `fchmod` of the temp file | `the_mode_of_an_existing_file_is_kept` |
  | directories without mode 0700 | `a_new_file_and_its_new_directories_are_private`, `a_link_to_a_missing_file_creates_the_target`, `init_creates_…_under_any_umask` |
  | temp file kept on failure | `a_failed_replace_leaves_no_temp_file` |
  | no own process group | both `run::` group/pipe tests |
  | unbounded pipe wait | `a_helper_that_keeps_the_pipe_open_is_stopped_at_the_deadline` |
  | only the direct child killed at timeout | `a_timeout_stops_the_whole_process_group` |
  | `.gitignore` without the pattern | `a_new_logbook_ignores_temp_files` |
  | layout files without mode 0600 | `init_creates_…_under_any_umask` |

- Manual runs (scratch HOME/XDG, `SELDON_TEST_GUARD`, made-up values),
  pre-fix binary vs this branch:
  - `capture --all` with a made-up snapper wrapper that runs `sleep 25 &`
    before printing a snapshot list: before 25.2 s, after 10.2 s (the
    snapper timeout), exit 0, snapper `ok`, no `sleep` left;
  - cost of the syncs on btrfs (debug build, average of 10 runs): `log`
    85 → 97 ms, `status` 37 → 46 ms, a recorded agent command
    (`hook claude-code`, mutating fixture) 23 → 38 ms. On tmpfs no
    difference.

**Learned** (added to `memory/pitfalls.md`)

- A mutation loop that restores the source with a copy made before the
  mutant gives the file an older mtime than the last build; cargo then
  keeps the mutant's binary. Touch the restored file.

**Decisions needed**

1. Switch the four remaining creators (`logbook/lock.rs`, `ledger.rs`
   `append_file`, `commands/mod.rs` `write_new`, `commands/agent.rs`
   launch log) to `sys::create_dir_private` / an `.mode(0o600)` open, in
   this WP or a follow-up. Until then the state directory is created
   0755 by the lock, and ledger and new case files are 0644 (inside a
   0700 logbook). The test's exception list names them, so the switch is
   a one-line test change each.
2. `docs/SPEC-LOGBOOK.md` line 40 lists the `.gitignore` contents; it
   lacks `.*.tmp-*` now. Not in my file list; one-word edit.
3. Sync cost: about +15 ms per recorded agent command on a real disk
   (two fsyncs per rewritten file, mostly the case file and
   `index.json`). Keep syncing every rewrite (WP text), or skip the
   syncs for rebuildable generated files (`index.json`, `STATUS.md`,
   ledger views)?
4. Process groups: a terminal Ctrl-C no longer reaches a program the
   engine runs; the engine itself stops and the program finishes on its
   own (at most the 10 s / 30 s timeouts it would have had). A program
   whose helper leaves the process group (`setsid`) and keeps the pipe
   open leaves one blocked reader thread behind; harmless for one-shot
   commands, and under `seldon watch` one per such timeout. No action
   proposed.

**Touched outside WP scope:** `engine/tests/init.rs` (one assertion:
the `.gitignore` text gains `.*.tmp-*`; needed for `just check`).
