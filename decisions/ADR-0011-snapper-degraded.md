# ADR-0011 — Snapper collector degrades without privileges; the user opts in

**Status:** accepted · **Date:** 2026-10-01

## Context
`snapper list` needs root unless the snapper config lists the user in
`ALLOW_USERS`. Omarchy's default config does not, so every Seldon user hits
this, not only the developer. Seldon never calls `sudo` (PROJECT.md).

## Decision
- The snapper collector runs **degraded by default**: when the command
  fails with a permission error the collector reports `ok: false` with a
  short message in the index; `seldon doctor` shows the same message.
  Nothing else fails; no events are invented.
- The fix is a single command the user runs once, printed by the wizard
  (as an optional step) and by `seldon doctor`:

  ```
  sudo snapper -c root set-config ALLOW_USERS=$USER SYNC_ACL=yes
  ```

- Seldon never runs this command itself. The plugin's banner for this
  state offers *Copy* and *Open terminal*, like the engine-missing banner.

## Consequences
Phase 0 works without touching `/etc`. Snapshot markers in the timeline,
`snapshotBefore` on cases and the System tab's snapshot list appear once
the user opts in. The collector tests cover both the privileged output and
the permission error.
