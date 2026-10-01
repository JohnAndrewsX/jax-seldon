WP-011 HANDOVER

Branch `wp/011-panel-tabs`, worktree `wt/WP-011`. Not pushed, no PR.

## Done

- **`plugin/Panel.qml`**: the tabbed panel (width `Style.space(380)`).
  - Top to bottom: title with last capture, `components/Tabs.qml`, the
    status banner (WP-010), the snapper-degraded banner, the red crisis
    strip, then the tab body (fixed height), the last engine error and the
    dev-mode line.
  - The banners and the strip sit above the body, so they show on every
    tab. Clicking the strip opens the Changelog.
  - Keyboard, through the shell's own `PanelKeyCatcher`:

    | Key | Does |
    |---|---|
    | Tab / Shift-Tab | only hand over to the bar's next (or previous) panel via `bar.switchPanelFrom`, as in the ten first-party panels (SPEC-PLUGIN §5; review follow-up 1) |
    | ← / →, h / l | previous / next tab (wraps) |
    | 1–6 | a tab by its fixed number, `Model.TAB_KEYS`: Today 1, Changelog 2, Work 3, Decisions 4, System 5, Memory 6. The digit of an absent tab is ignored (review follow-up 2) |
    | ↑ / ↓, k / j | move the cursor in the tab's list. The first press only shows the cursor, the bluetooth panel pattern |
    | Enter / Space | open the row: a group's members, full text, or the yesterday row |
    | f / F | cycle the Changelog source filter |
    | c | capture |
    | Esc | close |

- **Today tab** (`components/TodayTab.qml`):
  - date heading with *Open in editor* (`seldon open journal --editor`);
  - summary counts: events today, events in 7 days, active, queued, open
    drift;
  - today's journal entries (time · actor · case, then the text);
  - yesterday collapsed behind one row; Enter or a click opens it.
- **Changelog tab** (`components/ChangelogTab.qml`):
  - `index.events` newest first, under day headers (Today / Yesterday /
    "Tue 1 Sep");
  - source filter chips with counts: all plus the nine schema sources;
  - "N events · newest first" and *Capture now*.
- **`components/EventRow.qml`**, one row per event:
  - line 1: Nerd Font source glyph, kind, subject, "+N" badge on a drift
    group's leader, time;
  - line 2: detail · actor · case;
  - line 3: the folded resolution (`explained: <resolutionDetail>`,
    `linked to C-…`), or for open drift *Unexplained*, *Needs a reason*
    (crisis) or *In the open firefox group*, plus "proposed for C-…";
  - one colour per row, `row.tone` (review follow-up 3). While the event
    is open drift it comes from the drift item's computed zone, so the
    routine firefox group is accent throughout although its members are
    red in the ledger. Otherwise it comes from the event's own zone.
    Red is `urgent`, yellow `accent`, green `Color.muted`. It paints the
    left stripe and, for open drift, the glyph, the status line and the
    "+N" badge;
  - snapshot rows: `CursorSurface.current`, the theme's selected fill;
  - expanded: a group lists its members from `index.events` by `txId`.
- **System tab** (`components/SystemTab.qml`):
  - sections: OMARCHY, PACKAGES (with deviations), PLUGINS, SNAPSHOTS,
    AREAS, COLLECTORS (ok / failing + message / off), SELDON (machine,
    engine, index time);
  - every field is optional, and a section appears only when it has a row.
- **`plugin/Model.js`**:
  - new helpers: `stateIndexPath`, `crisisText`, `snapperBanner` with the
    constant `SNAPPER_FIX_COMMAND` (ADR-0011), `changelogRows`,
    `sourceCounts`, `filterChips`, `cycleFilter`, `groupMembers`,
    `rowMeta`, `rowStatus`, `todayView`, `entryMeta`, `systemSections`,
    plus date and actor formatting.
  - `validateArgs` now accepts exactly the CONTRACT.md forms:
    - free text is one non-empty, non-blank argument after `--`;
    - `drift link|explain|dismiss … [--only]`;
    - `drift dismiss <id> [--only] -- <reason>`, the form settled at
      review: it shares the explain branch, and the old `--reason <text>`
      form is refused;
    - `drift show <eventId> --json`, where `--json` is required;
    - the WP-010 forms without `--` are refused;
    - `--json` is only recognised before the separator.
