WP-039 HANDOVER

Branch `wp/039-panel-width`, worktree `wt/WP-039`. No PR, no push.

## Done

**1. Panel width `Style.space(460)`** (`plugin/Panel.qml`).
- First-party convention, read from `$OMARCHY_PATH/shell/plugins/`: every
  panel sets `contentWidth: panel.fittedContentWidth(Style.space(n))`. The
  list panels (agents, audio, bluetooth, network, power, tailscale,
  monitor, dropbox) use 380, weather 480, clock 560.
- So there is no single "panel width" to follow, only the pattern: a
  `Style.space` value capped by `fittedContentWidth`. 460 follows that
  pattern and sits between the list panels and weather. It is the WP's
  number and the smallest round value at which the six tabs fit with
  slack (natural sum ≈ 370 + 30 gaps at scale 1.0).
- The overlay is unaffected.

**2. Tabs sized to their labels** (`plugin/components/Tabs.qml`, now a
`Flow`).
- A cell's own width is its Button's `implicitWidth` (label +
  `controlPaddingX` × 2 + border) plus the bold/plain `TextMetrics`
  difference while the cell is not selected. The selected label is bold,
  so selecting a tab never moves the strip; the harness asserts identical
  widths with Today and with Changelog selected.
- If every label fits an equal share, the cells share the width equally.
  Otherwise, as at 460, "Changelog" and "Decisions" are wider than the
  share, and each cell gets its own width plus an equal part of the rest.
  Widths are whole pixels; the last cell takes the rounding rest, so the
  strip ends flush.
- If the six do not fit on one line (a screen narrower than the panel),
  the strip wraps instead of clipping.
- Keyboard digits and keys are unchanged. The tab font stays
  `Style.font.caption`, the same as the chips and header buttons.

**3. Chips.** They were already a `Flow` with `Style.spacing.sm` in both
directions, and zero-count chips stay dimmed. The fix is the width: the
sample's ten chips now take two rows at both scales, not three. No code
change was needed.

**4. Changelog header.** "N events from <source> · newest first" wraps
(`Text.Wrap`) instead of eliding. The sort words are never cut; at a
narrow width the line breaks into two. `objectName: "changelogHeader"`.

**5. Every tab checked at scale 1.0 and 1.25.** The probe is listed under
"Verified by".
- Before the fix, at both scales, the only label overflow on every tab
  was `button:Changelog` and `button:Decisions` (2–4 px short). That is
  the operator's bug.
- After the fix: nothing.
- Today stats row (a `Flow` already), System table, Decisions list and
  Memory topics elide nothing at either scale.
- Changelog row text and Work mini-card titles elide by design. They are
  user content; Enter expands a row, and the card under the cursor shows
  the full title.
- Two extra fixes came from the narrow-screen (300 px) check: the System
  hint "From the index; the full report is STATUS.md" and the Memory
  hint "Opens the logbook folder; the files are in memory/" now wrap
  instead of eliding. At 460 they fit anyway.

