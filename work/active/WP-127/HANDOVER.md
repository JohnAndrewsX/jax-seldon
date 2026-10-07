# WP-127 — handover

Branch `wp/127-index-details`, from `next` 109d02eb, to merge into `next`.
Role: Engine Dev with the plugin part. ADR-0038 is **proposed**: the
operator accepts it (ADR-0035 §6 requires an accepted ADR before the merge).

## What was done

- **ADR-0038 (proposed)**, `decisions/ADR-0038-index-details.md`. It adds four
  optional fields within contract 2 under ADR-0035 §6 and the frontmatter key
  `source`. DECISIONS.md has the row.
- **Schema.**
  - `drift[].rule`: a slug, at most 64 characters, open set.
  - `cases[].intent` and `cases[].result`: at most 256 characters each.
  - `cases[].source`: `~/…`, at most 512 characters, no control, bidi or
    format characters. It is also valid as a frontmatter key.
  - `decisions[].lead`: at most 256 characters.
  - `contractVersion` stays 2.
- **Engine.**
  - `DriftItem.rule` is the classification's rule, the same one `drift show`
    and `drift list` report. Under `attention = "all"` it is `attention-all`.
  - `load` reads the first paragraph of a case's `## Intent` and `## Result`
    and of a decision's `## Decision`. It uses `cases::first_paragraph` and
    `cases::paragraphs`.
  - `build::shown_text` handles the three texts:
    1. Control characters other than `\n` and `\t` become spaces.
    2. The text goes through the logbook's redaction (`Input.redactor`). With
       `[redaction] patterns` that do not compile, the four fields are
       withheld.
    3. `clip_with(…, "in the file")` clips it. That is ADR-0025's clip with
       its own marker.
  - `build::shown_source` redacts `source` again. It keeps the value only
    while `import::is_case_source` holds; otherwise it drops the value and
    adds a build warning naming the case.
  - `Case.source`:
    - Written only when present; `to_values` has no empty `source:` line.
    - Read leniently: a non-string value counts as none.
  - `import task` writes `source: "~/…#line"` into each new case. A path of
    more than 512 characters is shortened to `~/…` plus its end
    (`import::case_source`).
  - `bad_path_char` moved to `import/mod.rs`, unchanged.
  - `plan show --json` carries `source` while it has the right shape.
- **Fixtures.**
  - C-2026-007 is now the sample's imported case: tag `imported`,
    `source: "~/Notizen/aufgaben.md#4"`, the provenance line, and the Log
    `created …: imported from …`. Case and event counts are unchanged.
  - `index.sample.json`, `index.attention-all.json` and the nine variants were
    regenerated (`validate-fixtures.py --write-index`). The engine's golden
    test equals them.
  - `scripts/validate-fixtures.py` derives all four fields: `rule` from
    `Classifier.group`, the paragraphs, the clip with `place`, the provenance
    skip and the source check. It does not redact; fixtures/README says so.
- **Plugin.**
  - `Model.driftRuleInfo(rules, shown, eventId, index)` takes the index
    item's `rule` first (a schema-shaped slug only). It falls back to
    `Service.driftRules` and `drift show` only when the field is absent.
    `EventDetail.qml` passes the index. With the field, a crisis click starts
    no process.
  - `caseDetail` carries `intent`, `result` and an "Imported from" key/value.
    `Work.qml` shows INTENT and RESULT as plain text, each hidden when empty;
    the PLAN line names the case file for the rest, and *Open in editor*
    stays.
  - `decisionRows` and `decisionDetail` carry the lead as `text`.
    `Decisions.qml` shows it as plain text under the note. The closing line
    reads "The whole text is in the file; …" when a lead is shown.
- **Docs.**
  - CONTRACT.md rule 5 (the clip for the new texts) and rule 9 (the four
    fields).
  - SPEC-ENGINE §3 (`import task` writes `source`) and §6 (how the fields
    are derived).
  - SPEC-LOGBOOK §3 (the `source` key; the first paragraphs).
  - SPEC-PLUGIN: "Why loud?" from the index, and the Work and Decisions
    details.
  - TESTING.md, CHANGELOG (Engine and Plugin), fixtures/README.

## Decisions (what the WP left open)

1. **`source` is an engine-written frontmatter key, not derived from the
   marker.** This follows Fable's advice and ADR-0038 §3: the marker is
   idempotency state, and the case file describes itself. The marker stays
   the only idempotency key; a test proves that an edited or removed
   `source` imports nothing again.
2. **Over-long source:** `~/…` followed by the last 509 characters, 512 in
   all, so the file name and the line survive.
3. **A bad `source` never makes a case unreadable.** A non-string counts as
   none. A string out of shape is dropped from the index with a build
   warning.
