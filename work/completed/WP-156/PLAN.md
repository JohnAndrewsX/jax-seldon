# WP-156 — Plan: the desk steps aside for what it opens; one agent per case

Branch `wp/156-desk-steps-aside` from `next` (d747daa); merge into `next`.

## What the live use showed, and why

C-2026-005 on the test host: three agents and three nvim windows on one
case, all behind the desk. Read against the code:

- The desk (`Desk.qml`) never hides after an action; the windows it starts
  open under a layer-shell overlay with exclusive keyboard focus.
- `Service.startAgent` already has a one-at-a-time rule (`planResult.pending`),
  so the three agents were three *sequential* launches (17:39:15 Run,
  17:39:40 and 17:42:01 Hand to agent): nothing knew a session was alive.
- `Service.openInEditor` has no guard at all: three clicks, three editors.

## What Omarchy's launchers can tell us (read: `$OMARCHY_PATH/bin`)

- `omarchy agent prompt` → `omarchy-agent` → `omarchy-launch-tui
  --app-id=org.omarchy.agent …` → `exec setsid uwsm-app -- xdg-terminal-exec
  …`. `uwsm-app` asks its daemon for a command line and `eval`s it in its
  own process (a `systemd-run --scope`), which execs the terminal.
- `omarchy-launch-editor` → `omarchy-launch-tui nvim <path>` (terminal
  editors) or `setsid uwsm-app -- <editor>` (GUI editors).
- **No launcher prints a window address or a pid.** And the pid
  `launch_detached` holds is useless: the engine starts the launcher as a
  process-group leader (`process_group(0)`), so `setsid` forks and its
  parent exits at once; the terminal ends up as an unrelated pid.
- What *does* survive the whole chain is the **environment**: every
  process from the launcher to the terminal and the agent inside it
  carries `SELDON_CASE`, `SELDON_LOGBOOK`, `SELDON_ACTOR` (ADR-0030 §1;
  WP-096's live check: not through a terminal server). The terminal
  process is the window's pid in `hyprctl clients -j`.
- The app-id `org.omarchy.agent` is shared by every agent window, and the
  title is the agent's own (Claude Code sets it), so "app-id and title"
  cannot name a case.
- Omarchy's menus hide *before* the launch (`Menu.qml`: `opened = false`,
  then `appLibrary.launch`; `agents/Panel.qml`: `bar.run(…)`, then
  `close()`). Focus of an existing window: `omarchy-launch-or-focus`
  dispatches `hl.dsp.focus({ window = "address:…" })` and falls back to
  `focuswindow address:…` (Hyprland 0.56 here takes only the Lua form:
  the legacy form exits 7; a missing window prints `warning: … window not
  found` with exit 0).

## Design

### Engine (small part)

1. **Sessions = the marker in `/proc`.** `agent::sessions(logbook)` reads
   `/proc/<pid>/environ` of every process it may read (the user's own;
   others fail with EACCES and are skipped) and keeps those with
   `SELDON_CASE=<id>` **and** `SELDON_LOGBOOK=<this logbook's root>`
   (a case id is unique only per logbook; this also keeps parallel tests
   apart), skipping the engine's own pid. Only those three keys are
   looked at (`SELDON_ACTOR` for the report); nothing else of any
   environment is kept. No state file: the marker dies with the session,
   so nothing goes stale, and there is no gap between the launch and the
   terminal (the chain carries it from the first fork).
2. **`agent start <ID>` refuses** with exit 1 while a session on `<ID>`
   lives: "an agent is already working on C-… (pid N); focus it with
   `seldon agent focus C-…`, or start another with `seldon agent start
   C-… --again`". Checked under the lock, before the active case is
   touched. **`--again`** (conflicts with `--new`) skips the check.
   `--new` never checks (a new case has no session).
