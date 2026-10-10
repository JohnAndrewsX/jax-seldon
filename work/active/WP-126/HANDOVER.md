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

- Screenshots (done in 126b part 1, below). Markers `<!-- TODO(WP-…) screenshot: docs/images/<file>.png; … -->`
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


## 126b part 1 (screenshots, theme sweep, preview.png)

Branch at next 5f6e5b8e plus this part's commits. The test host ran
0.2.0+next.5f6e5b8e (the orchestrator's deploy); I deployed nothing.

### What was done, and where

| Step | Where |
|---|---|
| Backup of the test host's `~/Seldon` (540 entries), `~/.local/state/seldon` (7), `~/.config/seldon` (2) and `~/.config/omarchy/shell.json` into `~/seldon-backup-126b/`, proven by `diff -r`/`cmp` before any change | test host |
| The sample in place: `fixtures/logbook/` as `~/Seldon`, the golden test's state (`cursors.json` with the pacman cursor, the proposal bound to the logbook, `recent-config.json` moved to now), a `config.toml` with every collector off and no autocommit, `seldon index` (83 events, 2 crises), `seldon rules update`, two `seldon log` notes; for Work's "agent working" a launcher that only opens a terminal of class `org.omarchy.agent` and `seldon agent start C-2026-003` | test host |
| 11 of the 12 `docs/images/desk-tokyo-night-*.png` and `plugin/preview.png` (1920×1080, Today at 100 %): live shots, Tokyo Night, rounding 0, cropped, 256-colour PNG, 22–106 KB each | test host |
| `desk-tokyo-night-triage.png`: the proposal's detail opens only on a click and the test host has no pointer tool (only `wtype`), so it is the harness's `shot:` of the real QML on the sample index, cropped above the dev-mode footer | headless |
| Every screenshot marker of this stage (34 in the guide, en and de, and both READMEs) is the picture with alt text that describes the shot as taken; a grep for the stage's marker is empty across the tree (STYLE.md's convention now reads `TODO(WP-NNN)`) | fixture (docs-check: 514 links) |
| `docs/images/panel-*.png` (6) and `overlay-tokyo-night-1920x1080.png` deleted: nothing references them; TESTING.md's preview recipe rewritten for the live shots | fixture |
| Theme sweep: Tokyo Night, Kanagawa, Catppuccin Latte × 100 % and 50 % × rounding 0, 6, 16, Today and Changelog (36 shots), plus the status chip at rounding 0 in Tokyo Night and Latte; kept in the private `refs/live-126b/`, each looked at | test host |
| `python3 tests/plugin/check-tokens.py` on every plugin QML file: ok (990 references, house rules ok) | fixture |
| Restore: `~/Seldon`, `~/.local/state/seldon`, `~/.config/seldon`, `shell.json` copied back from the backup and proven identical (`diff -r`, `cmp`; 540/7/2), no `deskWidth` key (`{"id":"jax.seldon"}`), theme Tokyo Night, rounding 0, `seldon-watch` inactive as before, shell restarted, plugin status ok on the host's own logbook; temp files removed. The backup stays in `~/seldon-backup-126b/` for you to delete | test host |

Note on the method: `omarchy theme set` reloads Hyprland's config and
resets a rounding set with `hyprctl eval`, and the shell reads the
rounding only when it starts. The first pass at 6 and 16 was therefore
really 0; it was shot again as theme set → rounding → `omarchy-restart-shell`
(`getoption` 6 and 16 confirmed before each set).

### Answers

- **F8, the tile at 50 %:** cut. On the live 50 % desk the tile read
  "4/5 steps · claude-code · agent …" (session live, `seldon agent
  sessions` showed it). Fixed: `Model.todayCaseTiles` now puts "agent
  working · " first, as Work's row does, so the elide cuts the agent's
  name (b0fb8968; model.test.js, desk-view `aside-tile`; SPEC, guide 03
  en/de, plugin README, CHANGELOG, TESTING). The deployed host still runs
  the old text; the docs' Today shots were taken without a session.
- **F9, square bars at rounding 16:** yes, the list rows' selection bar
  and stripe and the sidebar's selection bar ran square past the rounded
  rows. An inset would make the sidebar's bar vanish (a 30 px row at
  rounding 16 is a pill), so each bar is now a slice of its row's own
  shape: an Item that clips a rectangle of the row's size and
  `Style.cornerRadius`, square along the straight edge, following the
  corner (3ea19e70; ListRow.qml, Sidebar.qml). The harness reports every
  such bar (`marks.bars`, `marks.barsOutside`); desk-view `rounding-16`
  (a `hyprctl` stub answering 16) and `rounding-0` pass, and the old
  plain-rectangle stripe fails both. Verified headless; a live look at
  rounding 16 fits after your deploy of this branch.
