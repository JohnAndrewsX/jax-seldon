WP-020 HANDOVER

Branch `wp/020-work-tab`, worktree `wt/WP-020`, on top of `main` at
`78aa1ff`. Not pushed, no PR. `git merge-tree` against the current `main`
(`f1a07c9`) is clean. Commits `main..HEAD`:
`3104df1` plan helpers and Service.plan · `1656750` Work tab, card, sheet ·
`b6a9b1e` panel tests · `e8056fb` README and TESTING · `dc6d11c` group
badge (orchestrator addition) · `88318e8` offscreen renders and memory ·
then this handover. `just check` exits 0 at HEAD.

## Done

- **`plugin/components/WorkTab.qml`** (tab id `work`, digit `3`, between
  Changelog and System):
  - Three columns: Queued · Active (active, then verification) · Completed
    (completed and dropped, as the index lists them, at most 50).
  - Each case is a tile with the zone stripe, id, steps done/total, title
    (2 lines), *verification* / *dropped* (struck through), and an
    "N proposed" badge from `proposedEvents`.
  - The WIP text "2 / 3 active" counts `cases.active` only. It turns accent
    "at the limit" and urgent "over the limit". The limit is a new bar
    widget setting `wipLimit` (1–20, default 3); see Decision 1.
  - A result line shows the engine's answer or its refusal.
  - *New case* button.
  - With the keyboard, ↑/↓ (j/k) walk the cases column by column. With the
    mouse, a click selects. Hover does not select, so moving to the card's
    buttons does not change the card.
  - The cursor follows a case by id, so after start/verify/done it moves
    to the case's new column. After `plan new` it lands on the new case.
- **`components/CaseCard.qml`**, the card of the case under the cursor:
  - It shows id · status, zone · risk, title, area · priority · steps,
    created/started/closed, and "N proposed event(s)".
  - Actions by status, from `Model.caseActions`: queued → Start; active →
    Verify, Drop; verification → Done, Drop; completed/dropped → Open.
    Every open case also gets Open, last (Decision 3).
  - Keys: Enter runs the first action. Open runs at once. Start, Verify
    and Done need two presses; the first one arms the action and the hint
    "Press Enter again: Start C-…" appears. `x` arms Drop, a second `x`
    drops. A cursor move, another key, a new index or a hidden tab
    disarms.
  - Mouse: a click runs the action, except Drop, which turns into
    "Confirm drop" and needs a second click.
  - `e` opens the case under the cursor in the editor.
  - Without write access the buttons are disabled and the card says why
    (dev mode, no engine, not initialised).
- **`components/NewCaseSheet.qml`** (`+` from any tab, or the button):
  - Fields: title (TextField), zone, risk and priority (`qs.Ui`
    ButtonGroups, starting at yellow / R1 / normal), and area (optional).
  - The area is checked against the schema slug `^[a-z0-9][a-z0-9-]*$`
    in the field and again before the call; see Decision 4.
  - Enter in a text field, or *Create*, sends `plan new --zone <z> --risk
    <r> [--area <a>] [--priority <p>] --json -- <title>`. The title is
    passed exactly as typed. `--priority` is sent only when it is not
    `normal`.
  - The fields keep their text until the engine has created the case (the
    QuickEntry pattern). A refusal shows in the sheet.
  - Esc and *Cancel* close the sheet and keep the draft.
  - While the sheet has focus, the panel's key catcher is blocked. Tab
    walks title → zone → risk → priority → area → Create → Cancel.
- **`plugin/Model.js`**:
  - New helpers: `workColumns`, `workCases`, `wipStatus`, `clampWipLimit`,
    `caseActions`, `caseAction`, `caseMeta`, `caseDates`, `planArgs`
    (returns `{args}` or `{error}`), `planResult`.
  - New constants: `PRIORITIES`, `AREA`, `PLAN_STEPS`,
    `NEW_CASE_DEFAULTS`, `WIP_LIMIT_DEFAULT`.
  - `validateArgs` now accepts `plan new`'s optional `--area <slug>` and
    then `--priority <p>`, in that order only.
- **`plugin/Service.qml`**:
  - `plan(action, input)` builds the call with `Model.planArgs`, runs it
    through the queue, and allows one plan call at a time.
  - `planResult` `{ok, pending, text, action, caseId}` is included in the
    snapshot.
  - A failed plan call does not set the panel-wide `lastError`; the tab
    shows the error itself, as the QuickEntry does.
