# WP-077 HANDOVER

Branch `wp/077-review`, worktree `wt/WP-077`. The branch was
fast-forwarded to `0af4209` (main's "start WP-077 and WP-078", which moved
this WP to `work/active/`). Commits on top of it:

- `f031919` engine: prepare the case save before the ledger write in log, event, hook and drift link
- `525b838` engine: name refused ids and values escaped in validation errors
- `4783538` engine: plan list warns of a case file that does not load and lists the rest
- `f4da93f` docs: SPEC-ENGINE §3 on the prepared case save and plan list's warnings
- `93ca3e9` scripts: the reference derive() clips index texts as the engine does
- `44fdd89` changelog: the prepared case save, escaped values, plan list warnings, the reference clip
- plus the commit with this handover and the pitfalls entry

## Done

### 1. A refused case save writes nothing (WP-066 follow-up)

- `logbook/cases.rs`: `CaseFile::prepare(&self, logbook, change)` clones
  the case, applies `change` to the clone and runs the save's checks on
  it. Those checks are now one private `checked()` that `save` uses too:
  - the file is still at its path;
  - `model::update` accepts the case (the WP-066 read-back);
  - the target folder has no other file of the same name.
- The ids the ledger will assign are not known yet. `cases::pending_ids(n)`
  gives `n` distinct stand-in ULIDs of the same shape, so the dry run
  makes the same change to `events:`.
- It is called before the ledger write in all four callers:
  - `log`: after `cases::find`, before `journal::prepare`. So nothing is
    written: no ledger line, no journal entry.
  - `event`: before `emit_one`.
  - the agent hooks (claude-code, generic), both through `record`: one
    stand-in id per event, before `ledger.append`. The hook still exits
    0. The refusal is printed on stderr and the command is not recorded.
    (Wording corrected in round 2, N2: it said only `hook claude-code`.)
  - `drift link`: the same `reconcile::attach` and `add_agent` closure,
    run on the clone before `emit` and on the file after it.
    `drift explain` writes a new case whole (`render_new`); it has no
    `model::update` and so needs no prepare.
- Tests: every one changes the case so that its flow list continues at
  column 0, runs the command, and compares every file of the logbook
  before and after with the new `common::tree`.
  - `frontmatter.rs::refused_saves::` has three tests (`log`, `event`,
    `drift link`) on a copy of the fixture logbook. C-2026-004's
    `events:` gets the column-0 line. Each command must exit 1, name
    "update refused", and leave every file unchanged.
  - `hooks.rs::claude_code::a_case_whose_save_is_refused_records_nothing`
    uses `agents:` on the active case. The hook exits 0, says "update
    refused" on stderr, records no command, and the logbook is
    byte-identical.

### 2. Validation errors print no raw control characters

- `escape_debug()` on the id or value in:
  - `model/case.rs`: id, area, agents, events, and the `str_enum!`
    `FromStr` message (status, zone, risk, priority);
  - `model/decision.rs`: id, supersedes, cases;
  - `model/journal.rs`: cases;
  - `model/area.rs`: name;
  - `model/event.rs`: actor, case;
  - `model/mod.rs`: `Language::from_str`;
  - `import/omarchy_agent.rs`: type, id, date value, status.
- serde's own messages name a refused value raw. One example is a case
  `status: "\e[31mX"` → `unknown variant `…``. I found it with the new
  test.
  - `FrontmatterError::Yaml` now shows its message through the new
    `frontmatter::printable`. That is `escape_debug` per character, with
    quotes and backslashes left as they are, so the rest of serde's
    message reads unchanged.
- Tests (an ESC sequence `\e[31mX` in every position): each message must
  hold no control character and must hold `\u{1b}[31mX`.
  - `frontmatter.rs::case_ids::a_refused_value_is_named_escaped`: 14
    positions over case, decision, journal and area, serde's enum
    messages included.
  - `import::omarchy_agent::tests::a_refused_value_is_named_escaped`:
    7 kit-case keys.
  - `model::event::tests::a_refused_value_is_named_escaped`: actor and
    case.
  - `model::tests::a_refused_language_is_named_escaped`.

### 3. `plan list` with one invalid case

- `cases::all` (used only by `plan list`) returns
  `(cases, warnings)`. A file that cannot be read or parsed is a warning
  with the same wording as the index's:
  `<path>: invalid case: …; skipped` or `<path>: cannot read: …; skipped`.
  The other cases are listed, and the exit is 0.
- The human output prints `warning: …` lines after the list (the
  `warnings_human` shape of `status`/`index`). `--json` gains
  `warnings`, which is empty when there is nothing to report.
- Test: `plan.rs::plan::list_warns_of_an_invalid_case_and_lists_the_rest`.
  Of three cases, the middle one gets an ESC status. The test checks the
  JSON ids and the warning (path, wording, escaped value), and that the
  human stdout has two case lines and one warning line with no control
  character. `list_and_show` asserts `warnings: []`.

