# Plugin harness coverage: from the panel and the overlay to the desk

ADR-0034 §8 (coverage gate): `panel-view.sh` (the 0.1.x bar popup) and
`overlay-view.sh` (the fullscreen Prime Radiant) were deleted in WP-121,
when `Desk.qml` replaced both surfaces. Every scenario they had is listed
here with its successor in `desk-view.sh` — a scenario that exists now
(WP-121), or the work package that ports it with the section it tests —
or the reason it has none. A work package that ports a row marks it
**done** with the `desk-view.sh` case name.

The row functions those scenarios exercised (`changelogRows`, `todayView`,
`workColumns`, `caseActions`, `decisionRows`, `systemSections`,
`memoryRows`, `periodTable`, `periodView`, `overlayGrid`, the drift and
plan argv builders) keep their unit tests in `model.test.js` throughout;
`service-states.sh` keeps the service's engine calls, argv exactness and
the sheets (scenario 35); `bar-view.sh` keeps the pill.

## panel-view.sh (the bar popup)

| # | Old scenario | Successor | Status |
|---|---|---|---|
| 1 | The sample: every tab renders its data, the strip on every tab, header mark, today's state | Header mark, KPI strip and counts: `desk-view.sh` widths, keys (`view.mark`, `view.kpis`, `view.counts`); the tabs' data: WP-122 (Today, Changelog), WP-123 (System); the red strip has no successor — the header's crises figure (urgent) and Today's "Needs you" (WP-122) carry it (ADR-0034 §2) | header done (WP-121); rest open |
| 2 | Keyboard: Tab/Shift-Tab hand over to the bar, ←/→ h/l and digits switch tabs, ↑/↓ j/k cursor, Esc closes | `desk-view.sh` keys: digits 1–8 and `,`, Alt+↑/↓ with wrap, Tab does nothing (the desk is not a bar popup, ADR-0034 §2), Esc order; the list cursor: WP-122/123 per section | done (WP-121) for the desk's keys |
| 3 | The yesterday row opens with Enter and stays in view | WP-122 (Today) | open |
| 4 | Snapper not readable: its banner on every tab, Run in terminal, Check again, the hint | `desk-view.sh` snapper (the notice under the header) | done (WP-121) |
| 5 | Not initialised: banner and pictogram, no strip, empty tabs, `+` and `d` do nothing | `desk-view.sh` uninit (notice, no KPI figures, no counts, the chip folds); empty sections and `+`/`d`: WP-122, WP-123 | notice done (WP-121) |
| 6 | Every system field optional (empty and sparse system) | WP-123 (System) | open |
| 7 | Live: QuickEntry, Open in editor on every tab, Capture now, argv | WP-122 (Today's journal field, `e` per section) and WP-123 (`e` in Decisions, System, Memory) | open |
| 8 | The engine refuses a note: message shown, text kept | WP-122 (Today) | open |
| 9 | Work on the sample: columns, WIP text, badge, card actions by status, cursor | WP-122 (Work) | open |
| 10 | Work live: new case sheet with keys, start → verify → done with Enter twice | WP-122 (Work; Arm.qml) | open |
| 10b | Start agent: arm with `a`, Enter re-arms the first action, `agent start <id>` | WP-122 (Work; Ask agent in WP-124b) | open |
| 11 | New case refused (lock held): message, title kept, Esc, `+` back | WP-122 (Work) | open |
| 12 | Drift sheet on the sample: defaults per item, group members, IPC route | WP-122 (Changelog event detail) | open |
| 13 | ADR-0020: fewer drift items listed than counted ("+N more") | WP-122 (Changelog) | open |
| 13b | Quiet surfaces, crisis in the yellow zone | WP-122 (Today, Changelog) | open |
| 13c | Attention alone: no strip, no "crisis" anywhere | WP-122 (Today, Changelog); the header part: the crises figure is not urgent at 0 (`Model.deskKpis`, model.test.js) | open |
| 14 | Drift sheet live: link, explain (text `--help`, risk, area), Open case | WP-122 (Changelog) | open |
| 15 | `--only` and a refusal in the plugin | WP-122 (Changelog) | open |
| 16 | Already resolved re-run | WP-122 (Changelog) | open |
| 17 | Drift refused (lock held): message, text kept, draft back | WP-122 (Changelog) | open |
| 18 | Group members beyond the index: `drift show` | WP-122 (Changelog) | open |
| 19 | The same from a member row | WP-122 (Changelog) | open |
| 20 | Decisions and Memory on the sample | WP-123 (Decisions, Memory) | open |
| 21 | Decisions live: `d`, title typed, Enter twice creates, opens | WP-123 (Decisions) | open |
| 22 | Decision refusals keep the title | WP-123 (Decisions) | open |
| 23 | Label fit at font scale 1.0 and 1.25, narrow card | `desk-view.sh` widths and thresholds check that no text leaves the window or the desk at 1366–3840 px and under 960 / 760 px; label fit inside each section's rows: WP-122, WP-123; font scale 1.25 sweep: WP-126 (theme and scale sweep) | partly done (WP-121) |
| 24 | Offscreen renders of the Today tab in three themes (PANEL_SHOTS) | `desk-view.sh` with `DESK_SHOTS` (Today at 100 % and 50 %, Settings, not initialised; Tokyo Night, Kanagawa, Catppuccin Latte) | done (WP-121) |
| 25 | A tab change gives the keys back (hidden field keeps focus) | `desk-view.sh` keys (the search gives the keys back on Enter and Esc; a section change calls `giveKeys`); fields inside sections: WP-122 | desk part done (WP-121) |
| 26 | The Changelog cursor follows its event across an index update | WP-122 (Changelog; acceptance "cursor stability") | open |
| 27 | Capture now replaces a pending lock retry | WP-122 (Changelog's Capture now); the `c` key: `desk-view.sh` capture-warned | key done (WP-121) |
| 28 | Capture warnings: neutral notice, tooltip, gone after a clean capture | `desk-view.sh` capture-warned (notice, `c` captures, notice gone); the tooltip popup's geometry: no successor — the desk shows the notice at the desk's width, where the first lines already fit, and the full text stays in the notice's `full` (hover) as before; WP-126 checks it live | done (WP-121) |
| 29 | Restart notice after a plugin update; Restart shell once | `desk-view.sh` restart-same, restart-updated | done (WP-121) |
| 30 | Closed by agent: "by agent", Reopen beside Open, By agent filter | WP-122 (Work) | open |
| 31 | Run: intent field, `--`, busy, refusal kept | WP-122 (Work, Today's New case) | open |
| 32 | Reopen: one click, `r` | WP-122 (Work) | open |
| 33 | Rules banner: doctor on open, Update rules, result line, damaged, failure, throttle | `desk-view.sh` rules-outdated, rules-fail; damaged and the 10-minute throttle stay covered by the service (`service-states.sh`, Model.rulesBanner unit tests) — the desk calls the same `checkRules(false)` on open | done (WP-121) |

## overlay-view.sh (the Prime Radiant overlay)

| # | Old scenario | Successor | Status |
|---|---|---|---|
| 1 | IPC and keys: toggle, periods 1–4 and ←/→, Esc, scrim click, summon with a payload, chip click, `setPeriod`, `view`, Close | Open/close/payload/`view`: `desk-view.sh` ipc (toggle twice, `{"period":"30"}` lands on section 7, a click beside the desk closes, Esc); periods, `setPeriod`, the slots in `view`: WP-123 (Radiant) | desk part done (WP-121) |
| 2 | Fresh open: no aggregation on the first frame, one paint per chart by frame 2, repaints only on data or size change | WP-123 (acceptance "aggregation and paint counters equal to the old overlay's"); the bare creation (`service === null` first): `desk-view.sh` (`bare`) | bare done (WP-121) |
| 3 | Hover read-outs from mouse moves and `call hover` | WP-123 | open |
| 4 | Layout at 2560×1440 and at a 1.25 output scale | WP-123 (the section's grid); the desk's own width at four screen widths: `desk-view.sh` widths | desk part done (WP-121) |
| 5 | QT_SCALE_FACTOR 1.25 | WP-123 | open |
| 6 | Narrow windows reflow the grid | WP-123 (acceptance "renders at 50 % width without a chart leaving its slot") | open |
| 7 | Not initialised: banner with Copy only, empty charts | The notice: `desk-view.sh` uninit (the desk shows every fix — the overlay's Copy-only rule existed because the overlay ran no engine command; the desk is the one surface and keeps the panel's fixes); empty charts: WP-123 | notice done (WP-121) |
| 8 | Every index variant renders every chart | WP-123 | open |
| 9 | Offscreen renders in three themes (OVERLAY_SHOTS) | WP-123 (section 7 in `DESK_SHOTS`) | open |
