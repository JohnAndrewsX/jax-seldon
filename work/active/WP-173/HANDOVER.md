# WP-173 — Handover (round 2)

Branch `wp/173-held-key` from `next` (07740c35). Not pushed (the
orchestrator pushes). Round 1 ended at 38411ecc; review 1 (APPROVE WITH
NITS) asked for N1, N2 and N3 in this WP and kept the form fix (Q3).

## What was done

### The rule (N3: no repeat triggers any writing key, armed or not)

**The desk** (`plugin/Desk.qml`, `plugin/Model.js`): keyCatcher's
`Keys.onPressed` calls `Desk.keyPressed(event)`. It drops every
auto-repeat before `root.key` unless the key moves:
`Model.deskKeyRepeats(key, text)` — the arrows (with or without Alt),
PageUp/PageDown, Home, End (Qt key codes), `j`/`k`, the Prime Radiant's
`h`/`l` and the graph's `-`/`=`. A dropped repeat is accepted and
changes nothing: no arm, no confirm, no write, no launch, no disarm — so
a held `x` keeps its arm and hint until it is released and pressed
again. This replaces round 1's `Arm.held` (Arm.qml is back to `next`'s
version): the filter now sits before `root.key`, the WP's second
option, and covers `r` (Reopen), `c`, `e`/`o`, Enter/Space (activate,
the graph's play), `p`, `0`, `f`/`F`, the section keys, `/`, `+`, `n`,
`i`, `d` and Esc as well. A whitelist, so a writing key added later is
covered without a change here.

**The forms and fields** (no field acts on Qt's `accepted` any more,
which a repeat emits too): Return and Enter go through the owner's
`keyPressed(event)`, which accepts the key and acts only when it is not
an auto-repeat —
- the arm-twice forms (round 1): `DriftForm` (Link, Explain, Dismiss),
  `NewDecisionForm`;
- the one-Enter fields: `JournalField` (the note), `NewCaseSheet` (both
  fields), `ImportForm` (both fields: the dry run), the intent fields of
  Work and Today (the guard on the `TextField` itself).

**The writing buttons** (`plugin/components/desk/KeyButton.qml`, new): a
focusable qs.Ui Button clicks on every Return/Enter/Space press, repeats
included (`$OMARCHY_PATH/shell/Ui/Button.qml:66-68`), so a held Space on
*Import* imported again on every repeat (pending was the only guard).
KeyButton is the arm-twice forms' `submitKey` pattern made reusable: a
Tab stop around a non-focusable Button, the key presses it once per
press, a click is a click, `enabled: false` makes it no Tab stop.
NewCaseSheet's *Create* and ImportForm's *Dry run* and *Import* use it.
The remaining focusable qs.Ui Buttons are the four *Cancel* buttons
(harmless: they close a form).

### The routing proven by real keys (N1)

Every guard has `keyGuard` (a name) and `keyEvents` (its call count). The
harness reports `keyGuard: { name, events }` of the guard the focus is in
(the nearest item up from the focus with `keyPressed` and a `keyGuard`,
else the desk) after every step. A real QtTest `keyDown:` must raise that
count, which proves the Keys handler under test routes into the guard;
`keyRepeat:` hands the same guard the event an auto-repeat is. New step
`focusName:<objectName>` focuses a KeyButton. A static check in
desk-view.sh covers the fields no case presses: no `onAccepted` in
`plugin/`, every Return/Enter/Space handler ends in `keyPressed(event)`
(except the search field's, which only leaves the field), the desk's
handler is `root.keyPressed(event)`, and the focusable qs.Ui Buttons are
as many as the *Cancel* buttons.

Cases (`tests/plugin/desk-view.sh` 8d'):
- `held-key`: as round 1 (Drop with 70 repeats, Start, Hand to agent,
  j/k/↓ repeat and disarm), plus the real keys counted at the desk, plus
  repeats of `r`, `e`, `c`, Return, Space, Esc on a completed case doing
  nothing (desk open, case completed, no engine call), then a real `r`
  reopens. Argv: exactly drop, start, agent start, reopen.
- `held-forms`: as round 1, plus the real Return counted at the
  decision's and the drift form's guard.
- `held-fields` (new): the note, Today's intent, the new-case sheet's
  title and *Create* (Return and Space repeats), Work's intent — repeats
  send nothing, the real press sends once and counts. Argv: one `log`,
  one `agent start --new` each, one `plan new`.
- `held-import` (new): repeats in the path field run no dry run, the real
  Return runs one; repeats on *Dry run* none more; Return/Space repeats
  on *Import* import nothing, a real Space imports once. Argv compared.
- `model.test.js`: `deskKeyRepeats` (moving keys yes; Return, Enter,
  Space, Esc, Tab, Backspace, the writing/launching letters, section
  keys, `/`, `+`, `f`/`F` no).

Mutations, each in a scratch copy (deleted after), the 8d' block alone:
- M5 the decision title back to `onAccepted: root.enterKey()`: 2 FAIL
  (held-forms #6, static check);
- M6 keyCatcher's old inline body: 8 FAIL;
- M7 the desk filter off: 16 FAIL;
- M8 KeyButton ignores `isAutoRepeat`: 4 FAIL;
- M9 *Import* back to a focusable qs.Ui Button: 2 FAIL;
- M10 Work's intent back to `onAccepted`: 2 FAIL.

### Docs (N2 and the rest)

SPEC-PLUGIN §5.3 (the rule as built now, replacing round 1's `Arm.held`
paragraph) and §2 (KeyButton in the file list); docs/KEYBINDINGS.md;
docs/user/en/03-daily-use.md and docs/user/de/03-daily-use.md (one
sentence each; the German source line moved to the English commit
d49a6794, so docs-check sees it in sync); plugin/README.md;
docs/TESTING.md (the new steps and `keyGuard`); CHANGELOG (the entry now
"A held key acts once").

## What was not done

- **The live check on the test host** (hold `x` on a queued case for two
  seconds → armed, not dropped): needs a dev build deployed to
  pbbau-lnx-tstr, the orchestrator's deploy step. Worth adding there:
  hold Space on *Import* after a dry run → one import.
- QtTest still cannot make a real auto-repeated QKeyEvent (Qt 6.11's
  `QTest::sendKeyEvent` always passes `repeat = false`), so the repeat
  itself stays synthetic; the real-key counters and the static check
  close the routing gap the review found.

## How it was verified

- `SELDON_FULL_CHECK=1 just check` on 3a3e121e: `check: ok`, exit 0 —
  desk-view 1878 passed / 0 failed (70 of them the 8d' held-key checks),
  service-states 344/0, bar-view 196/0, ipc-restart 44/0,
  terminal-scripts 65/0; the real `~/.local/state/seldon` and
  `~/.config/seldon` untouched, no leftover in either runtime dir.
  docs-check prints one existing warning (the German CLI reference is
  behind the English page), not from this WP.

- The four 8d' cases alone in a scratch copy of the script: 73 passed,
  0 failed; the mutations above.
- `node tests/plugin/model.test.js`: 192 passed. `just plugin-validate`
  ok, `just qmllint` ok (49 files; no new warning in the changed files).
- TMPDIR, the cargo target (the worktree's `engine/target`) and
  XDG_RUNTIME_DIR (0700, removed after) on disk under the private
  `gates/` folder; the harnesses' own Quickshell runtime dir stays
  `mktemp -d /tmp/seldon-rt.*` as the repo designs it (WP-161).

## Open questions

1. `f`/`F` (the Changelog's source filter) and the section keys no longer
   repeat: they do not write, but they do not move a selection either.
   If the operator wants them to repeat, they go into
   `Model.deskKeyRepeats` (one line and the unit test).
