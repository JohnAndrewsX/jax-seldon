# WP-078 HANDOVER

Branch `wp/078-review`, worktree `wt/WP-078`. Commits on top of `bb1c7bd`:

- `db8b214` plugin: callWarning cuts the JSON fallback to its first line
- `a36a568` plugin: new-decision sheet shows the busy refusal like the other sheets
- `571b5e6` plugin: Changelog Capture now replaces a pending lock retry
- `ebb8129` plugin: README States table gains the Engine too old row
- `3508dfe` plugin: the IPC owner prefers a drawn bar widget over a placeholder
- `007e0e6` plugin tests: capture-gives-up checks its log
- plus the commit with this handover and the pitfalls entry

Note: the WP file is still in `work/queued/WP-078.md`; I did not move it
(the orchestrator does). This handover sits at the path the brief named.

## Done

1. **`callWarning` cuts the JSON fallback to its first line.**
   `Model.callWarning` now passes both stderr and the engine's JSON
   message through one helper, `Model.firstLine` (the first line that is
   not blank, trimmed). If the message is blank too, the warning reads
   "seldon exited with code N", as it already did with no JSON.
   `captureResult.text` (the line shown in the UI) is unchanged and keeps
   the full message.
2. **The new-decision sheet shows the busy refusal.** One Service serves
   every panel, and the bar builds one panel per monitor, so a pending
   `decide` can come from another monitor's sheet. Before, the sheet
   blocked Enter and Create on *any* pending decide, so the service's
   busy guard never answered it. Now:
   - The sheet blocks only its own pending call (`ownPending`:
     `pending && sentTitle !== ""`).
   - Any other pending decide reaches `Service.decide`, which refuses it.
     The sheet compares `busyRefusals` around its call, as NewCaseSheet
     and DriftSheet do, and shows `Model.BUSY_TEXT` in the neutral tone
     (`resultOk` follows DriftSheet).
   - The notice stays until the next Create or a change to the title.
   - The Create button's label, spinner and enabled state follow
     `ownPending`.
   - A second Create of its own pending decision is still refused
     silently, the same as in the other sheets.
3. **The Changelog's *Capture now* replaces a pending retry.** The button
   used to emit only `if (!root.capturing)`. `capturing` is also true
   while a locked capture waits for its retry, so a click did nothing.
   It now always emits, the same as the bar's right click and the `c`
   key. `Service.captureNow` still ignores a click while a capture is
   queued.
4. **README States table has an "Engine too old" row.** It has the banner
   text and *Update in terminal* (the GitHub one-liner, ADR-0024),
   *Copy* and *Check again*.
5. **The IPC owner prefers a drawn instance.**
   - New `Model.isDrawnWidget` and `Model.pickDrawnWidget` mirror the
     shell's `BarModel.isDrawnSlot` and `pickDrawnSlot`, applied to the
     widget itself (`moduleWidgets` returns widgets, not slots). Drawn
     means visible and not zero-size. A placeholder owns the target only
     when no instance is drawn.
   - BarWidget has `drawn` and `reclaimIpc()`. When an instance completes,
     is destroyed, changes `bar`, or its `drawn` changes, every instance
     recomputes. The current owner goes first, so it lets go before the
     next one enables its handler.
   - A side effect: a new instance that sorts before the owner no longer
     enables its handler for a moment while the old owner still holds the
     target. With WP-067's `claimIpc`-only start-up, it could.
6. **Harness fixes.**
   - `capture-gives-up` has its `clean_log`. It allows exactly the
     `capture exit 4: another seldon process holds the lock` warnings.
   - `work-live`: **already closed by WP-076** (`4a77d92`, `settle`
     before every `wait:` on an engine step), so I did not change it.
     Verification is below.

Docs: SPEC-PLUGIN §3 has three changed sentences (callWarning, busy
sheets, *Capture now* everywhere). The §5 Decisions sentence now covers
the busy text, and §8 the IPC owner. CHANGELOG `[Unreleased]` → Plugin
has five lines. memory/pitfalls.md has a WP-078 entry.

## Not done

- No CHANGELOG line for the harness-only item 6 (it has no user-facing
  effect).
- Nothing else is open from the WP list.

## Verified by

Each case below was run alone in a trimmed copy of its script (setup plus
that case, in the scratchpad). Every mutant was restored afterwards and
checked with `cmp`/`git diff`.

