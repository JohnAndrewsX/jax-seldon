# ADR-0042 — ADR-0028 amended: a file pacman left is attention, a crisis beside the boot and login files

**Status:** proposed (WP-141 round 2; the operator accepts).
**Date:** 2026-10-08

> Amends [ADR-0028](ADR-0028-attention-by-consequence.md) §2 by two rows
> (below), as [ADR-0037](ADR-0037-adr-0028-amendment-toggles-and-links.md)
> did for its four. ADR-0028 stays accepted and unedited; its three tests
> of §1, its other rows and §3–§7 stand. ADR-0013 §1 (grouping by `txId`)
> stands: the new event is no member of a group.

## Context

Operator decision 2026-10-07 (P1, WP-141): Seldon names the most common
cause of breakage after an Arch update, a configuration file pacman left
as `.pacnew` (the package's new default was not applied; the user's file
stays in use), `.pacsave` (on removal the user's modified file was moved
aside) or `.pacorig` (an untracked file was moved aside for the package's
own). pacman writes each to `/var/log/pacman.log` as `[ALPM] warning:
<file> installed as <file>.pacnew` or `… saved as <file>.pacsave|.pacorig`.
WP-141 records each as a `pacman` `note` whose subject is the file pacman
left (SPEC-ENGINE §4). Seldon never reads `/etc` (AGENTS.md §6): whether
the file was merged later is not known.

ADR-0028 §2 names no row for such an event; its total row would make it
attention `other`. The WP asks for a crisis beside the files boot and
login depend on. A new crisis row changes what the bar's one signal
means, which an implementing WP may not decide on its own (ADR-0028 §1:
"crisis … must stay rare, or it is no signal").

Ownership evidence (read-only, the installed Omarchy tree, Omarchy
4.0.4-1, and the stage-1 review of WP-141):

- **mkinitcpio** (`/etc/mkinitcpio.conf`, `.conf.d/`, `.d/` presets):
  Omarchy writes drop-ins under `/etc/mkinitcpio.conf.d/`
  (`install/hardware/nvidia.sh`, `fix-surface-keyboard.sh`,
  `apple/fix-t2.sh`, `apple/fix-spi-keyboard.sh`, two migrations); the
  hooks line decides whether the next initramfs boots.
- **Limine** (`/etc/default/limine`, `/etc/limine-entry-tool.d/`): Omarchy
  writes both (two migrations, `install/hardware/apple/fix-t2.sh`);
  `/etc/default/limine` overrides every drop-in.
- **PAM** (`/etc/pam.d/`): Omarchy edits upstream-owned files there
  (`install/config/increase-lockout-limit.sh`, `install/login/sddm.sh`,
  the fingerprint first-run hook, a migration); a stack that names a
  module the package no longer ships stops every login. A `.pacnew` there
  is therefore likely on an Omarchy machine and worth the signal.
- **`/etc/security/`**: Omarchy overrides `faillock.conf` (upstream owned
  by `pam`; `etc-overrides/security-faillock.conf`, a migration) and
  appends to `pam_env.conf`. A `.pacnew` beside these is expected after
  routine `pam` updates and the user's file stays in use: attention.
- **`/etc/systemd/`**: Omarchy uses drop-ins (`logind.conf.d/`,
  `oomd.conf.d/`, `install/hardware/network.sh`, migrations), not edits
  of the shipped files; a `.pacnew` of a shipped `*.conf` leaves the
  drop-ins in force: attention.
- **`/etc/fstab`, `/etc/crypttab`, `/etc/sudoers`** stay attention (the
  reviewer's evidence): pacman never replaces the file in use with a
  `.pacnew`, so the working mount, unlock and sudo configuration stays;
  Omarchy keeps its sudo rules in `/etc/sudoers.d/`
  (`migrations/1788025225.sh`).

## Decision

### 1. Two rows for ADR-0028 §2

| Event | Class | Rule / resolution |
|---|---|---|
| pacman `note` (a file pacman left: `.pacnew`, `.pacsave`, `.pacorig`) beside a boot or login file — `/etc/mkinitcpio.conf`, `/etc/mkinitcpio.conf.d/**`, `/etc/mkinitcpio.d/**`, `/etc/default/limine`, `/etc/limine*` (incl. `/etc/limine-entry-tool.d/**`), `/boot/limine*`, `/etc/pam.d/**` | **crisis** | `pacnew-red`: harm test — an unmerged default there can stop the next boot or every login, and nobody asked for it in a case |
| pacman `note`, any other file (incl. `/etc/security/**`, `/etc/systemd/**`, `fstab`, `crypttab`, `sudoers`, another root such as `pacman -r /mnt`) | **attention** | `pacnew`: reason test — a state a rebuild would reproduce wrongly and nothing but a merge changes; never routine, whatever its transaction |

The list is built into the engine (`class.rs` `PACNEW_RED`), not a
`[drift]` key: `alwaysRedPaths` holds `~`-paths of the config collector.
The suffix is stripped before the match; a directory covers what lies
below it. Both rows apply whatever the transaction was (a plain full
upgrade included): the event is its own item, its transaction in
`meta.transaction`, never in `txId`.

### 2. What the user sees

The Changelog's detail of such an event reads "Merge with pacdiff (from
pacman-contrib) in a terminal. Seldon does not read /etc, so it cannot
tell whether that happened since." — text, never a command the plugin
runs (AGENTS.md §8). A `pacnew-red` crisis says why it is loud
(SPEC-PLUGIN).

### 3. Contract

No bump. `kind: note` and `source: pacman` exist; `meta.transaction` is
a scalar `meta` key, now named among the conventional keys of
`event.schema.json`, and `txId`'s description says it is shared by the
package lines of a transaction. Descriptions only (AGENTS.md §3 asks an
ADR for a semantic change; this is it).

## Consequences

- A `pam` or `mkinitcpio` update that leaves a `.pacnew` turns the bar
  red until the user (or an agent with evidence) resolves the item. That
  is the signal the operator asked for; it stays rare because pacman
  leaves a `.pacnew` only where the user's file differs from the old
  default.
- Seldon cannot clear the item by itself when the user merges: it does
  not read `/etc`. The user explains or dismisses it (a crisis cannot be
  explained or dismissed by an agent, ADR-0028 §3).
- Widening or narrowing the list is a new ADR, as this one.

## Alternatives considered

- **`/etc/systemd/**` and `/etc/security/**` as crisis** (WP-141 round
  1): rejected in review — Omarchy's own drop-ins and overrides make a
  `.pacnew` there routine on its machines, and the file in use keeps
  working; a red bar after every `systemd` or `pam` update spends the one
  signal.
- **A `[drift] alwaysRedPacnew` key**: deferred; no user asked for it.
- **The note inside its transaction's group** (`txId`): rejected in
  WP-141 (SPEC-ENGINE §4): "I wanted that package" would resolve the
  merge still to do, and a routine `-Syu` would become drift as a whole.
