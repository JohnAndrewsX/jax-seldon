WP-023 HANDOVER

Branch `wp/023-decisions-memory`, worktree `wt/WP-023`, on top of `main`
at `90c3dec`. Not pushed, no PR. Commits `main..HEAD`:
`5bb793e` Model helpers · `1722626` Service.decide · `13c075d` DecisionsTab
and MemoryTab · `b322bb3` tests and fake engine · `385b37a` README and
TESTING · then `work: WP-023 handover and renders` (this file, the
renders, the memory note). `just check` exits 0 at
HEAD (see Verified by).

## Done

- **`plugin/components/DecisionsTab.qml`** (digit 4).
  - The list comes from `index.decisions`, newest first. Each row shows:
    - id and status; *proposed* has the accent stripe and accent status,
      *superseded* is struck through and dim;
    - the title (two lines at most);
    - date · file.
  - Header: "4 decisions · 1 proposed" and *New decision* (key `d`,
    disabled with the write blocker as tooltip when `canWrite` is false).
  - Open: Enter, Space, `e`, a double click, or the row's *Open* button
    (shown on the row under the cursor). Each runs `seldon open ADR-NNNN
    --editor --json`. The id is checked against `^ADR-[0-9]{4}$`; a row
    with another id is shown last and never opened.
  - The cursor follows a decision by id across index changes; after *New
    decision* it sits on the new one once the index lists it.
