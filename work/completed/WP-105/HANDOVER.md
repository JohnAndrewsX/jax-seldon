# WP-105 HANDOVER

```
WP-105 HANDOVER
Done: a collector's message is redacted once at the source (capture.rs, collect_all) before it is saved in cursors.json, printed (capture text and --json) or embedded in the snapper note; messages an older engine saved in cursors.json are redacted when a capture loads the file; the index build redacts each state.collectors message again (legacy files before the next capture), with the built-in rules when a [redaction] pattern is invalid; 4 new tests (messages::), 10 mutants all killed; SPEC §7 sentence, CHANGELOG (neutral), TESTING row, pitfalls
Not done: doctor (not in the WP's file list) — see Decision 2
Verified by: flock /tmp/seldon-check.lock just check → exit 0 (`check: ok`), run on the final code and docs; cargo test 1594 passed, 0 failed; fmt + clippy --all-targets -D warnings clean; docs-check ok; mutants 10/10 killed
Learned: memory/pitfalls.md, section "WP-105"
Decisions needed: 2, none blocking (below)
Touched outside WP scope: docs/TESTING.md (one row), memory/pitfalls.md
```

Branch `wp/105-redact-messages`, worktree `wt/WP-105`, from `ac2c4ee`
(main, WP-104 merged; main has not moved since). No push, no PR.
Commits (oldest first):

- `2b9d3cc` engine: redact collector messages before cursors.json, capture output and the index (WP-105)
- `0b1a816` engine: tests that a planted token in snapper's stderr is masked everywhere (WP-105)
- `ff15f18` docs: SPEC-ENGINE §7, collector messages are redacted before they are saved or shown (WP-105)
- `9d97b43` memory: pitfalls from WP-105
- this commit: handover

## What changed

`engine/src/commands/capture.rs`:

- `collect_all`: `out.message` goes through `ledger.redactor()` (the
  built-in rules plus the user's `[redaction] patterns`, the same
  redactor the ledger uses) right after the collector returns. The
  `CollectorState` (→ `cursors.json`), the `CollectorReport` (→ `capture`
  text and `--json`) and `access_change` (→ the snapper note) all take
  this one redacted value. That is the WP's "one redaction pass".
- `redact_messages`: right after `Cursors::load`, every entry's message
  is redacted, *before* the WP-099 "as loaded" copy. So an entry an
  older engine saved raw (also of a collector this capture does not run,
  which is otherwise saved unchanged) is masked in the final save and in
  the marked file a crash leaves. For entries this engine wrote, it is
  an idempotent second pass on already redacted text.

`engine/src/index/mod.rs` (`collector_state`): each `state.collectors`
message read from `cursors.json` goes through `Redactor::for_config`;
an invalid pattern falls back to the built-in rules (the capture refuses
to run then anyway, so only an older file can hold a raw message). The
redactor is built only for the first message (most states hold none),
so the hook's index rebuild pays nothing on a host where every
collector is ok. `STATUS.md` (`views.rs`) and `seldon status` read the
index, so they are covered by this too.

No contract change: `message` stays a string. The plugin harness runs in
`just check` unchanged.

The ledger copy is unchanged: the note's detail is built from the
redacted message and redacted again by `Ledger::append`, which gives the
same text (the WP-099 test `the_note_is_redacted_like_every_event`
passes unchanged, and the new test pins the note again).

## Tests (`engine/tests/idempotency.rs`, module `messages::`)

The stub snapper prints `token=fake0123456789 at fake-host-alpha` on
stderr (exit 3); config has `[redaction] patterns = ["fake-host-[a-z]+"]`,
so a built-in rule and the user's pattern both have to fire. Expected
message everywhere: `snapper failed (exit 3): token=‹redacted› at ‹redacted›`.

- `the_message_is_masked_where_it_is_saved_and_shown`: after the
  capture that degrades: `capture --json` (row and whole output),
  `cursors.json` (whole file), `index.json` (whole file and the row); the
  ledger note is the expected text. A second capture in text mode writes
  0, prints the masked message, and leaves all of them the same
  (idempotent). `seldon status` writes `STATUS.md` with the masked message
  and no raw part.
- `a_message_an_older_engine_saved_is_masked`: the raw message planted
  into `cursors.json` (as 0.1.4 saved it); `seldon index` masks it before
  any capture; `capture --source pacman` (snapper not run) saves it
  masked; no note.
- `an_invalid_pattern_leaves_the_built_in_rules_to_the_index`: planted
  raw message, pattern `(`: `seldon index` exits 0 and shows
  `token=‹redacted› at fake-host-alpha`.