### 4. SPEC-ENGINE §3

- One comment block under `seldon event` covers the prepared case save
  of log, event, the agent hooks (claude-code, generic) and drift link
  (exit 1; a hook records nothing and says so on stderr).
- One comment on `seldon plan list` covers the warning line, `--json
  warnings` and exit 0.

### 5. The reference `derive()` clips as the engine (ADR-0025)

- `scripts/validate-fixtures.py` gains `TEXT_MAX = 256`, `json_len`,
  `clip` and `clipped`, mirroring `engine/src/index/build.rs`:
  - the bytes per character as serde_json writes them: 2 for
    `" \ \n \r \t \b \f`, 6 for other C0 characters, UTF-8 length
    otherwise;
  - room = 256 minus the UTF-8 length of the marker for the whole
    character count;
  - the cut on a character boundary;
  - the head stripped of Rust's White_Space (`WHITE_SPACE`; Python's
    `rstrip()` would also strip U+001C..U+001F);
  - the marker `… (N more character[s] in the ledger)`, where N is
    `len(text) - len(head)`.
- It is applied where the engine applies it:
  - `index.events`: `detail`, `resolutionDetail` and every string in
    `meta`;
  - `index.drift`: `detail`.
- New `--derive LOGBOOK [--today DATE]` prints the derivation as JSON,
  with problems on stderr and exit 1 when there are any.
  `just schema-validate` is unchanged and still passes: every fixture
  text is short.
- Test: `index.rs::the_reference_derive_clips_texts_as_the_engine_does`
  is fixture-independent in its texts.
  - On a copy of the fixture logbook it appends 42 caseless
    `config-change` events. Their `detail`, `meta.command` and
    `meta.note` hold long probes: 7 units × 6 paddings, so that the cut
    lands on each kind of character: multi-byte, 2- and 6-byte escapes,
    `\b\f`, Unicode white space, and U+001C..U+001F.
  - Half are dismissed with a long reason (`resolutionDetail`); the
    other half stay open drift items.
  - `index.events` and `index.drift` of `seldon index` must equal those
    of `validate-fixtures.py --derive` on the same copy.
  - The test also asserts that all 147 probe texts are clipped and that
    each is ≤ 256 JSON bytes.
  - It skips with a note when no `python3`/`python` is on PATH. CI has
    it, because `schema-validate` needs it.
- `docs/TESTING.md`:
  - a new `engine/tests/index.rs` row names the two clip tests
    (`clip_keeps_short_texts_and_marks_long_ones`,
    `the_reference_derive_clips_texts_as_the_engine_does`) and the other
    WP-076 budget tests;
  - the `frontmatter.rs` and `plan.rs` rows name the WP-077 tests;
  - "Monorepo layout" notes that `index.rs` runs the script.

### 6. CHANGELOG

- Three Engine lines: the prepared save, the escaped values, and
  `plan list`.
- One "Packaging and docs" line: the reference clip and `--derive`.

## Not done

- Not the whole `just check-perf`. The index bench and `status` are
  untouched (no change in `index/` or `status`), so I ran only the hook
  half (see "How verified").
- The "already listed" skip reason in `import/omarchy_agent.rs` is not
  escaped. It is a report line, not a validation error, and I had no
  test for it, so I took the change out again.
- The YAML parse error of the kit importer (`frontmatter is not valid
  YAML`) is not passed through `printable`. A mutant showed that no test
  can reach it: a control character only gets into YAML through a
  quoted escape, and then the YAML parses. Taken out again.

## How verified

- `cargo fmt --check` and `cargo clippy --locked --all-targets -- -D warnings`
  are clean after every commit.
- `bash scripts/validate-fixtures.sh`: ok (109 instances, 25 self-checks).
- `just check`: see the last section.
- Hook budgets (`hook.rs` changed: a recorded command with a case clones
  the case once more and runs one extra `model::update` before the
  ledger write).
  - Command: `cargo test --profile bench --test hooks -- --ignored
    --test-threads=1`, load 0.13, after WP-078's check had finished.
  - At 10 000 lines: not recorded 1.33 ms, recorded 1.88 ms.
  - At 950 lines: not recorded 1.33 ms, recorded 3.81 ms.
  - Budget 5 ms. WP-076 measured about 1.8 ms and 3.5 ms; the
    difference is within noise.
- Mutants, one per claim. Each was applied alone and run against its
  test, and each was killed (the test failed) unless noted:

