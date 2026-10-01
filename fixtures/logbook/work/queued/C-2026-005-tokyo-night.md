---
id: C-2026-005
type: case
title: "Theme-Wechsel auf Tokyo Night durchziehen (Zed, Neovim)"
status: queued
zone: yellow
risk: R1
priority: normal
area: themes
created: 2026-09-29
started:
closed:
snapshotBefore:
agents: []
events: []
tags: []
---
# C-2026-005 — Theme-Wechsel auf Tokyo Night durchziehen (Zed, Neovim)

## Intent
Ein Theme überall: Omarchy, Zed und Neovim in Tokyo Night.

## Plan
- Goal: Alle drei zeigen Tokyo Night nach einem einzigen Theme-Wechsel.
- Steps:
  - [ ] `omarchy theme set tokyo-night`
  - [ ] Zed-Theme an Omarchy koppeln
  - [ ] Neovim-Colorscheme prüfen
  - [ ] Screenshots in den Case
- Affected paths:
  - `~/.config/zed/settings.json`
  - `~/.config/nvim/lua/plugins/theme.lua`
- Rollback: `omarchy theme set osaka-jade`.
- Verification: Theme-Wechsel ändert alle drei ohne Handarbeit.

## Log
<!-- append-only; engine and agents add dated lines -->
- 2026-09-29 19:30 · created (zone yellow, risk R1) · human

## Result

