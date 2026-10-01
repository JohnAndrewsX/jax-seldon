# TESTING.md — How Seldon is checked

One command gates every work package: `just check`, run from the repository
root. It must exit 0 before a handover (AGENTS.md §5).

## `just check`

| Step | Recipe | What it runs | CI |
|---|---|---|---|
| Format | `fmt-check` | `cargo fmt --check` on `engine/` | yes |
| Lint | `clippy` | `cargo clippy --all-targets -- -D warnings` | yes |
| Tests | `test` | `cargo test` (unit + CLI tests in `engine/tests/`) | yes |
| Contract | `schema-validate` | `bash scripts/validate-fixtures.sh` (WP-002); skipped with a notice while the script does not exist | yes |
| Plugin manifest | `plugin-validate` | `omarchy plugin validate plugin/` | **no** (dev host) |
| QML lint | `qmllint` | `qmllint` on `plugin/*.qml` and `plugin/components/*.qml` against `$OMARCHY_PATH/shell`, then the token check `tests/plugin/check-tokens.py` | **no** (dev host) |
| Plugin logic | `plugin-test` | `node tests/plugin/model.test.js`, `bash tests/plugin/service-states.sh`, `bash tests/plugin/panel-view.sh` (see "Plugin") | **no** (dev host) |

Other recipes: `just build-release` (static musl binary,
`x86_64-unknown-linux-musl`), `just fixtures-refresh` (stub until the engine
builds an index).

## Engine tests

Run them all with `just test`, or directly:

```
cargo test --manifest-path engine/Cargo.toml --locked            # everything
cargo test --manifest-path engine/Cargo.toml round_trip::         # frontmatter vs fixtures/logbook
cargo test --manifest-path engine/Cargo.toml --test init          # `seldon init`
cargo test --manifest-path engine/Cargo.toml --test doctor        # `seldon doctor`
```

| Where | What |
|---|---|
| `engine/src/**` (`#[cfg(test)]`) | unit tests: frontmatter parser and writer, models, config precedence, lock, templates, subprocess runner |
| `engine/tests/cli.rs` | `--version`, `contract-version`, parse errors (exit 1, JSON error shape, `--json` detection), `--help` |
| `engine/tests/frontmatter.rs` | `round_trip::` every case, journal, decision, area, memory file and `PROJECT.md` of `fixtures/logbook/` parses into its typed record and re-serialises byte-identical; a lossless update changes only the edited lines |
| `engine/tests/init.rs` | `init::` layout (SPEC-LOGBOOK §2), JSON output, git first commit, `--no-commit`, German templates, Obsidian, path precedence, refusals (existing logbook, non-empty dir, no terminal), lock held → exit 4 |
| `engine/tests/doctor.rs` | `doctor::` green after init with snapper degraded, exit 3 when not initialised, invalid frontmatter, misplaced case, the fixture logbook (and that doctor leaves it untouched) |

**Isolation.** The integration tests never see the real home, config,
state or logbook (AGENTS.md §6). `engine/tests/common/mod.rs` gives each
test a temporary `HOME` (so `~/.config/seldon` and `~/.local/state/seldon`
live under it), and a `PATH` that contains only:
- stub `omarchy-version` and `snapper` scripts (the snapper stub prints
  `No permissions.`, a snapshot list, or is absent, per test);
- a link to the host's `git`.

So the results do not depend on what the host has installed or how snapper
is configured. Tests that need git skip themselves when the host has none.
`fixtures/logbook/` is read-only input.

**Monorepo layout.** Some tests read files outside the crate, so they only
compile and pass in a checkout of the whole repository, not from the
`engine/` directory alone (for example a crate tarball or an AUR source
that ships only the engine):
- `cli.rs::contract_version_matches_plugin_manifest` embeds
  `plugin/manifest.json` with `include_str!`;
- `frontmatter.rs` and `doctor.rs` read `fixtures/logbook/`.

The packaging WP (WP-040) has to either ship those directories or build
with `cargo build` only (no tests).

**Manual acceptance (WP-003).** To try `init` and `doctor` against the real
`snapper`/`omarchy-version` without writing the operator's
`~/.config/seldon`, redirect the XDG dirs:

```
export XDG_CONFIG_HOME=$(mktemp -d) XDG_STATE_HOME=$(mktemp -d)
engine/target/debug/seldon init --non-interactive --path /tmp/seldon-wp003
engine/target/debug/seldon doctor --path /tmp/seldon-wp003 --json   # "ok": true, snapper "degraded"
```

`seldon init` refuses an existing logbook, so remove `/tmp/seldon-wp003`
before running it again.

The engine needs Rust ≥ 1.89 (`File::try_lock`, let-chains); both hosts
have 1.98.

