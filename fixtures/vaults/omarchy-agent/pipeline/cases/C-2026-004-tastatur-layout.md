---
id: C-2026-004
type: case
title: "Tastaturbelegung für Sonderzeichen korrigieren"
status: verification
zone: yellow
risk: R1
priority: high
created: 2026-09-03
closed:
tags: [keyboard, input]
---

# C-2026-004 — Tastaturbelegung für Sonderzeichen korrigieren

## Auftrag
Einige Sonderzeichen erscheinen nicht auf den erwarteten Tasten.

## Plan
- **Ziel:** Alle Sonderzeichen liegen wie beschriftet.
- **Betroffene Pfade:** `/home/user/.config/hypr/input.lua`
- **Verifikationsplan:** Prüfskript tippt jede Taste einmal.

## Protokoll
- 2026-09-03 · angelegt (G1: zone yellow, risk R1, priority high)
- 2026-09-03 · Prüfskript gegen den Testdienst mit token=abc123geheim aufgerufen
- 2026-09-03 · Änderung geschrieben, Verifikation läuft

## Ergebnis
