WP-006 HANDOVER

Branch `wp/006-commands`, worktree `wt/WP-006`. Not pushed, no PR.
Rebased onto `main` at `aae3ef1` (WP-004 foundation). Commits `main..HEAD`:
`b442cb2` case store, journal, templates · `792b38e` commands and `--config` ·
`7309d51` emit through `Ledger::append` · `716aa0c` tests · `3849a4a` docs and
memory · `51c3447` whole-second timestamps · plus this handover. `just check`
exits 0 at HEAD.

## Done

- **Rebase on the WP-004 foundation.** `git rebase main` was done as the
  orchestrator asked.
  - Conflicts were only in `Cargo.toml`/`Cargo.lock`, where both sides added
    `ulid` and `jsonschema`. I took main's manifest, so there are no
    dependency changes of my own.
  - `commands/mod.rs` and `main.rs` were resolved additively: `Capture` plus
    my five variants, with one dispatch arm each.
  - I then switched `commands::emit` to `Ledger::append` with
    `model::event::Event`, as FOUNDATION.md shows, and deleted my minimal
    JSONL writer (my own ULID generator, month file and redaction gap).
  - `emit(lock, config, logbook, events)` builds the `Ledger` with
    `Redactor::with_patterns(config.redaction.patterns)`. The ledger assigns
    ids, redacts, cuts and validates. Nothing is redacted twice.
- **`seldon log <text> [--case ID] [--actor human|agent:N] [--tag T]…`.**
  Writes a `manual/note` event in the fixture's shape: subject is the case
  id, or `journal` when there is no case; `detail` is the text.
  - Appends a journal entry `## HH:MM · actor · case?` with the same time.
    A day's file is created with canonical frontmatter.
  - On an existing day, only the text is appended, plus the case id added to
    `cases:` when it is new.
  - A pasted line that looks like an entry heading is escaped with `\`.
  - CRLF days stay CRLF.
  - With `--case`, the note's id goes into the case's `events` and an agent
    actor into `agents`. The case body is not touched.
- **`seldon event <source> <kind> --subject S [--detail D] [--case ID] [--actor A] [--meta k=v]…`.**
  - Refused with exit 1: source `seldon`, and the kinds
    `case-*`, `resolution`, `correction`. Those belong to `plan`/`drift`.
  - The zone comes from `zone_for` (ADR-0014 §2).
  - `--meta` fills the typed keys: `pairOf` is a number, `enabled` a bool,
    and `txId` is refused. Anything else goes to `meta.extra`.
  - The case is attributed as in `log`.
- **`seldon plan new|start|verify|done|drop|list|show`.**
  - **`new`** writes `work/queued/C-YYYY-NNN-slug.md`.
    - Canonical frontmatter, and the body from `.seldon/templates/case.md`
      (the logbook's copy if it exists, else the built-in one in the
      logbook's language).
    - The Log line `created (zone Z, risk R) · actor`, and a `case-created`
      event whose detail is the title.
    - An area is created under `areas/` on first use.
    - Ids: per year, max+1 over all three work folders and the `work/C-…/`
      workpiece folders, so an id is never reused.
    - The slug transliterates umlauts and is at most 40 bytes.
  - **State machine:** `queued → active → verification → completed`, and
    queued/active/verification `→ dropped`.
    - A refused step exits 1 with e.g. `C-2026-004 is active; `seldon plan
      done` needs a case that is verification; run `seldon plan verify`
      first`, and writes nothing.
    - Each step changes frontmatter keys losslessly (`status`, `started`,
      `closed`, `snapshotBefore`, `agents`).
    - It appends one Log line after the last non-blank line of `## Log`.
      Code fences are respected, and a section is created if one is missing.
    - It moves the file per ADR-0012 §9 (rename, then atomic write; never
      two files) and emits `case-started|verified|completed|dropped` with
      `--reason` as detail.
  - **`start`** writes `.seldon/active-case` and `--snapshot N` sets
    `snapshotBefore`. `--snapshot` with any other step is exit 1.
  - **`done`/`drop`** clear `.seldon/active-case` only if it names the case.
    `done` appends the journal stub `Case completed: <title>` (German:
    `Case abgeschlossen: …`).
  - **`list [--status --area]`** and **`show`** give text, or JSON in the
    `case.schema.json` index shape (with `path` and `steps`).
- **`seldon decide <title> [--case ID] [--no-edit]`.**
  - Writes `decisions/ADR-NNNN-slug.md`: status `proposed`, today's date,
    `cases`, and the body from `.seldon/templates/decision.md`.
  - The editor opens after the lock is released.
