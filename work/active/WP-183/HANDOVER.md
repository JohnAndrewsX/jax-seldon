# WP-183a — handover

Branch `wp/183-ux-skill` (from `next` at 9d776f9a), worktree `wt/WP-183`.
Part (a) only; part (b) (readability harness) is not started.

## What was done

- **SPEC-PLUGIN, normative rules moved in.** §5.3 gets the house rules for
  keys: the selection is the cursor and a ring only on real `activeFocus`;
  digits are the desk's (WP-181's registry decides the Graph's `0`); one
  meaning per letter, nothing on Super; no undo key without a true inverse
  (`plan reopen` makes a new case, `plan.rs` `reopen`); copying copies the
  id (ADR-0036 §1); every hinted key is a click. §7 gets Colour (five roles,
  `Color.popups.*`, no hex, no fourth hue without `Color.pick` + a SPEC
  line, never colour alone), Geometry (rows and `qs.Ui` controls take
  `Style.cornerRadius`; only stripes, accent bars, heatmap cells square —
  B10), States (Omarchy's state tokens with the corrected defaults),
  Motion (Omarchy's values) and Glyphs (font coverage).
- **`docs/skills/omarchy-ux/SKILL.md`**: a 100-line checklist. Precedence
  AGENTS.md → operator decisions and ADRs → SPECs → skill → generic skills;
  every item cites a SPEC/ADR/AGENTS/CONTRACT line or an Omarchy file:line,
  or is marked *proposal*.
- **Store review boundaries** (§8 of the skill, 12 lines with heading and
  the attribution line): the seven rules of the WP, each with the
  maintainer's public review link as Tom's references give it plus a
  Seldon line or WP. No paragraph copied.
- **AGENTS.md §7**, Plugin (QML), first bullet, the operator's line (E60)
  verbatim.

SYNTHESIS §7 corrections applied to the draft before anything was taken:
fill table (pressed 0.22, selection 0.35, focus = hover 0.08/0.25); Panel
row "not a Seldon surface"; `ConfirmDialog` "not used; if ever
`selectedIndex: 0`, reset on each open"; the pill row without `⏸` and
without "tooltip only for anomalies"; §7 keys as listed above; §8 `plan
done → plan reopen` dropped, `report undo` → `report reopen` (*proposal*,
Reports ADR); §12 (HTML parity) removed; §14 V3/V4 replaced by
`desk-view.sh DESK_SHOTS` offscreen renders with a private HOME, live
checks only on the test host; `omarchy-app` / `visual-verification.md`
not cited ("when shipped"); B10 rounding; glyphs ⏸ ↺ ↻ ⏰.
One further correction found in the code: the draft's "160 ms colour"
is not Omarchy's; `CursorSurface.qml:39` uses 60 ms, `Button.qml:128`
120 ms.

## What was not done

- WP-183b (harness) — not in this assignment.
- The D3 record in the private research file — the orchestrator's (WP text).
- The text-tone derivation rule for SPEC §7 — WP-177's acceptance names it;
  the skill points there.

## How it was verified

- **headless** (dev host, offscreen Quickshell, private HOMEs): `just
  check` with `SELDON_FULL_CHECK=1` on bcdfd00c, private 0700
  `XDG_RUNTIME_DIR` and `JUST_TEMPDIR`/`TMPDIR` on disk, cargo target on
  disk outside the repo, `CARGO_BUILD_JOBS=4`: `check: ok`, exit 0. Cargo
  2786 passed, 0 failed; desk-view 1915/0; service-states 344/0;
  ipc-restart 44/0; model.test.js 195; qmllint ok (49 files);
  plugin-validate ok; validate-fixtures ok; `scripts/docs-check.sh` ok
  (472 links). `/run/user/1000` 2 %, no leftover there.
- **fixture**: none needed (no plugin code or schema changed).
- By hand (dev host, read only): every relative link in the skill
  resolves; the skill is 100 lines, its store block 12; each Omarchy
  file:line was read in `/usr/share/omarchy/shell`; the glyph claim was
  checked with `fc-match monospace` and `fc-list ':charset=…'` (no
  JetBrainsMono face for 23F8, 21BA, 21BB, 23F0).
- `docs-check` covers `docs/user/` and the front pages, not
  `docs/skills/`; the skill's links were checked by hand only.
- **test host**, **CI**, **desktop**: not run (docs-only change).

## Open questions

1. WP-126 lists "one SPEC-PLUGIN §7 paragraph" for B10. That paragraph
   now exists here (Geometry); WP-126 only needs the code sweep (the
   `ListRow.qml` stripe is still a capsule, SPEC says so) and the
   rounding 0/6/16 check.
2. SPEC §5.3's "Digits are the desk's" states the rule; the Graph's `0`
   stays an exception until WP-181's registry test.
