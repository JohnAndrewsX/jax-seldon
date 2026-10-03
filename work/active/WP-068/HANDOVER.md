WP-068 HANDOVER

Branch `wp/068-review` (worktree `wt/WP-068`), commits on top of `3f03693`:
`770dd46` plugin + tests, `5c7df1c` docs, then memory and this handover.

Done:
- **F-456** — `Service.warnFailure()` → `Model.callWarning()`: every engine
  call (probe included) that exits above 0 logs one `console.warn`
  `jax.seldon: seldon <command> exit <code>: <first non-empty stderr
  line>`; with an empty stderr (the engine's `--json` errors go to stdout)
  the JSON error message stands in.
- **F-551** — exit 4 of `capture`/`status` → `Service.retryLater()`: run
  again after 30 s (`Model.LOCK_RETRY_MS`), at most 3 times
  (`Model.LOCK_RETRIES`); a locked capture takes its queued `status` along
  (`captureNow` re-queues both). Meanwhile `captureResult` is
  `{ ok: true, text: "waiting for another seldon process; trying again
  shortly" }` (neutral in the Changelog), `lastError` is not touched and
  `capturing` stays true (button shows Capturing). After the third retry
  the exit is an error as before and the queued status runs. An explicit
  Capture now replaces a pending retry. Dev override
  `SELDON_LOCK_RETRY_MS` (harness only, documented in Service.qml's header).
- **F-553** — the guards of `plan`, `startAgent`, `drift` (and `decide`,
  same pattern, one line) call `refuseBusy()`: return false, set
  `busyRefusal = { family, action, caseId, eventId, text: Model.BUSY_TEXT }`
  and count `busyRefusals`. The pending result line is *not* overwritten
  (that would have released the guard). NewCaseSheet and DriftSheet
  compare `busyRefusals` around their own call and show
  "Another action is running — try again in a moment" in the neutral tone
  (new-case: until the next Create; drift: until a form change / next run).
  The new-decision sheet was not changed (not in the file list); it gets
  the same `busyRefusal` from the service if wanted later.
- **F-555** — DriftSheet: `onFormChanged` → `onFormKeyChanged` on
  `formKey: JSON.stringify(form)`. A reload with the same item no longer
  clears `armedSig` or `notice`; a real edit (or a changed item) still does.
- **F-457** — `Model.engineMinOf(manifest)`, `versionCore`/`versionBelow`
  (numeric major.minor.patch; a pre-release suffix counts as its version),
  `Model.engineOutdatedBanner()`: "Engine too old" / "… Update the engine
  to at least X …", installer command, actions Update in terminal, Copy,
  Check again. Service: `engineMin` from the injected `manifest`; the
  banner takes precedence over every status banner but engineMissing and
  contractMismatch; `probeDone` also logs one warning when outdated.
- All new Model.js code is appended at the end (coordination with WP-067).
- docs/SPEC-PLUGIN.md §3 (four bullets) and §5 banner row; CHANGELOG
  `[Unreleased]` → Plugin (five bullets, appended); memory/pitfalls.md.

Not done:
- No harness run of the engineMin banner through Service.qml: the
  harness creates the service without the shell's `manifest` (as before),
  and tests/plugin/harness/shell.qml is not in this WP's files. Covered by
  model.test.js only, as the WP asks.
- See "Decisions needed": panel-view.sh.

Verified by:
- `node tests/plugin/model.test.js` → `84 passed` (new: engineMin banner
  below/equal/above, numeric 0.9 < 0.10, pre-release, unknown versions,
  precedence; callWarning; constants).
- `bash tests/plugin/service-states.sh` → `247 passed, 0 failed`. New
  scenarios 31–35:
  31 capture-fails (fake exit 2, two stderr lines → exactly one warning
  with code and first line), 32 capture-locked (2 locked captures →
  snapshot during the wait shows the neutral text, no lastError,
  capturing true; argv `capture ×3, status`; result "nothing new"),
  33 capture-gives-up (argv `capture ×4, status`, lock error as result,
  4 warnings), 34 busy (second plan / drift / decide refused with
  busyRefusal; pending lines stay pending), 35 a small sheet harness
  (written by the script next to copies of the shell's Commons/Ui) —
  sheets-busy (both sheets show the busy text, neutral, still after the
  other calls finish; no extra engine call) and sheets-rearm (notice and
  arm survive two engine index rewrites; typing clears the notice; the
  second Enter after the rewrite runs `drift dismiss … -- because`).
  Real-home guard: ok.
- fake-seldon: `FAKE_SELDON_CAPTURE_EXIT` / `_STDERR`,
  `FAKE_SELDON_CAPTURE_LOCKED=N`.
- Mutants (trimmed copy of service-states.sh with scenarios 31–35, and
  model.test.js), each restored with `git checkout` afterwards:
  - F-456 drop `warnFailure` in runnerDone → 3 failed (capture-fails,
    capture-locked, capture-gives-up warnings).
  - F-551 `if (false)` instead of `retryLater` → 7 failed (waiting
    snapshot shows the lock error, argv, result, warnings).
  - F-553 `refuseBusy` returns false without setting anything → 8 failed
    (busy refusals; both sheets show nothing).
  - F-555 `onFormKeyChanged` back to `onFormChanged` → 6 failed
    (notice and arm lost after the rewrite, second Enter does not run,
    the drift busy notice is cleared by the reload).
  - F-457 `engineOutdatedBanner` always null → model test FAIL; string
    instead of numeric comparison → model test FAIL.
- `just qmllint` ok (29 files), `plugin-validate` ok, `docs-check` ok.
- `just check` → exit 1: everything passes (fmt, clippy, engine tests,
  watch, packaging, install, schema, docs, plugin-validate, qmllint,
  model.test, real-home-guard, service-states 247/0) up to panel-view.sh
  (687 passed, 6 failed, all six `log has errors` on the new expected
  warning line). See Decisions needed.

Learned (memory/pitfalls.md): a `var` property notifies on every new
object (key real changes on a string); expected call warnings must be
named in `clean_log`; the harness settles during a retry wait unless
`capturing` covers it; sheets can be driven from service-states.sh.

Decisions needed:
- **panel-view.sh (WP-067's file).** The F-456 warnings are logged in six
  panel-view cases that make the fake engine refuse on purpose (refuse,
  work-live, work-agent, work-locked, drift-locked, decisions-locked);
  their `clean_log` flags them, so `just check` → plugin-test exits 1
  (`panel-view: 687 passed, 6 failed`; everything before it green).
  Proposed fix (one filter in panel-view.sh `clean_log`):
  `grep -a -v -E 'jax\.seldon: seldon [a-z-]+ exit [0-9]+: '`. I asked
  the orchestrator session (jax-seldon-8c); the message is held for the
  operator's approval, so the file is **unchanged** and `just check`
  currently exits **1** for this reason only. Either approve the edit in
  this WP, or let WP-067 or the merge apply it. Other option: name each
  expected warning per case, as service-states.sh does.

Touched outside WP scope: none (tests/plugin/service-states.sh writes a
sheet harness into its temp dir at run time; no new repository file).

Addendum (after the orchestrator's decision to edit panel-view.sh):
- `b07bc29` tests: panel-view accepts the engine-call warnings (WP-068).
  One `expected_warnings` pattern above `clean_log` and one more
  `grep -v` in it. The pattern names exactly the six warnings that the
  cases cause on purpose: `log exit 1: unknown case C-2026-004`,
  `plan exit 1: C-2026-008 is active; …`, `agent exit 1: C-2026-004 is
  queued; …`, and `plan|drift|decide exit 4: the logbook is locked by
  another seldon (pid 4242)`. Checked against the six lines of the failed
  run: all 6 match. Two near misses (another case id, a `capture exit 4`)
  do not match. WP-067 only adds lines near the end of the file (around
  line 1041), and its new cases make only start-up calls, so no new
  warnings come in with that merge.
- `just check` (run once) → **exit 1**. Everything is green up to
  plugin-test: model.test 84 passed, real-home-guard 11/0,
  service-states 247/0. panel-view: 692 passed, 1 failed:
  `work-live #32: .view.work.result = Completing C-2026-005…` (the
  pending text). This is the known transient result-line flake of
  `work-live` under load (memory/pitfalls.md, WP-063). That step passed
  in the first `just check` run, and WP-068 does not change when a plan
  result arrives. To classify it I re-ran only `bash
  tests/plugin/panel-view.sh` once: **693 passed, 0 failed, exit 0**,
  real-home guard ok.

