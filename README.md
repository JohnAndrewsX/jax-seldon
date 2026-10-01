# jax-seldon

**Seldon** is a flight recorder and planning desk for an Omarchy system.
It keeps a Markdown logbook of everything that changes on the machine —
packages, configs, themes, plugins, Omarchy updates — reconciles those
facts against planned work, and shows the whole picture in the Omarchy
Quattro shell: a bar pill, a panel, and a full-screen overlay called the
*Prime Radiant*.

Named after Hari Seldon (Asimov, *Foundation*): the Plan predicts, the
Crises are where reality deviates. Seldon makes the deviations visible.

| Part | Where | Language | Ships as |
|---|---|---|---|
| Logbook | `~/Seldon` (configurable) | Markdown + YAML, Obsidian-compatible | user data, git repo |
| Engine | `engine/` | Rust, single static binary `seldon` | AUR package `jax-seldon` |
| Plugin | `plugin/` | Quickshell QML | `omarchy plugin add …` |
| Contract | `schema/` | JSON Schema | the only link between engine and plugin |

This repository is the **development kit**. Read in this order:

1. `PROJECT.md` — goal, scope, definition of done
2. `AGENTS.md` — rules for every agent working here
3. `docs/CONCEPT.md` — the idea, end to end
4. `docs/SPEC-LOGBOOK.md`, `docs/SPEC-ENGINE.md`, `docs/SPEC-PLUGIN.md`, `docs/CONTRACT.md`
5. `docs/PLAN.md` — phases and work packages
6. `docs/ORCHESTRATION.md` — how the agent team is run (Herdr)
7. `DECISIONS.md` — ADR index

The human-facing concept lives in `docs/seldon-concept.html` (offline, single file).

Author: Eugen (`JohnAndrewsX`). Plugin ID `jax.seldon`. License MIT.
