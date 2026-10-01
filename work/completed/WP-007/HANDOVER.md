WP-007 HANDOVER

Branch `wp/007-index-status`, worktree `wt/WP-007`, **rebased onto `main` at
`cff6f8a`** (WP-015 merged). Not pushed, no PR. Commits `main..HEAD`:
`63408aa` index builder, views, `index`/`status` · `630f8d2` rebuild wired
into the writing commands · `7353ff9` tests, golden test, bench · `7c9b0f1`
cleanup · `603a04c` memory · `af7f7dd`/`6af8580` handover · then the review
follow-ups `44d852d` engine · `334c77c` tests · `c6936ea` bench profile ·
`cc1bba5` memory · and this update. `just check` exits 0 at HEAD (220
tests, 17 suites); `just bench` exits 0.

## Review follow-ups (after APPROVE)

0. **Rebase.** `git rebase main` was clean except `memory/pitfalls.md`.
   I kept both sections: WP-015's first, then WP-007's.
   - Against WP-015's fixture (71 ledger lines, 62 events, the `zone: green`
     event, snapshots 114/115) the golden test passes **unchanged, with zero
     differences**, and the ledger views are still byte-identical.
   - Two tests had pre-WP-015 numbers hard-coded: the ×10 line count, and
     the event counts in the expected STATUS.md (now 30/41, as in the
     sample's `summary`). Both are updated.
1. **Tests.**
   - `lines == 710` (71 × 10).
   - `the_engine_checker_agrees_with_jsonschema` now validates every file
     in `fixtures/index-variants/` (all five, `index-stale` included) with
     both validators.
   - Both new variants are derivable, so each got a golden test:
     - `plugins_degraded_equals_the_variant` uses a `cursors.json` with
       `plugins` `ok: false` and the plugins collector's own timeout
       message, `omarchy plugin list --json: timed out`
       (`collectors/plugins.rs`, `{WHAT}: timed out`).
     - `omarchy_git_checkout_equals_the_variant`: `repoHead` comes from
       the dossier fence `omarchy.summary`, which the index already read.
       The test adds `- repoHead: 3f9c2e1` to the copy's
       `system/omarchy.md`. No new environment variable was needed.
2. **Warnings no longer vanish.**
   - `rebuild_if_initialised` prints every load warning to stderr
     (`seldon: warning: …`), like the failure path.
   - An unreadable `memory/*.md`, and also an unreadable `system/*.md`,
     now warns `…: cannot read: …; skipped`, like the other readers.
3. **`drift_weeks`** "opened" also excludes `kind == resolution`, as the
   script does. A resolution's source is `seldon`, so this was unreachable
   before, but it is now explicit.
4. **Snapshot cap test.** `at_most_ten_snapshots_newest_first` adds 15
   snapshots and checks:
   - 10 are kept, newest first;
   - the newest added one is in and the oldest is out;
   - the timeline shows the same 10.
5. **`[profile.bench]`** sets `lto = "thin"` and `codegen-units = 16`. The
   bench build drops from about 58 s to 28 s here, and the timing is
   unchanged (×10 median 4.7 ms). The CI step stays.
6. **Fast rebuild for the hook (for WP-009).**
   - `index::rebuild_if_initialised_fast(ctx)` is the same rebuild but
     spawns no `git`. `logbook.git.head` is read from the `.git` files
     (`index::git_head_fast`: `HEAD`, a loose ref or `packed-refs`; it
     follows a `.git` file and `commondir`), and `dirty` is left out.
   - The schema allows that: both `git` members are optional, and the
     plugin reads neither. `GitInfo.dirty` is now `Option<bool>`; the full
     path still writes it, and the next full rebuild restores it.
   - Test: `the_fast_rebuild_reads_head_without_git` compares against
     `git rev-parse --short=7 HEAD` for a loose ref, after `pack-refs`, and
     for a detached HEAD. It then runs the fast rebuild and checks
     `logbook.git == {"head": …}`.
   - **What WP-009 should know about the 5 ms budget:**
     - Use `crate::index::rebuild_if_initialised_fast(ctx);`, one line,
       after the hook's ledger write. Call it only when the hook actually
       wrote an event (non-mutating commands write nothing, so they need
       no rebuild).
     - The rebuild still reads the whole logbook, so its cost grows
       linearly. A whole `seldon index` process on the fixture (71 lines)
       takes 2.4 ms in release; the in-process build is 4.7 ms at ×10
       (710 lines) and 79 ms at ×150.
     - So the hook stays under 5 ms only up to roughly 500–700 ledger
       lines. Beyond that WP-009 needs a deferred rebuild (e.g. leave it to
       the next `capture`/`status`, which the plugin runs every 15 min) or
       an incremental index. That is a design decision for WP-009 and the
       orchestrator; I did not build either.
7. **ADR-0020 (drift cap) is implemented**, a few lines in
   `build.rs::cap_drift`. ADR-0020 is not on `main` yet, so I read the
   orchestrator's sentence like this:
   - at most 200 items in `index.drift`;
   - crises are kept before any other item (all of them, up to 200), then
     the newest other items fill the rest;
   - the kept items stay **newest first**, the schema's order;
   - `summary.openDrift` and `summary.crisis` still count **every** open
     item, so the bar pill shows the true numbers;
   - `proposedEvents` and the timeline's crises come from the capped list,
     the rows the plugin can show.

   Test `drift_is_capped_at_200_crises_first` adds 30 old crises and 220
   newer yellow items: 200 rows, every crisis kept, the oldest yellow items
   cut, newest first, the summary counts all of them, and the result is
   schema-valid. With the cap, the ×150 index is 765 KB, down from 897 KB.
   If ADR-0020 means something else (for example crises first in the
   output order), the change is in `cap_drift` only.

## Done

- **`engine/src/index/`**
  - `load.rs` reads the logbook once:
    - every `ledger/*.jsonl`;
    - case files (with their `## Plan` text and steps);
    - the journal of today and yesterday;
    - `decisions/*.md`;
    - the `seldon:begin/end` fences of `system/*.md`;
    - `memory/*.md`;
    - `areas/*/`.

    A broken file (invalid case frontmatter, torn ledger line) is skipped
    with a warning (`warnings` in `--json`) instead of failing the index.
  - `build.rs` derives the index per ADR-0012 §6–§15, ADR-0013 §1–§4 and
    ADR-0015, mirroring `derive()` of `scripts/validate-fixtures.py`:
    - resolution folding (with `resolutionDetail` and the linked `case`);
    - drift items: grouping by `txId`, the leader rule, the computed zone,
      and `proposedCase` taken from the leader;
    - case groups with `path`, `steps` and `proposedEvents`;
    - summary, today, decisions and system (fences, snapshots, areas);
    - memory;
    - series: heatmap, packages, drift weeks counted per item and per
      write, risk, timeline;
    - the caps: 500 events, 50 completed, 366 days, 10 snapshots.
  - `drift.rs`:
    - the token rule of ADR-0015 §4 (a scan with neighbour checks; `regex`
      has no look-around);
    - `[drift] alwaysRed` as fnmatch globs (`*`, `?`, `[…]`, `[!…]`);
    - the routine class. It uses the shared command parser
      (`collectors::pacman::parse_command`, SPEC-ENGINE §4) and also
      requires the program to be `pacman`.
  - `model.rs` holds the typed index structs, in the fixture's key order;
    every schema object stays closed.
  - `views.rs` renders `ledger/YYYY-MM.md` (SPEC-LOGBOOK §5) in exactly the
    format of `fixtures/logbook/ledger/*.md`, and `STATUS.md`:
    - first line `<!-- generated by seldon; do not edit -->`;
    - a `<!-- seldon:begin status -->` fence; text outside it is kept;
    - headings in English, prose in the logbook language (en/de);
    - a file is written only when its text changed.
  - `check.rs` is the schema check for `--check`. `jsonschema` is a test-only
    crate, so the engine carries the Draft 2020-12 subset the contract uses
    (the same as the script's `builtin` backend), with the schemas compiled
    in via `include_str!`. An unknown keyword is an error. The tests hold it
    to `jsonschema` on the sample, both variants and `fixtures/invalid/`.
  - `mod.rs`:
    - `derive`/`derive_at`;
    - `write`: atomic, temp file + rename through `sys::write_atomic`;
      compact JSON plus a newline;
    - `collector_state`: `state` from `cursors.json` when it is bound to
      this logbook. Rows are in the schema's collector order, with
      `enabled` from `config.toml`; `lastCapture` is the latest `lastRun`;
    - `git_info`: `head`/`dirty`, only for a repository;
    - `not_initialised` (the variant's shape);
    - `rebuild_if_initialised(ctx)`.
- **`seldon index [--check]`** (`commands/index.rs`):
  - takes the lock, writes the ledger views and the index;
  - `--check` validates first and refuses to write an invalid index
    (exit 2);
  - before `init` it writes the `notInitialised` index and exits 3;
  - human output plus `--json` (`ok`, `logbook`, `index`, `generatedAt`,
    `events`, `summary`, `files`, `warnings`, `valid`).
- **`seldon status`** (`commands/status.rs`):
  - regenerates the views, `STATUS.md` and the index;
  - autocommits as `seldon: status` only when a logbook file changed;
  - prints an English summary (cases, drift, events, degraded collectors);
  - `--json` adds `status`, `state` and `git`;
  - exits 3 (and writes the `notInitialised` index) before `init`, which
    is what the plugin's `runnerDone` expects.
- **CONTRACT rule 2**: `crate::index::rebuild_if_initialised(ctx);` is one
  line each in `capture`, `log`, `event`, `plan new`, the plan steps and
  `decide`.
  - It runs after the autocommit and before the lock is released, so
    `logbook.git.dirty` is false after a command.
  - A failed rebuild is a warning on stderr. It never fails the command,
    because the command's write already happened.
- **Bench**: `engine/benches/index.rs` (`harness = false`, std timing) and
  `just bench`. The ×10 logbook is built by `tests/common/scale.rs`:
  fresh ids, cases `C-2026-kNNN`, separate transactions and decisions.
  The bench fails if the ×10 median is over 100 ms. It also reports ×150,
  which is about 10 000 events.
- **Tests**: 17 integration tests plus 7 unit tests, all through
  `common::Env`, temp dirs, or the read-only fixture loaded in-process.
  - `tests/index.rs` (12):
    - golden test;
    - ledger views byte-identical to the fixture;
    - the `snapper-degraded` and `not-initialised` variants;
    - the engine checker agrees with `jsonschema`;
    - all 18 ADR-0013 mutation self-checks of the script, plus "missing
      `explicit` errs red";
    - fan-out resolution counted once;
    - the three ADR-0015 §4 token cases end to end;
    - the caps (500 events, 50 completed, 366 days);
    - atomic write against a concurrent reader, in-process and through
      the CLI;
    - the ×10 timing (asserted in release builds).
  - `tests/status.rs` (5):
    - STATUS.md: header, fence, user text above and below kept, template
      replaced, no rewrite or commit when nothing changed;
    - the exact German STATUS.md of the fixture logbook;
    - a degraded collector in STATUS.md and in `state`;
    - every writing command rebuilds the index (with a clean `git`);
    - a broken case file is skipped with a warning.

## Golden test result

`seldon index` on a copy of `fixtures/logbook/` with
`SELDON_NOW=2026-10-01T17:05:12+02:00` gives `fixtures/index.sample.json`
**with zero differences**, modulo `generatedAt`, `engineVersion` and
`logbook.git` (as the WP says), plus `logbook.path` (see Decisions 1).
`state` comes from a `cursors.json` that the test writes: all six collectors
ok, `lastRun` 17:05:00, bound to the copy.

- `ledger/2026-09.md` and `ledger/2026-10.md` are byte-identical to the
  fixture's.
- With snapper degraded in `cursors.json`, the output equals
  `index-variants/snapper-degraded.json`.
- Before `init`, the output equals `index-variants/not-initialised.json`.

No fixture edit was needed and none was made.

## Not done

- **CONTRACT rule 5 (size budget)**: no `meta.truncated`. The ADR-0020
  drift cap is in (follow-up 7); open cases stay uncapped, as the schema
  has them.
- **`DECISIONS.md`**: its `decisions.index` fence is not regenerated. It is
  not in this WP; `decide` does not do it either (WP-006 note).
- **`seldon open journal --editor`** creates the day file but does not
  rebuild the index. It is not a ledger write; the next command or
  `status` picks it up.
- **`seldon hook`** (WP-009) is not wired; it is their module. The fast
  path and its limits are in follow-up 6.
- **`just fixtures-refresh`** is still the stub. It would write
  `fixtures/`, which belongs to WP-015.
- **`drift show`** belongs to WP-008, as the orchestrator notes say.

## Verified by

```
$ just check                                  → exit 0   (after the rebase and the follow-ups)
  fmt-check ok · clippy -D warnings ok · test: 220 passed (17 suites)
  validate-fixtures: ok — 101 instances, 71 ledger events traced …, 5 variants, 22 self-checks
  plugin-validate: ok · qmllint: ok · plugin-test: ok · check: ok
$ just bench                                  → exit 0   (thin-LTO bench profile, build 28 s)
  index build ×10  (710 ledger lines, 80 cases, 211177 bytes): median 4.7 ms
  index build ×150 (10650 ledger lines, 1200 cases, 765454 bytes): median 79.1 ms
$ release `seldon index` on a fixture copy, whole process, 30 runs → median 2.4 ms
$ loop: 15 × cargo test --test index --test status --test commands --test plan → 0 failures
  (before the follow-ups: 25 × index/status/commands/log/plan → 0 failures)
```

- **Negative control for the atomic-write test.** I replaced `index::write`
  with `std::fs::write` and ran the tests three times:
  - the in-process reader test failed all three times ("partial index …
    0 bytes");
  - the CLI reader test failed once;
  - I reverted with `git checkout`.
- **Manual run.** With HOME pointing into the scratchpad:
  - `seldon --logbook <copy> index --check`, then `status`;
  - output diffed against the sample with the script's own `diff()`:
    0 differences;
  - STATUS.md as in `tests/status.rs`.
- **Real XDG dirs.** Nothing touched the real `~/.config` or
  `~/.local/state`; every run used a temp or scratchpad HOME.

## Learned (in memory/)

- `rust-notes.md`:
  - typed index structs and a hand-written `Serialize` for key order;
  - `Option<Option<_>>` for absent, null or a value;
  - the load → build → write pipeline, testable in-process;
  - rebuild after the autocommit;
  - write a view only when it changed;
  - `jsonschema::Retrieve` for cross-file `$ref`s;
  - the built-in checker;
  - benches with `harness = false` and a `#[path]` include;
  - the token rule without look-around;
  - two clippy 1.98 lints;
  - `[profile.bench]` with thin LTO;
  - reading `HEAD` without git.
- `pitfalls.md`:
  - fixture ULIDs do not sort by time within one second (stable view sort);
  - what the golden test needs beyond `SELDON_NOW`;
  - a "now" timestamp in STATUS.md means a commit per plugin cycle;
  - the fixture STATUS.md is stale;
  - index size versus the 1 MB budget;
  - `scripts/__pycache__` left behind by importing the reference script;
  - an atomic-write test needs a negative control;
  - fixture counts hard-coded in tests break on fixture updates; run the
    whole suite after a rebase.

## Decisions needed

Settled at the review (no action): `logbook.path` normalised; `fix` stays
out of the index; the fixture STATUS.md goes to the schema track; the
STATUS.md layout, stamp and commit rules; compact JSON; the size budget
(ADR-0020); the CI bench step.

Open:

1. **My reading of ADR-0020** (follow-up 7; the ADR is not on `main` yet):
   - crises are kept before other items;
   - the output stays newest first;
   - `summary` counts every item;
   - proposals and the timeline come from the capped list.

   Confirm, or correct `cap_drift`.
2. **The hook's rebuild cost grows with the logbook** (follow-up 6). Past
   roughly 500–700 ledger lines even the fast rebuild exceeds the hook's
   5 ms. WP-009 and the orchestrator decide: a rebuild on each hook write
   (fine for small logbooks), a deferred rebuild left to the next
   `capture`/`status`, or an incremental index later.
3. **Merge notes for WP-009** (unchanged):
   - `index/drift.rs` imports
     `collectors::pacman::{parse_command, split_logged}`; if the parser
     moves to `pkgcmd.rs`, that one import changes.
   - `main.rs` and `commands/mod.rs` conflicts are additive.
   - ADR-0019's green `agent` zone needs nothing here: green events are
     not drift-eligible, and zones are copied from the ledger.
4. **Spec edits for the orchestrator** (unchanged, `docs/` is yours):
   - SPEC-ENGINE §3: `capture` now rebuilds the index;
   - SPEC-ENGINE §3: the `index --json` / `status --json` shapes;
   - SPEC-ENGINE §6:
     - the `notInitialised` index and exit 3;
     - `--check` refuses to write an invalid index (exit 2);
     - the views are written by `index` and `status`;
     - the rebuild runs after the autocommit;
     - plus `rebuild_if_initialised_fast` (no `git`, no `dirty`).

For the record, unchanged: the engine and the reference script differ only
where the script fails or a format is lenient (plan steps with `*`/`+`
bullets, Plan section ends outside code fences, invalid journal actors and
broken files skipped with a warning, language/machine from `logbook.toml`).
None of these affects the fixture.

## Touched outside WP scope

- One line each in `commands/{capture,log,event,decide}.rs`, and two in
  `commands/plan.rs` (`new` and steps): the
  `index::rebuild_if_initialised(ctx)` call.
- `main.rs`: the `Index` and `Status` variants with one arm each.
  `commands/mod.rs`: two `mod` lines. `lib.rs`: `pub mod index;`.
- `tests/common/mod.rs`: `index_errors`, `assert_valid_index` (jsonschema
  with a retriever) and `pub mod scale;`. `tests/common/scale.rs` is new.
- `engine/Cargo.toml`: the `[[bench]] name = "index", harness = false`
  stanza and `[profile.bench]` (thin LTO, 16 codegen units). No new
  dependency; `Cargo.lock` is unchanged.
- `justfile`: a `bench` recipe. `.github/workflows/ci.yml`: a `just bench`
  step.
- `memory/rust-notes.md` and `memory/pitfalls.md`: appended.
- I did not touch `schema/`, `fixtures/`, `scripts/`, `plugin/` or `docs/`.