- **`Panel.qml`**:
  - Work tab wired in; `+` opens the new-case sheet from any tab.
  - `x` (the catcher's delete key) arms Drop on Work.
  - `editing` covers the sheet as well as the QuickEntry.
  - `view()` has a `work` block: columns, ids, wip, cursor, card (actions,
    armed, hint), result, sheet.
  - BarWidget's IPC `tab` accepts `work`. The manifest has the `wipLimit`
    setting.
- **Orchestrator addition:** the drift group badge now reads
  `+(members − 1)`, because `members` counts the leader. The sample's
  3-package firefox group now shows "+2"; 2 members give "+1"; 1 member
  gives no badge.
  - Code: `Model.js` (`changelogRows`) and the `EventRow.qml` comment.
  - Tests: `model.test.js` (+1 test), `panel-view.sh` ("firefox +2",
    "+2").
  - Docs: `docs/TESTING.md` (3 mentions) and `plugin/README.md` ("+N" =
    N more packages).
- **Tests:**
  - `fake-seldon` answers `plan new|start|verify|done|drop` in the
    engine's JSON shapes (`case`, `from`, `to`, `movedFrom`, `activeCase`,
    `journal`, `event`, `git`; `areaCreated` for new).
    - It treats the state index it last wrote as its logbook. A line in
      `$HOME/cases` can override a case's status.
    - It applies the engine's transition rules and uses its refusal
      message, byte for byte (checked against the real engine, see below).
    - Allowed steps rewrite the index atomically, with the case moved to
      its new column. It uses jq, which is now in both harness PATHs.
    - `FAKE_SELDON_LOCKED` makes every plan call exit 4.
  - `harness/shell.qml`: new actions `["plan", action, input]` and
    `["wait"]`.
  - `model.test.js`: 39 tests (+7).
  - `service-states.sh`: 117 checks (+29).
    - Exact argv of `plan new` with title `--help`, and with `Zed "second"
      editor` plus area and priority.
    - Exact argv of start, verify, done and drop.
    - The fake's index after those calls.
    - The refused `done` on an active case: the engine's message is the
      result, `lastError` stays empty.
    - Five refusals that never reach the engine.
    - Lock held (exit 4); dev mode.
  - `panel-view.sh`: 261 checks (+132). The key test now has digit 3 →
    Work, and 4 as the absent tab.
    - **work** (sample, dev mode): Queued 3 / Active 3 / Completed 2 in
      cursor order; "2 / 3 active"; the "1 proposed" badge on exactly one
      tile; cards and actions by status; nothing armed without write
      access.
    - **work-live** (fake engine, real keys):
      1. A new case through the sheet: title `--help`, pickers set by key,
         the area `Dev` refused in the plugin, `dev-env` accepted.
      2. The new case appears in Queued through the FileView, with the
         cursor on it.
      3. Start → verify → done on C-2026-005, Enter twice each; the case
         changes columns after each step ("3 / 3 active · at the limit"
         after start).
      4. Open on the completed case.
      5. **Done on C-2026-008, which the index shows in verification but
         the fake logbook has active: the engine's message "C-2026-008 is
         active; `seldon plan done` needs a case that is verification;
         run `seldon plan verify` first" is the result line, and nothing
         else changes.**
      6. A cursor move disarms; `x x` drops C-2026-004; `e` opens it.
      7. The exact argv of all 11 calls.
    - **work-locked:** a refused new case keeps its title through Esc and
      `+`.
- **Docs:**
  - `plugin/README.md`: Work tab, actions table, keys (`x`, `+`, Enter
    twice, `e` on Work), sheet focus rules, the `wipLimit` setting, IPC
    `tab work`, and the security line (case title after `--`, case ids
    from the index checked).
  - `docs/TESTING.md`, plugin section: layers 1 to 3 as above; smoke
    step 5 for Work; the step 8 note on `~/.config/seldon` (the guard has
    allowed it over ssh since `cff6f8a`); the theme sweep now includes
    Work.
