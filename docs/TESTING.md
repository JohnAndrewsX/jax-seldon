# TESTING.md — How Seldon is checked

One command gates every work package: `just check`, run from the repository
root. It must exit 0 before a handover (AGENTS.md §5).

## `just check`

| Step | Recipe | What it runs | CI |
|---|---|---|---|
| Format | `fmt-check` | `cargo fmt --check` on `engine/` | yes |
| Lint | `clippy` | `cargo clippy --all-targets -- -D warnings` | yes |
| Tests | `test` | `cargo test` (unit + CLI tests in `engine/tests/`) | yes |
| Watch feature | `check-watch` | `cargo clippy --all-targets --features watch -- -D warnings`, `cargo test --features watch` (see "The `watch` feature") | yes |
| Contract | `schema-validate` | `bash scripts/validate-fixtures.sh` (WP-002); skipped with a notice while the script does not exist | yes |
| Plugin manifest | `plugin-validate` | `omarchy plugin validate plugin/` | **no** (dev host) |
| QML lint | `qmllint` | `qmllint` on `plugin/*.qml`, `plugin/components/*.qml` and `plugin/components/overlay/*.qml` against `$OMARCHY_PATH/shell`, then the token check `tests/plugin/check-tokens.py` | **no** (dev host) |
| Plugin logic | `plugin-test` | `node tests/plugin/model.test.js`, `node tests/plugin/model.bench.js`, `bash tests/plugin/service-states.sh`, `bash tests/plugin/panel-view.sh`, `bash tests/plugin/overlay-view.sh` (see "Plugin") | **no** (dev host) |

