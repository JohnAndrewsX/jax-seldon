# WP-178 — Handover

Branch `wp/178-status-words` (from `next` at `f0bcf5b2`), worktree
`wt/WP-178`. Not merged. Role: Plugin Dev. Plugin, tests and docs only:
no contract, schema, fixture or engine change; `contractVersion` stays 2.

## What was done

- **B3, the status table** (`plugin/Model.js`, in WP-167's "Words"
  section, right after `WORDS`): `STATUS[kind][status] = { glyph, word,
  tone }`, `word` a key of `WORDS` (so EASY reads its own column).
  Kinds: `drift` (crisis `!` + `word.crisis`, urgent; attention
  `word.openDrift`, accent; in case and routine quiet), `case` (active
  `status.active`, accent; the rest quiet), `decision` (proposed
  `status.proposed`, accent), `tx` (failed / interrupted / unfinished,
  urgent; PRO's words are the engine's values, as WP-137 showed them),
  `collector` (failing, accent). `Model.statusOf(kind, status, mode)` →
  `{ glyph, word, tone, stripe }` or null for a quiet or unknown status.
  New `WORDS` keys: `status.*`, `empty.*`, `link.*` (the key-prefix regex
  of the word-table test takes them; `link.*` run nothing, so they are not
  `action.*` and rule 4 does not apply).
- **The word beside every stripe.** `ListRow` has no settable `stripe`
  any more: it takes `mark` (the status) and draws the stripe from it,
  with the glyph and word before the meta line in the same tone
  (`objectName: "rowStatus"`). The row builders set `mark`:
  `deskChangelog` (crisis / attention), Today's NEEDS YOU, `deskWork`
  (active), `systemTiles` (Collectors), `decisionRows` (proposed; the
  word replaces "proposed" in the meta, not twice). The tx alert reads
  `STATUS.tx`. Routine, in-case and closed rows stay quiet.
- **B7, data states.** `Model.emptyState(section, ctx)` → `{ text,
  action }`: no index ("No index to show"); no match ("Nothing here
  matches “…”." with *Clear search*, beside "or Esc"); Changelog (nothing
  recorded / the open chip / another chip), Work (*New case*, `+`; round 1 said `n`, wrong: review F1),
  Decisions (*New decision*, `d`), Memory, Graph; Today and System only
  on no match. A first step that writes is not offered where the desk
  cannot write (dev mode). `ListColumn` shows the action as one
  `qs.Ui` Button under the text; a click runs `Section.runEmptyAction`
  (`clearSearch` → `Desk.clearSearch`, what Esc does; else the section's
  `emptyFirstStep`: Work `openSheet`, Decisions `openForm`). The Today
  journal's empty row, Work's and the event detail's "No index to show"
  read the table too. No footer freshness line, no "paused". The empty
  detail with the keys waits for WP-181's registry (not done, below).
  Every section's `view()` reports `empty` and `emptyAction`.
