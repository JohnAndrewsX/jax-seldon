# ADR-0008 — Drift is a first-class object

**Status:** accepted; the meaning of "Crisis" superseded in part by ADR-0028 · **Date:** 2026-10-01

> Superseded in part by [ADR-0028](ADR-0028-attention-by-consequence.md) (2026-10-06): a crisis is a change without a case that can break boot, login, the shell or security, or lose data (the harm test), no longer every red-zone change.

## Context
Seldon's distinguishing feature is reconciling captured events against
planned work. Events that match no case are "drift". Without a durable
representation, drift would be recomputed and re-shown forever.

## Decision
- Drift is computed by the engine (`seldon drift`) and stored in the index
  with the event's own `id` as the drift key.
- Resolution is recorded by appending a `resolution` event to the ledger
  (`refersTo: <event id>`, `resolution: linked | explained | dismissed`,
  `case`, `actor`). The original line is never edited.
- High-risk drift (red zone) is labelled **Crisis** in the UI.

## Consequences
Drift state survives index rebuilds; the ledger stays append-only; the UI
has one list to show and three actions to offer.
