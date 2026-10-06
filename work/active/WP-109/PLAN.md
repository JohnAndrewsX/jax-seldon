# WP-109 — Plan

Engine side of ADR-0028 (WP-A). Normative: the §2 table. Small commits in
this order; `flock /tmp/seldon-check.lock just check` at the end.

## 1. Classification core (`engine/src/index/class.rs`, new)

- `Class { Routine(rule), Attention(rule?), Crisis }`, ordered
  routine < attention < crisis; a group's class is the max of its open
  members; a dependency of a named transaction follows the max of the
  transaction's explicit members (read from every member, open or not).
- `Rules` compiled once per build from `[drift]`: `alwaysRed` (existing
  lazy globs), `routinePackages`, `routinePaths` and `alwaysRedPaths`
  (the `skipPaths` glob semantics, applied to `~`-path subjects), the
  enabled routine rule ids, `attention = normal | all`.
- One function per §2 row family: pacman member (plain full upgrade by
  the shared argv parser incl. `-Syyuu`, `-Su`, bare `yay`; `:: Replace`
  removals; `-U <cache>`; keyring), omarchy `update` (package-shaped
  versions + an `omarchy`/`omarchy-dev` install/upgrade with the same
  version in a plain full upgrade, ≤ 31 days before), plugins, theme,
  config (marks, routinePaths, alwaysRedPaths, theme rules of the WP's
  Omarchy-first section, `config-remove`), total row = attention.
- Rule ids: `sysupgrade`, `upgrade`, `keyring`, `omarchy-update`,
  `plugin-toggle`, `theme`, `omarchy-default`, `system-link`,
  `routine-paths`, `theme-assets`, `theme-repo`; attention labels
  `sysupgrade-red`, `upgrade-red`.
- `attention = "all"`: the pre-ADR derivation unchanged (computed pacman
  zone, crisis iff red) — the rollback of §5.

## 2. Index build (`build.rs`) and the drift model

- Items built from the *linkable* set (eligible, no case, no resolution),
  each with class and rule; `index.drift` = non-routine items, plus
  routine items an open case's Plan names (shown as attention with
  `proposedCase`); `crisis` = class crisis; `zone` = ledger zone of the
  lead. `Built` gains `linkable` and per-item class; `open_drift` keeps
  meaning "listed as drift".
- `series.drift`: opened = non-routine items; a resolution counts only if
  its target's item was counted as opened (never negative).
- Users of `open_drift`/`is_open_drift` (views, REBUILD.md, setup) follow.

## 3. Commands (`commands/drift.rs`, `reconcile.rs`)

- `drift --all` lists routine items too (uncapped, from folded events);
  `drift show` reports `class` and `rule`.
- `drift link` resolves routine (linkable) events; `explain|dismiss` of a
  routine event exits 1 "routine".
- Agent actor: `explain|dismiss` of a crisis exits 1; `link` of a crisis
  only to an *active* case whose `agents` lists the actor. Humans never
  refused.

## 4. Capture

- Config collector: `meta.matches = "omarchy-default"` (new content equals
  `$OMARCHY_PATH/config/<rel>`, `applications/<name>`,
  `default/alacritty/Alacritty.desktop`) and `"system-link"` (symlink
  resolving under `/usr/`), and `"theme-repo"` (file of a theme directory
  with `.git`; WP Omarchy-first section) on new add/change events only.
- Rule 7 evidence: built-in templates (theme hook script; watcher unit
  with any `ExecStart` prefix) explain matching config events even
  without `owned.json`.
- `[drift]` new keys with defaults; six new default `watchPaths`; an
  existing config whose list equals the previous default gains them once
  (capture says so); `doctor` prints the effective `[drift]` set, marks
  non-defaults, names missing default watch paths of a user-changed list.

## 5. Reference, fixture, docs

- `scripts/validate-fixtures.py`: port the rules (`routine()`, crisis,
  zone, series) and keep the legacy derivation for `attention = "all"`.
- Fixture rows of ADR §8: crisis on a hook path (yellow zone), attention
  with and without `proposedCase`, `plugin-enable`, a downgrade, a
  `config-remove`, routine `theme-set`, `-Syu` group and `shell.json`
  change as plain events; golden pinned to the default config; an
  `attention = "all"` golden derived by the legacy path.
- Plugin harness expectations that pin fixture counts (`tests/plugin`)
  follow the fixture (counts only; `plugin/` untouched).
- SPEC-ENGINE §2/§4/§5/§6, schema *description* of `crisis` (and of
  `zone`, which named the computed yellow), CHANGELOG.

## 6. Tests

Table-driven test over every §2 row (incl. `-Syyuu`, `:: Replace`,
`config-remove`, the line of `omarchy-update-system-pkgs`); two builds
byte-identical; zero ledger writes; migration fixture (open
theme/toggle/update/`-Syu` → `drift` empty, ledger byte-identical, old
resolutions fold, series ≥ 0); `attention = "all"` = pre-ADR golden;
watch-list upgrade; agent refusal; manual mutants; `just check`,
`just check-perf`.
