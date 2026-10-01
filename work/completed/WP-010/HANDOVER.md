WP-010 HANDOVER

Branch `wp/010-plugin-skeleton`, worktree `wt/WP-010`. Not pushed, no PR.

## Done

- **`plugin/manifest.json`** (Phase 1): kinds `service`, `bar-widget`,
  `overlay` (no `panel`, SPEC-PLUGIN §1). `barWidget` gains `aliases`,
  `defaults` and a settings `schema` with one key, `captureIntervalMin`
  (integer, 5–120, default 15). The setting is forwarded to the service.
  `omarchy plugin validate` passes.
- **`plugin/Model.js`**: pure helpers with no Qt and no I/O, also run under
  node. Covers:
  - `parseIndex`, which checks `contractVersion` and reports the version
    found;
  - `deriveStatus`, with precedence engineMissing > file missing/invalid >
    contractMismatch > notInitialised > indexStale (> 2 h) > ok;
  - pill text `⟡ A · D` and tone (urgent on crisis, accent on A > 0);
  - tooltip text, matching the SPEC example word for word;
  - `bannerFor`, one per non-ok status, with constant fix commands only;
  - `validateArgs`, which accepts exactly the CONTRACT.md command forms,
    checks ids against the schema regexes, and allows an optional
    `--json`;
  - the dev clock.
- **`plugin/Service.qml`**:
  - Index: `FileView` with `watchChanges` and `printErrors: false`, parsed
    in try/catch. A 5 s re-read runs while the file is missing or invalid.
  - `status` state machine with all six states. Engine detection runs
    `seldon --version --json`. A missing binary is detected through
    `started`/`running`, because Quickshell emits no `exited` in that case.
  - Engine calls: one queue serialised by `busy` (never two at once), plus
    a 5 min watchdog.
  - Capture cycle: at start and every N min (ADR-0005), running capture and
    then status. Exit 3 sets notInitialised. An index newer than that exit,
    or a successful call, clears it.
  - `run(args)` takes fixed argument lists only. Free text and ids are
    single argv elements. `fix(actionId)` runs the banner fixes;
    `wl-copy` and the Omarchy floating terminal get constant commands.
  - Dev overrides: `SELDON_INDEX` switches to a read-only dev mode (the
    engine is probed, never run). A relative path resolves against the
    shell cwd. `SELDON_NOW` pins the clock; without it the clock is pinned
    to the index's `generatedAt`, so the fixture never goes stale on its
    own.
  - IPC target `jax.seldon.service`: `status`, `refresh`, `capture`.
- **`plugin/BarWidget.qml`**:
  - The pill: `WidgetButton` with accent / urgent (`bar.urgent`) colours,
    dimmed while status is not ok, glyph only on a vertical bar, and the
    tooltip.
  - Clicks: left toggles the panel, middle runs `bar.shell.toggle` (the
    overlay), right captures.
  - The service comes from `bar.shell.serviceFor(id)` via a
    `QtObject`-typed property, with polling until it appears.
  - IPC target **`jax.seldon.panel`**: `open`, `close`, `show`, `hide`,
    `toggle`, plus `pill` (JSON of what the button shows).
  - The misleading routing comment is replaced. It now explains that
    `shell summon|toggle jax.seldon` reaches the overlay, and why
    `open/close/opened` remain: the bar's Tab navigation and popout
    coordinator read them.
- **`plugin/Panel.qml`**: a skeleton not listed in the WP outputs. It is
  needed to show the banners: a `KeyboardPanel` with the title, the banner,
  the summary counts, the last engine error and the dev-mode path. Tab and
  Esc work; `c` captures. Tab passes the slot widget to
  `bar.switchPanelFrom`, as the clock does.
- **`plugin/components/Banner.qml`**: renders `Model.bannerFor()` (title,
  detail, command, one `qs.Ui` `Button` per fix), all as plain text. Only
  `Style`/`Color`/`Border` tokens are used.

  | Status | Fix |
  |---|---|
  | engineMissing | Install in terminal / Copy `omarchy pkg aur add jax-seldon`, Check again |
  | notInitialised | Run in terminal / Copy `seldon init`, Check again |
  | indexMissing | Build index (`seldon status`) |
  | indexStale | Capture now |
  | contractMismatch | Update in terminal / Copy `omarchy plugin update jax.seldon` or `yay -S jax-seldon` |
- **`plugin/README.md`**: covers the pill, the states, the IPC table
  (names both targets and explains the overlay routing), the binding,
  settings, security/privacy/privileges, and the dev overrides.
