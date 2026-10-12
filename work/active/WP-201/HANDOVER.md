# WP-201 handover — a crisis inside a triage proposal by key

Branch `wp/201-crisis-by-key` from next 1e872c28 (worktree `wt/WP-201`).
Not merged. Commits:

| Commit | What |
|---|---|
| 21f84c63 | work: WP-201 active |
| 79c02f69 | plugin: Model.js (`crisisMove`, `crisisKey`, `crisisArmHint`, `crisisHint`, `proposalHint` with crises), TriageDetail.qml, Changelog.qml |
| 0f1a05c2 | tests: model.test.js, desk-view cases `triage-crisis-keys`, `triage-back-apply`, `triage-back-crisis`, triage-keys updated; fake-seldon `FAKE_SELDON_FIXTURE_BACK` |
| dadf6b69 | docs: SPEC-PLUGIN §5.3/§5.4, KEYBINDINGS, plugin/README, guide 03 en, TESTING, CHANGELOG `[Unreleased]` |
| 116b3bbd | docs: guide 03 de, source line at dadf6b69 |
| 43ae8b3e | tests: `triage-crisis-stacked` |

## What was done

Inside the shown proposal the crises are key stops between the bar and
the list's first row (`Model.crisisMove`): ↓ from the bar reaches the
first crisis, ↓/↑ walk them, ↑ from the first goes back onto the bar
(*Apply proposals* again), ↓ from the last goes to the list (an empty
list keeps it). Only while the proposal's detail is shown: on a stacked
desk showing the list, ↓ on the proposal row still goes back to the list
(WP-126c F1 unchanged); after Enter shows the detail, ↓ walks the crises.

- The crisis under the cursor: its *Apply this crisis* button wears the
  panel cursor (`hasCursor`) and the `FocusRing`, and is scrolled into
  view.
- Enter arms it, arm id `triage:crisis:<proposal>:<eventId>`
  (`Model.crisisKey`; the `triage:` prefix, so `Changelog.disarmTriage`
  covers it), bar hint "Apply the crisis <subject> as you? Press Enter
  again. Only this item; the other crises stay." The second Enter calls
  `TriageDetail.applyCrisis(eventId)`, which the button's click now calls
  too: `Service.applyProposal(seenId, eventId)` → `drift apply <id>
  --item <eventId> --json`, one item per run (ADR-0028 §3, ADR-0036).
  Enter on a crisis that is no longer open does nothing. Enter never
  starts an agent (nothing here launches one).
- What drops the arm and the cursor's place (SPEC-PLUGIN §5.3, WP-126c
  round 3): every key that does not press (the desk), a mouse press on
  the desk (`Changelog.leaveKeys`, from `Desk.pointerPressed`), IPC
  `select` (`Desk.select` → `leaveKeys`), another selection or leaving
  the proposal (`disarmTriage`, `onShownChanged`), the stacked Back,
  another proposal id (`onSeenIdChanged`), a replaced proposal
  (`onCurrentChanged` now drops every `triage:` arm and the cursor, and
  `crisisArmed` requires `current`).
- The hint lines name the keys: under the proposal row
  (`Model.proposalHint`) "Enter twice applies · ↓ its crises, then the
  list", on a crisis "Enter twice applies the crisis under the cursor ·
  ↑/↓ the crises"; under CRISES — EACH ON ITS OWN a dim line
  (`Model.crisisHint`) "↓ from the proposal reaches each crisis · Enter
  twice applies the one under the cursor" / "Enter twice applies this
  crisis alone · ↑/↓ the other crises".
- `view()` of the triage detail gains `crisisAt`, `crisisArmed`,
  `crisisHint`.

