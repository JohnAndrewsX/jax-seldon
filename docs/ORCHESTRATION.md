# ORCHESTRATION.md — Running the dev-agent team with Herdr

This is the operating manual for the **orchestrator** agent (a Herdr
session steering several Claude Code / Codex workers). It assumes
`AGENTS.md`, `PLAN.md` and the specs are in context.

## 1. Roles

| Role | Does | Typical model/tool |
|---|---|---|
| **Orchestrator** | reads PLAN.md and `work/`, assigns WPs, runs acceptance tests, moves WPs between folders, writes STATUS.md, talks to the operator (in their language) | long-running Herdr session |
| **Architect / Reviewer** | reviews every handover against specs and ADRs; owns contract changes; writes ADRs when a WP forces a decision | one session, invoked per review |
| **Schema Keeper** | owns `schema/`, `fixtures/`, golden tests | part of Architect in small teams |
| **Engine Dev** | Rust WPs | up to 2 parallel sessions in separate worktrees |
| **Plugin Dev** | QML WPs, works against fixtures | 1–2 sessions |
| **QA** | integration tests on a real Omarchy, smoke tests, theme sweeps, performance checks | one session, Phase 1+ |
| **Docs** | READMEs, templates, marketplace material | one session, late phases |

Small team variant (what to start with): Orchestrator + 1 Engine + 1 Plugin
+ Reviewer. Scale out only when WPs are genuinely independent.

## 2. The loop

```
for each tick:
  1. read work/active/*, work/queued/*, STATUS.md
  2. for each active WP: check handover present? → run acceptance tests
       pass → ask Reviewer → approve → move to completed, update STATUS
       fail → return to worker with the failing output, same WP
  3. for each idle worker: pick highest-priority queued WP whose deps are
     completed and whose role matches → create worktree → send brief
  4. if a worker reports a decision needed → Reviewer drafts ADR →
     Orchestrator runs a debate (§10) and decides; only a decision that
     changes scope, cost or the red zone goes to the operator (one question,
     options listed)
  5. commit STATUS.md; summarise to the operator only when a WP completes or blocks
```

A tick is whatever Herdr's cadence is; the loop must be safe to re-run.

## 3. Briefing a worker

Every brief has the same shape (keep it under 40 lines):

```
WP-004 — Collectors: pacman, snapper, omarchy
Role: Engine Dev · Worktree: wt/WP-004 · Branch: wp/004-collectors-core
Read first: AGENTS.md, docs/SPEC-ENGINE.md §4 §7, docs/CONTRACT.md,
            schema/event.schema.json, fixtures/logs/
Inputs: engine/ (WP-003 merged), fixtures/logs/pacman.log, fixtures/logs/snapper.json
Outputs: engine/src/collectors/{pacman,snapper,omarchy}.rs, tests, golden files
Acceptance: cargo test -p seldon collectors::  (all green)
            second run on same fixtures emits 0 events (test idempotency::)
            redaction:: tests green
Constraints: no new crates without justification; no network; read-only on host
Hand back: PR against main, or work/active/WP-004/HANDOVER.md
Questions: ask the orchestrator; do not guess shell/omarchy behaviour —
           read ~/.local/share/omarchy first and note what you learned in
           memory/omarchy-shell.md
```

## 4. Handover format (worker → orchestrator)

```
WP-004 HANDOVER
Done: …
Not done: …
Verified by: commands + key output
Learned: … (goes to memory/)
Decisions needed: … (or none)
Touched outside WP scope: none | list
```

## 5. Gates

- **G1 Assign:** deps completed, role available, WP has acceptance tests.
- **G2 Accept:** tests pass on the orchestrator's machine, Reviewer approves,
  no files touched outside scope, STATUS updated.
- **G3 Phase exit:** the phase's exit criterion in PLAN.md is demonstrated
  to the operator (screenshot or terminal transcript in `work/completed/`).

## 6. Parallelism rules

- Engine and plugin tracks are independent from day one because of the
  fixture index. Run them in parallel.
- Two engine workers never share a module; the orchestrator assigns by
  module path and says so in the brief.
- Contract changes serialise everything: freeze both tracks, do the change,
  unfreeze.

## 7. Memory for the team

`memory/` in this repository (not the logbook) holds what workers learn:
`omarchy-shell.md` (QML API findings), `rust-notes.md`, `pitfalls.md`. The
orchestrator includes the relevant file in every brief. Workers append;
the Reviewer prunes.

## 8. What the orchestrator never does