## What CI cannot run, and where it runs instead

CI (`.github/workflows/ci.yml`) runs `just check` in an `archlinux:base-devel`
container with `SELDON_SKIP_HOST_CHECKS=1`. That variable makes the
host-only steps print a skip notice and exit 0:

- **`omarchy plugin validate`** needs the `omarchy` CLI, which only exists on
  an Omarchy install.
- **qmllint against the shell** needs the installed shell tree
  (`$OMARCHY_PATH/shell`, default `/usr/share/omarchy/shell`) and Quickshell's
  QML modules (`/usr/lib/qt6/qml/Quickshell`).
- **`plugin-test`** needs `node`, `quickshell` and `jq`.

All run on the **dev host**: `just check` there runs them, and a missing
tool is an error, not a skip. Never set `SELDON_SKIP_HOST_CHECKS` on the dev
host.

## qmllint details

- **Binary.** `qmllint` from `qt6-declarative`. On the dev host it is at
  `/usr/lib/qt6/bin/qmllint` (not on `PATH`); on the test host it is
  `/usr/bin/qmllint`. The recipe uses `PATH` first, then the Qt libexec dir.
- **`qs.*` imports.** Quickshell serves the shell root as the module prefix
  `qs` (`import qs.Ui`, `import qs.Commons`), but the installed tree has no
  `qs/` directory, so `-I $OMARCHY_PATH/shell` alone does not resolve them.
  The recipe builds a temporary import root with `qs/<Module>/qmldir` files
  whose entries point back at the shell's files by relative path. Nothing
  from the shell is copied or linked.
- **Zero warnings** (`--max-warnings 0`), except two categories demoted to
  info because first-party plugins hit them as well and no plugin code can
  avoid them:
  - `missing-property` — nested token objects such as `Style.font.body` or
    `Color.popups.text` are declared as `QtObject` properties, so qmllint
    sees them as plain `QObject`.
  - `uncreatable-type` — Quickshell's qmltypes register `PanelWindow` with
    `isCreatable: false`; it is created fine at runtime.
- **Token check.** Because `missing-property` is demoted, qmllint cannot see
  a typo such as `Style.font.bodySmal`. `tests/plugin/check-tokens.py` reads
  `Commons/{Style,Color,Border,Util}.qml` of the installed shell and fails on
  any member the plugin references that they do not declare (it passes the
  first-party clock, agents and media plugins without a false positive). It
  does not replace the runtime smoke test: it knows nothing about other
  types.

## Dev host vs test host

| | Dev host | Test host |
|---|---|---|
| Rust | rustup, musl target | pacman `rust`, no musl target → `build-release` fails |
| `just` | via mise | not installed |
| qmllint | `/usr/lib/qt6/bin/qmllint` | `/usr/bin/qmllint` |

The first build fetches the engine's crates from crates.io; after that the
build itself needs no network (`cargo build --offline` works).

## Plugin

Four layers, cheapest first. The first three run in `just check`
(`just plugin-test`); the fourth is the hard acceptance gate of every plugin
WP (SPEC-PLUGIN §9).

### 1. `Model.js` under node

`node tests/plugin/model.test.js` loads `plugin/Model.js` into a plain VM
context (the file has no Qt dependencies, by design) and checks pill text
and colour rules, status precedence, tooltip wording, one banner with a
constant fix per non-ok status, that `validateArgs` accepts exactly the
command forms of CONTRACT.md (free text one non-empty argument after `--`,
`drift show <id> --json`, `[--only]`), the `XDG_STATE_HOME` index path, and
the tab helpers against the fixture: 58 Changelog rows, one "+3" group, 7
folded resolution details, 6 snapshot rows, the source filter, the crisis
strip text, the snapper banner, the Today view and the System sections with
every field optional.

### 2. `Service.qml` in a private headless Quickshell

`bash tests/plugin/service-states.sh` starts `tests/plugin/harness/shell.qml`
with `quickshell -p` and `QT_QPA_PLATFORM=offscreen`, once per scenario. The
harness loads `plugin/Service.qml` the way the shell loads a third-party
service (no parent), prints a JSON snapshot (`status`, `pill`, `banner`,
`engine`, …) and quits. It is a separate Quickshell instance: it never talks
to the running omarchy-shell and writes only to a temp dir.

