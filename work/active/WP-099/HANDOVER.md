```
WP-099 HANDOVER
Done: a crash between the ledger append and the cursors.json save no longer repeats the state-reset note or the snapper access note; mark saved before the append (`pendingNotes`), dedup on the next capture by note identity (snapper: subject; state reset: per source in meta.sources); 5 new tests (4 crash, 1 redaction); SPEC §2 §3 §4 §7, CHANGELOG, TESTING, pitfalls
Not done: the WP's ledger-only time bound ("dated at or after the old lastRun") — replaced, reason below (Decision 1); the §7 sentence is not the WP's wording (Decision 2)
Verified by: flock /tmp/seldon-check.lock just check → exit 0 (`check: ok`), run once after the last mutant; cargo test --no-fail-fast 0 failures; fmt + clippy --all-targets -D warnings clean; docs-check ok; 18 mutants: 16 killed, 2 equivalent (below)
Learned: memory/pitfalls.md, section "WP-099"
Decisions needed: 3, none blocking (below)
Touched outside WP scope: engine/src/collectors/mod.rs (one field on `Cursors`, 8 lines), docs/SPEC-ENGINE.md §2 (one clause), CHANGELOG.md, docs/TESTING.md, memory/pitfalls.md
```

Branch `wp/099-crash-proof-notes`, worktree `wt/WP-099`, from `1b23cf4`.
No push, no PR. Commits (oldest first):

- `790845e` engine: a crash before the cursor save does not repeat seldon notes (WP-099)
- `cb416be` engine: test that the snapper note is redacted like every event (WP-099)
- `18839c5` docs: SPEC-ENGINE crash-proof seldon notes, pendingNotes, notes and redaction (WP-099)
- `578a7b8` docs: changelog and testing rows for crash-proof notes (WP-099)
- `00aecdf` memory: pitfalls from WP-099
- `4825c60` engine: a hand-written note at the marked time is not Seldon's (WP-099)
- this commit: handover

## Design

### Why not the WP's "dated at or after the old `lastRun`"

A rule that looks only at the ledger and the old `lastRun` fails in the
most common reset: **a lost state directory** (binding none, no
`cursors.json`). After the crash there is no file, so there is no old
`lastRun` either. A "genuine" second loss (the capture completed and
saved, and later the directory is gone again) looks exactly the same:
the ledger holds the earlier note, and there is no file. Without a bound,
every later reset of the same sources is silenced. With no dedup, the
crash repeats the note. The ledger alone cannot tell the two apart.

There is a second problem. "At or after" also matches the *genuine* note
of the last completed capture: that note's `ts` equals the `lastRun` it
saved. So a real second reset (or, for snapper, the next change in the
same direction after a clock step back) would be swallowed. Strictly
"after" fixes that but repeats the note when the crash falls in the same
second as the previous save.

### What it does instead (`capture.rs`)

Only a capture that appends a `seldon` note does this:

1. Before the append it saves `cursors.json` **as it loaded it**. That
   means the same binding and entries, or, with no file, no logbook and
   no entries. `pendingNotes` gets the note's time (= the capture time,
   whole seconds) added to the times already there.
2. It appends (collector events + notes), then saves the new state with
   `pendingNotes` cleared (as today).

A crash between 1 and 2 leaves the old state plus the mark. The next
capture loads it and therefore makes the same comparison: same binding,
same losses, same snapper `ok`. `pending_notes` reads the ledger months
of the marked times and keeps the `seldon` `note`s at those times.
Then:

- **snapper note:** skipped when a recorded note has subject `snapper`.
  The old state is the same across the whole crash chain, so a change
  seen again has the same direction as the crashed one.
- **state reset:** each lost source that a recorded `state-reset`
  note's `meta.sources` names is left out of the note. The note is
  written only for the rest. Example: the crashed capture ran
  `--source pacman`, the next runs `--all`; the new note names
  `snapper` only. The warning still names every lost source, because the
  crash hid it.
- A capture that writes a note of its own while an earlier mark is
  pending keeps that mark too (two crashes in a row).

