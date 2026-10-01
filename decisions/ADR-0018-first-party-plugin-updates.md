# ADR-0018 — No `plugin-update` events for first-party plugins

**Status:** accepted (clarifies ADR-0014 §3)
**Date:** 2026-10-01

## Context
ADR-0014 §3 says `plugin-update` fires when a plugin's manifest version or
git HEAD changes. WP-005 found that every first-party Omarchy plugin
carries version `1.0.0` in its manifest and ships inside the `omarchy`
package. An Omarchy release that bumps them would produce about 37 yellow
`plugin-update` drift items next to the one red `omarchy` `update` event;
ADR-0013 groups pacman transactions only, so nothing would fold them.

## Decision
`plugin-update` is emitted only for plugins with `firstParty: false`
(third-party installs and clones). First-party plugins are part of the
`omarchy` package; their version changes are covered by the `omarchy`
`update` event and are tracked only in the plugins cursor. `plugin-enable`
and `plugin-disable` still fire for first-party plugins (a user action).

## Consequences
Five lines in the plugins collector (WP-005); SPEC-ENGINE §4 names the
rule. A later Omarchy that ships plugins as separate packages would come
through pacman anyway.
