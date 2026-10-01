WP-021 HANDOVER

Branch `wp/021-drift-sheet`, worktree `wt/WP-021`, on top of `main` at
`9c2cdaf`. Not pushed, no PR. Commits `main..HEAD`:
`21838b4` drift helpers and Service.drift · `c7bc19a` DriftSheet on the
Changelog · `ad5ae33` tests and fake engine drift · `9bcc1bd` README and
TESTING · `19d231f` offscreen theme renders · `3fc4b16` memory ·
`2a322f7` handover · then the review follow-up below. `just check` exits 0
at HEAD.

## Review follow-up (after APPROVE)

1. **`drift show` answer ignored when the sheet was opened from a member
   row.**
   - The bug: `fetchMembers()` asks for the leader's id. `showResult`
     compared the answer's `eventId` with the row's id. Opened from a
     non-leader member, with members missing from `index.events`, the
     answer was fetched and then ignored.
   - The fix: `showResult` now matches against `shown.leaderId` (only for
     a group). An answer for an old leader, after `--only` re-keyed the
     group, is ignored too.
   - **New panel-view scenario 19, `drift-show-member`:** on the index
     with noto-fonts missing from `index.events`, Enter on the libinput
     row (cursor 30) opens the sheet. Checked:
     - the sheet names libinput ("Only libinput") and shows the firefox
       group;
     - the engine is asked `drift show <firefox> --json`;
     - all three members are listed.
   - **Negative control:** with the committed `DriftSheet.qml` restored,
     step 5 fails ("… and 1 more" stays; 418 passed, 1 failed). With the
     fix it passes.
2. **Optional, done: no stale draft for a resolved event.** `saveDraft()`
   now returns early when the sheet's result is a completed resolution
   (ok, not pending, not a no-op). `openFor()` and Esc/Cancel therefore no
   longer re-save a draft that `onResultChanged` has just forgotten.
3. **Decisions settled on main** (no longer open):
   - the link picker shows open cases only;
   - the IPC method `resolve` stays;
   - any change to the form disarms;
   - the fixture overlays go to the schema track;
   - the live smoke and theme sweep follow the unlock.

## Done

