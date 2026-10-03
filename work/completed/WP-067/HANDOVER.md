```
WP-067 HANDOVER — plugin: tab focus, changelog cursor, IPC handler once
Branch: wp/067-review (worktree wt/WP-067), commits on 54be2c3, not pushed.
```

## Done

1. **A tab change gives the keys back** (`plugin/Panel.qml`,
   `components/TodayTab.qml`, `components/WorkTab.qml`).
   - `Panel.selectTab()` moves the focus to the key catcher after every tab
     change while the panel is open. Keys (`←/→`, `h/l`, digits), a click on
     the tab strip and IPC `tab` all go through it.
   - TodayTab: `onVisibleChanged` calls `quickEntry.leave()` while it has
     the keys. That closes an open case picker, which a focus move alone
     leaves open: `editing` stays true through `popupOpen` and the panel
     stays blocked.
   - WorkTab: `onVisibleChanged` disarms (as before) and, while the
     new-case sheet has the keys, sends `leaveRequested`. The sheet stays
     open with its title, as the Changelog and Decisions sheets already do.
   - The draft is kept in every case. `view()` gained `keys` (the key
     catcher has the active focus).
2. **The Changelog cursor follows its event**
   (`components/ChangelogTab.qml`). This is the WorkTab/DecisionsTab
   pattern: `selectedId`, `select(i)` and `findSelected()`. Every cursor
   write goes through `select()`: keys, row click, hover, *Resolve…* and
   the crisis strip. `onRowsChanged` puts the cursor back on its event. If
   the event is gone, the cursor keeps its row number. If the rows are
   empty for a moment, the event id is kept.
   - A new filter still starts at the top. That reset moved from
     `onFilterChanged` to `onRowsChanged` (`rowsFilter`), so it does not
     depend on which of the two handlers Qt runs first.
   - `view().changelog.selected` reports the id.
3. **One handler for `jax.seldon.panel`** (`plugin/BarWidget.qml`).
   Reproduced first (see "Verified by"). The installed shell builds a bar
   per monitor (`Bar.qml`: `Variants { model: Quickshell.screens }`) and
   says in `moduleWidgets` / `findPanelWidget` that a widget is live once
   per screen, plus a placeholder for an anchored centre module. The
   first-party bar widgets (`omarchy.clock`) keep a per-instance
   `IpcHandler` and relay to their peers (`Ui/BarWidget.broadcast`). The
   shell's own `Ui/Panel.qml` uses `IpcHandler.enabled`.
   - Our widget now enables its handler only when it is the first live
     instance in `bar.moduleWidgets(moduleName)`. It checks with
     `Qt.callLater` after completion and after `bar` is injected.
   - The owner's `Component.onDestruction` first disables its own handler,
     then asks the others to claim the target (`claimIpc(leaving)`).
   - Without the bar's list (an empty facade), the widget owns the target
     alone, as before. IPC calls act on the owning instance's panel. That
     is what Quickshell's first-registered rule did before, minus the
     warning.
