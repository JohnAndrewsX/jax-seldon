WP-057 HANDOVER

Branch `wp/057-review` (worktree `wt/WP-057`), on top of `f056ee1`. No
PR, no push.

## Done

Commits: `c3a6b1a` (index --check), `f156b1f` (journal), `a861ed5`
(hook, session-stop, rebuild threshold), `0f44b84` (SPEC §8, CHANGELOG,
constant comment), `e9df280` (pitfalls).

- **F-500 — case read under the lock.** `record()` (hook.rs) takes the
  state lock first, then reads `.seldon/active-case` / `--case`, runs
  `cases::find` and filters the records; the case is saved before the
  lock is released. Only green records with no case at all (no `--case`,
  no active case) are dropped before the lock, so an agent working
  without a case still takes no lock for green commands. Extra guard:
  `CaseFile::save` (cases.rs) refuses to write a case whose file is gone
  from its path (moved by another writer), so a stale copy can never
  recreate a case in its old folder.
  `index --check` reports a case id found in two files: `index/check.rs`
  adds one rule beyond the schema for `index.schema.json`
  ("/cases: case C-… exists more than once (work/queued/…,
  work/completed/…); keep one file"), so `index --check` exits 2 and
  writes nothing, like any other invalid index. I put the rule into the
  validator because `commands/index.rs` (the caller) is not in the WP's
  file list; see open questions.
- **F-502 / F-401 — patience and lock window.** `LOCK_PATIENCE` 2 s → 8 s
  (below the 10 s PreToolUse timeout `hook install` writes). The index
  rebuild is no longer inside the record's critical section: the lock is
  dropped after the ledger append and the case save, then `rebuild_after`
  takes it again with a non-waiting `lock::acquire`; if another `seldon`
  holds it, that writer rebuilds after its own write (or, above the
  threshold, the next `capture`/`status` does). I kept the rebuild under
  a lock rather than lock-free so two rebuilds never race (an older
  derivation could otherwise overwrite a newer `index.json`).
  `session_stop` has no `?` after the logbook is opened: the ledger count,
  journal, capture, views, commit and index each report a failure on
  stderr (`seldon hook: <step>: <error>`) and the next step runs; exit 0.
  Only a lock it cannot get within 8 s skips the steps that need it.
- **F-501 — journal without frontmatter.** `journal.rs`: a day file
  without frontmatter gets the canonical block (date, cases) in front of
  its text; broken frontmatter (invalid YAML, unterminated) stays a user
  error. New `journal::prepare` → `Pending::write`: `seldon log` and
  `plan done` read and check the day **before** the ledger append and
  write it after. Decision: **order, not rollback** (the ledger is
  append-only): a day file the engine cannot read fails the command
  before anything is written, so a retry leaves no second ledger event.
  Only an I/O error on the journal write itself can still leave a ledger
  event without its journal line.
- **F-507 — STATUS.md.** session-stop derives the index once under the
  held lock, writes the views `seldon status` writes (`ledger/*.md`,
  `STATUS.md`, the `decisions.index` fence of `DECISIONS.md`), commits,
  then sets `logbook.git` and writes `index.json` (same order as
  `rebuild_with`, which cannot be called while the lock is held).
- **F-400 — hook rebuild threshold.** Decision: **threshold, not always**.
  `rebuild_if_initialised_fast` skips the rebuild above
  `FAST_REBUILD_MAX_LINES` = 1000 ledger lines and returns `false`; the
  count reads `ledger/*.jsonl` only until it passes 1000 (bounded cost).
  Why not always: the plugin's own capture runs every 15 min, so a young
  logbook would show agent commands up to 15 min late; below 1000 lines
  the rebuild costs at most ~3 ms. Above it the hook costs ~2 ms flat.
  The signature of `rebuild_if_initialised_fast(ctx)` is unchanged apart
  from the `bool` result (tests/status.rs calls it).
- **SPEC-ENGINE §8**: PreToolUse paragraph (8 s wait, case under the
  lock, rebuild after release and below 1000 lines, measured numbers);
  session-stop paragraph (views before the commit, every step, stderr).
  The old "beyond roughly 500 ledger lines … accepted for v1" sentence is
  gone. CHANGELOG `[Unreleased] ### Engine`: three bullets.

## Not done

- Nothing of the WP's outputs is left open.
- `claude_code()`'s PostToolUse dedupe (`already_recorded`) still reads
  the ledger before the lock; two PostToolUse calls for the same
  `tool_use_id` racing could record twice. Out of this WP's functions
  (not `record`), harmless for the PreToolUse-only install; noted only.

## Verified by

Pre-fix run: `git checkout f056ee1 -- engine/src` plus a one-line shim
for the new `FAST_REBUILD_MAX_LINES` constant, new tests unchanged,
`cargo test --no-fail-fast --test hooks --test log --test journal
--test plan`: **all 11 new integration tests fail** (hooks 7/39, journal
1/4, log 1/11, plan 2/15), the old ones pass. Then `git checkout HEAD --
engine/src`.

