# ADR-0003 — Markdown is the record; the ledger's JSONL is the machine source

**Status:** accepted · **Date:** 2026-10-01

## Context
"File over app": the logbook must be readable without Seldon and must work
as an Obsidian vault. But machine-captured events (thousands per year) are
awkward to parse back out of prose, and the plugin must never parse Markdown.

## Decision
- Human-authored content (journal, cases, decisions, memory, areas, system
  dossier) is **Markdown with YAML frontmatter** and is the source of truth.
- Machine-captured events are written to `ledger/YYYY-MM.jsonl` (append-only,
  one event per line, schema `event.schema.json`). A human-readable
  `ledger/YYYY-MM.md` view is generated from it and marked as generated.
- `STATUS.md` and `~/.local/state/seldon/index.json` are generated
  projections and can be deleted at any time.

## Consequences
The logbook stays portable; the engine can rebuild every derived file from
Markdown + JSONL; Obsidian users see a readable ledger without Dataview.
