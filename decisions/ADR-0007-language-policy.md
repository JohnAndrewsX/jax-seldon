# ADR-0007 — Language policy

**Status:** accepted · **Date:** 2026-10-01

## Decision
Repository, code, specs, UI strings, engine messages: **English**.
Conversation with Eugen: **German**. Logbook content: the user's language;
`seldon init` asks and stores `language` in the logbook's `PROJECT.md`
frontmatter and in `config.toml`. Templates ship in English and German.

## Consequences
The plugin shows user text verbatim and never translates. UI i18n is
deferred (future ADR).
