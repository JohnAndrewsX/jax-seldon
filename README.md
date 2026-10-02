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
| Engine | `engine/` | Rust, single static binary `seldon` | GitHub release (`install.sh`), AUR package `jax-seldon` (see [Install](#install)) |
| Plugin | `plugin/` | Quickshell QML | `omarchy plugin add …` |
| Contract | `schema/` | JSON Schema | the only link between engine and plugin |

## Install

**AUR package: coming soon. Until then install the engine from GitHub
(below).**

### Engine from GitHub

`install.sh` downloads the static `seldon` binary of a release, checks it
against the release's `SHA256SUMS` and refuses on a mismatch, then
installs `~/.local/bin/seldon` (and the alias `jax-seldon`). It runs as
your user and never asks for root.

Checked form — download, read, verify, run:

```sh
cd "$(mktemp -d)"
curl -fsSLO https://github.com/JohnAndrewsX/jax-seldon/releases/latest/download/install.sh
curl -fsSLO https://github.com/JohnAndrewsX/jax-seldon/releases/latest/download/SHA256SUMS
less install.sh                                    # read what it does
sha256sum -c --ignore-missing SHA256SUMS && bash install.sh
```

One-liner — the script still verifies the engine against `SHA256SUMS`;
only the script itself goes unchecked:

```sh
curl -fsSL https://github.com/JohnAndrewsX/jax-seldon/releases/latest/download/install.sh | bash
```

Options (with the one-liner: `| bash -s -- --unit`):

| Option | Effect |
|---|---|
| `--version vX.Y.Z` | that release instead of the latest |
| `--prefix DIR` | install into `DIR/bin` instead of `~/.local/bin` |
| `--unit` | also install the optional watcher unit into `~/.config/systemd/user/` (installed, not enabled; see `engine/systemd/README.md`) |
| `--force` | replace a `seldon` (or unit) the script did not install, such as a self-built binary; without it the script refuses and changes nothing |
| `--uninstall` | remove what the script installed (give the same `--prefix`) |

Then create your logbook once with `seldon init`, and add the plugin:
`omarchy plugin add https://github.com/JohnAndrewsX/jax-seldon-plugin.git --enable`
(`plugin/README.md`).

- **Update:** run `install.sh` again, either form. It replaces `seldon`
  when the release is newer; with the same version it changes nothing.
- **Remove:** `bash install.sh --uninstall`, or with the one-liner
  `… | bash -s -- --uninstall`. It removes exactly the files it
  installed (a file you changed since is kept, and it says so). Your
  logbook, `~/.config/seldon/` and `~/.local/state/seldon/` stay.
- **v0.1.0** predates `install.sh`, so `releases/latest/download/install.sh`
  exists from the next release on (v0.1.1). Until then take the script from
  `main` (https://raw.githubusercontent.com/JohnAndrewsX/jax-seldon/main/install.sh)
  and run `bash install.sh --version v0.1.0`: the engine is checked the
  same way, the script itself is not covered by `SHA256SUMS`.

### Engine from the AUR

```sh
omarchy pkg aur add jax-seldon   # install
yay -S jax-seldon                # update
omarchy pkg drop jax-seldon      # remove
```

Install from one source only: both put a `seldon` on your `PATH`.

This repository is the **development kit**. Read in this order:

1. `PROJECT.md` — goal, scope, definition of done
2. `AGENTS.md` — rules for every agent working here
3. `docs/CONCEPT.md` — the idea, end to end
4. `docs/SPEC-LOGBOOK.md`, `docs/SPEC-ENGINE.md`, `docs/SPEC-PLUGIN.md`, `docs/CONTRACT.md`
5. `docs/PLAN.md` — phases and work packages
6. `docs/ORCHESTRATION.md` — how the agent team is run (Herdr)
7. `DECISIONS.md` — ADR index

The human-facing concept lives in `docs/seldon-concept.html` (offline, single file).

Author: `JohnAndrewsX`. Plugin ID `jax.seldon`. License MIT.

## For AI agents

An agent started inside a Seldon logbook follows that logbook's `AGENTS.md`; the long form is
[`docs/AGENT-GUIDE.md`](docs/AGENT-GUIDE.md), the one-page index is [`llms.txt`](llms.txt).
An agent working on this repository follows [`AGENTS.md`](AGENTS.md).
