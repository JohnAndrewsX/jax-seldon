# WP-224 — Handover

Branch `wp/224-edge-compat` (from `e99a3c14`), worktree `wt/WP-224`. Not
pushed, not merged. Role: dev (Opus). Plugin (two QML ids, one rule text,
one transaction text, `Commons.Color` in 41 QML files), engine (`index/drift.rs`, `index/class.rs`,
`collectors/omarchy.rs`, a doc comment in `config.rs`), the qmllint recipe,
the Omarchy-assumption checker, its ledger and test, specs, TESTING,
CHANGELOG. No schema, contract, fixture or crate change. The `rule` set of
a drift item is open (ADR-0038 §1), so the new rule id needs no
`contractVersion` bump. `collectors/config.rs` and `skills.rs` are not
touched (WP-225 owns them).

## What was done

### 1. Qt 6.12 qmllint

- `id: form` → `id: driftForm` (`EventDetail.qml`) and `id: top` →
  `id: topColumn` (`ListColumn.qml`), with every reference updated. The
  public alias `property alias form` keeps its name, so `detail.form.*` in
  the harnesses and `Decisions.qml` are unchanged. The node test C1 pins
  the call `driftForm.showForm` now.
- **Not in the survey:** with the two ids renamed, qmllint 6.12 still
  exited 255 and printed no `Warning:` line. `--json` shows the cause:
  133 `block-scope-var-declaration` warnings (`var` inside a block, mostly
  `for (var i …)`) in 20 plugin files. qmllint 6.12 counts these against
  `--max-warnings` but does not print them in text mode. Omarchy's own
  shell has 336 `for (var` and no `let`/`const`, so this is the house idiom.
  The `qmllint` recipe now demotes the category to info, like
  `missing-property`. It passes the flag only when `qmllint --help` lists
  it, so Qt 6.11 (CI's runner) gets the same command line as before.
  `id-shadows-member` stays a warning. Documented in TESTING.md
  ("qmllint details"). **Reviewer:** the other option is rewriting the
  133 sites to `let`, which changes scoping and departs from Omarchy's
  idiom. I chose the demotion; it is one line to revert.

### 1b. Qt 6.12's `Color` and the harnesses on edge (orchestrator addenda)

- **The full check's first run failed** in `plugin-test` → service-states
  (`sheets-busy`, `sheets-rearm`: "log has errors"); desk-view, bar-view
  and ipc-restart never ran. The base commit `e99a3c14`, run in a scratch
  worktree on this host, fails the same two cases the same way, so the
  failure predates this WP.
- **Two causes, not one.** (a) Edge's `Commons/Style.qml` imports
  Quickshell.Hyprland, and in an offscreen Quickshell the Hyprland IPC
  module logs `WARN: Unable to find hyprland socket. Cannot connect to
  hyprland.` (survey B4). (b) Separate from (a), and not a renamed Style
  property: **Qt 6.12's QtQuick exports a singleton `Color`
  (`QtQuick/Color 6.12`, `QQuickColor`) that shadows the unqualified
  `Color` of `qs.Commons` in any import order.** In the plugin every
  `Color.popups.text` and `Color.accent` was undefined, which caused the
  Tone.qml TypeErrors. A minimal offscreen reproduction showed this:
  unqualified `Color` is `QQuickColor`, and `Commons.Color.popups.text`
  gives the theme's colour. Warming the singletons first changes nothing.
  This hits the **live** edge shell too, not only the harness: on an edge
  host the released plugin reads undefined theme colours. Omarchy's own
  edge code writes `Commons.Color` (194 uses, no unqualified ones).
- **Fix (b):** all 41 plugin and harness QML files that used `Color.` now
  add `import qs.Commons as Commons` and write `Commons.Color.` (142 refs),
  as Omarchy does; Qt 6.11 reads it the same. `check-tokens.py` fails a
  bare `Color.` and a `Commons.Color` without the alias import (qmllint
  cannot see it: missing-property is demoted), with self-test rows.
  SPEC-PLUGIN §7 and `memory/omarchy-shell.md` say why.
- **Fix (a):** `tests/plugin/upstream-warnings.sh`, sourced by the four
  harnesses. The Hyprland IPC module prints one warning, in one of two
  exact texts: `WARN: Unable to find hyprland socket. Cannot connect to
  hyprland.` where the harness inherits the session's signature
  (service-states), and `WARN: $HYPRLAND_INSTANCE_SIGNATURE is unset.
  Cannot connect to hyprland.` under `env -i` (desk-view, bar-view,
  ipc-restart; the second full-check run showed it on all 225 desk-view
  cases). The filter accepts exactly those two whole lines, and only while
  the loaded tree's `Style.qml` imports Quickshell.Hyprland and every
  plugin importer is a file the harness names as never loaded. The
  plugin's `DeskWindow.qml` imports it and uses `Hyprland.focusedMonitor`;
  desk-view swaps it for a stub, and bar-view, ipc-restart and the
  service-states sheets never load `Desk.qml`, its only creator. On stable
  or rc nothing is accepted. Every other WARN still fails, and so does a
  plugin file's warning with the same words. No socket is faked and
  `HYPRLAND_INSTANCE_SIGNATURE` is never set. `upstream-warnings.test.sh`
  (34 rows, in `plugin-test` everywhere) covers both texts, near misses,
  stable, a plugin importer, the not-loaded list (matched whole), the real
  plugin's importers and creators, and each harness's wiring.
- **Qt 6.12's locale warning:** under `env -i` Qt 6.12 logs `Detected
  locale "C" … not UTF-8` and switches to C.UTF-8 itself. desk-view,
  bar-view and ipc-restart now pass `LANG=C.UTF-8` to Quickshell, which
  is what Qt falls back to anyway, so behaviour is unchanged and no filter
  is needed. A test row checks it.
- Ledger: new `style-hyprland-warning` (Style.qml, symbol `Hyprland`, cites
  DeskWindow.qml's import); Style.qml now has its edge hash, so
  `style-icon-canvas` does too. Its re-measure steps are qmllint's token
  check and bar-view on edge, both in the full check below. `color-popups`
  names `Commons.Color` and why.

### 2. Omarchy's edge packages (engine)

- `index::drift::family(subject)`: `omarchy-dev` → `omarchy`,
  `omarchy-settings-dev` → `omarchy-settings`, `linux-omarchy-<variant>` →
  `linux-omarchy` (not `-headers` or `-docs`). `AlwaysRed::matches` tries
  the subject, then the name that stands for it. So the twins get
  **exactly** the verdict of their stable names, also in configs `init`
  wrote earlier (`alwaysRed` is always serialised into config.toml, so
  only changing the default list would not have reached existing
  installs). The default list itself is unchanged. A `linux-omarchy*` glob
  was not an option: `linux-omarchy-bore-headers` exists (pacman.log) and
  headers are not R3 (ADR-0023, WP-050; an existing test asserts it).
  Removing `omarchy` from a user's list also un-reds `omarchy-dev`.
- **Channel switch** (`index::drift::channel_switches`): a pacman
  transaction that removes a channel package and installs its twin, in
  either direction. The transaction was already one drift item (grouped by
  `txId`); `Classifier::group` now gives it rule `omarchy-channel`, keeping
  the class its members give it. With the default list that is a
  **crisis**: the explicit `omarchy-dev` install is `alwaysRed` by name
  (ADR-0028 §4c), exactly as a named `omarchy` install is today. The
  Omarchy `update` row the switch causes (4.0.4-2 → 4.0.0.r7017…, a
  version drop) is routine `omarchy-update`, because `History` counts a
  switch's install of `omarchy`/`omarchy-dev` like a plain full upgrade's.
  So the switch shows as one open item, not two.
- Omarchy collector: the fallback queries `pacman -Q omarchy`, then
  `pacman -Q omarchy-dev`. Attribution accepts a pacman event for either
  package.
- Plugin: *Why loud?* has a text for `omarchy-channel`, and the
  unfinished-transaction text names `omarchy-settings-dev` as well.
  SPEC-PLUGIN, SPEC-ENGINE (alwaysRed default, the rule table, the
  collector) and CHANGELOG (Unreleased highlights) are updated.

### 3. Omarchy-assumption ledger

- A ledger entry may carry `edge: {sha256, omarchyVersion, measured}`, the
  same assumption re-measured on the edge channel. `check-upstream` treats
  a file that matches either hash as unchanged. The ok line counts those
  files ("N as measured on edge"); a NOTICE names the edge hash too. Why
  not just overwrite `sha256`: the rc test host's release gate
  (`--gate`) runs on this ledger, and an edge-only hash would refuse it
  there. The stable `sha256` stays the pin's tree (ADR-0055).
- `shell-readme-contract` → **`plugin-registry-contract`** on
  `shell/services/PluginRegistry.qml`. The stable hash comes from the
  pinned tree (`c668141e`, the survey's checkout, identical to rc 4.0.4-1
  per ADR-0055 §5), the edge hash from the installed tree. Correction to
  the survey: the line "Full schema: PluginRegistry.qml" is in
  `docs/omarchy-shell.md` in Omarchy's git, not in the installed README.
- `button-focusable`: the sentence now says what edge changed (a Button
  narrower than its label shrinks it to 75 % and then elides it); edge
  hash added.
- `restart-shell`: the sentence and the re-measure step now cover edge's
  slower restart (up to 30 s + 60 s, then the notification service, exit 1
  if the shell does not come back); edge hash added.
- Ten stale line hints refreshed (the rule: the next commit that touches
  the ledger does it).
- SPEC-PLUGIN §1 and `memory/omarchy-shell.md` now name `PluginRegistry.qml`
  as the contract source instead of `shell/README.md`.

### 4. Review round 1 (packet `WP-224-review-1.md`)

- **F1:** `upstream-warnings.test.sh` has a row where the upstream text
  follows a plugin file's prefix and must fail. The start-anchor mutant
  (S4) now fails that row (34 passed, 1 failed; restored from git).
- **F2:** `family()` leaves out `linux-omarchy-…-debug` like `-headers`
  and `-docs`. Rows in `family_names_stand_for_their_twins` cover
  `linux-omarchy-bore-debug`, `linux-omarchy-debug` and
  `linux-omarchy-bore-docs`; `linux-omarchy` does not match
  `linux-omarchy-bore-debug`.
- **F3:** done in code. `Classifier::group` keeps the reason of another
  package that makes the transaction a crisis by its own rule, e.g. a
  `limine` named beside the switch stays `always-red`. Dependencies follow
  and are no second reason, and an attention member is not one either. The
  new rows are in `a_channel_switch_is_one_omarchy_channel_item`. It costs
  one check per member, and the class never changes.
- **F4:** the *Why loud?* text now reads "replaced omarchy,
  omarchy-settings or both with their -dev twins, or the other way round"
  (Model.js and SPEC-PLUGIN). A model.test.js row pins the whole callout
  and the Rule row.
- **F5:** the CHANGELOG says that with the default crisis list a switch
  is a crisis unless done in a case (ADR-0060).
- **F6:** the SPEC-ENGINE sentence about the twins now comes after
  "WP-050." and no longer sits inside the R3 list. It also names `-debug`
  and ADR-0060.
- **Q1:** `decisions/ADR-0060-adr-0028-amendment-omarchy-channels.md`
  amends ADR-0028 §2 and §4c and has a DECISIONS.md row. Its status is
  "accepted (E96 delegation, orchestrator 2026-10-14; operator confirms;
  stage 2 with the 0.4.1 release)". ADR-0059 is left to WP-225.
- **Q4:** the demotion is kept. It covers 133 sites in 20 files, not
  "few". The justfile comment now names the count and that the sites stay
  in the shell's idiom.

## How it was verified

All on the dev host (Omarchy edge `4.0.0.r7017.g84cfb93-1`, Qt 6.12.0,
Quickshell 0.3.2), worktree `wt/WP-224`, `CARGO_TARGET_DIR` = the
worktree's `engine/target`.

- **Review round 1, full check at `6bf9957b`** (every later commit only
  touches this HANDOVER): `flock /tmp/seldon-check.lock env
  SELDON_FULL_CHECK=1 just check` gave `check: ok`, exit 0. Results:
  `qmllint: ok (53 files)`, `tokens: ok (1042 references)`,
  `model.test.js: 246 passed` (the new channel row sits in an existing
  test), `upstream-warnings.test: 35 passed`, `assumptions.test: ok`,
  `service-states: 366`, `desk-view: 2755`, `bar-view: 195`,
  `ipc-restart: 43`, each with 0 failed. The harnesses ran *headless*
  against the edge tree; everything else is *fixture*. The guard lines
  for service-states and desk-view said "changed by the operator's live
  engine (not a leak)"; for bar-view and ipc-restart, "untouched".
  `just check-upstream`: exit 0, the same "10 of 17 files differ" NOTICE
  as before, none for the re-measured files. Engine fmt and clippy
  `-D warnings` are clean.
- **`flock /tmp/seldon-check.lock env SELDON_FULL_CHECK=1 just check`:
  `check: ok`, exit 0, at `ffee1a42`** (round 0; until round 1 every later commit touched
  only this HANDOVER). *headless* for the Quickshell harnesses, *fixture* for
  the rest: engine fmt, clippy `-D warnings`, all engine tests,
  check-packaging, check-guard, schema-validate, docs-check,
  plugin-validate, `qmllint: ok (53 files)` with
  `tokens: ok (1042 references)`, `model.test.js: 246 passed`,
  `check-tokens.test: 20 passed`, `upstream-warnings.test: 34 passed`,
  `assumptions.test: ok`, `service-states: 366 passed, 0 failed`,
  `desk-view: 2755 passed, 0 failed`, `bar-view: 195 passed, 0 failed`,
  `ipc-restart: 43 passed, 0 failed`. The harnesses ran against the
  installed **edge** tree, so this is also the first headless run of the
  plugin on edge. desk-view's guard line: "the real ~/.local/state/seldon
  changed by the operator's live engine (not a leak)"; the other three
  report it untouched.
- The two earlier full-check runs failed and led to §1b: run 1 at
  service-states (`sheets-busy`, `sheets-rearm`); run 2 (after the Color
  fix) at desk-view, 225 cases, each with only the `env -i` Hyprland text
  and Qt's locale warning.
- **Base comparison** (*headless*): `bash tests/plugin/service-states.sh`
  in a scratch worktree at `e99a3c14` failed the same two sheets cases
  with the same lines. So did my branch before the Color fix. The scratch
  worktree is removed.
- **Minimal reproduction** (*headless*, own HOME, private 0700 runtime
  dir, removed): unqualified `Color` is `QQuickColor` (`QtQuick/Color 6.12`
  in Qt's qmltypes) whatever the import order, and `Commons.Color.popups`
  is Omarchy's. Tone.qml's `dim` is `#000000` before the fix and the
  theme's `#9ba0a4` after it.
- **`just check-upstream`: exit 0.** No NOTICE for the re-measured files
  (`shell/services/PluginRegistry.qml`, `shell/Ui/Button.qml`,
  `bin/omarchy-restart-shell`, `shell/Commons/Style.qml`); "10 of 17 files
  differ" for the rest (open questions).
- **Engine acceptance** (*fixture*, in the check above and run alone):
  `twins_get_their_stable_names_verdict` (omarchy-dev, omarchy-settings-dev,
  linux-omarchy-bore, linux-omarchy-ptl-novrr-mm against their stable
  names in nine transaction shapes, a routine `omarchy-dev` sysupgrade
  included, plus the colours themselves),
  `family_names_stand_for_their_twins`, `always_red_globs` (bore red,
  bore-headers not), `a_channel_switch_is_one_omarchy_channel_item` (both
  directions, a quiet list, twins side by side),
  `tests/drift.rs::an_omarchy_channel_switch_is_one_item` (the real edge
  pacman.log lines of the switch, no host names or user paths in them: one
  open item `omarchy-channel` with 4 members, the update row routine;
  **a second capture writes 0 events**), the collector's
  `omarchy_falls_back_to_the_edge_package` and
  `omarchy_update_takes_the_edge_package_install`.
- **Mutants** (*fixture*, each applied, `cargo test --lib --test drift
  --test collectors`, then restored from git): 9 of 9 killed: each `-dev`
  name in `OMARCHY_CHANNELS`, the collector's `omarchy-dev` query,
  History's `omarchy-dev`, the kernel prefix, the `-headers` exclusion,
  the `omarchy-channel` naming, the family fallback in `matches`, and the
  reverse switch direction.
- **CI** (Qt 6.11, the pinned stable shell): *not run* (nothing pushed).
  The qmllint flag is feature-detected and `Commons.Color` is plain QML,
  so 6.11 should see no change; CI is the place to confirm it.
- **Test host** (rc): *not run*. The harnesses on an rc tree, and the
  release gate's `check-upstream --gate` with the `edge` field, are for the
  next deploy.
- **Desktop**: *not run* (dev host runs releases only, E38).

## Not done / open questions

- **AGENTS.md §1** still names `shell/README.md` as a reference. AGENTS.md
  changes only on the operator's instruction, so I left it: operator
  decision.
- **The other 10 ledger files** still NOTICE on edge (`check-upstream`:
  "10 of 17 files differ"). The survey read their diffs and judged that
  they hold, but I did not run all of their re-measure steps (several are
  live steps on the test host), so they got no edge measurement. Exit 0;
  a follow-up.
- **restart-shell's edge measurement comes from reading the script diff**,
  not from its live step: restarting the operator's live shell is out of
  bounds, and the test host runs rc. Say so if the ledger should only take
  an edge hash measured by its own step. (button-focusable's steps, the
  desk's Tab and Enter scenarios and the overflow/label-fit probe, ran in
  desk-view on edge: *headless*, passed.)
- **The released plugin on an edge host very likely reads undefined theme
  colours** (Qt 6.12's `Color`; same imports as in the harness). Not
  observed on the live desktop (*not run*: I did not touch the operator's
  session). The dev host runs releases (E38), so the operator's desk there
  is likely affected until a release carries this fix: a release question
  for the orchestrator.
- **Crisis or attention for a channel switch?** ADR-0060 §3 keeps the
  existing rule (a named `alwaysRed` install outside a case is a crisis),
  so `omarchy version channel edge` without a case is red. Making a
  switch that Omarchy runs itself attention would be a further amendment;
  that is the operator's call (review Q3).
- The item's lead (subject) is the lowest-id explicit member, i.e.
  `omarchy-settings-dev` (installed first). The rule and *Why loud?* name
  the channel change; `drift show` lists all four members. I did not
  change lead selection, which is generic for every transaction.
- `docs/user/*/06-configuration.md` lists a six-name `alwaysRed` default
  that was stale before this WP; not changed.
