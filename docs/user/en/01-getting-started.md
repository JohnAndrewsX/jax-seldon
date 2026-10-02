# Getting started

This page takes you from nothing to a working logbook in about fifteen
minutes. You install the engine, create your logbook, add the bar plugin,
record one unplanned change and plan one change as a case. Each step
shows the command and what you should see.

## What you need

- Omarchy 4 with the Omarchy shell.
- `git` (Omarchy has it).
- A terminal. The commands on this page run as your user; none needs
  `sudo`.

Seldon only reads your system. It never installs a package and never
blocks a command. It writes three things: your logbook (`~/Seldon` by
default), its config file (`~/.config/seldon/config.toml`) and its state
(`~/.local/state/seldon/`). It writes outside these folders only where
you opt in during setup: Omarchy's theme hook and Claude Code's hook
settings.

## Step 1: Install the engine

The engine is one program, `seldon`. The AUR package is coming soon. Until
then you install it from the project's GitHub release. The install script
checks the engine against the release's checksums and installs
`~/.local/bin/seldon`.

While the current release is v0.1.0, take the script from the project's
`main` branch and name the version. The v0.1.0 release does not carry
the script yet:

```sh
cd "$(mktemp -d)"
curl -fsSLO https://raw.githubusercontent.com/JohnAndrewsX/jax-seldon/main/install.sh
less install.sh
bash install.sh --version v0.1.0
```

From v0.1.1 on, the release carries the script and its checksum. Then
download both, read the script, verify it and run it:

```sh
cd "$(mktemp -d)"
curl -fsSLO https://github.com/JohnAndrewsX/jax-seldon/releases/latest/download/install.sh
curl -fsSLO https://github.com/JohnAndrewsX/jax-seldon/releases/latest/download/SHA256SUMS
less install.sh
sha256sum -c --ignore-missing SHA256SUMS && bash install.sh
```

Check that your shell finds the engine:

```sh
seldon --version
```

```text
seldon 0.1.0
```

If your shell says `command not found`, `~/.local/bin` is not on your
`PATH` yet. Omarchy's default bash setup adds it when a shell starts, so
open a new terminal and try again. If you use another shell or your own
startup file, add `export PATH="$HOME/.local/bin:$PATH"` to it.
[Update and uninstall](11-update-and-uninstall.md) lists every install
option.

## Step 2: Create your logbook

Run the wizard once:

```sh
seldon init
```

It asks a few questions. Each one has a sensible default; press Enter to
take it. In a list, Space ticks or unticks an item and Enter confirms.

| Question | What to answer the first time |
|---|---|
| Where should the logbook live? | `~/Seldon` |
| Language of the logbook prose | the language you write notes in (English or German) |
| Add Obsidian settings (.obsidian/)? | yes if you use Obsidian, else no |
| Collectors | keep all six |
| Watched config paths | keep the defaults |
| More paths | leave empty |
| Agent harnesses | tick Claude Code hooks with Space, if you use Claude Code; else none |
| Record theme switches the moment they happen? | no (you can add it later) |
| Make the logbook a git repository with a first commit? | yes |
| Backfill since | leave empty |

