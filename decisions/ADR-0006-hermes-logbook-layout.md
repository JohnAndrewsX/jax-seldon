# ADR-0006 — Logbook follows the Hermes project layout

**Status:** accepted · **Date:** 2026-10-01

## Context
The earlier Omarchy-Agent kit separated a "base" (agent workshop) from a
"vault" (documentation). The HermesWatcher project foundation
(`AGENTS.md`, `PROJECT.md`, `STATUS.md`, `DECISIONS.md`, `inbox/`, `areas/`,
`work/{queued,active,completed}`, `resources/`, `outputs/`, `archive/`) is a
simpler, cross-agent convention.

## Decision
The logbook *is* a Hermes-style project named after the machine. Seldon adds
`journal/`, `ledger/`, `decisions/`, `system/`, `memory/`. Agents visit the
project; they do not live in it. Agent harnesses (`.claude/`, `.codex/`)
may sit inside, excluded from the Obsidian view. Code never goes into the
logbook except under `work/<case>/` as referenced workpieces.

## Consequences
Any agent that understands AGENTS.md can work in the logbook. The
Omarchy-Agent kit's zone model, gates and guard can be installed into
`.claude/` by the wizard as an optional harness.
