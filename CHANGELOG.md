# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.1] - 2026-10-02

### Engine

- Files the engine installs for you — the Claude Code hooks, the theme
  hook, the harness launcher — are recorded as Seldon's own writes
  (`owned.json`), so the capture after `seldon init` explains them
  instead of opening drift (WP-038).
- `seldon import omarchy-agent <vault>` imports the omarchy-agent kit's
  Obsidian vault into the logbook: cases, sessions, knowledge and
  deviation rows, with redaction and a dry run by default; `--apply`
  writes once, marked, and reports id collisions it renumbered (WP-043).
- `seldon plan start` warns when an R2 or R3 case starts without a
  snapshot (`warnings` in `--json`); R3 also asks for the human's explicit
  go per step. Advice only, never refused (ADR-0023, WP-050).
- The default `[drift] alwaysRed` list follows ADR-0023's R3 subjects: new
  `omarchy-settings`, `limine*`, `grub`, `mkinitcpio*`, `filesystem` and
  the login path `pam`, `sddm`, `uwsm`; `linux*` is narrowed to the
  kernels (`linux`, `-lts`, `-zen`, `-hardened`, `-rt`, `-rt-lts`,
  `-omarchy`), so firmware and header upgrades stay routine.
  `init` writes the list into `config.toml`, so an existing config keeps
  its old list; add the new globs by hand.
- `seldon decide` and `seldon status` fill the `decisions.index` table in
  the logbook's `DECISIONS.md` from `decisions/`; text outside the fence
  stays yours.
- A generated fence whose begin marker lost its end marker is now left
  alone with a warning (`dossier`, `decide`, `status`; an error in
  `import`) instead of getting a second fence that a later run would
  replace together with your text.
- `seldon doctor` prints `~`-shortened paths in the `logbook` row, like
  its header; the wizard's harness question says how to toggle and
  confirm.
- SPEC-ENGINE no longer lists `hook install generic`: there is nothing to
  install, other agents pipe into `seldon hook generic` themselves.
- `seldon completions bash|zsh|fish` prints a completion script and
  `seldon mangen` the man page seldon(1), both generated from the help
  (WP-049).
- `seldon hook uninstall claude-code` and `seldon init --remove-theme-hook`
  undo what `hook install` and `init --theme-hook` installed, and nothing
  else; the next capture explains the removal instead of opening drift.
- Every `--help` reviewed: one sentence per command, value names that say
  what they are (`<ACTOR>`, `<ZONE>`, …), a line for every argument,
  examples for `--since` and free text after `--`. `--snapshot` is
  offered by `plan start` only.
- Piping a command's output into a reader that stops early (`| head`)
  no longer crashes the engine.

### Plugin

- The panel is 460 px wide and tab labels no longer clip at the default
  font size (WP-039).
- *Update in terminal* on the "Index format mismatch" banner runs the
  GitHub installer while the AUR package does not exist, the same
  one-liner as *Install in terminal* (ADR-0024); it flips back to the
  AUR command when the package is live.
- The Prime Radiant mark replaces the `⟡` fallback glyph (WP-051). The
  pill shows the bar glyph before the counts (`2 · 3`): hand-hinted at the
  16 and 20 px boxes of scale 1.0 and 1.25, the vector at every other
  scale, its centre on the digits' centre (within 1 px, measured by the
  new `tests/plugin/bar-view.sh` in three themes), in the counts' colour.
- The panel header is the mark and "Seldon" with the designer's metrics.
  The status banner shows its state pictogram (engine missing, logbook not
  initialised, index missing, index stale), 48 px in the panel and 96 px
  in the Prime Radiant; the Today tab shows the day's state (crisis, open
  drift, active cases or all clear).
- The Timeline draws releases as diamonds, snapshots as dots, crises as
  the Prime Radiant spindle (no longer a second diamond) and case spans
  between brackets, with a legend in its title row.
- Every image follows the theme: the masks are tinted through their SVG
  root colour, no colour is written into QML. The images are copies under
  `plugin/assets/`; `preview.png` is re-rendered.

### Packaging and docs

- The AUR package installs the man page and the bash, zsh and fish
  completions; `install.sh` installs the man page and the completions of
  the shells you have under its prefix and removes them on `--uninstall`;
  the release's binary tarball carries both (WP-049).
- `assets/` holds the Prime Radiant files of record: the round-3 masks,
  the round-2 rasters and brand files, `DELIVERY.md`, `LICENSE` and a
  README listing every file and where the project uses it (WP-051). Both
  READMEs open with the hero image.
- New README for the project and the plugin repository (WP-046): what
  Seldon is and why, six features, a quick start from install to the
  plugin, a 60-second tour and a table of every document. The developer
  reading order and layout moved to `docs/DEVELOPMENT.md`. `docs-check`
  now also checks both READMEs, `docs/DEVELOPMENT.md` and `llms.txt`,
  including links into the public repositories and plugin links that
  must survive the subtree split.
- User guide in English and German (WP-045): `docs/user/en/` and
  `docs/user/de/` with thirteen pages each, from getting started to the
  glossary, a style sheet and a translation policy (English is the
  source; each German page names the commit it matches).
  `just docs-check`, part of `just check`, checks links, the page sets and
  their structure, and every `seldon` command in the guide against the
  engine's `--help`; the CLI reference is the engine's own help text.
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

[Unreleased]: https://github.com/JohnAndrewsX/jax-seldon/compare/v0.1.1...HEAD
[0.1.1]: https://github.com/JohnAndrewsX/jax-seldon/releases/tag/v0.1.1
[0.1.0]: https://github.com/JohnAndrewsX/jax-seldon/releases/tag/v0.1.0