4. **"First paragraph":**
   - HTML comments are removed first, so template placeholders do not count.
   - Leading blank and heading lines are skipped. A heading is `#` to
     `######` followed by a space or a tab; `#3 merged` is text.
   - The paragraph runs up to the next blank line, each line trimmed at the
     end and joined by `\n`.
5. **An imported case's intent** skips the provenance line when that line is
   the whole first paragraph and the case is tagged `imported`. Showing
   "Imported from … — read before you start this case." as the intent would
   repeat `source`. An untagged case keeps it as its intent (tested).
6. **`lead` is the first paragraph of `## Decision`.** There is no fallback
   to the body: that would be the Context, not the decision. Both built-in
   templates use English headings.
7. **The cut marker is "… (N more characters in the file)".** There is no
   `truncated` flag: the plugin offers the editor either way and keys
   nothing off the cut.
8. **Redaction runs on every build**, because these texts are hand-written.
   Patterns that do not compile withhold all four fields. `rule` is never
   withheld; it is the engine's own.
9. **`rule` is written under `attention = "all"` too** (`attention-all`), as
   `drift show` reports it. The schema leaves the set open.
10. **The plugin shows `source`** as an "Imported from" key/value, text only.
    The acceptance asks the sample to render every field. The full imported
    UX, including the provenance line in the detail, stays with WP-102b.
11. **Field order:** the new keys come last in each object (`rule` after
    `truncated`; `intent`, `result`, `source` after `proposedEvents`; `lead`
    after `cases`).
12. **The WP file stays in `work/queued/`.** Moving it is the orchestrator's
    act.

## How it was verified

- `flock /tmp/seldon-check.lock just check` → see "Check" below. The log is
  `engine/target/check-wp127-r1.log` on the dev host and is not committed.
- **Engine.**
  - Golden index equals the regenerated sample; the variants and the
    attention-all index are also checked.
  - `drift.rs`: `drift list`'s `rule` equals the index's.
  - New in `index.rs`:
    - clipping with the file marker and control characters as spaces;
    - the imported intent, with and without the tag;
    - no redactor → the texts are withheld;
    - six sources out of shape are dropped with a warning, and 512
      characters still pass;
    - `rule` per item and `attention-all`;
    - the four fields are optional, and seven bad values fail both
      validators.
  - The reference-derive test now also compares `cases` and `decisions`, with
    long, escaped and control-character texts in the three sections:
    engine = script.
  - `redaction.rs` `the_index_masks_intent_result_lead_and_source`:
    - a token, an e-mail and a `[redaction] patterns` hit in hand-edited
      Intent, Result, Decision and `source` → `‹redacted›` in the index;
    - a broken pattern → the four fields are withheld.
  - `import_task.rs`:
    - `source` in the frontmatter and the index, for an item and for a whole
      file;
    - the intent after the provenance line;
    - the marker, not the source, keeps the import idempotent;
    - a 600+ character path keeps its end.
  - Unit tests: `first_paragraph`, `case_source` / `is_case_source`, and
    `Case.source` (written only when present, read leniently).
  - Tests ran with `TZ=UTC`.
- **Mutants:** `python3 work/active/WP-127/mutants.py` (own target
  `engine/target/mutants-wp127`; arguments filter by name) runs 19 engine
  and 6 plugin mutants. All 25 are killed.
  - The first run left "invalid patterns fall back to the built-in rules"
    alive. The broken-pattern part of the redaction test was added for it,
    and the rerun killed it.
  - Logs: `engine/target/wp127/mutants-r1.log` and `mutants-r2.log`.
- **Plugin.**
  - `node tests/plugin/model.test.js`: 127 passed. New:
    - the index rule first (any `drift show` answer loses), the fallback
      without it, bad rule shapes;
    - case intent, result and source, including an index without them and
      non-string values;
    - the decision lead.
  - `bash tests/plugin/desk-view.sh`: 1376 passed, 0 failed (standalone run,
    and again inside `just check`).
    - `why-loud` (live): the argv is only the start-up calls. Together with
      `today-resolve`, `drift-live` and `tab-focus` (no `drift show` any
      more), this proves no new process per click.
    - `why-loud-bare` and `why-loud-bare-dev`: an index without `rule` keeps
      the `drift show` path exactly as before. `why-loud-all` and
      `why-loud-group` moved onto that copy.
    - `details` and `details-bare`: the new texts render, and an index
      without them renders as before.
  - The 0.1.x case is unchanged: the contract-1 copy of the sample still
    gives the mismatch (`model.test.js`).
- **Perf** (`just check-perf`, bench profile): ×150 index build median 72
  ms (budget 100); `status` at 10 540 lines 52 ms; hooks and redaction within
  budget.
- `grep /home/` over the diff: only the fake `/home/alice` and `/home/user`
  in an existing TESTING row.