4. **Harnesses.**
   - `tests/plugin/panel-view.sh` case 25 `tab-focus` and case 26
     `cursor-follow`, both live against the fake engine.
   - `tests/plugin/bar-view.sh` case 4 `ipc`, using the new `HARNESS_IPC`
     mode of `tests/plugin/harness/bar.qml`. It runs two widgets (the
     facade's `_moduleWidgets` lists them) and drives the target with real
     `quickshell ipc call` processes. It checks the owners, that `open`
     reaches the owner, then destroys the owner ("monitor gone") and checks
     that `open` reaches the survivor. It also checks for no "another
     handler is registered" line in the log.
   - The IPC socket needs a short `XDG_RUNTIME_DIR`
     (`mktemp -d /tmp/seldon-ipc.XXXXXX`, removed after the run).
5. **Docs.** SPEC-PLUGIN §5 keyboard paragraph (tab change hands the keys
   back; the Changelog cursor follows its event) and §8 IPC paragraph (the
   bar panel target is registered by one instance, with takeover).
   CHANGELOG `[Unreleased]` › Plugin: three bullets, appended.

## Not done

- No live test on a real two-monitor Omarchy session (no such host here,
  and the plugin must not be edited in the production shell). The harness
  reproduces the warning with the real `IpcHandler` and the real
  `quickshell ipc` client, but `bar.moduleWidgets` is the harness's
  stand-in list. Its order and timing in the real bar come from reading
  `Bar.qml` (slots register in `Component.onCompleted`; `bar` is injected
  in the slot loader's `onLoaded`, after `activeItem` exists).
- Which instance a keyboard-less IPC `open` should use on several monitors
  (e.g. the focused one, like `findPanelWidget`) is unchanged: the owner,
  as before. The shell does not expose `BarModel.pickPanelSlot` to
  plugins.
- `tests/plugin/harness/KeyboardPanel.qml` and `harness/panel.qml` needed
  no change. The existing `click:` and `tab:` steps cover the mouse and
  IPC paths.

## Verified by

New cases alone (scratch copy of the scripts with cases 1–24 / 1–3 cut;
pre-fix = the plugin with the fix lines removed, run on a scratch copy of
`plugin/`, worktree untouched):

```
panel-view (cases 25, 26), fix:        41 passed, 0 failed
bar-view (case 4), fix:                12 passed, 0 failed

mutant "all" (selectTab focus line and both onVisibleChanged handlers removed = pre-fix behaviour):
FAIL tab-focus #3: .view.today.quickEntry.editing = true (want false)
FAIL tab-focus #3: .view.keys = false (want true)
FAIL tab-focus #4: .view.today.quickEntry.text = abcj (want abc)
FAIL tab-focus #6: .view.today.quickEntry.result = Saved to the journal · 01M3W1FAKE0000000000000NTE (want )
FAIL tab-focus #12: .view.work.sheet.editing = true (want false)
FAIL tab-focus #15: .view.work.result = Created C-2026-009 · xyz (want )
FAIL tab-focus: engine argv differs        (log -- abcj and plan new … xyz were sent)
… 14 failures in all

mutant "visible" (both onVisibleChanged handlers removed, selectTab kept):
FAIL tab-focus #20: .view.today.quickEntry.editing = true (want false)   (case picker left open)

mutant "select" (selectTab focus line removed, handlers kept):
41 passed, 0 failed   — survives, see below

mutant "rows" (ChangelogTab onRowsChanged back to the old clamp):
FAIL cursor-follow #5: .view.cursor = 4 (want 6)
FAIL cursor-follow #6: .view.drift.open = false (want true)
FAIL cursor-follow #6: .view.changelog.expanded = 01M3VXYHJ0CDTAWEV5WRW4C0C2 (want )   (event 114 expanded instead)
FAIL cursor-follow #8: .view.cursor = 4 (want 0)   (artefact: the filter reset now lives in onRowsChanged)

bar mutant "main" (BarWidget.qml from main = pre-fix):
FAIL ipc owners = null,null (want true,false)
FAIL ipc one handler = 1 (want 0)
FAIL ipc: log has errors
     WARN scene: QML IpcHandler at …/BarWidget.qml[140:3]: Handler was registered but will not be used because another handler is registered for target jax.seldon.panel

bar mutant "no handover" (Component.onDestruction removed):
FAIL ipc owners after = null,false (want null,true)
FAIL ipc open after output = Target not found. (want )
FAIL ipc open after reaches the new owner = null,false (want null,true)
```

(The "no handover" run is the rerun after the output check was added:
9 passed, 3 failed.)

The `select` mutant survives because, for every tab that has a text field
today, the tab's own hide handler gives the keys back too. The selectTab
line is the catch-all the WP asks for (and the one that also covers a
future tab). No harness case can reach it alone without a tab lacking a
handler. The `visible` mutant is killed by the case-picker step.

Pre-fix, Quickshell already hands the target to the remaining handler
when the active one is destroyed (bar mutant "main": `openedAfter
null,true`). So the old behaviour was the warning only. A handler that
starts disabled is not a candidate, so the fix needs the explicit
takeover; the "no handover" mutant shows that.

Full gate: `just check` → see the last section.

`omarchy plugin validate` and qmllint: ok (29 files). The real-home guard
line is green in both scripts.

## Learned

Appended to memory/pitfalls.md (WP-067).

## Decisions needed

None.

## Touched outside WP scope

None. The files are those listed under *Files touched*, plus `CHANGELOG.md`
`[Unreleased]`. `KeyboardPanel.qml` and `harness/panel.qml` were not
changed.

## just check

`just check` on c9927a1..51c207b: exit 0, `check: ok`. The plugin lines:
model.test.js 81 passed, service-states 205/0, panel-view 733/0,
overlay-view 319/0, bar-view 131/0, real-home-guard.test 11/0. Every
script's real-home line is green. docs-check, plugin-validate and qmllint
are ok, and the engine suites pass (fmt, clippy, test, check-watch).
