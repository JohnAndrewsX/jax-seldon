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
  Changelog and Work; 10 new unit tests in `model.test.js` (107 → 117; corrected in round 2).
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

## Round 2

Review 1 (stage 1): SEND BACK on B1 and B2; brief
`WP-122-round-2-brief.md` with the orchestrator's decisions. Not merged
with `next` (WP-120 first, then one merge). Commits 7d78bbd (B1, N5),
5bac4c9 (B2), 0765d33 (Enter, N4), c2c4cc6 (tests), the docs commit after.

- **Enter never launches an agent.** `Model.caseEnterAction` (the first
  action that launches nothing): on an active case Enter twice is To
  verification; *Hand to agent* stays the first button, runs only from `a`
  twice or click + Confirm. Tests: model.test "caseDeskActions … Enter
  never launches" (every status), desk-view `work-agent` #6 (Enter after
  `a` re-arms verify), `work-live` #26–#29 (Enter twice on an active case
  → `plan verify`), `tab-focus` #6. SPEC §5.3, §5.4 Work; KEYBINDINGS.md;
  COVERAGE 10b.
- **B1 — why loud from the rule.** EventDetail asks `seldon drift show
  <id> --json` (CONTRACT argv, read-only; a group's leader) for a selected
  crisis; `Service.driftRules` keeps the answer while the item stays a
  crisis (`Model.keptDriftRules`); `Model.whyLoud` words `always-red`,
  `always-red-paths` (naming the config lists, not their meaning),
  `attention-all` ("a crisis is a change in the red zone"); until the
  answer only class and source. A planned crisis says "C-… plans it …;
  nothing has linked it yet", and the Case and Rule rows agree. Tests:
  model.test "eventDetail … why loud from the engine's rule only (B1)"
  (incl. a proposed crisis), "driftRuleInfo and driftShowResult";
  desk-view `why-loud`, `why-loud-planned`, `why-loud-all`,
  `why-loud-group` (live, argv checked), `today`/`quiet-crisis` (dev mode:
  the neutral text). The fake engine names `class`/`rule` in `drift show`
  (`FAKE_SELDON_ATTENTION_ALL`). Live argv now includes one `drift show`
  for a selected crisis (`today-resolve`, `drift-live`, `tab-focus`).
- **B2 — one count.** The drift chips (open, crisis, attention) list and
  count changes, a group once as its leader; Hide counts a group once;
  the count line says "N changes" there. On the sample: open 6 = sidebar
  6, crisis 2 = header crises, attention 4 = header attention = "4 changes
  without a case"; Hide on mesa → "1 change hidden", chips 5/2/3. Tests:
  model.test "one count everywhere …(B2)"; desk-view `changelog` #1 and
  #16, `hide-group`.
- **N2** — the unit-test count corrected above (10 new in round 1; round 2
  adds 3 more: 120 in all).
- **N3** — exactly as typed: model.test "free text goes exactly as typed,
  surrounding blanks included" (note, note with case, intent, title,
  explain, dismiss); desk-view with blanks in every field: `today-live`
  (note "  --help 2 "), `today-new` (intent with a trailing blank),
  `work-run` (leading), `work-live` (title " --help"), `drift-live`
  (explain " --help ", dismiss "routine update  "), each argv compared.
  Mutants `startAgentNew(text.trim())` in Today.qml and
  `logArgs(String(text).trim(), …)` in Service.qml: killed (today-new,
  today-live argv), run from a runner script on the worktree, restored.
- **N4** — KeyValues: `Text.Wrap` (word boundaries, anywhere only for a
  token longer than the line). desk-view `kv-wrap` (50 %, no overflow);
  render checked by eye ("… can break boot / needs your go").
- **N5** — Open case for a case outside Work's lists: one line "C-… is not
  in the index any more (it keeps the last 50 completed cases)." and *Open
  in editor*; the plugin cannot tell whether the file exists (it reads
  only the index), so the button asks the engine and its answer replaces
  the line. desk-view `case-gone` (live; the engine's refusal shown, argv
  `open C-2026-001 --editor --json`).
- Also: a payload's chip and its `select` arriving together keep the
  selection (`Changelog.chipPending`), found while porting
  `drift-show-member` to the new chips.

Verified: `omarchy plugin validate plugin/` ok; `just qmllint` ok (48
files); `model.test.js` 120; `desk-view.sh` 819 passed, 0 failed.

Round 2 final check: `flock /tmp/seldon-check.lock just check` on a4e808d
(log `check-wp122-r2.log`, a name of its own): **exit 0** (`check: ok`;
docs-check ok, qmllint ok 48 files, model.test 120, real-home-guard 11,
service-states 316, desk-view 819, bar-view 194). Only this handover
changed after that commit.

## Merge of next

`origin/next` at aab1d26 (WP-120, contract v2) merged into this branch:
e60db45, the merge alone; git merged it without a conflict (`Model.js`
`CONTRACT_VERSION` 2 and `manifest.json` from next; `model.test.js`,
`service-states.sh`, SPEC-PLUGIN, TESTING merged cleanly, both sides kept).

Then 3139526 brought WP-122 to the v2 sample and its fields:

- Expectations moved, as the review packet listed: Today tiles 30/51 →
  32/53 (`today`), 73 → 75 events (chips routine 30, in case 37, all 75 in
  `changelog`, `today-live` 76 after its capture, `cursor-follow` 77),
  C-2026-003's log 2 → 3 (`work`: the new `case-updated` line). The open,
  crisis and attention counts did not move (6/2/4): the two new events
  (`case-updated`, `state-loss`) are no drift; `state-loss` shows as a
  routine row.
- `meta.truncated` / a drift item's `truncated`: the event detail's What
  row says "(clipped in the index; the ledger has it in full)". Tests:
  model.test (the sample's clipped note, a truncated drift item),
  desk-view `clipped`, `why-loud-truncated` (live: the truncated crisis
  still gets its rule from `drift show`).
- The case log names the risk a `case-*` line carries (`meta.risk`):
  "case-started · human · R3", "case-updated · human · R3".
- why-loud / `drift show` with v2: the fake and the real engine agree. The
  real engine (`engine/target/debug/seldon`, this branch) on a copy of the
  v2 fixture logbook in a scratch HOME (runner script, removed after):
  `contractVersion` 2; `drift show` on both crises answers `{open: true,
  class: "crisis", rule: "always-red-paths", item: {…, proposedCase:
  null}}` — the fields `Model.driftShowResult` reads; the desk's
  `why-loud*` cases pass on the v2 sample.
- The `triage` object of v2 is not read here (WP-124b; the slot is empty).

Check: `flock /tmp/seldon-check.lock just check` on 3139526 (log
`check-wp122-merge-next.log`): **exit 0** (`check: ok`; docs-check ok,
qmllint ok 48 files, model.test 121, real-home-guard 11, service-states
328, desk-view 824, bar-view 194). After it: SPEC-PLUGIN §5.4 got two
lines for the clipped detail and the log's risk (`docs-check` ok again)
and this handover.
