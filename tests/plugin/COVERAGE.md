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
plan argv builders) keep their unit tests in `model.test.js` throughout
(WP-123 dropped `systemSections`' tab use for `systemTiles`, built on it,
and the overlay-only `overlayMeta`, `overlayBanner`, `overlayPayloadPeriod`
and `periodForKey` with their tests: the desk's header, notices and
`deskPayload` took their place);
`service-states.sh` keeps the service's engine calls, argv exactness and
the sheets (scenario 35); `bar-view.sh` keeps the pill.

## panel-view.sh (the bar popup)

| # | Old scenario | Successor | Status |
|---|---|---|---|
| 1 | The sample: every tab renders its data, the strip on every tab, header mark, today's state | Header mark, KPI strip and counts: `desk-view.sh` widths, keys (`view.mark`, `view.kpis`, `view.counts`); the tabs' data: WP-122 (Today, Changelog), `desk-view.sh` system (WP-123); the red strip has no successor — the header's crises figure (urgent) and Today's "Needs you" (WP-122) carry it (ADR-0034 §2) | header done (WP-121); System done (WP-123: system); Today, Changelog open |
| 2 | Keyboard: Tab/Shift-Tab hand over to the bar, ←/→ h/l and digits switch tabs, ↑/↓ j/k cursor, Esc closes | `desk-view.sh` keys: digits 1–8 and `,`, Alt+↑/↓ with wrap, Tab does nothing (the desk is not a bar popup, ADR-0034 §2), Esc order; the list cursor: WP-122/123 per section | done (WP-121) for the desk's keys |
| 3 | The yesterday row opens with Enter and stays in view | WP-122 (Today) | open |
| 4 | Snapper not readable: its banner on every tab, Run in terminal, Check again, the hint | `desk-view.sh` snapper (the notice under the header) | done (WP-121) |
| 5 | Not initialised: banner and pictogram, no strip, empty tabs, `+` and `d` do nothing | `desk-view.sh` uninit (notice, no KPI figures, no counts, the chip folds); sections 4–6 and `d`, `e`: `desk-view.sh` uninit-sections (WP-123); sections 1–3 and `+`: WP-122 | notice done (WP-121); 4–6 done (WP-123: uninit-sections); 1–3 open |
| 6 | Every system field optional (empty and sparse system) | `desk-view.sh` system-empty, system-sparse, system-degraded; `model.test.js` systemTiles | done (WP-123: system-empty, system-sparse) |
| 7 | Live: QuickEntry, Open in editor on every tab, Capture now, argv | WP-122 (Today's journal field, `e` per section); `e` and Open in editor in Decisions, System, Memory with exact argv and editor paths: `desk-view.sh` decisions-live (WP-123) | 4–6 done (WP-123: decisions-live); 1–3 open |
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
| 20 | Decisions and Memory on the sample | `desk-view.sh` decisions, decision-cases, memory | done (WP-123: decisions, memory) |
| 21 | Decisions live: `d`, title typed, Enter twice creates, opens | `desk-view.sh` decisions-live (the form in the detail pane; Accept on a proposed decision) | done (WP-123: decisions-live) |
| 22 | Decision refusals keep the title | `desk-view.sh` decisions-locked | done (WP-123: decisions-locked) |
| 23 | Label fit at font scale 1.0 and 1.25, narrow card | `desk-view.sh` widths and thresholds check that no text leaves the window or the desk at 1366–3840 px and under 960 / 760 px; sections 4–7: `desk-view.sh` fit-1366, fit-3840, stacked-sections, radiant-half (WP-123); sections 1–3: WP-122; font scale 1.25 sweep: WP-126 (theme and scale sweep) | partly done (WP-121, WP-123: fit-1366, fit-3840) |
| 24 | Offscreen renders of the Today tab in three themes (PANEL_SHOTS) | `desk-view.sh` with `DESK_SHOTS` (Today at 100 % and 50 %, Settings, not initialised; Tokyo Night, Kanagawa, Catppuccin Latte) | done (WP-121) |
| 25 | A tab change gives the keys back (hidden field keeps focus) | `desk-view.sh` keys (the search gives the keys back on Enter and Esc; a section change calls `giveKeys`); the new-decision form: `desk-view.sh` decisions-live, decisions-locked (Esc and a row click give the keys back; WP-123); fields of sections 1–3: WP-122 | desk part done (WP-121); Decisions done (WP-123) |
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
| 1 | IPC and keys: toggle, periods 1–4 and ←/→, Esc, scrim click, summon with a payload, chip click, `setPeriod`, `view`, Close | Open/close/payload/`view`: `desk-view.sh` ipc (toggle twice, `{"period":"30"}` lands on section 7, a click beside the desk closes, Esc); periods (←/→ and h/l; the digits are the desk's sections, so the overlay's 1–4 have no successor), chip click, 90 d on every entry, a payload on the open desk, `setPeriod`, `hover` only while shown, the slots in `view`: `desk-view.sh` radiant-ipc (WP-123); Close and the scrim are the desk's Esc and click beside it (WP-121) | done (WP-121: ipc; WP-123: radiant-ipc) |
| 2 | Fresh open: no aggregation on the first frame, one paint per chart by frame 2, repaints only on data or size change | `desk-view.sh` radiant-fresh: the same counters as the old overlay on the fixture (first frame 0 passes and 0 paints, every chart once by frame 2, `2,2,2,1,2,1` after a period switch, nothing for a hover, `3,3,3,2,3,2` after a resize, a second fresh open the same); the bare creation (`service === null` first): `desk-view.sh` (`bare`) | done (WP-121: bare; WP-123: radiant-fresh) |
| 3 | Hover read-outs from mouse moves and `call hover` | `desk-view.sh` radiant-hover (every read-out and malformed argument of the old case) | done (WP-123: radiant-hover) |
| 4 | Layout at 2560×1440 and at a 1.25 output scale | `desk-view.sh` radiant-size-2560x1440, -1536x864, -2048x1152 (the section's grid, every period); the desk's own width at four screen widths: `desk-view.sh` widths | done (WP-121: widths; WP-123: radiant-size-*) |
| 5 | QT_SCALE_FACTOR 1.25 | `desk-view.sh` radiant-scaled | done (WP-123: radiant-scaled) |
| 6 | Narrow windows reflow the grid | `desk-view.sh` radiant-half (50 % on 1920 and 1366: medium; under 960 px: narrow and scrolling; no text outside its slot), radiant-reflow (one paint per size change) | done (WP-123: radiant-half, radiant-reflow) |
| 7 | Not initialised: banner with Copy only, empty charts | The notice: `desk-view.sh` uninit (the desk shows every fix — the overlay's Copy-only rule existed because the overlay ran no engine command; the desk is the one surface and keeps the panel's fixes); empty charts, no paints, empty hovers: `desk-view.sh` radiant-uninit | done (WP-121: uninit; WP-123: radiant-uninit) |
| 8 | Every index variant renders every chart | `desk-view.sh` radiant-variant-* (one paint each; two where a notice settles under the header in the first frame: index-stale, and now snapper-degraded, whose notice the overlay did not show) | done (WP-123: radiant-variant-*) |
| 9 | Offscreen renders in three themes (OVERLAY_SHOTS) | `desk-view.sh` with `DESK_SHOTS`: shot-sections-* (the Prime Radiant at 100 %, 50 % and with a 365 d hover; Decisions, System, Memory) | done (WP-123: shot-sections-*) |