Other recipes: `just check-rss` (the `seldon watch` memory bound on an
optimised build; not in `check`, not in CI, required before the handover
of a WP that touches `engine/src/index/` or `engine/src/commands/watch.rs`;
see "The `watch` feature"), `just build-release` (static musl binary,
`x86_64-unknown-linux-musl`), `just fixtures-refresh` (stub until the engine
builds an index), `just e2e` (engine ↔ plugin end to end, host only; see
"Integration").

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
| `engine/tests/init.rs` | `init::` layout (SPEC-LOGBOOK §2), JSON output, git first commit, `--no-commit`, German templates, Obsidian, path precedence, refusals (existing logbook, non-empty dir, no terminal), lock held → exit 4. `setup::` (WP-024): the first capture (stubbed sources, cursors set, a second capture writes nothing, `--no-capture`), `--since` backfill → open drift, `--baseline` → zero open drift with one `dismissed` "pre-Seldon baseline" line per member and the commit `seldon: first capture and pre-Seldon baseline`, flag errors before anything is written, `--harness claude-code` (settings in the first commit, `hook install` afterwards changes nothing), `--harness omarchy-agent` with a kit (copied, modes kept, merged with Claude Code's hooks) and without one, the theme hook (a recording `omarchy` stub: exactly one `hook install theme-set <script>` on opt-in, none without, a failure with its fix, an existing hook not reinstalled), the templates (written as rendered; frontmatter keys, headings, fences and table headers identical in `en` and `de` and equal to `tests/golden/init-skeleton.txt`, `SELDON_BLESS=1` rewrites it; German prose) |
| `engine/tests/plan.rs` | `plan::` new (template, canonical frontmatter, area on first use, ids never reused), start/verify/done/drop (folder moves per ADR-0012 §9, `started`/`closed`/`snapshotBefore`, `.seldon/active-case`, body byte-identical outside the Log), invalid transitions → exit 1 and nothing written, list/show against `case.schema.json`, the fixture logbook (a copy), git autocommit with `--no-commit` and `git.autocommit = false` |
| `engine/tests/log.rs` | `log::` notes with and without a case (`case.events`, `agents`), the Log section append-only over three steps, the journal appended not rewritten, free text as one argument (spaces, quotes, `$(…)`, `--json` after `--`), month and day by timestamp, redaction, exit 3/4 |
| `engine/tests/journal.rs` | `journal::` appends to a fixture day (only the `cases:` line changes), CRLF days, the `plan done` stub in the logbook language |
| `engine/tests/commands.rs` | `event::` (fixture line shape, typed meta, engine-only kinds refused), `decide::` (ADR numbering, the logbook's own template, the editor gets the path as one argument), `open::` (paths, `--editor` without a terminal) |
| `engine/tests/agent.rs` | `seldon agent start` (WP-022): a recording stub `omarchy` gets exactly one argv, the prompt one element (a title with quotes, `$(…)` and backticks stays text), cwd and `SELDON_LOGBOOK` the logbook, the case becomes the active case, under 1 s; `[agent] launcher` and `[agent.launchers]` from config; a shell launcher refused before anything changes; a queued case → exit 1 with the `seldon plan start` hint; verification, unknown and malformed ids; a missing launcher and one that exits 1 at once (its stderr is the message) → exit 1 with the previous active case restored; a launcher that keeps running is detached (own process group, alive); exit 3 without a logbook. Unit tests in `commands/agent.rs` check the launcher rules |
| `engine/tests/rebuild.rs` | `seldon rebuild` (WP-032) on a copy of `fixtures/logbook/`: `outputs/REBUILD.md` equals `tests/golden/REBUILD.md` (`SELDON_BLESS=1 cargo test --test rebuild` rewrites it; the golden test first runs `seldon dossier --section packages` with the query shims, so the document has the "Before the logbook" group and the test checks that it lists every `pre-logbook` package of class `user` of `packages.explicit` under the command of its origin, WP-035, and one line counting the six `omarchy-base` ones, WP-036), the seven English headings in order, German prose, `--json` `sections` counts, and every package line's last code span is the id of an explicit `install` event of that package; a second run at a later clock writes nothing (`files: []`, same bytes, no commit); text above and below the `rebuild` fence survives a change; the autocommit `seldon: rebuild` happens once per change (git repository made in the test); `drift dismiss`/`explain` move items to "Deliberately not reproduced" and out of the open questions; appended ledger lines prove `pacman -U` → `omarchy pkg aur add`, no command → "repository unknown", a later `remove` drops the package (English logbook); an empty logbook says "none"; exit 3 without a logbook. Unit tests in `rebuild/mod.rs` check the repo/AUR rule and the fence merge |
| `engine/tests/dossier.rs` | `seldon dossier` (WP-035) on a copy of `fixtures/logbook/` with every host query shimmed (`Env::query_shims`: the package manager, `systemctl`, `omarchy` print `fixtures/logs/pacman-Q*.txt`, `systemctl-*.txt`, `plugin-list-after.json` for exactly the query argument lists, exit 64 for anything else, and log each call; `SELDON_HARDWARE_ROOT=fixtures/logs/hardware`; `common::Env` sets `SELDON_OMARCHY_PACKAGES=fixtures/logs/omarchy-packages`, copies of Omarchy's two package lists, for every test, WP-036): the system files equal `tests/golden/dossier.md` (`SELDON_BLESS=1` rewrites it), all eight fences present, the text outside the fences byte-identical, only read-only queries ran (each once), `seldon: dossier` committed once; a second run a day later writes nothing ("Nothing changed"); emptied fences are all filled (the ledger's four installs marked `since`, eleven `pre-logbook`, seven of class `omarchy-base` and the rest `user`, cased config rows, hardware from files); without Omarchy's lists every package is `user`, with exactly one warning, and a second run changes nothing; a later cased `config-change` fills only the empty case cell of an existing deviations row (a row with a case keeps it), and a second run changes nothing; a missing program skips its fences with a warning and keeps them; `--section` writes only its fences (comma list and repeats, unknown value exit 1); a cased `config-change` adds a deviations row and keeps the old rows byte for byte; an agent's `systemctl --user enable` with a case fills the unit's case; `capture` and `status` never touch the dossier; exit 3 and 4. Unit tests in `dossier/` cover history rows, the explicit-line format (with and without a class), reading Omarchy's lists (comments, missing files), unit cases, the deviations rows and the case fill (reason with `|`, row date newer than the event, other column orders), appending a missing fence under a heading in the logbook language, and `/proc` parsing |
| `engine/tests/doctor.rs` | `doctor::` green after init with snapper degraded, exit 3 when not initialised, invalid frontmatter, misplaced case, the fixture logbook (and that doctor leaves it untouched) |

| `engine/tests/watch.rs` | `seldon watch` (WP-034). Without the feature: exit 1, "built without the watch feature", JSON error. With `--features watch` (`just check-watch`): one rebuild at start (`trigger: "start"`; an edit made before the start is in it), then one change → exactly one rebuild after the 2 s quiet interval and nothing after it (the rebuild's own reads and its `index.json` write stay silent); a burst of 24 writes plus a new folder → one rebuild, and a later write in that folder is seen; generated `ledger/*.md`, `STATUS.md`, temp/backup files, `PROJECT.md`, reads of every watched file, and `seldon index`/`status` runs → none, while `.seldon/logbook.toml` counts; a held lock → no rebuild and still running, the rebuild within 2 s of the release; a folder renamed away and recreated → its watch moves to the new folder (a write in the old one is quiet, one in the new one counts); SIGTERM and SIGINT → exit 0 with a final `stopped` line; not initialised → exit 3; `--interval 1` → exit 1; RSS on the ×10 fixture (below) |

**Isolation.** The integration tests never see the real home, config,
state or logbook (AGENTS.md §6). `engine/tests/common/mod.rs` gives each
test a temporary `HOME` (so `~/.config/seldon` and `~/.local/state/seldon`
live under it), and a `PATH` that contains only:
- stub `omarchy-version` and `snapper` scripts (the snapper stub prints
  `No permissions.`, a snapshot list, or is absent, per test);
- a link to the host's `git`.

So the results do not depend on what the host has installed or how snapper
is configured. Every run also gets `SELDON_TEST_GUARD=<temp dir>`: the
engine then refuses to start (exit 2, "refusing to run outside the test
guard") unless its home, config and state directories, resolved with
symbolic links and `..`, all lie under that directory
(`config::Dirs::from_vars`, unit test
`config::tests::the_test_guard_checks_the_resolved_dirs`, integration test
`init.rs::setup::the_test_guard_refuses_a_home_outside_it`). Two engine variables make tests deterministic:
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
goes to `omarchy-launch-editor`, which a test stubs to record its argv.
No agent is ever started either: `agent start` tests stub `omarchy` (the
default launcher) or a configured launcher with a script that records its
argv NUL-separated. A manual `agent start` demo on the dev host follows
the same rule: its `PATH` holds only stub launchers (a temp dir, no
`/usr/bin`, no `/usr/local/bin`; reach other tools by absolute path), and
`HOME` is a temp dir. With `/usr/bin` on `PATH` a missing stub falls
through to the host's real `omarchy agent prompt`, which opens the
operator's default agent (WP-022 handover: it happened once, harmlessly,
because the dev host has no default agent). Tests that need git skip themselves when the host has none.
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

**Manual runs: scratch dirs and the test guard.** Every manual run of the
engine on a dev or test host starts with one scratch directory that holds
home, config, state and data, and `SELDON_TEST_GUARD` set to it:

```
S=$(mktemp -d)
export SELDON_TEST_GUARD=$S HOME=$S/home \
       XDG_CONFIG_HOME=$S/config XDG_STATE_HOME=$S/state XDG_DATA_HOME=$S/data
mkdir -p $HOME
B=$PWD/engine/target/debug/seldon
$B --json doctor     # the "config" check names $S/config/seldon/config.toml
```

Set all four, not only `HOME`: a desktop session usually exports
`XDG_CONFIG_HOME`/`XDG_STATE_HOME`/`XDG_DATA_HOME` pointing into the real
home, and an absolute XDG variable wins over `HOME`. With the guard set, a
run whose directories still point outside `$S` exits 2 before it reads or
writes anything. The real package log and `snapper` are still read (they
are absolute paths); the theme file and the watched config paths are read
under `$S/home` (point `SELDON_THEME_FILE` at the real `theme.name` to read
that one).

**Manual acceptance (WP-003).** With the scratch environment above:

```
$B init --non-interactive --no-capture --path $S/logbook
$B doctor --path $S/logbook --json   # "ok": true, snapper "degraded"
```

`seldon init` refuses an existing logbook, so remove `$S/logbook`
before running it again.

**Manual run of `seldon rebuild` (WP-032).** With the scratch environment
above, on a copy of the fixture (never on `fixtures/logbook/` itself):

```
cp -r fixtures/logbook $S/lb
SELDON_NOW=2026-10-01T17:05:12+02:00 $B --logbook $S/lb --json rebuild
#   {"files":["outputs/REBUILD.md"], "sections":{"packages":4,"deviations":5,
#    "plugins":3,"units":2,"open":4}, …}
$B --logbook $S/lb --json rebuild        # "files": [] — nothing changed
diff $S/lb/outputs/REBUILD.md engine/tests/golden/REBUILD.md
```

The document has no clock in it ("as of" is the newest ledger event), so
the second run writes nothing whatever the time. A plain fixture copy has
an empty `packages.explicit` fence (WP-036), which counts as none, so this
`diff` shows exactly the golden's "Before the logbook" group (the golden
test runs the dossier first).

**Manual run of `seldon dossier` (WP-035).** The dossier only *reads* the
host: `pacman -Qqe`, `-Qqm`, `-Q`, `systemctl --system|--user
list-unit-files --state=enabled`, `omarchy plugin list --json`,
`omarchy-version`, the theme file and `/proc`, `/sys` files. With the
scratch environment above, on a copy of the fixture:

```
cp -r fixtures/logbook $S/lb
export SELDON_THEME_FILE=/home/<you>/.local/state/omarchy/current/theme.name  # optional, read-only
$B --logbook $S/lb --no-commit --json dossier
#   {"files":[…], "sections":{"packages.explicit":"written",…},
#    "counts":{"explicit":…,"preLogbook":…,"omarchyBase":…,"total":…,"aur":…,"units":…,"plugins":…}, "warnings":[]}
$B --logbook $S/lb dossier              # "Nothing changed (8 fence(s) checked)"
$B --logbook $S/lb --no-commit rebuild  # §2 now has "### Before the logbook"
```

Programs and files can be pointed elsewhere: `SELDON_PACMAN`,
`SELDON_SYSTEMCTL`, `SELDON_OMARCHY`, `SELDON_OMARCHY_VERSION`,
`SELDON_THEME_FILE`, `SELDON_HARDWARE_ROOT` (default `/`),
`SELDON_OMARCHY_PACKAGES` (default `$OMARCHY_PATH/install`, else
`/usr/share/omarchy/install`; WP-036). Dev host,
2026-10-01: 169 explicit packages (167 from before the logbook), 966 in
total, 0 foreign, 40 enabled units, 38 plugins, no warnings; nothing
under the real `~/.config` or `~/.local/state` was written. WP-036, same
host with the real Omarchy 4.0.4-1 lists: 157 of the 169 are
`omarchy-base` (156 of them from before the logbook), 12 `user` (11);
REBUILD.md's "Before the logbook" block shrinks to 11 packages in 3
lines (at most 71 columns), plus "156 more come with Omarchy 4.0.4-1".

**Tests that need a logbook without a capture.** Since WP-024, `init` runs
the first capture, which records the first state of every diff collector
(config, theme, plugins) and the pacman cursor. A test that builds the
machine state *after* `init` and expects its own first capture to be the
baseline passes `--no-capture`; `Env::init_logbook*` does so for every
test. Only `tests/init.rs` exercises the wizard's capture.

**Real-host run of the wizard (WP-024).** With the scratch environment
above. It reads the real package log and `snapper` (read-only) and writes
only under `$S`. Never pass `--theme-hook` on the dev host: it runs
`omarchy hook install`, a red-zone write under `~/.config/omarchy/hooks/`
(the repository's guard blocks it; the tests stub `omarchy`).

```
$B init --non-interactive --path $S/logbook --language de \
   --harness claude-code --harness omarchy-agent --since "$(date -d '-7 days' +%F)"
#   First capture: N event(s) since …; M open drift item(s), M crisis  (dev host
#   2026-10-01: 1230 events, 22 items, all crises — WP-013 FINDINGS §2.2)
#   Harness omarchy-agent: no kit at $S/data/seldon/harness/omarchy-agent; nothing copied …
rm -rf $S/logbook $S/state $S/config
$B --json init --non-interactive --path $S/logbook --since "$(date -d '-7 days' +%F)" --baseline
#   capture.baseline {"items": 22, "events": 1230, "reason": "pre-Seldon baseline"}, openDrift 0
$B --json drift          # "openDrift": 0, "crisis": 0
$B --json capture --all  # "written": 0
git -C $S/logbook log --format=%s   # first capture and pre-Seldon baseline / init logbook
jq .logbook.git $S/state/seldon/index.json   # head = git rev-parse --short HEAD, dirty false
```

The interactive wizard needs a terminal; `script` provides one. Keys:
Enter takes the default, Space toggles a multi-select item, `y`/`n` answer a
confirmation. Export the scratch environment and `SELDON_TEST_GUARD`
*before* `script` (as above; `script` passes the environment on), pass
`--path` so the path step is skipped (its default is `~/Seldon`), and stub
`omarchy` with `SELDON_OMARCHY` in case the theme hook is answered with yes:

```
export SELDON_OMARCHY=$S/omarchy-stub    # a script that only records "$*"
(sleep 1; for k in '\r' '\r' '\r' '\r' '\r' ' ' '\r' '\r' '\r'; do printf "$k"; sleep 0.4; done
 printf "$(date -d '-3 days' +%F)\r"; sleep 4; printf '\r'; sleep 3) \
  | script -qec "$B init --path $S/logbook" /dev/null
# language, Obsidian, collectors, watched paths, more paths, harnesses (Space:
# claude-code), theme hook (no), git (yes), backfill date, then after the
# capture: "The backfill opened N drift item(s) … Mark them as the pre-Seldon
# baseline?" (Enter: yes)
```

Why the guard: on 2026-10-01 a wizard run with only `HOME` overridden
(`env HOME=<scratch> script -qec "seldon init …"`) wrote the real
`~/.config/seldon/config.toml` and `~/.local/state/seldon/`. The session
exported `XDG_CONFIG_HOME` and `XDG_STATE_HOME` into the real home, and
those win over `HOME` (WP-024 handover). With `SELDON_TEST_GUARD` set, the
same command exits 2 and writes nothing.

The engine needs Rust ≥ 1.89 (`File::try_lock`, let-chains); both hosts
have 1.98.

### The `watch` feature

`seldon watch` is compiled only with `--features watch` (ADR-0005: optional,
off by default); `just build-release` leaves it out. Its unit tests
(`commands::watch::tests`: which paths and event kinds count) and
`engine/tests/watch.rs` run with the feature:

```
cargo test --manifest-path engine/Cargo.toml --features watch --test watch
cargo test --manifest-path engine/Cargo.toml --features watch watch::
```

The watcher tests wait for real inotify events and the 2 s debounce, so the
file takes ~12 s; the timing assertions allow 4 s of slack for a loaded
machine. `Watch::start` consumes the `watching` line and the rebuild at
start, so each test sees only the rebuilds its own writes cause.

**Memory bound (PLAN.md: RSS < 10 MB).** The test runs the watcher on the
×10 fixture (`tests/common/scale.rs`) with the state lock held (so the
rebuild at start waits), reads the idle size, releases the lock, lets the
rebuild at start and one change-triggered rebuild run (500 events in the
index) and reads `VmRSS` and `VmHWM` (peak) from
`/proc/<pid>/status`. The bound is about the shipped, optimised binary; a
debug binary carries ~6 MB more code, so under the test profile only the
growth over the idle watcher is bounded (< 6 MB). `just check-rss` runs
the test under `--profile bench`, where the peak must stay under 10 MB.
It is not part of `just check` and CI does not run it; run it before the
handover of any WP that touches `engine/src/index/` or
`engine/src/commands/watch.rs`. To measure another binary, e.g. the musl release build with the
feature:

```
cargo build --manifest-path engine/Cargo.toml --release --features watch --target x86_64-unknown-linux-musl
SELDON_WATCH_BIN=$PWD/engine/target/x86_64-unknown-linux-musl/release/seldon \
  cargo test --manifest-path engine/Cargo.toml --features watch --test watch rss -- --nocapture
```

Dev host, 2026-10-01 (idle → after the rebuild at start and one change,
peak): debug 13.0 → 17.4 MB; bench profile 6.0 → 9.1 MB; musl release
4.8 → 7.5 MB, peak 7.9 MB.

The systemd user unit (`engine/systemd/seldon-watch.service`) is never
enabled or started by a test or an agent (AGENTS.md §6); `systemd-analyze
--user verify` checks its syntax.

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
more", ADR-0020). For Decisions and Memory (WP-023): `decisionRows` lists
the sample's four decisions newest first (by id; ADR-0004 *proposed* with
the accent tone), puts a malformed id last and never actionable, and
survives broken entries; `decisionSummary` ("4 decisions · 1 proposed");
`decideArgs` builds `decide --no-edit --json -- <title>` with the title
one argument after `--` (`--help`, quotes, `-rf --case …`, `$(reboot)`,
`--`) and refuses a blank, two-line or non-text title; `decideResult`
reads the `decide --json` shape and passes on only an id matching
`^ADR-[0-9]{4}$`; `openArgs` and `validateArgs` take `logbook` and a
decision id (not `ADR-4`, `adr-0004`, a padded id, a path or `memory`);
`memoryRows` gives the sample's three lessons and two topics (path and
`updated`), every part optional, all opening the fixed target `logbook`. For the Prime Radiant (WP-030): the period ids, keys `1`–`4` and
←/→ wrapping, the summon payload (`{"period":"30"}`, anything else keeps
the period); `periodWindow` (inclusive days ending on the index's today,
across a leap day, *All* unbounded); `isoWeekMonday` (week 53 only in long
years); `seriesInPeriod` (heatmap and packages by date, a drift week that
touches the window, case spans that overlap it, open cases, broken rows
left out); `periodTable` on the sample (rows per slot for 30/90/365/All:
`30,2,5,3,17`, `90,3,5,3,18`, `365,3,5,3,18`, `366,3,5,3,18`, with the
count and detail lines) and without an index; `overlayMeta`;
`overlayBanner` keeps only *Copy*; `overlayGrid` in its three modes (exact
gaps, rows by weight, minimum heights that make the grid scroll).
For the charts (WP-031): six slots (The Plan last, `…,plan=2`); the grid
gives rows at their minimum height first and shares the rest by weight,
filling the height exactly (`rowHeights`); `dayNumber`/`dateOfDay` against
`Date` for every day of 1899–2101 and impossible dates (2026-02-30)
rejected; ISO weeks across year ends; colour steps (square root of
count/max, 0 for none); `splitSeries` gives exactly `seriesInPeriod`'s rows
for every period, on the sample and on edge rows; per chart on the sample:
heatmap cells, offset, months, hover text with sources and the hit test;
series lanes, padded flat lanes and the sample that holds at a day; drift
weeks with gaps filled (also across week 53); risk shares and the part at
an angle; the drift peak is the week with the most opened (a
non-monotonic series, ties to the later week); timeline markers, spans clipped to the window, lanes, months
and the hit test; the plan's cards and columns; empty charts without an
index; `aggregationCount` counts exactly the table's passes.

`node tests/plugin/model.bench.js` times `periodTable` (what the service
does on every index write) on the sample, the sample ×10 and 7000 timeline
rows, in a plain function scope and in a vm sandbox (as the tests load
Model.js; slow global lookups, about 8× slower and load-sensitive, so only
reported), against the WP-030 cut. It fails when the fastest of 31 plain
runs on ×10 takes more than 10 ms (idle about 1.9 ms, about 4 ms with the
host fully loaded).

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

Decisions (WP-023) use `["decide", title]`; the fake engine's `decide`
adds the next ADR (status proposed, newest first) to the state index and
answers in the SPEC-ENGINE §3 shape, its `open ADR-NNNN` opens the path
the index lists for it and `open logbook` the logbook folder;
`FAKE_SELDON_LOCKED` covers `decide` too. Checked: the exact argv of two
`decide --no-edit --json -- <title>` calls (`--help`, a title with
quotes), each followed by `open ADR-0005|ADR-0006 --editor --json` with
the id from the answer, then `open ADR-0004` and `open logbook`; the new
decisions in the fake's index; the editor launcher's paths; eight
refusals that never reach the engine (blank, two-line and non-text
titles, `ADR-4`, `ADR-0004; reboot`, a path, `memory`); a held lock (the
decide result, nothing opened) and an unknown decision (the open result
and the panel's error line); dev mode.

*Start agent* (WP-022) uses `["agent", caseId]` (`Service.startAgent`).
The fake engine's `agent start` checks the case like the engine (active
only; queued gets the `seldon plan start` hint), launches nothing, writes
the id to `$HOME/active-case` and answers with the engine's JSON for the
default launcher; `FAKE_SELDON_NO_LAUNCHER` makes it answer as the engine
does without `omarchy`. Checked: a malformed id never reaches the engine;
the exact argv `agent start <id> --json`; a second call while one runs is
refused (one at a time, shared with `plan`); the answer is `planResult`
with `action: "agent"` and "Agent started on C-2026-003 · launcher default
(omarchy)"; the queued refusal and the missing launcher are `planResult`,
`lastError` stays empty; dev mode refuses.

Isolation: the scenarios run with a `PATH` made of symlinks to the few
tools the fakes need, so a `seldon` installed system-wide never leaks in.
Every run, here and in layer 3, gets its own `HOME`, `XDG_STATE_HOME` and
`XDG_CONFIG_HOME` inside the temp dir (a case may name its own `HOME`; the
XDG dirs follow it). Both scripts end with a check
(`tests/plugin/real-home-guard.sh`) that the real `~/.local/state/seldon`
and `~/.config/seldon` neither appeared nor changed during the run; it
compares existence, size, mtime and ctime of every entry, so an engine run
by hand at the same time also fails it. One exception (WP-039): on a host
where the operator's real Seldon is live (the installed plugin runs
`seldon capture` every 15 minutes), a capture during the run rewrites the
state dir. The guard passes that as "changed by the operator's live engine
(not a leak)" only if nothing under `~/.config/seldon` changed
(`config.toml` byte-identical), no path appeared or disappeared, every
changed entry is the state dir or its `index.json`, `lock`,
`cursors.json` or `manifest.json`, and the post-run `index.json` names the
logbook `config.toml` names (`logbook.path`) and the machine it named
before the run (`logbook.machine`). It reads only `config.toml` and those
two index fields, never the logbook. Anything else still fails, with the
reason. `tests/plugin/real-home-guard.test.sh` (part of `just plugin-test`)
proves both sides in scratch HOMEs.

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
(Tab/Shift-Tab hand over to the bar, ←/→ and h/l switch tabs and wrap over all six, digits fixed per tab id 1–6, ↑/↓, Enter, Esc), the snapper banner on every
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