- **Tests and gate** (`tests/plugin/`, new):
  - `model.test.js`: 17 node tests.
  - `service-states.sh` with `harness/shell.qml`, `fake-seldon` and
    `fake-recorder`: 48 checks. Each runs Service.qml in a private
    headless Quickshell (`QT_QPA_PLATFORM=offscreen`). Scenarios cover:
    - every status;
    - a relative path, a late-appearing index, and an atomic replace;
    - an engine installed at runtime;
    - the live capture→status loop, including proof of serialisation;
    - exit 3, and `seldon init` followed by Check again;
    - the exact argv of the copy/terminal fixes;
    - dev mode never running the engine.
  - `check-tokens.py` (the bonus): checks every
    `Style|Color|Border|Util.member[.sub]` against the shell's Commons. It
    catches typos and gives no false positives on 173 first-party
    references.
  - `justfile`: `qmllint` runs the token check, and the new `plugin-test`
    recipe (host-only, skips under `SELDON_SKIP_HOST_CHECKS`) is part of
    `check`.
- **`docs/TESTING.md`**: adds the plugin section, covering the three
  layers, the dev-host rules, and the test-host procedure (install, restart,
  forcing states via the real paths, IPC read-out, `grim`, `qs log`
  pass/fail criteria including the known shell-wide noise, and cleanup).
- **`memory/omarchy-shell.md`**: appended the WP-010 findings (see
  Learned).

## Not done

- **Physical mouse clicks on the pill were not performed** (no pointer
  injection over ssh). Their IPC equivalents were exercised live:
  - panel `open`/`close`;
  - `shell toggle jax.seldon` for the overlay;
  - `jax.seldon.service capture`.

  Tab and Esc were sent with `wtype`. The left/middle/right handler is 5
  lines and was reviewed.
- **Copy / Install / Run in terminal were not clicked on a live shell.**
  Their exact argv is verified in the harness with fake `wl-copy` and
  terminal launcher. Running them for real would have installed packages or
  opened terminals on the test host.
- **SPEC-PLUGIN §7 theming test** (three themes incl. a light one) not done.
  Only Osaka Jade on the test host. Colours come only from tokens.
- **Not tested:** multi-monitor (the test host has one output), a vertical
  bar, and the settings UI in Setup > Plugins.
- **Overlay.qml unchanged** (stub). Only its route was verified.
- **No snapper-degraded banner and no crisis strip** (SPEC-PLUGIN §5).
  They are not among the five states and belong to the Panel WP.
  `fixtures/index-variants/snapper-degraded.json` renders as `ok`, with
  counts.
- **On the dev host the plugin was only copied and validated**
  (`~/.config/omarchy/plugins/jax.seldon`, disabled). It was not enabled:
  red zone.

## Verified by

```
$ just check                                   → exit 0
  cargo fmt/clippy/test ok (8 CLI tests), schema-validate ok
  plugin-validate: ok
  tokens: ok (40 references) · qmllint: ok (5 files), --max-warnings 0
  model.test.js: 17 passed
  service-states: 48 passed, 0 failed       (final suite run twice, both green)
$ SELDON_SKIP_HOST_CHECKS=1 just plugin-test qmllint plugin-validate → all "skipped"
$ find plugin -type l | wc -l                  → 0
$ omarchy plugin validate ~/.config/omarchy/plugins/jax.seldon   (dev install) → ok
```

Headless, with `SELDON_INDEX=fixtures/index.sample.json` and the engine on
`PATH`:

```
status ok · pill "⟡ 2 · 3" · tone urgent
```

That is 2 active cases and 3 open drift, 2 of them crises. With `PATH`
lacking `seldon` the status is engineMissing (pill still `⟡ 2 · 3`,
dimmed).

**Runtime smoke test on the test host** (Omarchy 4.0.4-1, quickshell 0.3.1,
28 third-party plugins). The final code is commit 3067c07. Steps:

1. rsync, `omarchy plugin validate` (ok, 0 symlinks), `rescanPlugins`,
   `omarchy plugin enable jax.seldon`, `omarchy-restart-shell`.
2. States forced through the real paths: a fixture copied atomically to
   `~/.local/state/seldon/index.json`, and the WP-001 engine binary added
   to or removed from `~/.local/bin`.

| Forced | `jax.seldon.panel pill` |
|---|---|
| no engine, fresh sample | engineMissing · `⟡ 2 · 3` urgent, dimmed |
| engine on PATH + `refresh` | ok · `⟡ 2 · 3` urgent, not dimmed |
| sample, generatedAt −3 h | indexStale · `⟡ 2 · 3` dimmed |
| not-initialised variant | notInitialised · `⟡` dimmed |
| contract-v2 | contractMismatch · `⟡` dimmed |
| index deleted | indexMissing · `⟡` dimmed |
| `shell toggle jax.seldon` | Prime Radiant overlay opens; `hide` closes it |

Screenshots (`grim`) of the bar and of the open panel for every state show
the pill and the matching banner with its buttons, in theme colours.

