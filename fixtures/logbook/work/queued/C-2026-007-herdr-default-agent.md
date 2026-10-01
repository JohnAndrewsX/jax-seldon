---
id: C-2026-007
type: case
title: "Herdr-Orchestrator als Default-Agent registrieren"
status: queued
zone: yellow
risk: R1
priority: high
area: dev-env
created: 2026-10-01
started:
closed:
snapshotBefore:
agents: []
events: []
tags: []
---
# C-2026-007 — Herdr-Orchestrator als Default-Agent registrieren

## Intent
Agenten sollen über Herdr starten, damit Sitzungen sichtbar bleiben.

## Plan
- Goal: „Start agent“ im Seldon-Panel öffnet eine Herdr-Sitzung mit Case-Kontext.
- Steps:
  - [ ] Herdr-Konfiguration lesen
  - [ ] Launcher-Befehl in `config.toml` eintragen
  - [ ] Session-Start-Hook testen
  - [ ] Panel-Button testen
  - [ ] Doku in `areas/dev-env`
- Affected paths:
  - `~/.config/seldon/config.toml`
  - `~/.config/herdr/`
- Rollback: Launcher-Eintrag entfernen.
- Verification: Klick auf „Start agent“ startet Herdr mit dem Case.

## Log
<!-- append-only; engine and agents add dated lines -->
- 2026-10-01 11:00 · created (zone yellow, risk R1) · human

## Result