- **`plugin/Service.qml`**:
  - The index path is `${XDG_STATE_HOME:-$HOME/.local/state}/seldon/index.json`.
    A relative `XDG_STATE_HOME` is ignored, per the XDG spec.
  - The engine is probed at start and on *Check again* /
    `jax.seldon.service refresh`. The capture timer runs only while the
    engine is present and never probes (WP-010 review, decision 5).
  - New: `crisisText`, `snapperBanner` and `indexShown`.
  - `fix(actionId, bannerId)` takes `"status"` or `"snapper"`. Copy and
    terminal use the constant command of that banner only.
  - The status snapshot gains `crisis` and `snapper`.
- **`plugin/BarWidget.qml`**: the `jax.seldon.panel` IPC target gains
  three methods. All are read-out or navigation only; none runs the engine.
  - `view`: JSON of tab, cursor, rows, badges, banners and strip;
  - `tab <today|changelog|system>`;
  - `filter <all|source>`.
- **Tests:**
  - `tests/plugin/model.test.js` (28 tests): the new `validateArgs` forms
    and refusals, plus every new helper against the sample and the
    snapper-degraded fixture. Covered: 58 rows, one "+3", 7 folded
    details, 6 snapshots, the filter sums, the crisis text (1 vs 2), and
    sparse and empty `system`.
  - `tests/plugin/service-states.sh` (61 checks):
    - The portability fix: the base `PATH` is a temp dir of symlinks to
      exactly the tools the fakes need, and `timeout` is called by
      absolute path. A `seldon` in `/usr/bin` no longer matters.
    - New scenarios: the crisis strip; snapper-degraded, with the exact
      argv of its Copy and Run in terminal; no strip or banner without
      an index; `XDG_STATE_HOME`, including the relative form, with the
      fake engine writing there.
  - **New `tests/plugin/panel-view.sh`** (86 checks after the review follow-ups), with
    `harness/panel.qml` and `harness/KeyboardPanel.qml`:
    - It runs the real Panel.qml offscreen against copies of the
      installed shell's `Commons/` and `Ui/`. Only the layer-shell
      `KeyboardPanel` is replaced.
    - It presses real keys (QtTest `keyClick`) through the shell's
      `PanelKeyCatcher`.
    - It asserts what is visible on every tab: every acceptance number,
      the keyboard, the snapper and not-initialised variants, and empty
      and sparse `system`. The log must be clean: no warnings,
      TypeErrors or binding loops.
  - It is part of `just plugin-test`, and therefore of `just check`.
- **Harness load flake (orchestrator note; commit 251f5c8).** `fix-engine`
  failed on main under parallel cargo builds with an empty record.
  - Cause: the fixes fired at a fixed 1 s, before the engine probe had
    settled. Without a banner there is no command, so nothing was
    recorded. A second race: the detached fix processes can outlive the
    harness, and the fake recorder appended line by line, so two
    concurrent launches could interleave. I hit that once myself, in
    `snapper-degraded`.
  - Fix, `harness/shell.qml`:
    - fixes run once the service is `ready`;
    - the final snapshot waits until the service has settled: ready, no
      probe (new read-only `Service.probing`) or engine call in flight,
      fixes and recheck done, and an optional
      `HARNESS_UNTIL=field=value`;
    - `HARNESS_MS` is now the earliest final snapshot, with 15 s of grace
      after it.
  - Scenarios that wait for an asynchronous outcome declare it with
    `HARNESS_UNTIL=status=…`: appears-later, atomic-replace,
    engine-appears, live, live-uninit, init-later and xdg.
  - `record_check` polls up to 15 s until the record holds the expected
    number of invocations, and compares them order-insensitively (the two
    detached launches have no defined order).
  - `fake-recorder` appends each invocation in one `write()`.
  - `panel-view.sh` starts its steps on service readiness (at least 1 s,
    at most 15 s), not after a fixed 1.5 s.
  - Load test: 32 busy loops on 16 cores, load average 28.
    `service-states` passed 61/61 (95 s) and `panel-view` 78/78.
