# WP-076 HANDOVER

Branch `wp/076-review`, worktree `wt/WP-076`. Commits on top of `402fa0a`:

- `78e35cf` engine: clip index texts to 256 bytes with a visible marker, warn over the 1 MB budget
- `3fdf35f` engine: assert the x150 bench; status and hook budgets at the stated scale in just check-perf
- `b8af497` engine: the collector test bench sets every source to a scratch path, none to the host's
- `4a77d92` plugin tests: work-live settles each engine step before waiting for the card
- `176b1c8` engine: hook budget also just below the rebuild threshold in just check-perf
- `ac47e23` docs: index size budget and the perf budgets at the stated scale in SPEC-ENGINE §6 and §8, CHANGELOG
- plus the commit with this handover and the pitfalls entry

## Done

- **F-134: the index size budget is enforced, with a visible marker.**
  `index/build.rs`:
  - `clip()` cuts any text that takes more than `TEXT_MAX` = 256 bytes
    in JSON (escapes counted, marker included). The cut falls on a
    character boundary, and the text ends in
    `… (N more characters in the ledger)`. N is the number of characters
    left out, so a cut is never silent.
  - It applies in `index.events` (`detail`, `resolutionDetail` and
    every string in `meta`, typed and `extra`) and in `index.drift`
    (`detail`).
  - The WP names `detail`, `meta.command` and `resolutionDetail`. I
    clip every `meta` string because `seldon event --meta key=value`
    accepts any key at any length, so clipping only `command` would not
    bound the size.
  - `subject` is not clipped: it is at most 512 characters by schema,
    and the plugin and drift use it as an identifier.
  - `built.folded` and `built.ledger` stay whole, so the ledger line,
    the `ledger/*.md` views and the member events of `drift show` keep
    the full text. `drift list` and the `item` of `drift show` come from
    `index.drift` and show the clipped detail. (Corrected in round 2,
    N3.)
  - `over_budget()` counts the serialised bytes without allocating. If
    the index is ≥ `SIZE_BUDGET` = 1 000 000 bytes, it adds a warning
    that names the largest section (`index`/`status` stdout and `--json`
    `warnings`). Open cases, decisions and topics are still uncapped.
  - **No schema change:** no new field, `contractVersion` stays 1, and
    `meta.truncated` stays reserved. The golden test is unchanged: the
    longest fixture text is 137 characters.
- **F-403: perf budgets at the stated scale.**
  - `benches/index.rs` asserts ×150 (10 650 lines, 1 200 cases) < 100 ms,
    as well as ×10.
  - New `just check-perf` (not in `check`, comment beside `check-rss`):
    it runs the bench and then
    `cargo test --profile bench --test index --test hooks -- --ignored --test-threads=1`.
  - The ignored tests are:
    - `index::status_at_10_000_ledger_lines_is_under_100_ms`:
      `scale::stated_scale` = the ledger ×141 (10 011 lines), 304 cases
      and 365 journal files, i.e. SPEC §6's 10 000 / 300 / 365. Median of
      11 CLI runs, process start included.
    - `hooks::claude_code::fast_enough_at_10_000_ledger_lines`.
    - `hooks::claude_code::fast_enough_just_below_the_rebuild_threshold`:
      950 lines, the hook's worst case. It asserts that the last command's
      rebuild put it in the index.
    - Each hook test checks a call the hook does not record and a
      recorded command, each < 5 ms, median of 21 calls.
  - New helpers:
    - Appended to `tests/common/mod.rs`: `assert_optimised`,
      `median_time` and `assert_within_budget`. The last re-measures
      once before it fails, so a load spike does not fail the check; a
      real slowdown fails both times.
    - In `scale.rs`: `scaled_logbook_with` (case factor), `stated_scale`,
      `filler_notes` and `journal_year`.
  - SPEC §8 gains a budget sentence that matches WP-057's threshold
    decision, with the measured numbers. SPEC §6 gains the size
    paragraph, and its bench sentence now names ×150 and `check-perf`.
- **Fold-in: the test bench never defaults to host paths.**
  - `Bench::new` (`tests/support/mod.rs`) no longer uses
    `..Sources::default()`. Every field is spelled out, so a new
    `Sources` field fails to compile until the bench sets it.
  - The remaining hole was `omarchy: "omarchy"`, i.e. the host's real
    program on PATH. No test runs the plugins collector through the
    bench yet. `plugins_dir` and `theme_file` already fell back under
    the scratch home; they are now explicit too.
  - New test `support::the_bench_reads_nothing_of_the_host` (runs in
    `collectors`, `idempotency` and `redaction`).