| Item | Test | Pre-fix failure |
|---|---|---|
| F-500 race (lock held, case changed under it: status + one attributed note) | `hooks::case_under_the_lock::a_change_made_while_the_hook_waits_is_kept` | `the change is kept`: status `active` instead of `verification` |
| F-500 repeat, 10 rounds × 2 concurrent PreToolUse | `hooks::case_under_the_lock::concurrent_hooks_lose_no_event` | 10 of 20 ledger ids missing from `events:` |
| F-500 `plan done` variant (case moved under the lock) | `hooks::case_under_the_lock::a_case_moved_while_the_hook_waits_is_not_copied_back` | 2 files: `work/active/C-2026-001-…` and `work/completed/C-2026-001-…` |
| `index --check` duplicate | `plan::index_check_reports_a_case_in_two_folders`, unit `index::check::tests::a_case_id_twice_is_an_error` | exit 0, index "valid" |
| F-502 5 s hold | `hooks::claude_code::a_lock_held_for_five_seconds_is_waited_for` | stderr "another seldon process holds the lock", no event |
| session-stop past a broken journal (capture, STATUS.md, index, commit, exit 0, stderr) | `hooks::sessions::session_stop_runs_every_step_past_a_broken_journal` | stops after the journal error: no capture, no commit |
| F-507 STATUS.md rewritten and committed | `hooks::sessions::session_stop_rewrites_status_md` | STATUS.md without the new case |
| F-501 empty / text-only day file | `journal::a_day_without_frontmatter_gets_the_block`, unit `logbook::journal::tests::a_day_without_frontmatter_gets_the_block_in_front` | "invalid journal frontmatter: no frontmatter" |
| F-501 `log` order (broken day → exit 1, no ledger note; empty day → exit 0, block prepended, ledger + journal) | `log::a_day_it_cannot_read_writes_no_note` | "no note without its journal line" (ledger had the note) |
| F-501 `plan done` (broken day → exit 1, nothing changed; empty day → exit 0) | `plan::done_with_a_day_file_without_frontmatter` | the `case-completed` event was written before the failure |
| F-400 above threshold: index unchanged, next `status` catches up | `hooks::index::a_large_ledger_is_left_to_the_next_status` | the hook rewrote `index.json` |

**Find-before-lock mutant** (current code, only `lock_patiently` moved
back below `cases::find` and the filter in `record`):
`cargo test --test hooks case_under` → 3 of 3 fail;
`concurrent_hooks_lose_no_event` lost 10 of 20 event ids in each of 3
runs. With the fix: `case_under_the_lock` 15 runs, 0 failures.
(The moved-case test fails on the mutant through the new `save` guard —
the hook reports "… is gone" instead of writing a duplicate.)

**Release timing** (`cargo build --release`, glibc, dev host, load ~4.4;
scratch HOME/XDG with `SELDON_TEST_GUARD`, made-up filler notes; 21 runs
of a mutating PreToolUse each, median):

| Ledger lines | Hook (ms) |
|---|---|
| ~20 | 3.1 |
| ~270 | 3.5 |
| ~520 | 4.1 |
| ~1000 (just below) | 4.9 |
| ~1020 (above, no rebuild) | 1.9 |
| ~10 000 | 2.3 |
| ~50 000 | 2.2 |

SPEC §8 now carries "about 2 ms … 3 to 5 ms with the rebuild"; SPEC §1's
"`hook` < 5 ms" holds on this host.

**Gates:** `just check` exit 0 ("check: ok"; 881 passed, 0 failed over the
`cargo test` runs with and without `watch`; plugin-test ok), run on `0f44b84`.
`just check-rss` exit 0 (`rss_stays_under_10_mb_on_the_x10_fixture` ok).

## Learned

Appended to `memory/pitfalls.md` (WP-057 section): lock-then-find for
every case writer, how to write the race test, no `rebuild_with` under a
held lock, `Ulid::from_parts` in tests, `--no-fail-fast` for pre-fix
runs, test output carries the dev host's name.

## Decisions needed

- `index --check`'s duplicate rule lives in `index::check::Validator`
  (a non-schema rule for `index.schema.json`) because
  `commands/index.rs` was outside the file list. If the reviewer prefers
  it as a separate check called from `commands/index.rs` (and a warning
  in plain `seldon index`), that is a one-line move. Plain `seldon index`
  and `status` still write an index with the duplicate (unchanged).
- SPEC-LOGBOOK §3 (journal) does not say that a frontmatter-less day file
  is accepted and completed; not my paragraph — a one-sentence follow-up
  if the spec should say it.

## Touched outside WP scope

- None outside the listed files. Listed and touched: `hook.rs`
  (`record`, `rebuild_after` (new, next to `record`), `session_stop`,
  `write_views` (new helper of `session_stop`), `LOCK_PATIENCE`),
  `cases.rs` (`save` guard), `journal.rs`, `log.rs`, `plan.rs` (`step`),
  `index/check.rs`, `index/mod.rs` (`rebuild_if_initialised_fast`, its
  constant and line counter), tests `hooks.rs`, `journal.rs`, `log.rs`,
  `plan.rs`, `docs/SPEC-ENGINE.md` §8 (the two paragraphs), CHANGELOG
  `[Unreleased]`, `memory/pitfalls.md` (append).
- `main` moved (WP-056: docs, CHANGELOG `### Packaging and docs`,
  `memory/pitfalls.md` append); no overlap with this WP's text, but the
  pitfalls and CHANGELOG appends may need a trivial merge.