**6. Harness.**
- `tests/plugin/harness/panel.qml`: every step report now carries
  `overflow` (`elided:` / `wide:` / `button:` / `outside:` per visible
  text, the last meaning past the panel's right edge) and
  `contentWidth`.
- `harness/KeyboardPanel.qml`: `HARNESS_CARD_WIDTH` stands in for the
  real panel's `availableCardWidth`.
- `Panel.view()` gains `tabStrip: {oneLine, widths}`.
- New case 23 in `panel-view.sh`:
  - `fit-100` and `fit-125` (`[font] base-size` 12 and 15 in the harness
    HOME's `~/.config/omarchy/shell.toml`) walk all six tabs.
  - They assert width 460 / 575, the strip on one line, the same cell
    widths across selection, and no `button:`/`wide:`/`outside:` entry
    on any step.
  - The Changelog header is never elided, and Today, Decisions, System
    and Memory elide nothing.
  - `fit-narrow` (300 px) asserts the strip wraps and no label is cut.
- `PANEL_FIT_SHOTS=<dir>` renders every tab at both scales in Tokyo
  Night, Osaka Jade and Catppuccin Latte.

**7. Renders.** In `work/active/WP-039/screenshots/`:
- `fit-{100,125}-{tokyo-night,osaka-jade,catppuccin-latte}-<tab>.png`,
  36 files;
- `fit-narrow-default-{today,changelog,work}.png`.
- Each is cropped to the panel width, trimmed at the bottom and reduced
  to a 128-colour palette (620 KB in all).
- Offscreen renders, not live screenshots. They use the fake engine on
  the sample fixture, so they have no dev-mode path line.

**8. Docs.**
- `plugin/README.md` ("The panel") describes the width and the wrapping.
- `docs/TESTING.md` documents the label-fit cases and the real-home
  guard exception.
- SPEC-PLUGIN wording, verbatim. It is in **§5**, not §3 (§3 is
  Service.qml; the brief's "§3" pointed at the sentence "Width
  `Style.space(380)` (WP-011)", which lives in §5):

  > Width `Style.space(460)` (WP-039; was 380 from WP-011, the
  > first-party list panels' width, too narrow for six tabs): the shell's
  > `fittedContentWidth` caps it at the screen. Tab cells are at least as
  > wide as their label in bold plus the Button padding (equal shares when
  > every label fits one; otherwise each its own width plus an equal part
  > of the rest; selection never changes the widths); when the six do not
  > fit one line the strip wraps, never clips. Filter chips wrap (`Flow`);
  > the Changelog header ("N events from <source> · newest first") wraps
  > instead of eliding, so the sort order is never cut off. Only user
  > content may elide (Changelog row text, Work mini-card titles; Enter or
  > the card shows it in full); labels never do.

**9. Additional item: the real-home guard and the operator's live
engine.**
- On the dev host, today at 12:00:00, `panel-view.sh` failed with "real
  paths changed during the run". The operator's engine had rewritten
  `~/.local/state/seldon/index.json` (958 KB) and `cursors.json`.
- `tests/plugin/real-home-guard.sh` now reads, before the run, the
  logbook from `~/.config/seldon/config.toml` (a leading `~` expanded),
  config.toml's sha256, and the state index's `logbook.machine`.
- A difference passes as "ok … changed by the operator's live engine
  (not a leak)" only if all of these hold:
  - config.toml is byte-identical and nothing under `~/.config/seldon`
    changed;
  - no path appeared or disappeared;
  - every changed entry is the state dir itself or its `index.json`,
    `lock`, `cursors.json` or `manifest.json`;
  - the post-run `index.json` has `logbook.path` = the config's logbook
    and `logbook.machine` = the pre-run value.
- Anything else fails, and the FAIL line names the reason.
- The guard reads only config.toml and those two index header fields
  (jq), never the logbook.
- Deviation from the brief's wording: the index has no `machineId`. The
  machine is `logbook.machine`, and the logbook's own machine id lives
  inside the logbook, which must not be read. So "still equal" is checked
  against the pre-run index, not against a value from config.
- `real_home_check` no longer aborts its script under `set -e`. A failing
  `diff | sed` used to exit before the summary line; the failure is
  still counted, and the script still exits 1.
- `tests/plugin/real-home-guard.test.sh` (new, run by `just
  plugin-test`) covers 11 cases in scratch HOMEs:
  - untouched → ok;
  - engine rewrite → live, also with `logbook = "~/Seldon"`;
  - FAIL: a new manifest.json, an index for another logbook, another
    machine, a new file, a removed file, a change in `hooks/`, an edited
    config.toml, no config.toml.
- I checked by hand that each FAIL names the right reason.

**10. Review follow-up (review: APPROVE, one follow-up before the
merge).**
- `docs/images/panel-tokyo-night-today.png` is the fresh `PANEL_SHOTS`
  render (Tokyo Night, Today, live fake engine, no dev-mode line).
  `docs/images/overlay-tokyo-night-1920x1080.png` is unchanged, because
  the overlay did not change.
- `plugin/preview.png` was recomposed with TESTING's recipe and the same
  theme and layout: the overlay at 1920×1080 on the left, and the framed
  panel on the right at `+1942+24`.
- The crop is now `460x536`. The old `382` included the tab row's 2 px
  spill past the panel, the bug fixed here; the strip now ends flush.
  536 still ends just below the journal's last entry.
- The framed panel grew from 422×576 to 500×576. To keep the 22 px gap
  and the 36 px right margin, the canvas grew from 2400×1080 to
  **2480×1080**: 153 036 bytes, under 1 MB.
- `docs/TESTING.md`: the recipe now crops `460x536` on a `2480x1080`
  canvas, and the crop paragraph explains the numbers.
- `plugin/README.md`: the alt text says 2480×1080.
- `work/active/WP-039/WP-039.md`: the three "SPEC-PLUGIN §3" pointers now
  read §5.

## Not done

- **No live check.** The dev host's plugin is the operator's; I did not
  copy into it. The test host reported `{"locked":true,"secure":true}`
  (`omarchy-shell lock status`, checked after the final `just check`, about 12:15 CEST), so per ORCHESTRATION §11 I
  sent nothing, copied nothing and restarted nothing. The offscreen
  renders are the record. The live check is still to do on the dev host
  (copy into the dev install, or the next plugin release) and on the
  test host once it is unlocked.
- Chips: no code change (see Done 3).

## Verified by

```
overflow probe, sample fixture, before the fix (380, base 12 and 15), every tab:
  ["button:Changelog","button:Decisions"]                           (+ content elisions)
after: tabs at 460 / 575, every tab, both scales                     → []
after, [spacing] scale-with-font = false at base 15                  → []
after, HARNESS_CARD_WIDTH=300: strip on two lines, header on two lines, no label cut
$ bash tests/plugin/real-home-guard.test.sh                          → 11 passed, 0 failed
$ PANEL_FIT_SHOTS=<dir> bash tests/plugin/panel-view.sh              → panel-view: 877 passed, 0 failed
$ just check                                                         → exit 0, "check: ok"
    qmllint: ok (28 files), tokens: ok (520 references), plugin-validate: ok
    real-home-guard.test: 11 passed · service-states: 189 passed · panel-view: 681 passed · overlay-view: 314 passed
test host: omarchy-shell lock status                                 → locked: true, secure: true (stopped)
follow-up:
$ PANEL_SHOTS=<dir> bash tests/plugin/panel-view.sh                  → panel-view: 690 passed, 0 failed
$ magick identify plugin/preview.png                                 → PNG 2480x1080, 153036 bytes
$ just check (follow-up tree)                                        → exit 0, "check: ok"
    real-home-guard.test: 11 passed · service-states: 189 passed · panel-view: 681 passed · overlay-view: 314 passed
    "ok   service-states: the real ~/.local/state/seldon changed by the operator's live engine (not a leak)"
    (a real capture by the operator's engine landed during that script: the guard's live-engine path, hit for real)
```

The first full panel-view run after adding case 23 (12:00) failed on the
real-home guard alone, because of the operator's capture at 12:00:00.
That is the reason for item 9.

## Learned (appended to memory/omarchy-shell.md and memory/pitfalls.md, "WP-039")

- First-party panel widths: 380 for list panels, 480 weather, 560 clock;
  `fittedContentWidth` caps at the screen.
- A qs.Ui `Button` never elides; a narrower width lets the label spill
  over. Its label is bold only while selected, so size cells with the
  bold width.
- Font scale offscreen: `[font] base-size` in
  `$HOME/.config/omarchy/shell.toml`.
- `Text.truncated` + `contentWidth` + `mapToItem` give a generic label-fit
  probe. `Flow` keeps exact-sum widths on one line.
- Measure, don't eyeball. The 380 renders looked fine while the Buttons
  were 2–4 px short.
- The real-home guard on a host with a live Seldon; `real_home_check`
  under `set -e`.
- The test host's ssh alias is in memory/local.md, not `test`.

## Decisions needed

- None.

## Touched outside WP scope

- `tests/plugin/real-home-guard.sh`, the new
  `tests/plugin/real-home-guard.test.sh` and the `justfile`
  (`plugin-test` runs the self-test). These are the operator's additional
  item for this WP.
- `plugin/components/SystemTab.qml` and `MemoryTab.qml`: one hint line
  each, from elide to wrap. This is within "every other tab checked".