- **Screenshots:** `work/active/WP-020/screenshots/offscreen-<theme>-{work,armed,sheet}.png`
  for Osaka Jade, Tokyo Night and Catppuccin Latte.
  - **These are offscreen renders, not live screenshots** (see Not done).
    They show the real Panel.qml against copies of the installed shell's
    `Commons/` and `Ui/`, with each theme's `colors.toml` in the harness
    HOME.
  - All colours follow the theme: zone stripes red = urgent, yellow =
    accent, green = muted; the proposed badge, the armed button, and the
    selected chips. Dim text stays legible on Latte.

## Not done

- **The panel-driven part of the runtime smoke and the live theme sweep
  were not done.** The test host's session was locked: `omarchy-shell
  lock status` gave `secure: true` from 15:13:21 on, right after my first
  `omarchy-restart-shell`. Decision 2 has what happened.
  - While the screen is locked, the lock screen holds the keyboard, so
    `wtype` cannot reach the panel and `grim` cannot take a screenshot
    (it hangs).
  - No themes were switched.
- **Mouse clicks** (card buttons, Confirm drop, tile selection) were not
  tried live. They call the same functions as the keys; the harness
  covers the keys.
- **`plugin/` has no `Kanban.qml`** (SPEC-PLUGIN §2 lists one). The three
  columns live in WorkTab.qml, as the WP's outputs name them.
- **No "Start agent" action** (in the SPEC-PLUGIN §5 table; not in this
  WP's outputs).
- **Completed shows up to 50 cases** (the WP), not the "last 5" in the
  SPEC-PLUGIN §5 table; see Decision 5.

## Verified by

```
$ just check                              → exit 0
  fmt-check, clippy, engine tests ok · validate-fixtures ok · plugin-validate: ok
  tokens: ok · qmllint: ok (14 files), --max-warnings 0
  model.test.js: 39 passed · service-states: 117 passed, 0 failed · panel-view: 261 passed, 0 failed
