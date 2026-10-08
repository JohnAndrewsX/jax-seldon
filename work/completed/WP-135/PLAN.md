# WP-135 — Plan: `seldon decide accept`

Branch `wp/135-decide-accept` from `next` (d95b7da3: contract v2, the
desk with WP-123's Decisions section, WP-127's `decisions[].lead`);
merge into `next`.

## Inputs read

AGENTS.md §3, §7, §8; ADR-0034 §2 (Decisions: "Accept when proposed";
writing actions arm twice); ADR-0035 (§6's rule for additions within
contract 2); ADR-0038; CONTRACT.md (rules, command table);
`engine/src/commands/decide.rs`; SPEC-LOGBOOK §3 (decision frontmatter);
SPEC-ENGINE §3 (`decide`: "writes no ledger event (no fitting kind)");
WP-123's handover (Decision 2: Accept opens the editor); the B2 pattern:
`plan.rs` `step()` / `refuse_agent_start_of_imported`,
`import/task.rs` `--include-done`, `triage.rs` `user_actor`.

## Engine

`seldon decide accept <ADR-NNNN> [--actor A] [--json]` — a subcommand of
`decide`; the title form `seldon decide [--case ID] [--no-edit] <title>`
is unchanged (`args_conflicts_with_subcommands`: a bare first word
`accept` is the subcommand; `seldon decide -- accept` still makes a
decision titled "accept", and the plugin always passes the title after
`--`).

Order (as `plan` steps: refuse before reading, ledger before the file):

1. **The user's act (B2).** Before anything is read: an agent `--actor`,
   an agent `$SELDON_ACTOR` without `--actor`, and `--actor human` in an
   agent's session are refused (exit 1), each naming the conflict and the
   way out (the desk's Accept, or `seldon decide accept` in the user's
   own terminal). `system` is refused by the person parser.
2. Open the logbook, take the lock, find `decisions/ADR-NNNN-*.md` (the
   lookup of `seldon open`, shared), load it.
3. `accepted` already → exit 0, `already: true`, nothing written, no
   event, no commit (idempotent). `superseded` → exit 1 "is superseded;
   only a proposed decision is accepted". An unknown id → exit 1. A
   decision file whose frontmatter does not read → exit 1 naming it.
4. `status: accepted`, `date` = today (`model::update`: only the two
   changed keys are touched, the rest of the file byte for byte).
   The read-back check runs before the ledger is written.
5. Ledger: one `seldon` `note`, subject the ADR id, detail `accepted:
   <title>`, actor `human` (redacted by the ledger as every detail).
6. Write the file atomically; fill `DECISIONS.md`'s `decisions.index`
   (as `decide`); autocommit `seldon: ADR-NNNN accepted`; rebuild the
   index.

`--json`: `{"decision": {id, title, status, date, cases, path},
"already", "event" (null when already), "git", "warnings"}`.

## Contract

The index's shape does not change (a `note` with source `seldon` and a
free subject is valid under contract 2 as it is). The new argv row is
not covered by ADR-0035 §6 (that rule allows optional fields and
proposal refinements, not commands), so **ADR-0040 (proposed)** records:
the plugin may run `seldon decide accept <ADR-NNNN> --json`; it is the
user's act (agents refused); the ledger record is a `seldon` note. No
`contractVersion` bump, no fixture change needed for validity; the
fake engine follows.

## Plugin

- `Model.acceptArgs(id)` → `["decide", "accept", id, "--json"]` (id
  against `DECISION_ID`); `validateArgs` admits exactly that form.
- `Model.acceptResult(exit, out, err)` → `{ok, text, decisionId,
  already}`: "Accepted ADR-0004 · <title>" / "ADR-0004 was already
  accepted" / the engine's error.
- `Service.acceptDecision(id)`, `acceptResult`; one call at a time.
- Decisions: Accept arms (Arm.qml, `decision:<id>:accept`), the bar
  reads "Confirm accept" with the hint "Accept ADR-0004? Click Confirm.
  It sets status accepted and today's date.", the second click runs.
  A new selection, another section, a new index or any key disarms. The
  detail's note says what Accept does. The result line shows the
  engine's answer.
- Harness: desk-view `decisions-live` arms and confirms Accept, the argv
  holds `decide accept ADR-0004 --json` and no `open`; dev mode refuses
  with dev mode's reason; a refusal from the fake engine shows in place.
  Fake engine: `decide accept` flips the state index's decision.

## Docs

SPEC-ENGINE §3 (the command, its JSON, its refusals; "decide writes no
ledger event" now only for a new decision), SPEC-LOGBOOK §3 (who changes
the status), SPEC-PLUGIN (Decisions' Accept), CONTRACT.md row,
guides en/de (02-concepts, 05-cli-reference), CHANGELOG, TESTING /
COVERAGE where the decisions cases are listed.

## Tests

Engine (`engine/tests/decide_accept.rs`): accept (frontmatter, date,
rest of the file unchanged, ledger note, commit, index, DECISIONS.md);
idempotent (second run: exit 0, `already`, ledger and git unchanged);
refuse superseded; refuse unknown; refuse agent `--actor`, agent
session, `--actor human` in an agent session (nothing changed in each);
`accept` as a title after `--` still creates a decision. Unit test for
the refusal function. Mutants on a separate target.

## Decisions (what the WP leaves open)

See HANDOVER.md §Decisions (filled as the work goes).
