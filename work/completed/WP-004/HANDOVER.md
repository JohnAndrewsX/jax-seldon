WP-004 HANDOVER

Branch `wp/004-collectors-core`, worktree `wt/WP-004`, on top of `3c7e2bd`. Not pushed, no PR.

Commits, oldest first:

| Commit | What |
|---|---|
| `a5ea486` | foundation: event model, ledger, redaction, collector registry, capture skeleton |
| `e75b900` | `random_hex` from ULID randomness |
| `d973f19` | `FOUNDATION.md` (**the early-merge slice ends here**) |
| `2eb8410` | package-log collector |
| `5b760eb` | snapper collector |
| `58aac81` | omarchy collector |
| `ffb7fda` | **redaction fix**: URL userinfo kept the `@` |
| `260fb98` | tests and golden files |
| `e063c04` | memory |
| `5d783a7` | first handover |
| `6b97256` | review: test flakes (ETXTBSY retry in `sys::run`; the bench keeps one lock) |
| `fc5c535` | review 1, 2, 4: pending Running line, ADR-0017 attribution, query forms |
| `c83f9b4` | review 3: URL userinfo up to its last `@` |
| `761dfbf` | review 5: dedupe of capture-time events |
| `29c1c8e` | review nice-to-haves: `--since` notice, canonical logbook key |
| `059a612`, `7033ef1` | handover for round 1; memory |
| `71a3ac1` | review round 2: a naming cause reaches only packages the transaction names |
| last | this handover update |

- **The foundation slice on its own:** I checked out `d973f19` in a throw-away
  worktree. fmt is clean, clippy `-D warnings` is clean, and 78 tests pass.
- **Take `ffb7fda` and `c83f9b4` with the slice.** The early-merged foundation
  has redaction bugs: `https://user:pw@host` became `https://‹redacted›host`,
  and `https://user:p@ss@h/` leaked `ss@h`. Both commits touch only
  `redact.rs` and `tests/redaction.rs`.

## Review round 2 (approve, one tightening) — what changed

- **Change.** In `attribute()`, a hook command that *names* a package can now
  attribute only members with `explicit != Some(false)`. That means the
  transaction's own command names the package too, or the transaction has no
  logged command (`find_cause(…, named_by_tx, tx_full_upgrade)`).
- **`explicit: false` members** (dependencies, or packages upgraded along the
  way) get only two routes:
  - the full-upgrade path (ADR-0017 §3);
  - inheritance from an attributed member of their own transaction.
- **Test:** `collectors::a_naming_command_does_not_reach_a_later_plain_upgrade`,
  the reviewer's case.
  - The agent runs `yay -S zed` at 12:00, in its own transaction. That install
    is attributed to `agent:claude-code`/C-2026-004.
  - At 12:05 a human's plain `-Syu` upgrades zed and firefox. Both stay
    `system`, with no case.
  - Before this change, the zed upgrade matched the naming command and
    firefox inherited from it, so the test would have failed.
- **The fixture story is unchanged and green.**
  - The keyring (named by `omarchy update`), zed and ollama are explicit in
    their own transactions.
  - The `-Syu` members come in through the full-upgrade path.
  - alsa-lib inherits from zed.
- **Gate:** `just check` exits 0 with **110 tests** (collectors 14).

## Review round 1 (send back) — what changed

All five findings are fixed, and both nice-to-haves are done. `just check`
exits 0 with **109 tests** (the first handover had 104).

1. **Blocker: Running line during the download** (`collectors/pacman.rs`, `parse`).
   - If `db.lck` is held, no transaction is open, and a Running line is pending,
     the cursor now rewinds to that Running line.
   - Without the lock it moves on as before, because the invocation was a
     no-op or failed.
   - Tests:
     - unit `transaction_buffering`: Running line only, lock held → `resume == base`;
     - `collectors::a_running_line_during_the_download_is_read_again`: the
       transaction then keeps `meta.command` and `explicit: false`.
