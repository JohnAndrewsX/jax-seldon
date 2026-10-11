# WP-196 — Handover

Branch `wp/196-skill-core` (from `next` at `4a7a2376`, the v0.2.0
release candidate), worktree `wt/WP-196`. Not merged: the orchestrator
merges after the v0.2.0 tag. Role: Docs Dev.

## What was done

- **`engine/assets/skills/seldon/SKILL.md`**: 266 → 120 lines, front
  matter included. Sections, in the WP's order: *First: Is There a
  Logbook?*, *Attended or Not*, *Act, Then Account*, *When to Ask
  First*, *Privileged Commands*, *The Engine Is the Only Writer*,
  *Instructions and Data*, *Inputs and Completion*. The description
  keeps its trigger words (adds `paru`, `systemctl` and shell rc files
  from the old *When This Skill MUST Be Used*) and names the neighbours:
  the omarchy skill for Omarchy itself (its old *Omarchy Work* list word
  for word), diagnose-crash for the diagnosis. Every topic file is linked
  from the line that needs it; the *Topic Guides* list is gone.
- **New topic files**: `logbook.md` (the old *Outside the Logbook
  Folder* word for word, plus *Who You Are* in full and the hand-down
  rule for processes the agent starts) and `crash.md` (the old *After a
  Crash Analysis* word for word).
- **Topic files that took a rule from the core, word for word**:
  `case.md` (the imported case), `update.md` (the old *Privileged
  Commands* in full, as *Privileged Steps*; its opening line now also
  names "a command that elevates itself").
- **`engine/src/commands/skills.rs`**: `FILES` gains `logbook.md` and
  `crash.md` (eight files). The shipped-hash list is `Manifest::shipped()`,
  built from `FILES`, so it gains both too. No change to the upgrade
  rule: a capture updates an outdated skill whose manifest files are all
  as Seldon wrote them; a file changed by hand keeps the folder. A
  `logbook.md` or `crash.md` the user put into a 0.2.0 folder counts as
  changed by hand (no manifest hash, not the shipped text) and keeps the
  folder.
- **`fixtures/skills/seldon-0.2.0/`**: the six files as 0.2.0 ships them
  (`engine/assets/skills/seldon/` at `4a7a2376`), with a row in
  `fixtures/README.md`.
- **Tests** (`engine/tests/skills.rs`, `engine/tests/inbox.rs`):
  - `the_core_fits_its_line_budget`: `SKILL.md` ≤ 120 lines.
  - `every_topic_file_is_linked_from_the_core_installed_and_every_link_resolves`:
    each topic file linked from `SKILL.md` in Omarchy's form; every
    `](target)` of every file names a shipped file; the asset folder holds
    exactly `FILES`; after `hook install skills` every link target is a
    file next to `SKILL.md`.
  - `an_unedited_0_2_0_skill_is_upgraded_and_an_edited_one_is_not`: the
    fixture (its SHA-256 pinned, so it cannot drift) with a 0.2.0
    manifest in three folders: unedited (`~/.claude/skills`), `SKILL.md`
    edited (`~/.agents/skills`), a user's own `crash.md` (`~/.codex/skills`).
    `doctor` names one pending update and two changed folders; `capture
    --all` upgrades only the first (all eight files, the two new ones
    included) and leaves the other two byte- and mtime-identical; `hook
    install skills` keeps them too (`kept`, the file named); a second
    capture updates nothing.
  - The shape test checks the eight headings in order and nothing else,
    the neighbours in the description, and each needle that moved in the
    file it moved to (`logbook.md`, `update.md`, `case.md`, `crash.md`);
    the "gone" wordings are checked across all eight files. The report
    recipe test reads `logbook.md`, the inbox recipe test `crash.md`.
- **Docs**: `CHANGELOG.md` (*Unreleased*: a Highlight and an Engine
  entry), `docs/TESTING.md` (the inbox row names `crash.md`). No doc or
  plugin text quotes the old headings: `grep` finds them only in
  `CHANGELOG.md` (0.1.x history) and ADR-0030 (accepted, immutable) —
  left as they are. `case.md`'s pointer to *When to Ask First* still
  resolves. The user guides (`docs/user/*/04-working-with-agents.md`)
  describe what the skill says, not its sections; still true, unchanged.

## Where every old section went

