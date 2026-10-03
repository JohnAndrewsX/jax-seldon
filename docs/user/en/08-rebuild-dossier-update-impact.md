# Rebuild, dossier and update impact

This page covers the three outputs that describe your machine as a
whole: the dossier in `system/`, the rebuild guide
`outputs/REBUILD.md`, and the update impact report, which is planned
but not part of the engine yet.

## The dossier

The dossier is the folder `system/`. It describes the machine as it is
now. Each file has generated parts between markers and room for your own
text around them:

```markdown
## Explicit packages

<!-- seldon:begin packages.explicit -->
- zed · repo · user · since 2026-10-01 [[C-2026-004]]
- …
<!-- seldon:end -->

My notes about packages go here. Seldon keeps them.
```

| File | Generated parts |
|---|---|
| `packages.md` | counts (explicit, total, AUR); a history row for each day the counts changed; every explicit package with its source, its class and the case that installed it |
| `services.md` | enabled systemd units, system and user, with the case that enabled them |
| `omarchy.md` | Omarchy version, theme, last update |
| `hardware.md` | CPU, memory, machine, root file system |
| `plugins.md` | shell plugins: id, enabled, first party, where it came from |
| `deviations.md` | files you changed against Omarchy's defaults: path, reason, date, case |

In `packages.md` each explicit package has a class. `omarchy-base` means
Omarchy's own package lists name it; `user` means you added it. A package
that was there before the logbook says `pre-logbook` instead of a date
and case.

`deviations.md` gets a row for every config file a case changed. The
reason column stays empty for you to fill. Your rows and reasons stay as
you wrote them; the engine only fills an empty case cell later.

### Refresh it

`seldon init` fills the dossier once. After that, refresh it yourself,
for example once a week or before you write the rebuild guide:

```sh
seldon dossier
```

`--section packages,services` refreshes only those parts. The command
only reads: it asks the package manager and systemd for lists, and reads
`/proc` and `/sys`. It never changes text outside the markers, writes a
file only when it changed, commits as `seldon: dossier`, and writes no
ledger event. If a query fails, that part keeps its old content and the
command prints a warning.

## The rebuild guide

```sh
seldon rebuild
```

writes `outputs/REBUILD.md`: the steps that take a fresh Omarchy install
to the state your logbook documents. Run `seldon dossier` first, because
the guide reads the package list from it.

| Section | Holds |
|---|---|
| 1. Base | the Omarchy version to install and update to |
| 2. Packages | the packages you installed since the logbook began, grouped by case, as `omarchy pkg add` and `omarchy pkg aur add` commands; your own packages from before the logbook; a count of the ones Omarchy brings itself |
| 3. Deviations | the files you changed, with your reasons; take the files from your dotfiles or backup |
| 4. Plugins | shell plugins beyond the first-party ones, and first-party plugins you disabled |
| 5. Theme | the theme to set |
| 6. User units | systemd units this logbook knows |
| 7. Open questions | drift nobody decided yet; decide before you rebuild. Below it, what you dismissed on purpose and should not set up again |

Each line names the case or event it comes from. The guide records paths
and reasons, never file contents. It names the files to restore; their
content comes from your dotfiles or backup.

Before a name goes into a command, the engine checks it: package names,
unit names, theme names, plugin ids and plugin URLs must have the form
the package manager, systemd and Omarchy use, and none may start with
`-`. A name that fails is listed with "not reproduced: invalid name"
and no command, and `seldon rebuild` prints a warning for it. Look at
that item and set it up by hand. A name that holds characters the shell
reads specially, such as the `~` in some URLs, is put in single quotes.

The guide is generated. Text you add outside its markers stays; text
inside is replaced each time. The command writes the file only when it
changed and commits as `seldon: rebuild`.

### Use it

1. Install Omarchy on the new machine and update it to the version in
   section 1.
2. Install the engine and create a logbook, or copy your old logbook
   over (see [The logbook](07-the-logbook.md#moving-or-copying-the-logbook)).
3. Work through sections 2 to 6 in order. The package commands skip what
   is already installed.
4. Restore the files from section 3 from your dotfiles or backup.

## Update impact

`seldon update-impact` is planned: before an `omarchy update`, it will
compare what the update changes against your deviations and list the
overrides that may break. The current engine does not have it yet, and
the plugin does not show it.

Until then, a safe routine for Omarchy updates:

1. Open a case for the update. It is red: it changes packages and Omarchy
   itself.

   ```sh
   seldon plan new --zone red --risk R2 --area packages -- "Omarchy update"
   ```

2. Read `system/deviations.md`. Each row is a file you changed on purpose
   that the update may touch.
3. Take a snapshot and start the case with its number:

   ```sh
   sudo snapper -c root create --description "before Omarchy update" --print-number
   seldon plan start C-2026-005 --snapshot <N>
   ```

   `<N>` is the number the first command prints. Use the id that
   `plan new` printed instead of `C-2026-005`.

4. Run the update, then `seldon capture`. Link the update's drift to the
   case with `seldon drift link`, and check your deviations.
5. Verify and close the case.

The case then holds the update, the snapshot to roll back to and your
notes on what broke.

---

Previous: [The logbook](07-the-logbook.md) · [Index](README.md) · Next: [Import from omarchy-agent](09-import-from-omarchy-agent.md)
