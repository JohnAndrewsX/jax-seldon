# WP-228 handover — the app skeleton (`app/`, Changes view)

Branch `wp/228-app-skeleton` (off `app`). Not pushed.

## What was done

- `app/seldon-desk.pro` (qmake6; `core gui qml quick quickcontrols2
  concurrent`; deprecated API off up to 6.9), flat `app/src/`,
  `app/bin/build` (Qt ≥ 6.9 gate as Hype's, `make -j8`), `app/bin/test`
  (builds the Qt Test binary and runs it offscreen with its own `HOME`,
  private `XDG_RUNTIME_DIR` 0700, `TMPDIR` on disk under `app/build/`,
  all removed afterwards), `app/.gitignore`, `app/LICENSE` (Seldon's MIT
  plus the template's MIT notice for David Heinemeier Hansson),
  `app/README.md` (screenshot, keys table, states, storage).
- `main.cpp`: GUI only, `QGuiApplication`, `setDesktopFileName("seldon")`,
  Material style; `Main.qml` sets `Material.Dark`, the accent, the theme
  background and `roundedScale: theme.radius`.
- `theme.{h,cpp}`: the template's Theme (accent, `accentForeground`,
  `hyprctl` rounding, re-armed watch) plus the shell's roles from
  `colors.toml` as `Commons/Color.qml` reads them (foreground, background,
  accent, urgent = `red`/`color1`, muted = `muted`/`color8`/foreground) and
  Style's fills over the foreground — what `Tones.js` needs. The only colour
  literals are its fallbacks (`#FFD60A` accent, the shell's defaults for the
  rest). Reads `$HOME/.local/state/omarchy/current`, as `Commons/Color.qml`
  and the template do, whatever `XDG_STATE_HOME` says (round 1, N5).
- `translator.{h,cpp}`: the template, unchanged (one attribution line added).
- `backend.{h,cpp}`: reads the index path it is given (the app: the state
  index, or `SELDON_INDEX`/`SELDON_NOW` read-only dev mode per CONTRACT.md
  rule 1); watches file, directory and nearest ancestor and re-arms after
  every read; reads and parses on a worker thread (`QtConcurrent`), refusing
  ≥ 1 000 000 bytes before reading; `parseIndex` mirrors `Model.parseIndex`
  (empty, parse, shape, contract incl. ADR-0051's `contractReadableFrom`);
  `deriveStatus` mirrors `Model.deriveStatus` plus `loading`; probes
  `seldon --version --json`; asks `status --json` once when there is no
  index (exit 3 → notInitialised); runs engine calls through a one-at-a-time
  `QProcess` queue with fixed argv, checked by `validateArgs` (only
  `--version`, `status`, `capture --all --json --quiet`, `drift
  show|link|explain|dismiss` in the CONTRACT.md forms; text one line after
  `--`); `openTerminal` starts only `omarchy-launch-floating-terminal-with-presentation`
  with one script, and refuses any script that is not in Model.js's
  `TERMINAL_SCRIPTS` (evaluated once from the compiled-in `Model.js`), whatever
  the caller passes. The version probe is killed after 10 s (engine missing).
  Dev mode runs nothing but the version probe.
- QML: `Main.qml` (mirrored root, header, `?` overlay, `Q` — asks while a
  typed text is unsent, Space — capture or the state's fix, Esc, ↑/↓/j/k,
  f/F, Enter; shortcuts off while typing), `StateCard.qml` (the four states
  plus contract mismatch and loading, one fix each: Install / Create logbook
  via Omarchy's terminal and Model.js's scripts, Build index = `status`,
  Capture now = `capture` + `status`; the stale state shows as a strip above
  the list), `ChangesView.qml` (chips with counts, events by day with
  localised day headers, selection by event id), `ChangeRow.qml`,
  `ChangeDetail.qml` (the change in full; Link with the case picker,
  Explain, Dismiss, "only this event" for groups; every resolution armed by
  the first press and run by the second, disarmed by any form change,
  another selection or Esc), `HelpOverlay.qml`, `SeldonButton.qml`.
  See "Review round 1" below for how arming, the pending call and selection
  work now.
- `Model.js` and `Tones.js` are compiled in from `plugin/` unchanged
  (`resources.qrc` aliases `../../plugin/…`), not copied.
- `src/i18n/en.json` (empty: no plural strings yet) and `de.json` (every
  literal `qsTr` of the window, plus the chip, status and fixed row-note
  words that come from Model.js).
- `docs/images/app-changes.png`: the fixture in dev mode, default accent,
  empty theme directory, from the test harness.

## Review round 1 (SEND BACK) — what changed

1. **B1, the armed case.** The picked case is kept by id (`caseId`), not by
   its position in the picker. The first press saves the argv; a new index
   or any form change that changes the argv disarms at once (`built`
   watch), and the second press runs only an argv equal to the armed one,
   else it disarms and says nothing ran. Tests: the reviewer's
   `reviewCaseShiftsWhileArmed` (a case above the pick leaves; the form
   stays armed and runs exactly `drift link … C-2026-004`) and
   `armedCaseLeavingDisarms` (the picked case leaves; disarmed, nothing runs).
2. **B2, `pending`.** The running argv is saved; the answer is matched to it
   (and read with its own action), so changing the action cannot strand the
   form. Another selection clears `pending`. The action tabs, the picker, the
   text field and the check box are locked while a call runs. Test: the
   reviewer's `reviewPendingSticks`, extended (lock, answer, selection).
3. **B3, rich text.** Every `Label` of the window is `Text.PlainText`; the
   picker has its own `contentItem` and an `ItemDelegate` with a plain,
   eliding `Label`. Test `reviewRichText`: a case title with
   `<font …><img …>` and an engine stderr with `<font …>` show as the
   characters they are, and every text item of the window (the open popup's
   included) except the window's own catalog words is plain.
4. **B4, the stale strip.** The card's column is placed by `x`/`y` and its
   implicit height instead of anchors that switch at runtime. Test
   `reviewStaleLayout` asserts the fix button lies inside the card and both
   lie above the Changes view.
5. **N1, ADR-0051.** A readable newer index shows the quiet line "The engine
   writes index vN; this window reads v2 — update Seldon." above the list
   (`contractNotice`; the window's wording of CONTRACT.md rule 3's plugin
   notice). Test `newerContractShowsTheNotice` (shows for v3 readable from
   2, goes for v2).
6. **N2, the terminal allow-list** in C++ (see backend above). Test
   `terminalFixStartsTheLauncherOnly`: the list equals Model.js's; arbitrary
   strings, a script with a suffix and a third argument are refused and
   start nothing; `INIT_SCRIPT` starts the launcher.
7. **N3.** `formChangesDisarm`: typing, another action tab (clicked), "only
   this event" and another case each disarm an armed form.
8. **N4, ask before discarding.** Selecting another change (j/k, ↑/↓,
   click) while a typed reason is unsent asks first ("Your text is not sent
   yet. Select another change again to drop it, Esc to keep it."); the second
   try goes ahead, Esc or typing on keeps it. A chip change with a draft keeps
   the change in the detail. Test `anotherChangeAsksBeforeDroppingText`.
9. **N5, the theme path** (see theme above). Test `themeIgnoresXdgStateHome`.
10. **N6/N7, the smaller points.** The version probe has a timeout (test
    `aHangingEngineCountsAsMissing`); `status --json` is asked once from
    whichever comes last, the probe's answer or the first read; the `.pro`
    comment no longer claims newer API is kept out; the action tabs are
    underlined tabs (no fill), so the checked "Link" tab no longer looks like
    the filled "Link" button; the picker elides; Space in the picker is the
    picker's, not a capture (tested in `formChangesDisarm`).

Decisions taken from the orchestrator: the plain-rectangle buttons stay;
the engine-install fix stays until the package exists.

Mutants run on the new code (each reverted afterwards): no disarm on a
reason change *and* no argv watch → `formChangesDisarm` fails; no plain
text on the popup's item → `reviewRichText` fails; no argv watch →
`armedCaseLeavingDisarms` fails; `pending` kept on another selection →
`reviewPendingSticks` fails; no discard ask → its test fails; the old
switching anchors → `reviewStaleLayout` fails (card 58+109, fix 452+48).
Removing only `onReasonChanged: disarm()` survives: the argv watch disarms
the same change (equivalent; both stay as two guards).

## How it was verified

| Check | Where | Result |
|---|---|---|
| `app/bin/build`, clean rebuild, 0 compiler warnings (`-Wall -Wextra`), round 1 | headless (built on the dev host, Qt 6.12.0; no window opened) | passed |
| `app/bin/test`, round 1: 44 Qt Test cases, all passed — the 34 below plus `themeIgnoresXdgStateHome`, `aHangingEngineCountsAsMissing`, `reviewCaseShiftsWhileArmed`, `armedCaseLeavingDisarms`, `reviewPendingSticks`, `reviewRichText`, `reviewStaleLayout`, `formChangesDisarm`, `anotherChangeAsksBeforeDroppingText`, `newerContractShowsTheNotice` (the terminal test now checks the Model.js list) | headless (offscreen, software scene graph), fake engine, temp dirs | passed |
| `app/bin/test`, round 0: 34 Qt Test cases — theme (fallback, accent, palette, live switch, hyprctl rounding), translator (template cases), `de.json` covers every literal `qsTr`, index parse/size/contract/newer-contract, constants equal Model.js's (evaluated with `QJSEngine`), status precedence, argv allow/refuse list, watcher re-arm across two renames, engine missing, exit 3 → notInitialised and back, fixes run `capture`+`status`, dev mode runs nothing, terminal launcher only; window: fixture renders rows with no QML warning, each state with title and fix (en and de), Enter → Link armed → second Enter runs exactly `drift link <id> C-2026-005 --json`, Esc disarms an armed Explain, `?` overlay, `Q` asks with a draft and is a letter while typing | headless (offscreen, software scene graph), fake engine, temp dirs | passed |
| `QT_FORCE_STDERR_LOGGING=1` offscreen runs of `app/build/seldon-desk` (`env -i`, `PATH=/usr/bin`, which holds no `seldon`), round 1 again: fixture dev mode (en), no index and no engine (en), fixture dev mode with language `de` — private `HOME`, `XDG_RUNTIME_DIR` 0700, `XDG_STATE_HOME` under `app/build/`, removed afterwards | headless | no output on stderr in any run |
| `docs/images/app-changes.png` refreshed (underlined tabs, eliding picker) | headless (fixture) | — |
| No symlink under `app/`, no `qs.*` import, no hex colour in `app/src`, `app/bin`, the `.pro` outside `theme.cpp`'s fallbacks | fixture (grep, find) | passed |
| Build and run against Qt 6.9 / 6.11 | not run | no older Qt on this host; WP-230's CI container build is the check. Nothing knowingly newer than 6.6 is used (`font.features`). |
| The window on a real compositor (GPU scene graph, Material shader layers, Hyprland rounding, a live theme or language switch by hand) | not run | desktop red zone; covered only by the offscreen tests above |
| `omarchy plugin validate`, `qmllint` | not run | not part of this WP's acceptance; `plugin/` is untouched |

The real `~/.local/state/seldon`, `~/Seldon`, `~/.config` and
`/run/user` were never read or written: every run got temp paths, and the
real engine binary was never started (the smoke runs had `PATH=/usr/bin`,
the tests a fake engine).

## What was not done / open points

1. **Model.js texts stay English.** Row notes, results ("Linked 1 event to
   …"), `driftSummary` and error texts come from Model.js unchanged; only
   the fixed words (chips, crisis/attention, "No case") pass through the
   catalog. A text seam in Model.js (ADR-0045 §10's `text()`) is the place
   to fix that; it touches the plugin, so not here.
2. **The skill's "single binary".** `seldon-desk` is GUI only and has no
   CLI half; per 04-decision §1 the CLI stays the engine's. `seldon open`
   starting it is WP-229.
3. **Material's own button fill** is drawn through a shader layer the
   software renderer leaves out (offscreen tests, screenshots), so
   `SeldonButton.qml` fills buttons with a plain rectangle in theme colours
   instead. Decided in round 1: this stays.
4. **Model.js is evaluated four times** (Main, StateCard, ChangesView,
   ChangeDetail each import it; it is not a `.pragma library`) plus once in
   Tones.js. Rows import nothing. Measurable later; a library shim would
   need a plugin-side change or a wrapper.
5. **No pacman `IgnorePkg`, triage proposal, hide, search, transaction
   detail or capture-warning surfaces** of the plugin's Changelog; the
   skeleton has the list, chips, detail and the three resolutions only.
6. **Engine missing → Install** reuses the plugin's GitHub installer
   script. Once the `seldon` package ships the app with the engine, this
   state means a broken install and its fix should change (WP-230/ADR-0056).
   Decided in round 1: it stays until the package exists.
7. **Exit 3 detection** asks `seldon status --json` once at start when no
   index exists (the plugin does it through its capture cycle). The app
   never captures by itself; Space does.
8. `shell.toml` overrides of the shell's surfaces and fills are not read;
   the Theme follows `colors.toml` only.
9. **The init/no-index race** (round 1, N6) is fixed by asking from both
   sides, but no test forces the probe to finish before the first read; the
   existing `findsTheEngineAndTheIndex` covers the usual order only.
10. **The newer-index note has no action.** The plugin's notice runs
    Omarchy's plugin update; the window's update path is the `seldon`
    package (WP-230), so the note only says to update Seldon.
