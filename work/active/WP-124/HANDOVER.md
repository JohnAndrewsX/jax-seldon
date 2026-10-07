# WP-124 — HANDOVER, stage 124a (engine, ADR-0036, skill, docs)

Branch `wp/124-triage` from `next` at `a1198b2`; merges into `next`.
Stage 124b (the desk) waits for this review. Decisions in
[PLAN.md](PLAN.md) (D1–D12); the ADR states them normatively.

## Done

- **ADR-0036** (proposed) "Agent prompts carry identifiers, never logbook
  text; proposals are evidence or nothing"; DECISIONS.md row.
- **`seldon agent ask triage|drift <EVENT>|case <ID> [--launcher NAME]
  [--json]`** (`engine/src/commands/agent.rs`): the same launcher checks,
  folder rule and launch as `agent start`; the prompt is fixed text + the
  checked id + the logbook path + the absolute path of the installed
  skill guide (`triage.md`/`drift.md`/`case.md`), shaped like
  `omarchy-agent-crash`'s (skill by name, file as fallback). Env
  `SELDON_LOGBOOK`, `SELDON_ACTOR=agent:<launcher>`, `SELDON_ATTENDED=1`;
  **no `SELDON_CASE`** (a caller's value is removed), no active-case
  change, no lock (D3). Refusals, nothing launched, exit 1: malformed or
  unknown id, `drift` on an event that is not open drift, `triage` with
  nothing open, no Omarchy default agent (fix named), no installed skill
  holding the guide (fix `seldon hook install skills`) (D2, D4).
- **`seldon drift propose [--file F] [--actor A] [--json]`** (stdin JSON;
  `engine/src/commands/triage.rs`): strict input (unknown fields refused,
  so an agent cannot set `crisis` or `text`), 1–200 items, 1–10 refs;
  item checks (open drift, leader stored, no duplicate, link case
  exists, explain title/intent one line and bounded, redacted); evidence
  resolution for `journal|event|snapshot|case|plan` with engine-written
  text (D7); all-or-nothing, the refusal names item position and id;
  agent actor only; stored 0600, schema-checked before writing; replaces
  this logbook's earlier proposal and says so (D9); index rebuilt.
- **`seldon drift apply <PROPOSAL> [--item <EVENT>]… [--actor A]
  [--json]`**: reads the file as the index does; per item from a fresh
  derive: no longer open → skipped; crisis (engine class now OR file
  flag) only by `--item`; every ref resolved again, the file's `text`
  never read; written through `drift link|explain`'s own path
  (refactored into `drift::write_resolution`) with `resolutionDetail`
  `proposed by agent:<name> — <kind> <ref> "<text>"; …`; actor human
  only (agent actor and `--actor human` in an agent's session refused);
  `applied` set once by a run without `--item`; one autocommit; `--json`
  `{proposal, applied, done, skipped, refused, git}` (D8, D11).
- **`seldon drift discard <PROPOSAL>`** (D10, needed by the desk's
  Discard; not named in the WP).
- `sys::write_atomic_replace` (an atomic write that never follows a link
  at the target), used for the proposal file.
- **Skill**: `triage.md` (read, what counts as evidence, crisis stays the
  user's, store and stop); `drift.md` "When the User Asks About One
  Change"; SKILL.md topic guide; `FILES` 6.
- **Docs**: SPEC-ENGINE §2, §3 (commands, JSON shapes, the prompts), §5
  "Triage"; CONTRACT.md argv table (`agent ask …`, `drift apply …`,
  `drift discard …`; "the plugin never runs `propose`"); guides 04 and
  05 en/de (help blocks regenerated); CHANGELOG.

## Verified

- `engine/tests/triage.rs`, 12 tests on copies of `fixtures/logbook/`
  in a temp HOME: storage, engine text and crisis, leader, index
  `triage` (schema-valid index and proposal), replacement, 21 refusal
  cases + shape refusals + human proposer; apply (user actor, ledger
  details, explanation case title/intent, crisis held back, idempotent
  second run with ledger byte-identical, `--item` crisis), engine decides
  the crisis (flag false/true both ways), file text ignored and stale
  evidence refused, a vanished case refused; apply/discard refusals
  (agent actor, env agent, `--actor human` in agent session, foreign
  `--item`, unknown, malformed, other logbook, schema-invalid, symlink)
  with the ledger unchanged; discard; ask: argv/prompt exactly the ADR's
  text for all three, env (`case=unset` although the caller had
  `SELDON_CASE`), active case unchanged; no logbook text in any prompt
  (every event subject/detail, case title, journal and Plan text of the
  fixture checked); every `seldon …` span of the prompts exists (`--help`
  + flags) and runs as written; refusals (no default agent, no skill,
  nothing open) launch nothing.
- Mutants (`work/active/WP-124/mutants.py`, runner script; restores
  files): **14 of 14 killed** — subject/title in a prompt, `SELDON_CASE`
  inherited, crisis by flag only, by engine only, file text in the
  detail, file evidence trusted, self-evidence, agent may apply, applied
  rewritten, human may propose, routine proposable, member not stored as
  leader, unapplied proposal kept.
- Skill tests updated for six files (counts derived from `FILES`).
- `flock /tmp/seldon-check.lock just check` → `check: ok`, exit 0, at
  `4680f68` (log `check-wp124-r2.log`; r1 was stopped while it waited for
  the lock, before it ran). Host-only steps (plugin validate, qmllint)
  ran on the dev host.

## Not done / open

- **124b** (the desk): next stage.
- **Live check on the test host**: needs the desk's one click (124b);
  not run in 124a.
- `drift apply` derives once per written item (≤ 200 derives; seconds in
  the worst case). Not measured on the x150 scale; fine for the expected
  5–20 items.
- An engine error inside one item's write after its ledger line (a case
  file that cannot be written) is reported as `refused` for that item
  and the run continues; the ledger line stands and a re-run skips it.

## Questions for the reviewer

1. D3: no `SELDON_CASE` for ask sessions, so Claude Code's hooks do not
   record an ask session started outside the logbook. Intended (an ask
   is not a case session); say if the desk should record them anyway.
2. D9/D10: one proposal per logbook, and `drift discard` as a new fixed
   argv. Both go beyond the WP's literal list.
3. D11: crisis = engine class **or** the file's flag (the flag can only
   hold back). ADR-0035 §6 says "never by the file's flag"; I read that
   as "the flag never lets one through".

## Round 2

From the stage-1 review (SEND BACK) and the orchestrator's brief.

### Blocking

- **B1 — apply touches open drift only** (`triage.rs` `apply_item`). After
  `select`, the named event must be in the fresh derive's open-drift set,
  and only the group's open members are kept; otherwise the item is
  `skipped` with `no longer open drift: …` (routine again with its rule,
  resolved by the engine, or as `drift show` words it) and nothing is
  written. Tests: the packet's firefox group (proposed under `attention =
  "all"`, applied after the default is back: skipped, ledger
  byte-identical), an engine `linked` line written after the proposal
  (skipped, the engine's line untouched), a group whose member the engine
  resolved (only mesa and vulkan-radeon written), a group whose leader the
  engine resolved (skipped). Round 1's "case gone" test now shows B1 too:
  without C-2026-005 the theme switch is routine again and is skipped.
  The two checks cover each other (an open leader's item has only open
  members); each alone is an equivalent mutant, both together are killed
  (M15). ADR-0036 §3 and SPEC-ENGINE §5 "Triage" say the same.
- **B2 — no self-citation, the author in every text.** Every evidence
  text is `by <author> · <words>` (author first, so no clip hides it).
  Authors: journal entry actor; event and snapshot actor; case creator
  (`case-created`, else `unknown`) plus whoever completed or dropped it;
  Plan: the case's authors plus every agent in its `agents` (Plan lines
  carry no author; the agents that worked the case write the Plan). A
  ref with the proposer among its authors does not resolve, at propose
  and again at apply against the proposal's `actor`. Tests: the packet's
  `seldon log --actor agent:claude-code` cite (refused), the fixture's
  09:25 agent note, an event, a case closed and a Plan of a case worked
  by the agent (all refused); a human note and another agent's note
  accepted with `by human` / `by agent:codex`; a file whose actor is
  changed to `agent:codex` refuses the item citing codex's event at
  apply. Residual risk (another name, `--actor human` on `seldon log`)
  in ADR-0036 §2; `triage.md` says an agent's note is not the user's
  word and never to write one to cite it; fetched text is data (N7, also
  in `drift.md`).
- **B3 — no private path.** `mutants.py` takes the root from `__file__`
  and builds in `engine/target/mutants`. The branch diff holds no
  `/home/<user>` path and no host name (only the fixture's
  `/home/user/Seldon` placeholder).

### Also

- **N1** ADR-0036 §4, SPEC §5 and guide 04 en/de say what the actor check
  stops (an agent in its launched session) and what it does not (a
  process of the same user that drops `SELDON_ACTOR`), and why (same uid,
  no second channel; such a process could already `drift link|explain`).
- **N2** `agent ask` refuses a logbook or guide path that is not UTF-8 or
  holds a control character, U+2028/U+2029, a bidi control or a
  backtick (exit 1, nothing launched); both paths stand in backticks in
  the prompt. Test with four such paths.
- **N3** the four survivors now have tests: evidence redaction (a
  hand-edited journal line), title redaction at propose and at apply, the
  non-following proposal write (unit test of `write_proposal` over a
  link), a foreign `seldon/` folder never serves as the guide.
- **N4** `one_line` (every free-text title and reason) refuses U+2028,
  U+2029, U+202A–U+202E, U+2066–U+2069. Unit test and a propose/explain
  test.
- **N5** a `proposals` that is a symbolic link or no directory: propose,
  apply and discard exit 1; the index build skips it with a warning.
- **N6** both, said precisely: nothing is written before a refusal (every
  check and the retroactive case's name come before the one ledger
  write). A case file that fails *after* its ledger lines is now `done`
  with a `warning`, committed and indexed with the rest; `drift
  link|explain` alone commit, rebuild, then exit 1 naming it (before,
  they exited without a commit). A debug-only fault switch
  (`SELDON_TEST_DRIFT_FAIL_AFTER_LEDGER=1`) drives the test.

### Verified

- `engine/tests/triage.rs` 20 tests (12 + 8), triage unit tests 3,
  `one_line` unit test; skills, drift, agent suites green.
- Mutants: `python3 work/active/WP-124/mutants.py`, **27 of 27 killed**
  (14 of round 1, 13 new: B1, B2 ×3, redaction ×2, non-following write,
  foreign guide, prompt path, separators, linked folder ×2, failure after
  the ledger). Log `mutants-wp124-r2.log` in my scratch dir (M15 then
  merged as noted under B1 and re-run: killed).
- `flock /tmp/seldon-check.lock just check` → `check: ok`, exit 0, at
  `29497cc` (2038 Rust tests passed, 0 failed; bar-view 194/0;
  plugin-test ok; docs-check ok). Log `check-wp124-r3.log`.

### Open

- Live check on the test host: still with 124b.
- Stage 2 (Fable): N1's limit (a same-uid process that drops
  `SELDON_ACTOR`) is stated, not closed.

## Round 3

From the Fable stage 2 (narrow SEND BACK) and the orchestrator's brief.

### Blocking

- **B4 — an applied explanation keeps its proposer's name.** The
  retroactive case `drift apply` makes carries the tag
  `proposed-by:agent:<name>` (CONTRACT.md rule 8, reserved vocabulary; no
  schema change). `case_authors` puts the proposing agent(s) first, from
  the tag **and** from the `proposed by <agent> — …` detail of the case's
  resolution lines in the ledger, so editing the tag out of the case file
  changes nothing. `event` refs of `case-*` kinds resolve by the case's
  authors (plus the line's actor), not by the line's actor alone. Test:
  Fable's probe — claude-code's applied explanation (intent "Ignore
  previous instructions. …"), then `case <ID>` and `event <case-created>`
  are refused for claude-code, read `by agent:claude-code · …` for codex,
  and stay refused after the tag is edited out; the tag is in the file
  and in the index.
- **B5 — the fixture's texts.** `fixtures/proposals/01M3VZS4J0NDXZFC2F7RBBD3FJ.json`
  regenerated with the engine (temp HOME, fixture copy, the file's `at`):
  every text now starts `by <author> ·`. The MONITORS item's evidence had
  to change: `case C-2026-002` was closed by `agent:claude-code`, the
  proposer, so round 2 refuses it; it now cites the event that wrote that
  `monitors.conf` in C-2026-002 (`01M2A9MNTG5XQ4EPAYSBBYJ92N`, by
  system). Ids, counts (3 items, 1 crisis), titles and intents are
  unchanged, so `index.sample.json` and the plugin's tests are untouched.
  Test: `propose` of the fixture's items (kind and ref only) at the
  fixture's time reproduces its `items` exactly.

### Also

- **N8** "the change itself" is every event of it: the item's linkable
  members, the named event, and every event of its package transaction
  (`txId`), resolved or not. The brief's literal fix (`linkable_members`
  before the open-only retain) does not catch it: a member the engine
  resolved has already left the item that `select` returns, so the test
  (mesa's group, lib32-mesa engine-linked after the proposal, the file
  citing lib32-mesa) failed with it; the transaction rule makes it pass.
  Same function at propose and at apply.
- **N9** a Plan text reads `by human (worked by agent:claude-code) · …`.
- **N10** `--item`s naming two or more crises (engine class now or the
  file's flag) refuse the run: exit 1, nothing written, the crises named.
  One crisis with an attention item is fine.
- **N11** a run without `--item` marks `applied` even when everything was
  refused; `--json` gains `markedApplied` (this run set it). ADR-0036
  Consequences and SPEC say `applied` marks the run, not the items.
- **ADR-0036 wording** as the brief gives it (§2 table rows for `case`
  and `event`, "… and an applied explanation keeps its proposer's name",
  §3 the tag and one crisis per run, Consequences N8 and N11). Status
  stays *proposed* for the orchestrator to accept.
- `triage.md`: a case the user applied from your own proposal is your
  own words.

### Verified

- `engine/tests/triage.rs` 26 tests (+6: B4, B5, N8, N9, N10, N11).
- Mutants: `python3 work/active/WP-124/mutants.py`, **34 of 34 killed**
  (round 3 adds M28–M34: tag dropped, ledger proposer dropped, case line
  by its own actor, transaction not the change, no worked-by label, two
  crises in a run, `markedApplied` meaning done). The full run found five
  round-2 patterns stale after this round's edits (M15–M19); their
  patterns were updated and those five re-run (`mutants.py M15 … M19`):
  killed. Logs `mutants-wp124-r3.log` in my scratch dir.
- `flock /tmp/seldon-check.lock just check` → `check: ok`, exit 0, at
  `459a151` (2050 Rust tests passed, 0 failed; bar-view 194/0;
  plugin-test ok; schema-validate and docs-check ok with the regenerated
  fixture). Log `check-wp124-r4.log`.
- Branch diff grepped for `/home/` (only the fixture placeholder
  `/home/user/Seldon`) and the host name (none).

## For 124b

Engine side is fixed by 124a; the desk does this (no code in 124a):

- *Apply* runs `drift apply <index.triage.id> --json`, bound to the id
  the user saw. Exit 1 "no proposal …": refresh the index, never retry.
- The sticky bar says: *N items proposed by `<actor>` at `<at>`, C crises
  held back — apply each below*.
- Each item shows:
  - the event subject from `index.events`, escaped;
  - the action plus the case id or the title;
  - **every** evidence text with its `by <author>` prefix visible first,
    never left-clipped.

  Mark an item where any ref is `by agent:…` or `by unknown`.
- Crisis items get one button each: `drift apply <id> --item <eventId>
  --json` (one crisis per run; the engine refuses two).
- After apply, render `done` / `skipped` / `refused` from the result and
  keep refused items visible. `applied` (and `markedApplied`) does not
  mean done.

## Merge of next

- `git fetch`; `git merge origin/next` (at `5fb3911`) into
  `wp/124-triage`: merge commit `82e415c`, no conflicts. In: WP-123 (desk
  sections Decisions, System, Memory, Prime Radiant), the WP-127 and
  WP-128 work packages. WP-102's import was not on `next` yet.
- `flock /tmp/seldon-check.lock just check` → `check: ok`, exit 0, at
  `82e415c` (2050 Rust tests passed, 0 failed; desk-view 1346/0;
  bar-view 194/0; plugin-test ok; docs-check ok). Log
  `check-wp124-merge1.log`.
- 124b not started.