`quickshell log --pid <shell>` over the whole run contains **no QML error
or warning from jax.seldon**. The only line naming it is the expected
`WARN: Process failed to start … ("seldon", "--version", "--json")`, once
per deliberate engine-missing probe. The other warnings appear with
jax.seldon disabled too: the `QObject::connect(QJSEngine, QtObject):
invalid nullptr parameter` pair, and duplicate IpcHandler lines for other
plugins.

Test host left with jax.seldon installed and enabled. Engine, index and
temp files are removed, so the plugin shows engineMissing. The shell was
restarted three times (`omarchy-restart-shell`), and the bar was rebuilt by
one enable/disable cycle.

## Learned (in memory/omarchy-shell.md)

- **Hot reload does not load new plugin code.** The shell calls
  `Qt.clearComponentCache` only if it exists, and it is `undefined` in
  Quickshell 0.3.1. After changing code, run `omarchy-restart-shell`.
  docs/HERDR-SETUP.md §5 should say so.
- **A Process that cannot start** emits neither `started` nor `exited`.
  Quickshell logs a WARN.
- **`onExited: function(exitCode)` fails the qmllint gate**
  (`QProcess::ExitStatus`). A `Connections` handler lints clean.
- **A third-party widget gets its service** through
  `bar.shell.serviceFor(ownId)`.
- **`bar.run()` executes a shell string.** Use `Quickshell.execDetached`
  with an argv list.
- **`shell togglePanelAt <section> <n>`** on our pill opens the overlay,
  not the panel, because it routes by id.
- **The running shell has Hyprland's environment**, so `SELDON_INDEX`
  cannot be injected from a terminal. The headless harness and the real
  paths cover that.
- **`omarchy pkg add` reaches the official repositories only.** AUR
  packages need `omarchy pkg aur add`.

## Decisions needed

1. **Engine install command.** ADR-0004 and SPEC-PLUGIN §5 say
   `omarchy pkg add jax-seldon`. That runs `pacman -S` (official repositories
   only) and cannot install an AUR package. The banner uses
   `omarchy pkg aur add jax-seldon`. Please supersede ADR-0004 on this
   point and amend SPEC-PLUGIN §5 — or tell me to change the banner.
2. **Update commands for contractMismatch.**
   - Index newer than the plugin: `omarchy plugin update jax.seldon`.
   - Engine older: `yay -S jax-seldon`. `pkg aur add` uses `--needed`
     plus a missing-check, so it never upgrades.

   Accept, or name the commands you want.
3. **A second IPC target, `jax.seldon.service`** (`status`, `refresh`,
   `capture`). The smoke test reads state from it headlessly. SPEC-PLUGIN §8
   names only the panel target, `jax.seldon.panel`. Accept both and amend
   §8, or drop the service target.
4. **Dev-mode semantics** (CONTRACT.md rule 1, SPEC-PLUGIN §9):
   `SELDON_INDEX` makes the plugin read-only (no engine calls), so it never
   writes the real index or logbook while showing a fixture. `SELDON_NOW`
   is new. Please record both in the docs, which are not mine to edit.
5. **Missing-engine log line.** While the engine is missing, Quickshell logs
   one WARN per probe: at start, every capture interval (15 min) and on each
   Check again. Accept it as the documented, expected line, or should the
   probe avoid it (for example by checking `PATH` via a constant
   `["sh", "-c", "command -v seldon"]`, which is outside the CONTRACT
   command list)?
6. **Index path.** The plugin reads `$HOME/.local/state/seldon/index.json`
   literally, as CONTRACT.md says, and ignores `XDG_STATE_HOME`. WP-003
   must write the same path.
7. **Spec updates** (orchestrator-owned):
   - HERDR-SETUP §5: restart the shell after copying changed plugin code.
   - SPEC-PLUGIN §2: `Panel.qml` now exists as a skeleton.
   - SPEC-PLUGIN §8: the `togglePanelAt` quirk.
8. **Fixtures** (owned by WP-014; nothing missing for WP-010): an
   `index-variants/index-stale.json` with `state.status: "indexStale"` would
   exercise that branch from data instead of the clock. The sample's
   `generatedAt`/`lastCapture` (17:05 on 2026-10-01) lie in the future for
   most of that day; the plugin treats a future timestamp as fresh and shows
   "just now".

## Touched outside WP scope

- `justfile`: the token check in `qmllint` (suggested by the WP) and the new
  `plugin-test` recipe added to `check`. WP-003 and WP-014 may also edit
  this file, so expect a small merge.
- `tests/plugin/` (new directory): the harness, fakes, node tests and token
  checker.
- `memory/omarchy-shell.md`: append, as instructed.

`schema/`, `fixtures/`, `scripts/` and `engine/` were not touched. The
engine binary used on the test host is a build of the WP-001 engine from
this branch.