- **Docs:**
  - `docs/TESTING.md` plugin section:
    - the four layers, with the new panel layer;
    - panel IPC and `wtype` keys in the runtime smoke;
    - the grim logical-pixel note and the multi-instance log check;
    - the three-theme procedure.
  - `plugin/README.md`: the tabs, the row anatomy, the keys, the snapper
    state, the new IPC methods, and the probe policy.
- **Screenshots** in `work/active/WP-011/screenshots/`: 13 PNGs, 348 KB,
  64 colours. The acceptance text says `work/completed/WP-011/`; I used
  the brief's path, and they move with the folder.

## Not done

- **Spec-table actions outside the WP outputs:**
  - QuickEntry (`seldon log`) on Today: `components/QuickEntry.qml`, a
    later WP;
  - the Changelog row sheet (link / explain / dismiss), which needs the
    Work tab's case picker;
  - *Rebuild doc* and *Update impact* on System.

  `validateArgs` already accepts their command forms.
- **`seldon drift show <id> --json` is validated but not called.** The
  group expansion reads `index.events`, enough for the fixture. Fetching
  members beyond the 500-event cap belongs to WP-021, the expand list.
- **Probe timing has no harness test.** "No probe on the capture interval"
  is not tested in the harness, because the interval is at least 5 min.
  It is a one-line `running: engineState === "present"` on the timer.
  Live, a shell instance with the new code logged exactly one failed probe
  per start; the WP-010 shell before it had logged 7.
- **Mouse clicks were not exercised on the live shell:** strip, chips,
  rows and buttons (no pointer injection over ssh). Keys, IPC and
  rendering were. Chip, row and button handlers are one-liners calling
  the same functions the keys use.
- **On the dev host the plugin was only copied and validated**
  (`~/.config/omarchy/plugins/jax.seldon`), not enabled.

## Verified by

```
$ just check                                  → exit 0   (after the review follow-ups)
  plugin-validate: ok
  tokens: ok (136 references) · qmllint: ok (10 files), --max-warnings 0
  model.test.js: 28 passed
  service-states: 61 passed, 0 failed
  panel-view: 86 passed, 0 failed
  (an earlier run, before 251f5c8, failed once on the recorder race
  described under Done, which led to that fix)
$ under load (32 busy loops, 16 cores, at 251f5c8): service-states 61/61, panel-view 78/78
$ find plugin -type l | wc -l                 → 0
$ omarchy plugin validate ~/.config/omarchy/plugins/jax.seldon   (dev install) → ok
```

**Runtime smoke on the test host** (Omarchy 4.0.4-1, quickshell 0.3.1,
display scale 1.25):

1. Setup:
   - rsync the plugin, validate, `omarchy-restart-shell` after every copy
     (four restarts);
   - install the sample as `~/.local/state/seldon/index.json` with
     `generatedAt` and `lastCapture` set to now;
   - put a constant stand-in in `~/.local/bin/seldon`: `--version` →
     JSON, `capture`/`status` → exit 0, writes nothing. It gives status
     `ok`, because the engine does not write indexes yet. The calls it
     received, in order: `--version --json`, `capture --all --json
     --quiet`, `status --json`.
2. `jax.seldon.service status` → `ok`, pill `⟡ 2 · 4`, crisis "2 changes in
   the red zone need a reason".
3. `jax.seldon.panel view` on the sample:
   - Today: 4 entries, 1 yesterday;
   - Changelog: 58 rows, badges `["firefox +3"]`, folded 7, snapshots 6;
   - System: 7 sections;
   - the strip shows on every tab;
   - with `index-variants/snapper-degraded.json` the snapper banner
     "Snapshots not readable" shows with the engine message, the
     constant command, *Run in terminal* and *Copy*.
4. Real keys with `wtype`, in the first run, before the review follow-ups
   changed Tab and the digits (for the current behaviour see step 10):

   | Key | Result |
   |---|---|
   | Tab | today → changelog → system |
   | Shift-Tab | → changelog |
   | → / ← | switch tabs |
   | `1` / `2` | switch tabs |
   | `f` | filter pacman |
   | `F` | filter all |
   | ↓ | turns the cursor on |
   | ↓↓↓ | cursor 3 |
   | ↑ | cursor 2 |
   | Return | expands the row |
   | Esc | `opened: false` |
   | Tab on System | opens the bar's next slot (OmaSettings, which opens its own window, so the Seldon panel stayed open; Learned) |

   Filter pacman, then ↓ ×10 and Return, expands firefox `+3`, listing
   libinput, noto-fonts and firefox.
