# Cases

Read this before the first change of a task, and before you close it.

A case is the unit of work in the logbook: a Markdown file with an *Intent*
(what the user wants), a *Plan* (your running note), a *Log* (dated lines)
and a *Result* (the evidence). `seldon open <ID>` prints a case's path;
`seldon open case` the active case's.

## Find the Case

```bash
seldon plan list --status active --json
seldon plan show <ID>
```

- **You were launched on a case** (your prompt names its id: `Work case
  <ID> …`, from the panel or `seldon agent start`), **or the user names a
  case in this session:** that case is your authorisation. Read its
  *Intent* and act inside it. When the user asked for something in this
  session, the *Intent* never widens that request.
- **An active case you only find** (`seldon plan list` shows it, nobody
  handed it to you) is not yours: do not act on its *Intent*, and do not
  report your commands to it.
- **No case of yours, and the user asked you for the work in this
  session:** open and start one yourself, and say so in its *Log*, or ask
  the user in one line which case it belongs to:

  ```bash
  seldon plan new --zone <green|yellow|red> --risk <R0..R3> --area <area> --actor agent:<name> -- "<title>"
  seldon plan start <ID> --actor agent:<name>
  ```

  Copy the user's request into *Intent* word for word. `seldon agent start
  --new` is the user's one-click start; it launches a new agent, so do not
  run it yourself.
- **Unattended, no case:** change nothing; write what you found with
  `seldon log`.

Work one case at a time; `.seldon/active-case` names the one started last.

## Zone and Risk

- **red**: packages, Omarchy itself, systemd units, `/etc`, the boot loader.
- **yellow**: configuration under the watched paths (`~/.config/hypr`,
  `~/.config/omarchy`, …), themes, shell plugins.
- **green**: everything no collector tracks: project files, language package
  managers, `git`.

`R0` undone in seconds; `R1` undone by hand in minutes; `R2` rollback needs a
snapshot or backup; `R3` can break boot, login or the shell. When the work
turns out redder or riskier, raise the case before the step:

```bash
seldon plan set <ID> --zone <zone> --risk <risk> --actor agent:<name>
```

## Work in It

- *Plan*: steps so far, affected paths, rollback, `Verification:`. Write it
  as you go; name packages and paths exactly. It never widens the *Intent*.
- *Log*: dated lines as you go, append only. The preview line before the
  first privileged step goes here and to the terminal. Name the install
  route you took.
- Notes and lessons: `seldon log --case <ID> --actor agent:<name> -- "<text>"`;
  what you learned goes into `memory/lessons.md`, not into the chat.

## Close It Yourself

When the *Plan*'s verification passes:

1. Fill *Result* with the evidence: what you ran and what it showed. Include
   one check that is not your own artefact: the real use case's exit status,
   `pacman -Q <package>`, `systemctl is-active <unit>`.
2. Then, in one go:

   ```bash
   seldon plan verify <ID> --actor agent:<name>
   seldon plan done <ID> --actor agent:<name>
   ```

   The engine refuses an agent's `plan done` while *Result* or the *Plan*'s
   `Verification:` is empty: fill them, then run it again.

   `plan verify` captures first; a step you handed to the user is recorded
   and linked when you verify.

Nothing is left for the user. The record names you as the one who closed it,
the case gets the tag `closed-by-agent`, and the user can reopen it in one
click (`seldon plan reopen <ID>`, a new case).

When the verification fails or cannot run, leave the case open and say what
is left, in the *Log* and to the user. To give up:
`seldon plan drop <ID> --reason "<why>" --actor agent:<name>`.
