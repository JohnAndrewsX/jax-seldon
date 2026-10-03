# CLI reference

This page lists every `seldon` command with every option, grouped by
task. The help blocks are the engine's own `--help` output; a check keeps
them equal to the engine. The text around them adds what the help does
not say.

## How to read this page

- `<ID>` is a case id (`C-2026-004`), `<EVENT>` an event id (the long id
  that `seldon drift` prints), `<DIR>` and `<FILE>` are paths.
- Free text (a note, a title, a reason) goes after `--`, as one argument:
  `seldon log -- "Text"`.
- Each block leaves out the global options below. Every command accepts
  them.
- `seldon help <command>` and `seldon <command> --help` print the same
  text in your terminal.

## Global options

<!-- help: seldon -->
```text
Flight recorder and planning desk for your Omarchy system

Usage: seldon [OPTIONS] [COMMAND]

Commands:
  contract-version  Print the engine/plugin contract version
  init              Create a logbook (wizard; --non-interactive takes defaults)
  doctor            Check engine, config, logbook, collector state, omarchy, snapper and git
  capture           Run collectors and append new events to the ledger
  log               Write a note: a ledger event and a journal entry
  event             Record an event by hand (hooks, scripts)
  plan              Plan and track cases: new, start, verify, done, drop, list, show
  decide            Create a decision record (ADR) and open it in the editor
  open              Print the path of a logbook file; --editor opens it
  index             Rebuild index.json and the ledger/*.md views; --check validates
  status            Regenerate STATUS.md, the ledger views and index.json; print a summary
  hook              Agent hooks: record commands, print session context, install into or uninstall from a harness
  drift             List open drift; link, explain, dismiss or show a drift event
  agent             Start an agent on an active case
  rebuild           Write outputs/REBUILD.md: the steps to rebuild this machine
  watch             Rebuild index.json when the logbook changes (feature "watch")
  dossier           Refresh the generated fences of system/*.md from read-only queries
  import            Import an earlier logbook (dry run unless --apply)
  completions       Print a shell completion script for bash, zsh or fish
  mangen            Print the man page seldon(1), generated from this help
  help              Print this message or the help of the given subcommand(s)

Options:
  -V, --version        Print the engine version
      --json           Machine-readable output
      --logbook <DIR>  Logbook directory (overrides config.toml and SELDON_LOGBOOK)
      --quiet          No human output on success
      --no-commit      Do not commit logbook changes to git
      --config <FILE>  Config file (overrides SELDON_CONFIG and ~/.config/seldon/config.toml)
  -h, --help           Print help
```
<!-- /help -->

- `--json` prints one JSON object instead of text. Agents and scripts use
  it. An error with `--json` is `{"error": {"code": 1, "message": "…"}}`.
- `--logbook <DIR>` picks the logbook for one command. Without it the
  engine takes `SELDON_LOGBOOK`, then `logbook` in `config.toml`, then
  `~/Seldon`.
- `--no-commit` writes the files but leaves the git commit out. The next
  command that commits takes the changes along.

## Exit codes

| Code | Meaning |
|---|---|
| 0 | ok |
| 1 | user error: a bad argument, an unknown case or event, a step the case cannot take |
| 2 | engine error: something failed that should not have |
| 3 | the logbook is not initialised; run `seldon init` |
| 4 | another `seldon` holds the lock; wait a moment and try again |

`--help` exits 0. An unknown option is a user error (1).

## Environment variables

| Variable | Effect |
|---|---|
| `SELDON_LOGBOOK` | the logbook directory, unless `--logbook` is given |
| `SELDON_CONFIG` | the config file instead of `~/.config/seldon/config.toml`, unless `--config` is given |
| `VISUAL`, `EDITOR` | the editor that `seldon open --editor` and `seldon decide` start in a terminal |
| `SELDON_NOW` | a fixed clock (RFC 3339), for demos and tests |
| `SELDON_TEST_GUARD` | a directory; the engine refuses to run (exit 2) when its home, config or state directory lies outside it. Use it when you try Seldon in a scratch home |
| `SELDON_OMARCHY_AGENT_KIT` | where `init --harness omarchy-agent` finds the kit, instead of `~/.local/share/seldon/harness/omarchy-agent/` |

