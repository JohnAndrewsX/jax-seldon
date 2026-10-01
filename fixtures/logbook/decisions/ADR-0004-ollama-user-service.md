---
id: ADR-0004
type: decision
title: "Ollama nur als User-Service mit Case"
status: proposed
date: 2026-10-01
supersedes:
cases: []
---
# ADR-0004 — Ollama nur als User-Service mit Case

## Context
Codex hat ollama samt systemd-User-Service ohne Case installiert (Krise im Ledger).

## Decision
Lokale Modelle nur über einen Case; ollama läuft, wenn überhaupt, als User-Service ohne Autostart.

## Consequences
Entweder nachträglich einen Case anlegen und verknüpfen oder ollama entfernen.
