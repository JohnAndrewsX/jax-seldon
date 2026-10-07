# WP-120 — Plan

Branch `wp/120-contract-v2` from `next` (94e1fab); merges into `next`.

## Steps (one commit each, roughly)

1. **ADR-0035 (proposed)** + DECISIONS.md row. Commit before any code.
2. **Schema**: `index.schema.json` (`contractVersion` 2, `logbook.git.
   autocommit`, drift `truncated`, `decisions[].cases`, `triage`),
   `event.schema.json` (kinds `case-updated`, `state-loss`; `meta.risk`,
   `meta.truncated`; the kind rules), new `proposal.schema.json`;
   `fixtures/invalid/index.contract-v3.json` (+ must-fail cases for the
   new rules).
3. **Engine model**: `Kind::CaseUpdated`, `Kind::StateLoss` (engine-only),
   `CONTRACT_VERSION` 2; `seldon event --meta truncated` refused.
4. **Risk record**: `meta.risk` on `case-created`/`case-started` (plan
   new/start/reopen, `agent start --new`, drift explain's retroactive
   case), `case-updated` from `plan set`; the harm guard reads the ledger
   timeline, falls back to the Log (`reconcile::PlanningCase`).
5. **State loss**: `capture` writes `state-loss`; every reader of the old
   note accepts both (capture dedup, crash marks, doctor).
6. **Index**: `meta.truncated` / drift `truncated` in `clip`; decision
   `cases`; `autocommit.json` written by `autocommit*`, read into
   `logbook.git.autocommit`; `triage` from `proposals/` with the schema
   check; `index --check` knows `proposal.schema.json`.
7. **Fixtures + reference derive**: sample logbook (C-2026-003 raise as a
   `case-updated` line, `meta.risk` on late case lines, a `state-loss`
   line, a long detail, ADR with two cases), `fixtures/proposals/<id>.json`,
   `scripts/validate-fixtures.py` (derive, case-log check for
   `case-updated`, proposal check, schema mapping), `--write-index`,
   variants regenerated; golden test writes the proposal into its state.
8. **Plugin**: `CONTRACT_VERSION` 2 (Model.js, manifest); `model.test.js`
   (v2 fixture parses, new fields/kinds tolerated); `service-states.sh`
   (v3 index → plugin older; v1 index → engine older; the v0.1.3 plugin
   against the v2 sample → mismatch with both numbers); harness
   expectations that move with the fixture.
9. **Docs**: CONTRACT.md, SPEC-ENGINE §3/§5/§6, SPEC-LOGBOOK kinds,
   CHANGELOG **Breaking**, fixtures/README.md, TESTING.md rows.
10. `flock /tmp/seldon-check.lock just check`; HANDOVER.md.

## Tests to add (engine)

- `case-created`/`case-started` carry `meta.risk`; `plan set` writes one
  `case-updated` with the new risk, nothing when unchanged; `seldon event
  --kind case-updated|state-loss` refused.
- an old ledger without `meta.risk` indexes and `--check`s valid; the
  guard falls back to the Log for it; the guard reads the ledger for a
  v2 case (Log edited by hand does not change the answer; frontmatter
  edited → tells nothing).
- `meta.truncated` exactly on clipped events; drift `truncated`; a
  ledger `meta.truncated` dropped.
- state loss writes `state-loss`; an old `note state-reset` still counts
  as recorded (no second line).
- autocommit ok / failed / skipped → index field present / absent; other
  logbook's record ignored; redaction.
- triage: none → absent; newest valid wins; invalid / wrong name / other
  logbook skipped with a warning; `applied`.
- decision `cases`; idempotency of `index` (two runs, same bytes).