- Merge without tests. Guess Omarchy internals. Change ADRs. Point the
  engine at the operator's real logbook before Phase 0 exit. Run anything red-zone.

## 9. Kickoff checklist (first session)

1. `git init`, commit this kit as `chore: seldon kit v1`.
2. Create `work/queued/WP-001.md` … `WP-009.md` from PLAN.md (one file per
   WP, acceptance tests copied verbatim). WP-001 and WP-002 are already
   provided as examples.
3. Verify the host: Omarchy version, `omarchy plugin list --json` works,
   `qmllint` present, Rust toolchain with `x86_64-unknown-linux-musl`.
   Record in `memory/host.md`.
4. Assign WP-001 (Scaffold) and WP-002 (Schema Keeper) in parallel.
5. Report to the operator: what was started, what you need from them (nothing,
   ideally).

## 10. Decisions by debate (autopilot)

The operator is not at the keyboard. A decision a WP forces is settled
inside the team, not by waiting:

1. The orchestrator writes the question in one paragraph with the options.
2. Two agents argue in parallel, each with the same inputs (specs, ADRs,
   the worker's handover) and opposite briefs:
   - **Devil's advocate** — attacks the proposed option, argues for the
     alternative, lists failure modes and hidden costs.
   - **Advocatus Dei** — defends the proposed option, answers the attacks,
     lists what breaks if the alternative is chosen.
   Use different model families when available (one Claude, one Codex).
3. The orchestrator judges: picks an option, writes `decisions/ADR-NNNN`
   with both arguments summarised under *Context*, updates STATUS.md, and
   continues. A debate is bounded: two rounds at most, then decide.
4. Only these go to the operator, as one question with options, and the
   orchestrator keeps working on everything else meanwhile: a change to
   project scope or PROJECT.md, anything in the red zone (AGENTS.md §6),
   spending beyond the usage budget (§12), a contract change that breaks
   the fixture index.

Run a debate as a workflow (two agents in parallel, then a judge step) or
as two subagents; both are allowed for this bounded fan-out.

## 11. Autopilot: how the loop keeps running

- The permission mode of the orchestrator and worker sessions is the
  operator's choice when starting them (docs/HERDR-SETUP.md §2). Whatever
  the mode, the red-zone guard hook in `.claude/settings.json` blocks what
  AGENTS.md §6 forbids before the command runs.
- Waiting is done with `herdr agent prompt … --wait` and
  `herdr agent wait <name> --timeout …`, never by polling in a shell loop.
- The tick in §2 is driven by the `/loop` skill in self-paced mode: the
  orchestrator starts it after the first tick with the instruction "run the
  orchestration loop (ORCHESTRATION.md §2)" and lets it pace itself:
  short delays while workers are active, 20–30 minutes while waiting on
  nothing in particular.
- A worker in state `blocked` is inspected (`herdr agent read`). An
  ordinary question inside the worker's own task is answered from the WP
  brief; a question that needs a decision goes through §10; a permission
  dialog is left for the operator and noted in the log (the orchestrator
  does not answer permission prompts on the operator's behalf).
- Every tick ends with `git status` clean on `main`, STATUS.md committed,
  and a one-line entry in `work/ORCHESTRATOR-LOG.md` (date, tick, what
  changed). This file is the operator's way to catch up.
- The loop stops at the phase exit (G3) and reports; the operator restarts
  it for the next phase. Within a phase it does not stop on its own.

## 12. Models, effort, and budget

Default matrix; the orchestrator states model and effort in every brief
and passes them on start (`herdr agent start … -- --model X --effort Y`).
Workers may use cheaper subagents internally for exploration.

| Role | Model | Effort | Why |
|---|---|---|---|
| Orchestrator | `fable` | high | judgement, long context, few tool calls |
| Architect / Reviewer | `fable` | high | reads specs against code; errors here are expensive |
| Debate: devil's advocate | Codex (if logged in) else `opus` | high | different model family gives a real second opinion |
| Debate: advocatus Dei | `opus` | high | |
| Engine Dev, Plugin Dev, Schema Keeper | `opus` | high | long coding runs; escalate a stuck WP to `fable` once |
| Docs, boilerplate, fixtures prose | `sonnet` | medium | cheap and good enough |
| QA (integration scripts, theme sweeps) | `opus` | medium | |

Budget: subscription windows are visible with
`omarchy agent usage claude --limits-only` (JSON). The orchestrator checks
it at every tick; above 85 % of a window it starts no new worker and lets
the loop wait; above 95 % it pauses running workers' prompts until the
window resets. It never switches providers or keys to get around a limit.
