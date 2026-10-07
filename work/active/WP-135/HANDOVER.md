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
