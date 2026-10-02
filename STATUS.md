<!-- maintained by the orchestrator; regenerate the WP table from work/ -->
# STATUS.md — Seldon

**Release:** v0.1.0 on GitHub (2026-10-02); AUR pending registration.
**Phase:** 0 exited — **gate G3 passed 2026-10-02** (operator's run on
the dev host, `work/completed/PHASE-0-EXIT-transcript.md`). Phases 1–3
complete and live-verified (G2 passed), Phase 4 packaging and docs merged.
Open: WP-042 marketplace submission (after the operator's release setup),
WP-038 (engine attributes its own installs), WP-033 and WP-043 (operator
decisions).
**Contract version:** 1 (draft)
**Last updated:** 2026-10-01

## Active work packages
| WP | Title | Role | Worker | Worktree | Since |
|---|---|---|---|---|---|
| WP-043 | Import the operator's omarchy-agent vault (dry run first) | Engine | `engine-043` (opus, high) | `wt/WP-043` · `wp/043-vault-import` | 2026-10-02 |

## Queued (next up)
WP-033 (update-impact, needs the operator's release-notes source decision)
· WP-042 (marketplace submission, after WP-040/041) · WP-043 (optional
logbook migration)
· WP-040/041 (packaging, docs) after WP-031. (see `work/queued/`)

## Preparation done (2026-10-01)
- Kickoff checklist steps 1–3: repo + first commit, WP files, host verified
  (`memory/host.md`, `memory/omarchy-shell.md`, `memory/local.md`).
- Dev host toolchain: rustup + musl target, just, qmllint. Test host: cargo
  (glibc), qmllint. GitHub remote: `JohnAndrewsX/jax-seldon` (private).
- How to run the team: `docs/HERDR-SETUP.md`.

## Blocked
*(none)*

## Recently completed
- 2026-10-02 WP-039 Panel 460 wide, tabs sized to their labels, chips
  wrap, Changelog header keeps the sort word, fit assertions at scale
  1.0/1.25, real-home guard recognises the operator's live engine,
  preview refreshed — merged (operator's live observation).
- 2026-10-02 WP-038 Engine attributes its own installs (owned.json →
  `explained` resolution on the next capture; actor stays `system`,
  source `seldon`), Phase 0 exit procedure for the shipped wizard,
  fresh-machine smoke list — merged.
- 2026-10-02 WP-037 Overlay live findings: memoised empty period view
  (no aggregation when the shell injects `service` after creation), the
  harness now creates the overlay like the shell and the old code fails
  its assertion with the live number (23), probe docs, Tokyo Night /
  Catppuccin Latte / Osaka Jade live screenshots, theme restored — merged;
  live `call view`: overlay 0, every chart paints 1. The Phase 1–3 live
  sweeps are complete.
- 2026-10-02 WP-013 Engine ↔ plugin e2e on the test host: two full runs
  47/47 after the unlock (gate G2), the live checklist of WP-020…031
  (keys, arming, drift and decisions flows, overlay paint counters,
  pill), FINDINGS with one real mismatch (overlay aggregates once on
  open in the live shell because the shell injects `service` after
  creation — WP-037) — merged. Incident: a `wtype` text ran in the test
  host's terminal after a click closed the panel (exit 127, harmless);
  rule added to ORCHESTRATION §11.
- 2026-10-02 WP-040 AUR package `jax-seldon` (PKGBUILD with the `watch`
  feature, `.SRCINFO`, user unit installed not enabled), release workflow
  (tag → musl asset + checksums, PKGBUILD bump, AUR push and plugin
  subtree split skipped without secrets, `workflow_dispatch` dry run),
  packaging/README.md with the operator's one-time setup — merged after
  one review round; ADR-0022 accepted. Test-host `makepkg` and the first
  real workflow run wait for the operator (guard exception, secrets).
- 2026-10-02 WP-041 Plugin README along the marketplace template,
  preview.png from offscreen renders (no bar pill until a live shot),
  Security section cross-checked against the plugin's three non-engine
  calls, KEYBINDINGS.md, plugin/LICENSE — merged after one review round;
  CONTRACT.md probe form `seldon --version --json`; SPEC-PLUGIN §10
  names wl-copy and the terminal launcher.
- 2026-10-02 WP-036 Dossier follow-ups: `omarchy-base` vs `user` origin
  from Omarchy's package lists (read-only), "Before the logbook" lists
  only the user's packages plus a base-count line, `packages.explicit`
  fence in the init templates and the fixture, empty deviation case
  cells filled from later cased events — merged; 111 dossier-related
  tests; real-host read-only run (counts only).
- 2026-10-02 WP-031 Prime Radiant charts (Heatmap, Series, DriftBars,
  RiskDonut, Timeline) and The Plan as the sixth slot; one-pass period
  table; hover read-outs via IPC; first-frame rule proven in the harness
  (per-file aggregation counters) — merged after one review round; 72
  node + 312 overlay checks; offscreen frame profile 4–7 ms; live profile
  pending the test host (operator item).
- 2026-10-02 WP-035 `seldon dossier` (eight generated fences in
  system/*.md from read-only queries, new `packages.explicit`, user text
  kept byte for byte, config redaction over host strings), "Before the
  logbook" group in REBUILD.md, `init` runs it once — merged after one
  review round; one real-host read-only run (counts only).
- 2026-10-02 WP-034 `seldon watch` behind the `watch` feature (notify
  only under the feature, debounce, generated-file filters, lock retry,
  one rebuild at start, clean SIGTERM/SIGINT), systemd user unit template
  and README (never installed by the engine), `check-rss` — merged after
  one review round; 8 watch tests; RSS 7.9 MB musl / 9.1 MB bench on the
  ×10 fixture.
- 2026-10-02 WP-032 `seldon rebuild` → outputs/REBUILD.md (seven sections,
  AUR rule from the command, open drift in place, dismissed changes
  "deliberately not reproduced", fence `rebuild`, only-on-change write)
  — merged after one small fix round; 329 engine tests; golden
  byte-identical on the fixture. Gap found by the review: pre-logbook
  packages are only counted → WP-035.
- 2026-10-02 WP-030 Prime Radiant overlay skeleton (five-slot 12-column
  grid with reflow, period selector 30/90/365/All, keyboard, precomputed
  period table in the service, overlay harness at 1080p/1440p/1.25 scale)
  — merged; 62 node + 180 overlay checks; live keys/screenshots on the
  test host pending the unlock.
- 2026-10-02 WP-024 Logbook templates en/de (English keys and headings,
  prose per language), logbook AGENTS.md, wizard first capture with
  `--since` backfill and pre-Seldon baseline, Claude Code / Omarchy-Agent
  harness installs, theme hook opt-in, `SELDON_TEST_GUARD` — merged; 308
  engine tests. **Phase 2 is code-complete** (live sweeps pending the
  test host unlock).
- 2026-10-01 WP-022 `seldon agent start` (config-driven launcher argv,
  `{prompt}` as one argument, detached, shell-string launchers refused)
  and the *Start agent* card action (key `a`) — merged after one review
  round; 548 panel checks on the rebased branch.
- 2026-10-01 WP-023 Decisions and Memory tabs (digits 4 and 6, new-decision
  sheet, six-tab panel) — merged; 53/171/519 checks; real-engine offscreen
  run on the test host; live sweep pending the unlock.
- 2026-10-01 WP-016 Fixture additions (three drift index variants,
  PreToolUse/Edit/Write hook payloads, regenerated fixture STATUS.md) —
  merged; validator 109 instances, 8 variants.
- 2026-10-01 WP-021 Drift sheet (link / explain / dismiss, group fan-out
  with `--only`, crisis strip → first crisis, "+N more", IPC `resolve`) —
  merged; 47/148/419 checks; real-engine run in a private offscreen shell
  on the test host; live sweep pending the unlock.
- 2026-10-01 WP-020 Work tab (three columns, case card with two-press
  arming, new-case sheet, `wipLimit`, badge `+(members−1)`) — merged; 261
  panel checks; real-engine smoke via IPC on the test host; live key smoke
  and theme sweep pending the test host unlock.
- 2026-10-01 WP-008 Reconciliation and `drift` commands (list/show/link/
  explain/dismiss with group fan-out and `--only`), `after_capture` case
  bookkeeping, ADR-0021 folding, detached editor launch — merged; 290+
  engine tests. **Phase 0 is code-complete.**
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
- **Test host:** unlocked 21:04 UTC, stay-awake on; G2 passed. The
  helper `/tmp/seldon-unlock.sh` there is yours. An incident line
  (`Live smoke: dismissed, test host package`, command not found) sits
  only in the memory of the open foot bash on the test host; `history -d`
  before closing it if you care. `scripts/guard.sh` comments cite
  "HERDR-SETUP.md §5" for the theme-sweep exception, which that doc does
  not mention — a comment fix in your file.
- **ssh alias `test`**: docs and the e2e script default to `ssh test`; this
  dev host has no such alias. Add `Host test` to `~/.ssh/config` (name in
  `memory/local.md`) or keep passing `SELDON_TEST_HOST=<alias>`.
- **Phase 0 exit done** (G3): real logbook at `~/Seldon`, snapper
  enabled by you (ADR-0011), one case worked by Claude Code, STATUS.md
  rendered. Finding F1 (the engine's own theme hook file shows up as
  drift) → WP-038 queued.
- **Guard updated on your decision (2026-10-02):** makepkg whitelist over
  ssh (two exact forms), service rule at the command position, comment
  fix; 74-row test table green.
- **Release v0.1.0 published 2026-10-02:** repo public, GitHub release
  with the three assets, `jax-seldon-plugin` filled (branch `main` + tag
  `v0.1.0`), PKGBUILD/.SRCINFO bumped on `main`. **AUR still open:**
  registration on aur.archlinux.org is temporarily closed; when it
  reopens, create the account, tell the orchestrator, and it generates a
  new key, sets `AUR_SSH_PRIVATE_KEY` and pushes `packaging/` by hand
  (no new tag needed). Until then users install the engine from the
  release asset.
- **ADR-0022** (AUR package builds against glibc; the static musl binary
  is the GitHub release asset) reads AGENTS.md §7 without changing it —
  accepted by the orchestrator, veto possible.
- **Live sweeps done.** Only the QSG frame-timing profile is left, see
  the next item; `call view` paint counters (1 per chart, 0–2 ms) are the
  live record.
- **Live frame profile of the Prime Radiant (WP-031)** needs
  `QSG_RENDER_TIMING` in Hyprland's environment on the test host
  (`hyprctl keyword env …`) plus a shell restart — a runtime change on the
  test host, so your call. The offscreen profile (4–7 ms frames) is the
  acceptance record; the live check via `call view` needs no env change.
- **FYI, veto possible:** the Prime Radiant's default period is 90 days
  (30/90/365/All available; resets to 90 d on every open). 365 d would
  leave the drift bars and timeline sparse on every logbook younger than
  a year. Say so if you prefer 365 d.
- **FYI, veto possible:** generated logbooks use English headings in every
  language (`# Decisions`, `## Purpose`, `## History` …) with German prose
  (ADR-0007, WP-024). The fixture logbook still has German headings; the
  schema track aligns it.
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
