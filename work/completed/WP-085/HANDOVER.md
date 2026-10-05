# WP-085 HANDOVER — The panel shows capture warnings

Branch `wp/085-review`, worktree `wt/WP-085`. Commits on top of `616be66`
(oldest first):

- `d6ff912` plugin: the panel shows a capture's warnings in a neutral notice
- `31c6480` plugin tests: capture warnings in service-states and panel-view
- `77928f4` docs: capture warnings in SPEC-PLUGIN, the user guide and the changelog
- `b06d47b` docs(de): troubleshooting capture warnings, source line at 77928f4
- `792ff45` plugin tests: the capture notice's tone is checked in a theme whose accent differs
- `71af003` memory: WP-085 pitfalls
- plus the commit with this handover

## Done

- **Model.js.** `captureResult` now always returns `warnings`: the
  non-blank strings of `capture --json` `warnings`, unchanged (the
  engine's text with its restore hint); `[]` for a failed capture or an
  engine without the key. New (appended at the end): `captureWarnings(data)`,
  `CAPTURE_WARNED_TITLE`, `captureWarningNotice(warnings)` → `null` or a
  banner-shaped object `{ status: "captureWarned", tone: "neutral",
  title: "Capture warned", detail, full, command: "", actions: [], hint: "" }`.
  `detail` is the first line of each warning (`Model.firstLine`), one per
  line; `full` is every warning in full for the hover, `""` when the
  detail already shows all of it (the engine's warnings today are one line).
- **Service.qml.** `captureWarnings` is set in `runnerDone` only by a
  `capture` that exited 0 (so: the timer, *Capture now*, *Check again*,
  the `c` key, the bar's right click and IPC `capture`, which all go
  through `captureNow`). A failed capture and a lock-retry wait leave it;
  the next capture that finishes without warnings clears it.
  `captureNotice` derives from it. The snapshot has `captureWarnings` and
  `captureNotice` (the detail).
- **Banner.qml.** Tone `"neutral"` draws in the foreground (else urgent /
  accent as before). A banner with `full` shows it in the shell's
  `PanelToolTip` while a `HoverHandler` sees the pointer; exposes
  `tooltipText` and `hovered`.
- **Panel.qml.** A third `Banner` (`captureNotice`) under the snapper
  banner, above the crisis strip, so every tab shows it; no
  `onActionRequested`. `view().captureNotice = { title, detail, tooltip,
  hovered, neutral, accentTone }`.
- **No action.** The WP allows *Open guide* only for a fixed page. None
  ships: the package installs only `README.md` under `/usr/share/doc`, and
  the plugin has no URL launcher; a GitHub `blob/main` link is not fixed
  to the installed version. So no action, and nothing built from the text
  ever reaches a command (AGENTS.md §8). The engine's text names the
  guide section itself. See open question 1.
- **Harness.** fake-seldon: `FAKE_SELDON_CAPTURE_WARNINGS` (JSON list
  that a successful capture reports, default `[]`; the fake now always
  prints `warnings`, as the engine does), `FAKE_SELDON_CAPTURE_WARNED`
  (only these capture calls warn) and `FAKE_SELDON_CAPTURE_EXIT_CALLS`
  (`FAKE_SELDON_CAPTURE_EXIT` only for these calls), call numbers counted
  in `$HOME/capture-calls`. Existing knobs unchanged in behaviour.
  service-states **36 `capture-warned`** (warn → exit 2 keeps it → clean
  clears it; exact argv) and **36b `capture-warned-locked`** (warn → exit
  4 lock wait keeps it → retry clears it). panel-view **28
  `capture-warned`** (Tokyo Night theme: title, detail on screen, neutral
  and not the accent, tooltip text, `hover:` → hovered, still shown on the
  Changelog, *Capture now* → gone, 2 captures, no overflow, clean log).
  The panel harness has a new `hover:<text>` step (QtTest `mouseMove`).
- **Docs.** SPEC-PLUGIN §3 bullet and §5 banner sentence; plugin/README
  States row (the WP-078 States test now includes the notice); user guide
  10 en+de: banner table row and one sentence in "A state reset was
  recorded"; de re-stamped at `77928f4` in its own commit; CHANGELOG
  `[Unreleased]` → Plugin line; memory/pitfalls.md entry.

## Not done

- ~~No assertion that the tooltip *popup* opens … the harness reports
  only the panel's item tree, not the popup layer.~~ **Wrong** (review
  B2): the popup's text does appear in `.texts` once its 400 ms delay has
  passed; I had looked only at the step right after the hover. Asserted
  in round 2.
- Captures run outside the plugin (CLI, hooks) are not seen; stated in
  SPEC-PLUGIN §3. The ledger's `state-reset` note still shows on the
  Changelog for those.
- The Prime Radiant overlay does not show the notice. It shows only the
  status banner (`Model.overlayBanner`), the same as the snapper banner.

## Verified by

- On every commit that touched `plugin/`: `omarchy plugin validate
  plugin/` ok and `just qmllint` ok (29 files, tokens ok, 544 references).
  `node tests/plugin/model.test.js`: 88 passed (new test "captureResult
  keeps the capture's warnings; captureWarningNotice (WP-085)"; the old
  shapes gained `warnings: []`).
- `bash scripts/docs-check.sh`: ok (408 links, 14 translated pages).
- New cases in isolation (trimmed copies of the scripts in my
  scratchpad: the setup plus only the new cases): service-states 23/0,
  panel-view 21/0.
- Mutants: one per claim, each applied alone, then model.test + the
  trimmed service-states + the trimmed panel-view; restored with
  `git checkout HEAD --` (tree clean afterwards). All 12 killed:

  | Mutant | Killed by |
  |---|---|
  | M1 `captureResult` drops the warnings | model test, service-states 36/36b, panel-view 28 (17 FAIL) |
  | M2 any capture result (also a failed one) sets `captureWarnings` | 36 snapshot 2 (the exit 2 capture must keep the notice) |
  | M3 a clean capture does not clear | 36 final, 36b final, panel-view 28 #7 |
  | M4 the lock wait clears the warnings | 36b snapshot 1 |
  | M5 the notice shows the full text, not the first lines | model test, 36, panel-view 28 #1 |
  | M6 the neutral tone drawn in the accent | panel-view 28 `neutral`, `accentTone` |
  | M7 no full text for the hover | panel-view 28 `tooltip` |
  | M8 the notice not in the panel | panel-view 28 (9 FAIL) |
  | M9 the hover not seen (`HoverHandler` disabled) | panel-view 28 #3 `hovered` |
  | M10 blank warnings kept | model test |
  | M11 the notice gets an action | model test |
  | M12 README States row removed | model test (States test) |

  M6 first **survived**: the shell's default palette has accent ==
  foreground, so the check could not fail. Fixed in `792ff45` (case 28
  runs on Tokyo Night and also asserts "not the accent"); then killed.
  My first M12 (renaming the State column) was a bad mutant: the States
  test matches the Banner column; the real one deletes the row.
- `just check`: see below.

`just check` (run once at the end, at `792ff45`; no other plugin harness
running before the start): **exit 0** on the first run, `check: ok`, no
transient and no re-run. It covers fmt, clippy, the engine tests, watch,
packaging, install.test 209/0, schema, docs-check, plugin-validate,
qmllint, model.test 88, real-home-guard.test 11/0, service-states 297/0,
panel-view 763/0, overlay-view 319/0 and bar-view 143/0. The commits after
it (`71af003` pitfalls, this handover) touch only `memory/` and `work/`.

## Open questions

1. *Open guide*: if the orchestrator wants it, the fixed target would be
   the release-tagged GitHub page of 07 "Back up and restore the state
   directory" (the tag from the engine's version), opened with a fixed
   argv (`xdg-open <constant URL>`). That is a new launcher in the
   plugin and a URL per release, so I left it out as the WP says
   ("otherwise no action").
2. Should the overlay (Prime Radiant) show the notice too? Today it shows
   only the status banner.

## Touched outside scope

- `tests/plugin/harness/panel.qml`: the `hover:<text>` step (needed for
  the "full text on hover" claim).
- `docs/user/{en,de}/10-troubleshooting.md`: besides the states row, one
  sentence in "A state reset was recorded" (the panel now shows it).
- Nothing outside the repository; every harness run checks the real home
  (real-home guard ok).

---

# Round 2 (review: SEND BACK, B1, B2, N1)

Commits on top of `9db6de2` (oldest first): `1a8230d` tooltip wraps and
is bounded (B1, N1), `b7bf87c` panel-view asserts the popup (B2),
`17752da` `tooltipFits` checks the label actually shown (closes a
surviving mutant, see R4), plus the commit with this section.

## What changed

- **B1.** `Banner.qml`: the `PanelToolTip` has `width: root.width` (the
  banner's width, so never wider than the panel) and its own
  `contentItem`: a plain-text `Text` with `wrapMode: Text.Wrap`, colour
  `panelForeground` (= `Color.tooltip.text`), `fontSize` (=
  `Style.font.bodySmall`), the panel font, and the shell's paddings
  (`Border.*(panelBorderSpec)` + `Style.spacing.controlPaddingX/Y`), as
  in the shell's own PanelToolTip. No hard-coded colour or size. Banner
  exposes `tooltipShown` (the popup's `visible`), `tooltipWidth` and
  `tooltipFits` (the tooltip shows this label, and its `contentWidth`
  fits inside the label's width minus padding).
- **B2.** panel-view case 28 steps: `view; hover:Capture warned;
  wait:captureNotice.tooltipShown=true; view; tab:changelog; click:Capture
  now; settle; view`. Asserted: step 1 `tooltipShown` false and the full
  text 0 times in `.texts`; step 4 the full text exactly once in `.texts`,
  `tooltipShown` true, `hovered` true, `0 < tooltipWidth <= notice width`,
  `tooltipWidth <= contentWidth` (the panel frame), `tooltipFits` true.
  The later steps moved by one (5, 8). `view().captureNotice` gains
  `tooltipShown`, `tooltipWidth`, `tooltipFits`, `width`.
- **Not done (a)** in round 1 is corrected above (struck through).
- **N1.** Panel.qml header comment reflowed.

## Verified

- `omarchy plugin validate plugin/`: ok. `just qmllint`: ok (29 files,
  tokens ok). `node tests/plugin/model.test.js`: 88 passed.
- `bash tests/plugin/panel-view.sh` once, at `17752da`: **771 passed,
  0 failed, exit 0**, no transient, no re-run. No full `just check` (per
  brief).
- Mutants (same script and method as round 1; model.test + trimmed
  service-states + trimmed panel-view; restored with `git checkout HEAD
  --`):

  | Mutant | Result |
  |---|---|
  | R1 tooltip `visible: false` | KILLED: #4 full text on screen 0 times, `tooltipShown` false |
  | R2 tooltip width unbounded (`width` line removed) | KILLED: #4 `tooltipWidth <= notice width`, `<= contentWidth` (the wide popup also covers *Capture now*, so #8 fails too) |
  | R3 tooltip text `NoWrap` | KILLED: #4 `tooltipFits` |
  | R4 the shell's text item back (own label not used as `contentItem`) | first **SURVIVED** (`tooltipFits` measured the unused label); fixed in `17752da` (`tooltip.contentItem === tooltipLabel`), then KILLED: #4 `tooltipFits` |

  Round-1 mutants M1–M12 re-run against round 2 at `b7bf87c`: all
  killed. M7 and M9 also re-run at `17752da`: killed (now also by the
  popup asserts).

## Note on the idle check

`pgrep -f '[t]ests/plugin/'` also matches shells whose command line merely
*contains* that text: the orchestrator's waiting shell (it holds this
brief) and the shell of the waiting loop itself. So the bounded loop ran
its full 10 minutes and then started panel-view. I checked the process
list then: the only harness process was my own panel-view; no other
worktree's harness ran. A pattern that matches only harness processes,
e.g. `pgrep -f '^bash tests/plugin/[a-z-]+\.sh'`, would avoid this
(proposal only, not changed anywhere).

## Open

- None. Decisions (lifetime, no action, no overlay, de re-stamp at the
  merge) taken as given; nothing else touched.
