WP-073 HANDOVER — collectors: plugins dedupe, capture-time attribution window, verb-aware plugin proofs, SELDON_NOW in capture, reused snapshot numbers
Branch: wp/073-review (worktree wt/WP-073), on c5d61ca, not pushed.

Commits:
- `4110327` engine: the hooks ignore an empty watch path, like the config collector
- `f2672f9` engine: capture runs on SELDON_NOW; snapper sees a reused snapshot number
- `64e3cfd` engine: plugins dedupe against the ledger, capture-time attribution window, verb-aware plugin proofs (+ config fold-in a)
- `6de4aa1`, `8e767bc` docs: SPEC-ENGINE §4 and CHANGELOG
- this handover

**Done**

- **F-100 (plugins half).** `collectors/plugins.rs` `unrecorded`: after
  the diff, events the ledger holds since the cursor's `checked` (same
  kind, redacted id, `enabled`, `version`, `from`, `to`) are dropped. The
  ledger is read only when the diff has events. A ledger read error
  degrades the collector (cursor kept), as in config. Same rule as WP-069
  for config.
- **F-105.** `attribution::Stamps` (`now` + per-source `since`): an event
  whose `ts` is the capture time happened after its collector's last
  check, so a cause counts in `[since − 10 min, ts]`. Every other event
  keeps `[ts − 10 min, ts]`. Collectors report their last check in a new
  field, `Outcome::since` (plugins: `prev.checked`; config:
  `prev.checked`). `capture` collects them per source
  (`Collected.stamps`) and calls `attribution::attribute_capture`. The
  causes are read from `earliest − 10 min`. `attribute` and
  `attribute_from_ledger` keep their signatures and behaviour (used by
  `seldon event` and the tests). The omarchy `update` event keeps its own
  logic, which is not in this WP.
- **F-106.** `attribution::plugin_verbs`: `add` → `plugin-add` and
  `plugin-enable`; `remove`, `enable`, `disable`, `update` → only their
  own kind. `omarchy-plugin-add` refuses an id that is already
  installed, so `add` can be the cause of an enabling only after a
  remove followed by `add --enable` between two captures.
- **F-103.** `capture::run` uses `ctx.now` with the nanoseconds removed,
  not `Local::now()`.
- **F-104.** `snapper::Known` has `date: Option<DateTime<FixedOffset>>`
  (the instant, so list and info files give the same value). A known
  number whose date differs gives a `snapshot-delete` (old type and
  description) and then a `snapshot`. Both get the **new snapshot's
  date**, not the capture time: a delete stamped with the capture time
  would sort after the new snapshot, and the ledger would say that 5 is
  deleted. On equal `ts` the delete is pushed first, the capture sort is
  stable, and the "latest per number" rule breaks a tie by file order,
  so the snapshot is the last word. Dedupe: a delete for a reused number
  is dropped when its new snapshot `(number, ts)` is already in the
  ledger, or when the number is already recorded as deleted. An old
  cursor (no `date`) gets no events, and the dates are filled in.
- **Fold-in (a), decided: fixed, not only documented.** The config
  collector now detects "cursor behind": the cursor names a different
  generation than the manifest's `current`. Only a failed cursor save
  (or a failed ledger write) causes that. In this case `replay` first
  applies the ledger's config events since the cursor's `checked` to the
  base generation. An event applies only when its `hashFrom` matches (no
  file, for an addition), repeated until nothing applies, so the ledger
  order does not matter. `A→B` recorded with a failed save, then the
  file back to `A`, now gives `B→A`. In the normal flow (cursor =
  current) nothing changes, so manual `seldon event config …` lines
  cannot move the base. The WP-069 `unrecorded` filter stays. A redacted
  subject that names no file of base or scan is ignored.
- **Fold-in (b).** `hook::Scope::new` builds `watched` with
  `Dirs::expand_config`, the collector's function, so an empty or blank
  `watchPaths` entry watches nothing. Before, it watched the whole home.
- **Fold-in (c), checked: not done by WP-065, and not added here.**
  WP-065's load-time check covers `actor` and `case` only
  (`Ledger::read_month`). `seldon event` checks only the subject's length
  and line breaks. I did not add the name check. See "Decisions needed".
- Docs: SPEC-ENGINE §4: the snapper, plugins and config paragraphs, and
  the attribution paragraph. The WP names "§5 (attribution window)", but
  that paragraph is in §4, the same slip as in WP-071. CHANGELOG
  `[Unreleased]` Engine: six lines added at the end of the section.

