# `seldon-watch.service` — optional systemd user unit

`seldon watch` rebuilds `~/.local/state/seldon/index.json` a moment after
the logbook changes, so the Seldon plugin shows an edit made in an editor
or in Obsidian within seconds instead of at the next timer run. It is
**optional and off by default** (ADR-0005): without it, the plugin runs
`seldon capture` at shell start, every 15 minutes and after each panel
action, and every `seldon` command that writes rebuilds the index itself.

Nothing installs this unit for you. `seldon init` never does, and
neither do agents working in this repository (AGENTS.md §6). Enable it
only if you want the watcher.

## What the watcher does

- Watches `ledger/`, `work/`, `journal/`, `decisions/`, `system/`,
  `memory/` (recursively) and `.seldon/logbook.toml` in the logbook that
  `seldon` resolves (`--logbook` > `SELDON_LOGBOOK` > `config.toml` >
  `~/Seldon`).
- Rebuilds once right after it starts, so the index reflects edits made
  while it was not running; after that it reacts to changes.
- Waits until the logbook has been quiet for `--interval` seconds (default
  and minimum 2), then rebuilds `index.json` under the state lock. A
  long burst of changes still rebuilds after at most five intervals.
- Rebuilds **only the index**. It does not run collectors (no capture),
  does not write `STATUS.md` or the `ledger/*.md` views, and does not
  commit. Changes to those generated files, hidden and temporary files,
  editor backups and plain reads never trigger a rebuild.
- Waits when another `seldon` command holds the lock; it does not fail.
- Logs one line per rebuild to stderr (the journal); with `--json`, one
  JSON object per line on stdout.
- Uses little memory: under 10 MB resident on a logbook ten times the
  size of the sample (`engine/tests/watch.rs`).

## Install

1. Until the package ships it (the Phase 4 PKGBUILD builds `seldon` with
   the feature), build `seldon` with the `watch` feature yourself. The
   default release build (`just build-release`) leaves it out, and
   `seldon watch` then exits 1 with "built without the watch feature;
   rebuild seldon with …":

   ```
   cargo build --manifest-path engine/Cargo.toml --release --locked \
     --features watch --target x86_64-unknown-linux-musl
   install -Dm755 engine/target/x86_64-unknown-linux-musl/release/seldon ~/.local/bin/seldon
   ```

2. Try it in a terminal first. You should see one `index rebuilt` line
   about two seconds after you save a file in the logbook. Stop it with
   Ctrl-C:

   ```
   ~/.local/bin/seldon watch
   ```

3. Install and start the unit:

   ```
   install -Dm644 engine/systemd/seldon-watch.service ~/.config/systemd/user/seldon-watch.service
   systemctl --user daemon-reload
   systemctl --user enable --now seldon-watch
   ```

4. Follow its log:

   ```
   journalctl --user -u seldon-watch -f
   ```

## Configure

The unit runs `%h/.local/bin/seldon watch` with your user manager's
environment, so it uses the logbook from `~/.config/seldon/config.toml`.
To change the interval or point it at another logbook, add a drop-in
with `systemctl --user edit seldon-watch`:

```
[Service]
ExecStart=
ExecStart=%h/.local/bin/seldon watch --interval 5
Environment=SELDON_LOGBOOK=%h/Seldon
```

## Exit codes and restarts

| Exit | Meaning | Restarted |
|---|---|---|
| 0 | stopped by SIGTERM or SIGINT, after the rebuild in progress | no |
| 1 | built without the `watch` feature, or a bad option (`--interval` below 2) | no (`RestartPreventExitStatus`) |
| 2 | engine error, e.g. the watches cannot be set up at start (inotify watch limit, `fs.inotify.max_user_watches`) | yes, after 10 s, at most 5 times in 5 minutes |
| 3 | the logbook is not initialised (run `seldon init`), or it disappeared | no |

## Hardening

The unit sets only options that work in a user unit without extra
privileges: `NoNewPrivileges`, `LockPersonality`, `RestrictRealtime`,
`RestrictSUIDSGID`, `SystemCallArchitectures=native`, plus `Nice=10` and
idle I/O priority. Options that need mount namespaces (`ProtectSystem`,
`ProtectHome`, `PrivateTmp`) are left out. They fail or do nothing in
many user managers, and the watcher must read the logbook in your home
and write the state directory.

## Remove

```
systemctl --user disable --now seldon-watch
rm ~/.config/systemd/user/seldon-watch.service
systemctl --user daemon-reload
```