- `the_file_a_crash_leaves_holds_it_masked` (debug builds, WP-099 crash
  point): planted raw degraded message, snapper ok again → the note marks
  the file, crash after the append; the marked `cursors.json` holds the
  masked message, the note embeds it.

## Mutants

Script in my scratchpad (`wp105-private/mutants.py`): each mutant
applied to the committed tree, `cargo test --no-fail-fast --test
idempotency --test doctor --test index --test collectors --lib`,
restored with `git checkout HEAD --` plus `touch`.

| # | Mutant | Result | Killed by |
|---|---|---|---|
| C1 | no redaction at the source | killed | `the_message_is_masked…` |
| C2 | source: built-in rules only | killed | `the_message_is_masked…` |
| C3 | no redaction of loaded messages | killed | `a_message_an_older…`, `the_file_a_crash…` |
| C4 | loaded messages: built-in rules only | killed | same two |
| C5 | loaded messages redacted after the "as loaded" copy | killed | `the_file_a_crash…` |
| C6 | report keeps the raw message | killed | `the_message_is_masked…` |
| C7 | saved state keeps the raw message | killed | `the_message_is_masked…` |
| I1 | index: no redaction | killed | `a_message_an_older…`, `an_invalid_pattern…` |
| I2 | index: built-in rules only | killed | `a_message_an_older…` |
| I3 | index: invalid pattern masks the whole message | killed | `an_invalid_pattern…` |

C1 shows why the WP needed new tests: the WP-099 note test alone does
not kill it (the ledger redacts the note anyway).

## Decisions needed (none blocking)

1. **Two passes, not literally one.** The message is redacted once at
   the source; the load-time pass in capture and the index pass exist
   only for what an older engine saved (and are idempotent on what this
   one saved). Without them, a disabled collector's old raw message would
   stay in `cursors.json` and the index indefinitely. Please confirm.
2. **doctor is not covered and was not in my file list.**
   - doctor's `collectors` row reads `cursors.json` directly: after the
     first capture with this engine it shows the masked message; before
     it (an upgrade, no capture yet) it shows the raw one an older engine
     saved.
   - doctor's own snapper probe (`check_snapper`, also in `init`'s output
     and `--json`) prints `snapper failed: <describe(run)>`, which can
     hold snapper's stderr, unredacted. It is written nowhere, only
     printed to the user's own terminal or the plugin's caller.
   A follow-up could run both through `Redactor::for_config`; tell me if
   you want it folded in here instead.

## `just check`

`flock /tmp/seldon-check.lock just check`: exit 0, ending with
`check: ok`, run after the last mutant on the final code and docs (the
pitfalls commit landed during the run; it touches no checked file).

| Suite | Passed | Failed |
|---|---|---|
| cargo test (engine, 70 binaries) | 1594 | 0 |
| bar-view | 143 | 0 |
| panel-view | 782 | 0 |
| overlay-view | 319 | 0 |
| service-states | 314 | 0 |
| install.test | 209 | 0 |
| deploy-test-host.test | 190 | 0 |
| real-home-guard | 11 | 0 |
| model.test.js | 89 | — |

The plugin harness passes unchanged (no plugin change).

## Notes

- No guard-hook block happened.
- The host was not touched: every run used the tests' scratch HOME and
  stubs; secrets are made up (`fake0123456789`, `fake-host-alpha`).
- `redact.rs` and `collectors/config.rs` were not touched.
- Cost: on a host with a degraded collector, each index build (also the
  hook's) builds a `Redactor` once (compiles the user's patterns, if any)
  and runs it over that short message; the built-in rules compile only
  when the message holds their trigger.

---

# Round 2 (stage 1 SEND BACK small: B1, N1, N2, N3)

Commits (oldest first), no rebase:

- `17ea1e3` engine: doctor and init show collector messages and probe output redacted; withheld with an invalid pattern; no growth on load (WP-105)
- `46cf160` engine: tests for doctor, init, a pattern across the marker and an invalid pattern (WP-105)
- `5f2b1d0` docs: SPEC-ENGINE §7, doctor and init redact, no growth on load, withheld with an invalid pattern (WP-105)
- `a900bb1` memory: pitfalls from WP-105 round 2
- this commit: handover round 2

## What changed

