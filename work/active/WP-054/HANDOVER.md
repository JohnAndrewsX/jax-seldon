WP-054 HANDOVER

Branch `wp/054-snapper-banner`, worktree `wt/WP-054`, on main `5c79c34`.
Commits: `7782866` plugin, `34081ab` tests, `17635d4` docs, plus this
handover (with the `memory/pitfalls.md` entry). No PR, no push. Nothing
under `engine/` was touched (WP-053 runs there).

Done:
- `plugin/Model.js`: `snapperBanner(index, hinted)` has a third action
  `{ id: "capture", label: "Check again" }` after *Run in terminal* and
  *Copy*. It uses the same action id as the stale banner's *Capture now*,
  so `Service.fix("capture", "snapper")` goes to the existing
  `captureNow()`: `capture --all --json --quiet`, then `status --json`.
  No new command string, and not `recheck`, which only reloads the index.
  New constant `SNAPPER_HINT` = "When the command has finished, press
  Check again"; the banner carries it in `hint` when `hinted === true`,
  otherwise `""`.
- `plugin/Service.qml`: *Run in terminal* on the snapper banner saves the
  index text at the click (`snapperHintIndex`). `ingest()` clears it when
  an index with different content arrives, and `ingestFailure()` clears
  it too. While it is set, `snapperBanner` has the hint. A reload of
  unchanged content keeps it. `snapshot()` adds `snapperDetail`,
  `snapperActions` (`id:label`) and `snapperHint` for the harness.
  Comments about the engine probe now say "the status banner's Check
  again".
- `plugin/components/Banner.qml`: a plain-text hint line under the
  buttons, shown only when `banner.hint` is non-empty. Tokens only
  (`root.foreground`, `Style.font.bodySmall`, `root.fontFamily`).
- Tests:
  - `model.test.js`: action ids and labels, the same id as Capture now,
    no `recheck`, the hint for `true` only (not `false` or `"yes"`), and
    the same actions with or without the hint.
  - Service harness (`harness/shell.qml`): new `HARNESS_ACTIONS` entries
    `["fix", action, banner]` and `["snapshot"]`.
  - `service-states.sh`, 14b (dev mode): the three actions; the hint
    after Run in terminal; a recheck reload of the unchanged index keeps
    the hint.
  - `service-states.sh`, 14f `snapper-live`: the banner, then the hint
    after Run in terminal (no engine call). Check again gives the argv
    `--version`, capture, status, capture, status, with nothing extra. The
    fake's index after the second capture has snapper ok, so the banner
    and the hint are gone.
  - `service-states.sh`, 14f `snapper-still`: snapper still fails after
    the capture, so the banner stays with the new message and without the
    hint.
  - `panel-view.sh`, case 4: the *Check again* button renders. After
    `click:Run in terminal` the hint text is visible; it is not there
    before the click. The terminal launcher in that run is a recorder.
  - The `argv_check`/`q` helpers in `service-states.sh` moved up beside
    `record_check`; they are unchanged.
- Docs:
  - SPEC-PLUGIN §5 banner states.
  - `plugin/README.md`: the states table row, and the "engine is looked
    for" sentence now says the status banner's Check again.
  - `docs/TESTING.md`: the scenario list.
  - CHANGELOG `[Unreleased]` `### Plugin`.

SPEC-PLUGIN §5 wording (new):

> snapshots not readable (ADR-0011) → the one-line snapper fix with *Run
> in terminal*, *Copy* and *Check again* (WP-054, issue #2); *Check
> again* runs a capture, the same call as *Capture now* (`capture --all
> --json --quiet`, then `status --json`), because only a capture rewrites
> the collector state this banner reads (reloading the index would not);
> after *Run in terminal* the banner shows "When the command has
> finished, press Check again" under its buttons until the index next
> changes;

The old text said "*Copy* and *Open terminal*", but the code's label was
already *Run in terminal*. I fixed the snapper part of the spec to match
the code. The engine-missing and not-initialised parts of the same
paragraph still say *Open terminal* (the code says *Install in terminal*
/ *Run in terminal*). I left those alone because they are outside this
WP.

CHANGELOG line:

> - The "Snapshots not readable" banner has a third action, *Check again*
>   (WP-054), which runs a capture (the same call as *Capture now*), so the
>   banner clears right after the snapper fix instead of at the next
>   automatic capture; after *Run in terminal* it says "When the command
>   has finished, press Check again" (fixes #2).

Not done:
- The user guide (`docs/user/en|de/10-troubleshooting.md`) has no snapper
  banner row today, so nothing was changed there.
- No live look in the real shell, and no dev install under
  `~/.config/omarchy/plugins/jax.seldon`; the WP does not need one.
- No new screenshots: the snapper banner is not in the `docs/images`
  crops.

Verified by:
- `node tests/plugin/model.test.js` → `model.test.js: 81 passed`
- `bash tests/plugin/service-states.sh` → `service-states: 204 passed, 0 failed`
- `bash tests/plugin/panel-view.sh` → `panel-view: 692 passed, 0 failed`
- `just check` → exit 0, `check: ok`. That includes `qmllint: ok (29
  files)`, `plugin-validate: ok`, `tokens: ok (544 references)`,
  `real-home-guard.test: 11 passed`, overlay-view 319 and bar-view 120
  passed, `docs-check: ok`, and every cargo suite green. shellcheck is not
  installed on this machine, so check-packaging ran `bash -n` only
  (unchanged by this WP).
- Mutation check: with the hint clearing in `ingest()` removed,
  `snapper-still` fails (the hint stays after the new index). The code is
  restored, and the final run above used the unmodified code.
- `find plugin -type l` → 0 symlinks. No colour or font constants were
  added.
- All harness runs use a scratch HOME; the real-home guard stayed green.

Learned (appended to `memory/pitfalls.md`):
- In a live run, `recheck` also captures (the probe calls
  `captureCycle`). Tell it apart from Capture now by the argv.
- The fake's `generatedAt` has one-second precision, so two writes of the
  same fixture can be identical.
- `HARNESS_FIX` fires before the start-up index exists in a live run. Use
  `["fix", …]` in `HARNESS_ACTIONS` for banners that need the index.

Decisions needed: none. Two points for the reviewer:
- A capture that the timer starts while the terminal is still open also
  clears the hint ("until the next index change", as the WP says).
  Keeping the hint until the banner is gone would be the alternative.
- WP-053 will probably add `### Engine` under `[Unreleased]` too. Expect
  a trivial CHANGELOG merge (keep both headings).

Touched outside WP scope:
- `tests/plugin/harness/shell.qml`: two new action kinds.
- `docs/TESTING.md`: the scenario list.
- `memory/pitfalls.md`: the new entry.
- The README sentence about engine probing, reworded because "Check
  again" is now on two banners and does different things on each.
