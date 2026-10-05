# ADR-0023 — An agent moves a case to verification; the human closes it. Risk levels are a normative scale, enforced only by advice

**Status:** accepted; §1 superseded in part by ADR-0027
**Date:** 2026-10-02

> Superseded in part by [ADR-0027](ADR-0027-act-then-account.md) (2026-10-05): a case the user started authorises the agent to act; the agent verifies and closes it. Only steps that can make the machine unbootable (R3) still need the user's explicit go.

## Context
The agent-facing texts (WP-047: `llms.txt`, `docs/AGENT-GUIDE.md`, the
generated logbook `AGENTS.md`) must say who may run `seldon plan done`.
The earlier template let an agent close a case after its own check. The
Phase 0 exit run (work/completed/PHASE-0-EXIT-transcript.md) showed the
better shape: Claude Code planned and ran the case and moved it to
`verification`; the human closed it. The case state machine
(SPEC-LOGBOOK §3) has the `verification` state for exactly this gate.

The same texts need a meaning for `risk: R0..R3`, which the specs listed
but never defined; the engine records risk and never blocks on it
(PROJECT.md: Seldon records, it does not enforce).

## Decision
1. **Verification is the human gate.** An agent working a case runs
   `seldon plan verify` when its own checks pass and stops there. `seldon
   plan done` is the human's call, in the panel or on the command line,
   unless the human pre-authorised closing in the case's Plan ("close
   when verified"). The generated `AGENTS.md`, the agent guide and
   `llms.txt` state this rule; the engine does not enforce it (an agent
   can run `plan done`; the ledger shows who did).
2. **Risk scale** (normative text in SPEC-LOGBOOK §3):
   - `R0` — reversible in seconds, nothing depends on it; undo by hand,
     no snapshot.
   - `R1` — reversible by hand in minutes with a known command; the Plan
     names the rollback step.
   - `R2` — rollback needs the plan and a snapshot or backup; `plan start
     --snapshot`, verify before closing.
   - `R3` — can break boot, login or the shell; snapshot mandatory, the
     human's explicit go per step, never unattended.
   The engine stays advisory: a later package may warn on `plan start`
   for R2/R3 without `--snapshot` (never refuse). No schema change — the
   `risk` enum is unchanged, so `contractVersion` stays 1.

## Consequences
Agents and humans share one reading of "done" and of risk. A case closed
by an agent is visible as such in the ledger (`actor`) and can be
questioned. The default `[drift] alwaysRed` list should match the R3
subjects when it is next touched.
