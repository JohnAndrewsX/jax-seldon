<!-- maintained by the orchestrator; regenerate the WP table from work/ -->
# STATUS.md — Seldon

**Phase:** 0 — Foundation (running, tick 1 started 2026-10-01)
**Contract version:** 1 (draft)
**Last updated:** 2026-10-01

## Active work packages
| WP | Title | Role | Worker | Worktree | Since |
|---|---|---|---|---|---|
| WP-008 | Reconciliation and `drift` commands (+ `open --editor` fix) | Engine | `engine-008` (opus, high) | `wt/WP-008` · `wp/008-drift` | 2026-10-01 |
| WP-013 | Engine ↔ plugin integration test on a real logbook | QA | `qa-013` (opus, medium) | `wt/WP-013` · `wp/013-integration` | 2026-10-01 |
| WP-020 | Work tab: three-column list, case card, new-case sheet | Plugin | `plugin-020` (opus, high) | `wt/WP-020` · `wp/020-work-tab` | 2026-10-01 |

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
- 2026-10-01 WP-009 Hooks (Claude Code PreToolUse, generic, session
  start/stop, `hook install`), shared command parser `pkgcmd.rs`, shared
  attribution pass, green zone per ADR-0019 — merged after one review
  round; 277 engine tests; hook under 2 ms in release.
- 2026-10-01 WP-012 Panel actions: QuickEntry, Capture now, Open in
  editor; harness HOME isolation and real-dir guard — merged; smoke on the
  test host with the real engine found the `open --editor` launcher bug
  (fix in WP-008).
- 2026-10-01 WP-007 Index builder, `index`/`status`, ledger views,
  STATUS.md, drift cap (ADR-0020), fast rebuild — merged; golden test
  zero differences against the fixture; ×10 build 4.7 ms, ×150 78 ms.
- 2026-10-01 WP-015 Fixture corrections — merged; verification step for
  C-2026-001, green `tee` event, pre/post snapshot pair, three index
  variants, case walker and index-time checks (101 instances, 71 events).
- 2026-10-01 WP-011 Panel tabs Today/Changelog/System, crisis strip,
  snapper banner, keyboard per §5, panel harness (86 checks), event-driven
  test waits, three-theme sweep on the test host — merged.
- 2026-10-01 WP-005 Collectors plugins/theme/config, SHA-256 in `sys`,
  theme-set hook script — merged; ADR-0018 applied; 21 collector tests;
  real-host read-only run: 38 plugins, 25 config files, second run 0.
- 2026-10-01 WP-006 `log`, `event`, `plan`, `decide`, `open`; case state
  machine; journal; `--config`; `--` forms — merged; 130 engine tests;
  ledger-first plan steps; specs amended (SPEC-ENGINE §3, SPEC-LOGBOOK §3).
- 2026-10-01 WP-004 Collectors pacman/snapper/omarchy + event model, ledger,
  redaction, collector registry, `capture` — merged after two review rounds
  (attribution per ADR-0017, Running-line rewind, greedy userinfo
  redaction, capture-time dedupe); 109 engine tests; real-host read-only
  capture 1230 events, second run 0.
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
- **Test host is locked** (since 15:03, a stranded lock after a shell
  crash during a plugin reload, now hardened in the e2e script). Please
  unlock it and enable stay-awake there (`omarchy-toggle-idle`) or raise
  `idle.lock` in its shell.json, so unattended e2e runs work. Then WP-013
  re-runs the full e2e twice (G2 gate).
- **ssh alias `test`**: docs and the e2e script default to `ssh test`; this
  dev host has no such alias. Add `Host test` to `~/.ssh/config` (name in
  `memory/local.md`) or keep passing `SELDON_TEST_HOST=<alias>`.
- **Phase 0 exit**: `work/PHASE-0-EXIT.md` has the procedure; it needs you
  (real logbook, `~/.config/seldon`).
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
- **AGENTS.md §7** lists `index.json` among the generated files that carry
  the `<!-- generated by seldon; do not edit -->` header; a closed-schema
  JSON file cannot carry one. One-word fix (strike `index.json`) on
  operator instruction.
- **CI may fetch `omarchy-plugin-validate`** from a pinned Omarchy tag with
  a checksum (network in CI, not red zone). FYI; scheduled unless vetoed.
- **Omarchy is a package install, not a git checkout.** Version comes from
  `omarchy-version`; WP-033 (update-impact from release notes) needs
  another source. Decide in Phase 3.
- **Sample index data.** The event list in `fixtures/index.sample.json` is
  partly generated filler (e.g. `pacman upgrade` with subject `110`,
  `snapper snapshot` with a config path). WP-002 rewrites it from the
  fixture logbook.
