# WP-167 — Handover

Branch `wp/167-word-table` (from `next` at `63b06112`), worktree
`wt/WP-167`. Not merged. Role: Plugin Dev. Stage (a) of WP-187: the word
table and the rule-4 test, the open part of the section "From the
prototype debate". Plugin only: no contract, schema, fixture, engine or
SPEC change; `contractVersion` stays 2.

**WP-167 can close.** Its ADR part is ADR-0045 (accepted, E39); this
branch builds the table slice that was still open. Stages (b) and (c) —
EASY Today / History / Help, the switch, the start rule and the cue — are
WP-187, which starts on this table.

## What was done

- **The word table** (`plugin/Model.js`, section "Words", right after the
  terminal scripts): `WORDS`, one object keyed `word.* | card.* |
  action.* | confirm.* | help.*`. Each key has `easy` and `pro`, `rows`
  (the rows 1–12 of ADR-0045 §6 it belongs to, in the ADR's order; `[]`
  for §5's vocabulary) and what an action runs, at most one of:
  - `argv` — the engine arguments as CONTRACT.md's command block lists
    them, `<…>` for an id or the user's text, EASY's fixed words where
    ADR-0045 §6 gives them (`drift explain <eventId> --json -- "It was
    me."`, `agent start --new --json -- "Finish the interrupted update
    (event <eventId>)"`);
  - `exec` — a fixed argv constant of Model.js (`RESTART_SHELL_ARGV`);
  - `scripts` — names of the §5.6 terminal scripts (`TERMINAL_SCRIPTS`).
  (Round 2 changed rows 3, 7 and 10 and how an action names what it
  runs; see "Round 2" below.) Every §6 row has keys and at least one
  action that runs something; §5's
  vocabulary (case → task, crisis, open drift, R3, verification) is in
  as `word.*`. EASY-only sentences (the two confirm lines, the agent-at-
  work card, Help's two entries) carry `easyOnly: true` and `pro: ""`.
  `DESK_MODES = ["easy", "pro"]`.
- **`Model.text(key, mode)`**: EASY's column for `"easy"`, PRO's for
  anything else (PRO is what every release so far shows); `""` for a key
  the table does not have (own properties only: `toString`,
  `__proto__` give `""`).
- **PRO reads its own column, unchanged.** These labels now come from
  `text(key, "pro")` with the same strings as before: the banners'
  *Install*, *Create logbook*, *Grant*, *Update*, *Capture now*, *Update
  rules*, *Restart shell*; the setup card's title (`SETUP_TITLE`), *Install*,
  *Create logbook*, *Grant*, *Not now*; the event bar's *Ask agent*,
  *Explain…*; the case bar's *Focus*, *Complete*, *Open in editor*; the
  decision detail's *Open in editor*; in QML Today (*Open in editor*,
  *Start agent*), Changelog (*Capture now*), EventDetail, Work, Memory,
  System (*Open in editor*) and the journal field's placeholder ("Note for
  today's journal, Enter saves"). **No visible PRO subtitles** (struck,
  E48–E63): no surface reads both columns. (Round 1 did not enforce this
  for a read on another line; round 2 does.)
