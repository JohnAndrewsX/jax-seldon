# WP-126a — handover (0.2.0 integration, repository part)

Branch `wp/126-integration` in `wt/WP-126`, from `next` 8b3096f8. Stage
126a only: nothing ran on the test host except shellcheck through the
E80 wrapper; no next→main merge, no tag.

## Done

1. **Docs.**
   - SPEC-PLUGIN final pass: §1 manifest 0.2.0 / engineMin 0.2.0; §3 the
     index size check and the plain-text rule; §5.4 Today's tile "· agent
     working"; §7 "Geometry" rewritten for B10 (rows, cards, `qs.Ui`
     controls take `Style.cornerRadius`; stripes, accent bars, progress
     bars and heatmap cells square; the sweep at rounding 0, 6, 16); §8
     the suggested binding is Super+Shift+L (E81, round 2); §9 the reload as measured
     (a reload happens, new code needs a shell restart); §10 why the
     engine is called by name on `PATH`.
   - CONCEPT.md "The plugin surfaces" and glossary, DESIGN-BRIEF §1 (C10).
   - User guide en/de, pages 01–06 and 10–13 and the index, for the desk:
     keys, width and sidebar, setup card and the header's chip, triage,
     graph, one agent per case and "agent working", and 11's "What
     changed from 0.1.x". STYLE.md: terms (desk, section, triage, panel
     only for 0.1.x) and the screenshot TODO marker form. German source
     lines name the English commits.
   - README: the desk, a states table (engine missing, logbook not
     initialised, index missing/unreadable, index stale, each with its
     one click; the header's chip names it), the tour, 0.2.0 status.
   - plugin/README.md: Usage rewritten for the desk (sections, keys,
     Prime Radiant, graph, states, `deskWidth`/`deskSidebar`, IPC with the
     shim), the security argv list brought to CONTRACT.md's 0.2.0 forms
     (six terminal scripts, `init --defaults`), troubleshooting, update
     from 0.1.x. `store-readme.test.sh` green.
   - KEYBINDINGS.md (Accept arms twice; the binding), TESTING.md (§1 the
     plain-text test, §2 the oversized index, §3 the agent-working tile,
     System's tiles, the rounding sweep on the test host), llms.txt,
     AGENT-GUIDE, packaging/store/submission.md.
2. **B10 code:** the list row's and the plan card's stripes and the
   progress bars are square, the banner card takes `Style.cornerRadius`;
   `qs.Ui` controls and rows unchanged (rounded with the theme).
3. **FEEDBACK 11:** `Model.todayCaseTiles` adds "· agent working" to
   Today's case tile from the service's sessions (starting counts, as on
   the Work row); model.test.js and desk-view.sh `aside-tile`.
4. **E72:** `tests/plugin/plaintext.test.js` (178 Text blocks in 52
   files; each block's own `textFormat: Text.PlainText`, no other
   `textFormat` anywhere; allow-list `tests/plugin/plaintext-allow.txt`,
   empty; 178 real mutants + 22 hand-made shapes; in `plugin-test`, so
   CI). Index size: `Model.parseIndex` refuses a text of
   `INDEX_MAX_CHARS` (1 000 000) or more before `JSON.parse`, error
   `size`, the existing "Index unreadable" banner with *Build index* and
   the detail "index.json is 1.2 MB, over the 1 MB the plugin reads;
   rebuild it from the logbook."; model.test.js and service-states.sh
   `index-oversized` (valid JSON of ~1.15 MB).
5. **WP-193 review:** `release-notes.sh` refuses a list item before
   `### Highlights` (`-`, `*`, `+`, `1.`, `2)`, indented); release.yml's
   comments describe the Highlights body; the notes are printed between
   `::stop-commands::<16 random bytes hex>` and `::<token>::`;
   release-notes.test.sh covers both (mutant-checked: removing the rule
   fails 7 cases, removing the stop line fails the workflow check).
6. **Versions:** 0.2.0 in `engine/Cargo.toml`, `Cargo.lock`, manifest,
   `Model.js`; `engineMin` 0.2.0. CHANGELOG: `## [0.2.0] - 2026-10-10`
   with ten Highlights (four Breaking), Breaking (desk replaces popup and
   overlay, the keyboard map, the `jax.seldon.panel` shim until 0.3.0,
   contract 2, linked folders and files), Added, this WP's Plugin and
   Docs lines, check-rss 12 MB (E79); `[Unreleased]` keeps the standing
   paragraph and an empty Highlights; link references updated.
7. **`packaging/acceptance/v0.2.0.json`** skeleton: status `partial`,
   scenarios a–d and `gate` `not run` with their limitations; `commit`,
   `date` and `omarchy` empty for 126b. `acceptance-check.sh 0.2.0`
   today refuses exactly those four fields.

Also, found on the way: the setup terminals' closing lines and
install.sh's next step said "panel" (now "desk"; pins updated); the
manifest's `wipLimit` text said "Work tab"; `seldon doctor --only`'s help
said "panel" (engine string, CLI reference regenerated); docs-check did
not see an option whose long help puts its text on the next line
(`doctor --only`), fixed in `scripts/docs-check.py`.

## Not done (stage 126b, on the test host)

- Screenshots. Markers `<!-- TODO(WP-126b) screenshot: docs/images/<file>.png; … -->`
  (same place in en and de) replace the 0.1 panel images; the old
  `docs/images/panel-*.png` and `overlay-*.png` files are still in the
  tree, now unreferenced by the guide. Files wanted:
  `desk-tokyo-night-today.png` (README of the plugin, guide 01, 03),
  `-setup.png` (01, 10), `-50.png` (01, 03), `-changelog.png`,
  `-triage.png` (03, 04), `-work.png` (03, 04), `-decisions.png`,
  `-system.png`, `-memory.png`, `-radiant.png`, `-graph.png`,
  `-settings.png` (03). `plugin/preview.png` and its alt text in both
  READMEs (marker beside it).
- Theme sweep (Tokyo Night, Kanagawa, Catppuccin Latte at 50/100 %,
  Hyprland rounding 0/6/16), the four live scenarios, filling
  `v0.2.0.json`, next→main, the tag question.

## How it was verified

| Check | Where |
|---|---|
| `SELDON_FULL_CHECK=1 just check` on the final commit | headless (dev host, private `XDG_RUNTIME_DIR`, `JUST_TEMPDIR`, `TMPDIR`, `CARGO_TARGET_DIR` on disk) — result below |
| shellcheck inside the check (check-packaging, -install, -deploy, -guard, -runtime-dir) | test host (E80 wrapper on `PATH`) |
| model.test.js, plaintext.test.js (+ mutants), terminal-scripts.sh, release-notes.test.sh (+ mutants), store-readme.test.sh, install.test.sh, docs-check | fixture |
| service-states.sh (364 passed), desk-view.sh (2027 passed), before the full check | headless |
| `omarchy plugin validate plugin/` | desktop (dev host CLI, read-only) |
| Theme sweep, rounding sweep, live scenarios a–d, WP-194 gate | not run (126b) |

**Gate:** `SELDON_FULL_CHECK=1 just check` exited 0 on e2e3bb40 (the
last code and docs commit; only this handover follows it, under
`work/`). Log in the private `gates/check-dev-126.log`: check-packaging
ok (shellcheck on the test host through the wrapper, 8 calls), install
230/0, deploy-test-host 327/0, runtime-dir 45/0, docs-check ok,
plugin-validate ok, qmllint ok (52 files), model.test.js and
plaintext.test.js ok, terminal-scripts 73/0, service-states 364/0,
desk-view 2027/0, bar-view 196/0, ipc-restart 44/0; no leftover in
either runtime dir. `cargo test` with `CARGO_BUILD_JOBS=4`.

The Quickshell harnesses keep their own `rt=$(mktemp -d /tmp/seldon-rt.XXXXXX)`
(the IPC socket path limit, TESTING.md "The session's runtime dir"); every
other temp dir and the cargo target were on disk.

## Release-notes draft (`bash packaging/release-notes.sh 0.2.0`)

```markdown
**Panel says `omarchy pkg aur add jax-seldon`?** That is plugin 0.1.0,
and the package does not exist yet. Update the plugin first:
`omarchy plugin update jax.seldon` (Omarchy shows the changes and asks
`Update jax.seldon?`; answer yes), then `omarchy-restart-shell`.

### Highlights

- **Breaking:** the bar popup and the fullscreen Prime Radiant are one desk now: nine sections on `1`–`8` and `,`, at the width you choose.
- **Breaking:** update the engine and the plugin together; both read and write index contract 2.
- **Breaking:** the old `jax.seldon.panel` command only forwards to the desk and goes away in 0.3.0; bind `omarchy-shell shell toggle jax.seldon`.
- **Breaking:** a logbook folder or file that is a symbolic link refuses writes; `seldon doctor` names it.
- A fresh install sets itself up from one card on Today: the engine and the logbook in one click each, snapshots optional.
- A new logbook looks back 90 days and records that as history, so your first day starts quiet.
- Your agent can sort every open change into one proposal; you apply it in one click, each crisis on its own.
- Hand a case to an agent from the desk: the desk steps aside, and the case says "agent working" while it runs.
- Section 8 draws your machine's memory as a graph that grows day by day.
- System lists the files you recently edited under `~/.config` and the packages pacman's upgrade skips.

Every change in 0.2.0: [CHANGELOG.md](https://github.com/JohnAndrewsX/jax-seldon/blob/v0.2.0/CHANGELOG.md#020---2026-10-10)
```

## Round 2 (review 1: SEND BACK)

| Finding | Fix | Verified |
|---|---|---|
| F1 Highlight said "no question asked" | "the engine and the logbook in one click each, snapshots optional"; the plugin README's card text names the password of the optional grant | fixture (release-notes.sh 0.2.0 output) |
| F2 step 2 misquoted | README and plugin README quote the card, "Create the logbook" (the plugin README row also names the banner "Create your logbook", which model.test.js's States check needs) | fixture (model.test.js) |
| F3 guide 06 said panel | en/de: *Hand to agent* on the desk, a new case from one sentence, "as from the desk" | fixture (docs-check) |
| F4 size dead end | CHANGELOG claim dropped and the engine's warn-only behaviour stated; the detail now ends "…rebuild it from the logbook, and if it stays, seldon status in a terminal names the largest section."; `Model.indexWarning` + `Service.indexWarned`: one warning per reason and size, not per 5 s re-read | fixture (model.test.js); headless (service-states `index-oversized` 12 s: three reads, one warning; mutant "log every read" → 3 warnings, red) |
| F5 regex-quote hole | the scan's block count must equal a plain `Text {` count on comment-stripped source; a regex-quote mutant in each of the 34 real files with Text blocks, plus two shapes | fixture (the reviewer's own `plugin/ZzMutant.qml` mutant fails) |
| F6 imperative routes | an assignment to `textFormat`, `"textFormat"` (Binding) and `.textFormat` (alias, script) fail as unsupported; one shape each, two passing shapes (a `==` comparison, a commented-out assignment; a URL in a string) | fixture |
| F7 fixed-token mutant | exactly one `stop=` in release.yml, the urandom line | fixture (`; stop=seldon` and `stop=seldon` both red) |
| F10 shim verbs | KEYBINDINGS.md lists `show\|hide` | fixture (docs-check) |
| E81 | Super+Shift+L everywhere (guide 01/03/06 en/de, KEYBINDINGS, both READMEs, SPEC §8, CONCEPT, CHANGELOG), no `hl.unbind` step; STATUS.md E81 after E80; no code or test named the key | fixture (grep: only the two notes that name the old key as Omarchy's Google Maps key) |

F8 and F9 are for the 126b shots (the tile caption at 50 %, square bars
against rows at rounding 16); Q-A (an engine cap on the index budget) is
the orchestrator's follow-up, not in this WP.

**Gate, round 2:** `SELDON_FULL_CHECK=1 just check` exited 0 on 4e3c1816
(only this handover follows, under `work/`); log `gates/check-dev-126-r2.log`:
check-packaging ok (shellcheck on the test host through the wrapper, 8
calls), install 230/0, deploy-test-host 327/0, runtime-dir 45/0,
docs-check ok, plugin-validate ok, qmllint ok (52 files), model.test.js
216, plaintext.test.js (178 + 34 regex-quote mutants, 30 shapes),
terminal-scripts 73/0, service-states 365/0, desk-view 2027/0, bar-view
196/0, ipc-restart 44/0. Where: headless (dev host, private runtime
dirs on disk), shellcheck on the test host.

## Open questions

1. ~~Super+Shift+S~~ decided: E81, Super+Shift+L.
2. **The header's status chip is a capsule** (`radius: height / 2`), as
   are the setup steps' number rings. B10 names only stripes, accent bars
   and heatmap cells; SPEC §7 now records the chip as round by design.
   Should it follow `Style.cornerRadius` like the rows? Judge on the
   126b rounding shots.
3. **CHANGELOG date** is 2026-10-10; if the tag is later, the heading and
   with it the release body's anchor change (`release-notes.sh` reads
   both, no other place).
4. The 0.1 crisis line (`Model.crisisText`) is no longer rendered on the
   desk, only in the service read-out; the guide describes where crises
   show now (header, NEEDS YOU, the detail). Remove the leftover later?
5. ~~The two setup texts~~ fixed in round 2 (F2).
