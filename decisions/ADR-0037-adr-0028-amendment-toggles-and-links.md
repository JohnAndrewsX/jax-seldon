# ADR-0037 — ADR-0028 amended: toggles are routine both ways, `system-link` is narrowed, `authorized_keys` is a default persistence path

**Status:** proposed (orchestrator, 2026-10-07, from WP-113 round 2; the
operator decides). §1 and §3 are implemented on branch
`wp/113-collector-hashes`, which is not merged into `next` before this
ADR is accepted. §2 is decided here and implemented by a follow-up WP
once its evidence item is checked.
**Date:** 2026-10-07

> Amends [ADR-0028](ADR-0028-attention-by-consequence.md) §2 (three rows,
> named below) and its §4d/§8 WP-E scope note. ADR-0028 stays accepted
> and unedited; its other rows, the three tests of §1 and §5–§7 stand.

## Context

WP-113 (ADR-0028 §8 WP-E) watches code the collectors could not see.
The operator approved three line items on 2026-10-06, hashes only:
third-party plugin trees, the toggle state folder
`~/.local/state/omarchy/toggles/**`, and `~/.ssh/authorized_keys` as an
opt-in. Two findings from WP-109's stage 2 and the WP-113 review need a
row change, which an implementing WP may not make on its own:

- **Toggles.** Omarchy writes the toggle folder in two ways (read-only
  check of `$OMARCHY_PATH`, dev host):
  - `omarchy-toggle <flag>` (the menu's *Toggle* entries: bar,
    screensaver, suspend, crash capture; `default/omarchy/omarchy-menu.
    jsonc`) creates and removes an **empty** flag file directly in
    `toggles/`, e.g. `toggles/bar-off`;
  - `omarchy-hyprland-toggle <flag> on|off` copies
    `$OMARCHY_PATH/default/hypr/toggles/<flag>.lua` to
    `toggles/hypr/<flag>.lua` and deletes it again; Hyprland loads every
    `.lua` there (`default/hypr/toggles.lua`).

  Under ADR-0028 §2 a flag file is attention `config` when it appears
  and attention `config-remove` when it goes (only the copy that matches
  Omarchy's file is routine, by `omarchy-default`, and only when it
  appears). Every *on* and every *off* from Omarchy's own menu would be
  a drift item. ADR-0028 §1 calls a toggle routine: "reversible in one
  step from the desktop … the normal operation of Omarchy itself (… a
  toggle …)".
- **`system-link`.** The row makes any symlink under a watched path
  routine when its resolved target lies under `/usr/`. Omarchy's hooks
  run `bash <file>` and `uwsm/env` is sourced, so a link from a hook or
  environment path to *any* root-owned script under `/usr/` runs at hook
  or login time, and the mark silences it. Root-owned content is not
  user persistence, but a link the user (or an attacker) chose *to* it is.
- **`authorized_keys`.** The operator decided that a change to it is a
  crisis when no case covers it, once the user opts in (WP-113 0.2.0
  form, 2026-10-06). The `alwaysRedPaths` row lists the default globs.

## Decision

### 1. A toggle is routine both ways (changes ADR-0028 §2: adds one row after the `omarchy-default` row; narrows the `config-remove` row)

New row, checked before the persistence paths like the other evidence
rows:

| Event | Class | Rule / resolution |
|---|---|---|
| `config-add` / `config-change` / `config-remove` under `~/.local/state/omarchy/toggles/**` whose content — `hashTo`, for a removal `hashFrom` — is empty (the SHA-256 of no bytes) | **routine** | `toggle-flag`: `omarchy-toggle`'s flag files carry no content; an empty file loads as nothing, also as `.lua` |
| `config-remove` under `~/.local/state/omarchy/toggles/**` with `meta.matches = "omarchy-default"`: at capture the removed content equals `$OMARCHY_PATH/default/<app>/toggles/<rel>` (trusted tree, as the `omarchy-default` row) | **routine** | `omarchy-default`: `omarchy-hyprland-toggle … off` |

