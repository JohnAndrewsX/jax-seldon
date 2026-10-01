# ADR-0002 — Engine in Rust, plugin in QML, JSON contract between them

**Status:** accepted · **Date:** 2026-10-01

## Context
The plugin must be Quickshell QML (that is the Omarchy plugin system). The
engine language was open: Bash, Python (stdlib, as `palccod.omalog` and
`ch.wertstifter.tyme` do), Rust, Go. Requirements: minimal footprint, no
interpreter dependency, fast start (hooks run on every agent tool call),
no rewrite later.

## Decision
- Engine in **Rust**, single static binary, no async runtime, no network.
- Plugin in **QML**, reads only the JSON index, calls the engine for writes.
- A versioned **JSON Schema contract** (`schema/`) is the only coupling.
  Fixtures let the plugin be built before the engine exists.

## Consequences
- Hook latency ~1–2 ms instead of 30–50 ms; optional watcher costs single-
  digit MB.
- Rust requires a build or a package for users → ADR-0004.
- Either side can be rewritten without touching the other, which is the
  actual insurance against the "rewrite Python later" scenario.
- No SIMD/assembly work: the workload (log parsing, a few hundred Markdown
  files) does not justify it. This is explicitly out of scope.
