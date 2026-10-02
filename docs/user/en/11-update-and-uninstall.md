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

It replaces `seldon` when the release is newer. With the same version it
changes nothing. It checks the engine against the release's `SHA256SUMS`
and refuses on a mismatch. If the latest release has no `install.sh`
yet (v0.1.0), use the script from `main` as in Getting started.

| Option | Effect |
|---|---|
| `--version vX.Y.Z` | that release instead of the latest |
| `--prefix DIR` | install into `DIR/bin` instead of `~/.local/bin` |
| `--unit` | also install the watcher's user unit, see [The optional watcher](#the-optional-watcher) |
| `--force` | replace a `seldon` the script did not install, such as one you built yourself |
| `--uninstall` | remove what the script installed |

With the one-liner, pass options after `bash -s --`, for example
`… | bash -s -- --version v0.1.1`.

The AUR package `jax-seldon` is coming soon. Once it exists, update it
with your AUR helper (`yay -S jax-seldon`). Install from one source only:
both put a `seldon` on your `PATH`.

## Update the plugin

```sh
omarchy plugin update jax.seldon
```

The plugin and the engine agree on the index format by its version. If
one is too old, the panel says "Index format mismatch" and names the one
to update. After an update, `seldon doctor` should show only `ok`.

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
except your logbook.

1. The plugin:

   ```sh
   omarchy plugin remove jax.seldon
   ```

2. The watcher, if you enabled it:

   ```sh
   systemctl --user disable --now seldon-watch
   ```

3. The engine. The installer removes exactly the files it installed; a
   file you changed since is kept, and it says so. Add the same `--prefix`
   if you gave one:

   ```sh
   curl -fsSL https://github.com/JohnAndrewsX/jax-seldon/releases/latest/download/install.sh | bash -s -- --uninstall
   ```

4. The theme hook, if you chose it in the wizard. Without the engine it
   does nothing, but it is cleaner to remove it:

   ```sh
   rm ~/.config/omarchy/hooks/theme-set.d/seldon-theme-set.sh
   ```

5. Claude Code's hooks live in the logbook's `.claude/settings.json`.
   They go with the logbook. If you installed them into
   `~/.claude/settings.json`, remove the three entries that call
   `seldon hook` from that file.

6. The engine's config and state:

   ```sh
   rm -r ~/.config/seldon ~/.local/state/seldon
   ```

What stays is your logbook, `~/Seldon` unless you chose another place.
It is plain Markdown in a git repository and stays readable without
Seldon. Delete it only if you are sure you do not need its history:

```sh
rm -r ~/Seldon
```

## Start over

To try Seldon from scratch on the same machine, keep the engine and the
plugin. Remove the theme hook, the engine's config and state and the
logbook (steps 4 and 6 above, and the logbook), then run `seldon init`
again. `init` refuses a folder that is not empty, so a new logbook never
overwrites an old one.

---

Previous: [Troubleshooting](10-troubleshooting.md) · [Index](README.md) · Next: [FAQ](12-faq.md)