2. **Blocker: full upgrades attributed any transaction** (ADR-0017 §2 §3).
   - `find_cause(causes, package, began, tx_full_upgrade)`.
   - **Window:** the command started at most 10 minutes before the transaction
     *began*, and not after it. "Began" is its Running line, else
     `transaction started`; an event outside a transaction begins at its own `ts`.
   - **Full upgrades:** a full-upgrade cause counts only when the transaction's
     own `meta.command` is a full upgrade, or the transaction has no command
     line. Otherwise the cause must name the package.
   - **Keyrings:** `command_intent("omarchy update")` also names
     `archlinux-keyring` and `omarchy-keyring` (`OMARCHY_UPDATE_NAMES`).
   - **Inheritance:** every member of an attributed transaction inherits,
     explicit members first (§2 "every member … inherits"). Before, only
     `explicit: false` members inherited.
   - Test `collectors::a_full_upgrade_command_does_not_cover_a_separate_install`,
     the 12:00/12:05 scenario: keyring and `-Syu` → `agent:claude-code`/C-2026-003;
     the human's `pacman -S tailscale` at 12:05 → `system`, no case.
   - The fixture story is unchanged and green.
   - **§4 confirmed:** the omarchy `update` inherits from the pacman event that
     moved `omarchy` to `meta.to` (same capture or ledger). It falls back to a
     full-upgrade command at most 10 minutes before the capture time.
     `omarchy.rs` already did exactly this; no change was needed.
3. **URL userinfo with `@` in the password** (`redact.rs`).
   - The class is now `[^/\s'"]+(@)`: greedy, up to the last `@` before the path.
   - Two new table rows: `https://user:p@ss@h…` and `ssh://git:se@cr@t@host…`.
4. **Query forms are not mutating** (ADR-0017 §5).
   - `PacmanCommand.query` is set for `-S` with `s|i|l|g|p|w|c`, `-R`/`-U` with
     `p`, and the long forms (`--search --info --list --groups --print
     --downloadonly --clean`). `is_mutating()` is false for them, and `-Q`,
     `-T`, `-F`, `-D` were never mutating.
   - With `-R`, `s` and `c` mean `--recursive` and `--cascade`, so they still
     mutate.
   - Tests:
     - unit `query_forms_are_not_mutating`: 19 query forms, 7 mutating forms;
     - `collectors::queries_and_late_commands_never_attribute`: an agent's
       `pacman -Ss zed`, then a human's `pacman -S zed` → `system`, no case.
       The same test covers a command that started after its transaction
       began → not attributed.
5. **Dedupe of capture-time events:**
   - **omarchy:** no `update` is written when the ledger's latest `update`
     within the 31-day lookback already is `from → to`. It is the *latest*
     one, so a real re-upgrade after a downgrade is still recorded.
   - **snapper:** no `snapshot-delete` is written when the ledger's latest
     snapper event for that number is already a deletion. This reads the whole
     ledger, but only when a deletion is pending.
   - Tests:
     - `idempotency::a_crash_before_the_cursor_save_does_not_duplicate`: the
       ledger is written, the old cursors are restored, and the next run writes
       0 snapshot-deletes and 0 updates; it also covers the re-upgrade case;
     - in `idempotency::capture_writes_the_ledger_once`: after the capture that
       wrote the update and the deletions, `cursors.json` is deleted, and
       `capture --all --since <fixture created>` writes 0.
6. **Nice-to-haves** (`commands/capture.rs`):
   - **`--since` notice.** `--since` with collectors that have a cursor
     prints `note: --since ignored for snapper, pacman, omarchy (they continue
     from their cursor)`, and the JSON carries `sinceIgnored: [...]`.
   - **Canonical logbook key.** The logbook path is canonicalised before
     `cursors.json` is bound to it, so `--logbook <lb>/../lb` keeps the
     cursors. A test checks this through `sinceIgnored`.
7. **Test flakes I found while doing this** (`6b97256`). Neither affects the
   single-threaded engine.
   - **`ETXTBSY`.** A freshly rewritten stub program can fail to exec while
     another test thread's forked child still holds its write descriptor.
     `sys::run` now retries a spawn that fails with `ETXTBSY`, up to 20 times
     with 5 ms between tries. That is harmless on a real host.
   - **Lock races.** The test bench re-took the state lock on every run and
     raced with forked children that held the inherited flock until exec. The
     bench now takes the lock once.
   - After the fixes, 60 consecutive runs of `collectors`, `idempotency` and
     `redaction` passed.

