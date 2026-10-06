# WP-116 — Plan

Normative: ADR-0030 (§1–§6), ADR-0031. No contract change.

## Engine

1. `agent start` (agent.rs): `SELDON_CASE=<ID>` in the launch environment
   (a parameter of `launch`, like the actor); starting folder by
   omarchy-agent's rule — the caller's folder, `~/Work` (else `$HOME`)
   when that is `$HOME`, `/` or not a directory; `[agent] workdir =
   "inherit" | "logbook"` (config.rs); `--json cwd` is the folder used;
   the new prompt text (§3).
2. Scope (hook.rs `in_scope`): clause (b) — `SELDON_CASE` that parses as a
   case id serves the call; used by `claude-code`, `generic`,
   `session-start`, `session-stop`.
3. Context (hook/context.rs): the launch line under the title when served
   by clause (b) only.
4. PreToolUse dedup: `unless_recorded` by `tool_use_id` for Pre as well.
5. `hook install|uninstall claude-code` default: `$CLAUDE_CONFIG_DIR/
   settings.json`, else `~/.claude/settings.json`; no logbook needed; a
   commit only when the file lies in the logbook. `init --harness
   claude-code` writes the same file. `scope_warning` → the one-line
   description of the two clauses.
6. `doctor` row `hooks`: user-wide / logbook only / both / none, fixes.
7. Rules block v4 (en/de) with the WP-111 stage-2 edits; v3 renderings
   under `templates/rules-v3/`, their hashes in `RELEASED_BLOCKS`.
8. Silent upgrades recorded: capture commits an upgraded `AGENTS.md`
   alone (`seldon: rules update (unedited, vN → vM)`), and writes a
   `seldon` note to the ledger when it updates an installed skill.

## Skill and docs

- SKILL.md: aim line (ADR-0031), "stay in your session", the
  `SELDON_CASE` hand-down, rewritten *Outside the Logbook Folder*;
  `snapshot.md`: `root` config only. Only these paragraphs (WP-115 edits
  the skill in parallel).
- SPEC-ENGINE §3 (`agent start`, `hook install`, `doctor`), §8 (scope,
  context); guides 04/05 en/de; AGENT-GUIDE where it repeats the rules;
  CHANGELOG.

## Tests (scratch HOME, `TZ` set where local times are read)

ADR-0030 acceptance 1–6: stub launcher folders and environment; scope
matrix incl. WP-063's outside forms; PreToolUse dedup; install targets
and foreign entries; doctor's four states; skill shape. Plus rules v3→v4
silent upgrade with its own commit, and the skill-update ledger note.
Mutants by hand on the new branches; `flock /tmp/seldon-check.lock just
check` at the end.
