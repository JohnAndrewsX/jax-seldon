# WP-202 handover — every desk button has a key path

Branch `wp/202-key-paths` from next 4a7a2376 (worktree `wt/WP-202`). Not
for merge before the v0.2.0 tag. Commits:

| Commit | What |
|---|---|
| d7deda0f | work: WP-202 active |
| 9c1a88f8 | plugin: the key paths (Desk, Notices, Banner, SetupCard, Section, Today, Changelog, Settings, Work, ReadingSection, Radiant, Model.js) and model.test.js |
| 0b84abf2 | tests: desk-view cases |
| 85570258 | docs: SPEC-PLUGIN §5.3–§5.6 and §8, KEYBINDINGS, plugin/README, guides 03/10 en |
| b05bf0ab | docs: guides 03/10 de, source lines at 85570258 |
| 36cceeb8 | docs: CHANGELOG `[Unreleased]` (one Highlights bullet, `### Plugin`) |

## What was done

One rule for every button the keys reach, in the pattern of WP-126c's
proposal row: the cursor reaches the button (a row above the list, or a
control in the detail), the button wears the focus ring (Omarchy's panel
cursor on a `ButtonGroup` chip), a dim line names the keys, **Enter arms,
the second Enter runs exactly what the click runs** (the same function,
so the same argv), and a detour drops the arm.

