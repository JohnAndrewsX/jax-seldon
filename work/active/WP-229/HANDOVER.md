# WP-229 — Handover

Branch `wp/229-app-commands` (off `app`). Not pushed.

## What was done

- **`seldon open` (bare)**: starts the app, `$SELDON_DESK` (must be an
  absolute path, else exit 1) or `/usr/lib/seldon/seldon-desk`.
  - Without `--json`: foreground, the engine `exec`s the app (same pid, the
    app's own stdout, stderr and exit code) — for the `.desktop` file's
    `Exec=seldon open` and the terminal.
  - With `--json` (the orchestrator's addendum, for the pill): spawned
    detached with `setsid(2)` in `pre_exec` (own session and process group,
    no controlling terminal), stdin/stdout/stderr on `/dev/null`, watched
    200 ms for an immediate failure, never waited for or killed; answers
    `{"started": true}` (exit 0) or the error object (exit 1).
  - Missing app (no regular file): exit 1, "the Seldon app is not
    installed (<path> is missing); install the seldon package".
  - No logbook, lock or config needed.
  - `setsid` is declared as `unsafe extern "C"`, the same way `sys.rs`
    already declares `fcntl`, `kill` and `mkfifo` (no new crate).
- **`seldon edit <target> [--editor]`**: what `open <target>` was, code
  unchanged (`commands/open.rs::run` with `EditArgs`).
- **`seldon open <target>`**: deprecated alias, same JSON on stdout, one
  stderr line "seldon: note: `seldon open <target>` is now `seldon edit
  <target>`; the old form goes in 0.6.0" (none under `--quiet`).
  `open --editor` without a target: clap error, exit 1.
- **`seldon skill [install]`** (new `commands/skill.rs`; `skills.rs` not
  touched): bare prints `SKILL.md` (`--json` → `{skill, file, text,
  files}`); `install` calls `skills::install(ctx, false)`, i.e. exactly
  `hook install skills`. No `--replace`/uninstall on the front (they stay
  on `hook`).
- **`seldon changes --around <TS> [--before 2h] [--after 30m]`** (new
  `commands/changes.rs`): ledger only, read-only (no lock, no state, no
  index), the ledger lines of the window in time order; at most 500, the
  nearest kept, `omitted` counts the rest. TS: RFC 3339, local
  `YYYY-MM-DD HH:MM[:SS]` (or `T`), `@<epoch seconds>`, `now`. Durations
  `<n><s|m|h|d>` combined, ≤ 31d each. Exit 3 without a logbook.
  `--json` → `{around, from, to, events, omitted}`.
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
  "<time>" --json` before the inbox report. `tests/skills.rs`: `edit` in
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
- No `--replace` on `seldon skill install` (not in the WP).
- `changes` does not re-apply the current `[redaction]` patterns to old
  lines; it shows them as written (redacted then), as `drift show` does.

## How it was verified

- `engine/tests/app_commands.rs` (17 tests), *fixture* (sample logbook) and
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
  Unit tests in `changes.rs` for the TS and duration parsers.
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

## Open questions

1. The plugin switch to `edit` and `open --json` (and its `engineMin`):
   which WP? The contract rows are ready for it.
2. `seldon open` (no `--json`) from a non-terminal caller that waits (an
   old plugin build) would block until the app exits: by design (the
   addendum); the pill must use `--json`.
3. Local times in `--around` follow the engine's time zone (`TZ`, else
   `/etc/localtime`); the integration tests use offsets only.
