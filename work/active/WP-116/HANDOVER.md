# WP-116 HANDOVER

```
WP-116 HANDOVER
Done: ADR-0030 §6 engine, skill, docs — agent start starts like omarchy agent (caller's folder, ~/Work from $HOME or /, [agent] workdir = "logbook"), SELDON_CASE in the launch, the new prompt; hooks user-wide by default (hook install/uninstall claude-code, init --harness claude-code; CLAUDE_CONFIG_DIR), in_scope clause (b), the context's launch line, PreToolUse dedup by tool_use_id, doctor row `hooks`; rules block v4 with the WP-111 stage-2 edits and ADR-0031 (v3 renderings + hashes in RELEASED_BLOCKS); silent upgrades on record (own commit for AGENTS.md, a seldon ledger note for a skill update); skill (hand-down, aim line, Outside the Logbook Folder, snapshot.md); SPEC-ENGINE §2/§3/§8, AGENT-GUIDE, user guide en/de 04 05 06 10 11 12, README, CHANGELOG
Not done: acceptance item 7, the live check on the fresh test host (the orchestrator's, after the merge); a panel banner for the doctor `hooks` row (plugin not in this WP)
Verified by: flock /tmp/seldon-check.lock just check → exit 0 (`check: ok`) on 655aeae; 21/21 manual mutants killed; hook budget (check-perf, hooks part) within 5 ms
Decisions needed: none blocking; 14 taken (below), 4 open questions
Touched outside the WP's list: README.md, docs/user/*/10, 11, 12 (they named the old default path); engine/src/ledger.rs (parse_line), engine/src/logbook/git.rs (commit_paths)
```

Branch `wp/116-start-like-omarchy`, worktree `wt/WP-116`, from `9411005`
(main). No push, no PR.

**No contract change.** Nothing in `schema/` or `fixtures/` changed;
`contractVersion` stays 1. New JSON keys are command output only:
`hook install claude-code --json` `scope`; `doctor --json`
`hooks.installed`; `capture --json` `rulesUpdated.git`.

## Commits (oldest first)

- `d3dd072` work: WP-116 plan
- `9ade680` engine: agent start starts like omarchy agent, with SELDON_CASE
- `1f4455c` engine: hooks user-wide, served by SELDON_CASE; PreToolUse dedup
- `88e3f55` engine: doctor row hooks — user-wide, logbook only, both, none
- `7e7a459` engine: rules block v4; silent upgrades on record
- `07e4a97` skill: SELDON_CASE hand-down, Outside the Logbook Folder, root snapshot
- `65ad985` docs: agent start like omarchy agent, user-wide hooks, rules v4
- `87038c6` docs: German user guide follows
- `899a0d7` engine: PreToolUse dedup reads the month file's end, confirms by line
- `7fb6bb8` docs: SPEC-ENGINE names the PreToolUse tail search
- `655aeae` engine: test the one-day window of the tool call check
- (this file)

## Done

Engine (`engine/src/`):

- `commands/agent.rs`: `CASE_ENV` (`SELDON_CASE`) set in `launch()` next to
  `SELDON_LOGBOOK`/`SELDON_ACTOR`/`SELDON_ATTENDED` (a `Launch` struct of
  actor, case, cwd, prompt). `start_dir()`: omarchy-agent's rule — the
  caller's folder unless it is `$HOME`, `/` or not a directory, then
  `~/Work` if it is a directory, else `$HOME`; for every launcher.
  `--json cwd` and the human line name the folder used. New prompt text,
  word for word ADR-0030 §3 (with `<root>/AGENTS.md` as the path).
- `config.rs`: `[agent] workdir = "inherit" | "logbook"` (`AgentWorkdir`,
  default not written).
- `commands/hook.rs`: `in_scope` = `launched_case()` (clause b,
  `SELDON_CASE` parses as a case id) or `in_scope_by_dir` (clause a,
  unchanged). `user_settings_file()` (`$CLAUDE_CONFIG_DIR/settings.json`,
  non-empty, else `~/.claude/settings.json`) is the default of `hook
  install|uninstall claude-code`; no logbook needed; a settings file inside
  the logbook is committed (also with `--settings`). `scope_warning` →
  `scope_line`: always one line (`--json` `scope`), a warning only under
  `scope = "all"`. `claude_hooks_in()` for doctor. PreToolUse dedup (see
  D6).
