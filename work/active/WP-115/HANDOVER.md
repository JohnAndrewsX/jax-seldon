# WP-115 — handover

Branch `wp/115-planned-link`, base `main` c9cac46. Normative: ADR-0029
§1–§5. No contract change (`contractVersion` 1; the schema gains one
description sentence).

## Done

- **Rule 9, the planned-and-active link** (`engine/src/reconcile.rs`:
  `case_windows`, `planned_links`, `link_planned`; wired in
  `commands/capture.rs` after rules 7 and 8 on one shared `read_all()`).
  Windows from the ledger's `case-started` … `case-completed|dropped`
  only; Plan token test = rule 3's `names_token`; any actor; uniqueness
  with a Log line per candidate when two or more; harm guard
  (`alwaysRed` → R3 only, else the existing R3 advisory line); pacman
  fan-out to the non-explicit members with `meta.txId`; `attach()` +
  `linked after the fact: …` Log line; catch-up over the whole ledger;
  idempotent. `capture --json` gains `linkedPlanned`.
- **`plan verify|done` capture first** (`commands/plan.rs`
  `capture_first`): default capture under its own lock, waiting up to
  8 s like a hook (`hook::LOCK_PATIENCE`, now `pub(crate)`), released
  before the step's lock; failure or a degraded collector → warning;
  `--no-capture`; `plan drop|start` never capture; `--json` gains
  `capture: {ok, written, linkedPlanned} | null`.
- **Engine resolutions re-resolvable** (`index/build.rs`: `fold` returns
  the ids whose winning line is by `system`; `Built::reresolvable`
  classified like any item, never listed; `reconcile::item_of` and
  `engine_resolved`; `commands/drift.rs`): `drift link|explain|dismiss`
  writes a later line that wins; ADR-0028 §3 agent refusals unchanged;
  `drift show` says "by the engine".
- **Fixtures**: C-2026-002 (completed) — a `plugin-add` of
  `io.github.example.display-profiles` at 10:59:20 during its
  verification, its Plan names the plugin, one `linked` line by `system`
  at 11:04, `events:` and Log line; `index.sample.json`, variants and
  `index.attention-all.json` regenerated (`--write-index`), ledger view
  `2026-09.md`, `fixtures/README.md` story row and counts (83 lines, 10
  resolutions, 73 index events), golden `REBUILD.md` (the plugin now
  appears with its case — the product promise).
- **Validator**: `scripts/validate-fixtures.py` ports rule 9
  (`case_windows`, `planned_links`, `check_planned_links`: the sample is
  a post-capture state) plus three self-checks (line missing, unplanned
  subject, two cases).
- **Docs**: SPEC-ENGINE §3 (`plan … [--no-capture]`, `capture` JSON,
  `linkedPlanned`), §5 rule 9 (+ a note under rule 3, the re-resolvable
  paragraph), §10; `schema/event.schema.json` description of
  `resolution`; AGENT-GUIDE §8; CHANGELOG; user CLI reference help
  blocks regenerated (`docs-check.sh --write`).
- **Skill sentence** (the only skill change, for WP-111's awareness):
  `engine/assets/skills/seldon/case.md`, section *Close It Yourself*,
  after the refusal paragraph: "`plan verify` captures first; a step you
  handed to the user is recorded and linked when you verify."

## Not done

- ADR-0029 §5 item 10 (live check on the test host, capture cost of
  `plan verify` into SPEC-ENGINE §3): the orchestrator's after the merge.
  SPEC §3 says where the number goes.

## Decisions (ADR silent)

1. **Window ends inclusive** on both sides: an event in the same second
   as `case-started` or the close counts (pacman logs whole seconds).
2. **Harm guard by subject**, as the ADR's text says (`alwaysRed`
   matches), whatever the event's class (so a named `upgrade-red`
   attention item is guarded too); the risk is the case's `risk` now.
3. **Pacman transactions:** with an explicit member, only explicit
   members are tested; non-explicit members follow only when every
   linked explicit member of the transaction went to one case (else they
   stay). Other explicit members the Plan does not name stay drift (ADR:
   "non-explicit members"). A transaction without explicit members tests
   each member alone.
