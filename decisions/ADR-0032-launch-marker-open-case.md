# ADR-0032 — The launch marker serves a session only while its case is open

**Status:** proposed (orchestrator decision 2026-10-06 in the WP-116 round-2 brief, from stage-1 finding N4; the operator's yes pending). Ships in 0.1.4 with WP-116.
**Date:** 2026-10-06

> Amends [ADR-0030](ADR-0030-start-like-omarchy.md) §1 clause (b) and §3
> (the launch line). Everything else in ADR-0030 stands. No contract
> change.

## Context

ADR-0030 §1 serves a hook call when its environment holds `SELDON_CASE`
"with a value that parses as a case id". The stage-1 review of WP-116
probed it: a well-formed marker for a case the logbook does not have
(`C-2099-999`) records to the active case. The variable lives as long as
the processes that inherit it: a tmux or editor server the agent starts
keeps it after the case is done, and every Claude Code session started
from there would be recorded, in any folder, until the server ends. A
user who exports the variable by hand (shell rc, `environment.d`) gets
the same. That is wider than "the session Seldon launched", ADR-0030's
privacy line.

## Decision

1. **Clause (b) narrowed.** A hook call is served by the marker only
   when `SELDON_CASE` names a case of this logbook that is **active or
   in verification**. An empty value, a value that is not a case id, a
   case the logbook does not have, a queued, completed or dropped case:
   no marker — the call is served by clause (a) or not at all. The
   check reads the folder of active cases only.
2. **The launch line** of `hook session-start` appears under the same
   condition.
3. **Session end.** The agent closes its own case (ADR-0027 §5) before
   the session ends. `hook session-stop` therefore also serves a call
   with a well-formed marker when the ledger already holds events of
   that session (`meta.sessionId`): the session that ran the case gets
   its journal line, capture and commit; a server's later session, which
   recorded nothing, does not.
4. **Never set it by hand.** The guides, the rules and the skill say
   that Seldon sets `SELDON_CASE` and nobody else should; the hand-down
   sentence names servers and multiplexers (tmux, an editor server)
   next to agents, jobs and timers.

## Consequences

- After `plan done` or `plan drop`, a launched agent's further commands
  outside the logbook are no longer recorded by the hooks (the
  collectors still record packages, services and watched paths).
- One directory read and one case file parse per hook call that carries
  the marker; unmarked calls are unchanged.
- Hook attribution still takes the case from `.seldon/active-case`
  (ADR-0030 §1); the marker only decides the scope.
