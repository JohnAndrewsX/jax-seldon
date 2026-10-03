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

- ~~`index/mod.rs` `git_info` reads git with the inherited
  environment~~: done in fix round 1 (Q4).
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

- None. B3 is done in the final round (below).

## Touched outside WP scope

None. The files are the ones listed under *Files touched*, plus
`CHANGELOG.md` `[Unreleased]` (append) and `memory/pitfalls.md`
(append). `engine/tests/common/mod.rs` is not changed. Small helpers
went into `doctor.rs` next to `check_git` (`autocommit_blocked`,
`human_age`) and into `import.rs` next to the undo code (`Undo`,
`commit_pending`, `shell_quote`, `undo_path`). The new `git.rs` helpers
are `is_detached`, `branches`, `commit_dry_run`, `is_dirty` and `head`.

## Fix round 1 (review SEND BACK)

B3 is not done yet; it waits for the message that WP-064 is merged.

Done:
- **B1**: `git.rs` `command()` sets `GIT_CEILING_DIRECTORIES` to the
  logbook's parent (resolved path), instead of removing it. `commit_all`
  calls the new `check_toplevel()` first: `git rev-parse --show-toplevel`
  must equal the logbook root (both canonicalised), otherwise nothing is
  committed.
  - An empty or broken `.git` gives "the logbook's .git is not a usable
    repository: fatal: not a git repository …".
  - A `.git` that resolves to another work tree gives "… belongs to
    another work tree (<path>)".
  - doctor runs the same check whenever the logbook has a `.git`:
    degraded, with fix `git -C <root> init` for an unusable one.
  - git messages now keep only git's `fatal:`/`error:` lines, so the
    advice text is left out.
