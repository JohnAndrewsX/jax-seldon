# seldon-desk

The Seldon window: what changed on your machine, who changed it, and why.
A native app for Omarchy, built the way Omarchy's own apps are (C++ and Qt
Quick, qmake6 and make, following the Omarchy theme and language live).

![The Changes view with the sample index](../docs/images/app-changes.png)

This is the skeleton of the app (WP-228, on the `app` branch): one view,
**Changes**. The engine (`seldon`) stays the only writer; the window reads
`${XDG_STATE_HOME:-~/.local/state}/seldon/index.json` and runs `seldon … --json`
with fixed argument lists, like the bar plugin (docs/CONTRACT.md). It reuses
the plugin's `Model.js` and `Tones.js` unchanged.

## Build and test

```sh
app/bin/build    # qmake6 + make into app/build/seldon-desk (Qt 6.9 or newer)
app/bin/test     # the Qt Test binary, offscreen, with its own HOME and runtime dir
```

Look at the sample index without touching a logbook (read-only dev mode,
CONTRACT.md rule 1):

```sh
SELDON_INDEX=fixtures/index.sample.json app/build/seldon-desk
```

`SELDON_DESK_SCREENSHOT=<file> app/bin/test windowShowsTheFixture` writes
the screenshot above from the fixture with the default accent.

## Keys

| Key | What it does |
|---|---|
| ↑ ↓, j k | Move through the changes |
| f, F | Next or previous chip |
| Enter | Open the form of an open change; in the form, press (twice to confirm) |
| Space | Capture now, or the fix of the state shown |
| Esc | Leave the form, disarm, close the key overlay |
| ? | Show or hide the keys |
| Q | Quit (asks first while a text is not sent) |

## States

Each state that keeps the changes from showing has one fix: the engine
missing (Install, in Omarchy's terminal), no logbook (Create logbook, in
Omarchy's terminal), no index (Build index: `seldon status`), a stale index
(Capture now: `seldon capture`). An index of a contract the window cannot
read says so and checks again; a newer one it can still read (ADR-0051)
shows, with a quiet note to update Seldon.

A typed reason is not lost by accident: Q, or selecting another change,
asks first; the second try goes ahead, Esc stays.

## Storage and settings

The window writes nothing itself. It reads the index, the Omarchy theme
(`~/.local/state/omarchy/current/theme/colors.toml`) and language
(`~/.config/omarchy/language`); no Omarchy config is changed.

## License

MIT. The template parts (Theme, Translator, build scripts) keep David
Heinemeier Hansson's MIT notice; see [LICENSE](LICENSE).