3. **`seldon agent focus <ID> [--json]`** — finds the session, then its
   window: `hyprctl clients -j` (read), the client whose pid is a marker
   pid, else the nearest ancestor of one (a terminal that runs the agent
   as a child); the address checked as `0x` + hex before it goes into
   `hyprctl dispatch hl.dsp.focus({ window = "address:…" })`, falling back
   to `focuswindow address:…` (Omarchy's order). Exit 1 with a fix when
   there is no session ("start one with `seldon agent start …`"), no
   Hyprland, or no window. JSON: `{focused, case, pid, address,
   workspace}`.
4. **`seldon agent sessions [--json]`** — read-only, no lock: the live
   sessions of this logbook, `[{case, pids, actor}]`, one per case. The
   desk asks it; the CLI user sees the same.
5. **`open --editor` focuses an editor it already opened.** On the
   no-terminal path (the plugin's), the engine sets `SELDON_OPEN=<path>`
   in the editor launcher's environment. Before a launch it looks for a
   live process with `SELDON_OPEN=<that path>`: found with a window →
   focus it (`editor: {launched: false, focused: true, …}`), found
   without one (still starting) → nothing launched, `{launched: false,
   running: true}` and exit 0; none → launch as today. The terminal path
   (`$VISUAL`/`$EDITOR` attached) is unchanged.

### Plugin

6. **The desk steps aside.** After a *successful* `agent start` (also
   `--new`), `agent focus`, `open --editor` (launched or focused), and on
   the notices' *Run in terminal* (a fire-and-forget `execDetached`, so at
   once, as Omarchy's own menus do), `Service.stepAside()` closes the
   desk through the facade (`Desk.dismiss`), if it is open. A failure
   keeps it open with the engine's text. The selection is remembered
   (`Desk.remember`), so the next open shows the case the agent works on.
   Focus: the desk passes nothing; Hyprland focuses the new window when
   it maps, or keeps it focused if it mapped behind the desk (the
   overlay held the keyboard, not the window focus) — checked live.
7. **One agent per case on the desk.** `Service.agentSessions` (from
   `agent sessions --json`, its own process beside the write queue, like
   `doctor`): asked when the desk opens, after every agent/open/focus
   answer, when the index changes while the desk is open, and every
   `Model.SESSIONS_POLL_MS` (15 s) while it is open. An active case with
   a session shows **Focus** (key `a`, runs at once, writes nothing) in
   place of *Hand to agent*, and *Agent working* in the bar's meta and as
   a key/value row; the Work list row says "agent working". A stale
   answer heals itself: the engine's refusal or "no agent is working"
   asks again.
8. **No double launch.** The agent family already refuses while pending;
   `openInEditor` gets the same rule (`openResult.pending`), plus "the
   same target within `Model.OPEN_REPEAT_MS` (2 s) of a successful open
   is not sent again". While pending, the pressed button is busy (label
   *Starting…*/*Opening…*, disabled), as Run and Capture are.
9. **validateArgs / CONTRACT.md**: `agent focus <caseId> --json` and
   `agent sessions --json` join the plugin's commands. `--again` is CLI
   only (the desk offers Focus, never a second agent).

## Decisions (what the WP leaves open)

- **D1 — tracking by environment marker, not pid or window title** (above:
  the launcher's pid exits at once; the app-id is shared; the title is the
  agent's). No state file.
- **D2 — the marker matches case *and* logbook.**
- **D3 — focus lives in the engine** (`agent focus`), not in QML: the CLI
  refusal can name a command, the plugin keeps "fixed argv to seldon" as
  its only way to act, and the harness tests it with a fake engine.
- **D4 — the desk hides after the engine's success, not before the call**
  (the WP's "after a successful launch"): a refusal ("already working",
  "no default agent") must stay readable. The terminal fix has no answer,
  so it hides at once.
- **D5 — the desk offers no `--again`.**
- **D6 — Focus runs at once** (no arm): it changes nothing.
- **D7 — the editor gets its own marker `SELDON_OPEN`**, so "open the same
  file twice" focuses the first window instead of a second nvim on the
  same swap file; the plugin's 2 s rule is the floor for editors the
  marker cannot follow (a GUI editor that hands off to a running
  instance and exits).
- **D8 — `agent ask` is covered by the same step-aside path** (any `agent`
  answer), ready for WP-124b; the plugin does not run `ask` yet.

## Acceptance → tests

- Engine (`engine/tests/agent.rs`): a stub launcher that stays alive
  (`exec sleep`) → second `agent start` exit 1 with the focus hint, the
  active case untouched, one launch; `--again` launches; `--new` is not
  refused; `--again --new` refused by clap; `agent sessions --json` lists
  the case, and nothing after the stub is killed (by its pid); another
  logbook's session with the same id is not counted; `agent focus` with a
  stub `hyprctl` (clients JSON with the stub's pid) dispatches the Lua
  form with the address, falls back to `focuswindow`, refuses a bad
  address, says "no agent" and "no window"; `open --editor` twice with a
  live stub editor → one launch, the second focused; unit tests for the
  environ parser and the client match.
- Harness (`desk-view.sh`): launched → desk hidden (Run, Hand to agent,
  Open in editor, Focus, Run in terminal); refused → desk stays with the
  text; an active case with a session → *Focus* in place of *Hand to
  agent*, `a` focuses; a stale *Hand to agent* refused → flips to Focus;
  double-click on Hand to agent / Open in editor → one call.
- `model.test.js`: validateArgs, the sessions parser, the bar actions with
  a session, the 2 s rule.
- Live check on the test host: one click shows the agent terminal in
  front; a second hand-off shows Focus and brings it back.
