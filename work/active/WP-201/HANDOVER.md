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