- **Tests** (`tests/plugin/model.test.js`, 8 new, 230 in all):
  - *rule 4, the block*: `contractPatterns` reads CONTRACT.md's "Commands
    the plugin may run" block at test time (comments dropped, every line
    a `seldon` command) into patterns: `<…>` one element that is not a
    flag, `a|b` alternatives, `[…]` optional, `[…]…` repeated. The
    matcher is pinned by 8 argv the contract has and 8 it has not.
  - *rule 4, the table*: every `argv` in `WORDS` fits a line of the block.
  - *rule 4, it fails*: five argv the contract does not list
    (`drift apply-all`, `agent start --new` without `--json`, `drift
    explain` without `--json`, `snapshot undo`, `drift propose`), each
    reported for exactly its key; and the block with the `capture` line
    removed fails `action.capture`.
  - *what the table names is what the desk runs*: each `argv` equals the
    builder the desk calls (`askArgs`, `driftArgs`, `agentFocusArgs`,
    `planArgs`, `agentNewArgs`, `logArgs`, `openArgs`), passes
    `validateArgs`, and Service.qml's two literal runs (`capture`, `rules
    update`) are the table's argv.
  - *scripts and fixed argv*: `exec` is `RESTART_SHELL_ARGV`; every
    script is one of `TERMINAL_SCRIPTS`; a `pending` script may be
    missing, and once its constant exists the test asks to drop
    `pending`.
  - *both columns*: every key has an EASY word and a PRO column (empty
    only with `easyOnly`), rows 1–12 all present, each with an action
    that runs something; §5's EASY words pinned.
  - *text()*: modes, unknown keys and modes.
  - *PRO unchanged, no subtitles*: PRO's words for the keys in use pinned
    to the strings shown before; the event bar, stale banner and restart
    notice labels; every `text("…", …)` call in `plugin/` names a key of
    the table, PRO never reads an EASY-only key, and no line reads the
    same key twice.
  Mutants tried by hand (each restored from git): the table's `plan done`
  argv changed to `plan finish` → 3 tests fail; the `agent focus` line
  removed from CONTRACT.md → 2 fail; a label reading `action.fokus` → 2
  fail (the WP-156 label test and the no-subtitle test).

The checks do not depend on line numbers or on the other entries of
Model.js, the QML files or CONTRACT.md: a WP that adds a contract row, a
section or a key keeps them green; one that drops a row the table names
makes them fail on purpose. **Corrected in round 2:** round 1 said here
that a PRO label renamed without the table fails too; that was false (the
review's m7, the literal "Open in Editor" back in Work.qml, passed every
test and `desk-view.sh`). Since round 2 a PRO word of the table put back
as a literal label in plugin/ QML fails (case and a trailing ellipsis
ignored); a label reworded to new words is still not caught by these
tests (that is a harness check).

## What was not done

- No EASY surface, no `deskMode` setting, no switch, cue or start rule:
  WP-187 (b)/(c).
- **Row 3 "Pick your agent"**: the words are in; since round 2
  `action.pickAgent` names `exec: "PICK_AGENT_ARGV"`, pending (E51:
  Omarchy's menu route; WP-179 builds the constant). Nothing shows it.
- PRO keeps literals that are not §6 rows: *Hand to agent*, *To verification*, *Drop*, *Reopen*, *Link
  to case…*, *Dismiss…*, *New case* (Work), *Copy*, *Check again*. They can
  join the table when EASY needs them (WP-178's STATUS table and
  `emptyState` go into the same section; a comment says so).
- No event-kind plain texts (ADR-0045 §5 "event kinds → plain text"):
  they need the kind list and belong with EASY's History (WP-187 b).

## How it was verified

| Check | Where | Result |
|---|---|---|
| `node tests/plugin/model.test.js` (private HOME and 0700 XDG_RUNTIME_DIR) | fixture | 230 passed |
| the three hand mutants above | fixture | each caught, restored |
| `node tests/plugin/plaintext.test.js`, `model.bench.js` | fixture | ok |
| `omarchy plugin validate plugin/` | fixture (dev host CLI on the tree, no session) | exit 0 |
| `just qmllint` (incl. `check-tokens.py`) | fixture (against the installed shell tree, no session) | ok, 53 files |
| `flock /tmp/seldon-check.lock env SELDON_FULL_CHECK=1 just check` | headless (Quickshell harnesses offscreen, own HOME and runtime dir) + fixture | `check: ok`, exit 0, on ec7ccd00 (the plugin commit; this handover adds only `work/`): cargo tests ok; check-packaging ok; install 229/0; deploy-test-host 326/0; runtime-dir 45/0; check-upstream ok (17 files); docs-check ok; plugin-validate ok; qmllint ok (53 files); model.test.js 230; plaintext ok; terminal-scripts 73/0; real-home-guard 79/0; service-states 366/0; desk-view 2722/0; bar-view 195/0; ipc-restart 43/0 |
| the desk in a live shell | not run (E38: the dev host runs releases only; the test host was not asked for) |

## Open questions (round 1; answered in the review, see round 2)

1. **Row 2's `["omarchy-restart-shell"]`.** ADR-0045 §6 names it a
   "CONTRACT.md row", but CONTRACT.md's block lists `seldon` commands only;
   the argv lives in SPEC-PLUGIN §5.6 and `RESTART_SHELL_ARGV`. The table
   names it as `exec` and the test pins it to that constant. Fine as is,
   or should CONTRACT.md list the plugin's non-engine argv too?
2. **Row 7's *Not done yet* → *Back to active*** (`plan start <id>`): the
   ADR names a PRO action PRO does not show today (a verification case
   has Complete, Drop, Open in editor). The word is in the table; whether
   PRO gains the button is WP-187's call.
3. **Row 5's PRO word**: PRO titles a stopped transaction by status
   (`TX_STATUS_TITLES`); the table's PRO column holds the generic
   "Transaction did not complete", which no PRO surface shows. Its PRO
   action is "New case" (Today's field), as the ADR names it.

## Round 2 (review 1: SEND BACK)

Commits on top of `f76c6053`: the plugin and test changes, the notes in
WP-178/WP-179, this section. Still plugin only, `contractVersion` 2.

### Blocking findings

- **B1, row 7 *Not done yet*.** The engine refuses `plan start` on a case
  in verification (`Transition::Start` only from queued). The words stay
  (*Not done yet* / *Back to active*); `action.notDone` has no argv any
  more, only `pending: "no engine verb takes a case from verification
  back to active (WP-179's ADR)"`. It is out of the builder-parity test,
  which now asserts it has no argv. Rule 4 allows an action without a run
  only for the keys named in the test's `PENDING_WITHOUT_RUN`
  (`action.notDone` alone), and fails a pending key that has an argv.
  Notes added: `work/queued/WP-179.md` (its amendment ADR decides:
  verification → active in the engine, or drop *Not done yet*) and
  `work/queued/WP-178.md` B4 (the way back is open until then).
- **B2, the no-subtitle guard.** `textCalls` scans `plugin/` recursively
  (every `.qml` and `.js`, any depth, multi-line calls).
  `textCallViolations` fails, outside Model.js, any `Model.text` call
  whose key is not a literal key of the table or whose mode is not the
  literal `"pro"`; inside Model.js a literal `"easy"` mode or an unknown
  literal key (a passed-in mode stays allowed for WP-187); and PRO reading
  an EASY-only key. The old one-line rule is gone.
- **B3, the false sentence.** Corrected above. New scan
  `literalProWords`: in plugin/ QML, a string literal in a `text:`,
  `label:`, `title:`, `placeholderText:` or `tooltipText:` line that is
  the PRO word of a key PRO reads (case and a trailing ellipsis ignored;
  only literals written as labels: starting with a capital, not all caps,
  so ids like `"focus"` and headings like `"EXPLAIN"` do not count).
  Work.qml's case *Ask agent* was such a literal; it now reads a new key
  `action.askCase` (`agent ask case <caseId> --json`, rows `[]`, PRO's
  own), same word.

### Non-blocking items, all taken

- **N1** row 3: `exec: "PICK_AGENT_ARGV"`, `pending` (E51). The test
  accepts a pending `exec` while the constant is missing, asks to drop
  `pending` once it exists, then requires an `omarchy-*` fixed argv.
- **N2** rule 4 fails every `action.*` key that runs nothing: exactly one
  of `argv`, `exec`, `scripts`, `settings`, or the pending exception
  above. `settings` is allowed only as `action.notNow: "SETUP_LATER_KEY"`
  (a constant of Model.js). A non-action key that runs something fails
  too.
- **N3** `confirm.agent` rows `[5, 8]`.
- **N4** row 10 has its own keys with the ADR's EASY words:
  `action.askChange` (*Ask agent*) and `action.explainChange` (*It was
  me*), same argv as row 4's `action.askDrift` / `action.explain` (*Ask
  agent to explain*, *It was me…*), which are now row 4 only. PRO's event
  bar keeps reading row 4's keys.
