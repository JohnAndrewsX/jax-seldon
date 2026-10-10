# Merge of main into next (0.2.0 integration branch)

Base: `next` at ac5bc9f9; merged `origin/main` (54 commits: 0.1.4 release
prep, WP-117/118 setup texts, WP-130 guard, CI and test fixes, the RSS
bound of 11 MB, AGENTS.md §1/§6, STATUS). Merge base 94e1fab2. Merge
commit 47ad5b34, then a follow-up commit for the German source line (see
below).

Why: PR #7 (next → main) conflicted, so GitHub ran no CI on `next`.

Rules applied: next's design wins (the desk, contract 2); main's facts win
(0.1.4 is released history; its texts and fixes apply where the desk has
the same surface).

## Versions

`next` was at 0.1.3, older than the released 0.1.4, so the versions are
0.1.4: `engine/Cargo.toml` and `Cargo.lock`, `plugin/manifest.json`
`version`, `Model.js` `PLUGIN_VERSION` (all auto-merged from main).
`manifest.json` `seldon`: `contractVersion` 2 (next) and `engineMin`
"0.1.4" (main). next had kept the base's "0.1.0"; main raised it because
the 0.1.4 plugin needs the 0.1.4 engine's commands, and next's plugin
contains that plugin code. An older engine also writes contract 1, so the
contract-mismatch banner shows first anyway. `packaging/PKGBUILD` stays at
0.1.3, as on main.

## Content conflicts

| File | Resolution |
|---|---|
| `CHANGELOG.md` | `[Unreleased]` holds only next's own entries (the diff base → next was pure additions: Breaking, Engine, the WP-127 Plugin entry), with their headings. Below it main's `[0.1.4] - 2026-10-07` section verbatim (highlights, the entries that were Unreleased at the base, WP-117/118), then the older releases. The link references come from main (`[Unreleased]` compares from v0.1.4). |
| `docs/SPEC-PLUGIN.md` | The §6 conflict: next's "Prime Radiant — desk section 7" kept; main's "Overlay.qml" §6 dropped. Main's §5 (panel) changes ported to the desk's §5.6: the WP-117 banner states (one-sentence details, *Install*/*Create*/*Grant*/*Update*, the hover text of the snapper banner, no "press Check again" hint), the new "Terminal scripts (WP-117)" paragraph, "1 event today". Next's wording kept where the desk differs ("under the header", "when the desk opens", the crisis strip has no successor). §3 and §10 auto-merged. |
| `docs/TESTING.md` | The plugin-test row lists next's scripts plus main's `terminal-scripts.sh`; no `panel-view.sh`/`overlay-view.sh`. |
| `docs/user/de/10-troubleshooting.md` | Only the source line conflicted; the body auto-merged both translations. Both sides had changed the English page, so neither side's commit matches the merged page: the follow-up commit points the line at the merge commit, which keeps `docs-check` free of the staleness warning. |
| `justfile` | Both sides' recipes: `check-guard` in `check` (main), `check-rss` with `rss_stays_under_11_mb` (main), `terminal-scripts.sh` and `desk-view.sh` in `plugin-test`. The plugin-test comment names the desk and the terminal scripts. |
| `plugin/manifest.json` | See Versions. next's description and desk settings kept. |
| `tests/plugin/model.test.js` | contractMismatch: main's test (script, actions, sentence form "… and this plugin reads v2: update the …", main's text) with contract-2 numbers (v3 → plugin, v1 and v0 → engine). todayView: next's values `[32, 53, …]` plus main's singular check. Also, outside the markers: the "one banner per non-ok status" test passed `indexContractVersion: 2`, which under contract 2 is the engine side, whose script is checked by name elsewhere; changed to 3 (the plugin side, as main meant). |
| `tests/plugin/service-states.sh` | main's fix-contract case (Copy and the terminal with `UPDATE_PLUGIN_SCRIPT`) on next's `index.contract-v3.json`. Outside the markers, next's two expectations of the contract-mismatch detail now use main's sentence form. The `old-plugin` case keeps the v0.1.3 tag's text (that plugin's own code). |

## Modify/delete conflicts

`plugin/Panel.qml`, `tests/plugin/panel-view.sh`, `tests/plugin/overlay-view.sh`:
deleted on next by the desk (WP-121), changed on main by WP-117. Removed
(`git rm`); main's changes ported:

- `Panel.qml`: main added `snapperTip` (the snapper banner's hover text,
  shown, fits) to the harness view. The desk now has the same:
  `components/desk/Notices.qml` exposes `snapperBanner`, `Desk.qml`'s view
  has `snapperTip`.
- `panel-view.sh` scenario 4 (snapper: Grant, Copy, Check again, one
  sentence, hover text, Grant launches `SNAPPER_FIX_SCRIPT`, no hint) →
  `desk-view.sh` 7a, rewritten to the same checks.
- `panel-view.sh` 4b (one event today) → `desk-view.sh` 8a' `one-event`;
  `Model.deskToday`'s tile now says "event today" for 1 (main had changed
  only the panel's `todayView`).
- `panel-view.sh`/`overlay-view.sh` "Create your logbook" title and
  `restart-updated` order → `desk-view.sh` 7b, 7c and 9f (radiant-uninit).
- `overlay-view.sh`'s "Fix it from the Seldon panel" line: no successor
  (the overlay window is gone; the desk shows every fix).
- `tests/plugin/COVERAGE.md`: rows 4 and 4b updated.

## Open question

Main's terminal scripts (pinned verbatim in `model.test.js`, review items
under AGENTS.md §8) say "In the Seldon panel, press Check again." and "The
panel updates by itself." On next the surface is the desk. Left unchanged
here (a script text change is a review item, not a merge decision); a
follow-up WP can reword them to "desk".

## Verification

`node tests/plugin/model.test.js` (148 passed) and
`bash tests/plugin/terminal-scripts.sh` (65 passed) before the commit; the
full `just check` result is in the handover reply (log
`check-merge-main.log`).

Gate round 2: `service-states.sh` old-plugin expected the v0.1.3 wording
on both paths, but without git (an archive copy) the case falls back to
this plugin at contract 1, which prints main's new sentence. The
expectation now follows the path the case took (tag → the 0.1.3 wording,
fallback → this plugin's wording); both paths were run (worktree with the
tag, `git archive` copy without `.git`).
