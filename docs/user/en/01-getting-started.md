# Getting started

This page takes you from nothing to a recording machine in about fifteen
minutes. You add the Seldon plugin to the bar and click through one setup
card: install the engine, create the logbook, and, if you want them,
snapshots. No question is asked. Then you record one unplanned change and
plan one change as a case. Each step shows what you click or type and
what you should see. If you prefer the terminal for the setup,
[The terminal path](#the-terminal-path) at the end of the page does the
same with commands.

## What you need

- Omarchy 4 with the Omarchy shell.
- `git` (Omarchy has it).
- A terminal for steps 3 to 6. The setup card opens its own terminals.
  Nothing on this page needs `sudo`, except the optional snapshot grant,
  which asks for your password once.

Seldon only reads your system. It never installs a package and never
blocks a command. It writes three things: your logbook (`~/Seldon` by
default), its config file (`~/.config/seldon/config.toml`) and its state
(`~/.local/state/seldon/`). It writes outside these folders only where
you opt in: Omarchy's theme hook and Claude Code's hook settings. The
setup card sets up neither.

## Step 1: Add the plugin

```sh
omarchy plugin add https://github.com/JohnAndrewsX/jax-seldon-plugin.git --enable
```

A small pill with the Seldon mark appears on the right of the bar. Click
it. The Seldon desk opens on Today, and its overview shows the setup card:

```text
Set up Seldon · 3 of 3 steps to go
  1  Install the engine          Install · Copy
  2  Create the logbook
  3  Read snapshots (optional)
```

The header shows the same line. Click it from anywhere in the desk to come
back to the card.

No pill? Check that `omarchy plugin list` shows `jax.seldon` as enabled
(else `omarchy plugin enable jax.seldon`), then restart the shell with
`omarchy-restart-shell`.

Seldon records the plugin's installation, and the bar layout Omarchy
saves in `~/.config/omarchy/shell.json`, as routine history: they are
not drift.

## Step 2: Set up Seldon on the card

Each step has one button. It opens a floating terminal that says what it
does, shows the command and runs it, then says what changed. Close the
terminal when it says "Done!". The card moves on by itself: it looks
again every few seconds after its terminal opened.

1. **Install the engine.** *Install* downloads the engine, one program
   called `seldon`, from the project's GitHub release into
   `~/.local/bin`. The install script checks it against the release's
   checksums. It runs as your user and asks for no password. The terminal
   ends with "The engine is installed. The Seldon panel finds it by
   itself." The card now says "2 of 3 steps to go". The AUR package is
   coming soon; until then this is the way.

2. **Create the logbook.** Before you click, the list on the left shows
   what this machine already remembers without Seldon: the package
   transactions and the files edited under `~/.config` in the last seven
   days, with no who and no why. *Create logbook* runs `seldon init
   --defaults`. It asks nothing: the logbook goes to `~/Seldon`, in the
   language of your locale, with git, and the first capture looks back
   90 days. That history is recorded as "before Seldon": it stays in the
   timeline and asks nothing of you. If Obsidian is installed, the
   logbook gets Obsidian's settings, so you can open it as a vault. If
   the folder already holds other files, the step says so and offers
   *Choose a folder* instead: `seldon init` asks where the logbook should
   go. The terminal ends like this (your numbers differ):

   ```text
   Logbook     ~/Seldon (English, git repository)
   Config      ~/.config/seldon/config.toml; list noisy or secret files in its [redaction] skipPaths
   Recording   snapshots, packages, Omarchy updates, plugins, themes, config files
   Agents      none; add one with seldon hook install claude-code (or skills)
   History     Looked back 90 days: 1692 changes recorded as history before Seldon
   Snapshots   not readable yet; optional, Seldon works without them

   Seldon is recording. Nothing else to do.

   Optional, snapshots in the timeline: read access to the snapshot list
   and info files, nothing else. Asks for your password once:
     sudo setfacl -m u:$USER:rx /.snapshots
   ```

3. **Read snapshots (optional).** Omarchy keeps its snapshots in
   `/.snapshots`, which your user cannot read by default. *Grant* runs
   `sudo setfacl -m u:$USER:rx /.snapshots` and asks for your password
   once. It lets your user read the snapshot list and the info files, so
   Seldon can put the snapshots on the timeline; it cannot create,
   change or delete snapshots. Files inside a snapshot keep their own
   permissions. Seldon never runs it for you. *Not now* puts the step
   away for good: the card does not ask again, and Settings › Capture
   (key `,`) offers it again when you change your mind. On a machine
   without snapper the card has two steps.

When nothing is left, the card gives way to "Seldon is recording. Nothing
to do." Routine changes stay quiet in the Changelog; only a change that
can affect boot, login or the shell and has no case colours the bar.

## Step 3: Record an unplanned change

Change something without telling Seldon first. An alias in your
`~/.bashrc` is a good test: Omarchy's `~/.bashrc` has a place for your own
aliases, and you can undo it in a second. (A theme switch would not do:
Seldon counts it as routine, and routine changes are history, not drift.)

1. Add the alias:

   ```sh
   echo "alias gs='git status'" >> ~/.bashrc
   ```

2. Let Seldon look for changes:

   ```sh
   seldon capture
   ```

   ```text
   Captured 1 new event(s).
     snapper     0  degraded  snapper: No permissions. This user can neither list the snapshots nor read the snapshot directory; `seldon doctor` prints the read grant.
              fix: sudo setfacl -m u:$USER:rx /.snapshots
     pacman      0
     omarchy     0
     plugins     0
     theme       0
     config      1
   ```

   The plugin also captures by itself every 15 minutes. Here you run it by
   hand so you do not have to wait. Until you grant the read access (step 2),
   the `snapper` line says why it read no snapshots and repeats the fix;
   after it, the line says how many it read.

3. Ask Seldon what is unexplained:

   ```sh
   seldon drift
   ```

   ```text
   yellow     2026-10-09 19:22  config/config-change  ~/.bashrc  01M4GV0KZY6ZYF5BBTFT5DXZ48
   1 open drift item(s), 0 crisis
   ```

   The change is **drift**: a change that no case covers. `yellow` is its
   zone: a config change, not a package
   ([Concepts](02-concepts.md#zones) explains zones). The last column is
   the event id (yours is different).

   It is no crisis, so the pill in the bar stays as it was: by default
   the number after the mark counts crises only. The panel's Today tab
   (key `1`) shows the change under *without a case*, and so does the
   pill's tooltip. To count every change without a case in the bar too,
   set the plugin setting `driftInBar` to `all`
   ([Configuration](06-configuration.md#plugin-settings)).

4. Explain it. The change already happened, so Seldon records your
   reason as a retroactive case: a new case, created already completed,
   with your text as its title:

   ```sh
   seldon drift explain <EVENT> -- "A short git status"
   ```

   Replace `<EVENT>` with the id from your `seldon drift` output.

   ```text
   Explained 1 event(s) with the new completed case C-2026-001
   Case: work/completed/C-2026-001-a-short-git-status.md
   ```

Your case ids carry the current year. `seldon drift` now says
`No open drift.`

## Step 4: Plan a change as a case

Now do it the planned way. Create a case for removing the alias again,
then start it:

```sh
seldon plan new --area shell -- "Remove the gs alias again"
seldon plan start C-2026-002
```

```text
Created C-2026-002 "Remove the gs alias again" in work/queued/C-2026-002-remove-the-gs-alias-again.md
C-2026-002 queued → active (now work/active/C-2026-002-remove-the-gs-alias-again.md)
```

`--area shell` files the case under the area `shell`, one of six
topics a new logbook has (`areas/shell/`). Use the id that `plan new`
printed. The case is now active. Write a note into today's journal:

```sh
seldon log --case C-2026-002 -- "Removing the gs alias again"
```

```text
Noted in journal/2026/2026-10-09.md (C-2026-002): Removing the gs alias again
```

Delete the line `alias gs='git status'` from `~/.bashrc` (in your editor, or
with `sed -i "/^alias gs=/d" ~/.bashrc`), then capture again and look at
the drift:

```sh
seldon capture
seldon drift
```

```text
yellow     2026-10-09 19:22  config/config-change  ~/.bashrc  01M4GV10EJ1Z4JDABAQH5DXQ4S
1 open drift item(s), 0 crisis
```

The change shows up as drift again. Seldon sees that the file changed,
but a collector cannot know that you did it for this case. Only agents
working through hooks are linked to the active case by themselves. The
case still pays off: it holds your note and, once linked, the change
itself, as its trace. You link your own change with one command:

```sh
seldon drift link <EVENT> C-2026-002
```

```text
Linked 1 event(s) to C-2026-002
```

Next time, name the file in the case's *Plan* before you change it, for
example `- Affected paths: ~/.bashrc` in the case file
(`seldon open case` prints its path). While the case is active, the
capture then links the change by itself and says
`note: 1 event(s) linked to the one case that planned them while it was open`.

Close the case. `verify` says the work is done; `done` says you checked
it:

```sh
seldon plan verify C-2026-002
seldon plan done C-2026-002
```

```text
C-2026-002 active → verification
C-2026-002 verification → completed (now work/completed/C-2026-002-remove-the-gs-alias-again.md)
Journal: journal/2026/2026-10-09.md
```

## Step 5: Look at the result

```sh
seldon status
```

```text
Status of <machine> (2026-10-09)
  cases   0 active · 0 in verification · 0 queued
  drift   0 open · 0 crisis
  events  11 today · 11 in 7 days
  snapper degraded: snapper: No permissions. This user can neither list the snapshots nor read the snapshot directory; `seldon doctor` prints the read grant.
Wrote ledger/2026-10.md, STATUS.md
Index: ~/.local/state/seldon/index.json
```

The `snapper degraded` line is there until you grant the read access (step 2).
Your counts differ.

Open the plugin's panel and press `2` for the Changelog. You see both
changes to `~/.bashrc`, the plugin's installation, the note and the case
steps. Press `3` for Work: both cases sit in the Completed column.

The logbook is plain Markdown, and each step you took is a git commit:

```sh
git -C ~/Seldon log --oneline
```

```text
b930a5c seldon: status
2dca14b seldon: C-2026-002 completed — Remove the gs alias again
657d138 seldon: C-2026-002 verification
2523819 seldon: drift linked: 1 event(s), C-2026-002
867d2ad seldon: note C-2026-002
5a52e79 seldon: C-2026-002 active
67476e6 seldon: C-2026-002 created
0923622 seldon: drift explained: 1 event(s), C-2026-001
4f92d76 seldon: dossier
be3ccc5 seldon: init logbook
```

## Step 6: Write the rebuild guide

```sh
seldon rebuild
```

```text
Wrote outputs/REBUILD.md: 0 package(s), 2 deviation(s), 1 plugin(s), 0 unit(s), 0 open
```

Your counts differ. Open `~/Seldon/outputs/REBUILD.md`. It lists what a
fresh Omarchy install needs to become this machine again: your own
packages, changed files, plugins and the theme. It grows
with each case you record.
[Rebuild, dossier and update impact](08-rebuild-dossier-update-impact.md)
explains each section.

## The terminal path

The setup card runs these commands for you. Use them when you prefer a
terminal, or on a machine without the plugin.

### Install the engine

The engine is one program, `seldon`. The AUR package is coming soon. Until
then you install it from the project's GitHub release. The install script
checks the engine against the release's checksums and installs
`~/.local/bin/seldon`. If the GitHub CLI (`gh`) is installed and logged
in, the script also checks that the project's release workflow built the
download; otherwise it prints one note that only the checksum was
checked. `--require-verified` makes it refuse instead of installing
without that check; `--skip-provenance` leaves `gh` out when `gh` itself
fails, for example behind a proxy.

Download the script and the checksum file from the latest release,
read the script, verify it and run it:

```sh
cd "$(mktemp -d)"
curl -fsSLO https://github.com/JohnAndrewsX/jax-seldon/releases/latest/download/install.sh
curl -fsSLO https://github.com/JohnAndrewsX/jax-seldon/releases/latest/download/SHA256SUMS
less install.sh
sha256sum -c --ignore-missing SHA256SUMS && bash install.sh
```

The script first says what it installs and where, and ends with the steps
that are left: `seldon init` and the plugin, unless you have them already.

Check that your shell finds the engine:

```sh
seldon --version
```

```text
seldon 0.1.4
```

If your shell says `command not found`, `~/.local/bin` is not on your
`PATH` yet. Omarchy's default bash setup adds it when a shell starts, so
open a new terminal and try again. If you use another shell or your own
startup file, add `export PATH="$HOME/.local/bin:$PATH"` to it.
[Update and uninstall](11-update-and-uninstall.md) lists every install
option.

### Create your logbook

```sh
seldon init
```

It asks one question, where the logbook should live (Enter takes
`~/Seldon`), and takes the rest as the card does: the language of your
locale, git, Obsidian's settings when Obsidian is installed, and a first
capture that looks back 90 days and records that history as "before
Seldon". It ends with the same summary as the card's terminal.

`seldon init --defaults` asks nothing at all; it is what the card runs.
`seldon init --ask` is the full wizard, for when you want to choose
everything yourself. Each question has a default; press Enter to take it.
In a list, Space ticks or unticks an item and Enter confirms.

| Question | What to answer the first time |
|---|---|
| Where should the logbook live? | `~/Seldon` |
| Language of the logbook prose | the language you write notes in (English or German) |
| Add Obsidian settings (.obsidian/)? | the default (yes when Obsidian is installed) |
| Collectors | keep all six |
| Watched config paths | keep the defaults |
| More paths | leave empty |
| Agent setup | tick Claude Code hooks with Space, if you use Claude Code; else none |
| Record theme switches instantly? | no (the next capture records them anyway) |
| Keep the logbook in git, with a first commit? | yes |
| Backfill since | Enter takes the date 90 days back; `none` records from now on |
| Dismiss them as "before Seldon"? | yes (asked only after a backfill that found something) |

A backfill records older changes too: the package log and the snapshots.
Most of them are routine history. The rest opens as drift, changes that
no case covers; dismissing them as "before Seldon" keeps the events and
asks nothing more of you. [Concepts](02-concepts.md#baseline) explains it.

`seldon init --non-interactive` asks nothing either, for scripts. It
takes `~/Seldon`, the language of your locale, all collectors and git,
and looks back 90 days too; it detects nothing (`--obsidian` adds
Obsidian's settings). `--since` sets another start, `--no-capture`
records nothing yet.

### Check the setup

```sh
seldon doctor
```

```text
seldon doctor · ~/Seldon
  ok        engine   seldon 0.1.4, contract 2
  ok        config   ~/.config/seldon/config.toml
  ok        logbook  ~/Seldon · machine <machine> · en · 0 cases, 0 decisions, 0 journal days
  ok        cases    every case id has one file
  ok        ledger   0 months, every line an event
  ok        fences   STATUS.md and DECISIONS.md: every generated fence has its end marker
  ok        rules    current (v5)
  ok        rollbacks no case has a rollback snapshot
  ok        workpieces no workpiece folders
  degraded  collectors last capture failed: snapper: snapper: No permissions. This user can neither list the snapshots nor read the snapshot directory; `seldon doctor` prints the read grant.
                     fix: sudo setfacl -m u:$USER:rx /.snapshots
  ok        layout   no linked folders or files where Seldon writes
  ok        state    ~/.local/state/seldon: cursors.json, manifest.json readable
  ok        skills   no agent skill folder (~/.agents/skills, ~/.claude/skills, ~/.codex/skills, ~/.pi/agent/skills, ~/.hermes/skills); nothing to install
  ok        hooks    none: no Claude Code harness is configured
  ok        omarchy  Omarchy 4.0.4-1
  degraded  snapper  No permissions. Snapshots are not recorded until you grant your user read access to the snapshot directory once (ADR-0026). The fix grants your user read access to the snapshot directory listing and the snapshot info files (files inside a snapshot keep their own permissions), nothing else: no snapshot creation, change or deletion.
                     fix: sudo setfacl -m u:$USER:rx /.snapshots
  ok        pacman   no db.lck: pacman is not running
  ok        git      git version 2.55.0; logbook is a repository; autocommit on
  ok        watch    watchPaths: 13 path(s), every default included
  ok        drift    attention normal · routine: sysupgrade, upgrade, keyring, omarchy-update, plugin-toggle, seldon-self, theme, omarchy-default, system-link, routine-paths, theme-assets, theme-repo, toggle-flag · routinePaths 2 · routinePackages 2 · alwaysRedPaths 9 · alwaysRed 20; all defaults; Omarchy's copies count as evidence (/usr/share/omarchy)
doctor: ok
```

All lines should say `ok`, except `snapper` and `collectors`, which say
`degraded` until you run the grant: the first capture could not read the
snapshots.
If you want snapshots on the timeline, run the fix that `doctor` prints,
the same grant as the card's step 3.

## Where to go next

- [Concepts](02-concepts.md): what cases, zones, risk and drift mean.
- [Daily use](03-daily-use.md): the pill, the panel and the Prime Radiant.
- [Working with agents](04-working-with-agents.md): let Claude Code or
  another agent work a case while Seldon records what it does.

---

[Index](README.md) · Next: [Concepts](02-concepts.md)