- **B2**: `Undo::is_sane` now requires:
  - a 40- or 64-hex `base` whenever `restore` is non-empty;
  - every path in `ledger/ work/ journal/ memory/ system/ outputs/`
    (at least one level deeper) or under `.seldon/imports/`;
  - no `.git` component, no `..`, no absolute or empty path;
  - no `* ? [ \`, no leading `:` and no control characters.
  The printed restore is `git --literal-pathspecs checkout <sha> -- …`.
- **N1**: test `a_successful_apply_removes_a_leftover_undo_file`.
- **N2**: doctor no longer runs `git commit --dry-run`. The checks after
  the index.lock check, all lock-free:
  - `.git` read-only (file metadata), fix `chmod u+w <root>/.git`;
  - detached HEAD;
  - `check_head` (`rev-parse --verify -q HEAD`, an unborn branch is fine
    via `symbolic-ref` + `show-ref --verify`), fix `git -C <root> status`;
  - `check_identity` (`git var GIT_COMMITTER_IDENT` and
    `GIT_AUTHOR_IDENT`, with the same fallback identity the autocommit
    uses), fix `git -C <root> config user.name "Your Name"`.
- **N3**: `docs/TESTING.md`: the import hint (~59), the doctor row, and
  a new `tests/git.rs` row.
- **Q4**: `index::git_info` runs `rev-parse` and `status` through
  `git::query` (same environment and ceiling).
- Docs: the SPEC-ENGINE autocommit paragraph (ceiling, toplevel check,
  index, the new doctor probes, "doctor writes nothing into .git").
  - Also restored: in round 0 I split the original "Broken files …"
    sentence off its paragraph; it is back where it was.
  - The SPEC `import` lines now say what a kept undo must look like.
  - en/de 09 show `--literal-pathspecs` and say that a tampered undo is
    not offered; the de page is stamped `779624f`.
  - My own `[Unreleased]` CHANGELOG lines are updated (they had said
    "commit dry run").

New and changed tests:
- `tests/git.rs`:
  - `an_empty_dot_git_inside_another_repository_commits_nowhere`: the
    parent's HEAD and index are unchanged, the empty `.git` stays empty,
    there is one warning with the reason, `--json` `git.error`, and
    `index` shows no HEAD.
  - `the_index_reads_the_logbooks_head_under_an_inherited_git_dir`.
- `tests/doctor.rs`:
  - `a_read_only_dot_git_is_degraded`, which replaces the dry-run test.
  - `an_identity_git_cannot_resolve_is_degraded` (empty `user.name`).
  - `an_empty_dot_git_is_degraded_with_its_fix` (inside a parent
    repository).
  - `doctor_leaves_dot_git_untouched`: a tracked file's mtime is moved
    an hour ahead, then `.git` is compared byte for byte.
- `tests/import.rs`:
  - `a_tampered_undo_file_is_not_offered_as_a_command`: eight tampered
    files (null or `HEAD` base with a restore, `.git/config`,
    `.git/HEAD`, `PROJECT.md`, `../outside`, `:/`, `memory/*`); each
    refusal prints no command, points to `git -C … status`, and writes
    nothing.
  - `a_successful_apply_removes_a_leftover_undo_file`.
- Unit test `a_kept_undo_outside_the_imports_files_is_not_offered`
  (17 bad paths, the base rules, one good set).

Mutants. Each was applied in place and restored from the index, and
each fails its test:
- F1 (B1 ceiling not set):
  - `an_empty_dot_git…` (tests/git.rs:210, the reason is "belongs to
    another work tree", not "not a usable repository");
  - doctor `an_empty_dot_git_is_degraded_with_its_fix` (tests/doctor.rs:485).
- F2 (B1 toplevel check removed from `commit_all`): `an_empty_dot_git…`
  (tests/git.rs:210).
- F3 (B2 `.filter(Undo::is_sane)` removed): `a_tampered_undo_file…`
  (tests/import.rs:688, the command is offered).
- F4 (N1 `remove_file` removed): `a_successful_apply_removes…`
  (tests/import.rs:714).
- F5 (N2 `git commit --dry-run` put back into doctor):
  `doctor_leaves_dot_git_untouched` (tests/doctor.rs:513, `.git`
  changed).
- F6 (Q4 `git_info` back on `sys::run`): `the_index_reads…`
  (tests/git.rs:287, it reports the other repository's HEAD).
- F7 (B2 base `None` allowed with a restore): the unit test
  (src/commands/import.rs:632).

Verified by:
- `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings`
  are clean.
- Suites: `--test git` 5/5, `--test import` 11/11, `--test doctor`
  16/16, `--test index` 17/17, lib 156/156. The full `cargo test` is
  483 passed, 0 failed.
- `bash scripts/docs-check.sh`: ok.
- `cargo build` was run after the mutant loop.

Not done in this round: B3 (waits for the go). Full `just check` will be
run after the B3 rebase.

Touched outside the original file list:
- `engine/src/index/mod.rs` (`git_info` only, Q4) and `docs/TESTING.md`
  (two rows, one new row, N3). Both were assigned in the review.

## Final round (review round 2: F1, F2; B3)

Done:
- **F1**: `check_toplevel` asks for `--show-toplevel --absolute-git-dir`.
  The git dir must be `<root>/.git`, or a linked work tree
  `<repo>/worktrees/<name>` whose `gitdir` file names `<root>/.git`. A
  `.git` file whose `gitdir:` names another repository's git dir gives
  "the logbook's .git points at another repository's git directory
  (<path>)". In that case the autocommit commits nothing and prints one
  warning, and doctor reports the `git` check as degraded with that
  reason, without a fix line.
- **F2**: `Undo::is_sane(root)` refuses a kept undo when any existing part
  of one of its paths under the logbook is a symbolic link (checked with
  `symlink_metadata`). The refusal then points to `git -C … status`.
- **B3**:
  - Rebased onto main (WP-064 merge `b275a0f`, `9ea4ec8`).
    - `git.rs` and `index/mod.rs` conflicts: I kept this WP's versions,
      in which every git call goes through `git::command()`, so WP-064's
      direct `run_in_engine_group` calls there are replaced.
    - `CHANGELOG.md` and `memory/pitfalls.md`: both sides kept, main's
      entries first.
  - New `sys::run_command_in_engine_group(Command, Duration)`
    (`run_with(.., Group::Engine)`). `git.rs` runs all its env-controlled
    commands through it, while every other program keeps
    `run`/`run_command` and its own group.
  - The de page 09 source line is restamped to the rebased en commit
    `ec1a8a5`.
- Docs: the SPEC autocommit paragraph (git-dir rule, process group) and
  the kept-undo rule (no symlinked part); TESTING rows for the new tests.

Tests:
- `tests/git.rs`:
  - `a_dot_git_file_pointing_at_another_repository_commits_nowhere`: the
    parent's HEAD and index are unchanged, there is one warning, and
    doctor is degraded.
  - `a_linked_work_tree_of_the_logbooks_repository_is_committed`
    (`git worktree add`): the commit lands on the work tree's branch.
  - `git_runs_in_the_engines_process_group`: the `git` link in the stub
    dir is removed first (never written through), then replaced by a
    wrapper that appends `/proc/$$/stat` to a log and `exec`s the host's
    git. For `log` and `doctor`, every git call's process group equals the
    test process's group, which seldon inherits. A snapper stub, recorded
    the same way, leads its own group.
- `tests/import.rs`: `a_kept_undo_through_a_symlinked_folder_is_not_offered`
  (`memory/` replaced by a link to a folder outside the logbook; no
  command is printed and the outside folder is unchanged).

Mutants (each one fails its test; each was restored from the index or
from a copy, checked with `cmp`):
- G1, any git dir accepted: `a_dot_git_file…` fails.
- G2, the linked-work-tree branch removed: `a_linked_work_tree…` fails.
- G3, the symlink check dropped: `a_kept_undo_through_a_symlinked_folder…`
  fails.
- B3 mutant, `git.rs` back on `sys::run_command`: `git_runs_in…` fails at
  tests/git.rs:461 ("git rev-parse ran in its own group").

Verified by:
- `just check`: exit 0 on `925b07c`, after the rebase. That covers fmt,
  clippy, the tests and `check-watch` (1017 passed, 0 failed, 60 suites),
  plus packaging, install, schema, docs-check (ok, 14 translated pages),
  plugin-validate, qmllint and plugin-test.
- Nothing else ran in this worktree during the run.

Touched outside the original file list (all assigned in the reviews):
- `engine/src/sys.rs` (one new function, B3);
- `engine/src/index/mod.rs` (`git_info`, Q4);
- `docs/TESTING.md` (rows).