*Start agent* (WP-022): on the sample (dev mode) the active case's card
lists *Verify, Start agent, Drop, Open* and "agent: claude-code", and
neither `a` nor a click arms it ("Dev mode is read-only"). Live: `a` on a
queued case does nothing; on C-2026-003 the first `a` arms (the hint
"Start agent on C-2026-003? Press a again or click Confirm start agent.",
the button "Confirm start agent"), Enter re-arms Verify instead, `a` twice
runs and the result line names the launcher; with the mouse on C-2026-004
a click arms and the second click runs; the fake engine's logbook has
C-2026-004 queued (`$HOME/cases`), so its refusal with the `seldon plan
start` hint is the result line and the banner stays empty; the exact argv
of both calls.

The drift sheet (WP-021) has eight scenarios. On the sample (dev mode):
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
shows "… and 1 more", asks `seldon drift show` (always for the group's leader) and
lists all three, also when opened from a member row.

Decisions and Memory (WP-023) have three scenarios. On the sample (dev
mode): `4` shows ADR-0004 (proposed) to ADR-0001 with id, status, title,
date and file, ↑/↓ and a click on a title move the cursor, Enter and `e`
are refused with dev mode's reason, `d` opens no sheet; `6` shows LESSONS
(3) and TOPICS (2, with path and `updated`). Live, with real keys: `d`,
the title `--help "q"` (no tab switch), Enter arms ("Press Enter again:
create the decision “…”"), Backspace disarms, Enter twice sends `decide
--no-edit --json -- '--help "q"'` and then `open ADR-0005`; the sheet
closes, the keys come back and the cursor sits on ADR-0005 once the index
lists it; Enter, *Open* and `e` open ADR-0003/ADR-0004; on Memory Enter
and *Open* open the logbook folder; the exact argv and editor paths.
Refused: Enter on a blank title is refused in the plugin; the engine's
refusal (lock held) shows in the sheet and keeps the title; Esc gives the
keys back, the tab shows the refusal, and `d` brings the title back.

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
`PANEL_SHOTS=<dir> bash tests/plugin/panel-view.sh` does this for the
Today tab in Osaka Jade, Tokyo Night and Catppuccin Latte
(`<dir>/panel-<theme>-today.png`): a live run against the fake engine, so
the render has no dev-mode note (which would print the index path) and the
QuickEntry looks as a user sees it.

Label fit (WP-039): every report also carries `overflow`, the visible
texts that do not fit (`elided:` a Text elided or cut at its line limit,
`wide:` content wider than its box, `button:` a qs.Ui Button narrower than
its label and padding, `outside:` text past the panel's right edge), and
`contentWidth`, the panel's width. The `fit-*` cases put a
`~/.config/omarchy/shell.toml` with `[font] base-size` 12 and 15 (font
scale 1.0 and 1.25; `Style.space` follows the font) into the harness HOME,
walk every tab and require: width 460 and 575, the tab strip on one line
with the same cell widths whatever tab is selected, no `button:`, `wide:`
or `outside:` entry anywhere, the Changelog header never elided, and no
elision at all on Today, Decisions, System and Memory (only Changelog row
text and Work mini-card titles, user content, may elide). `fit-narrow`
sets `HARNESS_CARD_WIDTH=300` (the stand-in KeyboardPanel's
`availableCardWidth`, a screen narrower than the panel): the strip wraps
and still nothing that is a label is cut. `PANEL_FIT_SHOTS=<dir>` runs
the two scales in Tokyo Night, Osaka Jade and Catppuccin Latte and saves
every tab (`<dir>/fit-100-<theme>-<tab>.png`, `fit-125-…`).

### 3b. `Overlay.qml` in a private headless Quickshell

`bash tests/plugin/overlay-view.sh` runs the Prime Radiant the same way:
copies of the shell's `Commons/` and `Ui/`, `tests/plugin/harness/overlay.qml`
as `shell.qml`, and a copy of `plugin/` whose layer-shell window
(`components/overlay/OverlayWindow.qml`) is replaced by
`tests/plugin/harness/OverlayWindow.qml`, an Item that fills the harness
window. The harness creates Overlay.qml the way the shell's overlay
Loader does: without properties, then it assigns `shell`, `manifest` and
`service`, so the overlay's bindings first run with `service === null`
and must do no work then (SPEC-PLUGIN §6; with `service` as a creation
property the harness missed the 23 aggregation passes the live shell
showed, WP-013 FINDINGS §5.1). `fresh` reports that as `firstFrame.bare`,
and the script asserts it next to `firstFrame.overlay == 0`. The harness
hands the overlay a stand-in shell facade whose
`hide()` records the id and calls `close()`, as the shell's does, and
after each step prints `Overlay.view()`, the hidden ids, every visible
text, and every text that leaves its slot or the window.

Steps (header of `harness/overlay.qml`): `toggle[:<json>]` (what `shell
toggle` does: hide when open, else `open(json)`), `fresh[:<json>]` (what
the shell's Loader does on summon: a new Overlay.qml, then `open(json)`;
the report's `firstFrame` holds the aggregation counts and paints sampled
on its first swapped frame and the frame by which every chart painted),
`summon[:<json>]`, `hide`, `key:<Left|Right|Escape|…>`, `text:<c>`,
`click:<text>`, `clickAt:<x>,<y>`, `hover:<slot>:<fx>,<fy>` and
`hoverItem:<slot>:<i>` (a real mouse move onto a point of a chart, or onto
its item i as `chart.locate(i)` places it), `leave`, `resize:<W>x<H>`,
`call:<method>:<arg>` (what `shell call jax.seldon` does), `shot:<name>`,
`view`.

Checks: closed until toggled; toggle opens on 90 d with the header (title,
machine, Omarchy version, index time, the period's dates) and the six
slots with the sample's counts and each chart's summary (its caption);
`1`–`4`, ←/→ and `h`/`l` pick periods (wrapping) and the counts and
summaries follow (`30,2,5,3,17,2` for 30 d, `366,…` for All); Esc closes through `shell.hide("jax.seldon")`; toggle closes; a click
on the scrim closes; `summon` with `{"period":"365"}` opens on 365 d; a
click on *30 d*, `call setPeriod all` (an unknown id changes nothing) and
`call view` work; *Close* closes. Layout at 1920×1080, 2560×1440 and, for
a 1.25 output scale, 1536×864 and 2048×1152 (each with every period) and
once with `QT_SCALE_FACTOR=1.25`: six slots with a size, all inside the
window, every chart with a plot of its own, no text outside its slot or
the window, no scrolling, three columns. 760×1000 reflows to two columns,
560×700 to one and scrolls. Charts (WP-031): after a `fresh` open the
first frame has run no aggregation (the service's count is the one from
before, the overlay's own 0) and painted nothing (Canvas gets its context
then); by frame 2 every chart has painted exactly once. A period switch
aggregates nothing and repaints only the charts whose data changed
(RiskDonut and The Plan have no period); hovering repaints nothing; a
resize (one dimension, the harness sets width and height separately)
repaints each chart once, also through the medium and narrow modes. Hover
read-outs from real mouse moves onto items of every chart and from `call
hover` (exact texts, e.g. the heatmap's 2026-10-01 with its counts by
source); a malformed `call hover` (no such slot, `.`, `1.2.3`, a point
outside [0, 1], one number) returns `{ error }` and leaves the hover as it
was. The aggregation count covers every chart file's own Model.js
instance: a `Model.heatmapChart(…)` call slipped into
`Heatmap.onPaintRequested` fails `fresh #2 .view.aggregations.overlay`. The not-initialised variant shows the banner with *Copy* only
and the hint, every chart in its empty state ("no data in this period",
"no cases yet · all time", "no active cases") and nothing painted; every
other index variant renders every chart. Every run's log is free of
warnings and errors.

`OVERLAY_SHOTS=<dir> bash tests/plugin/overlay-view.sh` also renders the
overlay at 1920×1080 and 2560×1440 in Osaka Jade, Tokyo Night and
Catppuccin Latte into `<dir>`, each once on 90 d and once on 365 d with
the pointer on the heatmap's last day (offscreen renders with each
theme's `colors.toml`, not live screenshots).

**`plugin/preview.png`** (the marketplace image, WP-041) is composed from
two of these renders, kept in `docs/images/` (monorepo only, so the plugin
repository stays small): `overlay-tokyo-night-1920x1080.png` (the
`OVERLAY_SHOTS` render on 90 d) and `panel-tokyo-night-today.png` (the
`PANEL_SHOTS` render). To refresh it after a visible change, render both,
copy them over the files in `docs/images/`, and run (ImageMagick 7; the
colours are Tokyo Night's `background` and `accent` from its
`colors.toml`):

```sh
magick docs/images/panel-tokyo-night-today.png -crop 460x536+0+0 +repage \
  -bordercolor '#1a1b26' -border 18 -bordercolor '#7aa2f7' -border 2 /tmp/panel-framed.png
magick -size 2480x1080 xc:'#1a1b26' \
  docs/images/overlay-tokyo-night-1920x1080.png -geometry +0+0 -composite \
  /tmp/panel-framed.png -geometry +1942+24 -composite \
  -strip -define png:compression-level=9 plugin/preview.png
```

The crop is the panel's 460-unit width (the tab strip ends flush with it
since WP-039) and the bottom of the journal's last entry at the default
font. The framed panel is 500×576, placed 22 px right of the overlay with
36 px to spare, so the canvas is 2480×1080. Re-check both if the panel's
layout changes. The image must stay under 1 MB (it is about 150 KB).

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
   omarchy-shell jax.seldon.panel tab changelog       # today | changelog | work | decisions | system | memory
   omarchy-shell jax.seldon.panel filter all          # or a source
   omarchy-shell jax.seldon.panel resolve crisis      # or an event id: the drift sheet
   omarchy-shell jax.seldon.panel view       # tab, cursor, rows, badges, banners, strip
   omarchy-shell shell toggle jax.seldon     # Prime Radiant
   omarchy-shell shell call jax.seldon view ""   # while it is open: period, slots, geometry, charts
   omarchy-shell shell call jax.seldon setPeriod 30
   omarchy-shell shell call jax.seldon hover "series 0.5,0.5"    # a chart's read-out at a point
   omarchy-shell shell call jax.seldon hover "heatmap 0.15,0.5"  # a heatmap cell: see below
   omarchy-shell shell hide jax.seldon
   ```
   Keys: `wtype -k Tab`, `wtype -M shift -k Tab -m shift`, `wtype -k Down`,
   `wtype -k Return`, `wtype f`, `wtype -k Escape`, each followed by
   `jax.seldon.panel view`. Tab opens the bar's next panel
   (`Bar.switchPanelFrom`); if that neighbour opens a window instead of a
   popup panel (OmaSettings on the test host), the Seldon panel stays open.
   Heatmap probe: the grid is square and bound by the slot's height, left
   aligned with the legend beside it, so a fixed fraction such as `0.9`
   lands on empty space except at 365 d/All on wide screens. Take the point
   from `Model.heatmapLayout(w, h, weeks, labelW, labelH)`: `w`, `h` are the
   heatmap slot's `chart.w`/`chart.h` in `view`, `weeks` the number of week
   columns (5–6 at 30 d, 13–14 at 90 d, 53–54 at 365 d), `labelW` =
   3 × `Style.font.caption`, `labelH` = `Style.font.caption` +
   `Style.spacing.sm` (30 and 14 at the default tokens); then `pitch` =
   ⌊min((w − labelW)/weeks, (h − labelH)/7)⌋ and the cell in column `c`
   (oldest week 0), row `r` (Monday 0) is at
   `fx = (labelW + (c + ½)·pitch)/w`, `fy = (labelH + (r + ½)·pitch)/h`.
   `fy = 0.5` is a middle row while the grid is height-bound. Example (test
   host, 1536×864 logical, 90 d, WP-037): `w` 1410, `h` 108 → `pitch` 13,
   14 columns; today (column 13) is at `0.15,0.5`. In the headless harness
   use `hoverItem:heatmap:<i>` instead (`chart.locate(i)`, `-1` = today).
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
   Decisions and Memory (WP-023): `jax.seldon.panel tab decisions` and
   `view` show `decisions.rows` (the logbook's ADRs, newest first);
   `wtype 4`, `wtype d`, `wtype -- "<title>"`, `wtype -k Return` twice
   creates a decision: `view` shows `decisions.result` "Created ADR-…",
   `~/Seldon-smoke/decisions/` has the file with `status: proposed`, and
   the editor opens it (`hyprctl clients`). Enter on a row opens that
   decision; on `tab memory`, Enter opens the logbook folder. Locked
   session: run the plugin's argv (`decide --no-edit --json -- <title>`,
   then `open <id> --editor --json`, `open logbook --editor --json`) over
   ssh and read the rows through `view`; the sheet itself only in the
   private offscreen instance.
   Start agent (WP-022) launches a real agent window, so only on an
   **unlocked** session (`omarchy-shell lock status`, ORCHESTRATION §11)
   and only with a default agent set (`omarchy default agent`): on an
   active case of the smoke logbook, `wtype 3`, move to the case, `wtype
   a` twice. Pass: `work.result` reads "Agent started on C-… · launcher
   default (omarchy)", a terminal window with app-id `org.omarchy.agent`
   appears (`hyprctl clients`), its agent starts in `~/Seldon-smoke` with
   the context block as its first prompt, and
   `~/Seldon-smoke/.seldon/active-case` names the case. Restore: close the
   agent window (end the agent session first, so its hooks finish) and
   remove `~/.local/state/seldon/agent-launch.log` with the state dir in
   step 8. Without a default agent the result line shows the launcher's
   own "Choose default agent with: omarchy default agent <name>".
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
   (the group, with "Press Enter again: …" armed); since WP-023 Decisions
   (the list, and the new-decision sheet armed) and Memory.
3. Look for: every colour follows the theme — panel border, tab and chip
   fills, the zone stripes (red = the theme's urgent colour, yellow = its
   accent), the red strip, banners, the snapshot row highlight; dim text
   stays legible on the light theme; nothing keeps a colour of the previous
   theme.
4. Restore the noted theme with `omarchy theme set "<noted>"` and check it
   with `omarchy theme current`.
5. Save the shrunk PNGs under `work/active/WP-NNN/screenshots/`, named
   `<theme>-<view>.png`.

## Integration

`tests/integration/e2e.sh` checks the whole chain on a real machine: the
engine writes a logbook and the index, and the plugin reads that index and
shows it (WP-013). It is not part of `just check`. Run it before every
phase exit.

```bash
just e2e --engine-only                    # dev host: engine steps in scratch dirs (about 50 s; the musl build dominates)
SELDON_TEST_HOST=<alias> just e2e         # test host over ssh, full chain (about 30 s after the build)
```

`SELDON_TEST_HOST` is the ssh alias of the test host (default `test`). The
real name is in the git-ignored `memory/local.md`; never commit it. Under
`SELDON_SKIP_HOST_CHECKS` the recipe prints a skip notice and exits 0. The
script exits 0 when every check passes and 1 otherwise. Either way it prints
a summary with one line per failed check.

The full run refuses to start when `SELDON_TEST_HOST` leads back to this
machine (`localhost`, or an alias for the dev host). It compares
`/etc/machine-id` on both sides (falling back to `hostname`) right after the
first ssh call. That way the steps that write `~/.local/bin`,
`~/.config/seldon` and restart the shell never run on the dev host.

**Steps.** Both variants first build the static engine
(`cargo build --release --target x86_64-unknown-linux-musl`) and run the
same engine steps:
1. `--version`, and `contract-version` = `plugin/manifest.json`
   `seldon.contractVersion`;
2. `init --non-interactive --path ~/Seldon-e2e --since <now − 7 days>`.
   `init` runs the first capture itself (WP-024), and `--since` backfills
   that capture. A fresh logbook records nothing older than its creation
   otherwise, and a later `capture --since` is ignored by collectors that
   already have a cursor. The first capture must record at least one
   package event. `SELDON_E2E_SINCE_DAYS` changes the window; it must be a
   whole number of days, and anything else stops the run before the build;
3. `capture --all` after `init` must write 0 events (idempotency);
4. `plan new`, then `plan start`;
5. `log --case <id> -- <note>`. The note contains quotes and `$(…)` and must
   arrive verbatim;
6. `status`, which must give `state.status` `ok`;
7. `index --check`, which must give `valid: true`;
8. `doctor`, which must give `ok: true`.

Then index.json must have the contract version, one active case, the note
in `today.entries`, and at least one package event.

- **`--engine-only`** (dev host):
  - It sets `HOME`, `XDG_CONFIG_HOME` and `XDG_STATE_HOME` to a temp dir.
    The real `~/.config` and `~/.local/state` are never touched; the
    script ends with `real-home-guard.sh` to prove it.
  - It loads `plugin/Service.qml` into a private headless Quickshell
    (`tests/plugin/harness/shell.qml`) in dev mode against the index the
    engine just wrote. Status must be `ok` and the pill must equal the
    index counts.
  - It never touches the running shell, and it never enables the plugin.
  - The theme collector reports degraded here, because it reads the
    theme from the scratch `HOME`.
- **Full run** (test host): after the engine steps on the test host, the
  script:
  1. rsyncs `plugin/` to the dev install (`--checksum`, so unchanged
     files are not rewritten);
  2. runs `omarchy plugin validate`, and enables the plugin if it was
     disabled;
  3. waits for the shell's hot reloads, then runs `omarchy-restart-shell`;
  4. waits until `jax.seldon.service status` has settled in `ok`, after
     the plugin's own start-up capture;
  5. compares:
     - `jax.seldon.panel pill` and the service pill with the pill
       computed from `summary` (SPEC-PLUGIN §4), and the tone;
     - Today's entry count in `jax.seldon.panel view` with
       `index.today.entries`;
     - the Changelog row count with the pacman filter and without it;
  6. opens the QuickEntry with `wtype n`, types a note that starts like
     an option, and presses Enter. The result line must say "Saved", and
     after the FileView refresh Today must show one more entry. The note
     must be in the index and in the ledger;
  7. reads the log of the shell instance (`quickshell list -a -j`, not
     `pgrep`, which also finds crash-report windows). The log must contain
     "Configuration Loaded" and no WARN/ERROR line that names `jax.seldon`;
  8. checks that no new crash report appeared under
     `~/.cache/quickshell/crashes`.

**Restore (test host).** Before it changes anything, the run:
- writes a fingerprint of the test host:
  - existence and content of `~/Seldon-e2e`, `~/.local/bin/seldon`,
    `~/.local/state/seldon` and `~/.config/seldon`;
  - the hash of the plugin dir and its enabled flag;
  - the theme;
  - the number of `quickshell` processes and of crash reports;
- copies the plugin dir to `~/.cache/seldon-e2e/`;
- writes `~/.cache/seldon-e2e/found.env`:
  - whether the plugin dir existed;
  - the enabled flag;
  - which of the four Seldon paths were absent;
- only then moves the Seldon paths that exist aside into
  `~/.cache/seldon-e2e/saved/`.

The enabled flag comes from `omarchy-shell shell listPlugins`, which
answers empty while the shell rescans. The script asks up to five times.
If it still gets no answer, the flag is recorded as unknown, and the
restore never changes it.

At the end, or on any failure through an EXIT trap, it restores the test
host in this order:
1. it removes the engine binary;
2. it restores the plugin dir;
3. it disables the plugin, but only when the flag was found `false` for
   certain and is `true` now;
4. it restarts the shell if the plugin code differs from what the shell
   runs, and otherwise sends `jax.seldon.service refresh`;
5. it waits until the service has no engine call in flight;
6. only then it removes the state, config and logbook dirs and moves the
   saved paths back.

A path is removed only when it was absent at the backup or was moved
aside. A path that the backup found but had not moved yet (a run killed
inside the loop) is the original, so the restore keeps it and says so.

It then compares the fingerprint, and checks that the service status is
the one it found (`engineMissing` on a host without an engine).

If a run was killed before its restore, `~/.cache/seldon-e2e/found.env` is
still there. The next run then restores that state first. After you kill a
run, wait about 10 s: its last ssh command may still be running on the test
host.

The lock check (below) comes before that recovery. So a run killed on a
host that is locked, or that locks later, leaves its leftovers on the test
host until the operator unlocks the session. These are `~/Seldon-e2e`,
`~/.local/bin/seldon`, the Seldon state and config dirs, the run's plugin
code and `~/.cache/seldon-e2e/`. The plugin may meanwhile keep capturing
into `~/Seldon-e2e`. The next run after the unlock restores them.

**Locked session.** The full run refuses to start while the test host's
session is locked (`omarchy-shell lock status`), because:
- `omarchy-restart-shell` refuses to restart a locked session (and
  re-locks a session that is locked);
- `wtype` would type into the lock screen.

The script checks the lock again before the restart and before every
keystroke. Unlock the test host, or keep it awake, before an unattended
run.

**Artifacts.** With `SELDON_E2E_OUT=<dir>`, the run saves these files into
`<dir>`:
- the index after the engine steps and after the plugin's capture;
- the service status and the pill;
- the panel views (Today, Changelog, Changelog with the pacman filter, and
  after the QuickEntry);
- on the dev host, the harness log.

They contain the machine id, which names the host: keep them out of the
repository. Mismatches go to the WP's `FINDINGS.md`, with the command and
an index excerpt.
