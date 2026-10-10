![Seldon: the Prime Radiant mark and the wordmark on night blue](assets/a6-readme-hero-dark-1280x640.png)

# Seldon

A flight recorder and planning desk for your Omarchy machine.

[![CI](https://github.com/JohnAndrewsX/jax-seldon/actions/workflows/ci.yml/badge.svg)](https://github.com/JohnAndrewsX/jax-seldon/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/JohnAndrewsX/jax-seldon)](https://github.com/JohnAndrewsX/jax-seldon/releases/latest)
[![Licence: MIT](https://img.shields.io/github/license/JohnAndrewsX/jax-seldon)](LICENSE)

![The Prime Radiant overlay on the left with heatmap, package series, drift bars, risk donut, timeline and the plan; the bar panel's Today tab on the right; Tokyo Night theme, sample data](plugin/preview.png)

Seldon keeps a plain-Markdown logbook of everything that changes on your
Omarchy system: packages, config files, themes, plugins, snapshots and
Omarchy updates. You plan changes as *cases*. Seldon matches what
actually happened against those cases and shows every change nobody
planned as *drift*, so you can explain it. A bar pill, a panel and a
fullscreen overlay, the *Prime Radiant*, show the picture in the Omarchy
shell. Seldon only records: it never installs anything, never changes
your system and never blocks a command.

The name comes from Hari Seldon in Asimov's *Foundation*: the Plan
predicts, and a crisis is where reality leaves it.

[Why](#why) · [Features](#features) · [Quick start](#quick-start) ·
[Tour](#a-60-second-tour) · [Install options](#install-options-update-and-removal) ·
[Documentation](#documentation) · [Status](#project-status) ·
[Contributing](#contributing)

## Why

Omarchy is built for working with AI agents. On a developer's machine,
agents and people install packages, edit configs and switch themes every
day. After a few weeks nobody can say why the system looks the way it
does. `pacman.log` and `journalctl` have the facts but not the reasons. A
hand-written changelog has the reasons, until you stop writing it. Seldon
keeps both in one place: the facts it collects itself, and the reasons
you or your agents give.

## Features

- Six collectors read pacman, snapper, Omarchy, plugins, the theme and
  your watched config files into an append-only ledger. A capture never
  records the same change twice.
- You plan a change as a case: a Markdown file with a zone (green,
  yellow, red), a risk class and a plan. It moves from queued to active,
  verification and completed.
- A change that no case covers is drift. You link it to a case, explain
  it or dismiss it, from the terminal or the panel.
- Claude Code's hooks, or one `seldon hook` command for any other agent,
  record each changing command with the agent's name and the active
  case, for agent sessions inside the logbook and for the sessions Seldon
  starts (from `~/Work`, as Omarchy's own agent). The hooks sit in
  `~/.claude/settings.json` and do nothing in your other sessions unless
  `config.toml` sets `[hooks] scope = "all"`. Every
  logbook carries an `AGENTS.md` with the rules agents follow there.
- In the Omarchy shell, a bar pill counts active cases and crises. A
  panel with six tabs, which also counts the other changes without a
  case, and the Prime Radiant overlay with charts follow your Omarchy
  theme.
- The logbook is Markdown and YAML in `~/Seldon`, a git repository and,
  if you want, an Obsidian vault. `seldon rebuild` writes a guide that
  turns a fresh install into this machine again. Nothing leaves your
  machine.

## Quick start

You need Omarchy 4 and a terminal. Nothing here needs `sudo`. Three
steps, about two minutes.

<a name="install"></a>

### 1. Install the engine

The engine is one program, `seldon`.

**AUR package: coming soon. Until then install the engine from GitHub
(below).**

`install.sh` downloads the static `seldon` binary of a release, checks it
against the release's `SHA256SUMS` and refuses on a mismatch, then
installs `~/.local/bin/seldon` (and the alias `jax-seldon`). It runs as
your user and never asks for root. With the GitHub CLI (`gh`) installed
and logged in, it also checks that the release workflow of this
repository built the download (`gh attestation verify`); without `gh` it
says that only the checksum was checked, and `--require-verified` makes
it refuse instead ([SECURITY.md](SECURITY.md#verifying-a-release)).

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

Check it with `seldon --version`. If your shell says `command not found`,
open a new terminal: Omarchy puts `~/.local/bin` on your `PATH` when a
shell starts.

### 2. Create your logbook

```sh
seldon init
```

It asks one question, where the logbook lives (`~/Seldon` by default),
and takes the defaults for the rest: the first capture looks back 90 days
and records that as history "before Seldon", without asking.
`seldon init --defaults` asks nothing; `seldon init --ask` is the full
wizard. Then check the setup:

```sh
seldon doctor
```

Every line should say `ok`. On Omarchy, `snapper` may say `degraded`:
your user cannot read snapshots yet. Seldon works without them. `doctor`
prints the one command that allows it; it changes the root snapper
config, so Seldon never runs it for you.

### 3. Add the plugin

```sh
omarchy plugin add https://github.com/JohnAndrewsX/jax-seldon-plugin.git --enable
```

Omarchy asks before it clones the plugin. A pill with the Seldon mark
then appears on the right of the bar: a left click opens the panel, a
middle click the Prime Radiant. If the panel shows a banner instead of
data, the banner's button is the fix. Seldon records the plugin's
installation as routine history, not as drift. You can also add the
plugin first: its setup card installs the engine and creates the logbook
for you, one button each
([Getting started](docs/user/en/01-getting-started.md)).

## A 60-second tour

Change something without telling Seldon first. Omarchy's `~/.bashrc` has
a place for your own aliases; one line there is quick and easy to undo.
Then ask Seldon what it saw:

```sh
echo "alias gs='git status'" >> ~/.bashrc
seldon capture
seldon drift
```

```text
yellow     2026-10-09 19:22  config/config-change  ~/.bashrc  01M4GV0KZY6ZYF5BBTFT5DXZ48
1 open drift item(s), 0 crisis
```

The change is drift: no case covers it. It is no crisis, so the pill
stays as it was: by default its number after the mark counts crises
only. The panel's Today tab counts it under *without a case*; the plugin
setting `driftInBar` set to `all` counts it in the bar too. The plugin
captures by itself every 15 minutes; `seldon capture` just saves you the
wait. Explain the change with the event id from the last column:

```sh
seldon drift explain <EVENT> -- "A short git status"
```

Seldon records your reason as a completed case and the drift is gone.
Some changes are no drift at all: a theme switch, a plugin toggle, a
system upgrade and adding Seldon's own plugin are routine history.
The next change you plan first:

```sh
seldon plan new --area shell -- "Remove the gs alias again"
seldon plan start <ID>
seldon log --case <ID> -- "Removing the gs alias again"
```

`plan new` prints the case id. Delete the alias line from `~/.bashrc`,
run `seldon capture` again and link the new change to the case with
`seldon drift link <EVENT> <ID>`. Close the case with
`seldon plan verify <ID>` and `seldon plan done <ID>`. A change an agent
makes through the hooks carries the active case on its own.

Open the panel and press `2` for the Changelog: both changes, the note
and every step of the case are there. A middle click on the pill opens
the Prime Radiant. Each step is also a git commit in your logbook:
`git -C ~/Seldon log --oneline`.

The full walk-through, with the output of every command, is
[Getting started](docs/user/en/01-getting-started.md).

## Install options, update and removal

Options (with the one-liner: `| bash -s -- --unit`):

| Option | Effect |
|---|---|
| `--version vX.Y.Z` | that release instead of the latest |
| `--prefix DIR` | install into `DIR/bin` instead of `~/.local/bin` |
| `--unit` | also install the optional watcher unit into `~/.config/systemd/user/` (installed, not enabled; see [`engine/systemd/README.md`](engine/systemd/README.md)) |
| `--force` | replace a `seldon` (or unit) the script did not install, such as a self-built binary; without it the script refuses and changes nothing |
| `--require-verified` | install only when `gh` verified the download's build provenance; refuses when `gh` is missing or not logged in, or for a release up to v0.1.1 (made before attestations) |
| `--skip-provenance` | do not ask `gh`; only the checksum is checked (a note says so). For a `gh` that fails on its own, for example behind a proxy; not with `--require-verified` |
| `--uninstall` | remove what the script installed (give the same `--prefix`) |

- **Update:** run `install.sh` again, either form. It replaces `seldon`
  when the release is newer; with the same version it changes nothing.
  Update the plugin with `omarchy plugin update jax.seldon`, then restart
  the shell with `omarchy-restart-shell`: until it restarts, the shell
  keeps running the old plugin code. The panel then shows "Restart the
  shell to finish the update" with a *Restart shell* button; a plugin
  version without this notice shows nothing, so restart after every
  update.
- **Remove:** `bash install.sh --uninstall`, or with the one-liner
  `… | bash -s -- --uninstall`. It removes exactly the files it
  installed (a file you changed since is kept, and it says so). Your
  logbook, `~/.config/seldon/` and `~/.local/state/seldon/` stay. Remove
  the plugin with `omarchy plugin remove jax.seldon`.

### Engine from the AUR

Once the package is live:

```sh
omarchy pkg aur add jax-seldon   # install
yay -S jax-seldon                # update
omarchy pkg drop jax-seldon      # remove
```

Install from one source only: both put a `seldon` on your `PATH`.

## Documentation

| You want to | Read |
|---|---|
| Use Seldon, step by step | User guide: [English](docs/user/en/README.md) · [Deutsch](docs/user/de/README.md) |
| Get going in fifteen minutes | [Getting started](docs/user/en/01-getting-started.md) · [Erste Schritte](docs/user/de/01-getting-started.md) |
| Know every pill, panel and overlay key | [Plugin README](plugin/README.md) |
| Let an AI agent work in your logbook | [Agent guide](docs/AGENT-GUIDE.md) · [`llms.txt`](llms.txt) |
| Understand how it works | [Concept](docs/CONCEPT.md) · specs for the [logbook](docs/SPEC-LOGBOOK.md), [engine](docs/SPEC-ENGINE.md), [plugin](docs/SPEC-PLUGIN.md) and the [contract](docs/CONTRACT.md) · [decisions](DECISIONS.md) |
| Work on Seldon itself | [Contributing](CONTRIBUTING.md) · [Development](docs/DEVELOPMENT.md) |
| See what changed | [Changelog](CHANGELOG.md) · [Versioning](docs/VERSIONING.md) |

An AI agent started inside a logbook follows that logbook's `AGENTS.md`;
an agent working on this repository follows the repository's
[`AGENTS.md`](AGENTS.md).

## Project status

v0.1.4 (2026-10-08): Seldon stays quiet (routine changes are history, the
bar counts only what can break boot, login or the shell), one click
starts an agent on a case, and the shell no longer crashes on a restart
with the pill on two monitors. Before 1.0.0 a
minor release may still change the CLI, the logbook layout or the
contract; the [changelog](CHANGELOG.md) then says so under Breaking.
Next: the AUR package, the listing in the Omarchy plugin directory and
the update-impact report; then 1.0.0.

## Contributing

Issues and pull requests are welcome. Please open an issue before you
write anything larger than a fix, so it does not collide with planned
work. [CONTRIBUTING.md](CONTRIBUTING.md) has the build (`just check`),
the rules and how review works; everyone taking part follows the
[Code of Conduct](CODE_OF_CONDUCT.md).

## Security

Report a vulnerability privately through
[GitHub's vulnerability reporting](https://github.com/JohnAndrewsX/jax-seldon/security/advisories/new),
never in a public issue. [SECURITY.md](SECURITY.md) has the scope and the
alternative by e-mail. The engine has no network code and needs no root.
The plugin runs inside the Omarchy shell; what it reads and runs is listed
in its [README](plugin/README.md#security-privacy-privileges).

## Licence

[MIT](LICENSE) © JohnAndrewsX. Plugin id `jax.seldon`, AUR package
`jax-seldon`.

## Acknowledgements

- [Omarchy](https://omarchy.org) and its shell, which Seldon is built for.
- [Quickshell](https://quickshell.org) and [Hyprland](https://hyprland.org),
  which the plugin runs on.
- Isaac Asimov's *Foundation*, for the name and the Prime Radiant.
- [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the
  [Contributor Covenant](https://www.contributor-covenant.org/), which
  this repository follows.
- The Rust crates the engine is built from, listed in
  [`engine/Cargo.toml`](engine/Cargo.toml).