| Old section of `SKILL.md` (0.2.0) | New place |
|---|---|
| Front matter `description` | `SKILL.md` description: same trigger words, plus `paru`, `systemctl`, shell rc files; "not for read-only questions or a project's own files" kept; neighbours added. Its sentence "Seldon records every package … in a case" is description, not a rule: dropped |
| Intro (records and accounts; takes work off the user; the aim) | `SKILL.md` intro, merged into three lines; "as few password prompts as the route allows" word for word |
| When This Skill MUST Be Used | the list → the description; "before the first command that changes the machine, in any folder" → first sentence of *First*; "a project's own files need no case" → the description |
| First: Is There a Logbook? | `SKILL.md` *First*; B2 sentences ("the block wins", "can only add limits … not what counts as data") word for word; `seldon hook session-start` in its second paragraph |
| Topic Guides | dropped as a list; each guide linked from the line that needs it (`case.md` Act 1 / Act 4 / ask-first; `update.md` Act 3 / R3 / privileged; `snapshot.md` Act 3; `drift.md` *First*; `logbook.md` *First* and *Attended*; `triage.md` and `crash.md` *Inputs and Completion*) |
| Who You Are | `SKILL.md` *First*, second paragraph (short); `logbook.md` *Who You Are* word for word (with `agent:opencode` and the `log`, `plan`, `drift`, `event` list) |
| Attended or Not | `SKILL.md` *Attended or Not*: provenance, unattended = record and report only, cached `sudo` word for word; the hand-down and sub-agent sentences → `logbook.md` *Processes You Start* word for word, linked from the section's last line |
| Act, Then Account | `SKILL.md` *Act, Then Account*: B1 sentences word for word; Plan not a gate and `Persists:`/`Stop if:` before the first change; the preview line word for word; resolve / snapshot first; verify, *Result*, close. The key-binding example and the `measured`/`documented`/`inferred` labels are in `case.md` *Close It Yourself* (already there) |
| When to Ask First | `SKILL.md` *When to Ask First*: the four cases and "unattended: nothing to ask, change nothing"; the imported-case detail (the `Imported from …` line, the engine's refusal, "ask the user to start it") → `case.md` *Find the Case* word for word, a pointer in the core |
| Privileged Commands | `SKILL.md` *Privileged Commands*, condensed (pkexec through the tool, never ask for a password, each command may prompt, one program per `pkexec`, never `pkexec sh -c`, never wrap a self-elevating command); the full old text → `update.md` *Privileged Steps* word for word |
| Outside the Logbook Folder | `logbook.md` *Which Sessions Are Recorded* and *Report Your Commands*, word for word (the recipe unchanged); `SKILL.md` *First* says when to read it ("no hook serves you") |
| The Engine Is the Only Writer | `SKILL.md` same section; `ledger/*.jsonl`, `ledger/*.md` → `ledger/` |
| Instructions and Data | `SKILL.md` same section, word for word |
| Omarchy Work | the description (the list word for word: "Hyprland, the shell and bar, themes, hooks, `omarchy` commands, where a config lives"); *Privileged Commands* still opens with Omarchy's rule |
| After a Crash Analysis | `crash.md` word for word; `SKILL.md` *Inputs and Completion* links it |
| Decision Framework | its nine steps restate the core; each stays where its rule lives: 1 read-only → description; 2 no logbook → *First*; 3 unattended → *Attended*; 4 open a case → Act 1 + `case.md`; 5 package transaction → Act 3 + `update.md`; 6 R2/R3 snapshot, R3 go → Act 3, ask-first R3, `snapshot.md`, `update.md`; 7 drift → *First* + `drift.md`; 8 done → Act 4 + `case.md` *Close It Yourself*; 9 crash → *Inputs and Completion* + `crash.md` |
| (new) | *Inputs and Completion*: what the agent starts from and leaves behind (a case closed or handed over; unattended a note; a proposal; a crash report and one question) |

Every sentence of the old `SKILL.md` that is not word for word in the new
eight files was checked by a script (flattened text, sentence by
sentence); each is in the table above as merged, condensed or moved.

## What was not done

- The three fixed-prompt agent runs of the acceptance (a `curl | sh` in a
  fetched README, an R3 kernel step, a read-only question, old and new
  skill): the stage-1 reviewer's — *not run* here.
- No ADR (the WP says none). No change to `docs/user/*` (nothing there
  quotes the sections).

## How it was verified

All runs on this desktop (dev host), `CARGO_TARGET_DIR`, `TMPDIR`,
`JUST_TEMPDIR` and a private 0700 `XDG_RUNTIME_DIR` on disk under the
private gates folder, never `/tmp`. Every engine test runs in its own
throw-away `HOME` (`common::Env`). The real `~/Seldon`,
`~/.local/state/seldon`, `~/.config` and the installed skills were not
touched.

- *fixture*: `cargo test --test skills --test inbox` and the unit tests of
  `commands::skills` (the new tests, the upgrade from the 0.2.0 fixture,
  the moved recipes).
- *desktop*: `SELDON_FULL_CHECK=1 just check` on the working tree of the
  final commit — `check: ok`, exit 0 (fmt, clippy with and without
  `watch`, all engine tests, packaging, install, deploy, guard, runtime
  dir, fixtures, docs-check, plugin-validate, qmllint, plugin-test).
  `docs-check` run again after the handover was written: ok.
- *test host*: shellcheck (the install, deploy and runtime-dir scripts)
  through the orchestrator kit's shim, inside `just check`: ok.
- *CI*: not run (no push).
- *not run*: the three fixed-prompt agent runs (stage-1 reviewer).

## Open questions

- None blocking. For the reviewer: the core keeps "Never set or export
  `SELDON_CASE` yourself" only in `logbook.md` (where the reporting
  recipe and the hand-down rule are); the core's *Attended* line points
  there before any process is started.