## Set up and check

### seldon init

Creates the logbook, the config file, the first capture and the dossier.
Without flags it asks questions; each flag skips its question. It refuses
a directory that is not empty and never overwrites a file.
`--non-interactive` takes the flags, then an existing config, then the
defaults. `--since` takes a date (`2026-09-29`, local midnight) or an RFC
3339 time; `--baseline` needs `--since`; `--no-capture` cannot go with
`--since`. See [Getting started](01-getting-started.md#step-2-create-your-logbook).
`--remove-theme-hook` is the one flag that does not create a logbook: it
removes the theme hook that `--theme-hook` installed and goes with no
other flag. See [Update and uninstall](11-update-and-uninstall.md#uninstall).

<!-- help: seldon init -->
```text
Create a logbook (wizard; --non-interactive takes defaults)

Usage: seldon init [OPTIONS]

Options:
      --path <DIR>           Logbook directory (default ~/Seldon)
      --non-interactive      Ask nothing; take flags, then the existing config, then the defaults: ~/Seldon, language from the locale, all collectors, git on, first capture from now on, no backfill, no theme hook
      --language <LANGUAGE>  Language of the logbook prose [possible values: en, de]
      --obsidian             Add Obsidian settings (.obsidian/)
      --harness <NAME>       Agent harness to set up (repeatable) [possible values: claude-code, omarchy-agent]
      --since <TS>           Backfill: the first capture also records changes since TS, a date (YYYY-MM-DD, local midnight) or an RFC 3339 time; each one opens as drift
      --baseline             Mark the backfilled drift as the pre-Seldon baseline (dismissed)
      --no-capture           Do not run the first capture
      --theme-hook           Install Omarchy's theme-set hook (`omarchy hook install theme-set`)
      --remove-theme-hook    Remove the theme-set hook that --theme-hook installed, and nothing else; needs no logbook
      --git                  Make the logbook a git repository with a first commit (default)
      --no-git               Do not use git

Examples:
  seldon init
  seldon init --non-interactive --since 2026-09-01 --baseline
  seldon init --remove-theme-hook
```
<!-- /help -->

### seldon doctor

Checks the engine, the config, the logbook (cases, ledger, generated
fences), the collectors' last capture and state files, Omarchy, snapper
and git. It only reads. Each line says `ok`, `degraded` or `error`, and a
broken check prints the command that fixes it. Exit 0 when nothing is an
error, 1 when a check is an error (also when `config.toml` cannot be read
or parsed), 3 when the logbook is not initialised.

<!-- help: seldon doctor -->
```text
Check engine, config, logbook, collector state, omarchy, snapper and git

Usage: seldon doctor [OPTIONS]

Options:
      --path <DIR>     Logbook to check (same as the global --logbook)
```
<!-- /help -->

### seldon contract-version

Prints the version of the index format that the engine writes and the
plugin reads. The plugin compares it with its own.

<!-- help: seldon contract-version -->
```text
Print the engine/plugin contract version

Usage: seldon contract-version [OPTIONS]

Options:
```
<!-- /help -->

## Record

### seldon capture

Runs the collectors and appends new events to the ledger, then rebuilds
the index. Without options it runs every collector that `config.toml`
enables; `--source` runs exactly the named ones, even disabled ones.
A collector that cannot read its source reports `degraded` with a fix
and the capture still exits 0. `--since` only affects a collector that
has never run.

<!-- help: seldon capture -->
```text
Run collectors and append new events to the ledger

Usage: seldon capture [OPTIONS]

Options:
      --source <NAMES>  Collectors to run, comma-separated (default: every enabled one)
      --all             Run every enabled collector (the default)
      --since <TS>      Baseline for collectors without a cursor, an RFC 3339 time (default: the logbook's creation)

Examples:
  seldon capture
  seldon capture --source pacman,config
  seldon capture --since 2026-09-01T00:00:00+02:00
```
<!-- /help -->

### seldon log

Writes a note: a `manual` event in the ledger and an entry under today's
date in the journal. `--case` files it under a case; `--tag` adds `#tag`
to the journal line.

<!-- help: seldon log -->
```text
Write a note: a ledger event and a journal entry

Usage: seldon log [OPTIONS] <TEXT>

Arguments:
  <TEXT>  The note, as one argument; after `--` when it starts with `-`

Options:
      --case <ID>      The case the note belongs to
      --actor <ACTOR>  Who writes the note: human or agent:NAME [default: human]
      --tag <TAG>      Tag the note (repeatable): `#tag` in the journal, `meta.tags` in the ledger

Examples:
  seldon log -- "Switched the terminal font to Iosevka"
  seldon log --case C-2026-004 --tag fonts -- "Tried two fonts, kept the first"
```
<!-- /help -->

### seldon event

Records an event by hand, for a change that no collector or hook sees,
such as a setting changed in a GUI. Hooks and scripts use it too. The
source and kind must be ones the ledger knows; the error lists them.

<!-- help: seldon event -->
```text
Record an event by hand (hooks, scripts)

Usage: seldon event [OPTIONS] --subject <SUBJECT> <SOURCE> <KIND>

Arguments:
  <SOURCE>  Event source (pacman, snapper, omarchy, plugins, theme, config, agent, manual)
  <KIND>    Event kind (install, theme-set, config-change, command, note, …)

Options:
      --subject <SUBJECT>  What it is about: package, ~-relative path, theme, plugin id, …
      --detail <TEXT>      Human-readable detail
      --case <ID>          Attribute the event to this case
      --actor <ACTOR>      Who did it: system (like a collector), human or agent:NAME; hooks and scripts name the one they act for [default: system]
      --meta <KEY=VALUE>   Extra key=value (repeatable), e.g. `--meta enabled=true`; `enabled` takes true or false
```
<!-- /help -->

### seldon status

Rebuilds `STATUS.md`, the monthly ledger views and the index, then prints
a summary. It commits only when a logbook file changed.

<!-- help: seldon status -->
```text
Regenerate STATUS.md, the ledger views and index.json; print a summary

Usage: seldon status [OPTIONS]

Options:
```
<!-- /help -->

### seldon index

Rebuilds the index and the ledger views without `STATUS.md` and without
a commit. `--check` refuses to write an index that does not match the
format (exit 2).

<!-- help: seldon index -->
```text
Rebuild index.json and the ledger/*.md views; --check validates

Usage: seldon index [OPTIONS]

Options:
      --check          Validate the index against schema/index.schema.json before writing it
```
<!-- /help -->

### seldon watch

Watches the logbook and rebuilds the index two seconds after the last
change, so the plugin shows edits from your editor or Obsidian at once.
It only rebuilds the index: no capture, no `STATUS.md`, no commit. Stop
it with Ctrl-C. The release binary has it; a build without the `watch`
feature exits 1. [Update and uninstall](11-update-and-uninstall.md#the-optional-watcher)
shows how to run it as a user service.

<!-- help: seldon watch -->
```text
Rebuild index.json when the logbook changes (feature "watch")

Usage: seldon watch [OPTIONS]

Options:
      --interval <SECS>  Quiet time before the index is rebuilt, in seconds (at least 2) [default: 2]
```
<!-- /help -->

## Cases and decisions

### seldon plan

The case commands. Each step is a ledger event, a line in the case's
*Log* and a commit; the engine moves the file between `work/queued/`,
`work/active/` and `work/completed/`.

<!-- help: seldon plan -->
```text
Plan and track cases: new, start, verify, done, drop, list, show

Usage: seldon plan [OPTIONS] <COMMAND>

Commands:
  new     Create a case in work/queued/
  start   Start a case: queued → active; it becomes the active case
  verify  Hand an active case to verification: active → verification
  done    Complete a verified case: verification → completed
  drop    Drop a case that is queued, active or in verification
  list    List cases, optionally by status or area
  show    Print one case file with its path
  help    Print this message or the help of the given subcommand(s)

Options:
```
<!-- /help -->

### seldon plan new

Creates a case in `work/queued/` from the template. Defaults: zone
yellow, risk R1, priority normal. A new area gets its folder under
`areas/`.

<!-- help: seldon plan new -->
```text
Create a case in work/queued/

Usage: seldon plan new [OPTIONS] <TITLE>

Arguments:
  <TITLE>  The case title, as one argument (after `--` when it starts with `-`)

Options:
      --zone <ZONE>          green, yellow or red [default: yellow]
      --risk <RISK>          R0 to R3 [default: R1]
      --area <AREA>          Area slug; created under areas/ on first use
      --priority <PRIORITY>  high, normal or low [default: normal]
      --actor <ACTOR>        Who creates the case: human or agent:NAME [default: human]
```
<!-- /help -->

### seldon plan start

Makes a queued case active and the active case. For a red case, take a
snapshot first and pass its number with `--snapshot`.

<!-- help: seldon plan start -->
```text
Start a case: queued → active; it becomes the active case

Usage: seldon plan start [OPTIONS] <ID>

Arguments:
  <ID>  The case id, e.g. C-2026-004

Options:
      --reason <TEXT>      Why, in one line; goes into the Log line and the event detail
      --actor <ACTOR>      Who takes the step: human or agent:NAME [default: human]
      --snapshot <NUMBER>  Snapper snapshot number taken before the work, e.g. 42 (an R2 or R3 case started without one gets a warning, ADR-0023)
```
<!-- /help -->

### seldon plan verify

Moves an active case to verification: the work is done and waits for
your check.

<!-- help: seldon plan verify -->
```text
Hand an active case to verification: active → verification

Usage: seldon plan verify [OPTIONS] <ID>

Arguments:
  <ID>  The case id, e.g. C-2026-004

Options:
      --reason <TEXT>  Why, in one line; goes into the Log line and the event detail
      --actor <ACTOR>  Who takes the step: human or agent:NAME [default: human]
```
<!-- /help -->

### seldon plan done

Closes a case in verification. It writes a journal line and clears the
active case if it named this case.

<!-- help: seldon plan done -->
```text
Complete a verified case: verification → completed

Usage: seldon plan done [OPTIONS] <ID>

Arguments:
  <ID>  The case id, e.g. C-2026-004

Options:
      --reason <TEXT>  Why, in one line; goes into the Log line and the event detail
      --actor <ACTOR>  Who takes the step: human or agent:NAME [default: human]
```
<!-- /help -->

### seldon plan drop

Gives up a queued, active or verification case. Say why with `--reason`.

<!-- help: seldon plan drop -->
```text
Drop a case that is queued, active or in verification

Usage: seldon plan drop [OPTIONS] <ID>

Arguments:
  <ID>  The case id, e.g. C-2026-004

Options:
      --reason <TEXT>  Why, in one line; goes into the Log line and the event detail
      --actor <ACTOR>  Who takes the step: human or agent:NAME [default: human]
```
<!-- /help -->

### seldon plan list

Lists cases: id, status, zone, risk, area and title. `--status` takes
`queued`, `active`, `verification`, `completed` or `dropped`.

<!-- help: seldon plan list -->
```text
List cases, optionally by status or area

Usage: seldon plan list [OPTIONS]

Options:
      --status <STATUS>  Only cases with this status (queued, active, verification, completed, dropped)
      --area <AREA>      Only cases in this area
```
<!-- /help -->

### seldon plan show

Prints a case's path and file, including its events.

<!-- help: seldon plan show -->
```text
Print one case file with its path

Usage: seldon plan show [OPTIONS] <ID>

Arguments:
  <ID>  The case id, e.g. C-2026-004

Options:
```
<!-- /help -->

### seldon decide

Creates `decisions/ADR-NNNN-<slug>.md` with the status *proposed* and
opens it in your editor; `--no-edit` skips the editor. It writes no
ledger event.

<!-- help: seldon decide -->
```text
Create a decision record (ADR) and open it in the editor

Usage: seldon decide [OPTIONS] <TITLE>

Arguments:
  <TITLE>  The decision title, as one argument

Options:
      --case <ID>      The case this decision belongs to
      --no-edit        Do not open the editor
```
<!-- /help -->

### seldon open

Prints the path of a logbook file. `case` is the active case, `journal`
today's journal, `ledger` this month's ledger view, `status` the
`STATUS.md`, `logbook` the folder; a case or decision id names that file.
`--editor` opens it: in a terminal with `$VISUAL` or `$EDITOR`, from the
plugin with Omarchy's editor launcher.

<!-- help: seldon open -->
```text
Print the path of a logbook file; --editor opens it

Usage: seldon open [OPTIONS] <WHAT>

Arguments:
  <WHAT>  case (the active one), journal (today), ledger (this month), status, logbook, or a case or decision id

Options:
      --editor         Open it in the editor
```
<!-- /help -->

## Drift

### seldon drift

Lists open drift, crises first: zone, time, source and kind, subject and
event id. It only reads. The totals count every open item, also when the
list is cut.

<!-- help: seldon drift -->
```text
List open drift; link, explain, dismiss or show a drift event

Usage: seldon drift [OPTIONS]
       seldon drift <COMMAND>

Commands:
  link     Link a drift event, and the open members of its group, to a case
  explain  Explain a drift event with a new retroactive, completed case
  dismiss  Dismiss a drift event with a reason
  show     Show a drift event and every open member of its group
  help     Print this message or the help of the given subcommand(s)

Options:
      --crisis-only    Only crises (red-zone items)
```
<!-- /help -->

### seldon drift link

Links the event, and every open event of its group, to a case. The case
may be open or closed. Then the events are no longer drift.

<!-- help: seldon drift link -->
```text
Link a drift event, and the open members of its group, to a case

Usage: seldon drift link [OPTIONS] <EVENT> <CASE>

Arguments:
  <EVENT>  The drift event id, as `seldon drift` prints it
  <CASE>   The case id, e.g. C-2026-004 (a completed or dropped case too)

Options:
      --only           Resolve the named event only, not the rest of its group
      --actor <ACTOR>  Who resolves it: human or agent:NAME [default: human]
```
<!-- /help -->

### seldon drift explain

Creates a completed case with your text as its title and links the event
and its open group to it. The case gets the drift item's zone unless you
pass `--zone`.

<!-- help: seldon drift explain -->
```text
Explain a drift event with a new retroactive, completed case

Usage: seldon drift explain [OPTIONS] <EVENT> <INTENT>

Arguments:
  <EVENT>   The drift event id, as `seldon drift` prints it
  <INTENT>  Why it happened, as one argument after `--`; the new case's title

Options:
      --only           Resolve the named event only, not the rest of its group
      --zone <ZONE>    Zone of the new case (default: the drift item's zone)
      --risk <RISK>    Risk of the new case [default: R1]
      --area <AREA>    Area slug of the new case; created under areas/ on first use
      --actor <ACTOR>  Who resolves it: human or agent:NAME [default: human]

Example:
  seldon drift explain <EVENT> --area hardware -- "Driver for the new GPU"
```
<!-- /help -->

### seldon drift dismiss

Marks the event and its open group as not needing a case, with your
reason.

<!-- help: seldon drift dismiss -->
```text
Dismiss a drift event with a reason

Usage: seldon drift dismiss [OPTIONS] <EVENT> <REASON>

Arguments:
  <EVENT>   The drift event id, as `seldon drift` prints it
  <REASON>  Why it can be ignored, as one argument after `--`

Options:
      --only           Resolve the named event only, not the rest of its group
      --actor <ACTOR>  Who resolves it: human or agent:NAME [default: human]

Example:
  seldon drift dismiss <EVENT> -- "Tried a theme, reverted it"
```
<!-- /help -->

### seldon drift show

Shows one drift event with every open member of its group. Use it before
you resolve a large group.

<!-- help: seldon drift show -->
```text
Show a drift event and every open member of its group

Usage: seldon drift show [OPTIONS] <EVENT>

Arguments:
  <EVENT>  The drift event id, as `seldon drift` prints it

Options:
```
<!-- /help -->

All three resolving commands write one resolution per open member of the
group in one step. `--only` resolves the named event alone. Running the
same command again writes nothing and exits 0.

## Agents and hooks

### seldon agent

<!-- help: seldon agent -->
```text
Start an agent on an active case

Usage: seldon agent [OPTIONS] <COMMAND>

Commands:
  start  Launch an agent on an active case, with the case as the active case and a prompt that names the case and the logbook
  help   Print this message or the help of the given subcommand(s)

Options:
```
<!-- /help -->

### seldon agent start

Makes the case the active case and starts an agent in the logbook folder.
The first prompt names the case and the logbook and tells the agent to
run `seldon hook session-start` and `seldon plan show <ID>`; it holds no
logbook text. The case must be active.
The launcher comes from `config.toml`; see
[Configuration](06-configuration.md#agent-launcher).

<!-- help: seldon agent start -->
```text
Launch an agent on an active case, with the case as the active case and a prompt that names the case and the logbook

Usage: seldon agent start [OPTIONS] <ID>

Arguments:
  <ID>  The case (must be active)

Options:
      --launcher <NAME>  A launcher from `[agent.launchers]` in config.toml; `omarchy` is the built-in one (default: `[agent] launcher`)
```
<!-- /help -->

### seldon hook

The commands that agents and their harnesses call. Hooks are silent and
always exit 0, so they never break an agent. `install` and `uninstall`
are the exceptions: you run them yourself, and they report like any
other command.

<!-- help: seldon hook -->
```text
Agent hooks: record commands, print session context, install into or uninstall from a harness

Usage: seldon hook [OPTIONS] <COMMAND>

Commands:
  install        Merge Seldon's hooks into an agent harness's settings
  uninstall      Remove Seldon's hooks from an agent harness's settings, keeping the rest
  claude-code    Record a Claude Code tool call (hook payload on stdin; silent, exit 0)
  generic        Record any agent's command ({"command","actor","cwd","startedAt"?,"case"?} on stdin)
  session-start  Print the context block an agent session starts with
  session-stop   End a session: journal stub, capture, commit (silent, exit 0)
  help           Print this message or the help of the given subcommand(s)

Options:
```
<!-- /help -->

### seldon hook install

Merges Seldon's three hooks into Claude Code's settings, by default the
logbook's `.claude/settings.json`. Hooks that are already there stay.
Running it again changes nothing.

<!-- help: seldon hook install -->
```text
Merge Seldon's hooks into an agent harness's settings

Usage: seldon hook install [OPTIONS] <HARNESS>

Arguments:
  <HARNESS>  The harness [possible values: claude-code]

Options:
      --settings <FILE>  Settings file (default: <logbook>/.claude/settings.json)
```
<!-- /help -->

### seldon hook uninstall

Takes Seldon's three hooks out of Claude Code's settings again and keeps
everything else, also a hook you added next to one of Seldon's. A file
that held nothing but Seldon's hooks is deleted. Running it again changes
nothing. The next `seldon capture` does not report the change as drift.

<!-- help: seldon hook uninstall -->
```text
Remove Seldon's hooks from an agent harness's settings, keeping the rest

Usage: seldon hook uninstall [OPTIONS] <HARNESS>

Arguments:
  <HARNESS>  The harness [possible values: claude-code]

Options:
      --settings <FILE>  Settings file (default: <logbook>/.claude/settings.json)
```
<!-- /help -->

### seldon hook claude-code

Claude Code's `PreToolUse` hook calls this with the tool call as JSON on
standard input. You do not run it yourself.

<!-- help: seldon hook claude-code -->
```text
Record a Claude Code tool call (hook payload on stdin; silent, exit 0)

Usage: seldon hook claude-code [OPTIONS]

Options:
```
<!-- /help -->

### seldon hook generic

The same for any other agent:
`{"command": "…", "actor": "agent:<name>", "cwd": "…"}` on standard
input. See [Working with agents](04-working-with-agents.md#other-agents).

<!-- help: seldon hook generic -->
```text
Record any agent's command ({"command","actor","cwd","startedAt"?,"case"?} on stdin)

Usage: seldon hook generic [OPTIONS]

Options:
      --case <ID>      The case the command belongs to (default: `.seldon/active-case`)
```
<!-- /help -->

### seldon hook session-start

Prints the context an agent session starts with: status, the active case
and its plan, the last journal lines and the lesson headings.

<!-- help: seldon hook session-start -->
```text
Print the context block an agent session starts with

Usage: seldon hook session-start [OPTIONS]

Options:
```
<!-- /help -->

### seldon hook session-stop

Ends a session: a journal line with the number of events the session
recorded, a capture, `STATUS.md`, the index and a commit.

<!-- help: seldon hook session-stop -->
```text
End a session: journal stub, capture, commit (silent, exit 0)

Usage: seldon hook session-stop [OPTIONS]

Options:
      --actor <ACTOR>  Who ends the session: human or agent:NAME [default: agent:claude-code]
```
<!-- /help -->

## Outputs and import

### seldon rebuild

Writes `outputs/REBUILD.md`, the steps that take a fresh Omarchy install
to this machine. See [Rebuild, dossier and update impact](08-rebuild-dossier-update-impact.md).

<!-- help: seldon rebuild -->
```text
Write outputs/REBUILD.md: the steps to rebuild this machine

Usage: seldon rebuild [OPTIONS]

Options:
```
<!-- /help -->

### seldon dossier

Refreshes the generated parts of `system/*.md` from read-only queries.
Text outside the generated parts is never changed. It writes no ledger
event.

<!-- help: seldon dossier -->
```text
Refresh the generated fences of system/*.md from read-only queries

Usage: seldon dossier [OPTIONS]

Options:
      --section <SECTION>  Sections to write, comma-separated or repeated (default: all) [possible values: packages, services, omarchy, hardware, plugins, deviations, all]
```
<!-- /help -->

### seldon import

<!-- help: seldon import -->
```text
Import an earlier logbook (dry run unless --apply)

Usage: seldon import [OPTIONS] <COMMAND>

Commands:
  omarchy-agent  Import the omarchy-agent kit's Obsidian vault, which is only read (dry run unless --apply)
  help           Print this message or the help of the given subcommand(s)

Options:
```
<!-- /help -->

### seldon import omarchy-agent

Reads a vault of the omarchy-agent kit and writes a report. With
`--apply` it imports cases, journal, memory and deviations in one commit.
See [Import from omarchy-agent](09-import-from-omarchy-agent.md).

<!-- help: seldon import omarchy-agent -->
```text
Import the omarchy-agent kit's Obsidian vault, which is only read (dry run unless --apply)

Usage: seldon import omarchy-agent [OPTIONS] <VAULT>

Arguments:
  <VAULT>  The vault directory (the one with pipeline/, journal/, knowledge/)

Options:
      --dry-run        Only write the report outputs/IMPORT-omarchy-agent.md (the default)
      --apply          Import: cases, journal, memory and deviation rows, in one commit
```
<!-- /help -->

## Shell completions and man page

The package and the installer put both in place for you. These commands
print them, for a self-built engine or another place.

### seldon completions

Prints the completion script for bash, zsh or fish. After you save it
where your shell looks, Tab completes commands, options and their values.

<!-- help: seldon completions -->
```text
Print a shell completion script for bash, zsh or fish

Usage: seldon completions [OPTIONS] <SHELL>

Arguments:
  <SHELL>  The shell [possible values: bash, zsh, fish]

Options:

Examples:
  seldon completions bash > ~/.local/share/bash-completion/completions/seldon
  seldon completions zsh > ~/.local/share/zsh/site-functions/_seldon
  seldon completions fish > ~/.config/fish/completions/seldon.fish
```
<!-- /help -->

### seldon mangen

Prints the man page seldon(1): every command with its options, the exit
codes, the environment variables and the files.

<!-- help: seldon mangen -->
```text
Print the man page seldon(1), generated from this help

Usage: seldon mangen [OPTIONS]

Options:

Example:
  seldon mangen > seldon.1 && man -l seldon.1
```
<!-- /help -->

---

Previous: [Working with agents](04-working-with-agents.md) · [Index](README.md) · Next: [Configuration](06-configuration.md)
