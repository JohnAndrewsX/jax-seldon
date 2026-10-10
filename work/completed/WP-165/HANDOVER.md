# WP-165 — Handover

Branch `wp/165-ignorepkg` (from `next` at `d5ad7759`, which has WP-176's
`contractReadableFrom`), worktree `wt/WP-165`. Inside the pre-tag window
of ADR-0035 §6 / ADR-0051: one optional v2 field, `contractVersion` and
`contractReadableFrom` stay 2.

## What was done

- **ADR-0052, proposed** (`decisions/ADR-0052-pacman-ignore-list.md`,
  DECISIONS.md row). The operator accepts it. It amends ADR-0028 §2 by
  one row (`ignore-list`, attention), as ADR-0042 did.
- **Reading, names only** (`engine/src/collectors/pacman_ignore.rs`, new).
  The pacman collector, on every run that reads `pacman.log`, reads
  `/etc/pacman.conf` (`Sources::etc_dir`; `<guard>/etc` under the test
  guard) as pacman's `conf.c`/`ini.c` do: `#` comments, `[section]`
  state shared across includes, `Include = <glob>` followed wherever it
  stands (sorted matches, no hidden files, at most 10 levels, only
  absolute paths under `/etc`, rebased on `etc_dir`), `IgnorePkg` and
  `IgnoreGroup` only in `[options]`, split on white space, repeated lines
  adding up, deduplicated. Nothing else of any line is kept. Bounds:
  regular files ≤ 1 MiB (`sys::read_regular`), 64 files, names 1–128
  chars of `[A-Za-z0-9@._+*?!^[]-]` (a byte check, no regex), 256 per
  list, 4096 entries per globbed folder; anything past them, an include
  not followed or unreadable → `partial`.
- **Cursor** (`PacmanCursor` + `ignore`, `ignoreKnown`, both optional;
  0.1.x cursors read unchanged). No `ignoreKnown` → baseline, no event; a
  partial read never writes an event and keeps `ignoreKnown`; an
  unreadable `pacman.conf` keeps the last list marked partial; names are
  compared as sets. A change whose names the ledger's newest
  ignore-list note already records (cursor save failed after the write,
  older state directory restored) is not written again; the ledger is
  read for that only when the list changed (commit `engine: a change the
  ledger records already…`).
- **Event**: one pacman `note`, subject `/etc/pacman.conf`, capture time,
  `actor: system`, detail `IgnorePkg: added …; removed …. IgnoreGroup: …`,
  `meta.ignorePkg` / `meta.ignoreGroup` (new lists, space-separated).
  `class.rs`: attention `ignore-list`, checked before the pacnew rows.
  `seldon event` refuses both meta keys (exit 1).
