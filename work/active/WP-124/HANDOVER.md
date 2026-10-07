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
