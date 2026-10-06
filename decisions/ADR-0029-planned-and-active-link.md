# ADR-0029 — A change made while exactly one case planned it is that case's: the engine links it, and a case captures before it closes

**Status:** accepted (operator decision 2026-10-06: as recommended — uniqueness means the one case whose Plan names the subject). Ships in 0.1.4 with WP-115.
**Date:** 2026-10-06

> Extends [ADR-0027](ADR-0027-act-then-account.md) (act, then
> account) and [ADR-0028](ADR-0028-attention-by-consequence.md)
> (attention by consequence). Amends ADR-0027's debate verdict H2 ("no
> heuristic link") for one narrowly evidenced rule; SPEC-ENGINE §5 gains
> rule 9. ADR-0012 §7/§8/§10, ADR-0014 §1, ADR-0021, ADR-0023 §1 (as amended
> by ADR-0027 §5) stand. No contract change.

## Context

Live test, test host, engine main `994015a`, 2026-10-06. The user started
case C-2026-004 "install glow" from the panel. The agent wrote a Plan that
names `glow`, then — by the user's own machine rule — handed the root step
to the user, who ran `pacman -S glow` in another terminal at 13:40:57 while
the case was active. The agent verified and closed the case at 13:41:34.
No capture ran between the install and the close. The next capture recorded
`pacman install glow` with actor `system` and no case; it became open
drift — a red crisis on today's engine, quiet attention once ADR-0028
lands. The case that asked for it was complete and lists no event.

Why the rules miss it (SPEC-ENGINE §5, `engine/src/reconcile.rs`,
`engine/src/index/build.rs:478`): rule 1 links only *agent* events with an
active case; rule 2 only dependencies of an event that already has a case;
rule 3 *proposes* for cases open *now* whose Plan names the subject, so a
closed case gets nothing. Rules 7 and 8 are the only writes the engine
makes on its own evidence, and both are `explained` lines for its own
files and its own package.

Three facts the engine does have: the case's window — from its last
`case-started` to its `case-completed`/`case-dropped` ledger event (status
active or verification in between; `plan verify` and `plan done` write
these lines, `plan.rs:871`); the Plan text of that case, which the agent
writes as it goes (ADR-0027 §2); and the event's `ts`. ADR-0027 H2 rejected
*token-only* auto-linking across open cases because a token match alone
writes a false "why" into REBUILD.md. The rule below is not that: it needs
all three facts at once and exactly one case that satisfies them.

The second cause is procedural: ADR-0027 §5 has the agent run `plan verify`
then `plan done` in one go; nothing in that path brings the ledger up to
date first, so whatever the user did by hand inside the case is recorded
only after the case is gone. The hook path cannot help: hooks serve the
agent's own tool calls, and a terminal the user opens has none.

Facts checked (read-only): `plan` transitions take the state lock
themselves (`ctx.lock()`, `plan.rs:422`) and `capture` takes its own
(`capture.rs:119`); a capture is incremental for pacman and hashes a dozen
or so config files; hooks wait up to 8 s for a capture's lock (§8), which
bounds what a capture may cost; `attach()` already maintains a case's
`events:` oldest-first and ignores `seldon` events; `plan reopen` appends a
Log line to a *completed* case (precedent for writing to a closed case's
Log); `drift link` already accepts a completed or dropped case as target
(retro-links, §5); ADR-0012 §8: later resolutions win.

Assumptions, named: (A1) a default capture on the test host takes well
under a second (not measured; WP-115 measures it); (A2) `.seldon/
active-case` keeps no history — it names the case started last and is
cleared or repointed by `done`/`drop`/`reopen` — so it cannot say which
case was active at a past instant; (A3) ADR-0028's class is computed after
rules 1–3 (its §1), so a rule that runs with them removes the event from
drift whatever its class; (A4) number 0029 is free.

## Decision

### 1. Rule 9 — the planned-and-active link (SPEC-ENGINE §5, after rule 2, before rule 3)

An event without `case` and without a resolution, from a drift-eligible
source, **by any actor** (human, agent without a served hook, `system`),
is **linked** to case `C` when all of the following hold at the event's
`ts`:

- (a) `C`'s window contains `ts`: `C` had a `case-started` at or before
  `ts` and no `case-completed`/`case-dropped` between that start and `ts`
  (status active or verification at that instant). The window is read
  from the **ledger's case events**, not from the case file's `status`
  or dates and not from `.seldon/active-case` (A2 — the pointer is
  present-tense evidence and is not used).
- (b) `C`'s `## Plan` names the event's subject as a whole-word token,
  with the token rule of ADR-0012 §7 and ADR-0015 — the same test rule 3
  uses, on the Plan text as it is when the rule runs.
