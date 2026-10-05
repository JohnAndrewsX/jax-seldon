# DEVELOPMENT.md — Working on Seldon

This repository is Seldon's development kit: the engine, the plugin, the
contract between them, the specs, the decisions and the work packages
the team runs. If you want to *use* Seldon, start with the
[README](../README.md) and the [user guide](user/README.md) instead.
Before you open a pull request, read [CONTRIBUTING.md](../CONTRIBUTING.md).

## The parts

| Part | Where | Language | Ships as |
|---|---|---|---|
| Logbook | `~/Seldon` (configurable) | Markdown + YAML, Obsidian-compatible | user data, git repo |
| Engine | [`engine/`](../engine) | Rust, single static binary `seldon` | GitHub release (`install.sh`), AUR package `jax-seldon` |
| Plugin | [`plugin/`](../plugin) | Quickshell QML | `omarchy plugin add …`, from the generated repository [`jax-seldon-plugin`](https://github.com/JohnAndrewsX/jax-seldon-plugin) |
| Contract | [`schema/`](../schema) | JSON Schema | the only link between engine and plugin |

The engine is the only writer of the logbook and of the index
`~/.local/state/seldon/index.json`. The plugin reads that index and
nothing else, and runs the engine only with fixed argument lists.
`plugin/` is published to `jax-seldon-plugin` with
`git subtree split --prefix=plugin` on every release
([ADR-0009](../decisions/ADR-0009-plugin-release-repo.md)), so it must stay
self-contained: nothing in it may reference a file outside `plugin/` by a
relative path.

## Reading order

1. [`PROJECT.md`](../PROJECT.md): goal, scope, definition of done
2. [`AGENTS.md`](../AGENTS.md): rules for everyone working here, human or
   agent
3. [`docs/CONCEPT.md`](CONCEPT.md): the idea, end to end
4. The specs: [`SPEC-LOGBOOK.md`](SPEC-LOGBOOK.md),
   [`SPEC-ENGINE.md`](SPEC-ENGINE.md), [`SPEC-PLUGIN.md`](SPEC-PLUGIN.md),
   [`CONTRACT.md`](CONTRACT.md)
5. [`docs/PLAN.md`](PLAN.md): phases and work packages
6. [`docs/ORCHESTRATION.md`](ORCHESTRATION.md): how the agent team is run
7. [`DECISIONS.md`](../DECISIONS.md): the index of decision records (ADRs)

The concept for people who prefer a picture is
[`docs/seldon-concept.html`](seldon-concept.html), one offline HTML file.

## Layout

```
jax-seldon/
├── PROJECT.md · AGENTS.md · STATUS.md · DECISIONS.md
├── README.md · CONTRIBUTING.md · SECURITY.md · CHANGELOG.md · llms.txt
├── docs/            concept, specs, plan, orchestration, user guide (docs/user/)
├── schema/          JSON Schema (contract)
├── fixtures/        sample index, sample logbook, sample logs for collectors
├── decisions/       ADR-NNNN-*.md
├── work/            queued/ active/ completed/: work packages (WP-NNN)
├── engine/          Rust crate `seldon`
├── plugin/          Quickshell plugin `jax.seldon`
├── packaging/       PKGBUILD, release scripts
├── scripts/         fixture validation, docs-check, the dev-host guard
├── tests/           install, plugin, release and integration tests
└── install.sh       the engine installer published with each release
```

## Build and test

`just check` is the gate for every change. It runs rustfmt, clippy and
the engine tests (with and without the `watch` feature), the packaging
and install checks, the fixture validation against `schema/`, the docs
check, and on an Omarchy host the plugin checks
(`omarchy plugin validate`, qmllint against the installed shell, the
headless plugin harnesses). Without an Omarchy host,
`SELDON_SKIP_HOST_CHECKS=1 just check` runs what CI runs. [`docs/TESTING.md`](TESTING.md) explains every
step, the manual runs in scratch directories, the end-to-end test and
how the plugin screenshots are made.

The project's test host runs the current `main` build: after each green
main check the orchestrator runs `just deploy-test-host`, which builds
the engine with a `+main.<sha>` version marker and puts it and the plugin
on that host ([`docs/TESTING.md`](TESTING.md#test-host-follows-main)).
Release builds carry no marker; installed machines run releases only.

## How the team works

Seldon is built by a small team of AI agents run by an orchestrator,
with the operator deciding scope and releases. The same rules apply to a
human contributor.

- Every unit of work is a **work package** `WP-NNN` under `work/`, with
  a goal, inputs, outputs, acceptance tests, a role and dependencies.
  The orchestrator moves it from `queued/` to `active/` to `completed/`.
- Each package is worked on its own **git worktree** `wt/WP-NNN` and
  branch `wp/NNN-short-slug`, never on `main`. Commits are small and in
  English: `engine: add pacman collector (WP-004)`.
- The handover is a pull request or `work/active/WP-NNN/HANDOVER.md`:
  what was done, what was not, how it was verified, open questions. A
  reviewer approves or sends it back; only the orchestrator completes a
  package.
- **Contract first:** a change to `schema/*.json` needs an ADR, a
  `contractVersion` bump, new fixtures and both sides changed together
  ([`CONTRACT.md`](CONTRACT.md)).
- What the team learns goes into [`memory/`](../memory), for example
  [`memory/pitfalls.md`](../memory/pitfalls.md).

[`docs/ORCHESTRATION.md`](ORCHESTRATION.md) is the full loop: roles,
briefings, gates, parallelism, autopilot.
[`docs/HERDR-SETUP.md`](HERDR-SETUP.md) sets up the terminal multiplexer
the agents run in. [`STATUS.md`](../STATUS.md) shows what is active now.

## More documents

| Document | What it covers |
|---|---|
| [`docs/TESTING.md`](TESTING.md) | every check, how to run it, what CI skips |
| [`docs/VERSIONING.md`](VERSIONING.md) | versions, `contractVersion`, the release checklist |
| [`packaging/README.md`](../packaging/README.md) | the release workflow, the AUR package, the plugin split |
| [`docs/KEYBINDINGS.md`](KEYBINDINGS.md) | where every key and IPC target is documented |
| [`docs/DESIGN-BRIEF.md`](DESIGN-BRIEF.md) | the brief for the Seldon mark and icons |
| [`docs/AGENT-GUIDE.md`](AGENT-GUIDE.md) | how agents work inside a logbook (not in this repository) |
| [`docs/user/STYLE.md`](user/STYLE.md) | voice, terms and checks of the user guide |

Author `JohnAndrewsX`. Plugin id `jax.seldon`. Licence MIT.
