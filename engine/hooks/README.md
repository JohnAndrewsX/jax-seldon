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
| Installed by | the wizard (`seldon init`, WP-024), only when the user opts in (`--theme-hook`, or "yes" to the wizard's question): it writes the script, compiled into the engine, to `${XDG_STATE_HOME:-~/.local/state}/seldon/hooks/seldon-theme-set.sh` and runs `omarchy hook install theme-set <that file>`, which copies it to `~/.config/omarchy/hooks/theme-set.d/seldon-theme-set.sh` with mode 755. The name is Seldon's own so the copy never replaces a hook of the user's. When that file exists already, nothing runs. A failure is reported with the command to run by hand; it never fails `init` |
| Removed by | deleting `~/.config/omarchy/hooks/theme-set.d/seldon-theme-set.sh` |

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

## Omarchy-Agent kit (`seldon init --harness omarchy-agent`)

The Omarchy-Agent kit's guard and skills stay the kit's (PROJECT.md: its
zone model and guard are not Seldon's). The wizard only copies them into
the logbook's `.claude/` when the kit is there as a template directory:

| | |
|---|---|
| Template directory | `$SELDON_OMARCHY_AGENT_KIT`, else `${XDG_DATA_HOME:-~/.local/share}/seldon/harness/omarchy-agent/` |
| Layout | that of `<logbook>/.claude/`, e.g. `hooks/guard.py`, `skills/<name>/SKILL.md`, `settings.json` (with the guard's `PreToolUse` hook) |
| Copied | every regular file, permissions kept (an executable guard stays executable); symbolic links are skipped; a file that exists in `.claude/` already is kept, never overwritten |
| Order | the kit first, then Claude Code's hooks, so `--harness claude-code` merges Seldon's hooks into the kit's `settings.json` next to the guard |
| Without the directory | nothing is copied; `init` says where it looked and what it would have copied |

Seldon ships no kit. Re-running the copy later is not a command yet: copy
the files by hand (`cp -rn <kit>/. <logbook>/.claude/`), then
`seldon hook install claude-code` if the kit brought a settings file.

## Claude Code (`seldon hook install claude-code`)

Not a script. `seldon init --harness claude-code` (or the wizard's harness
step) installs it into the new logbook, inside the first commit;
`seldon hook install claude-code [--settings PATH]` does the same later and
merges
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
