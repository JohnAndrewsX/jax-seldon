# WP-232 handover — the app's Plan view (cases)

Branch `wp/232-app-plan` (off `app`). Not pushed.

## What was done

- **Two views.** The header has the view tabs *Changes* and *Plan*
  (underlined tabs); `1` and `2` switch (off while typing). Both views stay
  loaded, so drafts survive a view change; a view change disarms what was
  armed (Changes too). The `?` overlay lists the shown view's keys, then
  the window's (`1 2`, `?`, `Q`); it scrolls (wheel, ↑/↓) when the window is
  shorter than the list. `Q` asks first while a Plan draft (the sentence,
  the New case form) is unsent.
- **`PlanView.qml`** (Model.js `deskWork`, `workView`, `wipStatus`,
  `caseDetail`, `withSession`, `caseDeskActions`, `caseEnterAction`,
  `emptyState`, `planArgs`, `agentArgs`, `agentNewArgs`, `agentFocusArgs`,
  `planResult`, `agentResult`, `focusResult`, `openResult`,
  `sessionsResult`, all unchanged): the one-sentence start (intent field
  and *Run*), the WIP line (warns at and over the limit, never blocks;
  default limit 3, there is no setting in the app), *By agent*, *New case*,
  the search (`/`), the groups Active · Verification · Queued · Completed
  with their headers (`ACTIVE · 2`, `COMPLETED · 1 / 2`), the cut-index
  gaps (ADR-0053, header, line and `seldon plan list --status …`), the
  engine's answer line. The empty list says what belongs there and offers
  *New case*; a search without a match says so and offers *Clear search*.
- **`CaseDetail.qml`**: heading, title, "completed by agent" / "reopens …",
  the bar by status (Start · Hand to agent or Focus · To verification ·
  Complete · Drop · Reopen · Open in editor), the armed act's question,
  key/values, Intent and Result (first paragraphs), the plan's progress,
  the log and the linked changes — from the index only.
- **`NewCaseForm.qml`**: title, zone, risk, priority (choices: Tab to them,
  ←/→ or h/l), area; Create; Close keeps the draft.
- **`CaseRow.qml`, `GroupGap.qml`, `KeyValueRow.qml`**: the row (active
  stripe with its word, "agent working · 4/5"), the gap, a log line.
- **Arming (the WP-228 lessons).** Every act that writes is armed by the
  first press and run by the second: Start, To verification, Complete,
  Drop, Hand to agent, Reopen, New case, Run. The first press saves the
  argv; `built` (what the second press would run now, from the selected
  case or the form or the sentence) disarms when it differs — another
  case, a new index that moves the case on, a changed field. The second
  press runs only an argv equal to the armed one (a second guard). Focus
  and Open in editor write nothing and run at once. Enter and Space take
  the case's first act that launches nothing (Enter never starts an agent;
  `a` or a click does). One call at a time for the view; its answer is
  matched to the argv that ran (a capture that answers first does not end
  it; another selection keeps it). A refusal keeps what was typed: a blank
  title and a bad area are refused before the engine, the engine's
  refusals (lock held, exit 4; "no default agent") show beside the form or
  the sentence, which keep their text. Everything from the index or the
  engine is `Text.PlainText`.
- **One agent per case.** While Plan shows, `agent sessions --json` runs in
  its own process beside the queue (`Backend::askSessions`, 10 s timeout,
  one at a time, never in dev mode or before the engine answered), on
  show, on every new index, after every agent answer and every 15 s. An
  active case with an open agent window shows *Focus* and "agent working".
- **Backend.** `validateArgs` accepts exactly CONTRACT.md's forms the view
  uses: `plan new --zone <z> --risk <r> [--area <a>] [--priority <p>]
  --json -- <title>`, `plan start|verify|done|drop|reopen <caseId> --json`,
  `agent start <caseId> --json`, `agent start --new --json -- <intent>`,
  `agent focus <caseId> --json`, `agent sessions --json`, and `edit
  <caseId> --editor --json` (cases only). Nothing else of `plan`, `agent`
  or `edit` (no `--again`, no `plan show`, no `agent ask`, no `open`).