- `commands/hook/context.rs`: `launch_line()` under the title for a
  session with the marker.
- `commands/setup.rs`: `init --harness claude-code` merges into the
  user-wide file and records its own write under a watched path (`seldon
  init`).
- `commands/doctor.rs`: row `hooks` and `--json` `hooks.installed`.
- `logbook/rules.rs`: `VERSION = 4`; seven v3 hashes in
  `RELEASED_BLOCKS` (D7); the tests take v2 and v3 renderings.
- `commands/capture.rs`, `commands/mod.rs`, `logbook/git.rs`: the silent
  rules upgrade is committed alone (`autocommit_paths` → `git commit --
  AGENTS.md`), `seldon: rules update (unedited, vN → vM)`; a skill update
  writes a `seldon` note (subject `skill`).
- `ledger.rs`: `parse_line()` (what `read_month` does per line), used by
  the dedup.

Templates: `engine/templates/{en,de}/AGENTS.md` → v4 with every edit the
WP lists under "Added 2026-10-06" (Omarchy first route sentence; one
program per `pkexec`, never `pkexec sh -c`; "stay in your session"; the
aim line; `root` snapshot only; Hooks section user-wide with which
sessions they serve and `hook generic --case <ID>`; `SELDON_CASE` in the
hand-down; German nits "Stufe … hoch", "der seine Rechte selbst erhöht").
`engine/templates/rules-v3/` holds the seven v3 renderings.

Skill (`engine/assets/skills/seldon/`), only the named paragraphs:
SKILL.md aim line, *Attended or Not* (hand-down with `SELDON_CASE`, "stay
in your session"), *Outside the Logbook Folder* rewritten (ADR-0030 §4;
"never leave out `cwd`" kept); `snapshot.md` (`root` only, the second
prompt counted).