- **Fold-in: `work-live` waits for the settled result.**
  - In `tests/plugin/panel-view.sh` (the only plugin test file touched),
    the verify, done and drop steps now `settle` (the engine call
    finished and its result line is set) before
    `wait:work.card.status=…` (the index reload moved the card), as the
    start step already did.
  - Step numbers after 28 are shifted (+1/+2/+3); no expectation's value
    changed.

## Not done

- `docs/TESTING.md` "Other recipes" does not name `just check-perf` yet:
  the file is not in this WP's list.
- `scripts/validate-fixtures.py` `derive()`, the reference
  implementation, does not clip. The two agree on every fixture (all
  texts ≤ 137 characters) but would differ for a text over 256 bytes.
  The file is not in this WP's list.
- ADR-0020 is not amended: ADRs are immutable, and the finding suggested
  amending it. See *Decisions needed*.

## Verified by

Measured 2026-10-04 on the dev host (shared; load 4 to 15 during the
runs), bench profile, temp dir on tmpfs:

| Check | Budget | Median |
|---|---|---|
| index build ×10 (710 lines, bench) | < 100 ms | 4.3 to 4.5 ms |
| index build ×150 (10 650 lines, 1 200 cases, bench) | < 100 ms | 78 to 81 ms |
| `status`, 10 011 lines / 304 cases / 365 journals | < 100 ms | 46 to 47 ms |
| hook, not recorded, 10 000 lines | < 5 ms | 1.3 ms |
| hook, recorded, 10 000 lines (no rebuild) | < 5 ms | 1.7 to 1.8 ms |
| hook, not recorded, 950 lines | < 5 ms | 1.3 ms |
| hook, recorded, 950 lines (with rebuild) | < 5 ms | 3.5 to 3.7 ms |

- ×150 is not a regression. An interleaved A/B gave the same result:
  v0.1.1 77.7 / 78.3 ms against this branch 81.1 / 79.7 ms. The
  pre-change `build.rs` against the new one gave 78.2 / 85.2 against
  78.0 / 81.3 ms, so the size count costs nothing measurable.
- `status` with the full ×141 case set (1 128 cases) is about 92 ms;
  the cases dominate (A/B in one process).
- The synthetic worst case is 500 events and 200 drift items with
  4096-character texts (quotes, tabs, control characters, `ä`, `🚀`).
  The index is 522 723 bytes (8 534 651 bytes without the clip).

Gates:
- `just check` exit 0 on `ac47e23` ("check: ok"; 1 171 passed, 0 failed,
  6 ignored = the 3 release tests × with and without `watch`;
  panel-view 733 passed).
- `just check-rss` exit 0.
- `just check-perf` exit 0 (numbers above).

**Mutants.** Each was applied to the working tree, run and reverted with
`git checkout`; none was committed:

| Mutant | Result |
|---|---|
| M1 `index.events` `.cloned()`, drift `detail` unclipped | `long_texts_keep_the_index_under_its_size_budget` FAILED (8 534 651 bytes), `a_long_command_line_is_whole_in_the_ledger_and_clipped_in_the_index` FAILED |
| M2 `over_budget` result dropped | `an_index_over_its_budget_warns` FAILED |
| M3 `clip` room ignores the marker (`room = TEXT_MAX`) | `clip_keeps_short_texts_and_marks_long_ones` and the two above FAILED |
| P1 every budget halved (bench 50 ms, status 50 ms, hook 2.5 ms) | `just bench` exit 1 ("index build ×150 over the 50ms budget", 79.4 ms); `fast_enough_just_below_the_rebuild_threshold` FAILED (recorded 3.69 / 3.81 ms); still passing with > 2× headroom: status 46.3 ms, hook not recorded 1.36 ms, hook recorded at 10 000 lines 1.76 ms |
| P2 `sleep(60 ms)` in `status::run`, `sleep(4 ms)` in `run_agent_hook` | status FAILED (105.4 / 106.2 ms); both hook tests FAILED (not recorded 5.49 / 5.43 ms and 5.48 / 5.52 ms) |
| B1 `Bench` back to `..Sources::default()` | `the_bench_reads_nothing_of_the_host` FAILED |
| B2 only `omarchy: Sources::default().omarchy` | FAILED: "omarchy is not a scratch or fixture path" |
| H1 `sleep 1` after `rewrite_index` in `fake-seldon` `plan_step` (the race made deterministic) | old steps: FAIL #29 "Moving to verification: C-2026-005…", #45 "Dropping C-2026-004…", plus the cascade (#32, #34, #41, engine argv); new steps: 733 passed, 0 failed. Without the mutant: 733 passed, 0 failed |

