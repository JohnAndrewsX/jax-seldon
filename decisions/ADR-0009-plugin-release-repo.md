# ADR-0009 — Monorepo for development, `jax-seldon-plugin` for the installable plugin

**Status:** accepted · **Date:** 2026-10-01

## Context
`omarchy plugin add <url>` is a plain `git clone` and expects `manifest.json`
at the repository root (see `memory/host.md` §7). This repository keeps the
plugin in `plugin/` next to the engine, contract and docs, which is the
right layout for development (one PR per contract change, shared fixtures).
ADR-0004 says the plugin repository contains QML only.

## Decision
- `JohnAndrewsX/jax-seldon` stays the **project home and monorepo**: engine,
  plugin, schema, fixtures, docs, ADRs, issues, releases, AUR source tarball.
- The installable plugin is published to a second repository
  `JohnAndrewsX/jax-seldon-plugin`, generated from `plugin/` with
  `git subtree split --prefix=plugin` by the release workflow (WP-040). It
  is never edited by hand; its README points to the monorepo.
- Names stay aligned: project `jax-seldon`, plugin id `jax.seldon`, AUR
  package `jax-seldon`, plugin repository `jax-seldon-plugin`.
- Marketplace install line: `omarchy plugin add
  https://github.com/JohnAndrewsX/jax-seldon-plugin.git`.
- Until the first release, testing installs the plugin by copying `plugin/`
  into `~/.config/omarchy/plugins/jax.seldon/` (docs/HERDR-SETUP.md §4).

## Consequences
One place to develop, one place for users to install from. The split is a
build step, so `plugin/` must stay self-contained: no relative imports
outside it, fixtures copied in by CI rather than linked (SPEC-PLUGIN §2).