- **Catalogs.** Every literal `qsTr` and every Model.js word the view passes
  through the catalog (action labels, statuses, zones, priorities, the
  detail's labels and its status/zone/priority values) has its German; one
  plural ("%n older change(s) …") in `en.json` and `de.json`.
- `docs/images/app-plan.png` (new) and `app-changes.png` (refreshed: the
  header now has the tabs): fixture, dev mode, default accent, empty theme.
- `app/README.md`: the Plan view, a keys table per view, the screenshot.

## Review round 1 (SEND BACK) — what changed

1. **B1, a held key never confirms** (SPEC-PLUGIN §5.3, WP-173), in both
   views since `Main.qml` is shared: every Shortcut that presses has
   `autoRepeat: false` (Return/Enter, Space, `a x r e i n b`, `/`, `1`,
   `2`, `?`, `Q`); only moving keys (↑/↓, j/k, f/F) repeat. No field acts on
   `accepted`: the New case title and area, the sentence field and
   Changes' reason field press from `Keys.onReturnPressed`/`onEnterPressed`
   only when `!event.isAutoRepeat` (the search field moves focus the same
   way). Tests: the reviewer's `reviewAutoRepeatDrop` and
   `reviewAutoRepeatEnterCreate`, plus `planHeldKeyNeverConfirms` (Space,
   `r`, Return in the sentence) and `changesHeldKeyNeverConfirms` (Return
   held on the case picker and in the reason field; a second press of its
   own still runs).
2. **B2a, no case keys behind the form.** While the New case form is open,
   `a x r e` do nothing (`PlanView.key`). Test `reviewKeysBehindTheForm`.
3. **B2b, no act under an ask.** `win.asking` (the `?` overlay or the quit
   question) turns off Enter, Space and every view key. Tests
   `reviewEnterUnderHelp`, `noActWhileTheQuitAskShows`.
4. **N1, an arriving index never closes a form.** A first index with cases
   selects its first case without touching `formOpen`; a form that closes
   gives its keys back to the list (no hidden field keeps typing). The
   Plan fields are read-only, not disabled, while a call runs, so they keep
   the keys through it. Tests `reviewFormClosesOnFirstIndex`,
   `reviewFocusLostAfterRefusal`, and `refusalsKeepTheTitle` now checks
   that the area field and the sentence field keep the focus after a
   refusal.
5. **N2, the missing tests.** `reviewViewSwitchDisarms` (Plan, outside dev
   mode) and `changesViewSwitchDisarms` (Changes: an armed link is
   disarmed by a view change, and the next press arms again, nothing
   runs); `engineErrorsArePlainText` feeds engine refusals with `<font …>`
   into `planResult` and `newCaseError` and checks both are plain.
6. **N4, the filled button is the act Enter takes** (`modelData.enter`):
   To verification on an active case, never Hand to agent;
   `startVerifyComplete` checks it. `docs/images/app-plan.png` refreshed.

Decisions taken from the orchestrator: the wider arming stays (Reopen, New
case and Run press twice); `--again` stays terminal-only (WP-231's list).

Mutants on the round-1 code, each reverted: letters repeating →
`reviewAutoRepeatDrop` fails; Enter without `asking` →
`reviewEnterUnderHelp` and `noActWhileTheQuitAskShows` fail; no form guard
→ `reviewKeysBehindTheForm` fails; the reason field pressing on a repeat →
`changesHeldKeyNeverConfirms` fails; the filled button by `primary` →
`startVerifyComplete` fails. Survived: `enabled: !pending` in place of
`readOnly: pending` on the Plan fields — equivalent on Qt 6.12, where a
field disabled for the call keeps its active focus; `readOnly` stays as
the form that does not depend on that.

## Before merge (round 2 APPROVE)

The reviewer's `reviewClosedFormGivesKeysBack` (a form closed by Create or
by Close leaves no hidden field with the keys) and `heldQAsksAndStays` (a
held Q asks once; its repeat never quits while a text is unsent; killed by
a Q Shortcut that repeats) are added. `app/bin/test` under the lock: 78
passed (headless).

## How it was verified

| Check | Where | Result |
|---|---|---|
| `flock /tmp/seldon-check.lock app/bin/build`, clean rebuild (`app/build` removed first), Qt 6.12.0, `-Wall -Wextra`, round 1 | headless (dev host; no window) | passed, 0 warnings |
| `flock /tmp/seldon-check.lock app/bin/test`, round 1: 76 Qt Test cases (44 of WP-228 + 20 of round 0 + the reviewer's 7 `review*` + 5 new), offscreen, own `HOME`, private `XDG_RUNTIME_DIR` 0700, temp dirs on disk, a fake engine that records argv | headless | 76 passed |
| The new cases: `sessionsRunBesideTheQueue`, `germanCoversThePlanWords`, `planShowsTheFixture`, `planInGerman`, `viewSwitchAndKeys`, `startVerifyComplete` (exact argv of start, verify, done), `armedCaseFollowsTheIndex` (a case above leaves: still armed, runs the armed case; the armed case moves on: disarmed, nothing ran; another selection disarms), `dropAndHandToAgent` (`x x`; `a` then Enter arms To verification, never the agent; `a a`), `focusTheWorkingAgent`, `byAgentAndReopen`, `openInEditor`, `newCaseFromOneSentence`, `newCaseByHand` (exact `plan new` argv from the keys), `refusalsKeepTheTitle` (blank title, bad area, exit 4, agent refusal), `emptyListAndSearch`, `importedCaseStartIsOff`, `answerMatchesTheCall`, `devModePlanWritesNothing`, `planRichText`, `planGeometry` (720×480 in German: header parts inside the list column without overlap, the bar inside the detail, the form's choices inside the form, the `?` overlay's last key reachable); `allowsOnlyTheContractForms` extended (12 allowed, 23 refused forms) | headless | passed |
| Every window test fails on any QML warning (the message handler), in `en` and `de` (`planInGerman`, `planGeometry`) | headless | none |
| Mutants on the new code, each reverted: no `built` watch → `armedCaseFollowsTheIndex` fails; answer not matched to its argv → `answerMatchesTheCall` fails; a refused New case closes and resets → `refusalsKeepTheTitle` fails; Enter takes the first act (the agent) → `dropAndHandToAgent` fails; the imported-case guard removed → `importedCaseStartIsOff` fails; a row title not plain → `planRichText` fails; the list column filling (Qt's layout default) → `planGeometry` fails. Survived: resetting the form even when the title changed meanwhile — equivalent, the fields are disabled while the call runs | headless | 7 killed, 1 equivalent |
| `QT_FORCE_STDERR_LOGGING=1` offscreen runs of `app/build/seldon-desk`, again in round 1 (`env -i`, `PATH=/usr/bin` without `seldon`, private `HOME` and `XDG_RUNTIME_DIR` 0700 under `app/build/smoke/`, removed after): fixture dev mode `en`, fixture dev mode `de`, no index and no engine | headless | no output on stderr in any run. These start on Changes; Plan is loaded beside it, and shown it is covered by the window tests above |
| `docs/images/app-plan.png`, `app-changes.png` from the test harness | fixture | — |
| No symlink under `app/`, no `qs.*` import, no hex colour in `app/src`, `app/bin`, the `.pro` outside `theme.cpp`'s fallbacks | fixture (grep, find) | passed |
| Shell scripts | not run | none added or changed (`bin/build`, `bin/test` untouched; the fake engine lives in the C++ test), so no shellcheck run |
| Build against Qt 6.9 / 6.11 | not run | no older Qt on this host; WP-230's CI package job is the check. New QML uses nothing newer than Qt 6.2 (inline components, `required` properties, `TextMetrics`) |
| The window on a real compositor (GPU scene graph, Hyprland, a real `seldon agent start`/`focus`, a live theme or language switch) | not run | desktop red zone; offscreen only |
| The real engine (`edit`, `plan`, `agent sessions` against a logbook) | not run | the fake engine only; the argv forms are CONTRACT.md's and the engine's tests own their answers |

The real `~/.local/state/seldon`, `~/Seldon`, `~/.config` and `/run/user`
were never read or written; the real engine was never started.

## What was not done / open points

1. **`--again` (APP-PARITY 8g)** — decided in round 1: terminal-only for
   now, on WP-231's list. Round 0's note: The row names `agent start`, `agent
   focus`, `--again`. `agent start <id> --again --json` is not in
   CONTRACT.md's command table, and the desk never ran it (it shows Focus
   instead). The app does the same: Focus while an agent works; the
   engine's refusal ("…or start another with `seldon agent start <id>
   --again`; nothing was launched") is shown as it is. A second agent on a
   case from the window needs a CONTRACT.md row and a fixture first. —
   Question for the orchestrator: add the row (and a key, e.g. `A`), or
   record 8g's `--again` as "terminal only"?
2. **Arming is wider than the desk's** — decided in round 1: it stays.
   Round 0's note: The WP says arm-twice for acts that
   write, so Reopen, New case and Run arm too (the desk ran Reopen at once
   and New case / Run on one Enter). — Confirm, or name the acts that
   should run on one press.
3. **An imported case is not started from the window.** The desk enables
   its Start only while the whole Intent from `plan show <id> --json` is
   shown (ADR-0027 §2(a), WP-102b). The app has no `plan show` yet, so
   Start is off for an imported case and the detail says to read the
   Intent in the editor (`e`) and run `seldon plan start <id>` in a
   terminal; no key or press runs it (tested). Import tasks (the portal
   file picker) and this review belong together in a later WP.
4. **Open in editor runs `edit <caseId> --editor --json`** (ADR-0058, the
   CONTRACT.md row WP-229 added), not the desk's `open …`; same answer
   shape (`Model.openResult`). The desk's 2 s repeat guard is replaced by
   the view's one-call-at-a-time rule.
5. **Model.js texts stay English** as in WP-228 #1: the engine's answers
   and Model.js's result lines ("C-2026-005: queued → active", "Agent
   started on …"), the row meta ("C-2026-005 · R1 · themes · 1
   proposed"), and the Risk, Rollback and Dates values. Fixed words go
   through the catalog (labels, statuses, zones, priorities).
6. **Not in this WP:** Ask agent on a case, Import tasks, the stacked
   (narrow) layout and the 1366–3840 px fit (APP-PARITY 2, 6, 10i), the
   ←/→ header-button cursor of WP-215 (the app's buttons are reached by
   their keys `b`, `n`, `i` instead), the WIP limit setting (the app uses
   Model.js's default 3), the `/` search in Changes (3b names it for
   Changes too).
7. **Model.js is now evaluated by six QML files** (Main, StateCard,
   ChangesView, ChangeDetail, PlanView, NewCaseForm), plus Tones.js;
   WP-228 #4 counted four. Not measured.
8. **A selected case that is filtered out** (By agent, a search) keeps its
   detail and its bar; moving with ↑/↓ goes back into the list.