5. Theme sweep: Osaka Jade (found), Tokyo Night, and Catppuccin Latte
   (light), each with Today, Changelog, System and the expanded group. All
   colours follow the theme: border, tabs, chips, zone stripes, strip,
   banner, snapshot highlight, and dim text, which stays legible on Latte.
   Restored to Osaka Jade; `omarchy theme current` confirms it.
6. Logs: `quickshell log` of **every** shell instance of the run (five
   pids; theme switches restart the shell). It shows no QML error or
   warning from jax.seldon. The only line naming it is the expected
   `Process failed to start … ("seldon", "--version", "--json")`, once at
   the one start without an engine. Other warnings come from other plugins
   (superproductivity, omalauncher, finder, the agents panel's missing
   light asset) or are the known shell-wide lines.
7. Bugs found live and fixed (commits 0261133, fd61bf1):
   - the Today heading scrolled out of view (ListView `header`);
   - an expanded group was clipped at the bottom;
   - a yellow group's members drew red glyphs, from the ledger zone.
8. Cleanup: the stand-in, `~/.local/state/seldon/`, the smoke scripts and
   the call log are removed. The plugin stays installed and enabled. It
   shows engineMissing, as WP-010 left it.
9. After 251f5c8 (adds `Service.probing`), I redeployed and restarted
   once more:
   - panel opened on the Changelog with the engine-missing banner;
   - the log's only jax.seldon line is the expected start probe.
10. After the review follow-ups: redeploy, restart, the sample in status
    `ok` with the stand-in again, theme Osaka Jade, no theme switch.
    - Real keys (`wtype`):

      | Key | Result |
      |---|---|
      | → | changelog |
      | `l` | system |
      | ← | changelog |
      | `h` | today |
      | `5` | system |
      | `3` | stays on System (no Work tab) |
      | `2` | changelog |
      | `1` | today |
      | `j` | turns the cursor on |
      | `j` | cursor 1 |
      | `k` | cursor 0 |
      | Esc | closes |
      | Tab (on Today) | opens the bar's next slot (OmaSettings, a window); the Seldon panel stays open on Today, no tab change |
      | Shift-Tab | opens the previous bar panel, which closes ours (`opened: false`) |

    - `view` gives driftTones `tokyo-night accent, ollama.service urgent,
      ollama urgent, libinput accent, noto-fonts accent, firefox accent`.
    - The new Osaka Jade screenshots show the group in accent throughout
      while cased rows stay red, and the snapshot times shown in full.
    - Logs: the instance after the restart has no jax.seldon line. The
      instance before it shows only the expected start probe.
    - Cleaned up as in step 8. The screen was checked clean, with no
      leftover OmaSettings window.

## Learned (appended to memory/omarchy-shell.md, "WP-011 findings")

- A private `quickshell -p <root>/shell.qml` serves `qs.*` from `<root>`.
  Copies of the shell's Commons/Ui run offscreen with only `KeyboardPanel`
  stubbed. This corrects the WP-010 note that `qs.*` cannot load there.
- QtTest `TestCase.keyClick` injects real key events in an offscreen
  Quickshell.
- A ListView `header` scrolls away when the model is replaced.
  `positionViewAtIndex` after a row grows must wait for its relayout
  (`onHeightChanged`). `height: visible ? implicitHeight : 0` in a delegate
  Column loops.
- `Bar.switchPanelFrom` opens the neighbour; ours closes only if the
  neighbour is a KeyboardPanel.
- `omarchy theme set` restarts the shell (new pid), so check logs of every
  instance.
- grim regions are logical pixels (the test host is scaled 1.25). Over
  ssh, grim needs `XDG_RUNTIME_DIR`, and hyprctl needs
  `HYPRLAND_INSTANCE_SIGNATURE`.
- `Color.muted` exists. There is no yellow token, so yellow maps to
  accent. `Util.alpha` dims text for light themes, `Qt.darker` does not.

## Review follow-ups (review: APPROVE with five follow-ups)

