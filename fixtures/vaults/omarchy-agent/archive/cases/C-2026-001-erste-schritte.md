---
id: C-2026-001
type: case
title: "Erste Schritte auf der neuen Maschine"
status: done
zone: green
risk: R0
priority: high
created: 2026-08-14
closed: 2026-08-15
tags: [setup, dossier]
---

# C-2026-001 — Erste Schritte auf der neuen Maschine

## Auftrag
Die Maschine einmal vollständig erfassen, bevor irgendetwas geändert wird.

## Plan
- **Ziel:** Ein Dossier mit Hardware, Paketen und Diensten.
- **Schritte:**
  - [x] Hardware lesen
  - [x] Dienste lesen
- **Betroffene Pfade:** keine (nur lesend)
- **Rollback-Rezept:** entfällt
- **Verifikationsplan:** Dossier gegen das laufende System prüfen.

## Befund
Die Maschine läuft auf dem Standard ohne eigene Änderungen.

### Einzelheiten
Ein Beispiel für einen Befehl im Codeblock:

```
## keine Überschrift, nur Ausgabe
uptime
```

## Protokoll
- 2026-08-14 · angelegt (G1: zone green, risk R0, priority high)
- 2026-08-15 · Dossier geschrieben → G3, status done

## Ergebnis
Das Dossier steht unter `system/`. Siehe [[C-2026-005-hostname-prompt]].
