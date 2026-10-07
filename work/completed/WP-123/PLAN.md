# WP-123 — Plan: Decisions, System, Memory; Prime Radiant inside the desk

Branch `wp/123-sections-system` from `next` (38a9103, WP-121's desk shell);
merge into `next`. Inputs read: AGENTS.md, ADR-0034, WP-123, WP-121's
handover and plan, `tests/plugin/COVERAGE.md`, the prototype
(`decisions`, `system`, `memory`, `radiant`; private, not copied), the
0.1 tabs (`DecisionsTab`, `NewDecisionSheet`, `SystemTab`, `MemoryTab`),
the deleted `Overlay.qml`, `overlay-view.sh`, `harness/overlay.qml` and
`panel-view.sh` scenarios 5–7, 20–23 from git history, SPEC-PLUGIN §5,
§6, §8, `Model.js` (row functions, periods, grid, desk), `Service.qml`
(`decide`, `openInEditor`, `periods`).

## Steps

1. `Model.js`: section models beside the old row functions —
   `decisionDetail`, `decisionCases` (v2 `decisions[].cases`, hidden on
   v1), `systemTiles` (five tiles from `index.system` / `state`),
   `memoryDetail`, `deskFilter` (sidebar search over a row's fields);
   unit tests. Drop what only the overlay used (`overlayBanner`,
   `overlayMeta`, `overlayPayloadPeriod`, `periodForKey`).
2. `sections/Decisions.qml`: list (proposed striped, newest first), New
   decision (`d` or the button) → the form in the detail pane (ported
   `NewDecisionSheet` → `components/desk/NewDecisionForm.qml`), detail with
   the sticky bar (Accept when proposed, Open in editor), linked cases.
3. `sections/System.qml`: five tiles → detail with the big value, lead,
   key/value rows and "From the dossier; rebuilt on every capture."; Open
   in editor (STATUS.md).
4. `sections/Memory.qml`: lessons and topics → detail; Open in editor.
5. `sections/Radiant.qml`: period chips, caption, the six `OverlaySlot`s
   in `Model.overlayGrid` of the section's own size, hover, `view()`;
   default 90 d on every entry; `Desk.setPeriod`, `Desk.hover`; ←/→ and
   h/l walk the periods.
6. Delete the old tab files, `SectionStub` users of 4–7.
7. Harness: `fresh`, `hoverItem`, `hoverAt`, `leave` steps, first-frame
   counters, slot boxes in the overflow check; scenarios ported per
   `COVERAGE.md`.
8. SPEC-PLUGIN §2, §5.4, §5.7 (rows 4–6 out), §6, §8; KEYBINDINGS;
   TESTING; COVERAGE.md rows; handover.

## Decisions (what the WP leaves open)

1. **No decision body, no memory text.** The index carries a decision's
   id, title, status, date, path (and in v2 its cases) and only the
   lesson headings and topic names of memory; the plugin reads only the
   index (AGENTS.md §3). The details show what the index has and "Open in
   editor" for the text. Adding bodies would be a contract change beyond
   this WP.
2. **Accept = the existing path.** The engine has no command that accepts
   a decision; a decision is accepted by setting `status: accepted` in its
   frontmatter (the index follows on the next capture). *Accept* on a
   proposed decision therefore runs the existing `seldon open <ADR>
   --editor --json` and the detail says what to change. It writes
   nothing itself, so it does not arm. A `seldon decide accept` command
   would be a new entry in CONTRACT.md's command table: an open question
   for the orchestrator, not built here.
3. **New decision in the detail pane.** `d` or *New decision* shows the
   form where the detail is (stacked: the detail), like WP-122's inline
   forms; Enter arms, Enter again creates (`decide --no-edit --json --
   <title>`, then the service opens it, unchanged); a click on Create
   runs at once; Esc closes the form and keeps the title. Once the index
   lists the new decision, it is selected.
4. **System's five tiles.** Omarchy (version; theme, last update,
   checkout, plugins), Packages (installed; explicit, AUR), Snapshots
   (newest number; the list), Deviations (the count; the list is in
   STATUS.md), Collectors (ok of enabled; each collector, then machine,
   engine, index time and the areas — how Seldon sees the machine). A
   tile whose data is missing shows "—" and "Not in the index".
5. **Periods in the desk.** Digits belong to the sections now, so the
   overlay's `1`–`4` are gone: ←/→ and `h`/`l` walk the periods (wrap),
   the chips take clicks. `setPeriod <id>` shows section 7 and sets the
   period (an unknown id changes nothing and returns the current one);
   `hover` works while section 7 is shown, else `{ error }`.
6. **Search.** The sidebar search filters the list of Decisions (id,
   title, status, date), System (title, lead) and Memory (title, meta).
7. **Grid size.** `overlayGrid(section width − padding, section height −
   chips row − padding, …)`, the overlay's gap and minimum sizes; the
   grid scrolls only when the minimum heights do not fit.
8. **Section data.** Row and tile models are bindings on the index (one
   evaluation per index change, as the 0.1 tabs); chart data stays the
   service's `periodTable` (ADR-0034 §3).
