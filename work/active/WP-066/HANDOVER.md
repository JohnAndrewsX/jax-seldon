WP-066 HANDOVER

Branch `wp/066-review`, worktree `wt/WP-066`. Commits:
`91a6815` (engine fix + tests), `547866b` (case id test),
`9fe8cbd` (SPEC-LOGBOOK §3 sentence, CHANGELOG), and this handover.

## Done

**F-533 — a save no longer turns valid hand-edited frontmatter invalid**
(`engine/src/frontmatter.rs`, `engine/src/model/mod.rs`).

- Line grouping (`group`, which replaces `push_line`): blank lines and
  column-0 comment lines join the keyed entry above when the next
  non-filler line continues it (indented or `- `). Otherwise they stay an
  entry without a key, as before.
- `key_of` accepts quoted keys (`"title": …`, `'it''s': …`). The key is
  decoded through serde_yaml, so escapes are handled.
- `Frontmatter::set` leaves an entry alone when its own text holds the
  value, *or* when the whole block gives the key that value. This covers
  valid YAML the line model cannot place, such as a flow list continued at
  column 0, which serde_yaml accepts. When an entry is rewritten, its
  column-0 comment lines are kept after the new `key: value` line.
- Read-back guard: `Frontmatter::check(values)` re-parses the rendered
  block (fences, YAML, duplicate keys) and checks that every key reads
  back as the value written. `model::update` now works on a copy, runs
  `check` and then `R::from_frontmatter`, and only then replaces the
  document. On failure it returns the new
  `FrontmatterError::Refused`, and the document and the file stay as they
  were. `model::update` now returns `Result`.
- Callers propagate the refusal. `CaseFile::save` gives a user error
  (exit 1) with the case path and writes before any rename. The journal
  (`journal.rs`, 2 lines) uses its existing "invalid journal frontmatter"
  error. The import (`omarchy_agent.rs`): a journal merge returns the
  error; on a memory file whose `updated` cannot be written, the import
  appends to the old text, as it already does for a memory file it
  cannot parse.

**F-535 — BOM and padded fences.**
- `Document::parse` skips a leading U+FEFF before the fence test and
  keeps it in the opening fence text, so `render` round-trips.
  `Document::set_frontmatter` (used by `model::update` for a file without
  frontmatter, e.g. an Obsidian daily note) moves a body-leading BOM in
  front of the new block.
- `fence_len` accepts spaces and tabs after `---` (opening and closing),
  and keeps them in the raw text. `---x` and `--- x` are still not fences.

**Case ids with control characters (brief item): no new check needed,
test added.**
`Case::validate` → `is_case_id` accepts only `C-`, 4 ASCII digits, `-`,
and 3 or more ASCII digits. Any control or format character in a case
file's id is therefore already refused on load (`CaseFile::load` → exit 1
"invalid case"). The other id-like case fields are ASCII-only too: `area`
and `agents` (`is_slug`), `events` (`is_ulid`).
`tests/frontmatter.rs::case_ids::control_characters_in_a_case_id_are_refused_on_load`
pins this for `\0 \a \t \n \r \e \x7f \N \L \P ​ ‮ ﻿`
before, inside and after the id.

**Docs:** SPEC-LOGBOOK §3 has one new sentence (hand edits accepted,
read-back before writing, refusal leaves the file unchanged). CHANGELOG
`[Unreleased]` → Engine has one new bullet.

## Tests (all fail without the fix)

`engine/tests/frontmatter.rs`, new `mod hand_edits` (CLI in a scratch
`Env`, test guard set):
- `a_comment_a_blank_line_and_a_quoted_key_survive_a_save`: `tags:` with
  a column-0 comment and a blank line, plus `"title":`. Runs `plan show`
  (exit 0, values right), then `plan start` and `log --case`. Checks: the
  file parses with one `title`, and the hand-written lines are still there
  byte for byte.
- `a_hand_edited_list_the_engine_changes_stays_valid`: `agents:` with a
  comment and a blank line, then `log --case --actor agent:codex`. Checks
  that the list is rewritten, `# who worked here` is kept and the file
  parses.
- `a_bom_and_padded_fences_parse_and_the_bom_stays`: a BOM case and a case
  with `--- ` / `--- \t` fences. Runs `plan list`, `plan show` and
  `plan start` (exit 0). Checks that the BOM is still first (once) and the
  padded fences are kept.
- `a_save_that_would_not_read_back_is_refused_and_the_file_is_unchanged`
  (library, `CaseFile::save`): `agents: [agent:claude-code,\nagent:codex]`.
  An unchanged save is a no-op, byte-identical. After `add_agent` the save
  is an error containing "refused", and the file bytes are unchanged.

Unit tests: `frontmatter::tests::{a_bom_and_padded_fences_round_trip,
set_frontmatter_moves_a_bom_to_the_front,
blank_and_comment_lines_inside_a_list_stay_with_it, quoted_keys_are_keys,
check_refuses_a_block_that_does_not_read_back}` and
`model::tests::{update_refuses_a_block_that_would_not_read_back,
update_puts_a_new_block_after_the_bom}`. The last two are the WP's
"re-parse guard refuses a crafted unparseable result, file unchanged"
unit test: the document renders byte-identical after the refusal.

## Verified by

