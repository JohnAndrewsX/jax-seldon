WP-061 HANDOVER

Branch `wp/061-review`, worktree `wt/WP-061`. No PR, no push.

## Done

- **F-504 (inherited git environment)**: `engine/src/logbook/git.rs`
  builds every git `Command` itself (`command()`): it runs in the logbook
  with 18 variables removed (`GIT_DIR`, `GIT_WORK_TREE`,
  `GIT_INDEX_FILE`, `GIT_OBJECT_DIRECTORY`,
  `GIT_ALTERNATE_OBJECT_DIRECTORIES`, `GIT_COMMON_DIR`, `GIT_NAMESPACE`,
  `GIT_CEILING_DIRECTORIES`, `GIT_QUARANTINE_PATH` and the rest of
  `git rev-parse --local-env-vars`). It goes through the existing
  `sys::run_command`, and `sys.rs` is not touched. git still runs in the
  engine's own process group (no `setsid`/`process_group`), so hooks,
  signing and passphrase prompts work as before (the WP-064 note). The
  user's git config and hooks still apply, and the spec says so now.
- **F-452 (detached HEAD)**: `commit_all` runs `git symbolic-ref -q
  HEAD` first. On a detached HEAD it stages nothing and returns
  `HEAD is detached (no branch is checked out)`.
- **F-503 (failed autocommit is silent)**: `commands/mod.rs`
  `autocommit` writes one line to stderr,
  `seldon: warning: git: not committed: <reason>`, and returns the new
  variant `Commit::Warned(reason)`. Its JSON is the same as `Failed`
  (`{"committed": false, "error": …}`). Its human text is empty, so the
  stdout line "Git: not committed" no longer duplicates the warning. Exit
  stays 0. git's multi-line stderr is joined into one line.
  `Commit::Failed` is kept for callers that build a failure themselves
  (init's lock error).
- **doctor (F-503 doctor part, F-541 index.lock row, F-452 doctor part)**:
  `check_git` → `autocommit_blocked()` (only while autocommit is on and
  the logbook is a repository). It reports the first problem it finds:
  - `.git/index.lock` exists: degraded, with its age, fix
    `rm <root>/.git/index.lock`. The message says to do this only when
    no git command is running.
  - detached HEAD: degraded, fix `git -C <root> switch <branch>` (the
    branch is named when there is exactly one, else `<branch>`).
  - `git commit --dry-run` fails (exit 1, "nothing to commit", counts as
    fine; `GIT_OPTIONAL_LOCKS=0`): degraded, fix
    `git -C <root> commit --dry-run`.
- **F-142 (import undo)**: I chose "commit first, refuse what cannot be
  committed". Before its first write, `import --apply` autocommits the
  logbook's pending changes as `seldon: before import omarchy-agent`. If
  the tree is still not clean (`--no-commit`, `autocommit = false`, a
  failed commit or a detached HEAD), the apply is refused with exit 1
  before it writes anything. The undo of a failed apply names only the
  import's files. They are noted before each write: files that existed
  go back to the commit before the import (`git checkout <sha> -- …`),
  files it created are removed (`rm -f -- …`), and paths are
  shell-quoted. The undo is also kept in
  `.seldon/imports/omarchy-agent.undo.json`, which the undo removes too.
  A later refusal ("import notes but no marker") prints that same
  command. It does so only if `base` is a hex hash and every path is
  relative and inside the logbook, because the logbook is agent-writable.
  Otherwise it says to look at `git -C <root> status` and offers no
  command. A successful apply removes a leftover undo file.
- Docs: `docs/SPEC-ENGINE.md`, with a new autocommit-rule paragraph after
  the `rebuild_if_initialised` paragraph and the `import` lines.
  `docs/user/en/09-import-from-omarchy-agent.md` and the `de` page
  (stamped `0b02abe`, the en commit). A `CHANGELOG.md` `[Unreleased]`
  Engine entry. `memory/pitfalls.md` appended.

## Not done

- `engine/src/index/mod.rs` `git_info` (`rev-parse`, `status
  --porcelain` for `logbook.git`) still runs git with the inherited
  environment. It only reads, but under an inherited `GIT_DIR` the index
  reports the other repository's HEAD and dirty state. That file is not
  in this WP. The fix is a one-liner once `git.rs` exposes a read helper
  (for example make `git::run` public). Proposed as a follow-up.
- F-503's index/plugin part (last commit result in `index.json`, a
  plugin banner) is still waiting for the operator decision, as the WP
  says.
