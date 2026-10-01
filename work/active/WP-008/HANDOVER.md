WP-008 HANDOVER

Branch `wp/008-drift`, worktree `wt/WP-008`. It is **rebased onto `main` at
`8d714a0`**, with WP-009 merged. Not pushed, no PR.

Commits `main..HEAD`:
- `c6fd584` drift commands and the reconcile module;
- `6c64714` the capture pass;
- `ae816f3` the detached editor;
- `136bcaa` tests;
- `6692d14` memory;
- then this handover.

The rebase had two conflicts, `lib.rs` and `main.rs`. Both were additive
(WP-009's `pkgcmd`/`Hook` and my `reconcile`/`Drift`), and I kept both
sides. `just check` exits 0 at HEAD after the rebase.

## Done

### `engine/src/reconcile.rs`

This module holds the rules that write. **There is no second drift
derivation.** Every function reads a `Built` from `index::derive`:
`folded`, `open_drift`, `ledger` and `index.drift`. Grouping, the leader,
the zone, the routine class, proposals and the cap all stay in
`index/` (WP-007).

- `select(built, id, only)` picks what a command resolves:
  - the open members of the named event's item: the same `txId`,
    pacman only, oldest first;
  - with `--only`, the named event alone;
  - nothing if the named event is not open, so a re-run writes nothing.

  It exits 1 when:
  - the id is not a ULID (regex, `model::is_ulid`);
  - the id is unknown;
  - the id is a resolution;
  - the source can never be drift (ADR-0012 §6).
- `resolutions(sel, r)` builds one `resolution` line per member:
  - the same `ts`, `actor`, `detail` and `case` on every line;
  - `subject` is the target's subject, as in the fixture;
  - `refersTo` names the member;
  - `meta.txId` is set only when the write fans out over ≥ 2 members
    (ADR-0013 §4).
- `attach(file, events, ts_of)` updates the case's `events:`:
  - it inserts ids **oldest first by the events' `ts`** (ADR-0012 §10),
    not appended at the end, so a link of a 09-30 event lands before
    the 10-01 ones;
  - `seldon` events are skipped;
  - agent actors go into `agents`.
- `after_capture(logbook, ledger, written)` covers rules 1 and 2. The
  attribution pass and the pacman transaction inheritance set the cases
  before the append. This pass then records the attributed collector
  ids in their case files. It reads the ledger only from the earliest
  new `ts` onwards, for the ordering. Failures are warnings on stderr,
  because the append already happened.
- `append_to_section` writes the intent into the `## Intent` section of
  the template.

### `engine/src/commands/drift.rs`

These are the forms of CONTRACT.md. Free text goes after `--`, and options
go before `--`.

- **`seldon drift [--crisis-only] [--json]`** lists the index's drift
  items: crises first within the cap, then newest first. `--json` gives
  `{drift, openDrift, crisis}`; the totals count every open item, also
  past the cap (ADR-0020). The human output is one line per item
  (`CRISIS`/zone, time, source/kind, subject, `(+N more)`, `→ C-…?`,
  id) and then the totals. It is read-only: no lock, no index write.
- **`seldon drift show <id> --json`** returns `{event, open, item, txId,
  members}`:
  - `event` is the named event, folded (resolution and
    `resolutionDetail` included);
  - `item` is its row, or `null` when it is resolved or capped out;
  - `members` is every open member of the group, oldest first, each as
    an index event.
- **`seldon drift link <id> <case> [--only]`**:
  - checks the case first, so an unknown case is exit 1 even when
    nothing is open;
  - writes `linked` lines with `case`;
  - adds the ids to the case's `events:`.
- **`seldon drift explain <id> [--only] [--zone Z] [--risk R] [--area A]
  -- <intent>`** creates a retroactive case through WP-006's case store:
  - it uses `cases::next_id`, `cases::new_body` (the logbook's own
    template, no second template), `cases::slug`, `ensure_area`,
    `render_new` and `write_new`;
  - the title is the intent, and the intent also goes into `## Intent`;
  - the status is **completed**, in `work/completed/`;
  - `created` and `started` are the day of the earliest resolved event;
    `closed` is today;
  - the zone is `--zone`, else the item's computed zone; the risk
    defaults to R1 and the priority is normal;
  - `events:` holds the resolved ids, and `agents:` holds the agents
    involved (the actors of the resolved events, and the resolving actor
    when it is an agent);
  - the Log gets two lines.

  One ledger write holds `case-created`, the `explained` lines (with
  `case` and `detail` = the intent) and `case-completed`.
- **`seldon drift dismiss <id> [--only] -- <reason>`** writes `dismissed`
  lines with `detail`.
- **Common to every resolving command:**
  - `--actor` (default `human`, as `plan`);
  - the text must be one line;
  - one lock, then derive, select, one `emit`, the case file, the
    autocommit (`seldon: drift <verb>: N event(s)[, C-…]`, never the
    subject), and then `index::rebuild_if_initialised` (CONTRACT rule 2);
  - `--json` gives `{eventId, resolution, only, txId, resolved, events,
    case, areaCreated, git}`;
  - a no-op gives `resolved: 0, events: [], already: {resolution,
    case}`, exit 0, and writes nothing.

### `main.rs`, `capture.rs`, `open.rs`

- **`main.rs`**: one variant, `Drift`, and one arm. `commands/mod.rs`
  gets `pub mod drift;` and `lib.rs` gets `pub mod reconcile;`.
- **`capture.rs`**: one marked call,
  `crate::reconcile::after_capture(&logbook, &ledger, &written);`. It runs
  after the append and the cursor save, and before the index rebuild.
  WP-009's attribution pass still runs before the append.
- **`open.rs`, the `--editor` fix** (WP-012 decision 1):
  - Without a terminal, `omarchy-launch-editor <path>` is started by
    `launch_detached`:
    - stdin, stdout and stderr are `Stdio::null()`;
    - it gets its own process group (`process_group(0)`);
    - it is never killed and never waited for;
    - it gets the ETXTBSY retry, as in `sys::run`.
  - It is watched by `try_wait` for at most 200 ms:
    - an exit 0 in that time counts as launched;
    - a non-zero exit is the error "`omarchy-launch-editor exited with
      N`" (exit 1);
    - still running counts as `{"launched": true, "program": …}`, exit 0;
    - `NotFound` keeps the existing message.
  - The terminal path (`$VISUAL`/`$EDITOR`, attached) is unchanged.
  - `decide` uses the same function, so it is fixed too.

## Not done

- **Rule 2 beyond capture.** "A dependency of a cased explicit event is
  linked" holds wherever a case can come from:
  - at capture, by the pacman collector's inheritance (tested end to
    end);
  - afterwards, by the group fan-out of `drift link`.

  `drift link <explicit> --only` links that event alone, literally as
  ADR-0013 §4 says. Its dependencies stay open as a smaller group.
  See Decision 3.
- **Zone table.** `model::event::zone_for` and its tests are unchanged.
  WP-009 already added the green `agent` case and its test, and nothing
  else was missing for drift. The resolution lines' missing zone is
  asserted in `tests/drift.rs`.
- **Config attribution test.** An extra end-to-end test (an agent's
  in-place edit of a watched config file → the case file) was blocked by
  the guard. See Decision 5. Rule 1 for config, theme and plugins is
  covered by WP-009's `tests/attribution.rs`. My `after_capture` path is
  covered by the pacman test, which uses the same code.
- **`capture` autocommit.** `capture` still does not commit; the spec
  says "the commit helper … follow". I did not add it, because it is not
  in the brief.
- **SPEC-ENGINE §3 command lines.** They still show `drift explain <EVENT>
  "<intent>"` and `drift dismiss <EVENT> --reason TEXT`. I implemented
  CONTRACT.md: free text as a positional after `--`, no `--reason`.
  `docs/` belongs to the orchestrator.

## Verified by

```
$ just check   → exit 0 (rebased on 8d714a0)
  fmt-check ok · clippy -D warnings ok · 290 tests passed across 20 test-result lines
  validate-fixtures: ok — 101 instances, 71 ledger events traced … 22 self-checks
  plugin-validate ok · qmllint ok · plugin-test ok (panel-view 87 passed) · check: ok
```

**New tests.** `engine/tests/drift.rs` has 8 tests, all through
`common::Env` (temp HOME, temp XDG dirs).

- **`lists_the_four_fixture_items`**: `drift --json` on a fixture copy
  equals the sample's `drift` exactly:
  - THEME (proposed C-2026-005), UNIT and OLLAMA (the 2 crises), and the
    FIREFOX group (`members: 3`, yellow, `txId`);
  - `--crisis-only` gives 2 items with `openDrift` still 4;
  - the human output has 5 lines;
  - listing writes no index.
- **`show_lists_every_open_member_of_a_group`**:
  - a member id gives the group's row and the 3 members, oldest first;
  - a single item has no `txId`;
  - a resolved event (btop) shows its resolution and
    `resolutionDetail`.
- **`link_explain_dismiss_append_valid_resolutions_and_the_rows_disappear`**:
  - every ledger line validates against `event.schema.json` (`allOf`:
    `refersTo` + `resolution`, and `case` when linked);
  - the explain case validates against `case.schema.json`: completed,
    red, R2, C-2026-009, `## Intent` filled;
  - the written index and `index --check` are schema-valid, and only
    FIREFOX is left;
  - folded `case` and `resolutionDetail` are correct.
- **`a_group_resolves_in_one_write_and_a_rerun_writes_nothing`**:
  - a call on a non-leader writes 3 lines with the same
    ts/actor/detail and `meta.txId`;
  - `series.drift` W40 resolved goes up by exactly 1;
  - re-runs on any member, also with `--only`, and a `link` all write 0.
- **`only_leaves_the_other_members_open`**:
  - the leader is linked alone, with no `meta`;
  - the group comes back with leader NOTO and `members: 2`;
  - the id lands first in C-2026-004's `events:`, before the 10-01 ids;
  - then the rest goes in one write.
- **`ids_and_cases_are_checked_before_anything_is_written`**: these exit
  1, the ledger is unchanged, and no case file is created:
  - an invalid or lowercase ULID;
  - an unknown id;
  - a snapshot id;
  - a resolution id;
  - an unknown case;
  - a blank or multi-line text.

  Before `init`, the command exits 3.
- **`a_dependency_of_a_cased_explicit_event_is_linked`** (pacman
  transaction): an agent `yay -S zed` command under C-2026-001, then
  `capture --source pacman`:
  - alsa-lib (explicit false) and zed both carry the case and the agent;
  - the case's `events:` is `[command, alsa-lib, zed]`;
  - drift is 0;
  - a second capture writes nothing and leaves the case file
    byte-identical.
- **`a_caseless_install_is_one_group_and_links_as_one`**: the same
  transaction without the command is one red crisis group of 2; a link
  on the dependency resolves both.

`engine/tests/commands.rs` (`mod open`) gets 2 tests:
- **`a_launcher_that_keeps_running_is_launched_and_left_alone`**: a stub
  that sleeps 12 s:
  - `open status --editor --json` returns `launched: true` with exit 0
    in under 1 s (measured around `output()`, which would block on an
    inherited pipe);
  - the stub is its own process-group leader;
  - it is alive, not a zombie, after 10.5 s;
  - the test then kills its group.
- **`a_launcher_that_fails_at_once_is_an_error`**: exit 3 → exit 1,
  "omarchy-launch-editor exited with 3".

The existing `editor_without_a_terminal` and decide editor tests pass
unchanged.

**Negative controls.**
- With `open.rs` from `main`, the launcher test fails with exit 1 (killed
  after 10 s).
- Without the `after_capture` call, the dependency test fails: the case
  `events:` is only `[command]`.
- I restored both files and checked them with `git diff`.

**Manual smoke.** On a scratchpad copy of the fixture with a scratchpad
HOME, I ran list, crisis-only, show, the group dismiss and its re-run,
explain and link. I read the resulting ledger lines and the retroactive
case file. Nothing touched the real `~/.config`, `~/.local/state` or a
real logbook.

## Learned (in memory/)

- **`rust-notes.md`**:
  - reusing `Built` instead of re-deriving;
  - clap's optional subcommand with `args_conflicts_with_subcommands`;
  - a detached child with `process_group(0)` and null stdio;
  - the chrono overflow in `read_range`;
  - reading `/proc/<pid>/stat` in tests.
- **`pitfalls.md`**:
  - the guard reads heredoc test code;
  - stubs need absolute host tools;
  - `init` stamps `created` with the real clock, so pass `--since`;
  - test helpers must put `--json` before the subcommand;
  - an old leader id is not a group handle.

## Decisions needed

1. **`explain` creates a case: confirm the reading.** I followed the brief
   and SPEC-ENGINE §3: a retroactive case, **completed**,
   created/started on the event's day, `explained` resolution lines that
   carry `case`.
   - ADR-0012 §8 folds `case` onto the index event only for `linked`.
     So an explained event shows `resolution: explained` but no `case`,
     although the case lists it in `events:`.
   - The fixture's explained resolutions (btop, the plugins, omarchy)
     have no case; they say "deliberately without a case".
   - **Options:**
     - (a) keep this behaviour, and amend ADR-0012 §8 so that the index
       also folds `case` for `explained`;
     - (b) make the case optional, for example `explain --no-case`, or
       create one only with `--zone`/`--risk`/`--area`;
     - (c) write `linked` lines for an explain.

   I recommend (a). The plugin's "Explain" then always leaves a record
   the human can open.
2. **A stale leader id writes nothing.** After `--only` on a leader,
   `drift <verb> <that id>` resolves nothing; the rest is a new item with
   its own leader, and the plugin uses the new id. ADR-0013 §4 could also
   be read as "resolve the open members of its transaction". Confirm, or
   I change one line in `reconcile::select`.
3. **`--only` and rule 2.** `drift link <explicit> --only` does not also
   link the explicit event's dependencies. That is literally what `--only`
   says, and the fan-out without `--only` covers rule 2. Confirm.
4. **SPEC-ENGINE §3.**
   - The `drift` lines should match CONTRACT.md: `explain <id> [--only]
     [--zone --risk --area] -- <intent>`, `dismiss <id> [--only] --
     <reason>`, `link … [--only]`, `show <id>`, and `[--json]`.
   - Add the JSON shapes of `drift`, `drift show` and the resolving
     commands (listed under Done).
   - §3 should also mention that the no-terminal `open --editor` path is
     detached.
5. **Guard block (reported, not worked around).** A Bash heredoc that
   appends a Rust test to `engine/tests/drift.rs` was blocked: "write
   under ~/.config outside the jax.seldon plugin dir". The test's text
   contained an agent command string, an in-place edit of a watched
   `~/.config/hypr` file. The test itself only writes inside a temp HOME.
   - I dropped that optional test and did not rephrase or reroute it.
   - If the orchestrator wants it, either allow it, or fix the false
     positive in `scripts/guard.sh` with a `guard-test.sh` row. I add the
     test in a follow-up.

## Touched outside WP scope

- `engine/src/main.rs`: the `Drift` variant and one arm.
  `engine/src/commands/mod.rs`: `pub mod drift;`. `engine/src/lib.rs`:
  `pub mod reconcile;`.
- `engine/src/commands/capture.rs`: one marked call, as briefed.
- `engine/tests/commands.rs`: two tests in `mod open` for the launcher
  fix, next to the existing open/editor tests.
- `memory/rust-notes.md` and `memory/pitfalls.md`: appended.
- I did not touch `index/`, `hook.rs`, `pkgcmd.rs`, `attribution.rs`,
  `schema/`, `fixtures/`, `docs/` or `plugin/`. `Cargo.toml` and
  `Cargo.lock` are unchanged: no new crate.
