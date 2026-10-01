# engine/hooks/

Scripts the engine ships for Omarchy's hook system (`omarchy-hook <name>`
runs `~/.config/omarchy/hooks/<name>` and every file in
`~/.config/omarchy/hooks/<name>.d/`, each as `bash <file> <args…>`).
Nothing here is installed by default.

## `theme-set.sh`

| | |
|---|---|
| Hook | `theme-set` — `omarchy-theme-set` runs it with the new theme slug after it wrote `~/.local/state/omarchy/current/theme.name` |
| Does | `seldon event theme theme-set --subject "$1"` |
| Installed by | the wizard (`seldon init`, WP-024), only when the user opts in: `omarchy hook install theme-set <path>/theme-set.sh` (copies it to `~/.config/omarchy/hooks/theme-set.d/` with mode 755) |
| Removed by | deleting `~/.config/omarchy/hooks/theme-set.d/theme-set.sh` |

Why it is optional: the `theme` collector (`engine/src/collectors/theme.rs`)
finds every change on the next `seldon capture` by comparing `theme.name`
with its cursor. The hook only adds the exact time of the switch. When the
ledger already holds a `theme-set` to the current slug since the
collector's last check, the collector writes nothing, so a switch is never
recorded twice.

Contract of the script:

- silent (stdout and stderr go to `/dev/null`) and always exit 0, so a
  missing or failing `seldon` never breaks a theme switch;
- does nothing when `seldon` is not on `PATH` or no slug is given;
- passes the slug as one argument; it is never evaluated (AGENTS.md §8).

`seldon event` is WP-006's command (SPEC-ENGINE §3); the script relies only
on `seldon event <source> <kind> --subject S`. The event's actor is
`system` (the default of `seldon event` since WP-009), unless an agent's
recorded `omarchy theme set <name>` started at most 10 minutes before:
then it takes that agent and case, through the same attribution pass the
capture runs. `meta.from` and `detail` name the theme it replaces: the
theme collector's cursor, or a newer `theme-set` in the ledger.

## Claude Code (`seldon hook install claude-code`)

Not a script: `seldon hook install claude-code [--settings PATH]` merges
three entries into `<logbook>/.claude/settings.json` (other hooks and keys
stay; running it again changes nothing):

| Event | Matcher | Command | What it does |
|---|---|---|---|
| `PreToolUse` | `Bash\|Edit\|Write\|MultiEdit` | `seldon hook claude-code` | records a mutating command (or a file edit) as an `agent/command` event at its start (ADR-0017 §1) |
| `SessionStart` | — | `seldon hook session-start` | prints the context block (status, active case, journal, lessons) |
| `SessionEnd` | — | `seldon hook session-stop` | journal stub, `capture --all`, commit; timeout 60 s, the most Claude Code allows a `SessionEnd` hook |

`SessionEnd`, not `Stop`: Claude Code runs `Stop` after every reply. Every
hook is silent and exits 0, even on a panic; problems go to stderr. Other
agents call `seldon hook generic [--case ID]` with
`{"command","actor","cwd","startedAt"?,"case"?}` on stdin before the
command runs, and `seldon hook session-start` /
`session-stop --actor agent:<name>` themselves.

What is recorded:

- **red / yellow, always:** what a collector tracks — pacman/yay/paru,
  `omarchy` package, update, plugin and theme commands, `systemctl`
  unit changes, writes into `watchPaths` (redirections, `tee`, `sed -i`,
  `cp|mv|install|ln`, `rm|truncate`, `Edit`/`Write`), `git` in `~/.config`.
- **green, only while a case is set** (ADR-0019): changes no collector
  tracks — files written outside `watchPaths` (not `/dev`, not the
  logbook), `npm|pnpm|yarn|bun|pip|pipx|uv|cargo|go` installs and removals,
  `git` sub-commands that change a repository. `git` in the logbook is
  green and always recorded.
- `sh|bash|zsh|dash -c '…'` and `eval '…'` are read as the commands
  inside. Not read yet: `xargs`, `find -exec`, interpreters (`python -c`,
  `node -e`).

## Starting an agent (`seldon agent start`, WP-022)

`seldon agent start <caseId> [--launcher NAME] [--json]` sends an agent to
work an **active** case (a queued case is refused with the hint `seldon
plan start <id>`). It:

1. checks the launcher (below) before it writes anything;
2. makes the case the active case (`.seldon/active-case`), so the agent's
   recorded commands land on it;
3. builds the prompt: `Work case C-… in the Seldon logbook at <path>;
   every mutating command is recorded.`, a blank line, then the block of
   `seldon hook session-start` (status, the case and its plan, journal,
   lessons);
