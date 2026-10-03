```
WP-069 HANDOVER — config collector: scope, default skips, home paths, rehash, dedupe
Branch: wp/069-review (worktree wt/WP-069), commits on b476c75, not pushed.
```

## Done

1. **F-101 / F-451: scope changes.** Each generation in `manifest.json`
   now stores the scope it was taken in (`scope: {watch, exclude, skip}`:
   watch paths and excluded folders as `~`-paths, `skipPaths` patterns as
   configured, sorted). The scope is not part of the generation `hash`.
   Before the diff, `rescope()` does two things to the base generation:
   - It drops every file the current scope does not reach: under no
     watch path, excluded, or matching `skipPaths` itself or through a
     folder (`ScopeRules::covers`, same rule as `Walker::ignored`).
   - If the base's own scope is known, it adds every scanned file that
     scope did not reach, with the file's current hash.

   So narrowing writes no `config-remove` and widening no `config-add`.
   The collector message gets one line: `watch scope changed: N file(s)
   left it, M entered it; no events for them`. A manifest written before
   WP-069 has no scope. For it, pruning still works, and nothing counts
   as entered.
2. **F-250: default skips.** `[redaction] skipPaths` defaults to
   `DEFAULT_SKIP_PATHS`: `~/.config/omarchy/**/history.json`,
   `**/history/`, `**/state.json`, `**/cache/` and `**/*.log`. The `**`
   needs a folder, so Omarchy's top-level `shell.json`,
   `extensions/`, `hooks/` and `themed/` stay watched. `init` writes the
   list into new files. An empty list (`skipPaths = []`, as `init` wrote
   it before) also means the defaults; a non-empty list replaces them
   (review round 1, Q1). `init` prints a `skipPaths` hint under the
   config line (`setup::SKIP_PATHS_HINT`), and its JSON has `skipPaths`.
3. **F-544: home-relative paths.** `Dirs::expand_config` resolves a
   config value (`~`, `~/…`, `$HOME/…`, `${HOME}/…`, or relative) under
   HOME, folds `.`/`..`, and ignores an empty value. It uses the same
   `attribution::home_path`/`normalise` the hooks already apply to
   `watchPaths`. Where it is used: `resolve_logbook` (the config
   branch), the collector's roots, and `is_watched`. The wizard's
   "more paths" answer is stored as `~/…` (`typed_watch_paths`). The
   prompt says "relative to your home". `--logbook`, `SELDON_LOGBOOK`,
   `--config` and `SELDON_CONFIG` stay relative to the cwd, as shell
   input.
4. **F-100 (config half): dedupe.** `unrecorded()` drops events the
   ledger already holds in `[prev.checked, now]` with the same kind,
   redacted subject, `hashFrom` and `hashTo` (the theme collector's
   pattern). A ledger read error degrades the collector and keeps the
   cursor.
5. **F-402: no rehash.** `Manifest.stats` stores
   `[size, mtimeNs, ctimeNs, inode]` for each hashed file of the current
   generation. On a match of all four, the walk reuses the stored hash
   and does not open the file. The ctime catches a same-size in-place
   edit whose mtime was put back (`touch -r`; review round 1, B1). A file modified
   less than 2 s before the walk started (wall clock) is not cached,
   because coarse timestamps let a write right after the read keep the
   mtime. `sys::sha256` now hashes whole blocks in place and copies only
   the padded tail (≤ 128 bytes). Known vectors for lengths
   55/56/63/65/119/120/128 replace the length-only checks.
6. **WP-059 follow-up.** A file whose `~`-path holds a control character
   (or is longer than 512 characters, as before) is skipped and counted
   in the collector message: `N file(s) not watched: the name holds a
   control character or is longer than 512 characters`. The name is
   never printed.