| # | Mutant | Test | Result |
|---|---|---|---|
| P1 | `log`: prepare removed | `refused_saves` | killed |
| P2 | `event`: prepare removed | `refused_saves` | killed |
| P3 | `drift link`: prepare removed | `refused_saves` | killed |
| P4 | hook: prepare's error swallowed (`.ok()`) | `a_case_whose_save_is_refused_records_nothing` | killed |
| P5 | `CaseFile::prepare` does not apply `change` | `refused_saves` | killed (all 3) |
| E1 | case id not escaped | `case_ids::a_refused_value_is_named_escaped` | killed |
| E1b | case `agents` not escaped | same | killed |
| E1c | decision `supersedes` not escaped | same | killed |
| E1d | journal `cases` not escaped | same | killed |
| E1e | area `name` not escaped | same | killed |
| E2 | `printable` = identity (serde message raw) | same | killed |
| E3 | `str_enum!` FromStr not escaped | `import::…::a_refused_value_is_named_escaped` | killed |
| E4 | kit id not escaped | same | killed |
| E5 | event `case` not escaped | `model::event::…` | killed |
| E5b | event `actor` not escaped | same | killed |
| E6 | language not escaped | `a_refused_language_is_named_escaped` | killed |
| E7 | kit YAML error through `printable` removed | import test | **survived** → change removed (see Not done) |
| L1 | `cases::all` errors on an invalid case again | `list_warns_…` | killed |
| L2 | warning lines left out of the human output | `list_warns_…` | killed |
| M1 | Python `rstrip()` instead of `rstrip(WHITE_SPACE)` | `the_reference_derive_…` | killed |
| M2 | no strip | same | killed |
| M3 | C0 control = 2 bytes | same | killed |
| M4 | `\b`/`\f` = 6 bytes | same | killed, after adding a `\b\f` probe (it survived the first probe set) |
| M5 | N counted from the cut, not the stripped head | same | killed |
| M6 | marker length in characters, not bytes | same | killed |
| M7 | events not clipped | same | killed |
| M8 | drift detail not clipped | same | killed |
| M9 | `meta` strings not clipped | same | killed |
| M10 | `resolutionDetail` not clipped | same | killed |
| M11 | `TEXT_MAX = 255` | same | killed |
| M12 | marker text changed | same | killed |
| M13 | every character 1 byte | same | killed |
| M15 | `used >= room` | same | killed |
| M14 | engine side: `trim_end()` removed in `build.rs` | same | killed |

- A trap in my mutant script, now fixed and re-run (memory/pitfalls.md,
  WP-077):
  - The script restored the original with an older mtime, so cargo kept
    the last mutated binary until another crate file changed.
  - It showed as a false failure of the parity test after M14.
  - The script now `touch`es the restored file. I re-ran every mutant
    above with it; the results in the table are from that run. The
    baselines after the last mutant are green.

## Open questions

1. **Hook and a refused case.** The hook now records nothing when the
   case save would be refused (stderr names it, exit 0). I chose this
   because the WP says "never leaves a ledger note". The alternative is
   to record the command without the case. Which does the orchestrator
   want?
2. **ADR-0025** still says "The reference `derive()` … does not clip yet
   (follow-up WP-077)". ADRs are immutable, so I left it. Superseding an
   ADR for that one line seems too much; a note in DECISIONS.md or the
   ADR index may be enough. Orchestrator's call.
3. `plan list --json` gains a `warnings` key. It is CLI output, not
   `schema/*.json`, so I made no contract bump. The plugin does not call
   `plan list`.

## Touched outside the listed inputs

- `engine/src/frontmatter.rs`: `printable`, and the `Yaml` variant's
  Display.
- `engine/src/model/{area,event,mod}.rs`: escapes and unit tests.
- `engine/src/logbook/cases.rs`: the `all` signature (its only caller is
  `plan list`).
- `engine/tests/common/mod.rs`: `tree()`, every file of a directory with
  its bytes (as in `import.rs`, which keeps its own copy).
- `memory/pitfalls.md`: the WP-077 section.

## `just check`

- Run once, at the end, on the code of `44fdd89` (the CHANGELOG commit
  was only reworded afterwards), plus the uncommitted handover and
  pitfalls entry.
- I waited for WP-078's own `just check` to exit first: its plugin
  harness was running.
- Result: **exit 0** (`check: ok`). It includes:
  - fmt, clippy, test, check-watch, check-packaging, check-install;
  - schema-validate, docs-check (393 links);
  - plugin-validate, qmllint, plugin-test (service 247, panel 733,
    overlay 319, bar 131 passed).

## Round 2 (review: SEND BACK, B1 and N1–N4)

Commits on top of `2773689`:

- `5be8ed4` engine: hook generic names a refused actor, case or startedAt escaped
- `295f61e` engine: plan list and index name an invalid case file escaped in their warnings
- `493d0a9` docs: CLI reference names plan list's warning line, en and de
- `ee6cafa` docs(de): CLI reference source line at 493d0a9
- `e5c93be` docs: the agent hooks (claude-code, generic) in SPEC-ENGINE §3 and CHANGELOG, TESTING table row fixed, ADR-0025 row notes the reference clip
- plus the commit with this section