Pre-fix (tests written first, run on the unchanged engine,
`cargo test --test frontmatter`):
```
test case_ids::control_characters_in_a_case_id_are_refused_on_load ... ok   (check already existed; see M7)
test hand_edits::a_save_that_would_not_read_back_is_refused_and_the_file_is_unchanged ... FAILED
test hand_edits::a_comment_a_blank_line_and_a_quoted_key_survive_a_save ... FAILED
test hand_edits::a_bom_and_padded_fences_parse_and_the_bom_stays ... FAILED
```
In the pre-fix run, the flow-list case was corrupted by an *unchanged*
save: it wrote `agents: [agent:claude-code, agent:codex]\nagent:codex]`.
The other two failed on exit 1 from `plan show` and `plan list`. (The
`agents` comment test was added later. M1 and M8 below show that it
fails without the fix.)

Mutants: each fix reverted on its own, then
`cargo test --lib --test frontmatter`, then the source restored with
`cp` and `touch`. `git diff` matched the pre-mutant diff after the
run (`cmp`).
```
M1 fillers never join the list above        2 failing: frontmatter::tests::blank_and_comment_lines_inside_a_list_stay_with_it, hand_edits::a_hand_edited_list_the_engine_changes_stays_valid
M2 quoted keys are not keys                 2 failing: frontmatter::tests::quoted_keys_are_keys, hand_edits::a_comment_a_blank_line_and_a_quoted_key_survive_a_save
M3 no read-back guard in update             2 failing: hand_edits::a_save_that_would_not_read_back_is_refused_and_the_file_is_unchanged, model::tests::update_refuses_a_block_that_would_not_read_back
M4 set ignores the block value              3 failing: frontmatter::tests::check_refuses_a_block_that_does_not_read_back, hand_edits::a_save_that_would_not_read_back_is_refused_…, model::tests::update_refuses_…
M5 no BOM handling                          4 failing: frontmatter::tests::a_bom_and_padded_fences_round_trip, …set_frontmatter_moves_a_bom_to_the_front, hand_edits::a_bom_and_padded_fences_parse_and_the_bom_stays, model::tests::update_puts_a_new_block_after_the_bom
M6 no spaces after a fence                  2 failing: frontmatter::tests::a_bom_and_padded_fences_round_trip, hand_edits::a_bom_and_padded_fences_parse_and_the_bom_stays
M7 case id number not checked (is_case_id)  1 failing: case_ids::control_characters_in_a_case_id_are_refused_on_load
M8 a rewritten entry drops its comments     2 failing: frontmatter::tests::blank_and_comment_lines_inside_a_list_stay_with_it, hand_edits::a_hand_edited_list_the_engine_changes_stays_valid
```
After the fix: `cargo test --locked --no-fail-fast` (whole engine)
passes, `cargo clippy --all-targets -- -D warnings` and `cargo fmt
--check` are clean. Existing fixture round-trip tests are unchanged and
green. `just check`: see the last line of this file.

No manual run against any real logbook. All CLI runs were in
`common::Env` (scratch HOME, `SELDON_TEST_GUARD`).

## Not done

- Comments *inside* a rewritten entry are kept only when they are at
  column 0. Indented comments and blank lines of a rewritten list are
  dropped together with the old list lines. An entry is rewritten only
  when the engine changes that key's value; in practice that means
  `status`, `started`, `closed`, `snapshotBefore`, `agents` and `events`.
- No canonical rewrite of the whole block as a fallback. A refused update
  is an error, as the WP asks.

## Learned (appended to memory/pitfalls.md)

- serde_yaml accepts a flow list continued at column 0
  (`agents: [a,\nb]`). No line-based model can place that line, so the
  read-back guard is the real safety net.
- A fallback can hide the fix it backs up. "The whole block decides"
  made an unchanged-`tags` test pass even with the grouping reverted.
  Test grouping on a key whose value the engine changes (`agents`).
- `"y"` is quoted by `is_plain` (YAML 1.1 bool). Pick test values like
  `z` when the test expects a plain scalar.

## Decisions needed

- `seldon log --case` writes the ledger note and the journal entry
  *before* `CaseFile::save` (commands/log.rs ~110-114, by design since
  WP-057: the ledger assigns the id the case records). If that save is
  refused (only the rare flow-list-at-column-0 edit), the note stays in
  the ledger and journal without being attached to the case, and the
  command exits 1. Should `log` (and the hook) dry-run `model::update` on
  the case before emitting? That would be a separate small WP touching
  `commands/log.rs` and `commands/hook.rs`, which are not in this WP. I
  did not test this path end to end; it is from reading the code.

## Touched outside WP scope

- `engine/src/logbook/cases.rs` (1 call, `CaseFile::save`): named in the
  brief's read list and in F-533 as the caller.
- `engine/src/logbook/journal.rs` (2 calls) and
  `engine/src/import/omarchy_agent.rs` (2 calls): `model::update` now
  returns `Result`, so the callers must handle it. They are one-line
  changes, and they put journal and import writes under the same guard.
  No active WP lists these files (WP-061 has `commands/import.rs`, not
  `import/omarchy_agent.rs`).
- `engine/src/model/case.rs`: one existing unit test gets `.unwrap()` on
  `update(…)`.
- I did not ask the orchestrator before touching these files. The
  signature change made these call sites necessary, and I checked that no
  parallel WP owns them. Please confirm or send back.

## just check

`just check` on `ff4ff49`: exit 0 (`check: ok`, 58 cargo test binaries ok, plugin tests ok).
