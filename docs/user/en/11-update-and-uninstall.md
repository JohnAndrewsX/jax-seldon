# Update and uninstall

This page shows how to update the engine and the plugin, how to run the
optional watcher, and how to remove Seldon completely or in parts. None
of it touches your logbook, unless you delete it yourself.

## Update the engine

Run the installer again, in either form from
[Getting started](01-getting-started.md#step-1-install-the-engine). The
one-liner:

```sh
curl -fsSL https://github.com/JohnAndrewsX/jax-seldon/releases/latest/download/install.sh | bash
```

It replaces `seldon` when the release's version differs from the
installed one; with `--version` that can also be an older release. With
the same version it changes nothing. It checks the engine against the release's `SHA256SUMS`
and refuses on a mismatch. With the GitHub CLI (`gh`) installed and
logged in it also verifies the build provenance (`gh attestation
verify`): the download must come from the project's release workflow for
that release's tag, or nothing is installed. Without `gh` one note says
that only the checksum was checked. Its next steps leave out what you
have: no `seldon init` when your logbook is set up, no plugin line when
the plugin is installed. When nothing is left it says "Your logbook is
already set up; nothing else to do." With zsh installed, the `fpath=(…)`
line below is printed on every run instead: it is for the zsh
completions and needs nothing once it is in your `~/.zshrc`. Update the
plugin too ([Update the plugin](#update-the-plugin)): it cannot see by
itself that a newer one exists.

It also installs the man page (`man seldon`) and the Tab completions for
bash, zsh and fish, each for a shell that is installed on your machine.
For zsh it prints one `fpath=(…)` line to add to `~/.zshrc` before
`compinit`, on every run (it does not check whether you added it).

| Option | Effect |
|---|---|
| `--version vX.Y.Z` | that release instead of the latest |
| `--prefix DIR` | install into `DIR/bin`, the man page and completions into `DIR/share`, instead of `~/.local` |
| `--unit` | also install the watcher's user unit, see [The optional watcher](#the-optional-watcher) |
| `--force` | replace a `seldon`, a completion or a man page the script did not install, such as one you built yourself |
| `--require-verified` | install only when `gh` verified the build provenance; refuse when `gh` is missing or not logged in, and for releases up to v0.1.1, which were made before attestations |
| `--skip-provenance` | do not ask `gh`, check only the checksum (a note says so); for a `gh` that fails on its own, for example behind a proxy. Not together with `--require-verified` |
| `--uninstall` | remove what the script installed |

With the one-liner, pass options after `bash -s --`, for example
`… | bash -s -- --version v0.1.1`.

The AUR package `jax-seldon` is coming soon. Once it exists, update it
with your AUR helper (`yay -S jax-seldon`). The package installs the man
page and the completions for all three shells. Install from one source
only: both put a `seldon` on your `PATH`.

From 0.1.3 to 0.1.4: the first capture after the update copies Seldon's
Claude Code hooks from the logbook's `.claude/settings.json` to
`~/.claude/settings.json`, once, keeping everything else there (not with
`[agent] workdir = "logbook"`; see
[Working with agents](04-working-with-agents.md#hooks-of-an-older-logbook)).

## Update the plugin

```sh
omarchy plugin update jax.seldon
omarchy-restart-shell
```

Omarchy shows every changed line of the plugin and asks
`Update jax.seldon?`; answer yes. The list is long when many files
changed; that is Omarchy's normal check, not an error.

Does the panel of plugin 0.1.0 offer `omarchy pkg aur add jax-seldon`?
That package does not exist yet, so the command fails. Update the plugin
first, with the two commands above; the new plugin installs the engine
from the GitHub release instead.

The update fetches the new plugin files and asks the shell to reload its
plugins, but the shell keeps running the plugin code it loaded first
until it restarts. Restart it with `omarchy-restart-shell`: the bar,
panels and overlays come back within seconds, your applications keep
running. It refuses while the screen is locked.

When the panel sees that the installed plugin is not the one running, it
shows "Restart the shell to finish the update" above the other banners,
with a *Restart shell* button that runs `omarchy-restart-shell`. A plugin
version that does not know this notice yet shows nothing, so restart the
shell after every plugin update.

The plugin and the engine agree on the index format by its version. If
one is too old, the panel says "Index format mismatch" and names the one
to update. When the engine is newer and its index says this plugin can
still read it, the plugin keeps working and only asks, in a quiet
notice, for its own update ("The engine is newer than the plugin").
After an update, `seldon doctor` should show only `ok`.

## The optional watcher

Without the watcher, the plugin sees your changes at the next engine
command or capture. The watcher rebuilds the index two seconds after any
change in the logbook, so an edit in your editor or in Obsidian shows in
the panel at once. It does not capture and does not commit. It is off by
default; you decide whether you want it.

Try it in a terminal first. Stop it with Ctrl-C:

```sh
seldon watch
```

To run it as a user service, install its unit with the installer and
enable it:

```sh
curl -fsSL https://github.com/JohnAndrewsX/jax-seldon/releases/latest/download/install.sh | bash -s -- --unit
systemctl --user enable --now seldon-watch
```

The installer only places the unit in `~/.config/systemd/user/`; it never
enables it. Follow its log with `journalctl --user -u seldon-watch -f`.

To stop using it:

```sh
systemctl --user disable --now seldon-watch
```

## Uninstall

Remove the parts you no longer want. The order below removes everything
except your logbook. Steps 3 and 4 need the engine, so run them before
step 5.

1. The plugin:

   ```sh
   omarchy plugin remove jax.seldon
   ```

2. The watcher, if you enabled it:

   ```sh
   systemctl --user disable --now seldon-watch
   ```

3. The theme hook, if you chose it in the wizard. This removes the hook
   and nothing else, and needs no logbook:

   ```sh
   seldon init --remove-theme-hook
   ```

4. Claude Code's hooks. They live in your `~/.claude/settings.json`,
   and without this step Claude Code keeps calling a `seldon` that is
   gone. The command removes only Seldon's hooks; your own settings and
   hooks stay, and a file that held nothing else is deleted:

   ```sh
   seldon hook uninstall claude-code
   ```

   A logbook from before 0.1.4 may hold them too, in its own
   `.claude/settings.json`. If you keep the logbook, take them out of it
   with the same command and `--settings`:

   ```sh
   seldon hook uninstall claude-code --settings ~/Seldon/.claude/settings.json
   ```

   Each command prints what it removed, or says that nothing was
   installed. The next `seldon capture` records the removals without
   opening drift. If you delete the logbook as well, its own hooks go
   with it and only the first command is needed.

5. The engine. The installer removes exactly the files it installed,
   the man page and the completions included; a file you changed since
   is kept, and it says so. Add the same `--prefix` if you gave one:

   ```sh
   curl -fsSL https://github.com/JohnAndrewsX/jax-seldon/releases/latest/download/install.sh | bash -s -- --uninstall
   ```

6. The engine's config and state:

   ```sh
   rm -r ~/.config/seldon ~/.local/state/seldon
   ```

   If you might install Seldon again and keep the logbook, back the state
   directory up first (see
   [Back up and restore the state directory](07-the-logbook.md#back-up-and-restore-the-state-directory))
   and restore it before the first capture. Otherwise that capture starts
   over and records a state reset in the ledger. After the reinstall,
   `seldon doctor` tells you while there is still time: until the state
   is back, its `state` row says that the next capture will record a
   state reset (see
   [doctor says the next capture will record a state reset](10-troubleshooting.md#doctor-says-the-next-capture-will-record-a-state-reset)).

What stays is your logbook, `~/Seldon` unless you chose another place.
It is plain Markdown in a git repository and stays readable without
Seldon. Delete it only if you are sure you do not need its history:

```sh
rm -r ~/Seldon
```

## Start over

To try Seldon from scratch on the same machine, keep the engine and the
plugin. Remove the theme hook, the engine's config and state and the
logbook (steps 3 and 6 above, and the logbook), then run `seldon init`
again. `init` refuses a folder that is not empty, so a new logbook never
overwrites an old one.

---

Previous: [Troubleshooting](10-troubleshooting.md) · [Index](README.md) · Next: [FAQ](12-faq.md)
