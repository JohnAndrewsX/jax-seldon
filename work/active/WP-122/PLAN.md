# WP-122 — Plan: sections Today, Changelog, Work

Branch `wp/122-sections-today` from `next` (38a9103, WP-121's desk shell);
merges into `next`.

## Build

- `Model.js` (appended after WP-121's desk block): the prepared views the
  service builds once per index (`deskChangelog`, `deskToday`,
  `deskWork`), the per-chip and search filters, the event detail
  (`eventDetail`, `eventActions`, `whyLoud`), the case detail
  (`caseDetail`, `caseDeskActions`), small text helpers. No new engine
  command: every argv still comes from `logArgs`, `openArgs`, `planArgs`,
  `agentArgs`, `agentNewArgs`, `driftArgs` and `drift show`.
- `Service.qml`: `deskChangelog`, `deskToday`, `deskWork` (bindings on the
  index, ADR-0034 §3), `deskHidden` (Hide, session only),
  `askAgentAvailable` (false until WP-124b).
- `components/desk/`: `EventDetail.qml` (shared by Today and Changelog:
  sticky bar, "why loud", key/values, group members, the inline forms),
  `DriftForm.qml` (the 0.1 `DriftSheet` without its card: same API, so
  `service-states.sh` scenario 35 keeps driving it), `JournalField.qml`
  (the 0.1 `QuickEntry`), `NewCaseSheet.qml` (moved), `Progress.qml`,
  `CaseTile.qml`.
- `sections/Today.qml`, `Changelog.qml`, `Work.qml` on `Section.qml`.
- Deleted: `components/{TodayTab, QuickEntry, ChangelogTab, EventRow,
  DriftSheet, WorkTab, CaseCard, NewCaseSheet}.qml`.
- Harness: `desk-view.sh` cases ported from the old `panel-view.sh`
  scenarios (COVERAGE.md rows 1, 3, 5, 7–19, 25–27, 30–32), the sticky-bar
  geometry case and `DESK_SHOTS` for the three sections; `service-states.sh`
  loads the moved forms.
- Docs: SPEC-PLUGIN §5.4 (sections 1–3 as their own paragraphs), §5.7
  shortened to what WP-123 still needs, KEYBINDINGS.md, TESTING.md,
  COVERAGE.md.

## Decisions (left open by the WP)

1. **Selection is the cursor.** ↑/↓ move the selection and the detail
   follows (as WP-121's Settings section). Enter/Space: in the stacked
   layout with the list shown, show the detail; otherwise the detail's
   primary action — Work: arm, then run the case's first status action
   (the 0.1 "Enter twice"); Changelog/Today on open drift: open the
   default inline form (Link when the engine proposes a case, else
   Explain) and give it the keys (the 0.1 "Enter opens the sheet");
   Today on the yesterday row: open or close it.
2. **Changelog chips** are the WP's classes: open (default, the
   prototype's) · crisis · attention · routine · in case · all. Class of a
   row: open drift with `crisis` → crisis; other open drift (group members
   included) → attention; else with a case → in case; else routine. `f` /
   `F` cycle the chips. The 0.1 source filter is gone; the shim's
   `filter <source>` and a payload `filter` that is a source name open
   "all" with the source in the sidebar search (the rows' meta carries the
   source).
3. **The selection survives a filter and an index update by id.** When
   the selected event leaves the filtered list (it was just linked), the
   detail keeps showing it — with the engine's answer — and the list has
   no highlighted row; ↑/↓ continue from the row number it had.
4. **Hide** (attention only; never a crisis, ADR-0028 §3) writes nothing:
   the event leaves the open and attention chips for this shell session
   (`Service.deskHidden`), a line "N hidden this session · Show" brings
   them back. `drift dismiss` is the recorded way.
5. **Ask agent** is not shown until WP-124b sets
   `Service.askAgentAvailable` (no engine has `agent ask` yet); the triage
   button slot is an empty `Item` (`objectName: "triageSlot"`) above the
   chips.
6. **The "why loud" callout** is derived from what the index carries (the
   drift item's `crisis` and its source), because the index has no rule
   field: config → a path that runs code at login, boot or from a hook;
   pacman → a package that can stop boot or login, changed by name;
   otherwise boot, login or the shell. The "Rule" row says the class and,
   for a group, "one pacman transaction (ADR-0013)".
7. **Work groups**: Active · Verification · Queued · Completed (completed
   and dropped, newest closed first) as `ListView` sections. Status bar:
   queued → Start · Drop · Open in editor; active → Hand to agent · To
   verification · Drop · Open in editor; verification → Complete · Drop ·
   Open in editor; completed → Reopen · Open in editor; dropped → Open in
   editor. Every writing action arms on the first press or click
   (label "Confirm …", hint in the sticky bar); Reopen is one click (it
   creates a case and destroys nothing, WP-101); Drop's hint says it is
   final. "Back to active" (prototype) is not built: no engine verb moves
   verification → active, and the WP adds no engine commands.
8. **Case detail from the index only** (AGENTS.md §3): key/values (risk,
   zone, area, priority, agent, rollback snapshot, dates), Plan = the
   step progress the index carries (`steps`), Log = the index's events of
   this case (lifecycle and notes, newest first), Linked changes = the
   case's `events` found in `index.events` (+N older ones the index no
   longer lists), Intent and Result = "in the case file" with *Open in
   editor*, because the index carries neither text (contract v2 does not
   add them either; see the handover's question).
9. **Today**: list head = date with the day's state pictogram, tiles
   (events today, 7 days), the journal field (and its case picker); the
   list = NEEDS YOU (the crises, newest first), JOURNAL (today's entries),
   the yesterday row (opens in place). Nothing is selected on entry, so
   the detail is the overview: one sentence, the active cases as tiles
   with progress, and **New case** (one sentence → `agent start --new`,
   the same call and result line as Work's Run). A selected crisis shows
   the event detail (the Changelog's component). WP-119's setup card gets
   an empty slot at the top of the overview (`objectName:
   "todaySetupSlot"`); the notices keep the setup banners until then.
10. **Keys**: Today `n` journal field, `i` New case field, `e` today's
    journal in the editor; Changelog `f`/`F` chips, `e` this month's
    ledger, Enter the default form; Work `i` intent field, `+` new-case
    sheet, `e` the case in the editor, `a` hand to agent, `x` drop, `r`
    reopen, Enter the primary action. Esc closes an inline form (draft
    kept) before the desk's own Esc order.

## Round 2 (review 1, orchestrator's decisions)

- Decision 1 amended: Enter takes the first action that launches nothing;
  on an active case that is To verification (0.1's habit). *Hand to
  agent* stays the first button and runs only from `a` or a click.
- Decision 2 amended: the drift chips count and list changes, a group
  once as its leader, so every count on the screen agrees.
- Decision 6 replaced: the "why loud" callout takes the engine's rule from
  `seldon drift show --json`; until it answers, only class and source.