Scenarios: every status (fixture index, `PATH` without `seldon`,
`index-variants/not-initialised.json`, a missing path, broken JSON,
`SELDON_NOW` three hours after `generatedAt`, `invalid/index.contract-v2.json`),
a relative `SELDON_INDEX`, an index that appears after start, an atomic
replace (temp file + rename), an engine installed while running ("Check
again"), the live loop without the dev override (capture, then status
writes the index; calls never overlap), engine exit 3, the exact argv of
the banner fixes (fake `wl-copy` and terminal launcher record it), the
crisis strip text, `index-variants/snapper-degraded.json` with the argv of
its *Copy* and *Run in terminal*, `XDG_STATE_HOME` (absolute and the
ignored relative form), and dev mode never running the engine.
`tests/plugin/fake-seldon` stands in for the engine. The scenarios run with
a `PATH` made of symlinks to the few tools the fakes need, so a `seldon`
installed system-wide never leaks in.

To watch one case by hand:

```bash
QT_QPA_PLATFORM=offscreen HARNESS_PLUGIN_DIR=$PWD/plugin \
  SELDON_INDEX=$PWD/fixtures/index.sample.json \
  quickshell -p tests/plugin/harness/shell.qml
```

A missing engine makes Quickshell log `WARN: Process failed to start,
likely because the binary could not be found` — expected, and the only log
line the plugin may cause. The service probes for the engine once at start
and again only on *Check again*, so it appears once per shell start.

### 3. `Panel.qml` in a private headless Quickshell

`bash tests/plugin/panel-view.sh` runs the real panel against the real shell
components. Quickshell serves `qs.*` from the config root of the instance,
so the script builds a temp root with copies of `$OMARCHY_PATH/shell/Commons`
and `shell/Ui`, replaces only `Ui/KeyboardPanel.qml` (a layer-shell window,
which an offscreen instance cannot create) with
`tests/plugin/harness/KeyboardPanel.qml`, and starts
`tests/plugin/harness/panel.qml` as its `shell.qml`. That harness loads
Service.qml (dev mode, a fixture), puts Panel.qml in an offscreen window,
opens it and runs `HARNESS_STEPS`: real key presses through the shell's own
`PanelKeyCatcher` (QtTest `keyClick`), tab and filter selection. After each
step it prints `Panel.view()` and every visible text.

Checks: with the sample every tab renders (Today: 4 entries, yesterday
collapsed and opened with Enter; Changelog: 58 rows, the "+3" group expanded
to its members, 7 folded resolution details, 6 highlighted snapshot rows,
the pacman filter narrows to 12, `f` cycles; System: seven sections), the
strip "2 changes in the red zone need a reason" on every tab, the keys
(Tab, Shift-Tab, ←/→, 1–3, ↑/↓, Enter, Esc), the snapper banner on every
tab, the not-initialised variant, an empty and a sparse `system`, and a log
free of warnings, `TypeError`s and binding loops. The shell's `Style.qml`
asks `hyprctl` and `fc-match` for gaps and the font; the script gives it
stubs that fail, and Style keeps its defaults.

The step format is documented in the header of
`tests/plugin/harness/panel.qml`, e.g.
`HARNESS_STEPS="view;tab:changelog;key:Down*5;key:Return"`; a new scenario
is one `run` line plus its `expect`/`shows` checks in `panel-view.sh`.

### 4. Runtime smoke test in the shell

The bar widget, panel and banner import `qs.Ui`/`qs.Commons`, which only the
running shell provides; layer 3 covers them against copies, the live shell
is the final check (layer-shell window, bar anchoring, focus, theme).

**Dev host.** You may copy the plugin to its dev install and validate it
there, nothing more: enabling it writes `~/.config/omarchy/shell.json` (red
zone, AGENTS.md §6).

```bash
rsync -a --delete plugin/ ~/.config/omarchy/plugins/jax.seldon/
omarchy plugin validate ~/.config/omarchy/plugins/jax.seldon
```

**Test host** (the runtime target; ssh alias in `memory/local.md`). Over
ssh, export `OMARCHY_PATH=/usr/share/omarchy` and put `$OMARCHY_PATH/bin` on
`PATH` first; non-interactive shells have neither.

1. Install and enable (docs/HERDR-SETUP.md §5):
   ```bash
   rsync -a --delete plugin/ test:.config/omarchy/plugins/jax.seldon/
   ssh test 'omarchy plugin validate ~/.config/omarchy/plugins/jax.seldon \
     && omarchy-shell shell rescanPlugins; omarchy plugin enable jax.seldon'
   ```
   `rescanPlugins` and `enable` may print "omarchy-shell is not responding"
   while the shell rescans many plugins; check with `omarchy-shell shell
   listPlugins`.
2. **After every code change, restart the shell** (`omarchy-restart-shell`,
   works over ssh). Saving files triggers the shell's hot reload, but it
   re-creates the plugin from cached components (`Qt.clearComponentCache`
   does not exist in Quickshell 0.3.1), so new code does not load.
