# ADR-0041 — One agent per case: a session is a window; `agent focus`, `agent sessions`, `--again`, and `open --editor` focuses its window

**Status:** proposed (WP-156, after the operator's live use on 2026-10-07,
case C-2026-005: three agents and three editors on one case, all behind
the desk; the orchestrator's round-2 decision of the same day: "a session
is a window, not a process")
**Date:** 2026-10-07

> Adds rows to CONTRACT.md's "Commands the plugin may run" (`agent focus
> <caseId> --json`, `agent sessions --json`) and a shape to `open --editor
> --json` (`focused`), as ADR-0040 (WP-135) did for `decide accept`: a new
> row in CONTRACT.md's "Commands the plugin may run" needs an ADR. The
> index's shape does not change: `contractVersion` stays
> 2, no schema or fixture changes. Builds on ADR-0030 §1 (the launch
> marker `SELDON_CASE`), ADR-0034 §1–§3 (the desk) and WP-022/WP-101
> (`agent start`). Adds one read of `/proc` the engine did not do before
> (§3); the operator is asked to acknowledge it with the acceptance.

## Context

`agent start <ID>` launched an agent every time it was asked. The desk is
a full-screen overlay, so every window it opened appeared under it; the
operator saw nothing and clicked again. Three agents waited for the same
R3 go in terminals nobody could see.

To refuse a second agent, the engine has to know that the first is still
there. Omarchy's launchers do not say: `omarchy agent prompt` →
`omarchy-agent` → `omarchy-launch-tui --app-id=org.omarchy.agent` → `exec
setsid uwsm-app -- xdg-terminal-exec …`; `setsid` forks (the engine starts
the launcher as a process-group leader), so the pid the engine holds exits
at once and the terminal is a pid it never saw. Every agent window shares
the class `org.omarchy.agent`, and the title is the agent's own. What
survives the whole chain is the environment the engine set: the terminal
and the agent in it carry `SELDON_CASE` and `SELDON_LOGBOOK` (ADR-0030 §1).

Round 1 of WP-156 called "any process of the user that carries the marker"
a session. The review found the flaw: a process the agent leaves behind —
a daemon, an ssh master, a gpg-agent — keeps the marker after the window
is closed, locks the case, and *Focus* fails for ever; and the engine read
the environment of every process the user owns.

## Decision

1. **A session is a window.** It is a Hyprland window whose class is an
   Omarchy launcher class and whose process, or one of its descendants,
   carries the marker:
   - an agent: class `org.omarchy.agent`, and the marker
     `SELDON_CASE=<ID>` with `SELDON_LOGBOOK=<this logbook's root>` (a
     case id is unique per logbook only);
   - an editor `open --editor` started: a terminal class `org.omarchy.*`
     (not `org.omarchy.agent`), and `SELDON_OPEN=<path>`, which the
     engine now sets on the editor launcher. A GUI editor launches as
     before (it hands off to its own instance, and that is not
     `omarchy-launch-tui`'s window).

   The engine first asks `hyprctl clients -j` (read-only). **Without
   hyprctl, or when it fails, nothing is tracked**: `agent start` never
   refuses, `agent sessions` says `tracking: false`, and `open --editor`
   launches as before. The plugin's busy state and its 2 s floor still
   hold.
2. **The launch grace.** A terminal takes a moment to map. A launch of a
   case counts as a session for 10 s while no window of it is found. The
   record is `<state>/launches.json`: `[{case, logbook, at}]`, written
   under the lock after a successful launch; records older than 10 s are
   dropped on the next write. It holds ids and a path, nothing else, and
   is read only while tracking is on. The file is 0600 in the 0700 state
   directory; a record that does not parse, or lies in the future, counts
   as no launch (fail open).
3. **The `/proc` read is narrowed to those windows.** For each window of a
   matching class, the engine walks the window's process and its
   descendants (`/proc/<pid>/task/<tid>/children`, at most 256
   processes). It reads each one's `/proc/<pid>/environ`, at most 64 KiB.
   The whole block is read into memory and discarded after the
   comparison; it may hold the agent's credentials. This covers every
   window of the class, including agents Seldon did not start. The read
   follows these rules:
   - the kernel serves the user's own processes only; others fail and are
     skipped;
   - only `SELDON_CASE`, `SELDON_LOGBOOK`, `SELDON_OPEN` are compared,
     and `SELDON_ACTOR` is read; nothing else of any environment is kept,
     printed or logged;
   - the engine's own process is skipped;
   - "oldest" is by start time (`/proc/<pid>/stat` field 22), not by pid;
   - an actor that does not read as `agent:<slug>` is not reported.

   The engine never reads the whole process table.
4. **`agent start <ID>` refuses** (exit 1, nothing launched, the active
   case untouched; checked under the lock after the status check) while
   a session on `<ID>` is open. The message names the window and
   workspace, or "started N s ago and its window is not open yet", then:
   "focus it with `seldon agent focus <ID>`, or start another with
   `seldon agent start <ID> --again`; nothing was launched".
   - **`--again`** (not with `--new`) skips the check.
   - `--new` is never refused: its case is new. It still records its
     launch.
   - `agent ask` sets no `SELDON_CASE` and is never a session.
5. **`seldon agent focus <ID> [--json]`** brings the session's window to
   the front, the way `omarchy-launch-or-focus` does it:
   - first `hyprctl dispatch 'hl.dsp.focus({ window = "address:<A>" })'`
     (Hyprland's Lua dispatcher), then `hyprctl dispatch focuswindow
     address:<A>`;
   - a dispatch counts when hyprctl prints `ok`;
   - the address must be `0x` and 1–16 hex digits before it enters the
     Lua expression.

   Answers:
   - a window: `{focused: true, case, address, workspace, pid, pids,
     actor}`;
   - a session within the grace: `{focused: false, starting: true, case}`,
     exit 0 (its window comes up when it maps);
   - exit 1 without tracking (naming why), with no session ("no agent is
     working on <ID>: no window of an agent `seldon agent start` launched
     on it is open; start one with …"), or when the dispatch is refused.
6. **`seldon agent sessions [--json]`** is read-only and takes no lock:
   `{tracking, sessions: [{case, starting, window: {address, workspace,
   pid} | null, pids, actor}]}`, by case id.
7. **`open --editor`** (no terminal, the plugin's path): when a window of
   the same path is open (§1), the engine focuses it and launches
   nothing: `editor: {launched: false, focused: true, address, pid,
   program}`. If the focus fails, it launches as before. The terminal
   path (`$VISUAL`/`$EDITOR` attached) is unchanged.
8. **The desk** (SPEC-PLUGIN §5, §5.4):
   - it closes through the facade after the engine's successful answer to
     a call that opened or focused a window; a refusal keeps it open with
     the text;
   - it asks `agent sessions` in its own process when it opens, after
     every agent or open answer, when the index changes, and every 15 s
     while it is open;
   - an active case with a session shows *Focus* in place of *Hand to
     agent*;
   - the plugin never sends `--again`.

## Consequences

- One click opens one agent, and the desk is out of the way. A second
  click brings the first agent back instead of starting another.
- A daemon the agent leaves running no longer locks the case. Closing the
  agent's window frees it, after at most the grace.
- Two new plugin commands and one new answer shape; no index change.
- The engine depends on `hyprctl` for this feature only. On another
  compositor, or over ssh without the Hyprland socket, the feature is off
  and nothing else changes.
- Known limits:
  - a terminal server that runs every window in one process (one pid,
    many windows) could name the wrong window: the descendant walk would
    find the marker under the shared pid. Omarchy's `xdg-terminal-exec`
    starts one process per window, and ADR-0030's live check found no
    terminal server.
  - an environment above 64 KiB whose markers sit past the cap is not
    seen; the session is then not tracked (fail open: a second agent can
    start).
  - a process of the same user can plant the marker under a window of
    class `org.omarchy.agent` and hold the case while that window is open;
    closing it or `--again` ends it. Same-user processes are inside the
    trust boundary (ADR-0030).

## Alternatives considered

- **Any marked process is a session** (round 1). Rejected: orphans lock
  the case, and it reads every process's environment.
- **The launcher's pid in a state file.** Rejected: that pid exits at
  once (`setsid` forks).
- **The window's app-id and title.** Rejected: the app-id is shared by
  every agent, and the title is the agent's.
- **Focus in QML** (`Hyprland.dispatch`). Rejected: the CLI refusal could
  not name a command, and the plugin would decide a session from data it
  must not read (`/proc`).
- **A marker file per launch, removed at session end.** Rejected: nothing
  removes it when the terminal is killed, so it goes stale exactly when
  it matters.