Verified again after the fixes:
- `just check` → exit 0.
- The manual acceptance is unchanged: fixtures 15 → 0 → 6 → 0; the real host,
  read-only into a throw-away logbook, 1230 → 0, with snapper degraded.
- Real `~/.config/seldon` and `~/.local/state/seldon` are still absent.

## Done

- **Foundation** (details in `work/active/WP-004/FOUNDATION.md`):
  - `model/event.rs`: a typed `Event` after `event.schema.json`.
    - `meta` is typed in fixture key order; all 67 fixture ledger lines round-trip byte for byte.
    - `zone_for` follows ADR-0014 §2.
    - `Event::validate` checks every schema rule serde does not enforce.
  - `ledger.rs`: `append(&Lock, events)`.
    - It assigns ULIDs, monotonic in input order.
    - It redacts `detail` and `meta.command`, and cuts `detail` at 4096 characters.
    - It validates every event; if one is invalid, nothing is written.
    - Lines go to the month file of each event's `ts`. A torn last line is followed by a newline first.
    - Reading: `read_month` (returns bad line numbers), `read_all`, `read_range`.
  - `collectors/mod.rs`:
    - the `Collector` trait and `Outcome` (`ok`, `message`, `fix`);
    - `REGISTRY`, the six collectors in SPEC order;
    - `Ctx` (`now`, `baseline`, `tz`, `sources`, `ledger`, and `earlier` = events of collectors that already ran);
    - `Sources`, with `SELDON_*` env overrides;
    - `Cursors`: `cursors.json` with each collector's cursor plus `ok`/`message`/`fix`/`lastRun`/`events` (input for `index.state.collectors`), bound to the logbook path.
  - Stubs `collectors/{plugins,theme,config}.rs` for WP-005.
  - `commands/capture.rs`: `seldon capture [--source a,b | --all] [--since TS]`, with `--json` and `--quiet`.
  - `main.rs`: one variant and one arm.
- **Package-log collector** (`collectors/pacman.rs`):
  - **Lines.** A table-driven line grammar (`LINE_RULES`). Only complete lines count; the unterminated last line is never read.
  - **Transactions.** `txId = tx-<local time of transaction started>`. The meta.command comes from the last Running line, and is consumed so it is not carried into a later transaction.
  - **Buffering (ADR-0013 §5).** A transaction is emitted after `completed`, after the next `started`, or when `db.lck` is absent. While pacman runs, the cursor rewinds to the transaction's Running line.
  - **Cursor and rotation.** The cursor is inode plus offset. On rotation, the rest of `<log>.1` is read first (when it has the old inode), then the new file from 0. A shrunk file is read from 0.
  - **Dedupe.** `(ts, kind, subject, version)` against the ledger. It runs on every capture, not only after a rotation, so a lost `cursors.json` does not duplicate anything.
  - **argv parser** (`parse_command`).
    - Option-argument tables are taken from the fixture check: `--overwrite`, `--config`, `--ask`, `-b`, `-r`, …. An unknown option takes no argument, which errs red.
    - `repo/` prefixes and version constraints are stripped. For `-U`, the name comes from the package file name.
    - `is_plain_full_upgrade()` implements the ADR-0013 §3 command test for WP-008.
  - **Explicit/dependency.** A package the command names is `explicit: true`, the rest of the transaction `false`. Without a Running line, `explicit` is omitted.
  - **Attribution (ADR-0014 §1).**
    - `command_intent` reads hook command lines: segments split by `&& || ; |`, leading `VAR=…` and `sudo`/`doas`/`env` skipped; pacman/yay/paru, `omarchy update`, `omarchy pkg add|aur add|drop|remove`, `omarchy-pkg-*`.
    - The latest hook `command` event (actor ≠ system) counts if it named the package or ran a full upgrade, and lies in [start of the pacman invocation − 10 min, event ts].
    - A dependency inherits from the attributed explicit member of its transaction.