## Learned

Six bullets are in `memory/pitfalls.md` (2026-10-04 · WP-076):
- load spikes on the shared host;
- the scaled fixture multiplies cases;
- the hook's two speeds around the 1000-line threshold;
- `--no-fail-fast` for multi-binary mutant runs;
- events at one fixed `SELDON_NOW` tie on `ts`;
- `settle` + `wait:` in the panel harness.

## Decisions needed

1. **CI `just bench` now asserts ×150 < 100 ms.** It measures 78–81 ms
   here, so the headroom is about 20 %. v0.1.1 is the same, so this is
   not a regression. I have no CI timing: if the GitHub runner is slower,
   CI goes red. The options are:
   - (a) keep it, as the WP and the finding ask;
   - (b) assert ×150 only in `check-perf` and keep ×10 in CI;
   - (c) assert the stated scale (304 cases, about 2× headroom) in the
     bench instead of ×150, and keep ×150 printed.
   `.github/workflows/ci.yml` is not in this WP's list.
2. **Should the clip get an ADR?** The behaviour is visible to the
   plugin even though the schema is unchanged: texts may end in the
   marker, and `meta.truncated` is still reserved. ADR-0020 deferred
   truncation on the assumption that only drift is unbounded; this WP
   shows that long command lines are unbounded too. A short ADR that
   supersedes or extends ADR-0020 would record it.
3. Name `just check-perf` in `docs/TESTING.md`, and make the reference
   `derive()` in `scripts/validate-fixtures.py` clip the same way. Both
   files are outside this WP.

## Touched outside WP scope

- `engine/tests/support/mod.rs` and `tests/plugin/panel-view.sh`: the two
  fold-ins named in the brief.
- `memory/pitfalls.md`: appended.
- Temporary, uncommitted mutants in `tests/plugin/fake-seldon`,
  `engine/src/commands/{hook,status}.rs`, `engine/src/index/build.rs`
  and `engine/tests/support/mod.rs`, all reverted (`git status` clean).
- A scratch worktree of `v0.1.1` and its target under the session
  scratchpad, for the A/B. Both are removed (`git worktree list`
  verified).
- No host writes, no guard block. The engine ran only in scratch
  HOME/XDG with `SELDON_TEST_GUARD` (`common::Env`).

## Review round 1 (SEND BACK): round 2

Commits on top of `902fde4`:

- `83ba56a` engine: the bench asserts x150 only under SELDON_BENCH_X150=1 (check-perf), re-measures once before failing
- `acd51ba` engine: the user-collector test bench sets scratch sources, none of the host's
- `51e2dc9` engine: the status budget test asserts the open cases and case files of its scale
- `d272c57` decisions: ADR-0025 (index text clip, extends ADR-0020)
- `ef3624c` docs: CONTRACT rule 5 as built (ADR-0025); SPEC §6 bench opt-in, drift show wording; CHANGELOG
- `ec029a8` docs: just check-perf in TESTING
- plus the commit that updates this handover

### What changed

- **B1, Decision 1 = (b).** `engine/benches/index.rs`:
  - ×10 is always asserted. ×150 is always printed and asserted only
    with `SELDON_BENCH_X150=1`. Each line says `asserted` or
    `printed only` and gives the attempt number.
  - An asserted case over budget is measured once more (the scaled
    logbook is kept) before the bench exits 1.
  - `.github/workflows/ci.yml` is not touched: CI's `just bench` runs
    without the variable, so the CI gate is ×10 only.
  - `justfile`: `check-perf` runs the bench with
    `SELDON_BENCH_X150=1`. Its comment now says that every check, the
    bench included, measures once more; the `bench` comment says that
    ×150 is printed and that `check-perf` asserts it.
- **N1.** `tests/collectors_user.rs` `Bench::new` spells out scratch
  sources (paths under its temp dir, a missing program for snapper,
  omarchy-version, pacman and omarchy) instead of `Sources::default()`.
  New test `the_bench_reads_nothing_of_the_host` (same shape as the one
  in `support`). The support helper is not reused: the file has its own
  `Bench`, and `mod support` would bring a second one.
