```
WP-065 HANDOVER
```

Branch `wp/065-review`. Commits: `8e79272` (fences, STATUS.md),
`6f20365` (ledger per-line decoding, actor/case on load), `0ce75bd`
(SPEC-ENGINE §6, SPEC-LOGBOOK fence paragraph, CHANGELOG), `e6d4985`
(memory/pitfalls.md), plus this handover.

## Done

- **F-130: fence markers in values.** `views::neutralise` puts a
  zero-width space (U+200B) after `<!--` in every `<!-- seldon:`. It
  is applied to the whole fence body in `try_merge_fence` (STATUS.md,
  `outputs/REBUILD.md`, the import report) and in
  `write_decisions_index`. That covers case titles, agents, drift
  subjects, details and actors, collector messages and decision titles
  in one place. It renders the same: inside a code span the character
  is invisible, outside one the text is still an HTML comment. It is
  idempotent.
- **F-131: a damaged STATUS.md is left alone.**
  - `write_status` now returns `views::Fill`, like
    `write_decisions_index`. The file stays byte-identical and a warning
    is given in two cases: a damaged fence (`fence_damaged`: a begin
    marker without an end marker of its own), or a file that has the
    generated header but no fence (the begin marker was removed, or
    both markers were).
  - The only header file without a fence that is still replaced is the
    `init` template, in any language and with any machine id
    (`is_status_template`, built from `templates::find("STATUS.md")`).
  - A file without the header is still kept below a fresh fence, as
    before.
  - Warning text: `STATUS.md: <why>; file not updated (restore the
    marker lines `<!-- seldon:begin status -->` and
    `<!-- seldon:end -->`)`. `status`/`index` put it in `warnings`
    (`--json`) and on stderr. `hook session-stop` prints it the same
    way it prints the DECISIONS.md warning.
- **CRLF.** `fence_body`/`replace_fence` (through the new `fence_span`)
  accept a begin-marker line that ends in `\r\n`. In a CRLF file the new
  body is written with CRLF too, so the file stays all-CRLF and the next
  run changes nothing.
- **`merge_fence`** (REBUILD.md and the import report, whose callers
  are outside this WP) never deletes text now. Where `try_merge_fence`
  refuses, the old text stays below a fresh fence (one header line). The
  next run finds the fresh fence first and is stable.
- **F-132: one bad line no longer fails the month.**
  - `Ledger::read_month` reads the month as bytes and decodes line by
    line (`from_utf8` per line, a trailing `\r` stripped). A line that
    is not UTF-8 or not an event goes to `bad_lines` and is skipped.
    `status`, `index`, `drift`, `capture` and every `read_range`/
    `read_all` caller therefore keep running.
  - `index/load.rs` gives one warning per month:
    `ledger/2026-10.jsonl: 1 line skipped, not a valid event (not UTF-8,
    not JSON, or a bad actor or case): line 2`. It lists at most five
    line numbers, then `and N more`.
  - For WP-070, `Ledger::bad_lines()` returns `(month, line numbers)`
    for every month that has bad lines.
- **WP-059 follow-up: actor and case are checked on load.** A parsed
  line whose `actor` fails `is_actor`, or whose `case` fails
  `is_case_id`, is a bad line. It is skipped and reported under the
  same torn-line rule, never accepted.
- **Docs.** SPEC-ENGINE §6 (the STATUS/views sentence, extended with
  ledger decoding, the damaged fence, CRLF and neutralising). The
  SPEC-LOGBOOK fence paragraph. CHANGELOG `[Unreleased]` has three
  Engine bullets.

## Not done / open

1. **Dossier fences (`system/*.md`) are not neutralised.**
   `dossier::Files::set` and its append path are in `dossier/mod.rs`,
   which is outside this WP. Its values come from host queries (plugin
   ids from `omarchy plugin list`, unit names) and from kept user cells.
   I did not put the neutralising into `replace_fence`, because `set`
   compares the old body with the raw content and would then report
   "written" for a body it did not change. A follow-up WP should
   neutralise in `dossier::Files::set`. SPEC-LOGBOOK names only
   STATUS.md, the `decisions.index` fence and `outputs/*.md`.
2. **`index::load::fences()`** (the reader of `system/*.md` for the
   index) still requires ` -->\n`, so the index does not see a CRLF
   dossier file. That is out of the WP's wording (CRLF STATUS.md); it is
   a one-line follow-up if wanted.
3. **No cleanup of files that 0.1.1 already grew.** The stale copies
   are outside the fence now and are treated as the user's text. The
   CHANGELOG does not promise a cleanup.
4. **Behaviour change for REBUILD.md and the import report.** A header
   file without a fence used to be replaced. Its text is now kept below
   a fresh fence. No test relied on the old behaviour; the header-only
   (blank) case is still replaced (`rebuild::tests` keeps it).
5. The `index` command does not read STATUS.md, so a damaged STATUS.md
   is reported by `status` and `hook session-stop` only (by design).

## Verified by

Every integration test runs in a temp HOME (`common::Env`). Nothing ran
against `~/Seldon` or `~/.config/seldon`.

New tests:
- `tests/status.rs::a_fence_marker_in_a_title_stays_in_its_fence`: a
  case title with `<!-- seldon:end -->` and a decision title with
  `<!-- seldon:begin status -->`. Over three `status` runs, STATUS.md
  and DECISIONS.md are byte-identical, with one begin and one end marker
  each, and there is one `seldon: status` commit.
