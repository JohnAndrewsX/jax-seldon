```
WP-092 HANDOVER
```

Branch `wp/092-hook-budget`, base `f07d5d1` (main with WP-087 merged).
Commits:
- `ca4e744`: alwaysRed globs compile on demand.
- `46f197c`: skipPaths globs compile on demand.
- `ddbdd51`: event lists sized up front.
- `d769224`: `id`/`ts` read without a `String`.
- `bc5a051` + `8fbc134`: fast path for a plain `seldon hook claude-code`.
- `a92f073`: a skipPaths test row.
- `6b8af9d`: SPEC-ENGINE, CHANGELOG and a doc comment.
- `ec383e9`: pitfalls.
- This file.

## Result in one line

The WP-084/087 case (curl line with a marker, 900 ledger lines) went from
**4.52–4.63 ms to 3.75–3.94 ms** in interleaved A/B (about −0.7 ms). That
leaves **about 1.1 ms of headroom** under 5 ms, where WP-087 had 0.4–0.5 ms.
**The 2.5 ms target is not reached, and the rebuild work alone cannot reach
it:** the same hook call without the rebuild (10 000-line case) takes
2.2 ms by itself. See "Decisions needed".

## Done

### Profile first (before any change)

Baseline, single run of `fast_enough_just_below_the_rebuild_threshold` on
`main`'s bench build, 16:07, load 7.5, 900 lines:

| Case | median |
|---|---|
| not recorded | 1.38 ms |
| recorded (fixture line) | 3.83 ms |
| recorded curl line with a marker | 4.86 ms |

Breakdown of one hook process (throwaway probes: an `eprintln!` of the
elapsed time behind an env var; the curl line at 900 lines; medians of 41
processes; 16:08–16:11, load 7–13; never committed):

| Step | before | after (16:47, load 0.1) |
|---|---|---|
| clap parse of the command line | 196 µs | 176 µs ¹ |
| `setup` (`SkipPaths::new` compiles 5 globs) | 460 µs | 14 µs |
| classify | 92 µs | 32 µs |
| redaction of the curl line (redact.rs, not mine) | 747 µs | 819 µs |
| case: prepare + save | 183 + 191 µs | 173 + 190 µs |
| **rebuild:** line count / config + open | 96 / 57 µs | 85 / 60 µs |
| ledger parse (900 lines) | 724 µs | 603 µs |
| rest of load (case, journal, memory …) | 122 µs | 110 µs |
| fold (clones every event) | 230 µs | 203 µs |
| drift + cases (`AlwaysRed::new` compiles 20 patterns as regexes: 18 literals, 2 globs) | 275 µs | 7 µs |
| series / clip 500 / size check / to_text / write | 64 / 50 / 90 / 134 / 51 µs | 59 / 94 / 85 / 153 / 46 µs |
| freeing the built index | 144 µs | 137 µs |
| **rebuild total** | **≈ 2.1 ms** | **≈ 1.65 ms** |

¹ Measured before the fast path; the fast path removes it (see the clap
mutant below).

The dominant cost was the rebuild, as the WP said. Inside it, the two
biggest single items were not the work itself:
- regex compilation the build mostly did not need (`alwaysRed`);
- allocation in a cold process. An `Event` is 464 bytes, and the
  900-event lists grew by doubling and were copied, so each step mapped
  fresh pages.

Outside the rebuild, `SkipPaths::new` compiled its regexes on every hook
call, the ones that record nothing included.

### Changes (behaviour unchanged)

1. **`AlwaysRed`** (`index/drift.rs`):
   - a literal pattern is compared as a string;
   - a glob compiles on first use, and only for a subject that starts
     with its literal head (the text before the first `*`, `?` or `[`).
2. **`SkipPaths`** (`collectors/config.rs`): the same idea.
   - The regex source is built eagerly and compiled lazily, only for a
     path or name that starts with the anchored literal head.
   - A relative pattern (`(?:^|/)` prefix) has no head and compiles on
     its first use.
3. **Allocations:**
   - `Ledger::read_month` sizes its list by the line count;
   - `index::load` moves the first month instead of copying it;
   - `fold` reserves one slot per event.
4. **`Event` `id` and `ts`** are parsed from the borrowed text through a
   `&str` visitor, with no `String` per field and line.
