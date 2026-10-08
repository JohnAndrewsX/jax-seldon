# WP-154 — Plan

Branch `wp/154-git-rules`, from `next` aaf7a0a5; merges into `next`.

The logbook is the user's repository: `git add`, `git commit` and
`git init` keep the user's config, hooks, signing, filters, transport and
terminal exactly as today. Only read-only queries change, and only in
what they may reach (no network) and keep (bounded output).

## Rule 1 — every run of a program is capped

- `sys::run`, `run_in_engine_group`, `run_command` and
  `run_command_in_engine_group` take a `cap: usize` (bytes kept of each
  output pipe). `run_command_capped` folds into `run_command`.
- A stdout over the cap is no answer: a new variant `Run::Cut`. No
  caller can parse a cut stdout by mistake. stderr over the cap is
  kept cut (only messages are read from it).
- Emptiness checks (`git status --porcelain` in `is_dirty`,
  `is_clean_path`, `index::git_info`) read `Cut` as "has changes":
  more than the cap of status lines proves the tree is not clean.
- Named caps:
  - `sys::OUTPUT_MAX` = 1 MiB, the default: every `logbook::git` call
    and every caller that reads a line or a message (omarchy-version,
    `pacman -Q omarchy`, the doctor probe, `omarchy hook install`).
  - `sys::WHOLE_OUTPUT` = `usize::MAX`, named at the calls that parse a
    whole list of a trusted system program: the dossier queries
    (package lists), the snapper list and config, the omarchy plugin
    list and catalog.
  - The plugins collector keeps its 64 KiB `GIT_OUTPUT_MAX`.

## Rule 2 — read-only git queries never reach the network

- `logbook::git` gets a second command builder for queries:
  `GIT_ALLOW_PROTOCOL=none`, `GIT_NO_LAZY_FETCH=1` and `--no-lazy-fetch`
  in argv. Everything else (user config, hooks path, terminal) stays.
- Covered: `query` (the index's HEAD and status), `head`, `is_dirty`,
  `is_clean_path`, `branches`, `check_toplevel`, `version`, and the other
  read-only calls the WP does not list but which are queries all the
  same: `is_detached`, `check_head`, `check_identity`, `has_identity`,
  and the `diff --cached --quiet` before a commit.
- Kept with transport: `init`, `add`, `commit`.
- `--no-lazy-fetch` "where git ≥ 2.44 takes it": no version probe (one
  more process per command, ~10 ms with the 10 ms poll). The query runs
  with the option; when git exits 129 and names the option as unknown,
  the engine remembers that for the process and runs the query once
  more without it. `GIT_ALLOW_PROTOCOL=none` alone already refuses
  every transport on an old git.

## Rule 3 — byte-level tests for every "refused" claim

Inventory of text scans of formats git parses, each with tests that
write the bytes (BOM, CRLF, lone CR, `\` continuation where the format
has one) and pin the answer:

- `plugins::includes` (config `[include`): pinned by WP-136 round 3;
  check for gaps.
- `plugins::head_from_files` (HEAD, loose refs, `packed-refs`): every
  refusal falls back to git, the authoritative parser. The dangerous
  direction is accepting what git refuses; compare with git's parser
  and fix any such case.
- `logbook::git::is_linked_work_tree_of` (`worktrees/<n>/gitdir`).
- `index::git_head_fast` (`.git` file `gitdir: `, `commondir`, HEAD,
  refs, packed-refs) — no refusal claim, but a gitdir file; pin what
  it reads against git.

## Acceptance tests

- A stub git that writes 100 MB to stderr: the engine keeps 1 MiB
  (end-to-end through `logbook::git`, visible in `seldon doctor --json`).
- A logbook that is a partial clone with an `ext::` promisor whose
  script writes a marker, an object removed that `git status` needs:
  `seldon status` never writes the marker. Fails with the guards off.
- A git that refuses `--no-lazy-fetch` still answers.
- Commits still run the user's hooks and sign with the user's
  `gpg.program` (a stub), and the commit carries the signature.
- `flock /tmp/seldon-check.lock just check` green.
