# ADR-0040 — Accepting a decision is the user's act: `seldon decide accept`, one click in the desk

**Status:** proposed (WP-135, under the operator's decision of 2026-10-07
(E7): "Accept accepts a decision in one click")
**Date:** 2026-10-07

> Adds a row to CONTRACT.md's "Commands the plugin may run". ADR-0035 §6
> does not cover it: that rule lets a later ADR on `next` add optional
> fields to the v2 schemas or refine `proposal.schema.json`, not
> commands. The index's shape does not change, so `contractVersion` stays
> 2 and no fixture changes. Supersedes WP-123's Decision 2 (Accept opens
> the editor). Follows ADR-0027 §5 and ADR-0036 §4 (an agent's act is
> never recorded as human; applying is the user's).

## Context

The desk's Decisions section (ADR-0034 §2) shows *Accept* on a proposed
decision. No engine command accepted one, so WP-123 made Accept open the
ADR in the editor and said "set `status: accepted`". That is two
windows and a hand edit for one decision the user has already made, and
the change leaves no trace in the ledger: SPEC-ENGINE §3 said `decide`
writes no event because no kind fits.

A decision is the user's: an agent may write one (`seldon decide` makes
it `proposed`), but whether the machine follows it is for the person to
say. The engine already refuses agents the user's acts in three places
— an agent's start of an imported case (WP-102), `--include-done` in an
agent's session (WP-102), and `drift apply|discard` (WP-124) — by the
resolved actor *and* the session (`$SELDON_ACTOR`), so `--actor human`
inside an agent's session does not get around it.

## Decision

1. **`seldon decide accept <ADR-NNNN> [--actor A] [--json]`** sets the
   decision's `status: accepted` and its `date` to the day of the
   acceptance (only these two frontmatter keys change; the body and
   every other byte stay), refreshes `DECISIONS.md`'s index fence,
   commits (`seldon: ADR-NNNN accepted`) and rebuilds the index. The
   ledger line is written first, as in a plan step: a ledger that cannot
   be written leaves the decision proposed; a file write that fails after
   it leaves the `accepted:` note with the decision still proposed, and a
   re-run adds a second note. The file stays the truth.
2. **The ledger record is a `seldon` `note`**, subject the decision id,
   detail `accepted: <title>`, actor `human`, no `case`. A `seldon` note
   is the engine's own record where no kind fits (the agent skill
   update, the snapper collector's state); no reader keys anything off
   it, the decision file stays the truth. A new kind would widen the
   event schema's enum, which ADR-0035 §6 does not allow within
   contract 2.
3. **Only a proposed decision is accepted.** An accepted one: exit 0,
   `already: true`, nothing written (idempotent). A superseded one, an
   unknown id, a file whose frontmatter does not read or names another
   id: exit 1, nothing written.
4. **The user's act.** An agent `--actor`, an agent `$SELDON_ACTOR`
   without `--actor`, `--actor human` in an agent's session, and a
   `$SELDON_ACTOR` that is set but does not read (with any `--actor`: the
   session may be an agent's; `drift apply|discard` follow the same rule)
   are refused with exit 1 before anything is read; each message names the
   conflict and the way out (the desk, or the user's own terminal).
5. **The plugin** runs exactly `seldon decide accept <ADR-NNNN> --json`
   (id checked against `^ADR-[0-9]{4}$`) after the button is pressed
   twice (the arm-twice rule of every writing action). The new row in
   CONTRACT.md is the whole contract change.

## Consequences

- One click (armed) replaces open-edit-save; the ledger shows when a
  decision was accepted and by whom.
- The proposal date is overwritten by the acceptance date in `date`; it
  stays in git's history (the commit `seldon: ADR-NNNN proposed`). The index
  and `DECISIONS.md` show the acceptance date.
- `seldon decide -- accept` is how a decision titled "accept" is made
  from the terminal; the plugin always passes titles after `--`.
- Superseding (`supersedes:`) is still a hand edit; a later ADR may add
  `decide supersede` the same way.
- SPEC-ENGINE §3, SPEC-LOGBOOK §3, SPEC-PLUGIN (Decisions), CONTRACT.md,
  the guides (concepts, CLI reference), CHANGELOG.

## Alternatives considered

- *A new event kind `decision-accepted`*: clearer in the ledger, but a
  new enum value in `event.schema.json` changes what a contract-2 reader
  must accept; that needs a contract bump, not this ADR.
- *Keep the date, add `accepted: <date>`*: a new frontmatter key the
  index does not carry and the plugin cannot show without a schema
  field; the WP asks for the date to be set.
- *Let an agent accept in an attended session*: the decision is the
  person's; an agent's acceptance recorded as anything would misstate
  who decided.
