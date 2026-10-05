# Working with agents

This page shows how an AI agent works a case while Seldon records what
it does: Claude Code, Omarchy's default agent and any other agent. It
covers the hooks, the active case, `seldon agent start` and how you
review the result.

## What Seldon does and does not do

Seldon records. Hooks tell the engine which commands an agent runs, and
the engine writes them into the ledger with the agent's name and the
active case. Afterwards you see the changes the agent made next to the
plan it was given.

Seldon does not guard. A hook never stops a command, never asks for
permission and never changes what the agent does. If you want limits,
set them in your agent's own permission settings. Seldon shows you
afterwards whether the agent kept to the plan.

## The rules agents read

`seldon init` writes `AGENTS.md` into your logbook, in the logbook's
language. It tells every agent how to work there:

- read `PROJECT.md`, `memory/lessons.md` and `STATUS.md` first;
- change the machine only inside an active case, and only after you
  agreed to the case;
- write the plan into the case before changing anything;
- move the case to verification when done, and leave closing it to you;
- record what it learned in `memory/`;
- never edit the ledger, generated files or engine-owned fields.

The file is yours. Add rules for your machine; agents follow the file in
the logbook. Rules for one area go into `areas/<area>/AGENTS.md`. The long
form of the rules is the project's
[agent guide](../../AGENT-GUIDE.md).

## Who closes a case

An agent runs `seldon plan verify` when its own checks pass, and stops
there. You check the result and run `seldon plan done`, or press *Done*
on the Work tab. If you want the agent to close the case itself, say so
in the case's *Plan*, for example "close when verified". The engine does
not enforce this rule; the ledger shows who closed each case.

## Claude Code

### Set it up

Claude Code needs three hooks in the logbook's `.claude/settings.json`.
If you chose "Claude Code hooks" in the wizard, they are already there.
Otherwise install them once:

```sh
seldon hook install claude-code
```

```text
~/Seldon/.claude/settings.json: installed the Seldon hooks.
  added    PreToolUse (Bash|Edit|Write|MultiEdit): seldon hook claude-code
  added    SessionStart: seldon hook session-start
  added    SessionEnd: seldon hook session-stop
```

The command keeps every other hook in the file. Running it again changes
nothing.

In the context that `SessionStart` prints, every line from your logbook
starts with `>`, under a note that these lines are data, not
instructions. Text in a note or a case therefore cannot pose as part of
Seldon's own structure. This makes the context clearer for the agent; it
does not guarantee that the agent ignores what the text says.