**Not done**

- A **plugins** version of fold-in (a): the plugin is enabled and
  recorded, the cursor save fails, and the plugin is disabled again
  before the next capture. The ledger then keeps the enabling without the
  disabling. The plugins cursor holds the snapshot itself, so unlike
  config there is no "behind" sign. A fix needs either a ledger replay on
  every capture (which would also react to manual `seldon event plugins
  …` lines) or a marker in the cursor. Follow-up candidate.
- The theme collector's `since` is not reported: its events are always
  timed by mtime, and `theme.rs` is not in the WP.
- No manual scratch-HOME run: the CLI tests below run each finding's
  scenario through the binary in a temp HOME with `SELDON_TEST_GUARD`
  (`common::Env`).

**Verified by**

- `just check` exits 0 (counts in the last section).
- New tests (each fails without its fix, see mutants):
  - `tests/idempotency.rs`
    `a_failed_cursor_save_does_not_repeat_plugin_events`: update,
    enable, add and remove written. The cursor is reset to the one
    before → 0 events, the ledger still has 4. Later real changes,
    including a step back and the same step again, are recorded. This is
    the crash-idempotency test for plugins; it is a new test next to
    `a_crash_before_the_cursor_save_does_not_duplicate`, not an edit of
    it.
  - `tests/idempotency.rs`
    `a_file_back_to_its_content_after_a_failed_cursor_save_is_recorded`
    (fold-in a): `A→B` written, the cursor is reset, the file goes back
    to A → one `config-change` whose hashes are the reverse of the first.
    The next run gives 0, the ledger has 2.
  - `tests/attribution.rs` `rules::a_capture_time_event_reaches_back_to_the_last_check`
    (F-105, in-process): the last check is 10:15 and the capture 10:30.
    A `remove` command at 10:08 is attributed; without stamps it is not.
    10:05:00 is in, 10:04:59 is out, after the capture is out. An event
    timed by its change keeps its own window. A config event (no
    `since` for config) is not widened.
  - `tests/attribution.rs` `capture_time::a_plugin_change_found_15_minutes_later_is_the_agents`
    (F-105 end to end through `seldon event agent command` and `seldon
    capture` with `SELDON_NOW`): a disable command at 10:05 and a capture
    at 10:20 → `plugin-disable` by `agent:claude-code`. An enable command
    at 10:09 and a capture at 10:30 (last check 10:20) → `system`.
  - `tests/attribution.rs` `rules::a_plugin_verb_proves_only_its_kind`
    (F-106): 5 verbs × 2 spellings × 5 kinds. `plugins_need_the_id` now
    expects `omarchy-plugin-enable <id>` not to prove a `plugin-add`
    (it did before, by design of the old rule).
  - `tests/collectors.rs` `capture_runs_on_seldon_now` (F-103, CLI): with
    `SELDON_NOW=2026-12-24T10:00:00+01:00`, a `config-remove` gets that
    `ts`, goes to `ledger/2026-12.jsonl`, and the config collector's
    `lastRun` and `cursor.checked` equal it.
  - `tests/collectors.rs` `snapper_sees_a_reused_number` (F-104): 5
    deleted and recreated with a new date → `snapshot-delete 5`, then
    `snapshot 5`, both at the new date, also in this order in the ledger.
    A re-run gives 0. A cursor reset (crash) gives 0. An old cursor
    without dates gives 0 events and gets the dates. A reuse after that
    is seen.
  - `src/commands/hook.rs` `tests::an_empty_watch_path_watches_nothing`
    (fold-in b).
