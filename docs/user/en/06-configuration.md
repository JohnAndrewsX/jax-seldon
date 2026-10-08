# Configuration

This page describes everything you can set: the engine's `config.toml`
with its collectors, watched paths, redaction, drift list and agent
launcher, then the plugin's settings and the keybinding.

## The config file

The engine reads `~/.config/seldon/config.toml`. `seldon init` writes it,
and you can edit it with any editor. A change takes effect with the next
`seldon` command; nothing needs a restart.

Before you edit it:

- `seldon init` rewrites the file. It keeps keys it does not know, but it
  drops comments and reorders the keys. Keep notes elsewhere.
- A file that is not valid TOML stops every command with exit 1. The
  error names the file and the problem.

To use another file for one command, pass `--config <FILE>` or set
`SELDON_CONFIG`.

## A complete example

This is the file the wizard writes when you take its defaults and tick
Claude Code. The `[agent]` section is left out of the file while it holds
the default; it is shown here so you see the key. Your `logbook` line
holds your own path.

```toml
harnesses = ["claude-code"]
language = "en"
logbook = "/home/you/Seldon"
watchPaths = ["~/.config/hypr", "~/.config/omarchy", "~/.config/waybar", "~/.bashrc", "~/.zshrc", "~/.local/share/applications", "~/.config/systemd/user", "~/.config/autostart", "~/.config/environment.d", "~/.config/uwsm", "~/.profile", "~/.bash_profile", "~/.local/state/omarchy/toggles"]

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
skipPaths = ["~/.config/omarchy/**/history.json", "~/.config/omarchy/**/history/", "~/.config/omarchy/**/state.json", "~/.config/omarchy/**/cache/", "~/.config/omarchy/**/*.log"]

[agent]
launcher = ["omarchy", "agent", "prompt", "{prompt}"]
```

## Keys

