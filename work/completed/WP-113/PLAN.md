# WP-113 — plan

Branch `wp/113-collector-hashes` from `next` (38a9103), merged into `next`.
Inputs read: AGENTS.md §6/§7, ADR-0028 (all; §2 rows, §4d, §6, §8 WP-E),
work/queued/WP-113.md incl. both *Added* sections and the *0.2.0 form*,
`engine/src/collectors/{plugins,config}.rs`, `engine/src/index/class.rs`,
`engine/src/config.rs`, `engine/src/pkgcmd.rs`, SPEC-ENGINE §4,
Omarchy's tree (`bin/omarchy-hyprland-toggle`, `default/hypr/toggles.lua`,
`default/hypr/toggles/`).

## Line items

1. **Plugin trees** (`~/.config/omarchy/plugins/<id>/`, third-party only).
   The plugins collector hashes each listed third-party plugin's directory
   as one tree; a tree-hash change is **one `plugin-update`** (with the
   version change when both move). Hashes only.
2. **Toggles directory** `~/.local/state/omarchy/toggles/**`: a new
   default watch path (config collector, hashes only); Omarchy's toggle
   copies get `omarchy-default` evidence.
3. **`~/.ssh/authorized_keys`**, opt-in: watched only when the user adds it
   to `watchPaths`; then a write is a crisis (default `alwaysRedPaths`
   gains the path, inert while unwatched).
4. **Hook-path blind spots** (WP-109 stage 1): under `alwaysRedPaths`, a
   symlinked directory is followed (loop-guarded), a binary file and a
   file over 1 MiB are hashed (streamed) — each yields its event.
5. **`system-link` narrowing** (WP-109 stage 2): changes an ADR-0028 row →
   proposed amendment note only, not implemented (operator brief).
6. **Baseline hash list** for persistence paths (WP-109 stage 2,
   "consider"): considered, decided below.
7. **yay/paru value options**: extend `LONG_WITH_ARG`.
8. **Fixture**: one plugin tree change in the sample logbook/index.
9. **Capture cost** before/after: a new ignored bench test in
   `just check-perf`.

## Decisions

- **D1 — a tree change is a `plugin-update` of the plugins collector,
  not a `config-change` of the config collector.** ADR-0028 §8 WP-E and
  the WP goal say "one `plugin-update` on a tree-hash change"; the 0.2.0
  form's fixture line says `config-change`. I follow the ADR because
  (a) a `git pull` of a plugin moves both its HEAD (version) and its tree:
  two collectors would give two events for one change, one collector
  gives one; (b) §5 rule 8 auto-explains `plugin-*` of `jax.seldon`, so
  the dev install and Omarchy-plugin-system updates of Seldon's own plugin
  stay quiet — a `config-change` of `~/.config/omarchy/plugins/jax.seldon`
  would be open attention on every deploy; (c) ADR-0028 §2 already has the
  row (`plugin-update`, third-party → attention) — no row changes. The
  fixture therefore gains a tree `plugin-update`, not a `config-change`.
- **D2 — class unchanged: attention.** The 0.2.0 form says the fixture
  event should show "as routine" in the Changelog. A routine class would
  change the ADR-0028 `plugin-*` row (F4 deliberately kept in-place edits
  of unsandboxed code visible). The fixture event is therefore *explained*
  (resolved), so it renders as an ordinary Changelog row and adds no open
  drift item.
- **D3 — what a tree is.** Every regular file below `<plugins dir>/<id>/`
  at any size and content (a QML plugin may ship a `.so`), `.git`
  excluded (the HEAD is the version), `[redaction] skipPaths` honoured
  (the user's escape hatch for a plugin that writes state into its own
  tree), linked files followed, linked directories followed with a loop
  guard (canonical ancestors) and the walker's depth limit. Tree hash =
  SHA-256 over the sorted lines `<relative path> NUL <file sha256> LF`.
  The cursor keeps a stat fingerprint per tree (size, mtime, ctime, inode
  of every file, the WP-069 rule incl. the 2 s racy window): an unchanged
  fingerprint reuses the tree hash without reading a byte.
