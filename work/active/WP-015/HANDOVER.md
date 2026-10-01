WP-015 HANDOVER

Branch `wp/015-fixture-corrections`, worktree `wt/WP-015`. Not pushed, no PR.
Commits: `508b38c` verification step + case walker · `a80ef81` green event +
snapshot pair · `fa01d93` three index variants · `3768aab` README + pitfalls ·
this handover. Each code commit passed `just check` (or, for `fa01d93`, the
validator) on its own state.

## Done

- **C-2026-001 verification step.** I added the Log line `2026-09-01 19:49 ·
  verification · human`. `ledger/2026-09.jsonl` has one `case-verified` line at
  19:49:05, id `01M1F1DGK8N56T9EW6KFSQX1PC` (ULID time part = ts; the random
  part is a sha256 of ts|source|kind|subject). It is inserted between the 19:45
  note and the 19:50 completion, and `ledger/2026-09.md` got the same line.
- **Case walker** in `scripts/validate-fixtures.py` (`check_case_logs`):
  - It walks every case's Log lines through the engine's `Transition::target`:
    created → queued, started, verification, completed, `dropped:` from any
    open status. Other Log lines are free text.
  - The walk must end in the frontmatter status. Its steps must equal the
    case's `case-*` ledger events (kind, minute, actor, order).
  - `created`/`started`/`closed` must be the dates of their steps, and
    `started (snapshot N)` must equal `snapshotBefore`.
  - Self-check 22 removes the new step in memory. The walk then reports
    "Log 'completed' from active".
- **Index-times check** (`check_times`), for the sample and every variant:
  `generatedAt` is not before any listed event, and `state.lastCapture` is not
  before any collector event.
- **`zone: green` event.** 10-01 10:38:50, `agent:claude-code`, C-2026-004,
  `command tee`, `meta.command "tee ~/.config/zed/settings.json"`, id
  `01M3V9VHCGTNRG2J62SMNR51TG`. The path is in C-2026-004's Plan
  ("Affected paths"), and no collector watches it. The id is added to
  C-2026-004's `events:`. See decision 2.
- **Surviving pre/post pair.** Snapshots 114 (pre) and 115 (post, `pairOf:
  114`), "tailscale: MagicDNS", 10-01 16:30:00 and 16:30:04, ids
  `01M3VXYHJ0CDTAWEV5WRW4C0C2` and `01M3VXYNF0XTBDWFTZP1412REN`. Both are in
  `logs/snapper.json` with `userdata.case: C-2026-008`, so the snapper
  collector reproduces them: the golden test checks this, and the blessed
  golden lines equal the ledger lines. `system.snapshots` now holds 6 entries,
  1 pair.
- **Variants** (RFC 6902 overlays in `VARIANTS`, generated with
  `--write-index`):
  - `index-stale`: `state.status: indexStale`. The overlay *tests*
    `generatedAt` 17:05:12 and `lastCapture` 17:05:00 rather than moving them.
    The validator requires both to lie more than 2 h before `STALE_NOW =
    2026-10-01T20:05:12+02:00`, which is the README's pinned `SELDON_NOW` and
    the plugin harness's existing stale clock.
  - `plugins-degraded`: collector `plugins` `ok: false`, message `omarchy plugin
    list --json: timed out`. This is the engine's exact text for that failure
    (`collectors/plugins.rs`).
  - `omarchy-git-checkout`: `system.omarchy.repoHead: 3f9c2e1`.
- **Counts.** Ledger 67 → 71 lines. Index events 58 → 62, `eventsToday` 27 →
  30, `events7d` 38 → 41. Unchanged: drift 4 (crisis 2, the `+3` group),
  folded 7, 8 cases, 4 decisions.
- **`fixtures/README.md`:**
  - the "(12 lines)" note;
  - story rows 09-01 (verified), 09-30 (marked WP-014) and 10-01 (green `tee`,
    pair 114/115);
  - the result counts;
  - a variant table naming `SELDON_NOW`;
  - the case-lifecycle and index-times rules;
  - the green assumption;
  - why `generatedAt` stays at 17:05:12;
  - the snapper diff.
