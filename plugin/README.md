# JAX Seldon — Omarchy plugin

Flight recorder and planning desk for your Omarchy system. The plugin shows
what the Seldon engine has recorded: active cases, unexplained changes
(drift) and the Prime Radiant overlay. It reads one file,
`${XDG_STATE_HOME:-~/.local/state}/seldon/index.json`, which the engine
writes.

> Phase 1. The panel has the Today, Changelog and System tabs; Work,
> Decisions, Memory and the Prime Radiant charts arrive in later releases.
> Project home: https://github.com/JohnAndrewsX/jax-seldon

## Requirements

- Omarchy 4 with the Omarchy shell.
- The `seldon` engine from the AUR: `omarchy pkg aur add jax-seldon`.
  Without it the plugin shows an "engine not installed" banner with that
  command.

## The pill

`⟡ A · D` — A active cases, D unexplained changes; zero parts are hidden
(`⟡`, `⟡ 2`, `⟡ · 3`). Accent colour when cases are active, the theme's
urgent colour when a change is in the red zone, dimmed while something needs
fixing. The tooltip says what and when the engine last captured.

| Click | Does |
|---|---|
| Left | toggle the panel |
| Middle | toggle the Prime Radiant |
| Right | capture now (`seldon capture`, then `seldon status`) |

## The panel

| Tab | Shows |
|---|---|
| Today | the date, today's counts (events today and in 7 days, active and queued cases, open drift), today's journal entries, yesterday's behind one row; *Open in editor* |
| Changelog | every event in the index, newest first, grouped by day; source filter chips with counts; *Capture now* |
| System | Omarchy version, theme and last update, package counts, deviations, plugins, snapshots, areas, collectors, machine and engine; a section appears only when the index has it |

A Changelog row shows the source glyph, kind, subject and time, then what
changed, who, and the case. The stripe on the left is the event's zone in
theme colours (red = urgent, yellow = accent, green = muted). Snapshot rows
are highlighted. An explained, dismissed or linked event shows its
resolution and the reason given. Open drift says *Unexplained* or, in the
red zone, *Needs a reason*, with the proposed case when there is one. A
package transaction that is open drift as one group shows "+N" on its
leader; Enter or a click lists the members.

Above every tab: the status banner (below), the snapper banner when
snapshots cannot be read, and a red strip "N changes in the red zone need a
reason" (a click opens the Changelog).

| Key | Does |
|---|---|
| Tab / Shift-Tab | next / previous tab; past the last (first) tab, on to the bar's next (previous) panel |
| ← / →, 1–3 | previous / next tab, or a tab directly |
| ↑ / ↓ | move in the list |
| Enter, Space | open the row (a group's members, the full text, yesterday's entries) |
| f / F | Changelog: next / previous source filter |
| c | capture now |
| Esc | close |

## States

When something is wrong the panel shows one banner with a one-click fix.

| Status | Banner | Fix |
|---|---|---|
| `engineMissing` | Seldon engine not installed | *Install in terminal*, *Copy* `omarchy pkg aur add jax-seldon`, *Check again* |
| `notInitialised` | Logbook not initialised | *Run in terminal* / *Copy* `seldon init`, *Check again* |
| `indexMissing` | No index yet / Index unreadable | *Build index* (`seldon status`) |
| `indexStale` | Index is stale (older than 2 h) | *Capture now* |
| `contractMismatch` | Index format mismatch | *Update in terminal* / *Copy* the plugin or engine update command |
| snapper collector failing | Snapshots not readable (with the engine's message) | *Run in terminal* / *Copy* `sudo snapper -c root set-config ALLOW_USERS=$USER SYNC_ACL=yes` (once; Seldon never runs it) |

The engine is looked for once when the shell starts and again on *Check
again* (or `jax.seldon.service refresh`), not on every capture interval.

## IPC

| Command | Reaches |
|---|---|
| `omarchy-shell shell toggle jax.seldon` | the Prime Radiant overlay (also `summon`, `hide`) |
| `omarchy-shell jax.seldon.panel open\|close\|toggle\|show\|hide` | the bar panel |
| `omarchy-shell jax.seldon.panel pill` | what the pill shows, as JSON |
| `omarchy-shell jax.seldon.panel view` | what the panel shows (tab, rows, banners, strip), as JSON |
| `omarchy-shell jax.seldon.panel tab today\|changelog\|system` | show a tab |
| `omarchy-shell jax.seldon.panel filter all\|<source>` | set the Changelog source filter |
| `omarchy-shell jax.seldon.service status` | the service state, as JSON |
| `omarchy-shell jax.seldon.service refresh` | look for the engine again and re-read the index |
| `omarchy-shell jax.seldon.service capture` | capture now |

Because the plugin declares an overlay, the shell's own `summon`/`toggle`
route opens the Prime Radiant, never the bar panel; the panel has its own
target, `jax.seldon.panel`.

Suggested binding (not installed by the plugin), in your Hyprland bindings:

```
o.bind("SUPER + SHIFT + S", "Seldon", "omarchy-shell shell toggle jax.seldon")
```

## Settings

Setup > Plugins > Seldon: **Capture interval (minutes)**, 5–120, default 15.

## Security, privacy, privileges

- No network access. No bundled binaries, units or installers.
- Reads one JSON file. Everything shown from it is displayed as plain text,
  never evaluated.
- Runs the `seldon` engine only with fixed argument lists from the
  contract; ids are checked against their patterns first, free text is one
  non-empty argument after `--`. Never builds a shell command from logbook
  content.
- Opens a terminal or touches the clipboard only when you click a banner
  button, and then only with a constant command.

## Development

- `SELDON_INDEX=/path/to/index.json` makes the service read that file
  instead (relative paths resolve against the shell's working directory).
  Dev mode is read-only: the engine is probed, never run.
- `SELDON_NOW=<RFC 3339>` (with `SELDON_INDEX`) sets the clock for the
  staleness check; without it a fixture never turns stale.
- After changing plugin code, restart the shell (`omarchy-restart-shell`):
  the shell's hot reload re-creates the plugin from cached components.

See `docs/TESTING.md` in the project repository for the smoke test.

## License

MIT