| Button | Key path | Runs (same as the click) |
|---|---|---|
| Notices: *Restart shell*, *Install*/*Update*/*Copy*/*Check again*, *Grant*, *Update rules* | `↑` at the top of any section's list and head (`Section.move` now answers false there) reaches the nearest notice with a button; `↑`/`↓` the notices, `←`/`→` (`h`/`l`) a notice's buttons; `↓` on the last notice or Esc back to the section (`Section.enterFromTop`); any other key leaves the notices first | `Notices.fix` → `Service.fix(action, banner)` |
| Setup card (Install, Create logbook, Choose a folder, Grant, Copy, Not now) | a new **setup row** in Today's list head ("Set up Seldon · N of T steps to go"), the row above the first one: `↑` on the first row (or the first `↑` with nothing selected) selects it, `←`/`→` pick a button of the current step; on a stacked desk `↑` only selects and the first Enter shows the card | `Today.setupAction` → `Service.setupAction` |
| Changelog *Agent sorts N open changes* | the row above the proposal: `↑` on the proposal (or on the first row without one, `Model.changelogAtTop`); on a stacked desk showing a detail the list comes back | `Service.askAgent("triage", "")` → `agent ask triage --json` |
| Settings › Appearance: slider, presets, sidebar; Capture: *Offer again* | Enter on the group puts the cursor on its first control (stacked: the first Enter shows the group); `↑`/`↓` the controls, `←`/`→` the value (width: a live preview, 10 % a notch or the next preset, `Model.keyWidth`; sidebar: the option); a stored value arms nothing; Esc back | `Desk.writeSetting("deskWidth"/"deskSidebar", v)`, `Service.setSetupLater(false)` |

Arm ids name what they would run: `notice:<id>:<title>:<action>`,
`setup:<step>:<banner>:<action>`, `ask:triage`,
`settings:<control>:<value>`. A notice, step or value that changes under
the cursor changes the id, so the next Enter only arms again (the
engine-missing → not-initialised switch keeps action id `terminal` but
not the title). What drops the arm besides every non-pressing key
(Desk.keyPressed, unchanged): **any mouse press on the desk**
(`Desk.pointerPressed`: a transparent `MouseArea` over the desk card that
sees every press and lets it through), a summon (`open`), a section
change, IPC `select` (also of the same place: `Desk.select` calls
`leaveKeys` first), another selection, the stacked Back, the button
going away. WP-126c's triage arms are untouched (a click on *Discard*
then *Confirm discard* still needs its arm to survive a click).

IPC, navigation only (never presses): `select notices`, `select
notice:<id>` (any section), `select setup` (Today), `select ask`
(Changelog), `select width|presets|sidebar|offer` (Settings). `view` has
`noticeKeys`, Today's `setupRow`, the Changelog's `triage.askFocused/
askArmed/askHint`, Settings' `control/sidebarPick/controlArmed/hint`.

Docs: SPEC-PLUGIN §5.3 (key table rows, a house rule "Every button has a
key path", "The notices by key"), §5.4 (setup card by key, *Agent sorts*
by key), §5.5 (Settings by key), §5.6, §8 (`select` routes); KEYBINDINGS
(rows, the arm paragraph, `select`); plugin/README (keys, IPC); guides
03 and 10 en/de; CHANGELOG `[Unreleased]`.

## How it was verified

| Check | Where | Result |
|---|---|---|
| model.test.js: `keyArmHint`, `noticeTargets` (only wired notices, title in the key), `noticeSelectTarget`, `setupTargets`/`pickAction`/`stepAction`, `todayMove`, `changelogAtTop`, `settingsControls`, `keyWidth`, the hints | fixture | 218 passed |
| desk-view, new cases `keys-notice`, `keys-notices` (+`-click`), `keys-setup` (+`-click`), `keys-setup-stacked` (+`keys-setup-copy`), `keys-setup-snap`, `keys-ask`, `keys-ask-stacked`, `keys-settings` (+`-click`): each button reached and activated by real key events; Enter, then a detour (↓, a click, a summon, a row click, another control, the stacked Back), then the next Enter only arms; the second Enter runs once; the launcher/wl-copy record, the engine argv and the facade writes compared with the click's own run | headless | 160 passed, 0 failed (trimmed runner; log `dk-202.log` in the private gates folder) |
| The same cases against the plugin of 4a7a2376 (archive, same test files) | headless | 100 failed, every case, including each record/argv/write comparison (`dk-202-old.log`, same folder) |
| Mutations: `pointerPressed` emptied → 7 failures (keys-notice #8–10, keys-setup #6, keys-setup-stacked #6, keys-settings #21, #26); the summon's `leaveKeys` removed → keys-notice #11, #13 | headless | caught |
| An earlier whole desk-view run (before the last plugin fix): every expectation that existed before passed; the only failure was `keys-ask #3` (a list row kept its highlight while *Agent sorts* had the keys), fixed before 9c1a88f8 | headless | superseded by the gate below |
| `SELDON_FULL_CHECK=1 just check` on 36cceeb8 (only this handover follows, under `work/`), log `check-dev-202.log` in the private gates folder | headless (dev host, private runtime dirs on disk), shellcheck on the test host (kit wrapper, 8 calls) | exit 0: check-packaging ok, runtime-dir 45/0, docs-check ok (514 links), plugin-validate ok, qmllint ok (52 files), model.test.js 218, terminal-scripts 73/0, service-states 367/0, desk-view 2214/0 (the new cases included), bar-view 196/0, ipc-restart 44/0, engine tests green |
| CI | CI | not run |
| Live scenarios on the test host (a, b, c, d without a pointer tool) | test host | not run |
| The desk on the dev host | desktop | not run |

Environment: `CARGO_TARGET_DIR`, `TMPDIR`, `JUST_TEMPDIR` and a private
0700 `XDG_RUNTIME_DIR` in the private gates folder (on disk),
`CARGO_BUILD_JOBS=4`; shellcheck through the kit wrapper (the only
network). Two exceptions to "never /tmp", both not mine to change in
this WP: the Quickshell harnesses (desk-view, bar-view, service-states,
ipc-restart) make their own short runtime dir with `mktemp -d
/tmp/seldon-rt.XXXXXX` (the IPC socket path must stay under 107 bytes;
removed by their EXIT trap, `check-runtime-dir` enforces that), and one
10 KB staging file of the new test block sat in the session scratchpad
(tmpfs). Nothing touched `~/Seldon`,
`~/.local/state/seldon` or `~/.config`; no guard blocked.

## Not done, and open questions

1. **Copy and Check again arm too.** One rule for every button the keys
   reach (the WP says twice where it writes or runs a command; *Copy*
   runs `wl-copy`, *Check again* a capture). `c` still captures on one
   press. If the reviewer wants one-press *Copy*/*Check again*, it is one
   condition in `Desk.pressNotice` and `Today.activate`.
2. **`Section.move` answers false for ↑ at the top** (Today, Changelog,
   Work, Decisions/System/Memory, Settings' group list, the Prime
   Radiant at scroll 0); the Graph has no list, so its ↑ goes straight
   to the notices. A held ↑ that reaches the top of a long list ends on
   a notice; that only selects (a held key's repeats never arm).
3. **A click anywhere on the desk drops the new key places.** Stricter
   than WP-126c (whose triage arm survives a click that keeps the
   selection); the triage arm is unchanged.
4. **Not in the WP's list, still pointer only:** the Changelog's *Show*
   (hidden changes), the sidebar's fold button (Settings' sidebar control
   writes the same key), the header chip and KPI figures (navigation the
   digits cover). The per-crisis key inside a proposal stays WP-201.
5. The live scenarios of the next release can now run with `wtype` only;
   the routes for them: `select notices|setup|ask|width` to place the
   cursor, then Return twice.