| Key | Default | Meaning |
|---|---|---|
| `logbook` | `~/Seldon` | the logbook folder; a relative path lies under your home folder |
| `language` | from your locale | `en` or `de`: the language `seldon init` gives a new logbook, see [Language](#language) |
| `watchPaths` | see [Watched paths](#watched-paths) | files and folders the config collector watches |
| `harnesses` | `[]` | the agent harnesses you chose in the wizard; for your information |
| `[collectors]` | all `true` | which collectors a capture runs |
| `[git] autocommit` | `true` | commit the logbook after every command that writes; `seldon init --no-git` writes `false` |
| `[redaction] patterns` | `[]` | your own secret patterns, see [Redaction](#redaction) |
| `[redaction] skipPaths` | plugin state files | files the engine never opens or names, see [Redaction](#redaction) |
| `[drift] alwaysRed` | six names | packages that can break boot, login or the shell, see [Drift](#drift) |
| `[drift] attention` | `"normal"` | `"all"` makes every change without a case drift again, see [Drift](#drift) |
| `[drift] routine` | every rule | the routine rules that apply, see [Drift](#drift) |
| `[drift] routinePaths`, `routinePackages`, `alwaysRedPaths` | see [Drift](#drift) | paths and packages that are routine, and the persistence paths |
| `[agent] launcher` | `omarchy agent prompt` | what `seldon agent start` runs, see [Agent launcher](#agent-launcher) |
| `[agent.launchers]` | none | more launchers by name |
| `[agent] workdir` | `"inherit"` | where the launcher starts; `"logbook"` starts it in the logbook folder, see [Agent launcher](#agent-launcher) |

The logbook path is taken from, in this order: `--logbook`,
`SELDON_LOGBOOK`, `logbook` in the config, `~/Seldon`.

Paths in the config file (`logbook`, `watchPaths`) may start with `~/`
or `$HOME/`. A relative path lies under your home folder, whatever folder
`seldon` runs in: the plugin and the agent hooks run it from different
folders. `--logbook` and `SELDON_LOGBOOK` are relative to the current
folder, as in any shell command.

### Language

`seldon init` writes the logbook in one language: the templates, and
later the text the engine adds (journal lines, `STATUS.md`). It takes
`--language`, else `language` from this file, else your locale (`de` for
a German locale, `en` otherwise), and writes the result here.

The logbook stores its language itself, in `.seldon/logbook.toml`, and
every command reads it from there. Changing `language` in this file
later does not change an existing logbook; it only sets the language
of the next logbook `seldon init` creates.

## Collectors

| Collector | Reads | Records |
|---|---|---|
| `snapper` | `snapper --jsonout list` | snapshots created and deleted |
| `pacman` | `/var/log/pacman.log` | package installs, removals, upgrades, downgrades, grouped by transaction |
| `omarchy` | `omarchy-version` | Omarchy version changes |
| `plugins` | `omarchy plugin list --json` | shell plugins added, removed, enabled, disabled, updated |
| `theme` | `~/.local/state/omarchy/current/theme.name` | theme switches |
| `config` | the files under `watchPaths` | files added, changed, removed |

Each collector only reads. Turn one off with `false`;
`seldon capture --source <name>` still runs it on demand.

A shell plugin you added with `omarchy plugin add` is a git clone. Its
update names the commits: how many a pull brought in or a rollback took
out, and up to 20 of their subjects, which the desk shows with the
change. Seldon only reads the clone with `git`; it never fetches. A
clone whose repository points outside the plugin folder (a linked
`.git`, shared objects, an included config) is not read, and the
change says so.

A collector that cannot read its source is `degraded`: the capture goes
on, and `seldon doctor` names the fix. Two cases are normal:

- `snapper` needs read access to the snapshot directory `/.snapshots`.
  Omarchy does not give it to your user. The fix is
  `sudo setfacl -m u:$USER:rx /.snapshots`, which you run yourself, or not
  at all. It lets Seldon read the snapshot list and the info files, nothing
  else: your user cannot create, change or delete snapshots with it. Files
  inside a snapshot keep their own permissions.
- `plugins` asks the running Omarchy shell. When you capture from a TTY
  without a desktop session, it is degraded for that capture. Over ssh or
  from cron it works while the shell runs on that machine: when
  `OMARCHY_PATH` is not set there, Seldon gives the Omarchy commands
  `/usr/share/omarchy`; a value you set is used as it is.

## Watched paths

`watchPaths` lists files and folders. The config collector hashes every
file below them at each capture and records what was added, changed or
removed. It records the path and two short hashes, never the content.

Defaults: `~/.config/hypr`, `~/.config/omarchy`, `~/.config/waybar`,
`~/.bashrc`, `~/.zshrc`, `~/.local/share/applications` (the desktop
entries of your web apps and TUIs), and the persistence paths
`~/.config/systemd/user`, `~/.config/autostart`,
`~/.config/environment.d`, `~/.config/uwsm`, `~/.profile`,
`~/.bash_profile` (files that run at login; a new one there is a crisis,
see [Drift](#drift)), and Omarchy's toggle folder
`~/.local/state/omarchy/toggles` (the switches of Omarchy's *Toggle*
menu and Hyprland's flags; turning one on or off is routine, anything
else there is listed). Missing paths are skipped. A relative path such as
`.config/nvim` means `~/.config/nvim`; the wizard stores the paths you
type in that form. Add your own, for example:

```toml
watchPaths = ["~/.config/hypr", "~/.config/omarchy", "~/.config/waybar", "~/.bashrc", "~/.zshrc", "~/.local/share/applications", "~/.config/systemd/user", "~/.config/autostart", "~/.config/environment.d", "~/.config/uwsm", "~/.profile", "~/.bash_profile", "~/.local/state/omarchy/toggles", "~/.config/nvim"]
```

The wizard writes the list into `config.toml`. A list that is still the
default of an earlier release gains the new defaults at the next capture,
which says so once in a `note:` line; only the `watchPaths` line of the
file changes. A list you changed yourself is kept as it is: `seldon
doctor` names the paths it lacks, and you add them the same way:

```toml
watchPaths = ["~/.config/hypr", "~/.config/omarchy", "~/.config/waybar", "~/.bashrc", "~/.zshrc", "~/.local/share/applications", "~/.config/systemd/user", "~/.config/autostart", "~/.config/environment.d", "~/.config/uwsm", "~/.profile", "~/.bash_profile", "~/.local/state/omarchy/toggles"]
```

The files already there when the path enters the list are taken as they
are: the next capture records no addition for them, and says
`watch scope changed: 0 file(s) left it, N entered it`.

`~/.ssh/authorized_keys` and `~/.ssh/authorized_keys2` (the two files
sshd reads by default) are not watched unless you add them; add both
lines. Once they are in the list, a change to either without a case is a
crisis (both are in the default `alwaysRedPaths`); only their hashes are
recorded:

```toml
watchPaths = ["~/.config/hypr", "~/.config/omarchy", "~/.config/waybar", "~/.bashrc", "~/.zshrc", "~/.local/share/applications", "~/.config/systemd/user", "~/.config/autostart", "~/.config/environment.d", "~/.config/uwsm", "~/.profile", "~/.bash_profile", "~/.local/state/omarchy/toggles", "~/.ssh/authorized_keys", "~/.ssh/authorized_keys2"]
```

Always left out:

- `~/.config/omarchy/plugins/` (the plugins collector covers it: it
  hashes each third-party plugin's folder as one whole and records an
  edit as one plugin update);
- `~/.local/share/applications/mimeinfo.cache`: a cache built from the
  desktop entries, rewritten on many package updates;
- `.git` folders, and folders reached through a symlink — except in the
  persistence paths (`alwaysRedPaths`), where a linked folder is followed:
  each folder once, at most 4096 entries below links per capture. A link
  with more is recorded as cut off, which is a crisis: nobody can see
  what runs from it. Links into your logbook or Seldon's own folders are
  never followed;
- binary files and files over 1 MiB (listed as skipped, without a hash) —
  except in the persistence paths, where every file is hashed: a hook
  runs whatever it holds. A file there over 64 MiB is hashed from its
  size, times and inode instead of being read, and one that cannot be
  read keeps its last hash until it can. Every file in the toggle folder
  is hashed the same way;
- files whose name holds a control character, or whose path is longer
  than 512 characters: the capture counts them in a warning;
- everything in `[redaction] skipPaths`. Its default holds the files that
  shell plugins keep in a folder of their own under `~/.config/omarchy/`
  and rewrite every few minutes: `history.json`, `state.json`, the
  `history/` and `cache/` folders and `*.log` files. Without it, each
  rewrite would open one more drift item. Add a plugin's other busy
  files the same way.

A file whose size, modification time, change time and inode are the
same as at the last capture is not read again. The change time catches
an edit whose modification time was put back (`touch -r`).

Unit files under `~/.config/systemd/` are red zone. Everything else here
is yellow. The zone says where a change acts; whether a change without a
case is a crisis is decided by [Drift](#drift).

Changing what is watched is not a change of files. When you add a path,
or take a pattern out of `skipPaths`, the next capture takes the files
that come into view as they are, without an event. When you remove a
path, or add a pattern, the files that leave the view are not recorded
as removed. The capture says it in one line, for example `watch scope
changed: 514 file(s) left it, 0 entered it; no events for them`. From
then on the new files are watched like the others. Add a folder with
many files only if you want to follow each one.

## Redaction

Before the engine writes anything, it removes secrets from the text: from
every field of an event (the command line, the subject, the detail text
and the other values), and from the text and tags you give `seldon log`,
`seldon plan new`, a step's `--reason`, `seldon decide` and
`seldon drift explain` or `dismiss`. The journal, the case and decision
files and `STATUS.md` therefore hold the same text as the ledger. A
redacted value reads `‹redacted›`. The built-in rules cover:

- `--password`, `--token`, `--with-token`, `--secret`, `--passphrase`
  and similar options, and `token=`, `PASSWORD=`, `PGPASSWORD=`,
  `MYSQL_PWD=`, `SECRET=` and other `…PASSWORD=`, `…SECRET=`, `…_PASS=`
  assignments, with any value;
- `--api-key`, `--access-key`, `--secret-key` and `API_KEY=` and other
  `…KEY=` assignments, when the value looks like a credential: at least
  16 characters, or at least 8 that mix two of lower case, upper case,
  digits and other characters (so `sort --key=2` and `hotkey=Super` stay
  as they are);
- `Authorization:`, `X-Api-Key:`, `Private-Token:` and other headers
  whose name ends in Key, Token, Secret or Auth, also a value in quotes
  (`Authorization: "Bearer …"`, `"Authorization": "…"` in JSON), and the
  cookies after
  `Cookie:` and `Set-Cookie:` (a `name=value`; `cookie: banner fixed`
  stays);
- the value of a JSON key such as `"password"`, `"passwd"`,
  `"client_secret"`, `"access_token"`, `"api_key"`, `"x-api-key"` or
  `"apiKey"` in
  inline JSON (`curl -d '{"password": "…"}'`); `"password_hint"` stays;
- AWS access keys (`AKIA…`, `ASIA…`), GitHub tokens (`ghp_…`, `gho_…`,
  `github_pat_…` and the other `gh…_` forms), GitLab tokens (`glpat-…`),
  Slack tokens (`xoxb-…`), API keys (`sk-…`, `sk_…`);
- the password after `-p` for `mysql`, `psql` and `smbclient`, after
  `sshpass -p` and after `docker login -p` (also `podman`);
- the user and password after `curl -u`, the cookies after `curl -b`
  or `--cookie` (not a cookie file name);
- the certificate and password after `curl -E`/`--cert`, the value
  after `http`/`xh -a`;
- the `pass:…` value of openssl's `-pass`, `-passin`, `-passout` and
  similar options; `env:`, `file:`, `fd:` and `stdin` stay as written;
- proxy credentials: after `curl -U`, `--proxy-user` and
  `--proxy-password`, and `user:pass@` in the proxy after `curl -x`,
  `--proxy` or in `https_proxy=`;
- a PEM private key (`-----BEGIN OPENSSH PRIVATE KEY-----` and the
  other `… PRIVATE KEY` blocks): everything between its BEGIN and END
  lines becomes one `‹redacted›`;
- nmcli's passwords and keys: the value after `password`, `wifi-sec.psk`,
  `802-1x.password`, `vpn.secrets` and the other secret properties;
- the user and password in a URL (`https://user:secret@host`), also
  when the password contains `/`, `?`, `#` or `:`.
- the part before the `@` of an e-mail address: `me@example.com` reads
  `‹redacted›@example.com`. An SSH remote (`git@github.com:owner/repo`),
  `user@host` without a dot, package versions (`pkg@1.2.3`) and systemd
  units (`getty@tty1.service`) stay; `ssh me@host.example` looks like an
  address and is masked too.

Text written before a rule existed stays as it is.

The rules redact rather too much than too little. Add your own as
regular expressions; each one replaces its whole match:

```toml
[redaction]
patterns = ["MYAPP_SESSION=\\S+", "acme_[0-9A-Za-z]{24}"]
```

The domain of an address stays visible. If yours names you, add a
pattern for it: `"@smith\\.example\\b"` turns `jo@smith.example` into
`‹redacted›‹redacted›`.

An invisible character inside a secret (a zero-width space, a variation
selector) does not hide it: the rules, yours included, read the text
without such characters. A text with nothing to redact keeps them.

An invalid pattern is an error (exit 1): Seldon refuses to write rather
than leak.

`skipPaths` names files the config collector and the hooks never open,
hash or name. The default covers busy plugin files (see [Watched
paths](#watched-paths)). An empty list, `skipPaths = []` as earlier
versions of `seldon init` wrote it, also means the default. A list of
your own replaces the default, so copy its patterns from the example
above into your list if you want to keep them.

```toml
[redaction]
skipPaths = ["~/.config/hypr/secrets.lua", "*.key", "**/tokens/**"]
```

The name of a file under a watched path is the subject of its events.
A desktop entry you named after an account, such as a web app
`Mail (me@example.com).desktop`, therefore reaches the logbook as
`Mail (‹redacted›@example.com).desktop`. To keep such a file out
altogether, add its name to `skipPaths`, for example `"*@*.desktop"`
for every desktop entry with an `@` in its name; events written before
keep the name they were written with.

| Pattern | Matches |
|---|---|
| with a `/`, such as `~/.config/app/secret.conf` | that full path, or that folder and everything in it |
| a relative path, such as `app/secret.conf` | that path below any folder |
| a name without `/`, such as `*.key` | any file or folder with that name |

`*` and `?` stay within one path part; `**` crosses parts and stands for
at least one folder: `~/.config/omarchy/**/state.json` matches
`~/.config/omarchy/app/state.json`, not `~/.config/omarchy/state.json`.

Redaction helps, but it only catches what it recognises. Never put a
secret into a note, a case or a command line if you can avoid it.

## Drift

Every change is recorded. `[drift]` decides which changes without a case
are drift, and which of those are a crisis (see
[Concepts](02-concepts.md#drift)). The defaults are quiet: routine changes
are history, packages and overrides are listed without a demand, and
only what can break boot, login or the shell is a crisis.

| Key | Default | Meaning |
|---|---|---|
| `alwaysRed` | `linux*`, `systemd`, `glibc`, `hyprland`, `omarchy`, `quickshell` | packages that can break boot, login or the shell: installed or removed by name outside a case, a crisis; upgraded with the system, routine |
| `attention` | `"normal"` | `"all"`: every change without a case is drift, a crisis when its zone is red (the behaviour up to 0.1.3) |
| `routine` | all rules | the routine rules that apply: `sysupgrade`, `upgrade`, `keyring`, `omarchy-update`, `plugin-toggle`, `theme`, `omarchy-default`, `system-link`, `routine-paths`, `theme-assets`, `theme-repo`, `toggle-flag` |
| `routinePaths` | `~/.config/omarchy/shell.json`, `**/*.bak.*` | config files whose changes are routine |
| `routinePackages` | `archlinux-keyring`, `omarchy-keyring` | packages whose own transactions are routine |
| `alwaysRedPaths` | `~/.config/systemd/user/**`, `~/.config/omarchy/hooks/**`, `~/.config/autostart/**`, `~/.config/environment.d/**`, `~/.config/uwsm/**`, `~/.profile`, `~/.bash_profile`, `~/.ssh/authorized_keys`, `~/.ssh/authorized_keys2` | persistence paths: a change there without a case is a crisis (the `authorized_keys` files only once you watch them) |

Want more? A few examples:

```toml
[drift]
# theme switches are drift again
routine = ["sysupgrade", "upgrade", "keyring", "omarchy-update", "plugin-toggle", "omarchy-default", "system-link", "routine-paths", "theme-assets", "theme-repo", "toggle-flag"]
# a kernel from NVIDIA counts too
alwaysRed = ["linux*", "systemd", "glibc", "hyprland", "omarchy", "quickshell", "nvidia*"]
```

```toml
[drift]
# everything without a case is drift, as up to 0.1.3
attention = "all"
```

`*` matches any ending. Keep the `alwaysRed` defaults: these packages can
break boot, login or the shell. The engine writes these keys into the
file only when you change them, and `seldon doctor` prints the rules in
use and marks what is not the default. `seldon drift --all` also lists the
routine changes.

The bar counts crises only. To count every change without a case, or
nothing, set the plugin's `driftInBar` (see
[Plugin settings](#plugin-settings)).

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
- Before `{prompt}`, programs known to run their arguments as code are
  refused: shells (`bash`, `sh`, `zsh`, `fish` and others), interpreters
  (`python`, `perl`, `node` and others), programs that hand a string to a
  shell (`script`, `watch`, `flock`, `su`, `ssh`, `tmux`, `screen`,
  `xargs` and others), `eval`, `hyprctl`, the Omarchy launchers that
  build shell strings, `env -S` and `sudo -s`/`-i`. A version suffix does
  not change the name (`python3.12` is `python`). The check goes by
  program name: a heuristic, not a sandbox.
- The prompt names the case and the logbook and holds no logbook text.
  It is a command-line argument, visible in the process list (`ps`) while
  the agent runs.

More launchers go under `[agent.launchers]`; pick one with
`seldon agent start <ID> --launcher <NAME>`. The name `omarchy` always
reaches the built-in default.

```toml
[agent]
launcher = ["alacritty", "-e", "claude", "{prompt}"]

[agent.launchers]
codex = ["alacritty", "-e", "codex", "{prompt}"]
```

The launcher starts detached where `omarchy agent prompt` would start the
agent: in the folder `seldon agent start` runs in, and in `~/Work` (your
home when there is none) when that folder is your home or `/`, as from
the panel. Agents trust `~/Work`, and Seldon's hooks record the session
there because Seldon started it. To start agents in the logbook folder,
as before 0.1.4, and keep the hooks in the logbook's settings (no
user-wide copy is made):

```toml
[agent]
workdir = "logbook"
```

Its error output goes to `~/.local/state/seldon/agent-launch.log`.

## Git

With `autocommit = true` every command that writes commits the logbook
with a message such as `seldon: C-2026-004 active`. Closing a case names
it: `seldon: C-2026-004 completed — Install Zed: SUPER+E opens Zed`, the
title and the first line of its *Result* (for a drop, the reason). The
commit takes the whole folder, so edits you made in your editor since the last command go
along with it. The history is your backup and your undo.

With `false`, or with `--no-commit` on one command, the engine writes the
files and leaves committing to you. Seldon never pushes.

When you answer no to git in `seldon init` (or pass `--no-git`), it
writes `autocommit = false`: the logbook is no repository, and
`seldon doctor` reports that as your choice, not as a fault. To start
using git later, run `git -C <logbook> init` and set `autocommit = true`.

`seldon init` takes its git default from `autocommit` in an existing
file: with `autocommit = false` it makes no repository and keeps the
value, unless you pass `--git` or say yes in the wizard.

## Plugin settings

Change them in Omarchy's settings (Setup, Plugins, Seldon) or with
`omarchy bar set`:

| Key | Default | Meaning |
|---|---|---|
| `captureIntervalMin` | `15` | minutes between captures while the shell runs (5 to 120) |
| `wipLimit` | `3` | active cases the Work tab compares against (1 to 20); it warns, never blocks |
| `driftInBar` | `crisis` | what the bar's second number counts: `crisis`, `all` (every change without a case, as up to 0.1.3) or `none`; the bar turns to the error colour on a crisis in every mode |

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