3. Force the states. The running shell gets Hyprland's environment, not
   yours, so `SELDON_INDEX` cannot be set for it; use the real paths instead
   (on the test host only):
   - engine: copy a `seldon` binary (`just build-release`) to `~/.local/bin`
     (on the shell's `PATH`) or remove it, then
     `omarchy-shell jax.seldon.service refresh`. For screenshots in status
     `ok` before the engine writes indexes, a stand-in script that answers
     `--version` with `{"name":"seldon","version":"…"}` and exits 0 on
     `capture`/`status` without writing anything is enough; remove it after;
   - index: `cp` a fixture to `~/.local/state/seldon/index.json.tmp`, then
     `mv` it over `index.json` (the watch picks it up; a file that did not
     exist at start is found by the 5 s poll). For `ok`, rewrite
     `generatedAt` (and `state.lastCapture`) to now first; for `indexStale`,
     to three hours ago; use `index-variants/not-initialised.json`,
     `index-variants/snapper-degraded.json` and
     `invalid/index.contract-v2.json` as they are, and delete the file for
     `indexMissing`.
4. Read the result:
   ```bash
   omarchy-shell jax.seldon.service status   # status, pill, banner, crisis, snapper, engine, lastError
   omarchy-shell jax.seldon.panel pill       # what the WidgetButton shows
   omarchy-shell jax.seldon.panel open
   omarchy-shell jax.seldon.panel tab changelog       # today | changelog | system
   omarchy-shell jax.seldon.panel filter all          # or a source
   omarchy-shell jax.seldon.panel view       # tab, cursor, rows, badges, banners, strip
   omarchy-shell shell toggle jax.seldon     # Prime Radiant
   ```
   Keys: `wtype -k Tab`, `wtype -M shift -k Tab -m shift`, `wtype -k Down`,
   `wtype -k Return`, `wtype f`, `wtype -k Escape`, each followed by
   `jax.seldon.panel view`. Tab on the last tab opens the bar's next panel
   (`Bar.switchPanelFrom`); if that neighbour opens a window instead of a
   popup panel (OmaSettings on the test host), the Seldon panel stays open.
5. Screenshots: `grim -g "<x>,<y> <w>x<h>"` takes **logical** coordinates;
   the test host's output is scaled 1.25, so a region read off a full
   screenshot (physical pixels) must be divided by the scale. Over ssh also
   export `XDG_RUNTIME_DIR=/run/user/$(id -u)` and `WAYLAND_DISPLAY=wayland-1`.
   Shrink before committing: `magick in.png -strip -resize 80% -colors 64 out.png`.
6. Check the log of the running shell:
   ```bash
   quickshell log --pid "$(pgrep -x quickshell)" | grep -E "WARN|ERROR"
   ```
   `omarchy theme set` and `omarchy-restart-shell` start a new shell
   process, so check every instance of the run: the logs are under
   `/run/user/$(id -u)/quickshell/by-pid/<pid>/`, and
   `quickshell log <path>/log.qslog` reads one.
   Pass: nothing naming `jax.seldon` or a plugin file except the expected
   "Process failed to start" while the engine is missing (once per shell
   start). Not ours: on every bar rebuild the shell logs two
   `QObject::connect(QJSEngine, QtObject): invalid nullptr parameter` lines
   and "Handler was registered but will not be used" for other plugins' IPC
   targets; both appear with jax.seldon disabled too. Other third-party
   plugins on the test host log their own warnings (superproductivity,
   omalauncher, finder); filter by path.
7. Clean up: remove `~/.local/bin/seldon` and `~/.local/state/seldon/` unless
   the next WP needs them.

**Three themes (SPEC-PLUGIN §7)**, on the test host only — switching the
theme is a system change, so never on the dev host. Every plugin WP that
changes what the panel draws repeats it:

1. Note the current theme: `omarchy theme current`.
2. With the sample index in status `ok` (step 3 above), for each of a dark
   theme, a second dark theme and a light theme — e.g. *Osaka Jade*, *Tokyo
   Night*, *Catppuccin Latte* (`omarchy theme list`):
   `omarchy theme set "<theme>"`, wait about 6 s (the shell restarts), open
   the panel and capture Today, Changelog, System and the Changelog filtered
   to pacman with the cursor on the "+3" row and Enter pressed.
3. Look for: every colour follows the theme — panel border, tab and chip
   fills, the zone stripes (red = the theme's urgent colour, yellow = its
   accent), the red strip, banners, the snapshot row highlight; dim text
   stays legible on the light theme; nothing keeps a colour of the previous
   theme.
4. Restore the noted theme with `omarchy theme set "<noted>"` and check it
   with `omarchy theme current`.
5. Save the shrunk PNGs under `work/active/WP-NNN/screenshots/`, named
   `<theme>-<view>.png`.