- **`plugin/components/NewDecisionSheet.qml`** (new file, in the list's
  place while open).
  - One field, the title.
  - Two-press arming: Enter in the field or on *Create* arms ("Press Enter
    again: create the decision “…”"); the second Enter sends; a click on
    *Create* sends at once; any change to the title disarms. This is the
    drift sheet's pattern: the focusable Item wraps a non-focusable
    Button.
  - The plugin refuses a blank, two-line or NUL title itself ("Give the
    decision a title"), before the engine is asked.
  - The title stays until the engine has created the decision, so a
    refusal keeps it. Esc/*Cancel* closes the sheet, keeps the title and
    gives the keys back; `d` brings it back.
  - While the sheet has focus the panel's key catcher is blocked
    (`Panel.editing`). A hidden tab hands the keys back
    (`onVisibleChanged`, WP-021 finding).
- **`plugin/components/MemoryTab.qml`** (digit 6).
  - LESSONS: the `## ` headings of `memory/lessons.md` (`memory.lessons`).
  - TOPICS: each memory file's topic, path and `updated` date, in index
    order (the engine sorts by `updated`).
  - Header: "3 lessons · 2 topics", "Opens the logbook folder; the files
    are in memory/", and *Open*.
  - Enter, Space, `e`, a double click and *Open* all run `seldon open
    logbook --editor --json`. The engine's `open` has no memory target
    (Decisions needed 2). Nothing from the index reaches the argv.
- **`plugin/Model.js`**:
  - new: `decisionRows`, `decisionSummary`, `decisionMeta`, `decideArgs`,
    `decideResult`, `memoryRows`, `memorySummary`, `DECISION_ID`,
    `DECISION_STATUSES`, `MEMORY_OPEN_TARGET`;
  - `OPEN_TARGETS` gains `logbook`;
  - `openArgs` and `validateArgs` take a decision id. `validateArgs`
    already allowed `decide --no-edit [--json] -- <title>`, unchanged.
- **`plugin/Service.qml`**:
  - `decide(title)` and a `decideResult` property `{ok, pending, text,
    decisionId}`. One decide call at a time. Its errors show in place, not
    in `lastError`.
  - After a successful decide, `runnerDone` queues `openInEditor(<id>)`
    with the id from the engine's answer, which `Model.decideResult` has
    checked against the pattern. A refused decide opens nothing.
  - `decideResult` is in the IPC snapshot.
- **`plugin/Panel.qml`**:
  - tabs Today, Changelog, Work, Decisions, System, Memory, in digit
    order; visibility goes by tab id;
  - `editing` includes the decision sheet;
  - `view()` gains `decisions` (rows, cursor, result, sheet) and `memory`
    (rows, sections, cursor);
  - the header comment lists the keys.
  - **`components/Tabs.qml`** got a `fontSize` property (default as
    before); the panel passes `Style.font.caption`, because six tabs at
    bodySmall touched their borders.
  - **`BarWidget.qml`**: only the comment of IPC `tab` names the two new
    ids. The method itself already took any tab id.
- **Tests**
  - **`model.test.js`:** 53 tests (+6 new, 3 extended).
    - The sample's 4 decisions newest first and the 3 lessons + 2 topics.
    - Broken and partial entries.
    - `decideArgs` with `--help`, quotes, `-rf --case …`, `$(reboot)`,
      `--`, padded text; refusals.
    - `decideResult`: the shape, a malformed id is not passed on,
      refusals.
    - `openArgs`/`validateArgs`: the new forms and 12 bad ones.
  - **`fake-seldon`:**
    - `decide` adds the next ADR (proposed, newest first) to the state
      index and answers in the SPEC-ENGINE §3 shape; it writes no ledger
      event, like the engine.
    - `open ADR-NNNN` opens the path the index lists; `open logbook` the
      folder.
    - `FAKE_SELDON_LOCKED` covers decide.
    - WP-022's `agent start` spot is untouched.
  - **`service-states.sh`:** 171 checks (+23), scenarios 28–30.
    - The exact argv of `decide --no-edit --json -- --help` → `open
      ADR-0005`, `decide … -- 'Zed "second" editor'` → `open ADR-0006`,
      `open ADR-0004`, `open logbook`.
    - The fake's index afterwards, and the launcher's four paths.
    - Eight refusals that never reach the engine.
    - Lock held: the decide result, nothing opened. Unknown decision: the
      open result and the panel's error line. Dev mode.
    - Scenario 19 now refuses `open memory` instead of `open logbook`.
  - **`panel-view.sh`:** 519 checks (+100).
    - **keys** rewritten for six tabs: ←/→ wrap over six, `4` and `6`
      select their tabs.
    - **uninit:** both new tabs show "No index to show"; `d` opens
      nothing.
    - **20 decisions** (dev mode): the list and texts, cursor by keys and
      by a click on a title. Enter/`e` are refused with dev mode's reason;
      `d` opens no sheet. Memory shows LESSONS/TOPICS with path and date.
    - **21 decisions-live**, real keys:
      1. `d`, then `--help "q"` typed (no tab switch).
      2. Enter arms with the hint; Backspace disarms; Enter twice sends.
      3. The exact argv: `decide …` then `open ADR-0005`.
      4. The sheet closes, the keys come back, and the cursor lands on
         ADR-0005; "5 decisions · 2 proposed".
      5. Enter, *Open* (click) and `e` open ADR-0003/ADR-0003/ADR-0004.
      6. On Memory, Enter and *Open* open the logbook.
      7. The editor paths are checked.
    - **22 decisions-locked:** a blank Enter is refused in the plugin. The
      engine's refusal stays in the sheet with the title kept; Esc, then
      the tab shows the refusal; `d` brings the title back.
  - `harness/shell.qml`: action `["decide", title]`.
- **Docs:**
  - `plugin/README.md`: tab table, Decisions and Memory paragraphs, keys
    `d`/`e`/Enter, sheet keys, IPC `tab` ids and `view`, the security line
    (decision ids, the decision title after `--`).
  - `docs/TESTING.md`, plugin section: layers 1–3, smoke step 5, the IPC
    tab list, and the theme sweep now includes Decisions and Memory.
- **Screenshots:** `work/active/WP-023/screenshots/`.
  - `offscreen-<theme>-{decisions,sheet-armed,created,memory}.png` for
    Osaka Jade, Tokyo Night and Catppuccin Latte. These are **offscreen
    renders** (the real Panel.qml, the shell's own Commons/Ui, each
    theme's `colors.toml`, the fake engine), **not live screenshots**.
    - The accent stripe and status of *proposed*, the armed hint (accent)
      and the urgent strip come from the theme.
    - Dim text stays legible on Latte.
  - `testhost-offscreen-real-engine-{created,memory}.png`: the same
    offscreen harness on the test host with the real engine (see Verified
    by B).

## Not done

- **Linked cases per decision.** The brief lists "cases" in the row, but
  the contract has none: `decisions[]` in `schema/index.schema.json` has
  `id, title, status, date, path`, with `additionalProperties: false`. The
  engine knows them (`decide --json` reports `cases`; the ADR frontmatter
  has `cases:`). See Decisions needed 1.
- **Opening a single memory file.** Every Memory row opens the logbook
  folder, as the brief says to do meanwhile. See Decisions needed 2.
- **The live part of the runtime smoke** (keys and screenshots on the
  seat, the live three-theme sweep, `omarchy-restart-shell`). The test
  host has been `secure: true` since 15:13:21 (checked again before and
  after my runs). I sent no `wtype`, ran no `grim`, did not restart the
  shell and did not touch the test host's plugin folder, which still holds
  WP-020's copy.
- `CONTRACT.md` / `SPEC-PLUGIN.md` wording (not my files; Decisions
  needed 3).

## Verified by

```
$ just check                                   → exit 0
  fmt-check, clippy ok · engine tests: 292 passed, 0 failed (20 test binaries)
  validate-fixtures: ok — 101 instances (incl. 8 expected failures), 71 ledger events traced, 5 variants, 23 self-checks
  plugin-validate: ok · tokens: ok (384 references) · qmllint: ok (18 files), --max-warnings 0
  model.test.js: 53 passed · service-states: 171 passed, 0 failed · panel-view: 519 passed, 0 failed
  real-home guard: untouched (both harnesses) · plugin-test: ok · check: ok
$ find plugin -type l | wc -l                  → 0
$ omarchy plugin validate ~/.config/omarchy/plugins/jax.seldon   → ok (dev-host copy, not enabled)
```

**Runtime smoke on the test host** (Omarchy 4.0.4-1, quickshell 0.3.1,
theme Osaka Jade, unchanged; session locked throughout, `secure: true`).

- **Engine:** the musl release built from this branch (`just
  build-release`). Its `engine/` is identical to `main` `90c3dec`.
- **Baseline:** none of `~/.local/bin/seldon`, `~/.local/state/seldon`,
  `~/.config/seldon` or `~/Seldon*` existed. The running plugin showed
  `engineMissing`.

**B. New plugin code + real engine, in a private offscreen Quickshell on
the test host** (the panel harness, as in WP-021).

- Setup:
  - temp dir `/tmp/seldon-wp023.*`, with the host's own
    `$OMARCHY_PATH/shell/Commons` and `Ui`;
  - `env -i`, a temp `HOME` with its own `.gitconfig`;
  - `PATH` = the real `seldon`, `git`, the usual tools, and a recorder
    named `omarchy-launch-editor`;
  - the Osaka Jade `colors.toml`.
  - It never talks to the running shell; no key reaches the seat.
- Logbook:
  - `seldon init --non-interactive --path $HOME/Seldon`, then the fixture
    logbook copied over it, `created` = now, `seldon status --json`
    (exit 0);
  - the index lists decisions ADR-0004…0001, 3 lessons, topics `omarchy`,
    `hyprland`.
- Real key presses (QtTest):
  1. `4`: the four decisions, status `ok`.
  2. `d`, typed `Smoke --help "q" title`, Enter → armed, Enter → `Created
     ADR-0005 · Smoke --help "q" title`. The cursor is on ADR-0005;
     "Opened …/decisions/ADR-0005-smoke-help-q-title.md in
     omarchy-launch-editor".
  3. ↓↓, Enter → ADR-0003 opened.
  4. `6`, ↓, Enter → the logbook folder opened.
  5. The run's log is clean.
- Results:
  - The recorder got exactly three launches: the new ADR, `ADR-0003-zed.md`
    and the logbook folder.
  - The file `decisions/ADR-0005-smoke-help-q-title.md` has `title:
    "Smoke --help \"q\" title"` and `status: proposed`; the quotes are
    intact, one argv element.
  - `git log`: `seldon: ADR-0005 proposed` on top of init/status.
  - `seldon index --check --json` → `valid: true`, no warnings.
- The real engine's refusals, the same messages as the fake:
  - `decide --no-edit --json -- $'two\nlines'` → `{"error":{"code":1,"message":"the title must be one line"}}`,
    exit 1;
  - `open ADR-0009 --editor --json` → `{"error":{"code":1,"message":"unknown decision ADR-0009"}}`,
    exit 1.
- `real-home-guard`: the real `~/.local/state/seldon` and
  `~/.config/seldon` are untouched.
- Restore:
  - the temp dir is removed;
  - nothing under `~/.local/bin`, `~/.local/state/seldon`, `~/.config/seldon`
    or `~/Seldon*`;
  - the theme is still Osaka Jade;
  - `faillock` still shows only WP-020's single entry from 15:13:40;
  - the running shell was never touched (no IPC call that writes, no
    restart).

**A (real engine at the real paths, read through the running shell's
IPC)** was not repeated: the running shell has WP-020's plugin code
without the new tabs, so `view` cannot show them. B covers the new code.

## Learned (appended to memory/omarchy-shell.md, "WP-023 findings")

- Six tabs need `Style.font.caption` in `Style.space(380)`.
- `omarchy-launch-editor <dir>` hands the folder to the editor (read from
  the script, not tried on a seat).
- A first-declared row MouseArea with click and double click inside a
  `CursorSurface` works, and the row's Button keeps its clicks.
- Harness: `click:` finds the first visible item; in `shows`, a `"` is
  written `\"`.
- The real-engine smoke recipe works on a locked host. It needs a temp
  `.gitconfig` for the engine's autocommit, and a recorder named
  `omarchy-launch-editor` on `PATH`; the detached launcher inherits the
  harness environment.

## Decisions needed

1. **Linked cases in the Decisions list.** The rows need them, but
   showing them takes a contract change:
   - `decisions[].cases` (array of case ids);
   - an ADR, a `contractVersion` bump, the schema and the fixture
     (WP-016 owns `fixtures/`);
   - engine `index/build.rs` (it already reads `Decision.cases`);
   - in the plugin, one line in `decisionRows` and one Text in the row.
   - Do it, or drop "cases" from the Decisions row?
2. **Engine: an `open` target for memory** (proposal, engine not mine):
   - `seldon open memory` → `memory/lessons.md`;
   - `seldon open memory/<topic>` → `memory/<topic>.md`, with `<topic>`
     matching `^[a-z0-9][a-z0-9-]*$` and the file resolved inside
     `memory/`;
   - in `engine/src/commands/open.rs::resolve` that is one match arm plus
     the check.
   - The plugin side is then `MEMORY_OPEN_TARGET`/`memoryRows` in
     Model.js (topic → `memory/<topic>`, regex-checked) and two
     `validateArgs` forms. Until then, `open logbook`.
3. **Spec and contract wording (docs not mine):**
   - **CONTRACT.md** "Commands the plugin may run":
     - `open <journal|ledger|status|logbook|caseId|ADR id> --editor
       --json` (it lists only journal|ledger|status|caseId);
     - `decide --no-edit --json -- <title>` (it lists the form without
       `--json`; the plugin sends `--json`, which `validateArgs` already
       allowed).
   - **SPEC-PLUGIN §5:**
     - key `d` = new decision on the Decisions tab (my choice: `+` is
       already "new case from any tab", `n` the QuickEntry);
     - Decisions: Enter/`e`/double click/*Open* open the ADR, newest first
       by id, *New decision* with two-press arming;
     - Memory: Enter/`e`/*Open* open the logbook until decision 2 lands;
     - the files list: `DecisionsTab.qml`, `MemoryTab.qml`,
       `NewDecisionSheet.qml`.
   - **SPEC-PLUGIN §8:** `tab decisions|memory`.
4. **"Newest first" means by id** (the engine's numbering, the order
   `index/build.rs` already writes), not by `date`, which the user may
   edit. On the sample both give the same order. Confirm, or sort by date?
5. **The live smoke and the live theme sweep follow the unlock**, as for
   WP-020/021: copy `plugin/` to the test host's plugin folder, run
   `omarchy-restart-shell`, `wtype 4`/`d`/title/Enter Enter, `wtype 6`,
   Enter, and the three themes.

## Touched outside WP scope

- `plugin/components/Tabs.qml`: a `fontSize` property (default
  unchanged). It is not one of WP-022's spots.
- `plugin/BarWidget.qml`: one comment line (the tab ids).
- `tests/plugin/harness/shell.qml`: one action verb.
- `memory/omarchy-shell.md`: appended, as the brief asked.
- `work/active/WP-023/`: this handover and 14 renders.
- **Merge note for WP-022** (no overlap in intent, only adjacent lines):
  - `Model.validateArgs`: I changed the `case "open"` lines; WP-022 adds an
    `agent` case.
  - `Service.runnerDone`/`setResult`: one `decide` line each.
  - `fake-seldon`: a `decide)` arm placed before `drift)`, the `open)` arm
    extended.
  - Expect trivial textual conflicts at most.
- `engine/`, `schema/`, `fixtures/`, `scripts/`, `justfile`,
  `CaseCard.qml` and the other `docs/` files were not touched. No new
  dependency.