For the v0.2.1 acceptance limitations (review 1, Q4: accepted for
0.2.1), still pointer only: the Changelog's *Show* (changes hidden this
session), the sidebar's fold button (Settings › Appearance › Sidebar
writes the same `deskSidebar` by key), the header's status chip (fold
the notices / lead to the setup card; `1` and ↑ reach the card, ↑ unfolds
the notices) and the KPI figures (navigation the digits cover), and each
crisis's *Apply this crisis* inside a triage proposal (WP-201).

## Round 2 (review 1: SEND BACK)

Review 1's open questions 1–4: kept as they are (orchestrator).

| Finding | Fix | Verified |
|---|---|---|
| F1 IPC `select notice:<id>` / `select notices` on the notice already under the cursor kept the arm; the next Enter ran *Restart shell* | `Desk.enterNotices` drops any `notice:` arm, so every entry (↑ from a list, IPC) places the cursor unarmed | headless: `keys-notice` steps 10–13 (Enter arms, `select notice:restart`, `select notices`: not armed; the next Enter only arms); they fail on e6d7d64d (#11, #12, #13: still armed, and that Enter ran the restart there) |
| F2 a wheel notch's pending write could carry the keys' preview into shell.json; a key leave could drop the wheel's notch | `Settings.moveAcross` calls `Desk.flushWheel()` before a key preview (the wheel's own value is written first); the slider's `onMoved` sets `keyPreview = false` (the wheel's or drag's preview is not the keys' to drop) | headless: `keys-settings-wheel` (wheel 90, ← → 90 written, 80 previewed, 1.5 s later still only 90 written; ← 70 by key, a wheel notch to 70 after it, ↓ leaves: the wheel's timer writes 70); fails on e6d7d64d (#5, #6, #9, #10, #12) |
| F3 the slider by key jumped 20 % from 67 / 75 | `Model.keyWidth("width")` steps to the next notch of the grid: floor + 1 on →, ceil − 1 on ← (67 → 70, 75 → 70 / 80, 63 → 60) | fixture: model.test.js (the old 80 corrected to 70, five more cases); fails on e6d7d64d |
| F4 no visible cursor on the presets at a width that is no preset | the presets' `ButtonGroup` in an `Item` with a `FocusRing`, shown while the keys are on the presets and no chip has the cursor | headless: `keys-settings` #13 (presets at 80 %: one ring); fails on e6d7d64d |
| F5 detours without a test | `keys-settings` #4 (Enter on the stored width arms nothing: catches m4) and #7 (`select width` while armed); `keys-setup` #9–10 and `keys-setup-snap` #15–16 (`select setup` / `select offer` on the armed place, whose arm id does not change: catches m5). m3 (`Today.onSetupKeyChanged`) and m6 (`onNoticeKeyChanged`) have no step: they need an index change under the cursor in a live run, and the arm ids already carry step, banner, title and action, so without them only a stale hint would remain | headless: m4 → `keys-settings` #4 FAIL; m5 → `keys-setup` #9, #10, `keys-setup-snap` #15, #16 FAIL (with m5, *Offer again* wrote on one press); `select width` alone did not catch m5 (entering the control again drops the preview, which changes the id) |
| F6 | `Changelog.activate` guards `root.service` as the click does | qmllint |
| F7 | no private paths in this handover: "the private gates folder", "the session scratchpad" | read |

SPEC-PLUGIN §5.5 names the wheel rule, the slider's grid and the presets'
ring.

The WP-202 cases (trimmed runner, 15 runs): 180 passed, 0 failed on the
final tree; the round-2 steps against e6d7d64d: 9 failed (F1, F2, F4),
model.test.js 1 failed (F3). Where: headless (dev host, private runtime
dirs on disk), fixture.

**Gate:** `SELDON_FULL_CHECK=1 just check` exited 0 on cae706bf (only
this handover follows, under `work/`); log `check-dev-202-r2.log` in the
private gates folder: check-packaging ok (shellcheck on the test host
through the kit wrapper, 8 calls), runtime-dir 45/0, docs-check ok (514
links), plugin-validate ok, qmllint ok (52 files), model.test.js 218,
terminal-scripts 73/0, service-states 367/0, desk-view 2234/0, bar-view
196/0, ipc-restart 44/0, engine tests green. The real-home guard was
green on every harness (service-states reports the real
`~/.local/state/seldon` changed by the operator's own running engine, its
known "not a leak" case). Where: headless (dev host, private runtime dirs
on disk), shellcheck on the test host. CI, the test host's desktop and
the dev host's desktop: not run. Not merged.