- (c) `C` is the **only** case for which (a) and (b) both hold. Two or
  more such cases: no link; the event is proposed as today (rule 3,
  lowest id) and each candidate case gets one Log line naming the
  others. No such case: rules 3–4 as today.
- (d) **Harm guard (ADR-0028 §1 test 1).** When the subject matches
  `[drift] alwaysRed`, the link is written only if `C`'s `risk` was R3 —
  the one case in which ADR-0027 §2c says the user gave an explicit go per
  step inside the case. Below R3 the event stays a proposal and `C` gets
  the existing R3-after-the-fact advisory line (§5 case notes), even
  though it is closed. Everything else — attention and routine classes —
  links without further condition.

**What is written.** Right after the capture's append, under the same lock
and in the same pass as rules 7 and 8: one `resolution` line per linked
event — `source: seldon`, `kind: resolution`, `resolution: linked`, actor
`system`, `case: C`, `refersTo` the event, `detail: planned by C; active at
the time`, `ts` = capture time or the event's time, whichever is later
(the WP-088 clock rule). A pacman event linked this way takes its
transaction's non-explicit members with it, one line each with
`meta.txId`, exactly as `drift link` fans out (rule 2 then holds
retroactively). Then `attach()` records the ids in `C`'s `events:` and
`C`'s Log gets one line by `system`: `linked after the fact: pacman
install glow at 13:40:57 (planned here, no capture ran before the close)`
— the second half only when `C` was already closed. The event line itself
is never touched: its actor stays `human`/`system`, the record says who
did it and, separately, who linked it and why.

**Why a `linked` line is not an explanation by the agent (ADR-0028 §3).**
ADR-0028 forbids an *agent* to explain or dismiss a crisis and lets it link
a crisis only to its own active case, because an agent's claim without
evidence is worth nothing. Rule 9 is the engine stating three facts it
read from the ledger and the case file, and `linked` points at a case whose
*Intent* the human wrote: the "why" is the case's, not the engine's and
not the agent's. It is the same standing rules 1 and 2 already have — an
agent's hook-served `pacman -S glow` inside C-2026-004 is linked without
anyone's approval today; rule 9 removes the accident that the user, not
the agent, typed the command. The harm guard (d) keeps the one thing
ADR-0028 must keep: a change that can break boot is never made quiet on
the engine's say-so unless the case was marked for it.

**Can an agent abuse it?** Only by writing a package name into the Plan
and waiting for the user to install it in the window — which links a
*user* action to the case the user started for it; the agent gains
nothing over `seldon drift link`, which it may already run on attention
items. The false-link risk (Plan says "do not install foo", user installs
`foo` for another reason, same window) is accepted and reversible (§3).

### 2. A case captures before it changes state

`seldon plan verify` and `seldon plan done` run a default `seldon capture`
**first** — a complete capture under its own lock hold, then released, then
the transition under the step's own lock, so neither lock is held twice
and the capture's rule 9 pass sees the case still open. A capture failure
(a collector in degraded state, a lock held past its wait) is a **warning**
in the output, never a refusal of the step; `--no-capture` skips it (the
panel's one-click `done` keeps its fixed argv and simply waits a little
longer; the `Process` is asynchronous). `plan drop` does not capture (the
brief's scope; a drop claims nothing). Chosen over a rule that tells the
agent to capture: the finding *is* an agent that did not, ADR-0027 §1
forbids a human or agent step the engine can take, and the hook budget
(§8, < 5 ms) is untouched because `plan` commands are not on the hook
path — they run once per case in the agent's or human's terminal. Cost
(A1): one capture per transition, incremental; WP-115 measures it on the
test host and records the number in the SPEC. The skill (WP-094) says one
sentence: "`plan verify` captures first; a step you handed to the user is
recorded and linked when you verify."

### 3. Migration, append-only, reversibility

Rule 9 reads the **whole ledger** on every capture, as rule 8 does
(WP-088; one `read_all()` shared by both passes): every event without case
and without any resolution is tested against every case's window, so
events recorded before 0.1.4 — C-2026-004's `install glow` among them —
are linked by the first capture after the upgrade. Nothing is rewritten;
the case file's `events:` is engine-maintained (ADR-0012 §10) and its Log
is appended. Idempotent: the second capture writes nothing. Index-time
derivation (no ledger line, like rule 3) was rejected: the case file, the
ledger and REBUILD.md would disagree with the index, and a reader of the
ledger months later would not see the link. **Reversibility:** an event
whose folded resolution was written by actor `system` (rules 7, 8, 9) is
*re-resolvable* — `seldon drift link|explain|dismiss <id>` by a human or
an agent writes a later line that wins (ADR-0012 §8); the engine never
writes over a human's or an agent's line, only onto events with none.
This widens WP-109's `linkable` set by one condition and adds no command.

### 4. Contract

None. `resolution: linked` with `case` folds already (ADR-0012 §8,
ADR-0021); `resolutionDetail` carries the detail; actor `system` on a
resolution exists since rule 7. The schema description of `resolution`
gains the sentence "`linked` by `system`: SPEC-ENGINE §5 rule 9".
`contractVersion` stays 1.

### 5. WP-115 (0.1.4, engine + skill text; after WP-109 merges, before the release)

Engine: rule 9 pass in `reconcile.rs` (windows from case events, Plan
token test shared with `build.rs`, uniqueness, harm guard, fan-out,
attach + Log line, catch-up, `--json` count `linkedPlanned`); capture in
`plan verify|done` with `--no-capture`; `linkable` widened to
engine-resolved events; `scripts/validate-fixtures.py` ports the rule;
fixture logbook gains one engine-linked line and the sample index renders
it. SPEC-ENGINE §3 (`plan`), §5 rule 9, §10; AGENT-GUIDE; skill sentence;
CHANGELOG.

Acceptance (tests on fixtures, `just check` 0):
1. **C-2026-004 as it happened:** case started 13:38, Plan names `glow`,
   pacman.log line `installed glow` 13:40:57 by nobody the hooks saw,
   `case-verified`/`case-completed` 13:41:34, capture 13:45 → one `linked`
   line by `system`, case `events:` lists the id, Log line "linked after
   the fact …", index: `resolution: linked`, `case: C-2026-004`,
   `index.drift` empty; second capture writes nothing.
2. **C-2026-004 with §2:** `plan verify` at 13:41:30 → the capture inside
   it records and links the event while the case is active; the ledger
   shows the pacman event, its resolution line, then `case-verified`.
3. Window edges: `ts` before `case-started`, after `case-completed`, in a
   queued case, in a reopened case's predecessor → no link; in
   verification → link; a dropped case's window → link (status at `ts`
   decides).
4. Uniqueness: two cases with overlapping windows both naming the subject
   → proposal (lowest id) and two Log lines; one naming it → link.
5. Actors: `human`, `system`, `agent:x` without case → link; an event with
   any resolution or a case → untouched.
6. Harm guard: `alwaysRed` subject, case R3 → link; case R1 → proposal and
   advisory Log line.
7. Fan-out: explicit `glow` with two dependencies in one `txId` → three
   lines with `meta.txId`, one `events:` entry each.
8. Reversibility: `drift dismiss <id>` on an engine-linked event writes a
   later line that wins; an agent `drift explain` of an engine-linked
   crisis exits 1 (ADR-0028 §3 unchanged).
9. `plan verify --no-capture` writes no collector event; a capture error
   is a warning and the transition happens.
10. Live check on the test host: repeat the C-2026-004 sequence by hand
    (agent plans, user installs in a terminal, agent closes) → zero open
    drift, the case lists the event, bar unchanged; and the capture cost
    of `plan verify` measured and written into SPEC-ENGINE §3.

## Consequences

- A step the user takes for a case — the one ADR-0027 still leaves to
  the user (R3), or any the user prefers to take — ends in the case, not
  in drift. Human steps per case (ADR-0027 §1) do not grow; one false
  to-do per such case disappears.
- The ledger gains lines that say exactly what the engine knew; it never
  claims an intent of its own. ADR-0027's H2 verdict is narrowed, not
  overturned: token matching alone still only proposes.
- `plan verify|done` take longer by one capture. The hook path is
  unchanged.
- Open: whether (c) should demand *exactly one active case at all*
  (stricter; breaks the parallel-agents pattern) — kept as "exactly one
  that planned it" and watched through the reopen metric and the false-
  link count in the next live round.

## Alternatives considered

- **Tell the agent to capture before closing** (rules only). Rejected:
  the engine can do it, so the rule is extra work and a forgettable step
  (ADR-0027 §1).
- **Set `case` on the event directly before the append** (rules 1–2
  style) for new events, resolution lines only for the catch-up. Rejected:
  two mechanisms, and the event would look hook-attributed when it was
  not; one honest line is better.
- **Index-time link, no ledger write.** Rejected (§3).
- **Use `.seldon/active-case` as evidence.** Rejected: no history (A2);
  the ledger's case events are the authoritative window.
- **Link crises unconditionally.** Rejected: the harm guard costs one
  comparison and keeps ADR-0028's one visible signal honest.
- **Prefer the Intent over the Plan for the token.** Not needed: linking
  is accounting, not authorisation (ADR-0027 H1 is about scope); the Plan
  is where the agent names what it planned, and it is already what rule 3
  reads.
