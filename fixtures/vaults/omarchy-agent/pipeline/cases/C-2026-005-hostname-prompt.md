---
id: C-2026-005
type: case
title: "Rechnername im Prompt bei entfernten Sitzungen"
status: done
zone: yellow
risk: R1
priority: normal
created: 2026-08-20
closed: 2026-08-20
tags: [prompt, terminal]
---

# C-2026-005 — Rechnername im Prompt bei entfernten Sitzungen

## Auftrag
In entfernten Sitzungen soll sichtbar sein, auf welchem Rechner man arbeitet.

## Plan
- **Ziel:** Der Rechnername steht vor dem Pfad, nur in entfernten Sitzungen.
- **Betroffene Pfade:** `~/.config/starship.toml`

## Protokoll
- 2026-08-20 · angelegt (G1: zone yellow, risk R1, priority normal)
- 2026-08-20 · geschrieben und geprüft → G3, status done

## Ergebnis
Der Prompt zeigt den Rechnernamen in entfernten Sitzungen.
