# WP-175 — Handover

Branch `wp/175-slow-paths` from `next` (7f6341c4, WP-174 merged).
Not pushed (the orchestrator pushes).

## What was done

**F7: a `.git/HEAD` (or `.git`) that is no regular file.**
(`engine/src/logbook/git.rs`, `commands/mod.rs`, `index/mod.rs`,
`commands/index.rs`, `commands/watch.rs`, `commands/doctor.rs`, `sys.rs`)

- `git::check_files(root)` uses only file metadata, with links followed
  as git follows them. `.git` must be a directory or a regular file. The
  `HEAD` of the git directory it leads to must be a regular file:
  `.git/HEAD`, or `<gitdir>/HEAD` for a `.git` file (`gitdir: …`, read
  bounded with `read_regular_string`). The error looks like this:
  "`<path>`: a FIFO | a device | a socket, not a regular file; git is not
  run in the logbook: make it a regular file and run the command again".
  The wording comes from the new `sys::irregular(meta)`, which
  `sys::not_regular` now uses too, so both say it the same way.
  A missing `HEAD` and a `HEAD` that is a directory are not refused:
  git does not block on them and answers at once.
- **The gate:** `git::run` and `git::ask` call the check first whenever
  they run in a root. On a refusal they return `Run::Failed(refusal)` and
  start no git. This covers every git call in the logbook, including
  the ones in `commit_all`, `doctor` and `import`, so the WP's "skip the
  remaining git calls of that command" holds. Each check is two or three
  `stat` calls and nothing is cached, so a long-running `watch` notices
  a fix.
- **Said once:** `git::say_refusal()` is a process-wide flag. The
  autocommit warning (`commands::warned`, now shared by `autocommit` and
  `autocommit_paths`) and the index rebuild's warning
  (`index::git_refused`, used by `index`/`status`, by the full
  `try_rebuild` and by `watch`) each print only if the other has not
  printed yet. `--json` `git.error` still carries the error every time.
  The hook's fast rebuild (`git_head_fast`) runs no git and says
  nothing, as before.
- **Doctor:** the `git` row checks `check_files` before its first git
  call. On a refusal the row is `degraded` with the message and a fix
  (`GIT_FILE_FIX`): what `HEAD` holds, and where the branch names are.
- **Test harness:** `within` in `tests/bounded_reads.rs` now starts the
  command in its own process group and kills the whole group on
  timeout. My first mutation run showed why: the old kill left a `git`
  child blocked on the FIFO after the test failed. I removed that
  orphan; it was in my temp dir.

**S1: a sparse ledger month in the fast rebuild.** `ledger_lines_exceed`
(`index/mod.rs`) treats a month whose size is over `LEDGER_MONTH_MAX`,
or whose size cannot be read, as "over the threshold" without reading
it. This is the reviewer's fix: the hook skips the fast rebuild, and
the full rebuild refuses the month with the reason.

**S2: a FIFO at `.seldon/logbook.toml`.** `Logbook::is_initialised` now
uses `exists()` instead of `is_file()`, so the file counts whatever its
type is. `Logbook::open` turns `InvalidInput` (no regular file) and
`FileTooLarge` into `Error::User` (exit 1), "`<path>`: a FIFO, not a
regular file; not read: make it a regular file and run the command
again". `init` on such a root says "`<path>`: not a regular file; init
does not write over it: remove it, or make it a regular file if this is
a logbook" (exit 1). Before, it would have gone on with "already a
logbook" or tried to write. Doctor's `logbook` row is that error.
Side effect: a directory at `logbook.toml` is now this refusal (exit 1)
instead of "not initialised" (exit 3).

