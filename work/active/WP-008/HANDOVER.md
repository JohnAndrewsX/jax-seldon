WP-008 HANDOVER

Branch `wp/008-drift`, worktree `wt/WP-008`. It is **rebased onto `main` at
`8d66b35`**, with WP-009, ADR-0021 and the guard fix merged. Not pushed,
no PR.

Commits `main..HEAD`:
- `e63b599` drift commands and the reconcile module;
- `bec43a6` the capture pass;
- `8605bc8` the detached editor;
- `eb6f76f` tests;
- `a059de7` memory;
- `e7ace06` the first handover;
- then the review follow-ups: `0b8cb2d` ADR-0021 fold, `feffa5d` tests;
- then this update.

The first rebase onto `8d714a0` had two conflicts, in `lib.rs` and
`main.rs`. Both were additive, and I kept both sides. The second rebase,
onto `8d66b35`, was clean. `just check` exits 0 at HEAD.

## Review follow-ups (after APPROVE)

1. **ADR-0021: fold `case` from any resolution that carries one.**
   - `engine/src/index/build.rs` (`fold`) now copies `r.case` whenever
     it is `Some`. The `Linked` test is gone, and so is the then-unused
     `Resolution` import. The doc comment cites ADR-0021.
   - `scripts/validate-fixtures.py`: the same one rule (`if
     r.get("case")`), plus one new self-check, "explained with a case
     folds it (ADR-0021)". It adds an `explained` line with `case:
     C-2026-004` for the ollama install and checks:
     - ollama is folded as `explained` with that case;
     - ollama is no longer drift;
     - the sample's btop explained line still folds no case.

     The script also cross-checks each case's `events:` against the
     ledger. So the self-check lists the event in C-2026-004's
     frontmatter in time order, as `drift explain` does. The script now
     reports 23 self-checks, up from 22. `scripts/__pycache__` is
     removed.
   - **Negative control:** with the old `linked`-only rule restored in
     the script, the new self-check fails (the case's `events:` no
     longer match the folded ledger). I put the rule back afterwards.
   - `engine/tests/index.rs` has a new test,
     `a_resolution_folds_its_case_whether_linked_or_explained`. It
     checks:
     - an explained line with a case folds it, and the event leaves
       `drift`;
     - btop stays explained without a case;
     - a later explained line without a case wins and folds none.
   - `tests/drift.rs`: the explain test now asserts that the explained
     ollama event carries `case: C-2026-009` in the rebuilt index.
   - The fixture and the golden test are unchanged, and the golden test
     passes. The fixture's explained lines carry no case.
2. **The end-to-end config attribution test is back:**
   `a_hooked_config_edit_lands_in_the_case_file` in `tests/drift.rs`.
   - It runs `plan new` + `start`, a baseline `capture --source config`,
     and then a real `seldon hook claude-code` PreToolUse payload. The
     Bash command is an in-place `sed` of `~/.config/hypr/bindings.conf`,
     inside the temp HOME. Then the file changes and `capture` runs.
   - It checks:
     - the `config-change` carries `agent:claude-code` and C-2026-001;
     - the case's `events:` is `[hook command id, config-change id]`;
     - `agents` is `[agent:claude-code]`;
     - drift is 0.
   - The guard did not block the heredoc this time.
   - **Negative control:** without the `after_capture` call, the test
     fails (`events:` holds only the command). I restored the call.

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
- **`capture` autocommit.** `capture` still does not commit; the spec
  says "the commit helper … follow". I did not add it, because it is not
  in the brief.

## Verified by

```
$ just check   → exit 0 (rebased on 8d66b35, after the review follow-ups)
  fmt-check ok · clippy -D warnings ok · 292 tests passed across 20 test-result lines
  validate-fixtures: ok — 101 instances, 71 ledger events traced … 23 self-checks
  plugin-validate ok · qmllint ok · plugin-test ok (panel-view 130 passed) · check: ok
```

**New tests.** `engine/tests/drift.rs` has 9 tests (8 from the first
handover, plus the hooked config edit from follow-up 2), all through
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

None open. The review settled them:

1. `explain` creates a case, and its case is folded: ADR-0021, now
   implemented.
2. A stale leader id writes nothing: kept.
3. `--only` stays literal: kept.
4. The spec edits are done by the orchestrator.
5. The guard false positive: fixed on `main`. The dropped test is back
   (follow-up 2).

## Touched outside WP scope

- `engine/src/main.rs`: the `Drift` variant and one arm.
  `engine/src/commands/mod.rs`: `pub mod drift;`. `engine/src/lib.rs`:
  `pub mod reconcile;`.
- `engine/src/commands/capture.rs`: one marked call, as briefed.
- `engine/tests/commands.rs`: two tests in `mod open` for the launcher
  fix, next to the existing open/editor tests.
- `memory/rust-notes.md` and `memory/pitfalls.md`: appended.
- Review follow-up 1, as the reviewer asked:
  - `engine/src/index/build.rs`: the ADR-0021 fold line and its doc
    comment;
  - `scripts/validate-fixtures.py`: the same rule and one self-check;
  - `engine/tests/index.rs`: one test.
- I did not touch `hook.rs`, `pkgcmd.rs`, `attribution.rs`, `schema/`,
  `fixtures/`, `docs/` or `plugin/`. `Cargo.toml` and
  `Cargo.lock` are unchanged: no new crate.
