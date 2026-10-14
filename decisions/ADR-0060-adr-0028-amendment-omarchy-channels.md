# ADR-0060 — ADR-0028 amended: Omarchy's channel packages and the channel switch

**Status:** accepted (E96 delegation, orchestrator 2026-10-14; operator confirms; stage 2 with the 0.4.1 release)
**Date:** 2026-10-14

> Amends [ADR-0028](ADR-0028-attention-by-consequence.md) §2 and §4c,
> as [ADR-0037](ADR-0037-adr-0028-amendment-toggles-and-links.md),
> [ADR-0042](ADR-0042-adr-0028-amendment-files-pacman-left.md) and
> [ADR-0050](ADR-0050-adr-0028-amendment-seldon-self.md) did. ADR-0028
> stays accepted and unedited. Its tests in §1, its other rows and
> §3–§7 stand. WP-224.

## Context

Omarchy's edge channel ships its own packages in place of the stable
ones. `omarchy-dev` replaces `omarchy`, and `omarchy-settings-dev`
replaces `omarchy-settings`. Omarchy's kernels come as
`linux-omarchy-<variant>`, such as `linux-omarchy-bore`. The dev host
runs edge (audit E92, survey §1–3).

ADR-0028 §4c says "`alwaysRed` unchanged". The list is matched against
the package name as written. So on edge a named install or removal of
`omarchy-dev` is only attention `package`, where the same act on
`omarchy` is a crisis `always-red`. A list that `init` wrote earlier
keeps its own entries, so a new default alone would not reach existing
users.

`omarchy version channel edge` (and back) runs one
`pacman -S omarchy-dev omarchy-settings-dev`. The stable packages go as
conflicts. Under ADR-0028 §2 that is one transaction with four named
`alwaysRed` members, and the Omarchy collector's `update` row drops
from `4.0.4-2` to `4.0.0.r…` (edge numbers its own versions). The
`update` row became a second, unexplained attention item
`omarchy-other`, which reads like a downgrade.

## Decision

1. **The channel family counts as the stable names.** An `alwaysRed`
   entry also matches the names it stands for. `omarchy` stands for
   `omarchy-dev`, and `omarchy-settings` for `omarchy-settings-dev`.
   `linux-omarchy` stands for every `linux-omarchy-<variant>` except
   names ending in `-headers`, `-docs` or `-debug`, which are not R3
   (ADR-0023). The list is unchanged. The matching happens at index time,
   so a list that `init` wrote before gets it too. An entry for the twin
   (`omarchy-dev`) matches only the twin; the stable name never stands in
   for it.

2. **A channel switch is one change, rule `omarchy-channel`.** A pacman
   transaction that removes one of the paired packages and installs its
   twin, in either direction, is one drift item with rule
   `omarchy-channel`. Its class is what its members give it under
   ADR-0028 §2. If another package in the same transaction makes it a
   crisis by its own name (say `limine` named beside the switch), that
   reason stays (`always-red`). The rule id is new, and rule ids are an
   open set (`index.schema.json`, ADR-0038 §1), so there is no contract
   change. The plugin gets a crisis text for it.

3. **Outside a case it stays a crisis.** With the default list, a channel
   switch outside a case is a crisis, like any install or removal of an
   `alwaysRed` package by name. The switch replaces the shell, its
   settings and `/etc` defaults, which passes the harm test of
   ADR-0028 §1. Whether a switch that Omarchy runs itself should be
   quieter is the operator's call and a later amendment.

4. **The Omarchy `update` row a switch causes is routine
   `omarchy-update`.** It counts a switch's install of `omarchy` or
   `omarchy-dev` the way it counts a plain full upgrade, even when the
   version drops. The switch item carries the change; the version row is
   history beside it.

## Consequences

- On edge, the default list reds the same acts as on stable. A routine
  `pacman -Syu` that upgrades `omarchy-dev` stays routine `sysupgrade`.
- One item per switch instead of a removal, an install and a version
  drop. A second capture writes no new events (collector idempotency is
  unchanged; `engine/tests/drift.rs`).
- Not covered: a new Omarchy package pair. `OMARCHY_CHANNELS` in
  `engine/src/index/drift.rs` is the list, and a new pair needs a code
  change and a SPEC-ENGINE line.
- SPEC-ENGINE §2 (`[drift] alwaysRed`) and §5 record the rules. The
  user docs' `alwaysRed` lists are a follow-up.

## Alternatives considered

- **Add the twins to the default list.** Rejected. A list that `init`
  wrote earlier keeps its own entries (ADR-0028 §4c), so existing edge
  users would not get it.
- **Attention for a channel switch.** Not taken here (Decision 3). It is
  the operator's call; this ADR keeps ADR-0028's existing rule.
- **Keep the removal and the install as separate items.** Rejected. It is
  one act of the user and one Omarchy command. Two red items and a fake
  downgrade spend the user's attention three times.
