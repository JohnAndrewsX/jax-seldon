# WP-154 — Handover

Branch `wp/154-git-rules`, from `next` aaf7a0a5; merges into `next`.
`next` e6dc86f3 (a desk-view test helper and TESTING.md) is merged in as
3f471813, with no conflict. Plan: `PLAN.md` next to this file.

Commits:

- 3c03be55: engine, rules 1 and 2.
- 9aeba14d: engine, rule 3.
- 71c7759b: docs and the mutant runner.
- c7f9a57f: a clippy fix in a test.
- 3f471813: the merge of `next`.

The logbook stays the user's repository. `init`, `add` and `commit` run
exactly as before, with the user's config, hooks, signing, filters,
transport and terminal. Only read-only queries changed: what they may
reach (no network) and how much output they keep.

## What was done

### Rule 1: every program run is capped

- **`sys`.** `run`, `run_in_engine_group`, `run_command` and
  `run_command_in_engine_group` each take a `cap: usize`.
  `run_command_capped` is folded into `run_command`.
- **New variant `Run::Cut`.** A stdout over the cap is not an answer, so
  no caller can parse a cut stdout by mistake. A stderr over the cap is
  kept, cut (only messages are read from it).
- **Named caps:**
  - `sys::OUTPUT_MAX` (1 MiB): every `logbook::git` call, plus
    `omarchy-version`, `pacman -Q omarchy`, `snapper get-config`,
    `omarchy hook install`, the doctor probe and `Ctx::run`.
  - `sys::WHOLE_OUTPUT` (`usize::MAX`), named at each call: the dossier's
    package and unit queries, `snapper list`, `omarchy plugin list` and
    `catalog`. A cut list there would read as entries removed.
  - The plugins collector keeps its 64 KiB.
- **Emptiness checks.** `is_dirty`, `is_clean_path` and `index::git_info`
  now go through `git::status_is_empty`, which reads `Cut` as "has
  changes": more than 1 MiB of status lines proves the tree is not
  clean.

### Rule 2: read-only git queries never reach the network