5. **`seldon hook claude-code`** (exactly that argv, the line `hook
   install` writes) skips `Cli::try_parse_from`. It builds the same `Cli`
   value itself and joins the existing dispatch. A test compares the two
   values via `Debug`, and a new `Cli` field breaks the build there.

No on-disk cache, so review stage 1 only.

## Not done

- **The optional lever** (case-sensitive `curl-user` trigger): not taken,
  as the brief says. redact.rs belongs to WP-093.
- **Smaller rebuild items, left as they are** (each ≤ 0.2 ms, each needs an
  API change wider than this WP):
  - the size check serialises the index a second time (85 µs). `build()`
    returns the warning to about ten callers;
  - `fold` clones 900 events, and `clipped` clones 500 again, because
    `Built` keeps `ledger` and `folded` for other commands;
  - the line pre-count reads the ledger once more (85 µs);
  - freeing the built index (≈ 0.14 ms).
- **The case file:** `prepare` and `save` each run `model::update` (0.17 +
  0.19 ms). That is `logbook/cases.rs` and WP-077's design, so I left it.

## Verified by

**Behaviour**
- Golden: `tests/index.rs` (golden sample, every variant) is green.
- Direct byte comparison: `main`'s and this branch's bench binaries each
  ran `seldon index --json` on a copy of `fixtures/logbook` (fixed
  `SELDON_NOW`, temp HOME/XDG). Result: `index.json` identical (38 594
  bytes), `--json` output identical, logbook views identical, stderr
  identical (empty).
- New tests:
  - `index::drift::tests::always_red_compiles_on_demand`;
  - `collectors::config::tests::skip_paths_compile_on_demand`, plus a row
    for a relative pattern without `**`;
  - `model::event::tests::id_and_ts_read_as_through_a_string`. Values
    and messages equal the `String` path for valid, escaped, wrong-type
    and unparsable input; only the error column differs by one, and
    `read_month` drops the error;
  - `tests::plain_claude_code_hook_is_what_the_parser_reads` (bin).
- `flock /tmp/seldon-check.lock just check`: see "Gates" below.

**Timings: interleaved A/B.** Method as in WP-087: bench builds of `main`
(`f07d5d1`, exported with `git archive` into the scratchpad) and of this
branch were swapped into `target/release/seldon`, and the `hooks` test
binary ran directly, alternating, all under the check lock. Medians of 21
processes each.

Set 1, 16:57–16:59, load 3.3 → 1.6, 4 rounds, main / branch:

| Case | main | branch |
|---|---|---|
| curl line, 900 lines | 4.52, 4.62, 4.58, 4.52 ms | 3.91, 3.75, 3.94, 3.89 ms |
| recorded, 900 lines | 3.58, 3.63, 4.01, 3.62 ms | 2.81, 2.87, 2.86, 2.80 ms |
| not recorded, 900 lines | 1.23, 1.23, 1.21, 1.22 ms | 0.62, 0.62, 0.65, 0.61 ms |
| curl line, 10 000 lines | 2.67, 2.70, 2.66, 2.64 ms | 2.19, 2.24, 2.25, 2.26 ms |
| recorded, 10 000 lines | 1.76, 1.70, 1.79, 1.79 ms | 1.25, 1.25, 1.25, 1.22 ms |

Set 2, 17:17–17:19, load 0.2 → 1.3, 3 rounds, with the perf-only mutants
as their own builds (see below):

| Case | main | branch | alloc group off | clap fast path off |
|---|---|---|---|---|
| curl line, 900 lines | 4.56, 4.63, 4.57 | 3.87, 3.92, 3.77 | 3.82, 3.79, 3.94 | 3.91, 3.94, 3.89 ms |
| recorded, 900 lines | 3.53, 3.60, 3.56 | 2.81, 2.94, 2.84 | 2.97, 2.84, 2.95 | 2.96, 2.94, 2.94 ms |
| not recorded, 900 lines | 1.22, 1.21, 1.23 | 0.63, 0.63, 0.62 | 0.61, 0.64, 0.64 | 0.76, 0.79, 0.77 ms |

Why 2.5 ms is out of reach for the rebuild alone. The branch's curl call
at 10 000 lines, which is the same call without the rebuild, takes
2.19–2.26 ms. The rebuild at 900 lines adds about 1.6 ms. Even a free
rebuild leaves about 2.2 ms plus the line count. The curl line pays about
1 ms more than the fixture line in both sizes; that is redaction (WP-093).

