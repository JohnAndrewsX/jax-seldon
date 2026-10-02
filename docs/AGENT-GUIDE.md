# AGENT-GUIDE.md — Working inside a Seldon logbook

For an AI agent (Claude Code, Codex, the Omarchy agent, any other) that a
user starts inside a Seldon logbook, `~/Seldon` by default. It is the long
form of the rules in the logbook's own `AGENTS.md`. If you are working on
Seldon's source code instead, read the repository's
[`AGENTS.md`](../AGENTS.md), the team rulebook.

Seldon is a flight recorder, not a guard. It records what changes on the
machine and who did it; it never blocks a command and never changes the
system itself. Keeping to these rules is your job.

## 1. Where the rules live

| File | What it is |
|---|---|
| `AGENTS.md` (logbook root) | The rules, short form. Written by `seldon init` in the logbook's language; the user may have edited it. **It wins** over this guide. |
| `areas/<area>/AGENTS.md` | Extra rules for one area, where the user wrote some. |
| `memory/lessons.md` | What earlier sessions learned on this machine. One `## ` heading per lesson. |
| `PROJECT.md` | What the machine is for, what must not happen on it, who works here. |
| `STATUS.md` | Generated summary: active cases, open drift, latest events. |

There is no `CLAUDE.md`: Claude Code reads `AGENTS.md`.

## 2. Session start

1. Read `AGENTS.md`, `PROJECT.md`, `memory/lessons.md` and `STATUS.md`.
2. Find the active case: `seldon plan list --status active` and
   `seldon plan show <ID>`. With the Claude Code hooks installed,
   `SessionStart` runs `seldon hook session-start` and gives you the same
   context (status summary, active case and its plan steps, the last
   journal lines, the lesson headings). Other agents run it themselves.
3. Read the active case file (`seldon open case` prints its path) and, if
   it names an area, `areas/<area>/README.md` and `areas/<area>/AGENTS.md`.
4. Check `seldon drift`. Open drift is not yours to fix unless the user
   asks, but you must know about it before you change the same things.

Write in the logbook's language (`language` in `PROJECT.md`). Headings,
frontmatter keys and enum values stay English in every language.

## 3. Plan before you change the machine

Every change to the machine happens inside a **case** (`C-YYYY-NNN`, a
Markdown file under `work/`). Work one case at a time; the engine allows
more, and `.seldon/active-case` names the one started last.

1. **No case yet?** Propose one to the user — title, zone, risk, area —
   and wait for their go. Then:
   `seldon plan new --zone yellow --risk R1 --area dev-env --actor agent:<name> -- "<title>"`.
2. **Write the plan** into the case file before you start: *Intent* (why,
   and what is different afterwards) and *Plan* — goal, steps, affected
   paths, rollback, verification. Name packages and paths exactly as they
   will appear: Seldon proposes linking an unexplained change to an open
   case whose *Plan* names its subject.
3. **Start it:** `seldon plan start <ID> --actor agent:<name>`. From then
   on every change the hooks and collectors see carries the case id. For a
   red case take a snapper snapshot first and pass it:
   `--snapshot <N>`.
4. **Work the steps.** Add dated lines to the case's *Log* (append only,
   never edit earlier lines) or use `seldon log --case <ID>`.
5. **Hand over:** fill *Result*, then
   `seldon plan verify <ID> --actor agent:<name>`. Close it with
   `seldon plan done <ID>` only when the verification has passed and the
   user agrees; give up with
   `seldon plan drop <ID> --reason "<why>"`.

The engine enforces the order `queued → active → verification →
completed` (or `dropped`) and moves the file between `work/queued/`,
`work/active/` and `work/completed/`. Never move or rename case files
yourself.

## 4. Zones and risk

| Zone | What | Rule |
|---|---|---|
| **red** | packages, Omarchy itself (`omarchy update`, `omarchy pkg …`), systemd units, `/etc`, the boot loader | only in a red case with a rollback plan, and a snapshot first |
| **yellow** | configuration under the watched paths (`~/.config/hypr`, `~/.config/omarchy`, …), themes, shell plugins | only inside a case |
| **green** | everything no collector tracks: project files, language package managers, `git` | recorded while a case is active |