- **`seldon open <case|journal|ledger|status|logbook|C-…|ADR-…> [--editor]`.**
  - It prints the path.
  - `case` is the active case. `ledger` is the month's generated `.md` view
    if one exists, else the `.jsonl`.
  - `--editor` passes the path as one argument:
    - on a terminal: `$VISUAL`, then `$EDITOR` (split at whitespace, no
      shell), then `omarchy-launch-editor --inline`;
    - without a terminal (the plugin): `omarchy-launch-editor <path>`.
  - `journal --editor` creates the day file first.
- **Git autocommit.** `commands::autocommit` runs `git add -A` and then
  `git commit -m "seldon: <summary>"`.
  - It runs only when `git.autocommit` is on, `--no-commit` is not given,
    and the logbook is a repository.
  - Summaries: `C-2026-001 created|active|verification|completed|dropped`,
    `note [C-…]`, `event <source>/<kind> <subject>`, `ADR-0001 proposed`.
  - A commit failure is reported in the output (`git.error`) but is not
    fatal.
  - `commit_all` no longer fails when nothing is staged.
- **Templates.** `engine/templates/{en,de}/.seldon/templates/{case,decision}.md`.
  - Section headings are English in both languages, because the engine
    parses `## Plan` and `## Log` (as the German fixture does); the hint
    comments are translated.
  - `init` now writes them, so `.seldon/templates/.gitkeep` is gone.
- **WP-003 follow-ups.**
  - The global `--config FILE` and `SELDON_CONFIG`: `--config` wins over the
    variable, which wins over the XDG default. `init`, `doctor` and
    `capture` all read the config through it.
  - `wants_json` now tracks positional slots the way clap does, with unit
    tests in `main.rs`.
  - `tests/cli.rs` runs through `common::Env`.
- **CONTRACT `--` forms.** `seldon log [--case <id>] -- <text>`,
  `plan new --zone z --risk r -- <title>` and `decide --no-edit -- <title>`
  are accepted and tested. The forms without `--` work too, for humans, as
  long as the text is not exactly a known option.
- **Tests: 130.**
  - `plan::` 11, `log::` 10, `journal::` 3, `event::`/`decide::`/`open::`
    9, `cli` 17.
  - Unit tests: state machine, Log append edge cases, slugs, `fill`,
    `wants_json`, meta, tags.
  - Every ledger line any test writes is validated against
    `schema/event.schema.json` (jsonschema, with formats). Case JSON is
    validated against `case.schema.json`.
  - The fixture logbook is used only as a copy.

## Not done

- **No index rebuild after writes.** CONTRACT rule 2 has no builder yet
  (WP-007). The plugin will see these changes only after `seldon index`
  exists.
- **`capture` does not commit yet.** `commands::autocommit(ctx, &config,
  &logbook, summary)` is ready to call after `ledger.append` (FOUNDATION.md
  leaves that to WP-004/005).
- **Generated views are not touched.** `decide` does not regenerate the
  `decisions.index` fence in `DECISIONS.md`, and nothing writes
  `ledger/YYYY-MM.md` or `STATUS.md` (status/index track).
- **No `engine/src/git.rs`.** The brief named that file, but the git helpers
  already live in `logbook/git.rs` (WP-003). The autocommit policy is
  `commands::autocommit` on top of `logbook::git::commit_all`; I did not add
  a second git module.
- **The interactive editor path is not tested automatically.** That is
  `$EDITOR` on a terminal, which needs a pty. The no-terminal path
  (`omarchy-launch-editor`) is tested with a stub. I did not launch a real
  editor.
- **`seldon log --case` adds no line to the case's `## Log`.** This matches
  the fixture: the 17:00 note on C-2026-004 is not in its Log.

## Verified by

```
$ just check                          → exit 0
  fmt-check ok · clippy -D warnings ok · test: 130 passed (11 suites)
  validate-fixtures: ok — 94 instances, 67 ledger events traced …
  plugin-validate: ok · qmllint: ok (5 files) · plugin-test: ok · check: ok

# manual, HOME/XDG_CONFIG_HOME/XDG_STATE_HOME redirected to the scratchpad
$ seldon init --non-interactive --path <scratch>/lb
$ seldon plan new --zone red --risk R2 -- "Zed installieren"   → 0, work/queued/C-2026-001-zed-installieren.md
$ seldon plan start C-2026-001 --snapshot 112                  → 0, moved to work/active/
$ seldon log --case C-2026-001 -- "--help"                     → 0, note text "--help"
$ seldon plan done C-2026-001                                  → 1, "… run `seldon plan verify` first"
$ seldon decide --no-edit -- "Zed statt VS Code"               → 0, decisions/ADR-0001-zed-statt-vs-code.md
$ seldon plan verify C-2026-001 --json ; seldon plan done C-2026-001 --json   → 0, 0
$ git log --oneline   → init, C-2026-001 created, active, note C-2026-001, ADR-0001 proposed, verification, completed
```

- This manual run found the bug fixed in `51c3447`: without `SELDON_NOW`,
  `ts` carried nanoseconds. The test `log::the_real_clock_writes_whole_seconds`
  now covers it.
