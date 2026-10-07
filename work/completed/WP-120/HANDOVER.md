# WP-120 — Handover

Branch `wp/120-contract-v2`, base `next` @ 94e1fab. Not merged: ADR-0035
is **proposed** (the orchestrator accepts it), review stage 1 Opus, stage
2 Fable (contract). Target: `next`.

## Done

- **ADR-0035** (proposed, e12b737 + wording fixes): contract 2 — the
  bundle of 2026-10-04 plus ADR-0034's two fields. DECISIONS.md row.
- **Schema.** `index.schema.json`: `contractVersion` const 2;
  `logbook.git.autocommit {ok, at, message}`; drift `truncated`;
  `decisions[].cases` (required, case ids); optional `triage {id, at,
  actor, counts {items, crises}, path, applied}`. `event.schema.json`:
  kinds `case-updated`, `state-loss`; `meta.risk` (only on
  `case-created|started|updated`, required on `case-updated`, which also
  needs `case` and `source: seldon`); `meta.truncated` (const true, index
  only). New `proposal.schema.json` (the file `triage.path` points at).
  Must-fail fixtures: `index.contract-v3` (was `-v2`), `event.risk-on-note`,
  `event.case-updated-without-risk`, `proposal.link-without-case`,
  `proposal.without-evidence`.
