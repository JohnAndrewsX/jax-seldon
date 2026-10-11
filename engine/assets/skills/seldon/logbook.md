# Outside the Logbook Folder

Read this when no hook serves you, when you work outside the logbook folder,
and before you start another agent, a job, a timer or a server.

## Which Sessions Are Recorded

A session `seldon agent start` launched is served wherever it runs, as
long as its case is open: the launch sets `SELDON_CASE` in your
environment, and Seldon records the session by it. Never set or export
`SELDON_CASE` yourself. Claude Code is served by its hooks (Seldon puts them into
the user-wide `~/.claude/settings.json`); every other agent reports its
commands through `seldon hook generic`, below. A session started any
other way is served only inside the logbook folder, or everywhere when
`hooks.scope` in `seldon doctor --json` is `"all"` (the user's setting);
elsewhere a report records nothing: skip it there. The
collectors still record every package, service and watched-path change.

## Report Your Commands

When no hook serves you — any agent but Claude Code, or Claude Code whose
session did not start with the `# Seldon logbook context` block — report
each changing command yourself, before it runs, where it is recorded:

```bash
jq -cn --rawfile command /dev/stdin --arg cwd "$PWD" \
  '{command: ($command | rtrimstr("\n")), actor: "agent:<name>", cwd: $cwd}' <<'SELDON_CMD' | seldon hook generic --case <ID>
<the command line, unchanged>
SELDON_CMD
```

Put the command line between the two `SELDON_CMD` lines exactly as you
will run it: the quoted heredoc expands nothing, so nothing in it runs
while you report it. If a line of the command is itself `SELDON_CMD`, use
another word as the delimiter in both places. `seldon hook generic` is
silent and always exits 0. Name your case with `--case <ID>`. Whether a
command run outside the logbook is recorded follows from how the session
started and the user's setting (`[hooks] scope`); never leave out `cwd` to
get around it. At the end of the session:
`seldon hook session-stop --actor agent:<name>`.

## Who You Are

Your actor is `$SELDON_ACTOR` when it is set (`seldon agent start` sets it),
else `agent:` and your program's name: `agent:claude-code`, `agent:codex`,
`agent:opencode`, `agent:pi`. Pass `--actor agent:<name>` wherever a `seldon`
command takes it (`log`, `plan`, `drift`, `event`). Put free text after `--`.

## Processes You Start

When you start another agent process, a job, a timer or a
server that outlives your step (tmux, an editor server), unset
`SELDON_ATTENDED` and `SELDON_CASE` and set `SELDON_ACTOR` to that agent's
name (`agent:<name>`); never leave it unset. What you start that way is
unattended: it records and reports only (`SKILL.md`, *Attended or Not*). A
sub-agent inside your own session shares your attendance and acts as you;
privileged steps stay in your session.
