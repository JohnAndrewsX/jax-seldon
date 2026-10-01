---
id: C-2026-008
type: case
title: "Tailscale für Fernzugriff einrichten"
status: verification
zone: red
risk: R2
priority: normal
area: dev-env
created: 2026-10-01
started: 2026-10-01
closed:
snapshotBefore: 112
agents: []
events: [01M3VDBX30F5DH0JNY7S0K95GC]
tags: []
---
# C-2026-008 — Tailscale für Fernzugriff einrichten

## Intent
Vom Laptop aus auf die Workstation zugreifen, ohne Ports zu öffnen.

## Plan
- Goal: Tailscale läuft als Dienst, Workstation im Tailnet erreichbar.
- Steps:
  - [x] `omarchy pkg add tailscale`
  - [x] `tailscaled` aktivieren
  - [x] `tailscale up`
  - [ ] Zugriff vom Laptop prüfen
- Affected paths:
  - `/etc/systemd/system/multi-user.target.wants/tailscaled.service`
- Rollback: `tailscale down`, Dienst deaktivieren, Paket entfernen.
- Verification: SSH vom Laptop über den Tailnet-Namen klappt.

## Log
<!-- append-only; engine and agents add dated lines -->
- 2026-10-01 11:20 · created (zone red, risk R2) · human
- 2026-10-01 11:21 · started (snapshot 112) · human
- 2026-10-01 11:44 · linked drift `tailscale` · human
- 2026-10-01 12:10 · verification · human

## Result