### Mutants

Each was applied alone and run against the targeted suites. The source
was restored with `git checkout` + `touch`, and `git diff` was clean
afterwards. Script in the scratchpad.

| # | Mutant | Result |
|---|---|---|
| M1 | alwaysRed: every pattern compared as a literal | killed: `always_red_globs`, `always_red_compiles_on_demand` |
| M2 | alwaysRed: no literal-head check | killed: `always_red_compiles_on_demand` |
| M3 | alwaysRed: literal compared by `starts_with` | killed: `always_red_globs` |
| M4 | alwaysRed: `[` not ending the head | killed: `always_red_globs` |
| M5 | skipPaths: a head for relative patterns too | killed: `skip_paths_match_paths_names_and_globs` (new row) |
| M6 | skipPaths: no literal-head check | killed: `skip_paths_compile_on_demand` |
| M7 | skipPaths: `?` not ending the head | killed: both skipPaths tests |
| M8 | `read_month` without capacity | **survived** (perf only) |
| M9 | load: every month replaces the list | killed: `golden_index_equals_the_sample` + 9 more in `tests/index.rs` |
| M10 | load: condition inverted | killed: same 10 |
| M11 | `fold` without capacity | **survived** (perf only) |
| M12 | visitor `expecting` text changed | killed: `id_and_ts_read_as_through_a_string` |
| M13 | visitor error text replaced | killed: same |
| M14 | `id` back to the `String` path | **survived** (perf only) |
| M15 | fast path with `json: true` | killed: `plain_claude_code_hook_is_what_the_parser_reads` |
| M16 | fast path takes extra arguments | killed: same |
| M17 | fast path runs `SessionStart` | killed: same + 29 `hooks` tests |
| M18 | fast path off | **survived** (perf only) |

The four survivors change no behaviour by design, so they were timed
instead (set 2):
- **M18** (clap): calls the hook does not record take 0.76–0.79 ms without
  the fast path against 0.62–0.63 ms with it, about −0.15 ms.
  Clearly measured.
- **M8 + M11 + M14 + the load move** ("alloc group off"): **not
  distinguishable from noise end to end** (fixture line 2.84 against
  2.95 ms median; curl line no difference). The probe in a cold process
  showed the ledger parse at 704 → 603 µs. I claim ≤ 0.1 ms for this
  group, no more. Reviewer's call whether to keep `ddbdd51`/`d769224`;
  both drop cleanly.

The wins are therefore, in order:
- `SkipPaths` (≈ 0.45 ms, every hook call);
- `AlwaysRed` (≈ 0.25 ms, every rebuild, also for `status`/`capture`);
- clap (≈ 0.15 ms, every hook call);
- allocations (≤ 0.1 ms).

### Gates

- `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings`:
  clean (also inside `just check`).
- `flock /tmp/seldon-check.lock just check` at `ec383e9`, 17:30, load 0.4:
  **exit 0**. The output ends in `check: ok`: every cargo suite green,
  plus install.test 209, docs-check ok, real-home-guard 11, and the
  plugin views 314/782/319/143, all passed.
- `flock /tmp/seldon-check.lock just check-perf` at `ec383e9`, 17:47,
  load 0.7 → 6.1 during the run: **exit 0**.
  - Index build: ×10 4.07 ms, ×150 78.3 ms.
  - Hooks at 10 000 lines: 0.75 / 1.33 / 2.35 ms (not recorded /
    recorded / curl line).
  - Hooks at 900 lines: 0.70 / 2.95 / **3.86 ms**.
  - `status` at 10 000 lines: 45.2 ms.
  - Redaction rows unchanged against WP-087 (16 KB 0.13 ms, 64 KB
    0.51–0.56 ms; two kinds at 128 KB 7.9 ms, password/token 3.8 ms).

## Learned

Appended to `memory/pitfalls.md` (WP-092 section):
- profile the hook as a process, not as a warm loop (cold-process
  allocation; the probe-patch method);
- no `Regex::new` in a constructor on the hook path;
- clap's cost per process and how the fast path stays honest;
- a serde visitor reports a different error column;
- perf-only mutants are timed, not called covered.