Risk is the case's own estimate, `R0` to `R3`, on the scale in
[`SPEC-LOGBOOK.md` §3](SPEC-LOGBOOK.md#3-frontmatter-conventions). In
short: `R0` reversible in seconds; `R1` by hand in minutes, with a named
rollback; `R2` needs the plan plus a snapshot or backup; `R3` can break
boot, login or the shell — snapshot mandatory, the user's explicit go for
each step. The engine stores the risk and does not enforce it.

Omarchy rules on top: install packages with `omarchy pkg add` (AUR:
`omarchy pkg aur add`), not with the package manager directly; never edit
files under `/usr/share/omarchy`, customise under `~/.config`.

## 5. The commands you use

Pass `--actor agent:<name>` wherever a command takes it (`log`, `event`,
`plan *`, `drift link|explain|dismiss`). Put free text after `--`. Add
`--json` when you parse the output.

**Read-only — use freely:**

| Command | What it tells you |
|---|---|
| `seldon doctor` | engine, config, logbook, Omarchy, snapper and git checks |
| `seldon plan list [--status S] [--area A]` | the cases |
| `seldon plan show <ID>` | one case |
| `seldon drift [--crisis-only]` | open drift, crises first |
| `seldon drift show <EVENT> --json` | one drift item and every member of its group |
| `seldon open <case\|journal\|ledger\|status\|logbook\|C-…\|ADR-…>` | the path of that file (without `--editor`) |
| `seldon hook session-start` | the session context block |
| `seldon --version`, `seldon contract-version` | versions |

**Writing — when the rules above say so:**

| Command | When |
|---|---|
| `seldon plan new … -- "<title>"` | after the user agreed to the case |
| `seldon plan start\|verify\|done\|drop <ID>` | the case steps of §3 |
| `seldon log --case <ID> --actor agent:<name> -- "<text>"` | a journal note: what you did, found or decided |
| `seldon event <source> <kind> --subject S --actor agent:<name>` | to record something no hook or collector sees, e.g. a change made through a GUI |
| `seldon decide --no-edit --case <ID> -- "<title>"` | a decision that shapes the machine; then fill in *Context*, *Decision*, *Consequences* of the new `decisions/ADR-NNNN-*.md` |
| `seldon drift link\|explain\|dismiss …` | §7, only when you know the reason |
| `seldon capture --all`, `seldon status` | to bring the ledger, `STATUS.md` and the index up to date |
| `seldon hook session-stop --actor agent:<name>` | at the end of a session, if no hook does it (§8) |

**Not for agents** unless the user asks for exactly this: `seldon init`,
`seldon hook install`, `seldon import … --apply`, `seldon agent start`,
any `--logbook` or `SELDON_LOGBOOK` that points at another logbook.

Exit codes: `0` ok, `1` user error (bad argument, unknown case, a step the
case cannot take), `2` engine error, `3` logbook not initialised, `4` lock
held — wait a moment and retry.

## 6. Hooks: what gets recorded

With the Claude Code harness (`seldon init --harness claude-code`, or
`seldon hook install claude-code` afterwards) the logbook's
`.claude/settings.json` has three hooks:

| Hook | Runs | Does |
|---|---|---|
| `PreToolUse` (`Bash\|Edit\|Write\|MultiEdit`) | `seldon hook claude-code` | records a mutating command or file edit as an `agent` event with your actor, the active case and the start time |
| `SessionStart` | `seldon hook session-start` | prints the context block of §2 |
| `SessionEnd` | `seldon hook session-stop` | journal line "session ended; N events recorded", `capture --all`, `STATUS.md` and index, one commit |

Other agents call the same commands themselves: before each command pipe
`{"command": "…", "actor": "agent:<name>", "cwd": "…"}` into
`seldon hook generic`; run `seldon hook session-start` at the start and
`seldon hook session-stop --actor agent:<name>` at the end.

What a hook records:

- package installs and removals, `omarchy` commands that change the
  system, `systemctl enable|disable|start|stop|mask|unmask` — red;
- writes into watched paths (`cp`, `mv`, `tee`, `sed -i`, `rm`,
  redirections, and the `Edit`/`Write` tools) — yellow;
- every other change (files elsewhere, `npm install`, `git push`) — green,
  and only while a case is active.

Read-only commands record nothing. Hooks are silent and always exit 0;
they never stop you. They read the command line and the path, never a
command's output or a file's content. Commands hidden inside `xargs`,
`find -exec` or an interpreter (`python -c`, `node -e`) are not read yet:
run changes as plain commands so the case's trace is complete. The
collectors (package log, snapper, Omarchy version, plugins, theme, config
files) see the effects at the next capture either way.

## 7. Drift, and how to explain it

A change without a case is **drift**; drift in the red zone is a
**crisis**. `seldon drift` lists the open items. Resolve one only when you
know why it happened — if you don't, ask the user:

- `seldon drift link <EVENT> <CASE>` — the change belongs to that case
  (also a completed one).
- `seldon drift explain <EVENT> -- "<why it happened>"` — creates a
  completed, retroactive case with that text as its title.
- `seldon drift dismiss <EVENT> -- "<why it does not matter>"`.

Package events of one transaction (a routine upgrade, say) form one
group: resolving any open member resolves every open member of the group
(SPEC-ENGINE §5); `--only` resolves the named event alone. Never hide drift by
editing or reverting files, and never edit the ledger to remove it.

## 8. Ending a session

1. Fill the case's *Result*; `seldon plan verify <ID>` when the work is
   done, or leave the case active and say what is left.
2. Write what you learned into `memory/` — `memory/lessons.md`, one `## `
   heading per lesson: what happened, what to do next time. Not into the
   chat.
3. A last journal note: `seldon log --case <ID> --actor agent:<name> --
   "<summary>"`.
4. With Claude Code, `SessionEnd` runs `seldon hook session-stop` for you.
   Other agents run `seldon hook session-stop --actor agent:<name>`.

## 9. A worked example

From the Phase 0 exit run on the operator's machine
(`work/completed/PHASE-0-EXIT-transcript.md`): Claude Code worked a case
with nothing but the logbook's `AGENTS.md` to go on.

```
# the user creates and starts the case
$ seldon plan new --zone yellow --risk R1 --area dev-env -- "Phase 0 exit: first real case"
Created C-2026-001 "Phase 0 exit: first real case" in work/queued/C-2026-001-phase-0-exit-first-real-case.md
$ seldon plan start C-2026-001
$ cd ~/Seldon && claude
```

The agent read `AGENTS.md`, filled the case's *Intent* and *Plan*,
switched the theme three times with `omarchy theme set`, wrote a note and
moved the case to verification. The ledger then held:

```
09:43:02  theme-set  flexoki-light  C-2026-001  agent:claude-code
09:43:23  theme-set  gruvbox        C-2026-001  agent:claude-code
09:43:43  theme-set  osaka-jade     C-2026-001  agent:claude-code
09:43:48  note                      C-2026-001  agent:claude-code
09:43:48  case-verified             C-2026-001  agent:claude-code
```

The theme switches came from the `PreToolUse` hook; the note and
`case-verified` from the agent's `seldon log` and `seldon plan verify`.
The user then captured, found one drift item that was not the agent's (a
hook script `seldon init` had installed), explained it and closed the
case. (That drift item is historical: since WP-038 the engine explains
the files it installs itself, SPEC-ENGINE §5 rule 7, so it no longer
appears.)

