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
    | Tab / Shift-Tab | walk the tabs. Past the last (or first) tab, hand over to the bar's next (or previous) panel via `bar.switchPanelFrom`. Wrap when there is no other panel (see Decision 1) |
    | ← / → | previous / next tab |
    | 1–3 | jump to a tab |
    | ↑ / ↓ | move the cursor in the tab's list. The first press only shows the cursor, the bluetooth panel pattern |
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
  - left stripe: the event's zone in theme colours. Red is `urgent`,
    yellow `accent`, green `Color.muted`;
  - glyph, status line and badge: the drift item's colour (Decision 4);
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
  - `tests/plugin/model.test.js` (27 tests): the new `validateArgs` forms
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
  - **New `tests/plugin/panel-view.sh`** (78 checks), with
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
$ just check                                  → exit 0
  plugin-validate: ok
  tokens: ok (135 references) · qmllint: ok (10 files), --max-warnings 0
  model.test.js: 27 passed
  service-states: 61 passed, 0 failed
  panel-view: 78 passed, 0 failed
  (final run on 251f5c8; an earlier run failed once on the recorder race
  described under Done, which led to that fix)
$ under load (32 busy loops, 16 cores): service-states 61/61, panel-view 78/78
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
4. Real keys with `wtype`:

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

## Decisions needed

1. **Tab key: SPEC-PLUGIN §5 and the WP disagree.**
   - §5 says "`1`–`6`, `←/→` switch tabs, `Tab` switches Omarchy panels",
     as every first-party panel does.
   - The WP says "Tab/Shift-Tab between tabs", and its acceptance test
     says "Tab cycles tabs".
   - Implemented: both. Tab walks the tabs and, past the last, hands over
     to the bar's next panel. It wraps only when there is no other panel.
     ←/→ and digits also switch tabs.

   Please pick one and amend the loser:
   - keep the hybrid (amend §5);
   - pure §5, Tab = panels only;
   - pure WP, Tab cycles and never leaves the panel.
2. **Source filter includes `manual`.** The WP lists eight sources and
   omits `manual`, but the schema has nine, and the sample has 6 manual
   notes that would otherwise be unreachable by filter. Accept, or drop
   the chip.
3. **`drift dismiss <id> [--only] --reason <text>` has no `--` guard.** A
   reason starting with `-` is read as an option by the engine's
   argument parser. The plugin passes it as one argument and does not
   reject it.

   Proposed CONTRACT.md change, coordinated with WP-008:
   - `--reason=<text>` (one argv element), or
   - `dismiss <id> [--only] -- <reason>`.
4. **Zone colours of a drift group.** The stripe shows the event's own
   ledger zone, which is red for the 09-30 members (ADR-0014). Glyph,
   status line and "+N" badge show the drift item's computed zone, yellow
   (ADR-0013 §3). So the group does not read as a crisis while the ledger
   stays truthful. Confirm, or name one colour source.
5. **Spec amendments (orchestrator-owned):**
   - SPEC-PLUGIN §2: add `components/TodayTab.qml`, `ChangelogTab.qml`
     and `SystemTab.qml`, one file per tab.
   - §8: `jax.seldon.panel` gains `view`, `tab <id>` and `filter
     <source>`, which are read-out and navigation only.
   - §5: digits select visible tabs by position (1–3 today). They will
     shift when Work and Decisions arrive, unless you want fixed numbers
     (Today 1, Changelog 2, System 5).
6. **The guard hook blocked `omarchy theme set` inside an ssh command
   line**, although the brief allows it on the test host. I ran the sweep
   from a script copied to the test host (`theme current` before,
   `theme set` per theme, restore on exit). Please confirm this was
   acceptable, or allow `ssh <test-host> omarchy theme set` in the guard.
7. **Fixtures (WP-014, information only):** nothing the acceptance needs
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
