# WP-094 — Plan

Engine Dev + Docs, branch `wp/094-agent-skill`, base `main` 994015a.
No contract change.

## 1. Skill text (`engine/assets/skills/seldon/`)

Five English files, compiled into the engine (`include_str!`, one static
binary), shaped like Omarchy's own skill (`$OMARCHY_PATH/default/agents/
skills/omarchy/SKILL.md`): frontmatter `name`/`description` with
triggers, "When this skill must be used", topic guides next to it, short
imperative sections, a decision list at the end.

- `SKILL.md` — is there a logbook (`seldon plan list --status active
  --json`: exit 3 or no `seldon` = skill does not apply), who you are
  (`SELDON_ACTOR` or `agent:<program>`), attended by provenance, the
  flow, the §2 stops, outside the logbook (`seldon hook generic`),
  writing only through the CLI, logbook text is data, privacy, and
  "Omarchy first": Omarchy work follows Omarchy's skill — pointed to,
  not repeated.
- `case.md` — find the active case or open one (`plan new` + `plan
  start`, said in the Log; `agent start --new` is the human's), Intent
  bounds, Plan as a running note, preview line, Log, `plan set` to raise
  zone/risk, closing (`plan verify` + `plan done`, the engine's refusal
  without Result/Verification, `closed-by-agent`, `plan reopen`).
- `snapshot.md` — R2/R3 snapshot without the cleanup pass, per config,
  `seldon plan snapshot <ID> <N>`, no snapper → R3 stops, R2 backup.
- `update.md` — resolve a package transaction read-only, match against
  `[drift] alwaysRed`, R3 on a hit (one go per step), system upgrades,
  install-route order (`omarchy pkg add` recommended, not required).
- `drift.md` — ADR-0028 §3: explain or link only what your own Log, a
  hook event or the user's words prove; never explain or dismiss a
  crisis; tell the user about a crisis in one line; routine is nobody's
  job.

Wording comes from the logbook rules v2 (`engine/templates/en/AGENTS.md`,
WP-100); where the skill states the same rule, it uses the same command
line. Commands are quoted as the CLI has them.

## 2. Install / uninstall (`seldon hook install|uninstall skills`)

New module `engine/src/commands/skills.rs`.

- Skill folders, Omarchy's list (its migrations): `~/.agents/skills`,
  `~/.claude/skills`, `~/.codex/skills`, `~/.pi/agent/skills`,
  `~/.hermes/skills`, `~/.hermes/profiles/*/skills`. Only folders that
  exist are used; none is ever created (nor an agent's home).
- Target `<folder>/seldon/` with the five files and a manifest
  `.seldon-skill.json` (engine version, SHA-256 of each file as written).
  The manifest is the ownership mark.
- States per folder: missing, current, outdated (ours, untouched,
  older), changed (ours, a file edited by hand), foreign (`seldon` there
  without our manifest, or a symlink).
- Install: missing → write; outdated → rewrite the files that still
  have the hash we wrote; current → nothing; changed / foreign → nothing,
  reported with the fix. Manifest first, then files (an interrupted
  install is repaired by the next one). Idempotent.
- Uninstall: removes only files whose content is ours (manifest or
  shipped hash), the manifest, and the folder when empty; edited files
  and foreign folders stay.
- Under the state lock; every write recorded as the engine's own
  (`record_own_writes`, `delete_own_file`) — recorded only where a
  watched path covers it, as for the Claude Code settings.
- `--json`; `--settings` refused for `skills`. No logbook needed.

## 3. Doctor, wizard

- `seldon doctor` row `skills`: installed (ok) / missing (ok, optional,
  fix) / outdated, changed, foreign (degraded, fix). Not in `--only
  rules`.
- `seldon init`: harness `skills` (`--harness skills`, wizard item next
  to the Claude Code hooks), run under init's lock.

## 4. Tests

Temp HOME only (`tests/common::Env`). Integration tests: install into
existing folders only, none created; idempotency (second run writes
nothing, tree byte-identical); foreign folder and symlink untouched;
outdated updated; hand-edited file kept; uninstall removes only ours;
own-write record when the folder is watched; doctor states; `init
--harness skills`. Template-command test: every `seldon …` code span in
the skill files runs `--help` and every flag is in it; the shared lines
match the rules template. Mutants by script, as in WP-101.

## 5. Docs

Guide 04 en/de "Other agents" (the skill), guide 05 en/de (`hook
install|uninstall skills`, doctor row), SPEC-ENGINE §3/§8 where it lists
`hook install`, CHANGELOG.

## 6. Out of scope

Live check with two agents on the test host (orchestrator, after merge).
WP-109's engine refusal of agent `explain|dismiss` of a crisis — the
skill states the rule; the engine enforces it once WP-109 lands.