## Decisions needed

1. **Accept ≈ 3.9 ms (curl line, 900 lines, load ≤ 3) as the margin, or
   go for ≤ 2.5 ms.** The remaining levers, largest first:
   - **a. Take the rebuild out of the hook's wall time** (≈ −1.6 ms;
     the only lever that reaches 2.5 ms on its own: 2.2 ms curl, 1.2 ms
     fixture line). After the lock is released, the hook would start a
     detached `seldon` rebuild (stdio to `/dev/null`, own session) and
     return. The index then arrives a few ms later.
     - This changes SPEC-ENGINE §8 ("rebuilds the index … after
       releasing the lock").
     - A rebuild warning would no longer reach the hook's stderr.
     - Each recorded command starts one more process.
     - `index::a_hook_write_rebuilds_the_index` and the perf test's
       index check would have to wait for the file.

     That is a design decision, not an optimisation, so I did not take
     it. Needs the orchestrator, perhaps an ADR.
   - **b. Redaction of the curl line** (≈ 0.8–1 ms over the fixture line,
     compile cost of the curl rules): WP-093's redact.rs. The WP's
     case-sensitive `curl-user` trigger is one part (≈ 0.1 ms, per WP-087).
   - **c. The rest of the rebuild:** the items under "Not done", ≈ 0.5 ms
     together, each needing an API change across commands.
2. **Keep or drop the allocation commits** (`ddbdd51`, `d769224`): correct
   and tested, but no end-to-end gain beyond noise (≤ 0.1 ms).

## Touched outside WP scope

- **`engine/src/collectors/config.rs`** (`SkipPaths`): on the hook path,
  and the largest single win. The config collector uses the same type,
  with matches unchanged.
- **`engine/src/main.rs`** (fast path), `engine/src/model/event.rs`
  (`id`/`ts` deserialisation) and `engine/src/ledger.rs` (capacity).
- `CHANGELOG.md`, `docs/SPEC-ENGINE.md` §1/§8 numbers, `memory/pitfalls.md`.
- Not touched: redact.rs, capture.rs, doctor.rs, scripts/.
- **No guard-hook blocks.** Nothing outside the repository except the
  session scratchpad (probe patch, A/B and mutant scripts, base export,
  bench binaries).
- During the A/B runs `engine/target/release/seldon` was swapped. The
  branch build is back in place (checked with `cmp`).

## Round 2 (review: APPROVE; F1 and F2 before the merge)

Decisions taken by the orchestrator: about 3.9 ms is accepted as the
margin (a detached rebuild would need its own ADR); the allocation
commits stay.

- **F1** (`engine/src/model/event.rs`): the "escaped" row of
  `id_and_ts_read_as_through_a_string` was a copy of row 1. The cause was
  my edit script: Python read `\u0033` in the replacement text as its own
  escape and wrote the plain character, which is how the `\u002b` of the
  last row was lost too. Row 2 now holds
  `"\u00301M1MB2M1GWZYF485HTGVZ1KS3"` and
  `"2026-09-03T21:14:06\u002b02:00"` (JSON escapes for the first `0` and
  the `+`), and the last row `"2026-09-03T21:14:06\u002b0200"` again.
  - Branch: `model::event` 6 passed.
  - Mutant R6 (`visit_borrowed_str` parses, `visit_str` returns an
    error, so every escaped value is refused): `id_and_ts_read_as_through_a_string`
    FAILED, so it is killed now. Source restored, diff only the fix.
- **F2**: `AlwaysRed`'s default list has 20 patterns, 18 literals and 2
  globs (`limine*`, `mkinitcpio*`), all compiled as regexes before; not
  "18 globs". The profile row above and the WP-092 pitfall are corrected
  (the pitfall is this WP's own, not merged yet). The pitfall also said
  `AlwaysRed` compiled "in every hook call"; it compiled in every index
  build.
- Learned: the Edit tool also turns `\u…` in its input into the
  character. Write JSON escapes with a script that doubles the
  backslash, and check the bytes with `cat -A`. Appended to the pitfalls.
- Checks: `cargo fmt --check`, `cargo clippy --all-targets -- -D
  warnings` clean; `cargo test --lib model::event index::drift
  collectors::config` green. No full `just check` (as asked).
