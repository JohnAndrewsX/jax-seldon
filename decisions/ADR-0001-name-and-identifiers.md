# ADR-0001 — Name, identifiers and binary name

**Status:** accepted · **Date:** 2026-10-01

## Context
The project needs a name that is free in the Omarchy plugin catalog
(4 537 entries checked on 2026-10-01), fits a sci-fi tone, and carries the
author's `jax` prefix. "Omalog" is taken (`palccod.omalog`).

## Decision
- Product name: **Seldon**. The full-screen overlay is the **Prime Radiant**.
- Plugin ID: `jax.seldon`. Display name: `JAX Seldon`.
- Repository: `JohnAndrewsX/jax-seldon`. AUR package: `jax-seldon`.
- Engine binary: `seldon`, installed with a `jax-seldon` symlink.
  Known risk: Seldon Core (MLOps) also ships a `seldon` CLI; it is not an
  Arch package and unlikely on an Omarchy desktop. If a conflict is reported,
  the symlink direction flips (`jax-seldon` primary).
- Config: `~/.config/seldon/config.toml`. State: `~/.local/state/seldon/`.
- Default logbook path: `~/Seldon`.

## Consequences
All docs, code and the marketplace listing use these names. Foundation
vocabulary is used sparingly: *Plan* (cases), *Crisis* (high-risk drift),
*Prime Radiant* (overlay). Everything else uses plain words.