Docs: SPEC-ENGINE §2 (config keys), §3 (`agent start`, `hook install`,
`doctor`, capture's silent upgrade, JSON shapes), §8 (session scope with
both clauses, install default, dedup, launch line); AGENT-GUIDE (aim,
`root` snapshot and its worked example, hooks section, hand-down);
user guide en/de 04 (set-up, sessions served, new section "Hooks of an
older logbook", start folder, three variables, other agents, skill),
05 (env vars `SELDON_CASE`, `CLAUDE_CONFIG_DIR`, doctor, agent start, hook
install/uninstall, regenerated help blocks), 06 (`[agent] workdir`), 10,
11, 12; README; CHANGELOG (Unreleased → Engine, four entries). German
source lines point at `65ad985`.

## Decisions (ADRs silent; taken and gone on)

- **D1** The launch line shows whenever the marker is valid and the
  session is served — also when clause (a) or `scope = "all"` would serve
  it anyway (a launch with `workdir = "logbook"`). It says something true
  in every such case; ADR-0030 §3 says "when served by clause (b)".
- **D2** `hook install|uninstall claude-code` need no logbook (the
  default file is the user's, not the logbook's); a file inside the
  logbook is committed whatever named it, so the doctor's tidy-up
  `uninstall --settings <logbook>/.claude/settings.json` commits.
  Before, `--settings` never committed.
- **D3** `scope_warning` became `scope_line`: one line on every install
  (`--json` `scope`), a warning (`warnings`, `warning:`) only under
  `scope = "all"` — the privacy-relevant case.
- **D4** Doctor `hooks`: "user-wide" needs all three hooks in the user
  file; 1–2 → degraded "incomplete"; a user file that is not JSON →
  degraded, `installed: "unknown"`. "none" is degraded when
  `harnesses` names `claude-code` (what "ok when no harness is
  configured" implies for the other case). `--json` gains
  `hooks.installed`.
- **D5** `start_dir` compares with `$HOME` and `/` as written and with
  links resolved (a home reached through a link is home); a path that is
  a file counts as "not a directory".
- **D6** Dedup cost (SPEC §1 budget): a `PreToolUse` searches the last
  256 KiB of each month file in range, a `PostToolUse` the whole files;
  only a line holding the id is parsed. Trade-off, pinned by a test: a
  second `PreToolUse` that arrives after more than ~256 KiB (about 500
  events) of other writes is recorded again. The pair from two settings
  files arrives within moments. Measured (bench profile, 10 000 lines,
  quiet host): recorded 1.64 ms vs 1.40 ms on main; curl lines 3.00/2.41
  ms vs 2.91/2.29 ms; 900 lines unchanged within noise (max median 4.45
  ms). A first version that read the whole month cost +1.6 ms.
- **D7** `RELEASED_BLOCKS` takes every distinct v3 block reachable from
  `main`: WP-111 stage 1 (243dac3), round 2 (9c3a7c4), the `--noconfirm`
  follow-up (0f09a3e, de only; its en block equals the merged one), and
  WP-111 as merged (070ff1f = dd41fe2) — 7 hashes, as WP-100's rounds
  were taken for v2. Only the merged pair was on main's first-parent
  line; the others cost nothing (an unedited Seldon text loses nothing).
- **D8** The silent rules commit is path-limited (`git commit -- AGENTS.md`):
  the user's staged and unstaged changes stay as they were (tested).
  `--no-commit` and `git.autocommit = false` leave it to the next commit.
- **D9** The skill note goes with the capture's other `seldon` notes
  (same append, the pending-note marker of WP-099 applies). Detail names
  the engine version and the folders.
- **D10** Skill edits limited to the paragraphs the WP names (WP-115 edits
  the skill in parallel). The rules' new "one program per `pkexec`"
  sentence is therefore not in SKILL.md's *Privileged Commands* (Q2).
- **D11** `init --harness claude-code` records its write under a watched
  path as `seldon init` (only matters when `~/.claude` is watched).
- **D12** The prompt's fallback names `<root>/AGENTS.md` (an absolute
  path, an identifier, WP-058).
- **D13** Docs beyond the WP's list (README, guide 10/11/12) changed,
  because they stated the old default path.
- **D14** `SELDON_CASE` is checked for its form only (`C-YYYY-NNN`), as the
  ADR says ("parses as a case id"); whether that case exists is not
  checked — the marker only widens the scope to the session Seldon
  launched.

## Tests

New or changed (all with a scratch HOME from `common::Env`; never the
real `~/.claude`, `~/.config`, `~/Seldon` or `~/.local/state/seldon`;
hook tests set `TZ=Europe/Berlin`):

- Acceptance 1 — `tests/agent.rs` `start_folder::*` (project folder,
  `$HOME` with/without `~/Work`, `/`, the logbook, `workdir = "logbook"`,
  an unknown `workdir`, `--new`; the four variables; `--json cwd`); unit
  `the_start_folder_follows_omarchy_agent` (links, gone, file); the two
  existing launch tests follow the new folder and prompt.
- Acceptance 2 — `tests/hooks.rs` `session_scope::a_session_seldon_launched_is_served_anywhere`
  (cwd and `CLAUDE_PROJECT_DIR` outside, marker valid: four records with
  the active case, the launch line, journal and commit, `hook generic`;
  the event follows the active case, not the marker),
  `a_marker_that_is_no_case_id_serves_nothing` (empty, `not-a-case`,
  leading blank, short, lower case × WP-063's six outside forms; inside
  without marker served without the line; `scope = "all"` with marker);
  WP-063's tests unchanged and green.
- Acceptance 3 — `post_tool_use::two_pre_tool_use_calls_for_one_tool_call_write_one_event`
  (sequential and both waiting for the lock; calls without an id),
  `a_post_tool_use_finds_its_call_behind_many_events` (D6),
  `an_old_event_with_the_id_does_not_count`.
- Acceptance 4 — `install::default_path_is_user_wide` (foreign
  `SessionStart` group, `theme`/`model` keys kept; second run writes
  nothing, mtime unchanged; no logbook commit; uninstall restores the
  foreign content exactly), `install::claude_config_dir_is_honoured`
  (also empty value, and without a logbook), `uninstall::the_logbook_file_commits`,
  `session_scope::install_says_which_sessions_are_served`;
  `tests/init.rs` `claude_code_hooks_are_installed_idempotently`,
  `german_templates_and_obsidian`,
  `omarchy_agent_kit_is_copied_and_claude_code_goes_user_wide`;
  `tests/own_writes.rs` follows.
- Acceptance 5 — `tests/doctor.rs` `doctor_says_where_the_hooks_are`
  (none, logbook only, both, user-wide, incomplete, not JSON, none with
  the harness configured), `doctor_names_the_hook_scope`.
- Acceptance 6 — `tests/skills.rs` `the_skill_has_omarchy_s_shape…`
  pins the new *Outside the Logbook Folder* sentences and that the
  WP-063 wording, "at most one password prompt" and "stay in your
  terminal" are gone; `the_skill_says_the_rules_in_the_rules_words` pins
  the hand-down with `SELDON_CASE`, "stay in your session", the aim line
  and the `root` snapshot command in rules and skill.
- Rules v4 — `logbook::rules` unit tests over every v2 and v3 rendering;
  `tests/rules.rs` v4 throughout, new `a_silent_upgrade_is_committed_alone`
  (v3 → v4, commit holds `AGENTS.md` only, staged and untracked user
  files untouched, `--no-commit`); `tests/init.rs` rule needles for v4;
  golden `init-skeleton.txt`.
- Skill note — `tests/skills.rs` `a_capture_updates_an_unedited_skill_and_nothing_else`
  (one note, not drift, once).

## Mutants (by hand, each against its tests; all restored)

| # | Mutant | Caught by |
|---|---|---|
| M1 | `in_scope` ignores the marker | `a_session_seldon_launched_is_served_anywhere` |
| M2 | any marker value counts | `a_marker_that_is_no_case_id_serves_nothing` |
| M3 | no launch line | both scope tests |
| M4 | `SELDON_CASE` not set | `start_folder::*` |
| M5 | `/` not treated like `$HOME` | `from_a_project_home_work_and_root` |
| M6 | home compared as written only | `the_start_folder_follows_omarchy_agent` |
| M7 | `workdir = "logbook"` ignored | `workdir_logbook_keeps_the_old_folder` |
| M8 | dedup for PostToolUse only | `two_pre_tool_use_calls_…` |
| M9 | PreToolUse reads the whole file | `a_post_tool_use_finds_its_call_behind_many_events` |
| M10 | install default in the logbook | `default_path_is_user_wide`, `claude_config_dir_is_honoured` |
| M11 | `CLAUDE_CONFIG_DIR` ignored | `claude_config_dir_is_honoured` |
| M12 | empty `CLAUDE_CONFIG_DIR` taken as a folder | same |
| M13 | no commit for a file inside the logbook | `the_logbook_file_commits` |
| M14 | `scope = "all"` not a warning | `install_says_which_sessions_are_served` |
| M15 | doctor "both" without the tidy-up | `doctor_says_where_the_hooks_are` |
| M16 | doctor "none" ok with the harness | same |
| M17 | rules upgrade commits everything | `a_silent_upgrade_is_committed_alone` |
| M18 | no skill note | `a_capture_updates_an_unedited_skill_and_nothing_else` |
| M19 | init writes elsewhere | `claude_code_hooks_are_installed_idempotently` |
| M20 | a v3 hash wrong | `the_released_blocks_are_exactly_the_shipped_blocks` |
| M21 | dedup without the one-day window | `an_old_event_with_the_id_does_not_count` (written after it survived) |

## Checks

- `flock /tmp/seldon-check.lock just check` → exit 0, `check: ok`, on
  `655aeae` (fmt, clippy incl. `watch`, tests, packaging, install,
  deploy, schema, docs-check, plugin validate, qmllint, plugin tests).
- `just check-perf`, the hooks part only (`--profile bench --test hooks
  -- --ignored`): within 5 ms; numbers in D6. The host was loaded by a
  parallel WP (load 8–10); one run under that load missed the budget and
  passed on the immediate repeat. The index and status benches were not
  run (untouched).
- `~/.claude/settings.json` on the dev host unchanged (mtime
  2026-10-05). `~/.local/state/seldon/{cursors,index}.json` changed at
  17:40 during the check window; the tests run with a scratch HOME under
  `SELDON_TEST_GUARD`, and the plugin tests assert the real state
  untouched, so this is most likely the installed panel's own capture —
  not verified further.

## Open questions

1. **Panel banner for the `hooks` row.** ADR-0030 §5 says "the panel's
   doctor banner shows the one-click fix"; the plugin asks only `doctor
   --only rules`. A follow-up plugin WP (an `--only hooks` or a second
   row in `--only`), or acceptable as doctor-only for 0.1.4?
2. **Skill *Privileged Commands*:** add "one program per `pkexec`; never
   `pkexec sh -c`" after WP-115 merges (kept out per the brief)?
3. **Live check (item 7):** the `pkexec` count per R2 case should now be
   two (snapshot, package); the dedup tail (D6) can be watched there with
   both settings files present.
4. **D1** (launch line also when clause (a) applies) — confirm or narrow
   to clause (b) only.

## Round 1b

Brief: `review-0.1.1/handovers/WP-116-round-1b-brief.md` (private),
orchestrator decisions on the four open questions; stage 1 reviews
`a0f4f57` in parallel.

```
WP-116 ROUND 1b
Done: (1) a capture carries logbook-only Claude Code hooks user-wide, once (as the user, never as root), with a marker so removed hooks stay removed; (3) the skill's *Privileged Commands* gains "One program per `pkexec`; never bundle privileged commands in `pkexec sh -c`."; SPEC-ENGINE §3/§8, guide 04 en/de, CHANGELOG
Not done: (2) panel banner — not needed per the brief; (4) noted for the live check; (5) D1 confirmed, nothing to change
Verified by: flock /tmp/seldon-check.lock just check → exit 0 (`check: ok`) on ebedad1; 5/5 new manual mutants killed
Guard: one read-only `grep` whose pattern held a package command was blocked by scripts/guard.sh (false positive, nothing ran); not repeated in another wording — the skill edit was made with the Read/Edit tools on the file's content
```

### Commits

- `9f95860` engine: a capture carries logbook-only Claude Code hooks user-wide, once
- `927090b` docs: the capture's one-time carry of the hooks user-wide
- `ebedad1` docs: German guide 04 follows
- (this section)

### The migration (ADR-0030 §5 made true for the new folder rule)

`hook::migrate_to_user_wide`, called by `capture`'s `upgrade_defaults`
after the rules and skill upgrades, so under the capture's lock and
after the WP-111 runner check (root and "cannot tell" skip it; the
latter with the existing warning).

- Marker `$XDG_STATE_HOME/seldon/hooks-user-wide` present → nothing.
- The logbook's `.claude/settings.json` holds none of Seldon's hooks
  (or no file, or not JSON) → nothing, no marker (doctor names the fix).
- The user-wide file (`hook install`'s default, `CLAUDE_CONFIG_DIR`
  honoured) holds at least one of Seldon's hooks → nothing added, marker
  written (doctor reports an incomplete set).
- It holds none → `merge_claude_hooks` (foreign hooks and keys kept, as
  `hook install`), own write recorded (`by: seldon capture`,
  `op: install`), marker written, one `note:` line, `--json`
  `hooksUserWide` (`~`-path).
- The user-wide file is not JSON → left alone, a `warnings` line, no
  marker (the next capture tries again after a fix).
- The logbook's own file is never touched; while both hold the hooks the
  dedup records a tool call once, and doctor offers the optional tidy-up.

Decisions (brief silent):

- **R1b-D1** The marker is per state directory (one per user), not per
  logbook: the user-wide file is per user too.
- **R1b-D2** "Present user-wide" means any one of the three hooks, so a
  user who kept only part of them is not overridden; doctor's
  "incomplete" row covers that case.
- **R1b-D3** The marker is also written when the hooks are found already
  user-wide (a fresh 0.1.4 install, or a manual `hook install`), so a
  later removal is respected in that case as well.

### Tests (scratch HOME; `tests/hooks.rs` `migration::*`)

- `once_with_the_foreign_entries_kept`: migration, the human note line,
  foreign `SessionStart` group and `theme`/`model` keys equal, the
  logbook's file byte-for-byte unchanged, marker; a second capture
  writes nothing (bytes equal); after `hook uninstall claude-code` a
  capture does not re-install.
- `json_names_the_file`: `hooksUserWide`, and `CLAUDE_CONFIG_DIR`.
- `never_as_root`: `SELDON_TEST_ROOT_PROBE=/` → nothing, no marker, the
  user file byte-for-byte; the next capture as the user migrates.
- `nothing_without_hooks_in_the_logbook_or_with_them_user_wide`: no
  hooks anywhere → no file, no marker; one hook already user-wide →
  file bytes unchanged, marker; user file not JSON → unchanged, warning,
  no marker.
- `tests/skills.rs`: the new *Privileged Commands* sentence is pinned in
  rules and skill.

### Mutants

| # | Mutant | Caught by |
|---|---|---|
| M22 | marker ignored | `once_with_the_foreign_entries_kept` |
| M23 | merged although user-wide hooks exist | `nothing_without_hooks_…` |
| M24 | no marker after migrating | `once_with_the_foreign_entries_kept` |
| M25 | migrates without hooks in the logbook | `nothing_without_hooks_…` |
| M26 | migration not called | all three other `migration::*` tests |

## Round 2

Brief: `review-0.1.1/handovers/WP-116-round-2-brief.md` (private), after
stage 1 APPROVE of `a0f4f57` with seven non-blocking findings
(`WP-116-review-1.md`); built on round 1b (`184f4e4`).

```
WP-116 ROUND 2
Done: N1 and N2 tests (both stage-1 survivors now killed); N3 PWD for the launcher; N4 narrower marker (ADR-0032, proposed) with session-end handling, docs "never set it yourself", hand-down names servers and multiplexers; N5 wizard wording; N6 skill wording; N7 no "unedited" commit over the user's own uncommitted AGENTS.md changes; scope check before setup() with a before/after bench; the round-1b migration kept as one marked call
Not done: nothing of the brief
Verified by: flock /tmp/seldon-check.lock just check → exit 0 (`check: ok`) on 343b6c2; 10 new manual mutants killed (one after a test was added)
Decisions needed: ADR-0032's acceptance (written as proposed); the round-1b migration ruling (operator, asked by the orchestrator)
```

### Commits

- `868c5da` engine: marker serves an open case only; scope before setup; PWD; own-change rules not committed
- `7ff67a6` docs: ADR-0032 (proposed) and round-2 docs
- `84e971c` docs: German guide 04 and 05 follow
- `343b6c2` engine: test that the marker's case status decides, not only its folder
- (this section)

### Per item

1. **N1** `session_scope::a_marker_that_is_no_case_id_serves_nothing`
   now also starts a session *inside* the logbook with `not-a-case`, a
   marker with a line break and `C-2099-999`: served by the folder, no
   `Launched by` line, the raw value never in the context. **N2**
   `post_tool_use::the_id_elsewhere_in_a_line_does_not_count`: an
   in-window event that holds the id as its `sessionId` (another
   `toolUseId`) does not stop the call from being recorded.
2. **N3** `launch()` sets `PWD` to the start folder.
   `start_folder::the_launcher_gets_pwd` uses an awk script as a named
   launcher (a shell stub would reset `PWD` itself) with a stale `PWD`
   in the caller: the launcher sees `~/Work` from `$HOME`.
3. **N4** `launched_case(logbook)` reads `work/active/` and serves only
   a case whose own status is active or verification. ADR-0030 §1 said
   "a value that parses as a case id"; **ADR-0032** (new, *proposed*:
   the brief is the orchestrator's decision, I did not mark it accepted
   for the operator) amends §1 clause (b) and §3, linked from ADR-0030's
   head and `DECISIONS.md`. Consequence I had to handle (R2-D1): the
   agent closes its case before its session ends, so `session-stop`
   would have lost the journal line acceptance 7 expects. It now also
   serves a call with a well-formed marker whose `session_id` has
   events in the ledger — the session that ran the case; a server's
   later session recorded nothing and stays unserved. Docs: guide 04/05
   en/de, SPEC §8, rules v4 en/de, SKILL.md, AGENT-GUIDE ("never set it
   yourself"; the hand-down now reads "another agent process, a job, a
   timer or a server that outlives your step (tmux, an editor server)").
   v4 is not released, so the block text changed without a new version.
4. **N5** wizard item: "Claude Code hooks into ~/.claude/settings.json
   (user-wide; Seldon records only logbook sessions and those it
   launches; claude-code)".
5. **N6** SKILL.md: "`hooks.scope` in `seldon doctor --json` is
   `"all"`".
6. **N7** before writing the silent upgrade, `git status --porcelain --
   AGENTS.md`; with an uncommitted change of the user's the update is
   written, not committed (`Commit::Skipped`, reason in `--json
   rulesUpdated.git`), and the `note:` line ends "; not committed:
   AGENTS.md has uncommitted changes of yours; the update goes with your
   next commit". Test `a_silent_upgrade_of_a_file_with_own_changes_is_not_committed`.
7. **Scope before setup.** `served()` opens config and logbook and runs
   `in_scope` (clause (a) first: it reads no file) before `Setup`
   (watch-path scope, skip-path globs) is built, in `hook claude-code`
   and `hook generic` (there also before the actor and `startedAt`
   checks, so an unrelated session's bad input costs nothing either).
   Bench `claude_code::fast_enough_for_an_unrelated_session` (ignored,
   `just check-perf`; budget 1 ms, process start included; 10 000 ledger
   lines; median of 20; two runs each, quiet host):

   | hook | before | after |
   |---|---|---|
   | claude-code | 706 / 724 µs | 686 / 698 µs |
   | session-start | 844 / 867 µs | 844 / 866 µs (already checked first) |
   | generic | 885 / 887 µs | 838 / 836 µs |

   What remains is the process start plus reading `config.toml` and the
   logbook's `.seldon/logbook.toml`, which the check itself needs.
8. The round-1b migration stays one call in `capture::upgrade_defaults`,
   marked with a comment; deleting that line makes it doctor-only.

### Mutants (round 2)

| # | Mutant | Caught by |
|---|---|---|
| M27 | dedup confirmation `toolUseId ==` → `true` (stage-1 R6) | `the_id_elsewhere_in_a_line_does_not_count` |
| M28 | launch line from the raw env (stage-1 R9) | `a_marker_that_is_no_case_id_serves_nothing` |
| M29 | marker: no status check | `a_closed_status_in_the_active_folder_serves_nothing` (added after it survived: the folder alone excluded closed cases) |
| M30 | marker: any well-formed id | `narrow_marker::*` |
| M31 | marker: verification not open | `a_closed_case_ends_its_session_and_serves_nothing_more` |
| M32 | session-stop without the session exception | same |
| M33 | session exception without ledger events | `narrow_marker::*` |
| M34 | no `PWD` | `the_launcher_gets_pwd` |
| M35 | rules committed despite own changes | `a_silent_upgrade_of_a_file_with_own_changes_is_not_committed` |
| M36 | rules never committed | `a_silent_upgrade_is_committed_alone` |

Not a behavioural mutant: the order "scope before setup" (only the bench
shows it).

### Note for the merge

`main` gained `3367ccb` (user-owned root probe in `common::Env`). The
round-1b test `migration::never_as_root` sets `SELDON_TEST_ROOT_PROBE=/`
on the command itself, which overrides the new default; the other
migration tests rely on the default probe being a user, which `main`
now provides under CI's root too.