Behaviour change to note: on the wide desk ↓ on a shown proposal with
crises now reaches the first crisis instead of the list (triage-keys #3,
#11 updated; ↓ again reaches the list).

**Also (round 2 M1, A→B→A):** `triage-back-apply` and
`triage-back-crisis`: A armed (Apply by Enter, or a crisis), a capture
brings B (A replaced), a second capture brings A back
(`FAKE_SELDON_FIXTURE_BACK`, new in fake-seldon: the index after the
third capture, standing in for "B discarded elsewhere, the engine names
A again"); no arm comes back, the cursor is on the bar, the next Enter
only arms *Apply proposals*, nothing is applied.

**Also (stage 2):** the per-crisis arm follows the Changelog's rule: a
row click, another selection, leaving the proposal or a replaced
proposal disarms it (prefix `triage:`), plus WP-202's pointer press.

Docs: SPEC-PLUGIN §5.3 (↑/↓ and Enter rows), §5.4 (the row's hints, "A
crisis by key"), KEYBINDINGS, plugin/README, guide 03 en/de, TESTING,
CHANGELOG `[Unreleased]` (Highlights bullet extended, `### Plugin`
entry).

## How it was verified

| Check | Where | Result |
|---|---|---|
| model.test.js "WP-201": `crisisMove` (bar, walk, leave, empty list, no crises, stale cursor), `crisisKey`, hints, a key run over the model with two crises (↓ ↓ Enter Enter → exactly `drift apply <id> --item <UNIT> --json`, the other crisis never named; a move between the presses only re-arms; a replaced proposal never applies) | fixture | 219 passed |
| desk-view `triage-crisis-keys` (two crises, 1920×1080): ↓ walks, ring and hints, Enter arms with the hint on screen, second Enter applies `--item UNIT` alone, THEME's crisis stays open with no outcome, ↓ from the last crisis to the list, ↑ back to the bar; a desk press, ↑ and IPC `select proposal` each drop the arm and cursor; argv exactly startup + one `--item` apply | headless | pass |
| desk-view `triage-crisis-stacked` (700×900): ↑ only selects, Enter shows the detail, ↓ the crisis, Enter twice applies it alone, Back drops the cursor | headless | pass |
| desk-view `triage-back-apply`, `triage-back-crisis` (A→B→A) | headless | pass |
| The triage cases against the plugin of 1e872c28 (same tests) | headless | 31 failed: triage-crisis-keys fails throughout, its argv lacks the `--item` apply; the A→B→A cases fail only on the missing `crisisArmed` field (the old code already dropped the arm there, as the WP says; the case pins it) |
| Mutation: `onCurrentChanged` without the disarm | headless | caught: triage-back-apply #5, #8, #10 and argv (one Enter applied A after it came back) |
| `SELDON_FULL_CHECK=1 just check` on 43ae8b3e (only this handover follows, under `work/`), log `check-dev-201.log` in the private gates folder | headless (dev host, private runtime dirs), shellcheck on the test host (kit wrapper) | exit 0: check-packaging ok, runtime-dir 45/0, docs-check ok (514 links), plugin-validate ok, qmllint ok (52 files), model.test.js 219, terminal-scripts 73/0, service-states 366/0, desk-view 2283/0 (the new cases included), bar-view 195/0, ipc-restart 43/0, engine tests green |
| shellcheck of the two changed shell files (`tests/plugin/desk-view.sh`, `tests/plugin/fake-seldon`; the gate shellchecks only install, deploy and guard scripts) | test host (kit wrapper) | no finding on a changed line; 70 findings elsewhere in both files, all from before this WP (SC2016, SC2088, SC1091, …), untouched |
| CI | CI | not run |
| Live scenario c (crisis by `wtype`) | test host | not run |
| The desk on the dev host | desktop | not run |

Environment: `CARGO_TARGET_DIR`, `TMPDIR`, `JUST_TEMPDIR` in the private
gates folder (on disk); every harness run its own `HOME`; the Quickshell
harnesses make their own 0700 runtime dir with `mktemp -d
/tmp/seldon-rt.XXXXXX` (socket path length; removed by their EXIT trap),
not mine to change. Iteration ran trimmed copies of desk-view.sh (the
triage block only) from `gates/dev-201/`. Nothing touched `~/Seldon`,
`~/.local/state/seldon`, `~/.config` or the installed plugin; no shell
restart.

## Not done / open questions

1. A pointer press on the desk drops the crisis arm, but not an *Apply
   proposals* arm made by Enter (WP-126c kept every `triage:` arm across
   a click, because *Discard* is armed and confirmed by clicks). Only a
   selection change drops that one. Should a press drop the key-made
   Apply arm too (exclude only `triage:discard:`)? Not changed here.
2. Scenario c on the test host can now apply the crisis with `wtype`
   (↑, ↓, Return, Return on a wide desk; a stacked desk needs one more
   Return first). The protocol wording from WP-126c stage 2 F3 ("crisis
   resolved by the user from its row") can go back to "the crisis
   applied alone". Not run.
3. Space confirms like Enter (§5.3 "Enter, Space"), as for every arm.

## Round 2 (stage 1 and stage 2: APPROVE WITH FIXES; Fable's I1–I8)

| Commit | What |
|---|---|
| 68bfc2e1 | plugin: TriageDetail.qml, Changelog.qml, Model.js (I1–I4, I6, I7) |
| 63d97cf2 | tests: model.test.js; desk-view `triage-crisis-big`, `triage-crisis-rewrite`, `triage-press-apply`, `triage-crisis-keys` extended, the walk waits for the build in `triage-keys`, `triage-crisis-stacked`, `triage-back-crisis`; fake-seldon `FAKE_SELDON_PROPOSAL_AFTER` |
| 2a137222 | docs: SPEC-PLUGIN §5.4, CHANGELOG `[Unreleased]`, TESTING |
| 8d3f216b | tests: `triage-crisis-big` split; the walk runs on a first build (`triage-crisis-big-walk`) |

| Instruction | Done | Verified |
|---|---|---|
| **I1** (blocking) gate the crises on the built items | `crisisCount` is 0 until the items' Loader is `Ready`, so until then ↓ on the shown proposal goes to the list and the row's line says "↓ back to the list" (`proposalHint` reads `crisisCount`); a delegate created already keyed (a rebuild after an index reload) registers its button and reveals itself (`Component.onCompleted`, plus `onYChanged`/`onHeightChanged` while keyed, since the Column positions it after creation). SPEC §5.4 says so in one clause | headless `triage-crisis-big` (live, the 200 × 10 proposal with crises at items 0, 1 and 199): ↑ ↓ Enter Enter with `built=false` → ↓ went to the list, `crisisAt=-1`, `arm.armed=""`, no `--item` (argv is the startup's only); on b9e3f57f its argv has `drift apply … --item 01M3VNJ9… --json`. `triage-crisis-big-walk` (same proposal, its own run): after `wait:…built=true` ↓ ↓ ↓ ↑ ↑ walk crisis 0, 1, 2, 1, 0 |
| **I2** reveal the button, prove it | `reveal(item, button)`: the whole item when it fits the pane, else the button's end at the pane's bottom, a scroll only when not in view; `view` has `crisisVisible` (the keyed button's rect inside `[contentY, contentY + height]`, false off a crisis) and `scroll` | headless: `crisisVisible=true` after every ↓/↑ landing on a crisis in `triage-crisis-keys` (#4, #5, #10, #14), `triage-crisis-stacked` #5 and `triage-crisis-big-walk` #4–#8. The big cases run at 1920×600 so each crisis (≈ 560 px) is taller than the pane (419 px): the button-end branch. Mutation: the round-1 top-aligned `reveal` → walk #7, #8 fail (↑ hides the button) |
| **I3** the arm id names what it would run | `Model.crisisKey(proposalId, item)` = `triage:crisis:<id>:<eventId>:` + action text (action and target), intent and evidence labels (kind and ref), joined by `|`; not the evidence text. `pressCrisis` and `crisisArmed` build it from the live item. The arm hint names the action too: "Apply the crisis <subject> (<Link to C-… / Explain: <title>>) as you? …", so a rewritten item asks again in its new words. Residual window (second Enter → engine read) as the click's, on screen, contract to close; SPEC says so | fixture: model.test.js pins that the key changes with the action and its target (explain → link, another case), the title, the intent, a ref, an added ref, and not with the evidence text. headless `triage-crisis-rewrite`: a crisis armed, a capture without a key makes the fake engine rewrite the item under the same proposal id (explain → `link` to C-2026-004, `FAKE_SELDON_PROPOSAL_AFTER`), the view shows "Link to C-2026-004", the old arm no longer matches (`crisisArmed=false`, the old id still held), Enter → armed with the new hint, Enter → applied; argv: startup, capture, status, one `--item`. On b9e3f57f #8 and #9 fail (the first Enter after the rewrite applied) |
| **I4** a key-made *Apply proposals* arm drops on a press | `Changelog.leaveKeys` also disarms `triage:apply:`; `triage:discard:` stays (made and confirmed by clicks). SPEC §5.4's arm paragraph says why | headless `triage-press-apply`: ↑ Enter (`applyArmed=true`), a click on the proposal row → `applyArmed=false`, `arm.armed=""`, Enter → armed only, argv the startup's only; fails on 1e872c28 and b9e3f57f (both send `drift apply <id> --json` on the next Enter). Also `triage-crisis-keys` #17/#18. Discard by two clicks still works (`triage-ask`, `triage-detour`) |
| **I5** Enter on a closed crisis does nothing | test only | headless `triage-crisis-keys` #9/#10 (after the crisis was applied): `arm.armed=""`, `applyArmed=false`, `crisisArmed=false`, argv unchanged. Stage 1's mutant m4 (`if (crisisAt >= 0 && pressCrisis()) return true`) now fails #10 |
| **I6** closed crises stay stops, honest hint | `Model.crisisState(item, canApply)`: "" / open / closed / blocked; `crisisHint` and `proposalHint` say "Resolved — nothing to apply · ↑/↓ the (other) crises" on a closed one and "<write blocker, else A run is under way> — cannot apply now · …" while nothing can be applied; the ring stays | fixture: model.test.js pins every variant of both; headless: `triage-crisis-keys` #10 both lines on screen, `triage-crisis-big-walk` #6 on a crisis that was never open |
| **I7** bind *Apply proposals* too | `applyKey` = `Model.triageApplyKey(id, view)`: `triage:apply:<id>:<open count>:<open regular eventIds>` | fixture: model.test.js (the key, and a different key when an open item leaves the proposal). Headless: the existing Apply cases (`triage-keys`, `-stacked`, `-detour`, `-swap-apply`, `-back-apply`) still arm and apply as before |
| **I8** handover honesty | below | — |

### Live scenario c (I8)

Not run in this WP (test host). It is a gate for the 0.2.1 acceptance
record, not for this merge (stage 2, Q5). Proposed wording for the
crisis step of the live protocol (the protocol is in the private review
folder, read-only for me): "The crisis applied alone, by key: ↑ from the
first Changelog row to the proposal, wait until its items show, ↓ to the
crisis, Return, Return (the bar asks "Apply the crisis … as you?" after
the first). On a stacked desk one Return first shows the detail. Pass:
the engine's answer names only that item done; the other crises and the
unevidenced change stay open." What that run must show and headless
cannot: wtype key delivery through the overlay's exclusive focus, the
real engine's `--item` answer on a real proposal, the ring and the
scroll on a real screen. No guard block occurred in this WP.

### How round 2 was verified

| Check | Where | Result |
|---|---|---|
| model.test.js (I3 key, I6 hints and states, I7 apply key) | fixture | 219 passed |
| The triage block of desk-view (trimmed runner in the private gates folder) | headless | 187 passed, 0 failed (twice) |
| The new and changed cases on the plugin of b9e3f57f / 1e872c28 (before the big case's split) | headless | every new case fails on b9e3f57f (28 failures; `triage-crisis-big` with a `--item` apply in its argv, `triage-press-apply` with a whole-proposal apply); `triage-press-apply` also fails on 1e872c28 |
| Mutations: the round-1 top-aligned `reveal`; stage 1's m4 | headless | caught (walk #7/#8; keys #10) |
| `triage-big` and `triage-crisis-big-walk`, 4 runs each, under the shared lock (`flock /tmp/seldon-check.lock`), this branch and 1e872c28 | headless | the first build of the 200 × 10 proposal finished within the harness's 15 s wait in every run: branch 4/4 for both cases (load 9.2 → 3.8), base `triage-big` 4/4 (load 3.0 → 1.5; the base has no crisis walk) |
| `SELDON_FULL_CHECK=1 just check` under `flock /tmp/seldon-check.lock` on 8d3f216b (only this handover follows, under `work/`), log `check-dev-201-r2d.log` in the private gates folder | headless (dev host), shellcheck on the test host (kit wrapper) | exit 0 (load about 5 when it started): check-packaging ok (shellcheck on the test host through the kit wrapper), engine tests green (112 `test result: ok`), runtime-dir 45/0, docs-check ok (514 links), plugin-validate ok, qmllint ok (52 files), model.test.js 219, terminal-scripts 73/0, service-states 366/0, desk-view 2313/0 (`aside`, `triage-big` and the new cases included), bar-view 195/0, ipc-restart 43/0 |
| shellcheck of the changed lines of `tests/plugin/desk-view.sh` and `tests/plugin/fake-seldon` | test host (kit wrapper) | no finding on a changed line (70 findings elsewhere in both files, from before this WP) |
| CI | CI | not run |
| Live scenario c | test host | not run |
| The desk on the dev host | desktop | not run |

**Earlier gates of this round, which failed on timing (kept for the record).**

- `check-dev-201-r2.log` (2a137222, before the split): 9 failures, all in
  the then single `triage-crisis-big`. After ↑ ↓ (the pane left while its
  200 × 10 items were still incubating) and ↑ again, the rebuild did not
  finish within the 15 s wait. Measured at low load with one report a
  second, that rebuild takes about 3–4 s offscreen, **the same on
  1e872c28** (3 runs each); it is not a stall, the build finishes. I
  tried keeping the Loader on until a started build is done: worse (a
  hidden Loader does not progress, so `triage-big` ended with 195 items
  still incubating at quit, 6 of 6 runs), and not in the branch. The
  test changed instead: the walk runs on a first build, in its own run
  (8d3f216b). Whether a 3–4 s rebuild matters on a real screen is a
  question for the R2 owner (a follow-up, not this WP).
- `check-dev-201-r2b.log` (8d3f216b, load average about 30 from parallel
  gates): `aside #1` (a Work case this WP does not touch) and
  `triage-big #4` (the first build over 15 s).
- `check-dev-201-r2c.log` (8d3f216b, load 11.5 at the start, about 16
  during desk-view): `aside #1` passed, `triage-big #4` and
  `triage-crisis-big-walk #3` (the same first-build wait; #4–#8 follow
  from it) failed, argv checks passing. The locked comparison above
  then showed both versions building in time without parallel gates,
  so the cause was the parallel load. From then on every full check
  ran under the shared lock (orchestrator's rule, 2026-10-12).

A cold-start miss of the first harness case of a run (the index not read
by step 1) also showed once in a trimmed run at low load; it predates
this WP.

The round-1 open question 1 (a press and the key-made Apply arm) is I4,
done. Questions 2 and 3 stand as written (2 is I8's wording above).
