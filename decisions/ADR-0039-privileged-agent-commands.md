# ADR-0039 — The hook records an agent's privileged commands

**Status:** proposed (WP-129; operator decision 2026-10-07 E14 a: "ja,
als kleines Paket für 0.2.0"). §1 and §2 are implemented on `next` by
WP-129; §3 waits for the contract decision it names.
**Date:** 2026-10-07

> Amends SPEC-ENGINE §8 (a new hook record class), ADR-0019 §1 (a green
> record gives way to it), ADR-0014 §2 (its zone) and, once §3 is
> decided, ADR-0028 §2 (a new row) and ADR-0012 §6 / ADR-0014 §5 (hook
> `command` events are never drift). ADR-0031 stands: this ADR records
> what Omarchy's route runs; it changes nothing about how the agent asks
> for privilege.

## Context

On 2026-10-07 an Omarchy agent set up a network printer from a session in
the user's work folder: it ran `pkexec lpadmin -p … -m everywhere -E`, the
user typed the password, `/etc/cups/printers.conf` changed — and Seldon
recorded nothing. The hook (SPEC-ENGINE §8) records six classes only:
package commands, `omarchy` routes and scripts, `systemctl` unit changes,
writes into watched paths, `git` in the config home or the logbook, and
snapshot commands. `lpadmin` is none of them, and no collector watches
CUPS (WP-131 adds hashes later).

`pkgcmd.rs` already reads past `sudo`, `doas`, `pkexec` and `run0` with
their options and knows their probes (`sudo -l`, `pkexec --version`,
`command -v …`, F-531). Omarchy's own skill tells agents to use `sudo`
where a terminal shows the prompt and `pkexec` where it cannot; ADR-0031
made that Seldon's rule too. So every privileged step an agent takes goes
through one of four words the hook already parses — and asks the user for
a password. A privileged command is rare, deliberate and changes the
system as root: exactly what the record must not miss.

## Decision

### 1. The record class

A **privileged command** is a simple command (after `bash -c '…'`, `eval`,
the other wrappers and `;`/`&&`/`||`/`|`) that runs under `sudo`, `doas`,
`pkexec` or `run0` — its own wrapper, or the wrapper of the shell it was
opened from (`pkexec sh -c 'lpadmin …'`) — a program that is not a probe.
Probes record nothing: the wrappers' own (`sudo -l|-v|-K|-V`, `pkexec
--version|--help`, `run0 --help`, `doas -C|-L`, `command -v|-V`), and the
programs that only test the privilege they run under: `true`, `false`,
`:`, `id`, `whoami`, `test`, `[` (`sudo -n true`).

It is recorded **unless a class records the command by itself**: a class
whose record needs no case (package, `omarchy`, `systemctl`, a write into
a watched path, `git` in the config home or the logbook), and a snapshot
command (green, with a case, as before; the snapper collector records the
snapshot either way). A green record that would need a case (ADR-0019: a
foreign package manager, a file outside the watched paths, `git`
elsewhere) gives way to the privileged one for that command: `sudo tee
/etc/x` is one privileged record, never also a green one.

At most one privileged record per command line, for its first privileged
command; the line's class record (the most severe, as before) is written
beside it when another command of the line has one (`pkexec pacman -S
cups && pkexec lpadmin …`: a `pacman` record and an `lpadmin` record, one
tool call).

### 2. What the record holds

- `source: agent`, `kind: command` — the kind every hook record has (a new
  kind would be a schema change); the class is told by `meta.wrapper`,
  which only privileged records carry.
- `subject`: the program's last path component (`lpadmin`).
- `detail` and `meta.command`: the command line, heredoc bodies cut,
  redacted (SPEC-ENGINE §7) before it is cut to 4096 characters; the index
  clips both (ADR-0025). Lines that name a `[redaction] skipPaths` path are
  `<program> ‹redacted›`, as for every hook record.
- `meta.wrapper`: `sudo`, `doas`, `pkexec` or `run0` — the first one the
  command runs under.
- `zone: red`: the command acts on the system as another user, root by
  default (ADR-0014 §2: where packages and system services act). It counts
  as the case's red change for `plan snapshot`'s order check.
- Actor, `ts`, `meta.toolUseId`/`sessionId`, case: as every hook record.
  It is recorded **with or without a case** (red; ADR-0019 does not drop
  it); with an active case it is linked to it. A replayed tool call writes
  nothing (`toolUseId`, ADR-0030 §5).
- A privileged `snapper` command (`sudo snapper delete 5`) is never a
  snapshot command for SPEC-ENGINE §5's case notes.

### 3. Its class under ADR-0028 (needs a contract decision)

Proposed row for ADR-0028 §2:

| Event | Class | Rule |
|---|---|---|
| hook `command` with `meta.wrapper` (privileged command), no case | **attention** | `privileged-command`: reason test — a root change "why is this here?" must answer; never routine (nothing Omarchy's UI does runs through the agent hook); never a crisis by itself (the harm test belongs to what it changed: an `alwaysRed` package or a persistence path is a crisis through its own event) |

This needs the event to be **drift-eligible**, and today it cannot be:
`index.schema.json` limits `drift[].source` to `pacman`, `omarchy`,
`plugins`, `theme`, `config` ("only system-changing collectors produce
drift", ADR-0012 §6). An `agent` item would fail `index --check`. The
pre-tag window of contract 2 (CONTRACT.md, ADR-0035 §6) admits new
optional fields only, not a wider enum. So the operator decides one of:

- **(a) Widen contract 2 before 0.2.0 is tagged** (recommended): an
  amendment of ADR-0035 §6 (or CONTRACT.md's window rule) lets
  `drift[].source` admit `agent` for privileged commands; the engine makes
  them drift-eligible (`linkable`, rule 3 proposals, rule 9, `drift
  link|explain|dismiss`, `series.drift`, the session-start list), class
  attention, rule `privileged-command`; `scripts/validate-fixtures.py`
  ports it; the fixture gains one such item. The plugin needs no code: it
  treats a drift row's `source` as a label and already has a glyph for
  `agent` (`Model.js` `SOURCE_GLYPHS`); an older plugin cannot meet this
  engine anyway (contract 2). One small engine WP.
- **(b) Defer to contract 3.** The record (§1, §2) stands as implemented;
  the event is in the Changelog and the case, but never open drift.

Until then the engine does (b).

## Consequences

- An agent's `pkexec lpadmin …` is one red `agent/command` event with
  `meta.wrapper: pkexec`, on the case when one is active, also without
  one. Nothing about the command is read but its text (AGENTS.md §6
  unchanged).
- More command lines reach the ledger than before: every privileged
  command, reads included (`sudo cat /etc/…`). The redaction rules of §7
  apply; a program that takes a secret as a plain positional argument
  (`nmcli … password X`) is not covered by them today (WP-129 handover,
  open question).
- A command whose class already records it is unchanged (`pkexec pacman
  -S x` is one package record, without `meta.wrapper`).
- Sessions outside the hooks' scope (ADR-0030 §1: not in the logbook, not
  launched by `seldon agent start`, `[hooks] scope = "logbook"`) stay
  unrecorded, privileged or not.

## Alternatives considered

- **A new kind `privileged-command`.** Rejected: a schema enum change and a
  contract bump for what `meta` already carries; every reader of hook
  records (attribution, case notes, the dossier) keys on `kind: command`.
- **Record every privileged command, also beside its own class.**
  Rejected: two events for one command, two drift items for one change.
- **Yellow zone.** Rejected: the command acts as root on the system, not on
  the user's configuration; red keeps `plan snapshot`'s "snapshot after
  the first red change" warning honest.
- **Treat reads as probes.** Rejected: a read as root is still a use of
  privilege the user granted with a password; the WP's goal is every
  privileged command in the record.