## Not done / open

- **`just check-rss` fails, but it already failed on `next`.**
  - On 109d02eb, measured in a temporary worktree that was removed
    afterwards, the peak is 10 580–10 716 kB against the budget of 10 240 kB
    (3 runs).
  - On this branch it is 10 780–10 848 kB (4 runs). WP-127 adds about
    80–100 kB of heap: the texts of 80 cases and the rules.
  - The budget or the cause on `next` is a separate WP for the orchestrator.
    I did not raise the budget.
- **WP-102b's "whole Intent as plain monospace text"** for an imported case
  is not in the index. `intent` is the first paragraph after the provenance
  line, clipped at 256 bytes. The whole text needs *Open in editor* or a
  further field and ADR; this is a question for the orchestrator before
  102b.
- **ADR-0038 needs the operator's acceptance** before this merges into
  `next` (ADR-0035 §6).
- The Python validator's `pattern` check for `source` accepts a trailing
  newline, because of `re`'s `$`. The engine's checker and the engine's own
  check are strict, and the engine never writes one.
- Open cases are not capped (CONTRACT rule 4), so each adds up to about
  0.8 KB. ADR-0038 names the worst case; the over-budget warning of ADR-0025
  covers it.

## Check

`flock /tmp/seldon-check.lock just check` on aa3d9d37 (the last code
commit; this handover adds only this file) gives **exit 0, `check: ok`**.

- Engine: fmt, clippy `-D warnings`, and all tests with the default and
  `watch` features.
- Packaging: shellcheck is not installed on the dev host, so `bash -n` only,
  as before.
- install, deploy, schema-validate.
- docs-check: 465 links.
- `omarchy plugin validate`; qmllint (46 files).
- Plugin tests: model 127, service-states 328, desk-view 1376, bar-view 194.

Round 1 (`check-wp127-r1.log`) stopped at qmllint: the new Intent/Result
delegate used `modelData` unqualified. aa3d9d37 fixes that.

Logs on the dev host, not committed: `engine/target/check-wp127-r2.log` and
`check-wp127-r1.log`.

## Round 2

Brief: `WP-127-round-2-brief.md`. Packet: `WP-127-review-1.md`, a SEND BACK on
2e0fe7d6 for B1 and N1–N5, plus a byte cap for `source`.

### Fixed

- **B1: redaction before the clip is now pinned.**
  `redaction.rs::a_secret_at_the_cut_or_over_lines_is_masked_before_the_clip`.
  - A `ghp_` token straddles the 256-byte cut in intent, result and lead.
    Each text is clipped and holds `‹redacted›`; no `ghp_` reaches the index.
  - In a second case, two multi-line secrets are masked: a `mysql … \`
    continued `-p…` in the Intent, and a JSON `"password":` with its value on
    the next line in the Result.
  - The same Result holds a token split by U+200B (N4). It is masked as well.
  - The swap mutant ("clip before redaction") is killed by this test.
- **N1:** the ADR-0038 size line now gives per-object figures:
  - about 74 B per drift item, 1.06 KB per case, 0.27 KB per decision;
  - about 68 KB at the caps of rule 4;
  - about 320 KB more for 300 full open cases, which the ADR-0025 warning
    names;
  - a note that JSON escaping of `"` or `\` in `source` can double its bytes.
- **N2:** CONTRACT.md rule 5 now reads "Cases, decisions and memory topics
  are not cut, except a case's `intent` and `result` and a decision's
  `lead`".
- **N3:**
  - R2 is killed by `import_task.rs::plan_show_carries_only_a_source_in_shape`:
    `plan show --json` leaves out a hand-edited `source` with U+202E, outside
    `~/`, or without `~/`.
  - R3 is killed in `index.rs::case_and_decision_texts_are_clipped_with_the_file_marker`:
    a Result made only of control and format characters gives no field.
