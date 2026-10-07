# ADR-0039 — The hook records an agent's privileged commands

**Status:** accepted (operator decision 2026-10-07, after Opus and Fable
stage 2; E16). WP-129 (operator decision 2026-10-07 E14 a: "ja, als
kleines Paket für 0.2.0") implements §1 and §2; §3 is option (b), the
orchestrator's decision of 2026-10-07 for 0.2.0.
**Date:** 2026-10-07

> Amends SPEC-ENGINE §8 (a new hook record class), ADR-0019 §1 (a green
> record gives way to it) and ADR-0014 §2 (its zone). §3 leaves ADR-0028
> §2 and ADR-0012 §6 / ADR-0014 §5 (hook `command` events are never
> drift) as they are. ADR-0031 stands: this ADR records what Omarchy's
> route runs; it changes nothing about how the agent asks for privilege.

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

**Limits.** The hook reads the command text with SPEC-ENGINE §8's parser
and nothing else. A privileged command inside `$(…)`, backticks or `env
-S`, in a script file, an interpreter, `xargs`, `find -exec`, a shell
function or alias is not seen; `su`, `sudoedit` and `systemd-run` are not
wrappers of this class. The record serves honest agents; it is not an
enforcement boundary (AGENTS.md §6 stays the rule).

### 2. What the record holds

- `source: agent`, `kind: command` — the kind every hook record has (a new
  kind would be a schema change); the class is told by `meta.wrapper`,
  which only privileged records carry.
- `subject`: the program's last path component (`lpadmin`).
- `meta.command`: the command line, heredoc bodies cut, redacted
  (SPEC-ENGINE §7) before it is cut to 4096 characters; `detail` the same
  after `asked to run: `. The hook runs at PreToolUse, before the password
  prompt, and nothing later confirms that the command ran: the record says
  what the agent asked to run. The index clips both (ADR-0025). Lines
  that name a `[redaction] skipPaths` path are `<program> ‹redacted›`, as
  for every hook record.
- **A password on the wrapper's stdin** (round 2): a line in which any
  command has sudo read the password from stdin (`-S`, `--stdin` or an
  abbreviation getopt takes, `--st`…; also in a cluster such as `-Su`, and
  beside a probe: `echo PW | sudo -S -v && …`) holds the password in a
  form no §7 rule knows. Every record of that line is `<program>
  ‹redacted›`, as for `skipPaths`. `doas` asks on the terminal, `pkexec`
  and `run0` through the polkit agent: none of them reads a password from
  stdin. The check reads the same commands as the classification: a
  `sudo -S` inside `$(…)` is not seen.
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

### 3. Its class: not drift in contract 2 (option b)

The reason test of ADR-0028 would make the record **attention** (rule
`privileged-command`; never routine, never a crisis by itself). It
cannot be drift today: `index.schema.json` limits `drift[].source` to
`pacman`, `omarchy`, `plugins`, `theme`, `config` ("only system-changing
collectors produce drift", ADR-0012 §6), and contract 2's pre-tag window
(CONTRACT.md, ADR-0035 §6) admits new optional fields only, not a wider
enum.

**Decided for 0.2.0 (orchestrator, 2026-10-07): no `agent` drift
source.** The record is an event: red, in the Changelog, on the case
when one is open. The system change it causes is attention drift through
its own collector — the printer configuration through WP-131's hashes,
packages, units and watched paths through theirs. Contract 3 is the place
for an `agent` drift source if the live week shows a need: the plugin
treats a drift row's `source` as a label and already has an `agent`
glyph, so the change would be the schema, eligibility in the index build
and reconciliation, `validate-fixtures.py` and one fixture item.

## Consequences

- An agent's `pkexec lpadmin …` is one red `agent/command` event with
  `meta.wrapper: pkexec`, on the case when one is active, also without
  one. Nothing about the command is read but its text (AGENTS.md §6
  unchanged).
- More command lines reach the ledger than before: every privileged
  command, reads included (`sudo cat /etc/…`). The redaction rules of §7
  apply; a program that takes a secret as a plain positional argument
  (`nmcli … password X`, `wifi-sec.psk X`, `htpasswd -b`, `usermod -p`)
  is not covered by them today; those forms are WP-140's follow-up. A
  password on the wrapper's own stdin is never written (§2).
- A privileged read (`sudo cat /etc/…`) counts as the case's first red
  change for `plan snapshot`'s order check: accepted noise, rare, and
  each use was asked for with a password.
- A privileged subject in `[drift] alwaysRed` (`sudo mkinitcpio -P`)
  in a case below R3 raises the index build's R3 advisory warning
  (SPEC-ENGINE §5, §6), as any red `alwaysRed` change does: intended. The
  case's advisory Log line is written by a capture for the events it
  writes, so a hook record gets the warning, not the Log line.
- The record is written when the agent asks (PreToolUse): a refused or
  cancelled password prompt still leaves a red record that says "asked to
  run".
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

## Amendment note (WP-140, 2026-10-07)

A follow-up of §2's stdin bullet, from WP-129's Fable stage 2, not a new
decision. Besides a password on sudo's stdin, a line is recorded as
`<program> ‹redacted›` when any command takes its secret as a plain
argument or from stdin that the line feeds, in a form no SPEC-ENGINE §7
rule can tell from its other words: `chpasswd`, `chgpasswd`, `htpasswd
-b`/`-i`, `smbpasswd -s`/`-w`, `passwd -s`/`--stdin`, `useradd`,
`usermod`, `groupadd` and `groupmod` with `-p`, and `cryptsetup` on a
line with a pipe, a here-string or a process substitution (SPEC-ENGINE
§8). The list is hook-local, as the stdin check is. nmcli's secrets are
named by their property, so they are a §7 rule (`nmcli-secret`) and an
nmcli line keeps its other words.
