# ADR-0017 — Hook command events carry the start time; attribution rules sharpened

**Status:** accepted (clarifies ADR-0014 §1; ADR-0014 otherwise stands)
**Date:** 2026-10-01

## Context
WP-004 implemented ADR-0014 §1 literally: a collector event inherits actor
and case from a hook `command` event that *precedes* it. Claude Code's
`PostToolUse` hook fires after the command has finished, so a command event
written there would carry an end time and always come *after* the pacman
lines it caused; attribution would never match real hook data. The WP-004
reviewer also found that a full-upgrade command attributed *any*
transaction inside the window, including a human's separate install five
minutes later, which would hide human drift under an agent case forever
(the ledger is immutable). Verified on Omarchy 4.0.4:
`omarchy-update-keyring` installs `archlinux-keyring` and `omarchy-keyring`
before the full upgrade.

## Decision
1. **Start time.** A hook `command` event's `ts` is the instant the command
   *started*. `seldon hook claude-code` writes the event on **PreToolUse**
   (the payload carries `tool_input.command` or `file_path` and
   `tool_use_id`); PostToolUse writes nothing for a command already
   recorded. `seldon hook generic` is invoked before the command runs or
   receives `startedAt`. A command that is denied or fails is still
   recorded; ADR-0012 §6 keeps it out of drift. `meta.toolUseId` (scalar,
   in `extra`) may be stored so a later PostToolUse can be matched without
   a second event. ADR-0014 §4 (Edit/Write events) moves to PreToolUse too.
2. **Window.** ADR-0014 §1(b) reads: the collector event belongs to a pacman
   transaction that *began* (its `[PACMAN] Running` line, else
   `transaction started`; an event outside a transaction begins at its own
   `ts`) at most 10 minutes after the command's `ts` and not before it;
   every member of that transaction inherits.
3. **Full upgrades attribute only full upgrades.** A full-upgrade command
   (`-Syu`/`-Su`, `omarchy update`, bare `yay`) attributes only
   transactions whose own logged command is a full upgrade (or that have
   no logged command); `omarchy update` additionally names
   `archlinux-keyring` and `omarchy-keyring`. Any other transaction needs
   a command that names the package.
4. **The omarchy `update` event** inherits from the pacman event that moved
   the `omarchy` package to its `meta.to` (same capture or ledger); failing
   that, from a full-upgrade command per 2, measured against the capture
   time.
5. **Query sub-commands are not mutating.** `pacman -Ss|-Si|-Sl|-Sg|-Sp|-Sw|-Sc`,
   `-Q*`, `-T` and their long forms never attribute and are not recorded
   as mutating by the hook (SPEC-ENGINE §8).

## Consequences
WP-004 implements 2–5 in the pacman and omarchy collectors; WP-009
implements 1 and 5 in the hook and reuses the collector's command parser
(moved to a neutral module when WP-009 lands). The fixture logbook is
unchanged: its hook command events already precede their transactions.