| Item | Harness case | Mutant → result |
|---|---|---|
| 1 | model.test `callWarning (WP-078)`; service-states **31b `capture-fails-json`** (new fake knobs `FAKE_SELDON_CAPTURE_STDOUT`, empty `FAKE_SELDON_CAPTURE_STDERR` = none): one warning `…capture exit 2: index unreadable: line 3`, and no "caused by" line in the log | old `first = engineError(…)` → 31b FAIL (the log shows the second line) and the model test FAILs |
| 2 | service-states **35 `sheets-busy`**, extended: a `decide` another panel sent, then Create → busy text, `resultOk` true, title kept; Enter arms with the notice kept; second Enter → busy again; the sheet's own pending call → second Create `false`, no notice, "Creating the decision…"; argv adds `decide First`, `open ADR-0005`, `decide Third`, `open ADR-0006`; the log is clean (stub `omarchy-launch-editor` in bin-base) | (a) guard back on `pending` → 6 FAIL; (b) no `busyRefusals` comparison → 5 FAIL; (c) old `resultOk` → 2 FAIL (urgent tone); (d) no own guard → 2 FAIL (`decision-own` shows the busy text) |
| 3 | panel-view **27 `capture-click`**: start-up capture locked, retry 50 s, button "Capturing"; `click:Capturing` → "nothing new", `capturing` false, "Capture now"; `locked-captures` 1, `captures` 1. `clean_log` gained an optional per-case regex, as in service-states | old `if (!root.capturing)` → 4 FAIL |
| 4 | model.test **"plugin/README.md States lists every banner with its fixes"**: every banner Model.js can produce (both index-missing variants, both mismatch directions, engineOutdated, snapper) has a States row whose Banner column has its title, and the row names each action label as `*Label*` | row removed → FAIL "States has a row whose banner is Engine too old"; *Check again* removed from the row → FAIL |
| 5 | model.test `pickDrawnWidget (WP-078)`; bar-view **5 `ipc-placeholder`** (bar.qml `HARNESS_IPC_PLACEHOLDER`): a hidden 0×0 placeholder listed first and a drawn widget → owners `false,true`, IPC `open` reaches the drawn one; a reconfiguration (placeholder drawn, other hidden) → `true,false` and `open` follows; the owner goes → the hidden last one takes over `null,true`; 0 × "another handler is registered"; log clean. bar.qml's phases are now a step list, and case 4 `ipc` still passes unchanged | (a) WP-067 rule "first live instance" → owners `true,false`, 2 FAIL; (b) no `onDrawnChanged` reclaim → 4 FAIL; (c) owner looks again *last* → "one handler = 1" + log FAIL |
| 6 | service-states 33 `capture-gives-up` `clean_log` | an extra `console.warn` on give-up → only `clean_log` FAILs; an async TypeError (`Qt.callLater`) on give-up → only `clean_log` FAILs |
| 6 (work-live) | the pitfalls reproducer: a temporary `sleep 1` after `rewrite_index` in `plan_step` of fake-seldon (not committed, restored) | current steps: **67 passed, 0 failed**; the pre-WP-076 steps (`4a77d92^`): 8 FAIL, e.g. `#29 .view.work.result = Moving to verification: C-2026-005…`. So WP-076's `settle` closes the flake |

On every commit: `omarchy plugin validate plugin/` ok, `just qmllint` ok
(29 files, tokens ok), `just docs-check` ok, `node tests/plugin/model.test.js` green (87 at the end).

`just check` (run once at the end, no other plugin harness running, load
about 2): **exit 0** on the first run, no transient and no re-run.
It covers fmt, clippy, the engine tests, schema/fixtures, docs-check,
plugin-validate, qmllint (29 files, tokens ok), model.test, install.test
132/0, real-home-guard.test 11/0, service-states 269/0, panel-view
743/0, overlay-view 319/0 and bar-view 143/0.

## Open questions

- **The user guides lack the "Engine too old" row too.**
  `docs/user/{en,de}/10-troubleshooting.md` have a states table without
  it. They were not in this WP's inputs, so I did not change them. This
  is a small docs follow-up (en and de together).
- **panel-view has other `wait:` steps without `settle`**:
  `wait:drift.isOpen=false` after Return (4 times). They wait on a sheet
  state that the result sets, not on the index, so the WP-076 race does
  not apply as far as I can tell. They were not changed or tested with
  the reproducer.
- The new-decision sheet's busy notice stays after the other panel's
  decision is created, until the next Create or a title change. That
  matches the new-case sheet (WP-068). Say so if it should clear when
  that call answers.

## Touched outside scope

- `tests/plugin/fake-seldon`: the `FAKE_SELDON_CAPTURE_STDOUT` knob, and
  a set-but-empty `FAKE_SELDON_CAPTURE_STDERR` now writes nothing. The
  existing callers pass a value, so they are unaffected.
- `tests/plugin/harness/bar.qml`: the placeholder mode, and the IPC
  phases restructured as a step list.
- Plugin `Model.js` functions are appended at the end.
- Nothing outside the repository. No `~/.config` and no real logbook were
  touched; the real-home guard is part of every harness script.
