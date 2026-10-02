# FAQ

Short answers to the questions people ask first. Each answer links to
the page with the details.

## Does Seldon change my system?

No. The engine reads the package log, snapper, the Omarchy version, the
plugin list, the theme and your watched config files. It writes only its
logbook, `~/.config/seldon/config.toml` and `~/.local/state/seldon/`.
Two things happen only if you choose them in the wizard: the theme hook
in Omarchy's hook folder, and Claude Code's hooks in the logbook's
`.claude/settings.json`. It never runs a package manager, `sudo` or
`systemctl` with a changing verb.

## Does Seldon stop an agent from doing something dangerous?

No. Seldon records; it does not guard. Hooks never block a command. Use
your agent's own permission settings for limits.
See [Working with agents](04-working-with-agents.md#what-seldon-does-and-does-not-do).

## Does anything leave my machine?

No. The engine and the plugin make no network connection. The only
download is the installer you run yourself. Seldon never pushes the
logbook's git repository anywhere.

## Do I need Obsidian?

No. The logbook is plain Markdown and works in any editor. Obsidian is an
optional viewer; see [The logbook](07-the-logbook.md#obsidian).

## Do I need the bar plugin?

No. Everything works from the terminal. The plugin makes drift and cases
visible at a glance, and it captures every 15 minutes. Without it,
run `seldon capture` yourself, or let an agent's session end do it.

## Why is my own change drift? I had a case open.

A collector sees that something changed, but not who did it or why. Only
an agent's command, recorded by a hook, carries the active case. Link your
own change with `seldon drift link <EVENT> <CASE>`, or with *Link* in the
drift sheet. See [Concepts](02-concepts.md#drift).

## Is every package upgrade a crisis?

No. A routine full upgrade is yellow drift, one item per transaction,
resolved with one command. It turns red, a crisis, when it installs or
removes a package or touches a package on the always-red list (kernel,
systemd, glibc, Hyprland, Omarchy, Quickshell).

## Can I edit the files by hand?

Yes, most of them: case text, journal, memory, decisions, areas,
`PROJECT.md`, `AGENTS.md`, your text in `system/`. Not the ledger, the
generated files, the engine's frontmatter fields or `.seldon/`.
See [The logbook](07-the-logbook.md#what-you-own).

## How do I undo a mistake?

The ledger is append-only, so a wrong event stays and a new event
corrects it. A drift resolution cannot be taken back with a command. For
files you edited, git has every earlier version:
`git -C ~/Seldon log -p <file>`.

## Can I use Seldon in German?

Yes. Choose German as the logbook language in the wizard, or run
`seldon init --language de`. Journal lines and `STATUS.md` are then
German. Headings, frontmatter keys and commands stay English in every
language. The plugin's labels are English in this version. This guide
is also [in German](../de/README.md).

## Can I have two logbooks?

One machine, one logbook works best: the engine's state belongs to one
logbook. For a test logbook, run `seldon init --path <DIR>`, and use
`--logbook <DIR>` for its commands. `init` makes the new folder the
default, so set `logbook` in `config.toml` back afterwards.

## What does the name mean?

In Isaac Asimov's *Foundation*, Hari Seldon's Plan predicts the future,
and a crisis is where reality leaves the Plan. The Prime Radiant is the
device that shows the Plan. Seldon makes the places where your machine
left your plan visible.

## Where do I report a bug?

At <https://github.com/JohnAndrewsX/jax-seldon/issues>. See
[Troubleshooting](10-troubleshooting.md#reporting-a-bug) for what to
include.

---

Previous: [Update and uninstall](11-update-and-uninstall.md) · [Index](README.md) · Next: [Glossary](13-glossary.md)
