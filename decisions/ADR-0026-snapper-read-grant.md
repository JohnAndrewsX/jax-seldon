# ADR-0026 — Snapper access by a read grant on the snapshot directory (supersedes ADR-0011)

**Status:** accepted
**Date:** 2026-10-04

## Context
ADR-0011 made the snapper collector degrade without privileges and gave
the user one opt-in command:
`sudo snapper -c root set-config ALLOW_USERS=$USER SYNC_ACL=yes`.
snapper(8) has no read-only level for `ALLOW_USERS`: a listed user may
also create, change and delete root snapshots without a password. Since
WP-060 the collector reads the snapshots from the info files
(`/.snapshots/<number>/info.xml`) when `snapper list` is not permitted,
with the same events and cursor. That path needs only read access to the
snapshot directory, which Omarchy creates as `root:root 0750`. The
operator decided on 2026-10-04 to recommend that read access instead.

## Decision
This replaces ADR-0011 as a whole; the parts it keeps are restated here.
- The snapper collector runs **degraded by default**: when `snapper list`
  fails with a permission error and the info files cannot be read either,
  the collector reports `ok: false` with a short message in the index;
  `seldon doctor` shows the same state. Nothing else fails; no events are
  invented.
- The fix is a single command the user runs once, printed by the wizard
  (as an optional step) and by `seldon doctor`:

  ```
  sudo setfacl -m u:$USER:rx /.snapshots
  ```

  It grants read access to the snapshot directory listing and the
  snapshot info files, and no snapshot creation, change or deletion. The
  engine then reads the info files (WP-060).
- Seldon never runs this command itself. The plugin's banner for this
  state offers *Run in terminal* and *Copy*, as before (and *Check again*,
  WP-054).
- The opt-in of ADR-0011 is no longer recommended. When
  `snapper -c root get-config` succeeds (it does only for a user the
  config lists, or root) and its `ALLOW_USERS` names the current user,
  `seldon doctor` keeps the row `ok` and prints as its fix the revert
  followed by the read grant:

  ```
  sudo snapper -c root set-config ALLOW_USERS="" && sudo setfacl -m u:$USER:rx /.snapshots
  ```

  With `SYNC_ACL=yes` snapper removes the ACL it gave the user when the
  users list changes, so the grant comes after the revert. `doctor` runs
  `get-config` read-only, in the C locale like `snapper list`.

## Consequences
- No schema change; `contractVersion` stays 1. The degraded message
  (`NO_PERMISSIONS`) and the `snapper-degraded` index variant change
  text only.
- The ACL covers `/.snapshots` itself, not its contents: the snapshot
  directories and the files inside a snapshot keep their own
  permissions. The user can therefore read in an old snapshot what was
  readable to them when it was taken, as on the live system; a file made
  private later stays readable in older snapshots.
- The revert empties `ALLOW_USERS`. A config that lists other users too
  loses them; `doctor` says that the revert empties the list.
- A user who reaches snapper through `ALLOW_GROUPS` gets no revert hint;
  `doctor` still says that this user may use the snapper config.
- The collector, `doctor`, `init` and the plugin banner print the new line;
  the tests assert it with stubs and never run `setfacl` or
  `snapper set-config`.