For the backfill, empty means Seldon records from now on. A date makes
the first capture also record older changes. None of those belong to a
case, so each one shows up as drift. The wizard then
offers to mark them as the pre-Seldon baseline. Leave the backfill for
later; [Concepts](02-concepts.md#baseline) explains it.

The wizard ends with a summary like this (shortened):

```text
Logbook created at ~/Seldon (machine <machine>, language en, 31 files).
Config: ~/.config/seldon/config.toml
Git: repository initialised, first commit "seldon: init logbook"
Snapper: degraded — No permissions. Snapshots are not recorded until you allow your user once (ADR-0011)
First capture: 0 event(s); degraded: snapper (see seldon doctor); 0 open drift item(s), 0 crisis
Dossier: Wrote system/hardware.md, system/omarchy.md, system/packages.md, system/plugins.md, system/services.md (7 fence(s) changed)
Next steps:
  seldon doctor
```

`Snapper: degraded` is normal on Omarchy. Your user may not list
snapshots by default. Seldon works without them; step 3 shows the fix.

To answer no questions at all, run `seldon init --non-interactive`. It
takes `~/Seldon`, the language of your locale, all collectors and git,
and records from now on.

## Step 3: Check the setup

```sh
seldon doctor
```

```text
seldon doctor · ~/Seldon
  ok        engine   seldon 0.1.0, contract 1
  ok        config   ~/.config/seldon/config.toml
  ok        logbook  /home/you/Seldon · machine <machine> · en · 0 cases, 0 decisions, 0 journal days
  ok        omarchy  Omarchy 4.0.4-1
  degraded  snapper  No permissions. Snapshots are not recorded until you allow your user once (ADR-0011)
                     fix: sudo snapper -c root set-config ALLOW_USERS=$USER SYNC_ACL=yes
  ok        git      git version 2.55.0; logbook is a repository; autocommit on
doctor: ok
```

All lines should say `ok`, except `snapper`, which may say `degraded`.
If you want snapshots on the timeline, run the fix that `doctor` prints.
It changes the root snapper config, so the decision is yours. Seldon never
runs it for you.

## Step 4: Add the bar plugin

The plugin shows your logbook in the Omarchy bar. It is optional. With
it you see at a glance what Seldon records.

```sh
omarchy plugin add https://github.com/JohnAndrewsX/jax-seldon-plugin.git --enable
```

A small pill `⟡` appears on the right of the bar. Click it to open the
panel. [Daily use](03-daily-use.md) explains every part of it.

No pill? Check that `omarchy plugin list` shows `jax.seldon` as enabled
(else `omarchy plugin enable jax.seldon`), then restart the shell with
`omarchy-restart-shell`. If the panel shows a banner instead of data,
its button is the fix; *Check again* looks for the engine once more.
[Troubleshooting](10-troubleshooting.md#banners-in-the-panel) lists
every banner.

## Step 5: Record an unplanned change

Change something without telling Seldon first. A theme switch is a good
test: it is visible and you can undo it in a second.

1. Note your current theme, so you can switch back later:

   ```sh
   omarchy theme current
   ```

2. Switch to any other theme, with Omarchy's theme switcher or with
   `omarchy theme set <name>`. `omarchy theme list` shows the names.

3. Let Seldon look for changes:

   ```sh
   seldon capture
   ```

   ```text
   Captured 1 new event(s).
     snapper     0
     pacman      0
     omarchy     0
     plugins     0
     theme       1
     config      0
   ```

   The plugin also captures by itself every 15 minutes. Here you run it by
   hand so you do not have to wait.

4. Ask Seldon what is unexplained:

   ```sh
   seldon drift
   ```

   ```text
   yellow  2026-10-02 19:54  theme/theme-set  gruvbox  01M3YW134EVKJ23C1GXVHDVVEH
   1 open drift item(s), 0 crisis
   ```

   The theme switch is **drift**: a change that no case covers. `yellow`
   is its zone: a config or theme change, not a package
   ([Concepts](02-concepts.md#zones) explains zones). The last column is
   the event id (yours is different). If you added the plugin, the pill
   in the bar now shows `⟡ · 1`.

5. Explain it. The change already happened, so Seldon records your
   reason as a retroactive case: a new case, created already completed,
   with your text as its title:

   ```sh
   seldon drift explain <EVENT> -- "Tried another theme"
   ```

   Replace `<EVENT>` with the id from your `seldon drift` output.

   ```text
   Explained 1 event(s) with the new completed case C-2026-001
   Case: work/completed/C-2026-001-tried-another-theme.md
   ```

Your case ids carry the current year. `seldon drift` now says
`No open drift.`

## Step 6: Plan a change as a case

Now do it the planned way. Create a case for switching back, then start
it:

```sh
seldon plan new --area themes -- "Switch back to my usual theme"
seldon plan start C-2026-002
```

```text
Created C-2026-002 "Switch back to my usual theme" in work/queued/C-2026-002-switch-back-to-my-usual-theme.md
C-2026-002 queued → active (now work/active/C-2026-002-switch-back-to-my-usual-theme.md)
```

`--area themes` files the case under the area `themes`, one of six
topics a new logbook has (`areas/themes/`). Use the id that `plan new`
printed. The case is now active. Write a note into today's journal:

```sh
seldon log --case C-2026-002 -- "Switching back to my usual theme"
```

Switch back to the theme you noted in step 5, then capture again and
look at the drift:

```sh
seldon capture
seldon drift
```

The switch shows up as drift again. Seldon sees that the theme changed,
but a collector cannot know that you did it for this case. Only agents
working through hooks are linked to the active case by themselves. The
case still pays off: it holds your note and, once linked, the change
itself, as its trace. If the case's *Plan* names the theme (for example
`tokyo-night`), Seldon proposes this case for the change, and the
panel's drift dialog preselects it. You link your own change with one
command:

```sh
seldon drift link <EVENT> C-2026-002
```

```text
Linked 1 event(s) to C-2026-002
```

Close the case. `verify` says the work is done; `done` says you checked
it:

```sh
seldon plan verify C-2026-002
seldon plan done C-2026-002
```

```text
C-2026-002 active → verification
C-2026-002 verification → completed (now work/completed/C-2026-002-switch-back-to-my-usual-theme.md)
Journal: journal/2026/2026-10-02.md
```

## Step 7: Look at the result

```sh
seldon status
```

```text
Status of <machine> (2026-10-02)
  cases   0 active · 0 in verification · 0 queued
  drift   0 open · 0 crisis
  events  9 today · 9 in 7 days
Wrote ledger/2026-10.md, STATUS.md
Index: ~/.local/state/seldon/index.json
```

Open the plugin's panel and press `2` for the Changelog. You see both
theme switches, the notes and the case steps. Press `3` for Work: both
cases sit in the Completed column.

The logbook is plain Markdown, and each step you took is a git commit:

```sh
git -C ~/Seldon log --oneline
```

## Step 8: Write the rebuild guide

```sh
seldon rebuild
```

```text
Wrote outputs/REBUILD.md: 0 package(s), 0 deviation(s), 1 plugin(s), 0 unit(s), 0 open
```

Your counts differ. Open `~/Seldon/outputs/REBUILD.md`. It lists what a
fresh Omarchy install needs to become this machine again: your own
packages, changed files, plugins and the theme from step 6. It grows
with each case you record.
[Rebuild, dossier and update impact](08-rebuild-dossier-update-impact.md)
explains each section.

## Where to go next

- [Concepts](02-concepts.md): what cases, zones, risk and drift mean.
- [Daily use](03-daily-use.md): the pill, the panel and the Prime Radiant.
- [Working with agents](04-working-with-agents.md): let Claude Code or
  another agent work a case while Seldon records what it does.

---

[Index](README.md) · Next: [Concepts](02-concepts.md)
