# WP-173 — Handover

Branch `wp/173-held-key` from `next` (07740c35). Not pushed (the
orchestrator pushes).

## What was done

**The rule, in one place for the desk** (`plugin/components/desk/Arm.qml`,
`plugin/Desk.qml`): the key handler's body moved from `keyCatcher`'s
`Keys.onPressed` into `Desk.keyPressed(event)`. It sets `arm.held =
event.isAutoRepeat` around `root.key(event)` (reset in a `finally`, so a
click never sees it). `Arm.press` with `held` set marks the key as
touched and returns false: it neither arms nor confirms, and since the
key "pressed", the desk does not disarm — the arm and the hint stay in
the sticky bar while the key is held. Every caller of `arm.press` (Work's
Start/Verify/Done/Drop/Hand to agent via keys and Enter, Decisions'
Accept, the triage Discard) follows without change; clicks go through
`press` with `held` false, unchanged. Keys that never reach `press`
(j/k, arrows, Alt+arrows, section keys, the graph's `-`/`=`, `h`/`l`)
repeat as before, and disarm as any other key.

**Also fixed, same class (beyond the WP's file list — please check the
scope):** the two forms with their own Enter-twice arm,
`DriftForm.qml` (Link, Explain, Dismiss) and `NewDecisionForm.qml`
(New decision). Their text fields handled Enter by `onAccepted`, and
Qt's TextInput emits `accepted` for an auto-repeated Return too: holding
Enter in the decision title created the decision; holding Enter on an
open drift row opened the form, then armed and ran Link. Return/Enter in
the fields and Return/Enter/Space on the submit item now go through the
form's `keyPressed(event)`: accept the key, call `enterKey()` only when
it is not an auto-repeat. `enterKey()` itself is unchanged (service-states
still calls it directly).

**Tests** (`tests/plugin/desk-view.sh`, section 8d'; harness
`tests/plugin/harness/desk.qml`): new steps `keyDown:`, `keyUp:` (real
QtTest press/release) and `keyRepeat:<Name|char>`. QtTest cannot make an
auto-repeated event, so `keyRepeat` builds the event object a repeat is
(`key`, `text`, `modifiers`, `isAutoRepeat: true`) and hands it to
`keyPressed(event)` of the focused item's nearest ancestor that has one
(a form), else the desk's.

- `held-key` (live, fake engine): on an active case, repeats alone arm
  nothing; `x` down arms Drop, 70 repeats (two seconds at rate 40) and the
  release leave it armed, hint and "Confirm drop" shown, case still
  active; a real `x` drops. Same for Enter (Start, 10 repeats) and `a`
  (Hand to agent). Armed Drop, then repeated `j`, `j`, ↓, `k` move the
  selection and disarm. Engine argv: exactly one `plan drop`, one `plan
  start`, one `agent start`.
- `held-forms` (live): New decision — repeated Enter in the title arms
  nothing, Enter down arms, 10 repeats and release keep it armed, a real
  Enter creates (argv: one `decide`). Changelog — Enter down on an open
  drift row opens the Link form, 11 repeats leave it unarmed, two real
  Enters arm and link (argv: one `drift link`).
- Against `next`'s plugin (with only a `keyPressed` shim added to the old
  Desk so `keyRepeat` can reach it), `held-key` fails 11 checks: the held
  `x` drops the case at once, held Enter and `a` arm then disarm/confirm.
  `held-forms` cannot show the old form bug this way (the old fields'
  `onAccepted` comes from Qt's TextInput, which the synthetic event does
  not pass through); the new routing is covered, the old failure is by
  reading Qt's behaviour.

**Docs:** SPEC-PLUGIN §5.3 (the sentence "A held key does not confirm",
plus the forms), plugin/README.md (two-press arming), docs/TESTING.md
(the new harness steps), CHANGELOG (Unreleased › Plugin).

## What was not done

- **The live check on the test host** (hold `x` on a queued case for two
  seconds → armed, not dropped) is not done: it needs a dev build of the
  plugin deployed to pbbau-lnx-tstr, which is the orchestrator's
  deploy step. The harness case above is its offline equivalent.
- Held keys that run **without** arming are unchanged (out of the WP's
  scope): `r` (Reopen runs at once), `e`/`o` (open in the editor, guarded
  by the 2 s repeat window), `c` (capture), Enter in the one-Enter fields
  (Today's note, the Work intent field, the new-case sheet, the import
  form's dry run). Each is guarded by a pending state or the step-aside,
  but a held key still sends its first repeat if that guard is not yet
  up. Worth a look before 0.2.0 if the operator wants "a held key never
  writes" rather than "never confirms".

## How it was verified

- `SELDON_FULL_CHECK=1 just check` — result below; TMPDIR
  and XDG_RUNTIME_DIR on disk under the private `gates/` folder (0700,
  removed after). The harnesses' own Quickshell runtime dir stays
  `mktemp -d /tmp/seldon-rt.*` as the repo designs it (socket path
  length, WP-161); not changed here.
- The two new cases alone, iterated in a scratch copy of the script (40
  checks, 0 failed), and once against `next`'s plugin (fails, see above).
- `omarchy plugin validate plugin/` and `just qmllint` clean (no new
  warning in the changed files).

Result of `SELDON_FULL_CHECK=1 just check` on 615bb747+docs (e61b67b4):
`check: ok`, exit 0 — desk-view 1845 passed / 0 failed (37 of them the
new held-key and held-forms checks), service-states 344/0, bar-view
196/0, ipc-restart 44/0, terminal-scripts 65/0; the real
`~/.local/state/seldon` and `~/.config/seldon` untouched, no leftover in
either runtime dir. docs-check prints one existing warning (the German
CLI reference is behind the English page), not from this WP.

## Open questions

1. Scope: the form fix (DriftForm, NewDecisionForm) goes beyond the WP's
   input list but is the same bug and the same rule. Keep, or split into
   its own WP?
2. The held keys that write without arming (above): a follow-up WP, or
   accepted as is for 0.2.0?