- `~/.config/seldon` and `~/.local/state/seldon` do not exist on the dev
  host afterwards.
- Nothing ran on the test host.

## Learned (in memory/)

- `rust-notes.md`:
  - command `Args` structs live in the command module, so `main.rs` adds one
    line per command;
  - one clock per invocation (`Context::now`);
  - ledger first when the id must be recorded;
  - clap: a known flag always beats a hyphen-accepting positional, so the
    text `--json` needs `--`;
  - `wants_json` mirrors that rule;
  - clippy `cloned_ref_to_slice_refs`;
  - jsonschema 0.58 with formats, and inlining the cross-file `$defs`.
- `pitfalls.md`:
  - test helpers must respect code fences;
  - strict `done` versus the fixture;
  - plugin free text goes after `--`;
  - `--editor` without a terminal;
  - the Cargo conflict when rebasing onto WP-004.

## Decisions needed

1. **The fixture contradicts the state machine.** `C-2026-001` went
   `active → completed` without `verification` (Log and ledger:
   created, started, completed). SPEC-LOGBOOK §3 says the transitions are
   engine-enforced, so the engine refuses `done` from `active`. I did not
   edit the fixture. Options:
   - (a) keep it strict, and have the Schema Keeper add the missing
     verification to the fixture;
   - (b) allow `done` from `active`, as an implicit verification.
2. **`* → dropped` is read as "any open case".** A completed or dropped case
   cannot be dropped (exit 1). Please confirm.
3. **`plan new` defaults are my choice:** `--zone yellow --risk R1
   --priority normal`. The spec is silent; the plugin always passes zone and
   risk.
4. **Autocommit commits the whole work tree** (`git add -A`), not only the
   files the command wrote. So edits made in Obsidian since the last command
   go in under that command's message. That is how `init` already worked;
   the logbook's history doubles as its backup. The alternative is a
   path-limited commit, which leaves editor-only edits uncommitted until the
   engine touches that file again.
5. **Additions not in SPEC-ENGINE §3**, so the spec should list them or I
   remove them:
   - `--actor` on `plan new|start|verify|done|drop`, so agents can identify
     themselves.
   - `log --tag`: `meta.tags` comma-joined in `meta.extra`, plus a `#tag`
     line in the journal.
   - `open logbook|C-…|ADR-…`.
   - `SELDON_NOW` (a clock override for tests and demos) and
     `SELDON_CONFIG`.
   - The JSON shapes of `log`, `event`, `plan *`, `decide` and `open`. Each
     has `event` (the ledger line), `git`, and for plan steps `from`, `to`,
     `movedFrom`, `activeCase`, `journal`. Should §3 fix them like `doctor`'s?
6. **Language of engine-written prose.** The `done` journal stub follows the
   logbook language (`Case completed:` / `Case abgeschlossen:`). Case Log
   words stay English (`created`, `started`, `verification`, `completed`,
   `dropped: <reason>`), as in the German fixture. Please confirm.
7. **`decide` writes no ledger event.** No event kind fits, and adding one
   (`decision-created`) needs an ADR and a contract bump.
8. **Several active cases.** `.seldon/active-case` names the case started
   last. `done`/`drop` of another active case leaves the marker alone.
   SPEC-ENGINE §5 rule 1 links agent collector events to "the" active case.
   Is "last started" the intended meaning?
9. **`open --editor` failure is exit 1**, with the message naming
   `$EDITOR`/`omarchy-launch-editor`. For `decide`, the ADR already exists,
   so an editor failure is reported in `editor.error` and the exit is 0.
10. **`snapshotBefore` only comes from `--snapshot N`.** `plan start` could
    take the newest snapper snapshot from the ledger when no `--snapshot` is
    given. I left that out; say if you want it.

## Touched outside WP scope

- `engine/src/main.rs`: the `--config` global, five variants with one arm
  each, `wants_json`, and its unit tests.
- `engine/src/commands/mod.rs`: `Context` (`config_file`, `now`),
  `open_logbook`, `lock`, `autocommit`, `write_new`.
- `commands/init.rs` and `commands/doctor.rs`: one line each,
  `ctx.config_file`.
- `logbook/git.rs`: `commit_all` treats nothing staged as success.
- `logbook/layout.rs`: no `.gitkeep` in `.seldon/templates`.
- `logbook/templates.rs`: the two body templates and `find`.
- `sys.rs`: `run_attached`.
- `tests/common/mod.rs`: `stub`, `init_logbook[_at]`, `at`, `copy_dir`,
  `ledger` (schema-validated), `assert_valid_case`, `find_file`.
- `docs/TESTING.md`: the engine section.
- `memory/rust-notes.md` and `memory/pitfalls.md`: appended.
- `schema/`, `fixtures/`, `scripts/` and `plugin/` were not touched.
