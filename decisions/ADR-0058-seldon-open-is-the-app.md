# ADR-0058 — `seldon open` is the app; `seldon edit` opens a logbook file; `seldon skill`; `seldon changes --around`

**Status:** proposed (E96 delegation; operator accepts)
**Date:** 2026-10-14

> Companion of [ADR-0056](ADR-0056-seldon-is-an-omarchy-app.md). Changes
> SPEC-ENGINE §3 (`open`) and one CONTRACT.md row (`open … --editor
> --json` → `edit … --editor --json`, same JSON shape). `--json` output a
> plugin runs is contract (AGENTS.md §3, E42; ADR-0040, ADR-0044,
> ADR-0047); a released 0.4 plugin keeps reading what it reads through
> the alias, so **no `contractVersion` bump**. ADR-0041 stands: its
> `--editor` focus rule moves with the command to `edit`. Implemented by
> WP-229 (engine) on the branch `app`.

## Context

1. **The omarchy-app convention:** "keep the GUI behind `<name> open`";
   commands that need no display take `--json`; `<name> skill install`
   installs a short agent skill bundled in the binary (Hype also prints
   it with `hype skill`). Hype and Signed strip a leading `open` and start
   the window; their `.desktop` files run `<name> open`.
2. **`seldon open` is taken.** `seldon open <case|journal|ledger|status|
   logbook|C-…|ADR-…> [--editor] [--json]` prints a logbook path or opens
   it in the editor (SPEC-ENGINE §3; WP-008, WP-156, ADR-0041). The
   plugin runs it (`Model.js`; CONTRACT.md's command table).
3. **The skill verb.** Seldon compiles its skill in and installs it into
   every agent folder that exists (`seldon hook install skills`: the
   shared `~/.agents/skills`, Claude Code, Codex, pi, Hermes), with a
   manifest of what it wrote — richer than Hype's, under a different
   name, with no print form.
4. **Exit codes.** omarchy-app says "exits 0 or 1 with errors on
   stderr". Seldon's are 0 ok, 1 user error, 2 engine error, 3 logbook
   not initialised, 4 lock held (AGENTS.md §7), and with `--json` the
   error is JSON on stdout. The plugin keys on 3 and 4 (the
   not-initialised state; "try again").
5. **The machine question.** Omarchy's `diagnose-crash` skill tells an
   agent to correlate a crash's time with recent package updates and
   file changes. Seldon's ledger is that timeline, but no command answers
   "what changed around T" for an agent.

## Decision

1. **`seldon open`** (no target) starts the app: it runs `$SELDON_DESK`
   when set, else `/usr/lib/seldon/seldon-desk`. Without `--json` the
   engine `exec`s the app, as a launcher expects. With `--json` it starts
   the app detached (null stdio, its own process group, never waited
   for; the rule `edit --editor` already follows), returns at once and
   prints `{"started": true}`, or the error as JSON with its exit code.
   A missing app binary is exit 1 with one sentence: "The Seldon window
   is not installed. Install the seldon package." Two callers, two
   forms: the `.desktop` entry runs bare `seldon open`; the pill's left
   click runs `seldon open --json`, so the app is never a child of
   `omarchy-shell` (ADR-0056 §4). The suggested Hyprland binding becomes
   `{ launch = "seldon" }`. A second `seldon open` while the window is
   open raises that window instead of starting another (the mechanism is
   the app's).
2. **`seldon edit <case|journal|ledger|status|logbook|C-…|ADR-…>
   [--editor] [--json]`** is today's `open <target>`, unchanged in
   behaviour and JSON shape (including ADR-0041's focus of the terminal
   window Omarchy opened on the same path, and `SELDON_OPEN`). The plugin
   0.5 and the app call `edit`.
3. **`seldon open <target>`** stays a **deprecated alias** of `edit
   <target>` for 0.5.x: same arguments, same JSON on stdout, same exit
   codes, plus one stderr line ("`seldon open <target>` is now `seldon
   edit <target>`; the old form goes in 0.6.0"). A 0.4 plugin on a 0.5
   engine keeps working. The alias is removed in 0.6.0, when every
   shipped plugin and app calls `edit`. CONTRACT.md: the row is renamed
   to `edit`, the fixtures follow; `seldon.engineMin` of plugin 0.5.0 is
   0.5.0.
4. **`seldon skill [install] [--json]`** is the Hype-shaped front of
   `hook install skills`. Bare, it prints the bundled skill
   (`SKILL.md`) to stdout. `seldon skill install` installs it exactly as
   `seldon hook install skills` does today: every existing agent folder,
   the manifest, never a foreign or hand-changed file, idempotent (twice
   installs nothing new), `--replace` copies a changed file aside first. `hook install skills` stays as it is. The
   skill text says what the exit codes mean (§5).
5. **Exit codes stay 0–4.** 2–4 refine "1"; "non-zero means failure"
   holds for every agent and script; collapsing them would change what
   the plugin keys on, a contract change for a cosmetic gain. A recorded
   exception to the omarchy-app skill (ADR-0056 §9). The bundled skill
   says: "Seldon exits 0 on success and non-zero on failure (3: no
   logbook yet; 4: try again)." With `--json`, errors stay JSON on
   stdout (CONTRACT.md); the human form prints them on stderr. Revisited
   at 1.0 only on the operator's word.
6. **`seldon changes --around <time> [--before <dur>] [--after <dur>]
   [--json]`** answers "what changed around T": the ledger's events
   from `time − before` to `time + after` (defaults 2 h and 30 min),
   oldest first, each with its kind, subject, actor, case and class
   (routine, attention, crisis). Read-only: it reads the ledger, writes
   nothing, needs no index. `time` takes the forms the engine already
   parses for timestamps. The Seldon skill names it as the step
   Omarchy's `diagnose-crash` flow can ask, and tells the diagnosing
   agent to file a finding with `seldon inbox add`. Its JSON shape is
   specified in SPEC-ENGINE and becomes a CONTRACT.md row the day a
   plugin or the app runs it; until then it is agent-facing CLI, covered
   by VERSIONING's promise on `--json` output. Crash metadata in the
   index is contract 3 (0.6.0), not this ADR.
7. **Help.** The engine's help lists `open` under "App" and the rest
   under "Commands that need no display", as Hype does. The man page
   and the shell completions follow from clap.

## Consequences

- The user and Omarchy's launcher see one verb for the window; agents
  keep every command they have, plus `skill` and `changes --around`.
- WP-229 implements §1–§4 and §6 with tests: the existing `open` tests
  pass unchanged through the alias; new tests per command; `skill
  install` twice is idempotent; `changes --around` on the sample ledger.
- SPEC-ENGINE §3 is rewritten for `open`, `edit`, `skill`, `changes` in
  that WP; until then it carries a note pointing here.
- The plugin 0.5 (the pill) changes two calls: left click `seldon open --json`,
  and none of the desk's `open <target>` calls survive (they move to the
  app as `edit`).
- 0.6.0 removes the alias; its CHANGELOG lists it under **Breaking**.
- Risk: an agent script written against `seldon open <target>` breaks in
  0.6.0. The stderr line for a whole minor and the skill's text are the
  mitigation.
