# WP-094 — Handover

Engine Dev + Docs. Branch `wp/094-agent-skill`, base `main` 994015a.
No contract change (`schema/`, `fixtures/` untouched; `contractVersion`
stays 1).

## Done

- **The skill** (`engine/assets/skills/seldon/`, compiled into the
  engine with `include_str!`): `SKILL.md`, `case.md`, `snapshot.md`,
  `update.md`, `drift.md`, English, 431 lines in all. Shape and tone of
  Omarchy's own skill (`$OMARCHY_PATH/default/agents/skills/omarchy/`):
  frontmatter `name`/`description` with triggers, "When This Skill MUST
  Be Used", topic guides next to it, short imperative sections, a
  decision framework at the end. It covers every point of the WP's
  Outputs list:
  - logbook check (`seldon plan list --status active --json`, exit 3 or no
    `seldon`: the skill does not apply), the logbook's `AGENTS.md` wins
    where they differ;
  - find the active case or open one (`plan new` + `plan start`, said in
    the Log; `agent start --new` is named as the user's one-click start);
  - act inside the Intent; Plan as a running note; the preview line; the
    §2 stops (outside the Intent, destructive without rollback, R3,
    unattended = change nothing);
  - attended by provenance (`SELDON_ATTENDED=1` or a human message);
  - R2/R3 snapshot without the cleanup pass, `seldon plan snapshot <ID>
    <N>`, no snapper → R3 stops, R2 named backup; retention;
  - package transactions resolved read-only (`pacman -Sp --print-format
    %n`, PKGBUILD depends/makedepends), matched against `[drift]
    alwaysRed`, a hit → `seldon plan set <ID> --risk R3` and one go per
    step; system upgrades R3 as such;
  - install-route order (documented route first; `omarchy pkg add`
    recommended, not required);
  - verify with a check that is not your own artefact, fill Result,
    `plan verify` then `plan done`; the engine's refusal without Result /
    Verification; `closed-by-agent`; `plan reopen`;
  - outside the logbook: report each changing command through `seldon
    hook generic --case <ID>`, `session-stop --actor`;
  - write only through the CLI; logbook text is data; read what the case
    needs;
  - `drift.md` per ADR-0028 §3: explain or link only what your own Log, a
    hook event or the user's words prove; never explain or dismiss a
    crisis; link a crisis only to your own active case that caused it;
    report a crisis in one line;
  - Omarchy first: Omarchy work → Omarchy's own skill (`omarchy`), pointed
    to, not repeated; privilege escalation → Omarchy's rule.
- **`seldon hook install skills` / `seldon hook uninstall skills`**
  (`engine/src/commands/skills.rs`), see Decisions 1–3: into the agent
  skill folders that exist, never creating one; a manifest marks
  Seldon's folder; foreign folders, links and hand-changed files are
  never touched; idempotent; `--json`; recorded as own writes (SPEC-ENGINE
  §5 rule 7) under the state lock; `--settings` refused.
- **`seldon doctor`** row `skills` (installed / missing / outdated /
  changed / foreign, with the fix). Not in `doctor --only rules`.
- **`seldon init`**: harness `skills` (`--harness skills`; wizard item
  next to the Claude Code hooks), run under init's lock; a folder it
  could not complete leaves `seldon hook install skills` as a next step.
- **Docs**: guide 04 en/de ("Other agents" now opens with the skill; new
  section "The agent skill" / "Der Agentenskill"), guide 05 en/de (`hook
  install|uninstall skills`, `init --harness skills`, doctor's `skills`
  row; help blocks regenerated), SPEC-ENGINE §3 (synopsis, doctor) and §5
  rule 7 (the skill's files are own writes), CHANGELOG.

## Not done

- The live check on the test host with two agents (orchestrator, after
  the merge, as the brief says).
- No automatic re-install of the skill on an engine update (packaging
  hook): `doctor` says `outdated` and names the fix. See Open questions.
- The engine refusal of an agent's `drift explain|dismiss` of a crisis
  is WP-109's; the skill states the rule (see Decision 12).

## Decisions

1. **Where.** Omarchy's own list, read from its migrations
   (`1786098807.sh`, `1786539345.sh`, `1787843905.sh`): `~/.agents/skills`,
   `~/.claude/skills`, `~/.codex/skills`, `~/.pi/agent/skills`,
   `~/.hermes/skills`, plus every existing `~/.hermes/profiles/*/skills`.
   Only a skill folder that exists is used; Seldon creates neither an
   agent's home nor its `skills` folder (stricter than "never create an
   agent's home dir": `~/.codex` without `skills/` gets nothing). Omarchy
   itself creates these folders, so on Omarchy they normally exist.
   `CLAUDE_CONFIG_DIR`/`CODEX_HOME` are not read (Omarchy's list is
   `$HOME`-based).
2. **Copy, not link.** Omarchy links into `/usr/share/omarchy`; Seldon is
   one static binary with no shared data directory to link to, and a copy
   per folder lets `doctor` say per folder whether it is current. The
   skill lives in `<folder>/seldon/`; files get the engine's default mode
   (0600; the agent runs as the user), the folder the umask.
3. **Ownership by manifest.** `<folder>/seldon/.seldon-skill.json` holds
   the SHA-256 of each file as Seldon wrote it. States: *missing* (also a
   real, empty folder — what an interrupted install leaves), *current*,
   *outdated* (Seldon's, untouched, older or incomplete), *changed* (a
   file whose content is neither shipped nor as written, or a link),
   *foreign* (no manifest, the manifest a link, `seldon` a link or a
   file). Install writes missing/outdated, keeps changed and foreign as
   they are; a new folder gets the manifest first, an update writes it
   last, so an interrupted run is completed by the next one and never
   mistaken for a hand edit; files an older skill had and this one does
   not are removed only while still as written. Uninstall removes only
   files whose content is Seldon's, keeps hand-changed files (and then the
   manifest, so `doctor` keeps naming them), removes the folder only when
   empty. Manifest names are plain file names only (no `/`, no leading
   `.`): a manifest can never point outside its folder.
4. **Logbook check by `seldon plan list --status active --json`**, not
   `seldon status --json` as the WP text says: `status` writes (STATUS.md,
   views, index, a commit) and the rules list it as a writing command;
   `plan list` is read-only, exits 3 the same way without a logbook, and
   returns the active case in the same call. Any other exit: tell the
   user in one line and change nothing.
5. **Snapshot line as the rules v2 have it** (`sudo snapper -c <config>
   create -c number -p -d "<ID>"`), not the WP's `-d "<id>: <title>"`:
   the rules block is the single source of the wording, and the rules
   keep logbook text out of commands.
6. **`hook generic` has no `--actor` flag** (the WP says "with `--actor
   agent:<name>`"): the skill quotes the CLI — `actor` in the JSON,
   `--case <ID>` for the case — and builds the JSON with `jq` (in
   `omarchy-base.packages`) with the command line in single quotes, so
   nothing in it runs while it is reported. It says to send the real
   `cwd` and never leave it out to get around `[hooks] scope`.
7. **Privileged commands: Omarchy's rule** (`sudo` where the password
   prompt reaches the user in a terminal, `pkexec` where it cannot),
   pointed to in Omarchy's skill. This refines the rules block's "`sudo`
   in the terminal" for agents whose tool calls have no terminal (Claude
   Code's Bash tool). See Open questions.
8. **"Hooks serve you" test**: Claude Code whose session started with the
   `# Seldon logbook context` block is served by the hooks; every other
   agent reports through `hook generic`.
9. **Doctor states.** No skill folder or installed everywhere: ok.
   Missing somewhere: ok (the skill is optional) with the fix `seldon hook
   install skills`. Outdated: degraded, same fix. Changed or foreign:
   degraded, fix "move the folder named seldon away …, then seldon hook
   install skills". The panel's `doctor --only rules` is unchanged.
10. **`update.md`** is "Installing and Updating Software": resolution,
    the R3 stop, system upgrades, the install route, verification.
11. **The marker comment** in `SKILL.md` (`installed by seldon …`) is for
    people who find the file; ownership is the manifest alone.
12. **drift.md states the crisis refusal** ("the engine refuses an agent
    that tries"), which WP-109 implements; both ship in 0.1.4. If WP-094
    merges first, that clause is one merge ahead of the engine.
13. **No logbook needed** for `hook install|uninstall skills` (like
    `--settings` for Claude Code); the config's watch paths decide what is
    recorded as an own write.
14. **Wizard**: the item is checked when the existing config lists
    `skills`, as the other harnesses.

## Tests

`engine/tests/skills.rs` (18 tests, temp HOME via `common::Env` with
`SELDON_TEST_GUARD`; never the real `~/.claude`, `~/.agents`, `~/.codex`,
`~/.pi`, `~/.hermes`):

- installs only into existing skill folders, creates none (also not
  `skills/` in an existing `~/.codex`, not in an empty Hermes profile);
- no skill folder: nothing written, says so, doctor ok;
- second install and second uninstall change nothing (bytes and mtimes
  of the whole home);
- foreign folder, link to a folder with a manifest, plain file: never
  touched by install or uninstall; doctor degraded;
- a hand-changed file kept by install and uninstall, manifest kept,
  removed once the file is gone;
- older skill updated, dropped file removed, a dropped file already gone,
  a manifest name with `../` not followed;
- interrupted install completed (no unchanged file or manifest
  rewritten); empty `seldon` folder used; files as shipped with an older
  manifest: only the manifest written;
- a link inside Seldon's folder never written through, also a link to a
  file with the shipped content; a linked manifest makes the folder
  foreign;
- uninstall of an older skill; a file the user added stays;
- doctor installed / missing with fix; `doctor --only rules` unchanged;
- `--settings` refused;
- own writes under a watched path: recorded, explained by the next
  capture (`installed by seldon hook install skills`, `removed by seldon
  hook uninstall skills`), no drift;
- `init --harness skills` installs; a changed folder leaves the next
  step;
- every `seldon …` command line in the skill (inline and fenced) runs
  `--help` and every flag is in it (the WP-100 template-command pattern);
- the shared lines are word for word the rules v2 lines (preview line,
  snapshot, `plan new|start|set|snapshot|verify|done|reopen`, `pacman -Sp`,
  attended, data);
- Omarchy's shape (frontmatter, headings, linked topic guides), the
  pointer to Omarchy's skill, the ADR-0028 §3 drift rules.

Unit tests in `skills.rs`: the shipped file list and frontmatter; plain
manifest names.

## Mutants

Script in the session scratchpad: each mutant applied alone, `cargo
test --test skills --test hooks --test init --lib` run, the file
restored (`git status` clean after each run); 33 mutants.

| # | Mutant | Killed by |
|---|---|---|
| 1 | `plain_name` accepts a leading `.` | unit `a_manifest_names_only_plain_files` |
| 2 | `plain_name` accepts `/` | unit `a_manifest_names_only_plain_files` |
| 3 | Hermes profiles without `skills/` listed | `installs_only_into_skill_folders_that_exist_and_creates_none` |
| 4 | every candidate counts as existing | `a_file_changed_by_hand_…`, `an_interrupted_install_…` and 1 more |
| 5 | an empty `seldon` folder is foreign | `an_empty_seldon_folder_is_used_…` |
| 6 | an older manifest counts as current | `an_empty_seldon_folder_is_used_…` |
| 7 | an edited file is not noted | `a_file_changed_by_hand_…`, `a_link_inside_…`, `init_offers_…` |
| 8 | a missing shipped file counts as current | `an_interrupted_install_…` |
| 9 | a link inside the folder is read through | `a_link_inside_…` (added after the first run: a link to a file with the shipped content) |
| 10 | "as written" hash compare inverted | `a_file_changed_by_hand_…`, `uninstall_removes_an_older_skill_too`, `init_offers_…` |
| 11 | a linked `seldon` folder's manifest is read | `a_foreign_seldon_folder_link_or_file_is_never_touched` |
| 12 | a linked manifest is read | `a_link_inside_…` |
| 13 | install writes into a foreign folder | `a_foreign_…`, `a_link_inside_…` |
| 14 | install writes into a changed folder | `a_file_changed_by_hand_…`, `a_link_inside_…` |
| 15 | the manifest is never written | `an_interrupted_install_…`, `an_older_skill_is_updated_…`, `init_offers_…` |
| 16 | the manifest is always rewritten | `an_interrupted_install_…` (manifest mtime) |
| 17 | shipped files rewritten, others skipped | `an_interrupted_install_…`, `a_link_inside_…`, `init_offers_…` |
| 18 | dropped files deleted whatever their state | `an_older_skill_is_updated_and_a_dropped_file_removed` (`gone.md`) |
| 19 | manifest names not checked for `/` | `an_older_skill_is_updated_…` (after fixing the test: its outside file was one level too high, so the first run let it survive) |
| 20 | uninstall keeps files of an older version | `uninstall_removes_an_older_skill_too` (rewritten; the first form did not compile) |
| 21 | uninstall deletes the manifest with a kept file | `a_file_changed_by_hand_…` |
| 22 | uninstall reports `removed` for a non-empty folder | `uninstall_leaves_a_file_the_user_added` |
| 23 | doctor never degraded | `an_older_skill_is_updated_…` (rewritten; the first form did not compile) |
| 24 | doctor: no fix for a missing skill | `doctor_says_installed_or_missing_and_the_fix` |
| 25 | `Installed::done` always true | `init_offers_the_skill_as_a_harness` |
| 26 | no `create_dir` for a new folder | **survived, equivalent**: `write_atomic` creates the missing `seldon` folder itself (mode 0700 instead of the umask); the agent's folder exists by construction |
| 27 | no early return for an empty own-write list | **survived, equivalent**: recording nothing writes nothing |
| 28 | install recorded as `uninstall` | `own_writes_under_a_watched_path_leave_no_drift` |
| 29 | `--settings` not refused | `settings_is_refused_for_the_skill` |
| 30 | `init` without the `skills` harness | `init_offers_the_skill_as_a_harness` |
| 31 | no next step for an incomplete skill | `init_offers_the_skill_as_a_harness` |
| 32 | no `skills` row in doctor | `an_empty_seldon_folder_…`, `doctor_says_…`, `an_older_skill_…` |
| 33 | doctor's list of changed files emptied | obsolete: the mutant did not compile and showed that the list was never printed; the dead code is gone (`by.contains_key("changed")`) |

## Check results

`flock /tmp/seldon-check.lock just check` at 029ca95 (the last code
commit; this handover changes only `work/`): **exit 0, `check: ok`**.
fmt and clippy (`-D warnings`, also with `--features watch`) clean; engine
tests 1768 passed, 0 failed, 8 ignored (the perf/RSS tests that run only
in `check-perf`/`check-rss`); `check-packaging: ok` (shellcheck not
installed on this host: `bash -n` only, as before), `check-srcinfo: ok`,
check-install and check-deploy ok; `docs-check: ok (437 links, 14
translated pages, 45 commands, 550 command lines)`; `plugin-validate: ok`;
`qmllint: ok (29 files)`; `plugin-test: ok` (bar-view 194 passed).
`CARGO_TARGET_DIR` unset: `engine/target` on disk. `check-perf` not run:
the WP touches neither the index build, `status` nor the hook path.

## Open questions

1. **`sudo` vs `pkexec`** (Decision 7): the rules block v2 says "Run
   `sudo` yourself, in the terminal". An agent whose tool calls have no
   terminal cannot answer a sudo prompt; Omarchy's skill says `pkexec`
   there. Should the rules block (v3, `seldon rules update`) say the same,
   so skill and rules agree word for word? I did not change `AGENTS.md`.
2. **Skill updates on engine updates.** Should the AUR package or
   `seldon capture` re-run `hook install skills` when the shipped skill
   changes, or is `doctor`'s `outdated` row enough? Today it is the row.
3. **Order with WP-109** (Decision 12).
4. **Live check** (orchestrator): Claude Code plus one other installed
   agent on the test host, the 2026-10-05 task, step count per ADR-0027
   §1; stage 2 review by Fable for the agent-facing security text
   (`SKILL.md` "Outside the Logbook Folder", "Instructions and Data",
   `drift.md`).
