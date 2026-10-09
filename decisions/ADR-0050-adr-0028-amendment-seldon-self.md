# ADR-0050 — ADR-0028 amended: installing Seldon's own plugin is routine

**Status:** proposed (WP-172, from the fresh-host test T1 of 0.1.4)
**Date:** 2026-10-09

> Amends [ADR-0028](ADR-0028-attention-by-consequence.md) §2 by one row
> (below), as [ADR-0037](ADR-0037-adr-0028-amendment-toggles-and-links.md)
> and [ADR-0042](ADR-0042-adr-0028-amendment-files-pacman-left.md) did.
> ADR-0028 stays accepted and unedited; its three tests of §1, its other
> rows and §3–§7 stand. SPEC-ENGINE §5 rule 8 (WP-086, review F4) stands:
> no resolution is written for an add.

## Context

T1 (orchestrator, 2026-10-09, a freshly installed Omarchy 4.0.4, README
word for word): the README's order is the engine, `seldon init`, then
`omarchy plugin add https://github.com/JohnAndrewsX/jax-seldon-plugin.git
--enable`. The next capture records `plugin-add jax.seldon` (and the
`shell.json` change, routine `routine-paths`). By ADR-0028 §2 a
third-party `plugin-add` is attention (`plugin`), so the first thing Seldon
asks a new user to explain is installing Seldon. That is the opposite of
ADR-0028 §1 ("the user should notice as little of Seldon as possible").

ADR-0028 §2's row "pacman of `jax-seldon`, `plugin-*` of `jax.seldon` |
auto-explained | §5 rule 8 unchanged" reads as if every plugin event of
Seldon's own id were covered. Rule 8 is narrower since WP-086 round 2
(review F4): it writes an `explained` resolution only for `plugin-update`,
`plugin-enable` and `plugin-disable` (and pacman `upgrade`/`reinstall` of
`jax-seldon`), because it matches only the id and nothing checks where the
code came from; an explanation is a statement about intent the engine
cannot back for an add. So the add falls to the third-party row.

The collector records `omarchy plugin add … --enable` as one `plugin-add`
with `meta.enabled: true`; a separate `plugin-enable` occurs only when the
plugin is enabled later (Omarchy's menu, `omarchy plugin enable`). Both
are already no drift today for an enable: rule 8 explains it, and the
class row `plugin-toggle` makes it routine anyway.

## Decision

1. **One routine row, its own rule id.** In ADR-0028 §2, before the
   `plugin-enable`/`plugin-disable` row:

   | Event | Class | Rule / resolution |
   |---|---|---|
   | `plugin-add` / `plugin-enable` of `jax.seldon` (`attribution::OWN_PLUGIN`, the manifest's `id`) | **routine** | `seldon-self`: the user installing Seldon; its first add is part of the documented setup |

   `seldon-self` is a `[drift] routine` rule id like the others: in the
   default list (`ROUTINE_RULES`), in `seldon doctor`'s `drift` row and in
   `doctor --json` `drift.routine`, and left out of a user's list it does
   not apply (the add falls to attention `plugin`, the enable to
   `plugin-toggle`). An enable is classed `seldon-self` rather than
   `plugin-toggle` so `drift show` names the reason; rule 8 still explains
   it (a resolution wins over any class, ADR-0028 §2).

2. **Routine, not explained.** No resolution line is written for the
   add. Routine is a statement about consequence, not intent (ADR-0028
   §3), so F4's objection — an explanation without provenance — does not
   apply. The event stays in the ledger and the Changelog, in
   `drift --all` as `routine · seldon-self`, and in `REBUILD.md`'s plugin
   list.

3. **Unchanged.** `plugin-remove` of `jax.seldon` stays attention
   (`plugin`): somebody took Seldon's panel off. `plugin-update` stays
   rule 8's. pacman `install` and `downgrade` of `jax-seldon` stay as they
   are (no logbook exists before the engine is installed, so the first
   install is never recorded).

## Consequences

- A fresh install by the README leaves `seldon drift` at 0 open items
  after the plugin is added (WP-172 acceptance; the `shell.json` change is
  routine `routine-paths`).
- **Residual risk, accepted.** Nothing checks where a plugin with the id
  `jax.seldon` came from. Code that names itself `jax.seldon` on a machine
  without Seldon's plugin is now routine, not attention: it is not listed
  to agent sessions as an attention item. The gain for an attacker is
  small: whoever can run `omarchy plugin add` as the user can already
  change any third-party plugin in place, and an in-place change of the
  installed `jax.seldon` tree is a `plugin-update`, which rule 8 explains
  since WP-086. Swapping a real Seldon plugin for another still needs a
  `plugin-remove` (attention) unless both happen between two captures, in
  which case the collector sees a tree change (`plugin-update`, rule 8)
  today already.
- Rule ids are an open set in `index.schema.json` (ADR-0038 §1): no
  contract change, no `contractVersion` bump, no fixture change. The
  plugin keeps texts only for crisis rules; a routine rule needs none.

## Alternatives considered

- **Extend rule 8 to the add (an `explained` line).** Rejected: it is
  exactly what F4 removed; a resolution claims a reason the engine cannot
  prove.
- **Routine only for the first add** (no earlier `plugin-add` or
  `plugin-remove` of `jax.seldon` in the ledger); a re-add stays
  attention. Narrows the residual risk to machines that never had the
  plugin, at the cost of a ledger look-up per item. Not taken: a re-add
  follows a removal that is already attention, and the troubleshooting
  path (remove, add again) would ask twice. A reviewer may choose it; the
  row's rule id stays the same.
- **Check the clone's `origin` URL against the plugin repository.** Not
  taken: the URL is the attacker's to write, so it proves nothing, and the
  collector would read `.git/config` for a check that only looks like one.
- **Reorder the README** (plugin before `seldon init`, so the add falls
  into the init baseline). Rejected: without a logbook the plugin shows
  only its *not initialised* banner, so engine, init, plugin stays the
  natural order; and anyone who adds the plugin later (an existing
  logbook, a second machine with a restored logbook) would still meet
  the drift. The classification should not depend on the order of setup
  steps.