Commits b98b79a (plugin), f627dc3 (tests), cebd728 (README), plus the screenshots with this handover.

1. **Tab follows SPEC-PLUGIN §5.** Tab and Shift-Tab only call
   `switchPanel(direction)`. ←/→ and h/l switch tabs (and wrap); Tab
   never cycles tabs. The panel-view keys scenario runs with a stand-in
   bar (`HARNESS_BAR=1`) whose `switchPanelFrom` records the direction:
   Tab → `1`, Shift-Tab → `-1`, and the tab does not change. The README
   key table names h/j/k/l, and the Panel.qml header comment is updated.
2. **Digits are fixed per tab id**: `Model.TAB_KEYS`, Today 1,
   Changelog 2, Work 3, Decisions 4, System 5, Memory 6. An absent tab's
   digit is ignored. The tab tooltips show the fixed digit. Tested in
   node (`TAB_KEYS`, `tabKeyFor`) and panel-view (`5` → System, `3`
   stays, `1`, `2`).
3. **One colour source per Changelog row.** `changelogRows` sets `tone`
   from the open drift item's zone (falling back to crisis → red, then
   the event's zone, if an item lacks `zone`), else from the event's own
   zone. EventRow paints stripe, glyph, status and badge from it. Node
   tests: firefox, libinput and noto-fonts are `accent`, ledger zone
   still `red`; resolved btop and cased hyprland are `urgent`; the
   fallback is covered. panel-view checks `view().changelog.driftTones`.
4. **Banner.qml** dims the command with `Util.alpha(foreground, 0.65)`
   instead of `Qt.darker`. No `Qt.darker` is left in `plugin/`.
5. **System snapshots**: the label is `#113`, and the value carries the
   time, `2026-10-01 14:30 · pre: ollama` (type appended for pre/post).
   Node tests are updated, and the live screenshot shows it in full.

Also, from the settled notes: `validateArgs` takes `drift dismiss <id>
[--only] -- <reason>` on the explain branch and refuses `--reason
<text>`. Node tests cover both.

Screenshots: `osaka-jade-group.png` and `osaka-jade-system.png` were
replaced with post-follow-up captures. The Tokyo Night and Catppuccin
Latte group and system shots still show the pre-follow-up colours and
snapshot labels (see Decision 1).

## Decisions needed

1. **The guard still blocks `ssh <host> omarchy theme set` from this
   worktree.** I tried once, to refresh the Tokyo Night and Latte shots
   after follow-ups 3 and 5. The hook answered "omarchy command that
   changes the system". Per the review ruling I stopped and did not
   route around it. Probable cause, not verified: the hook runs
   `$CLAUDE_PROJECT_DIR/scripts/guard.sh` from this branch, which
   predates the change on main.

   Either merge as is (the other two themes' group and system shots
   show the old row colours and snapshot labels; the code is covered by
   tests and the Osaka Jade shots), or let the sweep be redone once the
   guard change is on this branch.
2. **docs/TESTING.md is now out of date in two places** (docs/ is not
   mine to touch):
   - the panel-harness paragraph lists the keys as "(Tab, Shift-Tab,
     ←/→, 1–3, ↑/↓, Enter, Esc)". Now it is "Tab/Shift-Tab hand over to
     the bar, ←/→ and h/l switch tabs, digits fixed per tab id (1, 2,
     5)";
   - the runtime-smoke note "Tab on the last tab opens the bar's next
     panel" should read "Tab opens the bar's next panel".
3. **Fixtures (WP-014, information only):** nothing the acceptance needs
   is missing. Not exercised by the sample, so covered only by node tests:
   - an event with `zone: green`;
   - a `system.snapshots` entry of type `pre`/`post`;
   - a `system.omarchy.repoHead`;
   - a failing non-snapper collector.

## Touched outside WP scope

- `justfile`: `plugin-test` also runs `tests/plugin/panel-view.sh`, and
  its skip message names the shell.
- `docs/TESTING.md`: one cell of the `just check` table (Plugin logic)
  outside the plugin section.
- `memory/omarchy-shell.md`: appended, as instructed, plus a one-line
  correction pointer on the WP-010 harness note.

`schema/`, `fixtures/`, `engine/` and `scripts/` were not touched.
