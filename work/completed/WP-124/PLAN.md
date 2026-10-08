# WP-124 — PLAN (stage 124a: engine, ADR-0036, skill, docs)

Branch `wp/124-triage` from `next` at `a1198b2` (contract v2, ADR-0035
accepted; desk sections 1–3). 124b (the desk) follows the 124a review.

## Inputs read

AGENTS.md; ADR-0034 (§6), ADR-0035 (§6), ADR-0027 (§2, §5, §9), ADR-0028
(§3), ADR-0030 (§1–§3); WP-124; `engine/src/commands/{agent,drift}.rs`,
`engine/src/reconcile.rs`, `engine/src/index/triage.rs`,
`schema/proposal.schema.json`, `fixtures/proposals/`, the skill
(`engine/assets/skills/seldon/`), SPEC-ENGINE §2, §3, §5, §8, CONTRACT.md
(rules, argv table). Omarchy first: `omarchy commands --json` (`omarchy
agent prompt`, `omarchy default agent`) and `omarchy-agent-crash`, whose
prompt names its skill and the skill file's path as the fallback for a
harness without skills — the pattern of the `agent ask` prompts.

## What 124a builds

1. `seldon agent ask triage|drift <EVENT>|case <CASE> [--launcher NAME]
   [--json]` — `engine/src/commands/agent.rs`.
2. `seldon drift propose [--file PATH] [--actor A] [--json]` (JSON on
   stdin), `seldon drift apply <PROPOSAL> [--item <EVENT>]… [--actor A]
   [--json]`, `seldon drift discard <PROPOSAL> [--json]` —
   `engine/src/commands/drift.rs` plus `engine/src/commands/triage.rs`
   (proposal input, evidence resolution, store).
3. ADR-0036 (proposed), DECISIONS.md row.
4. Skill `triage.md`; `drift.md` and `SKILL.md` point at it.
5. SPEC-ENGINE §2, §3, §5; CONTRACT.md argv table; guides 04 and 05
   en/de; CHANGELOG.
6. Tests: `engine/tests/triage.rs` (new) and the skill tests.

## Decisions (what the WP leaves open)

D1 **Ask prompts.** Fixed templates; the only variable parts are the
validated id, the logbook root and the absolute path of the installed
skill guide (`triage.md`, `drift.md`, `case.md`). Every prompt names the
seldon skill and the file's path as the fallback (as `omarchy-agent-crash`
does). No subject, title, journal line or any other logbook text.