4. **Log lines:** one `linked after the fact` line per tested event (not
   per dependency), in event order; the time is the event's `HH:MM:SS` in
   its own offset (no TZ dependence); the ambiguity line names the event
   id and the `drift link` command; every line is written once
   (whitespace-normalised text compare, as `log_line` writes it).
5. A case file that does not load is no candidate (its Plan cannot be
   read).
6. **`plan verify|done` capture**: runs only when the step is allowed
   (case found, transition valid; checked again under the step's lock);
   each degraded collector is its own warning (`capture before the step:
   snapper degraded: …`).
7. **Re-resolution** away from an engine-linked case removes the id from
   that case's `events:` and logs `no longer linked here: <ids> (<verb>
   [to <case>] by <actor>; the engine had linked it)`; ADR §3 is silent
   on the case file, and leaving the id would make the case list an event
   the index folds elsewhere. Engine items are not in `drift --all`.
8. Rule-8 (`jax-seldon`, `jax.seldon`) and rule-7 explanations become
   re-resolvable too (ADR §3 names rules 7, 8, 9).
9. **Fixture event is a `plugin-add`, not pacman**: a pacman line would
   have to go into `fixtures/logs/pacman.log` (byte offsets, rotation
   fixtures, collector goldens), a config change adds a dossier
   deviation row; a plugin touches neither. The pacman path is covered
   end to end in `tests/planned_link.rs`.
10. `linkedPlanned` counts resolution lines (dependencies included).

## Tests

- Unit (`reconcile.rs`): `the_window_is_read_from_the_case_events`,
  `one_case_any_actor_and_never_over_a_resolution`,
  `the_harm_guard_wants_r3`, `dependencies_follow_their_explicit_member`.
- Integration (`tests/planned_link.rs`, temp HOME, `TZ=Europe/Berlin` in
  the helper): acceptance 1 (C-2026-004 as it happened, incl. second
  capture writes nothing), 2 (verify captures; ledger order event →
  resolution → `case-verified`), 3 (window: closed, queued, dropped), 4
  (two cases), 6+8 (harm guard R1/R3, agent `explain` of an engine-linked
  crisis exits 1, human `dismiss` wins, case loses the id, re-run writes
  nothing), 7 (fan-out with `meta.txId`), 9 (`--no-capture`, failed
  capture is a warning, `drop` never captures). Acceptance 5 (actors) in
  the unit tests.
- Existing tests adjusted: `tests/agent.rs` (two `plan verify|done` get
  `--no-capture`: the capture would call the recording `omarchy` stub),
  `tests/plan.rs` (one `verify --no-capture`: it asserts no warnings),
  counts in `model/event.rs` and `tests/index.rs`, `tests/rebuild.rs`
  plugins 3 → 4.

## Mutants

25 manual mutants, each applied alone to an on-disk copy of the branch
(`git archive HEAD`, own target dir; the worktree untouched while the
gate waited for the lock), run against the tests that should catch it,
source restored after each (runner in the session scratchpad). 24
killed, 1 survived:

| # | Mutant | Caught by |
|---|---|---|
| M1 | window end exclusive | `the_window_is_read_from_the_case_events` |
| M2 | window never ends | same, `the_window_comes_from_the_case_events` |
| M3 | first of several cases links | `one_case_any_actor…`, `two_cases_that_planned_it…` |
| M4 | no harm guard | `the_harm_guard_wants_r3`, `the_harm_guard_and_a_later_line_that_wins` |
| M5 | harm guard ignores R3 | same two |
| M6 | dependencies never follow | `dependencies_follow_their_explicit_member`, `the_dependencies_follow_the_planned_package` |
| M7 | no `meta.txId` | same two |
| M8 | links over a resolution | `one_case_any_actor…`, acceptance 1, harm/later-line test |
| M9 | links a cased event | `one_case_any_actor…` |
| M10 | line at capture time, not `max(now, ts)` | `one_case_any_actor…` |
| M11 | "no capture ran before the close" never | unit + acceptance 1, window test |
| M12 | Log lines written again | `two_cases_that_planned_it…` (said once) |
| M13 | `attach()`'s result ignored for the save | **survived** — equivalent in practice: every new id comes with its `linked after the fact` Log line in the same case, so the file is saved anyway; it differs only if that exact Log line already exists without the id |
| M14 | no `attach()` at all | four integration tests |
| M15 | dependencies follow when explicit members went to two cases | `dependencies_follow_their_explicit_member` |
| M16 | every non-explicit pacman member follows | same |
| M17 | `has_planned_candidates` always false | six integration tests |
| M18 | capture skips rule 9 | six integration tests |
| M19 | `plan verify|done` never capture | `plan_verify_captures_first…`, `no_capture_and_a_failed_capture` |
| M20 | `--no-capture` ignored | `no_capture_and_a_failed_capture`, acceptance 1 |
| M21 | a failed capture fails the step | `no_capture_and_a_failed_capture` |
| M22 | engine lines not re-resolvable | `the_harm_guard_and_a_later_line_that_wins` |
| M23 | engine-resolved items listed as drift | same |
| M24 | any resolution re-resolvable (not only `system`) | same, and three `tests/drift.rs` tests |
| M25 | the engine-linked case keeps the id after a re-resolution | `the_harm_guard_and_a_later_line_that_wins` |

The validator port has its own three mutation self-checks (see Done).

## Checks

All under `flock /tmp/seldon-check.lock`, `CARGO_TARGET_DIR=~/.cache/
seldon-target-wp115` (on disk), head 5c52283 (code identical to the
handover commit; only this file follows):

- `just check`: **exit 0** (`check: ok`). Rust 1852 passed, 0 failed, 8
  ignored (the release-timing tests); clippy `-D warnings` and fmt clean
  (also with `--features watch`); `validate-fixtures: ok` (83 ledger
  events, 46 self-checks); `docs-check: ok`; schema; packaging, install,
  deploy tests; `omarchy plugin validate`; `qmllint: ok (29 files)`;
  plugin harness: `model.test.js` 96, `panel-view` 900, `overlay-view`
  326, `bar-view` 194, `service-states` 316, `real-home-guard` 11 — all
  passed.
- `just check-perf` (the WP touches the index build): **exit 0**. Index
  build ×10 median 4.5 ms, ×150 median 55 ms (budget 100 ms); `status` at
  the stated scale (now 10 292 lines) median 43.5 ms (budget 100 ms);
  hooks ≤ 4.4 ms median (budget 5 ms, unchanged code path); redaction
  within budget.
- On the way: the first full runs failed in the plugin harness and in
  `check-perf` only because they pin the fixture's counts (72 → 73
  events, 7 → 8 folded resolutions, 11/6 → 12/7 drift opened/resolved,
  10 044 → 10 292 stated-scale lines); updated in `tests/plugin/
  model.test.js` (plus one assertion that the engine link renders as
  `linked to C-2026-002: planned by …`), `panel-view.sh`,
  `overlay-view.sh`, `tests/index.rs`, `scale.rs`, TESTING.md, SPEC §1,
  justfile comment. No plugin code changed.
- Not run here: ADR-0029 §5 item 10 (live check, test host).

## Open questions

1. **Warnings on every close.** On a host where snapper degrades (no
   read grant) or a plugin collector cannot run, every `plan
   verify|done` carries `capture before the step: … degraded` warnings,
   as ADR-0029 §2 says. If that is noise for the panel or the agent,
   warn only on a failed capture and leave degraded collectors to
   `doctor`? (An ADR wording question; I kept the ADR.)
2. ADR-0029's open point (exactly one *active case at all* vs. one that
   planned it) is untouched; the live round counts false links.

## Round 2

Brief: `review-0.1.1/handovers/WP-115-round-2-brief.md` (stage 1
`WP-115-review-1.md`). Main not merged (orchestrator resolves).

### Done

1. **B1 / Q1 — harm guard = harm test.** `reconcile::planned_links`
   guards when the subject matches `[drift] alwaysRed` **or** the
   classifier (`index::class::Classifier`, the same rules as the index)
   makes the single event a crisis — i.e. `always-red-paths` (user units,
   autostart, environment.d, uwsm, `~/.profile`, Omarchy hooks; an inert
   `*.sample` hook or an evidence row stays unguarded, as in the index).
   Below R3 the event stays drift/crisis and the case gets one advisory
   Log line: the existing R3 advisory for an `alwaysRed` package with a
   known risk, else `advisory: not linked: <event> can affect boot, login
   or the shell (`[drift] alwaysRedPaths`), which only an R3 case takes,
   and <ID> was R<n> at the time (ADR-0027 §2c); if this case made it:
   `seldon drift link <EVENT> <ID>``. Reviewer's probe and a hook path
   are tests (unit + e2e).
2. **Q3 — risk at the event's time.** `risk_timeline` reads the case's
   own Log: the `created (…, risk Rn…)` line (also `created
   retroactively …`, reopen's `created …: reopens …`), then every `set …
   risk A → B`. `PlanningCase::risk_at` takes the last change before the
   event's minute; a change in that very minute, or no `created` line,
   cannot tell → below R3. Log lines are local time to the minute, so the
   engine compares in `chrono::Local` (`link_planned`); the unit tests and
   the validator use the event's own offset (fixture and tests are one
   offset). Test: an agent raises the case to R3 after `linux-zen` → no
   link; `linux-lts` installed after the raise → linked.
3. **B2 / Q2 — unreadable case blocks.** `unreadable_windows`: a
   candidate whose time lies in the window of a case id with no loadable
   file (does not parse, or lies outside the status folders) is not
   linked (`PlannedLinks::blocked`); the capture warns on stderr and
   `seldon doctor` gets a `planned` row (degraded, only when it applies,
   fix: repair the file / put it back). Reviewer's probe is a test; after
   the repair the next capture sees two planners and links nothing.
4. **Q4 — Plan comments.** `cases::strip_comments` on the Plan for rule 3
   (`index/load.rs`) and rule 9 (`PlanningCase::of`); validator
   `plan_section` too. Fixture and golden index unchanged (no fixture
   Plan names a subject only in a comment). Test: `<!-- do not install
   glow -->` neither links nor proposes.
5. **R1 — capture warning.** `capture_first` warns only on `Err` (an
   error, or the lock still held after 8 s); degraded collectors are
   doctor's. `tests/plan.rs` lost its `--no-capture`; `plan_verify_
   captures_first…` asserts `warnings: []` with snapper and plugins
   degraded. SPEC §3, AGENT-GUIDE, CHANGELOG.
6. **N1** — MI killed by `rule_8_wins_over_rule_9_in_the_same_capture`
   (a `jax-seldon` upgrade inside a case whose Plan names it: one
   `explained` line, `linkedPlanned: 0`, the case lists nothing); MK
   killed by a same-case human re-link in acceptance 1 (the case keeps the
   id, no "no longer linked" line). **N2** — CHANGELOG ("while a case
   whose Plan names it is open, and no other case open at the time names
   it") and AGENT-GUIDE ("no other case that was open at the time") now
   say the implemented rule.
7. Validator (`scripts/validate-fixtures.py`): guard via its
   `Classifier`, `risk_timeline`/`risk_at`, unknown-case block,
   comment-stripping; five new self-checks (path below R3 not linked;
   path linked to a case raised to R3 before it; unreadable case blocks
   with a second planner; Plan comment) — 50 self-checks.
8. `docs/user/de/05-cli-reference.md` source marker moved to 62e6048
   (round 1 changed only generated help blocks, regenerated in both
   languages); this removes the docs-check translation warning round 1
   left.

### Decisions (round 2)

- The guard asks the classifier for the **single event** (not its
  pacman group): rule 9 decides per event; a dependency follows its
  explicit member as before.
- An `alwaysRed` package keeps the subject test whatever its class
  (round 1, D2) — the brief's "alwaysRed packages *or* alwaysRedPaths".
- Same-minute ambiguity resolves to "cannot tell" (below R3) unless the
  change leaves the risk as it was.
- `plan set` without `risk` (zone/area only) does not touch the
  timeline.
- A blocked event writes no Log line into the readable case (it cannot
  name the other planner's Plan); the capture warning and doctor carry
  it.
- ADR-0029 §1(d)'s clarifying note (guard = harm test, risk at the
  time, unreadable case) is written into SPEC-ENGINE §5 rule 9; the ADR
  itself is accepted and immutable — a note there is the orchestrator's.

### Mutants (round 2)

Engine: on-disk copy (`git archive HEAD` at 19022cf, own target dir),
each alone, sources restored. Validator: the same copy, each applied to
the script alone.

| # | Mutant | Result |
|---|---|---|
| N1 | guard ignores the classifier's crisis | killed: `the_harm_guard_covers_persistence_paths`, `a_persistence_path_the_plan_names_stays_a_crisis_below_r3` |
| N2 | risk ignores `plan set` lines | killed: `the_guard_reads_the_risk_at_the_events_time`, `raising_the_risk_afterwards…` |
| N3 | a change in the event's minute counts as before | killed: unit |
| N4 | unknown risk counts as R3 | killed: unit |
| N5 | risk now, not at the time | killed: unit, `raising_the_risk_afterwards…` |
| N6 | unreadable case does not block | killed: `a_window_of_an_unreadable_case_links_nothing`, `a_case_file_that_does_not_load_blocks_the_link` |
| N7 | rule 9 reads Plan comments | killed: `a_plan_comment_is_no_plan` |
| N8 | rule 3 reads Plan comments | killed: `a_plan_comment_is_no_plan` |
| N9 | doctor has no `planned` row | killed: `a_case_file_that_does_not_load_blocks_the_link` |
| N10 | (MI) rule 9 without rule 8's lines of the same capture | killed: `rule_8_wins_over_rule_9_in_the_same_capture` |
| N11 | (MK) same-case re-link drops the id | killed: acceptance 1 |
| N12 | a degraded collector warns again | killed: `plan_verify_captures_first…` |
| N13 | same-minute values not compared | killed: unit |
| V1 | validator: guard ignores `alwaysRedPaths` | killed: self-check "persistence path below R3" |
| V2 | validator: unreadable case does not block | killed after a stronger self-check (first try survived: no second planner) |
| V3 | validator: Plan comments count | killed: self-check "Plan comment" |
| V4 | validator: risk at the time ignored | killed after a new self-check (case raised to R3 before the change) |

17 of 17 killed (V2, V4 needed the stronger self-checks, committed in
3f117d9). Reviewer's ME (follower test `explicit == Some(false)`)
stays equivalent as they noted.

### Checks (round 2)

Under `flock /tmp/seldon-check.lock`, `CARGO_TARGET_DIR` on disk, head
3f117d9 (code identical to the round-2 handover commit; only this file
follows):

- `just check`: **exit 0** (`check: ok`). Rust 1868 passed, 0 failed, 8
  ignored; clippy/fmt clean (also `--features watch`);
  `validate-fixtures: ok` (83 events, 50 self-checks); `docs-check: ok`
  (no translation warning); `qmllint: ok`; plugin harness model 96,
  panel 900, overlay 326, bar 194, service-states 316, real-home-guard 11.
- `just check-perf` (`index/load.rs` is the index build): **exit 0**.
  Index ×10 median 4.6 ms, ×150 55 ms; `status` at the stated scale
  42.8 ms; hooks ≤ 4.3 ms median.

### Open

- N4 (cost): `has_planned_candidates` stays true while any unresolved
  event lies in any window; every such capture reads the case files. The
  item-10 measurement should use a logbook with open attention items.
- ADR-0029 §1(d) note (see Decisions) — orchestrator.
