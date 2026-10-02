# Configuration

This page describes everything you can set: the engine's `config.toml`
with its collectors, watched paths, redaction, drift list and agent
launcher, then the plugin's settings and the keybinding.

## The config file

The engine reads `~/.config/seldon/config.toml`. `seldon init` writes it,
and you can edit it with any editor. A change takes effect with the next
`seldon` command; nothing needs a restart.

Two things to know before you edit it:

- `seldon init` rewrites the file. It keeps keys it does not know, but it
  drops comments and reorders the keys. Keep notes elsewhere.
- A file that is not valid TOML stops every command with exit 1. The
  error names the file and the problem.

To use another file for one command, pass `--config <FILE>` or set
`SELDON_CONFIG`.

## A complete example

This is the file the wizard writes with its defaults, plus an agent
launcher. Your `logbook` line holds your own path.

```toml
harnesses = ["claude-code"]
language = "en"
logbook = "/home/you/Seldon"
watchPaths = ["~/.config/hypr", "~/.config/omarchy", "~/.config/waybar", "~/.bashrc", "~/.zshrc"]

[collectors]
config = true
omarchy = true
pacman = true
plugins = true
snapper = true
theme = true

[drift]
alwaysRed = ["linux*", "systemd", "glibc", "hyprland", "omarchy", "quickshell"]

[git]
autocommit = true

[redaction]
patterns = []
skipPaths = []

[agent]
launcher = ["omarchy", "agent", "prompt", "{prompt}"]
```

## Keys