- `hook.rs` `session_stop` (WP-063's file) still has
  `if let Commit::Failed(e) = autocommit(…) { report("git", &e) }`.
  `autocommit` now returns `Warned`, so that branch no longer fires, and
  the hook prints the single `seldon: warning: git: …` line. No double
  line, no behaviour lost, but the branch is dead code that WP-063 or a
  later WP can remove.
- No `-c core.hooksPath=` for the autocommit. The user's hooks still
  run, and the spec now says so.

## Verified by

- New tests:
  - `engine/tests/git.rs`:
    - `an_inherited_git_dir_does_not_redirect_the_autocommit`:
      `GIT_DIR`, `GIT_WORK_TREE`, `GIT_INDEX_FILE`,
      `GIT_OBJECT_DIRECTORY` and `GIT_COMMON_DIR` of a second scratch
      repository are set. The logbook gets `seldon: note`, while the
      other repository's HEAD and index are unchanged.
    - `a_detached_head_is_not_committed_and_says_so`: exactly one stderr
      line, `--json` `git.error`, no commit, nothing staged. After going
      back to the branch, the next write commits the waiting note.
    - `a_stale_index_lock_is_one_warning_and_exit_0`: empty
      `.git/index.lock`. `log --json` exits 0 with one warning line and
      `git.error` naming `index.lock`. The human mode has the warning on
      stderr only. After the lock is removed, the next write commits
      everything.
  - `engine/tests/doctor.rs`:
    - `a_stale_index_lock_is_degraded_with_its_fix`: back to `ok` once
      the lock is removed.
    - `a_detached_head_is_degraded_with_its_fix`: `ok` with autocommit
      off.
    - `a_commit_that_would_fail_is_degraded`: `.git` mode 555; the test
      is skipped as root after a probe write.
  - `engine/tests/import.rs`:
    - `a_dirty_logbook_is_committed_first_and_the_undo_keeps_other_changes`:
      pending before the failed apply: a `--no-commit` note (ledger line
      and journal day), `inbox/idee.md`, and an edit of
      `memory/lessons.md`, which the import also writes. After the
      failure, `inbox/later.md` is added and `PROJECT.md` is edited. The
      printed undo, run with `sh -c` in the logbook, gives back exactly
      the pending tree plus the two later changes, with no import notes.
      `git status` = ` M PROJECT.md` / `?? inbox/later.md`.
    - `a_dirty_logbook_without_a_commit_is_refused`: `--no-commit`
      leads to exit 1 with "uncommitted changes (not committed:
      --no-commit)", and nothing is written.
    - The existing `a_failed_apply_says_how_to_undo_it` and the
      marker-removed refusal are updated to the new hint. The first now
      runs the printed undo instead of the old `checkout`/`clean`.
  - Unit tests: `logbook::git::tests::the_command_drops_every_repository_variable`;
    in `commands::import::tests`, the undo command quoting and the
    refusal of a kept undo with a non-hash base, `..`, an absolute or an
    empty path.
- Mutants. Each one was applied in place and restored from
  `git show HEAD:` (checked with `cmp`), and each fails its test:
  - M1 (F-504): no `env_remove` loop. Fails
    `an_inherited_git_dir…` (tests/git.rs:79 `the logbook got no
    commit`) and the unit test.
  - M2 (F-452): no detached check in `commit_all`. Fails
    `a_detached_head…` (tests/git.rs:102: a commit, no warning line).
  - M3 (F-503): no stderr warning. Fails `a_detached_head…` and
    `a_stale_index_lock…` (warnings.len() 0 ≠ 1).
  - M4 (doctor lock): the index.lock branch is never taken. Fails
    `a_stale_index_lock_is_degraded…` (tests/doctor.rs:375: the dry run
    catches it, but not as ".git/index.lock exists").
  - M5 (doctor detached): `is_detached` is always false. Fails
    `a_detached_head_is_degraded…` (tests/doctor.rs:400, status ok).
  - M6 (doctor dry run): the dry run is always Ok. Fails
    `a_commit_that_would_fail…` (tests/doctor.rs:441).
  - M7 (F-142 commit first): no `commit_pending` call. Fails both
    `a_dirty_logbook_…` tests (no "before import" commit; the
    `--no-commit` apply is not refused).
  - M8 (F-142 hint): `Undo::command` returns the old
    `git checkout -- . && git clean -fd`. Fails
    `a_dirty_logbook_is_committed_first…` (tests/import.rs:589: the
    later changes are gone) and `a_failed_apply_says_how_to_undo_it`.
  - After the loop, `cargo build` was run again (the stale-binary
    pitfall).
- Manual run in a scratch HOME/XDG with `SELDON_TEST_GUARD`, on a fresh
  logbook with a pending lessons edit, a pending inbox note and
  `system/` set to 555. `import --apply` exits 2 with the undo
  `git checkout <sha> -- memory/lessons.md system/deviations.md && rm -f
  -- ledger/2026-08.jsonl … .seldon/imports/omarchy-agent.undo.json`,
  and `git log` shows `seldon: before import omarchy-agent` on top of
  `init logbook`. A second `--apply` exits 1 and prints the same
  command. The scratch directory was deleted afterwards.
- `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings`
  are clean, and the full `cargo test` is green.
- `just check`: exit 0 on `a179349` (fmt, clippy, tests and `check-watch`: 961 passed, 0 failed in 58 suites; packaging, install, schema, docs-check, plugin-validate, qmllint, plugin-test all ok). Nothing else ran in this worktree during the run.

## Learned

Appended to `memory/pitfalls.md`, under "2026-10-03 · WP-061":
- `env.git` in tests has no identity, so a test's own commit fails
  silently.
- git's stderr is multi-line.
- `commit --dry-run` exit codes and its lock behaviour.
- Run a printed shell hint inside the test, and change things *after*
  the failure, so that a file-scoped undo is told apart from a
  whole-tree undo.

## Decisions needed

- `docs/TESTING.md` is not in this WP's file list, but its rows for
  `tests/import.rs` (it still names the `git checkout -- . && git clean
  -fd` hint) and `tests/doctor.rs` are now stale, and the new
  `tests/git.rs` has no row. May I (or a docs WP) update those three
  rows?
- Follow-up for `index/mod.rs` `git_info` (inherited environment; see
  "Not done").

## Touched outside WP scope

None. The files are the ones listed under *Files touched*, plus
`CHANGELOG.md` `[Unreleased]` (append) and `memory/pitfalls.md`
(append). `engine/tests/common/mod.rs` is not changed. Small helpers
went into `doctor.rs` next to `check_git` (`autocommit_blocked`,
`human_age`) and into `import.rs` next to the undo code (`Undo`,
`commit_pending`, `shell_quote`, `undo_path`). The new `git.rs` helpers
are `is_detached`, `branches`, `commit_dry_run`, `is_dirty` and `head`.