- **`memory/pitfalls.md`:** WP-015 section appended.

## Not done

- **The sample's `generatedAt`/`lastCapture` were NOT moved to 09:30.** The
  brief asked to check that the README story and journal times stay
  consistent, and they don't. See decision 1. Everything else in the WP is
  done.
- **No plugin tests for the three new variants.** The brief limits
  `tests/plugin` to count changes, and none of main's plugin assertions
  changed. I loaded each variant once by hand in the headless harness
  (`tests/plugin/harness`, fake seldon; script not committed):
  - `index-stale` → `status indexStale`, banner "Index is stale", from the
    data alone (no `SELDON_NOW`). With `SELDON_NOW` 20:05:12 the tooltip shows
    "last capture 3 h ago".
  - `plugins-degraded` → status ok. Main's Service does not surface a
    non-snapper collector failure yet.
  - `omarchy-git-checkout` → ok.
  - The log was clean in all three.
  The next plugin WP should add them to `service-states.sh`.
- **jsonschema / check-jsonschema backends still untested.** Only the
  builtin backend ran, as in WP-014.

## Verified by

```
$ bash scripts/validate-fixtures.sh
validate-fixtures: ok — 101 instances (101 incl. 8 expected failures), 71 ledger events traced to index.sample.json, 5 variants, 22 self-checks; backend builtin
$ just check                     # fmt, clippy, cargo test (15 suites), schema, plugin validate, qmllint, plugin-test
service-states: 48 passed, 0 failed
check: ok                        # exit 0
$ cargo test plan::              # 12 passed
$ git diff --stat main -- fixtures/logbook/ledger/
 4 files changed, 8 insertions(+)        # no '-' lines; all 67 old lines byte-identical; both months chronological
```

- **Engine on a scratch copy of the logbook** (`engine/target/debug/seldon
  --logbook <copy>`):
  - `plan show` lists all 8 cases with the expected status and folder.
  - `plan done C-2026-008` (verification) succeeds.
  - `plan done C-2026-003` (active) exits 1 with "run `seldon plan verify`
    first".
  - `doctor` reports 8 cases, 4 decisions, 10 journal days and no misplaced
    case.
  - The copy was deleted afterwards. These runs were read-only on the host
    (doctor reads the Omarchy version and snapper).
- **The new checks can fail:**
  - In memory, `generatedAt`/`lastCapture` = 09:30 gives two `check_times`
    failures.
  - `STALE_NOW` = 18:00 gives two stale failures.
  - C-2026-001 with status `active` gives "the Log ends in completed".
  - Self-check 22 covers the old C-2026-001.
- **The new ledger lines round-trip byte for byte** through the engine's
  `Event::to_line()` (`fixture_ledger_lines_round_trip_byte_for_byte`).

## Learned (memory/pitfalls.md, WP-015)

- Fixture ledger lines are pinned by engine tests in five places (list in
  pitfalls). Snapper events need their `logs/snapper.json` entry. Run
  `cargo test --no-fail-fast`, because a failing lib test hides every
  integration test.
- Build ledger lines in the engine's key order (byte-for-byte round trip).
- Non-`seldon` case events belong in the case's `events:` frontmatter, in ts
  order.
- An index cannot be stamped earlier than its newest event. "Fresh for a live
  clock all day" is impossible, because fresh lasts 2 h.
- ADR-0014 §2 has no producer for green.
- `tar x` restores old mtimes, and cargo then skips the rebuild (it bit me
  while splitting commits). Touch restored sources.
- The guard reads file content in Bash arguments as commands (below).

## Decisions needed

