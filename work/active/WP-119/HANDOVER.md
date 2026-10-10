# WP-119 handover — Set up Seldon end to end (the setup card)

Branch `wp/119-setup-card` (from `next`), worktree `wt/WP-119`. Not pushed
(the orchestrator pushes). No contract change (see "Contract").

Operator decisions taken during the WP (asked through the orchestrator,
2026-10-10):

- `seldon init --defaults` sets up **no** agent harness on its own: the
  harnesses come from the config, as with `--non-interactive` (none on a
  fresh machine). The UX review had proposed `claude-code` when `~/.claude`
  exists and `skills` always; not built.
- `seldon init --non-interactive` stays **unchanged** (record from now on,
  no backfill, no detection); it is not an alias of `--defaults`.

## What was done

### Engine — `init` modes and the look-back (ADR-0033)

- `engine/src/commands/init.rs`: `InitMode` — plain `seldon init` asks only
  the location (ADR-0010's options), `--defaults` asks nothing,
  `--ask` is the full wizard, `--non-interactive` unchanged. `--json`
  reports `mode`.
- `--defaults` and plain `init`: Obsidian's settings when Obsidian is
  installed (`obsidian.desktop` or `md.obsidian.Obsidian.desktop` in the
  `applications/` folder of `$XDG_DATA_HOME` or a `$XDG_DATA_DIRS` entry;
  a stat, nothing read), and the **look-back**: the first capture records
  from local midnight 90 days before `ctx.now` (`SELDON_NOW` sets it), and
  the baseline dismisses every opened item "before Seldon" without a
  question. A `--since` is dismissed the same way; `--no-capture` records
  nothing. History row (ADR-0033 §4): "Looked back 90 days: N changes
  recorded as history before Seldon". `capture.lookbackDays` in `--json`.
- `--ask`: the Obsidian question defaults to the detection, the backfill
  question to the look-back's date (`none` records from now on), the
  baseline question reads `Dismiss them as "before Seldon"?`.
- A terminal is needed only when a question comes: `--ask`, or plain
  `init` without `--path`/`--logbook`. The refusal names `--defaults` and
  `--non-interactive`. `--defaults`, `--ask`, `--non-interactive` exclude
  each other (clap).
- `setup::BASELINE_REASON` is now `before Seldon` (ADR-0033 §2), also for
  `--since --baseline`; the History suffix reads `N drift item(s)
  dismissed as "before Seldon"`. The commit message `seldon: first capture
  and pre-Seldon baseline` is unchanged.
- `collectors/snapper.rs`: `NOT_INSTALLED` names the message the plugin
  reads as "no snapper here" (pinned literally in `tests/collectors.rs`).

### Plugin — one setup card, then the first-run card

- `components/desk/SetupCard.qml` (new; no mode logic) renders
  `Model.setupCard()`: "Set up Seldon · N of T steps to go", " (optional)"
  when only the snapshot step is left; engine → logbook → snapshots; done
  steps ticked, the current one with its buttons, its command and its line.
  T = 2 once the index says the machine has no snapper (collector off, or
  the engine's `snapper is not installed`).
- The banners stay the model behind it: a step's *Install* / *Create
  logbook* / *Grant* / *Copy* go to `Service.fix` with the banner's id; a
  step is `ready` only while its banner is up. An engine older than
  `engineMin` before init: the logbook step waits and says "First: Engine
  too old (the notice above)." (an old engine would refuse `--defaults`).
- `Service.setupWatch`: after a setup terminal (card or notice) the
  service looks again by itself every 5 s for at most 10 min — the engine
  probe (step 1), the index (step 2), a capture every 30 s (step 3,
  `SELDON_SETUP_CAPTURE_MS` for the harness). A desk opened while the
  engine is missing probes it once. No *Check again* on the card.
- *Not now*: `setupSnapshots: "not-now"` written once into the plugin's
  own `shell.json` entry through `Desk.writeSetting` (a state key, not in
  `barWidget.schema`, like ADR-0045's `deskIntro`); held at once by
  `Service.setupLaterOverride` until the shell sends the entry back;
  without a bar entry or on a refusal it holds for the shell's life and
  Settings › Capture says so (the card is gone by then; the key is not in
  Omarchy's bar settings, so the desk's usual "change it there" sentence
  does not apply). Settings › Capture names the snapshot state
  and, only while the step is put off and still open, **Offer again**
  (takes the key out; `Model.deskSettingsWrite` learned `undefined`).
- `Notices.qml` leaves out the setup banners while the card stands for
  them (`Model.isSetupNotice`), the snapshot notice also after Not now.
  The header chip shows the card's headline (+N notices) and leads to
  Today's overview (`Desk.showSetup`) instead of folding.
- Step 2 carries WP-138's preview line ("The last 7 days: … This is
  without memory: …"); the *Before Seldon* card and its *Set up Seldon*
  button are gone (`PREVIEW_TITLE`, `PREVIEW_SETUP` removed).
- First-run card (`Model.deskToday().firstRun`: no case in any column, no
  open drift, no crisis): "Seldon is recording. Nothing to do." in place of
  the sentence and its lead; tiles whose figure is 0 are muted (`dim`).
- Texts: `INIT_COMMAND` = `seldon init --defaults`; `INIT_SCRIPT` and the
  notInitialised banner say what `--defaults` does ("No questions, no
  password."); the button is *Create logbook*. WP-117 stage 2's grant line:
  "Read access granted. The snapshots were not recorded yet; Seldon tries
  again at its next capture." The install line: "The engine is installed.
  The Seldon panel finds it by itself."
- `snapperBanner` returns null for `snapper is not installed` (no grant
  helps; before, a machine without snapper got "Read snapshots").
- `Service.ingest`: an index written in the same second as the exit 3 is
  the new one (`>=` the second); the engine writes whole seconds, and
  `init` right after the card's probe hit that edge in the harness.
- The card shows in dev mode too (from the fixtures), so the plugin team
  sees it against `not-initialised.json` and `snapper-degraded.json`; its
  own captures are refused there like every engine call.
- `Model.needs` (WP-179) can count the card from `Service.setup.open`.

### Docs

- SPEC-PLUGIN §1 (the state key), §3 (when the engine is probed), §5.4
  ("Setup card", the first-run card, the preview line), §5.5 (Capture's
  Offer again), §5.6 (setup states are not notices; script texts).
- SPEC-ENGINE §3 synopsis and `--json`, §9 (the four modes, the
  look-back, Obsidian detection, "before Seldon").
- User guide 01 rewritten plugin-first (en/de): add the plugin, the card's
  three steps, then the old steps 5–8 as 3–6; "The terminal path" holds the
  install, `seldon init` / `--defaults` / `--ask` (the wizard table) /
  `--non-interactive`, and `doctor`. Guides 02, 05, 10, 11, 13 and
  STYLE.md: "before Seldon", the modes, the card's rows; links moved to the
  new anchors. German source lines at `30f3d0b5`.
- CHANGELOG (Engine, Plugin), `plugin/README.md` States, `README.md` step
  2/3, `docs/TESTING.md` (new tests, the wizard recipe now `--ask`),
  `tests/plugin/COVERAGE.md`.

## Contract

No change. The plugin runs no new argv (the init runs in the terminal
script, a constant; ADR-0045 §6's "no argv row"); no schema, fixture or
`contractVersion` change. Two soft points for the reviewer:

1. The plugin reads one engine **message** as a signal: snapper's
   `snapper is not installed` (index `state.collectors[].message`, "shown,
   never executed"). Pinned on both sides by tests; an explicit field
   would be contract 3 material (NEW-9).
2. The plugin writes one more key into its own `shell.json` entry
   (`setupSnapshots`), through the existing write path.

## Not done

- **Live check on the fresh test host from a clean home, counted per
  ADR-0027 §1: not run.** This session had no network (orchestrator rule),
  so no deploy to the test host. It is WP-126's scenario (a); to count:
  plugin add → Install (0 passwords) → Create logbook (0) → Grant (1
  password) or Not now (0) → first-run card; clicks 3–4, questions 0.
- EASY's use of the card (WP-187), `Model.needs` (WP-179): not in scope.
- The UX review's six-line result output ("Agents … installed") is not
  built: no harness is set up by default (operator decision above).

## How it was verified

| Check | Where | Result |
|---|---|---|
| `cargo test -j 4` (all engine tests; new: `init::defaults::*`, `plain_init_with_a_path_asks_nothing`, `the_ways_to_ask_exclude_each_other`, the no-terminal refusal, unit tests for the look-back start, `asks_with`, the wizard texts) | fixture (scratch HOME, stubbed sources, `SELDON_NOW`) | pass |
| `--defaults` reads no stdin: a pipe held open and never written, deadline 120 s | fixture | pass |
| `cargo clippy -D warnings`, `cargo fmt --check` | dev host | pass |
| `node tests/plugin/model.test.js` (6 new setup-card tests) | fixture | 203 passed |
| `bash tests/plugin/terminal-scripts.sh` | headless (stubs) | 65 passed |
| `desk-view.sh` setup cases (also inside the full check below) (`snapper`, `snapper-refused`, `uninit`, `restart-updated`, `fold`, `setup-flow`, `setup-partial`, `setup-old-engine`, `no-snapper-*`, `radiant-uninit`, `preview-*`) | headless (offscreen Quickshell, private HOME and XDG_RUNTIME_DIR) | pass |
| `service-states.sh` | headless | 359 passed |
| plain `seldon init` (one question) and `seldon init --ask` (full wizard, Enter on the backfill takes 2026-07-12) driven through `script` | headless pty on the dev host, scratch HOME/XDG and `SELDON_TEST_GUARD`, the real `/var/log/pacman.log` read-only | pass |
| `init --defaults` on the real package log: 1285 events in 90 days, 25 items (1022 events) dismissed "before Seldon", 0 open | same scratch environment | pass |
| `SELDON_FULL_CHECK=1 just check` on `2f470880` (log: private `gates/check-wp119-3.log`) | dev host (headless harnesses, private XDG_RUNTIME_DIR, disk TMPDIR and CARGO_TARGET_DIR) | `check: ok` — model 203, terminal-scripts 65, service-states 359, desk-view 1960, bar-view 196, ipc-restart 44, docs-check ok, qmllint ok, plugin-validate ok |
| shellcheck | not run, CI (not installed locally) | — |
| Live setup on the test host, counted | not run (no network) | — |

Note: `desk-view.sh` keeps its own short runtime dir under `/tmp`
(`mktemp -d /tmp/seldon-rt.XXXXXX`, WP-161: the socket path limit); that is
the harness's design, not changed here.

## Open questions

- Should the snapper "not installed" state become an explicit index field
  in contract 3 instead of the message match? (Recommendation: yes, with
  NEW-9.)
- The first-run card shows while no case exists and nothing is open, with
  no close button (the UX review allowed "or the user closes it"). Enough,
  or a dismiss stored like *Not now*?