- **N2.** `status_at_10_000_ledger_lines_is_under_100_ms` asserts 228
  open cases in the index and 304 case files under `work/`; the
  completed group in the index is capped at 50, so the files carry the
  full count.
- **N3.** The wording is corrected here (Done, F-134), in SPEC-ENGINE
  §6 and in the CHANGELOG bullet, which had the same mistake.
- **Decision 2.**
  - `decisions/ADR-0025-index-text-clip.md` (accepted, 2026-10-04)
    extends ADR-0020, with the content as decided.
  - Its consequences also note the plugin over-budget warning as an
    operator decision (Decision 5).
  - A row in the `DECISIONS.md` index; that file is outside the WP
    list, but an ADR is indexed there.
  - CONTRACT.md rule 5 now describes the actual behaviour and cites
    ADR-0025: the counts of rule 4 plus the clip, cases, decisions and
    topics uncut, a warning at ≥ 1 000 000 bytes, `meta.truncated`
    deferred. Wording only; no schema or fixture change. SPEC §6 cites
    ADR-0025 too.
- **Decision 3.** `docs/TESTING.md` "Other recipes" names
  `just check-perf`: what it runs, that it needs an optimised build and
  a quiet host, that it is opt-in and not in `check` or CI, and that it
  re-measures once.
- **CHANGELOG.** I edited this WP's own two unreleased bullets instead
  of adding contradicting ones. The `check-perf` bullet says that
  `just bench` (CI) asserts ×10, only prints ×150, and re-measures once;
  the clip bullet has the `drift show` correction and cites ADR-0025.
- SPEC §6's performance paragraph now says the same as the bench: ×10
  in `just bench`/CI, ×150 under `check-perf`.

### Mutants (round 2)

Each was applied to the working tree, run and reverted; none was
committed.

| Mutant | Result |
|---|---|
| bench `BUDGET` 50 ms, `SELDON_BENCH_X150=1` | ×150 77.2 ms (attempt 1), 76.8 ms (attempt 2), "index build ×150 over the 50ms budget", exit 1 |
| bench `BUDGET` 50 ms, no opt-in | ×150 81.0 ms "(printed only, attempt 1)", exit 0 |
| `collectors_user` `Bench` back to `Sources::default()` | `the_bench_reads_nothing_of_the_host` FAILED |
| `stated_scale` case factor 38 → 10 | `status_at_10_000_ledger_lines_is_under_100_ms` FAILED: "open cases of the stated scale", left 60, right 228 |

### Verified by (round 2, 2026-10-04, load 3.5 to 5)

- `cargo fmt --check` ok.
- `cargo clippy --all-targets -- -D warnings` exit 0.
- `--test index --test status --test hooks --test collectors_user`:
  24 + 11 + 51 + 31 passed, 0 failed (3 ignored = the release tests).
- `docs-check` ok.
- `just bench` three times in a row without the opt-in, all exit 0,
  with ×150 printed only:
  - run 1: ×10 4.53 ms, ×150 77.9 ms;
  - run 2: ×10 4.22 ms, ×150 77.6 ms;
  - run 3: ×10 4.43 ms, ×150 78.0 ms.
- `just check-perf` exit 0, every check on attempt 1:
  - ×150 77.3 ms (asserted);
  - `status` 45.4 ms;
  - hook at 10 000 lines: 1.34 ms not recorded, 1.73 ms recorded;
  - hook at 950 lines: 1.31 ms not recorded, 3.67 ms recorded.
- `just check` was not re-run in round 2: the brief listed the suites
  above.

### Decisions resolved

1. CI bench gate: option (b), done as above.
2. ADR-0025 written, CONTRACT rule 5 updated. I agree rule 5 should
   change: its old text promised a section truncation with a field that
   does not exist.
3. `just check-perf` documented in TESTING.md; the clip in
   `validate-fixtures.py` stays with WP-077.
5. The over-budget warning for the plugin is recorded, with no action.

### Touched outside WP scope (round 2)

- `engine/tests/collectors_user.rs`: N1, as the review asked.
- `decisions/ADR-0025-index-text-clip.md`, `DECISIONS.md` (index row),
  `docs/CONTRACT.md` rule 5, `docs/TESTING.md`: Decisions 2 and 3.

