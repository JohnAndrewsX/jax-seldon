# ADR-0004 — Engine ships via AUR, plugin stays thin

**Status:** accepted · **Date:** 2026-10-01

## Context
Plugins are installed as git checkouts. Binaries in git are bad practice;
downloading binaries at install time is flagged by marketplace security
scans; compiling Rust on the user's machine requires a toolchain.
Precedent: 37signals' Basecamp plugin requires `omarchy pkg add basecamp-cli`
and uses that CLI.

## Decision
- The engine is an AUR package `jax-seldon` (source build; a `-bin` variant
  may follow). Installed with `omarchy pkg add jax-seldon`.
- The plugin repository contains QML only (plus docs and fixtures). It
  detects a missing engine and shows a banner with the install command
  (pattern borrowed from `palccod.omalog`'s "tracking is degraded" banner).
- The plugin can offer to run the install command in a terminal; it never
  runs it silently.

## Consequences
Two artefacts, two release cadences, one contract version pinned in both.
Marketplace scan stays clean (no network, no bundled binaries, no units).
