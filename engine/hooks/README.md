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