1. **The sample's `generatedAt` (and `lastCapture`): keep 17:05:12, or move
   it?** Moving it to 09:30 as the brief suggests makes the sample
   inconsistent:
   - The index would list 23 events from after 09:30 (up to the 17:00 note).
   - `lastCapture` 09:30 would precede collector events it contains (the
     16:10 plugin-add, snapshots 113–115) and contradict `logs/snapper.json`
     (17:05).
   - No engine can write that.
   - The goal is also out of reach: an index is fresh for only 2 h, so no
     single time is fresh for a live clock all day. 09:30 is stale again from
     11:30.
   - Main's `tests/plugin/service-states.sh` pins `SELDON_NOW` 19:05 (fresh)
     and 20:05:12 (stale) against 17:05:12; a move would break both.

   *Recommend:* keep 17:05:12 (done; `check_times` now enforces consistency)
   and rely on `SELDON_NOW` and the `index-stale` variant. If a live reader
   before 17:05 must not see "just now", that is a plugin change: a future
   `generatedAt` could show the time instead of a relative age.

   *Alternative:* move it and drop `check_times`, accepting an index no engine
   produces.
2. **Producer rule for `zone: green`.** ADR-0014 §2 assigns green to nothing,
   and hook `command` events "take the zone of what the command would
   produce".
   - The fixture assumes that a hook command whose target no collector
     watches is **green**.
   - The other reading is "no zone". If that holds, the green event can only
     live in an index variant, and the sample keeps none.
   - WP-009 (hooks) is deciding hook zones right now.

   *Recommend:* an ADR-0014 successor with that green row, and tell WP-009.
   If you rule "no zone", I will move the event into a variant.
3. **Small assumptions; confirm or correct:**
   - `repoHead` is a 7-character short hash (like `logbook.git.head`; SPEC-ENGINE
     §4 gives no format).
   - `index-stale` keeps the sample's times and only sets the status. An
     engine writing `indexStale` itself is not specified anywhere.

## For other WPs (counts that moved)

- **WP-011** (not on main yet). `tests/plugin/model.test.js` and
  `panel-view.sh` on `wp/011-panel-tabs` assert the old sample. After a
  rebase onto this branch:
  - 58 → **62** rows / "62 events · newest first";
  - snapshot rows 6 → **8**;
  - Today stats `[27, 38, 2, 3, 4]` → **`[30, 41, 2, 3, 4]`**;
  - `+3`, folded 7 and crisis 2 are unchanged;
  - the System tab now has 6 snapshots, 2 of them a pre/post pair.
- **WP-007** (golden index). `index.sample.json` changed; rebase before
  freezing it.
- **WP-007 / WP-009** both edit `engine/`. My test-expectation edits in
  `engine/tests/support/mod.rs`, `engine/tests/idempotency.rs`,
  `engine/tests/golden/snapper.jsonl`, `engine/src/model/event.rs` (the test
  module) and `engine/src/collectors/snapper.rs` (the test module) may give
  one-line merge conflicts.

## Guard blocks (reported, not worked around)

Two Bash calls were blocked:
1. A heredoc writing a README edit script. Its *text* contained `tee
   ~/.config/zed/settings.json`, and the guard said "write under ~/.config
   outside the jax.seldon plugin dir".
2. A `sed` on README whose pattern contained `| pacman -Syu`. The guard said
   "privileged or package command".

Neither command touched `~/.config` or ran a package manager. As
`memory/guard-block-stop.md` allows for file content with trigger words, I
made those README edits with the Write/Edit tools. Nothing blocked was
executed by other means. The guard may want to ignore heredoc bodies and
quoted `sed` patterns.

## Touched outside WP scope

- `engine/` test expectations only, which WP-015.md allows ("No engine or
  plugin code beyond test expectations"). The brief said not to touch
  `engine/`, but `just check` cannot pass otherwise:
  - `src/model/event.rs`: ledger line count 67 → 71;
  - `src/collectors/snapper.rs`: list + 114, 115;
  - `tests/support/mod.rs`: `story()` + snapshot 114, 115;
  - `tests/idempotency.rs`: written 6 → 8, snapper 8 → 10;
  - `tests/golden/snapper.jsonl`: + 2 lines, blessed.
- `fixtures/logs/snapper.json` (collector input; under `fixtures/`).
- No changes to `schema/`, `plugin/`, `tests/plugin/`, `docs/` or
  `decisions/`. No `contractVersion` bump.