- **One helper for what is shown:** `collectors::ShownMessages`
  (collectors/mod.rs). It wraps `Redactor::for_config`, built on the
  first message only. When the redactor cannot be built (an invalid
  pattern) or `config.toml` cannot be parsed (`ShownMessages::new(None)`),
  every message becomes the constant `collectors::MESSAGE_WITHHELD` =
  `‹redacted› (withheld: config.toml or one of its [redaction] patterns
  cannot be used)`. The index (`collector_state`) and doctor both use it.
- **B1, doctor and init.**
  - The `collectors` row redacts each message read from `cursors.json`.
  - The `snapper` probe (`check_snapper`, whose row `init` also prints,
    as text and in `--json`) redacts `describe(run)`.
  - The `omarchy` probe (`check_omarchy`) does the same; it was found in
    the sweep below.
  - `check_snapper` takes `&ShownMessages` now. `init` passes its own
    config.
  - doctor passes `None` while `config.toml` is invalid or unreadable.
    The patterns are unknown then, so the probe rows show the withheld
    text.
- **N1, growth.** The capture's load pass (`redact_messages`) skips a
  message that already holds `‹redacted›`. A message from an older
  engine holds none, unless the program itself printed the marker. In
  that case the file keeps it, and every display still redacts it.
  The index and doctor redact from the stored text each time, so they
  cannot accumulate; a comment says so (`ShownMessages`,
  `collector_state`).
  - Probe with patterns `fake-host-[a-z]+` and `›.`: stored
    `snapper failed (exit 3): token=‹redacted‹redacted›at ‹redacted›`,
    the same bytes after 3 more captures.
  - The index shows one pass more, the same every time:
    `…token=‹redacted‹redacted‹redacted›t ‹redacted›`.
  - I kept "the display always redacts", even though the index then
    differs from `cursors.json` for such a pattern. The display never
    trusts the file.
- **N2, fail closed.** With an invalid pattern, `seldon index` exits 0.
  The row shows `MESSAGE_WITHHELD`, and so do doctor's `collectors` row
  and its `snapper` probe. The capture still refuses with exit 1. SPEC
  §7 says this in the WP-105 sentence and at the general "invalid user
  pattern" sentence.
- **N3:** below, under "Checks".

## Sweep: places that print or save a collector message or program output

`grep` for `.message`, `stderr`, `first_line` and `describe(` in
`engine/src`:

| Place | What | Status |
|---|---|---|
| capture.rs `collect_all` | collector message → state, report, snapper note | redacted at the source (round 1) |
| capture.rs `render` (text, `--json`) | report messages | from the redacted report |
| capture.rs `redact_messages` | messages loaded from `cursors.json` | redacted on load, marker skip (N1) |
| hook.rs session end, init.rs first capture | through `capture::run` | covered |
| index/mod.rs `collector_state` → views.rs `STATUS.md`, status.rs | `state.collectors.message` | `ShownMessages` |
| doctor.rs `check_collectors` | messages from `cursors.json` | `ShownMessages` (B1) |
| doctor.rs `check_snapper` → init.rs | snapper's first stderr line | `ShownMessages` (B1) |
| doctor.rs `check_omarchy` | `omarchy-version`'s first stderr line | `ShownMessages` (found in the sweep) |
| snapper.rs:218 `first_line` | builds the collector message | goes through the capture's pass |
| plugins.rs, omarchy.rs, theme.rs, pacman.rs, config.rs `degraded(…)` | collector messages | through the capture's pass |
| open.rs:190 (editor launcher), setup.rs:554 (`omarchy hook install`), logbook/git.rs:115/351 (git), dossier/query.rs | stderr of programs that are not collectors | not touched, out of scope; printed to the user only, not saved |
| `fix` strings | constants in the collectors | nothing to redact |

## Tests (`messages::`, now 8)

New in this round:

- `doctor_masks_the_saved_message`: raw message planted (as an older
  engine saved it), user pattern. doctor exits 0. The `collectors` row
  reads `last capture failed: snapper: <masked>` as text and in
  `--json`; neither output holds a raw part.
- `the_snapper_probe_masks_what_snapper_printed`: the stub snapper
  prints the token on stderr. doctor's `snapper` row is
  `snapper failed: exit 3: token=‹redacted› at ‹redacted›` (text and
  `--json`). `init` on a second path prints `Snapper: degraded — <same>`,
  and `--json init` has the same `snapper.message`; no raw part.
- `the_omarchy_probe_masks_what_it_printed`: the same for the stub
  `omarchy-version`.
- `a_pattern_across_the_marker_does_not_grow_the_message` (N1):
  patterns `fake-host-[a-z]+` and `›.`. The saved message holds the
  marker. Three `capture --source pacman` runs leave it byte-identical,
  and the index row stays the same.