- **The status chip at rounding 0:** the capsule looks right in Tokyo
  Night and Latte, like Omarchy's own chips. But on a 50 % desk its text
  was elided to "▾ 1 not…": even the short form did not fit beside the
  KPI strip in the bar font. Fixed: the chip falls back to the count
  alone ("▾ 1") before it would elide (3ea19e70; Header.qml; desk-view
  `chip-50` and `chip-narrow` at a 920 px window, where headless "1
  notice" stops fitting; the old two-step version fails `chip-narrow`).
  Below about 912 px even "▾ 1" is cut (a screen that narrow only).
- **Catppuccin Latte:** no colour or contrast problem seen at 100 % or
  50 %, rounding 0, 6, 16: primary and dim text, the stripes (urgent and
  accent), the crisis figures, the selection and the chip read clearly.
  Kanagawa (accent near white) is fine too.

### Images

`docs/images/`: `desk-tokyo-night-today.png` (1920×640),
`-50.png` (1120×1080), `-changelog.png` (1920×780), `-triage.png`
(1920×650, headless), `-work.png` (1920×980), `-decisions.png`,
`-system.png`, `-memory.png` (1920×560), `-radiant.png`, `-graph.png`
(1920×1080), `-settings.png` (1920×820), `-setup.png` (1330×320, the
card only: the preview beside it lists the host's own packages);
`plugin/preview.png` (1920×1080). The bar in the shots names the parked
sample-agent window ("Seldon sample agent"); no host name, user or
private path is visible.

**Gate:** `SELDON_FULL_CHECK=1 just check` exited 0 on 18d389f4 (the
last plugin and image commit; only this handover follows, under
`work/`); log `gates/check-dev-126b.log`: check-packaging ok (shellcheck
on the test host through the kit wrapper, 8 calls), install 230/0,
deploy-test-host 327/0, runtime-dir 45/0, docs-check ok (514 links),
plugin-validate ok, qmllint ok (52 files), model.test.js 216,
plaintext.test.js ok, terminal-scripts 73/0, service-states 367/0,
desk-view 2040/0 (with `rounding-16`, `rounding-0`, `chip-50`,
`chip-narrow`), bar-view 196/0, ipc-restart 44/0. Where: headless (dev
host, private runtime dirs on disk), shellcheck on the test host.

## 126c (the triage proposal by key)

Finding (Fable, 126b part 2 Q3.3): the proposal's detail opened only on a
mouse click on its row; no key and no IPC reached it.

| Change | Verified |
|---|---|
| The proposal row is the row above the Changelog's list (`Model.changelogMove`): ↑/`k` on the first row selects it, as a click does, and no list row stays selected (one highlight); ↓/`j` goes back to the row the list was at; a page up stops at the first row | fixture (model.test.js); headless (desk-view `triage-keys`) |
| Enter on the proposal arms *Apply proposals* (hint "Apply N proposed changes as you? Press Enter again. Crises stay, each with its own button."), the second Enter applies it (`drift apply <id> --json`, once); a click still applies at once; on a replaced proposal Enter is *Review the new proposal*. Each crisis keeps its own button, by click only; by key a crisis is resolved from its own row (Link, Explain, Dismiss work by key), as before | headless (`triage-keys`: armed, then done+done, the crisis skipped; exact argv) |
| A dim line under the proposal row names the keys: "↑ from the first row, or a click, opens it"; while shown "Enter twice applies · ↓ back to the list" (`Model.proposalHint`) | fixture; headless |
| IPC: `shell call jax.seldon select proposal` (or the proposal's id, `Model.isProposalTarget`; "not found" in another section), a payload's `"select":"proposal"`, the shim's `resolve proposal` | headless (`triage-keys` steps 9–16) |
| The case against the plugin of 6e4672e0: fails at every step (no proposal by ↑, the row stays selected, no hint, `select proposal` "not found", `resolve proposal` "unknown target") | headless |
| Docs: KEYBINDINGS.md (key row, `select`, shim), SPEC-PLUGIN §5.4 Changelog and §8, plugin README (keys, triage, IPC), guide 03 (key table, triage) and 04 en/de, CHANGELOG | fixture (docs-check, store-readme) |

For the live scenario c: `omarchy-shell jax.seldon.panel resolve
proposal` (or `omarchy-shell shell call jax.seldon select proposal` with
the Changelog shown) opens the proposal, then `wtype` Return twice applies
it; the crisis needs its own button (a click), which `wtype` cannot give.

**Gate:** `SELDON_FULL_CHECK=1 just check` exited 0 on 7dbcd949 (only
this handover follows, under `work/`); log `gates/check-dev-126c.log`:
check-packaging ok (shellcheck on the test host through the kit
wrapper, 8 calls), docs-check ok, qmllint ok (52 files), model.test.js
217, service-states 367/0, desk-view 2056/0 (with `triage-keys`),
bar-view 196/0, ipc-restart 44/0. Where: headless (dev host, private
runtime dirs on disk), shellcheck on the test host. Nothing ran on the
test host's desktop in this stage.
