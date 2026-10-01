---
id: C-2026-006
type: case
title: "Snapper-Retention auf 14 Tage"
status: queued
zone: red
risk: R2
priority: low
area: packages
created: 2026-09-30
started:
closed:
snapshotBefore:
agents: []
events: []
tags: []
---
# C-2026-006 — Snapper-Retention auf 14 Tage

## Intent
Snapshots sollen nicht unbegrenzt wachsen; 14 Tage reichen.

## Plan
- Goal: Number-Cleanup behält 14 Tage, Timeline bleibt aus.
- Steps:
  - [ ] `/etc/snapper/configs/root` lesen und sichern
  - [ ] `NUMBER_LIMIT` und `NUMBER_MIN_AGE` anpassen (sudo, Red Zone)
  - [ ] Eine Woche beobachten
- Affected paths:
  - `/etc/snapper/configs/root`
- Rollback: Gesicherte Konfiguration zurückspielen.
- Verification: `snapper list` zeigt nach einer Woche nur noch 14 Tage.

## Log
<!-- append-only; engine and agents add dated lines -->
- 2026-09-30 10:00 · created (zone red, risk R2) · human

## Result