- **D4 — tree baseline.** An old cursor (no tree hash) and a newly listed
  plugin record the tree without an event; a tree that cannot be read this
  time keeps the last hash (like the version). First-party plugins are
  never tree-hashed (ADR-0018: the `omarchy` package covers them).
- **D5 — event shape.** Tree-only change: `plugin-update`, `meta.hashFrom/
  hashTo` = tree hashes, `meta.from/to` absent, `detail` `files changed
  (sha256 <8> → <8>)`. Version and tree: `from/to` and `hashFrom/hashTo`,
  detail `<from> → <to>` as today. `ts` = latest mtime in the tree clamped
  to [last check, now]. `hashFrom/hashTo` are existing conventional meta
  keys (open `meta` object): no schema change; the schema's description of
  the conventional keys names them for `config-*` only — named as an
  overlap for WP-120 (cosmetic, not done here).
- **D6 — toggles evidence.** `omarchy-hyprland-toggle <flag> on` copies
  `$OMARCHY_PATH/default/hypr/toggles/<flag>.lua` to
  `~/.local/state/omarchy/toggles/hypr/<flag>.lua`; `off` deletes it. The
  `omarchy-default` evidence gains the mapping
  `~/.local/state/omarchy/toggles/<app>/<rel>` ↔
  `$OMARCHY_PATH/default/<app>/toggles/<rel>` (an evidence rule, which
  §8 allows WP-E without a row change). Unknown content there stays
  attention (`config` row; Lua loaded at login like `~/.config/hypr/**`).
  *Toggle off* is a `config-remove` = attention by the existing row; making
  it routine changes a row → **proposed amendment B** (note below). The
  capture records the evidence now (`meta.matches = omarchy-default` on a
  `config-remove` in the toggles directory whose `hashFrom` is the shipped
  copy), because the ledger is append-only and a mark cannot be added
  later; the classifier ignores marks on removals until B is accepted.
- **D7 — `authorized_keys`.** Opt-in = the user adds `~/.ssh/authorized_keys`
  to `watchPaths` (doctor and SPEC say how). Default `alwaysRedPaths`
  gains `~/.ssh/authorized_keys` so that the one line suffices; while
  unwatched the glob matches nothing. Operator-approved class (0.2.0 form:
  "its change is a crisis … when no case covers it"); recorded in the
  amendment note as a default-list addition already approved.
- **D8 — blind spots apply to the configured `alwaysRedPaths`**, at
  capture. A file that moves between `skipped` and hashed is no event
  either way (existing rule), so the upgrade itself writes nothing.
- **D9 — baseline hash list: not built.** Files present when a
  persistence path first becomes watched (`init`, the §4d upgrade, a user
  adding a path) yield no event by design (a baseline is no change; Omarchy
  ships three autostart entries, which would otherwise be crises at
  install). Proposal for a later WP, not done: `doctor` names
  persistence-path files that no ledger event ever recorded.
- **D10 — `system-link` narrowing: proposed amendment A**, not
  implemented: the narrowing changes when the capture writes the mark,
  i.e. the ADR-0028 `system-link` row.

## Overlaps

- **WP-120** (contract v2, fixture v2): `fixtures/logbook/ledger/*`,
  `fixtures/index.sample.json`, `fixtures/index.attention-all.json`,
  `fixtures/index-variants/*`, `fixtures/README.md` counts — the fixture
  commit is separate and last, so it can be rebased onto WP-120's fixture.
  Schema: untouched (D5 note).
- **WP-114** (`/etc/pacman.conf` hashes): may touch `config.rs`/collector
  registration; none of my changes touch the pacman collector.

## Verification

Unit and integration tests per item (temp HOME, TZ set where times
matter), idempotency (second capture: no events) per item, `just
check-perf` with the new capture-cost test (numbers before/after in
HANDOVER), `flock /tmp/seldon-check.lock just check`.