D2 **The guide must be installed.** `agent ask` looks for Seldon's skill
(a folder with Seldon's manifest) holding the guide as a regular file in
the skill folders `hook install skills` uses, in that order; none → exit 1
"the seldon skill is not installed … Fix: `seldon hook install skills`",
nothing launched. An agent without the procedure would guess.

D3 **Ask sessions are not case sessions.** `agent ask` sets
`SELDON_LOGBOOK`, `SELDON_ACTOR=agent:<launcher>`, `SELDON_ATTENDED=1`
(the user clicked), `SELDON_CONFIG` when overridden — the same launcher
checks, folder rule and launch as `agent start` — but **not**
`SELDON_CASE` and it never touches `.seldon/active-case`: `SELDON_CASE`
says "launched to work this case" (ADR-0030 §1; the skill: a case is
yours only when the prompt says `Work case <ID>`). An ask prompt hands no
case to work. No lock is taken (nothing in the logbook changes).

D4 **Ask validation.** `triage`: refused (exit 1, nothing launched) when
nothing is open (`openDrift` 0). `drift <EVENT>`: a ULID that names an
open drift event (attention or crisis; a member of a group counts);
anything else exit 1. `case <CASE>`: an existing case, any status. The
built-in launcher without an Omarchy default agent: exit 1 "no default
agent … nothing was launched. Fix: `omarchy default agent <name>`".

D5 **Propose input** (stdin, at most 4 MiB, or `--file` for tests):
`{"items": [{eventId, action, caseId | title + intent, evidence: [{kind,
ref}]}]}`, unknown fields refused (`crisis` and `text` are the engine's:
an agent cannot set them). 1–200 items, 1–10 refs each; `title` one line
≤ 256 characters, `intent` one line ≤ 4096; both redacted. All or
nothing: the first bad item exits 1 naming its position and event id,
nothing is written.

D6 **Item checks.** `eventId` names an open drift item (attention or
crisis) — stored as the item's leader (the id `index.drift` shows); two
items for one drift item are refused. `link`: the case exists (any
status, as `drift link`). `crisis` is the engine's class at propose time.

D7 **Evidence resolution** (the engine writes `text`, ≤ 256 characters,
one line, redacted):
- `journal` `YYYY-MM-DD HH:MM`: an entry with that heading time in that
  day's journal file → its text;
- `event` `<ULID>`: a ledger event that is not a resolution and not a
  member of the item itself (an item is no evidence for itself) →
  `kind subject` and its detail;
- `snapshot` `<N>`: the newest snapper `snapshot` event with subject N →
  its description;
- `case` `<CASE>`: an existing case → its title;
- `plan` `<CASE>`: the first line of that case's *Plan* that names the
  subject of a member of the item as a whole word (ADR-0015 §4's token
  rule) → that line.
Anything else does not resolve; an item with a ref that does not resolve
is refused (every ref must resolve, not just one).

D8 **Actors.** `propose`: the actor (`--actor`, else `$SELDON_ACTOR`)
must be `agent:<name>`; a human is refused (exit 1: "a human resolves
directly: `seldon drift link|explain`"). `apply` and `discard`: the
user's acts — the actor (`--actor`, else `$SELDON_ACTOR`, else human)
must be `human`; an agent actor, and `--actor human` in an agent's
session, exit 1 (an agent's resolution is never recorded as human;
ADR-0028 §3, WP-109 round 2).

D9 **One proposal per logbook.** `propose` stores
`<state>/proposals/<ULID>.json` (schema-checked before it is written,
mode 0600) and removes this logbook's earlier proposal; the output names
it (`replaced: {id, applied}`), and the human line says so when it was
unapplied. The resolutions in the ledger are the record of what was
applied; an old file adds nothing and would come back in the index after
a discard. Other logbooks' files are never touched.

D10 **Discard.** `seldon drift discard <PROPOSAL> --json` removes the
file (this logbook's only); the desk's *Discard* (124b) needs a fixed
argv. Unknown id: exit 1. A second discard: exit 1 "no proposal".

D11 **Apply** (under the lock, one autocommit, one index rebuild):
- reads the file as the index does (regular file ≤ 4 MiB, schema, name =
  id) and this logbook's only; `--item` must name items of the proposal
  (exit 1 before any write);
- per item, in file order, from a fresh derive: unknown or no longer
  open → `skipped` ("already linked to C-…", …); a **crisis** — the
  engine's class now **or** the file's flag (the flag can hold an item
  back, never let one through) — without `--item` naming it →
  `skipped` ("crisis: only by name"); every evidence ref is resolved
  again from the logbook (the file's `text` is never read) — one that no
  longer resolves → `refused`; `link` to a case that is gone, an
  `explain` of an event that became routine → `refused`;
- `link` and `explain` go through `drift link|explain`'s path (group
  fan-out, case `events:`, retroactive completed case for explain with
  the proposal's title and intent) with `resolutionDetail` `proposed by
  agent:<name> — <kind> <ref> "<text>"; …` (each text ≤ 120 characters,
  the whole ≤ 1024);
- a run without `--item` marks the proposal `applied` (once; the first
  time stays); a run with `--item` does not;
- exit 0 whenever the proposal was read; `--json` → `{proposal, applied,
  done: [{eventId, action, resolved, case}], skipped: [{eventId,
  reason}], refused: [{eventId, reason}], git}`. A second apply writes
  nothing and lists every item under `skipped`.

D12 **No contract change.** `proposal.schema.json` and `index.triage`
stay as ADR-0035 §6 fixed them; the stdin format of `propose` is CLI,
not contract (the plugin never runs `propose`). CONTRACT.md's argv table
gains `agent ask …`, `drift apply …`, `drift discard …`.

## Tests (engine/tests/triage.rs; agent ask through a recording stub)

Id validation (ask, propose, apply); every prompt equal to its template
with no logbook text (markers in titles, subjects and the journal; one
mutant puts a title in the prompt); every `seldon …` span of the prompts
exists (`--help`, flags); no default agent; no skill; env without
`SELDON_CASE`, active case untouched; propose: unknown fields, human
actor, each evidence kind resolved and refused, self-evidence, duplicate,
routine item, all-or-nothing, replace; apply: idempotent second run,
crisis guard (flag false + engine crisis, flag true + engine attention),
`--item` crisis, evidence gone, tampered file text ignored, agent actor
refused, `--json` shapes, exit codes, `applied` set once, index `triage`;
discard. Skill: `triage.md` shipped, linked, its commands exist.

## Not in 124a

The desk (124b); the live check on the test host (after 124b: it needs
the one click); `drift propose` for single items from `ask drift` is
offered by the skill, not required.
