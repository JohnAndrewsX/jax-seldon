# WP-122 — Handover: sections Today, Changelog, Work

Branch `wp/122-sections-today` from `next` (38a9103, WP-121's desk shell);
merge into `next`. Plan and the decisions the WP left open:
[PLAN.md](PLAN.md).

## What was done

- **`sections/Today.qml`**: date and day state (A11 pictogram), *Open in
  editor* (`e`), tiles events today / 7 days, the journal field
  (`components/desk/JournalField.qml`, the 0.1 QuickEntry, `n`), NEEDS YOU
  (the crises, the red strip's successor), JOURNAL with the yesterday row
  opening in place; the overview detail (WP-119's empty `todaySetupSlot`,
  one sentence, active-case tiles with progress → Work, **New case** →
  `agent start --new`, `i`); a selected crisis shows the event and stays
  shown once resolved here; the sidebar search filters crises and entries.
- **`sections/Changelog.qml`**: the empty `triageSlot` (WP-124b), chips open
  · crisis · attention · routine · in case · all with counts (`f`/`F`),
  *Ledger*, *Capture now*, the quiet attention line, "+N more", "N hidden
  this session · Show", rows newest first under their day with the class
  stripe; the event detail (`components/desk/EventDetail.qml`): sticky bar
  (Ask agent hidden until WP-124b sets `Service.askAgentAvailable`; Link to
  case…, Explain…, Dismiss…, Hide/Show; Open case), "why loud" callout,
  key/values When · Who · What · Case · Rule · Source · Zone · Resolved ·
  Event, group members (`drift show`), and the inline form
  (`components/desk/DriftForm.qml`, the 0.1 DriftSheet's logic and API,
  without its card).
- **`sections/Work.qml`**: intent field + Run, WIP line against `wipLimit`,
  By agent, New case (the 0.1 sheet, moved to `components/desk/
  NewCaseSheet.qml`), the engine's answer; groups Active · Verification ·
  Queued · Completed; the case detail from the index (key/values, plan
  progress, log, linked changes; Intent and Result → *Open in editor*);
  the sticky status bar (Start / Hand to agent / To verification /
  Complete / Drop / Reopen / Open in editor) with arm-twice for writing
  actions (Reopen once), `a`, `x`, `r`, `e`, `i`, `+`, Enter.
- **`Model.js`** (appended after WP-121's block): `deskChangelog`,
  `changelogView/Chips`, `cycleChip`, `eventClass`, `hiddenCount`,
  `eventDetail`, `eventActions`, `whyLoud`, `deskToday`, `todayRows`,
  `deskWork`, `workView`, `caseDetail`, `caseDeskActions`, `caseArmHint`,
  `caseActionVerb`, `rowAge`, `lastSegment`. **No new engine command**;
  every call still goes through `logArgs`, `openArgs`, `planArgs`,
  `agentArgs`, `agentNewArgs`, `driftArgs` and `drift show`
  (`validateArgs` unchanged; CONTRACT.md's argv table unchanged).
- **`Service.qml`**: `deskChangelog`, `deskToday`, `deskWork` built once per
  index (ADR-0034 §3), `deskHidden`, `askAgentAvailable` (false).
- **`Desk.qml`**: `takeKeys()` — a section's field hands the keys back
  (Esc, Cancel, a resolved form); `giveKeys()` refused while the field still
  had the focus. `DetailPane`'s Flickable got `objectName:
  "deskDetailScroll"` (the harness scrolls it).
- New desk components: `GroupedRow` (a `ListRow` with an optional group
  header), `Progress`, `CaseTile`, `EventDetail`.
- **Deleted**: `components/{TodayTab, ChangelogTab, EventRow, WorkTab,
  CaseCard}.qml`; `QuickEntry`, `DriftSheet`, `NewCaseSheet` moved (git
  mv) into `components/desk/` as `JournalField`, `DriftForm`,
  `NewCaseSheet`.
- **Harness**: `desk-view.sh` section 8 (8a–8e) with the 0.1 panel's
  scenarios for these tabs one to one, plus the sticky bar, the stacked
  layout, the sidebar search on Today, not-initialised sections and a crisis
  resolved from Today; `harness/desk.qml`'s overflow check honours clipping
  (rows scrolled out of a list are not on screen); `service-states.sh`
  scenario 35 loads the moved forms; `DESK_SHOTS` also renders the
  Changelog and Work; 12 new unit tests in `model.test.js`.
- **Docs**: SPEC-PLUGIN §2, §5.3, §5.4 (Today, Changelog, Work as their
  own paragraphs; §5.7 now covers sections 4–6 only), §8; KEYBINDINGS.md;
  TESTING.md; `tests/plugin/COVERAGE.md` — every row WP-122 owns is
  **done** with its case name.

## Decisions

In [PLAN.md](PLAN.md) "Decisions", 1–10. The ones a reviewer should look at
first:

- **Enter** runs the detail's first action (Work: arm, then run — the 0.1
  "Enter twice"; open drift: open the default form), because the selection
  is the cursor, so "select the row" alone would do nothing. §5.3 says so.
- **Active's primary is *Hand to agent*** (the prototype's order), so Enter
  twice on an active case launches the agent; *To verification* is a click
  (and Confirm). The 0.1 card had Verify first.
- **Hide** writes nothing (a session's quiet, attention only); *Dismiss*
  is the recorded way. No engine command exists for a hide and the WP adds
  none.
- **"Back to active"** (prototype) is not built: no engine verb moves a
  case from verification to active.
- **Open case** in an event's bar goes to Work with the case selected
  (the 0.1 sheet opened the file in the editor; Work's *Open in editor* is
  one click from there).
- The 0.1 **source filter chips** are gone; `filter <source>` (shim,
  payload) shows "all" with the source in the sidebar search.
- **Why loud** is derived from the drift item's `crisis` and source, since
  the index has no rule field (see the questions).

## What I expect from contract v2 (WP-120) and later

Built against the current (v1) fixture; nothing here reads a v2 field.

- v2 as planned (ADR-0034 §5–§6, WP-120) needs nothing from these sections
  except: the **`triage`** object → WP-124b fills `triageSlot` and sets
  `askAgentAvailable`; **`meta.truncated`** → the event detail could mark a
  clipped detail ("…, truncated") in its What row — a one-line follow-up in
  `Model.eventDetail` once the fixture carries one; **`state-loss`** events
  show as routine rows (class by the existing rule) unless the engine
  makes them drift.
- Not in v2, and what would make these details exact (a later contract
  question, not this WP's): a drift item's **rule** (which harm test made
  it a crisis — the callout guesses from source and class), and a case's
  **intent** / **result** one-liners in `index.cases` (the detail sends the
  user to the editor for both today).

## How it was verified

- `omarchy plugin validate plugin/` and `just qmllint` (0 warnings, 48
  files, token check ok) before every commit.
- `node tests/plugin/model.test.js` — 117 passed; `model.bench.js` ok. The
  three prepared views cost ~5 ms per index under node at 500 events
  (once per index change, not per paint).
- `bash tests/plugin/desk-view.sh` — **789 passed, 0 failed** (373 before:
  WP-121's cases all still pass beside the new ones; 798 with `DESK_SHOTS`,
  which adds the theme renders' checks); with `DESK_SHOTS` the
  three themes at 100 % / 50 % / Changelog / Work rendered and checked by
  eye (one fix from that: Today's date wrapped beside its button at 50 %).
- `bash tests/plugin/service-states.sh` — 316 passed (scenario 35 drives
  the moved `DriftForm` and `NewCaseSheet` through the same API).
- Acceptance items: every COVERAGE row for Today, Changelog, Work done; the
  argv table unchanged (each live case compares `argv.log` argument by
  argument); cursor stability across an index update (`cursor-follow`, and
  `drift-live` for an event leaving its chip); the sticky bar
  (`sticky`: its scene position unchanged while the detail scrolls by
  ~100 px in Work and in the Changelog, plus `shot:sticky-work`).
- No live check on the dev or test host in this WP (not required by the
  acceptance; ADR-0034's live sweep is WP-126). Everything ran headless,
  temp `HOME`, fake engine; the real `~/.local/state/seldon` and
  `~/.config/seldon` untouched (the real-home guard checks it).

## Not done / for later

- Ask agent and the triage button: WP-124b (slot and flag in place).
- WP-119's setup card: slot in Today's overview; the setup banners stay in
  the notices until then.
- `plugin/README.md`, user guides, CHANGELOG **Breaking**, `preview.png`:
  WP-126 (as WP-121 left them).
- Font scale 1.25 sweep of the three sections: WP-126.

## Merge notes (for the orchestrator)

- `Model.js`: my block is appended after WP-121's desk block; WP-120 and
  WP-123 also touch `Model.js` — expect a trivial end-of-file conflict
  with WP-123.
- `service-states.sh` scenario 35: I changed the two load lines of
  NewCaseSheet and DriftSheet (→ `components/desk/`); WP-123 will change
  the NewDecisionSheet line beside them.
- SPEC-PLUGIN §5.4/§5.7 and §2's file list: WP-123 rewrites §5.7 and adds
  its sections' paragraphs after mine.
- `tests/plugin/harness/desk.qml` `overflow()` now takes a clip box; a
  WP-123 case that relied on clipped text being reported would change
  (none of WP-121's did).

## Security (for review)

- No new process, no new command, no shell string. Free text (note,
  intent, title, explanation, reason) is still one argv element after
  `--`; ids are checked against the schema patterns in `Model.js` before
  any call.
- Hide keeps a set of event ids in the service's memory only; nothing is
  written.
- The plugin reads only `index.json` (AGENTS.md §3): the case detail shows
  what the index carries and opens the case file in the editor through the
  engine (`open <id> --editor --json`), never reads it.

## Open questions

None blocking. For the operator/orchestrator (a contract question beyond
this WP): should a later contract carry a drift item's rule and a case's
intent/result one-liners, so the event and case details need no guess and
no editor round trip?

## Final check

`flock /tmp/seldon-check.lock just check` on 23854e4: **exit 0** (`check:
ok`; docs-check ok, qmllint ok 48 files, model.test 117, real-home-guard 11,
service-states 316, desk-view 789, bar-view 194). Only this handover changed
after that commit.
