---
id: C-2026-009
type: case
title: "Reopen: Hyprland-Monitorlayout für Dual-WQHD"
status: verification
zone: yellow
risk: R1
priority: normal
area: hyprland
created: 2026-10-01
started: 2026-10-01
closed:
snapshotBefore:
agents: [agent:claude-code]
events: []
tags: [reopens:C-2026-002]
---
# C-2026-009 — Reopen: Hyprland-Monitorlayout für Dual-WQHD

## Intent
Zweiter WQHD-Monitor: sauberes Layout statt Auto-Erkennung.

## Plan
- Goal: Skalierung auf beiden Monitoren wieder 1.25.
- Steps:
  - [x] `monitors.conf` geprüft: Skalierung steht auf 1.25
  - [ ] Neustart und prüfen
- Affected paths:
  - `~/.config/hypr/monitors.conf`
- Rollback: nichts geändert.
- Verification: nach einem Neustart beide Monitore mit 1.25, Workspaces am Platz.

## Log
<!-- append-only; engine and agents add dated lines -->
- 2026-10-01 17:02 · created (zone yellow, risk R1): reopens C-2026-002 · human
- 2026-10-01 17:02 · started · human
- 2026-10-01 17:04 · verification · agent:claude-code

## Result
