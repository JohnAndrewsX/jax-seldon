```
WP-086 HANDOVER
Done: SPEC-ENGINE §5 rule 8: a capture explains every new event of Seldon's own plugin (`jax.seldon`: plugin-add|update|enable|disable) and own package (`jax-seldon`: install|upgrade|downgrade|reinstall) without a case with one `explained` resolution (source seldon, actor system, detail "seldon's own plugin" / "seldon's own package"); the event keeps its actor and stays in the Changelog; removals stay drift; `capture --json` adds `explainedSelf`, the human capture prints a note; 6 CLI tests + 2 unit tests; 13 mutants, all killed; SPEC §3 §5, CHANGELOG
Not done: no literal `seldon` actor (contract has none, see Decisions); no fixture change (no fixture test needs it)
Verified by: just check exit 0 (`check: ok`, run once after the bounded wait: no plugin harness running); cargo test --test own_changes (6 passed); cargo test --lib own_change (2 passed); full cargo test 0 failures; fmt + clippy --all-targets -D warnings clean
Learned: memory/pitfalls.md, section "WP-086"
Decisions needed: none blocking (actor wording, below)
Touched outside WP scope: engine/src/reconcile.rs (resolutions next to rule 7's), engine/src/commands/capture.rs (one call after rule 7, `explainedSelf` in the output; WP-083 also edits capture.rs: expect a small conflict near `render`)
```

Branch `wp/086-review`, worktree `wt/WP-086`, from `5833c81`. No PR, no
push. Commits:

- `3fb19ae engine: explain Seldon's own plugin and package changes (WP-086)`
- `33eda4e engine: tests for Seldon updating itself (WP-086)`
- `a2f67ef docs: SPEC-ENGINE §5 rule 8 and changelog for own changes (WP-086)`
- `eac674e memory: WP-086 pitfalls (WP-086)`
- this handover

## What was already covered (the WP asked to check)

- **WP-038 covered only files the engine writes itself** under a watched
  path (theme hook, `hook install` settings; `owned.json`, rule 7). It
  did not cover plugin events, and not the engine's own package.
- **The operator's case** (`plugin-update jax.seldon 0.1.0 → 0.1.2` by
  `system` as drift) was uncovered. It is covered now.
- **The engine's package**: a pacman `upgrade jax-seldon` was drift (in
  a `-Syu` group with everything else). It is covered now.
- **`install.sh`** writes under its prefix (`~/.local`: binary, man page,
  completions) and, with `--unit`, into `~/.config/systemd/user/`. None of
  these are in the default `watchPaths`, so it produces no event and
  needs no rule. SPEC §5 rule 8 says so. A path a user adds to
  `watchPaths` stays ordinary config drift.

## The rule and why it looks like this

The same mechanism as WP-038, so the ledger stays truthful and
append-only:

- `attribution::OWN_PLUGIN = "jax.seldon"` and `OWN_PACKAGE =
  "jax-seldon"` are the one constant pair.
  - `attribution::own_change(&Event)` decides if an event is Seldon's own
    change and returns the detail.
  - The test `the_own_ids_are_the_shipped_ones` pins the constants to
    `plugin/manifest.json` `id` and `packaging/PKGBUILD` `pkgname`.
- `reconcile::own_change_resolutions` and `explain_own_changes` build and
  append one resolution per written event. The append happens right
  after the capture's append, under the same lock, after rule 7.
  - This runs on every capture. Only the plugins and pacman collectors
    produce such events, so a capture without them has nothing to
    explain.
  - A failed append is a warning on stderr. The capture does not fail,
    the same as rule 7.
- **Actor.** The event keeps the actor that the collector or the
  attribution pass gave it.
  - An agent's `omarchy plugin update jax.seldon` stays
    `agent:claude-code`, and is still explained (test
    `an_agents_update_of_its_plugin_keeps_the_actor_and_is_no_drift`).
- **Events with a case** are left alone (rule 1/2 linked them already).
- **Removals stay drift.** `plugin-remove jax.seldon` and pacman
  `remove jax-seldon` are not Seldon updating itself: somebody took it
  off the system. The WP goal lists add/update/enable/disable for the
  plugin. I treated the package the same way. See the open questions.
- **pacman groups.** Only the `jax-seldon` member is explained. The
  rest of the transaction stays one drift item (the index already
  regroups around resolved members, ADR-0013 §4). The test checks the
  shared `txId` and that only `zed` is left as drift.
- **Idempotency** is unchanged:
  - a second capture writes 0 events and 0 resolutions, and the ledger
    length is the same (plugins and pacman tests);
  - the plugins collector's `unrecorded` dedupe means a repeated diff
    never produces a second event to explain.

## Tests

