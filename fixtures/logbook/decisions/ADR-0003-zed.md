---
id: ADR-0003
type: decision
title: "Zed statt VS Code als Zweiteditor"
status: accepted
date: 2026-10-01
supersedes:
cases: [C-2026-004, C-2026-005]
---
# ADR-0003 — Zed statt VS Code als Zweiteditor

## Context
Für große Refactorings ist Neovim allein mühsam; VS Code ist schwer und telemetrielastig.

## Decision
Zed wird Zweiteditor, Neovim bleibt Standard.

## Consequences
SUPER+E startet Zed; das Theme muss mit Omarchy synchron laufen ([[C-2026-005]]).