- **N4** (orchestrator's decision): `shown_text` drops direction and format
  characters before the control-to-space step and before the redaction. These
  are U+200B–U+200F, U+202A–U+202E, U+2060, U+2066–U+2069 and U+FEFF
  (`import::is_direction_or_format`, which `bad_path_char` now shares).
  - Test: `x‮evil​zw\u7bell⁦﻿‏!` becomes `xevilzw bell!`.
  - Updated: ADR-0038 §2, CONTRACT rule 9, SPEC-ENGINE §6, the schema
    descriptions and the reference derive (`DIRECTION_OR_FORMAT`).
- **N5:** `cases::paragraphs(text, max)` now streams.
  - It leaves comments out as `strip_comments` does, also across line breaks
    and when unclosed. It stops after `max` paragraphs and copies only those.
  - `first_paragraph` asks for 1; an imported Intent for 2.
  - Equivalence test: `paragraphs_leave_comments_out_as_strip_comments_does`,
    which compares against the old two-step read for every `max` on nine edge
    cases.
  - Timing line in `just check-perf`:
    `the_first_paragraphs_of_a_large_intent_cost_what_they_hold`. Two
    paragraphs of a 1 MiB Intent take a median of 261 ns against a budget of
    50 µs. The mutant that reads on is killed by it.
- **`source` cap:** 512 **bytes** (UTF-8), which also caps it at 512
  characters.
  - `is_case_source` checks bytes, so a 302-character, 602-byte path is
    dropped with a warning.
  - `case_source` keeps `~/…` and as many of the last characters as fit in
    507 bytes, never splitting a character. Unit tests use `ä` and `🚀`.
  - The import test now expects 512 bytes.
  - Updated: ADR §3, CONTRACT, SPEC-ENGINE, SPEC-LOGBOOK, the schema
    description and the Python check.

### Verification

- `python3 work/active/WP-127/mutants.py` runs in its own target
  (`engine/target/mutants-wp127`). Log: `engine/target/wp127/mutants-r3.log`.
  - **32 of 32 killed**: 26 engine and 6 plugin mutants.
  - New in round 2: clip before redaction, `plan show`'s filter, a blank
    text, direction characters kept, the cap counted in characters, a split
    character, reading past the paragraphs needed. The last one runs against
    the bench-profile timing test.
- `flock /tmp/seldon-check.lock just check`: see "Check (round 2)".
- `grep /home/` over the diff: no new hits.

### Not here (brief)

- `seldon decide accept` (E7) is a separate WP.
- A PEM / private-key redaction rule is a separate redaction item.
- check-rss already fails on `next`. The reviewer measured base and branch
  the same (10 788 kB), so it is not a WP-127 delta.

### Correction to round 1

The "Not done / open" note says open cases add "up to about 0.8 KB" each.
Since the byte cap, the figure is about 1.06 KB (ADR-0038, Consequences).

### Check (round 2)

`flock /tmp/seldon-check.lock just check` on b9ee5e98 (the last code and
docs commit; this section adds only text) gives **exit 0, `check: ok`**.

- install 209, deploy 190.
- docs-check: 465 links.
- `omarchy plugin validate`; qmllint (46 files).
- Plugin tests: model 127, service-states 328, desk-view 1376, bar-view 194.
- Engine: fmt, clippy and all tests, with the default and `watch` features.
- shellcheck is not installed: `bash -n` only.

Log: `engine/target/check-wp127-r3.log` (dev host, not committed).

`just check-perf` (log `engine/target/wp127/perf-r2.log`):

- ×150 index build: median 68 ms.
- N5 line: 261 ns.
- One hook timing, `recorded curl line with a marker, 900 lines`, missed its
  5 ms budget at load average 10.9 (5.30 / 5.44 ms).
- At load ≈ 2.6 I re-measured. Base 109d02eb (temporary worktree, removed)
  gave 4.42 / 4.48 ms; this branch gave 4.47 / 4.48 / 4.55 / 4.42 ms. That is
  no measurable delta, so the miss was the busy host.
- The budget leaves little room on this host. That is for the orchestrator,
  not this WP.

## Accepted and merged next

- The operator accepted **ADR-0038** on 2026-10-07 (E15), after Fable's
  stage 2 approved 2a9dc3d5.
  - The ADR's status reads "accepted (operator decision 2026-10-07, after
    Opus ×2 and Fable stage 2)".
  - The DECISIONS.md row reads accepted.
  - §3 gained Fable's sentence: "A hand-written source on any case is shown
    the same way: it is the user's own file, and the field moves nothing."
  - Commit 63756c1b.
- `git fetch`, then `origin/next` (d4a01867) was merged into this branch as
  d828aa8d. It brings in WP-125 (the graph) and the work files of WP-129,
  WP-131 and WP-135 to WP-140.
- Two conflicts, both resolved by keeping both sides:
  - `tests/plugin/model.test.js`: the ADR-0038 tests and the graph tests.
  - `docs/TESTING.md`: the ADR-0038 sentence and the graph paragraph in the
    Model.js section.
  - The rest merged on its own. The graph's sample counts hold with the
    imported C-2026-007.
- `flock /tmp/seldon-check.lock just check` on d828aa8d: **exit 0, `check: ok`**.
  - validate-fixtures ok; docs-check 465 links; install 209; deploy 190.
  - `omarchy plugin validate`; qmllint (46 files).
  - Plugin tests: model 143, service-states 328, desk-view 1478, bar-view 194.
  - Engine: fmt, clippy, and all tests with the default and `watch` features.
  - shellcheck is not installed: `bash -n` only.
  - Log: `engine/target/check-wp127-merge.log` (dev host, not committed).
