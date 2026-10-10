# WP-198 — Handover

Branch `wp/198-pacman-stream` (from `next` at `0b07f825`, WP-119's
look-back), worktree `wt/WP-198`. From the review of WP-119, N1.

## What was done

- **Streamed read** (`engine/src/collectors/pacman.rs`). `read_from`
  (`read_to_end` of the whole log) is gone. `Lines` reads one complete
  line at a time through a 64 KiB `BufReader`; what is held is the buffer
  and one line. The parser is now `Parser` (`line()` per complete line,
  `finish(end)`); `parse(bytes, base, lock, tz)` is a thin wrapper over
  it, so every existing unit and fixture test runs the same code. The
  cursor semantics are unchanged: inode plus offset of the first line not
  emitted, the unterminated last line never passed, the rewind to a held
  transaction's block, the Running-line hold (WP-160), rotation through
  `<log>.1` from the old offset, shrink → 0, dedupe by `(ts, kind,
  subject, version)`.
- **Skip to the look-back start** (`look_back_start`). Only without a
  cursor (the baseline: ADR-0033's look-back, or a lost cursor). Lines
  are skipped up to the first whose time is at or after the baseline: of
  each only the bracketed time is parsed (`parse_ts`, the same function
  the line table uses), and the line table runs only on a `] [PACMAN]
  Running '` or `] [ALPM] transaction ` line. The parse then begins at
  that line, or earlier at the block of the transaction open there (its
  Running line, else `transaction started`), or at a Running line no
  transaction has taken yet; the file is seeked back there and parsed
  normally. Why that is exact (not just for ordered times): every line
  before the start is older than the baseline, so none of its events is
  kept; the only state a line passes on to later lines is the open
  transaction and the pending Running line, and both begin at or after
  the start. The baseline filter (`e.ts >= ctx.baseline`) is unchanged,
  so a clock set back after the start is handled as before. I did **not**
  seek from the end / binary-search: a clock set back makes the times
  unordered and a search could land after a kept event (the spec says
  so).
- **Inode from the open file.** `collect` opens the log first and takes
  inode and length from the open file's metadata, not from a separate
  `metadata(path)` (the cursor's inode is the file actually read).
- **Bounded discipline as WP-174.** The log and `<log>.1` are opened with
  `sys::open_regular`: a FIFO or a device at `SELDON_PACMAN_LOG` is a
  degraded pacman source naming it ("cannot read …: a FIFO, not a
  regular file; …"), where before `File::open` blocked. A line longer
  than `MAX_LINE` (1 MiB) is passed like a line the table does not know
  and only 1 MiB of it is held (a Running line naming 2000 packages is
  ~40 KiB). `tests/bounded_reads.rs`' reader list: pacman.rs 2 → 1 (only
  `/proc/stat` is a std read now).
- **Tests.**
  - Unit (`collectors/pacman.rs`): `lines_through_the_buffer` (lines
    ending before, on and after the 64 KiB edges, CRLF, exactly
    `MAX_LINE`, over-long lines passed empty, unterminated last line not
    passed, seek to an offset; `parse` treats an over-long line the
    same); `lines_open_regular_files_only`;
    `the_look_back_start_keeps_what_the_whole_log_keeps` (three logs of
    every shape — a transaction open across the baseline, Running without
    a transaction, a start without an end, `failed`/`interrupted`,
    scriptlet and hook lines, package and `.pacnew` lines outside any
    transaction, the old time format, a clock set back, lines without a
    time, CRLF, an open end, a download-phase Running line — times every
    baseline one second before, at and after every line, times four lock
    states: the events and the resume offset equal a whole-log parse);
    `the_look_back_start_rewinds_to_the_open_block` (named offsets).
  - Integration (`engine/tests/pacman_stream.rs`, its own binary because
    the peak is `getrusage(RUSAGE_CHILDREN)`): `init --defaults` with
    `SELDON_NOW` fixed on a synthetic 96 MiB log generated in `TMPDIR`
    (removed with the test's temp dir, never committed), nearly all older
    than the look-back. **Budget (debug build): < 30 s, peak < 64 MiB**;
    the run is killed and the test fails at 180 s. It asserts exactly the
    look-back's 121 pacman events, that the transaction open across the
    look-back start keeps `txId` `tx-20260716T235901`, `meta.command` and
    `explicit`, and that two `capture --all` after it write 0 (and the
    ledger is unchanged). `SELDON_PACMAN_STREAM_MIB=<n>` runs another
    size and reports only.
- **Docs.** SPEC-ENGINE §4 pacman: a **Reading** paragraph (stream,
  1 MiB line limit, regular file, the skip and why it is exact, why no
  search from the end, the budget). TESTING.md: a row for
  `pacman_stream.rs` and the new unit tests. CHANGELOG `[Unreleased]` →
  Engine: one bullet.

## Numbers

Same synthetic log generator, same host, `init --defaults`:

| Log | Build | Before (`0b07f825`) | After |
|---|---|---|---|
| 93 MiB | debug | 24.7 s, peak 922 732 kB (the test fails on the peak) | 8.4 s, peak 22 344 kB |
| 234 MiB | debug | 61.5 s, peak 2 274 188 kB | 17.7 s, peak 22 804 kB |
| 93 MiB | bench profile | — | 0.90 s, peak 10 896 kB |
| 234 MiB | bench profile | — | 1.53 s, peak 10 976 kB |

The "before" rows match the review's 60.6 s / 2 447 724 kB at 240 MB.

## Mutants

Seven hand mutants of the new code, each run against the pacman unit
tests, each killed: start = the crossing line only; start ignores a
pending Running line; `transaction completed` does not close the open
block; `>` for `>=` at the baseline; a start rewinds to `transaction
started` instead of its Running line; the over-long flag reset per
buffer chunk; the next offset one byte short. The integration test
fails against the old collector (peak 922 MB > 64 MiB).

## How it was verified

| Check | Where | Result |
|---|---|---|
| `cargo test --locked -j 4` | fixture | 55 binaries, 1404 passed, 0 failed (after the reader-list fix in `bounded_reads.rs`; the first run failed there, 2 → 1) |
| `SELDON_FULL_CHECK=1 just check` | fixture, headless | `check: ok`, exit 0: cargo 2818 passed, 0 failed (with and without `watch`); desk-view 2009, bar-view 196, service-states 359, ipc-restart 44, terminal-scripts 73, install 229, deploy-test-host 326, runtime-dir 45, real-home-guard 40, check-tokens 14 passed, 0 failed |
| `just check-rss` | fixture | **fails on this host, on the base too** — not worse. Five runs each, peak: branch 11 572 / 11 416 / 11 572 / 11 224 / 11 568 kB (mean 11 470), base `0b07f825` 11 548 / 11 612 / 11 224 / 11 356 / 11 540 kB (mean 11 456); limit 11 264 kB, 4 of 5 red on both. The watcher does not run the pacman collector. Open question 4 |
| `pacman_stream` at 96 and 240 MiB, debug and bench profile, before and after | fixture (synthetic log in a temp dir on disk, removed) | see Numbers |
| Hand mutants (7) | fixture | 7/7 killed |
| Real `/var/log/pacman.log`, old vs new engine in a scratch HOME | **not run**: the guard blocked the command ("the command name is computed (a variable, $(…) or a glob); write it literally"); reported, not worked around. Open question 5 |
| Live first capture on the test host | **not run** | — |

All runs: `CARGO_TARGET_DIR` and `TMPDIR` on disk under the private
gates folder, a private 0700 `XDG_RUNTIME_DIR` on disk, never `/tmp`;
the real `~/Seldon`, `~/.local/state/seldon` and `~/.config` untouched.

## Not done / open questions

1. **The busy 90-day case** of the review (40 000 events in the window,
   15 s, 147 MB) is not changed by this WP beyond the read itself: the
   events inside the look-back are held because they are written. I did
   not measure where its remaining time goes (index, dismissals,
   attribution). Separate WP if it matters.
2. `attribute`'s inheritance loop is quadratic in the events of a run
   when there is a cause in the ledger; on a first capture there usually
   is none. Not touched.
3. The 30 s / 64 MiB budget is for the debug build on this host; CI may
   be slower. The peak is the assertion that catches a whole-file read
   (the old code fails it by 14×); the time limit has 3.5× headroom here.
4. `just check-rss` is red on this host already on `0b07f825` (mean
   11 456 kB against 11 264 kB); this branch is the same within noise.
   Someone should look at the bound or the host separately; WP-198 does
   not touch the watcher or the index.
5. The old-vs-new comparison on the real package log was blocked by the
   guard (a computed command name in my loop over the two binaries). I
   stopped there as instructed. If you want it, it needs either a guard
   decision or a literal per-binary command you approve; the base build
   is still at `jax-seldon-private/gates/target-dev198-base`
   (bench profile) and can be removed otherwise. The equivalence it would
   show is covered on synthetic logs by
   `the_look_back_start_keeps_what_the_whole_log_keeps`.
