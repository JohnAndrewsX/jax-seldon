---
id: ADR-0001
type: decision
title: "Logbuch-Sprache Deutsch, Struktur Englisch"
status: accepted
date: 2026-09-01
supersedes:
cases: [C-2026-001]
---
# ADR-0001 — Logbuch-Sprache Deutsch, Struktur Englisch

## Context
Ich schreibe schneller auf Deutsch; Werkzeuge und Agenten erwarten englische Schlüssel.

## Decision
Prosa auf Deutsch. Dateinamen, YAML-Schlüssel und Enum-Werte auf Englisch.

## Consequences
Obsidian-Suche funktioniert in beiden Sprachen; Seldon muss nichts übersetzen.