- `tests/status.rs::a_damaged_status_md_is_left_alone`: end marker
  removed, then begin marker removed. Each time there is a warning
  `STATUS.md: … file not updated`, STATUS.md is not in `files`, and the
  file is byte-identical with the user's lines kept. A CRLF STATUS.md
  then merges: the new case is in it, it ends in
  `\r\n\r\n## My notes\r\nKeep this.\r\n`, it has only CRLF, and the
  second run writes nothing.
- `tests/index.rs::a_torn_ledger_line_is_skipped_with_a_warning`: a
  `log` with `ü`, then a line cut after `0xC3` with no newline.
  `status` exits 0 with the warning `ledger/2026-10.jsonl: 1 line
  skipped` and the note is in the index. `log` after the torn line works
  too. Two hand-copied lines follow, one with actor
  `Robot ]] <!-- seldon:end -->` and one with case `../../x`. Then
  `index --check` and `status` exit 0 with `3 lines skipped`, both notes
  are in the index, `Robot` is not in STATUS.md, and
  `capture --source theme` exits 0.
- Unit tests: `ledger::tests::bad_lines_are_skipped_one_by_one` (torn
  `ü`, a CRLF line accepted, bad actor, bad case, `bad_lines()`).
  In `views::tests`: `a_damaged_status_file_is_not_merged`,
  `a_crlf_fence_is_merged_with_crlf`, `values_cannot_close_the_fence`,
  and the existing merge test, now with the real `init` template in
  both languages.

Mutants (each applied to HEAD, the named test run, then the file
restored with `git checkout`; script output):

```
== M1 F-130: no neutralising: KILLED (cargo test --test status a_fence_marker_in_a_title_stays_in_its_fence exit 101)
   assertion `left == right` failed: [[400, 472], [462, 472], [524, 472]]
== M2 F-131: header file without a complete fence replaced (pre-fix): KILLED (cargo test --test status a_damaged_status_md_is_left_alone exit 101)
   panicked at tests/status.rs:588:9   ("files":["ledger/2026-10.md","STATUS.md"] — no warning, file rewritten)
== M3 F-131: CRLF begin marker not accepted: KILLED (cargo test --test status a_damaged_status_md_is_left_alone exit 101)
   panicked at tests/status.rs:607:5   (CRLF file: "files":[] , fence not found)
== M4 F-132: whole month must be UTF-8 (pre-fix): KILLED (cargo test --test index a_torn_ledger_line_is_skipped_with_a_warning exit 101)
   ["status", "--json"]: {"error":{"code":2,"message":"incomplete utf-8 byte sequence from index 277"}}
== M5 WP-059 follow-up: actor and case not checked on load: KILLED (cargo test --test index a_torn_ledger_line_is_skipped_with_a_warning exit 101)
   ["index", "--check", "--json"]: {"error":{"code":2,"message":"the new index does not validate against schema/index.schema.json; …"}}
== M6 F-132: warning does not lead with month and count: KILLED (cargo test --test index a_torn_ledger_line_is_skipped_with_a_warning exit 101)
   panicked at tests/index.rs:918:9
all killed
```

M1 shows the F-130 growth: STATUS.md is 400 → 462 → 524 bytes, 62
bytes per run, as in the finding. M5 shows that without the load check
a bad actor did not just leak: it made `index --check` fail with exit 2.

Gates:
- `just check-rss` → exit 0 (`rss_stays_under_10_mb_on_the_x10_fixture
  ... ok`).
- `just check` → exit 0, `check: ok`. That covers fmt and clippy
  (`-D warnings`, plain and `watch`), all cargo tests (1077 passed, 0
  failed over the plain and `watch` runs), packaging, schema and docs
  checks, `plugin-validate`, qmllint and the plugin tests (bar-view 120
  passed).
- `main` moved by one orchestration commit (`d7615cd`, queue only). It
  touches nothing in `docs/`, `decisions/`, `engine/src/index` or
  `ledger.rs`.

## Learned

Appended to `memory/pitfalls.md`:
- Tightening a reader breaks tests that inject bad values through it.
- Before the load check, a bad actor failed `index --check` instead of
  giving a warning.
- A mutant that does not compile proves nothing.
- `merge_fence`'s three callers have different needs.

## Decisions needed

None open. The orchestrator decided two points during the WP: the
`write_status` → `Fill` call sites, and the `tests/rebuild.rs`
adjustment. Open items 1 and 2 above are follow-up candidates.

## Touched outside WP scope

All of these were confirmed by the orchestrator during the WP:
- `engine/src/commands/index.rs`: `write_status` returns `Fill`; a
  `Skipped` warning goes into `built.warnings`.
- `engine/src/commands/hook.rs`: `write_views` prints the STATUS.md
  `Skipped` warning, as it already did for DECISIONS.md.
- `engine/tests/rebuild.rs`:
  `a_value_with_line_breaks_stays_inside_its_code_span` no longer
  expects the ACT/CAS values in REBUILD.md. It asserts that they, and
  their events' subjects, are absent (the skip is the tested behaviour).
  The other tags still test the renderer's escaping.
- `memory/pitfalls.md`: append only.