```
$ seldon capture --all
$ seldon drift
yellow  2026-10-02 09:20  config/config-add  ~/.config/omarchy/hooks/theme-set.d/seldon-theme-set.sh  01M3XS7Z0PZFWA88Y83771YG2R
$ seldon drift explain 01M3XS7Z0PZFWA88Y83771YG2R -- "Installed by seldon init (theme hook opt-in); engine-owned file"
Explained 1 event(s) with the new completed case C-2026-002
$ seldon plan done C-2026-001
C-2026-001 verification → completed
```

Every change carried the case id; the case's trace is the list above.

## 10. Do not

- Edit `ledger/*.jsonl` (append-only; a mistake is corrected with a new
  event), the generated `ledger/*.md` and `STATUS.md`, the frontmatter
  fields the engine keeps (`status`, `events`, `agents`), `.seldon/`, or
  text inside `<!-- seldon:begin … -->` / `<!-- seldon:end -->` fences.
- Move, rename or delete case files; delete the logbook or any part of it.
- Change the machine without an active case; run a red-zone command
  without a red case, a rollback plan and a snapshot.
- Start a case the user has not agreed to, or close one they have not
  accepted.
- Resolve drift you cannot explain, or hide it by editing files.
- Run `seldon` against a logbook that is not the one you were started in
  (`--logbook`, `SELDON_LOGBOOK`), least of all another user's.
- Write secrets, tokens or passwords into the logbook. The engine redacts
  what it recognises; do not rely on it.
- Rewrite history: no `git push --force`, no amended or rebased commits in
  the logbook.
- Install packages with the package manager directly, or edit files under
  `/usr/share/omarchy`.
- Build shell strings out of logbook text and run them.

## See also

[`CONCEPT.md`](CONCEPT.md) (the idea) ·
[`SPEC-LOGBOOK.md`](SPEC-LOGBOOK.md) (files and frontmatter) ·
[`SPEC-ENGINE.md`](SPEC-ENGINE.md) §3 commands, §5 drift, §8 hooks ·
[`CONTRACT.md`](CONTRACT.md) (engine ⇄ plugin) ·
[`../llms.txt`](../llms.txt) (the one-page index)
