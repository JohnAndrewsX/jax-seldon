# ADR-0059 — ADR-0028 amended: what Omarchy's commands write to files it ships no copy of is routine `omarchy-written`

**Status:** accepted (E96 delegation, orchestrator 2026-10-14; operator
confirms; stage 2 with the 0.4.1 release). WP-225, from the E92 edge
survey, rows B9 and B10. Condition: the trust rule of §1 stays pinned by
`engine/tests/collector_hashes.rs` `omarchy_written_files_are_routine`
(a group-writable `default/i18n`, `locales.json`,
`bin/omarchy-default-agent` or `bin/omarchy-install-dictation-<backend>`
gives no mark; review F1). The number follows WP-227's ADR-0056 to
ADR-0058.
**Date:** 2026-10-14

> Amends [ADR-0028](ADR-0028-attention-by-consequence.md) §2 and §5 by
> one evidence row, as
> [ADR-0037](ADR-0037-adr-0028-amendment-toggles-and-links.md) and
> [ADR-0050](ADR-0050-adr-0028-amendment-seldon-self.md) did. ADR-0028
> stays accepted and unedited; its three tests of §1, its other rows and
> §3–§7 stand. ADR-0037's toggle rows stand.

## Context

Omarchy edge (the E92 survey, 2026-10-14, read from `$OMARCHY_PATH` on
the dev host) writes files of its own under Seldon's default watch paths
that it ships no copy of:

- `omarchy language <code>` (Setup > Language) writes `<code>\n` to
  `~/.config/omarchy/language` and
  `# The session language, set by omarchy-language (Setup > Language).\nexport LANG=<locale>\n`
  to `~/.config/uwsm/env.d/50-omarchy-language`, which uwsm sources at
  login. The code and the locale come from
  `$OMARCHY_PATH/default/i18n/locales.json`.
- `omarchy default agent <agent>` writes `<agent>\n` to
  `~/.config/omarchy/defaults/agent` (migration 1786719479 rewrites
  `gemini` as `agy`); the agents are the `# omarchy:args=[…]` line of
  `bin/omarchy-default-agent`.
- `omarchy dictation use <backend>` writes `<backend>\n` to
  `~/.config/omarchy/defaults/dictation` (migration 1791479273 writes
  `voxtype`); a backend has an installer
  `bin/omarchy-install-dictation-<backend>`. Omarchy ships a copy of this
  file (`superwhisper`), so that content is already `omarchy-default`.
- `omarchy crash mute <program>` and `omarchy toggle crash capture` go
  through `omarchy-toggle`: empty flags
  `~/.local/state/omarchy/toggles/crash-ignore/<name>` and
  `toggles/crash-capture-off`.

Under ADR-0028 §2 the first three are attention `config` — the user
changed the interface language and Seldon asks why — and the session
file is a **crisis**: it lies under the persistence path
`~/.config/uwsm/**`. The crash flags are empty and already routine
`toggle-flag` (ADR-0037 §1) both ways; they need no new row, only the
attribution below.

A path rule (`routinePaths`) would make any content there routine,
including an `export LD_PRELOAD=…` in the session file. What makes these
changes routine is their content: exactly what Omarchy's command writes,
built from Omarchy's own data.

## Decision

1. **One evidence row, its own rule id.** In ADR-0028 §5 a new
   `config-add` or `config-change` carries `meta.matches =
   "omarchy-written"` when its new hash equals one of the contents
   Omarchy's commands write to that path, as listed above, each built
   from `$OMARCHY_PATH` alone (a code and locale of `locales.json`, an
   agent of the args line, a backend with an installer), never from the
   file itself. The trust rule of `omarchy-default` (WP-109 round 1b)
   applies to every file read for it: owned by root, neither group- nor
   world-writable, and every directory on the way. Only the fact is
   recorded. The mark states content equality, not who wrote the file:
   a hand edit to exactly Omarchy's content carries it too, which is
   right by consequence (it equals the menu action), and anything else
   there keeps its class.

   In ADR-0028 §2, next to the `omarchy-default` row and before the
   persistence paths:

   | Event | Class | Rule / resolution |
   |---|---|---|
   | `config-add` / `config-change` with `meta.matches = omarchy-written` | **routine** | `omarchy-written`: the content Omarchy's own command writes there |

   `omarchy-written` is a `[drift] routine` rule id like the others: in
   the default list (`ROUTINE_RULES`), in `seldon doctor`'s `drift` row;
   left out of a user's list it does not apply and the event falls to
   the next row (the session file to `always-red-paths`).

2. **Attribution.** An agent's `omarchy language <code>`, `omarchy crash
   mute [--] <program>`, `omarchy toggle crash capture`, `omarchy-toggle
   <flag>`, `omarchy default agent <agent>` and `omarchy dictation use
   <backend>` (and each as its script) name the files they write
   (ADR-0014 §1 proof), so their config events take the agent's actor and
   case.

3. **Unchanged.** A removal of these files stays attention
   `config-remove` (Omarchy never removes them). Any other content stays
   where ADR-0028 puts it: a line more in the session file is a crisis.

## Consequences

- Changing the language from Omarchy's menu, picking a default agent or
  a dictation backend, and muting a crash leave `seldon drift` at 0 open
  items. The events stay in the ledger as `routine · omarchy-written`
  (or `toggle-flag`).
- **Assumptions about Omarchy:** the session file's exact text, the
  `# omarchy:args=` line and the installer names. When Omarchy changes
  one, the evidence stops matching and the event falls back to attention
  (or crisis): fail-safe. The assumption ledger (WP-224) should name
  them.
- **Residual risk, accepted.** A default agent is a command name Omarchy
  launches; the evidence accepts only names Omarchy's own script lists.
- No schema change: `meta.matches` is engine-internal; contractVersion
  stays.
- SPEC-ENGINE §4 (the marks, attribution) and §5 (the order of rows);
  the user guide's `[drift] routine` list; `scripts/validate-fixtures.py`.
- Rollback: drop `omarchy-written` from `[drift] routine`.

## Alternatives considered

- *`routinePaths` defaults for the four files:* any content would be
  routine, the session file included (a persistence path); rejected.
- *Reuse `omarchy-default`:* its meaning is "equals the shipped copy";
  `drift show` would claim a copy that does not exist.
- *Read the files and check their shape:* the hash the collector has is
  enough when the candidates are built from Omarchy's data.
