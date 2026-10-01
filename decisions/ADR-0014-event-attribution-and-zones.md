# ADR-0014 — Event attribution, zones, plugin version source, agent file edits

**Status:** accepted
**Date:** 2026-10-01

## Context
WP-002 built the fixture logbook and found four producer rules that no spec
states: how a collector event inherits actor and case from an agent's hook
`command` event (SPEC-ENGINE §4 "only hooks set agent actors" contradicts
§5 rule 1, which links collector-found agent events); which zone an event
has (the fixtures use a table nobody wrote down); where the `plugins`
collector gets a version from (neither `omarchy plugin list --json` nor
`omarchy plugin catalog` carries one, verified on Omarchy 4.0.4); and
whether the Claude Code hook records `Edit`/`Write` tool calls. The
WP-002 reviewer classified all four as orchestrator decisions and
recommended the rules below; they are producer rules, not index derivation,
so they live here and not in ADR-0012.

## Decision
1. **Attribution.** A collector event inherits `actor` and `case` from an
   agent `command` event when (a) the command named the event's subject
   (package name, path, or a full upgrade — `-Syu`/`-Su`/`omarchy update` —
   for every member of the resulting transaction) and (b) the collector
   event's `ts` lies within the same pacman transaction or at most 10
   minutes after the command. Otherwise the actor is `system`. Time
   proximity alone is never proof (SPEC-ENGINE §4 stays). The fixture
   logbook already encodes this reading.
2. **Zones.** The engine assigns zones by this table; `crisis` derives from
   it (ADR-0008, ADR-0013):
   - red: `pacman` events, `omarchy` events, systemd units including
     `config-change` under `~/.config/systemd/`;
   - yellow: other `config` events, `theme`, `plugins`;
   - none: `snapper`, `seldon` notes and case events, `resolution`.
   - hook `command` events take the zone of what the command would have
     produced, by the same table.
   A later `config.toml [zones]` override is a UX feature, not a contract
   change, because the index carries the resolved zone.
3. **Plugin version.** The `plugins` collector reads `version` from the
   manifest at the plugin's `manifestPath`; when the manifest has no
   version, it uses the plugin directory's git HEAD (short hash).
   `plugin-update` fires when either changes.
4. **Agent file edits.** The Claude Code hook records `Edit`, `Write` and
   `MultiEdit` tool calls whose `file_path` lies under the config
   collector's `watchPaths` as `source: agent, kind: command, subject:
   <tool name lowercased>`, `meta.command: "<Tool> <path>"`, actor and
   case as for Bash commands. Paths matching `[redaction] skipPaths` are
   recorded with the path redacted. No schema change.
5. **Known gap (acknowledged, not solved).** An agent `command` without a
   case that produces no collector event (e.g. `git push`, a `systemctl`
   query) is never drift (ADR-0012 §6). Revisit when WP-009 has real hook
   data.

## Consequences
SPEC-ENGINE §4 (plugins collector, attribution paragraph) and §5 (zone
table) are amended. WP-004 implements rule 1 for pacman, WP-005 rules 2–3,
WP-006/WP-009 rule 4. `fixtures/logbook` needs no change; the WP-007
golden test is the arbiter.
