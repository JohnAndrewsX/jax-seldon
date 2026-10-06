# ADR-0030 — The Seldon agent starts like the Omarchy agent: from the caller's folder, hooks user-wide, served by a launch marker

**Status:** accepted (operator decision 2026-10-06: the Seldon agent starts like the Omarchy agent, from `~/Work`; this design confirmed as recommended, including Seldon's hooks in the user-wide Claude Code settings). Ships in 0.1.4 with WP-116.
**Date:** 2026-10-06

> Amended in part by [ADR-0032](ADR-0032-launch-marker-open-case.md)
> (proposed, 2026-10-06): clause (b) serves only a marker that names an
> open case of the logbook; §3's launch line likewise.

> Extends [ADR-0027](ADR-0027-act-then-account.md) §6 (the
> one-click start) and §8 (work outside a Seldon-started session). Amends
> WP-063's session scope (SPEC-ENGINE §8) by one clause and the default
> settings file of `seldon hook install claude-code` (SPEC-ENGINE §3, §8).
> WP-096 (`SELDON_ACTOR`, `SELDON_ATTENDED`), WP-094 (the skill), WP-058
> (identifiers, never logbook text, in prompts), ADR-0007 stand. No
> contract change.

## Context

`seldon agent start` (`engine/src/commands/agent.rs`, `launch`) runs the
launcher with `current_dir(logbook.root)` and sets `SELDON_LOGBOOK`,
`SELDON_ACTOR`, `SELDON_ATTENDED=1`. Omarchy's own launcher,
`$OMARCHY_PATH/bin/omarchy-agent`, runs the default agent in `$PWD`, moves
from `$HOME` to `~/Work` when that exists ("agents refuse to remember trust
for `$HOME`"), adds each agent's "don't stop to ask" flag (`claude
--permission-mode auto`, `codex --approve-for-me`, `opencode --auto`, …)
and opens its own terminal window (`omarchy-launch-tui --app-id=
org.omarchy.agent`); `omarchy agent prompt <text>` passes the prompt
through. Seldon's default launcher is exactly `omarchy agent prompt
{prompt}` (`DEFAULT_AGENT_LAUNCHER`), so today the only difference is the
starting folder: the logbook.

The folder is load-bearing for the record. Seldon's Claude Code hooks are
installed by default into `<logbook>/.claude/settings.json` — project
settings Claude Code reads only when the session's project is the logbook —
and the session scope of WP-063 serves only sessions whose
`CLAUDE_PROJECT_DIR` (else payload `cwd`) is inside the logbook. Started
from `~/Work`, neither holds: no `# Seldon logbook context` block at
session start, no command recording, no journal line at session end. The
agent Seldon launched would leave the same trace as a stranger's — drift,
as in ADR-0027's side-by-side observation. The logbook's `AGENTS.md` is
also read by proximity; from `~/Work` the agent has the skill (WP-094,
installed into the agents' skill folders) and whatever the prompt and the
context block carry.

Facts checked (read-only, dev host): `in_scope` (`hook.rs:716`) is one
function used by `hook claude-code`, `hook generic`, `session-start` and
`session-stop`; `merge_claude_hooks` (`hook.rs:1380`) parses the whole
file, adds only the three Seldon hooks that are missing, keeps every other
key and hook, writes only when something was added (pretty, sorted keys;
the operator's user-wide file today holds a foreign `SessionStart` hook
and `model`/`theme` keys, which it would keep); `settings_file`
(`hook.rs:1762`) defaults to the logbook's file; `scope_warning` already
explains a user-wide install; `unless_recorded` dedups by `tool_use_id`
for PostToolUse only (`hook.rs:817`); `hook claude-code` and
`session-stop` do not read `SELDON_ACTOR` (WP-096); the engine reads
`SELDON_ATTENDED` nowhere (SPEC §8); WP-096's live check showed the
`SELDON_*` variables reach the agent process through `omarchy-launch-tui`
→ uwsm → Alacritty and foot (not through a terminal server, guide 04 A1).
The skill's "Outside the Logbook Folder" (`SKILL.md:142`) describes the
WP-063 scope and says never to omit `cwd` to get around it. Omarchy's
`omarchy-agent-crash` prompt names its skill and the skill file's path as
the fallback for harnesses without a skill mechanism — the pattern for
Seldon's prompt.

Assumptions, named: (A1) Claude Code runs hook commands as child processes
that inherit the agent's environment, so a variable the launcher set
reaches `seldon hook …` (WP-116's live check confirms; the e2e test pins
it with a stub); (A2) Claude Code deduplicates identical hook commands
found in two settings files — the engine does not rely on it (§5); (A3)
the panel's `Process` cwd is `$HOME` or `/` (unknown; §2's rule covers
both); (A4) ADR number 0030 and WP-116 are free.

## Decision

### 1. Where the hooks live, and whom they serve

**User-wide by default.** `seldon hook install claude-code` and `seldon
init --harness claude-code` merge the three hooks (`PreToolUse`,
`SessionStart`, `SessionEnd`, unchanged) into `~/.claude/settings.json`
(`CLAUDE_CONFIG_DIR` honoured when set, else `~/.claude`), with the
existing merge: foreign hooks and keys kept, idempotent, written only when
something is missing, recorded as the engine's own write under a watched
path (§5 rule 7), under the state lock. `--settings FILE` still overrides;
the logbook's `.claude/settings.json` remains a valid target. `uninstall`
mirrors it.

**Scope = logbook, or launched by Seldon.** `seldon agent start` sets one
more variable, **`SELDON_CASE=<case id>`**, in the launched agent's
environment. `in_scope` serves a call when (a) `CLAUDE_PROJECT_DIR` (else
payload `cwd`) is inside the logbook — unchanged — **or** (b) the hook's
environment carries `SELDON_CASE` with a value that parses as a case id.
Everything else is ignored as today: no event, no context block, no
journal line, exit 0. `[hooks] scope = "logbook"` keeps its name and
default and gains clause (b) in its definition; `"all"` is unchanged. A
user's unrelated Claude Code sessions carry no marker and are never
recorded: the user-wide hooks fire, the engine stays silent (< 1 ms, the
existing early return).

Why a new variable and not `SELDON_LOGBOOK`, `SELDON_ACTOR` or
`SELDON_ATTENDED`: the first two are legitimately exported by a user in
their own shell (a non-default logbook path; a fixed actor), and reading
them as "Seldon launched this" would record every session of that user;
the third is defined as the agent's signal that the engine never reads
(SPEC §8), and a session can be attended without any variable (a human
message). `SELDON_CASE` says exactly one thing. The skill's hand-down
sentence ("when you start another agent process, a job or a timer, unset
`SELDON_ATTENDED` …") adds `SELDON_CASE`. Attribution does **not** change
in this ADR: hook events keep taking the case from `.seldon/active-case`
(§5 rule 1); using the marker's id for attribution is a later, separate
decision.

### 2. The starting folder: omarchy-agent's rule, applied by the engine

`agent start` no longer sets `current_dir` to the logbook. It starts the
launcher where it was itself called; when that folder is `$HOME`, `/`, or
not a directory, it starts in `~/Work` if that exists, else in `$HOME`.
This is `omarchy-agent`'s rule word for word for the default launcher
(the result is identical to `omarchy agent prompt` typed in a terminal)
and extends it to named launchers (`claude` directly), so a logbook user
gets the same trust folder whichever launcher they configured. `[agent]
workdir = "inherit"` (default, not written) `| "logbook"` keeps today's
behaviour for a user who wants project-level hooks only. `--json cwd`
reports the folder used.

### 3. What the prompt and the context carry

The prompt stays identifiers and fixed text (WP-058): "Work case `<ID>` in
the Seldon logbook at `<root>`. Use the seldon skill; if your harness has
no skill mechanism, read `<root>/AGENTS.md` (Seldon's rules) instead.
First run `seldon hook session-start` unless your harness already gave you
the block `# Seldon logbook context`, then `seldon plan show <ID>`. Every
mutating command is recorded." A path is an identifier; no logbook text
enters the argument. The session-start block, when served by clause (b),
opens with one fixed engine line under the title: `Launched by seldon
agent start on <ID>; logbook <~-path>; this session is recorded.` — the
agent then knows its case and that the hooks serve it without probing.
The block's content otherwise stands (WP-111 adds the drift lines).

### 4. Agents without hooks

Unchanged in mechanism: the skill (installed by `hook install skills`) and
`SELDON_ACTOR`/`SELDON_ATTENDED`/`SELDON_LOGBOOK`/`SELDON_CASE` in the
environment; the agent reports through `seldon hook generic --case <ID>`,
which `in_scope` gates with the same two clauses, so a codex or opencode
session Seldon launched from `~/Work` is served. The skill's "Outside the
Logbook Folder" paragraph is rewritten: a session `seldon agent start`
launched is served wherever it runs (Claude Code by its hooks, every
other agent by `hook generic`); a session started any other way is served
only inside the logbook, or everywhere under `scope = "all"`; "never leave
out `cwd`" stays.

### 5. Migration, no double recording

Project-level settings in an existing logbook keep working (clause a).
Where both files hold the hooks, Claude Code may run the same command
twice per tool call (A2); the engine makes that harmless itself:
`unless_recorded` by `tool_use_id` applies to PreToolUse as well, so the
second call records nothing. `seldon doctor` gets a row `hooks`:
*user-wide* (ok), *logbook only* (degraded: "sessions started from `~/Work`
are not recorded — fix: `seldon hook install claude-code`"), *both* (ok,
with the optional tidy-up `seldon hook uninstall claude-code --settings
<logbook>/.claude/settings.json`), *none* (ok when no harness is
configured). Nothing runs on its own; the CHANGELOG names the new default
and the panel's doctor banner shows the one-click fix. `scope_warning`
becomes the one-line description of the two clauses (no longer a warning
for the default path). If the rules block (`AGENTS.md`, WP-100) names the
hook location, its version bumps and `seldon rules update` carries the
sentence; the `de` template changes in the same WP (ADR-0007).

### 6. WP-116 (0.1.4, engine + skill + docs; after WP-111 merges, before the tag)

Engine: `SELDON_CASE` in `launch()` (a parameter, like the actor);
`in_scope` clause (b) via `parse_case_id`; `[agent] workdir` and the folder
rule; `settings_file` default and `CLAUDE_CONFIG_DIR`; PreToolUse dedup;
prompt text; the context's launch line; `doctor` row `hooks`. Skill:
hand-down sentence, "Outside the Logbook Folder". Docs: SPEC-ENGINE §3
(`agent start`, `hook install`, `doctor`), §8 (scope, context); guides
04/05 en/de; CHANGELOG. Fixture and schema untouched.

Acceptance (`just check` 0):
1. Stub launcher: called from a project folder → that folder; from `$HOME`
   with `~/Work` → `~/Work`; from `$HOME` without it → `$HOME`; from `/` →
   `~/Work`; `workdir = "logbook"` → the logbook; the environment holds
   `SELDON_CASE=<ID>`, `SELDON_LOGBOOK`, `SELDON_ACTOR`, `SELDON_ATTENDED=1`;
   `--json cwd` matches.
2. Scope: `cwd` and `CLAUDE_PROJECT_DIR` outside, `SELDON_CASE` valid →
   `hook claude-code` records with the active case, `session-start` prints
   the block with the launch line, `session-stop` journals and commits,
   `hook generic` records; `SELDON_CASE` empty, `not-a-case`, or absent →
   nothing (WP-063's six outside forms still pass); inside the logbook
   without the marker → served as before; `scope = "all"` unchanged.
3. Two PreToolUse calls with one `tool_use_id` → one event.
4. `hook install claude-code` without `--settings` writes
   `$HOME/.claude/settings.json` in a scratch HOME; a foreign hook group and
   foreign keys survive byte-for-byte in content; second run writes
   nothing; `CLAUDE_CONFIG_DIR` set → that folder; `init --harness
   claude-code` the same; `uninstall` leaves the foreign entries.
5. `doctor` states user-wide / logbook only / both / none, with the fixes.
6. Skill shape test pins the new paragraph and the `SELDON_CASE` hand-down;
   the WP-063 wording ("in the logbook folder … `scope = "all"`") must be
   gone.
7. Live check on the fresh test host (counted per ADR-0027 §1): `seldon
   init` with the Claude Code harness → hooks in `~/.claude/settings.json`,
   the host's other hooks intact; panel *New case → Run* → the agent window
   opens in `~/Work` (`pwdx`), the context block with the launch line is in
   the session, the agent's `pacman -S <pkg>` is one event with the case
   and actor `agent:claude-code`, zero drift, one journal line at session
   end; then `claude` by hand in `~/Work/other` → zero events, no block
   (privacy); `claude` by hand inside the logbook → served. Then the same
   with the second installed agent through `hook generic`.

## Consequences

- The panel's one-click start and `omarchy agent prompt` now differ in
  nothing the user can see: same folder, same trust, same flags, same
  window — plus the case, the actor and the record. Human steps per case
  do not grow; the trust prompt in `$HOME` is gone.
- The hooks move from the logbook into the user's Claude Code settings.
  Seldon writes one more file under `~/.claude`, idempotently and merged;
  it reads nothing there but its own three entries. What it records is
  decided by two clauses, both of which the user or Seldon set, never by
  the hooks' mere presence.
- One new environment variable, one config key, one doctor row, one
  changed default path. `contractVersion` stays 1.
- Open: whether hook attribution should prefer `SELDON_CASE` over
  `.seldon/active-case` when two agents work in parallel — measured on
  the test host first, decided separately.

## Alternatives considered

- **Keep starting in the logbook.** Rejected by the operator's decision;
  it also costs a trust prompt per session and puts the agent's working
  folder where project files never are.
- **Delegate the folder to `omarchy-agent` and set nothing.** Nearly the
  same; rejected only because a named launcher (`claude` directly) would
  then not get the `~/Work` rule, and the e2e test could not pin the
  folder.
- **`[hooks] scope = "all"` as the default.** Rejected: it records every
  Claude Code session of the user; the operator's privacy line (ADR-0027
  §8 left it opt-in) stands.
- **`SELDON_LOGBOOK`, `SELDON_ACTOR` or `SELDON_ATTENDED` as the marker.**
  Rejected (§1): either exportable by the user for other reasons, or
  defined as unread by the engine.
- **`claude --settings <logbook>/.claude/settings.json` in the launcher.**
  Rejected: it needs Seldon's own argv instead of `omarchy agent prompt`,
  serves Claude only, and departs from Omarchy's default.
- **A marker file under `~/Work`.** Rejected: a file in a folder the user
  owns, for the agent's whole life, with the same privacy question and a
  cleanup problem; the environment dies with the session.
