# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Engine

- `seldon plan start` warns when an R2 or R3 case starts without a
  snapshot (`warnings` in `--json`); R3 also asks for the human's explicit
  go per step. Advice only, never refused (ADR-0023, WP-050).
- The default `[drift] alwaysRed` list follows ADR-0023's R3 subjects: new
  `omarchy-settings`, `limine*`, `grub`, `mkinitcpio*`, `filesystem`.
  `init` writes the list into `config.toml`, so an existing config keeps
  its old list; add the new globs by hand.
- `seldon decide` and `seldon status` fill the `decisions.index` table in
  the logbook's `DECISIONS.md` from `decisions/`; text outside the fence
  stays yours.
- `seldon doctor` prints `~`-shortened paths in the `logbook` row, like
  its header; the wizard's harness question says how to toggle and
  confirm.
- SPEC-ENGINE no longer lists `hook install generic`: there is nothing to
  install, other agents pipe into `seldon hook generic` themselves.

### Packaging and docs

- Repository hygiene (WP-048): CONTRIBUTING.md, SECURITY.md (GitHub
  private vulnerability reporting), CODE_OF_CONDUCT.md (Contributor
  Covenant 2.1), issue forms for bugs and features, a pull request
  template, docs/VERSIONING.md (SemVer, `contractVersion`, tag flow).
- Release notes come from the version's CHANGELOG.md section; the
  release workflow (dry run included) fails when it is missing
  (`packaging/release-notes.sh`).
- `cargo audit` runs weekly and on lock-file changes as a non-blocking
  advisory workflow (`audit.yml`); the plugin repository has a security
  policy too.

## [0.1.0] - 2026-10-02

First release. Engine `seldon` (Rust, static musl binary as the release
asset, AUR package against glibc per ADR-0022) and the Omarchy shell
plugin `jax.seldon` (published from `plugin/` as `jax-seldon-plugin`).

### Engine

- `seldon init` wizard: logbook layout, config, collectors, watched config
  paths, backfill with `--since`, pre-Seldon baseline, Obsidian vault,
  Claude Code / Omarchy-Agent harness hooks, theme hook opt-in, git
  autocommit; templates in English and German.
- Collectors: pacman (transactions, attribution), snapper (degraded until
  the user allows it, ADR-0011), omarchy, plugins, theme, config
  (manifest, redaction per SPEC-ENGINE §7). Idempotent captures.
- Ledger (append-only JSONL, contract v1), index.json for the plugin,
  generated STATUS.md and ledger views; `status`, `index`, `doctor`.
- Cases: `plan new|start|verify|done|drop|list|show`, journal, `log`,
  `event`, `decide`, `open`; agent attribution through hooks (ADR-0017,
  ADR-0019, ADR-0021); `agent start` with a config-driven launcher.
- Drift: `drift list|show|link|explain|dismiss` with transaction groups
  (ADR-0013), crises first, 200-item cap (ADR-0020).
- `rebuild` → outputs/REBUILD.md (seven sections a fresh install can
  follow); `dossier` → generated fences in system/*.md incl. the explicit
  package list with Omarchy-base vs user origin.
- `watch` (feature `watch`, off by default, ADR-0005) with a documented,
  never-enabled systemd user unit.
- Exit codes 0/1/2/3/4; every command supports `--json`.

### Plugin

- Bar pill (active cases · open drift), panel with Today, Changelog,
  Work, Decisions, System and Memory tabs, keyboard per SPEC-PLUGIN §5,
  QuickEntry, drift sheet (link/explain/dismiss), new case and decision
  sheets, Start agent.
- Prime Radiant overlay: heatmap, package series, drift bars, risk donut,
  timeline and The Plan for 30/90/365 days or all time; precomputed in
  the service, one paint per chart, hover read-outs; IPC targets
  `jax.seldon.panel` and `jax.seldon.service`.
- Degraded states with one-click fixes: engine missing, logbook not
  initialised, index missing or stale, contract mismatch, snapper.
- Theme tokens only; headless harnesses and an engine↔plugin e2e on a
  test host.

### Packaging and docs

- PKGBUILD, .SRCINFO, release workflow (tag → musl asset, GitHub
  release, AUR push and plugin subtree split when the secrets exist,
  `workflow_dispatch` dry run), packaging README.
- Specs (engine, plugin, logbook, contract), 22 ADRs, plugin README with
  security section, keybinding docs, preview image.

[Unreleased]: https://github.com/JohnAndrewsX/jax-seldon/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/JohnAndrewsX/jax-seldon/releases/tag/v0.1.0