**Docs:** SPEC-ENGINE §3 (after "Bounded reads": the `logbook.toml`
paragraph and the git paragraph) and §8 (the fast rebuild's cap), plus
a CHANGELOG Unreleased › Engine bullet.

Commits: `c8d13f1f` (S1), `a4427516` (S2), `6019f2d6` (F7), `d932529f`
(docs), then this handover.

## Tests (each with a time limit)

- Unit `logbook::git::tests::a_git_file_that_is_no_regular_file_runs_no_git`
  runs in a thread with a 20 s limit. Cases: a FIFO `HEAD`, a `HEAD`
  linked to `/dev/zero`, a FIFO `HEAD` behind a `.git` file, a FIFO
  `.git`. Each is refused with the exact message, and `query` and
  `commit_all` answer without spawning git. A regular `HEAD`, a linked
  one and a missing one, as well as no `.git` at all, pass.
- Unit `index::tests::a_month_over_the_cap_is_not_counted`: a sparse
  1 TiB month without a newline, in a thread with a 10 s limit. A month
  exactly at the cap is still read and counted.
- Integration `bounded_reads::a_git_head_that_is_no_regular_file_holds_no_command`
  covers a FIFO `HEAD`, a `/dev/zero` `HEAD` and a FIFO `.git`. For
  each, `status`, `index` and `capture` exit 0 in under 8 s (below one
  10 s git timeout) and say the refusal exactly once (stdout and stderr
  together). `doctor` takes under 8 s, its `git` row is `degraded`,
  names the file and has the fix. The harness kills after 30 s.
- Integration `bounded_reads::a_logbook_toml_that_is_no_regular_file_is_named`
  covers a FIFO and a `/dev/zero` link. `status` exits 1, names the file,
  and does not mention `seldon init`. `init` exits 1 and leaves the file
  alone. Doctor's `logbook` row is `error` and names it.

**Mutation checks** (source restored after each):

- Without the size bound, the S1 unit test fails at its 10 s limit.
- Without the gate, the F7 integration test fails at the 30 s harness
  limit. On the second run, with the process-group kill, no `git` was
  left behind.

## How it was verified

All runs used `CARGO_TARGET_DIR=jax-seldon-private/gates/target-wp175`,
`TMPDIR=gates/tmp-wp175` and a private 0700
`XDG_RUNTIME_DIR=gates/run-wp175`, all on disk, with `-j 4` /
`CARGO_BUILD_JOBS=4`.

- `cargo fmt --check` and `cargo clippy --all-targets -D warnings` are
  clean. `cargo test` passes, exit 0, no failures (log:
  `gates/test-wp175-r1.log`).
- `SELDON_FULL_CHECK=1 just check` passes, exit 0, "check: ok"
  (2784 tests passed, 0 failed, including the watch feature, plus
  packaging, install, deploy, guard, runtime dir, schema, docs, plugin
  validate, qmllint and the Quickshell harnesses, e.g. ipc-restart 44/0,
  no leftovers in the private runtime dir). Log: `gates/check-wp175.log`.
- Probe with the debug binary in a scratch HOME on disk
  (`SELDON_TEST_GUARD`, private runtime dir), with a FIFO `.git/HEAD`:
  `status` 24 ms, `index` 13 ms, `capture` 119 ms, `log` 35 ms,
  `doctor` 62 ms. Each exits 0 and says the refusal once. No git process
  was left. For comparison, the review measured 40 s for `status` and
  30 s for `doctor`.
- The real `~/Seldon`, `~/.local/state/seldon` and `~/.config` were not
  touched. No guard block occurred.
- One rule slip: I staged-tree-checked commit `a4427516` in a source
  copy under the session scratchpad, which is in `/tmp`. The build and
  test temp dirs were on disk. The copy (38 MB) was removed right after.

**`just check-rss` fails, and it fails on the base too.** The justfile
asks for it for changes under `index/` and `watch.rs`. Peak RSS on this
branch was 11 264 to 11 388 kB in four runs; the limit is 11 264 kB.
`next` before this WP (7f6341c4, a detached worktree on disk, same
target dir, three runs) gave 11 328 to 11 396 kB. The heap is the same
on both (RssAnon 908 → 3180 kB). So the overrun is not from WP-175: the
host or the toolchain sits at the edge of the limit. Log:
`gates/check-rss-wp175.log`.

**Not run:** `just check-perf`. It wants a tmpfs temp dir and a quiet
host, which conflicts with this brief's "never /tmp". The hot-path
change is one `fstat` per ledger month in the hook's fast rebuild, plus
two or three `stat` calls before each git call.

## Open questions

1. **Other files git opens** (`.git/config`, a loose ref such as
   `.git/refs/heads/main`, `.git/index`, `packed-refs`) are not checked.
   A FIFO there would still hold git until its timeout. The WP named
   `HEAD` and `.git`. Should that be extended, or accepted as is?
2. **"Once" is per process.** `watch` says the refusal at its first
   rebuild only, not at every rebuild. That seemed right for a log;
   please confirm.
3. **Exit codes.** `import --apply` on such a logbook stops with exit 2:
   "cannot read the logbook's git status: cannot run git: …"
   (`commit_pending` maps a git error to an engine error). Make it
   exit 1 like the other refusals?
4. A directory at `.seldon/logbook.toml` now exits 1 instead of 3 (see
   S2). I believe that is right, since `init` cannot help there either.
5. `check-rss` sits above its 11 MB limit on `next` itself on this
   host (see above). Raise the limit, or open a WP to look into it?
6. The plugins collector's git queries of a plugin clone (outside the
   logbook) do not go through this gate. That is out of scope here.

## Round 2 (review 1: APPROVE, small fold-in)

Commits `206f5043` (engine), `1c1634d5` (docs), then this section.

- **N1:** the CHANGELOG headline now reads "No git waits on a FIFO at
  `.git/HEAD`". It also says that other files git opens (`.git/config`,
  a loose ref) are not checked and still cost one git timeout. Per Q1
  this is accepted; SPEC §3 states the same limit, unchanged.