$ find plugin -type l | wc -l             → 0
$ omarchy plugin validate ~/.config/omarchy/plugins/jax.seldon   (dev host copy, not enabled) → exit 0
```

**Runtime smoke on the test host** (Omarchy 4.0.4-1, quickshell 0.3.1,
theme Osaka Jade, unchanged):

1. Baseline: no `~/.local/bin/seldon`, `~/.local/state/seldon`,
   `~/.config/seldon` or `~/Seldon-wt`. The plugin was installed and
   enabled.
2. I built the engine from `main` (`8d66b35`, exported with `git archive`
   into a scratch dir, musl release), copied it to `~/.local/bin/seldon`
   and ran `seldon init --non-interactive --path ~/Seldon-wt`. I rsynced
   the plugin (validate ok) and restarted the shell. The service showed
   status `ok`, engine `present`, `canWrite: true`, and the start-up
   capture "nothing new · failing: snapper".
3. Keys over `wtype` did not reach the panel (`view` stayed on Today).
   Cause: the locked session (Decision 2).
4. Instead, I ran the plugin's exact argv with the real engine over ssh
   and read the running panel through IPC (`jax.seldon.panel view`
   works while the screen is locked):
   - `plan new --zone red --risk R2 --area dev-env --priority high --json
     -- '--help smoke: Zed "second" editor'` created C-2026-001 with the
     title intact and committed. The file is
     `work/queued/C-2026-001-help-smoke-zed-second-editor.md`, and the
     panel showed `queued 1`.
   - `plan start` moved the file to `work/active/`; the panel showed
     `active 1` and "1 / 3 active".
   - `plan verify` left the file in place (`movedFrom: null`); the panel
     showed "0 / 3 active", still in the Active column.
   - `plan done` moved the file to `work/completed/` with a journal entry;
     the panel showed `completed 1`.
   - `plan done` on a freshly started C-2026-002 exited 1 with **exactly
     the fake's message** ("C-2026-002 is active; `seldon plan done` needs
     a case that is verification; run `seldon plan verify` first").
   - Every step reached the running plugin through the FileView, without
     a restart.
5. Log: `quickshell log` of the running shell instance has no warning or
   error from jax.seldon, except the expected "Process failed to start"
   probe after the engine was removed.
6. Restore:
   - I removed `~/Seldon-wt`, `~/.local/state/seldon`,
     `~/.local/bin/seldon` and `~/.config/seldon`. The guard allowed the
     last one over ssh. All four are confirmed absent.
   - I rsynced the final plugin (validate ok). The plugin reports
     `engineMissing`. The theme is still Osaka Jade (never changed).
   - **The final `omarchy-restart-shell` refused** ("Refusing to restart
     Omarchy shell while the session is locked"). The running shell
     therefore still runs the copy from step 2. That copy lacks only
     `dc6d11c` (the badge); the files on disk are the final ones.
   - No temp files are left (the hung `grim` was killed and its target
     removed).

## Learned (appended to memory/omarchy-shell.md, "WP-020 findings")

- Check `omarchy-shell lock status` before driving the panel with `wtype`.
  Under a secure lock, keys go to the password field, and each Return is
  a failed unlock attempt counted by faillock. `grim` hangs under the
  lock; IPC still works.
- `omarchy-restart-shell` refuses while a secure locker holds the session.
  If the compositor is locked but no secure locker runs, it restarts and
  locks again.
- Inline components do not share the file's id scope. A parent's
  `visible` bound to its children's `visible` never turns true. Inner
  delegates can see a null outer delegate id during teardown.
- `qs.Ui` ButtonGroup and focusable Button keyboard behaviour.
- Offscreen theme renders: put a theme's `colors.toml` under the harness
  HOME. `grabToImage` leaves out the window colour.

## Decisions needed

1. **Where the WIP limit lives.** No spec, schema or engine config defines
   one. I made it a plugin setting `wipLimit` (default 3; warns, never
   blocks) and count only status `active`, which gives the WP's "2 / 3
   active" on the sample. Please confirm. The alternative is a
   logbook-level limit (config.toml plus an index field, i.e. an ADR and a
   contract bump).
2. **Test host: the smoke ran into a locked session, and I typed into the
   lock screen.**
   - `omarchy-shell lock status` has reported `secure: true` since
     15:13:21, a few seconds after my `omarchy-restart-shell` at about
     15:13:10. I cannot tell whether the session was already locked by
     idle before that (the restart script re-locks a compositor-locked
     session) or became locked then.
   - Before I noticed, my `wtype` input (digits, `+`, a title,
     Tab/Arrow/Return) went to the lock screen.
   - `faillock` recorded **one** failed attempt
     (`omarchy-lock-password`, 15:13:40; deny is 10), so the account is
     not locked out. I sent no more keys after that.
   - Needed from the operator: unlock the test host, run
     `omarchy-restart-shell` once (loads the final plugin copy), and let
     the orchestrator re-run the panel-key smoke (TESTING step 5, Work)
     and the live three-theme sweep. Both take about 10 minutes.
   - Also: after the restart a second process `/usr/bin/quickshell`
     without arguments (pid 1377038, parent `systemd --user`, no IPC dir)
     stayed running next to the shell. I left it alone. Please check
     whether it is expected.
3. **Open on every card.** The WP lists Open for completed cases only. I
   added it last on every card, and `e` opens the case under the cursor.
   Keep, or limit Open to completed?
4. **Spec and contract wording to align (docs not mine):**
   - CONTRACT.md lists `plan new --zone <z> --risk <r> -- <title>` and
     `plan start|verify|done|drop <id>`. The plugin sends `[--area <a>]
     [--priority <p>] --json` (the brief's form plus `--json`, as for
     `log`/`open`) and `plan <step> <id> --json`.
   - The WP writes the area pattern as `^[a-z0-9-]+$`. I used the
     schema's and the engine's stricter `^[a-z0-9][a-z0-9-]*$`.
   - SPEC-PLUGIN §5 should gain the keys `+` and `x` (Drop, twice), and
     Enter twice for writes on Work.
5. **SPEC-PLUGIN §5 says Completed "(last 5)", the WP says last 50.** I
   followed the WP. If 5 is wanted, it is a one-line slice in
   `workColumns`.

## Touched outside WP scope

- `memory/omarchy-shell.md`: appended, as the brief asked.
- `work/active/WP-020/`: this handover and nine offscreen renders.
- `plugin/manifest.json`: the new `wipLimit` setting (plugin/ is mine;
  noted because it adds a user-visible setting).
- `engine/`, `schema/`, `fixtures/`, `scripts/`, `justfile` and the other
  `docs/` files were not touched.