- **snapper** (`collectors/snapper.rs`):
  - It runs `snapper --jsonout list` (via `SELDON_SNAPPER`), never with sudo.
  - New numbers become `snapshot` events: `ts` is the snapshot date in the local zone; `meta.type`, `cleanup`, and `pairOf` on a post.
  - Vanished numbers become `snapshot-delete` events at capture time, with `type` and `description` taken from the cursor.
  - The cursor is the set of known snapshots.
  - Degraded per ADR-0011: `No permissions.` → `ok: false`, with the index-variant message and doctor's `SNAPPER_FIX`; the cursor is kept. A missing snapper, a failed run or unreadable output also degrade.
- **omarchy** (`collectors/omarchy.rs`):
  - `omarchy-version`, with `pacman -Q omarchy` as the fallback. No git.
  - A change → `update` event at capture time with `from`/`to`. Without a cursor: baseline only.
  - Attribution comes from the pacman upgrade of `omarchy` to the same version; else from a full-upgrade hook command at most 10 minutes earlier.
- **`redact.rs`:**
  - every SPEC-ENGINE §7 rule (url-userinfo, `--password`, `token=`, `Authorization:`, AKIA, `ghp_`, `sk-`, `mysql|psql|smbclient -p`);
  - user `[redaction] patterns`. An invalid pattern is exit 1: Seldon refuses to write rather than leak.
- **Tests: 104 in total** (WP-003 had 71).
  - **New unit tests:** event 4, ledger 3, package log 4, snapper 1.
  - **`collectors::` (10):**
    - The fixture story reproduces **every** package-log, snapper and omarchy line of `fixtures/logbook/ledger/*.jsonl`, actor/case attribution included; only ids differ.
    - Offsets: cursor 6129 → the same 12 events; complete lines end at 11159.
    - Rotation: the copytruncate repeat is not duplicated; the old file's tail is read before the new file.
    - Transaction buffering, including an interrupted transaction.
    - Attribution: proximity alone is no proof; 11 minutes is too early; a dependency inherits.
    - Degraded snapper; the omarchy fallback; the hook-attributed update; registry order.
  - **`idempotency::` (5):**
    - Every collector run twice writes 0.
    - Lost cursors do not duplicate.
    - CLI `capture --all --json`: 15 → 0 → 6 → 0.
    - A degraded snapper still exits 0, and the others run.
    - Source selection and exit codes 1 and 3.
  - **`redaction::` (6):** one table row or more per built-in rule, with a check that every rule has a row; harmless text stays; user patterns; the secret hook fixture; ledger end to end; a collected command line.
  - Every emitted event is validated with `jsonschema` against `schema/event.schema.json`.
  - Golden files: `engine/tests/golden/{pacman,snapper,omarchy}.jsonl` (`SELDON_BLESS=1` rewrites them).

## Not done

- **`capture` does not commit, rebuild the index, or reconcile.**
  - Committing needs WP-006's commit helper; I did not touch `logbook/`.
  - The index is WP-007; drift is WP-008.
  - `capture` also does not generate the `ledger/YYYY-MM.md` view (SPEC-LOGBOOK §5); it needs drift and case data.
- **`seldon init` still skips the first capture** (WP-003 left a stub).
  - With my baseline rule, the first capture after init takes the logbook's `created` as its baseline. That is the same as the README's "baseline cursor 6129" on the fixture.
  - Wiring it in touches `init.rs`, which I left alone.
- **`repoHead` is not read.** Omarchy is not a git checkout on either host; SPEC says to omit it.
- **Only the snapper `root` config is read**, because the event subject is the bare number.
- **plugins, theme and config are WP-005's stubs.** They report `ok: true` with "not implemented yet (WP-005)".

## Verified by

On the dev host, in `wt/WP-004`:

```
$ just check                                  → exit 0
  fmt-check ok · clippy -D warnings ok · test: 104 passed (unit 39, cli 13,
  collectors 10, doctor 7, frontmatter 9, idempotency 5, init 15, redaction 6)
  validate-fixtures: ok — 94 instances … 67 ledger events traced … backend builtin
  plugin-validate: ok · qmllint: ok (3 files) · check: ok
```

