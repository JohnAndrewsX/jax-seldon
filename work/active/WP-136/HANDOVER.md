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
  `rev-parse`, `rev-list`, `log`; the tests run offline.
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