`engine/tests/own_changes.rs` (new, `common::Env`, stubbed `omarchy`,
temp pacman log, `SELDON_NOW`):

- `updating_its_own_plugin_is_no_drift_another_plugin_is`:
  - the operator's case, with `jax.seldon` 0.1.0 → 0.1.2 next to a
    third-party bump;
  - written 2, `explainedSelf` 1, `explainedOwn` 0;
  - the event has actor `system` and detail `0.1.0 → 0.1.2`, and one
    resolution;
  - the other plugin has no resolution and is the only drift row;
  - the index validates, with `openDrift` 1 and the row `resolution:
    explained`, `resolutionDetail: seldon's own plugin`;
  - a second capture writes nothing.
- `adding_enabling_disabling_its_plugin_is_no_drift_removing_it_is`.
- `an_agents_update_of_its_plugin_keeps_the_actor_and_is_no_drift`.
- `upgrading_its_own_package_is_no_drift_another_package_is`:
  - a `-Syu` with `jax-seldon` + `zed`, then idempotent;
  - then `pacman -R jax-seldon` is drift.
- `the_human_capture_says_what_it_explained`.
- `the_own_ids_are_the_shipped_ones`.

Unit tests:

- `attribution::tests::own_changes_are_seldons_plugin_and_package_but_no_removal`:
  every kind, removals, look-alike ids, the names in the wrong source.
- `reconcile::tests::seldons_own_changes_without_a_case_are_explained`:
  the line shape, an agent actor explained, a cased event skipped.

## Mutants (one per claim; each applied to the committed tree, `cargo test --lib own_change` + `--test own_changes`, reverted)

| # | Mutant | Claim | Result |
|---|--------|-------|--------|
| M1 | plugin arm returns `None` | own plugin update is explained | killed (4 CLI tests + unit) |
| M2 | drop `subject == OWN_PLUGIN` | another plugin stays drift | killed |
| M3 | add `PluginRemove` to the kinds | plugin removal stays drift | killed |
| M4 | package guard `if false` | own package upgrade is explained | killed |
| M5 | package guard `if true` | another package stays drift | killed |
| M6 | add `Remove` to the package kinds | package removal stays drift | killed |
| M7 | case filter → `true` | cased events are left alone | killed (unit test) |
| M8 | capture passes `&[]` | capture applies the rule | killed (4 CLI tests) |
| M9 | capture explains twice | one resolution per event | killed (4 CLI tests) |
| M10 | note text changed | human output names it | killed |
| M11 | `OWN_PLUGIN = "jax.seldon.x"` | the constant is the shipped id | killed |
| M12 | only actor `system` explained | an agent's update is explained too | killed (unit test) |
| M13 | `OWN_PACKAGE = "jax-seldon-bin"` | the constant is the shipped pkgname | killed |

I re-ran M7, M8, M9 and M12 with their output. Each fails on test
assertions, not on compile errors.

## Decisions needed

- **Actor wording (same as WP-038).** The WP says "`actor: seldon` (or
  the actor WP-038 uses)".
  - The contract's actor pattern is `human|system|agent:<slug>`, so I
    used WP-038's way: a `source: seldon` resolution, and the event
    keeps its actor.
  - A visible `seldon` actor would need an ADR and a `contractVersion`
    bump.
  - Recommendation: no.

## Open questions

- **Removal of Seldon.** It stays drift, both `plugin-remove jax.seldon`
  and `remove jax-seldon`. If the operator wants uninstalling Seldon
  explained too, it is one kind each in `own_change` plus a test flip.
- **Other package names.** Only `jax-seldon`, the PKGBUILD name, is
  covered. A future `jax-seldon-bin` or `-git` would need adding to the
  constant (deliberately not guessed).
- **Harness wait (process note for the orchestrator).**
  - A plain `pgrep -af tests/plugin` loop matches the command lines of
    other sessions' wait loops, and its own.
  - One sibling loop (PID 241247, `until ! pgrep -af
    "tests/plugin/(…)"`) matches itself. It can only end by its
    `timeout 1500`.
  - I waited with `pgrep -f '^(/usr/bin/)?(bash|sh|node|python3)
    [^ ]*tests/plugin/'`. That pattern matches only running harness
    scripts. Nothing was running, and `just check` ran once, exit 0.

## Notes

- No guard block happened.
- I did not touch the real host: no `~/Seldon`, no real `~/.config` or
  `~/.local/state`. All tests used scratch dirs.
- Plugin: `plugin/README.md` names no drift reasons that need changing,
  so `plugin/` is untouched (WP-085 owns it).
- CHANGELOG `[Unreleased]` and `memory/pitfalls.md` appends will
  conflict with the sibling WPs, as expected.