A crash *before* the append (after 1) leaves the mark but no note. The
next capture finds nothing at the marked time and writes the note. The
mark is in the same atomic file as the binding, so it describes exactly
the state the next capture compares against. No time heuristics, any
binding.

Not changed: the ledger (ids are still assigned in `append`; an event
has no id before it, so the key is the time), doctor, the index, the
schema. `pendingNotes` is an internal state field. It is written only
while set, and an older engine ignores it (`Cursors` has no
`deny_unknown_fields`) and drops it on save, which only brings back the
old duplicate after a downgrade.

Test hook: `SELDON_TEST_CAPTURE_CRASH=before-append|after-append` ends
the process with exit 99 at that point, in debug builds only
(`#[cfg(debug_assertions)]`, the `SELDON_TEST_HOOK_PANIC` pattern). It
does not exist in release builds.

## Tests (`engine/tests/idempotency.rs`)

New module `crash::`, each capture with its own `SELDON_NOW`
(`Cli::capture_at`, `Cli::crash`, `crash::clock`):

- `the_state_reset_note_is_written_once`: lost state directory, crash
  after the append → one note; `cursors.json` is exactly
  `{"collectors": {}, "pendingNotes": [<note ts>]}`. The next capture
  writes 0 lines, gives the reset warning, and saves bound and without
  `pendingNotes`. Then 0 and no warning. A later loss writes a second
  note. A crash before the append writes nothing, the mark holds that
  time, and the next capture writes the third note at its own time.
  Then 0. Ledger is schema-valid.
- `a_source_the_note_did_not_name_is_recorded`: crash of a `--source
  pacman` capture after a lost state directory; the next `--all` writes
  one note with `sources: snapper`, `files: cursors`. The warning names
  `snapper, pacman`.
- `a_second_crash_keeps_the_first_mark`: bound here (an unreadable
  pacman cursor); crash → reset note, and the marked file keeps the
  entries and the binding as loaded. Snapper fails, second crash → only
  the access note, `pendingNotes` = both times. The next capture writes
  0, warns, and clears the mark.
- `the_snapper_note_is_written_once`: ok → `exit 3`, crash after the
  append → one `degraded` note; the marked file still has `ok: true`.
  The next capture writes 0 and saves `ok: false`. The way back writes
  `ok again`. A crash before the append, then a hand-written `manual`
  note with subject `snapper` at the marked time; the next capture still
  writes the `degraded` note.
- `snapper_access::the_note_is_redacted_like_every_event`: snapper
  prints `token=…` on stderr; the note's detail holds
  `token=‹redacted›` (SPEC §7 sentence).

All WP-081/088/091 tests pass unchanged. The old library-level crash
tests (`idempotency::a_crash_before_the_cursor_save_does_not_duplicate`
etc.) still cover the collector events.

## Mutants

The script is in my scratchpad (`mutants/run.py`). It applies each
mutant to the committed tree, runs `cargo test --no-fail-fast --test
idempotency --test doctor --test index --lib`, and restores with `git
checkout HEAD --` plus `touch`. A final clean build and the full suite
ran after the last mutant. Build errors are reported separately; there
were none.

| # | Mutant | Result | Killed by |
|---|---|---|---|
| M0 | no dedup (the WP-091 behaviour) | killed | all 4 `crash::` |
| M1 | no save before the append | killed | all 4 `crash::` |
| M2 | marked file = bound state, not as loaded | killed | `the_state_reset…`, `a_source…` |
| M3 | save after the append keeps `pendingNotes` | killed | 3 incl. `the_snapper…` (the way back is swallowed) |
| M4 | no carry-over of an earlier mark | killed | `a_second_crash…` |
| M5 | snapper dedup ignores the subject | killed | `a_second_crash…` |
| M6 | no snapper dedup | killed | `a_second_crash…`, `the_snapper…` |
| M7 | reset dedup by subject only (any recorded reset skips the note) | killed | `a_source…` |
| M8 | no reset dedup | killed | 3 incl. `the_state_reset…` |
| M9 | marked lookup ignores the time | killed | `the_snapper…`, `the_state_reset…` |
| M10a | marked lookup ignores the source | killed | `the_snapper…` (the `manual` note) |
| M10b | marked lookup ignores the kind | **equivalent** | — |
| M11 | warning only when the note is written | killed | `a_second_crash…`, `the_state_reset…` |
| M12 | noted sources taken from notes of any subject | **equivalent** | — |
| M13 | mark saved only for a reset note | killed | `a_second_crash…`, `the_snapper…` |
| M14 | marked month read wrongly (`%Y`) | killed | all 4 `crash::` |
| M15 | mark holds the baseline, not the capture time | killed | all 4 `crash::` |
| M16 | `seldon` notes skip redaction (`ledger.rs`, §7 claim) | killed | `the_note_is_redacted…` |

