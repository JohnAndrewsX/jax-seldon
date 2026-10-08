# WP-162 plan — the shell crashes on every IPC restart with two bar widgets

Role: Plugin Dev. Branch `wp/162-ipc-restart`, worktree `wt/WP-162`.

## Inputs read

- `work/queued/WP-162.md`, AGENTS.md.
- The incident report of 2026-10-08 (private folder; symbolised core:
  `IpcHandler::updateRegistration`, `ipchandler.cpp:318`, during
  `~QQmlElement<ShellRoot>`, target `jax.seldon.panel`).
- Omarchy skill `plugins.md`; the shell's `Ui/BarWidget.qml`,
  `Ui/PluginBarApi.qml`, `plugins/bar/Bar.qml` (one Bar, `Variants` per
  screen, `moduleWidgets` over all monitors; a slot unregisters itself in
  its own `Component.onDestruction`).
- Quickshell 0.3.1 `io/ipchandler.cpp` 286–334 (source from the local
  debuginfod cache): only a handler that is **not** registered looks up the
  generation's registry (`forGeneration`, line 318); a registered one uses
  its own cached pointer to let go.

## Cause, as the code shows it

`plugin/BarWidget.qml` `Component.onDestruction` of the owner calls
`reclaimIpc(root)`, which sets a sibling's `ipcOwner = true` synchronously.
During a shell exit that sibling's `IpcHandler { enabled: root.ipcOwner }`
goes from not registered to enabled inside the engine teardown → line 318
on a dying generation → SIGSEGV.

## Change

1. `plugin/BarWidget.qml`: the leaving owner only sets `ipcOwner = false`.
   The hand-over runs later, `Qt.callLater(<first sibling>.reclaimIpc)`:
   the call is the sibling's own method, so Qt drops it when the sibling
   is destroyed too (the teardown, one event), and runs it when one
   instance goes at run time (a monitor unplugged, the widget removed).
   The `leaving` parameter goes (every path is deferred now, and the bar
   no longer lists a destroyed instance by then).
2. Regression test `tests/plugin/ipc-restart.sh` + `HARNESS_IPC_KILL` in
   `tests/plugin/harness/bar.qml`: two widgets (two drawn; placeholder +
   drawn), owner created first and last, ended by `quickshell kill`.
   Checks: no widget becomes the owner after "Exiting due to IPC request",
   kill and exit status 0, no crash report in the scratch HOME, nothing of
   the shell's session left, clean log, real-home guard. Own runtime dir
   under `/tmp` per quickshell (WP-161 rule), own session (`setsid`).
   Added to `just plugin-test`.
3. Runtime proof: the existing `bar-view.sh` §4/§5 (owner dropped at run
   time → the other widget owns, IPC `open` reaches it) must stay green.
4. Corrections: `work/completed/WP-013/FINDINGS.md` §4.1 and §5.5 (crash
   unexplained; an unsymbolised trace does not clear the plugin);
   `memory/pitfalls.md` is append-only → a new WP-162 section that
   corrects the WP-013 entry.
5. Docs: TESTING.md §3d and the plugin-test row; the release step in
   VERSIONING.md "Tag flow" (live restart with two monitors, crash dir and
   `coredumpctl list quickshell` before and after); CHANGELOG 0.1.4.

## Verification

- `ipc-restart.sh` red on the old widget, green on the fix.
- `bar-view.sh` green (runtime hand-over).
- `just check` under `flock /tmp/seldon-check.lock`, `XDG_RUNTIME_DIR` a
  private dir, `JUST_TEMPDIR` in the scratch dir; `df -h /run/user/$UID`
  before; `by-id` counts before and after.
- Push, CI green.
- Not here: the live test on the dev host with two monitors (restarts the
  operator's shell) — later, with the operator's go.