### Done

- **B1: `hook generic` escapes agent-supplied values.**
  - `escape_debug()` at three sites:
    - `commands/event.rs` `parse_actor`;
    - `commands/event.rs` `parse_case_id`;
    - the `startedAt` message in `commands/hook.rs` `generic`.
  - Also the latent `logbook/cases.rs` `check_id`. Every caller of
    `cases::find` checks the id first today, so it has a unit test only.
  - On the clap path the value now shows escaped once. Example:
    `seldon log --case $'C-\e[31mX' x` gives
    `invalid value 'C-X' for '--case <ID>': `C-\u{1b}[31mX` is not a case id`.
    clap strips the control characters from its own `'C-X'` part, so
    nothing is escaped twice.
  - Tests:
    - `hooks.rs::generic::a_refused_payload_value_is_named_escaped`:
      an ESC sequence in `actor`, `case` and `startedAt` in turn. Each
      run must exit 0, show no control character on stderr (newline
      aside), show `\u{1b}[31mX`, and leave the ledger empty.
    - `logbook::cases::tests::a_refused_id_is_named_escaped`.
- **N1: file names in warnings.**
  - `cases::all` (the `plan list` warnings) and `index/load.rs` (the
    index's `invalid case … skipped` warning) pass the relative path
    through `frontmatter::printable`. Only the warning text changes; the
    path a loaded case carries is unchanged.
  - Test: `plan.rs::plan::a_warning_names_a_case_file_escaped`. It adds a
    file `work/queued/C-2026-009-<ESC>[31mX.md` without frontmatter.
    In both `plan list --json` and `index --json --check`, exactly one
    warning names it. That warning starts with
    `work/queued/C-2026-009-\u{1b}[31mX.md: ` and holds no control
    character.
- **N2:** SPEC-ENGINE §3, this handover (§1 and §4 above) and the
  CHANGELOG now say "the agent hooks (claude-code, generic)". The
  escaped-values CHANGELOG line also names a `hook generic` payload and
  a case file name.
- **N3:** the CLI reference's `plan list` section has hand-written
  prose above a generated `--help` block, so I added one sentence to
  that prose, not to the help text.
  - `docs/user/en/05-cli-reference.md`: an English sentence.
  - `docs/user/de/05-cli-reference.md`: the same sentence in German.
  - The German page is re-stamped to `493d0a9` the way earlier pages
    were: a separate `docs(de)` commit, because the stamp must name a
    commit that holds the current English page.
  - `docs-check` reports no freshness warning.
- **N4:** the blank line in `docs/TESTING.md` before the `watch.rs` row
  is deleted, so the `watch.rs` and `index.rs` rows are table rows
  again. The `plan.rs` and `frontmatter.rs` rows name the two new tests.
- **Decision 3:** the ADR-0025 row in DECISIONS.md now ends "; the
  reference clip followed in WP-077". The ADR itself is unchanged.
- **Decision 2** (the hook records nothing) needed no change. Open
  questions 1 and 2 above are answered by it and by decision 3.

### Mutants (round 2)

Each was applied alone, with the fixed restore (`touch`):

| # | Mutant | Test | Result |
|---|---|---|---|
| R2-B1a | `parse_actor` value not escaped | `generic::a_refused_payload_value_is_named_escaped` | killed |
| R2-B1b | `parse_case_id` value not escaped | same | killed |
| R2-B1c | `startedAt` value not escaped | same | killed |
| R2-B1d | `check_id` value not escaped | `cases::tests::a_refused_id_is_named_escaped` | killed |
| R2-N1a | `plan list` warning path raw | `a_warning_names_a_case_file_escaped` | killed |
| R2-N1b | index warning path raw | same | killed |

### How verified (round 2)

- `cargo fmt --check`: clean.
- `cargo clippy --locked --all-targets -- -D warnings`: clean.
- `cargo test --lib --test hooks --test plan --test index --test frontmatter`:
  - lib 192 passed;
  - frontmatter 18 passed;
  - hooks 53 passed, 2 ignored (perf);
  - index 25 passed, 1 ignored (perf);
  - plan 17 passed.
- `bash scripts/validate-fixtures.sh`: ok.
- `just docs-check`: ok (393 links, 14 translated pages, 40 commands,
  469 command lines).
- `just check`: not run in this round, as instructed; the orchestrator's
  gate runs it.

### Touched outside the listed inputs (round 2)

- `engine/src/index/load.rs` (N1), `DECISIONS.md` (decision 3), and
  `docs/user/{en,de}/05-cli-reference.md` (N3). All of these were named
  by the review.
