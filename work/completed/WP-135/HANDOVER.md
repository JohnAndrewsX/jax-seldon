# WP-135 — Handover: `seldon decide accept`, Accept in one click

Branch `wp/135-decide-accept` from `next` (d95b7da3); merge into `next`.
`next` has moved to 41e90151 since (work/queued/WP-140–143 only, no
overlap); no merge was needed. Plan: [PLAN.md](PLAN.md).

## What was done

- **Engine** (`engine/src/commands/decide.rs`): `seldon decide accept
  <ADR-NNNN> [--actor A] [--json]`, a subcommand of `decide`
  (`args_conflicts_with_subcommands`; the title form is unchanged, a
  title "accept" goes after `--`). Order, as a plan step:
  1. the user's act, checked before anything is read (`user_actor`): an
     agent `--actor`, an agent `$SELDON_ACTOR` without `--actor`, and
     `--actor human` in an agent's session → exit 1 naming the conflict
     and the way out; `system` is refused by `parse_person`;
  2. logbook, lock, `Logbook::decision_file` (the lookup `seldon open`
     had, moved to `logbook/mod.rs` and shared), load;
  3. accepted already → exit 0, `already: true`, nothing written, no
     event, no commit; superseded, unknown id, unreadable file, or a
     frontmatter naming another id → exit 1, nothing written;
  4. `status: accepted`, `date` today via `model::update` (only the two
     keys change; read back before anything is written);
  5. the ledger first: one line `source: seldon`, `kind: note`,
     `subject` the id, `detail` `accepted: <title>`, actor `human`;
  6. the file (atomic), `DECISIONS.md`'s fence, autocommit `seldon:
     ADR-NNNN accepted`, index rebuilt.
  `--json`: `{decision: {id, title, status, date, cases, path}, already,
  event, git, warnings}`.
- **Contract**: CONTRACT.md's command table gains `seldon decide accept
  <ADR-NNNN> --json`. The index's shape does not change. ADR-0035 §6 does
  **not** cover a new command (it allows optional fields and proposal
  refinements), so **ADR-0040 (proposed)** records the row, the ledger
  kind and the B2 rule. DECISIONS.md lists it as proposed.
- **Plugin**: `Model.acceptArgs` / `acceptResult` / `acceptArmHint`;
  `validateArgs` admits exactly `decide accept <ADR-NNNN> --json`;
  `decisionDetail` marks Accept as a write and says what it does.
  `Service.acceptDecision` (one at a time, busy family `accept`) and
  `acceptResult` (in the snapshot); a created decision still opens in
  the editor, an accepted one does not. `sections/Decisions.qml`: Accept
  arms (Arm.qml id `decision:<id>:accept`; the bar reads *Confirm
  accept* with the hint "Accept ADR-NNNN? Click Confirm: it becomes
  accepted with today's date."), the second click runs; any key, a click
  on another decision, another section or a new index disarms; disabled
  while nothing can write and while an accept is pending; the engine's
  answer at the top of that decision's detail (only that one's).
- **Tests**: `engine/tests/decide_accept.rs` (5 tests: accept with the
  file byte-compared except the two keys, ledger line, DECISIONS.md,
  index valid against the schema, commit, clean tree; idempotent over
  two runs incl. a later day; superseded / unknown / another id / a bad
  id with `--json`; four agent refusals with the tree and HEAD unchanged,
  the refusal without a logbook, a `SELDON_ACTOR=human` session accepts;
  `decide -- accept` makes a decision). `model.test.js`: acceptArgs,
  acceptResult, validateArgs good/bad forms, decisionDetail. Fake engine
  speaks `decide accept` (+ `FAKE_SELDON_ACCEPT_REFUSE`). `desk-view.sh`
  10c' `decisions-accept` (arm, key disarms, row click disarms, re-arm,
  confirm, argv exactly `decide accept ADR-0004 --json` and no `open`,
  accepted with the index, the answer only on its decision),
  `decisions-accept-refused`, `decisions-accept-locked`; 10a: Accept
  disabled in dev mode; 10c `decisions-live` now opens ADR-0004 by *Open
  in editor*. `service-states.sh` busy: a second accept is refused while
  one is pending. Harness `shell.qml` action `accept`.
- **Docs**: SPEC-ENGINE §3 (command, JSON, refusals; "no ledger event"
  now only for a new decision), SPEC-LOGBOOK §3 (who sets which status),
  SPEC-PLUGIN (Decisions' Accept; the busy families), AGENT-GUIDE (an
  agent's decision stays proposed), guides en/de (02-concepts,
  05-cli-reference with the regenerated help and a `decide accept`
  section; German source lines at 503ba989), CHANGELOG (Engine, Plugin),
  TESTING.md, COVERAGE.md.

## Decisions (what the WP left open)

1. **Ledger kind: a `seldon` `note`**, subject the ADR id, detail
   `accepted: <title>`, no `case`. "Kind as the SPEC fits": no event
   kind is about decisions; a `seldon` note is the engine's own record
   where none fits (capture's skill and snapper notes). A new kind would
   widen `event.schema.json`'s enum — a contract change ADR-0035 §6 does
   not allow within v2. No `case` even when the decision names one: a
   decision may name several, and the index carries `decisions[].cases`.
2. **"With the date" = `date` becomes the day of acceptance.** The
   proposal date stays in git (`seldon: ADR-NNNN proposed`). A separate
   `accepted:` key would be a frontmatter key the index cannot show
   without a schema field.
3. **Idempotent vs. "refuses not proposed"**: accepted already → exit 0,
   `already: true`, nothing written (the `drift` re-run pattern);
   superseded → exit 1. The agent refusal comes first, also for an
   accepted decision.
4. **Subcommand shape**: `decide accept` as the WP names it; a bare
   `accept` as the first word is the subcommand, so a decision titled
   "accept" needs `--` (documented; the plugin always passes `--`). The
   title form's options do not mix with the subcommand (clap error, 1).
5. **B2 refusal**: as `triage.rs` `user_actor` (WP-124) — the resolved
   actor and the session — with WP-102's message style; checked before
   the logbook is opened, so an agent gets exit 1, never 3 or 4.
6. **The argv row needs an ADR** (ADR-0040, *proposed*): ADR-0035 §6 is
   about schema fields. No `contractVersion` bump: the index is
   unchanged, and earlier rows were added without a bump too (WP-101's
   `plan reopen` under contract 1, WP-124's `drift apply` with
   ADR-0036).
7. **Plugin**: Accept has no key (it is a click, twice; WP-123's Enter
   shows the detail and `a` is Work's hand-to-agent); the hint says
   "Click Confirm". In dev mode Accept is shown but disabled, like Work's
   writing actions (before, it opened the editor and showed dev mode's
   refusal). The answer is shown only on the decision it is about.
8. **Engine's `--help` about** for `decide` gains "; accept a proposed
   one" (the CLI reference's help blocks are regenerated from it).
9. **AGENTS.md rules templates not touched**: the logbook's rules block
   is versioned (rules-v3, WP-111) and changing it means an upgrade
   path; the agent guide (docs) says it, and the engine enforces it.

## How it was verified

- `cargo fmt` clean; `cargo clippy --all-targets -- -D warnings` clean.
- `cargo test --test decide_accept`: 5 passed; `cli`, `commands`,
  `status`, `contract_v2` suites pass.
- Mutants, engine (`mutants.py`, `CARGO_TARGET_DIR=engine/target/mutants`):
  13/13 killed (agent actor, session, refusal after open, superseded,
  re-accept, date, no ledger line, actor, another id, DECISIONS.md, no
  commit, no index rebuild, file not written).
- Mutants, plugin (`plugin-mutants.py` on desk-view trimmed to 10a and
  10c' by `make-trim.sh`, and model.test.js): 11/11 killed (no arming,
  selection keeps it armed, enabled without a writer, answer in
  decideResult, an accepted decision opens, answer on every decision,
  no `--json`, any id, `already` ignored, label, a key keeps it armed).
  The first run had P2 surviving (a `key:Down` disarms through the
  desk's any-key rule); the case now changes the selection by a click.
- `omarchy plugin validate plugin/` ok; `just qmllint` ok (46 files).
- `node tests/plugin/model.test.js`: 145 passed.
- `bash tests/plugin/desk-view.sh`: 1512 passed (in the final check).
- `bash tests/plugin/service-states.sh`: 328 passed.
- `bash scripts/docs-check.sh`: ok.
- `git diff d95b7da3..HEAD | grep /home/`: nothing.
- Not run: a live check on the test host (the Accept click in a real
  desk); worth one look when `next` goes there.

## Final check

`flock /tmp/seldon-check.lock just check` on 7c7ad534: **exit 0**
(`check: ok`; engine tests ok, check-srcinfo ok, check-packaging ok,
install.test 209, deploy-test-host 190, docs-check ok, qmllint ok 46
files, model.test.js 145, real-home-guard 11, service-states 328,
desk-view 1512, bar-view 194). Only this handover changed after that
commit.

## Open questions

- **ADR-0040 is proposed**: the orchestrator/operator accepts it (or
  asks for a dedicated event kind in a later contract bump).
- Superseding (`supersedes:`) stays a hand edit; a `decide supersede`
  would follow the same pattern.

## Security (for stage 2)

- New write path, the user's only: the refusal is by resolved actor and
  session before any read; tests cover `--actor agent:…`,
  `SELDON_ACTOR=agent:…`, `--actor human` in an agent session and
  `--actor system`.
- Plugin: fixed argv `["decide", "accept", <id>, "--json"]`, the id
  checked against `^ADR-[0-9]{4}$` in `acceptArgs` and again in
  `validateArgs` (no other shape admitted: no `--actor`, no extra
  argument, no `--`); arm-twice. All shown text is `Text.PlainText`; the
  engine's answer is shown, never evaluated.
- The ledger detail carries the decision title through the ledger's
  redaction (as every detail); the frontmatter write changes two keys and
  is read back before the ledger is written.

## Round 2

Review 1 (stage 1): SEND BACK for B1, with the orchestrator's decisions on
N1–N5. Commits cfc27b58 (engine), c9b77f2e (tests, docs). No merge of
`next` (it moved only in `work/queued/`).

- **B1 — the write order is tested.** `a_ledger_failure_accepts_nothing`
  (as `tests/plan.rs` `a_ledger_failure_transitions_nothing`): an invalid
  `[redaction] patterns` → exit 1, `tree()` unchanged including the
  decision file, HEAD unchanged; a read-only `ledger/` with a new month →
  exit 2, nothing written (skipped where permissions do not bind, as
  root in CI); the decision stays proposed. The swap mutant (file before
  ledger, M14) is killed.
- **N1 — broken YAML.** `refuses_a_decision_that_is_not_proposed` adds a
  frontmatter that does not read (`id: [ADR-0001`): exit 1 "invalid
  frontmatter", nothing written. M15 (the load error as an engine error)
  killed.
- **N2 — documented.** SPEC-ENGINE §3 and ADR-0040 §1: the ledger first;
  a ledger that cannot be written leaves the decision proposed; a file
  write that fails after the ledger line leaves the `accepted:` note with
  the decision still proposed, and a re-run adds a second note (the plan
  step's pattern; the file stays the truth).
- **N3 — an unreadable `SELDON_ACTOR` refuses.** New
  `event::session_actor_for_user_act`: a `SELDON_ACTOR` that is set but
  does not read (not UTF-8, not `human`/`agent:<name>`, `system`) is
  exit 1 whatever `--actor` says, because the session may be an agent's.
  Used by `decide accept` and by `triage.rs` `user_actor` (`drift
  apply|discard`; it read the session with `if let Ok(Some(..))`). Tests:
  three cases in `an_agent_never_accepts` (`--actor human` with
  `agent:Not Valid`, with `system`; no `--actor` with `nobody`), two in
  triage's `apply_and_discard_are_the_user_s_and_check_the_file`. M16,
  M17 killed. SPEC-ENGINE §3 (both commands, §5's apply paragraph) and
  ADR-0040 §4 say so.
- **N4 — a new index disarms.** desk-view `decisions-accept-index`:
  Accept armed, then a capture from the pill's right click (no key, no
  click in the desk) makes the fake write an index with ADR-0005; Accept
  is disarmed, the selection stays, only capture and status ran. P12
  (no `onAllRowsChanged` disarm) killed.
- **N5 — the fake refuses as the engine does.** `decisions-accept-refused`
  uses the engine's own agent-session text; the fake's bad-id message is
  clap's (`… is not a decision id (ADR-NNNN)`); its superseded and
  unknown texts already were the engine's.
- **N6 (not in the orchestrator's list) — not changed.** An inline
  comment on the `status:` line is dropped by `Frontmatter::set`, the
  shared lossless update every plan step uses too; a fix belongs there.

Open for the orchestrator: the same lenient session read
(`env_actor(parse_person).ok().flatten()` / `if let Ok(Some(..))`)
remains in `plan.rs` (l.44 an imported case's start, l.474 `plan done`
with `--actor human`), `drift.rs` l.491 (`link|explain|dismiss` with
`--actor human`) and `import/task.rs` l.205 (`--include-done`). N3's
helper fits each one; not changed here (outside this WP's scope).

Verified: `cargo fmt`, clippy (default and `--features watch`) clean;
`decide_accept` 6 passed, `triage` passes; engine mutants M1–M17 all
killed; plugin mutants P1–P12 all killed (trimmed desk-view 85/85);
`omarchy plugin validate plugin/` ok; docs-check ok.
`flock /tmp/seldon-check.lock just check` on c9b77f2e: **exit 0**
(`check: ok`; check-srcinfo ok, check-packaging ok, install.test 209,
deploy-test-host 190, docs-check ok, qmllint ok 46 files, model.test.js
145, real-home-guard 11, service-states 328, desk-view 1517, bar-view
194). Only this handover changed after that commit.

## Round 3

Fable stage 2 approved ba486b68 and advised accepting ADR-0040 after
one wording edit; the orchestrator asked for that and two small changes.
Commit e9c235e8. ADR-0040 stays **proposed** until the operator accepts
it.

1. **ADR-0040 §4** gains "What this stops, and what it does not" in the
   orchestrator's words: it stops an agent in the session Seldon
   launched (which carries `SELDON_ACTOR`), not a process of the same
   user that unsets or overrides the variable, nor a hand edit; `decide
   accept` adds a ledger line, not power; the desk's Accept runs from
   the shell process.
2. **An ambiguous id is refused.** `Logbook::decision_files_of` returns
   every file of an id (`decision_file`, which `seldon open` uses, is its
   first, unchanged). `decide accept` with two or more → exit 1 "ADR-NNNN
   is ambiguous: <file> and <file> carry it; keep one of them, then
   accept it again", nothing written. ADR-0040 §3 and SPEC-ENGINE §3 say
   so; `refuses_a_decision_that_is_not_proposed` tests it; mutant M18
   (take the first file) killed.
3. **The message.** `session_actor_for_user_act` reads the variable
   itself: "<what> is not done: SELDON_ACTOR (the session's actor): <why>.
   …: fix or unset SELDON_ACTOR" — no "(the actor when none is named)"
   when `--actor` is given; the test asserts the text and its absence.

Verified: fmt and clippy clean; `decide_accept` 6, `triage` 26,
`commands` 11 passed; engine mutants M1–M18 all killed; docs-check ok.
`flock /tmp/seldon-check.lock just check` on e9c235e8: **exit 0**
(`check: ok`; install.test 209, deploy-test-host 190, docs-check ok,
qmllint ok 46 files, model.test.js 145, real-home-guard 11,
service-states 328, desk-view 1517, bar-view 194). Only this handover
changed after that commit.

## Merge of next

ADR-0040 accepted by the operator 2026-10-08 (E18): status line and
DECISIONS.md row in 040ce77d. `next` (35b1b3de: WP-143, WP-154, WP-102a,
WP-124, WP-140 and others) merged in 04088da1. Conflicts, both sides kept:

- `DECISIONS.md`: ADR-0039 (next) and ADR-0040 in number order.
- `CHANGELOG.md`: next's Engine entries (WP-154, WP-140, WP-129, WP-113)
  after this WP's `decide accept` entry; next moved WP-127's plugin entry
  into its own Plugin list, and *Accept accepts* now stands beside it
  there.
- `plugin/Service.qml`: next's `askResult`, `triageResult` and snapshot
  keys, plus `acceptResult`.
- `docs/user/de/02-concepts.md`: both bodies merged on their own; the
  source line now names the merge (92d13cde), where the English page has
  both changes.

`triage.rs` (N3's `session_actor_for_user_act`) and the harness files
merged without conflict.

Verified: `flock /tmp/seldon-check.lock just check` on 92d13cde with
`XDG_RUNTIME_DIR` a private `mkdir -m 700` directory and `JUST_TEMPDIR`
in the scratch directory: **exit 0** (`check: ok`; install.test 229,
deploy-test-host 190, docs-check ok, qmllint ok 47 files, model.test.js
160, real-home-guard 11, service-states 342, desk-view 1595, bar-view
194). The runtime directory was removed afterwards. Only this handover
changed after that commit.

Note: the harness runs of rounds 1 and 2 (a full desk-view and
service-states run, the trimmed desk-view runs of the plugin mutants)
ran with the session's `XDG_RUNTIME_DIR`, before the orchestrator's
rule; their instance folders may be under the real runtime directory.
Nothing there was touched.