- **`logbook::git::ask`** adds `GIT_ALLOW_PROTOCOL=none` and
  `GIT_NO_LAZY_FETCH=1` (`QUERY_ENV`), and puts `--no-lazy-fetch` first
  in argv. It is used by `query` (the index's HEAD and status), `head`,
  `is_dirty`, `is_clean_path`, `branches`, `check_toplevel` and
  `version`.
- **Beyond the WP's list,** the same rule applies to the other read-only
  calls: `is_detached`, `check_head` (`rev-parse`, `symbolic-ref`,
  `show-ref`), `check_identity` (`git var`), `has_identity`
  (`config --get`), and the `diff --cached --quiet` before a commit.
  When that diff cannot answer (exit 128), it counts as "changes", and
  the commit runs as usual, with transport.
- **Old git.** git before 2.44 rejects `--no-lazy-fetch`: it exits 129
  and names the option in stderr; the message may be translated, the
  option name is not. The engine then asks once more without the
  option, and leaves it out for the rest of the process (an
  `AtomicBool`). This costs nothing on a current git, where a version
  probe would cost one extra process (~10 ms) per command. On an old
  git, `GIT_ALLOW_PROTOCOL=none` alone still refuses the fetch.

### Rule 3: byte-level tests for every "refused" claim

Inventory of the engine's text scans of formats git parses. Each git
answer below was checked against git 2.55 in a scratch repository.

- **`plugins::includes`** (the `[include` scan of a config): already
  pinned by WP-136 round 3 (BOM, lone CR, CRLF, same line, continuation).
  Added: the 1 MiB limit to the byte.
- **`plugins::head_from_files`**: three mismatches fixed. In each, the
  engine read a HEAD that git does not read:
  - a `packed-refs` with CRLF lines, with no final LF, or with a `#` line
    further down (git refuses or does not resolve these; `lines()` read
    them);
  - a 64-digit object name in a SHA-1 clone (git reads it as broken),
    and a 40-digit one in a SHA-256 clone;
  - a loose ref that exists but is refused (a link, or not UTF-8): the
    engine fell through to `packed-refs`; git reads the loose one.

  The function now reads only the bytes git itself writes:
  - HEAD: `ref: <name>` or 40 lower-case hex digits, with exactly one LF;
  - a strict `packed-refs` parser (`packed_ref`);
  - a config or `config.worktree` that names an object format is left to
    git;
  - ref names: no component may end in `.lock`, and the name may not end
    in `.`.

  Everything else goes to git as before. git reads some of these cases
  itself (CRLF HEAD, upper-case digits); those only cost one git
  process.
- **`index::git_head_fast`** (the hook path's HEAD): the `.git` file is
  now read as git reads it. It must start with exactly `gitdir: `, and
  only the CRs and LFs at the end are dropped. `commondir` is read the
  same way, and a `commondir` that exists but cannot be read means no
  HEAD. Before, `gitdir:/x`, two spaces, or a trailing space were all
  followed; git says "invalid gitfile format" or "not a git repository"
  for them.
- **`logbook::git::is_linked_work_tree_of`** (the
  `worktrees/<n>/gitdir` back link): no change needed. It already drops
  only CR and LF, as git does: git keeps a CRLF back link and calls one
  with a BOM or a trailing space "prunable". Pinned.

## Acceptance

| Acceptance | Test |
|---|---|
| a 100 MB stderr from a stub git is capped | `git::a_hundred_megabytes_from_git_are_capped`: a wrapper `git` writes 100 MB of `x` lines for `rev-parse --show-toplevel`; doctor's message is 1 MiB. `sys::tests::a_hundred_megabytes_of_stderr_keep_one_mebibyte` |
| a partial clone does not fetch during `seldon status` | `git::a_partial_clone_logbook_never_fetches_during_status` (details below) |
| commits still sign and run hooks as configured | `git::commits_still_sign_and_run_hooks_as_configured` (details below) |
| `just check` green | see Verification |

- **Partial clone.** The logbook gets `extensions.partialClone` and a
  promisor remote whose URL is an `ext::` script that would leave a
  marker; its own config sets `protocol.ext.allow=always`. HEAD's tree
  is removed. Then `status`, `doctor` and `import --apply` (through
  `is_dirty`) run, and the marker is never written. With the query
  rules off, git runs the script.
- **Signing and hooks.** The user's global `commit.gpgSign` and a
  `gpg.program` stub (it speaks git's status-fd contract) produce a
  `gpgsig` header in the commit. The user's `core.hooksPath`
  `pre-commit` and `post-commit` run, and they see neither
  `GIT_ALLOW_PROTOCOL` nor `GIT_NO_LAZY_FETCH`.

Also added:

- `git::a_git_without_no_lazy_fetch_still_answers`: a wrapper refuses the
  option the way old git does. `status` still gets its HEAD, doctor says
  `ok`, and the option is tried once per process.
- `plugins::tests::head_files_are_read_byte_for_byte_as_git_writes_them`
- `status::the_fast_rebuild_reads_git_files_as_git_does`
- `logbook::git::tests`: query and commit argv and env as literals, the
  old-git refusal, a status over the cap, the back link byte for byte.
- `tests/common` `Env::wrap_git`: it removes the `git` link before it
  writes the wrapper. `Env::stub("git", …)` would have written through
  the link into the host's git (CI runs as root). The existing
  `git_runs_in_the_engines_process_group` wrapper now logs the verb
  after `--no-lazy-fetch`.

## Decisions (what the WP left open)

1. **The cut is a variant, `Run::Cut`, not a flag.** A flag in the
   return value or in `Run::Exited` can be ignored by a caller that
   pattern-matches `Exited { code: Some(0), stdout, .. }`; a variant
   cannot.
2. **Whole output for trusted list parsers.** Besides the pacman and
   dossier queries the WP names, the snapper list and the omarchy plugin
   list and catalog keep the whole output: a cut list would read as
   snapshots or plugins removed.
3. **The no-network rule covers every read-only query,** not only the
   six the WP names. `init`, `add` and `commit` keep transport (hooks,
   signing, LFS-style filters).
4. **No version probe for `--no-lazy-fetch`:** a retry on refusal
   instead (see rule 2).
5. **No `--no-renames` or other changes to `status`.** In the
   partial-clone test, `git status` fails without HEAD's tree. `index`
   then leaves `logbook.git` out, as for any failing query, and the
   import refuses ("cannot read the logbook's git status"). That is
   honest for a repository missing objects locally.
6. **SHA-256 plugin clones are always read by git** (one process,
   ~10 ms per capture each). Reading their files would need a parse of
   `extensions.objectFormat`, which is one more config scan with edge
   cases.
7. **`git_head_fast`'s HEAD and ref parsing is unchanged** (it trims
   all white space). Only the `.git` file and `commondir`, the "gitdir
   files" the WP names, were made exact. The value is display only, and
   the next full rebuild replaces it.

## How it was verified

- **Gate:** `flock /tmp/seldon-check.lock just check` on 3f471813 (dev
  host; Omarchy installed, so the host steps ran): `check: ok`, exit 0 (log `check-wp154-1.log`, outside
  the repository). It waited on the lock behind two other agents' checks.
  desk-view: 1556 passed, 0 failed; the log has no ENOSPC and no "No
  space left" line (0 matches). `/run/user/1000` was at 100 % after this
  run (orchestrator note); nothing there was touched, and the check was
  not run again.
- **Perf** (bench profile, before the merge, which touches no engine
  code). Every budget holds:
  - `status` at the stated scale: 51.1 ms (budget 100);
  - hook at 10 000 lines: 0.76 / 1.62 ms (budget 5);
  - plugins capture warm: 22.6 ms (budget 60);
  - plugins capture with cold trees: 88.4 ms (budget 150).
- **Mutants:** `python3 work/active/WP-154/mutants.py` (target
  `engine/target/mutants`, `--no-fail-fast`) runs 34 mutants, and all
  34 are killed.
  - The first runs (rules 1–2, then rule 3) left four survivors; each
    became a test:
    - `is_dirty` with transport: `import --apply` is now in the
      partial-clone test;
    - a bad ref name on another `packed-refs` line;
    - the config limit, which the test had sized from the constant
      under test;
    - an unreadable `commondir` when the git directory has the ref
      itself.
  - Removing only `QUERY_ENV`, or only the option, is caught by the
    literal argv/env unit test alone. The other layer still stops the
    fetch: defence in depth, as in WP-136.
- **Diff hygiene:** the added lines of `git diff next...HEAD`, grepped
  for `/home/`: none.

## Not done / notes

- **Guard block.** The guard hook blocked a shell probe that set an
  `ext::` URL with `git config` ("can run a program"). I did not reword
  around it. The partial-clone behaviour is proven by the Rust test
  instead, which builds the same repository from Rust (as in WP-136
  round 2).
- **Old git.** No run against a real git older than 2.44; the refusal is
  simulated by a wrapper that prints git's message and exits 129.
- **`seldon watch`** (feature `watch`) also calls `index::git_info`; it
  is covered by `just check-watch` and was not run separately.

## Open questions (orchestrator)

1. WP-136's open question stands: should `packaging/PKGBUILD` depend on
   `git>=2.44`? The logbook no longer needs it (the retry), but the
   plugins collector still fails closed on an older git (no commit
   lists).
