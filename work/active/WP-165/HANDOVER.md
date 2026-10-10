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
