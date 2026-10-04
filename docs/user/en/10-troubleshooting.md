# Troubleshooting

This page helps when something looks wrong: it starts with
`seldon doctor`, then goes through the panel's banners, the engine's
exit codes and the most common problems.

## Start with doctor

```sh
seldon doctor
```

It checks eleven things and prints a fix for each one that is not `ok`.
It only reads: it changes no file, takes no lock and runs nothing with
`sudo`.

| Check | `ok` means | When it is not ok |
|---|---|---|
| `engine` | the engine runs; its version and contract | (if you can run `doctor`, this is ok) |
| `config` | `~/.config/seldon/config.toml` was read, and its `[redaction] patterns` compile | `degraded`: no config yet, defaults in use; fix `seldon init`. `error`: the file cannot be read, is not valid, or has a pattern that does not compile; every command stops on it. When the file cannot be read or parsed, the logbook is "not checked" (its path is in that file) |
| `logbook` | the logbook exists; machine, language, counts | `error`: not initialised at that path; the fix names the `seldon init` command |
| `cases` | every case id has one file | `error`: a case exists twice (a stale copy); keep the file in the folder of its status |
| `ledger` | every line of `ledger/*.jsonl` is an event | `degraded`: lines that are not events (a torn write, a hand edit) are skipped; the row names month, count and lines |
| `fences` | the generated parts of `STATUS.md` and `DECISIONS.md` have their marker lines | `degraded`: a marker line is missing, so `seldon status` leaves the file alone; or an end marker closes no fence. `error`: the file cannot be read |
| `collectors` | the last capture of every enabled collector succeeded | `degraded`: the row lists each failing collector with its message and fix |
| `state` | `cursors.json`, `manifest.json` and `owned.json` in `~/.local/state/seldon` can be read | `error`: the file is corrupt or unreadable; the row says what that breaks; the fix moves a corrupt file away or makes an unreadable one readable |
| `omarchy` | `omarchy-version` answered | the Omarchy collector cannot read the version |
| `snapper` | snapshots can be listed | `degraded`: your user may not list snapshots; see [Snapshots are not recorded](#snapshots-are-not-recorded) |
| `git` | git is there; the logbook is a repository | git is missing, or the logbook is not a repository; autocommit is off then. `degraded`: something keeps every autocommit from committing (a stale `.git/index.lock`, a detached HEAD, …); the fix says what to do |

`doctor --json` prints the same as JSON. The plugin reads it to choose
its banner.

## Banners in the panel

When something is wrong, the panel shows one banner at the top, with a
button that fixes it.

| Banner | Cause | Fix |
|---|---|---|
| Seldon engine not installed | the plugin cannot run `seldon` | *Install in terminal* runs the GitHub installer in a terminal you see; or install it yourself ([Getting started](01-getting-started.md#step-1-install-the-engine)), then *Check again* |
| Logbook not initialised | there is no logbook yet | *Run in terminal* runs `seldon init` |
| No index yet / Index unreadable | `~/.local/state/seldon/index.json` is missing or broken | *Build index* runs `seldon status` |
| Index is stale | the index is more than two hours old | *Capture now* |
| Index format mismatch | the plugin and the engine speak different versions of the index | update the older one. Plugin: `omarchy plugin update jax.seldon`. Engine: *Update in terminal* runs the installer again (until the AUR package exists; see [Update and uninstall](11-update-and-uninstall.md)) |
| Engine too old | the engine is older than this plugin needs (the `engineMin` in its manifest) | *Update in terminal* runs the installer again (until the AUR package exists; see [Update and uninstall](11-update-and-uninstall.md)), then *Check again* |
| Snapshots not readable | snapper refuses your user | *Run in terminal* runs the one-time snapper fix; you type your password there |

The plugin looks for the engine when the shell starts and when you press
*Check again*. After you installed the engine, press *Check again*, or
restart the shell with `omarchy-restart-shell`.

An index goes stale when no capture ran for two hours, for example after
the machine slept. The next timed capture fixes it on its own.

## Exit codes

Every `seldon` command ends with one of these codes:

| Code | Meaning | What to do |
|---|---|---|
| 0 | ok | |
| 1 | user error: a wrong argument, an unknown case or event, a step the case cannot take | read the message; `seldon <command> --help` |
| 2 | engine error | run the command again with `--json` and keep the message for a bug report |
| 3 | the logbook is not initialised | `seldon init`, or check `--logbook` and `SELDON_LOGBOOK` |
| 4 | another `seldon` holds the lock | wait a moment and run it again |

## Common problems

### "another seldon process holds the lock"

Only one `seldon` writes at a time. A capture from the plugin or an
agent's hook may be running. Wait a second and try again. If it never
clears, look for a hanging process with `pgrep -a seldon`. The lock is
`~/.local/state/seldon/lock`; it is released when the process ends, so
you never delete it by hand.

### Snapshots are not recorded

`seldon doctor` says `snapper degraded: No permissions`. Omarchy does not
let your user list snapshots. Seldon works without them; the timeline
then has no snapshot markers. To allow it, run once:

```sh
sudo snapper -c root set-config ALLOW_USERS=$USER SYNC_ACL=yes
```

This changes the root snapper config. Seldon never runs it for you.
It adds your user to `ALLOW_USERS`, which has no read-only level: your
user can then also create, change and delete root snapshots without a
password, not only list them. Decide whether you want that.

### A change does not show up

- Run `seldon capture` and look at its per-collector lines. A collector
  with 0 may be off in `config.toml` or degraded.
- A config file is only seen when it lies under a watched path, is not
  larger than 1 MiB, is not binary and is not in `skipPaths`
  ([Configuration](06-configuration.md#watched-paths)).
- A package transaction that is still running is recorded once it ends.
- Changes by a program the hooks do not see (a GUI, a script) appear at
  the next capture, as drift without an agent name.

### The panel shows old data

The plugin reads the index, and the engine rewrites it after every
command. If you edited files in an editor, the index catches up with the
next command or capture. Press `c` in the panel, or run `seldon status`.
For edits to show at once, use the
[watcher](11-update-and-uninstall.md#the-optional-watcher).

If an action in the panel says the index is behind your logbook, you
changed the logbook elsewhere in the meantime. Capture, then try again.

### A lot of drift after a backfill

A backfill records older changes, and none of them has a case. Mark them
as the baseline: dismiss them with one reason. A group of packages is
one item, so this is usually a handful of commands:

```sh
seldon drift
seldon drift dismiss <EVENT> -- "pre-Seldon baseline"
```

### Drift I do not recognise

```sh
seldon drift show <EVENT> --json
```

shows the event with every member of its group, the actor and, for
packages, the command that ran. Check the journal and the agents' notes
of that day. If you still do not know, leave it open and write a note
in the journal. A wrong explanation misleads you later.

### An agent's commands are not recorded

- Claude Code: did you start it in the logbook folder and accept the
  trust prompt? Is `.claude/settings.json` there?
  `seldon hook install claude-code` adds what is missing.
- Is a case active? Green commands are recorded only then.
- Commands inside `xargs`, `find -exec` or `python -c` are not read.
- Other agents call `seldon hook generic` themselves
  ([Working with agents](04-working-with-agents.md#other-agents)).

### `seldon agent start` refuses or nothing opens

- The case must be active: `seldon plan start <ID>` first.
- A launcher that is a shell, or has no `{prompt}`, is refused; the
  message says why.
- If the agent window does not appear, read
  `~/.local/state/seldon/agent-launch.log`.

### The theme collector is degraded

It reads `~/.local/state/omarchy/current/theme.name`. Omarchy writes this
file the first time you set a theme. Set any theme once and the next
capture is ok.

### The plugins collector is degraded

It asks the running Omarchy shell. A capture from a TTY, over SSH or
while the shell restarts cannot reach it. The next capture in your
desktop session catches up.

## Logs

| What | Where |
|---|---|
| plugin warnings | `journalctl --user -t omarchy-shell` |
| agent launcher errors | `~/.local/state/seldon/agent-launch.log` |
| the watcher, if you run it | `journalctl --user -u seldon-watch` |
| what the plugin sees | `omarchy-shell jax.seldon.service status` and `omarchy-shell jax.seldon.panel view` (JSON) |

## Reporting a bug

Open an issue at <https://github.com/JohnAndrewsX/jax-seldon/issues>
with the command, the output of `seldon --version` and `seldon doctor`,
and the message from the same command with `--json`. Replace your user
name, machine name and private paths before you post; the output can
contain them.

---

Previous: [Import from omarchy-agent](09-import-from-omarchy-agent.md) · [Index](README.md) · Next: [Update and uninstall](11-update-and-uninstall.md)
