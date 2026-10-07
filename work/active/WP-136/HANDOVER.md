# WP-136 — Handover

Branch `wp/136-plugin-commits`, from `next` e9851606; merges into `next`.
Plan: `PLAN.md` next to this file (the HEAD-from-files step came after
it, see Decisions 7).

## What was done

**Engine** (`engine/src/collectors/plugins.rs`; `logbook/git.rs` makes
`REPOSITORY_VARS` public):

- A third-party plugin whose directory is its own git clone (`<dir>/.git`;
  never a repository further up) keeps its **full HEAD** in the cursor
  (`plugins.<id>.head`).
- `plugin-add` of a clone: `meta.git: "clone"`, detail
  `<version>, installed by git clone`.
- `plugin-update` (same trigger as before: a version change of a
  third-party plugin) whose HEAD moved: two read-only git queries,
  `rev-list --left-right --count OLD...NEW` and `log -z --max-count=20
  --format=%<(200,trunc)%s RANGE`:
  - `pull` (old HEAD an ancestor of the new): the commits that came in;
  - `rollback` (new HEAD an ancestor of the old, a reset back): the ones
    that left;
  - `reset` (neither: history replaced): the ones that came in, and both
    counts.
  `meta.git` names it; `meta.commits` holds at most 20 subjects, newest
  first, one per line; the detail is
  `1.2.0 → 1.3.0, pulled 3 commits: Release 1.3.0 …`
  (`rolled back 2 commits: …`, `reset: 1 commit in, 1 out: …`; no `…`
  for one commit).