Why M10b and M12 are equivalent:

- **M10b:** besides notes, Seldon writes only resolutions (subject = the
  explained package or config path) and `case-created`/`case-completed`
  (subject = a case id). None of them can carry the subject `snapper` or
  `state-reset`.
- **M12:** only `state-reset` notes carry `meta.sources`.

Both guards keep the identity explicit (source `seldon`, kind `note`,
subject), as the WP names it.

## Decisions needed (none blocking)

1. **Design differs from the WP text.** The WP asks for "dated at or after
   the old `lastRun`". I use a mark saved before the append instead, for
   the reasons in "Design". The cost is one more atomic write of
   `cursors.json`, only in a capture that writes a note (rare), and one
   new optional field. Please confirm, or name the rule you want for a
   lost state directory without a file.
2. **SPEC §7 sentence: the WP's wording would be false.** The WP asks for
   "§7 redaction does not apply to seldon notes". But `Ledger::append`
   redacts every event, these notes included (M16 and the new test prove
   it). So I wrote the true sentence: the notes are redacted like every
   event, and the collector message they embed is carried unredacted by
   `cursors.json`, `capture --json` and `index.json`
   (`state.collectors`). If the intent was that this unredacted copy is
   acceptable, the sentence says so implicitly. If the intent was to
   *exempt* the notes from redaction, that is a code change and I did
   not make it. A related observation, not fixed and not in scope:
   collector messages (snapper's stderr, for example) reach `index.json`
   unredacted. That could be a §7 item for WP-097 or later.
3. **Small leftovers, all accepted in my view:**
   - doctor's "the last capture recorded a state reset" row disappears
     after the capture that completes a crashed reset, one capture
     earlier than in the normal flow (its `lastRun` is newer than the
     crashed note). That capture prints the warning instead.
   - A crashed capture's note says "recorded <crash time>", but the
     baselines were saved one capture later, so the gap really ends
     there.
   - If snapper flips back before the next completed capture, the ledger
     keeps the crashed capture's note and no note for the flip back (the
     saved state never had the crashed run).
   - Whole-second ambiguity: the time key could match a note of an
     earlier completed capture in the *same second* only if that capture
     wrote the same note and the next one crashed before its append. In
     practice that cannot happen: the reset would need the same source
     lost twice in one second, and the snapper note would need two flips
     in one second.

## `just check`

`flock /tmp/seldon-check.lock just check`: exit 0, run once after the
last mutant, ending with `check: ok`.

| Suite | Passed | Failed |
|---|---|---|
| bar-view | 143 | 0 |
| panel-view | 782 | 0 |
| overlay-view | 319 | 0 |
| service-states | 314 | 0 |
| install.test | 209 | 0 |
| real-home-guard | 11 | 0 |
| model.test.js | 89 | — |

## Notes

- No guard-hook block happened.
- The host was not touched; all runs used the tests' scratch HOME and
  stubs. `redact.rs` (WP-097) and `collectors/config.rs` (WP-103) were
  not touched; `ledger.rs` changed only as mutant M16 and was restored
  (`git status` clean).
- SPEC §2 was touched for the `cursors.json` shape (`pendingNotes`),
  besides the §3/§4/§7 sentences the brief names.