Seldon itself sends nothing over the network. The agent does: it sends
what it reads to its model provider, including this context, the files
it opens and the output of its commands. Redaction covers recorded
commands, not the text of notes, cases and `memory/`
(see [Configuration](06-configuration.md#redaction)). Keep secrets out of
the logbook.

| Hook | Runs | Does |
|---|---|---|
| `PreToolUse` | `seldon hook claude-code` | records each changing command or file edit, with the agent and the active case |
| `SessionStart` | `seldon hook session-start` | gives the agent the status, the active case with its plan, the last journal lines and the lesson headings, each line from the logbook quoted |
| `SessionEnd` | `seldon hook session-stop` | writes "session ended; N events recorded" into the journal, captures, rebuilds `STATUS.md` and commits |

### Work a case

1. Create the case and start it, in the panel or in a terminal:

   ```sh
   seldon plan new --zone yellow --risk R1 --area hyprland -- "Larger gaps between windows"
   seldon plan start C-2026-003
   ```

2. Start Claude Code in the logbook folder:

   ```sh
   cd ~/Seldon && claude
   ```

   Accept the workspace trust prompt. The hooks live in the folder's
   `.claude/settings.json`, and Claude Code only runs them in a trusted
   folder.

3. Give it the task, for example: "Work case C-2026-003. Write the plan
   into the case first. Move it to verification when you are done."

4. The agent reads `AGENTS.md`, fills the case's *Intent* and *Plan*,
   does the work and runs `seldon plan verify C-2026-003`.

5. Leave Claude Code with `/exit`. The `SessionEnd` hook adds a journal
   line, captures and commits.

6. Check the result (see [Review what the agent did](#review-what-the-agent-did))
   and close the case:

   ```sh
   seldon plan done C-2026-003
   ```

The hooks only run when Claude Code starts in the logbook folder. You
can also install them into your user settings, so that Claude Code runs
them in every folder:
`seldon hook install claude-code --settings ~/.claude/settings.json`.
Seldon still records commands and prints its context only for sessions
in the logbook folder or below it; in other projects the hooks do
nothing, and `hook install` says so. To have every Claude Code session on
this machine write into your logbook, in any project, with the active
case, set this in `~/.config/seldon/config.toml`:

```toml
[hooks]
scope = "all"
```

Red and yellow commands are then recorded in every session; green
commands only while a case is active.

## The active case

The case you started last is the active case. Its id is in
`.seldon/active-case` in the logbook. The hooks read it, so each command
an agent runs lands on that case, wherever the agent works.

- `seldon plan start <ID>` makes a case active.
- `seldon plan done` and `seldon plan drop` clear it, if it names that
  case.
- The engine allows several active cases, but only the one started last
  receives the agents' commands. Work one case at a time.

Without an active case, hooks still record red and yellow commands, but
without a case. The changes those commands cause then show up as drift,
with the agent's name. Green commands are not recorded at all.

## What a hook records

| Zone | Recorded |
|---|---|
| red | installing, removing and upgrading packages (also `pacman -Syu` and `omarchy update`), `omarchy` commands that change the system, `systemctl enable`, `disable`, `start`, `stop`, `mask`, `unmask` |
| yellow | writes into watched paths: `cp`, `mv`, `tee`, `sed -i`, `rm`, redirections, and Claude Code's Edit and Write tools |
| green | every other changing command (`npm install`, `git push`, files elsewhere), only while a case is active |

A hook records the command line and the path. It never records a
command's output or a file's content. Before writing, the engine removes
passwords and tokens it recognises (see [Configuration](06-configuration.md#redaction)).
Read-only commands record nothing.

Commands hidden inside `xargs`, `find -exec` or an interpreter
(`python -c`, `node -e`) are not read. The collectors still see their
effect at the next capture, but without the agent's name and the case.
Tell your agent to run changes as plain commands.

## Start an agent from the panel

*Start agent* on an active case's card (or `a` twice on the Work tab)
runs:

```sh
seldon agent start C-2026-003
```

The engine makes the case the active case and starts an agent in the
logbook folder. The agent's first prompt names the case and the logbook
and tells the agent to run `seldon hook session-start` and
`seldon plan show C-2026-003`; it holds no text from your logbook. The
prompt is a command-line argument, so it is visible in the process list
(`ps`) while the agent runs, and a session journal that logs app
launches keeps it. Once the agent's first command is recorded, the card
shows its name.

The agent also gets two environment variables. `SELDON_ACTOR` is
`agent:` and the launcher's name (`agent:default` for the default
launcher). `seldon log`, `plan`, `drift`, `event` and `hook generic`
record that name when `--actor` (for `hook generic`, `"actor"`) is
missing, so a note or a case step the agent forgets to sign is recorded
as the agent, never as you. `event` first takes the agent command it
finds in the ledger, with that command's case. `SELDON_ATTENDED=1` tells the agent
that you started it: the logbook's rules (`AGENTS.md`) say what it may
do then. Seldon itself never reads it.

By default the engine starts Omarchy's default coding agent, through
`omarchy agent prompt`, in its own terminal window. Which agent that is
depends on your Omarchy setup. If it is Claude Code, the hooks from the
section above record its commands, because it starts in the logbook
folder.

You can name another launcher in `~/.config/seldon/config.toml`, for
example Claude Code in a terminal window (here Alacritty):

```toml
[agent]
launcher = ["alacritty", "-e", "claude", "{prompt}"]

[agent.launchers]
omarchy = ["omarchy", "agent", "prompt", "{prompt}"]
```

`{prompt}` is replaced by the prompt, as one argument. The engine never
runs the launcher through a shell. It refuses shells, interpreters and
other programs known to run their arguments as code, before `{prompt}`.
The check goes by program name: a heuristic, not a sandbox.
`--launcher NAME` picks one from `[agent.launchers]`. If the launcher
fails, its error is in `~/.local/state/seldon/agent-launch.log`.
[Configuration](06-configuration.md#agent-launcher) has the rules.

`agent start` refuses a case that is not active. Start it first.

## Other agents

Codex, a script or an agent of your own reports to Seldon with three
commands. At the start of a session, for context:

```sh
seldon hook session-start
```

Before each command, the command as JSON on standard input:

```sh
echo '{"command": "systemctl --user enable syncthing", "actor": "agent:codex", "cwd": "/home/you/Seldon"}' | seldon hook generic
```

At the end of the session:

```sh
seldon hook session-stop --actor agent:codex
```

The JSON may also carry `"startedAt"` (a timestamp) and `"case"` (a case
id instead of the active case). The actor is always `agent:` and a name.
Without `"actor"`, `seldon hook generic` takes `SELDON_ACTOR`.
`seldon hook generic` is silent and always exits 0, so it never breaks
the agent.

## The Omarchy-Agent kit

If you used the Omarchy-Agent kit before Seldon, the wizard can copy its
guard and skills into the logbook's `.claude/` folder: choose the
`omarchy-agent` harness, or run `seldon init --harness omarchy-agent`.
The engine looks for the kit in `~/.local/share/seldon/harness/omarchy-agent/`.
It never overwrites a file that exists. To bring the kit's old logbook
into Seldon, see [Import from omarchy-agent](09-import-from-omarchy-agent.md).

## Review what the agent did

`seldon plan show C-2026-003` prints the case file. Its `events:` line
holds only event ids. To read the events themselves, open this month's
ledger view; each line names the time, the source, the case and, for
an agent, its name:

```sh
seldon open ledger --editor
```

In the panel:

- Changelog, filter `agent`: the commands agents ran, with their case.
- Today: the agent's journal notes and the "session ended" line.
- Work: the case card with its steps.

Each engine step is a git commit in the logbook. `git -C ~/Seldon log -p`
shows what changed in each file, including the case's *Plan* and *Log*.
Then run `seldon drift`. Anything the agent changed without the hooks
seeing it shows up there.

## Keep agents in their lane

- Give the agent a case before it changes anything. Without one, its
  changes become drift.
- Do not let agents run `seldon init`, `seldon hook install`,
  `seldon import … --apply` or `seldon agent start` unless you ask for
  exactly that. The logbook's `AGENTS.md` says the same.
- Let the agent verify and close the case yourself.
- Read the case's *Result* and the trace before you press *Done*.

---

Previous: [Daily use](03-daily-use.md) · [Index](README.md) · Next: [CLI reference](05-cli-reference.md)