- **Index**: `system.pacmanIgnore {packages, groups, partial?}` from the
  pacman cursor (this logbook's, collector enabled), names the redaction
  would change dropped (→ partial), withheld while `[redaction] patterns`
  do not compile (`index/mod.rs`, `index/model.rs`).
- **Schema**: `index.schema.json` `system.pacmanIgnore` (optional);
  `event.schema.json` descriptions (subject, the two meta keys).
- **Fixtures**: `fixtures/state/pacman-cursor.json` (the 17:05 capture's
  cursor, `IgnorePkg = zoom slack-desktop`, pinned before the logbook:
  a baseline, so the sample's ledger is unchanged); the golden test puts
  it into `cursors.json`, the reference derive reads it
  (`derive_pacman_ignore`) and learns the `ignore-list` row plus a
  self-check. New variant `index-variants/pacman-ignore-changed` (the
  09-27 mesa pin as an open attention item), index only like
  `boot-config`. All index files regenerated with `--write-index`.
- **Plugin**: `Model.systemTiles` gains the seventh tile *Ignored by
  pacman* (lead "pacman's full upgrade skips them; `pacman -S` still
  updates them.", none/partial texts, rows IgnorePkg/IgnoreGroup as plain
  text, no stripe, no action); `Model.pacmanIgnore` (shape filter);
  each tile now carries its `source` footer, which `System.qml` shows
  instead of a hard-coded `id === "recent"` switch; `eventDetail` adds
  the Hint row for the change event (`isIgnoreChange`).
- **Docs**: CONTRACT.md rule 9; SPEC-ENGINE §2 (cursor), §4 (pacman
  collector), §5 (the row), §6 (the field); SPEC-PLUGIN (tile, hint);
  TESTING.md row; fixtures/README.md (state file, variant, story row,
  result line); CHANGELOG `[Unreleased]` Engine and Plugin.

## Not done / deviations

- **The story's change is a variant, not a ledger line.** I first added
  the 09-27 pin to the sample ledger (commit `9bebf351`); it moved 13
  count-based plugin tests and the harness lists. I reverted that
  (`823506df`) and followed `boot-config`'s precedent: the change lives
  in `index-variants/pacman-ignore-changed`. The sample still renders
  every surface except the Hint row, which the variant shows.
- `seldon preview` (before `init`) does not show the list; not asked.
- SPEC-LOGBOOK has no list of meta keys; the ADR's consequence names
  `event.schema.json` instead.

## How it was verified

All on the **desktop** (dev host), with `CARGO_TARGET_DIR`, `TMPDIR` and a
0700 `XDG_RUNTIME_DIR` under `jax-seldon-private/gates/` (on disk, never
`/tmp`), `CARGO_BUILD_JOBS=4`; no network; the real `~/Seldon`,
`~/.local/state/seldon` and `~/.config` untouched (the harnesses' own
guards say so).

- **`SELDON_FULL_CHECK=1 just check`: `check: ok`** (desktop, headless
  Quickshell harnesses included) at `6c4a8d0b`: cargo 2848 passed / 0
  failed (plain and `--features watch`), clippy and fmt clean,
  validate-fixtures ok (154 instances, 14 variants, 57 self-checks),
  docs-check ok, plugin-validate and qmllint ok, model.test.js 214,
  service-states 359, desk-view 2022, bar-view 196, ipc-restart 44, all 0
  failed. The first full run (at `07fbeb1a`) failed 3 desk-view rows that
  still counted six tiles; fixed in `6c4a8d0b`.
- **shellcheck: not run** (not installed; the recipes ran `bash -n`; CI).
  No shell script was changed except `tests/plugin/desk-view.sh` (three
  expected strings and one scenario step).
- **New tests** (desktop, in the gate): `engine/tests/pacman_ignore.rs`
  (4: baseline → no event; same names → nothing; Omarchy's template plus a
  new pin → one note, attention `ignore-list`, no crisis; second capture
  and a capture with the cursors from before the change → nothing; names
  only — no mirror URL, `SigLevel` or `HoldPkg` in logbook or state;
  partial reads never reported; no field without a list or with the
  collector off; `seldon event` refuses the keys); 11 unit tests in
  `collectors/pacman_ignore.rs` (includes, comments, several lines,
  shared sections, glob order, depth 10, not-followed includes, bounds,
  names, `step`, `last_recorded`, `shown`); 3 `class.rs` rows; reference
  derive self-check `a changed ignore list is attention`; two node tests
  (the tile; the Hint, on the variant).
- **`check-rss`: fails, on the base too** (desktop, bench profile). Peak
  of `seldon watch` on ×10: this branch 11 512 / 11 412 / 11 408 kB, base
  `next` `d5ad7759` (temporary detached worktree, removed) 11 280 /
  11 400 kB, limit 11 264 kB. Heap (`RssAnon`) 916 → 3176 kB on the base,
  920 → 3180 kB here: the difference is code pages, within run-to-run
  noise. Pre-existing on this host; not caused by WP-165, but the
  orchestrator should know `check-rss` is red on `next`.
- **`check-perf`** (desktop, bench profile, load ≈ 2.5): index build ×10
  13.6 ms and ×150 80.2 ms (budget 100 ms) ok; `status` at 10 000 lines
  59.7 ms ok; redaction all ok; config and plugins capture costs ok. Over
  budget: the recent-config scan delta 11.1 / 11.3 ms (budget 10 ms,
  WP-139 code, untouched) and the hook "recorded (tmpfs)" timings 11.6 ms
  and 20.6 ms (budget 5 ms). Both assume a tmpfs temp dir; mine is on
  disk as instructed, so they measure fsync. Not compared with the base
  under the same setup; neither path runs code this WP changed (the index
  build only reads the pacman cursor already loaded).
- **Test host: not run.** **CI: not run** (no push).

## Open questions

1. ADR-0052 is **proposed**; the operator accepts it (or not) before the
   0.2.0 tag. If it misses the tag, the field goes into contract 3
   (WP-184) unchanged.
2. `omarchy refresh pacman` wiping the list gives one attention item per
   machine that had pins — intended (ADR-0052 Context 2), worth a look
   on the test host after the operator's acceptance.

## Round 2 (review 1: SEND BACK; orchestrator decisions F1–F7, Q3)

Commits `ea15c210` (engine), `59c50e97` (contract, fixtures), `81860483`
(plugin), `c4bb02aa` (docs), this file.

- **F1, fixed.** The cursor gains `ignoreAt` (when `ignoreKnown` was read,
  set by every complete read). Only the newest ignore-list note at or
  after `ignoreAt` can count as "already recorded"; the ledger is read
  only when the list changed, and only from `ignoreAt` on
  (`read_range`, no longer `read_all`). The review's scenario is the test
  `a_change_after_a_state_loss_is_recorded` (baseline → note →
  `cursors.json` removed → baseline → re-pin: a second note).
- **F2, fixed.** `recorded_since` compares the note's `meta` with the new
  lists as the ledger writes them (`(hidden)` for unshown names, then the
  logbook's redaction over the whole value, as `Ledger::append` does).
  Test `a_redacted_change_is_not_written_twice` (`corp-[a-z]+`, cursors
  restored → still one note).
- **F3, fixed.** `Limits` per read, counted, never timed: 64 files, 64
  `Include` lines (a line matching nothing counts), 16 384 directory
  entries over all globs together; past any → `partial`.
- **F4, fixed.** Two-name reorder in both lists (unit), reorder across
  lines and files (integration), 65 includes → 63 names + partial with
  the defaults, the include and entry budgets at their edges, a broken
  `[redaction] patterns` entry → no field (in-process
  `index::derive_at`, since every command refuses such a config). I re-ran
  the review's survivors as mutations of a copy of the module (restored,
  `cmp` checked): M2 (ordered compare), M12 (files cap), M13 (entries
  cap) killed, plus F1 (drop the `ignoreAt` bound), F2 (compare
  unredacted) and the includes cap — all killed by `--lib`. M14 is killed
  by `an_invalid_pattern_withholds_the_list` (reasoned: the derive's
  `zip` with the redactor is the only withholding; not run as a mutation).
- **F5, fixed.** User guide en/de `06-configuration.md`: the pacman row
  of the collector table, a paragraph (pacman.conf and its includes, names
  only, nothing else kept, first read a baseline, hidden names,
  incomplete reads), and "besides these, only `/etc/pacman.conf` and its
  includes … nothing else under `/etc`". `03-daily-use.md` en/de: the
  System tile. The low item too: the pacdiff hint and SPEC-PLUGIN now say
  "Seldon does not read that file" (ADR-0042, accepted, left as is).
- **F6, done as decided.** `partial` only for an incomplete read (a file
  that cannot be read, a relative include, depth, `[]`, the budget, names
  past 256). Every name is kept in the cursor (raw, Q3; over 512 bytes as
  `sha256:<hex>`) and counts for a change; the note writes an unshown one
  as `(hidden)`; the index shows names of the shown shape that the
  redaction leaves unchanged and counts the rest in the new optional
  `system.pacmanIgnore.hidden` (schema, CONTRACT rule 9, derive, plugin
  row "Not shown · N names (…)", the big value counts them). An include
  outside `/etc` is read: an absolute path below `etc_dir`'s parent (`/`
  on the host, `<guard>` in tests; `..` resolved lexically). Test
  `a_name_seldon_does_not_show_silences_nothing` (the review's comma
  typo).
- **F7.** Verified against `man 5 pacman.conf` on the desktop (read with
  `man`, no pacman or pacman-conf run — red zone): keys are CamelCase
  (we match exactly), `Include` uses glob(7) rules, `IgnorePkg = package
  ...` / `IgnoreGroup = group ...`. Changed: names split on spaces only
  (a tab or comma stays in the name; unit test). The page documents no
  include depth, and says comments only begin a line. Not changed, and
  **not verified**: (a) the depth stays 10 levels below `pacman.conf`
  (11 files in a chain) — the review recalls pacman refusing at
  `depth + 1 >= 10` (10 files), I recall the check before the increment;
  neither of us ran pacman; (b) `#` still cuts the rest of a line (my
  reading of pacman's `ini.c`, "end of line comments"); following the
  page literally would turn `IgnorePkg = linux # kernel` into the names
  `linux`, `#`, `kernel`. Both are stated in ADR-0052 §1 as parser
  readings, not page facts. If the operator wants certainty, one
  `pacman-conf --config <scratch> IgnorePkg` on the test host settles
  both.
- **Q3** recorded in ADR-0052 §2 (raw names in `cursors.json`).
- ADR-0052 stays **proposed**; §1, §2, §3, §5, §6 and the alternatives
  updated for F1/F3/F6/F7.

### How round 2 was verified (desktop; target, TMPDIR, 0700
XDG_RUNTIME_DIR on disk; no network)

- `cargo fmt`, `cargo clippy --all-targets -D warnings`: clean.
- `cargo test -j 4 --no-fail-fast`: **1426 passed, 0 failed** (plain
  features; `--features watch` not run this round).
- `node tests/plugin/model.test.js`: 214 passed.
- `validate-fixtures.sh`: ok (154 instances, 14 variants, 57
  self-checks). `docs-check`: ok.
- **Not run this round:** the Quickshell harnesses, qmllint,
  plugin-validate (no QML changed; `Model.js` only), `SELDON_FULL_CHECK=1
  just check`, check-rss/check-perf, shellcheck (not installed), test
  host, CI.
