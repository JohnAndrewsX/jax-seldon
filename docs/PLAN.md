# PLAN.md — Phases and work packages

Eight phases (0–7). Phases 5–7 are proposed with ADR-0056–0058 (E96
delegation; the operator accepts) and summarised on one page in
[ROADMAP.md](ROADMAP.md). Each phase ends with something the operator can use. Work packages
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
| WP-042 | Store submission preparation for the plugin (`work/*/WP-042.md`, `packaging/store/`) | Docs | — | README without `curl … \| bash` fences, privilege wording in the store's form; predicted baseline without findings, capabilities `privilege` and `package-manager`; submission body for the operator. Filing after `v0.2.0` on the operator's go |
| WP-043 | The operator's own logbook migration from `~/Omarchy-Agent` (optional) | Engine | WP-024 | importer maps kit cases/journal into Seldon layout; dry-run report first |

## Phase 5 — Seldon is an Omarchy app (0.5.0)

Proposed (ADR-0056, ADR-0057, ADR-0058). Works on the branch `app`, which
merges into `next` only after the operator accepts the ADRs. The gate is
[APP-PARITY.md](APP-PARITY.md), not a date. Rows marked — are queued by
the orchestrator when their dependencies land.

| WP | Title | Role | Depends on | Done when |
|---|---|---|---|---|
| WP-227 | The app decided: ADR-0056–0058, CONCEPT v2, this phase, ROADMAP, the parity list, the proposed AGENTS.md rows | Architect | — | docs check green; every superseded ADR named in DECISIONS.md; no AGENTS.md or PROJECT.md edit |
| WP-228 | App skeleton: `app/` (qmake6), `main.cpp`, `Theme`, `Translator`, `Backend` (index watch, off-thread parse, fixed-argv `QProcess`), `Main.qml` with `?`, `Q`, Space, the four states, and the *Changes* view reusing `Model.js`/`Tones.js` | App | — | `app/bin/build` and `app/bin/test` green; no 6.12-only API; no symlink, no `qs.Ui`, no hex colour outside the `Theme` fallback; no QML warning; a fixture screenshot |
| WP-229 | Engine: `open` (the app), `edit`, the `open <target>` alias, `skill`, `changes --around`; CONTRACT.md row `edit` | Engine | — | the existing `open` tests pass through the alias; a test per command; `skill install` twice is idempotent; `just check` green |
| WP-230 | Packaging as `seldon`: PKGBUILD with both binaries, `.desktop`, icon, `.install`, the plugin under `/usr/share/seldon/plugin/`; CI and release builds with Qt from Omarchy stable's mirror; an OPR recipe draft (not submitted); root `bin/` wrappers | Scaffold | WP-228's `bin/build` | `just check-packaging` green; `makepkg -f` in the CI container builds both binaries; the file list matches |
| — | *Plan* view: cases by status, the case detail, start/verify/complete/drop/reopen, new case, one agent per case, import tasks through the portal picker | App | WP-228, WP-229 | the APP-PARITY rows for Work (8d, 8g, 3c, 10d) pass offscreen |
| — | *Record* view: journal and note, decisions (accept, new), dossier, memory, as documents | App | WP-228, WP-229 | the APP-PARITY rows 10a–10i pass offscreen |
| — | *Radiant* view: the charts and the graph (WP-163's scope moves here) | App | WP-228 | the APP-PARITY rows 9 and 11 pass offscreen |
| — | The pill: plugin cut to pill and capture timer, left click `seldon open`, catalogs `en`/`de` (ADR-0057), the desk and the `jax.seldon.panel` shim removed (WP-188's part), binding `{ launch = "seldon" }` | Plugin | WP-229, the three views | validate, qmllint, `bar-view.sh`, `service-states.sh` green in `en` and `de`; no desk file left |
| — | App catalogs: `de.json` for every string, the language look (`en`, `de`, `ar` mirrored), the bare-literal check | App | the three views | no bare visible literal; labels fit in `de` |
| — | README in the omacom shape (screenshot first, `omarchy pkg add seldon`, keys table, command line), user guide en/de, CHANGELOG **Breaking** | Docs | the pill | docs check green |
| — | Release 0.5.0: the parity run, acceptance record, test host smoke, stage-2 review | Orchestrator | all above | every APP-PARITY row app or drop; the daily acts pass; CI green on stable |

**Phase 5 exit:** the operator opens Seldon from the launcher, resolves a
change, works a case, and the desk is gone without a missing act.

## Phase 6 — Contract 3 (0.6.0)

| WP | Title | Role | Depends on | Done when |
|---|---|---|---|---|
| — | Contract 3 ADR (advisor-written): crash metadata, the `changes --around` fields in the index, reports as records; `contractReadableFrom` per ADR-0051 | Architect | Phase 5 | accepted by the operator; schema, fixtures, both readers in one PR |
| WP-185 | Crashes in the record, re-scoped to "record and answer": `COREDUMP_*` metadata as events (E27); no Seldon crash UI | Engine | contract 3 ADR | fixtures → events; idempotent; the skill points `diagnose-crash` at `changes --around` |
| WP-186 | Reports remembered, re-scoped: a report the user files through Omarchy's flow is recorded, never drafted by Seldon | Engine | contract 3 ADR | as its WP, re-scoped |
| — | The other 48 languages, agent-made and sample-reviewed (ADR-0057 §7) | App + Plugin | Phase 5 | every registry code has a catalog; the sampling review recorded |
| — | Capture hooks `post-update.d` and `post-boot.d` through `omarchy hook install` | Engine | — | installed and removed by Omarchy's command; a capture after `omarchy update` |
| — | Package recipe for Omarchy's package repository ready (`source: local`, `auto_merge`, `upstream.watch`); submission is the operator's act | Scaffold | WP-230 | the recipe builds in the CI container |
| — | Removals: the `open <target>` alias, the `jax-seldon` symlink | Engine + Scaffold | — | CHANGELOG **Breaking**; tests updated |
| WP-189 | Graph: the layouts and the whole-logbook export, in the app | App | *Radiant* | as its WP, in the app |

**Phase 6 exit:** crashes and their surroundings are on the record, Seldon
speaks every Omarchy language, and the package can be submitted.

## Phase 7 — 1.0.0

| WP | Title | Role | Depends on | Done when |
|---|---|---|---|---|
| — | The package published (Omarchy's package repository or the AUR; the operator's acts) | Operator | Phase 6 | `omarchy pkg add seldon` installs it on a fresh Omarchy |
| — | One Omarchy stable cycle without a contract change; exit-code policy final (ADR-0058 §5); SPEC-LOGBOOK frozen | Orchestrator | the package | the operator's go for 1.0.0 |

**Phase 7 exit:** 1.0.0, and "breaking = major" from then on.

## Milestones

- **M0** Phase 0 exit — target: first weekend.
- **M1** pill in the bar.
- **M2** planning desk.
- **M3** Prime Radiant.
- **M4** public release (0.1.0, 2026-10-02; the 1.0.0 it once named is M7).
- **M5** Seldon is an Omarchy app (0.5.0; proposed, ADR-0056).
- **M6** contract 3 and every Omarchy language (0.6.0).
- **M7** 1.0.0.

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
