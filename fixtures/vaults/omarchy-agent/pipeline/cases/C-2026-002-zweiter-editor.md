---
id: C-2026-002
type: case
title: "Einen zweiten Editor einrichten"
status: planned
zone: red
risk: R2
priority: normal
created: 2026-08-20
closed:
tags: [editor]
---

# C-2026-002 — Einen zweiten Editor einrichten

## Auftrag
Neben dem Standard-Editor einen zweiten für große Projekte bereitstellen.

## Plan
- **Ziel:** Zweiter Editor startet aus dem Menü.
- **Schritte:**
  - [ ] Editor auswählen
  - [ ] Installieren
  - [ ] Tastenkürzel setzen
- **Betroffene Pfade:** `~/.config/hypr/bindings.lua`
- **Rollback-Rezept:** Paket entfernen, Binding zurücknehmen.
- **Verifikationsplan:** Menüeintrag und Kürzel öffnen den Editor.

## Protokoll
- 2026-08-20 · angelegt (G1: zone red, risk R2, priority normal)

## Ergebnis
