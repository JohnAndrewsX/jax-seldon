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