- **N5** the `<…>` matcher refuses any token that starts with `-` (pinned
  with `-x`, `-`, `--only`).
- Row 5's PRO word: comment says PRO does not read it until WP-179's
  Needs-you title replaces it.

**Left to WP-187 (review Q3):** a PRO action id per EASY row (ADR-0045 §9,
"every EASY action id maps to a PRO action id"). The table maps key →
argv; the builder-parity test bridges to what the desk runs. WP-187 adds
the id when it builds EASY's actions.

### Mutants (each applied to the real file, run, restored from git)

| Mutant | Where | Result |
|---|---|---|
| m5: `"action.undoSnapshot": { easy: "Undo it", pro: "Undo", rows: [4] }` added to `WORDS` | fixture | 3 FAIL (rule 4, table; rule 4, argv list; rule 4, runs nothing) |
| m6: `Today.qml:739` `tooltipText: Model.text("action.startAgent", "easy")` | fixture | 1 FAIL (no PRO subtitle) |
| m7: `Work.qml` `text: root.opening ? "Opening…" : "Open in Editor"` | fixture | 1 FAIL (PRO word back as a literal label) |

The guards are also pinned in-memory (no file written): a probe QML in a
new folder `plugin/components/easy/` with an `"easy"` read split over two
lines, an unknown key, a passed-in mode, "Open in Editor" and "Capture
now…" literals, an id, an all-caps heading and a comment.

### How it was verified (round 2)

| Check | Where | Result |
|---|---|---|
| `node tests/plugin/model.test.js` (private HOME and 0700 XDG_RUNTIME_DIR) | fixture | 234 passed |
| m5, m6, m7 above | fixture | each caught, restored |
| `node tests/plugin/plaintext.test.js` | fixture | ok |
| `omarchy plugin validate plugin/` | fixture (dev host CLI on the tree, no session) | exit 0 |
| `just qmllint` (incl. `check-tokens.py`) | fixture (against the installed shell tree, no session) | ok, 53 files |
| `flock /tmp/seldon-check.lock env SELDON_FULL_CHECK=1 just check` | headless (Quickshell harnesses offscreen, own HOME and runtime dir) + fixture | `check: ok`, exit 0, on d361add6 (plugin, tests and the WP-178/WP-179 notes; this handover adds only `work/active/`): cargo tests ok; check-packaging ok; install 229/0; deploy-test-host 326/0; runtime-dir 45/0; check-upstream ok (17 files); docs-check ok; plugin-validate ok; qmllint ok (53 files); model.test.js 234; plaintext ok; terminal-scripts 73/0; real-home-guard 79/0; service-states 366/0; desk-view 2722/0; bar-view 195/0; ipc-restart 43/0 |
| the desk in a live shell | not run (E38) | |