- `an_invalid_pattern_withholds_the_message` (N2): replaces round 1's
  `an_invalid_pattern_leaves_the_built_in_rules_to_the_index`.
  - Pattern `(`: `seldon index` exits 0; the index row, doctor's
    `collectors` row and its `snapper` row show `MESSAGE_WITHHELD`.
  - Then a `config.toml` that does not parse: doctor exits 1 and the
    `snapper` row is withheld.

## Mutants (round 2)

Same method as round 1 (`wp105-private/mutants2.py`).

| # | Mutant | Result | Killed by |
|---|---|---|---|
| R1 | load pass without the marker skip | killed | `a_pattern_across_the_marker…` |
| R2 | invalid pattern → built-in rules (the round-1 behaviour) | killed | `an_invalid_pattern_withholds…` |
| R3 | unparsable config → default patterns in doctor | killed | `an_invalid_pattern_withholds…` |
| R4 | doctor `collectors` row unredacted | killed | `doctor_masks…`, `an_invalid_pattern…` |
| R5 | omarchy probe unredacted | killed | `the_omarchy_probe…` |
| R6 | snapper probe unredacted | killed | `the_snapper_probe…`, `an_invalid_pattern…` |
| R7 | init: the default config's redactor (no user patterns) | killed | `the_snapper_probe…` |
| R8 | index: always withheld | killed | 2 `messages::` + 2 index variant tests |
| R9 | index: no redaction | killed | `a_message_an_older…`, `an_invalid_pattern…` |

## Checks

All under `flock /tmp/seldon-check.lock`, with the worktree's default
`target/`:

- **`just check-rss`:** exit 0
  (`rss_stays_under_10_mb_on_the_x10_fixture` ok).
- **`just check-perf`, first run:** exit 101. One case was over budget:
  `fast_enough_just_below_the_rebuild_threshold`, "recorded curl line
  with -e and -am, 900 lines", medians 5.11 and 5.26 ms against 5 ms.
  The load average was 4–7 at the time, from parallel workers.
- **A/B against the base:** I built two bench binaries of `seldon`, base
  `ac2c4ee` and HEAD, swapped each into `target/release/seldon`, and ran
  the same hooks test binary under the lock, alternating 4 rounds each.
  - Base: medians 4.68 / 4.82 / 4.82 / 4.68 ms.
  - HEAD: 4.70 / 4.72 / 4.73 / 4.75 ms.
  - The other three cases also match within noise.
  - So this change costs nothing measurable here. That case already has
    only about 5 % headroom on main, so it can fail under load whatever
    the change.
- **`just check-perf`, second run:** exit 0. Key medians:
  - index build ×10: 3.97 ms; ×150: 80.3 ms (budget 100 ms)
  - `status` at 10 000 lines: 44.5 ms (budget 100 ms)
  - hook at 10 000 lines: 0.74 / 1.32 / 2.68 / 3.17 ms
  - hook at 900 lines: 0.69 / 2.84 / 4.15 / 4.77 ms (budget 5 ms each)
  - redaction of long lines: all within budget (128 KB, two option
    kinds: 8.9 ms of 20 ms)
- **`flock /tmp/seldon-check.lock just check`:** exit 0, ending with
  `check: ok`, on `5f2b1d0` (code, tests and docs of this round; the
  pitfalls commit came after it).

| Suite | Passed | Failed |
|---|---|---|
| cargo test (engine, 70 binaries) | 1602 | 0 |
| bar-view | 143 | 0 |
| panel-view | 782 | 0 |
| overlay-view | 319 | 0 |
| service-states | 314 | 0 |
| install.test | 209 | 0 |
| deploy-test-host.test | 190 | 0 |
| real-home-guard | 11 | 0 |
| model.test.js | 89 | — |

The plugin harness passes unchanged.

Also: `cargo clippy --all-targets -- -D warnings` and `cargo fmt --check`
are clean, and `bash scripts/docs-check.sh` is ok.

## Notes

- **Merge with current main:** `git merge-tree main HEAD` gives one
  conflict, in `memory/pitfalls.md`. Both sides appended a section at the
  end (main: WP-106/WP-107). Resolve by keeping both. `CHANGELOG.md`,
  `docs/SPEC-ENGINE.md` and `engine/tests/idempotency.rs` merge cleanly.
- No guard-hook block. The host was not touched; the secrets are made up.
- `redact.rs` and `collectors/config.rs` were not touched. The default
  `target/` of the worktree was used (no `CARGO_TARGET_DIR` under /tmp).