- **C11.** "Agent sorts N open changes" → "Ask agent to sort N changes"
  (`Model.triageButton`, `TriageDetail`'s no-proposal line, comments):
  SPEC (every mention), `docs/user/{en,de}` guides (02, 03, 04, 11, 13,
  STYLE.md), `docs/KEYBINDINGS.md`, both READMEs, `desk-view.sh` (3×),
  `model.test.js` (2×). Not changed: ADRs (immutable; C11 is a copy
  change), CHANGELOG history, `packaging/acceptance/v0.2.0.json` (a
  released version's record), `fixtures/skills/seldon-0.2.0/`, the
  engine's skill (open question 2), and the two screenshots' alt texts
  (the images still show the old label; open question 3).
- **C1 guard.** SPEC-PLUGIN §5.3 house rule "Enter-Enter only where there
  is a way back" with the `ConfirmDialog` binding (`selectedIndex: 0`,
  reset in `onOpenedChanged`). `model.test.js`: a grep over `plugin/` QML
  that fails a `ConfirmDialog` block without both (pinned with in-memory
  probes); the SPEC sentence must be there; `Model.caseEnterAction` may
  return only today's Enter steps. See open question 1: the sentence
  cannot be true without naming four existing exceptions.
- **B4 copy.** No toast, no `z`. After *Complete* the result line ends
  "· Reopen makes a new case" (`Model.PLAN_DONE_WAY_BACK`). After *To
  verification* it names no way back (the engine starts a case only from
  queued; WP-179's ADR decides *Not done yet*); a test pins that it says
  no "undo", "not done yet", "plan start" or "reopen" there. SPEC §5.3
  says so.
- **Glyphs.** The two glyphs the plugin used that JetBrainsMono Nerd Font
  lacks: `↻` (reinstall, `TX_GLYPHS`, `TX_SAFE_STEP`) → Material Design
  reload U+F0453 (`Model.TX_REINSTALL_GLYPH`); `⟡` (U+27E1, the
  `seldon` source glyph; the harness found it, the WP named only `↻`) →
  four-pointed star outline U+F0AE3. SPEC §5.4 and §7 updated, the
  "Before you reboot …" text in SPEC too (same words, the new glyph).
- **Glyph-coverage harness** `tests/plugin/glyphs.test.js`, in `just
  plugin-test` and docs/TESTING.md: every non-ASCII character in
  `plugin/`'s `.qml`, `.js` and `manifest.json`, written anywhere or as a
  `\u` escape inside a string literal (a regex's escaped range is not a
  glyph; control and format characters are skipped), must be covered.
  **fc-list is usable:** where fontconfig and the font are installed the
  test reads every JetBrainsMono Nerd Font face with `fc-list --format
  '%{charset}\n' ':family=JetBrainsMono Nerd Font'`, spawned with a
  private HOME, XDG_CACHE_HOME, XDG_CONFIG_HOME, XDG_DATA_HOME and a 0700
  XDG_RUNTIME_DIR under a temp dir it removes; elsewhere (CI has no such
  font) it reads the pinned charset
  `tests/plugin/glyphs/jetbrainsmono-nerd-font.charset`, made 2026-10-13
  with `fc-list --format '%{charset}\n' ':family=JetBrainsMono Nerd
  Font:style=Regular'` under a private HOME and runtime dir (fontconfig
  2.18.3, fontversion 150994; the four faces have the same charset; the
  header says so and how to refresh it). With fc-list it prints a notice
  when the installed faces differ from the pin. `SELDON_GLYPHS_PINNED=1`
  forces the pin. Self-tests: a probe with `↻`, `⏸` in a comment, `↺`
  and `'\u{23F0}'` escapes is caught; a regex range and a `"\u0000"` are
  not.
- **Harness** (`desk-view.sh`, `harness/desk.qml`): the report's `marks`
  gains `statuses` (the visible status words); the Changelog and Work
  cases check the words and that visible stripes and words are as many;
  a new case `data-states` (search without a match in the Changelog, Work
  and Decisions; *Clear search* by click and by Esc; no overflow).
- **CHANGELOG** (Unreleased: two Highlights and a Plugin entry), user
  guides en/de 03 (the word beside the stripe), the de source lines.

## What was not done

- **The empty detail lists the keys:** waits for WP-181's key registry,
  as the WP says. Today an empty detail still says "Nothing selected."
- No EASY surface reads the new words yet (WP-187); the EASY columns of
  `status.*`, `empty.*`, `link.*` are first drafts for WP-187 to judge
  ("in progress", "suggested", "not working", "update failed", "New task",
  …).
- KPI strip, chips and the pill: unchanged (they already carry words or
  counts; the pill's marks are B8, WP-180/NEW-7).
- The engine's triage skill, the screenshots, the ConfirmDialog report to
  Omarchy (E54: the operator's), any change to what Enter does in Work.

## How it was verified

| Check | Where | Result |
|---|---|---|
| `node tests/plugin/model.test.js` (private HOME and 0700 XDG_RUNTIME_DIR) | fixture | 244 passed (234 before; 10 new tests) |
| `node tests/plugin/glyphs.test.js` | fixture (fc-list on the dev host's installed font, private HOME) | 4 passed, against 4 installed faces |
| `SELDON_GLYPHS_PINNED=1 node tests/plugin/glyphs.test.js` | fixture (the pinned charset, as CI reads it) | 4 passed |
| hand mutants, each applied to the real file, run, restored from git (table below) | fixture | 9 of 9 caught |
| `node tests/plugin/plaintext.test.js`, `model.bench.js` | fixture | ok |
| `omarchy plugin validate plugin/` | fixture (dev host CLI on the tree, no session) | exit 0 |
| `just qmllint` (incl. `check-tokens.py`) | fixture (against the installed shell tree, no session) | ok, 53 files |
| `bash scripts/docs-check.sh` | fixture | ok (after the de source lines) |
| `bash tests/plugin/desk-view.sh` (SELDON_FULL_CHECK=1) | headless (offscreen Quickshell, own HOME and runtime dir) | 2740 passed, 1 failed on 79854c36: Decisions' "ADR-0004 · proposed" (the word is now the row's mark); expectation fixed in 21cd8d26 |
| `flock /tmp/seldon-check.lock env SELDON_FULL_CHECK=1 just check` | headless (Quickshell harnesses offscreen, own HOME and runtime dir) + fixture | `check: ok`, exit 0, on 02384d5c (this handover adds only `work/`): cargo tests ok; check-packaging ok (shellcheck not installed: bash -n only); install 229/0; deploy-test-host 326/0; runtime-dir 45/0; check-upstream ok (17 files); docs-check ok; plugin-validate ok; qmllint ok (53 files); model.test.js 244; plaintext ok; glyphs.test.js 4 (fc-list, 4 faces); terminal-scripts 73/0; real-home-guard 79/0; service-states 366/0; desk-view 2741/0; bar-view 195/0; ipc-restart 43/0. The first run on 21cd8d26 failed `check-runtime-dir`: glyphs.test.js named the runtime-dir variable in a comment and as a JS object key; fixed in 02384d5c (`NAME=value` pairs), then green |
| the desk in a live shell | not run (E38: the dev host runs releases only; the test host was not asked for) | |

Mutants:

| Mutant | Caught by |
|---|---|
| m1 `ListRow` status word `visible: false` | "QML: a row's stripe comes only with its word" |
| m2 `STATUS.drift.attention.word` = "" | "STATUS: every tone has a word…", "no row shows a tone without its word" |
| m3 routine Changelog rows get an attention stripe without a mark | "no row shows a tone without its word" |
| m4 a `ConfirmDialog` with `selectedIndex: 0` but no reset in Memory.qml | "C1: …" (`Memory.qml:121 reset`) |
| m5 `TX_REINSTALL_GLYPH` back to `↻` | glyphs.test.js (both sources) and "glyphs: the two that left the font's gaps" |
| m5b `"⏸ paused"` added to System.qml | glyphs.test.js, fc-list and pinned: `System.qml:56 U+23F8 ⏸` |
| m6 Changelog's no-match text back as a literal | "QML: every list's empty text comes from Model.emptyState" |
| m7 no-match without *Clear search* | "emptyState: …" |
| m8 the way back also after verify | "B4: the result line names the real way back" |

## Open questions

1. **C1 as worded is not true of today's desk.** "An action without an
   opposite verb never completes on Enter-Enter from a list": Work's
   *Start* (queued → active), *To verification* and *Complete* run on
   Enter twice from their row (WP-122, operator: "as in 0.1"), and
   *Reopen* on one Enter (WP-101); none has an opposite verb (SYNTHESIS
   §0: `plan reopen` is not the inverse of `plan done`; verification →
   active is WP-179's question). G1-god's premise "Enter-Enter runs only
   steps the contract can reverse" rested on the corrected D3 record. I
   did not change what Enter does (an operator decision); the SPEC
   sentence names the four as existing exceptions, "whether they keep
   Enter is open", and the test pins that no other case step joins them.
   Decision needed (orchestrator → operator): keep the exceptions, or take
   Enter off *Complete* (and *Start*, *To verification*) so they arm by
   click or a letter only.
2. **The engine's triage skill** (`engine/assets/skills/seldon/triage.md`)
   still says "the user clicked *Agent sorts N open changes*". Changing
   it is an engine asset change (installed skills would read as
   outdated); left for an engine WP or the next skill update.
3. **Screenshots** `docs/images/desk-tokyo-night-changelog.png` (and its
   alt texts en/de) show the old label and no status words; a docs pass
   with `DESK_SHOTS` should refresh them.
4. The new glyphs (U+F0453, U+F0AE3) are Nerd Font icons and sit a little
   wider than the arrows beside them in the transaction list; not looked
   at in a live shell (not run, E38).

The round-1 open questions were answered in review 1; see round 2.

## Round 2 (review 1: SEND BACK)

Commits on top of `8d4fade3`. Still plugin, tests and docs only;
`contractVersion` 2. Enter behaviour is unchanged.

### Blocking findings

- **F1, the empty Work list's key.** *New case* on the empty Work list
  takes `+` (`link.newCase`), the key Work's `textKey` opens the sheet
  with; `n` goes to Today's note field (`Desk.textKey`). Fixed in
  Model.js, SPEC-PLUGIN §5.4, CHANGELOG and model.test.js. New test
  "every link.* key does what its button does": for each `link.*` key
  except *Clear search*, exactly one section's `emptyFirstStep(id)` runs
  it, and that section's `textKey` branch for the key calls the same
  function and takes the key (Work `+` → `openSheet`, Decisions `d` →
  `openForm`); *Clear search*'s Esc is checked against `Desk.escapeKey`
  and `Desk.clearSearch`. A probe pins the parser (an `n` branch that does
  not take the key is not a key). New desk-view case `empty-lists`: a
  live index (the fake engine on the sample with no case and no
  decision) shows Work's "No cases yet. …" with *New case* and "or +";
  its click opens the sheet, as `+` does; Decisions shows *New decision*
  and "or d"; its click opens the form, as `d` does; no overflow, no
  settings write.
- **F2, C1.** Text fix only. SPEC-PLUGIN §5.3 names all six steps that
  keep their Enter without an opposite verb: Work's *Start*, *To
  verification* and *Complete* (Enter-Enter), *Reopen* (one Enter), and
  the Changelog's *Apply proposals* and a single crisis (Enter-Enter;
  `drift apply` has no opposite verb). Whether they keep Enter is decided
  with WP-179's verification ADR; the SPEC says so. It also says a button
  the cursor reached (WP-202) is not a list step. New test "C1: no new
  Enter step anywhere": `ACTIVATE_CALLS` pins what every `activate()` in
  plugin/ QML calls (nine files); a new `activate()` or a new call in one
  fails until the SPEC's list names it. The test also checks the SPEC
  names the six and WP-179's ADR.

### Not blocking

- **N1.** The `ConfirmDialog` check takes a reset only inside an
  `onOpenedChanged` handler: its `{ … }` block, or its one statement (up
  to the line's end or a `;` outside parentheses). Probes: the review's
  one-line mutant and a two-line form fail; a block handler and an
  `if (opened) { … }` handler pass.
- **N2.** The glyph scan also reads `\u` escapes in template strings (on
  one line) and the number literals passed to `String.fromCodePoint` and
  `String.fromCharCode`. The probe pins the review's mutant (a template
  `\u{23F0}`, `fromCodePoint(0x23F8)`, `fromCharCode(8635, 65)`).
- **N3.** Not built here: a note in `work/queued/WP-180.md` (The Plan's
  zone stripe needs its word).
- **N4.** Not taken (optional): "No events under “crisis”." stays.

### The review's surviving mutants, and two more

Each applied to the real file after the round-2 commits, run, restored
from git.

| Mutant | Where | Result |
|---|---|---|
| R1 (review): `function activate() { return root.pressAccept() }` in Decisions.qml | fixture | caught: "C1: no new Enter step anywhere" |
| R2 (review, N1): `ConfirmDialog { selectedIndex: 0; onOpenedChanged: console.log(opened); Component.onCompleted: selectedIndex = 0 }` in Memory.qml | fixture | caught: "C1: …" (`Memory.qml:121 reset`) |
| R3 (review, N2): `` `\u{23F0} alarm` `` and `String.fromCodePoint(0x23F8)` in Model.js | fixture (pinned charset and fc-list) | caught: `Model.js:7419 U+23F0`, `Model.js:7420 U+23F8` |
| R4: `link.newCase` key back to `n` | fixture | caught: "emptyState …", "every link.* key …" (`Work.qml textKey takes no "n"`) |
| R5: Work's `+` opens the import form instead of the sheet | fixture | caught: "every link.* key …" (`"+" runs openImport, the button openSheet`) |

The review's caught mutant (Drop as the queued Enter action) and round
1's m1–m8 are unchanged tests.

### How it was verified (round 2)

| Check | Where | Result |
|---|---|---|
| `node tests/plugin/model.test.js` (private HOME and 0700 XDG_RUNTIME_DIR) | fixture | 246 passed |
| `node tests/plugin/glyphs.test.js`, also with `SELDON_GLYPHS_PINNED=1` | fixture (fc-list on the installed font; the pinned charset) | 4 passed each |
| R1–R5 above | fixture | 5 of 5 caught |
| `omarchy plugin validate plugin/` | fixture (dev host CLI on the tree, no session) | exit 0 |
| `just qmllint` (incl. `check-tokens.py`) | fixture (against the installed shell tree, no session) | ok, 53 files |
| `bash tests/plugin/desk-view.sh` (SELDON_FULL_CHECK=1) | headless (offscreen Quickshell, own HOME and runtime dir) | 2756 passed, 0 failed on ea51a1c2 (incl. `empty-lists` 14/0, `data-states` 13/0) |
| `flock /tmp/seldon-check.lock env SELDON_FULL_CHECK=1 just check` | headless (Quickshell harnesses offscreen, own HOME and runtime dir) + fixture | `check: ok`, exit 0, on ea51a1c2 (the handover commit adds only `work/`): cargo tests ok; check-packaging ok (shellcheck not installed: bash -n only); install 229/0; deploy-test-host 326/0; runtime-dir 45/0; check-upstream ok; docs-check ok; plugin-validate ok; qmllint ok (53 files); model.test.js 246; plaintext ok; glyphs.test.js 4 (fc-list, 4 faces); terminal-scripts 73/0; real-home-guard 79/0; service-states 366/0; desk-view 2755/0 (one check fewer than the run alone, which had a second runtime dir to check for leftovers); bar-view 195/0; ipc-restart 43/0. The real-path guard reported "the real ~/.local/state/seldon changed by the operator's live engine (not a leak)" as ok in desk-view (here and in round 1's full check, there also in service-states); the guard attributes the change to the live engine, and I did not look into the real dir |
| the desk in a live shell | not run (E38) | |

### Follow-ups (answers to round 1's open questions)

1. C1's six Enter steps: kept as they are; their Enter is decided with
   WP-179's verification ADR (SPEC §5.3 says so).
2. The engine's triage skill (`engine/assets/skills/seldon/triage.md`,
   "*Agent sorts N open changes*"): the next engine WP that touches the
   skills changes the line; `fixtures/skills/seldon-0.2.0/` stays.
3. The Changelog screenshots and their alt texts (en/de 03): refreshed
   with `DESK_SHOTS` in the next docs pass, in one commit.
4. The width of U+F0453 and U+F0AE3 beside the arrows: WP-180's live pass
   on the test host.
5. The Plan's zone stripe without a word (review N3): WP-180 (noted in
   its file).