7. **Docs.**
   - SPEC-ENGINE §2: config.toml row (`skipPaths` default, path rule)
     and manifest.json row (`scope`, `stats`).
   - SPEC-ENGINE §4: the config paragraph (scope changes, dedupe, stat
     reuse, control characters, relative paths).
   - `docs/user/{en,de}/06-configuration.md`: example, keys table, path
     rule, watched paths, `skipPaths`.
   - CHANGELOG `[Unreleased]`: four Engine bullets, appended.

Commits: `2d9f547` sha256, `8f7b908` paths and defaults, `74b6007`
collector and tests, `fc60331` docs, plus the handover commit.

## Not done

- Manual end-to-end run in a scratch HOME (docs/TESTING.md "Manual
  runs"): **blocked by the guard hook** (see Decisions needed). Not
  rephrased. The same scenarios run as tests against the binary
  (`capture::relative_config_paths_do_not_follow_the_working_directory`)
  and in-process (the scope, skip, cache and crash tests below).
- The plugins half of F-100 belongs to WP-073 and is not touched.
- Not measured: the speed-up on a large tree (F-402 timing). The test
  proves that an unchanged file is not read; no benchmark ran.

## Verified by

- `just check` → `check: ok`, exit 0. That covers fmt, clippy
  `-D warnings`, all tests with and without `--features watch`,
  packaging and the plugin harnesses. It ran once at the end, with
  nothing else running in the worktree.
- New tests:
  - `tests/collectors_user.rs`:
    - `config::a_narrowed_scope_is_no_removal`: watch path removed, then
      a `skipPaths` pattern added. Only the real deletion is an event.
      The notice counts 3, then 1. The same scope again gives no notice.
    - `config::a_widened_scope_is_no_flood`: a path added and a pattern
      removed, 22 entered, no events. A new file under an old path is
      still a `config-add`. Later changes to entered files are events.
    - `config::a_manifest_without_a_scope_still_drops_what_left_it`
    - `config::default_skip_paths_skip_plugin_state`: a made-up
      `example-timer/history.json`.
    - `config::an_unchanged_file_is_not_read_again`: same size, written
      in place, old mtime put back → no event. Same size with a new
      mtime → re-hashed. A just-written file is not cached.
    - `config::a_name_with_control_characters_is_skipped_with_a_warning`
    - `capture::relative_config_paths_do_not_follow_the_working_directory`
      (CLI, guarded scratch HOME): `logbook = "Logbook"` and
      `watchPaths = ["dotfiles"]`, captured from two cwds. Same logbook,
      0 events, and a change is seen from either cwd. It also checks the
      init `skipPaths` hint and that init writes the defaults.
  - `tests/idempotency.rs`:
    `a_failed_cursor_save_does_not_repeat_config_events`. Cursors are
    restored after the ledger write, and the next capture writes 0
    config events. A real later change, including the same step again
    after the cursor moved on, is recorded.
  - Unit tests:
    - `config::tests::config_paths_resolve_under_home_whatever_the_cwd`
    - `config::tests::default_skip_paths_skip_plugin_state_not_omarchy_config`
    - `commands::init::tests::typed_watch_paths_are_stored_under_home`
    - `sys::tests::sha256_matches_the_fips_vectors` (extended)
- Changed test: `config::skip_paths_are_never_hashed`. It now checks
  that the matched files never appear in `files`/`skipped`/`stats`, and
  that `scope.skip` holds the patterns as written in config.toml. Before,
  it searched the whole manifest text for the pattern words, which the
  new `scope` field now contains.
- Mutants. Each one reverts one fix (one line or a few) on the committed
  tree, runs the target test, and is restored with `git checkout`
  (script in the session scratchpad). All 11 failed:

  | Mutant | What it reverts | Failing test |
  |---|---|---|
  | F-101-narrow | `base.files.retain(\|_, _\| true)` | `a_narrowed_scope_is_no_removal` (collectors_user.rs:1114) |
  | F-451-widen | entered filter `false` | `a_widened_scope_is_no_flood` (:1164) |
  | F-250-defaults | `skip_paths: Vec::new()` | `default_skip_paths_skip_plugin_state` (:1219) |
  | F-544-expand | `expand_config` → `Some(self.expand(value))` | `relative_config_paths_do_not_follow_the_working_directory` (:1479) |
  | F-544-wizard | stores `p.display()` | `typed_watch_paths_are_stored_under_home` (init.rs:810) |
  | F-100-dedupe | `unrecorded` returns early | `a_failed_cursor_save_does_not_repeat_config_events` (idempotency.rs:138) |
  | F-402-reuse | never reuse a stored hash | `an_unchanged_file_is_not_read_again` (:1252) |
  | F-402-racy | no racy window | `an_unchanged_file_is_not_read_again` (:1259) |
  | F-402-sha-pad | `rest.len() <= 56` | `sha256_matches_the_fips_vectors` (sys.rs:562) |
  | WP-059-control | no control-char check | `a_name_with_control_characters_is_skipped_with_a_warning` (:1273) |
  | F-250-hint | init hint removed | `relative_config_paths_…` (:1456) |

## Learned

Appended to `memory/pitfalls.md` (WP-069):
- The guard blocks scratch-HOME `$HOME/.config` writes.
- Pruning needs only the current scope; detecting what entered needs the
  old one.
- The racy-mtime window, measured against the wall clock.
- In-place writes keep the inode for "not read again" fixtures.

## Decisions needed

1. **Guard false positive.** `scripts/guard.sh` blocked the documented
   manual recipe (docs/TESTING.md "Manual runs"). The command was a
   capture in `S=$(mktemp -d …)` with `HOME=$S/home` and
   `SELDON_TEST_GUARD=$S`, which writes `$HOME/.config/hypr/*.conf`.
   The guard reads the text `$HOME/.config` as the real home. Should
   the guard learn that rule, or should the recipe say to build test
   files through `$S/home/…`? I did not route around it.
2. **Default `skipPaths` also reach the hooks.** `[redaction] skipPaths`
   is shared with the agent hook (ADR-0014 §4). A recorded command that
   names e.g. `~/.config/omarchy/x/state.json` is now recorded with the
   path redacted. The WP asks for default `skipPaths`, so I followed it.
   A separate collector-only key would avoid this but is a new config
   key. Accept, or queue a follow-up?
3. ~~Existing installs keep `skipPaths = []`.~~ Decided in review round
   1 (Q1): an empty list means the defaults. Done.

## Touched outside WP scope

- `engine/src/commands/init.rs` outside the wizard path storage: one
  output line (the `skipPaths` hint) and the `skipPaths` key in init's
  JSON. The goal asks that "`init`/setup output mentions `skipPaths`",
  and the output is built in `init::run`.
- Otherwise none. No other WP's files, `tests/common/mod.rs` unchanged,
  only the named SPEC paragraphs.

## Notes for the reviewer

- After a failed cursor save, the next capture now drops the repeated
  config event. That run's `written` is then empty, so
  `explain_own_writes` cannot explain an engine-owned `config-add` from
  the failed run, and the record is forgotten. Before WP-069 the event
  was written twice and only the copy was explained, so the original
  was unexplained drift then too. Not a regression, but rule 7 still
  misses this case.
- `manifest.json` now holds `skipPaths` patterns, which are the same
  text as in config.toml, in the same user's 0600 state dir. Matched
  files are still never named.

## Review round 1 (SEND BACK → fixed)

Commits:
- `426483d` engine: empty `skipPaths` means the defaults
- `bd36bb4` engine: ctime in `FileStat`, tests reworked
- `f40bcb1` docs: SPEC §2/§4/§7, en guide, CHANGELOG
- `docs(de)` commit: German guide follows `en` at `f40bcb1`

- **B1. ctime in `FileStat`.** `FileStat(size, mtimeNs, ctimeNs, inode)`.
  - The 2 s racy window still checks the mtime only. A write sets both
    times, and a ctime window would leave every freshly written file
    uncached, which a test cannot prove without a 2 s sleep.
  - `an_unchanged_file_is_not_read_again` now writes a planted hash
    (`"f" × 64`) for `a.conf` into `manifest.json`. The next capture
    keeps that hash with no event, which proves the file was not read.
  - The test then does a same-size in-place edit with the mtime put
    back. That edit is read again: `config-change` with
    `hashFrom = fake`.
  - A 20 ms sleep before the edit lets coarse clocks tick.
  - SPEC §2 (manifest row), SPEC §4 and guide 06 en/de now name size,
    mtime, ctime and inode.
- **Q1. Empty `skipPaths` = defaults.** `Redaction.skip_paths` is read
  through `skip_paths_or_defaults`. A missing key or `[]` gives
  `DEFAULT_SKIP_PATHS`; a non-empty list replaces them. Because the
  change is in deserialisation, the hooks and the collector see the
  same list without touching `hook.rs`. `init` writes the defaults
  explicitly. Tests:
  - unit: `default_skip_paths_skip_plugin_state_not_omarchy_config`
    covers `[]`, a missing key, an empty file and an own list;
  - CLI: `relative_config_paths_do_not_follow_the_working_directory`
    saves `skipPaths = []` and checks that a plugin `history.json` under
    a watched `~/.config/omarchy` is not in the manifest.

  SPEC §2, guide 06 en/de and the CHANGELOG bullet say so.
- **N1.** `docs/user/de/06-configuration.md` is stamped
  `<!-- source: en/06-configuration.md @ f40bcb1 -->`, the commit of the
  last English change. `docs-check` passes with no warning.
- **N3. Manifest growth.** `stats` and `scope` make `manifest.json`
  about 2.2× larger: 222 KB → 489 KB at 2000 files. That is an estimate
  from serde's pretty layout, where each stats number gets its own line.
  `scope` is constant (< 1 KB).
- **§7.** One clause: `**` stands for at least one folder (`a/**/b`
  matches `a/x/b`, not `a/b`). The defaults rely on that to leave the
  files directly in `~/.config/omarchy/` watched. Also in guide 06
  en/de.
- **Checks:**
  - `cargo fmt --check` and `cargo clippy --all-targets -D warnings`:
    clean.
  - Suites: `collectors` 16, `collectors_user` 30 and `idempotency` 7
    tests pass, and `--lib` 160.
  - `just docs-check`: `docs-check: ok (391 links, 14 translated pages, …)`.
- **Mutants of this round.** All four failed their test:

  | Mutant | What it reverts | Failing test |
  |---|---|---|
  | B1-no-ctime | ctime stored as 0 | `an_unchanged_file_is_not_read_again` (collectors_user.rs:1265) |
  | B1-no-reuse | never reuse a stored hash | `an_unchanged_file_is_not_read_again` (:1252) |
  | Q1-empty-cli | empty list stays empty | `relative_config_paths_…` (:1526) |
  | Q1-empty-unit | empty list stays empty | `default_skip_paths_skip_plugin_state_not_omarchy_config` (config.rs:749) |

  Mutant lines in the table of the first round may have shifted by a few
  lines.

### Follow-ups (not this round)

- **A failed cursor save, then the file goes back.** Capture 1 writes
  `A→B`, then its cursor save fails. The file returns to `A` before the
  next capture, which diffs `A` against `A` and writes nothing. The
  ledger then holds `A→B` without the `B→A` that would close it.
- **The hooks read an empty watch path as the whole home.**
  `attribution::home_path("")` yields the home folder, so the hooks'
  `Scope` treats `watchPaths = [""]` as the whole home. The collector
  (`Dirs::expand_config`) ignores an empty value. They should agree; the
  fix belongs in `attribution.rs`/`hook.rs`, outside this WP.
