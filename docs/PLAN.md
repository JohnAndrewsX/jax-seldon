# PLAN.md — Phases and work packages

Five phases. Each phase ends with something the operator can use. Work packages
(WP) are the unit the orchestrator assigns; each has acceptance tests the
orchestrator can run without a human. Roles: see ORCHESTRATION.md.

Dependencies are explicit; everything else may run in parallel.

## Phase 0 — Foundation (usable from the terminal and Obsidian, no plugin)

| WP | Title | Role | Depends on | Done when |
|---|---|---|---|---|
| WP-001 | Repo scaffold, CI, toolchain | Scaffold | — | `cargo build --release` (musl) and `omarchy plugin validate plugin/` run in CI; `just check` runs fmt, clippy, tests, schema validation, qmllint |
| WP-002 | Contract and fixtures | Schema Keeper | — | `schema/*.json` final for v1; `fixtures/index.sample.json` validates; `fixtures/logbook/` exists with 3 cases, 10 journal days, 2 months of ledger, dossier; `fixtures/logs/pacman.log` with rotation case |
| WP-003 | Engine core: config, logbook model, frontmatter, `init` | Engine | WP-002 | `seldon init --non-interactive --path /tmp/x` produces the SPEC-LOGBOOK layout; `seldon doctor` green; round-trip tests for case/journal/decision frontmatter |
| WP-004 | Collectors: pacman, snapper, omarchy | Engine | WP-003 | fixture logs → expected events (golden); second run yields 0 events; rotation handled; redaction tests pass |
| WP-005 | Collectors: plugins, theme, config manifest | Engine | WP-003 | diff-based events correct on fixtures; skipPaths honoured; files > 1 MB skipped |
| WP-006 | `log`, `event`, `plan` commands, case state machine, journal append | Engine | WP-003 | state transitions enforced; files move folders; Log section append-only; `plan start` writes `.seldon/active-case` and records snapshot when given |
| WP-007 | `index`, `status`, ledger `.md` view, STATUS.md | Engine | WP-004, WP-006 | `seldon index` output on `fixtures/logbook/` equals `fixtures/index.sample.json` modulo timestamps; < 100 ms on ×10 fixtures (release); atomic write |
| WP-008 | Reconciliation and `drift` commands | Engine | WP-007 | fixtures produce the three expected drift rows; link/explain/dismiss append resolution events and remove rows on rebuild; dependency auto-link works |
| WP-009 | Hooks: `hook install claude-code`, `hook claude-code`, session-start/stop, generic | Engine | WP-006 | hook JSON fixtures classify correctly; non-mutating commands produce nothing; always exit 0; session-start prints context block; `hook install` writes a valid `.claude/settings.json` fragment without clobbering existing hooks |

**Phase 0 exit:** The operator runs `seldon init` on their machine, works one real
case with Claude Code, sees events, drift and a STATUS.md. Logbook opens in
Obsidian.

## Phase 1 — Visible (bar pill and panel, read-mostly)

| WP | Title | Role | Depends on | Done when |
|---|---|---|---|---|
| WP-010 | Plugin skeleton: manifest, Service.qml, BarWidget.qml, states | Plugin | WP-002 | validate + qmllint pass; pill shows counts from `SELDON_INDEX` fixture; all five banner states render; engine-missing detection works with `PATH` manipulated |
| WP-011 | Panel: Today, Changelog, System tabs | Plugin | WP-010 | tabs render fixture data; keyboard nav; source filter; snapshot rows highlighted; theme-switch test with 3 themes |
| WP-012 | Panel actions: QuickEntry, Capture now, Open in editor | Plugin | WP-011, WP-006 | `seldon log` called with the text as one argv element; capture triggers index refresh via FileView; editor opens the right path |
| WP-013 | Engine ↔ plugin integration test on a real logbook | QA | WP-007, WP-012 | end-to-end script: init → capture → index → shell reads it; documented in `docs/TESTING.md` |

**Phase 1 exit:** the pill lives in the operator's bar.

## Phase 2 — Planning desk (cases, agents, drift in the UI)

