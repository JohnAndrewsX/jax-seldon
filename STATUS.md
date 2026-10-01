<!-- maintained by the orchestrator; regenerate the WP table from work/ -->
# STATUS.md — Seldon

**Phase:** 0 — Foundation (prepared, not started)
**Contract version:** 1 (draft)
**Last updated:** 2026-10-01

## Active work packages
*(none)*

## Queued (next up)
WP-001 repo scaffold · WP-002 contract & fixtures · WP-003 engine core
(WP-001 … WP-009 are written out in `work/queued/`)

## Preparation done (2026-10-01)
- Kickoff checklist steps 1–3: repo + first commit, WP files, host verified
  (`memory/host.md`, `memory/omarchy-shell.md`, `memory/local.md`).
- Dev host toolchain: rustup + musl target, just, qmllint. Test host: cargo
  (glibc), qmllint. GitHub remote: `JohnAndrewsX/jax-seldon` (private).
- How to run the team: `docs/HERDR-SETUP.md`.

## Blocked
*(none)*

## Recently completed
*(none)*

## Open questions for the operator
- Default logbook path: `~/Seldon` (proposed) — confirm.
- Binary name `seldon` vs `jax-seldon` (ADR-0001 proposes `seldon`, with a
  `jax-seldon` symlink from the package).
- **Plugin git root.** `omarchy plugin add` clones a URL and expects
  `manifest.json` at the repo root; ours is in `plugin/`. Options:
  (a) separate repo `JohnAndrewsX/omarchy-seldon` holding only the plugin,
  (b) monorepo plus a published `plugin` branch via `git subtree split`,
  (c) move the plugin to the repo root. ADR-0004 leans to (a). Needs an ADR.
- **snapper permissions.** `snapper list` needs root or `ALLOW_USERS` in
  `/etc/snapper/configs/root`. Collector degrades gracefully either way;
  the question is whether the operator wants to grant the user account that access.
- **Omarchy is a package install, not a git checkout.** Version comes from
  `omarchy-version`; WP-033 (update-impact from release notes) needs
  another source. Decide in Phase 3.
- **Sample index data.** The event list in `fixtures/index.sample.json` is
  partly generated filler (e.g. `pacman upgrade` with subject `110`,
  `snapper snapshot` with a config path). WP-002 rewrites it from the
  fixture logbook.
