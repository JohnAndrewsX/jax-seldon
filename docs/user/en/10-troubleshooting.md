# Troubleshooting

This page helps when something looks wrong: it starts with
`seldon doctor`, then goes through the panel's banners, the engine's
exit codes and the most common problems.

## Start with doctor

```sh
seldon doctor
```

It checks thirteen things and prints a fix for each one that is not `ok`.
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
| `workpieces` | always ok, information only: how many `work/<case-id>/` folders no case owns (orphaned) or a closed case left over 10 MiB (oversized), their size, the oldest | (never; move or delete such a folder yourself when you no longer need it — git keeps its history) |
| `collectors` | the last capture of every enabled collector succeeded | `degraded`: the row lists each failing collector with its message and fix |
| `layout` | no folder or file where Seldon writes is a symbolic link, and none is a file where a folder belongs or the other way round | `error`: the row names what a command refuses (the first five); every command that writes there stops with exit 1, a linked `ledger/` stops `status` and `capture`. Fix: replace each link with the real folder or file it points to (move it into its place). `degraded`: only what no command refuses — a linked `STATUS.md`, `DECISIONS.md` or `ledger/<month>.md`, which `status` skips with a warning, or a link beside Seldon's files in `decisions/`, `work/…/`, `ledger/`, `system/` or `memory/`. Links elsewhere (a case template under `.seldon/templates/`, a note in an area, your own report in `outputs/`) are not named. The logbook folder itself may be a link |
| `state` | `cursors.json`, `manifest.json` and `owned.json` in `~/.local/state/seldon` can be read | `error`: the file is corrupt or unreadable; the row says what that breaks; the fix moves a corrupt file away or makes an unreadable one readable. `degraded`: the next capture will record a state reset, see [doctor says the next capture will record a state reset](#doctor-says-the-next-capture-will-record-a-state-reset); or the last capture recorded one, see [A state reset was recorded](#a-state-reset-was-recorded); or a collector waits for its baseline since a state reset (degraded or not run): run `seldon capture --source <name>` once it can run |
| `omarchy` | `omarchy-version` answered | the Omarchy collector cannot read the version |
| `snapper` | snapshots can be listed, or read from `/.snapshots` | `degraded`: your user may neither list snapshots nor read `/.snapshots`; see [Snapshots are not recorded](#snapshots-are-not-recorded). An `ok` row with a fix: your user is still in the old snapper opt-in; see [doctor suggests reverting the snapper opt-in](#doctor-suggests-reverting-the-snapper-opt-in) |
| `git` | git is there; the logbook is a repository | git is missing, or the logbook is not a repository; autocommit is off then. `degraded`: something keeps every autocommit from committing (a stale `.git/index.lock`, a detached HEAD, …); the fix says what to do |

`doctor --json` prints the same as JSON. The plugin does not run
`doctor`: it chooses its banner from the index and from its own engine
calls.

## Banners in the panel

When something is wrong, the panel shows one banner at the top, with a
button that fixes it.

| Banner | Cause | Fix |
|---|---|---|
| Install the engine (a setup step); Seldon engine missing, in red, when the engine was there before | the plugin cannot run `seldon` | *Install* opens a terminal that says what it does and runs the GitHub installer; or install it yourself ([Getting started](01-getting-started.md#step-1-install-the-engine)); then *Check again* |
| Create your logbook | there is no logbook yet | *Create* opens a terminal that runs `seldon init`; the panel updates by itself when the logbook is there |
| No index yet / Index unreadable | `~/.local/state/seldon/index.json` is missing or broken | *Build index* runs `seldon status` |
| Index is stale | the index is more than two hours old | *Capture now* |
| Index format mismatch | the plugin and the engine speak different versions of the index | update the older one. Plugin: `omarchy plugin update jax.seldon`, then `omarchy-restart-shell`. Engine: *Update* runs the installer again (until the AUR package exists; see [Update and uninstall](11-update-and-uninstall.md)) |
| Engine too old | the engine is older than this plugin needs (the `engineMin` in its manifest) | *Update* runs the installer again in a terminal (until the AUR package exists; see [Update and uninstall](11-update-and-uninstall.md)), then *Check again* |
| Read snapshots (optional) | snapper refuses your user and `/.snapshots` is not readable | *Grant* opens a terminal that says what the grant allows, runs the one-time read grant (you type your password there) and records the snapshots; the banner then goes by itself. Seldon works without snapshots |
| Restart the shell to finish the update | the plugin was updated, but the shell still runs the code it loaded before (it loads new plugin code only when it restarts) | *Restart shell* runs `omarchy-restart-shell`; the bar and panels come back within seconds. See [Update the plugin](11-update-and-uninstall.md#update-the-plugin) |
| Capture warned | a capture the plugin ran finished with a warning, such as [a state reset](#a-state-reset-was-recorded); the notice shows the first line of each warning, the pointer over it shows all of it | no button: do what the warning says. The notice goes away after the next capture without warnings |

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
let your user list snapshots or read the snapshot directory. Seldon works
without them; the timeline then has no snapshot markers. To allow it, run
once:

```sh
sudo setfacl -m u:$USER:rx /.snapshots
```

Seldon never runs it for you. It gives your user read access to
`/.snapshots`: Seldon then reads the snapshot list and the info files.
Your user cannot create, change or delete snapshots with it. Files inside
a snapshot keep their own permissions, so you can read in an old snapshot
what you could read when it was taken.

### doctor suggests reverting the snapper opt-in

Earlier versions of Seldon suggested adding your user to the snapper
config's `ALLOW_USERS`. That also lets your user create, change and
delete root snapshots without a password. When your user is still listed
there, `seldon doctor` says so in the `snapper` row and prints:

```sh
sudo snapper -c root set-config ALLOW_USERS="" SYNC_ACL=no && sudo setfacl -m u:$USER:rx /.snapshots
```

The first command empties the list (add back any other user that should
stay in it) and stops snapper from managing the access list of
`/.snapshots`, so a later snapper change does not take your read access
away again. It may remove the read access the old opt-in gave your user;
the second command grants it. Seldon keeps recording snapshots either
way.

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

- Claude Code: did Seldon start it (*Run*, *Start agent*,
  `seldon agent start`), or did you start it in the logbook folder? A
  session you start by hand elsewhere is not recorded. Are the hooks in
  `~/.claude/settings.json`? The `hooks` row of `seldon doctor` says;
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

### doctor says the next capture will record a state reset

`seldon doctor` shows a row like this:

```
  degraded  state    the next capture will record a state reset for pacman, config: cursors missing in ~/.local/state/seldon, …
```

The engine's state directory, `~/.local/state/seldon`, has no usable
cursors for this logbook (it was deleted and not restored yet, or a
value in `cursors.json` cannot be read) while your ledger already holds
events of those collectors. Nothing is lost yet: the next capture would
start those collectors over, as described in
[A state reset was recorded](#a-state-reset-was-recorded).

- If you have a backup of the state directory, restore it now, before
  the next capture (see
  [Back up and restore the state directory](07-the-logbook.md#back-up-and-restore-the-state-directory)).
  With the agent hooks installed, a capture also runs when an agent
  session ends, so do it first. Run
  `seldon doctor` again: the row is gone, and the next capture records
  what changed since the backup.
- Without a backup, run `seldon capture` to accept the new baseline.
  It records the state reset; the row below then takes over until the
  following capture.
- When the row says `bound to another logbook`, there is nothing to
  restore: the state belongs to another logbook path (you moved the
  logbook, or ran a command with `--logbook` for another one). Run
  `seldon capture`.
- When the row says `the next capture will warn of the state reset for …`,
  a capture recorded the reset and stopped before it saved its state (a
  crash, a kill). The same steps apply; the next capture does not record
  the reset a second time, it only prints the warning, and no row stays
  after it.

doctor cannot know whether a collector will degrade in that capture
(snapper without the read grant, for example). Such a collector takes
no new baseline, so the capture may name fewer collectors than the row.
A collector you have never captured with here is not named: it has
nothing to lose.

### A state reset was recorded

`seldon capture` printed a line like this:

```
warning: state reset recorded: pacman, config took a new baseline because ~/.local/state/seldon was missing, unreadable or bound to another logbook, …
```

The engine's state directory, `~/.local/state/seldon`, was gone, belonged
to another logbook, or held a file it could not read, while your ledger
already had events of those collectors. They started over from the
machine as it is now. Changes made since their last capture may be
missing from the ledger: `pacman` and `snapper` read their sources again
and miss little, the other collectors miss every change made in between.

The capture wrote a `state-loss` line with the subject `state-reset`
(a note in 0.1.x) to the ledger,
so the gap stays visible, and `seldon doctor` shows a `degraded` `state`
row until the next capture. When the plugin ran that capture, the panel
shows the warning as a "Capture warned" notice until a capture without
warnings.

If you have a backup of the state directory, restore it and run a
capture; that capture records what changed since the backup (see
[Back up and restore the state directory](07-the-logbook.md#back-up-and-restore-the-state-directory)).
Without a backup there is nothing to restore: the next capture clears
the row, and the note stays in the ledger. The same holds when the
state belonged to another logbook (you moved the logbook, or ran a
command with `--logbook` for another one); the warning then says so.

A capture that finds a corrupt `owned.json` moves it to
`owned.json.bad` in the same folder. Seldon never reads that file
again; it is kept only for a look, you can delete it, and the next
corrupt `owned.json` replaces it.

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