Manual acceptance, with `XDG_CONFIG_HOME`/`XDG_STATE_HOME` in a scratch dir
and `TZ=Europe/Berlin`. A fresh logbook comes from
`seldon init --non-interactive --no-git --path <tmp>/lb`. **How the collectors
are pointed at the fixtures:**

```
export SELDON_LOGBOOK=<tmp>/lb
export SELDON_PACMAN_LOG=fixtures/logs/pacman.log         # package log
export SELDON_PACMAN_DB_LOCK=<tmp>/no-db.lck             # absent = pacman idle
export SELDON_SNAPPER=<tmp>/bin/snapper                  # #!/bin/sh: cat fixtures/logs/snapper-before.json
export SELDON_OMARCHY_VERSION=<tmp>/bin/omarchy-version  # #!/bin/sh: echo 4.0.5-1
# (SELDON_PACMAN = the program used for `-Q omarchy`)

$ seldon capture --all --since 2026-09-01T19:00:42+02:00 --json
  written 15, files [ledger/2026-09.jsonl, ledger/2026-10.jsonl]
  snapper 3 · package log 12 · omarchy 0 (baseline) · plugins/theme/config 0 (stubs)
$ seldon capture --all --json                → written 0
  (stubs switched to snapper.json and 4.0.7-1)
$ seldon capture --all --json                → written 6   (+111 +112 +113 −108 −109, one update)
$ seldon capture --all --json                → written 0
```

Read-only against the real host (real package log, real `snapper`, real
`omarchy-version`), into a throw-away logbook with redirected XDG dirs:

```
$ seldon capture --all --since 2026-09-01T00:00:00+02:00 --json
  written 1230 (package log) · snapper ok:false "snapper: No permissions. …" + fix · omarchy baseline 4.0.4-1
$ seldon capture --all --json                → written 0
```

- All 1230 real events validate against `event.schema.json`. I checked them
  with `scripts/validate-fixtures.py`'s builtin validator, from a scratch script.
- The run covers 22 transactions. Both offsets (`+0000` and `+0200`) occur,
  and archinstall's `-b /mnt -r /mnt` lines parse.
- `~/.config/seldon` and `~/.local/state/seldon` do not exist on the host
  afterwards, and the scratch dirs are deleted.
- Not run on the test host: it has no `just` and no rustup (memory/host.md).

## Learned (in memory/)

- **`rust-notes.md`:**
  - the event/ledger/collector conventions;
  - the `SELDON_*` source overrides;
  - Tz injection;
  - the `tests/support` bench and `SELDON_BLESS`;
  - ulid 3 API quirks (`generate()`; nil passes a pattern check);
  - serde_json key order → typed `Meta`;
  - no look-around in `regex` → replacement templates;
  - jsonschema offline setup;
  - chrono instant equality and offset formatting.
- **`pitfalls.md`:**
  - the guard also blocks the word inside comments and notes;
  - count fixture events from the file (12, not 13);
  - capture-time events need the fixture's `now`;
  - the 10-minute window alone cannot produce the fixture;
  - the PostToolUse timing problem;
  - real-host runs need redirected XDG dirs.

## Decisions needed

**After review round 1: none open.**
- Decision 1 is settled by ADR-0017 §1: WP-009 records the PreToolUse start time.
- Decisions 2–11 are confirmed as implemented. Decision 2 is ADR-0017 §4.
  The orchestrator writes 3–11 into SPEC-ENGINE.
- The original text stays below for the record.

1. **Hook timing vs ADR-0014 §1 (affects WP-009).**
   - Claude Code's `PostToolUse` hook fires *after* the command has finished, so a hook `command` event's `ts` would lie after the pacman lines it caused. The ADR requires the command to *precede* them, so attribution would never match real hook data.
   - The fixture has the command first; the code follows the ADR literally.
   - Options:
     - (a) WP-009 records the command's start time (PreToolUse, or the payload's own timing);
     - (b) amend the ADR to accept a command event within N minutes *after* the transaction began;
     - (c) both.
   - I recommend (a) with (b) as a fallback. That is an orchestrator/ADR call.