- Subjects: control characters → spaces, direction and invisible format
  characters dropped (ADR-0038's set), trimmed, **redacted with the
  logbook's redactor, then clipped** to 100 characters with `…`; empty →
  `(no subject)`. The ledger redacts once more on append, as for every
  event (§7).
- **git hardening** for every git call of the collector, the existing
  version fallback included: fixed argv, `-C <clone>`, `--no-pager`,
  `--no-replace-objects`, `-c core.hooksPath=/dev/null`,
  `-c core.fsmonitor=false`, `-c protocol.allow=never`,
  `-c log.showSignature=false`, `-c color.ui=false`;
  `GIT_CONFIG_NOSYSTEM=1`, `GIT_CONFIG_GLOBAL=/dev/null`,
  `GIT_TERMINAL_PROMPT=0`, `GIT_OPTIONAL_LOCKS=0`, `GIT_NO_LAZY_FETCH=1`,
  the repository variables (`GIT_DIR` …) removed, the clone's parent as
  `GIT_CEILING_DIRECTORIES`; own process group, 2 s per call;
  `--end-of-options` before revisions, and a cursor head that is not a
  hex object name never reaches git. git missing, failing or timing out
  → the same event without `meta.git`/`meta.commits`.
- **HEAD without a process**: `.git/HEAD` (detached) or a plain
  `refs/heads/…` ref, loose or in `packed-refs` (≤ 1 MiB), read with
  `sys::read_small_file` (no links, bounded). Anything else (a `.git`
  file, reftable, a link, an odd ref name) → `git rev-parse HEAD --short
  HEAD`, which also gives the short version fallback in one call. A HEAD
  unreadable for one capture keeps the last one.

**Plugin** (`plugin/Model.js` `eventDetail`): a key/value row right after
*What*: `Commits` (or `Rolled back` when `meta.git` is `rollback`) with
`meta.commits` as plain text (KeyValues renders `\n` as line breaks,
`Text.PlainText`). No row when the event has no such string.

**Fixtures**: the 09-24 `plugin-update` of weather-plus is now a pull of
three commits (`meta.git`, `meta.commits`, the new detail); the index
fixtures regenerated with `scripts/validate-fixtures.py --write-index`;
`fixtures/README.md` story row. `plugins::enable_disable_remove_and_update`
now builds weather-plus as a real clone and reproduces the fixture line
byte for byte (detail and meta), so the fixture is the engine's output.

**Docs**: SPEC-ENGINE §2 (cursor) and §4 (plugins), SPEC-PLUGIN (event
detail), the guide's configuration page (en, de), TESTING.md.

## Decisions (what the WP left open)

1. **`meta.commits` is one string, subjects joined by `\n`, not an
   array.** `event.schema.json` allows only scalar `meta` values
   (`additionalProperties: string|number|boolean|null`); an array would
   be a schema change with an ADR. A string needs no schema change, no
   index field and no `contractVersion` bump; ADR-0038 is not needed.
   `meta.git` is a string too. Both are conventional `meta` keys like
   `hashFrom`/`hashTo`.
2. **The index clips `meta.commits` like every meta string** (ADR-0025,
   256 bytes of JSON, `meta.truncated`): a long list shows its first
   subjects and the marker in the desk; the ledger keeps all ≤ 20. The
   desk's existing "(clipped in the index …)" note covers *What* only;
   the Commits row shows the marker text itself. See open question 1.
3. **"First subject" = the newest** (git log order); the detail keeps the
   version step in front (`<from> → <to>, `): the desk shows no
   `meta.from/to`, so dropping it from the detail would lose it.
4. **Names**: `clone`, `pull`, `rollback`, `reset` (a backward move is a
   rollback whatever command made it; "reset" is reserved for a replaced
   history, the one case where both sides have commits).
5. **The trigger is unchanged** (a version change). A HEAD that moves
   without a version change (a manifest with a version that the author
   did not bump) gives no event until WP-113's tree hash lands; then its
   tree-change `plugin-update` should carry the commits too (see
   overlap). Kept out to stay separable.
6. **Subject length 100 characters**; git's output bounded at 200
   columns per subject before that (`%<(200,trunc)`), and 20 subjects
   (`--max-count` and `.take`).
7. **HEAD from files, git as fallback** (after measuring, below): one git
   process costs about 10 ms in a capture because `sys::run_with` polls
   `try_wait` every 10 ms; eight clones made a capture ~84 ms slower.
8. **`GIT_CONFIG_GLOBAL=/dev/null`** besides `GIT_CONFIG_NOSYSTEM`: the
   answer depends on the clone only, a user's `core.pager`,
   `log.showSignature`, `core.fsmonitor`, `include.path` never apply, and
   tests never read the real git config. Consequences: a user's global
   `core.abbrev` no longer shapes the short-HEAD version (for such a user
   one `plugin-update` of a version-less clone after the upgrade, rare),
   and a global `safe.directory` no longer covers a clone owned by
   another user (git refuses; the last version is kept).
9. **Pre-existing fix**: the version fallback ran `git -C <dir> rev-parse`
   with the engine's environment; a `GIT_DIR` inherited from a git hook
   or `rebase -x` would have answered for every plugin. It now uses the
   hardened command.
10. **Fixture**: the existing 09-24 line changed instead of adding one
    (WP-113 adds lines to the same month file).

## Overlap with WP-113 (not on `next` yet)

- `PluginState`: WP-113 adds `tree` and `partial`, this WP `head` —
  adjacent fields, a textual conflict only.
- `collect_from`'s loop: WP-113 hashes the tree per third-party plugin;
  this WP computes `repo`/`head` and puts `repo` into `Seen` (which is no
  longer `Copy`; `seen_of` returns a reference). WP-113 also uses
  `seen_of(id)`.
- `diff()`'s update branch: WP-113 composes one `plugin-update` from a
  version and/or tree change. At the merge, call `step()` whenever that
  event fires and both heads differ — then a pull without a version bump
  names its commits. WP-113 leaves `.git` out of the tree ("the HEAD is
  the version"); the HEAD is now also in the cursor.
- Fixtures: both touch `fixtures/logbook/ledger/2026-09.{jsonl,md}` and
  every index fixture; after the merge run
  `python3 scripts/validate-fixtures.py --write-index`.
- WP-113's `engine/tests/capture_cost.rs` and this WP's ignored
  `capture_cost_of_clone_heads` (in `collectors_user.rs`) do not collide.

## Capture cost (measured)

`cargo test --profile bench --test collectors_user capture_cost_of_clone_heads -- --ignored --nocapture`,
dev host, medians of 31 (11 for the update):

| plugins capture | median |
|---|---|
| 8 third-party plugins with a manifest version, no clone | 20.62 ms |
| the same 8 as clones (HEAD read from files) | 20.69 ms |
| **delta** | **+66 µs per capture, ~8 µs per clone** |
| 8 clones, one update whose HEAD moved (2 git queries) | 41.4 ms (+~20 ms, only on that capture) |

Before the files reader (git `rev-parse` per clone): 104.6 ms, +84 ms.
The 20 ms base is the two `omarchy` stub calls at ~10 ms each — the same
10 ms poll. That poll is a general cost of every program the engine runs
(follow-up suggestion, not changed here: it touches every collector).

## How it was verified

- `flock /tmp/seldon-check.lock just check` on 871661a6 (dev host, Omarchy installed, so the host steps `plugin-validate`, `qmllint` and `plugin-test` ran): `check: ok`, exit 0 (log `check-wp136-r1.log`, outside the repository).
- New tests: `engine/tests/collectors_user.rs` `plugin_commits::` (4
  tests: pull/rollback/reset/one/25 commits, packed ref and detached HEAD,
  idempotency; hostile subjects; no clone, a pre-WP-136 cursor, an
  unreadable HEAD for one capture, a head git does not have, an
  option-shaped head; an empty `.git` inside another repository) and
  unit tests in `collectors/plugins.rs` (git argv and environment as
  literals, a slow git cut off at the timeout, heads that are not object
  names never reach git, HEAD from files incl. traversal/dot/`.lock`/tag/
  reftable names, a `.git` file and a linked HEAD, summaries, subject
  cleaning); `tests/plugin/model.test.js` (the Commits row, Rolled back,
  no row without a string). Tests build clones in a temp home with no
  system or global git config; they skip without git.
- Mutants: `python3 work/active/WP-136/mutants.py` (target
  `engine/target/mutants`): 22 mutants, 20 killed; 2 survive by design —
  the subject count is bounded twice (`--max-count` and `.take`), and a
  first-party clone's HEAD would only sit in the cursor (no event reads
  it).
- No network: `protocol.allow=never`, `GIT_NO_LAZY_FETCH=1`, only
  `rev-parse`, `rev-list`, `log`; the tests run offline. **Corrected in
  round 2:** `protocol.allow=never` is only the default for protocols the
  config does not name, and a clone's own `protocol.ext.allow=always`
  beats it. In round 1 only `GIT_NO_LAZY_FETCH=1` stopped a lazy fetch.
  Round 2 adds `--no-lazy-fetch` and `GIT_ALLOW_PROTOCOL=none`, and a
  test proves the fetch command never runs.
- `git diff next...HEAD` added lines grep'd for `/home/`: none.

## Not done

- No run against a real third-party plugin on the test host (Omarchy
  plugin installed by `omarchy plugin add`, then `git pull` in it); the
  tests use real git clones in a temp home.
- The schema's `meta` description (conventional keys) does not list
  `git`/`commits`: a description edit in `schema/` is still a schema
  file change; see open question 2.

## Open questions (orchestrator)

1. Should `meta.commits` be exempt from the 256-byte index clip (or get a
   larger budget) so the desk shows all ≤ 20 subjects? That is an
   ADR-0025 amendment (index size: ≤ 20 × ~100 characters per plugin
   update event).
2. Name `git` and `commits` in `event.schema.json`'s conventional-keys
   description with the next ADR that touches the schema (ADR-0038's PR
   on `next`)?
3. When WP-113 merges: attach the commits to its tree-change update as
   described under Overlap (a few lines in `diff()`).

## Round 2

Stage-1 review `WP-136-review-1.md` sent it back for B1 plus N1–N4. Base
is unchanged (`next` e9851606). Commits: 421ac6c9 (engine) and 73071a12
(docs, mutants runner).

### What changed

- **B1, stderr flood from grafts.**
  - `GIT_GRAFT_FILE=/dev/null` is set after the repository variables are
    removed, so the clone's `info/grafts` is never read.
  - New `sys::run_command_capped(cmd, timeout, cap) -> (Run, bool)`. It
    keeps at most `cap` bytes of each pipe; the drain thread reads 8 KiB
    chunks and drops the rest, so the writer never blocks and memory
    never grows. The flag says whether stdout was cut.
  - The plugins `Git` runner uses it with 64 KiB. A cut stdout is no
    answer: the event has no commits and keeps its version detail.
  - Every other caller keeps `run_command` and the whole output, because
    a package list or a dossier query may be large. That answers the
    review's question 3: the cap is opt-in in `sys.rs`, used here only.
- **N1, lazy fetch through `ext::`.**
  - `--no-lazy-fetch` is now in argv on every query, and
    `GIT_ALLOW_PROTOCOL=none` is set. `none` is no protocol's name, and
    the variable overrides any configuration.
  - The code comments (`GIT_OPTIONS`, `GIT_ENV`) and SPEC-ENGINE §4 now
    say what each setting stops, including that `protocol.allow=never`
    is only a default.
  - On git older than 2.44, git refuses the option. Every query then
    fails closed: no commit list, and no git-derived short version (a
    version-less clone keeps its last version). See open question 1.
- **N2, reading outside the clone.** New `GitDir` with three values:
  `None`, `Contained` and `Outside`.
  - `Contained` means `.git` is a real directory per `symlink_metadata`,
    there is no `objects/info/alternates`, no `commondir`, and no
    `[include` / `[includeIf` section (a plain, case-insensitive scan) in
    `config` or `config.worktree`.
  - A config that cannot be read (a link, a FIFO, not UTF-8, over 1 MiB)
    counts as `Outside`.
  - For `Outside`, nothing is read: no HEAD from files, no git, no git
    version. The `plugin-update` keeps its version step and appends
    `commit history not read (the repository points outside the plugin
    folder)` (`plugins::OUTSIDE`).
  - That answers the review's question 2: a linked worktree or a
    symlinked `.git` is refused. A plugin directory that is itself a link
    to a checkout still works, because its `.git` inside is a real
    directory.
  - The FIFO `include.path` case from N4 is gone too: such a config is
    `Outside`, so git is never started for it.
- **N3/N4.**
  - Subjects: U+2028 and U+2029 now become spaces, like control
    characters.
  - `-c i18n.logOutputEncoding=UTF-8` is passed, and output that is not
    UTF-8 is read lossily (as `sys` always did).
  - Tests now kill R1 (the fake git's child is killed with it), R2 (the
    timeout is pinned at 2 s) and R7 (U+2066–U+2069 dropped).

### Tests added

All probes are Rust tests. The one temporary probe file
(`engine/tests/wp136_probe.rs`) was deleted by its explicit path and
never committed.

- `plugin_commits::a_grafts_file_is_not_read`: a graft line that makes
  the new HEAD a root (with grafts read, the pull would read as a
  reset), plus 200 000 bad lines. The capture completes in time and the
  event names the pull "C\nB\nA".
- `plugin_commits::a_partial_clone_never_fetches`: a partial clone with
  `extensions.partialClone`, a promisor `remote.origin.url =
  ext::<script that writes a marker>`, the clone's own
  `protocol.ext.allow=always`, and the old HEAD's object removed. The
  marker is never written, and the event has no `meta.git` or
  `meta.commits`. With the three lazy-fetch guards removed, git runs the
  script and this test fails (mutant `R2-N1 lazy fetch guards off`).
- `plugin_commits::a_repository_pointing_outside_names_no_commits`: one
  fresh clone per case — a linked `.git`, a `gitdir:` file,
  `objects/info/alternates`, `commondir`, `include.path`, `includeIf`.
  Each gives detail `<from> → 2.0.0, commit history not read (…)`, no
  `meta.git` and no `head` in the cursor.
- `plugin_commits::the_log_is_utf8_whatever_the_clone_says`: the clone's
  `i18n.logOutputEncoding=UTF-16` changes nothing. A Latin-1 subject
  comes out as git converts it ("Café au lait"; git itself re-encodes
  that byte).
- Unit tests:
  - `sys::tests::a_capped_run_keeps_the_head_of_a_flood_and_reads_the_rest`:
    16 MiB to stderr and 1 MiB to stdout; 64 KiB kept of each, and the
    cut is flagged.
  - `a_flooding_git_is_no_answer_and_costs_no_memory`: 32 MiB to stderr.
  - `a_slow_git_is_killed_with_what_it_started`: `sleep 3 & … wait`; the
    child is gone within 1 s.
  - `the_timeout_is_two_seconds_and_the_program_git`.
  - `bytes_that_are_not_utf8_are_read_lossily`: a fake git prints `\351`.
  - `a_git_dir_that_points_outside_is_not_read`: every `GitDir` case, and
    git is never asked.
  - Subject cleaning: isolates and the line and paragraph separators.
- The argv/env literal test now covers `--no-lazy-fetch`,
  `i18n.logOutputEncoding`, `GIT_GRAFT_FILE` and `GIT_ALLOW_PROTOCOL`.

### Mutants

`python3 work/active/WP-136/mutants.py` (target `engine/target/mutants`)
runs 36 mutants: 34 killed, 2 survive as in round 1 (the double count
bound; a first-party clone's HEAD is cursor-only).

- **Runner fix:** the round 1 runner did not pass `--no-fail-fast`, so
  whenever a unit test killed a mutant, the integration tests never ran.
  The round 1 kills stand; the runner now reports every failing test.
- **Behaviour tests that fail with their guard removed:** grafts (B1),
  the `ext::` fetch (N1), the outside cases (N2), the log encoding (N4).
- **Single-layer mutants:** removing only `GIT_ALLOW_PROTOCOL`, or only
  the lazy-fetch pair, is caught by the literal argv/env test alone. The
  other layer still blocks the fetch; that is the intended defence in
  depth.

### Capture cost (re-measured)

Same test, `--profile bench`, dev host.

| plugins capture | median |
|---|---|
| 8 plugins, no clone | 20.62 ms |
| 8 plugins, 8 clones | 20.76 ms |
| delta | +138 µs per capture, ~17 µs per clone |
| one update with commits | 41.6 ms |

The delta was +66 µs in round 1. The difference is the `GitDir` check,
which stats three paths and reads `config` once per clone per capture.

### Verification

`flock /tmp/seldon-check.lock just check` on 73071a12 (dev host, host steps included): `check: ok`, exit 0 (log `check-wp136-r2.log`, outside the repository). `git diff` added lines grep'd for `/home/`: none.

### Open questions (round 2)

1. Should `packaging/PKGBUILD` depend on `git>=2.44` (for
   `--no-lazy-fetch` and `GIT_NO_LAZY_FETCH`)? Without it, an older git
   fails closed (no commit lists, no git-derived versions), which is
   safe but quiet.
2. Review question 1, the guard and the probes: this round ran every
   probe as a Rust test that sets the environment from Rust. No guard
   block occurred.
3. Round 1's open questions 1–3 stand: the index clip of
   `meta.commits`, the schema description of `git`/`commits`, and
   attaching the commits to WP-113's tree-change update.

## Round 3

Fable stage 2 confirmed rounds 1–2 and asked for one small round: S1
and P7. Commit 918c0de7.

- **S1, an `[include]` hidden from the scan.** `includes()` used to look
  at the start of each line. It now looks for `[include` anywhere in the
  config text, case-insensitive. A line scan misses what git's parser
  reads as a section header:
  - after a UTF-8 BOM (git skips EF BB BF; `trim_start` does not);
  - after a lone CR (white space to git; `lines` does not end a line
    there);
  - after another header on the same line (`[core] [include]`);
  - on the line after a value continued with a backslash.

  CRLF lines were already safe (`lines` strips `\r\n`) and are now
  tested.
- **Decision: no explicit BOM strip.** The brief asked to strip a
  leading U+FEFF before `.lines()`. The whole-text scan needs no special
  case, so a strip line would be dead code: its mutant survived. I left
  it out.
- **Accepted cost:** `[include` inside a comment or a value now refuses
  a clone that does not need refusing (no commit list). The error only
  ever goes toward not reading.
- **P7, links inside `.git`.** A symlink at `.git/objects`, `.git/refs`,
  `.git/packed-refs` or `.git/HEAD` makes the clone `Outside`: no HEAD,
  no git, and the update says why.
- **Tests:**
  - `a_git_dir_that_points_outside_is_not_read` adds the BOM, lone CR,
    CRLF, same-line and continuation configs, and a link at each of the
    four paths, each undone and checked back to `Contained`.
  - `a_repository_pointing_outside_names_no_commits` adds two cases: a
    BOM before `[include]`, and a linked `.git/objects`.
- **SPEC-ENGINE §4** names the four link paths and the parser edge cases
  next to the include sentence. TESTING.md is updated.
- **Mutants:** `R3-S1 the round 2 line scan` and
  `R3-P7 links in .git not checked` are both killed, by the unit test
  and by the integration test. The whole list is now 38 mutants: 36
  killed, and the same 2 survivors by design.
- **Verification:** `flock /tmp/seldon-check.lock just check` on 918c0de7 —
  `check: ok`, exit 0 (log `check-wp136-r3.log`, outside the repository). `git diff` added lines grep'd for `/home/`: none.
