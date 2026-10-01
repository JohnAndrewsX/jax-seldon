---
id: C-2026-002
type: case
title: "Hyprland-Monitorlayout für Dual-WQHD"
status: completed
zone: yellow
risk: R1
priority: normal
area: hyprland
created: 2026-09-12
started: 2026-09-12
closed: 2026-09-13
snapshotBefore: 108
agents: [agent:claude-code]
events: [01M2A9MNTG5XQ4EPAYSBBYJ92N, 01M2ACN5Q043TW9W44NJEZ4K9S, 01M2ADMJK8H2NCRVDF2RA9WA56]
tags: []
---
# C-2026-002 — Hyprland-Monitorlayout für Dual-WQHD

## Intent
Zweiter WQHD-Monitor: sauberes Layout statt Auto-Erkennung.

## Plan
- Goal: DP-1 links, DP-2 rechts, beide 144 Hz, gleiche Skalierung.
- Steps:
  - [x] Snapshot vorher (`snapper create -t pre`)
  - [x] `monitors.conf` neu schreiben
  - [x] Neustart und prüfen
- Affected paths:
  - `~/.config/hypr/monitors.conf`
- Rollback: Snapshot 108 oder alte `monitors.conf` aus Git.
- Verification: Nach Neustart beide Monitore korrekt, Workspaces bleiben am Platz.

## Log
<!-- append-only; engine and agents add dated lines -->
- 2026-09-12 09:05 · created (zone yellow, risk R1) · human
- 2026-09-12 09:31 · started (snapshot 108) · human
- 2026-09-12 10:02 · linked drift `~/.config/hypr/monitors.conf` · human
- 2026-09-13 10:58 · verification · human
- 2026-09-13 11:00 · completed · human

## Result
Layout steht, Skalierung 1.25. Abweichung in `system/deviations.md` eingetragen.