2. **The omarchy `update` event inherits from the pacman upgrade of `omarchy` to the same version.**
   - The fixture's 10-01 update (09:21:00) is 10 min 58 s after `omarchy update` (09:10:02), outside the 10-minute window. Only this rule reproduces the fixture.
   - The window rule stays as the fallback.
   - Please confirm as the reading of ADR-0014 §1 "within the same pacman transaction".
3. **The baseline rule and `--since`.** SPEC gives no semantics for either.
   - A collector without a cursor emits only events at or after the logbook's `created` (or `--since TS`). On the fixture, that equals README's "baseline cursor 6129".
   - Diff collectors (snapper, omarchy, and WP-005's) record state on the first run.
   - `--since` has no effect on collectors that already have a cursor.
   - Please confirm, and put it in SPEC-ENGINE §3/§4.
4. **The snapper cursor is the set of known snapshots, not "last snapper id"** (SPEC-ENGINE §2). A `snapshot-delete` event needs the deleted snapshot's type and description. Please fix the SPEC wording.
5. **Dedupe runs on every capture, not only after a rotation.** It is cheap (one ledger range read) and makes a lost `cursors.json` harmless. Please confirm.
6. **`cursors.json` is bound to the logbook path.** Switching logbooks re-baselines every collector instead of reusing another logbook's cursors. Its shape is `{logbook, collectors: {name: {cursor, ok, message, fix, lastRun, events}}}`; WP-007 reads `ok`/`message`/`lastRun` from it for `index.state.collectors`.
7. **The `capture` JSON shape is my choice:** `{ok, logbook, written, files, collectors: [{name, enabled, ran, ok, events, message?, fix?}]}`, exit 0 even when degraded.
   - `--source` runs named collectors even when they are disabled in config.
   - `--source` together with `--all` is exit 1.
   - Should SPEC-ENGINE §3 list this shape next to `doctor`?
8. **Redaction choices beyond the letter of §7.** All of them redact more, never less:
   - `sk-` also matches `-`/`_` (`sk-proj-…`);
   - mysql's attached `-pSECRET` is redacted;
   - `psql -p` is the *port* but is redacted, as §7 says;
   - `token=` is case-insensitive (`GITHUB_TOKEN=`);
   - quoted values are redacted whole.
9. **The hook command classifier lives in `collectors/pacman.rs` (`command_intent`, `parse_command`).** SPEC §8 says the hook uses "the same rules as the pacman command parser", so WP-009 should call these and not write a second parser. WP-008's routine rule should call `PacmanCommand::is_plain_full_upgrade` on `split_logged(meta.command)`.
10. **New crates.**
    - `ulid` 3 (`serde`) and `regex` 1 are on the allowed list. `ulid` brings `rand` 0.10 and `getrandom` transitively.
    - `jsonschema` 0.58 is a dev-dependency only, with `default-features = false`. It is allowed for tests and pulls about 40 transitive dev crates.
    - Justification for `jsonschema`: real draft 2020-12 validation of every emitted event, including `format: date-time`. Shelling out to the Python validator from `cargo test` would make the tests depend on python3.
11. **No fixture is wrong.** Every package-log, snapper and omarchy line of the fixture ledger is reproduced exactly, including `txId`, `explicit`, `meta` key order, and actor/case attribution. The README offsets (6129, 11159, 7359) hold, and so does the rotation expectation. One note for the README: "the pacman events of `logbook/ledger/*.jsonl`" are 12; a count next to it would help the next agent.

## Touched outside WP scope

- `engine/src/sys.rs`: `random_hex` (the nit the orchestrator assigned). In
  review round 1, also the `ETXTBSY` spawn retry in `sys::run`.
- `decisions/` and `docs/` were not touched; ADR-0017 was only read, from main.
- `engine/src/model/mod.rs`: `pub mod event` and a re-export.
- `engine/src/lib.rs` and `engine/src/commands/mod.rs`: module registration.
- `engine/Cargo.toml` and `Cargo.lock`.
- `memory/rust-notes.md` and `memory/pitfalls.md`: appended, as the brief asked.
- `schema/`, `fixtures/`, `scripts/`, `docs/`, `plugin/`, and WP-006's files were not touched.
- I read `commands/doctor.rs` and use its `SNAPPER_FIX` constant, but did not change it.