| Key | Default | Meaning |
|---|---|---|
| `logbook` | `~/Seldon` | the logbook folder |
| `language` | from your locale | `en` or `de`: the language of the text the engine writes into the logbook (journal lines, `STATUS.md`) |
| `watchPaths` | see [Watched paths](#watched-paths) | files and folders the config collector watches |
| `harnesses` | `[]` | the agent harnesses you chose in the wizard; for your information |
| `[collectors]` | all `true` | which collectors a capture runs |
| `[git] autocommit` | `true` | commit the logbook after every command that writes |
| `[redaction] patterns` | `[]` | your own secret patterns, see [Redaction](#redaction) |
| `[redaction] skipPaths` | `[]` | files the engine never opens or names |
| `[drift] alwaysRed` | six names | packages whose upgrade is always a crisis, see [Drift](#drift) |
| `[agent] launcher` | `omarchy agent prompt` | what `seldon agent start` runs, see [Agent launcher](#agent-launcher) |
| `[agent.launchers]` | none | more launchers by name |

The logbook path is taken from, in this order: `--logbook`,
`SELDON_LOGBOOK`, `logbook` in the config, `~/Seldon`.

## Collectors

| Collector | Reads | Records |
|---|---|---|
| `snapper` | `snapper --jsonout list` | snapshots created and deleted |
| `pacman` | `/var/log/pacman.log` | package installs, removals, upgrades, downgrades, grouped by transaction |
| `omarchy` | `omarchy-version` | Omarchy version changes |
| `plugins` | `omarchy plugin list --json` | shell plugins added, removed, enabled, disabled, updated |
| `theme` | `~/.local/state/omarchy/current/theme.name` | theme switches |
| `config` | the files under `watchPaths` | files added, changed, removed |

Each collector only reads. Turn one off with `false`; `seldon capture
--source <name>` still runs it on demand.

A collector that cannot read its source is **degraded**: the capture goes
on, and `seldon doctor` names the fix. Two cases are normal:

- `snapper` needs your user in the snapper config's `ALLOW_USERS`. Omarchy
  does not set it. The fix is
  `sudo snapper -c root set-config ALLOW_USERS=$USER SYNC_ACL=yes`, which
  you run yourself, or not at all.
- `plugins` asks the running Omarchy shell. When you capture from a TTY
  without a desktop session, it is degraded for that capture.

## Watched paths

`watchPaths` lists files and folders. The config collector hashes every
file below them at each capture and records what was added, changed or
removed. It records the path and two short hashes, never the content.

Defaults: `~/.config/hypr`, `~/.config/omarchy`, `~/.config/waybar`,
`~/.bashrc`, `~/.zshrc`. Missing paths are skipped. Add your own, for
example:

```toml
watchPaths = ["~/.config/hypr", "~/.config/omarchy", "~/.config/waybar", "~/.bashrc", "~/.zshrc", "~/.config/nvim", "~/.config/systemd/user"]
```

Always left out:

- `~/.config/omarchy/plugins/` (the plugins collector covers it);
- `.git` folders, and folders reached through a symlink;
- binary files and files over 1 MiB (listed as skipped, without a hash);
- everything in `[redaction] skipPaths`.

Unit files under `~/.config/systemd/` are red zone. Everything else here
is yellow.

When you add a path, the next capture records every file in it as
added, and each one opens as drift. Dismiss them, for example with
`seldon drift dismiss <EVENT> -- "started watching ~/.config/nvim"`.
Add a folder with many files only if you want to follow each one.

## Redaction

Before the engine writes an event, it removes secrets from the command
line and the detail text. A redacted value reads `‹redacted›`. The
built-in rules cover:

- `--password`, `token=`, `Authorization:` and their values;
- AWS access keys (`AKIA…`), GitHub tokens (`ghp_…`), API keys (`sk-…`);
- the password after `-p` for `mysql`, `psql` and `smbclient`;
- the user and password in a URL (`https://user:secret@host`).

The rules redact rather too much than too little. Add your own as
regular expressions; each one replaces its whole match:

```toml
[redaction]
patterns = ["MYAPP_KEY=\\S+", "xoxb-[0-9A-Za-z-]+"]
```

An invalid pattern is an error (exit 1): Seldon refuses to write rather
than leak.

`skipPaths` names files the config collector and the hooks never open,
hash or name:

```toml
[redaction]
skipPaths = ["~/.config/hypr/secrets.lua", "*.key", "**/tokens/**"]
```

| Pattern | Matches |
|---|---|
| with a `/`, such as `~/.config/app/secret.conf` | that full path, or that folder and everything in it |
| a relative path, such as `app/secret.conf` | that path below any folder |
| a name without `/`, such as `*.key` | any file or folder with that name |

`*` and `?` stay within one path part; `**` crosses parts.

Redaction helps, but it only catches what it recognises. Never put a
secret into a note, a case or a command line if you can avoid it.

## Drift

`[drift] alwaysRed` lists package names whose routine upgrade is still
a crisis. A routine upgrade is a transaction that only upgrades packages
from a full system upgrade (`omarchy update` does one). It opens yellow
drift, so it does not raise an alarm. A package on this list makes the
whole transaction red.

```toml
[drift]
alwaysRed = ["linux*", "systemd", "glibc", "hyprland", "omarchy", "quickshell", "nvidia*"]
```

`*` matches any ending. Keep the defaults: these packages can break boot,
login or the shell.

## Agent launcher

`seldon agent start` (and *Start agent* in the panel) runs the launcher.
The default starts Omarchy's default agent in its own terminal window:

```toml
[agent]
launcher = ["omarchy", "agent", "prompt", "{prompt}"]
```

A launcher is a list: the program, then its arguments. Rules:

- Exactly one element is `{prompt}`. The engine replaces it with the
  prompt, as one argument.
- The program is a name on your `PATH` (without `/`) or an absolute path.
- Shells (`bash`, `sh`, `zsh`, `fish` and others), `eval`, `hyprctl` and
  the Omarchy launchers that build shell strings are refused. The prompt
  carries text from your logbook, and Seldon never runs that text as a
  command.

More launchers go under `[agent.launchers]`; pick one with
`seldon agent start <ID> --launcher <NAME>`. The name `omarchy` always
reaches the built-in default.

```toml
[agent]
launcher = ["alacritty", "-e", "claude", "{prompt}"]

[agent.launchers]
codex = ["alacritty", "-e", "codex", "{prompt}"]
```

The launcher starts detached in the logbook folder. Its error output goes
to `~/.local/state/seldon/agent-launch.log`.

## Git

With `autocommit = true` every command that writes commits the logbook
with a message such as `seldon: C-2026-004 active`. The commit takes the
whole folder, so edits you made in your editor since the last command go
along with it. The history is your backup and your undo.

With `false`, or with `--no-commit` on one command, the engine writes the
files and leaves committing to you. Seldon never pushes.

## Plugin settings

Change them in Omarchy's settings (Setup, Plugins, Seldon) or with
`omarchy bar set`:

| Key | Default | Meaning |
|---|---|---|
| `captureIntervalMin` | `15` | minutes between captures while the shell runs (5 to 120) |
| `wipLimit` | `3` | active cases the Work tab compares against (1 to 20); it warns, never blocks |

```sh
omarchy bar set jax.seldon captureIntervalMin 30 --json
```

To move the pill: `omarchy bar move jax.seldon --section right`.

## A key for the Prime Radiant

The plugin does not install a key binding. Add this line to your
Hyprland bindings to open the Prime Radiant with Super+Shift+S:

```text
o.bind("SUPER + SHIFT + S", "Seldon", "omarchy-shell shell toggle jax.seldon")
```

The panel has its own target. To toggle it from a key or a script:
`omarchy-shell jax.seldon.panel toggle`.

---

Previous: [CLI reference](05-cli-reference.md) · [Index](README.md) · Next: [The logbook](07-the-logbook.md)
