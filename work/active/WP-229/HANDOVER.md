# WP-229 — Handover

Branch `wp/229-app-commands` (off `app`). Not pushed. Round 2 after the
stage-1 review (SEND BACK, round 1): what changed is under *Review round 1*
near the end; the sections above are updated to match.

## What was done

- **`seldon open` (bare)**: starts the app, `$SELDON_DESK` (must be an
  absolute path, else exit 1) or `/usr/lib/seldon/seldon-desk`.
  - Without `--json`: foreground, the engine `exec`s the app (same pid, the
    app's own stdout, stderr and exit code) — for the `.desktop` file's
    `Exec=seldon open` and the terminal.
  - With `--json` (the orchestrator's addendum, for the pill): spawned
    detached with `setsid(2)` in `pre_exec` (own session and process group,
    no controlling terminal), stdin/stdout/stderr on `/dev/null`, every
    other descriptor marked close-on-exec in the same `pre_exec`
    (`close_range(3, ~0, CLOSE_RANGE_CLOEXEC)`, else `fcntl` one by one up
    to the highest open fd), watched 200 ms, never waited for or killed;
    answers `{"started": true}` (exit 0, also for an exit 0 within the
    200 ms: the hand-over to an open window) or the error object (exit 1:
    a non-zero exit or a signal within the 200 ms).
  - Missing app: exit 1, ADR-0058 §1's sentence with the path: "The Seldon
    window is not installed (<path> is missing). Install the seldon
    package." A path that is there but no regular executable file (a
    directory, a FIFO, no x bit) says that instead; an executable whose
    interpreter or loader is missing (ENOENT at exec/spawn) says "needs an
    interpreter or loader that is missing" — no editor advice.
  - No logbook, lock or config needed.
  - `setsid`, `syscall` (close_range is 436 on every Linux architecture)
    and `fcntl` are declared as `unsafe extern "C"`, the same way `sys.rs`
    already declares `fcntl`, `kill` and `mkfifo` (no new crate).
- **`seldon edit <target> [--editor]`**: what `open <target>` was, code
  unchanged (`commands/open.rs::run` with `EditArgs`).
- **`seldon open <target>`**: deprecated alias, same JSON on stdout, one
  stderr line "seldon: note: `seldon open <target>` is now `seldon edit
  <target>`; the old form goes in 0.6.0" (none under `--quiet`).
  `open --editor` without a target: clap error, exit 1.
- **`seldon skill [install [--replace]]`** (new `commands/skill.rs`;
  `skills.rs` not touched): bare prints `SKILL.md` (`--json` → `{skill,
  file, text, files}`); `install` calls `skills::install(ctx, replace)`,
  i.e. exactly `hook install skills [--replace]` (the orchestrator's
  decision). Uninstall stays on `hook`.
- **`seldon changes --around <TS> [--before 2h] [--after 30m]`** (new
  `commands/changes.rs`): ledger only, read-only (no lock, no state, no
  index file), the ledger lines of the window in time order, each with
  `class` (ADR-0058 §6); at most 500, the nearest kept, `omitted` counts
  the rest. TS: RFC 3339, local `YYYY-MM-DD HH:MM[:SS]` (or `T`),
  `@<epoch seconds>`, `now`; a date alone is exit 1 and the message asks
  for a time and names the forms (the orchestrator's decision). The DST
  overlap resolves to the first pass (summer time), the gap is exit 1.
  Durations `<n><s|m|h|d>` combined, ≤ 31d each. The window must lie in
  the years 0001–9999 (checked arithmetic; else exit 1). Exit 3 without a
  logbook. `--json` → `{around, from, to, events, omitted}`.
  - `class`: the engine's classifier (`index::class::Classifier` over the
    whole ledger, as an index build): ADR-0028's verdict of the change's
    drift item — a pacman transaction's members together, its lead as
    the build picks it; `attention = "all"` by zone as the build does —
    resolved or not, *without* the lift to attention that an open case's
    Plan gives a routine item in the index (the case's state, not the
    change's). `null` for what is no machine change (source not drift
    eligible — agent commands, cases, notes, snapshots, sessions — and
    resolution lines). Sample: ollama install `attention`, the systemd
    user unit `crisis`.
- `main.rs`: dispatch for `Open` (now `run_open`), `Edit`, `Changes`,
  `Skill`. Man page and completions are generated from clap and follow on
  their own (`tests/manual.rs` passes).
- **Contract** (`docs/CONTRACT.md`): row `seldon edit <…> --editor --json`,
  row `seldon open --json`, the `open <…>` row kept and marked as the
  deprecated alias (plugin 0.4 still runs it, and `tests/plugin/
  model.test.js` checks every plugin argv against the block); a paragraph
  on why there is no `contractVersion` bump. `decide`'s row names `edit`.
- **Fixtures** `fixtures/commands/{edit,open-app,changes-around}.json`,
  held equal by `engine/tests/app_commands.rs`
  (`SELDON_WRITE_COMMAND_FIXTURES=1` rewrites them); registered in
  `scripts/validate-fixtures.py` and `fixtures/README.md`.
- **Docs**: SPEC-ENGINE §3 (open, open <target>, edit, changes, skill;
  `hook install skills` names `skill install`); fixed the old claim that
  `open --json` returns `{"event", "git"}` (it never did; the spec now
  says `{what, path, editor}`). AGENT-GUIDE table, llms.txt, CLI reference
  en/de (new help blocks, `--write`), guide pages 01/04/07 en/de
  (`seldon edit` for paths), de source lines bumped.
- **Skill assets**: `seldon open logbook|case|<ID>` → `seldon edit …`
  (SKILL.md, case.md, triage.md); crash.md runs `seldon changes --around
  "<time>" --json` before the inbox report and names `class`. SKILL.md
  opens its first section with ADR-0058 §5's sentence verbatim: "Seldon
  exits 0 on success and non-zero on failure (3: no logbook yet; 4: try
  again)." (that paragraph re-wrapped to stay in the 120-line budget). `tests/skills.rs`: `edit` in
  the read-only list, the must-run lines say `edit`, the half-way update
  test stops at 1..=6 (triage.md now differs from 0.2.0's, so six files
  are written before the manifest instead of five).

## What was not done

- **The plugin side.** `plugin/` still runs `open <target> --editor --json`
  and has no left click to `seldon open --json`; it works against this
  engine through the alias. Switching it (and `engineMin` 0.5.0) is the
  plugin's 0.5.0 work, outside this WP's file list. `plugin/` comments and
  SPEC-PLUGIN still say `open`.
- No ADR: ADR-0058 is WP-227's (proposed). The contract rows cite it.
- CHANGELOG: not touched (the review's N6 asks for a line that the alias's
  error texts now say `edit` — `cannot edit`, `seldon edit C-…`; JSON shape
  and exit codes unchanged, plugin 0.4 only shows `error.message`). Left
  for the 0.5.0 release notes, since `[Unreleased]` is shared with the
  other `app` WPs.
- The plugin switch to `edit` and `open --json` (and `engineMin` 0.5.0)
  is a later 0.5.0 WP (the pill), by the orchestrator's decision.
- ADR-0058 §7 (help grouping "App" / "no display"): not in WP-229's
  §1–§4 and §6.
- `changes` does not re-apply the current `[redaction]` patterns to old
  lines; it shows them as written (redacted then), as `drift show` does.

## How it was verified

- `engine/tests/app_commands.rs` (23 tests), *fixture* (sample logbook) and
  temp homes: the foreground exec (same pid, exit 42 passed through, no
  args), the detached start (`/proc/<pid>/stat`: session = pgrp = pid,
  tty 0; fds 0–2 → `/dev/null`; the app outlives the engine; answer within
  2 s), an app that exits at once (error object, exit 1), a missing app
  and a directory (exit 1, names the package), a relative `SELDON_DESK`
  (refused), `edit` paths, the alias (same JSON as `edit`, one stderr
  note, none under `--quiet`), the `edit` fixture, `skill` and `skill
  install` twice (second run `unchanged`, nothing written, same JSON as
  `hook install skills`), `changes` (window, both month files, RFC 3339
  and epoch spellings, empty window, bad values exit 1, exit 3, 520 events
  → nearest 500), the logbook tree unchanged and no state dir.
  Unit tests in `changes.rs` for the TS and duration parsers. Round 2
  adds: see *Review round 1*.
- The existing `open` tests in `tests/commands.rs` and
  `tests/one_agent_per_case.rs` pass unchanged through the alias.
- Full check: `flock /tmp/seldon-check.lock env SELDON_FULL_CHECK=1 just
  check` on the dev host — result below. *desktop*/*test host*: not run
  (no app binary exists yet; the fake `SELDON_DESK` stands in).

**Full check result (dev host, Omarchy edge, Qt 6.12): not green, for
reasons outside this WP's diff** (`git diff d3c0842a -- plugin/` is empty):

- Passed, in order: runtime space, `fmt-check`, `clippy -D warnings`, the
  whole `cargo test` (every engine test, new and old), `check-watch`,
  `check-packaging`, `check-install`, `check-deploy`, `check-guard`,
  `check-runtime-dir`, `check-perf-dirs`, `check-assumptions`,
  `schema-validate` (fixtures included), `docs-check`, `plugin-validate`.
- **`qmllint` failed** (exit 255): `id-shadows-member` in
  `plugin/components/desk/EventDetail.qml:480` (`id: form`) and
  `ListColumn.qml:109` (`id: top`) — Qt 6.12's new check, exactly WP-224's
  finding; its fix is on `next`, not yet on `app`.
- `just plugin-test` run on its own afterwards (same lock): `model.test.js`
  246 passed (with the CONTRACT.md command-block tests, rule 4),
  glyphs, terminal-scripts, real-home-guard, check-tokens all passed;
  **`service-states` 364 passed, 2 failed** (`sheets-busy`,
  `sheets-rearm`: "log has errors" — `Tone.qml` cannot read `ui`,
  `themeFocus`, `focusRing`, `divider` of the edge shell's `Style`,
  `NewDecisionForm.qml:36` an undefined colour): edge shell drift in
  plugin code this WP does not touch. The harnesses after
  `service-states` did *not run*.
- So: engine and contract gates *headless*/local green; plugin gates red
  on edge for pre-existing reasons; *CI*: not run (not pushed). A rerun
  after WP-224 (and the edge `Style` fix) reaches `app` is needed before
  this can be called green.

## Review round 1 (SEND BACK) — what changed

| Finding | Fix | Test (where) |
|---|---|---|
| B1 panic on extreme `@` moments | `window()`: `checked_sub_signed`/`checked_add_signed`, the window inside the years 0001–9999, else exit 1 with a JSON error; `Ledger::read_range`'s ±1 day slack is checked too (stops at chrono's range) | `bad_values_are_user_errors`: `@8210266876799 --after 31d`, `@8210266800000 --after 0s`, `@253402300799 --after 1d`, `0001-01-01T00:30Z --before 1h` → exit 1 JSON; `9999-12-30 --after 1d` still exit 0 (*fixture*) |
| B2 no `class` | `class` per event from the engine's classifier (above); fixture `changes-around.json` rewritten | the window test asserts each event's class and that the rest equals the ledger lines; the human form shows the class (*fixture*) |
| B3 exit codes in the skill | ADR-0058 §5's sentence verbatim at the top of SKILL.md's first section | `tests/skills.rs` (line budget, must-run lines, half-way update) pass |
| N4 nearest-500 | moment at n419: kept n20…n519 (first-500 would keep n0…n499) | `a_crowded_window_keeps_the_nearest_500` |
| N4b DST overlap | `first_instant`: the offsets a day either side, each checked against the instant it gives; the earliest wins; none is the gap | `local_times_across_the_clock_changes`: `TZ=Europe/Berlin` and the same POSIX rule string, 2026-10-25 01:59/02:30/02:59:59/03:00 and 2026-03-29 01:59/02:30 (exit 1, "does not exist")/03:00 |
| N1 200 ms sentence | SPEC-ENGINE: "a non-zero exit or a signal within 200 ms is an error … an exit 0 in that time too" | `an_app_that_exits_0_at_once_has_started`; the failing-app test also covers a signal |
| N2 interpreter missing | the app's own sentence, both forms | `a_missing_interpreter_is_the_apps_error` (open and open --json) |
| N3 wording | ADR §1's sentence; "not a regular executable file" for a directory or a file without x bit | `a_missing_app_names_the_package`, `a_path_that_is_no_executable_is_said_so` |
| N5 fds | close_range CLOEXEC in the existing `pre_exec`, fcntl fallback | `an_inherited_descriptor_does_not_reach_the_app` (fd 7 dup2'd without CLOEXEC into the engine; the app sees none) |
| decision: `skill install --replace` | passed through to `skills::install(ctx, replace)` | `skill_install_replace_is_hook_install_skills_replace` (same `dirs` report as `hook install skills --replace`, file restored; no logbook: exit 3) |
| decision: date-only `--around` | refused with its own message naming the forms | `bad_values_are_user_errors` |

Round 2 verification (dev host, under `flock /tmp/seldon-check.lock`,
`CARGO_TARGET_DIR=engine/target`): `cargo fmt --check` clean, `cargo
clippy --all-targets -- -D warnings` clean, the whole `cargo test` 57
suites, 1491 passed, 0 failed (*fixture*/temp homes); `docs-check` ok.
The full `just check` was *not run* again: its plugin gates (`qmllint`,
`service-states`) stay red on edge for the pre-existing reasons below
until WP-224 reaches `app`. *CI*, *test host*, *desktop*: not run.

Docs: SPEC-ENGINE §3 (`open`, `changes`, `skill`), CLI reference en/de
(changes: a date alone is not enough, the class; skill install
`--replace`; help blocks via `scripts/docs-check.sh --write`, de source
line bumped), crash.md.

## Open questions

1. Decided: the plugin switch is a later 0.5.0 WP (the pill).
2. `seldon open` (no `--json`) from a non-terminal caller that waits (an
   old plugin build) would block until the app exits: by design (the
   addendum); the pill must use `--json`.
3. Local times in `--around` follow the engine's time zone (`TZ`, else
   `/etc/localtime`); tested with `TZ=Europe/Berlin` and a POSIX TZ rule.
4. **Subreaper (test host, later):** whether `omarchy-shell` (Quickshell)
   is a child subreaper, so that the detached app is reparented to it
   rather than to systemd --user. A test-host check of the app's ppid
   after a pill click settles ADR-0056 §4; *not run* (no app yet).
5. `class` leaves out the index's lift of a routine item named by an open
   case's Plan; if stage 2 wants `changes` to equal the desk exactly, that
   needs the cases loaded (a small change).
