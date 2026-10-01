---
id: ADR-0002
type: decision
title: "Snapshots vor jedem Red-Zone-Eingriff"
status: accepted
date: 2026-09-02
supersedes:
cases: []
---
# ADR-0002 — Snapshots vor jedem Red-Zone-Eingriff

## Context
Ein kaputtes Update ohne Snapshot kostet einen Abend.

## Decision
Vor jedem Case in der Red Zone gibt es einen Snapper-Snapshot; die Nummer steht als `snapshotBefore` im Case.

## Consequences
Mehr Snapshots; Aufräumen regelt [[C-2026-006]].