- Mutants (each applied alone to the committed code; the file was
  restored with `git checkout HEAD --` + touch; all are assertion
  failures, none is a build error):

  | # | Mutant | Killed by |
  |---|---|---|
  | M1 | plugins: no `unrecorded` | `a_failed_cursor_save_does_not_repeat_plugin_events` (idempotency.rs:245, 4 events repeated) |
  | M2a | `Stamps::earliest` always `e.ts` (the old window) | `a_capture_time_event_reaches_back…` (`system` ≠ agent) and `a_plugin_change_found_15_minutes_later…` |
  | M2b | capture passes no `since` | `a_plugin_change_found_15_minutes_later…` (`system`) |
  | M3 | `remove` proven by every verb | `a_plugin_verb_proves_only_its_kind` ("add → plugin-remove") |
  | M3b | `enable` proves `plugin-add` | `plugins_need_the_id`, `a_plugin_verb_proves_only_its_kind` |
  | M4 | capture on `Local::now()` (the pre-fix line) | `capture_runs_on_seldon_now` (`ledger/2026-10.jsonl` ≠ `2026-12`) |
  | M5a | date never compared (pre-fix behaviour) | `snapper_sees_a_reused_number` (`[]` ≠ delete + snapshot) |
  | M5b | reuse delete at capture time | `snapper_sees_a_reused_number` (ts 09:15 ≠ 09:05) |
  | M5c | no dedupe of the reuse delete | `snapper_sees_a_reused_number` (crash re-run not empty) |
  | M5d | old cursor without date reports a reuse | `snapper_sees_a_reused_number` ("an old cursor cannot tell") |
  | M6 | no config `replay` | `a_file_back_to_its_content…` (0 ≠ 1) |
  | M7 | hook scope via `home_path` (pre-fix) | `an_empty_watch_path_watches_nothing` |

**Learned** (appended to memory/pitfalls.md)

- `common::Env` CLI tests have only the stub directory on PATH: a stub
  that runs `cat` fails, and the plugins collector shows the failure as
  "… (it needs the running Omarchy shell)". Use `/bin/cat`.
- A deletion that frees a number must not carry a time after the
  creation that reuses it. The snapper dedupe reads "the latest event per
  number", and a capture-time delete would make a snapshot that exists
  look deleted.
- No `git add -p`: splitting by hunk works with `git diff -U2`, the
  chosen hunks fed to `git apply --cached --recount -`.

**Decisions needed**

1. **Fold-in (c), `seldon event` name check on write.** I recommend not
   adding it now, for three reasons:
   - It is not a full second guard. The theme, plugins and pacman
     collectors write names without that check too, so REBUILD.md must
     keep its own check (WP-059) anyway.
   - Refusing the write loses a true record. Exit 1 drops a real theme
     change (the theme-set hook) or a manual note about an odd package.
     Today the record is kept, and the rebuild only refuses to build a
     command from it.
   - `tests/rebuild.rs` (`a_theme_name_that_fails_its_check_is_not_set`,
     and the plugin part of `names_that_fail_their_check_never_reach_a_command`)
     uses `seldon event` to put such names in. Those tests would have to
     move to ledger lines, which is outside this WP's files.

   If the orchestrator wants it anyway: refuse in `commands/event.rs`
   for `plugins plugin-*` (`is_plugin_id`), `theme theme-set`
   (`is_theme_slug`) and `pacman` kinds (`is_package_name`), and let
   the rebuild tests write ledger lines.
2. **ADR note for the widened window?** ADR-0014 §1 / ADR-0017 §2 say
   "precedes by at most 10 minutes". The spec now measures that from the
   earliest moment a capture-time change can have happened (the last
   check). I read this as the ADR's intent, not a change to it. If the
   advisor sees it as a change, it needs a superseding ADR note. A long
   gap between two captures (laptop off for days) widens the window by
   the same amount. Proof is still needed: the verb and the id since
   F-106, or the path for config.
3. The plugins follow-up under "Not done" (a "return" after a failed
   cursor save).

**Touched outside WP scope**

- `engine/src/commands/hook.rs` (fold-in b: `Scope::new`, + a unit test).
  The brief named this fold-in; no parallel WP (074 init, 075
  import/dossier/watch, 076 index) owns the file.
- `engine/src/collectors/config.rs` (fold-in a: `replay`, `since`, the
  module docs). Named by the brief, built on WP-069.
- `docs/SPEC-ENGINE.md` §4 config "Dedupe" sentence (fold-in a), next to
  the named plugins, snapper and attribution paragraphs.
- `engine/tests/support/mod.rs` and `engine/tests/common/mod.rs`: not
  touched.

## `just check`

`just check` on `8e767bc` (plus this handover and the pitfalls entry,
docs only) → exit 0, ending in `check: ok`. Cargo: 1205 tests passed,
0 failed (62 suites, with and without `watch`). clippy `-D warnings`
and fmt are clean. `docs-check: ok (391 links, 14 translated pages, 40
commands, 455 command lines)`. Plugin harnesses: panel-view 733,
overlay-view 319, bar-view 131, service-states 247, install.test 132,
real-home-guard 11, all passing. shellcheck is not installed on this
host, so only `bash -n` ran.