- **N2:** for a `.git` that is no directory, the refusal now ends "make
  it a directory or a `gitdir:` file and run the command again". For
  `HEAD` it still says "make it a regular file". SPEC §3 mentions both.
- **N3:** `check_files` now uses the file type (`meta.is_dir()`) to
  decide that a `HEAD` directory is no refusal, instead of comparing
  the text of `sys::irregular`. The unit test gained that case: a
  `HEAD` that is a directory passes.
- **N5:** `GIT_FILE_FIX` says the branches are under `refs/heads` and in
  `packed-refs` of the repository's git directory, which is `.git` or,
  for a linked work tree, the main work tree's `.git`. It also says
  `.git` is a directory or a `gitdir: <path>` file.
- **Q3:** `import --apply` checks `git::check_files` first in
  `commit_pending`, so a refused `.git`/`HEAD` stops with exit 1 before
  anything is written. Before, it exited 2 with "cannot read the
  logbook's git status: cannot run git: …". New integration test
  `bounded_reads::an_import_into_a_logbook_with_a_fifo_head_is_refused`:
  a FIFO `HEAD`, the harness's time limit, under 8 s, exit 1, the
  message starts with the path, no import marker written. Mutation
  check: without the new line the test fails (exit 2 instead of 1).
- N4 (no `fix` on the `logbook` row) was not in the fold-in list and is
  not done.

**Verified:** `cargo fmt --check` and `cargo clippy --all-targets -D
warnings` are clean. `cargo test -j 4` passes, exit 0: 1388 passed,
0 failed. Log: `gates/test-wp175-r2.log`. Same environment as round 1:
target, TMPDIR and a private 0700 runtime dir under `gates/`, all on
disk.

### Q4: which commit on `next` raised `check-rss`

**Method:** a detached worktree on disk (removed afterwards). For each
commit: `cargo test --profile bench --features watch --test watch
rss_stays_under -- --nocapture`, which prints idle, after-rebuild and
peak RSS plus the heap (RssAnon). Then the bench binary, read with
`size -A`. Points measured: `main` (603d832b; `next` contains it) and
all 38 merges on `next`'s first-parent line. The peak was taken three
times per commit. Scripts and logs are in `gates/`: `rss-at-wp175.sh`,
`rss-bisect*-wp175.log`, `rss-merges-wp175.log` (peaks ×3),
`rss-text-wp175-table.txt` (text, heap, peak per merge).

**Result: there is no single culprit. The binary grew steadily over
about ten WPs, and the heap grew a little.** The limit is 11 264 kB.

| | `.text` | heap after rebuild | peak RSS (runs) |
|---|---|---|---|
| `main` 603d832b | 5 802 782 B | 2 816 kB | 10 268 to 10 432 kB |
| `next` 7f6341c4 | 6 588 222 B (+785 kB) | 3 184 kB (+368 kB) | 11 196 to 11 460 kB (about +1 000 kB) |

The peak is noisy. Between runs of the same commit it varies by up to
about 300 kB, and consecutive merges with no engine change differ by
±300 kB (WP-164 +308, then WP-168 −296). So a bisect on the peak
alone points at random merges. My first two bisects named cd75339a
(WP-162, plugin only, no engine change) and 49d9abb9 (a work file), and
both are noise. `.text` and the heap are deterministic. The peak
follows them: about +1 000 kB = +368 kB heap + the file-backed share of
the larger code.

Largest steps in `.text` (merge into `next`, then Δ):

- WP-124a (bulk triage, engine): +146 kB
- WP-102a (import task files): +105 kB
- WP-156 (desk steps aside, sessions): +94 kB
- WP-139 (recent ~/.config edits): +69 kB
- WP-166 (crash inbox): +54 kB
- WP-127 (optional index fields): +51 kB
- WP-113 (code the collectors could not see): +42 kB
- WP-120 (contract v2): +37 kB
- WP-159 (two-pass redaction): +32 kB
- the rest: < 30 kB each

Largest steps in the heap: WP-137 (transaction packages, `meta.txStatus`)
+132 kB, WP-127 +80, WP-120 +52, WP-113 +36, WP-141 +32. The rest are
±12 kB or less.

**When the limit was first crossed:** the median peak first reaches
the limit around the merge of WP-137 (9577717f: 11 260 / 11 128 kB in
two passes). From WP-156 (f5f7581b) on, every merge's median is above
it. WP-138's merge (3535cc11) does not compile the watch test at that
commit, so there is no number for it; it was fixed by 66257638 and
b4a11dba. WP-175 itself adds nothing measurable: branch 11 264 to
11 388 kB against `next` 11 328 to 11 396 kB, same heap.

The limit is not raised (per the brief). Options for the orchestrator:
raise it with a recorded reason (code growth, not a leak); or measure
heap and anonymous RSS instead of the peak, since they are stable while
file-backed pages track binary size; or open a WP on the size of the
largest additions.