4. runs the launcher **detached** in the logbook directory, with
   `SELDON_LOGBOOK` set to it (and `SELDON_CONFIG` when a non-default
   config was used): stdin and stdout null, stderr appended to
   `~/.local/state/seldon/agent-launch.log` (never truncated: an earlier
   agent's terminal may still hold it), its own process group, never
   waited for. A launcher still running after 200 ms counts as launched;
   one that exits non-zero in that time is an error (exit 1) with the last
   lines it wrote to stderr (only this launch's), and the previous active
   case is put back.

`--json` → `{"launched": true, "launcher": "<name>", "program": "<argv[0]>",
"argv": [… "{prompt}" …], "case": "C-…", "cwd": "<logbook>",
"previousActiveCase": "C-…" | null}`. Every failure is exit 1 with the
message (exit 3 without a logbook, 4 when the lock is held).

### The launcher in `config.toml`

```toml
[agent]
# the default; `seldon agent start` without --launcher runs this
launcher = ["omarchy", "agent", "prompt", "{prompt}"]

[agent.launchers]
# more, for `seldon agent start C-… --launcher <name>`
claude = ["omarchy-launch-tui", "--app-id=org.omarchy.agent", "claude", "--", "{prompt}"]
codex  = ["xdg-terminal-exec", "-e", "codex", "--", "{prompt}"]
```

Rules, checked on every start (a broken launcher is exit 1, nothing runs):

- an argv list, never a string; no shell reads it;
- the first element is a program name without `/` (looked up on `PATH`)
  or an absolute path;
- exactly one element is exactly `{prompt}`; it is replaced by the
  prompt as **one** argument, whatever the prompt contains. `--x={prompt}`
  is refused;
- no shell before `{prompt}` (`sh`, `bash`, `zsh`, `dash`, `ksh`, `mksh`,
  `fish`, `nu`, `xonsh`, `eval`), none of the Omarchy launchers that turn
  their arguments into shell code
  (`omarchy-launch-floating-terminal-with-presentation`: `bash -c "$*"`;
  `omarchy-launch-or-focus`, `omarchy-launch-or-focus-tui`,
  `omarchy-launch-or-focus-webapp`: `eval exec setsid $LAUNCH_COMMAND`;
  `omarchy-launch-terminal-tmux`: a fixed `bash -c "tmux …"` that also
  drops its arguments, so the prompt would be lost), and no `hyprctl`
  (`dispatch exec` takes a shell string): the prompt carries logbook text
  and is never run as code (AGENTS.md §8);
- no `omarchy launch …` before `{prompt}`: the `omarchy` CLI dispatches
  it by route to an `omarchy-launch-*` script, so the list above could not
  see which one. Name the launcher itself (`omarchy-launch-tui`). Other
  `omarchy` routes (`omarchy agent prompt`) are allowed.

The refusal list is a **heuristic**, not a sandbox: it knows the shells
and Omarchy launchers above, not every program that runs a string as code
(`python -c`, `perl -e`, `node -e`, `xargs`, `ssh host …`, a wrapper
script of your own). `config.toml` is your own file; what it names runs
with your rights. The rules make the common mistakes impossible, nothing
more.

Names: `default` is `[agent] launcher`; `omarchy` is the built-in
`["omarchy", "agent", "prompt", "{prompt}"]`, reachable even when
`[agent] launcher` is changed; any other name comes from
`[agent.launchers]` (an unknown one is exit 1 listing the known names).
`seldon init` does not write an `[agent]` section while it is the default.

### Fallbacks

- **The default, `omarchy agent prompt <prompt>`:** opens Omarchy's
  default coding agent (`omarchy default agent <name>`) in its own
  terminal window (`omarchy-launch-tui --app-id=org.omarchy.agent`),
  with that agent's "do not stop to ask" flag. Without a default agent it
  exits 1 with `Choose default agent with: omarchy default agent <name>`,
  which `agent start` reports.
- **Not `--inline`:** `omarchy agent prompt --inline` runs the agent in
  the caller's terminal. A detached launch (and the plugin) has none, so
  the agent would start without a terminal. Use `--inline` only by hand:
  `omarchy agent prompt --inline "$(seldon hook session-start)"`.
- **A specific agent in a window:**
  `["omarchy-launch-tui", "--app-id=org.omarchy.agent", "<agent>", …, "{prompt}"]`
  — `omarchy-launch-tui` passes its arguments through as an argv.
- **Not `omarchy-launch-floating-terminal-with-presentation`:** it joins
  its arguments into `bash -c "…"`, so the prompt would be run as shell
  code. Refused, as are the `omarchy-launch-or-focus*` launchers (`eval`)
  and `omarchy launch …`.
- **Not `omarchy-launch-terminal-tmux`:** it ignores its arguments and
  attaches tmux, so the agent would get no prompt. Refused.
- **Claude Code hooks:** the agent starts in the logbook directory, so
  `<logbook>/.claude/settings.json` (`seldon hook install claude-code`)
  applies: its `SessionStart` hook prints the same block again as context,
  and `PreToolUse` records the agent's commands on the case.
