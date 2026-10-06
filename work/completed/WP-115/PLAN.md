# WP-115 — plan

Normative: ADR-0029 §1–§5. No contract change.

## Engine

1. **Rule 9 pass** (`engine/src/reconcile.rs`, new `link_planned`):
   - windows from the ledger's `seldon` case events (`case-started` …
     next `case-completed`/`case-dropped`, both ends inclusive); never
     `.seldon/active-case`, never the case file's status or dates;
   - Plan text from the case files (`cases::section(body, "Plan")`, the
     section the index reads), token test `index::drift::names_token`
     (shared with rule 3);
   - candidates: drift-eligible events with no case and no resolution line,
     any actor; a pacman event with a `txId` is tested only when it is
     explicit (its non-explicit members follow it); exactly one case whose
     window holds `ts` and whose Plan names the subject → link; two or more
     → no link, one Log line per candidate naming the others; harm guard:
     an `alwaysRed` subject links only to an R3 case, else the R3 advisory
     Log line;
   - writes: one `resolution: linked` line per member (`source: seldon`,
     actor `system`, `case`, `refersTo`, detail `planned by C; active at
     the time`, `ts = max(capture time, event ts)`, `meta.txId` when the
     fan-out has two or more lines), one append under the capture's lock,
     after rules 7 and 8, reading the whole ledger once (shared with rule
     8); then `attach()` and the Log line `linked after the fact: <source>
     <kind> <subject> at HH:MM:SS (planned here[, no capture ran before the
     close])` by `system`; Log lines are written once (dedupe by text).
   - `seldon capture --json` gains `linkedPlanned`.
2. **`plan verify|done` capture first** (`commands/plan.rs`): a default
   capture (its own lock, released) before the step's lock; failure is a
   warning in the step's output; `--no-capture` skips it; `plan drop` does
   not capture. The step's `--json` gains `capture` (null when skipped).
   The capture waits for a held lock as a hook does (8 s).
3. **Re-resolvable engine resolutions** (`index/build.rs`, `reconcile.rs`):
   an eligible event whose latest folded resolution is by actor `system`
   (rules 7, 8, 9) is linkable for `drift link|explain|dismiss` (a later
   line wins, ADR-0012 §8), classified like any item but never open drift
   or listed; agent refusals for crises unchanged.

## Tests (temp HOME, `TZ` set in helpers)

Acceptance 1–9 of ADR-0029 §5 as integration tests (`engine/tests/
planned_link.rs`) plus unit tests of the window/uniqueness logic in
`reconcile.rs`; idempotency (second capture writes nothing).

## Fixtures, validator, docs

- `fixtures/logbook/`: one engine-linked line (a completed case whose
  Plan names a package installed by `human` in its window), the case's
  `events:` and Log line; `index.sample.json` and variants regenerated.
- `scripts/validate-fixtures.py`: port rule 9 (the fixture is a
  post-capture state: every engine `planned by` line is what rule 9
  writes, and nothing rule 9 would link is left).
- SPEC-ENGINE §3 (`plan verify|done`, `capture` JSON), §5 rule 9, §10;
  schema description of `resolution`; AGENT-GUIDE; the skill sentence
  (one sentence, `engine/assets/skills/seldon/`); CHANGELOG.

## Not mine

Item 10 (live check, capture cost into SPEC §3) — orchestrator after the
merge. WP-111 owns the bigger skill edits, rules v3 and the session-start
context.