- **`plugin/components/DriftSheet.qml`**, the drift sheet. It takes the
  Changelog tab's place while it is open.
  - **Opened by:**
    - Enter on an open drift row;
    - the row's new *Resolve…* button;
    - a click on the red strip, which picks the first crisis and puts the
      cursor on its row;
    - IPC `jax.seldon.panel resolve crisis|<event id>` (navigation only;
      kept on main).
  - **Summary card:** glyph, kind, subject and "+N"; zone, plus "crisis"
    when it is one; detail · actor · day and time; the proposed case. A
    group also lists its members, oldest first: "3 packages in one
    transaction:", then the member lines, capped at 8 with "… and N more".
  - **Action picker:** Link · Explain · Dismiss. It starts on Link when the
    engine proposes a case, otherwise on Explain (a crisis "needs a
    reason").
    - **Link:** a case picker. The proposed case comes first and is
      preselected, otherwise "Pick a case", which Link refuses. Then the
      open cases (active, verification, queued).
    - **Explain:** the intent; zone (starts at the item's zone), risk
      (starts at R1), area (optional, slug checked).
    - **Dismiss:** the reason.
    - **Groups:** *All N* or *Only <the row's package>* (`--only`). The
      sheet names the event it was opened from, so a member row's *Only*
      resolves exactly that member.
  - **Writing (the WP-020 pattern):**
    - Enter in a text field, or on the action button, arms the call:
      "Press Enter again: Link firefox and 2 more to C-2026-004". The
      second Enter sends it. A click sends at once.
    - Any change to the form disarms (settled on main).
    - The fields keep their text until the engine has resolved the item.
    - Esc closes the sheet. The draft is kept per event, so another item
      opens with its own defaults.
    - The panel's key catcher is blocked while the sheet has focus. It gets
      the keys back on Esc, Cancel or Close, and also when another tab is
      shown.
    - Without `canWrite` the hint says why (dev mode, no engine, not
      initialised).
  - **After the call:** the result line shows the engine's answer, e.g.
    "Linked 1 event to C-2026-005" or "Explained 1 event · created
    C-2026-009 · new area dev-env", or its refusal. A no-op re-run shows
    "Already resolved: linked to C-2026-005".
    - Once the index no longer lists the item, the card says "Resolved:
      <the folded resolution>".
    - The keys move to *Open C-…* (for link and explain) or *Close*. The
      index may arrive before or after the engine's answer; both orders
      are handled.
  - **Group members:** when `index.events` no longer lists every member,
    the sheet asks `seldon drift show <leader> --json` once (read-only) and
    then lists all of them.
- **Changelog:**
  - Open drift rows (leaders, single items and group members) carry
    *Resolve…*.
  - Enter or a click on an open drift row opens the sheet. Before, they
    expanded a group's members; the sheet now lists them. Other rows still
    expand.
  - Folded resolutions read `linked to C-…`, `explained · C-…: <intent>`
    (ADR-0021) and `dismissed: <reason>`.
  - When `summary.openDrift > drift.length` (ADR-0020), a line above the
    list says "+N more open drift items not listed here".
  - The strip and the pill follow the index; nothing in the plugin counts
    on its own.
- **`plugin/Model.js`:**
  - New helpers: `driftItemFor(index, eventId)`, `caseOptionsFor(index,
    item)` (proposed first), `driftDefaultAction`, `driftArgs` (`{args}` /
    `{error}`), `driftSummary`, `driftResult`, `driftShowResult`,
    `memberLines`, `eventResolution`, `firstCrisis`, `moreDriftText`, and
    `resolutionHead` (shared with `rowStatus`).
  - `validateArgs` now accepts `drift explain <id> [--only] [--zone]
    [--risk] [--area] [--json] -- <text>`, with the options in that order.
    The other drift forms are unchanged.
  - `driftArgs` sends `--zone` only when it differs from the item's zone
    (the engine's default) and `--risk` only when it is not R1. It refuses
    blank or multi-line text (the engine does too) and NUL.
- **`plugin/Service.qml`:**
  - `drift(action, form)` runs through the queue, one drift call at a time.
  - `driftShow(eventId)` is read-only.
  - `driftResult` `{ok, pending, text, action, eventId, caseId, already}`
    and `driftShown` are in the snapshot.
  - A drift error does not set the panel-wide `lastError`; the sheet shows
    it.
- **`Panel.qml` / `BarWidget.qml`:**
  - `editing` includes the sheet. The red strip calls `resolve("crisis")`.
  - `view()` gains `pill`, `changelog.resolved`, `changelog.more` and a
    `drift` block (open, editing, eventId, isOpen, subject, badge, zone,
    members, action, caseId, cases, only, the texts, zone/risk/area, armed,
    hint, result, resultOk, already, pending, resolution, openCase).
  - IPC `resolve`.
- **Tests:**
  - **`fake-seldon`** answers `drift link|explain|dismiss|show` on its
    state index, like the engine:
    - checks and messages: a link's case first, then the event; a blank
      or multi-line text;
    - selection: the group's open members, or the named event alone with
      `--only`;
    - the fold: resolution, detail and case, with explain's case per
      ADR-0021;
    - the item is dropped, or re-keyed to the new leader (lowest-id
      explicit, else lowest id), with `members` dropped below 2;
    - the summary is recounted by the items that went away, so a capped
      index stays truthful, and `proposedEvents` is cleaned;
    - explain creates a completed case;
    - a no-op is `{resolved: 0, events: [], already}`;
    - `$HOME/resolved` stands for a logbook the index lags behind;
      `$HOME/extra-events.json` holds members the index has dropped;
    - `FAKE_SELDON_LOCKED` also covers drift writes.
    - **The real engine on the test host gave the same no-op shape and the
      same "unknown case C-2026-999" message** (see Verified by).
  - **Harnesses:**
    - `shell.qml`: actions `["drift", action, form]` and `["driftShow",
      id]`.
    - `panel.qml`: steps `resolve:<target>`, `click:<text>` (the centre
      of the first visible item with that text, e.g. the red strip) and
      `shot:<name>` (grabToImage to `$HARNESS_SHOTS`), plus the theme
      background under the panel.
    - Step tags are reported with whitespace replaced. A step with a space
      (`type:for the case`) used to break its own report line; earlier
      scenarios just never checked such a step.
  - **`model.test.js`:** 47 tests (+8).
  - **`service-states.sh`:** 148 checks (+31), in 25–27:
    - exact argv of `drift show`; link with the proposed case; explain
      with text `--help` (zone as the item's, so no `--zone`); dismiss of
      a group member with `--only` and `say "hi"; $(reboot)`; explain of
      the rest of the group with `--zone red --risk R3 --area browser`;
      the re-run; a link to an unknown case;
    - the fake's index afterwards: five folded events, one item left,
      summary 1/1, C-2026-009 and C-2026-010;
    - the re-run alone ("Already resolved: linked to C-2026-005");
    - eight refusals that never reach the engine; a held lock; dev mode.
  - **`panel-view.sh`:** 419 checks (+158):
    - The sample run now expects the firefox sheet with 3 members where it
      used to expect the group's expansion.
    - **drift-sample** (dev mode): the four items open the sheet with the
      right defaults (theme item: Link, C-2026-005 first and preselected;
      crises and group: Explain with the item's zone). The group lists 3
      members with "All 3 / Only firefox"; a member row names its own
      package; the strip click opens the first crisis at cursor 7.
    - **drift-capped:** "+246 more open drift items not listed here".
    - **drift-live** (real keys, fake engine):
      1. Enter, Enter, Enter links the theme item.
      2. A click on the strip opens the first crisis. It is explained with
         `--help`, R2 (the change disarms) and `dev-env`.
      3. *Open C-2026-009* opens the new case, and Work lists it as
         completed.
      4. The group is dismissed: three rows `dismissed: routine update`,
         no badge left.
      5. The pill goes 4→3→2→1 and the strip 2→1. The exact argv is
         checked.
    - **drift-only:** Link without a case is refused in the plugin.
      C-2026-004 is picked in the case picker by keys, then *Only
      firefox*. The rest returns as "noto-fonts +1" with 2 members.
    - **drift-already:** "Already resolved", and nothing changes.
    - **drift-locked:** the refusal keeps the text. Esc and reopening
      bring the draft back, and another item gets its own defaults.
    - **drift-show:** "… and 1 more", then `drift show`, then 3 members;
      **drift-show-member** the same from the libinput row (review
      follow-up 1).
- **Docs:**
  - `plugin/README.md`: the sheet, its actions table and keys, the
    folded-resolution wording, "+N more", the strip, IPC `resolve`, and
    the security line (event ids checked; drift texts after `--`).
  - `docs/TESTING.md`, plugin section:
    - layers 1–3 as above, plus the new step verbs and how to make
      offscreen theme renders;
    - smoke step 5 for the drift sheet (fixture logbook with `created` =
      now; the locked-session caveat);
    - IPC `resolve`;
    - the theme sweep now includes the sheet.
- **Screenshots:** `work/active/WP-021/screenshots/offscreen-<theme>-{rows,link,explain,explained,dismiss}.png`
  for Osaka Jade, Tokyo Night and Catppuccin Latte. **These are offscreen
  renders, not live screenshots.** They show the real Panel.qml with the
  shell's own `Commons/` and `Ui/` and each theme's `colors.toml`.
  - Every colour comes from the theme: the stripes (red = urgent, yellow
    = accent), the strip, the armed hint (accent), the urgent Dismiss
    button and the selected chips.
  - Text stays legible on Latte.

## Not done

- **The live part of the runtime smoke (keys, screenshots, the live
  three-theme sweep).** The test host has been `secure: true` since
  15:13:21 (WP-020's lock). Per the brief I sent no `wtype`, ran no `grim`
  and did not run `omarchy-restart-shell`.
  - Consequence: the running shell still runs WP-020's step-2 plugin copy,
    because a restart is the only way to load new plugin code.
  - **I did not copy the new plugin into the test host's
    `~/.config/omarchy/plugins/jax.seldon`.** It cannot load before a
    restart, and the restart must wait for the unlock. The folder holds
    WP-020's final copy, as before.
- **No retro-links to completed or dropped cases from the picker.** The
  engine accepts them, but the WP says "open cases". It would be a
  one-line change in `caseOptionsFor`; settled on main: open cases only.
- **No IPC method that sends a drift call.** That is by design (SPEC §8:
  the panel target never runs the engine).

## Verified by

```
$ just check                                   → exit 0
  fmt-check, clippy, engine tests ok · validate-fixtures ok · plugin-validate: ok
  tokens: ok · qmllint: ok (15 files), --max-warnings 0
  model.test.js: 47 passed · service-states: 148 passed, 0 failed · panel-view: 419 passed, 0 failed
$ find plugin -type l | wc -l                  → 0
```

**Runtime smoke on the test host** (Omarchy 4.0.4-1, quickshell 0.3.1,
theme Osaka Jade, unchanged; session locked throughout).

- **Engine:** a musl release built from this branch. Its `engine/` is
  identical to `main` `9c2cdaf`.
- **Baseline:** none of `~/.local/bin/seldon`, `~/.local/state/seldon`,
  `~/.config/seldon` or `~/Seldon*` existed. The plugin was enabled and
  showed `engineMissing`.

**B. New plugin code + real engine, in a private offscreen Quickshell on
the test host.** This is the panel harness: the host's own
`$OMARCHY_PATH/shell/Commons` and `Ui`, `env -i`, a temp `HOME` and
`XDG_RUNTIME_DIR` under `/tmp/seldon-wp021.*`, the real `seldon` first on
`PATH`. It never talks to the running shell, and no keys reach the real
seat.

1. Setup: `seldon init --non-interactive --path $TMP/Seldon`, then the
   fixture logbook copied over it with `created` set to now. `seldon
   drift --json` listed the sample's four items.
2. Real key presses (QtTest) in the private instance:
   - Enter, Enter, Enter on the theme row → "Linked 1 event to
     C-2026-005", pill `⟡ 2 · 3`.
   - A click on the strip opens the unit file. Typed `--help "smoke"
     text`, then Tab Tab → R2, Tab → `dev-env`, Enter, Enter →
     "Explained 1 event · created C-2026-009". The strip reads "1
     change …".
   - `resolve` firefox, Link, C-2026-004 picked in the case picker, *Only
     firefox*, Enter, Enter → "Linked 1 event to C-2026-004".
   - The new leader noto-fonts: Dismiss "routine update", Enter, Enter →
     "Dismissed 2 events", pill `⟡ 2 · 1`.
   - The run's log has no WARN, ERROR or TypeError.
3. In the logbook afterwards:
   - resolution lines `tokyo-night linked C-2026-005`;
   - the unit file `explained C-2026-009 "--help \"smoke\" text"` (the
     quotes intact, one argv element);
   - `firefox linked C-2026-004` without `meta.txId` (`--only`);
   - `noto-fonts`/`libinput` `dismissed` with `meta.txId
     tx-20260930T214115`.
   - The case file `work/completed/C-2026-009-help-smoke-text.md` has
     zone red, risk R2, area dev-env, `events: [01M3VNJ9…]` and
     `agents: [agent:codex]`.
   - `seldon index --check` is valid, with openDrift 1 and crisis 1.
4. Re-run of the plugin's argv `drift link 01M3VTGN… C-2026-005 --json`
   → `{"already":{"case":"C-2026-005","resolution":"linked"},…,"resolved":0}`,
   exit 0. `drift link 01M3VNFT… C-2026-999 --json` → `{"error":{"code":1,"message":"unknown case C-2026-999"}}`,
   exit 1. Both are exactly what the fake answers.

**A. The real engine at the real paths, the running shell read over IPC**
(old plugin code, see Not done).

1. Setup: `~/.local/bin/seldon`, `seldon init --non-interactive --path
   ~/Seldon-wt`, the fixture logbook copied over it with `created` = now,
   `seldon status`, then `jax.seldon.service refresh`. The service showed
   `ok`, engine present, `canWrite: true`, "nothing new · failing:
   snapper", pill `⟡ 2 · 4`, strip "2 changes …", 62 rows.
2. The plugin's exact argv over ssh, with `view` after each call:

   | Call | Strip | Pill | Rows / badge |
   |---|---|---|---|
   | link theme → C-2026-005 | 2 changes | `⟡ 2 · 3` | — |
   | explain unit `--risk R2 --area dev-env -- '--help "smoke" text'` | 1 change | `⟡ 2 · 2` | `folded` 7→8 |
   | link firefox C-2026-004 `--only` | 1 change | `⟡ 2 · 2` | badge moves to `noto-fonts` |
   | dismiss noto-fonts `-- 'routine update'` | 1 change | `⟡ 2 · 1` | no badge, `folded` 10 |

   - The re-run gives `resolved: 0` with `already`.
   - Four autocommits were made: `seldon: drift linked: 1 event(s),
     C-2026-005` and the three that followed.
   - Every step reached the running panel through the FileView.
   - The old copy shows the badge as "+3"/"+2", WP-020's step-2 formula,
     the known gap.
3. Log of the running shell (`quickshell log --pid`): no jax.seldon line
   except the expected "Process failed to start" from before the engine
   was installed.
4. Restore:
   - I removed `~/.config/seldon` (a plain ssh command), `~/Seldon-wt`,
     `~/.local/state/seldon`, `~/.local/bin/seldon` and the temp dir. All
     are confirmed gone.
   - `jax.seldon.service refresh` → `engineMissing`.
   - The theme is still Osaka Jade.
   - `faillock` still shows only WP-020's single entry from 15:13:40.

## Learned (appended to memory/omarchy-shell.md, "WP-021 findings")

- While the session is locked, the running shell keeps its old plugin
  code. A private offscreen Quickshell on the test host, with the real
  engine on `PATH`, runs the new code end to end without touching the
  seat.
- Deterministic drift for a real-engine smoke: `init`, then the fixture
  logbook copied over it, then `created` = now.
- A `qs.Ui` focusable Button clicks on Enter. For two-press arming, wrap a
  non-focusable Button in a focusable Item.
- A row-wide MouseArea declared last swallows the clicks of inner Buttons.
- A hidden tab can leave a FocusScope "focused"; hand the keys back on
  `visibleChanged`.
- The FileView can deliver the new index before the writing Process has
  exited.
- A non-ASCII `keyClick` under `env -i` (locale C) crashed the harness's
  Quickshell.

## Decisions needed

The first handover's Decisions 1–4 and 6 are settled on main (see Review
follow-up 3):
- the live smoke follows the unlock;
- the link picker shows open cases only;
- the IPC method `resolve` stays;
- any change to the form disarms;
- the fixture overlays go to the schema track.

Still open:

1. **Spec and contract wording (docs not mine):**
   - **CONTRACT.md** lists `drift link <eventId> <caseId> [--only]`,
     `drift explain <eventId> [--only] -- <text>` and `drift dismiss …`
     without `--json`, and explain without `--zone/--risk/--area`. The
     plugin sends the WP's forms: `… --json` and `drift explain <id>
     [--only] [--zone] [--risk] [--area] --json -- <text>`. `validateArgs`
     accepts the options only in that order.
   - **SPEC-PLUGIN §5:**
     - Enter on an open drift row (and a click on it) opens the drift
       sheet. It used to expand a group's members; the sheet now lists
       them.
     - A click on the red strip opens the sheet for the first crisis.
       Before, it only showed the Changelog.
     - The "+N more" line (ADR-0020).
     - The sheet's keys (as in the README).
     - Any change to the form disarms (settled on main).
   - **SPEC-PLUGIN §8:** the IPC method `resolve crisis|<event id>` (kept
     on main).
2. **Guard false positive (reported, not worked around).**
   `scripts/guard.sh` blocked a `python3 - <<'EOF'` file edit as
   "privileged or package command". The edit's text only contained the
   name of the package manager inside a QML comment. Nothing ran. I made
   the same edits with the Edit tool, which the memory note allows for
   file content. Proposed fix: a row in `scripts/guard-test.sh` for a
   heredoc whose body names a package tool but does not run it.
3. **WP-020's leftovers, unchanged on the test host:**
   - the extra `/usr/bin/quickshell` (pid 1377038) is still running next
     to the shell;
   - the running shell's plugin copy still lacks the badge fix.

## Touched outside WP scope

- `memory/omarchy-shell.md`: appended, as the brief asked.
- `work/active/WP-021/`: this handover and 15 offscreen renders.
- `plugin/BarWidget.qml`: one IPC method (`resolve`, kept on main).
  `plugin/` is mine; it is named because it adds IPC surface.
- `engine/`, `schema/`, `fixtures/`, `scripts/`, `justfile` and the other
  `docs/` files were not touched. No new dependency.
