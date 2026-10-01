<!-- maintained by the orchestrator; regenerate the WP table from work/ -->
# STATUS.md — Seldon

**Phase:** 0 — Foundation (running, tick 1 started 2026-10-01)
**Contract version:** 1 (draft)
**Last updated:** 2026-10-01

## Active work packages
| WP | Title | Role | Worker | Worktree | Since |
|---|---|---|---|---|---|
| WP-004 | Collectors: pacman, snapper, omarchy (+ event model, ledger, collector registry) | Engine | `engine-004` (opus, high) | `wt/WP-004` · `wp/004-collectors-core` | 2026-10-01 |
| WP-006 | `log`, `event`, `plan` commands, case state machine, journal | Engine | `engine-006` (opus, high) | `wt/WP-006` · `wp/006-commands` | 2026-10-01 |
| WP-011 | Panel: Today, Changelog, System tabs | Plugin | `plugin-011` (opus, high) | `wt/WP-011` · `wp/011-panel-tabs` | 2026-10-01 |

## Queued (next up)
After WP-003: WP-004, WP-005, WP-006, WP-024 in parallel (own module paths)
· then WP-007, WP-009 · then WP-008. Plugin track: WP-011 after WP-010.
(see `work/queued/`)

## Preparation done (2026-10-01)
- Kickoff checklist steps 1–3: repo + first commit, WP files, host verified
  (`memory/host.md`, `memory/omarchy-shell.md`, `memory/local.md`).
- Dev host toolchain: rustup + musl target, just, qmllint. Test host: cargo
  (glibc), qmllint. GitHub remote: `JohnAndrewsX/jax-seldon` (private).
- How to run the team: `docs/HERDR-SETUP.md`.

## Blocked
*(none)*

## Recently completed
- 2026-10-01 WP-010 Plugin skeleton — merged; Service state machine, pill,
  banners, headless harness (48 checks), token check; smoke-tested on the
  test host; ADR-0016 (install via AUR helper); plugin test expectations
  updated to the 4-item drift fixture at merge.
- 2026-10-01 WP-003 Engine core — merged; config, logbook layout, lossless
  frontmatter, typed models, `init` wizard, `doctor`; 71 tests; specs
  amended (SPEC-ENGINE §2 §3 §9, SPEC-LOGBOOK §6).
- 2026-10-01 WP-014 Contract v1 follow-ups — merged; drift group fields,
  `resolutionDetail`, open `-Syu` group in the sample index (4 drift items),
  token rule per ADR-0015; `just check` green on main.
- 2026-10-01 WP-002 Contract and fixtures — merged `wp/002-contract-fixtures`;
  `just check` incl. schema-validate green on main; ADR-0012 accepted with
  review edits; ADR-0013 (drift grouping, debate) and ADR-0014 (attribution,
  zones) decided; follow-ups in WP-014.
- 2026-10-01 WP-001 Repo scaffold, CI, toolchain — merged `wp/001-scaffold`;
  `just check` green on main; handover in `work/completed/WP-001/`.

## Decided 2026-10-01
- Default logbook path `~/Seldon`, wizard options → ADR-0010.
- Binary name `seldon` with `jax-seldon` symlink → ADR-0001 confirmed.
- Monorepo `jax-seldon`, installable plugin published as
  `jax-seldon-plugin` via subtree split → ADR-0009.
- Snapper collector degraded by default, user opts in with one command
  → ADR-0011.

## Open questions for the operator
- **AGENTS.md §7 exit codes** say 0/1/2/3; SPEC-ENGINE §3 and the code add
  `4 lock held`. AGENTS.md needs a one-line fix on operator instruction.
- **`scripts/guard.sh` false positives** (operator-owned): the "write under
  /etc, /usr" rule fires when such a path is a read-only *source* argument;
  the package-manager rule fires on any command text that merely mentions
  the package manager (e.g. a heredoc writing a CI file). Both are
  trade-offs for the operator; the WP-001 review has concrete proposals.
- **`serde_yaml` is deprecated upstream** (0.9.34+deprecated). It works and
  the YAML surface is flat, but AUR reviewers may flag it. Before WP-040,
  AGENTS.md §7 should allow a maintained fork with the same API
  (`serde_yaml_ng`); one-commit swap. Operator instruction needed for the
  AGENTS.md line. `rust-version` was raised to 1.89 for `File::try_lock`
  (both hosts have 1.98) — orchestrator approved.
- **CI may fetch `omarchy-plugin-validate`** from a pinned Omarchy tag with
  a checksum (network in CI, not red zone). FYI; scheduled unless vetoed.
- **Omarchy is a package install, not a git checkout.** Version comes from
  `omarchy-version`; WP-033 (update-impact from release notes) needs
  another source. Decide in Phase 3.
- **Sample index data.** The event list in `fixtures/index.sample.json` is
  partly generated filler (e.g. `pacman upgrade` with subject `110`,
  `snapper snapshot` with a config path). WP-002 rewrites it from the
  fixture logbook.