| WP | Title | Role | Depends on | Done when |
|---|---|---|---|---|
| WP-020 | Work tab: three-column list, case card, new-case sheet | Plugin | WP-011 | create/start/verify/done from the panel; WIP limit shown; proposedEvents badge |
| WP-021 | Drift sheet: link / explain / dismiss | Plugin | WP-008, WP-011 | each action calls the engine with validated ids; crisis strip; list empties after rebuild |
| WP-022 | Start agent from a case | Plugin + Engine | WP-020 | button runs the configured launcher (`omarchy agent prompt` or `config.toml [agent] launcher`) with `seldon hook session-start` output as prompt context; documented fallbacks |
| WP-023 | Decisions and Memory tabs | Plugin | WP-011 | lists render; "New decision" calls `seldon decide --no-edit` then opens editor |
| WP-024 | Logbook templates (en, de), AGENTS.md for the logbook, optional Omarchy-Agent harness install | Engine + Docs | WP-003 | `seldon init --language de` yields German prose templates; harness option copies guard/skills if the template dir exists and documents what it did |

**Phase 2 exit:** The operator plans a case in the panel, sends an agent, sees its
trace, and resolves drift without a terminal.

## Phase 3 — Prime Radiant and reproducibility

| WP | Title | Role | Depends on | Done when |
|---|---|---|---|---|
| WP-030 | Overlay skeleton, grid, period selector, keyboard | Plugin | WP-010 | opens via IPC and middle click; closes on Esc; layout holds at 1920×1080 and 2560×1440 |
| WP-031 | Charts: Heatmap, Series, DriftBars, RiskDonut, Timeline | Plugin | WP-030 | each renders from fixture series; hover details; theme colours only; no frame drops on open (profiled with `qs` tooling) |
| WP-032 | `seldon rebuild` → outputs/REBUILD.md | Engine | WP-007 | document lists explicit packages with cases, deviations with reasons, plugins, theme, user units; a reviewer can follow it |
| WP-033 | `seldon update-impact` | Engine | WP-032 | reads installed vs. available release notes from the local omarchy repo (git log/tags), matches changed paths against deviations; report in outputs/ |
| WP-034 | `seldon watch` (feature-gated) and systemd user unit template | Engine | WP-007 | inotify triggers debounced index rebuild; RSS < 10 MB; unit documented, off by default |

**Phase 3 exit:** the screenshot. And a REBUILD.md that works.

## Phase 4 — Release

| WP | Title | Role | Depends on | Done when |
|---|---|---|---|---|
| WP-040 | AUR package `jax-seldon` (PKGBUILD, .SRCINFO), release workflow | Scaffold | WP-009 | `makepkg -si` works on a clean Omarchy; `omarchy pkg add jax-seldon` documented; version from git tag |
| WP-041 | Plugin README, preview.png, security section, keybinding docs | Docs | WP-031 | README follows the marketplace template; preview shows panel + Prime Radiant |
| WP-042 | Marketplace submission (plugins.omarchy.org and omahub.dev) | Docs | WP-040, WP-041 | listings live; security scan clean (no network, no units, no binaries) |
| WP-043 | The operator's own logbook migration from `~/Omarchy-Agent` (optional) | Engine | WP-024 | importer maps kit cases/journal into Seldon layout; dry-run report first |

## Milestones

- **M0** Phase 0 exit — target: first weekend.
- **M1** pill in the bar.
- **M2** planning desk.
- **M3** Prime Radiant.
- **M4** public release 1.0.0.

No dates beyond M0 on purpose; the orchestrator reports velocity after
Phase 0 and the operator sets the rest.

## Risks and how the plan handles them

| Risk | Mitigation |
|---|---|
| Shell API details differ from the public guide | WP-010 reads `shell/README.md` and built-in plugins first; findings go to `memory/omarchy-shell.md` in this repo |
| Omarchy changes the plugin contract in a release | manifest `schemaVersion` pinned; CI runs against the installed Omarchy; `seldon doctor` reports mismatch |
| pacman.log format edge cases | fixture corpus grows with every bug; parser is table-driven |
| Index grows past 1 MB | contract caps per section; truncation flag reserved |
| Agents write to the logbook directly | AGENTS.md forbids it; `seldon doctor` detects unregistered files and offers `seldon index` repair |