- **Engine.** `CONTRACT_VERSION` 2 (`seldon contract-version` prints 2).
  - Risk record: `meta.risk` on every new `case-created` (plan new, reopen,
    `agent start --new`, drift explain's completed case) and `case-started`;
    `plan set` writes `case-updated` (detail = the Log line's words,
    `meta.risk` = risk after; `--json` gains `event`); ledger first, then
    the case file. `Event::validate` enforces the kind rules.
  - Harm guard (`reconcile::PlanningCase`, `RiskRecord`, `ledger_risks`,
    `told`): the ledger record when the case's `case-created` carries
    `meta.risk`, to the second; otherwise WP-115's Log record unchanged;
    the fail-safe (record's last risk ≠ frontmatter) in both.
  - `state-loss`: the capture's state reset writes kind `state-loss`
    (subject `state-reset`, same detail and meta); `collectors::
    is_state_loss` accepts the old `note` everywhere it is read (capture
    dedup and crash marks, doctor).
  - Index: `meta.truncated` on clipped events (a hand-written one is
    dropped), drift `truncated`; `decisions[].cases` (as written, no
    repeats); `logbook.git.autocommit` from `<state>/autocommit.json`,
    written by `commands::autocommit*` on every attempt, attached at all
    four places that set `logbook.git`; `triage` from `<state>/proposals/`
    in `index::derive_at` (`index/triage.rs`, schema-checked with the
    compiled-in `proposal.schema.json`).
  - `seldon event` refuses `case-updated`, `state-loss`, `--meta risk`,
    `--meta truncated`. Month views show a `case-updated` line's detail.
- **Fixtures.** The sample logbook speaks contract 2 from the start of
  10-01: `state-loss` at 08:55 (config, owned), `case-updated` C-2026-003
  R2 → R3 at 09:00:40 (its Log line existed), `meta.risk` on the five case
  lines of 10-01; the 09-12 note is long (the one clipped text, also in
  the journal); ADR-0003 names C-2026-004 and C-2026-005;
  `fixtures/proposals/01M3VZS4J0NDXZFC2F7RBBD3FJ.json` (link tokyo-night →
  C-2026-005, explain monitors.conf, crisis ollama unit); the sample has
  `logbook.git.autocommit` and `triage`. Regenerated: sample, attention-all,
  all nine variants (`not-initialised` drops `triage`; two overlay indices
  moved by 2), `ledger/2026-10.md` (from the engine), `STATUS.md` counts.
  85 ledger lines, 75 index events.
- **Reference derive** (`scripts/validate-fixtures.py`): truncated, decision
  cases, `ledger_risks`/`told` in rule 9, `derive_triage` and
  `check_proposals` (items are open drift items of the sample, crisis flags
  agree, link cases open, one link/explain/crisis, `at` not after
  `generatedAt`), proposal schema mapping, ledger `meta.truncated` refused,
  the case walk checks `case-updated` and `meta.risk` from
  `CONTRACT_2_FROM`; three new self-checks (54).
- **Plugin.** `CONTRACT_VERSION` 2 in `Model.js` and `manifest.json`
  (`engineMin` stays 0.1.0 until WP-126). `parseIndex` needed no change
  (it never rejected unknown fields); `model.test.js` proves it accepts
  every v2 field and a v2 index without the optional ones.
- **Harness.** `harness/shell.qml` emits `bannerDetail` and
  `pluginContractVersion` from outside the snapshot (so an old plugin
  reports them). `service-states.sh`: v3 index → "Update the plugin", v1
  index → "Update the engine", both numbers in the detail; **the plugin of
  the `v0.1.3` tag** (`git archive`; without the tag: this plugin set back
  to contract 1) against the v2 sample → `contractMismatch`, "The index
  uses contract v2, this plugin reads v1. Update the plugin." Count and
  cursor expectations of `panel-view.sh`, `overlay-view.sh` and
  `model.test.js` follow the two new 10-01 events.
- **Docs.** CONTRACT.md (rules 1, 5, 8, new 9), SPEC-ENGINE §2 (two state
  files), §3 (`plan new`/`set`, state reset), §5 rule 9, §6;
  SPEC-LOGBOOK §4; guides 07 and 10 en/de (state-loss), German source
  lines moved; SPEC-PLUGIN §1 manifest line; CHANGELOG **Breaking** and an
  Engine line; fixtures/README.md; TESTING.md rows. `docs/VERSIONING.md`
  unchanged.

## Decisions (what the WP left open)

1. **One new kind for `plan set`: `case-updated`**, written for every
   change (zone, risk or area), `meta.risk` always the risk after. A
   risk-only kind would leave zone/area changes without a ledger line; a
   `note` would put engine state under a kind the user writes.
2. `meta.risk` only on created/started/updated; verified/completed/dropped
   never change the risk.
3. **No mixing** of records: the ledger tells a case iff its
   `case-created` line has `meta.risk`; a pre-v2 case stays on its Log even
   when later lines carry `meta.risk` (its Log has every `set` line too).
   Same-second lines count only if they agree (WP-115's same-minute rule,
   at the ledger's precision).
4. `state-loss` keeps the subject `state-reset` and WP-081's detail/meta,
   so the readers and the docs' wording stay; only the kind changes.
5. **Drift items get a top-level `truncated`** (they have no `meta`).
6. `truncated` is index-only: refused on write, dropped on read.
7. Autocommit record: attempts only (a skip changes nothing), one line,
   redacted at write and again at build, ≤ 256 characters, bound to the
   canonical logbook path; absent with `[git] autocommit = false`, without
   `logbook.git`, or for another logbook's record.
8. `decisions[].cases` is required (always written), deduplicated in
   order, not resolved against the index.
9. **Triage:** newest proposal by id, `path` relative to the index's
   directory (works for the plugin's `SELDON_INDEX` dev mode), counts as
   proposed (the desk reads each item's open state from `index.drift`),
   `applied` null or the time. Invalid / misnamed / unreadable → skipped
   with a build warning; another logbook's (`logbook` field) → skipped
   silently. **The proposal file's shape** (`proposal.schema.json`) is
   fixed here because the fixture and the desk need it; ADR-0035 says
   ADR-0036 may refine it before 0.2.0 is tagged without another bump —
   see Open questions.
10. Fixture story: contract 2 from the start of 10-01 (one capture's
    state loss, the C-2026-003 raise as a ledger line); the clipped text is
    an existing note made long (no new event).
11. The 0.1.x-plugin case runs the real `v0.1.3` plugin; the fallback only
    exists for clones without tags.

## Not done

- `engineMin` 0.2.0, versions 0.2.0, release notes: WP-126.
- `docs/user/*/01-getting-started.md` shows a doctor sample "seldon 0.1.3,
  contract 1": a release-version sample, left for WP-126.
- SPEC-PLUGIN beyond the manifest line: WP-121 rewrites it.
- Writing/applying proposals (`drift propose|apply`, `agent ask`): WP-124.

## Verification

- `flock /tmp/seldon-check.lock just check` (`CARGO_TARGET_DIR` =
  `engine/target` in the worktree, on disk): at f5eb3e9 **exit 0**, `check: ok`.
  Rust 1970 passed, 0 failed, 10 ignored (default and `watch` feature
  runs together); clippy and fmt clean; `validate-fixtures: ok` (130
  instances incl. 12 expected failures, 85 ledger events, 9 variants, 54
  self-checks); `docs-check: ok`; `plugin-validate: ok`; `qmllint: ok`
  (29 files); model 98, service-states 328, panel-view 908, overlay-view
  326, bar-view 194, real-home-guard 11. `check-packaging` ran `bash -n`
  only (no shellcheck on this host).
- Earlier full run at 4dec6a7: exit 0, the same numbers.
- Engine suite also with `TZ=UTC` (as CI): 0 failed. Root: not runnable
  here (no `sudo`, red zone); the tests' user-owned root probe ran as the
  user. New tests: `engine/tests/contract_v2.rs` (6), `planned_link.rs`
  (2), `idempotency.rs` (1), `index.rs` (1 + the truncated assertions in
  the reference clip test), `reconcile.rs` unit (1), `index/autocommit.rs`
  unit (1).
- Mutant: `ledger_risks` always `None` → `the_guard_reads_the_ledger_
  record_when_there_is_one` (unit) and `the_ledger_record_wins_over_an_
  edited_log` (e2e) fail; restored.
- `just check-perf` (bench profile, under the lock, load < 4): exit 0.
  Index ×10 median 4.9 ms, ×150 (12 750 lines) 57.3 ms; `status` at the
  stated scale (now 10 540 lines) 44.4 ms; hooks ≤ 4.5 ms (900-line curl
  marker), unrelated session ≤ 0.85 ms; redaction within budget. A first
  run at load 15 missed two hook budgets (1.18 ms / 7.1 ms); the quiet
  rerun passed.
- `just check-rss`: **fails, also on the base.** `seldon watch` on ×10,
  bench profile, three runs each: base 94e1fab peak 10 420 / 10 296 /
  10 488 kB, WP-120 10 584 / 10 596 / 10 592 kB, limit 10 240 kB. WP-120
  adds ~100–170 kB (heap 2 772 → 2 832 kB: the larger fixture ×10, the
  compiled-in proposal schema). Not changed here: the budget or the
  host's baseline is the orchestrator's (see Open questions).
- Harness (in `just check`): the v0.1.3 plugin case ran the real tag.

## Open questions

- **Orchestrator:** accept ADR-0035, including §6's last sentence (ADR-0036
  may refine the proposal file before the 0.2.0 tag without a bump, since
  no released engine or plugin speaks v2). If that is not wanted, any
  WP-124 change to `proposal.schema.json` is contract 3.
- **`check-rss`** is red on `next` itself on this host (peak 10.3–10.5 MB
  against 10 MB at 94e1fab); WP-120 adds ~0.15 MB. Either the budget or
  what the test measures needs a decision (a WP of its own).
- **WP-121** rebases on this fixture: the sample has two more events on
  10-01 (75 rows, the heatmap's 10-01 cell 32), so its new `desk-view.sh`
  counts should start from these numbers; `panel-view.sh` and
  `overlay-view.sh` here already carry them.

## Merge of next

`origin/next` @ 38a9103 (WP-121, the desk shell) merged into this branch.

- Conflicts: `docs/SPEC-PLUGIN.md` §1 manifest — next's desk manifest
  (`Desk.qml`, `deskWidth`, `deskSidebar`) with this WP's `contractVersion`
  2. `tests/plugin/panel-view.sh` and `overlay-view.sh` (modify/delete) —
  next's side, deleted (orchestrator decision; `COVERAGE.md` maps them to
  `desk-view.sh`). This WP's changes there were count and cursor shifts
  only (two more 10-01 events, 23 lines); no contract-2 row was lost: the
  contract-2 cases (v3 index, v1 index, the `v0.1.3` plugin against the v2
  sample, `bannerDetail`, `pluginContractVersion`) live in
  `service-states.sh` and the harness `shell.qml`, both kept.
- Auto-merged and checked: `plugin/Model.js` `CONTRACT_VERSION = 2`,
  `manifest.json` `seldon.contractVersion: 2`, justfile `plugin-test` runs
  `desk-view.sh`.
- WP-121 expectation brought to the v2 sample: `model.test.js` "deskKpis
  and deskCounts on the sample" — today's KPI 30 → 32. `desk-view.sh`
  needed no change (its scenarios assert no row or day counts of the
  sample); it passes on the v2 fixture.
- `flock /tmp/seldon-check.lock just check` on the merge state: **exit 0**,
  `check: ok`. Rust 1970 passed, 0 failed, 10 ignored; `validate-fixtures:
  ok` (130 instances, 85 ledger events, 9 variants, 54 self-checks);
  `docs-check: ok`; `plugin-validate: ok`; `qmllint: ok` (49 files); model
  108, service-states 328, desk-view 373, bar-view 194, real-home-guard 11.
  (The run started before the two `git rm`s; neither file is run by the
  justfile on next.)
- `check-rss`: unchanged, goes to the operator as its own question.

## Round 2

Stage-1 review (Opus, SEND BACK) and the orchestrator's brief; code at
54c998e. Nothing new merged from `next`.

- **B1** (a contract-1 ledger with a user `meta.risk`): `Meta.risk` is read
  leniently (`lenient_risk`: R0–R3 or none; the line always loads);
  `index::build::clipped` drops `meta.risk` unless the line is the engine's
  `case-created|started|updated` (`source: seldon`); the same in
  `validate-fixtures.py` (`risk_line`, `RISKS`; its `ledger_risks` takes only
  valid values). The guard already read only `source: seldon` case lines;
  now pinned. ADR-0035 §1 and CONTRACT.md rule 9 carry the sentence ("a
  `meta.risk` on another kind, or written by hand before v2, is ignored on
  read and dropped from the index"), SPEC-ENGINE §6 too. Tests:
  `contract_v2::a_contract_1_ledger_with_a_user_risk_still_indexes` (notes
  with `R1`, `banana`, `high` + `truncated: "yes"`: `index --check` exit 0,
  `valid`, no warning, all three listed without `risk`/`truncated`, a user
  key kept, doctor `ledger` ok); `planned_link::a_hand_written_risk_does_
  not_fool_the_guard` (a 0.1.x note `risk: R3` on an R1 case: not linked,
  "was R1 at the time", the note listed); `reconcile::only_the_engines_
  case_lines_tell_the_risk` (note, manual `case-updated`, agent
  `case-created`, seldon `case-verified` and note with R3 count for
  nothing); `event::a_user_risk_reads_leniently`; the reference parity test
  (`index::the_reference_derive_clips_texts_as_the_engine_does`) now also
  carries two such notes, engine = Python derive.
- **B2** (autocommit redaction untested): `contract_v2::a_failed_autocommit_
  is_redacted_everywhere` (a refusing pre-commit hook prints
  `https://user:geheim@…`, `token=abc123geheim`, `--password hunter2` and
  300 characters: none of the secrets on stderr, in `--json` `git.error`,
  in `autocommit.json` or in the index; `‹redacted›` in each; the index
  message ≤ 256 characters, ends in `…`, one line; a planted unredacted
  record is redacted at build); `index::autocommit::tests::the_record_is_
  redacted_when_written`.
- **N1**: `seldon event` names the writer per kind (`writer()`): case kinds
  → `seldon plan`, resolution/correction → `seldon drift`, `state-loss` →
  `seldon capture`. Test: `contract_v2::event_refuses_the_v2_kinds_and_keys`.
- **N2**: chose **warn**: a `.json` in `proposals/` not named `<ULID>.json`
  (also a lowercase ULID) gets "not named <ULID>.json, so not a proposal";
  dotfiles and non-JSON files pass silently. ADR §6, SPEC §6 say so. Test:
  `contract_v2::triage_points_at_the_newest_proposal_of_this_logbook`.
- **N3**: `invalid/event.case-updated-without-risk.json` has `meta:
  {zone: red}`; it fails for the missing `risk` (schema mutant below).
- **N4**: `state-loss` requires subject `state-reset` — schema allOf and
  `Event::validate` (`STATE_LOSS_SUBJECT`, `collectors::STATE_RESET` is
  it); must-fail `invalid/event.state-loss-other-subject.json`; unit test in
  `event::tests::validation_rules`.
- **N5**: `plan set` writes a new area's README before the `case-updated`
  line. Test: `close_path::set::an_area_that_cannot_be_made_writes_nothing`
  (`areas/boot` a file: exit ≠ 0, no line, case file unchanged; after the
  fix one line).
- **N6**: `commands::redacted_git_error` puts the git error of
  `autocommit`/`autocommit_paths` through the logbook's redaction before
  the stderr warning, `--json` `git.error` and the record. SPEC-ENGINE §7.
  Test: the B2 e2e.

Mutants (applied, the named tests run, restored from HEAD; all killed):
M9 both `shown()` raw → `the_record_is_redacted_when_written`; M9a record
raw → same; M9b build raw → `a_failed_autocommit_is_redacted_everywhere`;
M10 git error raw (N6) → same; M11 strict `meta.risk` read →
`a_user_risk_reads_leniently`; M12 index keeps a foreign `risk` →
`a_contract_1_ledger_with_a_user_risk_still_indexes`; M13 guard takes any
source → `only_the_engines_case_lines_tell_the_risk`; M14 `state-loss` any
subject → `validation_rules`; M15 misnamed proposals silent → `triage_…`;
M16 `state-loss` refusal names `seldon plan` → `event_refuses_…`. Schema
mutants: `case-updated` without `meta.required: [risk]` and `state-loss`
without the subject const each let their must-fail fixture pass →
`validate-fixtures` fails.

`flock /tmp/seldon-check.lock just check` at 54c998e: **exit 0**, `check:
ok`. Rust 1984 passed, 0 failed, 10 ignored; `validate-fixtures: ok` (131
instances incl. 13 expected failures, 85 ledger events, 9 variants, 54
self-checks); `docs-check: ok`; `plugin-validate: ok`; `qmllint: ok` (49
files); model 108, service-states 328, desk-view 373, bar-view 194,
real-home-guard 11.

Open for the orchestrator, from the packet: the review's question 1 (drop
vs. rename a v1 user `risk`) — this round drops it, as the brief says;
questions 2 (ADR §6 last sentence) and 4 (`check-rss`) unchanged; N6 is
fixed here, not a WP of its own.

## Round 3

Fable stage 2: accept ADR-0035 after this round; code at 25e114b.

- **State-file guard:** `sys::read_small_file` (`STATE_FILE_MAX` 4 MiB)
  checks `symlink_metadata` before the open — only a regular file, no
  link, FIFO, device or directory — and the size; the read itself stops
  after `max + 1` bytes. `index::triage` reads every proposal through it
  ("not read (…); the index skips it"), and `autocommit::attach` reads
  `autocommit.json` through it. `attach` now returns a build warning for
  an unreadable, oversized or non-regular record or one that is not a
  record (another logbook's stays silent). All four call sites pass it
  on: rebuild, `index`/`status` and `watch` into their warnings, the
  session-stop hook on stderr.
  Tests: `contract_v2::state_files_that_are_no_regular_small_files_are_
  skipped`. As a proposal named `<ULID>.json` and as `autocommit.json` it
  puts a FIFO (made by `mkfifo` from the test, no shell string), a symlink
  to `/dev/zero` and a 5 MiB file. Each gets one warning, the build
  finishes within the 20 s limit (`run_within` kills a hung run) with a
  valid index, and the older valid proposal is still found. Also
  `sys::tests::small_regular_files_only`.
- **Control characters:** `autocommit::shown` maps every
  `char::is_control` to a space before the redaction and the clip. Test:
  `index::autocommit::tests::control_characters_are_spaces` with
  `fatal: \x1b[31mred\x1b[0m\ttab \x07bell`.
- **Wording:** all five edits:
  - ADR-0035 §6: the last sentence replaced (optional fields and
    proposal refinements within v2 until the tag, under the stated
    conditions); the `crisis` sentence (the engine decides at apply
    time); the 4 MiB regular-file rule.
  - CONTRACT.md rule 9: the `crisis` sentence, the 4 MiB rule and the
    optional-field rule.
  - SPEC-ENGINE §2: the rule on the `autocommit.json` and `proposals/`
    rows.
  - VERSIONING.md: the downgrade paragraph names the `case-updated` and
    `state-loss` lines and says not to delete them.
  - ADR-0035 Consequences, first bullet: the exact 0.1.x behaviour.

  The optional fields of the follow-up WP are not added here.
- **Mutants:** all killed:
  - R1, no type check: `state_files_…`.
  - R2, no size check before the open: survived at first, because the
    read cap gives the same result. It is killed now by the exact
    message in `small_regular_files_only` (25e114b).
  - R3, `attach` reads plainly: hangs on the FIFO, `state_files_…`
    fails.
  - R4, control characters kept: `control_characters_are_spaces`.
- **Gate:** `flock /tmp/seldon-check.lock just check` at 25e114b (log
  `wp120-round3-check-25e114b.log`): **exit 0**, `check: ok`.
  - Rust 1990 passed, 0 failed, 10 ignored.
  - `validate-fixtures: ok` (131 instances incl. 13 expected failures, 85
    ledger events, 9 variants, 54 self-checks).
  - `docs-check: ok`; `plugin-validate: ok`; `qmllint: ok` (49 files).
  - Plugin tests: model 108, service-states 328, desk-view 373, bar-view
    194, real-home-guard 11.
