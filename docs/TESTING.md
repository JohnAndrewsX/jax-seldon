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
cargo test --manifest-path engine/Cargo.toml plan::               # case state machine, folder moves
cargo test --manifest-path engine/Cargo.toml log::                # notes, journal, Log section
```

| Where | What |
|---|---|
| `engine/src/**` (`#[cfg(test)]`) | unit tests: frontmatter parser and writer, models, config precedence, lock, templates, subprocess runner |
| `engine/tests/cli.rs` | `--version`, `contract-version`, parse errors (exit 1, JSON error shape, `--json` detection past free text), `--help`, `--config` > `SELDON_CONFIG` > XDG config |
| `engine/tests/frontmatter.rs` | `round_trip::` every case, journal, decision, area, memory file and `PROJECT.md` of `fixtures/logbook/` parses into its typed record and re-serialises byte-identical; a lossless update changes only the edited lines |
| `engine/tests/init.rs` | `init::` layout (SPEC-LOGBOOK §2), JSON output, git first commit, `--no-commit`, German templates, Obsidian, path precedence, refusals (existing logbook, non-empty dir, no terminal), lock held → exit 4 |
| `engine/tests/plan.rs` | `plan::` new (template, canonical frontmatter, area on first use, ids never reused), start/verify/done/drop (folder moves per ADR-0012 §9, `started`/`closed`/`snapshotBefore`, `.seldon/active-case`, body byte-identical outside the Log), invalid transitions → exit 1 and nothing written, list/show against `case.schema.json`, the fixture logbook (a copy), git autocommit with `--no-commit` and `git.autocommit = false` |
| `engine/tests/log.rs` | `log::` notes with and without a case (`case.events`, `agents`), the Log section append-only over three steps, the journal appended not rewritten, free text as one argument (spaces, quotes, `$(…)`, `--json` after `--`), month and day by timestamp, redaction, exit 3/4 |
| `engine/tests/journal.rs` | `journal::` appends to a fixture day (only the `cases:` line changes), CRLF days, the `plan done` stub in the logbook language |
| `engine/tests/commands.rs` | `event::` (fixture line shape, typed meta, engine-only kinds refused), `decide::` (ADR numbering, the logbook's own template, the editor gets the path as one argument), `open::` (paths, `--editor` without a terminal) |
| `engine/tests/doctor.rs` | `doctor::` green after init with snapper degraded, exit 3 when not initialised, invalid frontmatter, misplaced case, the fixture logbook (and that doctor leaves it untouched) |

**Isolation.** The integration tests never see the real home, config,
state or logbook (AGENTS.md §6). `engine/tests/common/mod.rs` gives each
test a temporary `HOME` (so `~/.config/seldon` and `~/.local/state/seldon`
live under it), and a `PATH` that contains only:
- stub `omarchy-version` and `snapper` scripts (the snapper stub prints
  `No permissions.`, a snapshot list, or is absent, per test);
- a link to the host's `git`.

So the results do not depend on what the host has installed or how snapper
is configured. Two engine variables make tests deterministic:
- `SELDON_NOW` (RFC 3339 with offset) fixes the clock of one invocation:
  event `ts`, journal headings, Log lines, the case id year
  (`Env::at(now, args)`). Not for normal use.
- `SELDON_CONFIG` (or the global `--config FILE`) points the engine at
  another `config.toml`; `--config` wins over the variable, which wins over
  `$XDG_CONFIG_HOME/seldon/config.toml`.

Every line any test writes to a ledger is validated against
`schema/event.schema.json` (`common::ledger`, `jsonschema` with formats),
and case JSON against `schema/case.schema.json` (`common::assert_valid_case`).
Editors are never started: tests run without a terminal, so `--editor`
goes to `omarchy-launch-editor`, which a test stubs to record its argv. Tests that need git skip themselves when the host has none.
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
the tab helpers against the fixture: 62 Changelog rows, one "+2" group (3 members), 7
folded resolution details, 6 snapshot rows, the source filter, the crisis
strip text, the snapper banner, the Today view and the System sections with
every field optional. For the panel actions (WP-012): the case picker lists
the open cases only, active first, with ids checked; `logArgs` keeps the
note one argument after `--` (`--help`, quotes, a newline, `$(…)`) and
refuses blank text and a malformed case id; `openArgs` takes journal,
ledger, status or a case id and nothing else; the result lines read the
`log`, `open` and `capture` JSON of SPEC-ENGINE §3. For the Work tab
(WP-020): `workColumns` puts the sample's cases into Queued 3, Active 3
(2 active + 1 verification, in that order) and Completed 2, with the
`proposedEvents` count on C-2026-005 only, and survives broken entries;
`wipStatus` says "2 / 3 active" (verification does not count) with its
tone at and over the limit; `caseActions` gives each status its actions;
`planArgs` builds `plan new --zone --risk [--area] [--priority] --json --
<title>` (the title one argument after `--` for `--help`, quotes,
`-rf --zone red`, `$(…)`) and `plan <step> <id> --json`, and refuses a
blank title, a bad zone, risk, priority or area slug, a malformed id and
any other step; `validateArgs` accepts `--area` and `--priority` only in
that order; `planResult` reads both `plan --json` shapes and the engine's
refusal. For the drift sheet (WP-021): `driftItemFor` finds the four
sample items (the theme item with its proposed C-2026-005, the two
crises, the firefox group with 3 members oldest first) and a group
member's item named by that member, and nothing for resolved events;
`caseOptionsFor` puts the proposed case first, else "Pick a case";
`driftArgs` builds `drift link <id> <case> [--only] --json`, `drift
explain <id> [--only] [--zone] [--risk] [--area] --json -- <text>` (zone
only when it differs from the item's, risk only when not R1) and `drift
dismiss <id> [--only] --json -- <text>`, and refuses a malformed id or
case, no case, blank or two-line text, a bad zone or slug and any other
action; `validateArgs` accepts explain's options only in that order;
`driftResult` reads the resolving shape, the no-op (`resolved: 0`,
`already` → "Already resolved: linked to C-…") and refusals;
`driftShowResult`, `memberLines` ("… and N more"), `rowStatus` with
`explained · C-…` (ADR-0021), `firstCrisis` and `moreDriftText` ("+N
more", ADR-0020).

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
`tests/plugin/fake-seldon` stands in for the engine.

The panel actions (WP-012) run through `HARNESS_ACTIONS`, a JSON array of
`["log", text, caseId]`, `["open", what]` and `["capture"]`, started once
the start-up capture is done. The fake engine appends each call's exact
argv to `$HOME/argv.log` (`printf %q`, one line per call), and its `open`
hands the path to a recorded `omarchy-launch-editor`, as the real engine
does without a terminal. Checked: the note is one argument after `--` for
`--help`, `a "b" c` and a two-line text, with and without `--case`; capture
runs before status and a second *Capture now* while one is queued is
dropped; the four open variants reach the launcher with the right path and
the result line reads `open --json`; calls never overlap; blank notes, a
malformed case id and an unknown open target never reach the engine; the
engine's errors reach the result lines; dev mode and a missing engine
refuse with a reason.

The Work tab's calls (WP-020) use `["plan", action, input]` (input: a case
id, or the new case's form) and `["wait"]`, which holds the remaining
actions until no engine call is queued or running, so plan calls run one
after another as the panel sends them. The fake engine's `plan` treats the
index it last wrote as its logbook (a line `<id> <status>` in
`$HOME/cases` overrides it, for a logbook the index has not caught up
with), follows the engine's transition rules and refusal message, and
rewrites the index with the case in its new column. Checked: the exact argv
of two `plan new` calls (an option-like title, one with quotes, zone, risk,
area and priority from the form) and of start, verify, done, drop; the
cases in the index the fake wrote afterwards; `done` on an active case
refused with the engine's message as `planResult`, `lastError` empty; five
refusals in the plugin that never reach the engine; a held lock (exit 4);
dev mode.

The drift sheet's calls (WP-021) use `["drift", action, form]` and
`["driftShow", eventId]`. The fake engine's `drift link|explain|dismiss`
works on the same state index with the engine's checks and messages (a
link's case first, then the event), selects a group's open members or,
with `--only`, the named event, folds the resolution (`resolution`,
`resolutionDetail`, `case`, also for explain per ADR-0021), drops the item
or re-keys the rest of a group to a new leader, recounts the summary,
creates explain's completed case, and answers a re-run with the engine's
no-op (`resolved: 0`, `already`); `$HOME/resolved` stands for a logbook the
index has not caught up with. `drift show` lists the open members plus
those in `$HOME/extra-events.json`. Checked: the exact argv of `drift
show`, a link with the proposed case, an explain with the text `--help`
(zone as the item's, so no `--zone`), a dismiss of a group member with
`--only` and the text `say "hi"; $(reboot)`, an explain of the rest of
that group with zone, risk and area, the re-run, and a link to an unknown
case (the engine's refusal is `driftResult`, `lastError` stays empty); the
fake engine's index afterwards (five folded events, one item left,
summary 1/1, the two new cases); the no-op alone ("Already resolved:
linked to C-2026-005"); eight refusals that never reach the engine; a held
lock; dev mode.

Isolation: the scenarios run with a `PATH` made of symlinks to the few
tools the fakes need, so a `seldon` installed system-wide never leaks in.
Every run, here and in layer 3, gets its own `HOME`, `XDG_STATE_HOME` and
`XDG_CONFIG_HOME` inside the temp dir (a case may name its own `HOME`; the
XDG dirs follow it). Both scripts end with a check
(`tests/plugin/real-home-guard.sh`) that the real `~/.local/state/seldon`
and `~/.config/seldon` neither appeared nor changed during the run; it
compares existence, size, mtime and ctime of every entry, so an engine run
by hand at the same time also fails it.

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
collapsed and opened with Enter; Changelog: 62 rows, Enter on the "+2"
group opens its drift sheet with the three members, 7 folded resolution details, 6 highlighted snapshot rows,
the pacman filter narrows to 12, `f` cycles; System: seven sections), the
strip "2 changes in the red zone need a reason" on every tab, the keys
(Tab/Shift-Tab hand over to the bar, ←/→ and h/l switch tabs, digits fixed per tab id (1, 2, 3, 5; 4 is absent and ignored), ↑/↓, Enter, Esc), the snapper banner on every
tab, the not-initialised variant, an empty and a sparse `system`, and a log
free of warnings, `TypeError`s and binding loops. The shell's `Style.qml`
asks `hyprctl` and `fc-match` for gaps and the font; the script gives it
stubs that fail, and Style keeps its defaults.

Two live scenarios (WP-012) run without `SELDON_INDEX`: the service reads
the state index the fake engine writes. They type into the QuickEntry with
real keys (`--help` would switch tabs if a key leaked to the panel), refuse
a blank note, pick a case with Tab, ↓ and Enter, open the journal, ledger
and STATUS.md with `e`, and press `c`: the fake engine's next `status`
writes an index with one more event, and the Changelog shows 59 rows
through the FileView, without a restart. The exact argv and the editor
paths are compared, and a note the engine refuses keeps its text.

The Work tab (WP-020) has three scenarios. On the sample (dev mode): the
columns Queued 3 · Active 3 · Completed 2 in cursor order, "2 / 3 active",
the "1 proposed" badge on exactly one tile, the card of the case under the
cursor with its actions by status, and nothing armed or run without an
engine. Live: `3`, `+`, the title `--help` typed into the sheet (no tab
switch), zone, risk and priority picked with Tab, ←/→ and Enter, the area
`Dev` refused in the plugin, `dev-env` accepted, Enter: the new case
appears in Queued through the FileView with the cursor on it; then start →
verify → done on C-2026-005 with Enter twice each, the case moving columns
(after start "3 / 3 active · at the limit") and the cursor following it;
Enter on the completed case opens it; Done on C-2026-008, which the fake
engine's logbook has active (`$HOME/cases`), shows the engine's refusal
and changes nothing; a cursor move disarms; x twice drops C-2026-004; `e`
opens it; the exact argv of all of it. Locked: the engine refuses the new
case (exit 4), the sheet shows the message and keeps the title, Esc and
`+` bring it back intact.

The drift sheet (WP-021) has seven scenarios. On the sample (dev mode):
Enter on the theme row and `resolve:<id>` for the other three items open
the sheet with Link and C-2026-005 preselected for the theme item, Explain
with the item's zone for the two crises and the group, the group's three
members and "All 3 / Only firefox", a member row naming its own package,
and a click on the red strip opening the first crisis with the cursor on
its row; nothing can be sent. Capped: `summary.openDrift` 250 shows "+246
more open drift items not listed here". Live, with real keys: Enter,
Enter, Enter links the theme item to C-2026-005 (hint "Press Enter again:
Link tokyo-night to C-2026-005", then `linked to C-2026-005` folded, pill
`⟡ 2 · 3`); a click on the strip opens the first crisis, which is
explained with the text `--help`, risk R2 (the change disarms) and area
`dev-env`; the strip drops to "1 change …", *Open C-2026-009* opens the
new case and Work lists it as completed; the firefox group is dismissed
as one (three rows `dismissed: routine update`, no badge, pill `⟡ 2 ·
1`); the exact argv. `--only`: Link without a case is refused in the
plugin, C-2026-004 is picked in the case picker by keys, *Only firefox*
links the leader alone and the rest returns as "noto-fonts +1" with two
members. Already: an item `$HOME/resolved` lists shows "Already resolved:
linked to C-2026-005" and nothing changes. Locked: the refusal keeps the
text, Esc and reopening bring the draft back, another item gets its own
defaults. Members: with one member missing from `index.events`, the sheet
shows "… and 1 more", asks `seldon drift show` and lists all three.

The step format is documented in the header of
`tests/plugin/harness/panel.qml`, e.g.
`HARNESS_STEPS="view;tab:changelog;key:Down*5;key:Return"`, plus
`type:<text>`, `settle` (no engine call queued or running),
`wait:<view path>=<value>`, `resolve:<event id|crisis>`, `click:<text>`
(the centre of the first visible item with that text) and `shot:<name>`
(saves the window to `$HARNESS_SHOTS/<name>.png`); a new scenario is one
`run` line plus its `expect`/`shows` checks in `panel-view.sh`.

Offscreen theme renders: copy a theme's `colors.toml` from
`$OMARCHY_PATH/themes/<theme>/` to
`<harness HOME>/.local/state/omarchy/current/theme/colors.toml` and add
`shot:` steps; the harness paints the theme's background under the panel.
They show the real components in the theme's colours, not the live
layer-shell window; the live sweep below stays the acceptance check.

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
   omarchy-shell jax.seldon.panel tab changelog       # today | changelog | work | system
   omarchy-shell jax.seldon.panel filter all          # or a source
   omarchy-shell jax.seldon.panel resolve crisis      # or an event id: the drift sheet
   omarchy-shell jax.seldon.panel view       # tab, cursor, rows, badges, banners, strip
   omarchy-shell shell toggle jax.seldon     # Prime Radiant
   ```
   Keys: `wtype -k Tab`, `wtype -M shift -k Tab -m shift`, `wtype -k Down`,
   `wtype -k Return`, `wtype f`, `wtype -k Escape`, each followed by
   `jax.seldon.panel view`. Tab opens the bar's next panel
   (`Bar.switchPanelFrom`); if that neighbour opens a window instead of a
   popup panel (OmaSettings on the test host), the Seldon panel stays open.
5. Panel actions (WP-012), with the real engine (`just build-release`, copy
   to `~/.local/bin/seldon`, `chmod 755`; `seldon init --non-interactive
   --path ~/Seldon-smoke`; restart the shell): `jax.seldon.panel open`,
   `wtype n`, `wtype -- "<note>"`, `wtype -k Return`, then `view` shows
   `today.quickEntry.result` with the event id; check the line in
   `~/Seldon-smoke/ledger/*.jsonl` and the journal. `wtype c` on the
   Changelog shows `capturing: true`, then the capture result. `wtype e`
   opens the tab's file through `omarchy-launch-editor` (see
   `hyprctl clients`); with a terminal editor (nvim, the Omarchy default)
   the engine on main waits 10 s for the launcher, then kills it and
   reports "did not return" (WP-012 handover). To see the case picker, put a fixture index in place
   (step 3): its cases are unknown to the smoke logbook, so a case note
   shows the engine's "unknown case" and keeps its text.
   Work tab (WP-020), with the engine's own index: `wtype 3`, `wtype +`,
   type a title, `wtype -k Tab` / `-k Right` / `-k Return` through the
   pickers, `wtype -k Return` in a text field; `view` shows `work.result`
   "Created C-…" and the case under `work.ids`; `find ~/Seldon-smoke/work`
   shows the file in `work/queued/`. Then `wtype -k Return` twice for each
   of start, verify and done: the file moves to `work/active/`, stays there
   for verification, and moves to `work/completed/`; `work.columns` follows
   each step without a restart.
   Drift sheet (WP-021) needs open drift, which a fresh logbook does not
   have: after `seldon init`, copy the fixture logbook over it (`rsync -a
   fixtures/logbook/ test:Seldon-smoke/`, no `--delete`, so init's
   `.seldon/templates` stay), set `created` in its `.seldon/logbook.toml`
   to now (so the start-up capture baselines instead of importing the
   host's history) and run `seldon status --json`; the panel then shows the
   sample's four items. `jax.seldon.panel resolve <event id>` (or
   `crisis`) opens the sheet without keys; `view` shows `drift` (action,
   case, members, hint, result). With keys: Enter twice on the action
   button, `wtype -- "<text>"` in a text field. Check the ledger lines
   (`resolution`, `refersTo`, `meta.txId` on a group), the explain case in
   `work/completed/`, and that `changelog.resolved`, `pill` and `crisis`
   follow. While the screen is locked the running shell keeps its old
   plugin code (`omarchy-restart-shell` refuses), so run the plugin's argv
   with the real engine over ssh and read the folded rows through `view`;
   the new sheet itself can then only be driven in a private offscreen
   instance (layer 3's harness with the real engine on `PATH`).
6. Screenshots: `grim -g "<x>,<y> <w>x<h>"` takes **logical** coordinates;
   the test host's output is scaled 1.25, so a region read off a full
   screenshot (physical pixels) must be divided by the scale. Over ssh also
   export `XDG_RUNTIME_DIR=/run/user/$(id -u)` and `WAYLAND_DISPLAY=wayland-1`.
   Shrink before committing: `magick in.png -strip -resize 80% -colors 64 out.png`.
7. Check the log of the running shell:
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
8. Clean up: remove `~/.local/bin/seldon` and `~/.local/state/seldon/` unless
   the next WP needs them; after step 5 also `~/Seldon-smoke` and the
   `~/.config/seldon/` that `seldon init` wrote, then restart the shell (the
   plugin shows engineMissing again). The guard hook allows writes to
   `~/.config/seldon` only in a plain `ssh <test host> …` command (no `;`,
   `&` or `|` before the path).

**Three themes (SPEC-PLUGIN §7)**, on the test host only — switching the
theme is a system change, so never on the dev host. Every plugin WP that
changes what the panel draws repeats it:

1. Note the current theme: `omarchy theme current`.
2. With the sample index in status `ok` (step 3 above), for each of a dark
   theme, a second dark theme and a light theme — e.g. *Osaka Jade*, *Tokyo
   Night*, *Catppuccin Latte* (`omarchy theme list`):
   `omarchy theme set "<theme>"`, wait about 6 s (the shell restarts), open
   the panel and capture Today, Changelog, System and the Changelog filtered
   to pacman with the cursor on the "+2" row and Enter pressed (since
   WP-021 that opens the drift sheet); since WP-020 also Work (the columns
   and a card) and its new-case sheet; since WP-021 the drift sheet in
   Link (theme item), Explain (a crisis, from the red strip) and Dismiss
   (the group, with "Press Enter again: …" armed).
3. Look for: every colour follows the theme — panel border, tab and chip
   fills, the zone stripes (red = the theme's urgent colour, yellow = its
   accent), the red strip, banners, the snapshot row highlight; dim text
   stays legible on the light theme; nothing keeps a colour of the previous
   theme.
4. Restore the noted theme with `omarchy theme set "<noted>"` and check it
   with `omarchy theme current`.
5. Save the shrunk PNGs under `work/active/WP-NNN/screenshots/`, named
   `<theme>-<view>.png`.
