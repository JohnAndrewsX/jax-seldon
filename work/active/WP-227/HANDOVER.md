# WP-227 — Handover

Branch `wp/227-app-adrs` (off `app`), not pushed. Role: architect (Opus).
Docs only; no code, schema, fixture, AGENTS.md or PROJECT.md change.

## What was done

- `decisions/ADR-0056-seldon-is-an-omarchy-app.md`,
  `ADR-0057-ui-strings-through-catalogs.md`, `ADR-0058-seldon-open-is-the-app.md`,
  all **proposed (E96 delegation; operator accepts)**. They write down the
  advisor's decision (option A) without re-deciding it: three artefacts in
  one package `seldon`, the one job, four views, EASY | PRO retired, the
  pill, the desk's end with 0.5.0, the install route, the recorded
  exceptions (two binaries, exit codes 0–4, no undo, static musl scoped to
  the engine) with precedent; catalogs per surface, `en`+`de` then 48, the
  language file as display; `open`/`edit`/alias, `skill`, `changes --around`.
- `DECISIONS.md`: three rows; "superseded in part by ADR-0056 (proposed)"
  on ADR-0001, 0002, 0004, 0016, 0022, 0024, 0034, 0045; "by ADR-0057" on
  ADR-0007; ADR-0041 noted as standing with its command moved (ADR-0058).
  The old ADR files are untouched (immutable once accepted).
- `docs/CONCEPT.md` v2 (the one job, four parts in one package, the
  surfaces from 0.5.0, "when something broke", glossary).
- `docs/PLAN.md` phases 5–7 and milestones M5–M7; `docs/ROADMAP.md` (one
  page, the decision's version table); `docs/APP-PARITY.md` (every
  desk-view scenario 1–11i → app acceptance or recorded drop, the daily
  acts as the gate, the app's new acceptances).
- Notes, not rewrites: SPEC-PLUGIN (top, §4, §5, §8), SPEC-ENGINE (§1.1,
  §3, the `open` synopsis line), `docs/skills/omarchy-ux/SKILL.md` §1.
- `docs/AGENTS-PROPOSAL.md`: today → proposed rows for AGENTS.md §2, §3,
  §4 (`app/`, `bin/`), §6, §7 (an App block, static musl scoped), plus §1
  (marked as beyond the decision's list).
- `work/active/WP-227/PACKET.md`: the ≤ 2-page packet for the advisor's
  look, with every point where the text goes beyond the decision.

## What was not done

- The advisor's (Fable) look on the packet before "proposed": not run by
  me; the orchestrator schedules it (Fable thresholds are the
  orchestrator's to check).
- No WP numbers invented: phase 5–7 rows not yet queued say "—".
- VERSIONING.md (the app's row), CONTRACT.md (`edit` row, WP-229),
  STATUS.md (E96 entry), `docs/seldon-concept.html` (still CONCEPT v1):
  untouched, outside this WP's outputs.

## How it was verified

- `flock /tmp/seldon-check.lock just docs-check`: **ok** (529 links, 14
  translated pages, 61 commands, 704 command lines) — *desktop* (dev
  host, docs recipe; engine debug build in the worktree's `engine/target`,
  help runs under the script's temp HOME).
- docs-check covers the user guide and front pages only; the links of
  every file this WP touched were listed and each target confirmed to
  exist — *desktop*.
- Scan of the changed files for host names, home paths and the private
  folder's name: no hit — *desktop*.
- English only: by reading — *desktop*.
- Full `just check`: **not run** (docs-only change; no engine, plugin,
  schema or fixture file touched).

## Open questions (for the orchestrator, then the operator)

1. The rulings asked in PACKET.md "Where the text goes beyond the
   decision", chiefly `seldon open --json` (detached launch instead of
   exec) and the proposed drop of a settings view (parity row 4).
2. The morning list of the decision (§4: ADRs, AGENTS.md rows, PROJECT.md,
   merging `app`, exit codes, languages, external acts) is unchanged; the
   ROADMAP repeats the operator's acts.
