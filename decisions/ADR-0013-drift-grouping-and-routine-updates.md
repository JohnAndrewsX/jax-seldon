# ADR-0013 — Drift is grouped per pacman transaction; routine upgrades are yellow

**Status:** accepted; §3 superseded in part by ADR-0028
**Date:** 2026-10-01

> Superseded in part by [ADR-0028](ADR-0028-attention-by-consequence.md) (2026-10-06): routine is decided by consequence: inside a plain full upgrade `alwaysRed` upgrades are routine and an `omarchy` `update` attributed to it is routine; crisis follows the harm test, not the zone.

## Context
SPEC-ENGINE §5 rule 4 makes every caseless event drift and every red-zone
event a crisis. A routine `omarchy update` or `pacman -Syu` without an
active case therefore yields one crisis row per upgraded package: 30–200
rows that bury the one signal Seldon exists for. WP-002 raised this
(handover, decision 3). Settled by a §10 debate, one round.

Options debated: **A** transaction-level drift with a routine class;
**B** status quo plus a bulk-dismiss action in the plugin; **C** upgrades of
installed packages are never drift, the omarchy `update` event (or a
synthesised per-transaction event) is the single drift row.

*Devil's advocate (against A, for a variant of C):* a leader-keyed group
turns ADR-0008's "drift key = event id" into a derived, order-dependent
key; the closed `$defs/drift` object has no room for a group, so A is a
contract change before any code exists; `txId` is the transaction-start
timestamp and can change across a rotation mid-transaction, orphaning
resolutions; resolution fan-out is either 200 ledger lines per click or an
unbalanced `series.drift`; `meta.command` is attacker-controlled text and
`pacman -Syu ollama` hides an install inside a full upgrade; a yellow
kernel or compositor upgrade throws away the red signal; the UI needs
grouped rows. Drift is a to-do list for the human, the audit value is
already in `index.events`; so one to-do per transaction via a real ledger
event with its own ULID keeps ADR-0008 intact and needs no schema change.

*Advocatus Dei (for A, refined to A′):* the drift key stays an event id
(the leader's), each resolution still `refersTo` exactly one event, so
ADR-0008 needs no superseding; two optional fields (`txId`, `members`) keep
contract v1 valid and the fixture unchanged; grouping must cover every
transaction, because a caseless `pacman -S zed` with 30 dependencies
floods just the same (rule 2 only links dependencies of an explicit event
that *has* a case); the routine class is narrow (upgrade/reinstall, not
explicit, full-upgrade argv, not in an always-red list), so any install,
remove or downgrade in the transaction turns the whole group red and the
`-Syu ollama` trick fails; per-member resolution lines make stragglers
after a rotation reappear as a new group instead of being swallowed; C
leaves a bare caseless `-Syu` with a kernel upgrade with no drift at all,
and ties the `update` event to the transaction only by time window, which
§4 explicitly calls "not proof".

## Decision
Option **A′** as refined in the debate, with the amendments below.

1. **Grouping (index time, after §5 rules 1–3).** Open drift-eligible
   `pacman` events that share a `txId` and have no `case` form one drift
   item. Leader = lowest-id explicit member, else lowest-id member. The
   item's `eventId` is the leader's id, so the drift key stays an event id
   (ADR-0008 unchanged). Events from other sources are never grouped.
2. **Index.** `$defs/drift` gains two optional, additive fields: `txId`
   (string) and `members` (integer ≥ 2, omitted for single-event items).
   Members get no rows of their own. `contractVersion` stays 1 (unreleased
   draft, as in ADR-0012). The plugin shows a "+N" badge and expands the
   group from `index.events` by `txId`; `seldon drift show <id> --json`
   returns the full member list, because `index.events` is capped.
3. **Zone and crisis.** The item's zone is computed, not copied: yellow iff
   every member is *routine*, else red; `crisis` iff red. A member is
   routine iff kind is `upgrade` or `reinstall`, `explicit` is false, the
   transaction's `meta.command` parsed as argv is `-S` with
   `-u`/`--sysupgrade` and names no package, and the subject is not in
   `config.toml [drift] alwaysRed` (default `linux`, `linux-lts`,
   `linux-zen`, `linux-hardened`, `systemd`, `glibc`). Ledger events keep
   their own zone; nothing is rewritten. The omarchy `update` event stays
   its own red row, so a caseless `omarchy update` yields at most one
   yellow group plus the red `update` row and the red keyring transaction.
4. **Resolution.** `seldon drift link|explain|dismiss <any member id>`
   resolves every member of that group that is open at that moment, by
   appending one `resolution` line per member in one write (same `ts`,
   `actor`, `detail`, `case`, `meta.txId`); `--only` resolves the named
   event alone. Re-running finds zero open members and writes nothing.
   `series.drift` counts drift items, not resolution lines.
5. **Collector.** The pacman collector emits a transaction only after
   `transaction completed`, the next `transaction started`, or when
   `/var/lib/pacman/db.lck` is absent at capture time; until then the
   cursor stays at `transaction started`. Rotation dedupe (§4) is
   unchanged, so the leader is stable across runs.

Amendments to the debated A′: `alwaysRed` also matches by glob
(`linux*`), and `hyprland`, `omarchy`, `quickshell` are in the default
list, because the compositor and the shell are what the operator wants a
red strip for.

## Consequences
- Contract: two optional fields in `schema/index.schema.json`; the existing
  fixture validates unchanged. The sample index gains one open caseless
  `-Syu` group so the plugin surface renders (AGENTS.md §3). Done as a
  small follow-up on the WP-002 track before WP-008 and WP-021 start.
- Specs: SPEC-ENGINE §4 (transaction buffering) and §5 (grouping, routine
  class, `alwaysRed`, resolve fan-out) are amended; WP-008's acceptance
  test "three drift rows" becomes "three drift items, one of them a group".
- Code: WP-004 buffers transactions and parses `meta.command` as argv;
  WP-008 implements grouping, zone rule and fan-out with tests for groups,
  stragglers and the routine/red table; WP-021 renders the badge and the
  expand list.
- Rejected: B keeps the crisis flood and needs the same schema addition for
  a bulk action; C loses the record that a kernel upgrade happened outside
  any case and relies on time-window attribution.