The row "`config-remove` of any non-routine path → attention" is
narrowed accordingly: it no longer covers these two removals. Any other
content in the toggle folder stays as ADR-0028 has it: attention
(`config` when written, `config-remove` when removed). The capture
already records the removal evidence (WP-113); the empty-content test
reads the recorded hash at index time, so it needs no new mark.
`[drift] routine` gains the rule id `toggle-flag` (in the default list);
dropping it makes flag files attention again.

The removal evidence is limited to the toggle folder on purpose:
elsewhere, removing a copy of an Omarchy default can break a `require`
in `hyprland.lua`, and a removal stays the reason test's business.

### 2. `system-link` is narrowed (changes the ADR-0028 §2 `system-link` row)

New row text: `config-*` with `meta.matches = "system-link"`: the
subject is a symlink under `~/.config/systemd/user/**` whose resolved
target lies under `/usr/lib/systemd/user/`, or under
`~/.config/autostart/**` whose resolved target lies under
`/usr/share/applications/` → **routine** `system-link`. The capture
writes the mark for no other link.

A link elsewhere (hooks, `environment.d`, `uwsm`, `~/.profile`,
`~/.bash_profile`) into `/usr/` then falls through to `alwaysRedPaths`
and is a crisis, unless its content is Omarchy's shipped copy
(`omarchy-default` by content still applies: a link to
`$OMARCHY_PATH/config/uwsm/env` is routine by that row). Old events keep
the mark they have and classify as before (ADR-0028 §5: marks are
capture-time facts; nothing is rewritten).

Evidence and its gap: Omarchy's migrations `1785095882.sh`,
`1785167800.sh` and `1786539345.sh` link units into
`~/.config/systemd/user/…wants/` with targets under
`/usr/lib/systemd/user/`, which the new row covers. Not yet checked:
which targets a `--user enable` of a packaged unit and Omarchy's install
scripts create, and whether an autostart link ever targets
`/etc/xdg/autostart/` (not under `/usr/`, so today's mark never covers
it either; this ADR keeps that). The implementing WP checks both on the
test host (ADR-0028 WP-D) before it changes the capture.

### 3. `~/.ssh/authorized_keys` is a default persistence path (changes the default list of the ADR-0028 §2 `alwaysRedPaths` row)

The row's default list gains `~/.ssh/authorized_keys` (operator decision
2026-10-06). It is no default watch path: the user opts in by adding it
to `watchPaths`, and until then the glob matches nothing. The class of
the row is unchanged: a change without a case is a crisis, only the hash
is recorded. This resolves the "`authorized_keys` … is a capture-cost
and scope question for WP-E" note of ADR-0028 §4d.

### Rows changed, in ADR-0028 §2's order

1. `config-*` with `meta.matches = "system-link"` — narrowed (§2).
2. New row after `config-*` with `meta.matches = "omarchy-default"` —
   toggle flags (§1).
3. `config-*` under `[drift] alwaysRedPaths` — default list (§3).
4. `config-remove` of any non-routine path — no longer the two toggle
   removals of §1.

## Consequences

- An Omarchy toggle, from the menu or from Hyprland's flags, never
  becomes a drift item, on or off; what else lands in the toggle folder
  is visible as attention.
- A link to an arbitrary system script from a hook or login path is a
  crisis again once §2 is implemented; packaged units and autostart
  entries stay routine.
- Not a row change, recorded here because WP-113 round 2 relies on the
  existing rows: a followed directory link under a persistence path that
  the walk cuts off at its budget is recorded as one entry of its own,
  so it classifies as that persistence path's crisis (nobody can see
  into it, so a payload hidden behind decoys is the harm test's case);
  where every file is hashed, a file over 64 MiB is hashed by its size,
  modification time and inode (`meta.hashBasis = "stat"`), so a `touch`
  changes it and reading a huge file never holds the capture lock.
- `seldon doctor`'s effective rule set lists `toggle-flag`.
- Rollback: drop `toggle-flag` from `[drift] routine`, or
  `attention = "all"` (ADR-0028 §5).
