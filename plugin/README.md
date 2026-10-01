# JAX Seldon — Omarchy plugin

Flight recorder and planning desk for your Omarchy system. The plugin shows
what the Seldon engine has recorded: active cases, unexplained changes
(drift) and the Prime Radiant overlay. It reads one file,
`~/.local/state/seldon/index.json`, which the engine writes.

> Skeleton (Phase 1). The panel tabs and the Prime Radiant charts arrive in
> later releases. Project home: https://github.com/JohnAndrewsX/jax-seldon

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

## States

When something is wrong the panel shows one banner with a one-click fix.

| Status | Banner | Fix |
|---|---|---|
| `engineMissing` | Seldon engine not installed | *Install in terminal*, *Copy* `omarchy pkg aur add jax-seldon`, *Check again* |
| `notInitialised` | Logbook not initialised | *Run in terminal* / *Copy* `seldon init` |
| `indexMissing` | No index yet / Index unreadable | *Build index* (`seldon status`) |
| `indexStale` | Index is stale (older than 2 h) | *Capture now* |
| `contractMismatch` | Index format mismatch | *Update in terminal* / *Copy* the plugin or engine update command |

## IPC

| Command | Reaches |
|---|---|
| `omarchy-shell shell toggle jax.seldon` | the Prime Radiant overlay (also `summon`, `hide`) |
| `omarchy-shell jax.seldon.panel open\|close\|toggle\|show\|hide` | the bar panel |
| `omarchy-shell jax.seldon.panel pill` | what the pill shows, as JSON |
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
  argument. Never builds a shell command from logbook content.
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
